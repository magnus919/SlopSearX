# Glossary

## Core concepts

**Adapter** — A single engine integration. One Python file in `engines/`, decorated with `@register_engine`. Must subclass `EngineAdapter` and implement `search()`.

**AdapterResponse** — Canonical return type from every adapter's `search()` method. Contains results list, status enum, error message, and latency.

**Agent-native** — Output format designed for AI agent consumption. YAML+Markdown provides structured data with readable prose.

**Answer cache** — Broader cache level keyed on query only (no language/safesearch). Returns the same response for any variant of the same query string.

**Backpressure** — Mechanism that temporarily stops dispatching to rate-limited or failing engines. Includes 30s cooldown after rate-limit denial and 3-strike deactivation.

**Category** — SearXNG-compatible tag that determines which `?categories=` queries include an engine. Supports namespace prefixes (`github:code`, `huggingface:datasets`).

**Circuit breaker** — Per-engine protection that opens after N consecutive errors (default 5) and stays open for T seconds (default 300). Half-open probes allow automatic recovery.

**Engine type** — Classification of adapter implementation: `"api"` (structured JSON API), `"scrape"` (HTTP + HTML parsing), `"structured"` (e.g., Wikipedia).

**EngineStatus** — Enum classifying adapter outcomes: `OK`, `RATE_LIMITED`, `BLOCKED`, `ERROR`, `TIMEOUT`.

**Fail-closed** — When `FAIL_CLOSED=true`, Valkey unavailability causes rate-limit checks to deny requests during a grace period, then fall back to an in-process token bucket. Default is fail-open (allow).

**Feature flags** — Safe-by-default boolean toggles that gate new behavior. Set in `config.yaml` under `features:` or via `FEATURE_<NAME>` env vars.

**Graceful degradation** — Design principle: a failing sub-component (scrape engine, cache, rate limiter) never blocks the overall response. The system returns HTTP 200 with whatever results are available.

**OpenMetrics** — Prometheus-compatible metrics format exposed at `/metrics`. stdlib-only implementation, no prometheus-client dependency.

**PresenceRanker** — V1 ranking strategy: wider-is-better. Results appearing in multiple engine feeds score higher. Documented quality ceiling.

**QueryRouter** — Topic-based query classifier. First-match-wins keyword matching against configurable topic signatures. No ML, no remote API.

**Registry** — `_ENGINE_REGISTRY` dict populated by `@register_engine` decorators at import time. Maps engine name → adapter class.

**ScrapeAdapter** — Base class for HTML-scrape engines (DDG, Google). Uses HTTP GET/POST with stealth headers + HTML parsing. No headless browser. Integrates with ProxyPool for rotation.

**SearchResult** — Internal normalized result dataclass. Decoupled from SearXNG wire format. Contains URL, title, content, engine metadata, score, tier, and media references.

**Semaphore** — Asyncio primitive that caps concurrent outbound HTTP connections per search request (`MAX_CONCURRENT_ENGINES`, default 10).

**Tier 1 / Tier 2** — Two-tier engine classification. Tier 1 (broad, general-purpose) forms the primary result set in unscoped searches. Tier 2 (specialized) surfaces below Tier 1.

**Valkey** — Redis-compatible in-memory data store. The only shared state in the system. Used for caching, rate limiting, engine stats, and audit trails.

**X-Request-ID** — UUIDv4 trace ID attached to every request/response. Preserves incoming IDs for distributed tracing. Stored in `request.state.request_id`.

## Output formats

**JSON (format=json)** — Default output. SearXNG-compatible with all 23 fields preserved plus `meta.*` extensions. Designed for programmatic consumption.

**YAML+Markdown (format=yaml)** — Agent-native output. YAML document with structured results + Markdown summary section with human-readable prose.
