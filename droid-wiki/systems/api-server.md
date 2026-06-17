# API server

Active contributors: Magnus Hedemark

## Purpose

FastAPI HTTP server that implements the SearXNG-compatible search API with graceful degradation. Manages engine lifecycle (startup, warmup, shutdown), concurrent dispatch, and response formatting.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `app` | `slopsearx/server.py` | FastAPI application instance. Title: "SlopSearX", version "0.1.0". Hosts all endpoints and lifecycle handlers. |
| `_active_engines` | `slopsearx/server.py` | Module-level dict of enabled engine instances. Populated at startup via `discover_engines()`. |
| `_ranker` | `slopsearx/server.py` | Module-level `PresenceRanker` instance. Used by every search request to merge and rank results. |
| `_cache` | `slopsearx/server.py` | Module-level `SearchCache` instance. Initialized at startup. Gracefully degrades if Valkey is unavailable. |
| `_rate_limiter` | `slopsearx/server.py` | Module-level `RateLimiter` instance for per-engine backpressure. Injected into each engine adapter at startup. |
| `_client_rate_window` | `slopsearx/server.py` | Module-level `ValkeySlidingWindow` instance for per-client rate limiting. Checked before any engine dispatch. |
| `_engine_semaphore` | `slopsearx/server.py` | Module-level `asyncio.Semaphore` that caps concurrent outbound HTTP connections per search request (default 10, configurable via `MAX_CONCURRENT_ENGINES`). |
| `_router` | `slopsearx/server.py` | Module-level `QueryRouter` instance. Initialized from `RoutingConfig` at startup. Determines engine set for queries with no explicit categories or engines params. |
| `_suggestion_service` | `slopsearx/server.py` | Module-level `SuggestionService` instance. Initialized at startup if `enable_suggestions` is True and a Brave API key is available. |
| `_stats_tracker` | `slopsearx/server.py` | Module-level `EngineStatsTracker` instance. Records per-engine quality telemetry in Valkey after each dispatch. |
| `_audit_logger` | `slopsearx/server.py` | Module-level `QueryAuditLogger` instance. Records every search query in a Valkey stream for operational analysis. |
| `search()` | `slopsearx/server.py` | `GET /search` handler. Accepts all standard SearXNG query parameters. Returns JSON by default or YAML+Markdown with `format=yaml`. |
| `health()` | `slopsearx/server.py` | `GET /health` handler. Runs per-engine health checks concurrently and returns aggregate status plus Valkey connectivity. |
| `metrics()` | `slopsearx/server.py` | `GET /metrics` handler. Returns OpenMetrics text via `render_metrics()`. |
| `config()` | `slopsearx/server.py` | `GET /config` handler. Returns the categories-to-engines mapping for runtime discovery. |
| `_dispatch_engine()` | `slopsearx/server.py` | Dispatches a query to one engine with a 3-second timeout. Returns `AdapterResponse` and never raises. |
| `_dispatch_with_semaphore()` | `slopsearx/server.py` | Acquires the engine dispatch semaphore before calling `_dispatch_engine()`. Ensures bounded concurrency. |
| `startup()` | `slopsearx/server.py` | FastAPI lifespan event handler. Discovers engines, initializes cache, both rate limiters, semaphore, router, suggestions, stats tracker, and audit logger. Warms up all engines concurrently. |
| `shutdown()` | `slopsearx/server.py` | FastAPI lifespan event handler. Gracefully shuts down all engines, both rate limiters, and cache. |

## How it works

### Full search request flow

```
1. GET /search?q=python&format=json&categories=general
2. Validate query (return 400 if empty)
3. Determine target engines:
   - If `engines` param is set, filter to those engines
   - If `categories` param is set, filter engines by category membership
   - Otherwise, use query-based routing: _router.route(q) matches query
     against topic keywords (first-match wins). If no topic matches,
     restrict to Tier 1 (broad, general-purpose) engines
4. Return 503 if no engines are available
5. Per-client rate limit check: acquire from _client_rate_window by client IP.
   Return 429 if the client has exceeded its budget
6. Check Valkey search cache (precise key):
   - HIT: return cached response immediately
   - NEGATIVE HIT (_error sentinel): return 503 without dispatching
7. Check Valkey answer cache (broad key):
   - HIT: return cached response immediately
   - NEGATIVE HIT: return 503 without dispatching
8. Fire background suggestion fetch via asyncio.ensure_future()
9. For each target engine, check circuit breaker:
   - circuit_allowed() == False: add to unresponsive list, skip dispatch
10. Dispatch to allowed engines concurrently via asyncio.gather(),
    bounded by _engine_semaphore (MAX_CONCURRENT_ENGINES)
11. For each engine, _dispatch_engine() calls engine.search() with 3s timeout
12. Collect AdapterResponse objects; call engine.record_failure() or
    engine.record_success() to update circuit breaker state
13. Record per-engine metrics (query count, latency, status) via OpenMetrics
14. Record per-engine quality telemetry via EngineStatsTracker.record_query()
15. Pass engine results to PresenceRanker.rank() for dedup and ranking
16. Await suggestions from the background task
17. Aggregate answers, corrections, and infoboxes
18. Build metadata (response time, engine status, unresponsive list)
19. Format response (JSON or YAML+Markdown)
20. Cache merged result set in both search and answer caches
21. Record query audit trail via QueryAuditLogger (fire-and-forget)
22. Return response (200 or 503 if all engines unresponsive)
```

