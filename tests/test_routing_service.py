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

import time
from typing import Any

import pytest

import engines  # noqa: F401 — triggers @register_engine to populate the registry
from slopsearx.adapter import AdapterResponse, EngineAdapter, EngineStatus, SearchResult
from slopsearx.capabilities import CapabilityCatalog, EngineCapability, load_mcp_policy, observed_health_stale_seconds
from slopsearx.config import Config, EngineEntry
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
        media_types: tuple[str, ...] = (),
    ) -> None:
        super().__init__()
        self.name = name
        self._status = status
        self._count = count
        self.categories = list(categories or ["general"])
        self.supported_media_types = tuple(media_types)
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


class _MediaRoutingEngine(EngineAdapter):
    """Mock engine advertising media types for the media routing path."""

    def __init__(self, name: str, media_types: tuple[str, ...] = ()) -> None:
        super().__init__()
        self.name = name
        self.categories = ["general"]
        self.supported_media_types = tuple(media_types)

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        del query, params
        return AdapterResponse(results=[], status=EngineStatus.OK, latency_ms=1.0)


class TestMediaRoutingPath:
    """Issue 192 review: media-intent searches run the routing pass too.

    Image/video scopes seed from media-advertising engines and must then go
    through the same cost/coverage-aware selection pass as the sibling
    automatic paths: unauthenticated and circuit-open media engines are
    excluded before dispatch, and configured budget bounds shape the mix.
    """

    def test_media_search_excludes_unauthenticated_media_engine(self) -> None:
        resolver = ScopeResolver(
            active_engines={
                "image_a": _MediaRoutingEngine("image_a", ("image",)),
                "image_b": _MediaRoutingEngine("image_b", ("image",)),
                "wikipedia": _MockEngine("wikipedia"),
            },
            router=None,
            catalog=_FakeCatalog(
                {
                    "image_a": _cap("image_a"),
                    "image_b": _cap("image_b", auth_class="required", auth_configured=False),
                    "wikipedia": _cap("wikipedia"),
                }
            ),
        )
        decision = resolver.resolve(SearchRequest(query="q", media_type="image"))
        assert decision.routing_rule == "media type"
        assert decision.selected_engines == ["image_a"]
        stages = {e.engine: e.stage for e in decision.excluded_engines}
        assert stages["image_b"] == EXCLUSION_STAGE_AUTH
        # Media-fit exclusions carry a closed-vocabulary stage, never "unknown".
        assert stages["wikipedia"] == "policy"

    def test_media_search_excludes_circuit_open_media_engine(self) -> None:
        resolver = ScopeResolver(
            active_engines={
                "video_a": _MediaRoutingEngine("video_a", ("video",)),
                "video_b": _MediaRoutingEngine("video_b", ("video",)),
            },
            router=None,
            catalog=_FakeCatalog(
                {
                    "video_a": _cap("video_a", circuit_open=True),
                    "video_b": _cap("video_b"),
                }
            ),
        )
        decision = resolver.resolve(SearchRequest(query="q", media_type="video"))
        assert decision.routing_rule == "media type"
        assert decision.selected_engines == ["video_b"]
        assert any(e.engine == "video_a" and e.stage == EXCLUSION_STAGE_HEALTH for e in decision.excluded_engines)

    def test_media_search_respects_budget_bound(self) -> None:
        resolver = ScopeResolver(
            active_engines={
                "image_a": _MediaRoutingEngine("image_a", ("image",)),
                "image_b": _MediaRoutingEngine("image_b", ("image",)),
                "image_c": _MediaRoutingEngine("image_c", ("image",)),
            },
            router=None,
            catalog=_FakeCatalog(
                {
                    "image_a": _cap("image_a", cost_class="free"),
                    "image_b": _cap("image_b", cost_class="free"),
                    "image_c": _cap("image_c", cost_class="free"),
                }
            ),
            budget=RoutingBudget(max_engines=1),
        )
        decision = resolver.resolve(SearchRequest(query="q", media_type="image"))
        assert decision.routing_rule == "media type"
        assert len(decision.selected_engines) == 1
        assert decision.routing_budget_applied is True
        assert any(e.stage == EXCLUSION_STAGE_BUDGET for e in decision.excluded_engines)

    async def test_media_search_never_dispatches_excluded_engine(self) -> None:
        """An unauthenticated media engine is excluded before dispatch: it is
        absent from both the scope and the per-engine outcome rows."""
        engines_map = {
            "brave": _MockEngine("brave", media_types=("image",)),
            "duckduckgo": _MockEngine("duckduckgo", media_types=("image",)),
            "wikipedia": _MockEngine("wikipedia"),
        }
        ctx = AppContext(
            active_engines=engines_map,
            router=None,
            cache=_FakeStore(),
            tier1_engines=set(engines_map),
            sensitive_engines=set(),
            catalog=_FakeCatalog(
                {
                    "brave": _cap("brave", auth_class="required", auth_configured=False),
                    "duckduckgo": _cap("duckduckgo"),
                    "wikipedia": _cap("wikipedia"),
                }
            ),
            routing_budget=RoutingBudget(),
        )
        service = SearchService(ctx)
        response = await service.search(SearchRequest(query="q", media_type="image"))
        assert "brave" not in response.scope.selected_engines
        assert all(o.engine != "brave" for o in response.engine_outcomes)
        stages = {e.engine: e.stage for e in response.scope.excluded_engines}
        assert stages["brave"] == EXCLUSION_STAGE_AUTH


