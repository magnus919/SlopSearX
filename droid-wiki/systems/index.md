# Systems overview

The systems lens views SlopSearX as a set of interacting subsystems, each with a well-defined boundary and contract. Each subsystem lives in exactly one file in the `slopsearx/` package.

## Subsystems

| Page | Subsystem | File | Purpose |
|---|---|---|---|
| [Adapter interface](adapter-interface.md) | Engine adapter interface | `slopsearx/adapter.py` | Abstract base classes, data types, circuit breaker, and the `@register_engine` registry |
| [API server](api-server.md) | HTTP server | `slopsearx/server.py` | FastAPI application with circuit breaker, semaphore-bounded concurrency, two-tier engine selection |
| [Configuration](configuration.md) | Configuration | `slopsearx/config.py` | Three-layer config model (defaults → YAML → env vars), feature flags |
| [Query router](query-router.md) | Query routing | `slopsearx/router.py` | Topic-based query routing for relevant engine dispatch |
| [Merging and ranking](merging-and-ranking.md) | Result merger | `slopsearx/merger.py` | URL normalization, deduplication, presence-weighted ranking |
| [Caching](caching.md) | Response cache | `slopsearx/cache.py` | Two-level Valkey-backed cache with negative caching |
| [Rate limiting](rate-limiting.md) | Rate limiter | `slopsearx/ratelimit.py` | Multi-layer distributed rate limiting with fail-closed fallback |
| [Proxy pool](proxy-pool.md) | Proxy rotation | `slopsearx/proxypool.py` | Proxy rotation with failure tracking and escalating cooloff |
| [Engine stats](engine-stats.md) | Quality telemetry | `slopsearx/stats.py` | Daily per-engine quality metrics stored in Valkey |
| [Suggestion service](suggestion-service.md) | Query suggestions | `slopsearx/suggest.py` | Background suggestion fetch from Brave Suggest + DDG fallback |
| [Observability](observability.md) | Metrics, logging, tracing | `slopsearx/metrics.py`, `logging.py`, `middleware.py` | OpenMetrics instrumentation, structlog JSON logging, X-Request-ID tracing |
| [Audit trail](audit-trail.md) | Query audit | `slopsearx/audit.py` | Durable query audit stream in Valkey |
