# Caching

Active contributors: Magnus Hedemark

## Purpose

Valkey-backed response cache that avoids redundant engine dispatch for repeated queries. Gracefully degrades: Valkey unavailable → cache is a no-op.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `SearchCache` | `slopsearx/cache.py` | Valkey-backed cache for merged search results. Two-level: search + answer |
| `cache_key()` | `slopsearx/cache.py` | SHA-256 key from `(normalized_query, language, safesearch)` |
| `_answer_cache_key()` | `slopsearx/cache.py` | SHA-256 key from normalized query only (broader) |
| `normalize_query()` | `slopsearx/cache.py` | Query normalization: URL-decode, strip, collapse whitespace, lowercase |
| `_ttl_for_query()` | `slopsearx/cache.py` | Category-aware TTL: 300s for news, 3600s for general |

## Cache architecture

### Two levels

1. **Search cache** (precise key) — keyed on `(query, language, safesearch)`. Same query with different language/safesearch gets different cached response
2. **Answer cache** (broad key) — keyed on `query` only. Returns the same response for any language/safesearch variant of the same query

### Lookup order

1. Check search cache → HIT: return immediately
2. Check answer cache → HIT: return immediately
3. Both MISS → dispatch engines
4. After dispatch → store in **both** caches (answer cache skipped for time-range queries)

### Negative caching

On dispatch failure (all engines unresponsive), a negative cache entry is stored:

- Signal: `{"_error": true, "timestamp": ...}`
- TTL: 60 seconds (configurable via `SEARCH_CACHE_NEGATIVE_TTL_SECONDS`)
- Effect: subsequent requests for the same key return HTTP 503 without dispatching

### Cache eviction

Valkey `allkeys-lru` handles eviction. No explicit eviction logic in SlopSearX.

## Integration points

- **Server startup:** `SearchCache.connect()` establishes Valkey connection
- **Server search handler:** `cache.get(key)` → `cache.get_answer(query)` → `cache.set(key, value, ttl)` → `cache.set_answer(query, value)`
- **Server shutdown:** `cache.close()` closes the connection
- **SuggestionService:** `cache.get(suggest_key)` + `cache.set(suggest_key, suggestions, ttl)`
- **EngineStatsTracker:** uses `cache._client` for Valkey pipeline operations
- **QueryAuditLogger:** uses `cache._client` for Valkey stream operations

## Entry points

- Change TTL: modify `_ttl_for_query()` or set `SEARCH_CACHE_TTL_SECONDS`
- Add cache level: add new key function + get/set pair in `SearchCache`
- Clear cache: `cache.clear()` flushes all keys

## Key source files

| File | Description |
|---|---|
| `slopsearx/cache.py` | SearchCache class, key functions, TTL logic |
