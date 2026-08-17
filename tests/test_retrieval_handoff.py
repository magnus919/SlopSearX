"""Tests for the search-to-retrieval handoff boundary (issue 189).

Covers the machine-readable ``retrieval`` handoff record exposed on MCP
cards and expanded records: eligibility classification for missing, unsafe,
non-HTTP, and canonicalization-ambiguous URLs; the stable linkage that lets a
downstream retriever (e.g. GroktoCrawl) associate a capture with the
originating result and snapshot without parsing prose; and the contract
fixtures showing search -> handoff -> downstream capture provenance.
"""

from __future__ import annotations

import json
from pathlib import Path
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

FIXTURES = Path(__file__).parent / "fixtures" / "retrieval_handoff"

LONG_CONTENT = "Word " * 200  # > SNIPPET_LENGTH (300)
SHORT_CONTENT = "Short snippet."

# The closed url_status vocabulary documented in docs/RETRIEVAL_HANDOFF.md.
URL_STATUS_VOCABULARY = {"ok", "missing", "non_http", "unsafe_scheme", "ambiguous"}


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


class _UrlEngine(EngineAdapter):
    """One result per configured URL, with optional content/provenance."""

    def __init__(
        self,
        name: str,
        urls: list[str],
        *,
        content: str = "Content.",
        contrib_engines: set[str] | None = None,
    ) -> None:
        super().__init__()
        self.name = name
        self._urls = urls
        self._content = content
        self._engines = contrib_engines
        self.categories = ["general"]

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        return AdapterResponse(
            results=[
                SearchResult(
                    url=url,
                    title=f"result {i}",
                    content=self._content,
                    engine=self.name,
                    engines=set(self._engines or {self.name}),
                    score=1.0,
                    position=i + 1,
                    category="general",
                    tier=1,
                )
                for i, url in enumerate(self._urls)
            ],
            status=EngineStatus.OK,
            latency_ms=1.0,
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


@pytest.fixture
def state() -> McpState:
    state_obj = _build_state(
        {
            "brave": _UrlEngine(
                "brave",
                ["https://example.com/0", "https://example.com/1"],
                content=LONG_CONTENT,
            ),
        }
    )
    set_state(state_obj)
    yield state_obj
    set_state(None)


def _first_card(result: dict[str, Any]) -> dict[str, Any]:
    return result["results"][0]


# ---------------------------------------------------------------------------
# The handoff record on cards and expanded records
# ---------------------------------------------------------------------------


class TestHandoffRecord:
    async def test_card_exposes_compact_retrieval_eligibility(self, state: McpState) -> None:
        """Cards carry a compact eligibility block; no fetch target is invented."""
        result = await t.slopsearx_search("hello")
        card = _first_card(result)
        retrieval = card["retrieval"]
        assert set(retrieval) == {"contract", "version", "eligible", "url_status", "url_reason", "scheme"}
        assert retrieval["contract"] == t.RETRIEVAL_HANDOFF_CONTRACT
        assert retrieval["version"] == t.RETRIEVAL_HANDOFF_VERSION
        assert retrieval["eligible"] is True
        assert retrieval["url_status"] == "ok"
        assert retrieval["url_reason"] is None
        assert retrieval["scheme"] == "https"

    async def test_record_exposes_stable_handoff_record(self, state: McpState) -> None:
        """The expanded record carries the full, self-contained handoff record."""
        result = await t.slopsearx_search("hello")
        card = _first_card(result)
        expanded = await t.slopsearx_read_result(card["result_id"])
        handoff = expanded["retrieval"]
        assert set(handoff) == {
            "contract",
            "version",
            "result_id",
            "url",
            "url_status",
            "url_reason",
            "scheme",
            "eligible",
            "snippet_only",
            "verified",
            "verification_note",
            "provenance",
        }
        assert handoff["contract"] == t.RETRIEVAL_HANDOFF_CONTRACT
        assert handoff["version"] == t.RETRIEVAL_HANDOFF_VERSION
        assert handoff["result_id"] == card["result_id"]
        assert handoff["url"] == card["url"]
        assert handoff["eligible"] is True
        assert handoff["snippet_only"] is False  # LONG_CONTENT exceeds the snippet bound
        assert handoff["verified"] is False
        assert handoff["verification_note"] == "SlopSearX did not fetch or verify the linked page"
        assert handoff["provenance"]["source_engines"] == card["source_engines"]

    async def test_snippet_only_tracks_content_availability(self, state: McpState) -> None:
        """snippet_only is True exactly when the record reports content unavailable."""
        short = _build_state({"brave": _UrlEngine("brave", ["https://example.com/x"], content=SHORT_CONTENT)})
        set_state(short)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert expanded["retrieval"]["snippet_only"] is True
            assert expanded["content_available"] is False
        finally:
            set_state(None)

    async def test_handoff_linkage_survives_cards_expansion_and_snapshot_reads(self, state: McpState) -> None:
        """Provenance survives compact cards, expansion, and snapshot reads.

        A downstream retriever can associate a capture with the originating
        result and snapshot without parsing prose: the handoff's result_id,
        url, and provenance match the card, the expanded record, and the
        paginated snapshot cards.
        """
        result = await t.slopsearx_search("some query", max_results=1)
        cursor = result["meta"]["cursor"]
        assert cursor is not None
        card = _first_card(result)

        expanded = await t.slopsearx_read_result(card["result_id"])
        page = await t.slopsearx_read_results(cursor, page=1, max_results=1)

        handoff = expanded["retrieval"]
        assert handoff["result_id"] == card["result_id"] == page["results"][0]["result_id"]
        assert handoff["url"] == card["url"] == page["results"][0]["url"]
        assert handoff["provenance"]["snapshot_cursor"] == cursor
        assert handoff["provenance"]["query_id"] == result["meta"]["query_id"]
        assert handoff["provenance"]["query"] == "some query"
        assert handoff["provenance"]["source_engines"] == card["source_engines"]
        assert handoff["eligible"] is True


# ---------------------------------------------------------------------------
# URL classification: missing / non-HTTP / unsafe / ambiguous
# ---------------------------------------------------------------------------


class TestUrlClassification:
    @pytest.mark.parametrize(
        ("url", "expected_status", "expected_scheme"),
        [
            ("https://example.com/page", "ok", "https"),
            ("http://example.com/page", "ok", "http"),
            ("", "missing", None),
            ("   ", "missing", None),
            ("mailto:someone@example.com", "non_http", "mailto"),
            ("tel:+15551234567", "non_http", "tel"),
            ("file:///etc/passwd", "unsafe_scheme", "file"),
            ("data:text/plain;base64,SGVsbG8=", "unsafe_scheme", "data"),
            ("javascript:alert(1)", "unsafe_scheme", "javascript"),
            ("gopher://example.com/1", "unsafe_scheme", "gopher"),
            ("/relative/path", "ambiguous", None),
            ("https://", "ambiguous", "https"),
            ("https://[::1", "ambiguous", None),
        ],
    )
    async def test_url_status_classification(
        self,
        url: str,
        expected_status: str,
        expected_scheme: str | None,
    ) -> None:
        """Each URL class maps to the documented machine-readable status token."""
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            assert card["retrieval"]["url_status"] == expected_status
            assert card["retrieval"]["scheme"] == expected_scheme
            assert card["retrieval"]["eligible"] == (expected_status == "ok")
            if expected_status == "ok":
                assert card["retrieval"]["url_reason"] is None
            else:
                assert card["retrieval"]["url_reason"]
        finally:
            set_state(None)

    @pytest.mark.parametrize(
        ("url", "expected_status", "expected_reason"),
        [
            ("", "missing", "result has no URL to retrieve"),
            ("mailto:someone@example.com", "non_http", "scheme 'mailto' is not HTTP(S); not retrievable over HTTP"),
            ("file:///etc/passwd", "unsafe_scheme", "scheme 'file' is unsafe for downstream HTTP retrieval"),
            ("/relative/path", "ambiguous", "URL has no scheme; canonicalization is ambiguous"),
            ("https://", "ambiguous", "URL has no host; canonicalization is ambiguous"),
        ],
    )
    async def test_url_reasons_are_machine_readable(
        self,
        url: str,
        expected_status: str,
        expected_reason: str,
    ) -> None:
        """Ineligible URLs carry an explicit, stable reason on card and record."""
        state_obj = _build_state({"brave": _UrlEngine("brave", [url])})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            assert card["retrieval"]["url_status"] == expected_status
            assert card["retrieval"]["url_reason"] == expected_reason
            assert expanded["retrieval"]["url_status"] == expected_status
            assert expanded["retrieval"]["url_reason"] == expected_reason
        finally:
            set_state(None)

    async def test_ineligible_url_is_never_handed_off_as_fetch_target(self, state: McpState) -> None:
        """unsafe/missing/ambiguous URLs are not exposed via handoff.url.

        SlopSearX is not an SSRF-capable proxy: the handoff record only
        carries a fetch target when the URL is classified ``ok``.
        """
        state_obj = _build_state(
            {"brave": _UrlEngine("brave", ["", "file:///etc/passwd", "javascript:alert(1)", "/relative/path"])}
        )
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("hello", engines=["brave"])
            for card in result["results"]:
                expanded = await t.slopsearx_read_result(card["result_id"])
                assert expanded["retrieval"]["eligible"] is False
                assert expanded["retrieval"]["url"] is None
                assert expanded["retrieval"]["url_status"] in {"missing", "unsafe_scheme", "ambiguous"}
                assert expanded["retrieval"]["url_reason"]
        finally:
            set_state(None)

    async def test_ok_url_handed_off_verbatim(self, state: McpState) -> None:
        """An eligible URL is handed off verbatim, never rewritten or fetched."""
        result = await t.slopsearx_search("hello")
        card = _first_card(result)
        expanded = await t.slopsearx_read_result(card["result_id"])
        assert expanded["retrieval"]["url"] == "https://example.com/0"
        assert expanded["url"] == "https://example.com/0"

    def test_closed_url_status_vocabulary(self) -> None:
        """The runtime vocabulary matches the documented closed token set."""
        assert set(t.RETRIEVAL_URL_STATUSES) == URL_STATUS_VOCABULARY
        assert len(t.RETRIEVAL_URL_STATUSES) == len(set(t.RETRIEVAL_URL_STATUSES))
        assert "file" in t.UNSAFE_RETRIEVAL_SCHEMES
        assert "data" in t.UNSAFE_RETRIEVAL_SCHEMES
        assert "javascript" in t.UNSAFE_RETRIEVAL_SCHEMES


# ---------------------------------------------------------------------------
# Contract fixtures: search -> handoff -> downstream capture provenance
# ---------------------------------------------------------------------------


class TestContractFixtures:
    def test_search_handoff_capture_fixture_is_internally_consistent(self) -> None:
        """The composition fixture links the capture back without parsing prose."""
        data = json.loads((FIXTURES / "search_handoff_capture.json").read_text())

        handoff = data["handoff"]
        card = data["search"]["card"]
        capture_ref = data["capture"]["handoff_ref"]

        # The handoff record references the originating result identity.
        assert handoff["result_id"] == card["result_id"]
        assert handoff["url"] == card["url"]
        assert handoff["url_status"] == "ok"
        assert handoff["eligible"] is True
        assert handoff["verified"] is False
        assert handoff["provenance"]["snapshot_cursor"] == data["search"]["cursor"]
        assert handoff["provenance"]["query_id"] == data["search"]["query_id"]
        assert handoff["provenance"]["query"] == data["search"]["query"]

        # The downstream capture references the handoff provenance directly.
        assert capture_ref["contract"] == handoff["contract"]
        assert capture_ref["result_id"] == handoff["result_id"]
        assert capture_ref["url"] == handoff["url"]
        assert capture_ref["snapshot_cursor"] == handoff["provenance"]["snapshot_cursor"]
        assert capture_ref["query_id"] == handoff["provenance"]["query_id"]

    async def test_runtime_handoff_matches_ok_fixture_shape(self, state: McpState) -> None:
        """A live ok handoff has the same shape and values as the fixture."""
        fixture = json.loads((FIXTURES / "search_handoff_capture.json").read_text())["handoff"]

        result = await t.slopsearx_search("CVE-2024-1234 impact analysis", engines=["brave"], max_results=1)
        card = _first_card(result)
        expanded = await t.slopsearx_read_result(card["result_id"])
        handoff = expanded["retrieval"]

        assert set(handoff) == set(fixture)
        assert handoff["contract"] == fixture["contract"]
        assert handoff["version"] == fixture["version"]
        assert handoff["url_status"] == fixture["url_status"]
        assert handoff["url_reason"] == fixture["url_reason"]
        assert handoff["scheme"] == fixture["scheme"]
        assert handoff["eligible"] == fixture["eligible"]
        assert handoff["verified"] == fixture["verified"]
        assert handoff["verification_note"] == fixture["verification_note"]
        assert handoff["provenance"]["query"] == fixture["provenance"]["query"]
        assert set(handoff["provenance"]) == set(fixture["provenance"])

    async def test_runtime_unsafe_handoff_matches_fixture(self) -> None:
        """An unsafe URL at runtime produces exactly the fixture classification."""
        fixture = json.loads((FIXTURES / "ineligible_url_unsafe.json").read_text())
        state_obj = _build_state({"brave": _UrlEngine("brave", ["file:///etc/passwd"], content=SHORT_CONTENT)})
        set_state(state_obj)
        try:
            result = await t.slopsearx_search("local file disclosure", engines=["brave"])
            card = _first_card(result)
            expanded = await t.slopsearx_read_result(card["result_id"])
            handoff = expanded["retrieval"]

            assert handoff["contract"] == fixture["contract"]
            assert handoff["url_status"] == fixture["url_status"]
            assert handoff["url_reason"] == fixture["url_reason"]
            assert handoff["scheme"] == fixture["scheme"]
            assert handoff["eligible"] == fixture["eligible"]
            assert handoff["url"] == fixture["url"]  # null — never handed off
            assert handoff["snippet_only"] == fixture["snippet_only"]
            assert handoff["verified"] == fixture["verified"]
            assert handoff["provenance"]["query"] == fixture["provenance"]["query"]
        finally:
            set_state(None)
