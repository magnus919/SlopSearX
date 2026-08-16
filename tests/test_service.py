"""Characterization tests for the normalized search service.

Covers scope resolution precedence, cache scoping, partial/all-failure
semantics, and the HTTP/MCP-shared pipeline in slopsearx.service.
These tests lock in the behavior the HTTP route used to own so the MCP
surface can rely on the same semantics.
"""

from __future__ import annotations

import asyncio
from typing import Any

import pytest

from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
)
from slopsearx.cache import cache_key
from slopsearx.ratelimit import RateLimitStrategy
from slopsearx.router import QueryRouter
from slopsearx.service import (
    AppContext,
    EngineExclusion,
    EngineOutcome,
    QueryValidationError,
    RateLimitExceededError,
    ScopeDecision,
    ScopeResolver,
    SearchRequest,
    SearchResponse,
    SearchService,
    engine_outcome_from_dict,
    search_response_from_payload,
    search_response_to_payload,
    search_result_from_dict,
)

# ---------------------------------------------------------------------------
# Mock engines
# ---------------------------------------------------------------------------


class _OkEngine(EngineAdapter):
    """Engine returning a fixed number of results."""

    name = "okeng"
    display_name = "OK Engine"
    env_prefix = "ENGINE_OK"
    engine_type = "api"
    categories = ["general"]

    def __init__(self, count: int = 3, delay: float = 0.0) -> None:
        super().__init__()
        self.count = count
        self.delay = delay
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        self.calls += 1
        if self.delay > 0:
            await asyncio.sleep(self.delay)
        return AdapterResponse(
            results=[
                SearchResult(
                    url=f"https://{self.name}{i}.com",
                    title=f"{self.name} result {i}",
                    content=f"Content for {self.name} result {i}.",
                    engine=self.name,
                )
                for i in range(self.count)
            ],
            status=EngineStatus.OK,
            latency_ms=5.0,
        )


class _FailEngine(EngineAdapter):
    """Engine returning a fixed failure status."""

    name = "faileng"
    display_name = "Fail Engine"
    env_prefix = "ENGINE_FAIL"
    engine_type = "api"
    categories = ["general"]

    def __init__(self, status: EngineStatus = EngineStatus.ERROR, message: str = "boom") -> None:
        super().__init__()
        self.status = status
        self.message = message
        self.calls = 0

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        self.calls += 1
        return AdapterResponse(results=[], status=self.status, error_message=self.message, latency_ms=2.0)


class _ExplodingEngine(EngineAdapter):
    """Engine that raises — must be classified as an error, never propagate."""

    name = "boomeng"
    categories = ["general"]

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        raise RuntimeError("Client error '403 Forbidden' for url 'https://api.example.com/search?key=secret&q=x'")


class _CircuitOpenEngine(EngineAdapter):
    """Engine whose circuit breaker is already open."""

    name = "circuiteng"
    categories = ["general"]

    def circuit_allowed(self) -> bool:
        return False

    async def search(self, query: str, params: dict[str, Any] | None = None) -> AdapterResponse:
        raise AssertionError("circuit-open engines must never be dispatched")


# ---------------------------------------------------------------------------
# Fake cache
# ---------------------------------------------------------------------------


class _FakeCache:
    """Minimal in-memory SearchCache stand-in."""

    def __init__(self, connected: bool = True) -> None:
        self.is_connected = connected
        self._data: dict[str, dict[str, Any]] = {}

    async def get(self, key: str) -> dict[str, Any] | None:
        if not self.is_connected:
            return None
        return self._data.get(key)

    async def set(self, key: str, value: dict[str, Any], ttl: int = 300) -> None:
        del ttl
        if self.is_connected:
            self._data[key] = value


class _DenyRateLimiter(RateLimitStrategy):
    """Rate limiter that always denies."""

    def __init__(self, deny: bool = True) -> None:
        self._deny = deny
        self.keys: list[str] = []

    async def acquire(self, engine: str, cost: int = 1) -> bool:
        del cost
        self.keys.append(engine)
        return not self._deny

    async def warmup(self) -> None:
        pass

    async def shutdown(self) -> None:
        pass


def _context(
    engines: dict[str, EngineAdapter] | None = None,
    *,
    router: QueryRouter | None = None,
    cache: _FakeCache | None = None,
    rate_window: RateLimitStrategy | None = None,
    tier1: set[str] | None = None,
    sensitive: set[str] | None = None,
) -> AppContext:
    return AppContext(
        active_engines=engines or {},
        router=router,
        cache=cache,
        client_rate_window=rate_window,
        tier1_engines=tier1 or set(),
        sensitive_engines=sensitive if sensitive is not None else set(),
    )


