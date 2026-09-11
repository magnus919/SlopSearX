"""FastAPI HTTP server — /search and /health endpoints.

Implements the full SearXNG-compatible API contract with graceful
degradation: scrape-engine failures never block the response.

The search pipeline itself lives in :mod:`slopsearx.service`
(:class:`SearchService`); this module is a thin adapter from HTTP query
parameters to the normalized service and back to SearXNG HTML/JSON/CSV/RSS
or SlopSearX YAML.
The MCP server uses the same service, so both surfaces share scope
resolution, ranking, deduplication, caching, and failure semantics.
"""

from __future__ import annotations

import asyncio
import os
from collections import defaultdict
from contextlib import asynccontextmanager
from importlib.metadata import PackageNotFoundError
from importlib.metadata import version as package_version
from typing import Any, AsyncIterator
from urllib.parse import parse_qs

from fastapi import FastAPI, Request
from fastapi.exceptions import RequestValidationError
from fastapi.responses import HTMLResponse, JSONResponse, PlainTextResponse, Response

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx import metrics as m
from slopsearx.adapter import EngineAdapter
from slopsearx.audit import QueryAuditLogger
from slopsearx.cache import SearchCache
from slopsearx.capabilities import CapabilityCatalog, MCPPolicy, build_engine_health, load_mcp_policy
from slopsearx.config import Config, load_config
from slopsearx.filters import resolve_filter_enforcement
from slopsearx.formatter import (
    format_csv,
    format_error_html,
    format_html,
    format_json,
    format_landing_page,
    format_rss,
    format_yaml_markdown,
)
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
    SearchFlights,
    SearchRequest,
    SearchResponse,
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
_search_flights = SearchFlights()
_cache: SearchCache | None = None
_rate_limiter: RateLimiter | None = None
_router: QueryRouter | None = None
_suggestion_service: SuggestionService | None = None
_stats_tracker: EngineStatsTracker | None = None
_audit_logger: QueryAuditLogger | None = None
_empty_scrape_diagnostics_enabled = False
_portal_policy: MCPPolicy | None = None

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
    global _portal_policy  # noqa: PLW0603
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
    # Freeze the browser policy beside the startup capability/config snapshot
    # so an environment change cannot alter access mid-process.
    _portal_policy = load_mcp_policy()
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


def _service_version() -> str:
    """Return the installed package version without exposing configuration."""
    try:
        return package_version("slopsearx")
    except PackageNotFoundError:
        return "0.0.0"


def _portal_default_theme() -> str:
    """Return the configured human-portal theme, defaulting to dark."""
    configured = os.getenv("SLOPSEARX_PORTAL_DEFAULT_THEME", "dark").strip().lower()
    return configured if configured in {"dark", "darker"} else "dark"


def _portal_policy_snapshot() -> MCPPolicy:
    """Return the startup-frozen policy used by the browser portal."""
    global _portal_policy  # noqa: PLW0603
    if _portal_policy is None:
        _portal_policy = load_mcp_policy()
    return _portal_policy


def _portal_security_headers(response: Response) -> Response:
    """Apply the portal's browser boundary headers to an HTML response."""
    response.headers["Content-Security-Policy"] = (
        "default-src 'self'; base-uri 'self'; connect-src 'none'; "
        "form-action 'self'; frame-ancestors 'none'; object-src 'none'; "
        "script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; "
        "img-src 'self' data: http: https:"
    )
    response.headers["Referrer-Policy"] = "no-referrer"
    response.headers["X-Content-Type-Options"] = "nosniff"
    response.headers["Permissions-Policy"] = "camera=(), microphone=(), geolocation=()"
    return response


