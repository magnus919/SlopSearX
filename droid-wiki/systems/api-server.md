# API server

Active contributors: Magnus Hedemark

## Purpose

The FastAPI HTTP server is the central orchestrator. It handles the full request lifecycle: rate limiting, cache lookup, query routing, engine dispatch, result merging, formatting, caching, audit logging, and observability.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `app` | `slopsearx/server.py` | FastAPI application with lifespan-managed startup/shutdown |
| `search()` | `slopsearx/server.py` | `GET /search` handler implementing the full request flow |
| `health()` | `slopsearx/server.py` | `GET /health` — server liveness + Valkey connectivity |
| `metrics()` | `slopsearx/server.py` | `GET /metrics` — OpenMetrics text output |
| `config()` | `slopsearx/server.py` | `GET /config` — categories → engines mapping |
| `_dispatch_engine()` | `slopsearx/server.py` | Single-engine dispatch with timeout and error classification |
| `_dispatch_with_semaphore()` | `slopsearx/server.py` | Semaphore-bounded dispatch |
| `_safe_metric_label()` | `slopsearx/server.py` | Sanitizes user-supplied strings for Prometheus labels |

## Request flow

1. **X-Request-ID middleware** applies `RequestIDMiddleware` — injects/propagates UUIDv4 trace ID
2. **Validate query** — empty `q` returns 400
3. **Engine selection** — explicit `engines` param wins over `categories` filter, which wins over `QueryRouter` topic matching. Unscoped queries without topic matches use Tier 1 engines only
4. **Per-client rate limit** — `ValkeySlidingWindow.acquire(client_ip)` with fail-closed logic
5. **Cache check** — search cache → answer cache → negative cache
6. **Suggestion fetch** — background task fires Brave Suggest + DDG fallback concurrently with engine dispatch
7. **Dispatch** — `asyncio.gather()` through semaphore-bounded `_dispatch_with_semaphore()`
8. **Circuit breaker update** — `record_success()` / `record_failure()` after each dispatch
9. **Metrcis recording** — per-engine query count, latency, status gauge; product analytics by category and format
10. **Stats tracker** — fire-and-forget `EngineStatsTracker.record_query()`
11. **Ranking** — `PresenceRanker.rank()` with sorting by `(tier, -score)`
12. **Formatting** — JSON or YAML+Markdown
13. **Caching** — store in both search and answer cache
14. **Audit trail** — fire-and-forget `QueryAuditLogger.record_query()`
15. **Response** — 200 with results, or 503 if all engines unresponsive

## Two-tier engine selection

```python
_TIER1_ENGINES = {"brave", "duckduckgo", "google", "wikipedia", "stackexchange", "reddit"}
```

Unscoped queries without topic matches use Tier 1 only. When `categories` or `engines` params are specified, they bypass tier restrictions.

## Concurrency control

The `_engine_semaphore` (`asyncio.Semaphore`) caps concurrent outbound HTTP connections per search. Default 10, configurable via `MAX_CONCURRENT_ENGINES`. Engines beyond the cap are queued.

## Error handling

Unhandled exceptions from engine dispatch are captured by Sentry (if configured) and classified as `EngineStatus.ERROR`. Timeouts produce `EngineStatus.TIMEOUT`. Neither blocks other engines.

## Entry points

- Modify request flow: change `search()` handler
- Add endpoint: use FastAPI decorator pattern
- Change engine selection: modify `_TIER1_ENGINES` or selection logic
- Adjust concurrency: set `MAX_CONCURRENT_ENGINES` env var

## Key source files

| File | Description |
|---|---|
| `slopsearx/server.py` | Main application, all endpoint handlers, dispatch logic |
| `slopsearx/middleware.py` | X-Request-ID middleware |