def _service(**kwargs: Any) -> SearchService:
    return SearchService(_context(**kwargs))


def _req(**overrides: Any) -> SearchRequest:
    defaults: dict[str, Any] = {"query": "test query"}
    defaults.update(overrides)
    return SearchRequest(**defaults)


# ---------------------------------------------------------------------------
# ScopeResolver precedence
# ---------------------------------------------------------------------------


class TestScopeResolver:
    def test_explicit_engines_win_over_categories(self) -> None:
        a = _OkEngine()
        b = _OkEngine()
        a.name, b.name = "eng_a", "eng_b"
        a.categories, b.categories = ["news"], ["science"]
        resolver = ScopeResolver(active_engines={"eng_a": a, "eng_b": b}, router=None)

        decision = resolver.resolve(_req(engines=["eng_b"], categories=["news"]))

        assert decision.selected_engines == ["eng_b"]
        assert decision.routing_rule == "explicit engine"

    def test_explicit_engines_drop_unknown_with_warning(self) -> None:
        a = _OkEngine()
        a.name = "eng_a"
        resolver = ScopeResolver(active_engines={"eng_a": a}, router=None)

        decision = resolver.resolve(_req(engines=["eng_a", "ghost"]))

        assert decision.selected_engines == ["eng_a"]
        assert decision.routing_rule == "explicit engine"
        assert any("ghost" in w for w in decision.warnings)
        assert decision.excluded_engines[0].engine == "ghost"

    def test_category_filter_is_or(self) -> None:
        a, b = _OkEngine(), _OkEngine()
        a.name, b.name = "eng_a", "eng_b"
        a.categories, b.categories = ["news"], ["science"]
        resolver = ScopeResolver(active_engines={"eng_a": a, "eng_b": b}, router=None)

        decision = resolver.resolve(_req(categories=["science"]))

        assert decision.selected_engines == ["eng_b"]
        assert decision.resolved_categories == ["science"]
        assert decision.routing_rule == "explicit category"

    def test_sensitive_engines_excluded_from_categories(self) -> None:
        hibp = _OkEngine()
        hibp.name = "hibp"
        hibp.categories = ["general", "security"]
        resolver = ScopeResolver(
            active_engines={"hibp": hibp, "brave": _OkEngine()},
            router=None,
            sensitive_engines={"hibp"},
        )

        decision = resolver.resolve(_req(categories=["security"]))

        assert "hibp" not in decision.selected_engines
        assert any(ex.engine == "hibp" for ex in decision.excluded_engines)

    def test_sensitive_engines_reachable_via_explicit_list(self) -> None:
        hibp = _OkEngine()
        hibp.name = "hibp"
        resolver = ScopeResolver(
            active_engines={"hibp": hibp},
            router=None,
            sensitive_engines={"hibp"},
        )

        decision = resolver.resolve(_req(engines=["hibp"]))

        assert decision.selected_engines == ["hibp"]

    def test_topic_match_reports_topic(self) -> None:
        resolver = ScopeResolver(
            active_engines={"wikipedia": _OkEngine(), "github": _OkEngine(), "brave": _OkEngine()},
            router=QueryRouter(),
            tier1_engines={"wikipedia", "brave"},
        )

        decision = resolver.resolve(_req(query="python async api docs"))

        assert decision.routing_rule == "topic match"
        assert decision.matched_topic == "code"

    def test_tier1_fallback_when_no_topic_matches(self) -> None:
        resolver = ScopeResolver(
            active_engines={"wikipedia": _OkEngine(), "github": _OkEngine(), "brave": _OkEngine()},
            router=QueryRouter(),
            tier1_engines={"wikipedia", "brave"},
        )

        decision = resolver.resolve(_req(query="zzzz no topic here"))

        assert decision.routing_rule == "tier-1 fallback"
        assert set(decision.selected_engines) == {"wikipedia", "brave"}

    def test_fallback_to_all_when_no_tier1_active(self) -> None:
        resolver = ScopeResolver(
            active_engines={"github": _OkEngine()},
            router=QueryRouter(),
            tier1_engines={"wikipedia", "brave"},  # none active
        )

        decision = resolver.resolve(_req(query="zzzz no topic here"))

        assert decision.routing_rule == "all active engines"
        assert decision.selected_engines == ["github"]

    def test_no_router_uses_all_active_engines(self) -> None:
        resolver = ScopeResolver(
            active_engines={"eng_a": _OkEngine(), "eng_b": _OkEngine()},
            router=None,
        )

        decision = resolver.resolve(_req())

        assert decision.routing_rule == "all active engines"
        assert set(decision.selected_engines) == {"eng_a", "eng_b"}


