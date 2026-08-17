"""Normalized search service — shared pipeline for HTTP and MCP surfaces.

Extracts the search orchestration from :mod:`slopsearx.server` into a
reusable service layer so both the FastAPI route and the MCP server run
the exact same pipeline:

- :class:`SearchService` runs the pipeline (scope resolution, per-client
  rate limiting, cache lookup, concurrent dispatch, ranking, suggestions,
  telemetry, audit) and returns a normalized :class:`SearchResponse`.
- :class:`ScopeResolver` extracts the engine-selection logic so routing
  decisions can be explained (dry-run) without executing a search.
- :func:`build_context` / :func:`destroy_context` wire the shared runtime
  (engines, cache, rate limiter, router, telemetry) for both the FastAPI
  lifespan and the MCP server.

The FastAPI route and the MCP tools are thin adapters over this service.
HTTP wire behavior (SearXNG JSON/YAML) is preserved by the route layer.
"""

from __future__ import annotations

import asyncio
import dataclasses
import logging
import os
import time
import uuid
from dataclasses import dataclass, field
from typing import Any

from slopsearx import metrics as m
from slopsearx.adapter import (
    AdapterResponse,
    EngineAdapter,
    EngineStatus,
    SearchResult,
    discover_engines,
    sanitize_url,
)
from slopsearx.audit import QueryAuditLogger
from slopsearx.cache import SearchCache, _ttl_for_query, cache_key
from slopsearx.capabilities import DEFAULT_SENSITIVE_ENGINES
from slopsearx.config import load_config
from slopsearx.logging import capture_exception
from slopsearx.merger import PresenceRanker, extract_empty_scrape_engines
from slopsearx.payload import payload_from_dict, payload_to_dict
from slopsearx.ratelimit import LocalTokenBucket, RateLimiter, RateLimitStrategy, ValkeySlidingWindow
from slopsearx.router import QueryRouter
from slopsearx.stats import EngineStatsTracker
from slopsearx.suggest import SuggestionService

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

# Broad, general-purpose engines used as the unscoped fallback set
# (the "tier-1 fallback" routing rule).
DEFAULT_TIER1_ENGINES: frozenset[str] = frozenset(
    {
        "brave",
        "duckduckgo",
        "google",
        "wikipedia",
        "stackexchange",
        "reddit",
    }
)

DEFAULT_ENGINE_TIMEOUT_S = 3.0
# Minimum fan-out deadline. The effective deadline also honors the selected
# engines' configured timeouts, including targeted engines above this floor.
DEFAULT_SEARCH_TIMEOUT_S = 10.0

# ---------------------------------------------------------------------------
# Request / response model
# ---------------------------------------------------------------------------


@dataclass
class SearchRequest:
    """Normalized search request accepted by :class:`SearchService`."""

    query: str
    categories: list[str] | None = None
    engines: list[str] | None = None
    language: str = "en"
    page: int = 1
    time_range: str | None = None
    safesearch: int = 0
    max_results: int | None = None
    include: set[str] = field(default_factory=lambda: {"results", "suggestions", "engine_status", "diagnostics"})
    # prefer_cache | prefer_fresh | no_preference
    freshness: str = "no_preference"
    # HTTP client IP or MCP tenant identifier; used for per-client rate
    # limiting and the audit trail.
    client_identifier: str | None = None


@dataclass
class EngineExclusion:
    """An engine that was deliberately excluded from a scope decision."""

    engine: str
    reason: str


@dataclass
class ScopeDecision:
    """The resolved engine selection for a request, with an explanation."""

    selected_engines: list[str] = field(default_factory=list)
    resolved_categories: list[str] = field(default_factory=list)
    routing_rule: str = ""
    matched_topic: str | None = None
    warnings: list[str] = field(default_factory=list)
    excluded_engines: list[EngineExclusion] = field(default_factory=list)


@dataclass
class EngineOutcome:
    """Per-engine outcome for one search dispatch."""

    engine: str
    status: str  # ok | rate_limited | blocked | error | timeout | unavailable
    result_count: int
    latency_ms: float | None = None
    message: str | None = None


@dataclass
class SearchResponse:
    """Normalized search response consumed by both HTTP and MCP adapters."""

    query: str
    results: list[SearchResult]
    scope: ScopeDecision
    engine_outcomes: list[EngineOutcome]
    suggestions: list[str] = field(default_factory=list)
    answers: list[dict[str, Any]] = field(default_factory=list)
    corrections: list[str] = field(default_factory=list)
    infoboxes: list[dict[str, Any]] = field(default_factory=list)
    query_id: str = ""
    cached: bool = False
    response_time_ms: int = 0
    partial: bool = False
    all_unresponsive: bool = False
    empty_engines: list[list[str]] = field(default_factory=list)
    cached_error: bool = False


# ---------------------------------------------------------------------------
# Service errors
# ---------------------------------------------------------------------------


class ServiceError(Exception):
    """Base class for service-level failures."""


