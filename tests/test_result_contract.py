"""Tests for the progressive-disclosure result card/record contract.

Covers VAL-SEARCH-004/005/017/018, VAL-SPEC-012, VAL-EXPAND-001/005/006/
007/008/009/010/011/012/013/017, VAL-TARGET-013, and VAL-CROSS-003: cards
stay compact and carry a stable ``<cursor>:<index>`` result_id; the
expanded record reveals full content, media, provenance, a
``content_available`` flag, and an explicit non-verification note.
"""

from __future__ import annotations

import time
from typing import Any

import pytest

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, MCPPolicy, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.research import ResearchJobRunner, ResearchJobStore
from slopsearx.service import SearchService
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


class _RichEngine(EngineAdapter):
    """Parameterizable mock engine with long content, media, and provenance."""

    def __init__(
        self,
        name: str,
        *,
        count: int = 3,
        content: str = "Content for %s result %d.",
        contrib_engines: set[str] | None = None,
        thumbnail: str | None = None,
        img_src: str | None = None,
        categories: list[str] | None = None,
        tier: int = 1,
        urls: list[str] | None = None,
    ) -> None:
        super().__init__()
        self.name = name
        self._count = count
        self._content = content
        self._engines = contrib_engines
        self._thumbnail = thumbnail
        self._img_src = img_src
        self.categories = list(categories or ["general"])
        self._tier = tier
        self._urls = urls
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        self.calls += 1
        engines_set = self._engines or {self.name}
        return AdapterResponse(
            results=[
                SearchResult(
                    url=(self._urls[i] if self._urls is not None else f"https://{self.name}{i}.com/{i}"),
                    title=f"{self.name} result {i}",
                    content=(self._content % (self.name, i)) if "%" in self._content else self._content,
                    engine=self.name,
                    engines=set(engines_set),
                    score=2.0 if i == 0 else 1.0,
                    position=i + 1,
                    category=self.categories[0],
                    published_date="2026-01-01",
                    thumbnail=self._thumbnail,
                    img_src=self._img_src,
                    tier=self._tier,
                )
                for i in range(self._count)
            ],
            status=EngineStatus.OK,
            latency_ms=2.0,
        )


def _build_state(engine_map: dict[str, EngineAdapter], *, policy: MCPPolicy | None = None) -> McpState:
    from slopsearx.service import AppContext

    policy = policy or load_mcp_policy(config_path=None)
    ctx = AppContext(
        active_engines=engine_map,
        router=None,
        cache=_FakeStore(),
        tier1_engines=set(engine_map),
        sensitive_engines=policy.sensitive_engines,
    )
    catalog = CapabilityCatalog(config=load_config())
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


LONG_CONTENT = "Word " * 200  # > 300 chars


@pytest.fixture
def state() -> McpState:
    state_obj = _build_state(
        {
            "brave": _RichEngine(
                "brave",
                count=3,
                content=LONG_CONTENT,
                thumbnail="https://brave.com/thumb.jpg",
                img_src="https://brave.com/img.jpg",
            ),
            "wikipedia": _RichEngine(
                "wikipedia",
                count=2,
                content="Short snippet %s %d.",  # snippet-only adapter
                contrib_engines={"wikipedia", "brave"},
            ),
        }
    )
    set_state(state_obj)
    yield state_obj
    set_state(None)


CARD_KEYS = {
    "title",
    "url",
    "snippet",
    "source_engines",
    "source_count",
    "primary_engine",
    "category",
    "published_at",
    "score",
    "position",
    "tier",
    "citation",
    "result_id",
}


# ---------------------------------------------------------------------------
# Cards (VAL-SEARCH-004, VAL-SEARCH-005)
# ---------------------------------------------------------------------------


