"""Tests for the dedicated image/video media result contract (issue 188).

Covers the structured media record (safe URLs, thumbnails, source
attribution, dimensions/duration, media type), the media-intent routing
behavior, media capability visibility in the live catalog/scope preview, and
the no-media/unsupported path.
"""

from __future__ import annotations

import time
from typing import Any

import pytest
import yaml

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    build_media,
    discover_engines,
    media_from_dict,
    media_to_dict,
)
from slopsearx.capabilities import CapabilityCatalog, MCPPolicy, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.formatter import format_json, format_yaml_markdown
from slopsearx.mcp import resources as r
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.research import (
    ResearchJob,
    ResearchJobRunner,
    ResearchJobStore,
    ResearchQuery,
    generate_job_id,
)
from slopsearx.service import (
    AppContext,
    ScopeDecision,
    SearchResponse,
    SearchService,
    search_result_from_dict,
    search_result_to_dict,
)
from slopsearx.snapshot import SnapshotStore

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


class _FakeStore:
    """In-memory key-value store (SearchCache-like)."""

    def __init__(self, connected: bool = True) -> None:
        self.is_connected = connected
        self._data: dict[str, dict[str, Any]] = {}

    async def get(self, key: str) -> dict[str, Any] | None:
        return self._data.get(key)

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        del ttl
        self._data[key] = value


class _MediaEngine(EngineAdapter):
    """Deterministic mock engine that returns media (or text) results."""

    def __init__(
        self,
        name: str,
        *,
        media_types: tuple[str, ...] = (),
        media: Any = None,
        count: int = 2,
        categories: list[str] | None = None,
    ) -> None:
        super().__init__()
        self.name = name
        self.categories = list(categories or ["general"])
        self.supported_media_types = tuple(media_types)
        self._media = media
        self._count = count
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        self.calls += 1
        results = []
        for i in range(self._count):
            m = self._media
            results.append(
                SearchResult(
                    url=f"https://{self.name}.example/{i}",
                    title=f"{self.name} result {i}",
                    content="media result content" if m else "text result content",
                    engine=self.name,
                    position=i + 1,
                    thumbnail=m.thumbnail if m else None,
                    img_src=m.url if m else None,
                    media=m,
                )
            )
        return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=2.0)


def _build_state(
    engine_map: dict[str, EngineAdapter],
    *,
    policy: MCPPolicy | None = None,
    catalog: CapabilityCatalog | None = None,
) -> McpState:
    policy = policy or load_mcp_policy(config_path=None)
    ctx = AppContext(
        active_engines=engine_map,
        router=None,
        cache=_FakeStore(),
        tier1_engines=set(engine_map),
        sensitive_engines=policy.sensitive_engines,
    )
    catalog = catalog or CapabilityCatalog(
        config=load_config(),
        adapters=engine_map,
        sensitive_engines=policy.sensitive_engines,
    )
    service = SearchService(ctx)
    store = _FakeStore()
    snapshots = SnapshotStore(store, ttl_seconds=policy.snapshot_ttl_seconds)
    job_store = ResearchJobStore(store)
    runner = ResearchJobRunner(service, job_store, snapshots, catalog, policy)
    return McpState(
        ctx=ctx,
        policy=policy,
        catalog=catalog,
        service=service,
        snapshots=snapshots,
        job_store=job_store,
        runner=runner,
        version="test",
    )


IMAGE_MEDIA = build_media(
    "image",
    url="https://img.example/a.jpg",
    thumbnail="https://img.example/a_thumb.jpg",
    source="https://page.example/a",
    width=640,
    height=480,
)

VIDEO_MEDIA = build_media(
    "video",
    url="https://video.example/clip.mp4",
    thumbnail="https://video.example/thumb.jpg",
    source="https://page.example/clip",
    duration=125.5,
)


@pytest.fixture
def media_state() -> McpState:
    state_obj = _build_state(
        {
            "brave": _MediaEngine("brave", media_types=("image", "video"), media=IMAGE_MEDIA),
            "duckduckgo": _MediaEngine("duckduckgo", media_types=("image",), media=IMAGE_MEDIA),
            "wikipedia": _MediaEngine("wikipedia", media_types=(), media=None),
        }
    )
    set_state(state_obj)
    yield state_obj
    set_state(None)


