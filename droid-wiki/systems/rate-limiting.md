# Rate limiting

Active contributors: Magnus Hedemark

## Purpose

Multi-layer distributed rate limiting with per-engine limits, per-client limits, fail-closed mode, and backpressure propagation. Prevents API quota exhaustion and ensures fair multi-tenancy.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `RateLimitStrategy` | `slopsearx/ratelimit.py` | Abstract base class for pluggable rate-limit strategies |
| `LocalTokenBucket` | `slopsearx/ratelimit.py` | In-memory token bucket. Correct for 1-3 replicas |
| `ValkeySlidingWindow` | `slopsearx/ratelimit.py` | Distributed sliding window via Valkey INCR + EXPIRE. Correct for 50+ replicas |
| `ExternalSidecar` | `slopsearx/ratelimit.py` | Delegates to a dedicated rate-limit service (stub) |
| `RateLimiter` | `slopsearx/ratelimit.py` | Wrapper with backpressure: 30s cooldown, 3-strike deactivation |
| `_EngineState` | `slopsearx/ratelimit.py` | Per-engine backpressure tracking struct |

## How it works

### Strategy layers

1. **Per-engine rate limiting** — each adapter calls `self.rate_limiter.acquire(engine_name)` before sending a request. The `ValkeySlidingWindow` strategy uses `INCR ratelimit:{engine}:{window_start}` with 2× window TTL
2. **Per-client rate limiting** — server calls `_client_rate_window.acquire(client_ip)` before dispatching. Same `ValkeySlidingWindow` strategy
3. **Engine dispatch semaphore** — `MAX_CONCURRENT_ENGINES` (default 10) caps simultaneous outbound HTTP connections per search

### ValkeySlidingWindow details

```python
window_start = floor(current_timestamp / window_seconds)
key = f"ratelimit:{engine}:{window_start}"
count = INCR(key, cost)
if count == cost: EXPIRE(key, window * 2)
return count <= rate_limit
```

Each replica atomically increments the shared counter. Exceeding the per-window limit denies the request.

### Fail-closed mode

When `FAIL_CLOSED=true` and Valkey is unreachable:
1. During **grace period** (`FAIL_CLOSED_GRACE_SECONDS`, default 30s): deny all requests
2. After grace period: fall back to in-process `LocalTokenBucket` with configured rate

When `FAIL_CLOSED=false` (default): allow all requests during Valkey outage (fail-open).

### Backpressure

The `RateLimiter` wrapper adds:
- **30s cooldown** after rate-limit denial
- **3-strike deactivation** — engine disabled until `reactivate()` is called
- **Per-engine state tracking** via `_EngineState` objects

## Integration points

- **Server startup:** `RateLimiter(LocalTokenBucket())` created, injected into each engine via constructor
- **Adapter search:** `await engine.rate_limiter.acquire(engine_name)` via `_check_rate_limit()`
- **Server search handler:** `await _client_rate_window.acquire(client_ip)` before dispatch
- **Server health:** Valkey connectivity check uses `_client_rate_window._connected`
- **Config:** Fail-closed behavior configured via `FAIL_CLOSED`, `FAIL_CLOSED_GRACE_SECONDS` env vars

## Entry points

- Change rate: modify `_default_rate` in `ValkeySlidingWindow`, or per-engine config via `rate_limit` field
- Change window: modify `_window` parameter
- Add strategy: implement new `RateLimitStrategy` subclass

## Key source files

| File | Description |
|---|---|
| `slopsearx/ratelimit.py` | Rate limit strategies, backpressure, fail-closed logic |
