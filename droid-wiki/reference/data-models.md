# Data models reference

Internal data types used throughout SlopSearX.

## Core types

### SearchResult

```python
@dataclass
class SearchResult:
    url: str                          # Full URL
    title: str                        # Page title
    content: str                      # Snippet or description
    engine: str                       # Primary engine name
    engines: set[str]                 # All engines returning this URL
    score: float = 0.0                # Relevance score
    position: int = 0                 # 1-based rank position
    category: str = "general"         # SearXNG category
    published_date: str | None = None # ISO 8601
    thumbnail: str | None = None      # Thumbnail URL
    img_src: str | None = None        # Full image URL
    tier: int = 1                     # 1=primary, 2=specialized
```

### AdapterResponse

```python
@dataclass
class AdapterResponse:
    results: list[SearchResult]                      # Search results
    status: EngineStatus                             # Outcome classification
    error_message: str | None = None                  # Error details
    latency_ms: float = 0.0                           # Request latency
    answers: list[dict] = field(default_factory=list) # Answer boxes
    corrections: list[str] = field(default_factory=list)  # Spelling corrections
    infoboxes: list[dict] = field(default_factory=list)   # Info boxes
```

### EngineStatus

```python
class EngineStatus(enum.Enum):
    OK = "ok"                   # Success
    RATE_LIMITED = "rate_limited"  # Rate limited by engine
    BLOCKED = "blocked"         # CAPTCHA or IP ban
    ERROR = "error"             # General error
    TIMEOUT = "timeout"         # Request timed out
```

## Config types

### Config

```python
@dataclass
class Config:
    engines: dict[str, EngineEntry]
    cache: CacheConfig
    ranking: RankingConfig
    routing: RoutingConfig
    feature_flags: FeatureFlags
    default_engines: list[str]
    log_level: str
    enable_suggestions: bool
```

### EngineEntry

```python
@dataclass
class EngineEntry:
    enabled: bool = True
    base_url: str = ""
    type: str = "api"
    timeout_ms: int = 5000
    max_results: int = 10
    rate_limit: float | None = None
    weight: float = 1.0
    api_key: str | None = None
    categories: list[str] | None = None
    categories_add: list[str] | None = None
    categories_remove: list[str] | None = None
    proxy_pool: str | None = None
    scrape_proxy_url: str | None = None
```

### CacheConfig

```python
@dataclass
class CacheConfig:
    ttl_seconds: int = 300
    max_result_sets: int = 10000
    revalidate_on_hit: bool = False
```

### RankingConfig

```python
@dataclass
class RankingConfig:
    strategy: str = "presence"
```

### RoutingConfig

```python
@dataclass
class RoutingConfig:
    enabled: bool = True
    topics: dict | None = None
    fallback: list[str] | None = None
```

### FeatureFlags

```python
@dataclass
class FeatureFlags:
    flags: dict[str, bool] = field(default_factory=dict)

    def is_enabled(self, name: str) -> bool:
        return self.flags.get(name, False)
```

## Rate limiting types

### _EngineState

```python
@dataclass
class _EngineState:
    consecutive_failures: int = 0
    cooldown_until: float = 0.0
    deactivated: bool = False
```

## Audit stream schema

```
query_audit:{YYYY-MM-DD} → Stream
  query               "search query text"
  client_ip           "10.0.0.1"
  timestamp           "2026-06-14T12:34:56.789Z"
  engines             "brave,wikipedia"
  engines_ok          2
  engines_error       1
  engines_timeout     0
  total_results       15
  latency_ms          1234.5
```

## Engine stats schema

```
engine_stats:{engine}:{YYYY-MM-DD} → Hash
  queries            15230
  results_returned   142301
  errors             234
  rate_limited       45
  total_latency_ms   640920
  total_score        11103
```

## Key source files

| File | Description |
|---|---|
| `slopsearx/adapter.py` | SearchResult, AdapterResponse, EngineStatus |
| `slopsearx/config.py` | Config, EngineEntry, CacheConfig, RankingConfig, FeatureFlags |
| `slopsearx/ratelimit.py` | _EngineState |