class QueryValidationError(ServiceError):
    """The query is empty or whitespace-only."""


class RateLimitExceededError(ServiceError):
    """The per-client rate limit was exceeded."""


# ---------------------------------------------------------------------------
# Shared runtime
# ---------------------------------------------------------------------------


@dataclass
class AppContext:
    """Wiring shared between the HTTP server and the MCP server.

    Instances are cheap and may be rebuilt per request from live module
    state (the HTTP route does this so test fixtures and runtime
    overrides that mutate globals keep working).
    """

    active_engines: dict[str, EngineAdapter]
    cache: SearchCache | None = None
    rate_limiter: RateLimiter | None = None
    router: QueryRouter | None = None
    suggestion_service: SuggestionService | None = None
    stats_tracker: EngineStatsTracker | None = None
    audit_logger: QueryAuditLogger | None = None
    engine_semaphore: asyncio.Semaphore | None = None
    client_rate_window: RateLimitStrategy | None = None
    tier1_engines: set[str] | frozenset[str] = field(default_factory=lambda: set(DEFAULT_TIER1_ENGINES))
    sensitive_engines: set[str] | frozenset[str] = field(default_factory=lambda: set(DEFAULT_SENSITIVE_ENGINES))
    empty_scrape_diagnostics_enabled: bool = False


async def build_context() -> AppContext:
    """Discover and wire the shared runtime (engines, cache, router, ...).

    This is the canonical startup path used by the FastAPI lifespan and
    the MCP server so both surfaces see identical wiring.
    """
    cache = SearchCache()
    await cache.connect()

    # Rate limiter (default: local token bucket for dev)
    rate_limiter = RateLimiter(LocalTokenBucket())
    await rate_limiter.warmup()

    # Global engine dispatch semaphore
    max_conc_str = os.environ.get("MAX_CONCURRENT_ENGINES", "10")
    try:
        max_conc = int(max_conc_str)
    except (ValueError, TypeError):
        max_conc = 10  # non-numeric defaults to 10
    if max_conc < 1:
        max_conc = 1  # zero/negative defaults to 1
    engine_semaphore = asyncio.Semaphore(max_conc)

    # Per-client rate limiter
    try:
        per_client_rate = float(os.environ.get("PER_CLIENT_REQUESTS", "30"))
    except (ValueError, TypeError):
        per_client_rate = 30.0
    try:
        per_client_window = float(os.environ.get("PER_CLIENT_WINDOW_SECONDS", "60"))
    except (ValueError, TypeError):
        per_client_window = 60.0

    # Parse FAIL_CLOSED env var (only 'true'/'1'/'yes' enable fail-closed)
    fail_closed_raw = os.environ.get("FAIL_CLOSED", "false")
    fail_closed = fail_closed_raw.strip().lower() in ("true", "1", "yes")

    client_rate_window = ValkeySlidingWindow(
        valkey_url=os.environ.get("VALKEY_URL", ""),
        default_rate=per_client_rate,
        window_seconds=per_client_window,
        fail_closed=fail_closed,
    )
    await client_rate_window.warmup()

    # Engine discovery
    cfg = load_config()
    empty_scrape_diagnostics_enabled = os.environ.get("FEATURE_EMPTY_SCRAPE_DIAGNOSTICS", "").lower() in (
        "true",
        "1",
    )
    engine_configs = {name: dataclasses.asdict(entry) for name, entry in cfg.engines.items()}
    # Opt in to Brave category-specific endpoints. The default retains
    # the established web endpoint behavior.
    brave_routing = os.environ.get("FEATURE_BRAVE_CATEGORY_ROUTING", "").lower() in ("true", "1")
    for ecfg in engine_configs.values():
        ecfg["_feature_brave_category_routing"] = brave_routing
    active_engines = discover_engines(engine_configs)

    # Inject rate limiter into each engine
    for engine in active_engines.values():
        engine.rate_limiter = rate_limiter

    # Warm up engines concurrently (warmup failure is non-fatal)
    warmup_tasks = [_warmup_engine(name, engine) for name, engine in active_engines.items()]
    await asyncio.gather(*warmup_tasks, return_exceptions=True)

    # Query router
    router_cfg = dataclasses.asdict(cfg.routing)
    router = QueryRouter(routing_config=router_cfg)

    # Suggestion service (opt-in: defaults to off)
    suggestion_service: SuggestionService | None = None
    if cfg.enable_suggestions:
        brave_entry = cfg.engines.get("brave")
        brave_api_key = (brave_entry.api_key or "") if brave_entry else ""
        if brave_api_key:
            suggestion_service = SuggestionService(brave_api_key=brave_api_key, cache=cache)

    return AppContext(
        active_engines=active_engines,
        cache=cache,
        rate_limiter=rate_limiter,
        router=router,
        suggestion_service=suggestion_service,
        stats_tracker=EngineStatsTracker(cache=cache),
        audit_logger=QueryAuditLogger(cache=cache),
        engine_semaphore=engine_semaphore,
        client_rate_window=client_rate_window,
        empty_scrape_diagnostics_enabled=empty_scrape_diagnostics_enabled,
    )


