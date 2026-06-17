# Rate limiting

Active contributors: Magnus Hedemark

## Purpose

Multi-layer rate limiting with backpressure propagation. Three strategies (`LocalTokenBucket`, `ValkeySlidingWindow`, `ExternalSidecar`) serve per-engine rate limiting, while a separate per-client rate limiter protects the server from noisy tenants. Fail-closed mode with graceful fallback prevents unbounded traffic during Valkey outages.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `RateLimitStrategy` | `slopsearx/ratelimit.py` | Abstract strategy interface. Subclasses implement `acquire(engine, cost)` which returns `True` if the request is allowed. Also provides optional `warmup()` and `shutdown()` lifecycle hooks. |
| `LocalTokenBucket` | `slopsearx/ratelimit.py` | In-memory token bucket using `time.monotonic()` for refill timing. Suitable for development with 1-3 replicas. Not suitable for 50+ replicas because each replica maintains independent state. |
| `ValkeySlidingWindow` | `slopsearx/ratelimit.py` | Distributed sliding window using Valkey `INCR` and `EXPIRE`. Correct for all replica counts. Supports fail-closed mode with local fallback after a grace period. |
| `ExternalSidecar` | `slopsearx/ratelimit.py` | Delegates rate limiting to a dedicated external service. Currently a stub that always allows. Intended for advanced deployments where rate limiting is managed by a separate infrastructure component. |
| `RateLimiter` | `slopsearx/ratelimit.py` | Backpressure wrapper around a strategy. Adds 30-second cooldown after rate-limit denial and 3-strike deactivation of engines. |
| `_EngineState` | `slopsearx/ratelimit.py` | Per-engine backpressure tracking dataclass. Fields: `consecutive_failures`, `cooldown_until`, `deactivated`. |

## How it works

### Strategy interface

Every adapter calls `self.rate_limiter.acquire(engine_name)` before sending a request. The `RateLimiter` delegates to the configured strategy, which decides allow or deny.

### LocalTokenBucket

For development, the local token bucket maintains per-engine token counts in memory. Each replica maintains its own token count. At 50+ replicas, the effective rate would be 50x the configured rate.

### ValkeySlidingWindow

For production, the Valkey sliding window uses a shared counter:

```
ratelimit:{engine}:{window_start}  →  INCR + EXPIRE
```

All replicas atomically increment the same counter. If the result exceeds the configured per-window rate, the request is denied. Keys expire after two window durations.

### Per-client rate limiting

The server also enforces a per-client request budget using a separate `ValkeySlidingWindow` instance keyed on `request.client.host`:

```
ratelimit:client:{client_ip}:{window_start}  →  INCR + EXPIRE
```

Configurable via `PER_CLIENT_REQUESTS` (default 30 requests) and `PER_CLIENT_WINDOW_SECONDS` (default 60 seconds). When a client exceeds its budget, the server returns HTTP 429 before any engine dispatch occurs.

### Fail-closed behavior

The `ValkeySlidingWindow` supports a fail-closed mode (`FAIL_CLOSED=true`) for security-sensitive deployments:

- **Default (fail-open):** When Valkey is unreachable, all requests are allowed through. This prioritizes availability over strict enforcement.
- **Fail-closed:** When Valkey is unreachable, all requests are denied during a configurable grace period (`FAIL_CLOSED_GRACE_SECONDS`, default 30s). After the grace period expires, the system falls back to an in-process `LocalTokenBucket` with the same configured rate so service can continue with approximate enforcement.

This progression (deny → local fallback) prevents unbounded traffic during transient Valkey outages while avoiding permanent denial of service during extended outages.

### Backpressure

The `RateLimiter` wraps a strategy and adds backpressure behavior:

1. **30-second cooldown:** When a strategy denies a request, the engine enters a 30-second cooldown. During cooldown, all requests to that engine are denied without consulting the strategy.
2. **3-strike deactivation:** If an engine receives three consecutive rate-limit denials, it is deactivated. Deactivated engines are skipped entirely until a health check passes and `reactivate()` is called.
3. **Automatic recovery:** On a successful `acquire()` call, the consecutive failure counter resets to zero, and the cooldown is lifted.

## Integration points

- **Server startup:** `startup()` in `server.py` creates two rate limiters: a `RateLimiter` with `LocalTokenBucket` for per-engine backpressure (injected into each adapter), and a `ValkeySlidingWindow` for per-client rate limiting (checked in the search handler before dispatch)
- **Adapter calls:** Each adapter calls `self.rate_limiter.acquire(self.name)` before sending HTTP requests
- **Health checks:** After a deactivated engine passes a health check, the server calls `_rate_limiter.reactivate(engine_name)`
- **Fail-closed recovery:** `ValkeySlidingWindow` attempts reconnection on each `acquire()` call and clears the disconnected state on success
- **Shutdown:** The server calls `_rate_limiter.shutdown()` and `_client_rate_window.shutdown()` during graceful shutdown

## Entry points for modification

- Changing the production strategy: modify server startup to use `ValkeySlidingWindow` instead of `LocalTokenBucket` for per-engine limiting
- Adjusting backpressure parameters: modify cooldown duration or strike count in `RateLimiter.acquire()`
- Changing per-client limits: set `PER_CLIENT_REQUESTS` and `PER_CLIENT_WINDOW_SECONDS` env vars
- Enabling fail-closed: set `FAIL_CLOSED=true` and optionally `FAIL_CLOSED_GRACE_SECONDS`
- Adding a new strategy: subclass `RateLimitStrategy` and implement `acquire()`

## Key source files

| File | Description |
|---|---|
| `slopsearx/ratelimit.py` | All rate-limiting strategies (`LocalTokenBucket`, `ValkeySlidingWindow`, `ExternalSidecar`), `RateLimiter` backpressure wrapper, and `_EngineState` tracking |
