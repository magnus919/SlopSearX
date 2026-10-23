# SlopSearX — Agent Guide

This document helps AI coding agents understand the project structure, architecture, and conventions.

## Start Every Session from Current Upstream Main

Before codebase analysis, planning, or edits in a fresh session:

1. Inspect `git status --short --branch`, `git worktree list --porcelain`, and
   `git remote -v`. Identify local changes, the current branch, and which
   worktree owns `main`; do not assume the starting checkout is current.
2. Fetch the authoritative upstream main branch. In this repository the
   upstream remote is normally `origin`: run `git fetch origin` and use
   `origin/main`. If remotes differ, verify the authoritative remote first.
3. Synchronize an available local `main` with `git switch main` followed by
   `git merge --ff-only origin/main`. Start new task branches or isolated
   worktrees from that freshly fetched main revision. For read-only analysis,
   use the synchronized checkout or a clean detached worktree at `origin/main`.
4. Preserve unrelated tracked and untracked files and existing branch commits.
   Inspect any other worktree that owns `main` before changing its branch;
   never disrupt active work. If the current checkout is dirty, diverged, or
   owned by another task, use an isolated worktree from `origin/main` instead
   of resetting, cleaning, force-checking out, or automatically stashing it.
5. Verify and report the baseline commit with `git rev-parse HEAD` and
   `git rev-parse origin/main`. Before new work, they must match;
   `git rev-list --left-right --count HEAD...origin/main` must return `0 0`.
   Re-read this guide and relevant contribution instructions after updating.

When explicitly continuing an existing branch/PR or examining a historical
revision, fetch main first, inspect divergence, and state the chosen revision
and reason. Preserve that task's history; integrate main only through its
normal reviewed workflow. A failed fetch is not evidence of freshness: report
the limitation and do not present the checkout as current. Keep work independent
of current-source assumptions moving while resolving access.

After delivering and merging changes, safely fast-forward the primary local
`main` again and verify it against a fresh fetch. Do not leave the primary
checkout on an obsolete task branch for the next session.

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
│   ├── reddit.py          rubygems.py
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
8. **Structured filter-enforcement report.** Every search tool returns a machine-readable `enforcement` object keyed by filter name (`language`, `time_range`, `safesearch`, `pagination`, plus specialist params like jobs `location`/`employment_type` and science `date_from`/`date_to`), each entry `{requested, status, reason, enforced_by}` where `status` is exactly one of `enforced`/`partially_enforced`/`unsupported`/`rejected`. `enforced_by` carries layer-qualified tokens (`upstream:<engine>` / `local:<engine>`) naming the enforcing engines **and** the enforcement layer. Enforcement is derived only from the audited `enforced_filters` adapter declaration (layer per filter) — `supported_filters` is a parameter-consumption hint and never marks a filter as enforced. OpenAlex enforces science `date_from`/`date_to` upstream and `time_range` locally from its publication dates; other adapters remain unsupported for these date filters. No adapter enforces language or SafeSearch; strict SafeSearch is rejected fail-closed **before dispatch** when the selected scope cannot satisfy it. Never report an unenforced filter as enforced. The shared resolver is `slopsearx/filters.py`.
9. **Canonical cache + view derivation.** The cache stores the canonical full `SearchResponse` (all include-able fields, unsliced results), keyed by query, language, safesearch, categories, engines, page, time range, and explicit publication-date bounds — not by `include`/`max_results`/`freshness`. The MCP read boundary derives the requested view (include-filtered fields and the `max_results` slice) from the current request, so a cached response never depends on the request that populated it. `max_results` is a presentation bound that slices the presented page; it never truncates the captured snapshot.
10. **JSON-safe serialization.** `SearchResult.engines` is a `set[str]`; it is canonicalized to a sorted list at the serialization boundary and rehydrated robustly (accepting a list or a legacy stringified set). Never rely on `json.dumps(default=str)` for any typed field that must round-trip through the cache or snapshots.
11. **MCP `state_factory` test-injection hook.** `slopsearx/mcp/server.py` accepts a `state_factory` callable that overrides the runtime wiring (engines + shared store) while keeping the transport, tool surface, and auth identical. It is used by `slopsearx/mcp/harness.py` to drive deterministic MCP client tests against fake engines and an in-memory store. Do not remove it; it is the user-testing validation seam.
12. **The web portal is a first-class service.** Changes to search requests/responses, result serialization, engine capabilities, filter enforcement, access policy, cache/snapshots, HTTP routes, configuration, packaging, or engine behavior require a portal impact review. Reuse the shared `SearchService`, capability catalog, policy inputs, and HTML formatter; add or update deterministic portal contract/browser coverage and the portal docs when the browser-visible contract changes. The CI portal gate is required evidence for these changes.
13. **Every built-in engine has Jev routing metadata.** Classify it as `general`, `broad_support`, or `specialist` in `slopsearx/jev.py`. Specialists require concise `purpose` and `use_when` routing-card fields. Run `tests/test_jev.py`; the completeness test must fail when the built-in adapter set and routing metadata diverge. Cards describe retrieval fit only—live policy, credentials, cost, and health remain authoritative eligibility gates.

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

