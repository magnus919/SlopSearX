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
| `_rate_limiter` | `slopsearx/server.py` | Module-level `RateLimiter` instance. Injected into each engine adapter at startup. |
| `_router` | `slopsearx/server.py` | Module-level `QueryRouter` instance. Initialized from `RoutingConfig` at startup. Determines engine set for queries with no explicit categories or engines params. |
| `_suggestion_service` | `slopsearx/server.py` | Module-level `SuggestionService` instance. Initialized at startup if `enable_suggestions` is True and a Brave API key is available. Fetches search suggestions concurrently with engine dispatch. |
| `_stats_tracker` | `slopsearx/server.py` | Module-level `EngineStatsTracker` instance. Initialized at startup. Records per-engine quality telemetry in Valkey after each engine dispatch. |
| `search()` | `slopsearx/server.py` | `GET /search` handler. Accepts all standard SearXNG query parameters. Returns JSON by default or YAML+Markdown with `format=yaml`. |
| `health()` | `slopsearx/server.py` | `GET /health` handler. Runs per-engine health checks concurrently and returns aggregate status. Returns 200 even if some engines are unhealthy. |
| `metrics()` | `slopsearx/server.py` | `GET /metrics` handler. Returns OpenMetrics text via `render_metrics()`. |
| `config()` | `slopsearx/server.py` | `GET /config` handler. Returns the categories-to-engines mapping for runtime discovery, built from instantiated engines. |
| `_dispatch_engine()` | `slopsearx/server.py` | Dispatches a query to one engine with a 3-second timeout. Returns `AdapterResponse` and never raises. |
| `startup()` | `slopsearx/server.py` | FastAPI lifespan event handler. Discovers engines, initializes cache and rate limiter, injects rate limiter into engines, initializes `QueryRouter`, `SuggestionService`, and `EngineStatsTracker`, and warms up all engines concurrently. |
| `shutdown()` | `slopsearx/server.py` | FastAPI lifespan event handler. Gracefully shuts down all engines, cache, and rate limiter. |

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
     fall back to the curated default engine set (general-purpose engines
     excluding security-only engines)
4. Return 503 if no engines are available
5. Check Valkey cache:
   - HIT: return cached response immediately (~2ms)
   - MISS: continue
6. Fire background suggestion fetch via asyncio.ensure_future(_generate_suggestions(q))
7. Dispatch to all target engines concurrently via asyncio.gather()
8. For each engine, _dispatch_engine() calls engine.search() with 3s timeout
9. Collect AdapterResponse objects
10. Record per-engine metrics (query count, latency, status) via OpenMetrics
11. Record per-engine quality telemetry via EngineStatsTracker.record_query()
    (result count, latency, status, average score — stored in Valkey daily keys)
12. Pass engine results to PresenceRanker.rank() for dedup and ranking
13. Await suggestions from the background task started in step 6
14. Aggregate answers, corrections, and infoboxes from all engine responses
15. Build metadata (response time, engine status, unresponsive list)
16. Format response (JSON or YAML+Markdown)
17. Cache merged result set (skip if all engines unresponsive)
18. Return response with appropriate status code (200 or 503)
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
| 503 | No target engines found or all engines returned non-OK status. Body includes empty results and unresponsive engine list. |
| 429 | Client-side rate limiting (optional, not yet implemented). |
| 200 | All other cases, including partial failures. Failing engines are reported in `unresponsive_engines`. |

The system never returns 500 for a valid request. All engine errors are caught and classified.

### Lifecycle

**Startup:** The `startup()` event handler runs these steps in order:

1. Initialize `SearchCache` (gracefully degrades if Valkey is unavailable)
2. Initialize `RateLimiter` with `LocalTokenBucket` strategy (defaults to dev mode)
3. Warm up the rate limiter
4. Load config and discover engines (if not already populated for test fixtures)
5. Inject the rate limiter into each engine
6. Concurrently warm up all engines
7. Initialize `QueryRouter` from `RoutingConfig` (topic keywords for query-based engine selection)
8. Initialize `SuggestionService` (only if `enable_suggestions` is True and Brave API key is available)
9. Initialize `EngineStatsTracker` (per-engine quality telemetry in Valkey)

**Shutdown:** The `shutdown()` event handler runs:

1. Concurrently shut down all engines
2. Shut down the rate limiter

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
- **Suggestion service:** `_suggestion_service` is initialized if `enable_suggestions` is True, fetching suggestions concurrently with engine dispatch
- **Stats tracker:** `_stats_tracker` records per-engine quality telemetry after each dispatch
- **Cache:** `SearchCache` is checked before dispatch and written after merging
- **Rate limiter:** Injected into every engine adapter instance during startup
- **Metrics:** Recorded on every search request for per-engine observability

## Entry points for modification

- Adding a new endpoint: add a FastAPI route decorator and handler in `server.py`
- Changing the timeout: modify the `timeout_s` parameter in `_dispatch_engine()` signature
- Modifying engine selection: change the filter logic in `search()` handler
- Changing routing behavior: modify `QueryRouter` initialization or the `_router.route()` call in `search()`
- Adding suggestion sources: extend `SuggestionService` in `slopsearx/suggest.py`
- Adding quality telemetry fields: modify `EngineStatsTracker.record_query()` in `slopsearx/stats.py`
- Adjusting startup sequence: modify `startup()` event handler

## Key source files

| File | Description |
|---|---|
| `slopsearx/server.py` | FastAPI application, all endpoints, engine lifecycle management, dispatch logic |