async def destroy_context(ctx: AppContext) -> None:
    """Gracefully shut down all engines, cache, and rate limiters."""
    shutdown_tasks = [_shutdown_engine(name, engine) for name, engine in ctx.active_engines.items()]
    await asyncio.gather(*shutdown_tasks, return_exceptions=True)

    if ctx.rate_limiter is not None:
        await ctx.rate_limiter.shutdown()

    if ctx.client_rate_window is not None:
        await ctx.client_rate_window.shutdown()

    if ctx.cache is not None:
        await ctx.cache.close()


async def _warmup_engine(name: str, engine: EngineAdapter) -> None:
    del name
    try:
        await engine.warmup()
    except Exception:
        pass  # Warmup failure is non-fatal


async def _shutdown_engine(name: str, engine: EngineAdapter) -> None:
    del name
    try:
        await engine.shutdown()
    except Exception:
        pass


# ---------------------------------------------------------------------------
# Scope resolution
# ---------------------------------------------------------------------------


class ScopeResolver:
    """Selects the engine set for a search request. Pure logic, no I/O.

    Precedence (matching the HTTP contract):
      1. explicit ``engines`` list (unknown names are dropped with a warning);
      2. explicit ``categories`` (OR filter);
      3. query-topic routing (first match wins);
      4. tier-1 fallback (or all active engines when none are tier-1);
      5. all active engines when no router is configured.

    Sensitive engines (see ``sensitive_engines``) are never selected by
    category or unscoped routing — only an explicit ``engines`` list or
    a policy grant can reach them.
    """

    def __init__(
        self,
        active_engines: dict[str, EngineAdapter],
        router: QueryRouter | None,
        tier1_engines: set[str] | frozenset[str] | None = None,
        sensitive_engines: set[str] | frozenset[str] | None = None,
    ) -> None:
        self._active = active_engines
        self._router = router
        self._tier1 = set(tier1_engines) if tier1_engines is not None else set(DEFAULT_TIER1_ENGINES)
        self._sensitive = set(sensitive_engines) if sensitive_engines is not None else set(DEFAULT_SENSITIVE_ENGINES)

    def resolve(self, request: SearchRequest) -> ScopeDecision:
        """Resolve a request to a :class:`ScopeDecision`."""
        decision = ScopeDecision()

        if request.engines:
            requested = [name.strip() for name in request.engines if name.strip()]
            unknown = [name for name in requested if name not in self._active]
            known = [name for name in requested if name in self._active]
            if unknown:
                decision.warnings.append(f"unknown engines ignored: {', '.join(sorted(unknown))}")
            decision.selected_engines = known
            decision.routing_rule = "explicit engine"
            decision.excluded_engines = [
                EngineExclusion(engine=name, reason="not an active engine") for name in sorted(unknown)
            ]
            return decision

        cats = [c.strip() for c in (request.categories or []) if c.strip()]
        if cats:
            selected = [name for name, engine in self._active.items() if any(cat in engine.categories for cat in cats)]
            decision.selected_engines = self._drop_sensitive(decision, selected, "sensitive engine excluded by policy")
            decision.resolved_categories = cats
            decision.routing_rule = "explicit category"
            return decision

        if self._router is not None:
            routed = self._router.route(request.query, cats)
            if routed is not None:
                selected = [name for name in routed if name in self._active]
                decision.selected_engines = self._drop_sensitive(
                    decision, selected, "sensitive engine excluded by policy"
                )
                decision.matched_topic = self._router.match_topic(request.query)
                decision.routing_rule = "topic match"
                return decision

            tier1 = [name for name in self._tier1 if name in self._active]
            if tier1:
                decision.selected_engines = self._drop_sensitive(decision, tier1, "sensitive engine excluded by policy")
                decision.routing_rule = "tier-1 fallback"
            else:
                decision.selected_engines = self._drop_sensitive(
                    decision, list(self._active), "sensitive engine excluded by policy"
                )
                decision.routing_rule = "all active engines"
            return decision

        decision.selected_engines = self._drop_sensitive(
            decision, list(self._active), "sensitive engine excluded by policy"
        )
        decision.routing_rule = "all active engines"
        return decision

    def explain(self, request: SearchRequest) -> ScopeDecision:
        """Dry-run routing preview.

        Identical to :meth:`resolve` (pure logic, no I/O) — exists so
        the MCP ``explain_search_scope`` tool can show which engines
        would be selected and why without spending rate limits.
        """
        return self.resolve(request)

    def _drop_sensitive(self, decision: ScopeDecision, engines: list[str], reason: str) -> list[str]:
        """Remove sensitive engines from a candidate list, recording why."""
        kept = [name for name in engines if name not in self._sensitive]
        for name in engines:
            if name in self._sensitive:
                decision.excluded_engines.append(EngineExclusion(engine=name, reason=reason))
                decision.warnings.append(
                    f"sensitive engine '{name}' requires an explicit engines list or a policy grant"
                )
        return kept


