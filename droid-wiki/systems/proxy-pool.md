# Proxy pool

Active contributors: Magnus Hedemark

## Purpose

Round-robin proxy pool for scrape-based engine adapters (DuckDuckGo, Google) with failure tracking and escalating cooloff. Designed to prevent CAPTCHA blocks and rate-limit bans when multiple requests originate from a single IP address. Supports both static proxy lists and dynamic single-endpoint proxies.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `ProxyPool` | `slopsearx/proxypool.py` | Round-robin proxy manager. Cycles through proxies, tracks consecutive failures, applies cooloff periods. |
| `get_proxy()` | `slopsearx/proxypool.py` | Returns the next healthy proxy as an httpx-compatible dict (`{"all://": "http://..."}`), or `None` if no proxies are configured. |
| `report_failure()` | `slopsearx/proxypool.py` | Marks a proxy as failed (CAPTCHA, 429, 403). Increments consecutive failure counter and sets cooloff timer. |
| `report_success()` | `slopsearx/proxypool.py` | Resets the failure counter and clears cooloff for a successfully used proxy. |
| `from_config()` | `slopsearx/proxypool.py` | Factory classmethod that creates a `ProxyPool` from an engine's config dict. Returns `None` if no proxy configuration is present. |
| `available_count` | `slopsearx/proxypool.py` | Read-only property returning the number of proxies not currently on cooloff. |
| `total_count` | `slopsearx/proxypool.py` | Read-only property returning the total number of proxies in the pool. |
| `ScrapeAdapter` | `slopsearx/adapter.py` | Base class for scrape engines. Creates a `ProxyPool` via `from_config()` and delegates proxy selection/success/failure reporting. |

## How it works

### Proxy rotation cycle

1. **Initialization.** `from_config()` reads `proxy_pool` (list of proxy URLs) and/or `scrape_proxy_url` (dynamic single endpoint) from the engine's config dict. Returns `None` if neither is configured.
2. **Dynamic endpoint mode.** If `scrape_proxy_url` is set, all `get_proxy()` calls return that single URL — no local rotation needed. This is used when an external proxy service (e.g., residential proxy provider) handles rotation.
3. **Static pool mode.** If a list of proxies is provided, `get_proxy()` cycles through them using `itertools.cycle()` for round-robin distribution.
4. **Cooloff check.** Before returning a proxy, `get_proxy()` checks whether it is on cooloff (current time < `cooloff_until` timestamp). Proxies on cooloff are skipped.
5. **Fail-open.** If all proxies are on cooloff after a full rotation, the next proxy in the cycle is returned anyway (logging a warning). This prevents total query failure when the entire pool is temporarily exhausted.

### Escalating cooloff

When a request through a proxy fails (CAPTCHA, HTTP 429, HTTP 403):

1. **Failure counter incremented.** `report_failure()` increments the consecutive failure count for that proxy.
2. **Cooloff duration calculated.** Base cooloff is `proxy_cooloff_seconds` (default 120). If failures >= 3, the duration is tripled.
3. **Cooloff timer set.** The proxy becomes unavailable until `time.monotonic() + duration`.
4. **Success resets.** `report_success()` clears both the failure counter and the cooloff timer.

Example cooloff progression:
- 1st failure: 120s cooloff
- 2nd failure: 120s cooloff
- 3rd failure: 360s cooloff (tripled)
- 4th+ failure: 360s cooloff (stays tripled)

### Proxy dict format

`get_proxy()` returns httpx-compatible proxy dicts:

```python
{"all://": "http://proxy1:8080"}   # HTTP proxy for all protocols
```

The `_extract_url()` static method handles extraction of the raw URL from any httpx proxy dict format by taking the first non-empty value.

### ScrapeAdapter integration

`ScrapeAdapter.__init__()` (the base class for scrape engines like DDG and Google) creates a `ProxyPool` instance from the engine's config:

```python
self._proxy_pool = ProxyPool.from_config(self.config)
```

Three methods on `ScrapeAdapter` delegate to the pool:

```python
def _get_proxy(self) -> dict[str, str] | None:
    if self._proxy_pool is None:
        return None
    return self._proxy_pool.get_proxy()

def _report_proxy_success(self, proxy):
    if self._proxy_pool is not None:
        self._proxy_pool.report_success(proxy)

def _report_proxy_failure(self, proxy):
    if self._proxy_pool is not None:
        self._proxy_pool.report_failure(proxy)
```

Scrape engines call `_get_proxy()` before making HTTP requests and pass the returned proxy dict to httpx. After the request, they call either `_report_proxy_success()` or `_report_proxy_failure()` based on the response.

### Configuration

Proxy configuration is per-engine in `config.yaml`:

```yaml
engines:
  duckduckgo:
    enabled: true
    proxy_pool:
      - "http://proxy1:8080"
      - "http://proxy2:8080"
      - "http://proxy3:8080"
    proxy_cooloff_seconds: 120

  google:
    enabled: true
    scrape_proxy_url: "http://residential-proxy-service:9000"
```

- `proxy_pool`: list of static proxy URLs for round-robin rotation
- `scrape_proxy_url`: single dynamic proxy endpoint (external rotation)
- `proxy_cooloff_seconds`: base cooloff duration in seconds (default 120)

Only one of `proxy_pool` or `scrape_proxy_url` is typically used at a time. If both are present, `scrape_proxy_url` takes precedence and the static pool is bypassed.

## Integration points

- **ScrapeAdapter base class:** `_get_proxy()`, `_report_proxy_success()`, and `_report_proxy_failure()` methods on `ScrapeAdapter` in `slopsearx/adapter.py` delegate to the pool
- **DuckDuckGo adapter:** Uses pool via `ScrapeAdapter._get_proxy()` before each scrape request, reports success/failure based on response status
- **Google adapter:** Uses pool via `ScrapeAdapter._get_proxy()` before each scrape request, reports success/failure based on response status
- **Config system:** Proxy settings are defined in per-engine `EngineEntry` config (fields: `proxy_pool`, `scrape_proxy_url`, `proxy_cooloff_seconds`)

## Entry points for modification

- Adding new proxy selection strategy (e.g., least-recently-used, weighted random): modify `get_proxy()` in `proxypool.py`
- Changing cooloff escalation logic: modify `report_failure()` — adjust thresholds, multipliers, or duration formula
- Adding proxy health checks: add a periodic health probe method to `ProxyPool`
- Exposing proxy pool metrics: add Prometheus gauge metrics for `available_count` and `total_count`
- Adding authentication support: extend `ProxyPool` to handle proxy credentials

## Key source files

| File | Description |
|---|---|
| `slopsearx/proxypool.py` | ProxyPool class, round-robin rotation, cooloff tracking, from_config factory |
| `slopsearx/adapter.py` | ScrapeAdapter base class with proxy delegation methods (_get_proxy, _report_proxy_success/failure) |
| `slopsearx/config.py` | EngineEntry proxy config fields (proxy_pool, scrape_proxy_url, proxy_cooloff_seconds) |
