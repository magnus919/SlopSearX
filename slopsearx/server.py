"""FastAPI HTTP server — /search and /health endpoints.

Implements the full SearXNG-compatible API contract with graceful
degradation: scrape-engine failures never block the response.

The search pipeline itself lives in :mod:`slopsearx.service`
(:class:`SearchService`); this module is a thin adapter from HTTP query
parameters to the normalized service and back to SearXNG JSON/YAML.
The MCP server uses the same service, so both surfaces share scope
resolution, ranking, deduplication, caching, and failure semantics.
"""

from __future__ import annotations

import asyncio
import re
from contextlib import asynccontextmanager
from typing import Any, AsyncIterator

from fastapi import FastAPI, Query, Request
from fastapi.responses import JSONResponse, PlainTextResponse

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx import metrics as m
from slopsearx.adapter import EngineAdapter
from slopsearx.audit import QueryAuditLogger
from slopsearx.cache import SearchCache
from slopsearx.capabilities import CapabilityCatalog, build_engine_health
from slopsearx.config import Config, load_config
from slopsearx.formatter import format_json, format_yaml_markdown
from slopsearx.logging import setup_logging
from slopsearx.middleware import RequestIDMiddleware
from slopsearx.ratelimit import RateLimiter, RateLimitStrategy, ValkeySlidingWindow
from slopsearx.router import QueryRouter
from slopsearx.routing import RoutingBudget, load_routing_budget

# ---------------------------------------------------------------------------
# Two-tier engine classification
# ---------------------------------------------------------------------------
# Tier 1: broad, general-purpose engines that return relevant results on
# any query. Used as the primary result set in unscoped searches.
# Tier 2: specialised engines (science, packages, security, etc.) whose
# results are surfaced below Tier 1 in unscoped searches.
# All new engines default to Tier 2 unless approved by maintainers.
# The canonical definition lives in slopsearx.service.
from slopsearx.service import DEFAULT_TIER1_ENGINES as _TIER1_ENGINES
from slopsearx.service import (
    AppContext,
    QueryValidationError,
    RateLimitExceededError,
    SearchRequest,
    SearchService,
    build_context,
    build_response_meta,
    destroy_context,
    unresponsive_from_outcomes,
)
from slopsearx.stats import EngineStatsTracker
from slopsearx.suggest import SuggestionService

# Populated at startup
_active_engines: dict[str, EngineAdapter] = {}
_cache: SearchCache | None = None
_rate_limiter: RateLimiter | None = None
_router: QueryRouter | None = None
_suggestion_service: SuggestionService | None = None
_stats_tracker: EngineStatsTracker | None = None
_audit_logger: QueryAuditLogger | None = None
_empty_scrape_diagnostics_enabled = False

# Concurrency and per-client rate limiting
_engine_semaphore: asyncio.Semaphore | None = None
_client_rate_window: RateLimitStrategy | None = None


# ---------------------------------------------------------------------------
# Lifecycle
# ---------------------------------------------------------------------------


async def _startup() -> None:
    """Discover and warm up all registered engines."""
    setup_logging()
    global _active_engines, _cache, _rate_limiter  # noqa: PLW0603
    global _engine_semaphore, _client_rate_window  # noqa: PLW0603
    global _empty_scrape_diagnostics_enabled  # noqa: PLW0603
    global _router, _suggestion_service, _stats_tracker, _audit_logger  # noqa: PLW0603
    global _routing_budget_cache  # noqa: PLW0603

    ctx = await build_context()

    # Only populate engines if not already set (allows test fixtures to
    # pre-seed); the rest of the wiring always comes from the context.
    if not _active_engines:
        _active_engines = ctx.active_engines
    _cache = ctx.cache
    _rate_limiter = ctx.rate_limiter
    _router = ctx.router
    _suggestion_service = ctx.suggestion_service
    _stats_tracker = ctx.stats_tracker
    _audit_logger = ctx.audit_logger
    _engine_semaphore = ctx.engine_semaphore
    _client_rate_window = ctx.client_rate_window
    _empty_scrape_diagnostics_enabled = ctx.empty_scrape_diagnostics_enabled
    # Freeze the routing budget from the startup context (resolved once,
    # beside the config/catalog snapshot) so the HTTP routed scope/digest
    # never track a runtime ``ROUTING_*`` env change.
    _routing_budget_cache = ctx.routing_budget


async def _shutdown() -> None:
    """Gracefully shut down all engines, cache, and rate limiter."""
    await destroy_context(_current_context())


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncIterator[None]:
    """Application lifespan — startup and shutdown."""
    del app
    await _startup()
    yield
    await _shutdown()


app = FastAPI(title="SlopSearX", version="0.1.0", lifespan=lifespan)
app.add_middleware(RequestIDMiddleware)


