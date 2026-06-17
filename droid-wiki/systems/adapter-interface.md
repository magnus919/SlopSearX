# Adapter interface

Active contributors: Magnus Hedemark

## Purpose

The adapter interface is the primary architectural invariant of SlopSearX. Every search engine is exactly one file, registered via `@register_engine`. Adding a new engine requires zero changes to the orchestrator.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `sanitize_url` | `slopsearx/adapter.py` | Strips sensitive query parameters (`api_key`, `key`, `apiKey`, `token`, `access_token`) from URLs to prevent credential leakage in error messages and logs |
| `EngineAdapter` | `slopsearx/adapter.py` | Abstract base class for all engine adapters. Provides `_merge_categories()`, `_check_rate_limit()`, circuit breaker state tracking, and sensible defaults for `health()`, `warmup()`, `shutdown()` |
| `ScrapeAdapter` | `slopsearx/adapter.py` | Base class for HTML-scrape engines. Subclasses of `EngineAdapter` that send HTTP requests with stealth headers and parse HTML. Integrates with `ProxyPool` |
| `SearchResult` | `slopsearx/adapter.py` | Internal normalized result dataclass. Decoupled from wire format. Contains URL, title, content, engine metadata, score, category, published date, media refs, and `tier` (1 or 2) |
| `AdapterResponse` | `slopsearx/adapter.py` | Canonical return type. Contains results list, `EngineStatus`, optional error message, latency, and SearXNG extended fields (`answers`, `corrections`, `infoboxes`) |
| `EngineStatus` | `slopsearx/adapter.py` | Error classification enum: `OK`, `RATE_LIMITED`, `BLOCKED`, `ERROR`, `TIMEOUT` |
| `register_engine` | `slopsearx/adapter.py` | Decorator registering adapter class in `_ENGINE_REGISTRY`. Validates subclass + non-empty name at import time |
| `discover_engines` | `slopsearx/adapter.py` | Instantiates all registered adapters with per-engine config. Respects `enabled` flag. Merges category overrides |

## How it works

### The decorator pattern

```python
@register_engine
class BraveAdapter(EngineAdapter):
    name = "brave"
    display_name = "Brave Search API"
    env_prefix = "ENGINE_BRAVE"
    engine_type = "api"
    categories = ["general", "news", "science", "images"]

    async def search(self, query, params=None) -> AdapterResponse:
        ...
```

The decorator validates at registration time that the class subclasses `EngineAdapter` and has a non-empty `name` attribute.

### Engine discovery

`discover_engines()` iterates the registry, looks up per-engine config, checks the `enabled` flag, restructures category config for `_merge_categories()`, and calls the constructor. Only enabled engines are instantiated.

### Category merging

The `_merge_categories()` method supports three modes:
- **Full override:** a bare list or `{"override": [...]}` replaces self-declared categories
- **Add:** `{"add": [...]}` appends to self-declared
- **Remove:** `{"remove": [...]}` suppresses from self-declared

Operators can override via config.yaml or env vars (`ENGINE_MYENG_CATEGORIES`, `_ADD`, `_REMOVE`).

### Circuit breaker

Each engine has a built-in circuit breaker:

- **Threshold:** 5 consecutive errors (configurable via `ENGINE_CIRCUIT_THRESHOLD`)
- **Timeout:** 300 seconds (configurable via `ENGINE_CIRCUIT_TIMEOUT`)
- **Half-open probes:** When timeout expires, the next request probes. Success closes the circuit; failure re-opens for another full timeout

The server checks `engine.circuit_allowed()` before dispatching. Open circuits skip dispatch entirely.

### URL sanitization

`sanitize_url()` strips known sensitive query parameters from URLs to prevent credential leakage in error messages. Called in the dispatch error handler before the error message is stored.

## The six adapter contract rules

1. **Every adapter is exactly one file** — add one Python file, zero orchestrator changes
2. **Adapters never raise exceptions** — all errors classified and returned in `AdapterResponse.status`
3. **Error classification** — adapters classify HTTP 429, CAPTCHA, DOM changes as transient/permanent
4. **Timeout budget** — adapters manage internal timeouts within the orchestrator's global deadline
5. **Rate limiting** — adapters call `self.rate_limiter.acquire()` before sending requests; rate limiter injected at construction
6. **No adapter cross-talk** — adapters don't share state, call each other, or depend on other results

## Integration points

- **Registry population:** `engines/__init__.py` imports trigger `@register_engine`
- **Server startup:** `discover_engines()` + rate limiter injection + concurrent warmup
- **Per-request dispatch:** `_dispatch_engine()` → `adapter.search()`
- **Lifecycle hooks:** `warmup()` at startup, `shutdown()` at graceful shutdown

## Entry points

- Add a new engine: create file in `engines/`, add import to `__init__.py`
- Modify adapter contract: change base classes or data types in `adapter.py`
- Change discovery: modify `discover_engines()` or `_merge_categories()`

## Key source files

| File | Description |
|---|---|
| `slopsearx/adapter.py` | Base classes, data types, registry, discovery |
| `engines/__init__.py` | Imports all engine modules |
| `engines/*.py` | Individual engine adapters |
| `docs/ENGINE_ADAPTERS.md` | Full adapter reference |
