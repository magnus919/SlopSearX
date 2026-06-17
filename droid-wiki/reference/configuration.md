# Configuration reference

Complete reference for all configuration options in SlopSearX.

## Configuration layers

Priority (highest wins):
1. Environment variables (`ENGINE_*`, `SEARCH_*`, `FEATURE_*`)
2. YAML config file (`/etc/slopsearx/config.yaml`)
3. Built-in defaults (hardcoded in `slopsearx/config.py`)

## YAML config file

Path: `/etc/slopsearx/config.yaml` (or custom via `config_path` parameter)

```yaml
# Engine overrides
engines:
  brave:
    enabled: true
    base_url: "https://api.search.brave.com/res/v1/web/search"
    rate_limit: 15
    timeout_ms: 5000
    max_results: 10
    weight: 1.0
  duckduckgo:
    enabled: true
    type: scrape
    timeout_ms: 10000
    max_results: 10
    proxy_pool: ["http://proxy1:8080", "http://proxy2:8080"]
    weight: 0.6

# Cache settings
cache:
  ttl_seconds: 300
  max_result_sets: 10000

# Ranking strategy
ranking:
  strategy: presence  # "presence" | "weighted_fusion" | "learning_to_rank"

# Query routing
routing:
  enabled: true
  topics:  # optional topic overrides
    code:
      keywords: [python, javascript, rust]
      engines: [brave, github, stackexchange]

# Feature flags (all default to false)
features:
  ai_dispatch: false
  experimental_ranking: false

# Global settings
default_engines: [brave, wikipedia]
log_level: INFO
enable_suggestions: false
```

## Environment variables

### Engine config

| Variable | Example | Description |
|---|---|---|
| `ENGINE_{NAME}_API_KEY` | `ENGINE_BRAVE_API_KEY=abc123` | API key for the engine |
| `ENGINE_{NAME}_ENABLED` | `ENGINE_DDG_ENABLED=true` | Enable/disable engine |
| `ENGINE_{NAME}_TIMEOUT_MS` | `ENGINE_DDG_TIMEOUT_MS=15000` | Per-engine timeout |
| `ENGINE_{NAME}_RATE_LIMIT` | `ENGINE_BRAVE_RATE_LIMIT=20` | Per-engine rate limit (req/s) |
| `ENGINE_{NAME}_CATEGORIES` | `ENGINE_MYENG_CATEGORIES=general,news` | Category override |
| `ENGINE_{NAME}_CATEGORIES_ADD` | `ENGINE_MYENG_CATEGORIES_ADD=finance` | Append categories |
| `ENGINE_{NAME}_CATEGORIES_REMOVE` | `ENGINE_MYENG_CATEGORIES_REMOVE=images` | Remove categories |
| `ENGINE_{NAME}_PROXY_POOL` | `ENGINE_DDG_PROXY_POOL=http://p1:8080,http://p2:8080` | Proxy list |

### Global config

| Variable | Default | Description |
|---|---|---|
| `VALKEY_URL` | (none) | Valkey connection string |
| `SENTRY_DSN` | (none) | Sentry DSN |
| `MAX_CONCURRENT_ENGINES` | 10 | Max concurrent HTTP connections |
| `PER_CLIENT_REQUESTS` | 30 | Per-client rate limit |
| `PER_CLIENT_WINDOW_SECONDS` | 60 | Rate limit window |
| `FAIL_CLOSED` | false | Deny on Valkey failure |
| `FAIL_CLOSED_GRACE_SECONDS` | 30 | Grace before local fallback |
| `SEARCH_CACHE_TTL_SECONDS` | 3600 | Cache TTL |
| `SEARCH_CACHE_NEGATIVE_TTL_SECONDS` | 60 | Negative cache TTL |
| `SEARCH_LOG_LEVEL` | INFO | Log level |
| `SEARCH_DEFAULT_ENGINES` | brave,wikipedia | Default engine list |
| `ENGINE_CIRCUIT_THRESHOLD` | 5 | Consecutive errors before circuit opens |
| `ENGINE_CIRCUIT_TIMEOUT` | 300 | Circuit breaker timeout (seconds) |

### Feature flags

| Variable | Equivalent config | Description |
|---|---|---|
| `FEATURE_{NAME}=true` | `features.{name}: true` | Enable feature flag |
| `FEATURE_{NAME}=false` | `features.{name}: false` | Disable feature flag |

Valid truthy values: `true`, `True`, `TRUE`, `1`, `yes`, `YES`.

## Built-in engine defaults

Hardcoded in `_DEFAULT_ENGINES` in `slopsearx/config.py`. Each engine has defaults for:

| Field | Description |
|---|---|
| `base_url` | API endpoint URL |
| `type` | `"api"`, `"scrape"`, or `"structured"` |
| `timeout_ms` | Request timeout in milliseconds |
| `max_results` | Maximum results to request |
| `rate_limit` | Requests per second (cross-replica) |
| `weight` | Default trust weight (1.0 = baseline) |
| `enabled` | Whether the engine is active (default: true, except internetarchive=false) |

## Config API

`GET /config` returns the current categories → engines mapping, respecting all config overrides:

```json
{
  "categories": {
    "general": ["brave", "duckduckgo", "google", "wikipedia"],
    "science": ["arxiv", "brave", "huggingface", "semanticscholar"],
    "security": ["shodan", "censys", "virustotal"]
  }
}
```

## Key source files

| File | Description |
|---|---|
| `slopsearx/config.py` | Config dataclasses, defaults, loaders |
| `config.yaml` | Optional operator config file |
