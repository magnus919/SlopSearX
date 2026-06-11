# Configuration

Active contributors: Magnus Hedemark

## Purpose

Three-layer configuration model: built-in defaults, optional YAML config file, environment variable overrides. Env vars always win.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `Config` | `slopsearx/config.py` | Top-level configuration dataclass. Contains `engines` (dict of `EngineEntry`), `cache` (`CacheConfig`), `ranking` (`RankingConfig`), `routing` (`RoutingConfig`), `default_engines`, `enable_suggestions`, and `log_level`. |
| `CacheConfig` | `slopsearx/config.py` | Cache subsystem configuration: `ttl_seconds` (default 300), `max_result_sets` (default 10000), `revalidate_on_hit` (default False). |
| `RankingConfig` | `slopsearx/config.py` | Ranking strategy selector: `strategy` (default `"presence"`, with `"weighted_fusion"` and `"learning_to_rank"` as future options). |
| `RoutingConfig` | `slopsearx/config.py` | Query routing configuration: `enabled` (default True), `topics` (dict of topic signatures), `fallback` (list of fallback engines). |
| `EngineEntry` | `slopsearx/config.py` | Per-engine configuration dataclass. Covers `enabled`, `base_url`, `type`, `timeout_ms`, `max_results`, `rate_limit`, `weight`, `api_key`, category overrides (`categories`, `categories_add`, `categories_remove`), and scrape-specific proxy fields (`proxy_pool`, `scrape_proxy_url`). |
| `load_config` | `slopsearx/config.py` | Public function that loads and layers all config sources. Returns a fully resolved `Config` dataclass. |
| `_load_env_overrides` | `slopsearx/config.py` | Reads all `ENGINE_*` and `SEARCH_*` environment variables and returns a flat dict of overrides. |
| `_DEFAULT_ENGINES` | `slopsearx/config.py` | Hardcoded default config dicts for all 24 engines, including base URLs, timeouts, rate limits, result caps, and weight values. |

## How it works

### Loading priority

Configuration is loaded in three layers. Each layer overrides the previous one:

```
1. Built-in defaults (hardcoded in _DEFAULT_ENGINES)
2. YAML config file (optional, at /etc/slopsearx/config.yaml by default)
3. Env var overrides (ENGINE_* and SEARCH_* vars always win)
```

### Step-by-step loading

`load_config()` executes these steps in order:

1. **Start with built-in defaults.** Creates a `Config` object with `EngineEntry` instances populated from `_DEFAULT_ENGINES`, a default `CacheConfig`, and a default `RankingConfig`.
2. **Layer the config file.** If a YAML file exists at the specified path, it is loaded with `yaml.safe_load()`. Engine configs from the file are merged over defaults: existing engines get their fields overridden on a per-key basis, and new engines are added. Cache and ranking configs are also overridden from the file.
3. **Layer environment variables.** `_load_env_overrides()` scans all environment variables starting with `ENGINE_` or `SEARCH_` and builds a flat override dict. These overrides are applied to the resolved `Config` object with `_apply_env_overrides()`.

### Environment variable naming convention

Env vars use two namespaces:

- `ENGINE_<NAME>_<SETTING>` for per-engine settings. Example: `ENGINE_BRAVE_API_KEY`, `ENGINE_DUCKDUCKGO_CATEGORIES_ADD`
- `SEARCH_<SETTING>` for global settings. Example: `SEARCH_CACHE_TTL_SECONDS`, `SEARCH_LOG_LEVEL`, `SEARCH_DEFAULT_ENGINES`

The `_load_env_overrides()` function parses these into a flat dict with dotted keys:

```
ENGINE_BRAVE_API_KEY       →  engines.brave.api_key
ENGINE_DUCKDUCKGO_ENABLED  →  engines.duckduckgo.enabled
SEARCH_CACHE_TTL_SECONDS   →  search_cache_ttl_seconds
```

### Config file format

The config file is optional YAML at `/etc/slopsearx/config.yaml`:

```yaml
engines:
  brave:
    enabled: true
    timeout_ms: 5000
    max_results: 10
  duckduckgo:
    enabled: true
    timeout_ms: 10000
    max_results: 10
    proxy_pool: "residential"

cache:
  ttl_seconds: 300
  max_result_sets: 10000

ranking:
  strategy: "presence"

routing:
  enabled: true
  topics:
    code:
      keywords: [python, javascript, rust, golang, react]
      engines: [brave, github, stackexchange, wikipedia]
    science:
      keywords: [quantum, physics, biology, paper, doi]
      engines: [brave, arxiv, semanticscholar, openalex, wikipedia]
  fallback: [brave, wikipedia]

default_engines:
  - brave
  - wikipedia
  - duckduckgo

log_level: "INFO"
enable_suggestions: true
```

### Why the hybrid model

At 50+ replicas with 10+ engines, env-var-only configuration has two hard limits:

1. **Kubernetes pod spec limit of ~32768 bytes** for all environment variables combined. Ten engines with ten config params each using namespace-prefixed names easily exceeds this.
2. **Operational friction.** Changing an engine's timeout or rate limit requires a full rollout with env-var-only. A mounted ConfigMap can be hot-reloaded by the application.

The hybrid model preserves the core stateless property: the file is read once at startup and never written to by the application. Replicas remain interchangeable.

### Per-engine category overrides

The `EngineEntry` dataclass supports three category override mechanisms:

- `categories`: a list that fully replaces the engine's self-declared categories
- `categories_add`: a list appended to the engine's self-declared categories
- `categories_remove`: a list suppressed from the engine's self-declared categories

These can be set via config file or env vars. When set via env vars, the values are comma-separated strings that get coerced to lists.

### Scrape engine proxy fields

Scrape-based engines (DuckDuckGo, Google) support two proxy configuration options on `EngineEntry`:

- `proxy_pool`: Name or list of proxy URLs for round-robin rotation. Typically set to a static pool or a service name.
- `scrape_proxy_url`: A dynamic proxy endpoint URL. When set, the proxy pool uses this single endpoint instead of local rotation.

These are consumed by `ProxyPool` which integrates with `ScrapeAdapter` at construction time.

## Integration points

- **Server startup:** `startup()` in `server.py` calls `load_config()`, then converts `EngineEntry` dataclasses to dicts for `discover_engines()`. The config also drives `QueryRouter` initialization, `SuggestionService` opt-in (`enable_suggestions`), and `EngineStatsTracker` setup.
- **Per-engine config:** Each engine's adapter instance receives its config dict as `self.config`
- **Cache config:** `SearchCache` TTL values are derived from the loaded config
- **Routing config:** `QueryRouter` reads the `routing` section from config to build topic-signature mappings for query-based engine selection
- **Suggestion opt-in:** The `enable_suggestions` flag controls whether `SuggestionService` (Brave Suggest API) is initialized at startup
- **Env var injection:** API keys are read from env vars at config loading time, never stored in the config file

## Entry points for modification

- Adding a new config parameter: add a field to the relevant dataclass (`Config`, `EngineEntry`, `CacheConfig`, `RankingConfig`)
- Adding a new engine default: add an entry to `_DEFAULT_ENGINES`
- Changing env var parsing: modify `_load_env_overrides()` or `_apply_env_overrides()`

## Key source files

| File | Description |
|---|---|
| `slopsearx/config.py` | All config loading logic, dataclasses, env var parsing, and engine defaults |
