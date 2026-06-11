# Data models reference

## SearchResult

The internal model for a single search result.

| Field | Type | Description |
|---|---|---|
| `title` | str | Result title |
| `url` | str | Result URL |
| `content` | str | Snippet or abstract text |
| `engine` | str | Name of the engine that produced this result |
| `score` | float | Relevance score (higher is better) |
| `category` | str | Result category tag |
| `published_date` | datetime or None | Publication date if available |
| `source` | str | Source name for display |
| `img_src` | str or None | Source image URL |
| `thumbnail` | str or None | Thumbnail image URL |
| `template` | str or None | SearXNG template name for rendering |

## AdapterResponse

Return type from every engine adapter's `search()` method.

| Field | Type | Description |
|---|---|---|
| `status` | EngineStatus | Status classification |
| `results` | list[SearchResult] | Search results from this engine |
| `error` | str or None | Error message if status indicates failure |
| `elapsed_ms` | float | Wall-clock time spent in the adapter |
| `answers` | list[dict] | Direct answer content from engines (default `[]`) |
| `corrections` | list[str] | Suggested query corrections (default `[]`) |
| `infoboxes` | list[dict] | Structured info box metadata (default `[]`) |

The three extended fields (`answers`, `corrections`, `infoboxes`) are aggregated from all engine responses and included in the JSON output. They default to empty lists and are populated only by adapters that support them.

## EngineStatus

Enumeration of possible adapter response statuses.

| Value | Description |
|---|---|
| `ok` | Search completed successfully |
| `rate_limited` | Engine returned rate limit response |
| `blocked` | Engine blocked the request (403, CAPTCHA, IP ban) |
| `error` | Unclassified error |
| `timeout` | Request timed out |

The `BLOCKED` status is used for CAPTCHA walls, IP bans, and HTTP 403/503 responses from scrape engines. All five values (OK, RATE_LIMITED, BLOCKED, ERROR, TIMEOUT) are defined.

## Config

Top-level configuration dataclass.

| Field | Type | Default | Description |
|---|---|---|---|
| `cache` | CacheConfig | default | Cache settings |
| `ranking` | RankingConfig | default | Ranking settings |
| `default_engines` | list[str] | all engines | Engines to query by default |
| `valkey_url` | str | `valkey://localhost:6379` | Valkey connection string |
| `log_level` | str | `INFO` | Logging verbosity |

## CacheConfig

| Field | Type | Default | Description |
|---|---|---|---|
| `ttl_seconds` | int | `300` | Time-to-live for cached responses |
| `max_result_sets` | int | `10_000` | Maximum number of cached result sets |
| `revalidate_on_hit` | bool | `False` | Revalidate cached entries on cache hit |

## RankingConfig

| Field | Type | Default | Description |
|---|---|---|---|
| `max_results_per_engine` | int | `10` | Max results to keep from each engine |
| `total_max_results` | int | `50` | Max results in the final merged response |
| `strategy` | str | `presence` | Ranking strategy (`presence`, `weighted_fusion`, or `learning_to_rank`) |

## RoutingConfig

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `True` | Whether topic routing is enabled |
| `topics` | dict or None | `None` | Topic-to-engine mapping configuration |
| `fallback` | list[str] or None | `None` | Fallback engines when routing yields no matches |

## EngineEntry

Configuration entry for a single engine.

| Field | Type | Description |
|---|---|---|
| `name` | str | Engine identifier matching the adapter class |
| `display_name` | str | Human-readable engine name |
| `categories` | list[str] | Category tags this engine supports |
| `api_key` | str or None | Optional API key |
| `enabled` | bool | Whether this engine is active |
