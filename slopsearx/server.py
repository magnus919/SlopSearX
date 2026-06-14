"""FastAPI HTTP server — /search and /health endpoints.

Implements the full SearXNG-compatible API contract with graceful
degradation: scrape-engine failures never block the response.
"""

from __future__ import annotations

import asyncio
import dataclasses
import os
import time
import uuid
from contextlib import asynccontextmanager
from typing import Any, AsyncIterator

from fastapi import FastAPI, Query, Request
from fastapi.responses import JSONResponse, PlainTextResponse

import engines  # noqa: F401 — triggers @register_engine to populate registry
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
from slopsearx.config import load_config
from slopsearx.formatter import format_json, format_yaml_markdown
from slopsearx.merger import (
    PresenceRanker,
    build_meta,
    extract_unresponsive,
)
from slopsearx.ratelimit import (
    LocalTokenBucket,
    RateLimiter,
    RateLimitStrategy,
    ValkeySlidingWindow,
)
from slopsearx.router import QueryRouter
from slopsearx.stats import EngineStatsTracker
from slopsearx.suggest import SuggestionService

# ---------------------------------------------------------------------------
# Two-tier engine classification
# ---------------------------------------------------------------------------
# Tier 1: broad, general-purpose engines that return relevant results on
# any query. Used as the primary result set in unscoped searches.
# Tier 2: specialised engines (science, packages, security, etc.) whose
# results are surfaced below Tier 1 in unscoped searches.
# All new engines default to Tier 2 unless approved by maintainers.
_TIER1_ENGINES: set[str] = {
    "brave", "duckduckgo", "google", "wikipedia", "stackexchange", "reddit",
}


# Populated at startup
_active_engines: dict[str, EngineAdapter] = {}
_ranker = PresenceRanker()
_cache: SearchCache | None = None
_rate_limiter: RateLimiter | None = None
_router: QueryRouter | None = None
_suggestion_service: SuggestionService | None = None
_stats_tracker: EngineStatsTracker | None = None
_audit_logger: QueryAuditLogger | None = None

# Concurrency and per-client rate limiting
_engine_semaphore: asyncio.Semaphore | None = None
_client_rate_window: RateLimitStrategy | None = None


# ---------------------------------------------------------------------------
# Lifecycle
# ---------------------------------------------------------------------------


async def _startup() -> None:
    """Discover and warm up all registered engines."""
    global _active_engines, _cache, _rate_limiter  # noqa: PLW0603
    global _engine_semaphore, _client_rate_window  # noqa: PLW0603

    # Initialize cache (gracefully degrades if Valkey unavailable)
    _cache = SearchCache()
    await _cache.connect()

    # Initialize rate limiter (default: local token bucket for dev)
    _rate_limiter = RateLimiter(LocalTokenBucket())

    await _rate_limiter.warmup()

    # Initialize global engine dispatch semaphore
    max_conc_str = os.environ.get("MAX_CONCURRENT_ENGINES", "10")
    try:
        max_conc = int(max_conc_str)
    except (ValueError, TypeError):
        max_conc = 10  # non-numeric defaults to 10
    if max_conc < 1:
        max_conc = 1  # zero/negative defaults to 1
    _engine_semaphore = asyncio.Semaphore(max_conc)

    # Initialize per-client rate limiter
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
    _fail_closed = fail_closed_raw.strip().lower() in ("true", "1", "yes")

    # Parse FAIL_CLOSED_GRACE_SECONDS env var
    try:
        _fail_closed_grace = float(os.environ.get("FAIL_CLOSED_GRACE_SECONDS", "30"))
    except (ValueError, TypeError):
        _fail_closed_grace = 30.0

    _client_rate_window = ValkeySlidingWindow(
        valkey_url=os.environ.get("VALKEY_URL", ""),
        default_rate=per_client_rate,
        window_seconds=per_client_window,
        fail_closed=_fail_closed,
        fail_closed_grace_seconds=_fail_closed_grace,
    )
    await _client_rate_window.warmup()

    # Only populate if not already set (allows test fixtures to pre-seed)
    cfg = load_config()
    if not _active_engines:
        engine_configs = {name: dataclasses.asdict(entry) for name, entry in cfg.engines.items()}
        _active_engines = discover_engines(engine_configs)

    # Inject rate limiter into each engine
    for engine in _active_engines.values():
        engine.rate_limiter = _rate_limiter

    # Warm up engines concurrently
    warmup_tasks = []
    for name, engine in _active_engines.items():
        warmup_tasks.append(_warmup_engine(name, engine))
    await asyncio.gather(*warmup_tasks, return_exceptions=True)

    # Initialize query router
    global _router  # noqa: PLW0603
    router_cfg = dataclasses.asdict(cfg.routing)
    _router = QueryRouter(routing_config=router_cfg)

    # Initialize suggestion service (opt-in: defaults to off to avoid extra Brave Suggest API calls)
    global _suggestion_service  # noqa: PLW0603
    _suggestion_service = None
    if cfg.enable_suggestions:
        brave_api_key = (cfg.engines.get("brave").api_key  # type: ignore[union-attr]
                         or "")
        if brave_api_key:
            _suggestion_service = SuggestionService(brave_api_key=brave_api_key, cache=_cache)

    # Initialize quality stats tracker
    global _stats_tracker  # noqa: PLW0603
    _stats_tracker = EngineStatsTracker(cache=_cache)

    # Initialize query audit logger
    global _audit_logger  # noqa: PLW0603
    _audit_logger = QueryAuditLogger(cache=_cache)