def _portal_restricted_engines(engine_selection: str) -> list[str]:
    """Return explicitly requested sensitive engines without the operator grant."""
    policy = _portal_policy_snapshot()
    if policy.targeted_sensitive_allowed:
        return []
    requested = {name.strip() for name in engine_selection.split(",") if name.strip()}
    return sorted(requested & policy.sensitive_engines)


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
        search_flights=_search_flights,
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
        ranking_strategy=_health_config().ranking.strategy,
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
        engine_health[name] = build_engine_health(adapter, capability)

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


@app.get("/healthz")
async def healthz() -> PlainTextResponse:
    """SearXNG-compatible readiness response."""

    return PlainTextResponse(content="OK")


# ---------------------------------------------------------------------------
# /config
# ---------------------------------------------------------------------------


@app.get("/config")
async def config() -> dict[str, Any]:
    """SearXNG-compatible config endpoint.

    The standard fields are intentionally kept at the top level so existing
    SearXNG clients can consume this response.  SlopSearX capability details
    and the historical category-to-engine mapping are additive fields.
    """
    cats: dict[str, list[str]] = defaultdict(list)
    engines_out: list[dict[str, Any]] = []
    try:
        catalog = _health_catalog()
    except Exception:  # noqa: BLE001 — discovery must never break /config
        catalog = None

    for name, engine in _active_engines.items():
        for cat in engine.categories:
            if name not in cats[cat]:
                cats[cat].append(name)

        record: dict[str, Any] = {
            "categories": list(engine.categories),
            "enabled": True,
            "name": name,
            # Adapters do not declare SearXNG shortcuts; the stable engine
            # name is the least surprising interoperable fallback.
            "shortcut": name,
            "display_name": engine.display_name or name,
            "type": engine.engine_type,
        }
        if catalog is not None:
            try:
                capability = catalog.get(name)
            except Exception:  # noqa: BLE001 — one bad catalog entry is local
                capability = None
            if capability is not None:
                record.update(
                    {
                        "sensitive": capability.sensitive,
                        "supported_filters": capability.supported_filters,
                        "enforced_filters": capability.enforced_filters,
                        "supported_result_types": capability.supported_result_types,
                        "supported_media_types": capability.supported_media_types,
                        "failure_classes": capability.failure_classes,
                        "cost_class": capability.cost_class or None,
                        "auth": {
                            "class": capability.auth_class,
                            "configured": capability.auth_configured,
                        },
                        "scope_hints": capability.scope_hints,
                        "caveats": capability.caveats,
                    }
                )
        engines_out.append(record)

    category_engines = {category: names for category, names in sorted(cats.items())}
    return {
        "autocomplete": "",
        "categories": sorted(category_engines),
        "default_locale": "en",
        "default_theme": "simple",
        "engines": engines_out,
        "instance_name": "SlopSearX",
        "locales": {"en": "English"},
        "plugins": [],
        "safe_search": 0,
        "version": _service_version(),
        "brand": "SlopSearX",
        "category_engines": category_engines,
    }


# ---------------------------------------------------------------------------
# /search
# ---------------------------------------------------------------------------

_STANDARD_FORMATS = frozenset({"html", "json", "csv", "rss"})
_SUPPORTED_FORMATS = _STANDARD_FORMATS | {"yaml"}
_FORMAT_MEDIA_TYPES = {
    "html": "text/html; charset=utf-8",
    "json": "application/json",
    "csv": "text/csv; charset=utf-8",
    "rss": "application/rss+xml; charset=utf-8",
    "yaml": "text/vnd.yaml+markdown; charset=utf-8",
}


def _configured_search_formats() -> set[str]:
    """Return the configured SearXNG formats, normalized to lower case."""
    try:
        return {value.strip().lower() for value in _health_config().search.formats if value.strip()}
    except Exception:  # noqa: BLE001 — format gating must fail closed
        return set()