class TestCards:
    async def test_cards_are_compact_and_carry_result_id(self, state: McpState) -> None:
        """VAL-SEARCH-004 — compact cards with stable result_id; no content/thumbnail/img_src."""
        result = await t.slopsearx_search("hello world")
        cursor = result["meta"]["cursor"]
        assert cursor is not None

        for card in result["results"]:
            assert set(card) == CARD_KEYS
            assert card["result_id"] == f"{cursor}:{card['position'] - 1}"
            assert "content" not in card
            assert "thumbnail" not in card
            assert "img_src" not in card
            assert card["source_count"] == len(card["source_engines"])  # VAL-SEARCH-017

    async def test_every_successful_search_issues_cursor(self, state: McpState) -> None:
        """VAL-SEARCH-005 — cursor usable with read_results to re-read the page."""
        result = await t.slopsearx_search("hello")
        cursor = result["meta"]["cursor"]
        assert cursor is not None

        reread = await t.slopsearx_read_results(cursor, page=1)
        assert [c["url"] for c in reread["results"]] == [c["url"] for c in result["results"]]

    async def test_ranking_semantics_present(self, state: McpState) -> None:
        """VAL-SEARCH-018 — score/position/tier present and meta.ranking stated."""
        result = await t.slopsearx_search("hello")
        for card in result["results"]:
            assert isinstance(card["score"], float)
            assert isinstance(card["position"], int)
            assert isinstance(card["tier"], int) and card["tier"] in (1, 2)
        assert result["meta"]["ranking"] == "tier_then_cross_engine_presence"


# ---------------------------------------------------------------------------
# Result records (VAL-EXPAND-005..013)
# ---------------------------------------------------------------------------


class TestRecords:
    async def test_read_result_returns_full_content_more_than_snippet(self, state: McpState) -> None:
        """VAL-EXPAND-005 — complete content, strictly more than the 300-char snippet."""
        result = await t.slopsearx_search("hello")
        card = result["results"][0]
        expanded = await t.slopsearx_read_result(card["result_id"])

        assert len(expanded["content"]) > len(card["snippet"])
        assert expanded["content"] == LONG_CONTENT

    async def test_media_fields_appear_in_record_not_card(self, state: McpState) -> None:
        """VAL-EXPAND-006 — thumbnail/img_src present in the record, absent from cards."""
        result = await t.slopsearx_search("hello")
        card = result["results"][0]
        assert "thumbnail" not in card and "img_src" not in card

        expanded = await t.slopsearx_read_result(card["result_id"])
        assert expanded["thumbnail"] == "https://brave.com/thumb.jpg"
        assert expanded["img_src"] == "https://brave.com/img.jpg"

    async def test_read_result_lists_all_contributing_engines(self, state: McpState) -> None:
        """VAL-EXPAND-007 — source_engines lists every engine with matching source_count."""
        # Two engines returning an overlapping URL merge into one result
        # whose engines set = {brave, wikipedia}.
        multi = _build_state(
            {
                "brave": _RichEngine("brave", count=2, content=LONG_CONTENT, urls=["https://shared.com/0", "https://b.com/1"]),
                "wikipedia": _RichEngine(
                    "wikipedia", count=2, content=LONG_CONTENT, urls=["https://shared.com/0", "https://w.com/1"]
                ),
            }
        )
        set_state(multi)
        try:
            result = await t.slopsearx_search("hello")
            shared = next(c for c in result["results"] if c["url"] == "https://shared.com/0")
            assert set(shared["source_engines"]) == {"brave", "wikipedia"}

            expanded = await t.slopsearx_read_result(shared["result_id"])
            assert set(expanded["source_engines"]) == {"brave", "wikipedia"}
            assert len(expanded["source_engines"]) == expanded["source_count"]
        finally:
            set_state(None)

    async def test_read_result_returns_provenance_and_snapshot_context(self, state: McpState) -> None:
        """VAL-EXPAND-008 — provenance (query/query_id/rank) plus citation/snapshot context."""
        result = await t.slopsearx_search("some query")
        cursor = result["meta"]["cursor"]
        card = result["results"][0]
        expanded = await t.slopsearx_read_result(card["result_id"])

        assert expanded["provenance"]["query"] == "some query"
        assert expanded["provenance"]["query_id"] == result["meta"]["query_id"]
        assert expanded["provenance"]["rank_explanation"] == "tier_then_cross_engine_presence"
        assert expanded["citation"]["url"] == expanded["url"]
        assert expanded["snapshot"]["cursor"] == cursor
        assert expanded["snapshot"]["query"] == "some query"

    async def test_content_available_flag(self, state: McpState) -> None:
        """VAL-EXPAND-009 — content_available is true for full content, false for snippet-only."""
        result = await t.slopsearx_search("hello")
        cards_by_engine = {c["primary_engine"]: c for c in result["results"]}

        full = await t.slopsearx_read_result(cards_by_engine["brave"]["result_id"])
        assert full["content_available"] is True

        snippet_only = await t.slopsearx_read_result(cards_by_engine["wikipedia"]["result_id"])
        assert snippet_only["content_available"] is False

    async def test_non_verification_note_present(self, state: McpState) -> None:
        """VAL-EXPAND-010 — explicit non-verification note on every record."""
        result = await t.slopsearx_search("hello")
        expanded = await t.slopsearx_read_result(result["results"][0]["result_id"])
        assert "SlopSearX did not fetch or verify the linked page" in expanded["note"]

    async def test_snippet_only_adapter_reports_content_unavailable(self, state: McpState) -> None:
        """VAL-EXPAND-011 — snippet-only adapter states full content is unavailable."""
        result = await t.slopsearx_search("hello")
        cards_by_engine = {c["primary_engine"]: c for c in result["results"]}
        snippet_only = await t.slopsearx_read_result(cards_by_engine["wikipedia"]["result_id"])

        assert snippet_only["content_available"] is False
        assert "adapter returned snippet only" in snippet_only["content_unavailable_note"]

    async def test_card_result_id_matches_the_record_it_expands(self, state: McpState) -> None:
        """VAL-EXPAND-012 — read_result(card.result_id) resolves the same title/url."""
        result = await t.slopsearx_search("hello")
        for card in result["results"]:
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert expanded["title"] == card["title"]
            assert expanded["url"] == card["url"]

    async def test_record_source_engines_equals_card_source_engines(self, state: McpState) -> None:
        """VAL-EXPAND-013 — record.source_engines/source_count match the card."""
        result = await t.slopsearx_search("hello")
        for card in result["results"]:
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert expanded["source_engines"] == card["source_engines"]
            assert expanded["source_count"] == card["source_count"]


