"""Misc-polish test coverage.

Adds dedicated, deterministic pytest nodes for the remaining specialist
source-resolution boundaries and the multi-tier / diagnostics invariants that
the flow validators surface through the MCP surface:

- VAL-SPEC-004 — jobs ``sources=[greenhouse,ashby,lever]`` resolves to exactly
  the declared ATS evidence boundary and nothing else.
- VAL-SPEC-006 — science ``source_types=[biomedical]`` resolves to exactly
  ``{pubmed, clinicaltrials, openfda}`` with the "resolved source_types"
  warning.
- VAL-SEARCH-018 — a multi-tier, distinct-score fake fixture exercises the
  ``tier_then_cross_engine_presence`` ordering over the MCP surface: all
  tier-1 cards precede tier-2 cards and scores are descending within a tier,
  even when a tier-2 result carries a higher cross-engine-presence score.
- VAL-DIAG-002 — the service version reported by the status tool is the
  authoritative installed package version (not a stale placeholder) and agrees
  with the health resource.
- VAL-DIAG-007 — cache / snapshot / job-store availability booleans agree with
  the reported Valkey state.

All tests are in-process and deterministic: no live engines, no Valkey.
"""

from __future__ import annotations

import datetime as _dt
import importlib.metadata as _metadata
from typing import Any

import pytest

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, MCPPolicy, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.mcp import server as _server
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.ratelimit import ValkeySlidingWindow
from slopsearx.research import ResearchJobRunner, ResearchJobStore
from slopsearx.service import AppContext, SearchService
from slopsearx.snapshot import SnapshotStore

RANKING = "tier_then_cross_engine_presence"


class _FakeStore:
    """In-memory key/value store (SearchCache-like) with controllable connectivity."""

    def __init__(self, connected: bool = True) -> None:
        self.is_connected = connected
        self._data: dict[str, dict[str, Any]] = {}

    async def get(self, key: str) -> dict[str, Any] | None:
        return self._data.get(key)

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        del ttl
        self._data[key] = value


class _FakeEngine(EngineAdapter):
    """Deterministic engine adapter producing scripted URLs (for overlap/tier tests)."""

    def __init__(
        self,
        name: str,
        *,
        count: int = 2,
        urls: list[str] | None = None,
        categories: list[str] | None = None,
    ) -> None:
        super().__init__()
        self.name = name
        self._count = count
        self._urls = urls
        self.categories = list(categories or ["general"])
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        self.calls += 1
        results = []
        for i in range(self._count):
            url = self._urls[i] if self._urls is not None else f"https://{self.name}{i}.example"
            results.append(
                SearchResult(
                    url=url,
                    title=f"{self.name} result {i}",
                    content=f"Content for {self.name} result {i}.",
                    engine=self.name,
                    score=float(self._count - i),
                    position=i + 1,
                    category=self.categories[0],
                )
            )
        return AdapterResponse(results=results, status=EngineStatus.OK, latency_ms=2.0)


def _make_engines(map: dict[str, list[str] | int] | list[str] | None = None) -> dict[str, EngineAdapter]:
    """Build fake engines from ``{name: urls}``, ``{name: count}``, or a list of names."""
    out: dict[str, EngineAdapter] = {}
    if isinstance(map, list):
        for name in map:
            out[name] = _FakeEngine(name, count=2)
        return out
    for name, spec in (map or {}).items():
        if isinstance(spec, int):
            out[name] = _FakeEngine(name, count=spec)
        else:
            out[name] = _FakeEngine(name, urls=spec)
    return out


def _build_state(
    engine_map: dict[str, EngineAdapter],
    *,
    tier1_engines: set[str] | None = None,
    version: str = "test",
    policy: MCPPolicy | None = None,
    store: _FakeStore | None = None,
    client_rate_window: Any = None,
) -> McpState:
    policy = policy or load_mcp_policy(config_path=None)
    store = store or _FakeStore()
    ctx = AppContext(
        active_engines=engine_map,
        router=None,
        cache=store,
        client_rate_window=client_rate_window,
        tier1_engines=tier1_engines if tier1_engines is not None else set(engine_map),
        sensitive_engines=policy.sensitive_engines,
    )
    catalog = CapabilityCatalog(config=load_config())
    service = SearchService(ctx)
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
        version=version,
    )


# ---------------------------------------------------------------------------
# VAL-SPEC-004 — jobs sources boundary
# ---------------------------------------------------------------------------


