# Adapter interface

Active contributors: Magnus Hedemark

## Purpose

The adapter interface is the primary architectural invariant of SlopSearX. Every search engine is exactly one file, registered via `@register_engine`. Adding a new engine requires zero changes to the orchestrator. The adapter contract is the boundary across which all engine-specific complexity is localized.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `EngineAdapter` | `slopsearx/adapter.py` | Abstract base class for all engine adapters. Subclasses override `search()`. Provides `_merge_categories()` for category override/add/remove, `_check_rate_limit()` for rate limiting, and sensible defaults for `health()`, `warmup()`, and `shutdown()`. The rate limiter is injected at construction time via `__init__(self, config, rate_limiter)`. |
| `ScrapeAdapter` | `slopsearx/adapter.py` | Base class for HTML-scrape engines. Subclasses of `EngineAdapter` that send HTTP requests with stealth headers and parse HTML responses. No headless browser required. Integrates with `ProxyPool` via `_proxy_pool` instance, `_get_proxy()`, `_report_proxy_success()`, and `_report_proxy_failure()` methods for proxy rotation. |
| `SearchResult` | `slopsearx/adapter.py` | Internal normalized result dataclass. Decoupled from wire format. Contains fields for URL, title, content, engine metadata, score, category, published date, and media references. |
| `AdapterResponse` | `slopsearx/adapter.py` | Canonical return type for every adapter's `search()` method. Contains a list of `SearchResult`, an `EngineStatus`, optional error message, measured latency, and SearXNG extended fields (`answers`, `corrections`, `infoboxes`). |
| `EngineStatus` | `slopsearx/adapter.py` | Standardized error classification enum. Members: `OK`, `RATE_LIMITED`, `BLOCKED`, `ERROR`, `TIMEOUT`. The orchestrator uses these to decide per-engine result inclusion and backpressure. |
| `register_engine` | `slopsearx/adapter.py` | Decorator that registers an adapter class in the global `_ENGINE_REGISTRY` dict. Enforces that the class subclasses `EngineAdapter` and has a non-empty `name` attribute. |
| `discover_engines` | `slopsearx/adapter.py` | Instantiates all registered adapters with their per-engine config. Respects the `enabled` flag. Merges category overrides from config before instantiation. |

## How it works

### The decorator pattern

When a Python file in the `engines/` directory is imported, any class decorated with `@register_engine` is added to the `_ENGINE_REGISTRY` dict, keyed by its `name` attribute:

```python
_ENGINE_REGISTRY: dict[str, type[EngineAdapter]] = {}

def register_engine(cls: type[EngineAdapter]) -> type[EngineAdapter]:
    assert issubclass(cls, EngineAdapter), f"{cls.__name__} must subclass EngineAdapter"
    assert cls.name, f"{cls.__name__} must set a non-empty class-level 'name'"
    _ENGINE_REGISTRY[cls.name] = cls
    return cls
```

The decorator validates at registration time that the class subclasses `EngineAdapter` and has a non-empty name. This catches misconfigured adapters at import time rather than at runtime.

### Engine discovery

`discover_engines()` iterates the registry and creates instances:

```python
def discover_engines(engine_configs: dict[str, dict] | None = None) -> dict[str, EngineAdapter]:
```

For each registered class, it looks up per-engine config, checks the `enabled` flag, restructures category config for the `_merge_categories()` method, and calls the constructor. Only enabled engines are instantiated.

### Category merging

The `_merge_categories()` instance method on `EngineAdapter` merges self-declared categories with config overrides. It supports three modes:

- A bare list in config acts as a full override (backward compatible)
- A dict with `override` replaces categories entirely
- A dict with `add` appends to self-declared categories
- A dict with `remove` suppresses categories from self-declared

The merge logic is invoked at construction time inside `__init__()`, ensuring categories are resolved before any search request is dispatched.

### Proxy pool integration (ScrapeAdapter)