# ---------------------------------------------------------------------------
# Snapshot-absolute indexing (VAL-EXPAND-001/017, VAL-CROSS-003)
# ---------------------------------------------------------------------------


class TestSnapshotNegativeLifecycle:
    async def test_store_unavailable_read_results(self, state: McpState) -> None:
        """VAL-EXPAND-019 — store-unavailable read_results returns store_unavailable."""
        result = await t.slopsearx_search("hello")
        cursor = result["meta"]["cursor"]
        assert cursor is not None

        state.snapshots._store = _FakeStore(connected=False)
        resp = await t.slopsearx_read_results(cursor, page=1)

        assert resp["error"]["code"] == "store_unavailable"

    async def test_store_unavailable_read_result(self, state: McpState) -> None:
        """VAL-EXPAND-019 — store-unavailable read_result returns store_unavailable."""
        result = await t.slopsearx_search("hello")
        rid = result["results"][0]["result_id"]

        state.snapshots._store = _FakeStore(connected=False)
        resp = await t.slopsearx_read_result(rid)

        assert resp["error"]["code"] == "store_unavailable"

    async def test_expired_handle_read_results(self, state: McpState) -> None:
        """VAL-EXPAND-015 — expired cursor yields expired_handle with expires_at."""
        result = await t.slopsearx_search("hello")
        cursor = result["meta"]["cursor"]
        assert cursor is not None

        payload = state.snapshots._store._data[f"mcp:snapshot:default:{cursor}"]
        payload["expires_at"] = time.time() - 1
        resp = await t.slopsearx_read_results(cursor, page=1)

        assert resp["error"]["code"] == "expired_handle"
        assert resp["error"]["handle"] == cursor
        assert resp["error"]["expires_at"]  # ISO timestamp present

    async def test_expired_handle_read_result(self, state: McpState) -> None:
        """VAL-EXPAND-015 — expired result_id yields expired_handle with metadata."""
        result = await t.slopsearx_search("hello")
        rid = result["results"][0]["result_id"]
        cursor = rid.split(":", 1)[0]

        payload = state.snapshots._store._data[f"mcp:snapshot:default:{cursor}"]
        payload["expires_at"] = time.time() - 1
        resp = await t.slopsearx_read_result(rid)

        assert resp["error"]["code"] == "expired_handle"
        assert resp["error"]["handle"] == rid
        assert resp["error"]["expires_at"]


# ---------------------------------------------------------------------------


