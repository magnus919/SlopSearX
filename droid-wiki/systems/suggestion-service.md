# Suggestion service

Active contributors: Magnus Hedemark

## Purpose

Fetches search query suggestions from engine suggest APIs. Runs as a background task concurrently with engine dispatch. Results cached in Valkey for 30 minutes. Gracefully degrades to empty list.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `SuggestionService` | `slopsearx/suggest.py` | Fetches suggestions from Brave Suggest (primary) + DDG (fallback) |
| `fetch()` | `slopsearx/suggest.py` | Public API: check cache, try Brave, fall back to DDG, cache result |
| `_fetch_brave()` | `slopsearx/suggest.py` | Brave Suggest API call (`/res/v1/web/suggest`) |
| `_fetch_ddg()` | `slopsearx/suggest.py` | DDG suggest API call (`ac.duckduckgo.com/ac/`) |
| `_suggest_cache_key()` | `slopsearx/suggest.py` | SHA-256 key for suggestion cache |

## Provider chain

1. **Brave Suggest API** (`api.search.brave.com/res/v1/web/suggest`) — primary. Requires `ENGINE_BRAVE_API_KEY`. Returns structured suggestion list. Max 5 suggestions fetched
2. **DuckDuckGo Suggest API** (`ac.duckduckgo.com/ac/`) — fallback. No auth required. Returns `[query_string, [suggestion_list]]`

## Caching

- Key: `suggest:{sha256(lowered_trimmed_query)}`
- TTL: 30 minutes (1,800 seconds)
- Empty results also cached to avoid repeated failed lookups
- Stored via `SearchCache.set()` (shared Valkey client)

## How it works

```python
suggestions = await suggestion_service.fetch(query)
```

1. Check Valkey cache for `suggest:{sha256}`
2. Cache hit → return cached suggestions
3. Try Brave Suggest (if API key configured)
4. Brave returns results → cache and return
5. Brave fails / no results → try DDG Suggest
6. DDG returns results → cache and return
7. DDG fails → cache empty list and return `[]`

The service is opt-in: `enable_suggestions: true` in config.yaml or Brave API key required. Disabled by default to avoid extra API costs.

## Integration points

- **Server startup:** `SuggestionService` initialized if `enable_suggestions=true` and Brave API key available
- **Server search handler:** `_generate_suggestions(q)` background task runs concurrently with engine dispatch
- **Response:** Suggestions appended to both JSON and YAML+Markdown responses

## Entry points

- Add provider: implement new `_fetch_*()` method, add to `fetch()` chain
- Change TTL: modify `_SUGGEST_CACHE_TTL`
- Change count: modify `count` param in `_fetch_brave()`

## Key source files

| File | Description |
|---|---|
| `slopsearx/suggest.py` | SuggestionService class and provider implementations |