### Engine selection logic

The server supports two modes of engine selection:

- **Explicit engine list:** When the `engines` query parameter is provided, only those engines are queried. Category filters are ignored.
- **Category-based selection:** When no explicit engine list is provided, engines are filtered by requested categories. An engine is included if it declares any of the requested categories.

### Timeout mechanism

Each engine dispatch has a 3-second timeout enforced by `asyncio.wait_for()`:

```python
result = await asyncio.wait_for(engine.search(query, params), timeout=timeout_s)
```

If an engine exceeds the timeout, `_dispatch_engine()` returns an `AdapterResponse` with `EngineStatus.TIMEOUT` and a latency of 3000ms. This prevents a single slow engine from delaying the entire response.

### Error responses

| Status | Condition |
|---|---|
| 400 | Missing or empty `q` parameter. Body: `{"error": "query_required", "message": "..."}` |
| 503 | No target engines found, all engines returned non-OK status, or negative cache hit. Body includes empty results and unresponsive engine list. |
| 429 | Per-client rate limit exceeded. Body: `{"error": "rate_limited", "message": "Too many requests. Please slow down."}` |
| 200 | All other cases, including partial failures. Failing engines are reported in `unresponsive_engines`. |

The system never returns 500 for a valid request. All engine errors are caught and classified.

### Lifecycle

**Startup:** The `startup()` event handler runs these steps in order:

1. Initialize `SearchCache` (gracefully degrades if Valkey is unavailable)
2. Initialize `RateLimiter` with `LocalTokenBucket` strategy for per-engine backpressure
3. Warm up the per-engine rate limiter
4. Create `asyncio.Semaphore` bounded by `MAX_CONCURRENT_ENGINES` (default 10)
5. Initialize `ValkeySlidingWindow` for per-client rate limiting with fail-closed support
6. Warm up the per-client rate limiter
7. Load config and discover engines (if not already populated for test fixtures)
8. Inject the per-engine rate limiter into each engine
9. Concurrently warm up all engines
10. Initialize `QueryRouter` from `RoutingConfig`
11. Initialize `SuggestionService` (only if `enable_suggestions` is True and Brave API key is available)
12. Initialize `EngineStatsTracker` for per-engine quality telemetry
13. Initialize `QueryAuditLogger` for query audit trail

**Shutdown:** The `shutdown()` event handler runs:

1. Concurrently shut down all engines
2. Shut down the per-engine rate limiter
3. Shut down the per-client rate limiter
4. Close the cache connection

### Endpoints

| Endpoint | Method | Description |
|---|---|---|
| `GET /search` | Search | Main search endpoint. Parameters: `q`, `format`, `categories`, `engines`, `language`, `pageno`, `time_range`, `safesearch`. |
| `GET /health` | Health check | Per-engine health status. Returns `{"status": "ok"}` or `{"status": "degraded"}`. |
| `GET /metrics` | Metrics | OpenMetrics text format for Prometheus scraping. |
| `GET /config` | Config discovery | Returns `{"categories": {"general": ["brave", ...], ...}}`. |

## Integration points

- **Engine adapters:** The server imports `engines` at module level to trigger `@register_engine` decoration before startup
- **Config system:** `load_config()` is called at startup to resolve the layered configuration
- **Query router:** `_router` is initialized from `RoutingConfig` and used in `search()` for query-based engine selection
- **Suggestion service:** `_suggestion_service` fetches suggestions concurrently with engine dispatch
- **Stats tracker:** `_stats_tracker` records per-engine quality telemetry after each dispatch
- **Audit logger:** `_audit_logger` records every query in a Valkey stream (fire-and-forget)
- **Cache:** `SearchCache` is checked at two levels (search + answer cache) before dispatch and written after merging; negative cache entries short-circuit failing queries
- **Rate limiter (per-engine):** Injected into every engine adapter instance during startup
- **Rate limiter (per-client):** Checked before any engine dispatch; returns 429 on violation
- **Engine semaphore:** Caps concurrent outbound HTTP connections per search request
- **Circuit breaker:** Checked before dispatching to each engine; open circuits skip dispatch
- **Metrics:** Recorded on every search request for per-engine observability

## Entry points for modification

- Adding a new endpoint: add a FastAPI route decorator and handler in `server.py`
- Changing the timeout: modify the `timeout_s` parameter in `_dispatch_engine()` signature
- Modifying engine selection: change the filter logic in `search()` handler
- Changing routing behavior: modify `QueryRouter` initialization or the `_router.route()` call
- Changing concurrency cap: set `MAX_CONCURRENT_ENGINES` env var
- Changing per-client rate limits: set `PER_CLIENT_REQUESTS` and `PER_CLIENT_WINDOW_SECONDS` env vars
- Adding suggestion sources: extend `SuggestionService` in `slopsearx/suggest.py`
- Adding quality telemetry fields: modify `EngineStatsTracker.record_query()` in `slopsearx/stats.py`
- Adding audit fields: modify `QueryAuditLogger.record_query()` in `slopsearx/audit.py`
- Adjusting startup sequence: modify `startup()` event handler

## Key source files

| File | Description |
|---|---|
| `slopsearx/server.py` | FastAPI application, all endpoints, engine lifecycle management, dispatch logic |