# ---------------------------------------------------------------------------
# Search pipeline
# ---------------------------------------------------------------------------


class SearchService:
    """Runs the search pipeline and returns a normalized :class:`SearchResponse`.

    Adapters never raise: every failure is classified into
    ``AdapterResponse.status`` and surfaced as an :class:`EngineOutcome`.
    """

    def __init__(self, context: AppContext) -> None:
        self._ctx = context
        self._ranker = PresenceRanker()
        self._resolver: ScopeResolver | None = None

    def _resolver_for(self) -> ScopeResolver:
        """Build (or refresh) the scope resolver for the live context.

        Rebuilt when the active engine dict or router identity changes so
        runtime overrides (test fixtures, config reloads) are honored.
        """
        resolver = self._resolver
        if (
            resolver is None
            or resolver._active is not self._ctx.active_engines  # noqa: SLF001
            or resolver._router is not self._ctx.router  # noqa: SLF001
        ):
            resolver = ScopeResolver(
                active_engines=self._ctx.active_engines,
                router=self._ctx.router,
                tier1_engines=self._ctx.tier1_engines,
                sensitive_engines=self._ctx.sensitive_engines,
            )
            self._resolver = resolver
        return resolver

    async def search(self, request: SearchRequest) -> SearchResponse:
        """Execute a search and return a normalized response.

        Raises:
            QueryValidationError: the query is empty or whitespace-only.
            RateLimitExceededError: the per-client rate limit denied the request.
        """
        t_start = time.monotonic()
        query_id = generate_query_id()

        if not request.query or not request.query.strip():
            raise QueryValidationError("query is required")

        scope = self._resolver_for().resolve(request)

        if not scope.selected_engines:
            # No engines available at all — 503 with no dispatch.
            return SearchResponse(
                query=request.query,
                results=[],
                scope=scope,
                engine_outcomes=[],
                query_id=query_id,
                response_time_ms=round((time.monotonic() - t_start) * 1000),
                all_unresponsive=True,
            )

        # Per-client rate limiting — checked before cache and dispatch
        if self._ctx.client_rate_window is not None and request.client_identifier:
            allowed = await self._ctx.client_rate_window.acquire(request.client_identifier, cost=1)
            if not allowed:
                raise RateLimitExceededError()

        cached = await self._read_cache(request)
        if cached is not None:
            return cached

        target = {name: self._ctx.active_engines[name] for name in scope.selected_engines}
        search_params: dict[str, Any] = {
            "language": request.language,
            "safesearch": request.safesearch,
            "pageno": request.page,
            "time_range": request.time_range,
            "categories": request.categories or ["general"],
        }

        # Dispatch to all engines concurrently (bounded by semaphore)
        tasks: list[asyncio.Task[AdapterResponse]] = []
        engine_names: list[str] = []
        started_engines: set[str] = set()
        circuit_open: list[str] = []
        for name, engine in target.items():
            if not engine.circuit_allowed():
                circuit_open.append(name)
                continue
            tasks.append(
                asyncio.create_task(
                    self._dispatch_with_semaphore(
                        name, engine, request.query, search_params, started_engines=started_engines
                    )
                )
            )
            engine_names.append(name)

        # Fire suggestion fetch concurrently with engine dispatch. Suggestions
        # are always fetched so the cached canonical response is complete; the
        # requested include view is derived at read time (the cache key omits
        # include/max_results/freshness, so the cache must hold the full form).
        suggestions_task: asyncio.Task[list[str]] | None = None
        if self._ctx.suggestion_service is not None:
            suggestions_task = asyncio.create_task(self._generate_suggestions(request.query))

        engine_timeouts = [
            self._resolve_engine_timeout_s(engine) for engine in target.values() if engine.circuit_allowed()
        ]
        # The deadline covers semaphore acquisition as well as engine work.
        # Use the sum, rather than the maximum, so serialized dispatch can give
        # every selected engine its configured execution budget.
        dispatch_deadline_s = max(DEFAULT_SEARCH_TIMEOUT_S, sum(engine_timeouts))

        dispatch_results = await self._gather_with_deadline(tasks, engine_names, dispatch_deadline_s, started_engines)

        # Collect results and metadata
        responses: dict[str, AdapterResponse] = {}
        for name, result in zip(engine_names, dispatch_results):
            # A task waiting behind the semaphore may be cancelled by the
            # search-wide deadline before its engine ever starts. Do not let a
            # scheduler timeout count as an upstream engine failure.
            engine = target[name]
            if result.status in (EngineStatus.ERROR, EngineStatus.TIMEOUT):
                if name in started_engines:
                    engine.record_failure()
            elif result.status != EngineStatus.UNAVAILABLE:
                engine.record_success()

            responses[name] = result

            # Annotate each result with its tier for unscoped searches
            tier = 1 if name in self._ctx.tier1_engines else 2
            for sr in result.results:
                sr.tier = tier

            self._record_engine_metrics(name, result)

            # Per-engine quality telemetry in Valkey (non-blocking)
            if self._ctx.stats_tracker is not None:
                avg_score = sum(r.score for r in result.results) / len(result.results) if result.results else 0.0
                asyncio.create_task(
                    self._ctx.stats_tracker.record_query(
                        engine=name,
                        result_count=len(result.results),
                        latency_ms=result.latency_ms,
                        status=result.status,
                        avg_score=avg_score,
                    )
                )

        # Add circuit-open engines as failures (no metrics — never dispatched)
        for name in circuit_open:
            responses[name] = AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message="circuit open",
            )

        # Merge and rank
        ranked = self._ranker.rank(
            {name: resp.results for name, resp in responses.items()},
            request.query,
            search_params,
        )

        elapsed_ms = (time.monotonic() - t_start) * 1000

        # Empty-scrape diagnostics (opt-in feature flag). Computed whenever the
        # flag is enabled so the cached canonical response is complete; the
        # include view is derived at read time.
        empty_engines: list[list[str]] = []
        if self._ctx.empty_scrape_diagnostics_enabled:
            scrape_engine_names = {name for name, engine in target.items() if engine.engine_type == "scrape"}
            empty_engines = extract_empty_scrape_engines(responses, scrape_engine_names)
            for name, reason in empty_engines:
                logger.warning("Empty scrape diagnostic: engine=%s query_id=%s reason=%s", name, query_id, reason)

        all_unresponsive = all(resp.status != EngineStatus.OK for resp in responses.values())
        non_ok = sum(1 for resp in responses.values() if resp.status != EngineStatus.OK)

        suggestions = await suggestions_task if suggestions_task is not None else []

        # Aggregate answers, corrections, and infoboxes from all engine responses
        all_answers: list[dict[str, Any]] = []
        all_corrections: list[str] = []
        all_infoboxes: list[dict[str, Any]] = []
        for resp in responses.values():
            if resp.answers:
                all_answers.extend(resp.answers)
            if resp.corrections:
                all_corrections.extend(resp.corrections)
            if resp.infoboxes:
                all_infoboxes.extend(resp.infoboxes)

        # Build the canonical full response: every include-able field present
        # and results unsliced. This is the form written to the cache, so a
        # cache entry is independent of the populating request's
        # include/max_results/freshness.
        canonical = SearchResponse(
            query=request.query,
            results=ranked,
            scope=scope,
            engine_outcomes=self._outcomes(responses, include_status=True),
            suggestions=suggestions,
            answers=all_answers,
            corrections=all_corrections,
            infoboxes=all_infoboxes,
            query_id=query_id,
            response_time_ms=round(elapsed_ms),
            partial=not all_unresponsive and non_ok > 0,
            all_unresponsive=all_unresponsive,
            empty_engines=empty_engines,
        )

        await self._write_cache(request, canonical, all_unresponsive)

        # Derive the requested include-filtered + max_results-sliced view.
        response = self._view_for_request(request, canonical)

        # Record audit trail (fire-and-forget)
        if self._ctx.audit_logger is not None:
            asyncio.create_task(
                self._ctx.audit_logger.record_query(
                    query=request.query,
                    client_ip=request.client_identifier or "unknown",
                    engine_results=responses,
                    latency_ms=elapsed_ms,
                )
            )

        return response

    # -- Cache ----------------------------------------------------------

    async def _read_cache(self, request: SearchRequest) -> SearchResponse | None:
        """Check the scoped search cache. Returns a cached response or None."""
        cache = self._ctx.cache
        if cache is None or not cache.is_connected or request.freshness == "prefer_fresh":
            return None

        key = _scope_cache_key(request)
        payload = await cache.get(key)
        if payload is None:
            m.cache_hits.inc({"type": "miss"})
            return None

        if payload.get("_error"):
            # Negative cache hit
            m.cache_hits.inc({"type": "negative"})
            return SearchResponse(
                query=request.query,
                results=[],
                scope=ScopeDecision(),
                engine_outcomes=[],
                query_id=generate_query_id(),
                cached=True,
                cached_error=True,
            )

        m.cache_hits.inc({"type": "hit"})
        response = search_response_from_payload(payload)
        response.cached = True
        # The stored entry is the canonical full response; derive the view
        # requested by THIS request (include filtering + max_results slicing)
        # so a cache hit never leaks fields from the populating request.
        return self._view_for_request(request, response)

    async def _write_cache(self, request: SearchRequest, response: SearchResponse, all_unresponsive: bool) -> None:
        """Persist a fresh response under the fully scoped cache key."""
        cache = self._ctx.cache
        if cache is None or not cache.is_connected or all_unresponsive:
            return

        payload = search_response_to_payload(response)
        key = _scope_cache_key(request)
        ttl = _ttl_for_query(request.categories or [])
        await cache.set(key, payload, ttl)

    # -- Dispatch -------------------------------------------------------

    async def _dispatch_engine(
        self,
        name: str,
        engine: EngineAdapter,
        query: str,
        params: dict[str, Any],
        timeout_s: float | None = None,
    ) -> AdapterResponse:
        """Dispatch a query to one engine with a timeout.

        Honors the engine's configured ``timeout_ms`` when set; falls back to
        :data:`DEFAULT_ENGINE_TIMEOUT_S` for engines with no explicit timeout.
        Returns AdapterResponse — never raises. Timeouts are caught and
        returned as EngineStatus.TIMEOUT.
        """
        del name
        timeout_s = timeout_s if timeout_s is not None else self._resolve_engine_timeout_s(engine)
        try:
            return await asyncio.wait_for(engine.search(query, params), timeout=timeout_s)
        except asyncio.TimeoutError:
            return AdapterResponse(
                results=[],
                status=EngineStatus.TIMEOUT,
                error_message=f"timed out after {timeout_s}s",
                latency_ms=timeout_s * 1000,
            )
        except Exception as exc:
            capture_exception(exc)
            m.server_errors_total.inc({"type": "internal"})
            return AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=sanitize_url(str(exc)),
            )

    async def _dispatch_with_semaphore(
        self,
        name: str,
        engine: EngineAdapter,
        query: str,
        params: dict[str, Any],
        timeout_s: float | None = None,
        started_engines: set[str] | None = None,
    ) -> AdapterResponse:
        """Dispatch engine query, bounded by the global semaphore."""
        if timeout_s is None:
            timeout_s = self._resolve_engine_timeout_s(engine)
        if self._ctx.engine_semaphore is not None:
            async with self._ctx.engine_semaphore:
                if started_engines is not None:
                    started_engines.add(name)
                return await self._dispatch_engine(name, engine, query, params, timeout_s)
        if started_engines is not None:
            started_engines.add(name)
        return await self._dispatch_engine(name, engine, query, params, timeout_s)

    @staticmethod
    def _resolve_engine_timeout_s(engine: EngineAdapter) -> float:
        """Resolve an engine's dispatch timeout from its configured ``timeout_ms``.

        Never raises: a missing, non-numeric, or non-positive ``timeout_ms``
        falls back to :data:`DEFAULT_ENGINE_TIMEOUT_S`.
        """
        try:
            ms = float(engine.config.get("timeout_ms") or 0.0)
            if ms > 0:
                return ms / 1000.0
        except (TypeError, ValueError):
            pass
        return DEFAULT_ENGINE_TIMEOUT_S

    async def _gather_with_deadline(
        self,
        tasks: list[asyncio.Task[AdapterResponse]],
        engine_names: list[str],
        deadline_s: float = DEFAULT_SEARCH_TIMEOUT_S,
        started_engines: set[str] | None = None,
    ) -> list[Any]:
        """Gather engine dispatch tasks under an overall deadline.

        Cancels any task still running after the caller-supplied effective
        deadline so a single slow engine cannot hold the whole fan-out hostage.
        A started engine that misses the deadline receives ``TIMEOUT``;
        an engine still waiting for the dispatch semaphore receives
        ``UNAVAILABLE`` so scheduler delay is not reported as upstream failure.
        The effective deadline is supplied by the caller and is at least the
        configured minimum fan-out deadline.

        Per-engine exceptions are isolated: ``Task.result()`` re-raises a stored
        exception, so we catch it and classify it as ``EngineStatus.ERROR`` —
        preserving the "adapters never raise" contract even if cancellation or
        another ``BaseException`` escapes the per-engine dispatch wrapper.
        """
        if not tasks:
            return []
        try:
            done, pending = await asyncio.wait(tasks, timeout=deadline_s)
        except asyncio.CancelledError:
            for task in tasks:
                if not task.done():
                    task.cancel()
            await asyncio.gather(*tasks, return_exceptions=True)
            raise
        for task in pending:
            task.cancel()
        if pending:
            await asyncio.gather(*pending, return_exceptions=True)
        results: dict[str, AdapterResponse] = {}
        for task in done:
            name = engine_names[tasks.index(task)]
            try:
                raw = task.result()
            except BaseException as exc:  # noqa: BLE001 - isolate one engine from fan-out
                results[name] = AdapterResponse(
                    results=[], status=EngineStatus.ERROR, error_message=sanitize_url(str(exc))
                )
            else:
                results[name] = raw
        for name in engine_names:
            if name not in results:
                started = started_engines is None or name in started_engines
                results[name] = AdapterResponse(
                    results=[],
                    status=EngineStatus.TIMEOUT if started else EngineStatus.UNAVAILABLE,
                    error_message=(
                        f"timed out after {deadline_s}s" if started else "not started before the search deadline"
                    ),
                    latency_ms=deadline_s * 1000 if started else 0.0,
                )
        return [results[name] for name in engine_names]

    # -- Helpers --------------------------------------------------------

    async def _generate_suggestions(self, query: str) -> list[str]:
        """Fetch search suggestions from engine suggest APIs."""
        if self._ctx.suggestion_service is None:
            return []
        return await self._ctx.suggestion_service.fetch(query)

    def _record_engine_metrics(self, name: str, result: AdapterResponse) -> None:
        """Record per-engine Prometheus counters and gauges."""
        m.engine_queries.inc({"engine": name})
        m.engine_latency.observe({"engine": name}, result.latency_ms / 1000.0)
        degraded = (EngineStatus.TIMEOUT, EngineStatus.RATE_LIMITED)
        status_code = 0 if result.status == EngineStatus.OK else (1 if result.status in degraded else 2)
        m.engine_status.set({"engine": name}, status_code)

    @staticmethod
    def _outcomes(responses: dict[str, AdapterResponse], include_status: bool) -> list[EngineOutcome]:
        """Build engine outcomes from adapter responses."""
        if not include_status:
            return []
        return [
            EngineOutcome(
                engine=name,
                status=resp.status.value,
                result_count=len(resp.results),
                latency_ms=round(resp.latency_ms, 1),
                message=resp.error_message,
            )
            for name, resp in responses.items()
        ]

    @staticmethod
    def _view_for_request(request: SearchRequest, response: SearchResponse) -> SearchResponse:
        """Derive the requested view from a canonical full response.

        Include filtering (``engine_outcomes``/``suggestions``/``empty_engines``)
        and the ``max_results`` presentation bound depend only on the current
        request. Applying them here — on both the fresh and the cache-hit path —
        guarantees a cached response never returns a representation inconsistent
        with the current request's requested fields or detail level.
        """
        if "engine_status" not in request.include:
            response.engine_outcomes = []
        if "suggestions" not in request.include:
            response.suggestions = []
        if "diagnostics" not in request.include:
            response.empty_engines = []
        if request.max_results is not None and request.max_results > 0:
            response.results = response.results[: request.max_results]
        return response


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def generate_query_id() -> str:
    """Generate a short, traceable query identifier."""
    return f"ssx-{uuid.uuid4().hex[:8]}"