def _routing_catalog() -> CapabilityCatalog | None:
    """Return the capability catalog for scope resolution, or None on failure.

    Reuses the memoized /health catalog (rebuilt when ``_active_engines`` is
    rebound), so the resolver and the health probe share one live catalog
    without extra registry walks per request. A catalog failure degrades the
    resolver to its deterministic fallback (``routing_fallback``), never a 500.
    """
    try:
        return _health_catalog()
    except Exception:  # noqa: BLE001 — routing must degrade, never raise
        return None


def _current_context() -> AppContext:
    """Build an AppContext snapshot from the live module globals.

    Rebuilt per request so test fixtures and runtime overrides that
    mutate the module-level state are honored.
    """
    return AppContext(
        active_engines=_active_engines,
        cache=_cache,
        rate_limiter=_rate_limiter,
        router=_router,
        suggestion_service=_suggestion_service,
        stats_tracker=_stats_tracker,
        audit_logger=_audit_logger,
        engine_semaphore=_engine_semaphore,
        client_rate_window=_client_rate_window,
        tier1_engines=_TIER1_ENGINES,
        empty_scrape_diagnostics_enabled=_empty_scrape_diagnostics_enabled,
        catalog=_routing_catalog(),
        routing_budget=_routing_budget_snapshot(),
    )


def _routing_budget_snapshot() -> RoutingBudget:
    """Return the operator routing budget, frozen at startup (memoized).

    ``load_routing_budget`` re-reads the ``ROUTING_*`` env vars on every
    call. The memo captures the value once — seeded by ``_startup`` from the
    startup context, beside the memoized config/catalog — so a runtime env
    change cannot silently alter the HTTP routed scope/digest and
    desynchronize it from the MCP lifespan budget, which resolves the same
    env a single time at startup (routing-coherence followup).
    """
    global _routing_budget_cache  # noqa: PLW0603
    if _routing_budget_cache is None:
        _routing_budget_cache = load_routing_budget(_health_config())
    return _routing_budget_cache


# ---------------------------------------------------------------------------
# /health
# ---------------------------------------------------------------------------

# Memoized startup snapshot for the /health probe path. The probe is polled
# continuously (k8s liveness/readiness every 10s/30s, Docker HEALTHCHECK),
# so it must not re-read config.yaml, re-scan ``ENGINE_*``/``SEARCH_*`` env
# vars, or rebuild the capability catalog (a registry walk) on every call.
# The snapshot reflects the startup state the running adapters were built
# from, so ``configured``/``auth_configured`` never silently contradict
# runtime reality when a config file or env var changes after boot.
_health_config_cache: Config | None = None
_health_catalog_cache: CapabilityCatalog | None = None
_health_catalog_engines: dict[str, EngineAdapter] | None = None

# Startup-frozen routing budget. ``load_routing_budget`` reads ``ROUTING_*``
# env vars, so it is resolved once beside the config/catalog snapshot
# (seeded in ``_startup``) instead of per request — the routed scope and its
# cache digest must freeze at startup exactly like the MCP lifespan budget.
_routing_budget_cache: RoutingBudget | None = None


def _health_config() -> Config:
    """Return the startup config snapshot for the health probe (memoized).

    ``load_config()`` re-reads the YAML file and re-scans every env var; the
    memo captures it once so a continuous probe does no disk I/O or env scan
    and a runtime env change cannot silently alter what the running adapters
    report as configured.
    """
    global _health_config_cache  # noqa: PLW0603
    if _health_config_cache is None:
        _health_config_cache = load_config()
    return _health_config_cache


def _health_catalog() -> CapabilityCatalog:
    """Return the capability catalog for the health probe (memoized).

    Rebuilt only when the ``_active_engines`` mapping is replaced (test
    fixtures and runtime overrides rebind it); steady-state probes reuse the
    cached catalog so the path does no registry walk and no config I/O while
    still reflecting the running adapters' live observed health.
    """
    global _health_catalog_cache, _health_catalog_engines  # noqa: PLW0603
    if _health_catalog_cache is None or _health_catalog_engines is not _active_engines:
        _health_catalog_cache = CapabilityCatalog(config=_health_config(), adapters=_active_engines)
        _health_catalog_engines = _active_engines
    return _health_catalog_cache


