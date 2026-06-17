# Engine stats

Active contributors: Magnus Hedemark

## Purpose

Fire-and-forget per-engine quality telemetry stored in Valkey. Used for operator dashboards, V2 ranking calibration, and silent-quality-degradation detection. Separate from OpenMetrics (`/metrics`) — not exposed via HTTP.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `EngineStatsTracker` | `slopsearx/stats.py` | Records per-engine query stats in Valkey via HINCRBY pipeline |
| `record_query()` | `slopsearx/stats.py` | Async fire-and-forget call after each engine dispatch |

## Schema

```
engine_stats:{engine}:{YYYY-MM-DD} → Hash
  queries: 15230
  results_returned: 142301
  errors: 234
  rate_limited: 45
  total_latency_ms: 640920
  total_score: 11103
```

- Keys auto-expire after 90 days
- Daily key rotation via date component
- Atomic batch increment via Valkey pipeline

## How it works

After each engine dispatch, the server calls:

```python
_stats_tracker.record_query(
    engine=name,
    result_count=len(result.results),
    latency_ms=result.latency_ms,
    status=result.status,
    avg_score=avg_score,
)
```

The tracker uses a Valkey pipeline for atomic batch increment of all counters. TTL set to 90 days on each write so old keys auto-expire. Valkey unavailability is non-fatal — the call is a silent no-op.

## Counters

| Field | Increment pattern | Purpose |
|---|---|---|
| `queries` | +1 per query | Total queries dispatched |
| `results_returned` | +N per query | Total results returned by this engine |
| `errors` | +1 if status is ERROR or TIMEOUT | Failure counter |
| `rate_limited` | +1 if status is RATE_LIMITED | Rate-limit hit counter |
| `total_latency_ms` | +latency per query | Cumulative latency for average calculation |
| `total_score` | +score×1000 per query | Cumulative score for quality analysis |

## Use cases

- **Operator dashboards:** Graph query volume, error rate, average latency per engine over time
- **V2 ranking calibration:** Historical quality metrics feed per-engine trust scores
- **Silent degradation detection:** Sudden changes in result counts or scores flag broken engines
- **Capacity planning:** Traffic patterns per engine inform scaling decisions

## Integration points

- **Server search handler:** Called after each `asyncio.gather()` dispatch result
- **Shared Valkey client:** Uses `cache._client` for pipeline operations
- **Stats tracker init:** Takes `cache` reference at server startup

## Entry points

- Add a counter: add `hincrby` call in `record_query()`
- Change retention: modify `_AUDIT_TTL` (actually `7_776_000` = 90 days) in `record_query()`
- Add aggregation: read stats from Valkey and compute derived metrics

## Key source files

| File | Description |
|---|---|
| `slopsearx/stats.py` | EngineStatsTracker class |