def _scope_cache_key(request: SearchRequest) -> str:
    """Build the fully scoped search cache key for a request."""
    return cache_key(
        request.query,
        request.language,
        request.safesearch,
        categories=request.categories,
        engines=request.engines,
        pageno=request.page,
        time_range=request.time_range,
    )


def build_response_meta(response: SearchResponse) -> dict[str, Any]:
    """Build the SearXNG-compatible ``meta.*`` extension dict from a response."""
    meta: dict[str, Any] = {
        "response_time_ms": response.response_time_ms,
        "cached": response.cached,
        "query_id": response.query_id,
        "engine_status": {
            outcome.engine: {
                "results": outcome.result_count,
                "latency_ms": round(outcome.latency_ms, 1) if outcome.latency_ms is not None else 0.0,
                "status": outcome.status,
            }
            for outcome in response.engine_outcomes
        },
    }
    if response.empty_engines:
        meta["empty_engines"] = response.empty_engines
    return meta


def unresponsive_from_outcomes(outcomes: list[EngineOutcome]) -> list[list[str]]:
    """Build the SearXNG ``unresponsive_engines`` list from engine outcomes."""
    return [[outcome.engine, outcome.message or outcome.status] for outcome in outcomes if outcome.status != "ok"]


# ---------------------------------------------------------------------------
# Cache payload serialization
# ---------------------------------------------------------------------------