async def _shutdown() -> None:
    """Gracefully shut down all engines, cache, and rate limiter."""
    shutdown_tasks = []
    for name, engine in _active_engines.items():
        shutdown_tasks.append(_shutdown_engine(name, engine))
    await asyncio.gather(*shutdown_tasks, return_exceptions=True)

    if _rate_limiter is not None:
        await _rate_limiter.shutdown()

    if _client_rate_window is not None:
        await _client_rate_window.shutdown()

    if _cache is not None:
        await _cache.close()


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncIterator[None]:
    """Application lifespan — startup and shutdown."""
    await _startup()
    yield
    await _shutdown()

app = FastAPI(title="SlopSearX", version="0.1.0", lifespan=lifespan)


async def _warmup_engine(name: str, engine: EngineAdapter) -> None:
    try:
        await engine.warmup()
    except Exception:
        pass  # Warmup failure is non-fatal


async def _shutdown_engine(name: str, engine: EngineAdapter) -> None:
    try:
        await engine.shutdown()
    except Exception:
        pass


# ---------------------------------------------------------------------------
# /health
# ---------------------------------------------------------------------------


@app.get("/health")
async def health() -> dict[str, Any]:
    """Health check with per-engine status and Valkey connectivity.

    Returns 200 even if some engines are unhealthy — engine status
    is reported in the response body.
    """
    engine_statuses: dict[str, dict[str, Any]] = {}

    if not _active_engines:
        return {"status": "ok", "version": "0.1.0", "engines": {}}

    # Run engine health checks concurrently
    async def check_engine(name: str, engine: EngineAdapter) -> tuple[str, EngineStatus]:
        try:
            status = await engine.health()
        except Exception:
            status = EngineStatus.ERROR
        return name, status

    tasks = [check_engine(name, eng) for name, eng in _active_engines.items()]
    results = await asyncio.gather(*tasks, return_exceptions=True)

    for result in results:
        if isinstance(result, tuple):
            name, status = result
            engine_statuses[name] = {"status": status.value}

    all_ok = all(e["status"] == "ok" for e in engine_statuses.values())

    # Check Valkey connectivity for rate limiting
    valkey_connected: bool = False
    valkey_device = _client_rate_window
    if valkey_device is not None and isinstance(valkey_device, ValkeySlidingWindow):
        valkey_connected = valkey_device._connected

    # Degrade status if Valkey is unreachable and fail-closed is enabled
    overall_status = "ok" if all_ok else "degraded"
    if not valkey_connected and isinstance(valkey_device, ValkeySlidingWindow):
        if valkey_device._fail_closed:
            overall_status = "degraded"

    return {
        "status": overall_status,
        "version": "0.1.0",
        "valkey_connected": valkey_connected,
        "engines": engine_statuses,
    }


# ---------------------------------------------------------------------------
# /metrics
# ---------------------------------------------------------------------------