# ---------------------------------------------------------------------------
# QueryRouter.match_topic
# ---------------------------------------------------------------------------


class TestMatchTopic:
    def test_matches_first_topic(self) -> None:
        router = QueryRouter()
        assert router.match_topic("python async") == "code"

    def test_no_match_returns_none(self) -> None:
        router = QueryRouter()
        assert router.match_topic("zzzz nothing matches") is None

    def test_disabled_returns_none(self) -> None:
        router = QueryRouter(routing_config={"enabled": False})
        assert router.match_topic("python") is None

    def test_config_dict_topics_get_names(self) -> None:
        router = QueryRouter(
            routing_config={
                "topics": {
                    "custom": {"keywords": ["unique-word"], "engines": ["brave"]},
                }
            }
        )
        assert router.match_topic("unique-word stuff") == "custom"


# ---------------------------------------------------------------------------
# SearchService — validation and empty cases
# ---------------------------------------------------------------------------


class TestValidation:
    async def test_empty_query_raises(self) -> None:
        with pytest.raises(QueryValidationError):
            await _service().search(_req(query=""))

    async def test_whitespace_query_raises(self) -> None:
        with pytest.raises(QueryValidationError):
            await _service().search(_req(query="   "))

    async def test_no_engines_returns_all_unresponsive(self) -> None:
        response = await _service().search(_req())

        assert response.all_unresponsive is True
        assert response.results == []
        assert response.engine_outcomes == []
        assert response.query_id.startswith("ssx-")

    async def test_no_engines_skips_rate_limiter(self) -> None:
        limiter = _DenyRateLimiter(deny=True)
        response = await _service(rate_window=limiter).search(_req())

        assert response.all_unresponsive is True
        assert limiter.keys == []


# ---------------------------------------------------------------------------
# SearchService — dispatch semantics
# ---------------------------------------------------------------------------