def search_result_to_dict(result: SearchResult) -> dict[str, Any]:
    """Serialize a :class:`SearchResult` to a JSON-safe dict.

    Built field-by-field (no ``dataclasses.asdict`` deep copy) so the optional
    ``payload`` is canonicalized exactly once through
    :func:`~slopsearx.payload.payload_to_dict`. ``engines`` (a ``set[str]``)
    is canonicalized to a sorted list so the value survives ``json.dumps``
    without being stringified to its repr.
    """
    return {
        "url": result.url,
        "title": result.title,
        "content": result.content,
        "engine": result.engine,
        "engines": sorted(result.engines),
        "score": result.score,
        "position": result.position,
        "category": result.category,
        "published_date": result.published_date,
        "thumbnail": result.thumbnail,
        "img_src": result.img_src,
        "tier": result.tier,
        "payload": payload_to_dict(result.payload),
    }


def search_response_to_payload(response: SearchResponse) -> dict[str, Any]:
    """Serialize a :class:`SearchResponse` for the Valkey cache (JSON-safe).

    Built field-by-field (no ``dataclasses.asdict`` deep copy) so the
    ``results`` payloads are not deep-copied and then discarded. The
    serialized payload is the canonical full response, with ``cached`` forced
    to ``False`` so a stored entry never reflects the request that populated
    it.
    """
    return {
        "query": response.query,
        "results": [search_result_to_dict(result) for result in response.results],
        "scope": {
            "selected_engines": list(response.scope.selected_engines),
            "resolved_categories": list(response.scope.resolved_categories),
            "routing_rule": response.scope.routing_rule,
            "matched_topic": response.scope.matched_topic,
            "warnings": list(response.scope.warnings),
            "excluded_engines": [
                {"engine": exclusion.engine, "reason": exclusion.reason}
                for exclusion in response.scope.excluded_engines
            ],
        },
        "engine_outcomes": [
            {
                "engine": outcome.engine,
                "status": outcome.status,
                "result_count": outcome.result_count,
                "latency_ms": outcome.latency_ms,
                "message": outcome.message,
            }
            for outcome in response.engine_outcomes
        ],
        "suggestions": list(response.suggestions),
        "answers": list(response.answers),
        "corrections": list(response.corrections),
        "infoboxes": list(response.infoboxes),
        "query_id": response.query_id,
        "cached": False,
        "response_time_ms": response.response_time_ms,
        "partial": response.partial,
        "all_unresponsive": response.all_unresponsive,
        "empty_engines": [list(entry) for entry in response.empty_engines],
        "cached_error": response.cached_error,
    }