@app.get("/metrics")
async def metrics() -> PlainTextResponse:
    """OpenMetrics endpoint for Prometheus scraping."""

    return PlainTextResponse(content=m.render_metrics(), media_type="text/plain; version=0.0.4")


# ---------------------------------------------------------------------------
# /config
# ---------------------------------------------------------------------------


@app.get("/config")
async def config() -> dict[str, Any]:
    """SearXNG-compatible config endpoint.

    Returns available categories and their engines. Built from
    instantiated engines (respects config overrides).
    """
    from collections import defaultdict

    cats: dict[str, list[str]] = defaultdict(list)
    for name, engine in _active_engines.items():
        for cat in engine.categories:
            cats[cat].append(name)
    return {"categories": dict(cats)}


# ---------------------------------------------------------------------------
# /search
# ---------------------------------------------------------------------------


@app.get("/search")
async def search(
    request: Request,
    q: str = Query(default="", description="Search query"),
    format: str = Query(default="json", description="Response format: json, yaml"),
    categories: str = Query(default="", description="Comma-separated category filter"),
    engines_param: str = Query(default="", alias="engines", description="Comma-separated engine filter"),
    language: str = Query(default="en", description="Language code"),
    pageno: int = Query(default=1, ge=1, description="Page number"),
    time_range: str = Query(default="", description="Time range: day, month, year"),
    safesearch: int = Query(default=0, ge=0, le=2, description="SafeSearch: 0=off, 1=moderate, 2=strict"),
) -> Any:
    """Execute a search across all enabled engines.

    Accepts all standard SearXNG query parameters. Returns JSON by
    default; set ``format=yaml`` for agent-native YAML+Markdown output.

    Graceful degradation: scrape-engine failures never block the
    response. Failing engines are reported in ``unresponsive_engines``
    and their results are omitted.
    """
    query_id = _generate_query_id()
    t_start = time.monotonic()

    # Increment request counter
    m.server_requests.inc({})

    # Validate query
    if not q.strip():
        return JSONResponse(
            status_code=400,
            content={
                "error": "query_required",
                "message": "The 'q' parameter is required.",
            },
        )

    # Determine which engines to query
    if engines_param.strip():
        # Explicit engine list wins over category filter
        requested = [e.strip() for e in engines_param.split(",") if e.strip()]
        target_engines = {
            name: eng
            for name, eng in _active_engines.items()
            if name in requested
        }
    else:
        target_engines = dict(_active_engines)
        # Category filter (only when engines not explicitly specified)
        cat_list = [c.strip() for c in categories.split(",") if c.strip()]
        if cat_list:
            target_engines = {
                name: eng
                for name, eng in target_engines.items()
                if any(c in eng.categories for c in cat_list)
            }
        elif _router is not None:
            # No category filter — try query-based routing
            routed = _router.route(q)
            if routed is not None:
                target_engines = {
                    name: eng
                    for name, eng in _active_engines.items()
                    if name in routed
                }
            else:
                # No topic matched — restrict to Tier 1 (broad, general-purpose)
                # engines only. Specialty engines require an explicit category
                # or topic match to avoid polluting unscoped results.
                tier1 = {
                    name: eng
                    for name, eng in _active_engines.items()
                    if name in _TIER1_ENGINES
                }
                # Fall back to all engines if no Tier 1 engines are active,
                # so the server returns results (even from specialty engines)
                # rather than a hard 503 error with no engines available.
                target_engines = tier1 if tier1 else dict(_active_engines)

    if not target_engines:
        # No engines available at all
        response_data = format_json(
            results=[],
            query=q,
            unresponsive_engines=[["all", "no engines available"]],
            meta={
                "response_time_ms": round((time.monotonic() - t_start) * 1000),
                "cached": False,
                "query_id": query_id,
                "engine_status": {},
            },
        )
        return JSONResponse(status_code=503, content=response_data)

    # Build search params
    search_params: dict[str, Any] = {
        "language": language,
        "safesearch": safesearch,
        "pageno": pageno,
        "time_range": time_range if time_range else None,
        "categories": [c.strip() for c in categories.split(",") if c.strip()] or ["general"],
    }

    # Per-client rate limiting — checked before semaphore acquisition
    if _client_rate_window is not None:
        client_ip = request.client.host if request.client else "unknown"
        allowed = await _client_rate_window.acquire(client_ip, cost=1)
        if not allowed:
            return JSONResponse(
                status_code=429,
                content={
                    "error": "rate_limited",
                    "message": "Too many requests. Please slow down.",
                },
            )

    # Check cache first
    if _cache is not None and _cache.is_connected:
        ck = cache_key(q, language, safesearch)
        cached = await _cache.get(ck)
        if cached is not None:
            # Negative cache hit — return 503 without dispatching
            if cached.get("_error"):
                return JSONResponse(
                    status_code=503,
                    content={
                        "error": "service_unavailable",
                        "message": "Temporarily unavailable (cached error)",
                        "meta": {"cached": True, "query_id": query_id},
                    },
                )
            cached["meta"]["cached"] = True
            return JSONResponse(status_code=200, content=cached)

    # Check answer cache (broader key, independent of language/safesearch)
    if _cache is not None and _cache.is_connected:
        answer_cached = await _cache.get_answer(q)
        if answer_cached is not None:
            if answer_cached.get("_error"):
                return JSONResponse(
                    status_code=503,
                    content={
                        "error": "service_unavailable",
                        "message": "Temporarily unavailable (cached error)",
                        "meta": {"cached": True, "query_id": query_id},
                    },
                )
            answer_cached["meta"]["cached"] = True
            return JSONResponse(status_code=200, content=answer_cached)

    # Dispatch to all engines concurrently (bounded by semaphore)
    tasks = []
    engine_names = []
    circuit_open_names: list[str] = []
    for name, engine in target_engines.items():
        if not engine.circuit_allowed():
            circuit_open_names.append(name)
            continue
        tasks.append(_dispatch_with_semaphore(name, engine, q, search_params))
        engine_names.append(name)

    # Fire suggestion fetch concurrently with engine dispatch (background)
    suggestions_task = asyncio.ensure_future(_generate_suggestions(q))

    dispatch_results = await asyncio.gather(*tasks, return_exceptions=True)

    # Collect results and metadata
    engine_results: dict[str, list[SearchResult]] = {}
    responses: dict[str, AdapterResponse] = {}
    for name, raw in zip(engine_names, dispatch_results):
        if isinstance(raw, BaseException):
            # Engine raised unexpectedly — classify as error
            result: AdapterResponse = AdapterResponse(
                results=[],
                status=EngineStatus.ERROR,
                error_message=str(raw),
            )
        else:
            result = raw

        # Update circuit breaker state
        engine = target_engines[name]
        if result.status in (EngineStatus.ERROR, EngineStatus.TIMEOUT):
            engine.record_failure()
        else:
            engine.record_success()

        responses[name] = result
        engine_results[name] = result.results

        # Annotate each result with its tier for unscoped searches
        tier = 1 if name in _TIER1_ENGINES else 2
        for sr in result.results:
            sr.tier = tier

        # Record per-engine metrics
        m.engine_queries.inc({"engine": name})
        m.engine_latency.observe({"engine": name}, result.latency_ms / 1000.0)
        degraded = (EngineStatus.TIMEOUT, EngineStatus.RATE_LIMITED)
        status_code = 0 if result.status == EngineStatus.OK else (1 if result.status in degraded else 2)
        m.engine_status.set({"engine": name}, status_code)

        # Record per-engine quality telemetry in Valkey (non-blocking)
        if _stats_tracker is not None:
            avg_score = (
                sum(r.score for r in result.results) / len(result.results)
                if result.results else 0.0
            )
            asyncio.create_task(
                _stats_tracker.record_query(
                    engine=name,
                    result_count=len(result.results),
                    latency_ms=result.latency_ms,
                    status=result.status,
                    avg_score=avg_score,
                )
            )

    # Add circuit-open engines as unresponsive (no metrics — they were never dispatched)
    for name in circuit_open_names:
        responses[name] = AdapterResponse(
            results=[],
            status=EngineStatus.ERROR,
            error_message="circuit open",
        )

    # Merge and rank
    ranked = _ranker.rank(engine_results, q, search_params)

    # Metadata
    elapsed_ms = (time.monotonic() - t_start) * 1000
    unresponsive = extract_unresponsive(responses)
    meta = build_meta(responses, elapsed_ms, query_id)

    # Check if ALL engines are unresponsive
    all_unresponsive = all(
        resp.status != EngineStatus.OK for resp in responses.values()
    )

    # Build suggestions from engine suggest APIs (already running in background)
    suggestions = await suggestions_task

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

    if format == "yaml":
        engine_count = len(target_engines)
        responsive_count = sum(
            1 for resp in responses.values() if resp.status == EngineStatus.OK
        )
        yaml_output = format_yaml_markdown(
            ranked,
            q,
            meta=meta,
            engine_count=engine_count,
            responsive_count=responsive_count,
            unresponsive_engines=unresponsive,
        )
        return PlainTextResponse(content=yaml_output, media_type="text/vnd.yaml+markdown")

    # Default: JSON
    response_data = format_json(
        results=ranked,
        query=q,
        answers=all_answers,
        corrections=all_corrections,
        infoboxes=all_infoboxes,
        suggestions=suggestions,
        unresponsive_engines=unresponsive,
        meta=meta,
    )

    status_code = 503 if all_unresponsive else 200
    response = JSONResponse(status_code=status_code, content=response_data)

    # Cache the result set (even partial results are cacheable)
    if _cache is not None and _cache.is_connected and not all_unresponsive:
        cat_list = search_params.get("categories", [])
        ck = cache_key(q, language, safesearch)
        ttl = _ttl_for_query(cat_list)
        await _cache.set(ck, response_data, ttl)
        # Also cache in answer cache (broader key, skip for time-sensitive queries)
        if not time_range:
            await _cache.set_answer(q, response_data)

    # Record audit trail (fire-and-forget)
    if _audit_logger is not None:
        client_ip = request.client.host if request.client else "unknown"
        asyncio.create_task(
            _audit_logger.record_query(
                query=q,
                client_ip=client_ip,
                engine_results=responses,
                latency_ms=elapsed_ms,
            )
        )

    return response


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


