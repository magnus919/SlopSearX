# Output formatters

Active contributors: Magnus Hedemark

## Purpose

Map the internal `SearchResult` dataclass to two output formats: SearXNG-compatible JSON (programmatic consumption) and YAML+Markdown (AI agent consumption). The internal schema is decoupled from the wire format — formatters are a translation layer.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `format_json()` | `slopsearx/formatter.py` | Produces SearXNG-compatible JSON response |
| `format_yaml_markdown()` | `slopsearx/formatter.py` | Produces YAML document with embedded Markdown section |
| `_result_to_searxng()` | `slopsearx/formatter.py` | Maps a single `SearchResult` to all 23 SearXNG fields |
| `_iso_to_epoch()` | `slopsearx/formatter.py` | Converts ISO 8601 datetime to Unix epoch for SearXNG `pubdate` field |

## JSON output (format=json)

Default output format. Maintains backward compatibility with all 23 SearXNG `MainResult` fields:

```json
{
  "query": "python async web scraping",
  "results": [
    {
      "url": "https://example.com/guide",
      "title": "Async Web Scraping with Python",
      "content": "A comprehensive guide...",
      "engine": "brave",
      "engines": ["brave"],
      "score": 0.92,
      "positions": [1],
      "category": "general",
      "publishedDate": "2025-11-15T00:00:00Z",
      "pubdate": 1763251200,
      "tier": 1,
      "length": null,
      "thumbnail": null,
      "img_src": null,
      "iframe_src": null,
      "audio_src": null,
      "views": null,
      "author": null,
      "metadata": null,
      "template": "default.html",
      "parsed_url": null,
      "open_group": false,
      "close_group": false,
      "priority": ""
    }
  ],
  "engines": [{"engine": "brave", "results": 10}],
  "number_of_results": 10,
  "answers": [],
  "corrections": [],
  "infoboxes": [],
  "suggestions": ["python aiohttp", "async web scraping tutorial"],
  "unresponsive_engines": [["duckduckgo", "CAPTCHA wall detected"]],
  "meta": {
    "response_time_ms": 1420,
    "cached": false,
    "query_id": "ssx-abc12345",
    "engine_status": {
      "brave": {"results": 10, "latency_ms": 340.0, "status": "ok"},
      "wikipedia": {"results": 2, "latency_ms": 89.0, "status": "ok"}
    }
  }
}
```

**Extensions beyond SearXNG:**

| Field | Description |
|---|---|
| `tier` | Engine tier (1 or 2) per result |
| `meta.*` | Server-side metadata: response time, cache status, query ID, per-engine status |

## YAML+Markdown output (format=yaml)

Designed for AI agent consumption where structured data + readable prose is more useful than raw JSON:

```yaml
query: python async web scraping
meta:
  response_time_ms: 1420
  cached: false
  query_id: ssx-abc12345
  engine_count: 4
  responsive: 3
results:
  - url: https://example.com/guide
    title: Async Web Scraping with Python
    engine: brave
    engines: [brave]
    content: |
      A comprehensive guide to building async web scrapers...
    score: 0.92
    position: 1
    published: 2025-11-15T00:00:00Z
    tier: 1
---
## Results Summary

**3 engines responded** of 4 active. 10 results returned in 1.4s.

For **python async web scraping**, the top results cover:
- Building async web scrapers with aiohttp and asyncio
- Rate limiting and concurrent page processing
- Best practices for production scraping pipelines

> DuckDuckGo (CAPTCHA wall detected). Results from Brave API and Wikipedia.
```

The response is a single string with two sections separated by `---`:
1. **YAML header** — structured results with all SearchResult fields
2. **Markdown body** — human-readable prose summary with top result snippets and engine status

## How they map

Both formatters consume the same ranked `SearchResult` list from the `PresenceRanker`. The formatter is selected based on the `format` query parameter:

- `format=json` → `format_json()`
- `format=yaml` → `format_yaml_markdown()`

The `Accept` header is also respected (`text/vnd.yaml+markdown`).

## Integration points

- **Server search handler:** Formatter called after ranking, before caching
- **Response:** Formatter output returned directly as `JSONResponse` or `PlainTextResponse`

## Entry points

- Add a field to JSON: modify `_result_to_searxng()`
- Add a field to YAML: modify the `results` list comp in `format_yaml_markdown()`
- Change Markdown summary: modify the Markdown body builder in `format_yaml_markdown()`

## Key source files

| File | Description |
|---|---|
| `slopsearx/formatter.py` | Both formatters and SearXNG field mapping |
