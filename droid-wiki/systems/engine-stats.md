# Engine stats tracker

Active contributors: Magnus Hedemark

## Purpose

Fire-and-forget per-engine quality metrics stored in Valkey for operator dashboards, V2 ranking calibration, and silent-quality-degradation detection. Each engine query produces a set of counters (results returned, errors, rate limits, latency, score) that are atomically accumulated into daily hash keys.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `EngineStatsTracker` | `slopsearx/stats.py` | Per-engine quality telemetry collector. Exposes `record_query()` for recording metrics after each engine dispatch. |
| `record_query()` | `slopsearx/stats.py` | Synchronous fire-and-forget method that increments Valkey counters via `HINCRBY` pipeline for a daily stats key. |
| `_daily_key()` | `slopsearx/stats.py` | Builds the Valkey key for a given engine and current date, e.g. `engine_stats:brave:2026-06-10`. |
| `EngineStatus` | `slopsearx/adapter.py` | Enum classifying engine health: `OK`, `RATE_LIMITED`, `BLOCKED`, `ERROR`, `TIMEOUT`. Used by `record_query` to categorize errors vs rate limits. |

## How it works

### Daily key scheme

Stats are organized by engine name and calendar date:

```
engine_stats:{engine_name}:{YYYY-MM-DD}
```

Examples:
- `engine_stats:brave:2026-06-10`
- `engine_stats:duckduckgo:2026-06-09`

Each key is a Valkey hash with the following fields:

| Field | Type | Description | Incremented by |
|---|---|---|---|
| `queries` | integer | Total queries dispatched | 1 per call |
| `results_returned` | integer | Total results across all queries | `result_count` |
| `errors` | integer | Error/timeout count | 1 if status is `ERROR` or `TIMEOUT` |
| `rate_limited` | integer | Rate-limit hit count | 1 if status is `RATE_LIMITED` |
| `total_latency_ms` | integer | Cumulative latency in milliseconds | `int(latency_ms)` |
| `total_score` | integer | Cumulative result score (scaled) | `int(avg_score * 1000)` |

### Write path

`record_query()` executes the following steps:

1. **Graceful degradation.** If the Valkey cache is uninitialized or disconnected, the call is a no-op. No exception propagates to the caller.
2. **Pipeline batch write.** A Valkey pipeline is used for atomic batch increment: all six hash fields are incremented in a single round-trip via `HINCRBY`.
3. **TTL expiration.** Each daily key gets a 90-day TTL (`7_776_000` seconds) set via `EXPIRE`. Old keys auto-expire, keeping Valkey memory usage bounded.

### Read path (external tools)

Stats are not read by the application itself. Operator dashboards and ranking calibration tools read Valkey directly:

```
HGETALL engine_stats:brave:2026-06-10
```

External tools can compute derived metrics from the raw counters:
- `avg_latency_ms` = `total_latency_ms / queries`
- `avg_score` = `total_score / queries / 1000`
- `error_rate` = `errors / queries`
- `rate_limit_rate` = `rate_limited / queries`

### Error classification

`record_query` maps `EngineStatus` to error counters:

- `EngineStatus.ERROR` or `EngineStatus.TIMEOUT` → increments `errors`
- `EngineStatus.RATE_LIMITED` → increments `rate_limited`
- `EngineStatus.OK` or `EngineStatus.BLOCKED` → increments neither (blocked is tracked separately in server metrics)

## Integration points

- **Server startup:** `startup()` in `server.py` creates `EngineStatsTracker(cache=_cache)` and assigns it to the module-level `_stats_tracker` variable
- **Post-dispatch:** After each engine's `search()` call completes in the `search()` handler, the server calls `_stats_tracker.record_query()` with the engine name, result count, latency, status, and average score
- **Cache dependency:** The tracker requires a connected Valkey `SearchCache` instance. Gracefully degrades when Valkey is unavailable
- **Prometheus complement:** Stats tracker provides long-term per-day aggregates in Valkey. Short-term per-engine metrics are also tracked via Prometheus counters (`m.engine_status`) in the same post-dispatch code path

## Entry points for modification

- Adding new tracked fields: add `HINCRBY` calls in `record_query()`, document the field in `_daily_key()` docstring
- Changing TTL: modify the `expire()` argument in `record_query()`
- Changing key scheme: modify `_daily_key()` method
- Adding read endpoints: add a new endpoint in `server.py` that queries Valkey stats hashes

## Key source files

| File | Description |
|---|---|
| `slopsearx/stats.py` | EngineStatsTracker class, daily key scheme, pipeline-based write logic |
| `slopsearx/adapter.py` | EngineStatus enum used for error classification |
| `slopsearx/server.py` | Stats tracker initialization at startup, invocation after each engine dispatch |