def _format_from_accept(accept: str) -> str:
    """Choose a format from Accept, defaulting to SearXNG's HTML surface."""
    media_to_format = {
        "text/html": "html",
        "application/xhtml+xml": "html",
        "application/json": "json",
        "application/*+json": "json",
        "text/csv": "csv",
        "application/rss+xml": "rss",
        "application/atom+xml": "rss",
        "application/xml": "rss",
        "text/xml": "rss",
        "text/vnd.yaml+markdown": "yaml",
        "application/yaml": "yaml",
        "text/yaml": "yaml",
    }
    candidates: list[tuple[float, int, str]] = []
    for index, value in enumerate(accept.split(",")):
        parts = [part.strip().lower() for part in value.split(";")]
        media_type = parts[0]
        quality = 1.0
        for part in parts[1:]:
            if part.startswith("q="):
                try:
                    quality = float(part[2:])
                except ValueError:
                    quality = 0.0
        if quality <= 0:
            continue
        selected = media_to_format.get(media_type)
        if selected is not None:
            candidates.append((quality, -index, selected))
    if candidates:
        return max(candidates)[2]
    return "html"


def _portal_state(
    *,
    query: str,
    categories: str,
    engine_selection: str,
    language: str,
    time_range: str,
    safesearch: int,
    page: int,
    response: SearchResponse,
) -> dict[str, Any]:
    """Build the redacted, capability-aware state used by the HTML portal."""
    try:
        catalog = _health_catalog()
    except Exception:  # noqa: BLE001 — the portal still renders without metadata
        catalog = None
    category_options: set[str] = set()
    for name, engine in _active_engines.items():
        capability = None
        if catalog is not None:
            try:
                capability = catalog.get(name)
            except Exception:  # noqa: BLE001 — one bad catalog entry is local
                capability = None
        if capability is not None and capability.sensitive:
            continue
        category_options.update(
            str(category) for category in getattr(engine, "categories", ()) if str(category).strip()
        )

    selected = list(response.scope.selected_engines)
    enforcement: dict[str, dict[str, Any]] = {}
    if time_range:
        enforcement["time_range"] = resolve_filter_enforcement(selected, "time_range", time_range, _active_engines)
    if safesearch:
        enforcement["safesearch"] = resolve_filter_enforcement(selected, "safesearch", safesearch, _active_engines)
    warnings = [str(warning) for warning in response.scope.warnings if str(warning).strip()]
    scope_label = ", ".join(part.strip() for part in categories.split(",") if part.strip()) or "All sources"
    return {
        "query": query,
        "categories": categories,
        "engines": engine_selection,
        "language": language,
        "time_range": time_range,
        "safesearch": safesearch,
        "page": page,
        "category_options": sorted(category_options),
        "scope_label": scope_label,
        "scope_note": warnings[0] if warnings else "",
        "selected_engine_count": len(selected),
        "responsive_engine_count": sum(1 for outcome in response.engine_outcomes if outcome.status == "ok"),
        "filter_enforcement": enforcement,
        "suggestions": list(response.suggestions),
        "all_unresponsive": response.all_unresponsive,
        "json_enabled": "json" in _configured_search_formats(),
    }


def _format_error_response(
    output_format: str,
    status_code: int,
    *,
    error: str,
    message: str,
    field: str | None = None,
    extra: dict[str, Any] | None = None,
) -> Response:
    """Serialize a compatibility error in the selected response format."""
    payload: dict[str, Any] = {"error": error, "message": message}
    if field is not None:
        payload["field"] = field
    if extra:
        payload.update(extra)
    if output_format == "html":
        return _portal_security_headers(HTMLResponse(
            content=format_error_html(error, message, field=field, default_theme=_portal_default_theme()),
            status_code=status_code,
        ))
    if output_format == "csv":
        return PlainTextResponse(
            content=format_csv([]) + f"{error},{message}\n",
            media_type=_FORMAT_MEDIA_TYPES[output_format],
            status_code=status_code,
        )
    if output_format == "rss":
        return PlainTextResponse(
            content=format_rss([], message),
            media_type=_FORMAT_MEDIA_TYPES[output_format],
            status_code=status_code,
        )
    if output_format == "yaml":
        body = "".join(f"{key}: {value!r}\n" for key, value in payload.items())
        return PlainTextResponse(content=body, media_type=_FORMAT_MEDIA_TYPES[output_format], status_code=status_code)
    return JSONResponse(status_code=status_code, content=payload)


