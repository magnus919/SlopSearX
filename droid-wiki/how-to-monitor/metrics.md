# Metrics

Active contributors: Magnus Hedemark

## Overview

SlopSearX exposes Prometheus text format 0.0.4 metrics at `GET /metrics` for Prometheus scraping. The metrics implementation is stdlib-only — no runtime `prometheus-client` dependency (the development suite uses its parser).

## Endpoint

```
GET /metrics
Content-Type: text/plain; version=0.0.4
```

## Metric inventory

### Engine metrics

| Metric | Type | Labels | Description |
|---|---|---|---|
| `slopsearx_engine_queries_total` | Counter | `engine` | Total queries dispatched per engine |
| `slopsearx_engine_latency_seconds` | Histogram | `engine`; `le` on `_bucket` series | Query latency distribution per engine |
| `slopsearx_engine_status` | Gauge | `engine` | 0=ok, 1=degraded (timeout/rate-limited), 2=down (error/blocked) |

### Cache metrics

| Metric | Type | Labels | Description |
|---|---|---|---|
| `slopsearx_cache_hit_total` | Counter | `type` (hit/miss) | Cache hit and miss counts |

### Server metrics

| Metric | Type | Labels | Description |
|---|---|---|---|
| `slopsearx_server_requests_total` | Counter | (none) | Total search requests |
| `slopsearx_server_requests_by_category_total` | Counter | `category` | Requests by category filter |
| `slopsearx_server_requests_by_format_total` | Counter | `format` | Requests by output format |
| `slopsearx_server_errors_total` | Counter | `type` (timeout, circuit_open, rate_limited, internal) | Server errors by type |

Latency observations update fixed cumulative bucket counts; raw observations are not retained.
Use `histogram_quantile(0.95, rate(slopsearx_engine_latency_seconds_bucket[5m]))`
for an estimated five-minute P95. For replicas, sum rates by `le` and the labels
you want to retain before applying `histogram_quantile`. Metrics are process-local.

`slopsearx_engine_errors_total` counts non-OK outcomes; successful empty results
are not failures. Compare its rate with `slopsearx_engine_queries_total` for the
engine failure ratio. See `docs/alerting/README.md` for the migration details.

## Sample output (selected histogram buckets)

```
# HELP slopsearx_engine_queries_total Total queries dispatched per engine
# TYPE slopsearx_engine_queries_total counter
slopsearx_engine_queries_total{engine="brave"} 15230
slopsearx_engine_queries_total{engine="duckduckgo"} 8920

# HELP slopsearx_engine_latency_seconds Query latency per engine in seconds
# TYPE slopsearx_engine_latency_seconds histogram
slopsearx_engine_latency_seconds_bucket{engine="brave",le="0.5"} 12000
slopsearx_engine_latency_seconds_bucket{engine="brave",le="5"} 15200
slopsearx_engine_latency_seconds_bucket{engine="brave",le="+Inf"} 15230
slopsearx_engine_latency_seconds_sum{engine="brave"} 5184.2
slopsearx_engine_latency_seconds_count{engine="brave"} 15230

# HELP slopsearx_engine_status Engine status (0=ok, 1=degraded, 2=down)
# TYPE slopsearx_engine_status gauge
slopsearx_engine_status{engine="google"} 1
```

## Prometheus configuration

```yaml
scrape_configs:
  - job_name: slopsearx
    scrape_interval: 15s
    static_configs:
      - targets: ['slopsearx:8080']
```

## Grafana dashboard

The repository includes a Grafana dashboard at `docs/grafana/per-engine-monitoring.json` with panels for:

- **Engine query rate** — time series of `rate(slopsearx_engine_queries_total[5m])` per engine
- **Engine latency (p50/p99)** — time series from latency histogram quantiles
- **Engine status** — state timeline from `slopsearx_engine_status`
- **Cache hit rate** — time series of `rate(slopsearx_cache_hit_total{type="hit"}[5m]) / rate(slopsearx_cache_hit_total[5m])`
- **Server request rate** — time series of `rate(slopsearx_server_requests_total[5m])`

## Key source files

| File | Description |
|---|---|
| `slopsearx/metrics.py` | Metric classes and global instances |
| `docs/grafana/per-engine-monitoring.json` | Grafana dashboard definition |