class TestDispatch:
    async def test_basic_search_returns_tiered_results(self) -> None:
        ok = _OkEngine(count=2)
        service = _service(engines={"okeng": ok}, tier1={"okeng"})

        response = await service.search(_req())

        assert response.all_unresponsive is False
        assert len(response.results) == 2
        assert all(r.tier == 1 for r in response.results)
        assert response.engine_outcomes == [
            EngineOutcome(engine="okeng", status="ok", result_count=2, latency_ms=5.0, message=None)
        ]

    async def test_partial_failure_marks_partial(self) -> None:
        ok = _OkEngine()
        fail = _FailEngine(EngineStatus.TIMEOUT, "slow")
        service = _service(engines={"okeng": ok, "faileng": fail})

        response = await service.search(_req())

        assert response.partial is True
        assert response.all_unresponsive is False
        statuses = {o.engine: o.status for o in response.engine_outcomes}
        assert statuses == {"okeng": "ok", "faileng": "timeout"}
        assert len(response.results) == 3

    async def test_all_failure_marks_all_unresponsive(self) -> None:
        service = _service(engines={"faileng": _FailEngine()})

        response = await service.search(_req())

        assert response.all_unresponsive is True
        assert response.partial is False
        assert response.results == []
        assert response.engine_outcomes[0].status == "error"
        assert response.engine_outcomes[0].message == "boom"

    async def test_raising_engine_is_classified_and_sanitized(self) -> None:
        service = _service(engines={"boomeng": _ExplodingEngine()})

        response = await service.search(_req())

        assert response.all_unresponsive is True
        outcome = response.engine_outcomes[0]
        assert outcome.status == "error"
        assert "secret" not in (outcome.message or "")
        assert outcome.message == "Client error '403 Forbidden' for url 'https://api.example.com/search?q=x%27"

    async def test_circuit_open_engine_reported_as_error(self) -> None:
        engine = _CircuitOpenEngine()
        service = _service(engines={"circuiteng": engine})

        response = await service.search(_req())

        assert response.all_unresponsive is True
        assert response.engine_outcomes[0].status == "error"
        assert response.engine_outcomes[0].message == "circuit open"

    async def test_max_results_is_a_presentation_bound(self) -> None:
        service = _service(engines={"okeng": _OkEngine(count=5)})

        response = await service.search(_req(max_results=2))

        assert len(response.results) == 2
        # Positions remain the global ranked positions, not renumbered
        assert [r.position for r in response.results] == [1, 2]

    async def test_include_filters_engine_outcomes(self) -> None:
        service = _service(engines={"okeng": _OkEngine()})

        response = await service.search(_req(include={"results"}))

        assert response.engine_outcomes == []
        assert response.partial is False

    async def test_dispatch_honors_engine_timeout_ms(self) -> None:
        """An engine configured with timeout_ms > the 3s default is not killed early.

        Regression: the dispatcher previously hardcoded a 3s ceiling and never
        consulted the engine's configured timeout_ms, so slow-but-legit engines
        (e.g. Internet Archive Wayback CDX) always surfaced as TIMEOUT.
        """
        slow = _OkEngine(delay=10.5)
        slow.config = {"timeout_ms": 11_000}
        service = _service(engines={"okeng": slow}, tier1={"okeng"})

        response = await service.search(_req())

        assert response.all_unresponsive is False
        assert response.engine_outcomes[0].status == "ok"

    async def test_dispatch_falls_back_to_default_timeout(self) -> None:
        """An engine without a configured timeout still uses the 3s default."""
        slow = _OkEngine(delay=4.0)  # config defaults to {} → no timeout_ms
        service = _service(engines={"okeng": slow})

        response = await service.search(_req())

        assert response.all_unresponsive is True
        assert response.engine_outcomes[0].status == "timeout"

    async def test_dispatch_never_raises_on_bad_timeout_ms(self) -> None:
        """A non-numeric timeout_ms falls back to the default instead of raising."""
        slow = _OkEngine(delay=0.0)
        slow.config = {"timeout_ms": "not-a-number"}
        service = _service(engines={"okeng": slow}, tier1={"okeng"})

        response = await service.search(_req())

        assert response.all_unresponsive is False
        assert response.engine_outcomes[0].status == "ok"

    async def test_overall_deadline_caps_fanout(self) -> None:
        """A slow engine is cut at the overall deadline, not allowed to hang the search."""
        service = _service(engines={"okeng": _OkEngine()})
        started = asyncio.Event()

        async def _started_slow() -> AdapterResponse:
            started.set()
            await asyncio.sleep(30.0)
            raise AssertionError("engine unexpectedly completed")

        task = asyncio.create_task(_started_slow())
        await started.wait()
        response = await service._gather_with_deadline(
            [task], ["okeng"], deadline_s=0.01, started_engines={"okeng"}
        )

        assert response[0].status.value == "timeout"
        assert task.done()

    async def test_semaphore_wait_timeout_is_not_reported_as_engine_timeout(self) -> None:
        """An engine that never starts is unavailable, not an upstream timeout."""
        service = _service(engines={"okeng": _OkEngine()})
        started = {"okeng"}

        async def _pending() -> AdapterResponse:
            await asyncio.sleep(60)
            raise AssertionError("pending task unexpectedly completed")

        first = asyncio.create_task(_pending())
        second = asyncio.create_task(_pending())
        results = await service._gather_with_deadline(
            [first, second], ["okeng", "second"], deadline_s=0.01, started_engines=started
        )

        assert results[0].status.value == "timeout"
        assert results[1].status.value == "unavailable"
        assert results[1].latency_ms == 0.0

    async def test_unavailable_engine_does_not_reset_circuit_breaker(self) -> None:
        """Scheduler-unavailable engines do not receive a synthetic success."""
        engine = _OkEngine()
        engine.consecutive_errors = 4
        service = _service(engines={"okeng": engine})

        async def _pending() -> AdapterResponse:
            await asyncio.sleep(60)
            raise AssertionError("pending task unexpectedly completed")

        await service._gather_with_deadline(
            [asyncio.create_task(_pending())],
            ["okeng"],
            deadline_s=0.01,
            started_engines=set(),
        )
        result = AdapterResponse(results=[], status=EngineStatus.UNAVAILABLE)
        if result.status == EngineStatus.OK:
            engine.record_success()
        assert engine.consecutive_errors == 4

    async def test_overall_deadline_drains_cancelled_tasks(self) -> None:
        """Deadline cancellation drains child tasks before returning the response."""
        service = _service(engines={"okeng": _OkEngine()})
        started = asyncio.Event()

        async def _started_slow() -> AdapterResponse:
            started.set()
            await asyncio.sleep(30.0)
            raise AssertionError("engine unexpectedly completed")

        task = asyncio.create_task(_started_slow())
        await started.wait()
        response = await service._gather_with_deadline(
            [task], ["okeng"], deadline_s=0.01, started_engines={"okeng"}
        )
        assert response[0].status.value == "timeout"
        assert task.done()

    async def test_raising_engine_isolated_with_deadline(self) -> None:
        """A raising engine is classified as ERROR, not allowed to fail the whole search."""
        ok = _OkEngine(count=2)
        boom = _ExplodingEngine()
        service = _service(engines={"okeng": ok, "boomeng": boom}, tier1={"okeng"})

        response = await service.search(_req())

        assert response.all_unresponsive is False
        statuses = {o.engine: o.status for o in response.engine_outcomes}
        assert statuses == {"okeng": "ok", "boomeng": "error"}
        assert len(response.results) == 2

    async def test_gather_isolates_baseexception_from_task_result(self) -> None:
        """task.result() re-raises stored exceptions; _gather_with_deadline must isolate them."""
        service = _service(engines={"okeng": _OkEngine()})

        async def _boom() -> AdapterResponse:
            raise asyncio.CancelledError()  # escapes `except Exception` in _dispatch_engine

        task = asyncio.create_task(_boom())
        results = await service._gather_with_deadline([task], ["boomeng"])

        assert len(results) == 1
        assert results[0].status == EngineStatus.ERROR
        assert "boomeng" not in results[0].results or results[0].results == []

    async def test_gather_cancellation_cancels_child_tasks(self) -> None:
        """Cancelling fan-out must not leave engine tasks running in the background."""
        service = _service(engines={"okeng": _OkEngine()})
        started = asyncio.Event()
        cancelled = asyncio.Event()

        async def _slow() -> AdapterResponse:
            started.set()
            try:
                await asyncio.sleep(60)
            except asyncio.CancelledError:
                cancelled.set()
                raise
            raise AssertionError("slow task unexpectedly completed")

        task = asyncio.create_task(_slow())
        await started.wait()
        gather_task = asyncio.create_task(service._gather_with_deadline([task], ["sloweng"]))
        await asyncio.sleep(0)
        gather_task.cancel()

        with pytest.raises(asyncio.CancelledError):
            await gather_task
        assert cancelled.is_set()
        assert task.done()


