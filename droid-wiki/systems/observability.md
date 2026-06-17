# Observability

Active contributors: Magnus Hedemark

## Purpose

Three observability subsystems: (1) OpenMetrics instrumentation with per-engine counters, latency histograms, and status gauges exposed via `/metrics` for Prometheus scraping; (2) per-engine quality telemetry stored in Valkey for operator dashboards; (3) query audit trail stored in Valkey streams for operational analysis and debugging.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `Counter` | `slopsearx/metrics.py` | Monotonically increasing counter with labeled dimensions. Incremented via `inc(labels, amount)`. |
| `Gauge` | `slopsearx/metrics.py` | Point-in-time value with labeled dimensions. Set via `set(labels, value)`. |
| `Histogram` | `slopsearx/metrics.py` | Client-side histogram with configurable quantiles. Stores observed values and computes quantiles on render via linear interpolation. |
| `engine_queries` | `slopsearx/metrics.py` | Per-engine query counter. Label: `engine`. Tracks total queries dispatched to each engine. |
| `engine_latency` | `slopsearx/metrics.py` | Per-engine latency histogram. Label: `engine`. Reports quantiles at 0.5, 0.9, and 0.99. |
| `engine_status` | `slopsearx/metrics.py` | Per-engine status gauge. Label: `engine`. Values: 0 = OK, 1 = degraded (timeout or rate limited), 2 = down (error or blocked). |
| `cache_hits` | `slopsearx/metrics.py` | Cache hit and miss counter. Label: `type`. Incremented as `type="hit"` or `type="miss"`. |
| `server_requests` | `slopsearx/metrics.py` | Total request counter. No labels. Tracks every request to the search endpoint. |
| `render_metrics` | `slopsearx/metrics.py` | Renders all metric instances into standard OpenMetrics text format. Called by the `/metrics` endpoint. |
| `EngineStatsTracker` | `slopsearx/stats.py` | Per-engine quality telemetry system. Stores daily aggregated stats in Valkey keyed by `engine_stats:{engine}:{YYYY-MM-DD}`. Not exposed via `/metrics`. |
| `QueryAuditLogger` | `slopsearx/audit.py` | Durable query audit trail. Records every search query in a daily Valkey stream (`query_audit:{YYYY-MM-DD}`) with dispatch statistics, client IP, and latency. |

## How it works

### Metric architecture

Each metric is a module-level global instance in `slopsearx/metrics.py`. There is no registry or discovery mechanism beyond the Python module itself. `render_metrics()` explicitly concatenates the output of each metric's `render()` method.

### Metric types

**Counter:** Accumulates values over time. Used for query counts, cache hits, and server requests. Values never decrease.

**Gauge:** Reflects a point-in-time measurement. Used for engine status codes. The server sets the gauge on every search request based on the engine's response status.

**Histogram:** Stores all observed values in a list keyed by label set. On render, it sorts the values and computes quantiles using linear interpolation. Used for per-engine latency. The histogram also reports `_sum` and `_count` suffixes.

### OpenMetrics output

The `/metrics` endpoint returns plain text in the OpenMetrics format:

```
# HELP slopsearx_engine_queries_total Total queries dispatched per engine
# TYPE slopsearx_engine_queries_total counter
slopsearx_engine_queries_total{engine="brave"} 15230
slopsearx_engine_queries_total{engine="duckduckgo"} 8920

# HELP slopsearx_engine_latency_seconds Query latency per engine in seconds
# TYPE slopsearx_engine_latency_seconds histogram
slopsearx_engine_latency_seconds{engine="brave",quantile="0.5"} 0.34
slopsearx_engine_latency_seconds{engine="brave",quantile="0.99"} 1.2
slopsearx_engine_latency_seconds_sum{engine="brave"} 5184.2
slopsearx_engine_latency_seconds_count{engine="brave"} 15230
```

### Where metrics are recorded

Metrics are recorded in the search request handler in `slopsearx/server.py`:

1. **At the start of every search request:** `m.server_requests.inc({})`
2. **After each engine dispatch:** `m.engine_queries.inc({"engine": name})` and `m.engine_latency.observe({"engine": name}, latency_seconds)`
3. **Engine status on every search:** `m.engine_status.set({"engine": name}, status_code)` where status_code is 0 for OK, 1 for timeout/rate limited, 2 for error/blocked
4. **Cache operations:** `m.cache_hits.inc({"type": "hit"})` or `{"type": "miss"}`

### EngineStatsTracker (quality telemetry)

`EngineStatsTracker` at `slopsearx/stats.py` stores per-engine quality metrics in Valkey for operator dashboards, future ranking calibration, and silent-quality-degradation detection. It is **separate from the OpenMetrics metrics** and is **not exposed via `/metrics`**.

**Schema:**

```
engine_stats:{engine_name}:{YYYY-MM-DD} → Hash
  queries: 15230
  results_returned: 142301
  errors: 234
  rate_limited: 45
  total_latency_ms: 640920
  total_score: 11103
```

Keys auto-expire after 90 days. Each call to `record_query()` uses a Valkey pipeline for atomic batch increment of counters.

**Recording flow:**

After each engine dispatch in the search handler, the server calls:

```python
_stats_tracker.record_query(
    engine=name,
    result_count=len(result.results),
    latency_ms=result.latency_ms,
    status=result.status,
    avg_score=avg_score,
)
```

The stats tracker is initialized at server startup with a reference to the `SearchCache` (which wraps the Valkey connection). Valkey unavailability is non-fatal — the call is a silent no-op.

### Grafana dashboard

The repository includes a Grafana dashboard at `docs/grafana/per-engine-monitoring.json` with these panels:

- Engine query rate per engine (time series, from `slopsearx_engine_queries_total`)
- Engine latency percentiles p50 and p99 (time series, from `slopsearx_engine_latency_seconds`)
- Engine status indicator (state timeline, from `slopsearx_engine_status`)
- Cache hit rate (time series, from `slopsearx_cache_hit_total`)
- Server request rate (time series, from `slopsearx_server_requests_total`)

### Query audit trail

The `QueryAuditLogger` at `slopsearx/audit.py` records every search query in a daily Valkey stream for operational analysis. Each entry captures:

- The raw query string and client IP
- Which engines were dispatched and their outcomes (ok, error, timeout counts)
- Total results returned and overall request latency

Streams are capped at ~10,000 entries and auto-expire after 90 days. This provides durable visibility into query patterns, engine health trends, and capacity planning data without relying on short-lived metrics scrapes.

For full details, see [Audit trail](../systems/audit-trail.md).

## Integration points

- **Server search handler:** Every engine dispatch records query count, latency, and status metrics; also records quality telemetry via `EngineStatsTracker.record_query()`
- **Server cache operations:** Cache hits and misses are recorded alongside the search flow
- **/metrics endpoint:** The FastAPI `metrics()` handler calls `m.render_metrics()` and returns it as `text/plain; version=0.0.4`
- **Valkey-based quality stats:** `EngineStatsTracker` writes daily aggregated stats directly to Valkey for operator dashboards (not exposed via `/metrics`)
- **Prometheus scraping:** The `/metrics` endpoint is designed for standard Prometheus scrape targets

## Entry points for modification

- Adding a new metric: create a new `Counter`, `Gauge`, or `Histogram` instance in `slopsearx/metrics.py` and add it to `render_metrics()`
- Changing quantile boundaries: modify the `quantiles` parameter of an existing `Histogram`
- Adding recording points: record metric values in `server.py` or adapter code as needed
- Adding quality telemetry fields: add new `hincrby` calls to `EngineStatsTracker.record_query()` in `slopsearx/stats.py`

## Key source files

| File | Description |
|---|---|
| `slopsearx/metrics.py` | Metric classes (`Counter`, `Gauge`, `Histogram`), global metric instances, and `render_metrics()` |
| `slopsearx/stats.py` | `EngineStatsTracker` — per-engine quality telemetry stored in Valkey |
| `docs/grafana/per-engine-monitoring.json` | Grafana dashboard definition for per-engine monitoring panels |