# ---------------------------------------------------------------------------
# SearchService — end-to-end with the real capability catalog
# ---------------------------------------------------------------------------


def _keyless_config(engine_names: list[str]) -> Config:
    """A pinned, keyless :class:`Config` for the routing tests.

    Mirrors ``slopsearx/mcp/harness.py::fixture_config()``: the routing pass
    derives authentication readiness from the config, so these tests must not
    inherit the ambient operator config (``load_config()`` reads ``ENGINE_*``
    env vars). A host with e.g. ``ENGINE_BRAVE_API_KEY`` set would make the
    fake brave engine look authenticated and flip routing assertions. Every
    fake engine gets an explicit empty key so fixture routing is
    deterministic in any environment.
    """
    cfg = Config()
    for name in engine_names:
        cfg.engines[name] = EngineEntry(api_key="")
    return cfg


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
        catalog=catalog
        if catalog is not None
        else CapabilityCatalog(config=_keyless_config(engine_names), adapters=engines_map),
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
# Cache scope freshness w.r.t. live routing state
# ---------------------------------------------------------------------------


class TestCacheScopeNotStale:
    """A cached scope is never reused under a different routing state.

    Issue 192 review: routing makes the scope a function of dynamic state
    (auth configuration, circuit state, catalog availability, routing
    budget), so the canonical cache key must fold those inputs in. Search
    once with an unauthenticated engine excluded (stage auth), flip the
    engine to authenticated, then search the same query again: the stale
    cache entry must not be served, and the fresh scope/results must reflect
    live routing.
    """

    async def test_scope_refreshes_after_auth_flip(self) -> None:
        engines_map = {
            "wikipedia": _MockEngine("wikipedia"),
            "brave": _MockEngine("brave"),
        }
        cfg = _keyless_config(list(engines_map))
        ctx = AppContext(
            active_engines=engines_map,
            router=None,
            cache=_FakeStore(),
            tier1_engines=set(engines_map),
            sensitive_engines=set(),
            catalog=CapabilityCatalog(config=cfg, adapters=engines_map),
            routing_budget=RoutingBudget(),
        )
        service = SearchService(ctx)
        request = SearchRequest(query="q")

        # Warm the cache with brave unauthenticated: excluded (stage auth).
        first = await service.search(request)
        assert first.cached is False
        assert "brave" not in first.scope.selected_engines
        assert any(e.engine == "brave" and e.stage == EXCLUSION_STAGE_AUTH for e in first.scope.excluded_engines)

        # Flip brave to authenticated and rebuild the catalog (a new catalog
        # object also rebuilds the scope resolver).
        cfg.engines["brave"].api_key = "secret"
        ctx.catalog = CapabilityCatalog(config=cfg, adapters=engines_map)

        # The routing-inputs digest changed, so the cached entry is no longer
        # reused: the fresh scope includes brave and results are live.
        second = await service.search(request)
        assert second.cached is False
        assert "brave" in second.scope.selected_engines
        assert not any(e.engine == "brave" for e in second.scope.excluded_engines)
        assert any(o.engine == "brave" and o.status == "ok" for o in second.engine_outcomes)

    async def test_scope_refreshes_after_observed_health_flip(self) -> None:
        """A cached scope is never served stale when observed health changes.

        Issue 192 review: the engine-count bound ranks engines by observed
        health (``_health_rank``), so a flip from ``ok`` to ``error`` changes
        which engines are selected under ``max_engines``. The routing-inputs
        digest must fold that health signal in: search once with arxiv
        observed healthy (selected under the count cap), flip its observed
        status to error, then re-search the same query and assert the stale
        cache entry is not served and the fresh scope reflects the health
        change.
        """
        engines_map = {
            "arxiv": _MockEngine("arxiv"),
            "wikipedia": _MockEngine("wikipedia"),
        }
        cfg = _keyless_config(list(engines_map))
        ctx = AppContext(
            active_engines=engines_map,
            router=None,
            cache=_FakeStore(),
            tier1_engines=set(),  # both tier-2: rank falls to health, then name
            sensitive_engines=set(),
            catalog=CapabilityCatalog(config=cfg, adapters=engines_map),
            routing_budget=RoutingBudget(max_engines=1),
        )
        service = SearchService(ctx)
        request = SearchRequest(query="q")

        # Warm the cache with both engines health-unknown; the count cap
        # prefers arxiv (stable name order breaks the health/cost tie), and
        # dispatch records arxiv as observed ``ok``.
        first = await service.search(request)
        assert first.cached is False
        assert first.scope.selected_engines == ["arxiv"]
        assert engines_map["arxiv"].last_observed_status == "ok"

        # Flip arxiv's observed status to error (fresh, not stale).
        engines_map["arxiv"].record_observation(EngineStatus.ERROR, latency_ms=1.0, result_count=0)

        # The routing-inputs digest now differs (observed health changed), so
        # the cached entry is not reused: arxiv is ranked last by health and
        # the count cap selects wikipedia instead.
        second = await service.search(request)
        assert second.cached is False
        assert second.scope.selected_engines == ["wikipedia"]
        assert any(e.engine == "arxiv" and e.stage == EXCLUSION_STAGE_BUDGET for e in second.scope.excluded_engines)

    async def test_scope_refreshes_when_health_goes_stale(self) -> None:
        """A stale observation invalidates the routing digest the same way.

        ``_health_rank`` treats a stale observation as unknown — never as
        current ``ok`` — so the digest must distinguish ``last_known_status``
        plus its staleness. Search once with arxiv observed healthy, age the
        observation past the freshness bound, re-search the same query, and
        assert the cached scope is not served stale.
        """
        engines_map = {
            "arxiv": _MockEngine("arxiv"),
            "wikipedia": _MockEngine("wikipedia"),
        }
        cfg = _keyless_config(list(engines_map))
        ctx = AppContext(
            active_engines=engines_map,
            router=None,
            cache=_FakeStore(),
            tier1_engines=set(),
            sensitive_engines=set(),
            catalog=CapabilityCatalog(config=cfg, adapters=engines_map),
            routing_budget=RoutingBudget(max_engines=1),
        )
        service = SearchService(ctx)
        request = SearchRequest(query="q")

        first = await service.search(request)
        assert first.cached is False
        assert first.scope.selected_engines == ["arxiv"]
        assert engines_map["arxiv"].last_observed_status == "ok"

        # Age arxiv's observation beyond the freshness bound so it reports
        # stale (treated as health-unknown, like wikipedia — name order then
        # decides, keeping arxiv selected).
        engines_map["arxiv"].last_observed_at = time.time() - observed_health_stale_seconds() - 1.0

        second = await service.search(request)
        assert second.cached is False
        assert second.scope.selected_engines == ["arxiv"]

    async def test_cache_hit_serves_scope_when_routing_state_is_unchanged(self) -> None:
        """With stable routing inputs the same query does hit the cache, and
        the served scope matches the routing state that produced it.

        The routing-inputs digest folds in observed health, so the health
        state is pinned before the first search: dispatching would otherwise
        record an observation (``unknown`` → ``ok``) that — as a routing-input
        change — correctly invalidates the entry for the immediate re-search.
        """
        engines_map = {"wikipedia": _MockEngine("wikipedia")}
        cfg = _keyless_config(list(engines_map))
        ctx = AppContext(
            active_engines=engines_map,
            router=None,
            cache=_FakeStore(),
            tier1_engines=set(engines_map),
            sensitive_engines=set(),
            catalog=CapabilityCatalog(config=cfg, adapters=engines_map),
            routing_budget=RoutingBudget(),
        )
        service = SearchService(ctx)
        request = SearchRequest(query="q")

        # Pin the observed-health signal so the first search's routing inputs
        # already equal the post-dispatch state; the re-search then sees
        # identical routing inputs and hits the cache.
        engines_map["wikipedia"].record_observation(EngineStatus.OK, latency_ms=2.0, result_count=2)

        first = await service.search(request)
        second = await service.search(request)

        assert second.cached is True
        assert first.scope.selected_engines == second.scope.selected_engines

    async def test_cache_hit_survives_health_staleness_without_count_cap(self) -> None:
        """Without an engine-count cap, a health staleness flip does not bust
        the cache.

        Issue 192 review: observed health only shapes the routed scope under
        ``max_engines > 0`` (``_priority_order`` is the sole consumer), so in
        the default permissive budget the routing-inputs digest must not fold
        the health/staleness signal in — folding it would invalidate the
        3600s cache entry every ~300s (the staleness window) with zero
        routing benefit. Search once, age the observation past the freshness
        bound so the only routing input that changed is the staleness flag,
        then re-search the same query: the cache entry must still hit.
        """
        engines_map = {
            "arxiv": _MockEngine("arxiv"),
            "wikipedia": _MockEngine("wikipedia"),
        }
        cfg = _keyless_config(list(engines_map))
        ctx = AppContext(
            active_engines=engines_map,
            router=None,
            cache=_FakeStore(),
            tier1_engines=set(engines_map),
            sensitive_engines=set(),
            catalog=CapabilityCatalog(config=cfg, adapters=engines_map),
            routing_budget=RoutingBudget(),  # default: no engine-count cap
        )
        service = SearchService(ctx)
        request = SearchRequest(query="q")

        first = await service.search(request)
        assert first.cached is False
        assert engines_map["arxiv"].last_observed_status == "ok"

        # Age the observation past the freshness bound so the catalog reports
        # it stale — the only routing input that changed between the searches.
        engines_map["arxiv"].last_observed_at = time.time() - observed_health_stale_seconds() - 1.0

        second = await service.search(request)
        assert second.cached is True
        assert second.scope.selected_engines == first.scope.selected_engines


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
    catalog = CapabilityCatalog(config=_keyless_config(engine_names), adapters=engines_map)
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

    async def test_explicit_targeted_envelope_marks_routing_bypassed(self) -> None:
        """Explicit-engine scopes mark the routing pass as bypassed
        (``applied: false``) instead of emitting a fallback/budget block that
        would misread as "the budget was evaluated and did not bite"."""
        state_obj = _build_mcp_state(budget=RoutingBudget(max_engines=1))
        set_state(state_obj)
        result = await t.slopsearx_search_targeted("hello", engines=["brave"])
        assert "error" not in result
        assert result["scope"]["routing_reason"] == "explicit engine"
        assert result["scope"]["routing"] == {"applied": False}

    async def test_automatic_envelope_marks_routing_applied(self) -> None:
        """Automatic scopes report the full routing block plus ``applied: true``."""
        state_obj = _build_mcp_state(budget=RoutingBudget(max_engines=2))
        set_state(state_obj)
        result = await t.slopsearx_search("hello")
        assert "error" not in result
        assert result["scope"]["routing"]["applied"] is True
        assert result["scope"]["routing"]["fallback"] is False
        assert isinstance(result["scope"]["routing"]["tradeoffs"], list)