# ---------------------------------------------------------------------------
# MediaInfo contract
# ---------------------------------------------------------------------------


class TestMediaInfoContract:
    def test_build_media_sanitizes_urls(self) -> None:
        media = build_media(
            "image",
            url="https://img.example/a.jpg?api_key=secret",
            thumbnail="https://img.example/t.jpg?token=abc",
            source="https://page.example/a?key=x",
        )
        assert media is not None
        assert "api_key" not in media.url  # type: ignore[union-attr]
        assert "token" not in media.thumbnail  # type: ignore[union-attr]
        assert "key" not in media.source  # type: ignore[union-attr]

    def test_build_media_rejects_unknown_media_type(self) -> None:
        assert build_media("gif") is None

    def test_media_to_dict_drops_absent_fields(self) -> None:
        media = build_media("image", url="https://img.example/a.jpg")
        assert media_to_dict(media) == {"media_type": "image", "url": "https://img.example/a.jpg"}

    def test_media_round_trip(self) -> None:
        serialized = media_to_dict(IMAGE_MEDIA)
        assert serialized == {
            "media_type": "image",
            "url": "https://img.example/a.jpg",
            "thumbnail": "https://img.example/a_thumb.jpg",
            "source": "https://page.example/a",
            "width": 640,
            "height": 480,
        }
        rebuilt = media_from_dict(serialized)
        assert rebuilt == IMAGE_MEDIA

    def test_media_from_dict_rejects_malformed_and_unknown_type(self) -> None:
        assert media_from_dict(None) is None
        assert media_from_dict("image") is None
        assert media_from_dict({"media_type": "gif"}) is None


# ---------------------------------------------------------------------------
# Adapter media records (Brave / DuckDuckGo style)
# ---------------------------------------------------------------------------


class TestAdapterMediaRecords:
    def test_brave_image_parse_attaches_media_with_dimensions(self) -> None:
        adapter = discover_engines({"brave": {"enabled": True, "api_key": "k"}})["brave"]
        results = adapter._parse_image_results(
            [
                {
                    "title": "Photo",
                    "page_url": "https://page.example/photo",
                    "description": "a photo",
                    "thumbnail": {"src": "https://img.example/thumb.jpg"},
                    "image": {"src": "https://img.example/full.jpg", "width": 1280, "height": 720},
                }
            ]
        )
        assert results[0].img_src == "https://img.example/thumb.jpg"
        assert results[0].media is not None
        assert results[0].media.media_type == "image"
        assert results[0].media.url == "https://img.example/full.jpg"
        assert results[0].media.thumbnail == "https://img.example/thumb.jpg"
        assert results[0].media.source == "https://page.example/photo"
        assert results[0].media.width == 1280
        assert results[0].media.height == 720

    def test_brave_image_parse_omits_dimensions_when_absent(self) -> None:
        adapter = discover_engines({"brave": {"enabled": True, "api_key": "k"}})["brave"]
        results = adapter._parse_image_results(
            [
                {
                    "title": "Photo",
                    "page_url": "https://page.example/photo",
                    "thumbnail": {"src": "https://img.example/thumb.jpg"},
                }
            ]
        )
        assert results[0].media is not None
        assert results[0].media.url == "https://img.example/thumb.jpg"
        assert results[0].media.width is None
        assert results[0].media.height is None

    def test_brave_video_parse_attaches_video_media_only_with_thumbnail(self) -> None:
        adapter = discover_engines({"brave": {"enabled": True, "api_key": "k"}})["brave"]
        results = adapter._parse_video_results(
            [
                {
                    "url": "https://video.example/clip",
                    "title": "Clip",
                    "description": "a video",
                    "thumbnail": {"src": "https://video.example/thumb.jpg"},
                    "duration": 90,
                },
                {"url": "https://video.example/no-thumb", "title": "No thumb"},
            ]
        )
        assert results[0].media is not None
        assert results[0].media.media_type == "video"
        assert results[0].media.duration == 90
        assert results[1].media is None

    def test_brave_image_parse_omits_media_without_image_source(self) -> None:
        """An item with no thumbnail/image source must not fabricate a media record."""
        adapter = discover_engines({"brave": {"enabled": True, "api_key": "k"}})["brave"]
        results = adapter._parse_image_results(
            [
                {
                    "title": "Text-only",
                    "page_url": "https://page.example/text",
                    "description": "no image source",
                },
                {
                    "title": "Photo",
                    "page_url": "https://page.example/photo",
                    "thumbnail": {"src": "https://img.example/thumb.jpg"},
                },
            ]
        )
        assert results[0].media is None
        assert results[1].media is not None
        assert results[1].media.media_type == "image"

    def test_duckduckgo_image_parse_attaches_image_media(self) -> None:
        adapter = discover_engines({"duckduckgo": {"enabled": True}})["duckduckgo"]
        html = (
            '<html><body><div class="tile--img">'
            '<img src="//img.example/thumb.jpg" alt="Photo">'
            '<a href="https://page.example/full">Source</a>'
            '<span class="tile__caption">caption</span>'
            "</div></body></html>"
        )
        results = adapter._parse_image_html(html, "query", 10)
        assert len(results) == 1
        assert results[0].img_src == "https://img.example/thumb.jpg"
        assert results[0].media is not None
        assert results[0].media.media_type == "image"
        assert results[0].media.url == "https://img.example/thumb.jpg"
        assert results[0].media.source == "https://page.example/full"

    def test_duckduckgo_image_parse_omits_media_without_img_src(self) -> None:
        """A tile with no <img> source must not fabricate a degenerate media record."""
        adapter = discover_engines({"duckduckgo": {"enabled": True}})["duckduckgo"]
        html = (
            '<html><body><div class="tile--img">'
            '<a href="https://page.example/full">Source only, no image</a>'
            "</div></body></html>"
        )
        results = adapter._parse_image_html(html, "query", 10)
        assert len(results) == 1
        assert results[0].media is None
        assert results[0].img_src is None


