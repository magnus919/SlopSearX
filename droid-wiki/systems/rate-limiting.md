# Rate limiting

Active contributors: Magnus Hedemark

## Purpose

Distributed rate limiting with backpressure propagation. Three strategies: `LocalTokenBucket` for development, `ValkeySlidingWindow` for production at 50+ replicas, and `ExternalSidecar` for advanced deployments.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `RateLimitStrategy` | `slopsearx/ratelimit.py` | Abstract strategy interface. Subclasses implement `acquire(engine, cost)` which returns `True` if the request is allowed. Also provides optional `warmup()` and `shutdown()` lifecycle hooks. |
| `LocalTokenBucket` | `slopsearx/ratelimit.py` | In-memory token bucket using `time.monotonic()` for refill timing. Suitable for development with 1-3 replicas. Not suitable for 50+ replicas because each replica maintains independent state. |
| `ValkeySlidingWindow` | `slopsearx/ratelimit.py` | Distributed sliding window using Valkey `INCR` and `EXPIRE`. Correct for all replica counts. Centralized rate-limit state in Valkey means all replicas share the same counter. |
| `ExternalSidecar` | `slopsearx/ratelimit.py` | Delegates rate limiting to a dedicated external service. Currently a stub that always allows. Intended for advanced deployments where rate limiting is managed by a separate infrastructure component. |
| `RateLimiter` | `slopsearx/ratelimit.py` | Backpressure wrapper around a strategy. Adds 30-second cooldown after rate-limit denial and 3-strike deactivation of engines. |
| `_EngineState` | `slopsearx/ratelimit.py` | Per-engine backpressure tracking dataclass. Fields: `consecutive_failures`, `cooldown_until`, `deactivated`. |

## How it works

### Strategy interface

Every adapter calls `self.rate_limiter.acquire(engine_name)` before sending a request. The `RateLimiter` delegates to the configured strategy, which decides allow or deny.

### LocalTokenBucket

For development, the local token bucket maintains per-engine token counts in memory:

```python
async def acquire(self, engine: str, cost: int = 1) -> bool:
    now = time.monotonic()
    tokens = self._tokens.get(engine, self.burst)
    last = self._last_refill.get(engine, now)

    elapsed = now - last
    tokens = min(self.burst, tokens + elapsed * self.max_rate)
    self._last_refill[engine] = now

    if tokens >= cost:
        self._tokens[engine] = tokens - cost
        return True
    self._tokens[engine] = tokens
    return False
```

Each replica maintains its own token count. At 50+ replicas, the effective rate would be 50x the configured rate, which could trigger upstream rate limits.

### ValkeySlidingWindow

For production, the Valkey sliding window uses a shared counter:

```python
window_start = int(time.monotonic() / self._window)
key = f"ratelimit:{engine}:{window_start}"
count = self._client.incrby(key, cost)
if count == cost:
    self._client.expire(key, int(self._window * 2))
return count <= self._default_rate
```

The rate limit key format is `ratelimit:{engine}:{window_start}` where `window_start` is the current time divided by the window duration. Each replica atomically increments the counter. If the result exceeds the per-window limit, the request is denied. The `EXPIRE` ensures keys are cleaned up after two window durations.

### Backpressure

The `RateLimiter` wraps a strategy and adds backpressure behavior:

1. **30-second cooldown:** When a strategy denies a request, the engine enters a 30-second cooldown. During cooldown, all requests to that engine are denied without consulting the strategy.
2. **3-strike deactivation:** If an engine receives three consecutive rate-limit denials, it is deactivated. Deactivated engines are skipped entirely until a health check passes and `reactivate()` is called.
3. **Automatic recovery:** On a successful `acquire()` call, the consecutive failure counter resets to zero, and the cooldown is lifted.

Deactivated engines can be monitored via the `deactivated_engines` property.

### Fail-open behavior

If Valkey is unavailable, `ValkeySlidingWindow` returns `True` (allow) for all requests. This prioritizes search availability over strict rate-limit enforcement. A warning is logged when the Valkey connection fails.

## Integration points

- **Server startup:** `startup()` in `server.py` creates a `RateLimiter` with `LocalTokenBucket` strategy and calls `warmup()`. The rate limiter is injected into each engine adapter instance via `engine.rate_limiter = _rate_limiter`.
- **Adapter calls:** Each adapter calls `self.rate_limiter.acquire(self.name)` before sending HTTP requests.
- **Health checks:** After a deactivated engine passes a health check, the server calls `_rate_limiter.reactivate(engine_name)`.
- **Shutdown:** The server calls `_rate_limiter.shutdown()` during graceful shutdown.

## Entry points for modification

- Changing the production strategy: modify server startup to use `ValkeySlidingWindow` instead of `LocalTokenBucket`
- Adjusting backpressure parameters: modify cooldown duration or strike count in `RateLimiter.acquire()`
- Adding a new strategy: subclass `RateLimitStrategy` and implement `acquire()`

## Key source files

| File | Description |
|---|---|
| `slopsearx/ratelimit.py` | All rate-limiting strategies (`LocalTokenBucket`, `ValkeySlidingWindow`, `ExternalSidecar`), `RateLimiter` backpressure wrapper, and `_EngineState` tracking |
