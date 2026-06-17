# Proxy pool

Active contributors: Magnus Hedemark

## Purpose

Manages a pool of proxies for scrape-based adapters (DuckDuckGo, Google, Exploit-DB). Cycles through proxies round-robin and tracks failures for escalating cooloff. Designed to prevent CAPTCHA walls and IP bans when connecting from a single IP.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `ProxyPool` | `slopsearx/proxypool.py` | Round-robin proxy pool with failure tracking and escalating cooloff |
| `get_proxy()` | `slopsearx/proxypool.py` | Returns next healthy proxy as `{"all://": "http://..."}` |
| `report_failure()` | `slopsearx/proxypool.py` | Marks proxy for cooloff after CAPTCHA/429/403 |
| `report_success()` | `slopsearx/proxypool.py` | Resets failure count for a proxy |
| `from_config()` | `slopsearx/proxypool.py` | Factory method creating `ProxyPool` from engine config dict |

## How it works

### Configuration

Two modes:
- **Static proxy list** — `proxy_pool: ["http://proxy1:8080", "http://proxy2:8080"]`
- **Dynamic endpoint** — `scrape_proxy_url: "http://scrape-proxy:8080/search"` (single URL, no rotation)

### Rotation

When a static list is configured:
1. `get_proxy()` returns the next proxy in round-robin order
2. Proxies on cooloff are skipped
3. If all proxies are on cooloff, the next one is returned anyway (fail-open)

### Failure tracking

- **Consecutive failures counted** per proxy
- **Base cooloff:** `DEFAULT_COOLOFF_SECONDS` (120s)
- **Escalation:** after 3 consecutive failures, cooloff triples (`base × 3`)
- **Success reset:** `report_success()` clears failure count and cooloff

### ScrapeAdapter integration

`ScrapeAdapter.__init__()` auto-creates a `ProxyPool` from config. Subclasses call:
- `self._get_proxy()` — returns proxy dict or `None`
- `self._report_proxy_success(proxy)` — after successful request
- `self._report_proxy_failure(proxy)` — after CAPTCHA, 429, or 403

## Integration points

- **ScrapeAdapter constructor:** `ProxyPool.from_config(self.config)` auto-creates pool
- **ScrapeAdapter subclasses:** `_get_proxy()`, `_report_proxy_success()`, `_report_proxy_failure()` used in `search()`
- **Engine config:** `proxy_pool` (list) or `scrape_proxy_url` (string) config keys

## Entry points

- Add proxies: set `proxy_pool` in per-engine config
- Change cooloff: set `proxy_cooloff_seconds` in per-engine config or modify `_DEFAULT_COOLOFF_SECONDS`
- Change rotation: modify `get_proxy()` round-robin logic

## Key source files

| File | Description |
|---|---|
| `slopsearx/proxypool.py` | ProxyPool class |
| `slopsearx/adapter.py` | ScrapeAdapter integration |