def _select_format(request: Request, requested_format: str | None) -> tuple[str, Response | None]:
    """Resolve explicit format/Accept negotiation and enforce format access."""
    if requested_format is not None:
        output_format = requested_format.strip().lower()
        if output_format not in _SUPPORTED_FORMATS:
            return "json", _format_error_response(
                "json",
                400,
                error="unsupported_format",
                message=f"Unsupported response format: {requested_format}",
                field="format",
            )
    else:
        output_format = _format_from_accept(request.headers.get("accept", ""))

    if output_format in _STANDARD_FORMATS and output_format not in _configured_search_formats():
        return output_format, _format_error_response(
            output_format,
            403,
            error="format_disabled",
            message=f"Response format '{output_format}' is disabled.",
            field="format",
        )
    return output_format, None


def _render_search_response(
    output_format: str,
    *,
    results: list[Any],
    query: str,
    status_code: int,
    answers: list[dict[str, Any]] | None = None,
    corrections: list[str] | None = None,
    infoboxes: list[dict[str, Any]] | None = None,
    suggestions: list[str] | None = None,
    unresponsive_engines: list[list[str]] | None = None,
    meta: dict[str, Any] | None = None,
    engine_count: int | None = None,
    responsive_count: int | None = None,
    portal_state: dict[str, Any] | None = None,
) -> Response:
    """Render one normalized search response in the requested format."""
    if output_format == "html":
        return _portal_security_headers(HTMLResponse(
            content=format_html(
                results,
                query,
                meta=meta,
                unresponsive_engines=unresponsive_engines,
                portal_state=portal_state,
                default_theme=_portal_default_theme(),
            ),
            status_code=status_code,
        ))
    if output_format == "yaml":
        yaml_output = format_yaml_markdown(
            results,
            query,
            meta=meta,
            engine_count=engine_count,
            responsive_count=responsive_count,
            unresponsive_engines=unresponsive_engines,
        )
        return PlainTextResponse(
            content=yaml_output, media_type=_FORMAT_MEDIA_TYPES[output_format], status_code=status_code
        )
    if output_format == "csv":
        return PlainTextResponse(
            content=format_csv(results), media_type=_FORMAT_MEDIA_TYPES[output_format], status_code=status_code
        )
    if output_format == "rss":
        return PlainTextResponse(
            content=format_rss(results, query), media_type=_FORMAT_MEDIA_TYPES[output_format], status_code=status_code
        )
    response_data = format_json(
        results=results,
        query=query,
        answers=answers,
        corrections=corrections,
        infoboxes=infoboxes,
        suggestions=suggestions,
        unresponsive_engines=unresponsive_engines,
        meta=meta,
    )
    return JSONResponse(status_code=status_code, content=response_data)


@app.exception_handler(RequestValidationError)
async def search_validation_error(request: Request, exc: RequestValidationError) -> Response:
    """Use SearXNG-compatible 400s for framework-level search validation."""
    if request.url.path not in {"/", "/search"}:
        return JSONResponse(status_code=422, content={"detail": exc.errors()})
    output_format, format_error = _select_format(request, request.query_params.get("format"))
    if format_error is not None:
        return format_error
    first_error = exc.errors()[0] if exc.errors() else {}
    location = first_error.get("loc", ())
    field = str(location[-1]) if location else None
    return _format_error_response(
        output_format,
        400,
        error="invalid_filter" if field else "invalid_request",
        message="Invalid search request.",
        field=field,
    )