class TestSnapshotIndexing:
    async def test_result_ids_are_unique_and_snapshot_absolute(self, state: McpState) -> None:
        """VAL-EXPAND-001 — every read_results card carries a unique <cursor>:<index> id."""
        result = await t.slopsearx_search("hello")
        cursor = result["meta"]["cursor"]
        assert cursor is not None

        page1 = await t.slopsearx_read_results(cursor, page=1, max_results=2)
        ids1 = [c["result_id"] for c in page1["results"]]
        assert ids1 == [f"{cursor}:0", f"{cursor}:1"]

        page2 = await t.slopsearx_read_results(cursor, page=2, max_results=2)
        ids2 = [c["result_id"] for c in page2["results"]]
        assert ids2 == [f"{cursor}:2", f"{cursor}:3"]

        all_ids = ids1 + ids2
        assert len(all_ids) == len(set(all_ids))

    async def test_initial_search_index_matches_page1(self, state: McpState) -> None:
        """VAL-EXPAND-017 — search result id equals page-1 read, page-2 uses absolute index."""
        result = await t.slopsearx_search("hello", max_results=2)
        cursor = result["meta"]["cursor"]

        page1 = await t.slopsearx_read_results(cursor, page=1, max_results=2)
        assert page1["results"][0]["result_id"] == result["results"][0]["result_id"]

        # The fixture set has 5 results; page 2's first card carries the
        # absolute index 2 (not a page-relative 0).
        page2 = await t.slopsearx_read_results(cursor, page=2, max_results=2)
        assert page2["results"][0]["result_id"].endswith(":2")

    async def test_result_id_stable_across_pages_and_repeated_reads(self, state: McpState) -> None:
        """VAL-CROSS-003 — the same result yields an identical result_id everywhere."""
        result = await t.slopsearx_search("hello", max_results=2)
        cursor = result["meta"]["cursor"]
        target = result["results"][0]["result_id"]

        first = await t.slopsearx_read_result(target)
        page1a = await t.slopsearx_read_results(cursor, page=1, max_results=2)
        page1b = await t.slopsearx_read_results(cursor, page=1, max_results=2)

        assert page1a["results"][0]["result_id"] == target
        assert page1b["results"][0]["result_id"] == target

        again = await t.slopsearx_read_result(target)
        assert again["url"] == first["url"]
        assert again["title"] == first["title"]


# ---------------------------------------------------------------------------
# Cross-tool consistency (VAL-SPEC-012, VAL-TARGET-013)
# ---------------------------------------------------------------------------


class TestCrossTool:
    async def test_specialist_cards_share_ordinary_card_schema(self) -> None:
        """VAL-SPEC-012 — jobs/security/science cards are field-identical to ordinary cards."""
        policy = load_mcp_policy(config_path=None)
        policy.enabled_tools["jobs"] = True
        policy.enabled_tools["security"] = True
        policy.enabled_tools["science"] = True
        engine_map = {
            "brave": _RichEngine("brave", content=LONG_CONTENT),
            "greenhouse": _RichEngine("greenhouse", content=LONG_CONTENT),
            "cve": _RichEngine("cve", content=LONG_CONTENT),
            "nvd": _RichEngine("nvd", content=LONG_CONTENT),
            "epss": _RichEngine("epss", content=LONG_CONTENT),
            "vulncheck": _RichEngine("vulncheck", content=LONG_CONTENT),
            "exploitdb": _RichEngine("exploitdb", content=LONG_CONTENT),
            "arxiv": _RichEngine("arxiv", content=LONG_CONTENT),
            "semanticscholar": _RichEngine("semanticscholar", content=LONG_CONTENT),
            "openalex": _RichEngine("openalex", content=LONG_CONTENT),
        }
        state_obj = _build_state(engine_map, policy=policy)
        set_state(state_obj)
        try:
            ordinary = await t.slopsearx_search("hello", engines=["brave"])
            jobs = await t.slopsearx_search_jobs("Acme", sources=["greenhouse"])
            security = await t.slopsearx_search_security("log4j", evidence_types=["vulnerability"])
            science = await t.slopsearx_search_science("attention", source_types=["papers"])

            ordinary_keys = set(ordinary["results"][0])
            for resp in (jobs, security, science):
                assert set(resp["results"][0]) == ordinary_keys
                assert "result_id" in resp["results"][0]
        finally:
            set_state(None)

    async def test_targeted_search_issues_snapshot_and_cursor(self, state: McpState) -> None:
        """VAL-TARGET-013 — targeted search issues a cursor usable with read tools."""
        result = await t.slopsearx_search_targeted("hello", engines=["brave"])
        cursor = result["meta"]["cursor"]
        assert cursor is not None

        reread = await t.slopsearx_read_results(cursor, page=1)
        assert reread["results"][0]["result_id"].startswith(f"{cursor}:")
        expanded = await t.slopsearx_read_result(reread["results"][0]["result_id"])
        assert expanded["title"] == reread["results"][0]["title"]
