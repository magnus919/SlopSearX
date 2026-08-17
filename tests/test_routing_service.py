"""Integration tests for cost/coverage-aware routing (issue 192).

Covers the service-level wiring (``ScopeResolver`` + ``SearchService``) and
the MCP surface:

- automatic routing excludes unauthenticated and circuit-open engines before
  dispatch, with machine-readable exclusion stages;
- configured budget bounds shape the source mix and trade-offs are reported;
- explicit-engine and targeted-search behavior is preserved;
- missing telemetry (no catalog) produces the deterministic fallback;
- scope preview matches the executed scope; the routing explanation survives
  the cache round-trip.
"""

from __future__ import annotations

from typing import Any

import pytest

import engines  # noqa: F401 — triggers @register_engine to populate the registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, EngineCapability, load_mcp_policy
from slopsearx.config import load_config
from slopsearx.mcp import tools as t
from slopsearx.mcp.state import McpState, set_state
from slopsearx.research import ResearchJobRunner, ResearchJobStore
from slopsearx.routing import EXCLUSION_STAGE_AUTH, EXCLUSION_STAGE_BUDGET, EXCLUSION_STAGE_HEALTH, RoutingBudget
from slopsearx.service import (
    AppContext,
    ScopeResolver,
    SearchRequest,
    SearchResponse,
    SearchService,
    engine_outcome_from_dict,
    search_response_from_payload,
    search_response_to_payload,
)
from slopsearx.snapshot import SnapshotStore


class _FakeStore:
    """In-memory key-value store (SearchCache-like)."""

    def __init__(self) -> None:
        self.is_connected = True
        self._data: dict[str, dict[str, Any]] = {}

    async def get(self, key: str) -> dict[str, Any] | None:
        return self._data.get(key)

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        del ttl
        self._data[key] = value


class _MockEngine(EngineAdapter):
    """Parameterizable mock engine with a registry name."""

    def __init__(
        self,
        name: str,
        status: EngineStatus = EngineStatus.OK,
        count: int = 2,
        categories: list[str] | None = None,
    ) -> None:
        super().__init__()
        self.name = name
        self._status = status
        self._count = count
        self.categories = list(categories or ["general"])
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        self.calls += 1
        if self._status != EngineStatus.OK:
            return AdapterResponse(results=[], status=self._status, error_message="simulated failure", latency_ms=1.0)
        return AdapterResponse(
            results=[
                SearchResult(
                    url=f"https://{self.name}{i}.com",
                    title=f"{self.name} result {i}",
                    content=f"Content for {self.name} result {i}.",
                    engine=self.name,
                )
                for i in range(self._count)
            ],
            status=EngineStatus.OK,
            latency_ms=2.0,
        )


def _cap(
    name: str,
    *,
    auth_class: str = "none",
    auth_configured: bool = False,
    circuit_open: bool = False,
    cost_class: str = "free",
    status: str = "unknown",
    stale: bool = False,
) -> EngineCapability:
    return EngineCapability(
        name=name,
        display_name=name,
        engine_type="api",
        categories=["general"],
        enabled=True,
        auth_class=auth_class,
        auth_configured=auth_configured,
        scope_hints=[],
        caveats=[],
        cost_class=cost_class,
        last_known_status=status,
        last_known_status_stale=stale,
        circuit_open=circuit_open,
    )


class _FakeCatalog:
    """Duck-typed catalog: ``get(name) -> EngineCapability | None``."""

    def __init__(self, caps: dict[str, EngineCapability]) -> None:
        self._caps = caps

    def get(self, name: str) -> EngineCapability | None:
        return self._caps.get(name)


def _engines(names: list[str]) -> dict[str, EngineAdapter]:
    return {name: _MockEngine(name=name) for name in names}


# ---------------------------------------------------------------------------
# ScopeResolver — automatic routing with a catalog
# ---------------------------------------------------------------------------


