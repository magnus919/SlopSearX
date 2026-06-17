# Systems overview

The systems lens views SlopSearX as a set of interacting subsystems, each with a well-defined boundary and contract. Each subsystem lives in exactly one file in the `slopsearx/` package. This section documents each subsystem's purpose, key abstractions, how it works, and integration points.

## Subsystems

| Page | Subsystem | File | Purpose |
|---|---|---|---|
| [Adapter interface](adapter-interface.md) | Engine adapter interface | `slopsearx/adapter.py` | Abstract base classes, data types, circuit breaker, and the `@register_engine` registry that defines the primary architectural invariant of SlopSearX |
| [Audit trail](audit-trail.md) | Query audit trail | `slopsearx/audit.py` | Durable audit log of every search query in Valkey streams for operational analysis |
| [Merging and ranking](merging-and-ranking.md) | Result merger | `slopsearx/merger.py` | URL normalization, deduplication, presence-weighted ranking, and metadata helpers |
| [Configuration](configuration.md) | Configuration | `slopsearx/config.py` | Three-layer configuration model: built-in defaults, YAML file, environment variable overrides |
| [Caching](caching.md) | Response cache | `slopsearx/cache.py` | Two-level Valkey-backed response cache with negative caching and graceful degradation |
| [Rate limiting](rate-limiting.md) | Rate limiter | `slopsearx/ratelimit.py` | Multi-layer distributed rate limiting with fail-closed mode, per-client limits, and backpressure propagation |
| [Observability](observability.md) | Metrics & telemetry | `slopsearx/metrics.py` | OpenMetrics instrumentation and per-engine quality telemetry in Valkey |
| [API server](api-server.md) | HTTP server | `slopsearx/server.py` | FastAPI HTTP server with circuit breaker, semaphore-bounded concurrency, and the SearXNG-compatible search API |
