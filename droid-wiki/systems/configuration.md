# Configuration

Active contributors: Magnus Hedemark

## Purpose

Three-layer configuration model: built-in defaults → optional YAML file → environment variable overrides. Env vars always win for the same key.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `Config` | `slopsearx/config.py` | Top-level configuration dataclass: engines, cache, ranking, routing, feature flags, globals |
| `EngineEntry` | `slopsearx/config.py` | Per-engine config: enabled, base_url, type, timeout, rate limit, weight, API key, categories |
| `CacheConfig` | `slopsearx/config.py` | Cache settings: TTL, max result sets, revalidate |
| `RankingConfig` | `slopsearx/config.py` | Ranking strategy: presence / weighted_fusion / learning_to_rank |
| `RoutingConfig` | `slopsearx/config.py` | Query routing: enabled flag, topic mappings, fallback engines |
| `FeatureFlags` | `slopsearx/config.py` | Boolean feature toggles: defaults → YAML → env vars. Unknown flags return `False` |
| `load_config()` | `slopsearx/config.py` | Public API: loads all three layers and returns a resolved `Config` object |

## Configuration layers

### Layer 1: Built-in defaults

Hardcoded in `_DEFAULT_ENGINES` dict in `config.py`. Contains production-ready defaults for all 48 engines: base URLs, rate limits, timeouts, weights. Also defaults for cache and ranking.

### Layer 2: YAML config file

Optional file at `/etc/slopsearx/config.yaml` (mounted via K8s ConfigMap or Docker volume). Overrides defaults. Example:

```yaml
engines:
  brave:
    rate_limit: 20
    max_results: 15
  duckduckgo:
    enabled: true
    proxy_pool: ["http://proxy1:8080", "http://proxy2:8080"]
cache:
  ttl_seconds: 600
ranking:
  strategy: presence
routing:
  enabled: true
features:
  ai_dispatch: false
  experimental_ranking: false
```

### Layer 3: Environment variables

**Engine-level:** `ENGINE_{NAME}_{SETTING}` maps to `engines.{name}.{setting}`. Example: `ENGINE_BRAVE_API_KEY=abc123`, `ENGINE_DDG_TIMEOUT_MS=15000`.

**Global:** `SEARCH_CACHE_TTL_SECONDS`, `SEARCH_LOG_LEVEL`, `SEARCH_DEFAULT_ENGINES`, `VALKEY_URL`, `MAX_CONCURRENT_ENGINES`, `PER_CLIENT_REQUESTS`, `PER_CLIENT_WINDOW_SECONDS`, `FAIL_CLOSED`, `FAIL_CLOSED_GRACE_SECONDS`.

**Feature flags:** `FEATURE_{NAME}=true|false|1|0|yes`. Example: `FEATURE_AI_DISPATCH=true`.

## Feature flags

Safe-by-default boolean toggles for gating new behavior:

```python
if config.feature_flags.is_enabled("ai_dispatch"):
    # new behavior here
```

Configured via:
- `config.yaml` → `features: { ai_dispatch: true }`
- Env vars → `FEATURE_AI_DISPATCH=true`
- Unknown flags → `False` (safe by default)

## Engine category override

Operators can reclassify engine categories without modifying adapter code:

```yaml
engines:
  myengine:
    categories:
      - general
      - news
```

Env var equivalent: `ENGINE_MYENG_CATEGORIES=general,news`.

## Entry points

- Add a config option: add field to `Config`/`EngineEntry` dataclass, add default in `_DEFAULT_ENGINES`, support env override in `_load_env_overrides()`
- Add a feature flag: document in `config.yaml` features section, gate code with `config.feature_flags.is_enabled()`
- Change defaults: modify `_DEFAULT_ENGINES` dict

## Key source files

| File | Description |
|---|---|
| `slopsearx/config.py` | Three-layer config model, defaults, loaders, feature flags |
| `config.yaml` | Optional operator config file |