def search_response_from_payload(payload: dict[str, Any]) -> SearchResponse:
    """Rehydrate a :class:`SearchResponse` from a cached payload dict."""
    scope = payload.get("scope") or {}
    decision = ScopeDecision(
        selected_engines=list(scope.get("selected_engines") or []),
        resolved_categories=list(scope.get("resolved_categories") or []),
        routing_rule=str(scope.get("routing_rule") or ""),
        matched_topic=scope.get("matched_topic"),
        warnings=list(scope.get("warnings") or []),
        excluded_engines=[
            EngineExclusion(engine=str(item.get("engine", "")), reason=str(item.get("reason", "")))
            for item in (scope.get("excluded_engines") or [])
        ],
    )
    return SearchResponse(
        query=str(payload.get("query", "")),
        results=[search_result_from_dict(item) for item in (payload.get("results") or [])],
        scope=decision,
        engine_outcomes=[engine_outcome_from_dict(item) for item in (payload.get("engine_outcomes") or [])],
        suggestions=[str(item) for item in (payload.get("suggestions") or [])],
        answers=list(payload.get("answers") or []),
        corrections=[str(item) for item in (payload.get("corrections") or [])],
        infoboxes=list(payload.get("infoboxes") or []),
        query_id=str(payload.get("query_id", "")),
        cached=bool(payload.get("cached", False)),
        response_time_ms=int(payload.get("response_time_ms", 0)),
        partial=bool(payload.get("partial", False)),
        all_unresponsive=bool(payload.get("all_unresponsive", False)),
        empty_engines=[[str(item) for item in entry] for entry in (payload.get("empty_engines") or [])],
        cached_error=bool(payload.get("cached_error", False)),
    )