@app.get("/health")
async def health() -> dict[str, Any]:
    """Health check — server liveness, Valkey connectivity, and observed engine health.

    Does NOT probe external search APIs. Engine health is *observed* from
    classified search outcomes (see ``slopsearx.adapter.EngineAdapter``) and
    reported with a consistent status vocabulary, freshness timestamp, and
    distinct circuit/auth signals. A configured-but-never-observed engine is
    ``unknown``, never ``ok`` (issue 190).

    The probe path is cheap and exception-proof: the layered config and the
    capability catalog are captured once (memoized) instead of re-read or
    rebuilt on every poll, and a config/catalog failure degrades to a minimal
    liveness record instead of a 500.
    """
    try:
        catalog = _health_catalog()
    except Exception:  # noqa: BLE001 — a liveness probe must never 500
        catalog = None

    engine_health: dict[str, dict[str, Any]] = {}
    for name, adapter in _active_engines.items():
        try:
            capability = catalog.get(name) if catalog is not None else None
        except Exception:  # noqa: BLE001 — one bad engine must not 500 the probe
            capability = None
        engine_health[name] = build_engine_health(name, adapter, capability)

    # Check Valkey connectivity for rate limiting
    valkey_connected: bool = False
    valkey_device = _client_rate_window
    if valkey_device is not None and isinstance(valkey_device, ValkeySlidingWindow):
        valkey_connected = valkey_device._connected

    # Degrade status if Valkey is unreachable and fail-closed is enabled
    overall_status = "ok"
    if not valkey_connected and isinstance(valkey_device, ValkeySlidingWindow):
        if valkey_device._fail_closed:
            overall_status = "degraded"

    return {
        "status": overall_status,
        "version": "0.1.0",
        "valkey_connected": valkey_connected,
        "engines": engine_health,
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
    # Increment request counters
    m.server_requests.inc({})
    m.server_requests_by_format.inc({"format": _safe_metric_label(format)})
    for cat in (c.strip() for c in categories.split(",") if c.strip()):
        m.server_requests_by_category.inc({"category": _safe_metric_label(cat)})

    service = SearchService(_current_context())
    search_request = SearchRequest(
        query=q,
        categories=[c.strip() for c in categories.split(",") if c.strip()],
        engines=[e.strip() for e in engines_param.split(",") if e.strip()],
        language=language,
        page=pageno,
        time_range=time_range if time_range else None,
        safesearch=safesearch,
        client_identifier=request.client.host if request.client else None,
    )

    try:
        response = await service.search(search_request)
    except QueryValidationError:
        return JSONResponse(
            status_code=400,
            content={
                "error": "query_required",
                "message": "The 'q' parameter is required.",
            },
        )
    except RateLimitExceededError:
        return JSONResponse(
            status_code=429,
            content={
                "error": "rate_limited",
                "message": "Too many requests. Please slow down.",
            },
        )

    if response.cached_error:
        # Negative cache hit — 503 without dispatching.
        return JSONResponse(
            status_code=503,
            content={
                "error": "service_unavailable",
                "message": "Temporarily unavailable (cached error)",
                "meta": {"cached": True, "query_id": response.query_id},
            },
        )

    if response.all_unresponsive and not response.engine_outcomes:
        # No engines available at all
        response_data = format_json(
            results=[],
            query=q,
            unresponsive_engines=[["all", "no engines available"]],
            meta={
                "response_time_ms": response.response_time_ms,
                "cached": False,
                "query_id": response.query_id,
                "engine_status": {},
            },
        )
        return JSONResponse(status_code=503, content=response_data)

    unresponsive = unresponsive_from_outcomes(response.engine_outcomes)
    meta = build_response_meta(response)

    if response.cached:
        # Preserve existing behavior: cache hits return the JSON
        # representation regardless of the requested format.
        response_data = format_json(
            results=response.results,
            query=q,
            answers=response.answers,
            corrections=response.corrections,
            infoboxes=response.infoboxes,
            suggestions=response.suggestions,
            unresponsive_engines=unresponsive,
            meta=meta,
        )
        return JSONResponse(status_code=200, content=response_data)

    if format == "yaml":
        engine_count = len(response.scope.selected_engines)
        responsive_count = sum(1 for o in response.engine_outcomes if o.status == "ok")
        yaml_output = format_yaml_markdown(
            response.results,
            q,
            meta=meta,
            engine_count=engine_count,
            responsive_count=responsive_count,
            unresponsive_engines=unresponsive,
        )
        return PlainTextResponse(content=yaml_output, media_type="text/vnd.yaml+markdown")

    # Default: JSON
    response_data = format_json(
        results=response.results,
        query=q,
        answers=response.answers,
        corrections=response.corrections,
        infoboxes=response.infoboxes,
        suggestions=response.suggestions,
        unresponsive_engines=unresponsive,
        meta=meta,
    )

    status_code = 503 if response.all_unresponsive else 200
    return JSONResponse(status_code=status_code, content=response_data)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_MAX_METRIC_LABEL_LEN = 100
_METRIC_LABEL_SAFE = re.compile(r"[^a-zA-Z0-9_./-]")


def _safe_metric_label(value: str) -> str:
    """Sanitize a user-supplied string for use as a Prometheus label value.

    Truncates to 100 chars and replaces unsafe characters with underscores
    to prevent OpenMetrics format corruption and cardinality explosion.
    """
    safe = _METRIC_LABEL_SAFE.sub("_", value)[:_MAX_METRIC_LABEL_LEN]
    return safe or "unknown"
