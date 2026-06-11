# Suggestion service

Active contributors: Magnus Hedemark

## Purpose

Fetches search query suggestions from engine suggest APIs and caches them in Valkey. Uses Brave Suggest API as primary provider and DuckDuckGo suggest API as fallback. Designed for agent-native search UIs where type-ahead suggestions improve the interaction experience.

Opt-in by default: disabled unless explicitly enabled in config and a Brave API key is available.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `SuggestionService` | `slopsearx/suggest.py` | Suggestion fetcher with caching. Exposes `fetch()` method that checks cache, tries Brave, falls back to DDG. |
| `fetch()` | `slopsearx/suggest.py` | Async method: checks Valkey cache first, then tries `_fetch_brave()`, then `_fetch_ddg()`. Results (even empty) are cached for 30 minutes. |
| `_fetch_brave()` | `slopsearx/suggest.py` | Calls `https://api.search.brave.com/res/v1/web/suggest` with `X-Subscription-Token` header. Requires `ENGINE_BRAVE_API_KEY` to be set. |
| `_fetch_ddg()` | `slopsearx/suggest.py` | Calls `https://ac.duckduckgo.com/ac/` with no auth. Fallback when Brave is unavailable or unconfigured. |
| `_suggest_cache_key()` | `slopsearx/suggest.py` | Builds a deterministic Valkey cache key: `suggest:{sha256_hash}` of the lowercased, stripped query. |
| `_SUGGEST_CACHE_TTL` | `slopsearx/suggest.py` | 1800 seconds (30 minutes). Suggestions change slowly, so a long TTL reduces API calls. |

## How it works

### Fetch flow

`fetch(query)` executes these steps:

1. **Empty query guard.** If the query is empty or whitespace-only, returns an empty list immediately.
2. **Cache check.** If Valkey is connected, computes `suggest:{sha256(query)}` and looks up the cached result. On cache hit, returns the stored suggestion list.
3. **Brave Suggest API (primary).** Calls `_fetch_brave(query)` with the query and `count=5`. Requires `ENGINE_BRAVE_API_KEY` to be set. On success, parses the JSON response — Brave returns `{"results": [{"q": "..."}, ...]}` or a flat list of strings depending on API version. On failure (non-200, exception, or empty response), proceeds to the fallback.
4. **DuckDuckGo suggest API (fallback).** Calls `_fetch_ddg(query)` with no authentication. DDG returns `["query", ["sug1", "sug2", ...]]`. Parses the second element as the suggestion list. On failure, returns an empty list.
5. **Cache write.** If Valkey is connected, the result (even if empty) is written to the cache with a 30-minute TTL. Caching empty results avoids repeated failed lookups for queries that yield no suggestions.
6. **Return.** The suggestion list (possibly empty) is returned to the caller.

### API details

**Brave Suggest API:**
- URL: `https://api.search.brave.com/res/v1/web/suggest`
- Auth: `X-Subscription-Token` header with Brave API key
- Response: `{"results": [{"q": "python tutorial"}, {"q": "python for beginners"}, ...]}`
- Timeout: 3 seconds

**DuckDuckGo suggest API:**
- URL: `https://ac.duckduckgo.com/ac/`
- Auth: None
- Response: `["python", ["python tutorial", "python for beginners", ...]]`
- Timeout: 3 seconds

### Cache key format

```
suggest:{sha256_hex_digest}
```

The cache value is a dict with a single key `suggestions` containing the list of suggestion strings.

### Opt-in design

Suggestion service is disabled by default (`enable_suggestions: false`). Enabling it requires:

1. `enable_suggestions: true` in `config.yaml` (or `SEARCH_ENABLE_SUGGESTIONS=true` env var)
2. `ENGINE_BRAVE_API_KEY` set to a valid Brave Search API key

Without the Brave API key, the service falls back to DDG only (which may have reliability and rate-limit issues).

## Integration points

- **Server startup:** `startup()` in `server.py` checks `cfg.enable_suggestions`. If true and a Brave API key is available, creates a `SuggestionService` instance. Otherwise, `_suggestion_service` remains `None`
- **Suggest endpoint:** The server exposes a suggest handler that returns `[]` immediately if `_suggestion_service is None`, or delegates to `_suggestion_service.fetch(query)` otherwise
- **Config system:** `enable_suggestions` is a boolean field on the top-level `Config` dataclass, loaded from YAML or env var (`SEARCH_ENABLE_SUGGESTIONS`)
- **Cache dependency:** Suggestions are cached in Valkey via the shared `SearchCache` instance. Gracefully degrades when Valkey is unavailable

## Entry points for modification

- Adding a new suggest provider: add a `_fetch_<provider>()` method and insert it in the fetch chain in `fetch()`
- Changing cache TTL: modify `_SUGGEST_CACHE_TTL` in `suggest.py`
- Changing suggestion count: update the `count` parameter in the Brave API call
- Changing cache key scheme: modify `_suggest_cache_key()` function
- Enabling by default: change `enable_suggestions` default from `False` to `True` in `config.py`

## Key source files

| File | Description |
|---|---|
| `slopsearx/suggest.py` | SuggestionService class, Brave and DDG fetch methods, cache key and TTL |
| `slopsearx/config.py` | `enable_suggestions` config field, YAML and env var loading |
| `slopsearx/server.py` | Suggestion service initialization at startup, suggest API handler |
