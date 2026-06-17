# SearchResult

Active contributors: Magnus Hedemark

## Purpose

`SearchResult` is the internal canonical data model for a single search result. It is decoupled from the SearXNG wire format — formatters map between `SearchResult` and the output serialization. This prevents carrying forward SearXNG's design decisions without questioning them.

## Definition

```python
@dataclass
class SearchResult:
    url: str
    title: str
    content: str
    engine: str                           # primary engine name
    engines: set[str] = field(default_factory=set)
    score: float = 0.0
    position: int = 0
    category: str = "general"
    published_date: str | None = None     # ISO 8601
    thumbnail: str | None = None
    img_src: str | None = None
    tier: int = 1                         # 1 = primary (broad), 2 = secondary (specialized)
```

## Fields

| Field | Type | Description |
|---|---|---|
| `url` | `str` | Full URL of the search result |
| `title` | `str` | Page title |
| `content` | `str` | Content snippet or description |
| `engine` | `str` | Primary engine that returned this result |
| `engines` | `set[str]` | All engines that returned this URL (populated by merger) |
| `score` | `float` | Relevance score. In V1: `1.0 * len(engines)` |
| `position` | `int` | 1-based rank position (populated by merger) |
| `category` | `str` | SearXNG-compatible category (default `"general"`) |
| `published_date` | `str | None` | ISO 8601 datetime string (if available) |
| `thumbnail` | `str | None` | Thumbnail image URL (if available) |
| `img_src` | `str | None` | Full image URL (if available) |
| `tier` | `int` | Engine tier: 1 = broad/general-purpose, 2 = specialized |

## Lifecycle

1. **Created by adapters** in `search()` — populated with engine-specific fields (url, title, content, category, published_date, thumbnail, img_src)
2. **Annotated by server** — `tier` field set based on `_TIER1_ENGINES` membership
3. **Merged by PresenceRanker** — `engines` set populated with all matching engine names, `score` computed as `1.0 * len(engines)`, `position` set by sort order
4. **Serialized by formatters** — `_result_to_searxng()` maps to SearXNG fields, `format_yaml_markdown()` selects subset for YAML

## Related types

- **`AdapterResponse`** — wrapper containing `list[SearchResult]` + `EngineStatus` + metadata
- **`EngineStatus`** — enum classifying adapter outcome: `OK`, `RATE_LIMITED`, `BLOCKED`, `ERROR`, `TIMEOUT`

## Key source files

| File | Description |
|---|---|
| `slopsearx/adapter.py` | SearchResult, AdapterResponse, EngineStatus definitions |
| `slopsearx/merger.py` | PresenceRanker populates engines, score, position |
| `slopsearx/formatter.py` | Maps SearchResult to output formats |