class TestJobsSourcesBoundary:
    async def test_jobs_sources_resolve_to_exactly_declared_set(self) -> None:
        """VAL-SPEC-004 — sources=[greenhouse,ashby,lever] selects exactly those ATS engines."""
        state = _build_state(_make_engines(["greenhouse", "ashby", "lever", "brave"]))
        state.policy.enabled_tools["jobs"] = True
        set_state(state)
        try:
            result = await t.slopsearx_search_jobs("Acme", sources=["greenhouse", "ashby", "lever"])

            assert "error" not in result
            assert set(result["scope"]["selected_engines"]) == {"greenhouse", "ashby", "lever"}
            # No engine outside the declared boundary may be dispatched.
            outcomes = {o["engine"] for o in result["engine_outcomes"]}
            assert outcomes <= {"greenhouse", "ashby", "lever"}
            # The ATS limitation note is present (VAL-SPEC-011 family note).
            assert any("no full job descriptions" in w for w in result["warnings"])
        finally:
            set_state(None)

    async def test_jobs_boundary_excludes_non_declared_active_engine(self) -> None:
        """VAL-SPEC-004 — an active non-ATS engine is never dispatched or selected."""
        state = _build_state(_make_engines(["greenhouse", "ashby", "lever", "brave"]))
        state.policy.enabled_tools["jobs"] = True
        set_state(state)
        try:
            result = await t.slopsearx_search_jobs("Acme", sources=["greenhouse", "lever"])
            assert "error" not in result
            assert set(result["scope"]["selected_engines"]) == {"greenhouse", "lever"}
            assert "ashby" not in result["scope"]["selected_engines"]
            assert "brave" not in result["scope"]["selected_engines"]
            outcomes = {o["engine"] for o in result["engine_outcomes"]}
            assert outcomes == {"greenhouse", "lever"}
        finally:
            set_state(None)


# ---------------------------------------------------------------------------
# VAL-SPEC-006 — science biomedical source-type resolution
# ---------------------------------------------------------------------------


class TestScienceBiomedicalResolution:
    async def test_biomedical_resolves_to_declared_profile(self) -> None:
        """VAL-SPEC-006 — source_types=[biomedical] -> {pubmed, clinicaltrials, openfda}."""
        state = _build_state(_make_engines(["pubmed", "clinicaltrials", "openfda", "arxiv"]))
        state.policy.enabled_tools["science"] = True
        set_state(state)
        try:
            result = await t.slopsearx_search_science("aspirin", source_types=["biomedical"])

            assert "error" not in result
            assert set(result["scope"]["selected_engines"]) == {"pubmed", "clinicaltrials", "openfda"}
            assert any("resolved source_types: biomedical" in w for w in result["warnings"])
            outcomes = {o["engine"] for o in result["engine_outcomes"]}
            assert outcomes <= {"pubmed", "clinicaltrials", "openfda"}
            assert "arxiv" not in outcomes
        finally:
            set_state(None)

    async def test_biomedical_profile_does_not_leak_other_source_types(self) -> None:
        """VAL-SPEC-006 — no engine outside the biomedical profile contributes results."""
        state = _build_state(_make_engines(["pubmed", "clinicaltrials", "openfda", "semanticscholar"]))
        state.policy.enabled_tools["science"] = True
        set_state(state)
        try:
            result = await t.slopsearx_search_science("gene", source_types=["biomedical"])
            assert "error" not in result
            assert set(result["scope"]["selected_engines"]) == {"pubmed", "clinicaltrials", "openfda"}
            assert "semanticscholar" not in result["scope"]["selected_engines"]
            for card in result["results"]:
                assert set(card["source_engines"]) <= {"pubmed", "clinicaltrials", "openfda"}
        finally:
            set_state(None)


# ---------------------------------------------------------------------------
# VAL-SEARCH-018 — multi-tier, distinct-score ordering at the MCP surface
# ---------------------------------------------------------------------------


class TestTierOrderingMcp:
    def _mixed_tier_state(self) -> McpState:
        """A tier-1 engine (brave) plus two tier-2 engines (github, pubmed).

        The two tier-2 engines share a URL, so that merged result carries a
        higher cross-engine-presence score (2.0) than any tier-1 result (1.0).
        This proves tier takes precedence over score, and scores are
        descending within a tier.
        """
        return _build_state(
            _make_engines(
                {
                    "brave": ["https://brave.example/0", "https://brave.example/1"],
                    "github": ["https://shared.example/0", "https://github.example/1"],
                    "pubmed": ["https://shared.example/0", "https://pubmed.example/1"],
                }
            ),
            tier1_engines={"brave"},
        )

    async def test_tier1_precedes_tier2_with_descending_score(self) -> None:
        """VAL-SEARCH-018 — all tier-1 results precede tier-2, scores descend within a tier."""
        state = self._mixed_tier_state()
        set_state(state)
        try:
            result = await t.slopsearx_search_targeted("hello", engines=["brave", "github", "pubmed"])

            assert "error" not in result
            assert result["meta"]["ranking"] == RANKING

            cards = result["results"]
            assert len(cards) == 5
            # Sanity: the shared tier-2 result really out-scores the tier-1 ones.
            scores = {c["url"]: c["score"] for c in cards}
            assert scores["https://shared.example/0"] > scores["https://brave.example/0"]

            # Every card carries numeric score, integer position, tier in {1,2}.
            for card in cards:
                assert isinstance(card["score"], (int, float))
                assert isinstance(card["position"], int)
                assert isinstance(card["tier"], int) and card["tier"] in (1, 2)

            tiers = [card["tier"] for card in cards]
            # All tier-1 entries precede all tier-2 entries.
            first_tier2 = tiers.index(2)
            assert all(t == 1 for t in tiers[:first_tier2])
            assert all(t == 2 for t in tiers[first_tier2:])
            # The high-score tier-2 result is still below every tier-1 result.
            assert cards[first_tier2]["url"] == "https://shared.example/0"

            # Scores are non-increasing within each tier.
            for tier in (1, 2):
                tier_scores = [c["score"] for c in cards if c["tier"] == tier]
                assert tier_scores == sorted(tier_scores, reverse=True), f"tier {tier} not descending"

            # positions are contiguous 1..N in the presented order.
            assert [c["position"] for c in cards] == list(range(1, len(cards) + 1))
        finally:
            set_state(None)

    async def test_generic_unscoped_all_tier2_orders_by_score(self) -> None:
        """VAL-SEARCH-018 — an unscoped all-tier-2 search still orders by descending score."""
        state = _build_state(
            _make_engines(
                {
                    "github": ["https://shared.example/0", "https://github.example/1"],
                    "pubmed": ["https://shared.example/0", "https://pubmed.example/1"],
                }
            ),
            tier1_engines=set(),  # no tier-1 engines → "all active engines" routing
        )
        set_state(state)
        try:
            result = await t.slopsearx_search("hello")
            assert "error" not in result
            cards = result["results"]
            assert cards, "expected results from all active tier-2 engines"
            assert all(c["tier"] == 2 for c in cards)
            scores = [c["score"] for c in cards]
            assert scores == sorted(scores, reverse=True)
            # The shared (presence-2) result ranks above the unique results.
            assert cards[0]["url"] == "https://shared.example/0"
        finally:
            set_state(None)


