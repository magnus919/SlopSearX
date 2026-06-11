# Systems overview

The systems lens views SlopSearX as a set of interacting subsystems, each with a well-defined boundary and contract. Each subsystem lives in exactly one file in the `slopsearx/` package. This section documents each subsystem's purpose, key abstractions, how it works, and integration points.

## Subsystems

| Page | Subsystem | File | Purpose |
|---|---|---|---|
| [Adapter interface](adapter-interface.md) | Engine adapter interface | `slopsearx/adapter.py` | Abstract base classes, data types, and the `@register_engine` registry that defines the primary architectural invariant of SlopSearX |
| [Merging and ranking](merging-and-ranking.md) | Result merger | `slopsearx/merger.py` | URL normalization, deduplication, presence-weighted ranking, and metadata helpers |
| [Configuration](configuration.md) | Configuration | `slopsearx/config.py` | Three-layer configuration model: built-in defaults, YAML file, environment variable overrides |
| [Caching](caching.md) | Response cache | `slopsearx/cache.py` | Valkey-backed response cache with category-aware TTL and graceful degradation |
| [Rate limiting](rate-limiting.md) | Rate limiter | `slopsearx/ratelimit.py` | Distributed rate limiting with pluggable strategies and backpressure propagation |
| [Observability](observability.md) | Metrics | `slopsearx/metrics.py` | OpenMetrics-compatible per-engine counters, latency histograms, and status gauges |
| [API server](api-server.md) | HTTP server | `slopsearx/server.py` | FastAPI HTTP server implementing the SearXNG-compatible search API |
