# System architecture

SlopSearX runs as a single replica type behind a load balancer. Every replica loads all configured engines and communicates with Valkey for shared state. All replicas are interchangeable.

```mermaid
graph TD
    LB["Load Balancer<br/>(Traefik / Nginx)"]
    R1["Replica 1<br/>(all engines)"]
    R2["Replica 2<br/>(all engines)"]
    RN["Replica N<br/>(all engines)"]
    V["Valkey<br/>(cluster)"]

    LB --> R1
    LB --> R2
    LB --> RN
    R1 --> V
    R2 --> V
    RN --> V
```

## Request flow

When a client sends `GET /search?q=...`, the following happens inside one replica:

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server (FastAPI)
    participant RL as Client RL
    participant CACHE as Valkey Cache
    participant QR as QueryRouter
    participant CB as Circuit Breaker
    participant SM as Semaphore
    participant E1 as Engine 1 (Brave)
    participant E2 as Engine 2 (Wikipedia)
    participant STATS as Stats Tracker
    participant AUDIT as Audit Logger
    participant R as Ranker
    participant F as Formatter

    C->>S: GET /search?q=python
    S->>RL: Check per-client rate limit
    alt Rate limited
        RL-->>S: Deny
        S-->>C: 429
    end
    S->>CACHE: Check search cache
    alt Cache HIT
        CACHE-->>S: Cached response
        S-->>C: 200 (cached)
    else Search cache MISS
        S->>CACHE: Check answer cache
        alt Answer cache HIT
            CACHE-->>S: Cached response
            S-->>C: 200 (cached)
        else Cache MISS
            S->>QR: route(query, categories)
            QR-->>S: matched engines list
            par Engine dispatch
                S->>CB: circuit_allowed() per engine
                CB-->>S: allow / deny
                S->>SM: acquire semaphore slot
                S->>E1: BraveAdapter.search(q)
                S->>E2: WikipediaAdapter.search(q)
                E1-->>S: AdapterResponse (10 results)
                E2-->>S: AdapterResponse (3 results)
                S->>CB: record_success / record_failure
            end
            S->>STATS: record(engine, status, latency)
            S->>R: rank(engine_results)
            R-->>S: ranked SearchResult list
            S->>F: format(results)
            S->>CACHE: store in cache
            S->>AUDIT: record query (fire-and-forget)
            S-->>C: 200 (fresh)
        end
    end
```

### Step by step

1. **Query normalization** — the raw query string is normalized into a tuple of `(query, language, pageno, categories, safesearch)`
2. **Per-client rate limiting** — the client IP is checked against its request budget. Exceeded budgets return HTTP 429 before any engine work
3. **Search cache lookup** — Valkey is checked with a SHA-256 key derived from the normalized tuple. A cache hit returns immediately
4. **Answer cache lookup** — if the search cache misses, the broader answer-level cache is checked (query only, no language/safesearch)
5. **Negative cache check** — if either cached entry has an `_error` sentinel, HTTP 503 is returned without dispatching
6. **Query routing** — the QueryRouter analyzes the query and selects relevant engines based on topic matching and category filters. Unscoped queries without topic matches are restricted to Tier 1 (broad) engines
7. **Circuit breaker check** — each target engine's circuit breaker is checked. Open circuits skip dispatch
8. **Concurrent dispatch** — allowed engines run concurrently via `asyncio.gather()`, bounded by a semaphore (`MAX_CONCURRENT_ENGINES`, default 10). Each engine has a 3-second timeout
9. **Result collection** — each adapter returns an `AdapterResponse` with results and a status. Circuit breaker state is updated (record_success/record_failure). The EngineStatsTracker records per-engine quality telemetry
10. **Merging and ranking** — the `PresenceRanker` normalizes URLs, deduplicates by normalized URL, and scores results by how many engines returned them
11. **Caching** — the merged result set is stored in both the search cache and answer cache
12. **Audit trail** — the query is recorded in a Valkey stream with dispatch statistics (fire-and-forget)
13. **Formatting** — the response is serialized as JSON (SearXNG-compatible) or YAML+Markdown

## Engine tiers

SlopSearX has two tiers of reliability:

- **Tier 1 (always-available)** — API-based engines like Brave and Wikipedia. These use structured JSON APIs, have SLAs, and are reliable
- **Tier 2 (quality multipliers)** — scrape-based engines like DuckDuckGo and Google. These send HTTP requests with stealth headers and parse HTML. They break frequently due to CAPTCHA walls, IP bans, and HTML structure changes

A CAPTCHA block or IP ban on a scrape engine never blocks the response. The failing engine is reported in `unresponsive_engines` and omitted from results. The response always returns HTTP 200 with whatever results are available.

## Core subsystems

| Subsystem | `slopsearx/` file | Purpose |
|---|---|---|
| Adapter interface | `adapter.py` | Base classes `EngineAdapter` and `ScrapeAdapter`, `@register_engine` decorator, engine registry, circuit breaker, URL sanitization |
| Merging and ranking | `merger.py` | `PresenceRanker`, URL normalization, deduplication, metadata helpers |
| Configuration | `config.py` | Three-layer config (defaults + YAML file + env vars), `Config` dataclass |
| Caching | `cache.py` | Valkey-backed two-level response cache (search + answer), negative caching, query normalization |
| Rate limiting | `ratelimit.py` | Distributed rate limiting with Valkey sliding window, fail-closed mode with local fallback, backpressure propagation |
| Metrics | `metrics.py` | OpenMetrics-compatible counters, gauges, and histograms (stdlib only) |
| Server | `server.py` | FastAPI application, `/search`, `/health`, `/metrics`, `/config` endpoints, per-client rate limiting, semaphore-bounded concurrency |
| Formatters | `formatter.py` | SearXNG JSON formatter and YAML+Markdown agent-native formatter |
| QueryRouter | `router.py` | Topic-based query routing that dispatches to relevant engines only |
| EngineStatsTracker | `stats.py` | Per-engine quality telemetry stored in Valkey (success/failure counts, latency) |
| SuggestionService | `suggest.py` | Background search query suggestions from engine suggest APIs |
| Query audit trail | `audit.py` | Durable audit log of every search query in Valkey streams |
| ProxyPool | `proxypool.py` | Configurable proxy rotation infrastructure for scrape-based adapters |

## Language breakdown

The project is 100% Python 3.12+ with supporting YAML, JSON, and Markdown configuration files.

| Language | Files | Lines |
|---|---|---|
| Python | ~100 | ~15,654 |
| YAML | 8 | ~230 |
| Markdown | 40 | ~3,600 |
| JSON | 2 | ~145 |
| Dockerfile | 1 | ~30 |

The `slopsearx/` core library is ~2,870 lines across 13 files, the `engines/` directory is ~6,670 lines across 49 files (48 adapters + `__init__.py`), and `tests/` is ~6,100 lines across 37 files.

## Key dependencies

- **FastAPI** — HTTP framework for the REST API
- **httpx** — async HTTP client for engine API calls and HTML scraping
- **lxml** — HTML parsing for scrape-based engines (DuckDuckGo, Google)
- **Valkey** — in-memory data store for caching, rate limiting, and engine stats (Redis-compatible)
- **uvicorn** — ASGI server
- **PyYAML** — YAML config parsing and YAML+Markdown output formatting
