# Features overview

SlopSearX provides three cross-cutting feature areas that extend beyond the core architecture: engine implementations that connect to 12 different search backends, output formatters that map the internal SearchResult model to wire formats, and the adapter interface that makes new engines pluggable without orchestrator changes.

## Feature areas

| Feature | Description | Key files |
|---|---|---|
| [Engine implementations](engine-implementations.md) | 12 built-in adapters covering Brave, Wikipedia, DuckDuckGo, Google, GitHub, HuggingFace, Internet Archive, OpenAlex, Stack Exchange, arXiv, Hacker News, and Semantic Scholar | `engines/*.py` |
| [Output formatters](output-formatters.md) | Two output formats: SearXNG-compatible JSON with all 23 MainResult fields, and YAML+Markdown for AI agent consumption | `slopsearx/formatter.py` |
| Adapter interface | Abstract base classes `EngineAdapter` and `ScrapeAdapter`, `@register_engine` decorator, and global engine registry | `slopsearx/adapter.py` |

## Cross-cutting design principles

- **Every engine is one file.** Adding a new engine requires zero changes to the orchestrator. Each adapter is registered via `@register_engine` in its own file under `engines/`.
- **Adapters never raise exceptions.** All error states are classified into `EngineStatus` (OK, RATE_LIMITED, BLOCKED, ERROR, TIMEOUT) and returned in `AdapterResponse.status`. The orchestrator never sees an unhandled exception from any adapter.
- **Internal schema is decoupled from wire format.** The `SearchResult` dataclass is the internal model. Output formatters handle the conversion to SearXNG JSON or YAML+Markdown.
- **Rate limiting is distributed.** All engines share a Valkey-backed sliding window rate limiter that is correct even at 50+ replicas.

## See also

- [Core primitives](../primitives/index.md) — foundational data types used by all features
- [System architecture](../overview/architecture.md) — request flow and system design
- [Glossary](../overview/glossary.md) — project-specific terms