# ---------------------------------------------------------------------------
# SearchService — rate limiting
# ---------------------------------------------------------------------------


class TestRateLimit:
    async def test_denied_raises_rate_limit_exceeded(self) -> None:
        service = _service(
            engines={"okeng": _OkEngine()},
            rate_window=_DenyRateLimiter(deny=True),
        )

        with pytest.raises(RateLimitExceededError):
            await service.search(_req(client_identifier="10.0.0.1"))

    async def test_no_client_identifier_skips_per_client_limit(self) -> None:
        limiter = _DenyRateLimiter(deny=True)
        service = _service(engines={"okeng": _OkEngine()}, rate_window=limiter)

        response = await service.search(_req())

        assert response.all_unresponsive is False
        assert limiter.keys == []

    async def test_allowed_passes_through(self) -> None:
        limiter = _DenyRateLimiter(deny=False)
        service = _service(engines={"okeng": _OkEngine()}, rate_window=limiter)

        response = await service.search(_req(client_identifier="tenant-a"))

        assert response.all_unresponsive is False
        assert limiter.keys == ["tenant-a"]


# ---------------------------------------------------------------------------
# SearchService — cache behavior
# ---------------------------------------------------------------------------


class TestCache:
    async def test_cache_hit_skips_dispatch(self) -> None:
        ok = _OkEngine(count=1)
        cache = _FakeCache()
        service = _service(engines={"okeng": ok}, cache=cache)
        await service.search(_req())

        assert ok.calls == 1
        # Second request hits cache — engine not called again
        response = await service.search(_req())

        assert response.cached is True
        assert ok.calls == 1
        assert len(response.results) == 1

    async def test_cached_response_keeps_original_query_id(self) -> None:
        cache = _FakeCache()
        service = _service(engines={"okeng": _OkEngine(count=1)}, cache=cache)
        first = await service.search(_req())

        second = await service.search(_req())

        assert second.cached is True
        assert second.query_id == first.query_id

    async def test_disconnected_cache_never_read(self) -> None:
        ok = _OkEngine(count=1)
        cache = _FakeCache(connected=False)
        service = _service(engines={"okeng": ok}, cache=cache)

        response = await service.search(_req())

        assert response.cached is False
        assert ok.calls == 1

    async def test_prefer_fresh_skips_cache_read(self) -> None:
        ok = _OkEngine(count=1)
        cache = _FakeCache()
        service = _service(engines={"okeng": ok}, cache=cache)
        await service.search(_req())

        response = await service.search(_req(freshness="prefer_fresh"))

        assert response.cached is False
        assert ok.calls == 2

    async def test_negative_cache_hit(self) -> None:
        cache = _FakeCache()
        cache._data[cache_key("test query", "en", 0)] = {"_error": True}
        service = _service(engines={"okeng": _OkEngine()}, cache=cache)

        response = await service.search(_req())

        assert response.cached_error is True
        assert response.cached is True

    async def test_scoped_requests_do_not_cross_contaminate(self) -> None:
        cache = _FakeCache()
        service = _service(engines={"okeng": _OkEngine(count=2)}, cache=cache)

        r1 = await service.search(_req(engines=["okeng"]))
        r2 = await service.search(_req(categories=["general"]))

        assert r1.cached is False and r2.cached is False
        assert len(cache._data) == 2

    async def test_cache_include_filtering_is_request_scoped(self) -> None:
        """VAL-CORRECT-009 — include filtering depends only on the current request."""
        cache = _FakeCache()
        service = _service(engines={"okeng": _OkEngine(count=2)}, cache=cache)

        # Populate the cache with a request that omits engine_status.
        without = await service.search(_req(include={"results"}))
        assert without.cached is False
        assert without.engine_outcomes == []

        # A request that requests engine_status hits the cache and must still
        # receive engine_outcomes — the cached canonical response is complete.
        with_status = await service.search(_req(include={"results", "engine_status"}))
        assert with_status.cached is True
        assert len(with_status.engine_outcomes) == 1

        # Reverse population order: warm with engine_status, read without.
        with_status2 = await service.search(_req(query="other query", include={"results", "engine_status"}))
        assert with_status2.cached is False
        assert len(with_status2.engine_outcomes) == 1
        without2 = await service.search(_req(query="other query", include={"results"}))
        assert without2.cached is True
        assert without2.engine_outcomes == []

    async def test_cache_max_results_slicing_is_per_request(self) -> None:
        """VAL-CORRECT-011 — max_results slicing is applied per request."""
        cache = _FakeCache()
        service = _service(engines={"okeng": _OkEngine(count=5)}, cache=cache)

        # Warm with a large max_results, then read with a smaller one.
        big = await service.search(_req(max_results=10))
        assert big.cached is False
        assert len(big.results) == 5
        small = await service.search(_req(max_results=3))
        assert small.cached is True
        assert len(small.results) == 3

        # Warm with a small max_results, then read with a larger one — the
        # cached unsliced set must serve the full requested count, never a
        # stale smaller slice.
        small2 = await service.search(_req(query="warm small", max_results=3))
        assert len(small2.results) == 3
        big2 = await service.search(_req(query="warm small", max_results=10))
        assert big2.cached is True
        assert len(big2.results) == 5