# ---------------------------------------------------------------------------
# Media capability visibility
# ---------------------------------------------------------------------------


class TestMediaCapabilityVisibility:
    def test_registry_declarations_for_media_types(self) -> None:
        catalog = CapabilityCatalog(config=load_config())
        assert catalog.get("brave").supported_media_types == ["image", "video"]  # type: ignore[union-attr]
        assert catalog.get("duckduckgo").supported_media_types == ["image"]  # type: ignore[union-attr]
        assert catalog.get("wikipedia").supported_media_types == []  # type: ignore[union-attr]

    def test_engines_for_media_type(self) -> None:
        catalog = CapabilityCatalog(config=load_config())
        image_engines = set(catalog.engines_for_media_type("image"))
        assert "brave" in image_engines
        assert "duckduckgo" in image_engines
        assert "wikipedia" not in image_engines
        assert set(catalog.engines_for_media_type("video")) == {"brave"}
        assert catalog.engines_for_media_type("gif") == []

    async def test_tool_exposes_supported_media_types(self) -> None:
        state = _build_state(
            {},
            catalog=CapabilityCatalog(config=load_config()),
        )
        set_state(state)
        try:
            result = await t.slopsearx_list_capabilities(include_disabled=True)
        finally:
            set_state(None)
        by_name = {e["name"]: e for e in result["engines"]}
        assert by_name["brave"]["supported_media_types"] == ["image", "video"]
        assert by_name["duckduckgo"]["supported_media_types"] == ["image"]
        assert by_name["wikipedia"]["supported_media_types"] == []

    def test_resources_render_supported_media_types(self) -> None:
        state = _build_state({}, catalog=CapabilityCatalog(config=load_config()))
        set_state(state)
        try:
            engine = r.render_engine_capability("brave")
            catalog = r.render_capabilities()
            routing = r.render_routing_profiles()
        finally:
            set_state(None)
        assert "supported media types: image, video" in engine
        assert "supported media types: image, video" in catalog
        assert "## images" in routing
        assert "- media types: image" in routing


# ---------------------------------------------------------------------------
# Media intent routing (MCP contract)
# ---------------------------------------------------------------------------


