# Core primitives overview

SlopSearX is built on a small set of foundational data types that appear throughout the codebase. These primitives define how search results are represented, how engines communicate status, and how the system is configured.

The three core primitives are:

- **SearchResult** (`slopsearx/adapter.py`) — the internal normalized result dataclass. Contains url, title, content, engine metadata, score, position, category, and optional date/thumbnail/image fields. Decoupled from any output format.
- **AdapterResponse** (`slopsearx/adapter.py`) — the canonical return type for every adapter's `search()` method. Wraps a list of SearchResult with a status enum, optional error message, and measured latency. Adapters never raise exceptions.
- **EngineStatus** (`slopsearx/adapter.py`) — an enum with five values: OK, RATE_LIMITED, BLOCKED, ERROR, and TIMEOUT. Every adapter classifies its results using this enum.

Supporting primitives include:

- **EngineEntry** (`slopsearx/config.py`) — per-engine configuration dataclass. Contains settings for base_url, type, timeout, max_results, rate_limit, weight, api_key, category overrides, and scrape-specific fields.
- **Config** (`slopsearx/config.py`) — top-level configuration that aggregates per-engine entries with global settings for caching, ranking, default engines, and log level.

## Data flow

A search query flows through these primitives in sequence:

1. Configuration defines which engines to use and their parameters (via `EngineEntry` and `Config` in `slopsearx/config.py`)
2. Each engine's `search()` method returns an `AdapterResponse` containing a list of `SearchResult` and an `EngineStatus`
3. The merger ranks and deduplicates results, producing a single ranked list of `SearchResult`
4. Formatters serialize `SearchResult` lists into the wire format (JSON or YAML+Markdown)

## See also

- [Search result types](search-result.md) — detailed reference for SearchResult, AdapterResponse, EngineStatus, and EngineEntry
- [Features overview](../features/index.md) — how primitives enable engine implementations and output formatters
- [System architecture](../overview/architecture.md) — end-to-end request flow