async def _search_endpoint(request: Request) -> Any:
    """Execute a search across all enabled engines.

    Accepts all standard SearXNG query parameters. Returns HTML by default;
    use ``format=json`` for the machine-readable response or ``format=yaml``
    for the agent-native YAML+Markdown extension.

    Graceful degradation: scrape-engine failures never block the
    response. Failing engines are reported in ``unresponsive_engines``
    and their results are omitted.
    """
    params = dict(request.query_params)
    if request.method == "POST":
        content_type = request.headers.get("content-type", "").split(";", 1)[0].strip().lower()
        if content_type == "application/x-www-form-urlencoded":
            body = await request.body()
            form = parse_qs(body.decode("utf-8", errors="replace"), keep_blank_values=True)
            params.update({key: values[-1] for key, values in form.items() if values})

    q = params.get("q", "")
    requested_format = params.get("format")
    categories = params.get("categories", "")
    engines_param = params.get("engines", "")
    language = params.get("language", "en")
    time_range = params.get("time_range", "")
    try:
        pageno = int(params.get("pageno", "1"))
        safesearch = int(params.get("safesearch", "0"))
        timeout_raw = params.get("interactive_timeout_ms")
        interactive_timeout_ms = None if timeout_raw is None or timeout_raw == "" else int(timeout_raw)
    except (TypeError, ValueError):
        output_format, format_error = _select_format(request, requested_format)
        if format_error is not None:
            return format_error
        return _format_error_response(output_format, 400, error="invalid_filter", message="Invalid search parameter.")

    if (
        pageno < 1
        or safesearch not in {0, 1, 2}
        or (interactive_timeout_ms is not None and not 1 <= interactive_timeout_ms <= 30000)
    ):
        output_format, format_error = _select_format(request, requested_format)
        if format_error is not None:
            return format_error
        field = "pageno" if pageno < 1 else "safesearch" if safesearch not in {0, 1, 2} else "interactive_timeout_ms"
        return _format_error_response(
            output_format, 400, error="invalid_filter", field=field, message=f"Invalid value for '{field}'."
        )

    # A human visiting the bare root gets the portal landing page. Keep the
    # SearXNG-compatible missing-query error on /search and for machine
    # formats, so API clients retain their existing contract.
    if request.url.path == "/" and request.method == "GET" and not q.strip():
        output_format, format_error = _select_format(request, requested_format)
        if format_error is not None:
            return format_error
        if output_format == "html":
            return _portal_security_headers(
                HTMLResponse(content=format_landing_page(default_theme=_portal_default_theme()))
            )

    # Increment request counters
    # Count every compatibility request, including malformed and unsupported
    # requests, without creating unbounded labels from user input.
    requested_metric_format = (
        requested_format.strip().lower()
        if requested_format is not None
        else _format_from_accept(request.headers.get("accept", ""))
    )
    m.server_requests.inc({})
    m.server_requests_by_format.inc(
        {"format": requested_metric_format if requested_metric_format in _SUPPORTED_FORMATS else "other"}
    )
    known_categories = {cat for engine in _active_engines.values() for cat in engine.categories}
    requested_categories = {c.strip() for c in categories.split(",") if c.strip()}
    for cat in {c if c in known_categories else "other" for c in requested_categories}:
        m.server_requests_by_category.inc({"category": cat})

    output_format, format_error = _select_format(request, requested_format)
    if format_error is not None:
        return format_error

    restricted_engines = _portal_restricted_engines(engines_param)
    if restricted_engines:
        return _format_error_response(
            output_format,
            403,
            error="engine_restricted",
            field="engines",
            message="Explicit selection of one or more sensitive sources requires operator authorization.",
            extra={"engines": restricted_engines},
        )

    def parse_int(
        value: str | int, field: str, *, minimum: int | None = None, maximum: int | None = None
    ) -> int | Response:
        try:
            parsed = int(value)
        except (TypeError, ValueError):
            return _format_error_response(
                output_format, 400, error="invalid_filter", field=field, message=f"Invalid value for '{field}'."
            )
        if (minimum is not None and parsed < minimum) or (maximum is not None and parsed > maximum):
            return _format_error_response(
                output_format, 400, error="invalid_filter", field=field, message=f"Invalid value for '{field}'."
            )
        return parsed

    parsed_page = parse_int(pageno, "pageno", minimum=1)
    if isinstance(parsed_page, Response):
        return parsed_page
    parsed_safesearch = parse_int(safesearch, "safesearch", minimum=0, maximum=2)
    if isinstance(parsed_safesearch, Response):
        return parsed_safesearch
    parsed_timeout: int | None = None
    if interactive_timeout_ms is not None:
        parsed_timeout_value = parse_int(interactive_timeout_ms, "interactive_timeout_ms", minimum=1, maximum=30_000)
        if isinstance(parsed_timeout_value, Response):
            return parsed_timeout_value
        parsed_timeout = parsed_timeout_value

    service = SearchService(_current_context())
    search_request = SearchRequest(
        query=q,
        interactive_timeout_ms=parsed_timeout,
        categories=[c.strip() for c in categories.split(",") if c.strip()],
        engines=[e.strip() for e in engines_param.split(",") if e.strip()],
        language=language,
        page=parsed_page,
        time_range=time_range if time_range else None,
        safesearch=parsed_safesearch,
        client_identifier=request.client.host if request.client else None,
    )

    try:
        response = await service.search(search_request)
    except QueryValidationError as exc:
        if exc.field != "query":
            return _format_error_response(output_format, 400, error="invalid_filter", field=exc.field, message=str(exc))
        return _format_error_response(
            output_format, 400, error="query_required", message="The 'q' parameter is required."
        )
    except RateLimitExceededError:
        return _format_error_response(
            output_format, 429, error="rate_limited", message="Too many requests. Please slow down."
        )

    if response.cached_error:
        # Negative cache hit — 503 without dispatching.
        return _format_error_response(
            output_format,
            503,
            error="service_unavailable",
            message="Temporarily unavailable (cached error)",
            extra={"meta": {"cached": True, "query_id": response.query_id}},
        )

    if response.all_unresponsive and not response.engine_outcomes:
        # No engines available at all
        return _render_search_response(
            output_format,
            results=[],
            query=q,
            status_code=503,
            unresponsive_engines=[["all", "no engines available"]],
            meta={
                "response_time_ms": response.response_time_ms,
                "cached": False,
                "query_id": response.query_id,
                "engine_status": {},
            },
            portal_state=_portal_state(
                query=q,
                categories=categories,
                engine_selection=engines_param,
                language=language,
                time_range=time_range,
                safesearch=parsed_safesearch,
                page=parsed_page,
                response=response,
            ),
        )

    unresponsive = unresponsive_from_outcomes(response.engine_outcomes)
    meta = build_response_meta(response)

    status_code = 503 if response.all_unresponsive else 200

    return _render_search_response(
        output_format,
        results=response.results,
        query=q,
        status_code=status_code,
        answers=response.answers,
        corrections=response.corrections,
        infoboxes=response.infoboxes,
        suggestions=response.suggestions,
        unresponsive_engines=unresponsive,
        meta=meta,
        engine_count=len(response.scope.selected_engines),
        responsive_count=sum(1 for o in response.engine_outcomes if o.status == "ok"),
        portal_state=_portal_state(
            query=q,
            categories=categories,
            engine_selection=engines_param,
            language=language,
            time_range=time_range,
            safesearch=parsed_safesearch,
            page=parsed_page,
            response=response,
        ),
    )


@app.api_route("/", methods=["GET", "POST"])
async def root_search(request: Request) -> Any:
    """SearXNG-compatible root search endpoint."""

    return await _search_endpoint(request)


@app.api_route("/search", methods=["GET", "POST"])
async def search(request: Request) -> Any:
    """SearXNG-compatible search endpoint."""

    return await _search_endpoint(request)