class TestMediaIntentRouting:
    async def test_images_intent_selects_image_capable_engines(self, media_state: McpState) -> None:
        result = await t.slopsearx_search("cats", intent="images")
        assert "error" not in result
        assert set(result["scope"]["selected_engines"]) == {"brave", "duckduckgo"}

    async def test_videos_intent_selects_video_capable_engines(self, media_state: McpState) -> None:
        result = await t.slopsearx_search("cats", intent="videos")
        assert "error" not in result
        assert result["scope"]["selected_engines"] == ["brave"]

    async def test_media_type_param_constrains_scope(self, media_state: McpState) -> None:
        result = await t.slopsearx_search("cats", media_type="video")
        assert "error" not in result
        assert result["scope"]["selected_engines"] == ["brave"]

    async def test_unknown_media_type_rejected(self, media_state: McpState) -> None:
        result = await t.slopsearx_search("cats", media_type="gif")
        assert result["error"]["code"] == "invalid_input"
        assert result["error"]["field"] == "media_type"

    async def test_no_media_coverage_reports_gap_explicitly(self, media_state: McpState) -> None:
        result = await t.slopsearx_search("cats", media_type="image", engines=["wikipedia"])
        assert result["error"]["code"] == "media_coverage_gap"
        assert result["error"]["media_type"] == "image"

    async def test_media_intent_no_false_gap_when_topic_set_omits_media_engines(self) -> None:
        """A media intent reaches media engines even when topic/tier-1 omit them.

        Routing a media search through the topic router (or tier-1 fallback)
        and only then intersecting with media-capable engines could yield an
        empty selection and a false media_coverage_gap — the media type must
        seed the candidate base directly.
        """
        from slopsearx.router import QueryRouter

        state_obj = _build_state(
            {
                "brave": _MediaEngine("brave", media_types=("image",), media=IMAGE_MEDIA, count=1),
                "duckduckgo": _MediaEngine("duckduckgo", media_types=("image",), media=IMAGE_MEDIA, count=1),
                "wikipedia": _MediaEngine("wikipedia", media_types=(), media=None, count=1),
            }
        )
        # Operator topic/tier-1 configuration that omits the media engines.
        state_obj.ctx.router = QueryRouter(
            routing_config={
                "enabled": True,
                "topics": {"code": {"keywords": ["python"], "engines": ["wikipedia"]}},
                "fallback": ["wikipedia"],
            }
        )
        state_obj.ctx.tier1_engines = {"wikipedia"}
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("python api", intent="images")
            assert "error" not in result, result
            assert set(result["scope"]["selected_engines"]) == {"brave", "duckduckgo"}
            assert any("media" in card for card in result["results"])
        finally:
            set_state(None)

    async def test_media_type_with_non_media_category_dispatches_media_not_text(self) -> None:
        """media_type + explicit non-media category never silently runs a text search.

        The dispatch category must be the media category translation
        (``images``), not the caller's text category, so category-aware
        adapters (Brave/DDG) reach their media endpoints.
        """
        captured: dict[str, Any] = {}

        class _RecordingMediaEngine(_MediaEngine):
            async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
                captured["categories"] = (params or {}).get("categories")
                return await super().search(query, params)

        state_obj = _build_state(
            {
                "brave": _RecordingMediaEngine("brave", media_types=("image",), media=IMAGE_MEDIA, count=1),
                "wikipedia": _MediaEngine("wikipedia", media_types=(), media=None, count=1),
            }
        )
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("cats", categories=["general"], media_type="image")
            assert "error" not in result, result
            assert set(result["scope"]["selected_engines"]) == {"brave"}
            assert captured["categories"] == ["images"]
            cards = result["results"]
            assert cards and all("media" in card for card in cards)
        finally:
            set_state(None)

    async def test_images_intent_with_explicit_media_engine_dispatches_media(self, media_state: McpState) -> None:
        """intent=images + explicit media-capable engine stays a media search.

        An explicit engines list must not discard the intent profile's media
        type: brave advertises images, so the search dispatches media cards
        instead of silently running a text search.
        """
        result = await t.slopsearx_search("cats", intent="images", engines=["brave"])
        assert "error" not in result, result
        assert result["scope"]["selected_engines"] == ["brave"]
        cards = result["results"]
        assert cards and all("media" in card for card in cards)
        assert all(card["media"]["media_type"] == "image" for card in cards)

    async def test_images_intent_with_non_media_engine_reports_coverage_gap(self, media_state: McpState) -> None:
        """intent=images + explicit non-media engine fails closed on a gap.

        wikipedia advertises no media type, so the media-intent scope
        resolves to no engines and must report an explicit
        media_coverage_gap rather than silently running a text search.
        """
        result = await t.slopsearx_search("cats", intent="images", engines=["wikipedia"])
        assert result["error"]["code"] == "media_coverage_gap"
        assert result["error"]["media_type"] == "image"
        assert result["error"]["field"] == "media_type"

    async def test_images_intent_with_explicit_category_dispatches_media_not_text(self, media_state: McpState) -> None:
        """intent=images + explicit categories stays a media search.

        An explicit category list must not discard the intent profile's media
        type: general-category engines that advertise images dispatch media
        cards, and media-incapable engines are filtered from the scope.
        """
        result = await t.slopsearx_search("cats", intent="images", categories=["general"])
        assert "error" not in result, result
        assert set(result["scope"]["selected_engines"]) == {"brave", "duckduckgo"}
        cards = result["results"]
        assert cards and all("media" in card for card in cards)

    async def test_extend_research_rejects_media_intent(self, media_state: McpState) -> None:
        """Extending research with a media intent errors clearly, never silently text.

        Research subqueries carry no media_type, so a media intent must be
        rejected explicitly rather than silently running a text search.
        """
        media_state.policy.enabled_tools["research"] = True
        job = ResearchJob(
            job_id=generate_job_id(),
            question="q",
            strategy="triangulate",
            deadline=time.time() + 3600,
            queries=[ResearchQuery(index=0, query="ok", intent="web", engines=["brave"])],
        )
        await media_state.job_store.save(job)

        result = await t.slopsearx_extend_research(job.job_id, "more", intent="images")
        assert result["error"]["code"] == "invalid_intent"
        assert result["error"]["field"] == "intent"

        loaded = await media_state.job_store.load(job.job_id)
        assert loaded is not None
        assert len(loaded.queries) == 1  # nothing appended, nothing executed

        result = await t.slopsearx_extend_research(job.job_id, "more", intent="videos")
        assert result["error"]["code"] == "invalid_intent"

    async def test_scope_preview_reports_media_type_and_exclusions(self, media_state: McpState) -> None:
        result = await t.slopsearx_explain_search_scope("cats", intent="images")
        assert result["media_type"] == "image"
        assert set(result["selected_engines"]) == {"brave", "duckduckgo"}
        assert any(e["engine"] == "wikipedia" for e in result["excluded_engines"])

    async def test_mixed_text_and_media_cards(self, media_state: McpState) -> None:
        """Text results keep text cards; media results carry a media triage key."""
        result = await t.slopsearx_search("cats", intent="images")
        cards = result["results"]
        media_cards = [c for c in cards if "media" in c]
        assert media_cards, "expected media cards in a media-intent search"
        for card in media_cards:
            assert card["media"]["media_type"] == "image"
            assert "url" not in card["media"]  # triage summary, not the full record
        # A text search over the same fixture produces no media keys.
        text_result = await t.slopsearx_search("cats", engines=["wikipedia"])
        for card in text_result["results"]:
            assert "media" not in card