# ---------------------------------------------------------------------------
# VAL-DIAG-002 — version authority
# ---------------------------------------------------------------------------


class TestVersionAuthority:
    def test_package_version_resolves_to_installed_distribution(self) -> None:
        """VAL-DIAG-002 — _package_version() is the installed version, not a placeholder."""
        installed = _metadata.version("slopsearx")
        reported = _server._package_version()  # noqa: SLF001
        assert reported == installed
        assert reported not in ("0.0.0", "0.1.0", "")

    async def test_status_version_matches_installed_and_health_resource(self) -> None:
        """VAL-DIAG-002 — status tool and health resource agree on the authoritative version."""
        installed = _metadata.version("slopsearx")
        state = _build_state(_make_engines({"brave": 2}), version=installed)
        set_state(state)
        try:
            result = await t.slopsearx_get_service_status()
            assert result["version"] == installed
            assert result["version"] not in ("0.0.0", "0.1.0", "")

            health = t.service_diagnostics(state, now=_dt.datetime.now(_dt.timezone.utc).isoformat())
            assert health["version"] == result["version"] == installed
        finally:
            set_state(None)


# ---------------------------------------------------------------------------
# VAL-DIAG-007 — cache / snapshot / job availability agreement
# ---------------------------------------------------------------------------


class TestAvailabilityAgreement:
    def _valkey_window(self, connected: bool) -> ValkeySlidingWindow:
        window = ValkeySlidingWindow(fail_closed=True)
        window._connected = connected  # noqa: SLF001 — simulate a live/absent Valkey
        return window

    async def test_all_available_agree_when_valkey_connected(self) -> None:
        """VAL-DIAG-007 — connected Valkey implies cache/snapshot/job all available."""
        store = _FakeStore(connected=True)
        state = _build_state(
            _make_engines({"brave": 2}),
            store=store,
            client_rate_window=self._valkey_window(connected=True),
        )
        set_state(state)
        try:
            result = await t.slopsearx_get_service_status()
            assert result["valkey"]["connected"] is True
            assert result["cache_connected"] is True
            assert result["snapshots_available"] is True
            assert result["job_store_available"] is True
            # All availability booleans agree with the reported Valkey state.
            assert result["cache_connected"] == result["valkey"]["connected"]
            assert result["snapshots_available"] == result["valkey"]["connected"]
            assert result["job_store_available"] == result["valkey"]["connected"]
        finally:
            set_state(None)

    async def test_all_degraded_agree_when_valkey_unavailable(self) -> None:
        """VAL-DIAG-007 — absent Valkey honestly reflects degraded cache/snapshot/job availability."""
        store = _FakeStore(connected=False)
        state = _build_state(
            _make_engines({"brave": 2}),
            store=store,
            client_rate_window=self._valkey_window(connected=False),
        )
        set_state(state)
        try:
            result = await t.slopsearx_get_service_status()
            assert result["valkey"]["connected"] is False
            assert result["cache_connected"] is False
            assert result["snapshots_available"] is False
            assert result["job_store_available"] is False
            assert result["cache_connected"] == result["valkey"]["connected"]
            assert result["snapshots_available"] == result["valkey"]["connected"]
            assert result["job_store_available"] == result["valkey"]["connected"]
            # Degradation summary honestly lists the unavailable stores.
            causes = " ".join(result["degradation"]["causes"]).lower()
            assert "valkey" in causes and "cache" in causes and "snapshot" in causes and "job store" in causes
        finally:
            set_state(None)


@pytest.fixture(autouse=True)
def _clear_state_after():
    yield
    set_state(None)