class TestResolverAutomaticRouting:
    def test_excludes_unauthenticated_and_circuit_open_before_dispatch(self) -> None:
        resolver = ScopeResolver(
            active_engines=_engines(["eng_a", "eng_b", "eng_c"]),
            router=None,
            catalog=_FakeCatalog(
                {
                    "eng_a": _cap("eng_a"),
                    "eng_b": _cap("eng_b", auth_class="required", auth_configured=False),
                    "eng_c": _cap("eng_c", circuit_open=True),
                }
            ),
            budget=RoutingBudget(),
        )
        decision = resolver.resolve(SearchRequest(query="q"))
        assert decision.selected_engines == ["eng_a"]
        stages = {e.engine: e.stage for e in decision.excluded_engines}
        assert stages["eng_b"] == EXCLUSION_STAGE_AUTH
        assert stages["eng_c"] == EXCLUSION_STAGE_HEALTH
        # The fallback flag is only set when no catalog exists.
        assert decision.routing_fallback is False

    def test_missing_telemetry_keeps_engine(self) -> None:
        resolver = ScopeResolver(
            active_engines=_engines(["eng_a", "eng_unknown"]),
            router=None,
            catalog=_FakeCatalog({"eng_a": _cap("eng_a")}),
        )
        decision = resolver.resolve(SearchRequest(query="q"))
        assert set(decision.selected_engines) == {"eng_a", "eng_unknown"}

    def test_no_catalog_is_deterministic_fallback(self) -> None:
        resolver = ScopeResolver(active_engines=_engines(["eng_a", "eng_b"]), router=None)
        decision = resolver.resolve(SearchRequest(query="q"))
        assert set(decision.selected_engines) == {"eng_a", "eng_b"}
        assert decision.routing_fallback is True
        assert decision.excluded_engines == []

    def test_topic_match_path_applies_routing(self) -> None:
        from slopsearx.router import QueryRouter

        resolver = ScopeResolver(
            active_engines=_engines(["brave", "github", "stackexchange"]),
            router=QueryRouter(),
            tier1_engines={"brave"},
            catalog=_FakeCatalog(
                {
                    "brave": _cap("brave", auth_class="required", auth_configured=False, cost_class="freemium"),
                    "github": _cap("github", cost_class="free"),
                    "stackexchange": _cap("stackexchange", cost_class="free"),
                }
            ),
        )
        decision = resolver.resolve(SearchRequest(query="python async api docs"))
        assert decision.routing_rule == "topic match"
        assert decision.matched_topic == "code"
        assert "brave" not in decision.selected_engines
        assert any(e.engine == "brave" and e.stage == EXCLUSION_STAGE_AUTH for e in decision.excluded_engines)

    def test_budget_cap_reports_tradeoff(self) -> None:
        resolver = ScopeResolver(
            active_engines=_engines(["wikipedia", "arxiv", "paideng"]),
            router=None,
            catalog=_FakeCatalog(
                {
                    "wikipedia": _cap("wikipedia", cost_class="free"),
                    "arxiv": _cap("arxiv", cost_class="free"),
                    "paideng": _cap("paideng", cost_class="paid"),
                }
            ),
            budget=RoutingBudget(max_engines=2, max_cost_class="free", coverage_target=3),
        )
        decision = resolver.resolve(SearchRequest(query="q"))
        assert decision.selected_engines == ["wikipedia", "arxiv"]
        assert decision.routing_budget_applied is True
        assert any(e.stage == EXCLUSION_STAGE_BUDGET for e in decision.excluded_engines)
        assert any(tr.kind == "cost" for tr in decision.routing_tradeoffs)
        assert any(tr.kind == "coverage" for tr in decision.routing_tradeoffs)

    def test_explicit_engines_preserved(self) -> None:
        """Explicit-engine scope never enters the routing pass (issue 192)."""
        resolver = ScopeResolver(
            active_engines=_engines(["brave", "wikipedia"]),
            router=None,
            catalog=_FakeCatalog(
                {
                    "brave": _cap("brave", auth_class="required", auth_configured=False),
                    "wikipedia": _cap("wikipedia"),
                }
            ),
            budget=RoutingBudget(max_engines=1),
        )
        decision = resolver.resolve(SearchRequest(query="q", engines=["brave", "wikipedia"]))
        assert set(decision.selected_engines) == {"brave", "wikipedia"}
        assert decision.routing_rule == "explicit engine"
        assert decision.routing_budget_applied is False

    def test_explain_matches_resolve(self) -> None:
        resolver = ScopeResolver(
            active_engines=_engines(["eng_a", "eng_b"]),
            router=None,
            catalog=_FakeCatalog(
                {
                    "eng_a": _cap("eng_a"),
                    "eng_b": _cap("eng_b", auth_class="required", auth_configured=False),
                }
            ),
        )
        request = SearchRequest(query="q")
        assert resolver.explain(request).selected_engines == resolver.resolve(request).selected_engines


# ---------------------------------------------------------------------------
# SearchService — end-to-end with the real capability catalog
# ---------------------------------------------------------------------------


