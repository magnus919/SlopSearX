# Caching

Active contributors: Magnus Hedemark

## Purpose

Valkey-backed response cache that stores merged result sets. Category-aware TTL (60 seconds for news, 300 seconds for general). Graceful degradation when Valkey is unavailable.

## Key abstractions

| Type | File | Description |
|---|---|---|
| `SearchCache` | `slopsearx/cache.py` | Valkey-backed cache client. Provides `get()`, `set()`, `clear()` methods. Gracefully degrades to no-op if Valkey is unreachable. |
| `cache_key` | `slopsearx/cache.py` | Builds a deterministic SHA-256 cache key from the normalized query tuple (query string, language, safesearch level). |
| `_ttl_for_query` | `slopsearx/cache.py` | Selects TTL based on query categories. News queries get 60 seconds; all other queries get 300 seconds. |

## How it works

### Cache key format

The cache key has the format `search:{sha256_hexdigest}`. The digest is computed from a normalized tuple:

```python
norm = f"{query.lower().strip()}|{language}|{safesearch}"
digest = hashlib.sha256(norm.encode()).hexdigest()
key = f"search:{digest}"
```

This ensures that queries differing only in case or whitespace produce the same cache key. Different languages and safesearch levels produce different keys.

### Cache flow

1. On each search request, the server computes the cache key and calls `_cache.get(key)`
2. If a cached result exists, it is returned immediately with `meta.cached = True`
3. If no cached result exists, the search executes normally, and the merged result set is stored via `_cache.set(key, response_data, ttl)`
4. Cache is skipped for completely failed searches (all engines unresponsive)

### TTL strategy

TTL is determined by `_ttl_for_query()`. The function checks if any requested category contains the string `"news"`:

- News queries: 60 seconds (fast turnover for time-sensitive content)
- All other queries: 300 seconds (5 minutes default)

The TTL is passed to Valkey's `SETEX` command, which handles both storage and expiration atomically.

### Cache eviction

Valkey uses the `allkeys-lru` eviction policy. When memory fills, Valkey evicts the least recently used keys. This is appropriate for a performance cache where data loss is acceptable.

### Graceful degradation

If Valkey is unreachable at startup, `SearchCache` logs a warning and sets `_connected = False`. All subsequent cache operations become no-ops:

- `get()` returns `None` (cache miss)
- `set()` returns without storing
- The server proceeds with a normal search

This ensures that a Valkey outage does not break search availability. The only impact is increased latency from cache misses.

### Sliding window

TTL is refreshed on cache hits via Valkey's `SETEX` (which resets the TTL on each write). This implements a sliding-window expiration pattern: frequently accessed queries stay cached, while stale queries expire naturally.

### What is NOT cached

- Individual engine responses are not cached independently. Only merged result sets are cached.
- API keys, secrets, and config are not cached (read directly from env or file).
- Rate-limit state is ephemeral with short TTLs in a separate Valkey keyspace.

## Integration points

- **Server lifecycle:** `SearchCache` is instantiated in the `startup()` event handler and stored in the module-level `_cache` global
- **Search flow:** The server checks the cache before dispatching to engines and stores results after merging
- **Category awareness:** The server passes the requested categories to `_ttl_for_query()` after determining them from search params

## Entry points for modification

- Changing key format: modify `cache_key()` function
- Adjusting TTL rules: modify `_ttl_for_query()` function
- Changing eviction policy: configure Valkey cluster parameters (not in code)
- Adding cache invalidation: implement `clear()` with more selective patterns

## Key source files

| File | Description |
|---|---|
| `slopsearx/cache.py` | `SearchCache` class, `cache_key()` function, `_ttl_for_query()` function |
