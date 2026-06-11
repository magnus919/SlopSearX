# Output formatters

Active contributors: Magnus Hedemark

## Purpose

Maps the internal `SearchResult` dataclass to two output formats: SearXNG-compatible JSON and agent-native YAML+Markdown. The formatter module lives in `slopsearx/formatter.py` and is the last step in the search pipeline, after results have been collected, deduplicated, and ranked.

## Key abstractions

- **`format_json()`** (`slopsearx/formatter.py`) — produces a SearXNG-compatible JSON response dict. Accepts the ranked result list plus optional metadata (answers, corrections, infoboxes, suggestions, unresponsive engines, meta extensions).
- **`format_yaml_markdown()`** (`slopsearx/formatter.py`) — produces a YAML document with structured data, followed by `---`, then a Markdown `## Results Summary` section with readable prose. Designed for AI agent consumption.
- **`_result_to_searxng()`** (`slopsearx/formatter.py`) — maps a single `SearchResult` to a 23-field SearXNG dict. All 23 SearXNG MainResult fields are present; null fields are preserved as `None` for JSON serialization.
- **`_iso_to_epoch()`** (`slopsearx/formatter.py`) — converts an ISO 8601 datetime string to Unix epoch seconds, or returns `None` if the input is empty or unparseable.

## How it works

### JSON formatter

The JSON formatter (`format_json()`) produces a response that is a superset of SearXNG's output. The same fields are present, plus `meta.*` extensions at the top level.

The `AdapterResponse` now includes three extended fields that are aggregated from all engine responses: `answers` (list of dicts with answer content), `corrections` (list of suggested query correction strings), and `infoboxes` (list of structured info box dicts). These are passed through to the JSON response and populated when adapters return them.

Response structure:

```json
{
  "query": "search query",
  "results": [...],
  "number_of_results": 10,
  "answers": [],
  "corrections": [],
  "infoboxes": [],
  "suggestions": [],
  "unresponsive_engines": [],
  "meta": {
    "response_time_ms": 1234,
    "cached": false,
    "query_id": "abc123",
    "engine_status": {
      "brave": {"status": "ok", "latency_ms": 320},
      "duckduckgo": {"status": "blocked", "latency_ms": 0}
    }
  }
}
```

Each result in the `results` array includes all 23 SearXNG `MainResult` fields:

```
url, title, content, engine, engines, score, positions, category,
publishedDate, pubdate, length, thumbnail, img_src, iframe_src,
audio_src, views, author, metadata, template, parsed_url,
open_group, close_group, priority
```

### YAML+Markdown formatter

The YAML+Markdown formatter (`format_yaml_markdown()`) produces a two-part response:

1. **YAML header** — structured data with query, meta (response time, cached, query_id, engine health), and an array of results with url, title, engine, content, score, position, and publication date.
2. **Markdown body** — a `## Results Summary` section with a human-readable summary: number of engines that responded, total results, elapsed time, and the top 5 result snippets.

The response is selected by the `format` query parameter (`format=json` is default, `format=yaml` triggers the YAML+Markdown formatter).

### Legacy stub

A backward-compatible `format_yaml()` wrapper converts legacy dict-based results to `SearchResult` objects and delegates to `format_yaml_markdown()`.

## Key source files

- `slopsearx/formatter.py` — all formatter logic

## See also

- [Search result types](../primitives/search-result.md) — the `SearchResult` dataclass that formatters consume
- [System architecture](../overview/architecture.md) — where formatting fits in the request flow