def _build_service(
    engine_names: list[str],
    *,
    budget: RoutingBudget | None = None,
    catalog: CapabilityCatalog | None = None,
) -> tuple[SearchService, AppContext]:
    engines_map = {name: _MockEngine(name=name) for name in engine_names}
    ctx = AppContext(
        active_engines=engines_map,
        router=None,
        cache=_FakeStore(),
        tier1_engines=set(engine_names),
        sensitive_engines=set(),
        catalog=catalog if catalog is not None else CapabilityCatalog(config=load_config(), adapters=engines_map),
        routing_budget=budget,
    )
    return SearchService(ctx), ctx


class TestServiceEndToEnd:
    async def test_automatic_search_excludes_unauthenticated_engine(self) -> None:
        service, _ctx = _build_service(["wikipedia", "brave", "duckduckgo"])
        response = await service.search(SearchRequest(query="q"))
        # brave requires a key that the test config does not configure.
        assert "brave" not in response.scope.selected_engines
        stages = {e.engine: e.stage for e in response.scope.excluded_engines}
        assert stages.get("brave") == EXCLUSION_STAGE_AUTH
        assert response.scope.routing_fallback is False
        # No outcome row for the never-dispatched engine.
        assert all(o.engine != "brave" for o in response.engine_outcomes)

    async def test_circuit_open_engine_excluded_from_scope(self) -> None:
        service, ctx = _build_service(["wikipedia", "brave"])
        # Wikipedia needs no credentials, so the only exclusion signal is its
        # open circuit — the health stage, not the auth stage.
        ctx.active_engines["wikipedia"].circuit_open_until = 10**12
        response = await service.search(SearchRequest(query="q"))
        assert "wikipedia" not in response.scope.selected_engines
        assert any(
            e.engine == "wikipedia" and e.stage == EXCLUSION_STAGE_HEALTH for e in response.scope.excluded_engines
        )

    async def test_budget_shapes_scope_and_reports_tradeoff(self) -> None:
        # All three engines are auth-ready and free; the engine-count bound
        # is the only constraint, so the cap itself is the budget trade-off.
        service, _ctx = _build_service(
            ["wikipedia", "arxiv", "openalex"],
            budget=RoutingBudget(max_engines=1),
        )
        response = await service.search(SearchRequest(query="q"))
        assert len(response.scope.selected_engines) == 1
        assert response.scope.routing_budget_applied is True
        assert any(tr.kind == "cost" for tr in response.scope.routing_tradeoffs)
        assert any("budget" in w for w in response.scope.warnings)

    async def test_explicit_engines_bypass_routing(self) -> None:
        service, _ctx = _build_service(["brave", "wikipedia"], budget=RoutingBudget(max_engines=1))
        response = await service.search(SearchRequest(query="q", engines=["brave"]))
        assert response.scope.selected_engines == ["brave"]
        assert response.scope.routing_rule == "explicit engine"
        assert response.scope.routing_budget_applied is False


# ---------------------------------------------------------------------------
# Cache round-trip of the routing explanation
# ---------------------------------------------------------------------------


class TestCacheRoundTrip:
    def test_routing_explanation_survives_payload_round_trip(self) -> None:
        from slopsearx.routing import RoutingTradeoff
        from slopsearx.service import EngineExclusion, ScopeDecision

        scope = ScopeDecision(
            selected_engines=["wikipedia"],
            routing_rule="all active engines",
            warnings=["routing trade-off: cost — bounded"],
            excluded_engines=[
                EngineExclusion(engine="dehashed", reason="exceeds budget", stage="budget"),
            ],
            routing_fallback=False,
            routing_budget_applied=True,
            routing_tradeoffs=[RoutingTradeoff(kind="cost", detail="coverage is bounded")],
        )
        response = SearchResponse(
            query="q",
            results=[],
            scope=scope,
            engine_outcomes=[engine_outcome_from_dict({"engine": "wikipedia", "status": "ok", "result_count": 1})],
        )
        rebuilt = search_response_from_payload(search_response_to_payload(response))
        assert rebuilt.scope.routing_fallback is False
        assert rebuilt.scope.routing_budget_applied is True
        assert rebuilt.scope.routing_tradeoffs[0].kind == "cost"
        assert rebuilt.scope.routing_tradeoffs[0].detail == "coverage is bounded"
        assert rebuilt.scope.excluded_engines[0].stage == "budget"
        assert rebuilt.scope.excluded_engines[0].engine == "dehashed"

    def test_legacy_payload_rehydrates_with_unknown_stage(self) -> None:
        from slopsearx.service import ScopeDecision

        scope = ScopeDecision(
            selected_engines=["wikipedia"],
            routing_rule="explicit category",
            excluded_engines=[],
        )
        payload = search_response_to_payload(SearchResponse(query="q", results=[], scope=scope, engine_outcomes=[]))
        # Simulate a payload written by a pre-issue-192 replica.
        payload["scope"]["excluded_engines"] = [{"engine": "hibp", "reason": "sensitive"}]
        payload["scope"].pop("routing_fallback")
        payload["scope"].pop("routing_budget_applied")
        payload["scope"].pop("routing_tradeoffs")
        rebuilt = search_response_from_payload(payload)
        assert rebuilt.scope.excluded_engines[0].stage == "unknown"
        assert rebuilt.scope.routing_fallback is False
        assert rebuilt.scope.routing_tradeoffs == []