# ---------------------------------------------------------------------------
# Cache key scoping
# ---------------------------------------------------------------------------


class TestCacheKeyScoping:
    def test_scope_inputs_change_the_key(self) -> None:
        base = cache_key("test", "en", 0)
        assert cache_key("test", "en", 0, categories=["news"]) != base
        assert cache_key("test", "en", 0, engines=["brave"]) != base
        assert cache_key("test", "en", 0, pageno=2) != base
        assert cache_key("test", "en", 0, time_range="day") != base

    def test_scope_inputs_are_order_independent(self) -> None:
        a = cache_key("test", "en", 0, engines=["brave", "wikipedia"])
        b = cache_key("test", "en", 0, engines=["wikipedia", "brave"])
        assert a == b

    def test_language_and_safesearch_still_scoped(self) -> None:
        assert cache_key("test", "en", 0) != cache_key("test", "fr", 0)
        assert cache_key("test", "en", 0) != cache_key("test", "en", 2)


# ---------------------------------------------------------------------------
# Payload round-trip
# ---------------------------------------------------------------------------


class TestPayloadRoundTrip:
    def test_full_round_trip(self) -> None:
        scope = ScopeDecision(
            selected_engines=["eng_a", "eng_b"],
            resolved_categories=["news"],
            routing_rule="explicit category",
            matched_topic=None,
            warnings=["warning"],
            excluded_engines=[EngineExclusion(engine="hibp", reason="policy")],
        )
        response = SearchResponse(
            query="test",
            results=[
                SearchResult(
                    url="https://a.com",
                    title="A",
                    content="c",
                    engine="eng_a",
                    engines={"eng_a", "eng_b"},
                    score=2.0,
                    position=1,
                    category="general",
                    published_date="2026-01-01",
                    tier=1,
                )
            ],
            scope=scope,
            engine_outcomes=[EngineOutcome(engine="eng_a", status="ok", result_count=1, latency_ms=5.5, message=None)],
            suggestions=["s1"],
            answers=[{"text": "answer"}],
            corrections=["did you mean"],
            infoboxes=[{"title": "info"}],
            query_id="ssx-abc",
            response_time_ms=123,
            partial=False,
            all_unresponsive=False,
            empty_engines=[["scrape", "no results"]],
        )

        payload = search_response_to_payload(response)
        assert payload["cached"] is False

        rebuilt = search_response_from_payload(payload)

        assert rebuilt.query == "test"
        assert rebuilt.query_id == "ssx-abc"
        assert rebuilt.results[0].url == "https://a.com"
        assert rebuilt.results[0].engines == {"eng_a", "eng_b"}
        assert rebuilt.results[0].tier == 1
        assert rebuilt.scope.routing_rule == "explicit category"
        assert rebuilt.scope.excluded_engines[0].engine == "hibp"
        assert rebuilt.engine_outcomes[0].latency_ms == 5.5
        assert rebuilt.suggestions == ["s1"]
        assert rebuilt.empty_engines == [["scrape", "no results"]]

    def test_result_dict_round_trip(self) -> None:
        result = SearchResult(
            url="https://x.com",
            title="X",
            content="y",
            engine="eng",
            engines={"eng"},
            score=1.0,
            position=3,
            category="news",
            published_date=None,
            thumbnail=None,
            img_src=None,
            tier=2,
        )
        response = SearchResponse(query="", results=[result], scope=ScopeDecision(), engine_outcomes=[])
        payload = search_response_to_payload(response)
        rebuilt = search_result_from_dict(payload["results"][0])
        assert rebuilt.url == result.url
        assert rebuilt.engines == {"eng"}
        assert rebuilt.tier == 2

    def test_outcome_dict_round_trip(self) -> None:
        outcome = EngineOutcome(engine="e", status="rate_limited", result_count=0, latency_ms=None, message="too many")
        response = SearchResponse(query="", results=[], scope=ScopeDecision(), engine_outcomes=[outcome])
        payload = search_response_to_payload(response)
        rebuilt = engine_outcome_from_dict(payload["engine_outcomes"][0])
        assert rebuilt.status == "rate_limited"
        assert rebuilt.latency_ms is None

    # ------------------------------------------------------------------
    # JSON-safe serialization (VAL-CORRECT-001/004/005/006/007/008)
    # ------------------------------------------------------------------

    def test_engines_serialized_as_sorted_list_not_set_repr(self) -> None:
        result = SearchResult(
            url="https://x.com",
            title="X",
            content="y",
            engine="github",
            engines={"github", "arxiv", "wikipedia"},
        )
        response = SearchResponse(query="", results=[result], scope=ScopeDecision(), engine_outcomes=[])

        payload = search_response_to_payload(response)

        stored = payload["results"][0]["engines"]
        assert stored == ["arxiv", "github", "wikipedia"]
        assert isinstance(stored, list)

    def test_cache_round_trip_preserves_engine_set_exactly(self) -> None:
        original_set = {"github", "arxiv", "wikipedia"}
        result = SearchResult(url="https://x.com", title="X", content="y", engine="github", engines=set(original_set))
        response = SearchResponse(query="", results=[result], scope=ScopeDecision(), engine_outcomes=[])

        payload = search_response_to_payload(response)
        rebuilt = search_result_from_dict(payload["results"][0])

        assert rebuilt.engines == original_set
        assert sorted(rebuilt.engines) == sorted(original_set)

    def test_engines_count_preserved_after_round_trip(self) -> None:
        for count in (1, 2, 0):
            engines = {f"eng{i}" for i in range(count)}
            result = SearchResult(url="https://x.com", title="X", content="y", engine="eng0", engines=engines)
            response = SearchResponse(query="", results=[result], scope=ScopeDecision(), engine_outcomes=[])
            payload = search_response_to_payload(response)
            rebuilt = search_result_from_dict(payload["results"][0])
            assert len(rebuilt.engines) == count

    def test_optional_fields_round_trip_without_coercion(self) -> None:
        result = SearchResult(
            url="https://x.com",
            title="X",
            content="y",
            engine="eng",
            engines={"eng"},
            score=4.5,
            position=7,
            category="science",
            published_date="2026-03-01",
            thumbnail="https://x.com/thumb.png",
            img_src="https://x.com/img.png",
            tier=2,
        )
        response = SearchResponse(query="", results=[result], scope=ScopeDecision(), engine_outcomes=[])

        payload = search_response_to_payload(response)
        rebuilt = search_result_from_dict(payload["results"][0])

        assert rebuilt.score == 4.5 and isinstance(rebuilt.score, float)
        assert rebuilt.position == 7 and isinstance(rebuilt.position, int)
        assert rebuilt.category == "science"
        assert rebuilt.published_date == "2026-03-01"
        assert rebuilt.thumbnail == "https://x.com/thumb.png"
        assert rebuilt.img_src == "https://x.com/img.png"
        assert rebuilt.tier == 2 and isinstance(rebuilt.tier, int)

    def test_optional_fields_none_stays_none(self) -> None:
        result = SearchResult(
            url="https://x.com",
            title="X",
            content="y",
            engine="eng",
            engines={"eng"},
            published_date=None,
            thumbnail=None,
            img_src=None,
        )
        response = SearchResponse(query="", results=[result], scope=ScopeDecision(), engine_outcomes=[])
        payload = search_response_to_payload(response)

        rebuilt = search_result_from_dict(payload["results"][0])

        assert rebuilt.published_date is None
        assert rebuilt.thumbnail is None
        assert rebuilt.img_src is None

    def test_empty_collections_round_trip_as_empty(self) -> None:
        response = SearchResponse(
            query="q",
            results=[],
            scope=ScopeDecision(),
            engine_outcomes=[],
            suggestions=[],
            answers=[],
            corrections=[],
            infoboxes=[],
            empty_engines=[],
        )

        payload = search_response_to_payload(response)
        rebuilt = search_response_from_payload(payload)

        assert rebuilt.results == []
        assert rebuilt.engine_outcomes == []
        assert rebuilt.suggestions == []
        assert rebuilt.answers == []
        assert rebuilt.corrections == []
        assert rebuilt.infoboxes == []
        assert rebuilt.empty_engines == []

    def test_nested_structured_metadata_round_trips(self) -> None:
        answer = {"text": "The answer", "engines": ["wikipedia"], "meta": {"score": 0.9}}
        correction = "did you mean foo"
        infobox = {"title": "Info", "attributes": {"k": "v"}, "urls": ["https://a.com"]}
        outcome = EngineOutcome(engine="wikipedia", status="ok", result_count=1, latency_ms=1.5, message="fine")
        response = SearchResponse(
            query="q",
            results=[
                SearchResult(
                    url="https://x.com",
                    title="X",
                    content="y",
                    engine="wikipedia",
                    engines={"wikipedia"},
                )
            ],
            scope=ScopeDecision(),
            engine_outcomes=[outcome],
            suggestions=["s"],
            answers=[answer],
            corrections=[correction],
            infoboxes=[infobox],
        )

        payload = search_response_to_payload(response)
        rebuilt = search_response_from_payload(payload)

        assert rebuilt.answers == [answer]
        assert rebuilt.corrections == [correction]
        assert rebuilt.infoboxes == [infobox]
        assert rebuilt.engine_outcomes[0].latency_ms == 1.5

    def test_legacy_stringified_set_rehydrates(self) -> None:
        data: dict[str, Any] = {
            "url": "https://x.com",
            "title": "X",
            "content": "y",
            "engine": "arxiv",
            "engines": "{'arxiv', 'github'}",
            "score": 1.0,
            "position": 1,
            "category": "science",
            "tier": 2,
        }

        rebuilt = search_result_from_dict(data)

        assert rebuilt.engines == {"arxiv", "github"}
        assert "a" not in rebuilt.engines  # not the set of single characters

    def test_legacy_single_engine_string_rehydrates(self) -> None:
        data: dict[str, Any] = {
            "url": "https://x.com",
            "title": "X",
            "content": "y",
            "engine": "brave",
            "engines": "{'brave'}",
            "score": 1.0,
            "position": 1,
            "category": "general",
            "tier": 1,
        }

        rebuilt = search_result_from_dict(data)

        assert rebuilt.engines == {"brave"}
