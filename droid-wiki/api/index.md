# API reference

SlopSearX exposes a REST API over HTTP compatible with the SearXNG query interface.

## GET /search

Main search endpoint. Returns search results in the requested format.

### Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `q` | string | Yes | Search query |
| `format` | string | No | Response format: `json` (default) or `yaml` |
| `categories` | string | No | Comma-separated category filter (e.g. `science`, `news`, `general`) |
| `engines` | string | No | Comma-separated engine list overrides category filter |
| `language` | string | No | Language code (e.g. `en`, `de`, `fr`) |
| `pageno` | integer | No | Page number (default: 1) |
| `time_range` | string | No | Time range filter (e.g. `day`, `week`, `month`, `year`) |
| `safesearch` | integer | No | Safe search level: 0 (off), 1 (moderate), 2 (strict) |

### Response (JSON format)

Returns a SearXNG-compatible JSON object with 23 standard fields plus `meta.*` extensions. Key response fields:

- `query` - Original search query
- `results` - Array of result objects with title, url, content, engine, etc.
- `infoboxes` - Array of infobox objects for entity-style results
- `suggestions` - Array of suggested queries
- `answers` - Array of direct answer strings
- `engines` - Array of engine names queried
- `page` - Current page number
- `meta` - Extension object with timing, cache status, and per-engine stats

### Example

```
GET /search?q=quantum+computing&format=json&categories=science

Response: 200 OK
{
  "query": "quantum computing",
  "results": [...],
  "infoboxes": [],
  "suggestions": ["quantum computing for beginners", ...],
  "answers": [],
  "engines": ["arxiv", "semanticscholar"],
  "page": 1,
  "meta": {
    "elapsed_ms": 342,
    "cache_hit": false,
    "engine_stats": {
      "arxiv": {"status": "ok", "elapsed_ms": 210, "results": 5},
      "semanticscholar": {"status": "ok", "elapsed_ms": 132, "results": 8}
    }
  }
}
```

## GET /health

Per-engine health check endpoint.

### Response

```json
{
  "status": "ok",
  "version": "0.1.0",
  "engines": {
    "brave": {"status": "ok", "last_check": "2026-06-09T12:00:00Z"},
    "wikipedia": {"status": "error", "error": "rate_limited", "last_check": "2026-06-09T12:00:00Z"},
    "duckduckgo": {"status": "ok", "last_check": "2026-06-09T12:00:00Z"}
  }
}
```

## GET /metrics

OpenMetrics endpoint for Prometheus scraping. Exposes request counts, latency histograms, cache hit ratios, and per-engine result counts in plain-text Prometheus exposition format.

## GET /config

Categories-to-engines mapping for runtime discovery.

### Response

```json
{
  "categories": {
    "general": ["brave", "duckduckgo", "google"],
    "science": ["arxiv", "semanticscholar"],
    "news": ["brave", "hackernews"],
    "code": ["github", "stackexchange"]
  },
  "engines": {
    "brave": {"display_name": "Brave Search", "categories": ["general", "news"]},
    "arxiv": {"display_name": "arXiv", "categories": ["science"]}
  }
}
```
