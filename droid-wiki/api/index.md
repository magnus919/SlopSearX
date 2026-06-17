# API reference

SlopSearX implements the SearXNG API contract with extensions for agent-native consumers.

## Endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/search` | Execute a search across enabled engines |
| `GET` | `/health` | Server liveness and Valkey connectivity check |
| `GET` | `/metrics` | OpenMetrics for Prometheus scraping |
| `GET` | `/config` | Categories-to-engines mapping for runtime discovery |

## GET /search

Execute a search across all enabled engines.

### Query parameters

| Parameter | Default | Description |
|---|---|---|
| `q` | (required) | Search query string |
| `format` | `json` | Response format: `json` or `yaml` |
| `categories` | (none) | Comma-separated category filter |
| `engines` | (all active) | Comma-separated engine filter |
| `language` | `en` | Language code |
| `pageno` | `1` | Page number (1-based) |
| `time_range` | (none) | Time filter: `day`, `month`, `year` |
| `safesearch` | `0` | SafeSearch: `0` (off), `1` (moderate), `2` (strict) |

### Responses

**200 OK** — Results found (even if empty):

```json
{
  "query": "python",
  "results": [...],
  "engines": [{"engine": "brave", "results": 10}],
  "number_of_results": 10,
  "answers": [],
  "corrections": [],
  "infoboxes": [],
  "suggestions": ["python tutorial", "python documentation"],
  "unresponsive_engines": [["duckduckgo", "CAPTCHA wall detected"]],
  "meta": {
    "response_time_ms": 1420,
    "cached": false,
    "query_id": "ssx-abc12345",
    "engine_status": {...}
  }
}
```

**400 Bad Request** — Missing or empty `q`:

```json
{"error": "query_required", "message": "The 'q' parameter is required."}
```

**429 Too Many Requests** — Per-client rate limit exceeded:

```json
{"error": "rate_limited", "message": "Too many requests. Please slow down."}
```

**503 Service Unavailable** — All engines unresponsive:

```json
{
  "error": "all_engines_unavailable",
  "meta": {...},
  "results": []
}
```

## GET /health

```json
{
  "status": "ok",
  "version": "0.1.0",
  "valkey_connected": true,
  "engines": {
    "brave": {"status": "ok"},
    "wikipedia": {"status": "ok"}
  }
}
```

Status values: `"ok"` or `"degraded"` (when Valkey is unreachable with fail-closed enabled).

## GET /metrics

Returns OpenMetrics text format for Prometheus scraping. Content-Type: `text/plain; version=0.0.4`.

## GET /config

```json
{
  "categories": {
    "general": ["brave", "duckduckgo", "google", "wikipedia"],
    "science": ["arxiv", "brave", "huggingface"],
    "security": ["shodan", "censys", "virustotal"]
  }
}
```