# ---------------------------------------------------------------------------
# Media result cards and records (progressive disclosure)
# ---------------------------------------------------------------------------


class TestMediaCardsAndRecords:
    async def test_card_carries_triage_summary(self, media_state: McpState) -> None:
        result = await t.slopsearx_search("cats", intent="images")
        card = next(c for c in result["results"] if "media" in c)
        assert card["media"] == {
            "media_type": "image",
            "thumbnail": "https://img.example/a_thumb.jpg",
            "width": 640,
            "height": 480,
        }

    async def test_record_returns_complete_media_record(self, media_state: McpState) -> None:
        result = await t.slopsearx_search("cats", intent="images")
        card = next(c for c in result["results"] if "media" in c)
        record = await t.slopsearx_read_result(card["result_id"])
        assert record["media"] == media_to_dict(IMAGE_MEDIA)
        # Full record reveals the media file URL and source attribution that
        # the card's triage summary deliberately omits.
        assert record["media"]["url"] == "https://img.example/a.jpg"
        assert record["media"]["source"] == "https://page.example/a"

    async def test_missing_media_fields_are_absent(self, media_state: McpState) -> None:
        """A media result without thumbnail/dimensions reports only present fields."""
        sparse = build_media("image", url="https://img.example/sparse.jpg")
        state_obj = _build_state(
            {
                "brave": _MediaEngine("brave", media_types=("image",), media=sparse, count=1),
                "wikipedia": _MediaEngine("wikipedia", media_types=(), media=None, count=1),
            }
        )
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("cats", engines=["brave"])
            card = result["results"][0]
            assert card["media"] == {"media_type": "image"}
            record = await t.slopsearx_read_result(card["result_id"])
            assert record["media"] == {"media_type": "image", "url": "https://img.example/sparse.jpg"}
            assert "thumbnail" not in record["media"]
            assert "width" not in record["media"]
        finally:
            set_state(None)

    async def test_video_record_carries_duration(self) -> None:
        state_obj = _build_state(
            {
                "brave": _MediaEngine("brave", media_types=("video",), media=VIDEO_MEDIA, count=1),
            }
        )
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("cats", intent="videos")
            card = result["results"][0]
            assert card["media"]["duration"] == 125.5
            record = await t.slopsearx_read_result(card["result_id"])
            assert record["media"]["duration"] == 125.5
            assert record["media"]["media_type"] == "video"
        finally:
            set_state(None)


