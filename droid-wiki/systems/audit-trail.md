# Audit trail

Active contributors: Magnus Hedemark

## Purpose

Durable query audit trail recording every search query in a Valkey stream. Provides operational visibility into query patterns, engine health trends, debugging data, and capacity planning without relying on short-lived metrics scrapes.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `QueryAuditLogger` | `slopsearx/audit.py` | Fire-and-forget audit stream writer |
| `record_query()` | `slopsearx/audit.py` | Writes query record to daily Valkey stream |

## Schema

```
query_audit:{YYYY-MM-DD} → Stream (XADD)
  query               "search query text"
  client_ip           "10.0.0.1"
  timestamp           "2026-06-14T12:34:56.789Z"
  engines             "brave,wikipedia,duckduckgo"
  engines_ok          2
  engines_error       1
  engines_timeout     0
  total_results       15
  latency_ms          1234.5
```

Each daily stream:
- Capped at ~10,000 entries (`MAXLEN ~10000`)
- Auto-expires after 90 days (`EXPIRE`)

## How it works

After every search request, the server calls:

```python
_audit_logger.record_query(
    query=q,
    client_ip=client_ip,
    engine_results=responses,
    latency_ms=elapsed_ms,
)
```

The call is fire-and-forget — an `asyncio.create_task()` in the server handler. The audit logger uses a Valkey pipeline for the `XADD` and `EXPIRE` commands.

**Engine aggregation:** The logger derives aggregated counts from per-engine `AdapterResponse` objects:
- `engines_ok` — count of engines with `EngineStatus.OK`
- `engines_error` — count with `ERROR`, `BLOCKED`, or `RATE_LIMITED`
- `engines_timeout` — count with `TIMEOUT`
- `total_results` — sum of all engine result counts

**Graceful degradation:** Valkey unavailability is non-fatal — the call is a silent no-op.

## Use cases

- **Debugging:** Replay a query's exact dispatch from the audit stream
- **Traffic analysis:** Query volume and pattern trending per client IP
- **Engine health:** Long-term pass/fail ratios across engines
- **Latency monitoring:** Request latency distribution over time
- **Capacity planning:** Identify peak traffic periods and query volumes

## Integration points

- **Server search handler:** `asyncio.create_task(_audit_logger.record_query(...))` after response building
- **Shared Valkey client:** Uses `cache._client` for stream operations
- **Logger init:** Takes `cache` reference at server startup

## Entry points

- Add a field: add field to `record_query()` fields dict
- Change retention: modify `_AUDIT_TTL` (90 days)
- Change cap: modify `_AUDIT_STREAM_MAXLEN` (10,000)
- Consume stream: use `XREAD` / `XRANGE` on `query_audit:{date}`

## Key source files

| File | Description |
|---|---|
| `slopsearx/audit.py` | QueryAuditLogger class and stream writer |