`ScrapeAdapter` subclasses automatically get proxy rotation support via the `ProxyPool` class:

- `_proxy_pool`: A `ProxyPool` instance created from the engine's config via `ProxyPool.from_config()`
- `_get_proxy()`: Returns an httpx-compatible proxy dict (`{"all://": "..."}`) or `None` if no proxy is configured
- `_report_proxy_success(proxy)`: Resets the failure count for a proxy after a successful request
- `_report_proxy_failure(proxy)`: Marks a proxy for cooloff after a CAPTCHA, 429, or 403 response; cooloff escalates after 3 consecutive failures

Proxy configuration is either a static list (`proxy_pool`) or a dynamic endpoint URL (`scrape_proxy_url`), set via the engine's config.

### Rate limiter injection

At server startup, the `RateLimiter` is injected into every adapter instance:

```python
class EngineAdapter(ABC):
    def __init__(self, config: dict | None = None, rate_limiter: Any = None) -> None:
        self.config = config or {}
        self.rate_limiter = rate_limiter  # injected by server at startup
        self._merge_categories()
```

The `_check_rate_limit()` method provides a safe guard — it returns an `AdapterResponse` with `RATE_LIMITED` status if the rate limiter denies the request, or `None` if allowed. It is safe to call even when `self.rate_limiter` is `None` (e.g., tests).

### The six adapter contract rules

From `spec.md`:

1. **Every adapter is exactly one file.** Adding an engine means adding one Python file with a `@register_engine` decorated class. No config file changes, no orchestrator modifications.
2. **Adapters never raise exceptions.** All error states are classified and returned in `AdapterResponse.status`. The orchestrator never sees an unhandled exception from any adapter.
3. **Adapters own their error classification.** An HTTP 429, a CAPTCHA wall, a DOM change that produces garbage each is classified as transient or permanent.
4. **Adapters own their timeout budget.** The orchestrator sets a global deadline per request; the adapter manages its internal timeouts within that budget.
5. **Adapters own their rate limiting.** Each adapter calls `self.rate_limiter.acquire()` before sending a request. The rate limiter is injected at construction time and is safe to use from `_check_rate_limit()`.
6. **No adapter cross-talk.** Adapters do not share state, do not call each other, and cannot depend on another adapter's results.

## Integration points

- **Registry population:** The `_ENGINE_REGISTRY` is populated when `engines/__init__.py` imports each engine module. The server imports `engines` (via `import engines` at module level in `server.py`) to trigger all `@register_engine` decorators before startup.
- **Server startup:** The `startup()` event handler in `server.py` calls `discover_engines()`, injects the rate limiter into each engine via the constructor, and warms up all engines concurrently.
- **Per-request dispatch:** Each incoming search request calls `_dispatch_engine()` per target engine, which calls the adapter's `search()` method.
- **Lifecycle hooks:** The server calls `warmup()` on startup and `shutdown()` on graceful shutdown for every engine.
- **Proxy rotation:** `ScrapeAdapter` subclasses automatically get proxy rotation via `ProxyPool`, configured through `proxy_pool` (static list) or `scrape_proxy_url` (dynamic endpoint) config keys.

## Entry points for modification

- Adding a new engine: create a new file in `engines/` with a `@register_engine` decorated class, add one import line to `engines/__init__.py`
- Modifying the adapter contract: change base classes or data types in `slopsearx/adapter.py`
- Changing discovery behavior: modify `discover_engines()` or `_merge_categories()`

## Key source files

| File | Description |
|---|---|
| `slopsearx/adapter.py` | Base classes (`EngineAdapter`, `ScrapeAdapter`), data types (`SearchResult`, `AdapterResponse`, `EngineStatus`), registry and discovery functions |
| `engines/__init__.py` | Imports all engine modules, triggering `@register_engine` decoration |
| `docs/ENGINE_ADAPTERS.md` | Full adapter reference with contract rules, data types, lifecycle hooks, sub-categories, and built-in adapter table |