# ---------------------------------------------------------------------------
# Serialization preservation (snapshots, cache, HTTP-compatible formatter)
# ---------------------------------------------------------------------------


class TestMediaSerialization:
    def test_search_result_media_round_trip(self) -> None:
        result = SearchResult(
            url="https://page.example/a",
            title="A",
            content="y",
            engine="brave",
            engines={"brave"},
            thumbnail="https://img.example/a_thumb.jpg",
            img_src="https://img.example/a.jpg",
            media=IMAGE_MEDIA,
        )
        serialized = search_result_to_dict(result)
        assert serialized["media"] == media_to_dict(IMAGE_MEDIA)
        rebuilt = search_result_from_dict(serialized)
        assert rebuilt.media == IMAGE_MEDIA

    def test_search_result_without_media_round_trips_none(self) -> None:
        result = SearchResult(url="https://x.com", title="X", content="y", engine="wikipedia")
        serialized = search_result_to_dict(result)
        assert serialized["media"] is None
        assert search_result_from_dict(serialized).media is None

    def test_json_formatter_preserves_media(self) -> None:
        result = SearchResult(
            url="https://page.example/a",
            title="A",
            content="y",
            engine="brave",
            engines={"brave"},
            media=IMAGE_MEDIA,
        )
        response = format_json(results=[result], query="cats")
        assert response["results"][0]["media"] == media_to_dict(IMAGE_MEDIA)

    def test_yaml_formatter_preserves_media(self) -> None:
        result = SearchResult(
            url="https://page.example/a",
            title="A",
            content="y",
            engine="brave",
            engines={"brave"},
            media=IMAGE_MEDIA,
        )
        output = format_yaml_markdown([result], "cats")
        parsed = yaml.safe_load(output.split("---\n", 1)[0])
        assert parsed["results"][0]["media"] == media_to_dict(IMAGE_MEDIA)

    def test_response_payload_round_trip_preserves_media(self) -> None:
        from slopsearx.service import search_response_from_payload, search_response_to_payload

        response = SearchResponse(
            query="cats",
            results=[
                SearchResult(
                    url="https://page.example/a",
                    title="A",
                    content="y",
                    engine="brave",
                    engines={"brave"},
                    media=IMAGE_MEDIA,
                )
            ],
            scope=ScopeDecision(),
            engine_outcomes=[],
        )
        rebuilt = search_response_from_payload(search_response_to_payload(response))
        assert rebuilt.results[0].media == IMAGE_MEDIA