async def _dispatch_engine(
    name: str,
    engine: EngineAdapter,
    query: str,
    params: dict[str, Any],
    timeout_s: float = 3.0,
) -> AdapterResponse:
    """Dispatch a query to one engine with a timeout.

    Returns AdapterResponse — never raises. Timeouts are caught and
    returned as EngineStatus.TIMEOUT.
    """
    try:
        result = await asyncio.wait_for(
            engine.search(query, params),
            timeout=timeout_s,
        )
        return result
    except asyncio.TimeoutError:
        return AdapterResponse(
            results=[],
            status=EngineStatus.TIMEOUT,
            error_message=f"timed out after {timeout_s}s",
            latency_ms=timeout_s * 1000,
        )
    except Exception as exc:
        return AdapterResponse(
            results=[],
            status=EngineStatus.ERROR,
            error_message=sanitize_url(str(exc)),
        )


async def _dispatch_with_semaphore(
    name: str,
    engine: EngineAdapter,
    query: str,
    params: dict[str, Any],
    timeout_s: float = 3.0,
) -> AdapterResponse:
    """Dispatch engine query, bounded by the global semaphore.

    Acquires a semaphore slot before dispatching to the engine.
    The ``async with`` block ensures the slot is released even
    when the underlying dispatch raises an exception or times out.
    If no semaphore has been configured (startup not yet complete),
    dispatches directly with no bound.
    """
    if _engine_semaphore is not None:
        async with _engine_semaphore:
            return await _dispatch_engine(name, engine, query, params, timeout_s)
    return await _dispatch_engine(name, engine, query, params, timeout_s)


def _generate_query_id() -> str:
    """Generate a short, traceable query identifier."""
    return f"ssx-{uuid.uuid4().hex[:8]}"


async def _generate_suggestions(query: str) -> list[str]:
    """Fetch search suggestions from engine suggest APIs.

    Uses the SuggestionService (Brave Suggest API primary, DDG fallback).
    Results are cached in Valkey for 30 minutes.
    Returns empty list on failure (graceful degradation).
    """
    if _suggestion_service is None:
        return []
    return await _suggestion_service.fetch(query)
