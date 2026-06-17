# Observability

Active contributors: Magnus Hedemark

## Purpose

Three-layered observability stack: (1) OpenMetrics instrumentation for Prometheus scraping, (2) structlog-based structured JSON logging with optional Sentry error tracking, and (3) X-Request-ID middleware for distributed tracing. Plus Valkey-stored quality telemetry and audit trails.

## Key abstractions

### Metrics (`slopsearx/metrics.py`)

| Type | Description |
|---|---|
| `Counter` | Monotonically increasing counter with labeled dimensions |
| `Gauge` | Point-in-time value with labeled dimensions |
| `Histogram` | Client-side histogram with configurable quantiles, sum, and count |
| `render_metrics()` | Concatenates all metric renders into OpenMetrics text format |

All metrics are stdlib-only — no prometheus-client dependency.

**Global metrics:**

| Metric | Type | Labels |
|---|---|---|
| `slopsearx_engine_queries_total` | Counter | `engine` |
| `slopsearx_engine_latency_seconds` | Histogram | `engine`, `quantile` (0.5, 0.9, 0.99) |
| `slopsearx_engine_status` | Gauge | `engine` (0=ok, 1=degraded, 2=down) |
| `slopsearx_cache_hit_total` | Counter | `type` (hit/miss) |
| `slopsearx_server_requests_total` | Counter | (no labels) |
| `slopsearx_server_requests_by_category_total` | Counter | `category` |
| `slopsearx_server_requests_by_format_total` | Counter | `format` |
| `slopsearx_server_errors_total` | Counter | `type` (timeout, circuit_open, rate_limited, internal) |

### Logging (`slopsearx/logging.py`)

| Function | Description |
|---|---|
| `setup_logging()` | Configures structlog for the entire application |
| `get_logger()` | Returns a structured logger bound to a name |
| `capture_exception()` | Reports an error to Sentry if configured, no-op otherwise |
| `_init_sentry()` | Initializes Sentry SDK with DSN and sample rate |

Log output is JSON-formatted (production) or human-readable console (dev). All log events carry timestamp, logger name, log level, and structured keyword arguments.

**Sentry integration:**
- Activated by `SENTRY_DSN` env var
- Auto-captures unhandled exceptions in engine dispatch
- Breadcrumbs, request-id tagging, full stack traces
- Zero overhead when not configured
- Supports GitHub integration for auto-creating issues from errors

### Tracing (`slopsearx/middleware.py`)

| Class | Description |
|---|---|
| `RequestIDMiddleware` | FastAPI/Starlette middleware injecting X-Request-ID header |

- If incoming request has `X-Request-ID`, it is preserved (distributed tracing)
- Otherwise generates UUIDv4
- Stored in `request.state.request_id` for downstream consumers
- Echoed in response headers

## How it works

### Metrics recording flow

In the server search handler (`slopsearx/server.py`):

```python
# At request start
m.server_requests.inc({})
m.server_requests_by_format.inc({"format": format})
m.server_requests_by_category.inc({"category": cat})

# After each engine dispatch
m.engine_queries.inc({"engine": name})
m.engine_latency.observe({"engine": name}, latency_s)
m.engine_status.set({"engine": name}, status_code)

# On exception
m.server_errors_total.inc({"type": "internal"})
```

### Logging setup

```python
setup_logging()  # called at server startup
log = get_logger(__name__)
log.info("request_processed", query="test", engine_count=5)
```

### Request tracing

```python
app.add_middleware(RequestIDMiddleware)  # applied to FastAPI app
# request.state.request_id now available in all handlers
```

## Grafana dashboard

Repository includes `docs/grafana/per-engine-monitoring.json` with panels for:
- Engine query rate (time series)
- Engine latency p50/p99 (time series)
- Engine status indicator (state timeline)
- Cache hit rate (time series)
- Server request rate (time series)

## Integration points

- **Server startup:** `setup_logging()` configures structlog/Sentry
- **Server middleware:** `RequestIDMiddleware` applied to FastAPI app
- **Server search handler:** Metrics recorded at dispatch, error, and response points
- **Engine dispatch:** `capture_exception()` for unhandled exceptions
- **`/metrics` endpoint:** `m.render_metrics()` returns OpenMetrics text
- **`/health` endpoint:** Valkey connectivity check via rate limiter

## Entry points

- Add a metric: create `Counter`/`Gauge`/`Histogram` in `metrics.py`, record in `server.py`, add to `render_metrics()`
- Change logging format: modify `setup_logging()` processors or `json_output` flag
- Add Sentry context: set tags/breadcrumbs before `capture_exception()`
- Add trace context: access `request.state.request_id` in handlers

## Key source files

| File | Description |
|---|---|
| `slopsearx/metrics.py` | Metric classes and global instances |
| `slopsearx/logging.py` | structlog setup, Sentry integration |
| `slopsearx/middleware.py` | X-Request-ID middleware |
| `slopsearx/stats.py` | EngineStatsTracker (Valkey-stored quality telemetry) |
| `slopsearx/audit.py` | QueryAuditLogger (Valkey stream audit trail) |
| `docs/grafana/per-engine-monitoring.json` | Grafana dashboard |
| `docs/alerting/rules.yml` | Prometheus Alertmanager rules |