## Rust Build Footprint and Post-Merge Cleanup

Keep new Rust build and scratch artifacts outside the checkout. On this Mac,
use `/Volumes/tank01/magnus/rust-builds/slopsearx/`; on another host, configure
an equivalent persistent build root. Put Cargo targets and caches, `TMPDIR`,
source copies, overlays, archives, logs, receipts, and handoffs under that
root. Do not fall back to `/private/tmp`, `/tmp`, or `.codex` worktree
locations without explicit user consent. A CI runner may use its designated
ephemeral job directory if the job cleans it up. Give each task/run a specific
directory and record its absolute paths, owner, issue or PR, artifact class,
measured bytes, and retain/delete condition in the persistent task ledger
under that root's `storage-ledger/` before starting work.

Before a build, record `df` availability, the task directory's current `du`
usage, and an estimate based on a comparable prior build or the planned inputs.
Reuse an exclusive target or an approved shared cache when safe; for shared
caches, record the owner, size, reuse justification, and review/prune date.
Avoid duplicate targets and concurrent writers to a shared target.
After moving a checkout, do not assume cached package artifacts match the new
workspace root. Check the affected package's dep-info and compile-time or runtime
fixture-root paths against the current checkout. If they still name the old root,
invalidate and rebuild only the affected owned package artifacts in the same
approved target under the normal exclusive source, target, and Cargo-cache leases; record target usage before and after, and
preserve the original failure output and provenance. Keep the registry/download
cache. Do not blanket-clean Cargo, create a duplicate target, or restore/copy the
checkout to its former absolute path. See
[`docs/RUST_BUILD_FOOTPRINT.md`](docs/RUST_BUILD_FOOTPRINT.md) for the run and
cleanup checklist.

Treat post-merge cleanup as a PR acceptance step. Audit active processes and
open-file handles and search receipts/handoffs for references before removing
owned build-only scratch and duplicate or superseded task targets/source
copies. Preserve the baseline SHA, patch/lock/toolchain/commands and exits, and
unique raw outputs needed to review the result. Compiled binaries and full
source copies are reproducible build artifacts; retain them only for an
explicit active investigation or release obligation with a review date. A
receipt reference alone does not require indefinite retention: after checking
references, record where an artifact was moved or that it was retired. Keep a
shared cache only with its owner, size, reuse reason, and review/prune date.
Archive a worktree with the Codex app tool when it is app-managed. For other
worktrees, inspect branch state and local changes before any removal. After
cleanup, record `du` and `df` again and report measured logical reduction,
available-space change, and any remaining artifacts with their retention
reasons.

## Design Documents

- `spec.md` — full architecture spec, API contract, deployment topology, caching strategy
- `CONTRIBUTING.md` — contribution workflow
- `docs/PORTAL_UX_SPEC.md`, `docs/PORTAL_VISUAL_SYSTEM.md`, and `docs/PORTAL_ACCEPTANCE.md` — portal behavior, visual, and release contracts
- `docs/PORTAL_DEPLOYMENT.md` — operator URL, access, smoke, rollout, rollback, and troubleshooting runbook

## graphify

This project has a graphify knowledge graph at graphify-out/.

Rules:
- Before answering architecture or codebase questions, read graphify-out/GRAPH_REPORT.md for god nodes and community structure
- If graphify-out/wiki/index.md exists, navigate it instead of reading raw files
- For cross-module "how does X relate to Y" questions, prefer `graphify query "<question>"`, `graphify path "<A>" "<B>"`, or `graphify explain "<concept>"` over grep — these traverse the graph's EXTRACTED + INFERRED edges instead of scanning files
- After modifying code files in this session, run `graphify update .` to keep the graph current (AST-only, no API cost)
