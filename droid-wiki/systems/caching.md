# Caching

Active contributors: Magnus Hedemark

## Purpose

Valkey-backed response cache that stores merged result sets at two levels: a precise key (query + language + safesearch) and a broader answer-level key (query only). Category-aware TTL (300 seconds for news, 3600 seconds for general). Supports negative caching to short-circuit failed queries. Graceful degradation when Valkey is unavailable.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `SearchCache` | `slopsearx/cache.py` | Valkey-backed cache client. Provides `get()`, `set()`, `get_answer()`, `set_answer()`, `set_error()`, `clear()` methods. Gracefully degrades to no-op if Valkey is unreachable. |
| `normalize_query` | `slopsearx/cache.py` | Normalizes a raw query string for deterministic cache key construction: URL-decodes, strips punctuation and whitespace, collapses whitespace, lowercases. |
| `cache_key` | `slopsearx/cache.py` | Builds a deterministic SHA-256 cache key from the normalized query tuple (query string, language, safesearch level). Prefix: `search:`. |
| `_answer_cache_key` | `slopsearx/cache.py` | Builds a broader cache key from query only (no language or safesearch). Prefix: `answer:`. |
| `_ttl_for_query` | `slopsearx/cache.py` | Selects TTL based on query categories. News queries get 300 seconds; all other queries get 3600 seconds. |

## How it works

### Query normalization

Before building a cache key, raw queries are normalized via `normalize_query()`:

1. URL-decode the query (`unquote_plus`)
2. Strip leading and trailing whitespace
3. Strip trailing punctuation (`. ! ? , ; :`)
4. Collapse multiple whitespace characters into one
5. Lowercase and strip again

This ensures that `"Python 3.12"`, `"python 3.12."`, and `"Python%203.12"` all produce the same cache key.

### Two-level cache

SlopSearX uses two cache levels:

- **Search cache** (`search:{sha256}`) — keyed on `(normalized_query, language, safesearch)`. Precise matching: different languages or safesearch levels get different cache entries.
- **Answer cache** (`answer:{sha256}`) — keyed on `normalized_query` only. Broader matching: the same cached response is returned regardless of language or safesearch variation.

At request time, the server checks both caches in order: search cache first, then answer cache. On a fresh result, both caches are populated.

### Cache flow

1. On each search request, the server computes the search cache key and calls `_cache.get(key)`
2. If a cached result exists at the search level, it is returned immediately with `meta.cached = True`
3. If not, the server checks the answer cache via `_cache.get_answer(q)`
4. If a cached result exists at the answer level, it is returned immediately
5. **Negative cache check:** If either cached entry has an `_error` sentinel, the server returns HTTP 503 without dispatching to engines. This prevents thundering-herd retries on failing queries.
6. If no cached result exists, the search executes normally
7. The merged result set is stored in both the search cache and answer cache via `_cache.set()` and `_cache.set_answer()`
8. Cache write is skipped for completely failed searches (all engines unresponsive)

### Negative caching

When a search fails (all engines unresponsive or other systemic failure), the server stores a negative cache entry via `_cache.set_error(key)`:

```python
payload = json.dumps({"_error": True, "timestamp": int(time.time())})
```

Negative entries have a short TTL (default 60 seconds, configurable via `SEARCH_CACHE_NEGATIVE_TTL_SECONDS`). When the server encounters a negative entry on a subsequent request, it returns HTTP 503 immediately rather than dispatching again to already-failing engines.

### TTL strategy

TTL is determined by `_ttl_for_query()`. The function checks if any requested category contains the string `"news"`:

- News queries: 300 seconds (fast turnover for time-sensitive content)
- All other queries: 3600 seconds (1 hour default, configurable via `SEARCH_CACHE_TTL_SECONDS`)
- Answer cache TTL: same as search cache TTL, configurable via `SEARCH_CACHE_TTL_SECONDS`

### Graceful degradation

If Valkey is unreachable at startup, `SearchCache` logs a warning and sets `_connected = False`. All subsequent cache operations become no-ops:

- `get()` returns `None` (cache miss)
- `set()` returns without storing
- The server proceeds with a normal search

This ensures that a Valkey outage does not break search availability. The only impact is increased latency from cache misses and no negative-cache protection.

### What is NOT cached

- Individual engine responses are not cached independently. Only merged result sets are cached.
- API keys, secrets, and config are not cached (read directly from env or file).
- Rate-limit state is ephemeral with short TTLs in a separate Valkey keyspace.

## Integration points

- **Server lifecycle:** `SearchCache` is instantiated in the `startup()` event handler and stored in the module-level `_cache` global
- **Search flow:** The server checks both cache levels before dispatching to engines and stores results after merging
- **Answer cache:** `get_answer()` and `set_answer()` are used alongside the primary search cache
- **Negative cache:** `set_error()` is called by the server when a systemic failure occurs
- **Category awareness:** The server passes the requested categories to `_ttl_for_query()` after determining them from search params

## Entry points for modification

- Changing key format: modify `cache_key()` or `_answer_cache_key()` functions
- Adjusting normalization: modify `normalize_query()` function
- Adjusting TTL rules: modify `_ttl_for_query()` function or env defaults
- Changing negative cache behavior: modify `set_error()` or `SEARCH_CACHE_NEGATIVE_TTL_SECONDS`
- Adding cache invalidation: implement `clear()` with more selective patterns

## Key source files

| File | Description |
|---|---|
| `slopsearx/cache.py` | `SearchCache` class, `cache_key()`, `normalize_query()`, `_ttl_for_query()`, `_answer_cache_key()` |
