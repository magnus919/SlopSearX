# SlopSearX — Agent Guide

This document helps AI coding agents understand the project structure, architecture, and conventions.

## Project Structure

```
slopsearx/
├── engines/            # Engine adapter plugins (one file per engine, 51 total)
│   ├── arxiv.py           brave.py           crates.py
│   ├── censys.py          clinicaltrials.py  courtlistener.py (removed)
│   ├── crtsh.py           cve.py             dehashed.py
│   ├── dockerhub.py       duckduckgo.py      edgar.py
│   ├── epss.py            exploitdb.py       fred.py
│   ├── github.py          google.py          greynoise.py
│   ├── hackernews.py      hibp.py            huggingface.py
│   ├── intelx.py          internetarchive.py mitreattack.py
│   ├── musicbrainz.py     nominatim.py       npm.py
│   ├── nvd.py             openalex.py        openfda.py
│   ├── openlibrary.py     otx.py             oyez.py
│   ├── pubchem.py         pubmed.py          pypi.py
│   ├── reddit.py          repology.py        rubygems.py
│   ├── semanticscholar.py shodan.py          stackexchange.py
│   ├── tmdb.py            uniprot.py         urlhaus.py
│   ├── virustotal.py      vulncheck.py       wikipedia.py
│   ├── abuseipdb.py       ashby.py           greenhouse.py
│   └── lever.py
├── slopsearx/          # Core library
│   ├── adapter.py      # EngineAdapter base class + ScrapeAdapter
│   ├── service.py      # Normalized search pipeline (SearchService, ScopeResolver, AppContext)
│   ├── capabilities.py # Runtime capability catalog, intent profiles, MCP policy
│   ├── snapshot.py     # Opaque search snapshots for cursor pagination
│   ├── research.py     # Async research jobs (Valkey-backed)
│   ├── mcp/            # MCP server (FastMCP): tools, resources, prompts, auth,
│   │                   #   remote gateway mode (gateway.py), OAuth 2.1 server
│   │                   #   (oauth.py), and the gateway's OAuth client flow (oauth_client.py)
│   ├── merger.py       # Fan-out, deduplication, ranking
│   ├── config.py       # Layered config (env + file + defaults)
│   ├── ratelimit.py    # Distributed rate limiting (Valkey)
│   ├── cache.py        # Response cache
│   ├── formatter.py    # SearXNG JSON + YAML+Markdown formatters
│   └── server.py       # HTTP API (uvicorn/FastAPI) — thin adapter over service.py
├── spec.md             # Full architectural specification
├── tests/
├── docs/MCP_SERVER.md  # MCP server install/config/usage docs (users + agents)
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
6. **README.md reflects every engine.** Adding or removing an engine file requires updating the Engines table in `README.md`. The table lists every registered adapter with its type, auth, and categories.
7. **One shared policy gate.** Every search-capable MCP path (generic `slopsearx_search`, `slopsearx_search_targeted`, jobs, security, science) and the scope-preview tool and research query planning reach a single fail-closed gate (`_enforce_policy` in `slopsearx/mcp/tools.py`) before any engine dispatch. Sensitive engines (`hibp`, `dehashed`) are unreachable on every path unless the uniform sensitive-engine grant `MCP_TARGETED_SENSITIVE_ALLOWED` is set. The specialist grants (`MCP_GRANT_JOBS/SECURITY/SCIENCE/RESEARCH`) enable their tools; they do **not** by themselves grant sensitive-engine access. A mixed sensitive + non-sensitive explicit engine list fails closed atomically.
8. **Structured filter-enforcement report.** Every search tool returns a machine-readable `enforcement` object keyed by filter name (`language`, `time_range`, `safesearch`, plus specialist params like jobs `location`/`employment_type` and science `date_from`/`date_to`), each entry `{requested, status, reason, enforced_by}` where `status` is exactly one of `enforced`/`partially_enforced`/`unsupported`/`rejected`. No adapter enforces language/time/safesearch today, so they report `unsupported`; strict SafeSearch is `rejected` (fails closed). Never report an unenforced filter as enforced.
9. **Canonical cache + view derivation.** The cache stores the canonical full `SearchResponse` (all include-able fields, unsliced results), keyed by query, language, safesearch, categories, engines, page, and time range — not by `include`/`max_results`/`freshness`. The MCP read boundary derives the requested view (include-filtered fields and the `max_results` slice) from the current request, so a cached response never depends on the request that populated it. `max_results` is a presentation bound that slices the presented page; it never truncates the captured snapshot.
10. **JSON-safe serialization.** `SearchResult.engines` is a `set[str]`; it is canonicalized to a sorted list at the serialization boundary and rehydrated robustly (accepting a list or a legacy stringified set). Never rely on `json.dumps(default=str)` for any typed field that must round-trip through the cache or snapshots.
11. **MCP `state_factory` test-injection hook.** `slopsearx/mcp/server.py` accepts a `state_factory` callable that overrides the runtime wiring (engines + shared store) while keeping the transport, tool surface, and auth identical. It is used by `slopsearx/mcp/harness.py` to drive deterministic MCP client tests against fake engines and an in-memory store. Do not remove it; it is the user-testing validation seam.

## API Contract

- `GET /search?q=...&format=json` — SearXNG-compatible JSON
- `GET /search?q=...&format=yaml` — YAML+Markdown (agent-native)
- `GET /search?q=...&categories=science` — filter by category
- `GET /health` — per-engine health check with metrics
- `GET /metrics` — OpenMetrics for Prometheus scraping
- `GET /config` — categories→engines mapping for runtime discovery

The JSON response is a superset of SearXNG's output — same fields, plus `meta.*` extensions.

## Two-Tier Engine System

Unscoped searches (no `categories` or `engines` param, no topic match) use all active engines split into two tiers:

- **Tier 1** — Broad, general-purpose engines (`brave`, `duckduckgo`, `google`, `wikipedia`, `stackexchange`, `reddit`). These form the primary result set, ranking above all Tier 2 results.
- **Tier 2** — All other engines (specialised: science, packages, security, finance, media, etc.). Results are ranked below Tier 1, keeping top results focused on broadly relevant content while still surfacing domain-specific results.

Each `SearchResult` carries a `tier` field (1 or 2) exposed in both JSON and YAML+Markdown outputs. The `PresenceRanker` sorts by `(tier, -score)`, and when deduplicating by URL, the higher-priority tier (lower number) is preserved.

All new engines are Tier 2 by default. See `CONTRIBUTING.md` for tier governance rules.

## Category System

Each engine declares its supported categories via a class attribute. Categories use SearXNG taxonomy — any string is valid, with namespace prefixes for sub-categories (`github:code`, `huggingface:datasets`).

- `?categories=science` — filters to engines declaring `science`
- `?categories=science,news` — OR semantics across requested categories
- `?engines=brave,wikipedia` — explicit engine list overrides category filter
- Operators can override categories via env vars: `ENGINE_MYENG_CATEGORIES=news`

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
- Run `pre-commit run --all-files` before pushing to catch lint, type, and dead-code issues locally
- One feature or fix per PR

## Design Documents

- `spec.md` — full architecture spec, API contract, deployment topology, caching strategy
- `CONTRIBUTING.md` — contribution workflow

## graphify

This project has a graphify knowledge graph at graphify-out/.

Rules:
- Before answering architecture or codebase questions, read graphify-out/GRAPH_REPORT.md for god nodes and community structure
- If graphify-out/wiki/index.md exists, navigate it instead of reading raw files
- For cross-module "how does X relate to Y" questions, prefer `graphify query "<question>"`, `graphify path "<A>" "<B>"`, or `graphify explain "<concept>"` over grep — these traverse the graph's EXTRACTED + INFERRED edges instead of scanning files
- After modifying code files in this session, run `graphify update .` to keep the graph current (AST-only, no API cost)
