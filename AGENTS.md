# SlopSearX — Agent Guide

This document helps AI coding agents understand the project structure, architecture, and conventions.

## Project Structure

```
slopsearx/
├── engines/            # Engine adapter plugins (one file per engine)
│   ├── arxiv.py
│   ├── brave.py
│   ├── duckduckgo.py
│   ├── github.py
│   ├── google.py
│   ├── hackernews.py
│   ├── huggingface.py
│   ├── semanticscholar.py
│   └── wikipedia.py
├── slopsearx/          # Core library
│   ├── adapter.py      # EngineAdapter base class + ScrapeAdapter
│   ├── merger.py       # Fan-out, deduplication, ranking
│   ├── config.py       # Layered config (env + file + defaults)
│   ├── ratelimit.py    # Distributed rate limiting (Valkey)
│   ├── cache.py        # Response cache
│   ├── formatter.py    # SearXNG JSON + YAML+Markdown formatters
│   └── server.py       # HTTP API (uvicorn/FastAPI)
├── spec.md             # Full architectural specification
├── tests/
├── CONTRIBUTING.md
├── AGENTS.md
└── README.md
```

## Key Architecture Rules

1. **The adapter interface is the primary invariant.** Every engine is one file, registered via `@register_engine`. Adding an engine requires zero changes to the orchestrator.
2. **Adapters never raise exceptions.** All errors are classified and returned in `AdapterResponse.status`. The orchestrator never sees an unhandled exception from any adapter.
3. **Internal schema is decoupled from wire format.** The `SearchResult` dataclass is the internal model. SearXNG JSON is one output formatter among many.
4. **Valkey is the only shared state.** No local volumes, no persistent DB, no per-replica state beyond what Valkey provides.
5. **Scrape engines use HTTP + HTML parsing.** No headless browsers. DDG and Google adapters use `httpx` + `lxml` for HTML parsing — the same approach SearXNG uses.

## API Contract

- `GET /search?q=...&format=json` — SearXNG-compatible JSON
- `GET /search?q=...&format=yaml` — YAML+Markdown (agent-native)
- `GET /search?q=...&categories=science` — filter by category
- `GET /health` — per-engine health check with metrics
- `GET /metrics` — OpenMetrics for Prometheus scraping
- `GET /config` — categories→engines mapping for runtime discovery

The JSON response is a superset of SearXNG's output — same fields, plus `meta.*` extensions.

## Category System

Each engine declares its supported categories via a class attribute. Categories use SearXNG taxonomy — any string is valid, with namespace prefixes for sub-categories (`github:code`, `huggingface:datasets`).

- `?categories=science` — filters to engines declaring `science`
- `?categories=science,news` — OR semantics across requested categories
- `?engines=brave,wikipedia` — explicit engine list overrides category filter
- Operators can override/add/remove categories via env vars: `ENGINE_MYENG_CATEGORIES=news`, `ENGINE_MYENG_CATEGORIES_ADD=finance`

## Engine Adapter Quick Reference

```python
from slopsearx.adapter import EngineAdapter, register_engine, AdapterResponse

@register_engine
class MyEngine(EngineAdapter):
    name = "myengine"
    display_name = "My Engine API"
    env_prefix = "ENGINE_MYENGINE"
    engine_type = "api"
    categories = ["general", "science"]  # SearXNG-compatible category tags

    async def search(self, query, params=None) -> AdapterResponse:
        """Execute search. Never raise — classify errors in AdapterResponse.status."""
        ...
```

**Full adapter reference:** `docs/ENGINE_ADAPTERS.md` — contract rules, data types, lifecycle hooks, sub-categories, built-in adapter table.

## Commit Conventions

- Conventional Commits: `feat:`, `fix:`, `docs:`, `chore:`, `ci:`, `refactor:`
- DCO sign-off required on every commit (`git commit -s`)
- One feature or fix per PR

## Design Documents

- `spec.md` — full architecture spec, API contract, deployment topology, caching strategy
- `CONTRIBUTING.md` — contribution workflow