# ---------------------------------------------------------------------------
# MCP surface — scope preview and search envelope
# ---------------------------------------------------------------------------


def _build_mcp_state(
    engine_names: list[str] | None = None,
    *,
    budget: RoutingBudget | None = None,
) -> McpState:
    engine_names = engine_names or ["wikipedia", "brave", "duckduckgo"]
    engines_map = {name: _MockEngine(name=name) for name in engine_names}
    policy = load_mcp_policy(config_path=None)
    ctx = AppContext(
        active_engines=engines_map,
        router=None,
        cache=_FakeStore(),
        tier1_engines=set(engine_names),
        sensitive_engines=policy.sensitive_engines,
    )
    catalog = CapabilityCatalog(config=load_config(), adapters=engines_map)
    ctx.catalog = catalog
    ctx.routing_budget = budget
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


class TestMcpRoutingSurface:
    @pytest.fixture(autouse=True)
    def _clear_state(self) -> Any:
        yield
        set_state(None)

    async def test_explain_scope_reports_routing_and_stages(self) -> None:
        state_obj = _build_mcp_state(budget=RoutingBudget(max_engines=2))
        set_state(state_obj)
        result = await t.slopsearx_explain_search_scope("hello", intent="auto")
        assert "error" not in result
        assert "routing" in result
        assert result["routing"]["fallback"] is False
        assert "budget_applied" in result["routing"]
        assert "tradeoffs" in result["routing"]
        excluded = {e["engine"]: e for e in result["excluded_engines"]}
        assert excluded["brave"]["stage"] == "auth"
        assert "wikipedia" in result["selected_engines"]

    async def test_explain_matches_executed_scope_with_catalog(self) -> None:
        state_obj = _build_mcp_state(budget=RoutingBudget(max_engines=2))
        set_state(state_obj)
        preview = await t.slopsearx_explain_search_scope("hello", intent="auto")
        search = await t.slopsearx_search("hello", intent="auto")
        assert "error" not in preview
        assert "error" not in search
        assert set(preview["selected_engines"]) == set(search["scope"]["selected_engines"])
        assert preview["routing_reason"] == search["scope"]["routing_reason"]
        assert preview["routing"] == search["scope"]["routing"]
        assert preview["excluded_engines"] == search["scope"]["excluded_engines"]

    async def test_search_envelope_surfaces_routing_block(self) -> None:
        state_obj = _build_mcp_state(budget=RoutingBudget(max_engines=2))
        set_state(state_obj)
        result = await t.slopsearx_search("hello")
        assert "error" not in result
        assert result["scope"]["routing"]["fallback"] is False
        assert isinstance(result["scope"]["routing"]["tradeoffs"], list)
        # Brave is unauthenticated in this config → excluded before dispatch
        # and absent from engine outcomes (never dispatched).
        assert all(o["engine"] != "brave" for o in result["engine_outcomes"])
        assert any(e["stage"] == "auth" for e in result["scope"]["excluded_engines"])

    async def test_preview_without_catalog_is_deterministic_fallback(self) -> None:
        state_obj = _build_mcp_state()
        state_obj.ctx.catalog = None  # simulate telemetry-unavailable
        set_state(state_obj)
        result = await t.slopsearx_explain_search_scope("hello", intent="auto")
        assert result["routing"]["fallback"] is True
        assert set(result["selected_engines"]) == {"wikipedia", "brave", "duckduckgo"}

    async def test_explicit_targeted_search_ignores_routing(self) -> None:
        state_obj = _build_mcp_state(budget=RoutingBudget(max_engines=1))
        set_state(state_obj)
        result = await t.slopsearx_search_targeted("hello", engines=["brave"])
        assert "error" not in result
        assert result["scope"]["selected_engines"] == ["brave"]
