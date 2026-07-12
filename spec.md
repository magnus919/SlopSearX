# SlopSearX — Cloud-Native AI-Agent-First Meta Search Engine

**Also known as:** `slopsearx`, `ssx`
**License:** Apache 2.0
**Target:** Replace SearXNG in the GroktoCrawl stack with a horizontally scalable, stateless, agent-first meta search engine.
**Status:** Architectural spec v1 (derived from multi-agent council debate, June 8 2026)

> The name isn't subtle. It's not supposed to be. This is a search engine built *for* AI agents, by someone who builds AI agents. JSON is the default. YAML+Markdown is a first-class citizen. The legacy SearXNG contract is supported for backward compatibility, but the design starts from "what does an agent need from a search API?" — not "what did SearXNG output."

---

## Table of Contents

1. [Design Principles](#1-design-principles)
2. [System Architecture](#2-system-architecture)
3. [API Contract](#3-api-contract)
4. [Engine Adapter Interface](#4-engine-adapter-interface)
5. [Result Merging & Ranking](#5-result-merging--ranking)
6. [Configuration Model](#6-configuration-model)
7. [Caching Strategy](#7-caching-strategy)
8. [Rate Limiting](#8-rate-limiting)
9. [Observability](#9-observability)
10. [Deployment Topology](#10-deployment-topology)
11. [Out-of-the-Box Engines](#11-out-of-the-box-engines)
12. [Migration Path from SearXNG](#12-migration-path-from-searxng)

---

## 1. Design Principles

### 1.1 The Adapter Interface Is the Primary Architectural Invariant

Everything else — caching, routing, response formatting, deployment topology — is secondary infrastructure around it. Adding a new engine must be possible in **one file** with **zero changes** to the orchestrator. The adapter contract is the boundary across which all engine-specific complexity is localized.

### 1.2 Agent-Native by Default

JSON output is the default serialization format. The response is structured for programmatic consumption first, human readability second. YAML+Markdown is a first-class alternative output format, not an afterthought.

### 1.3 Stateless at the Application Layer

No local volumes, no persistent database, no per-replica state. Valkey is the only shared state in the system. Every replica is interchangeable — kill any one at any time with zero data loss. State that must persist (rate-limit counters, response cache, per-engine quality metrics) lives in Valkey.

### 1.4 The SearXNG Contract Is a Backward-Compatibility Layer, Not an Inheritance

The internal data model is a normalized `Result` dataclass decoupled from the SearXNG wire format. SearXNG JSON is **one output formatter among many**, not the internal schema. This prevents carrying forward SearXNG's design decisions (category routing model, template field semantics, undocumented behavior) without questioning them.

### 1.5 Graceful Degradation Is the Default Failure Mode

The system has two tiers of reliability:
- **Tier 1 (always-available):** API-based engines (Brave, Wikipedia) — reliable, structured, paid or free API tier
- **Tier 2 (quality multipliers):** Scrape-based engines (DuckDuckGo, Google) — best-effort, fragile, no SLA

A CAPTCHA block, IP ban, or HTML structure change on a scrape engine must **never** block the response. The failing engine is omitted from results and reported in `unresponsive_engines`. The response always returns HTTP 200 with whatever results are available.

### 1.6 Brave API Is the Reliability Backbone

With a paid Brave API key, 80%+ of queries can be satisfied without touching scrape engines. The architecture assumes Brave is the primary path; scrape engines add breadth but are never required for a valid response.

---

## 2. System Architecture

### 2.1 Topology: Single Replica Type

```
┌──────────────────────────────────────────────────────┐
│                    Load Balancer                       │
│                   (Traefik / Nginx)                    │
└────┬────────────┬────────────┬──────────────┬────────┘
     │            │            │              │
┌────▼────┐ ┌────▼────┐ ┌────▼────┐    ┌─────▼──────┐
│ replica │ │ replica │ │ replica │... │   replica   │
│  (all   │ │  (all   │ │  (all   │    │   (all     │
│ engines)│ │ engines)│ │ engines)│    │  engines)  │
└────┬────┘ └────┬────┘ └────┬────┘    └─────┬──────┘
     │            │            │              │
     └────────────┴────────────┴──────────────┘
                        │
                  ┌─────▼──────┐
                  │   Valkey    │
                  │  (cluster)  │
                  └────────────┘
```

Every replica loads all configured engines — API-based (Brave, Wikipedia) and scrape-based (DuckDuckGo, Google). There is no separate scrape proxy. This works because scrape-based search engines (DDG, Google) are accessed via HTTP GET/POST requests with appropriate headers and HTML parsing — the same approach SearXNG uses. No headless browser is required.

All replicas are identical, interchangeable, and scaled behind a single load balancer. Image size: ~200MB. Cold start: <2s.

**Why one type is correct:**
- SearXNG's DuckDuckGo engine uses an HTTP POST to `https://html.duckduckgo.com/html/` and parses the HTML response with lxml — no browser needed.
- SearXNG's Google engine uses HTTP GET to google.com/search with proper headers and cookie handling — no browser needed.
- No Playwright dependency means no image bloat, no cold-start penalty, no per-replica memory overhead.
- All 50 replicas share the same rate-limit exposure (one NAT egress) regardless of architecture. A separate proxy doesn't change this — only distributed rate limiting or proxy rotation does.

**Scrape adapters are just adapters with stricter timeout/retry/stealth configuration.** They run in the same process as API adapters, use the same `async def search()` interface, and return the same `AdapterResponse` type. Their complexity is in request configuration (user-agent rotation, cookie management, CAPTCHA detection in HTML response), not process isolation.

### 2.2 Request Flow

```
1. GET /search?q=...  →  HTTP handler
2. Normalize query into internal tuple (q, language, pageno, categories, safesearch)
3. Check Valkey cache keyed by normalized tuple
   └─ HIT:  return cached result (fast path, ~2ms)
   └─ MISS: continue
4. Dispatch to all active engines concurrently:
   ┌─────────────────────────────────┐
   │ BraveAdapter.search(q, params)  │  →  HTTPS request to Brave API  →  Brave Results
   │ WikipediaAdapter.search(q,...)  │  →  HTTPS request to Wikipedia   →  Wiki Results
   │ DDGAdapter.search(q, params)    │  →  scrape-proxy:8080/search     →  Scraped Results
   │ GoogleAdapter.search(q, params) │  →  scrape-proxy:8080/search     →  Scraped Results
   └─────────────────────────────────┘
5. Each adapter returns (results: Result[], engine_health: EngineStatus)
   → EngineStatus: ok, rate_limited, blocked, error, timeout
6. Merge results:
   a. URL normalization (strip tracking params, resolve redirect targets)
   b. Deduplication by normalized URL (first occurrence wins)
   c. Cross-engine ranking (see Section 5)
7. Record per-engine quality signals in Valkey (latency, result count, error type)
8. Cache merged result set in Valkey with per-engine-aware TTL
9. Format response (JSON or YAML+Markdown depending on Accept header or format param)
10. Return with cache-control headers
```

---

## 3. API Contract

### 3.1 Baseline: SearXNG JSON Compatibility

The service responds to `GET /search` and accepts all standard SearXNG query parameters:

| Parameter | Default | Description |
|---|---|---|
| `q` | required | Search query string |
| `format` | `json` | Response format: `json`, `yaml`, `markdown`, or `html` (legacy, minimized) |
| `categories` | `general` | Comma-separated category filter |
| `engines` | all active | Comma-separated engine filter |
| `language` | `en` | Language code |
| `pageno` | `1` | Page number |
| `time_range` | none | `day`, `month`, `year` |
| `safesearch` | `0` | `0`, `1`, `2` |

### 3.2 SearXNG-Compatible Response (`format=json`)

```json
{
  "query": "python async web scraping",
  "results": [
    {
      "url": "https://example.com/guide",
      "title": "Async Web Scraping with Python",
      "content": "A comprehensive guide to building async web scrapers...",
      "engine": "brave",
      "engines": ["brave"],
      "score": 0.92,
      "positions": [1],
      "category": "general",
      "publishedDate": "2025-11-15T00:00:00Z",
      "pubdate": 1763251200,
      "length": null,
      "thumbnail": null,
      "img_src": null,
      "iframe_src": null,
      "audio_src": null,
      "views": null,
      "author": null,
      "metadata": null,
      "template": "default.html",
      "parsed_url": null,
      "open_group": false,
      "close_group": false,
      "priority": ""
    }
  ],
  "answers": [],
  "corrections": [],
  "infoboxes": [],
  "suggestions": ["python aiohttp", "async web scraping tutorial", "python httpx scraping"],
  "unresponsive_engines": [
    ["duckduckgo", "CAPTCHA wall detected"],
    ["google", "HTTP 429 rate limited"]
  ]
}
```

All 23 fields of the SearXNG `MainResult` struct are preserved for backward compatibility. Fields that are not applicable to a given result are `null` or `""`. The `engine` field contains the primary engine; `engines` contains all engines that returned this URL (if multiple engines matched the same URL).

### 3.3 Extended Response (Additional Fields)

The response includes these additional fields not present in SearXNG:

| Field | Type | Description |
|---|---|---|
| `meta.response_time_ms` | int | Total server-side processing time |
| `meta.cached` | bool | Whether the response was served from cache |
| `meta.engine_status` | object | Per-engine status summary |
| `meta.query_id` | string | Traceable query identifier |

```json
{
  "query": "...",
  "results": [...],
  "meta": {
    "response_time_ms": 1420,
    "cached": false,
    "query_id": "js-abc123",
    "engine_status": {
      "brave": {"results": 10, "latency_ms": 340, "status": "ok"},
      "wikipedia": {"results": 2, "latency_ms": 89, "status": "ok"},
      "duckduckgo": {"results": 0, "latency_ms": 0, "status": "blocked"}
    }
  }
}
```

### 3.4 YAML+Markdown Output (`format=yaml`)

When `format=yaml` or `Accept: text/vnd.yaml+markdown` is specified, the response is a YAML document with embedded Markdown. Designed for AI agent consumption where structured data + readable prose is more useful than raw JSON:

```yaml
query: python async web scraping
meta:
  response_time_ms: 1420
  engines: 3 of 4
results:
  - url: https://example.com/guide
    title: Async Web Scraping with Python
    engine: brave
    content: |
      A comprehensive guide to building async web scrapers using Python's
      asyncio and aiohttp libraries. Covers connection pooling, rate limiting,
      and concurrent page processing.
    published: 2025-11-15
---
## Results Summary

**3 engines responded** of 4 active. 12 results returned in 1.4s.

For **Python async web scraping**, the top results cover:
- Building async scrapers with aiohttp and asyncio
- Rate limiting and concurrent page processing
- Best practices for production scraping pipelines

> DuckDuckGo and Google are currently blocked by rate limiting. Results
> from Brave API and Wikipedia.
```

### 3.5 Error Responses

| Status | Condition | Body |
|---|---|---|
| `200` | Normal response, zero results is valid | Standard response with empty `results` array |
| `400` | Missing or empty `q` parameter | `{"error": "query_required", "message": "The 'q' parameter is required"}` |
| `503` | All engines unresponsive | `{"error": "all_engines_unavailable", "meta": {...}, "results": []}` |
| `429` | Per-client or engine-level rate limit exceeded | `{"error": "rate_limited", "retry_after": 30}` |

The 429 response is returned when a client IP exceeds its per-client request allowance (`PER_CLIENT_REQUESTS` / `PER_CLIENT_WINDOW_SECONDS`) or when all requested engines are rate-limited. When `FAIL_CLOSED` is enabled and Valkey is unreachable, rate-limit checks deny requests by default rather than allowing them through.

The system never returns 500 for a valid request. An empty result set with all engines in `unresponsive_engines` is a valid 200 response — distinguishable from a service failure by the consumer.

---

## 4. Engine Adapter Interface

### 4.1 Core Adapter Contract

```python
from dataclasses import dataclass, field
from typing import Optional
import enum

@dataclass
class SearchResult:
    """Internal normalized result dataclass. Decoupled from wire format."""
    url: str
    title: str
    content: str
    engine: str                           # primary engine name
    engines: set[str] = field(default_factory=set)
    score: float = 0.0
    position: int = 0
    category: str = "general"
    published_date: Optional[str] = None  # ISO 8601
    thumbnail: Optional[str] = None
    img_src: Optional[str] = None

class EngineStatus(enum.Enum):
    OK = "ok"
    RATE_LIMITED = "rate_limited"
    BLOCKED = "blocked"
    ERROR = "error"
    TIMEOUT = "timeout"

@dataclass
class AdapterResponse:
    results: list[SearchResult]
    status: EngineStatus
    error_message: Optional[str] = None
    latency_ms: float = 0.0

class EngineAdapter:
    """Base class for all engine adapters.

    Each adapter lives in one file, registered via @register_engine.
    Zero changes to orchestrator code to add a new engine.
    """

    # Engine identity — used for routing, logging, and config discovery
    name: str = ""                       # e.g. "brave", "duckduckgo"
    display_name: str = ""               # e.g. "Brave Search API"
    env_prefix: str = ""                 # e.g. "ENGINE_BRAVE"
    engine_type: str = "api"             # "api", "scrape", "structured"

    @abstractmethod
    async def search(
        self,
        query: str,
        params: dict | None = None,
    ) -> AdapterResponse:
        """Execute a search against this engine.

        Args:
            query: The search query string
            params: Opaque bag of normalization hints:
                - language: str
                - safesearch: int (0, 1, 2)
                - pageno: int
                - categories: list[str]
                - time_range: str | None

        Returns:
            AdapterResponse with results + status.
            Never raises exceptions — errors are classified and returned
            in the AdapterResponse.status field.

        The adapter owns its full lifecycle:
        - Auth (API key signing for Brave, no auth for Wikipedia)
        - Rate limiting (calls self.rate_limiter.acquire() before sending)
        - HTTP transport (httpx/aiohttp)
        - Response parsing (JSON decode, HTML parse via readability)
        - Error classification (transient vs permanent)
        """
        ...

    async def health(self) -> EngineStatus:
        """Lightweight probe for health check endpoint.
        Default: sends a minimal query and checks for non-error response.
        """
        result = await self.search("healthcheck", {"pageno": 1})
        return result.status

    # Optional lifecycle hooks (default no-op)
    async def warmup(self) -> None: ...
    async def shutdown(self) -> None: ...
```

### 4.2 Adapter Discovery & Registration

```python
# Internal registry — auto-populated at import time
_ENGINE_REGISTRY: dict[str, type[EngineAdapter]] = {}

def register_engine(cls: type[EngineAdapter]) -> type[EngineAdapter]:
    """Decorator that registers an adapter class."""
    assert issubclass(cls, EngineAdapter)
    _ENGINE_REGISTRY[cls.name] = cls
    return cls

def discover_engines() -> dict[str, EngineAdapter]:
    """Instantiate all registered adapters with their config."""
    instances = {}
    for name, cls in _ENGINE_REGISTRY.items():
        config = _load_adapter_config(cls.env_prefix)
        if config.get("enabled", True):
            instances[name] = cls(config)
    return instances
```

Usage in an engine file:

```python
# engines/brave.py
@register_engine
class BraveAdapter(EngineAdapter):
    name = "brave"
    display_name = "Brave Search API"
    env_prefix = "ENGINE_BRAVE"
    engine_type = "api"

    async def search(self, query, params=None) -> AdapterResponse:
        # Brave-specific implementation
        ...
```

### 4.3 Scrape Adapter

```python
class ScrapeAdapter(EngineAdapter):
    """Base class for scrape-based engines (DDG, Google).

    Scrape adapters send HTTP GET/POST requests with stealth headers
    and parse HTML responses — no headless browser required.
    """
    engine_type = "scrape"

    # Defaults for scrape adapters (can be overridden per engine)
    request_headers: dict = field(default_factory=lambda: {
        "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
        "Accept": "text/html,application/xhtml+xml",
        "Accept-Language": "en-US,en;q=0.9",
    })
    timeout_ms: int = 10000
    retry_on_captcha: bool = True

    async def health(self) -> EngineStatus:
        """Lightweight probe: can we reach the engine's homepage?"""
        try:
            async with httpx.AsyncClient(timeout=5.0) as client:
                resp = await client.get(self.base_url, headers=self.request_headers)
                return EngineStatus.OK if resp.status_code == 200 else EngineStatus.ERROR
        except Exception:
            return EngineStatus.ERROR
```

### 4.4 Adapter Contract Rules

1. **Every adapter is exactly one file.** Adding an engine means adding one Python file with a `@register_engine` decorated class. No config file changes, no orchestrator modifications.
2. **Adapters never raise exceptions.** All error states are classified and returned in `AdapterResponse.status`. The orchestrator never sees an unhandled exception from any adapter.
3. **Adapters own their rate limiting.** Each adapter calls `self.rate_limiter.acquire()` — which dispatches to the correct strategy (local, distributed, or external) based on config.
4. **Adapters own their error classification.** An HTTP 429, a CAPTCHA wall, a DOM change that produces garbage — each is classified as transient or permanent, and the adapter decides when to retry vs give up.
5. **Adapters own their timeout budget.** The orchestrator sets a global deadline per request; the adapter manages its internal timeouts within that budget.
6. **No adapter cross-talk.** Adapters do not share state, do not call each other, and cannot depend on another adapter's results.

---

## 5. Result Merging & Ranking

### 5.1 V1: Presence-Weighted Ranking (Honest Baseline)

V1 uses a deliberately simple ranking strategy that is honest about its limitations:

1. **URL normalization:** All result URLs are normalized before deduplication — strip tracking parameters (`utm_*`, `fbclid`, `gclid`), resolve known redirect domains, collapse AMP-to-canonical.
2. **Deduplication:** First occurrence of a normalized URL wins. Subsequent occurrences from other engines add their engine name to the `engines` set field but do not change the result's position.
3. **Scoring:** Results from all engines are interleaved in arrival order within a per-engine result budget (top N per engine, where N is configurable per engine). No cross-engine weight calibration.
4. **Ordering:** Results are ordered by a composite of: presence count (how many engines returned this URL), then arrival order. This biases toward consensus without assuming any engine's ranking is meaningful.

**Documented quality ceiling:** V1 ranking is not better than any individual engine's ranking. It provides breadth (more coverage) at the cost of precision (less accurate ordering). The ranking is **presence-weighted**, not quality-weighted — a result that appears in 2 engine feeds is preferred over one that appears in 1, regardless of which engine.

### 5.2 V2: Weighted Fusion (Target State)

When production traffic generates click-through data, V2 adds:

1. **Per-engine trust scores** stored in Valkey, calibrated against:
   - Historical click-through rate per query category
   - Result position stability (engines whose top-N results change less between queries have higher trust)
   - Latency correlation (slow engines that return high-quality results vs fast engines that return noise)
2. **Weighted fusion** using Reciprocal Rank Fusion (RRF) with per-engine weight multipliers
3. **Presence-weighted ranking** is implemented directly for V1:

```python
class PresenceRanker:
    """V1: presence-weighted, document quality ceiling."""
```

If a second ranking strategy is implemented in a later version, extract a shared interface then.

---

## 6. Configuration Model

### 6.1 Layered Config Strategy

```
Priority: env var overrides > config file > built-in defaults
```

The configuration is layered to support both development simplicity and production scale:

| Layer | Source | What Goes Here | When to Use |
|---|---|---|---|
| Built-in defaults | Code constants | Engine URLs, timeouts, cache TTLs | Development / demo |
| Config file | Mounted file (YAML/TOML) | Engine registry, per-engine tuning, proxy config | Production (K8s ConfigMap) |
| Env vars | `ENGINE_*` namespace | API keys, secrets, operational toggles | All environments |

```yaml
# /etc/slopsearx/config.yaml (optional — mounted via ConfigMap)
engines:
  brave:
    enabled: true
    base_url: "https://api.search.brave.com/res/v1/web/search"
    rate_limit: 15  # requests per second
    timeout_ms: 5000
    max_results: 10
    weight: 1.0  # V2 trust score initial value

  duckduckgo:
    enabled: true
    type: scrape
    timeout_ms: 10000
    max_results: 10
    proxy_pool: "residential"    # proxy pool identifier
    scrape_proxy_url: "http://scrape-proxy:8080/search"
    weight: 0.6

  wikipedia:
    enabled: true
    base_url: "https://en.wikipedia.org/w/api.php"
    rate_limit: 200  # Wikipedia allows generous rate limits
    timeout_ms: 3000
    max_results: 3   # Wikipedia results are narrow but high value
    weight: 0.9

cache:
  ttl_seconds: 300
  max_result_sets: 10000
  revalidate_on_hit: false

ranking:
  strategy: "presence"  # "presence" | "weighted_fusion" | "learning_to_rank"
```

### 6.2 Environment Variable Naming Convention

Each adapter declares an `env_prefix` (e.g., `ENGINE_BRAVE_`, `ENGINE_DDG_`).

| Variable | Purpose |
|---|---|
| `ENGINE_BRAVE_API_KEY` | Secret — Brave API key |
| `ENGINE_DDG_ENABLED` | Toggle — enable DuckDuckGo |
| `ENGINE_DDG_TIMEOUT_MS` | Per-engine timeout override |
| `SEARCH_CACHE_TTL_SECONDS` | Global cache TTL |
| `SEARCH_LOG_LEVEL` | Log level |
| `SEARCH_DEFAULT_ENGINES` | Comma-separated list of default engines |
| `VALKEY_URL` | Valkey connection string |
| `MAX_CONCURRENT_ENGINES` | Concurrency — Max simultaneous outbound HTTP connections per search (default: 10) |
| `PER_CLIENT_REQUESTS` | Rate limit — Allowed search requests per client IP per window (default: 30) |
| `PER_CLIENT_WINDOW_SECONDS` | Rate limit — Sliding window duration for per-client rate limiting (default: 60) |
| `FAIL_CLOSED` | Toggle — When Valkey is unreachable, deny rate-limit checks instead of allowing all (default: false) |

Env var values **override** config file values for the same key. This is how secrets are injected without putting them in the config file.

### 6.3 Why Not Env-Var-Only

At 50+ replicas with 10+ engines, env-var-only config hits two hard limits:

1. **Kubernetes pod spec limit:** ~32768 bytes cumulative for all env vars. 10 engines × 10 config params each × namespace-prefixed names easily exceeds this.
2. **Operational friction:** Changing an engine's timeout or rate limit requires a rollout, not a config reload. A mounted ConfigMap can be hot-reloaded by the application.

The hybrid model preserves the core stateless property: the file is read once at startup and never written to by the application. Replicas remain interchangeable.

---

## 7. Caching Strategy

### 7.1 Response Cache (Valkey)

**Cache key:** `search:{hash(normalized_query + language + safesearch)}`

```python
def cache_key(query: str, params: dict) -> str:
    """Build deterministic cache key from normalized query tuple."""
    norm_query = query.lower().strip()
    norm = f"{norm_query}|{params.get('language','en')}|{params.get('safesearch',0)}"
    return f"search:{sha256(norm.encode()).hexdigest()}"
```

**Value:** Serialized merged result set (after ranking), stored as JSON.

**TTL strategy:**
- Default: 300 seconds (5 minutes)
- Shorter TTL for news/time-sensitive queries (categories includes "news")
- Longer TTL for general queries (up to 30 minutes)
- TTL is refreshed on cache hit (sliding window)

**Cache eviction:** Valkey `allkeys-lru` when memory fills. Critical query types are not prioritized — this is a performance cache, not a durability store.

### 7.2 Rate-Limit State (Valkey)

See Section 8. Rate-limit counters live in a separate Valkey keyspace (`ratelimit:*`) with short TTLs (seconds to minutes) and their own eviction policy.

### 7.3 Per-Engine Quality Metrics (Valkey)

```python
# Key: engine_stats:{engine_name}:{date}
# Value: hash with counters
{
    "queries": 15230,
    "results_returned": 142301,
    "errors": 234,
    "rate_limited": 45,
    "avg_latency_ms": 420,
    "avg_result_score": 0.73
}
```

These are updated asynchronously after each query and used for:
- V2 trust score calibration
- Operator dashboards
- Automatic engine deprioritization (if error rate > 10% over 5 minutes)

### 7.4 What Is NOT Cached

- Individual engine responses are not cached independently — only merged result sets are cached
- API keys, secrets, and config are not cached (read directly from env/file)
- Rate-limit state is ephemeral with TTL

---

## 8. Rate Limiting

### 8.1 Distributed Rate Limiting Strategy

Every adapter call to `self.rate_limiter.acquire()` dispatches through a configurable strategy:

```python
class RateLimitStrategy(ABC):
    @abstractmethod
    async def acquire(self, engine: str, cost: int = 1) -> bool:
        """Try to acquire 'cost' tokens. Returns True if allowed."""
        ...

class LocalTokenBucket(RateLimitStrategy):
    """In-memory token bucket. Correct for 1-3 replicas.
    NOT suitable for 50+ replicas — each replica maintains independent state."""
    ...

class ValkeySlidingWindow(RateLimitStrategy):
    """Distributed sliding window via Valkey INCR + EXPIRE.
    Correct for all replica counts. Centralized rate-limit state."""
    ...

class ExternalSidecar(RateLimitStrategy):
    """Delegates to a dedicated rate-limit service.
    For advanced use cases (e.g., global API key budget across services)."""
    ...
```

**Default at 50+ replicas:** `ValkeySlidingWindow` using:

```
ratelimit:{engine_name}:{window_start}  INCR + EXPIRE
```

Where `window_start = floor(current_timestamp / window_seconds)`. Each replica atomically increments the counter; if the result exceeds the per-window limit, the request is denied.

**Per-engine rate limit parameters** (from config):

```yaml
brave:
  rate_limit: 15        # requests per second (across ALL replicas)
  burst: 30             # max burst
  window: 1             # sliding window in seconds
```

### 8.2 Backpressure Propagation

When an adapter is rate-limited or blocked:
1. The adapter returns `AdapterResponse(status=EngineStatus.RATE_LIMITED)` or `BLOCKED`
2. The orchestrator includes the engine in `unresponsive_engines`
3. The engine is temporarily deprioritized (not removed — a 30-second cooldown, then retry)
4. Over 3 consecutive failures, the engine is deactivated until a health check passes

This prevents a single rate-limited engine from hanging the entire query and allows automatic recovery when the rate limit resets.

### 8.3 Per-Client Rate Limiting

In addition to per-engine rate limits, the server enforces a **per-client request budget** keyed on `request.client.host` using the same `ValkeySlidingWindow` strategy:

```
ratelimit:client:{client_ip}:{window_start}  INCR + EXPIRE
```

When a client exceeds `PER_CLIENT_REQUESTS` within `PER_CLIENT_WINDOW_SECONDS`, the server returns HTTP 429 with `{"error": "rate_limited", "retry_after": N}`. This prevents a single noisy client from starving other tenants and provides a uniform throttle regardless of which engines are requested.

**Fail-closed behavior** (`FAIL_CLOSED`): When Valkey is unreachable, the default (`false`) allows requests through (fail-open) to avoid blocking legitimate traffic during transient Valkey outages. When set to `true`, rate-limit checks deny all requests until Valkey recovers — appropriate for security-sensitive deployments where unbounded traffic is riskier than downtime.

### 8.4 Engine Dispatch Concurrency

The engine dispatch layer uses a semaphore (`MAX_CONCURRENT_ENGINES`, default 10) to cap the number of simultaneous outbound HTTP connections per search request. This prevents resource exhaustion when many engines are active and ensures predictable memory and file-descriptor usage under load. Engines beyond the concurrency cap are queued and dispatched as earlier requests complete.

---

## 9. Observability

### 9.1 Per-Engine Metrics (OpenMetrics)

```
# HELP slopsearx_engine_queries_total Total queries dispatched per engine
# TYPE slopsearx_engine_queries_total counter
slopsearx_engine_queries_total{engine="brave"} 15230
slopsearx_engine_queries_total{engine="duckduckgo"} 8920

# HELP slopsearx_engine_latency_seconds Query latency per engine
# TYPE slopsearx_engine_latency_seconds histogram
slopsearx_engine_latency_seconds{engine="brave",quantile="0.5"} 0.34
slopsearx_engine_latency_seconds{engine="brave",quantile="0.99"} 1.2

# HELP slopsearx_engine_status Engine status (0=ok, 1=degraded, 2=down)
# TYPE slopsearx_engine_status gauge
slopsearx_engine_status{engine="google"} 1

# HELP slopsearx_cache_hit_total Cache hit/miss counters
# TYPE slopsearx_cache_hit_total counter
slopsearx_cache_hit_total{type="hit"} 45000
slopsearx_cache_hit_total{type="miss"} 12000
```

### 9.2 Health Check Endpoint

`GET /health` returns:

```json
{
  "status": "ok",
  "engines": {
    "brave": {"status": "ok", "latency_ms": 120, "results_24h": 12000},
    "wikipedia": {"status": "ok", "latency_ms": 45, "results_24h": 800},
    "duckduckgo": {"status": "degraded", "results_24h": 2000, "last_error": "CAPTCHA"},
    "google": {"status": "down", "results_24h": 0, "last_error": "IP banned"}
  },
  "cache": {"size": 4500, "hit_rate": 0.79},
  "version": "0.1.0",
  "uptime_seconds": 86400
}
```

Load balancer liveness probe uses a 3-strike rule: an engine that fails health 3 consecutive times is deactivated until it recovers. This prevents a single broken engine from causing the replica to be killed by the load balancer.

### 9.3 Quality Degradation Monitoring (Critical)

The hardest operational problem is **silent quality degradation** — where the SearXNG contract returns a valid 200 response with plausible-looking JSON, but the results are garbage. This produces no HTTP error, no alarm, and no operator signal.

**Mitigation:**
1. **Per-engine result distribution monitoring:** Grafana panel comparing the distribution of result counts, scores, and categories across engines over time. A sudden collapse of DuckDuckGo's result distribution relative to Brave's is flagged.
2. **Query diversity tracker:** Monitors whether the search result URLs change between identical queries. Static results = engine returning stale/cached data.
3. **Cross-engine consistency check:** For queries where Brave and DuckDuckGo both return results, what percentage of top-10 URLs overlap? A sudden drop in overlap suggests one engine is returning off-topic results.
4. **Nightly regression test suite:** A set of 100+ curated queries with known good results. Response quality (NDCG@10, MRR) is tracked per-engine over time.

---

## 10. Deployment Topology

### 10.1 Container Image

```dockerfile
# Dockerfile — single image for all replicas
FROM python:3.12-slim
COPY --from=build /app /app
# Dependencies: httpx, pyyaml, lxml (for HTML parsing), valkey
# No Playwright, no headless browsers — scrape engines use HTTP GET/POST + HTML parsing
```

### 10.2 Kubernetes Manifests (Suggested)

```yaml
# deployment.yaml — single deployment type
apiVersion: apps/v1
kind: Deployment
metadata:
  name: slopsearx
spec:
  replicas: 50
  selector:
    matchLabels:
      app: slopsearx
  template:
    metadata:
      labels:
        app: slopsearx
    spec:
      containers:
        - name: slopsearx
          image: slopsearx:0.1.0
          env:
            - name: VALKEY_URL
              value: redis://valkey-cluster:6379
            - name: ENGINE_BRAVE_API_KEY
              valueFrom:
                secretKeyRef:
                  name: brave-api-key
                  key: key
          volumeMounts:
            - name: config
              mountPath: /etc/slopsearx/config.yaml
              subPath: config.yaml
      volumes:
        - name: config
          configMap:
            name: slopsearx-config
```

### 10.3 Horizontal Pod Autoscaler

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: slopsearx
spec:
  maxReplicas: 100
  minReplicas: 3
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: slopsearx
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
```

### 10.4 GroktoCrawl Integration

Replace the current SearXNG service in `docker-compose.yml`:

```yaml
# Current (SearXNG):
# searxng:
#   image: searxng/searxng:latest
#   volumes:
#     - ./searxng/settings.yml:/etc/searxng/settings.yml:ro
#   environment:
#     - SEARXNG_BASE_URL=http://localhost:8081/

# New (SlopSearX):
slopsearx:
  image: slopsearx:0.1.0
  environment:
    - VALKEY_URL=redis://valkey:6379
    - ENGINE_BRAVE_API_KEY=${BRAVE_API_KEY}
  configs:
    - source: jasper-config
      target: /etc/slopsearx/config.yaml
  ports:
    - "8081:8080"  # same port as SearXNG for drop-in replacement
```

The `SEARXNG_URL` env var in GroktoCrawl's agent-svc points to `slopsearx:8080` instead of `searxng:8080`. The response format is backward compatible, so no code changes are needed in `searxng_client.py`.

---

## 11. Out-of-the-Box Engines

### 11.1 Brave Search API

| Property | Value |
|---|---|
| Type | `api` |
| Auth | API key via `ENGINE_BRAVE_API_KEY` |
| Rate limit | 15 req/s (paid), 1 req/s (free) |
| Output | Structured JSON with relevance scores |
| Reliability | High — production API, SLA-backed |
| Payload | Rich metadata: publishedDate, thumbnail, description |

**Implementation:** Direct HTTPS call to `https://api.search.brave.com/res/v1/web/search`. Response is already close to our internal `SearchResult` shape. The adapter primarily maps field names and adds the `engine` tag.

### 11.2 DuckDuckGo (Scrape)

| Property | Value |
|---|---|
| Type | `scrape` |
| Auth | None (IP-based rate limiting) |
| Output | HTML, parsed with readability-lxml |
| Reliability | Low — DOM changes without notice, CAPTCHA walls |
| Payload | Minimal — title, content snippet, URL only |

**Implementation:** Routes through scrape-proxy replica. Uses Playwright to render the search results page, extracts results via CSS selectors. Proxy rotation via `rotate_proxy()` when CAPTCHA detected. No date metadata, no quality signals.

**Note:** DuckDuckGo's anti-bot posture is escalating. This adapter will need ongoing maintenance. Documented as best-effort, no SLA.

### 11.3 Google Search (Scrape)

| Property | Value |
|---|---|
| Type | `scrape` |
| Auth | None (IP-based rate limiting, aggressive anti-bot) |
| Output | HTML, parsed with readability-lxml |
| Reliability | Very low — regular HTML changes, reCAPTCHA, legal risk |
| Payload | Minimal — title, content, URL |

**Implementation:** Same scrape-proxy pattern as DuckDuckGo. Additional note: Google search scraping carries legal and ToS risk. The adapter must include a prominent disclaimer in its docstring and README.

### 11.4 Wikipedia API

| Property | Value |
|---|---|
| Type | `structured` |
| Auth | None (generous rate limits, 200 req/s) |
| Output | JSON via `action=opensearch` or `action=query` |
| Reliability | High — well-documented API, rate-limit polite |
| Payload | Rich — title, extract, URL, optionally thumbnail |

**Implementation:** Direct HTTPS call to `https://en.wikipedia.org/w/api.php`. Uses `action=opensearch` for quick suggestions and `action=query&prop=extracts|pageimages` for detailed results with thumbnails. Limited to top 3 results per query — Wikipedia results are narrow but high-value.

---

## 12. Migration Path from SearXNG

### 12.1 Phase 1: Drop-In Replacement (Day 1)

1. Build merger replicas with Brave + Wikipedia adapters only (no scrapers yet)
2. Point GroktoCrawl's `SEARXNG_URL` to the new service
3. Validate that the existing `/v1/search`, `/v2/search`, health check, and answer/agent pipelines work with the new response
4. Run in parallel with SearXNG, compare result quality on a representative query set

**Risk:** Brave API cost increases. Mitigation: cache hit rate absorbs 80%+ of repeated queries, reducing actual Brave API calls.

### 12.2 Phase 2: Add Scrape Engines (Day 2+)

1. Deploy scrape-proxy replicas with DuckDuckGo adapter
2. Monitor CAPTCHA rate and proxy pool requirements
3. Add Google adapter if Brave + DDG coverage is insufficient
4. Tune per-engine rate limits and timeouts based on real traffic

**Risk:** DDG/Google scrapers break. Mitigation: the two-tier design means a broken scraper degrades quality but doesn't break search. The system continues functioning on Brave + Wikipedia.

### 12.3 Phase 3: Remove SearXNG (Day 30+)

1. Verify that all GroktoCrawl consumers work correctly with the new service for 30+ days
2. Teardown SearXNG container and `searxng/settings.yml`
3. Remove the `search-svc` fixture (no longer needed, replaced by real service)
4. Update documentation and dashboards

---

## Appendix: Council Convergence Summary

| Tension | Starting Range | Converged Position |
|---|---|---|
| Single service vs two-process | Split 3-2 for single | **Single replica type** — all engines in one process, no separate scrape proxy needed because scrape engines use HTTP + HTML parsing, not headless browsers |
| Env-var-only config vs hybrid | Split 2-3 for hybrid | **Hybrid** — env vars for secrets/toggles, mounted config for engine tuning |
| Distributed rate limiting vs per-replica | All agreed on distributed | **Valkey-backed sliding window**, required from day one |
| SearXNG contract as internal schema vs wire-only | All agreed on wire-only | **Internal Result dataclass**, SearXNG JSON is one output formatter |
| Weighted fusion in V1 vs V2 | Split 2-3 for V2 | **Presence-weighted in V1**; extract an interface when a second strategy exists |
| Wikipedia pre-dispatch vs concurrent | All agreed on concurrent | **All engines concurrent**, no query classification needed |
| Brave as primary vs equal-weight engines | All agreed on primary | **Brave API backbone**, scrape engines as optional quality multipliers |

**Data Architect's confidence drop (0.85 → 0.73):** Genuine intellectual growth — recognized that silent ranking degradation from heterogeneous result quality is the hardest operational problem, and that the two-service topology's dependency-surface argument was correct. Confidence dropped from understanding the problem depth, not from disagreement.
