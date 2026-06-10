"""FastAPI HTTP server — /search and /health endpoints.

Implements the full SearXNG-compatible API contract with graceful
degradation: scrape-engine failures never block the response.
"""

from __future__ import annotations

import asyncio
import dataclasses
import time
import uuid
from typing import Any

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
)
from slopsearx.cache import SearchCache, _ttl_for_query, cache_key
from slopsearx.config import load_config
from slopsearx.formatter import format_json, format_yaml_markdown
from slopsearx.merger import (
    PresenceRanker,
    build_meta,
    extract_unresponsive,
)
from slopsearx.ratelimit import LocalTokenBucket, RateLimiter

app = FastAPI(title="SlopSearX", version="0.1.0")

# Populated at startup
_active_engines: dict[str, EngineAdapter] = {}
_ranker = PresenceRanker()
_cache: SearchCache | None = None
_rate_limiter: RateLimiter | None = None


# ---------------------------------------------------------------------------
# Lifecycle
# ---------------------------------------------------------------------------


@app.on_event("startup")
async def startup() -> None:
    """Discover and warm up all registered engines."""
    global _active_engines, _cache, _rate_limiter  # noqa: PLW0603

    # Initialize cache (gracefully degrades if Valkey unavailable)
    _cache = SearchCache()

    # Initialize rate limiter (default: local token bucket for dev)
    _rate_limiter = RateLimiter(LocalTokenBucket())

    await _rate_limiter.warmup()

    # Only populate if not already set (allows test fixtures to pre-seed)
    if not _active_engines:
        cfg = load_config()
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


@app.on_event("shutdown")
async def shutdown() -> None:
    """Gracefully shut down all engines, cache, and rate limiter."""
    shutdown_tasks = []
    for name, engine in _active_engines.items():
        shutdown_tasks.append(_shutdown_engine(name, engine))
    await asyncio.gather(*shutdown_tasks, return_exceptions=True)

    if _rate_limiter is not None:
        await _rate_limiter.shutdown()


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
    """Health check with per-engine status.

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
    return {
        "status": "ok" if all_ok else "degraded",
        "version": "0.1.0",
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

    # Check cache first
    if _cache is not None and _cache.is_connected:
        ck = cache_key(q, language, safesearch)
        cached = await _cache.get(ck)
        if cached is not None:
            cached["meta"]["cached"] = True
            return JSONResponse(status_code=200, content=cached)

    # Dispatch to all engines concurrently
    tasks = []
    engine_names = []
    for name, engine in target_engines.items():
        tasks.append(_dispatch_engine(name, engine, q, search_params))
        engine_names.append(name)

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
        responses[name] = result
        engine_results[name] = result.results

        # Record per-engine metrics
        m.engine_queries.inc({"engine": name})
        m.engine_latency.observe({"engine": name}, result.latency_ms / 1000.0)
        degraded = (EngineStatus.TIMEOUT, EngineStatus.RATE_LIMITED)
        status_code = 0 if result.status == EngineStatus.OK else (1 if result.status in degraded else 2)
        m.engine_status.set({"engine": name}, status_code)

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

    # Build suggestions from query (simple approach: strip and split)
    suggestions = _generate_suggestions(q)

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
            error_message=str(exc),
        )


def _generate_query_id() -> str:
    """Generate a short, traceable query identifier."""
    return f"ssx-{uuid.uuid4().hex[:8]}"


def _generate_suggestions(query: str) -> list[str]:
    """Generate simple query suggestions.

    This is a minimal stub — no external suggestion API is called.
    Real suggestion generation would use a search engine's suggest API
    or NLP model.
    """
    return []