def _rehydrate_engines(value: Any) -> set[str]:
    """Coerce a serialized ``engines`` value back into a ``set[str]``.

    Accepts a JSON list of engine names (the canonical form) and, for
    backward compatibility, the legacy stringified-set repr such as
    ``"{'arxiv', 'github'}"`` — never iterating the string's characters.
    """
    if value is None:
        return set()
    if isinstance(value, str):
        text = value.strip()
        if text.startswith("{") and text.endswith("}"):
            inner = text[1:-1]
            return set(part.strip().strip("'\"") for part in inner.split(",") if part.strip())
        # A single engine name written as a bare string.
        return set(text.split())
    if isinstance(value, (list, tuple, set, frozenset)):
        return {str(item) for item in value}
    return set()


def search_result_from_dict(data: dict[str, Any]) -> SearchResult:
    """Rehydrate a :class:`SearchResult` from a serialized dict."""
    raw_category = data.get("category")
    raw_score = data.get("score")
    raw_position = data.get("position")
    raw_tier = data.get("tier")
    return SearchResult(
        url=str(data.get("url", "")),
        title=str(data.get("title", "")),
        content=str(data.get("content", "")),
        engine=str(data.get("engine", "")),
        engines=_rehydrate_engines(data.get("engines")),
        score=float(raw_score) if raw_score is not None else 0.0,
        position=int(raw_position) if raw_position is not None else 0,
        category=raw_category if raw_category is not None else "general",
        published_date=data.get("published_date"),
        thumbnail=data.get("thumbnail"),
        img_src=data.get("img_src"),
        tier=int(raw_tier) if raw_tier is not None else 1,
        payload=payload_from_dict(data.get("payload")),
    )


def engine_outcome_from_dict(data: dict[str, Any]) -> EngineOutcome:
    """Rehydrate an :class:`EngineOutcome` from a serialized dict."""
    latency = data.get("latency_ms")
    return EngineOutcome(
        engine=str(data.get("engine", "")),
        status=str(data.get("status", "error")),
        result_count=int(data.get("result_count", 0)),
        latency_ms=float(latency) if latency is not None else None,
        message=data.get("message"),
    )
