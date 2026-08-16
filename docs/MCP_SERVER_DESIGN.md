# SlopSearX MCP Server Design

Status: **implemented** — see [docs/MCP_SERVER.md](MCP_SERVER.md) for the
operator and agent guide. This document is the original discovery and
architecture proposal; implementation notes and deviations:

- Co-located in-process deployment (FastMCP), per §2.1 and §7.
- Cache scoping and representation identity (§1.5): cache keys now include
  categories, engines, page, and time range; the answer cache is no longer
  used by the search path. The cache stores the **canonical full response**
  and the MCP read boundary derives the requested `include`/`max_results`
  view, so a cached response's representation never depends on the request
  that populated it.
- Specialist tools (jobs, security, science) implemented and config-gated
  (§5); research jobs implemented with Valkey-backed state (§2.2).
- Snapshot pagination uses opaque cursors (§10.8); `read_results`/`read_result`
  accept only server-issued handles.
- Strict SafeSearch fails closed because no adapter enforces it (§1.5).
- HTTP transport auth = bearer token (`mcp.auth_token`); stdio trusted by
  launch boundary (§10.6).
- Remote deployment: `slopsearx-mcp --remote <url>` adds a stdio gateway
  mode that proxies tools/resources/prompts to a remote SlopSearX MCP
  server over streamable HTTP (§2.1 deployment stance, agent-host option).
- OAuth (§10.6): standard MCP OAuth 2.1 authorization-server mode with
  dynamic client registration (RFC 7591) and PKCE, auto-approving (no user
  accounts); static bearer-token mode remains the alternative.
- The gateway also speaks OAuth 2.1 as a *client* (`--remote <url> --oauth`):
  loopback-callback authorization from the agent host, PKCE, and token
  persistence (0600 file) so later runs reuse tokens without re-authorizing.

This document translates SlopSearX into an agent-facing MCP surface. It is deliberately not a one-to-one wrapper around the HTTP API. The HTTP API remains the compatibility boundary for SearXNG consumers; MCP should expose intent-level operations, capability discovery, explicit scope controls, and result metadata that help an agent search correctly without learning SlopSearX's transport quirks.

## 1. Repository findings

### 1.1 The public HTTP surface is small

The live FastAPI route table contains four application endpoints, plus FastAPI's generated documentation routes:

| Method | Path | Role | Evidence |
|---|---|---|---|
| GET | `/search` | Fan-out search, routing, rate limiting, cache lookup, merge, ranking, suggestions, and response formatting | `slopsearx/server.py:341-619` |
| GET | `/health` | Server liveness and Valkey connectivity; it does not actively probe external engines | `slopsearx/server.py:268-300` |
| GET | `/config` | Runtime category-to-engine mapping built from active adapters | `slopsearx/server.py:320-333` |
| GET | `/metrics` | OpenMetrics text for Prometheus, not an agent search operation | `slopsearx/server.py:308-312` |

FastAPI also exposes `/openapi.json`, `/docs`, `/docs/oauth2-redirect`, and `/redoc`. Those are framework documentation routes, not SlopSearX product capabilities.

`GET /search` accepts:

- `q` (required)
- `format` (`json` or `yaml` in the implementation)
- `categories` (comma-separated, OR semantics)
- `engines` (comma-separated; explicit engine selection overrides categories)
- `language` (default `en`)
- `pageno` (1-based, minimum 1)
- `time_range` (engine-dependent `day`, `month`, or `year`)
- `safesearch` (`0`, `1`, or `2`)

The endpoint has important behavior that an MCP layer must preserve and explain:

1. Empty queries return `400 query_required`.
2. Explicit engines win over category filters.
3. With no explicit scope, `QueryRouter` uses first-match topic routing; otherwise it falls back to the broad tier-1 set.
4. Engine calls run concurrently behind a semaphore and per-engine circuit breakers.
5. Individual engine failures are returned as partial degradation. An all-unresponsive request returns `503`.
6. A successful response may be served from Valkey cache.
7. Suggestions are fetched in the background, but the service is opt-in and disabled by default.
8. The JSON response is the stable machine contract. YAML+Markdown is a secondary representation, not the MCP data model.

### 1.2 The current adapter registry is larger than the documentation says

The live source registry contains 51 adapters, discovered by importing `engines/` and registering classes with `@register_engine`. The source-level count includes the job adapters `ashby`, `greenhouse`, and `lever`.

The repository's README and adapter reference previously described the system
as having 48 engines; that count has since been reconciled to the live 51-adapter
registry (`docs/ENGINE_ADAPTERS.md`, `README.md`). The MCP capability catalog is
generated from the runtime registry, not copied from those prose counts.

The registry spans these capability families:

- General and web: Brave, DuckDuckGo, Google, Hacker News, Reddit, Wikipedia
- Developer and packages: GitHub, Stack Exchange, npm, PyPI, Crates.io, RubyGems, Docker Hub, Repology
- Science and research: arXiv, Hugging Face, Internet Archive, OpenAlex, Open Library, Semantic Scholar, UniProt
- Medical and health: ClinicalTrials.gov, openFDA, PubChem, PubMed
- Security and threat intelligence: AbuseIPDB, OTX, Censys, CRT.sh, CVE, DeHashed, Exploit-DB, EPSS, GreyNoise, HIBP, IntelX, MITRE ATT&CK, NVD, Shodan, URLhaus, VirusTotal, VulnCheck
- Finance and economics: FRED, SEC EDGAR
- Media: MusicBrainz, TMDB
- Geography: Nominatim
- Legal: Oyez
- Jobs: Ashby, Greenhouse, Lever

The authoritative per-engine metadata is the registered class (`name`, `display_name`, `engine_type`, `categories`, `env_prefix`) plus its implementation and tests. `GET /config` exposes categories, but not display names, auth requirements, result semantics, or engine-specific query constraints.

### 1.3 Existing CLI behavior confirms the intended agent workflow

`ssx` already offers four user-level operations:

- `search`
- `engines`
- `health`
- `config`

It defaults to YAML output for humans and agents, while allowing JSON. The MCP server should improve on this by returning typed structured content and by making scope and limitations explicit rather than asking the agent to parse YAML or reverse-map `/config` and `/health` itself.

### 1.4 Internal primitives worth reusing

The MCP implementation should call a service-layer abstraction around the existing internals rather than issue HTTP requests to the server from inside the same process. Relevant primitives are:

- `EngineAdapter`, `AdapterResponse`, `EngineStatus`, and `SearchResult` in `slopsearx/adapter.py`
- `discover_engines()` and the registry in `slopsearx/adapter.py`
- `QueryRouter` in `slopsearx/router.py`
- ranking and deduplication in `slopsearx/merger.py`
- response serialization in `slopsearx/formatter.py`
- configuration loading in `slopsearx/config.py`
- cache, rate limiting, audit logging, suggestions, and quality statistics used by `server.py`

The MCP boundary should consume a normalized result model and add an MCP-specific envelope. It should not expose SearXNG's 23 compatibility fields as the primary contract.

### 1.5 Additional contract hazards

The independent source review identified several behaviors that must be treated as compatibility hazards rather than normalized away silently:

- The HTTP service has no authentication or authorization middleware. Provider credentials are server-side only. An MCP transport exposed beyond a trusted network therefore needs its own access control.
- The primary cache key contains query, language, and SafeSearch, but not engines, categories, page, or time range. The answer cache is broader still. An MCP layer must either repair cache scoping in the shared service or surface that a cached response may not fully represent the requested filters.
- `format=yaml` takes a separate return path. All-engine failure can produce HTTP 200 on that path, while JSON returns 503. MCP should use structured internal results and never inherit this ambiguity.
- The JSON all-engine failure response can contain a normal result-shaped body without a top-level `error` field. MCP must classify failure from engine outcomes and service state, not merely from the presence of an `error` key.
- Successful dispatched searches schedule audit writes containing the raw query and client IP. The current audit stream is retained in Valkey for approximately 90 days. MCP documentation and authorization must account for that privacy boundary.
- `/health` and the FastAPI application metadata report version `0.1.0`, while `pyproject.toml` reports package version `0.2.0`. The MCP server should not use `/health.version` as its authoritative compatibility version.

## 2. Product goal for MCP

An agent should be able to:

1. discover what SlopSearX can search without memorizing engine names;
2. express a search goal in plain terms;
3. choose between automatic routing and an explicit evidence scope;
4. understand which sources actually answered and which failed;
5. paginate or refine without reconstructing URL query strings;
6. distinguish search results from evidence about engine health;
7. use specialist source families such as jobs, security, science, packages, and medical sources without accidentally sending the query to an irrelevant set;
8. receive stable, bounded, citation-ready result objects;
9. see caveats when a result set is partial, cached, stale, unranked, or produced by scrape adapters.

The MCP layer should optimize for correct agent decisions, not for maximum parity with REST parameters.

### 2.1 Architectural stance

The recommended deployment is a co-located orchestration MCP server that uses the adapter, router, merger, and Valkey layers directly. A remote sidecar that calls the existing REST endpoint is not safe for scoped MCP searches until the cache key includes every result-affecting input, or the HTTP path provides a verified cache bypass. Otherwise two semantically different searches can receive the same cached response.

The MCP layer should introduce immutable search snapshots and opaque handles. Pagination must read from a captured merged snapshot, not translate directly into the HTTP `pageno` parameter. This prevents a later page from silently rerunning the query against changing engines and gives the agent a stable evidence set.

SlopSearX's merged `score` is a cross-engine presence signal, not relevance confidence. MCP output should expose `source_count` and a textual ranking explanation such as `tier_then_cross_engine_presence`, never call that score confidence, truth, or evidence quality.

The current HTTP parameters also overstate enforcement: `pageno`, `language`, `time_range`, and `safesearch` are placed in the adapter parameter bag, but several adapters do not consume them. GitHub currently hardcodes page 1, and Brave currently hardcodes SafeSearch off. MCP must either verify enforcement per selected engine or return an explicit unsupported-filter warning. Strict SafeSearch should fail closed when policy enforcement is unavailable rather than silently claim compliance.

### 2.2 Progressive disclosure and lifecycle

The default search response should contain only title, URL, short snippet, publication date, source count, and stable result IDs. A diagnostic detail level can add per-engine latency, failures, empty-scrape diagnostics, and routing decisions. A `read_result` operation can expand normalized metadata and provenance, but must never imply that SlopSearX fetched the full page body.

For multi-query research, an asynchronous job model is appropriate. It should use Valkey-backed shared state, bounded query/engine/result budgets, caller-supplied idempotency keys, immutable completed evidence, best-effort cancellation of undispatched work, and explicit `queued`, `running`, `partial`, `succeeded`, `failed`, `cancelled`, and `expired` states. This is a proposed MCP capability, not an existing SlopSearX HTTP feature.

## 3. Proposed MCP surface

### 3.1 Core tools

#### `slopsearx_search`

Primary entry point for ordinary web and knowledge search.

Input:

```json
{
  "query": "string, required",
  "intent": "auto | web | news | science | reference | code | social | historical | jobs | security | medical | finance | packages | media | legal | geography, default auto",
  "scope": {
    "categories": ["string"],
    "engines": ["string"],
    "language": "string, default en",
    "time_range": "day | month | year | null",
    "safesearch": "off | moderate | strict"
  },
  "page": "integer >= 1, default 1",
  "max_results": "integer, bounded by server policy",
  "include": ["results", "suggestions", "engine_status", "diagnostics"],
  "freshness": "prefer_cache | prefer_fresh | no_preference"
}
```

Behavior:

- `intent=auto` uses the existing `QueryRouter` behavior.
- An intent maps to a documented category profile, not a guessed engine list. The profile is resolved against live capabilities and reports its selected categories and engines.
- `scope.engines` is an explicit override and must be honored exactly for known active engines.
- `scope.categories` is an OR filter. If both categories and engines are supplied, engines win, matching the HTTP contract.
- `max_results` is an MCP presentation bound. It must not pretend to increase an adapter's configured maximum.
- `include` controls response size. Engine diagnostics should be opt-in for normal searches and automatically included when the result set is materially degraded.

Output:

```json
{
  "query": "string",
  "results": [
    {
      "title": "string",
      "url": "string",
      "snippet": "string",
      "source_engines": ["string"],
      "primary_engine": "string",
      "category": "string",
      "published_at": "string | null",
      "score": "number",
      "position": "integer",
      "tier": "integer",
      "citation": {"label": "string", "url": "string"}
    }
  ],
  "scope": {
    "requested_intent": "string",
    "resolved_categories": ["string"],
    "selected_engines": ["string"],
    "routing_reason": "string"
  },
  "engine_outcomes": [
    {
      "engine": "string",
      "status": "ok | rate_limited | blocked | error | timeout",
      "result_count": "integer",
      "latency_ms": "number | null",
      "message": "string | null"
    }
  ],
  "meta": {
    "query_id": "string",
    "cached": "boolean",
    "response_time_ms": "integer",
    "partial": "boolean",
    "suggestions": ["string"]
  }
}
```

The MCP output uses `snippet`, `source_engines`, and `published_at` instead of forcing an agent to understand SearXNG fields such as `content`, `engines`, `publishedDate`, and `pubdate`. The HTTP formatter remains unchanged for compatibility.

#### `slopsearx_search_targeted`

Explicit source selection for an agent that knows the evidence boundary it needs.

Input:

```json
{
  "query": "string, required",
  "engines": ["string, required, at least one"],
  "language": "string, default en",
  "time_range": "day | month | year | null",
  "safesearch": "off | moderate | strict",
  "page": "integer >= 1, default 1",
  "max_results": "integer"
}
```

This is intentionally separate from the ordinary search tool. It makes a deliberate, auditable choice to query named sources and should return an error listing valid engine names when any requested engine is unknown or inactive. It must not silently broaden scope.

#### `slopsearx_search_jobs`

Job-search entry point that hides ATS-specific query syntax.

Input:

```json
{
  "company": "string, required",
  "keywords": ["string"],
  "location": "string | null",
  "employment_type": "string | null",
  "sources": ["greenhouse", "ashby", "lever"],
  "page": "integer >= 1, default 1",
  "max_results": "integer"
}
```

The current adapters only produce jobs when they can extract a company from query text such as `Senior AI Engineer at Anthropic`. The MCP tool should construct that internal query safely and call the jobs engines explicitly, instead of asking the agent to know the `at`/`for`/`@` extraction convention. It must clearly report that current adapters return title, URL, location, salary or department where available, and source update time, but do not provide a full job description or cross-ATS global search.

This tool is a translation of the existing capability, not a promise of a new job-search backend. A future implementation may add normalized job filters, but it must not claim that the current adapters already support them.

#### `slopsearx_search_security`

Security and threat-intelligence entry point.

Input:

```json
{
  "query": "string, required",
  "evidence_types": ["vulnerability", "exposure", "reputation", "malware", "threat_intel", "exploit"],
  "engines": ["string"],
  "max_results": "integer",
  "page": "integer >= 1"
}
```

The tool resolves evidence types to live category/subcategory capability profiles and returns an explicit warning that results are search findings, not a complete security assessment. It must never imply that absence from the selected engines means absence of a vulnerability or exposure.

#### `slopsearx_search_science`

Research-oriented entry point.

Input:

```json
{
  "query": "string, required",
  "source_types": ["papers", "scholarly_index", "biomedical", "chemistry", "datasets", "general_reference"],
  "date_range": {"from": "date | null", "to": "date | null"},
  "engines": ["string"],
  "max_results": "integer",
  "page": "integer >= 1"
}
```

The tool should expose source provenance and engine coverage, but it must not manufacture peer-review status, study quality, or citation completeness from the normalized search result fields.

#### `slopsearx_start_research`

Asynchronous multi-query evidence gathering. This is a proposed workflow tool, not a wrapper around an existing endpoint.

Input includes `question`, strategy (`triangulate`, `broad`, `fresh`, or `counterevidence`), allowed scopes, maximum queries, maximum engines per query, result budget, deadline, and an idempotency key. It returns a job handle immediately. The job preserves partial results and exposes underlying SlopSearX query IDs.

#### `slopsearx_get_job`

Returns job state, progress by query and engine, warnings, result count, and an opaque results cursor. States are `queued`, `running`, `partial`, `succeeded`, `failed`, `cancelled`, or `expired`.

#### `slopsearx_cancel_job`

Best-effort cancellation. It stops undispatched work and reports whether already-completed evidence remains readable. It must not claim that in-flight upstream calls were interrupted if they were not.

#### `slopsearx_read_results`

Reads a stable page from a completed search snapshot or research job using an opaque, tenant-bound cursor. It never reruns the query. Pagination is over captured merged results, not a translation of the HTTP `pageno` parameter.

#### `slopsearx_read_result`

Expands one server-issued result ID into normalized metadata, full engine provenance, publication data, rank explanation, and diagnostics. It must not accept an arbitrary URL and must not imply that SlopSearX fetched or verified the full page content.

### 3.3 Discovery and explanation tools

#### `slopsearx_list_capabilities`

Returns a generated catalog from the live registry and configuration.

Input:

```json
{
  "family": "string | null",
  "category": "string | null",
  "include_disabled": "boolean, default false",
  "include_auth_requirements": "boolean, default true"
}
```

Output includes engine name, display name, type, categories, subcategories, enabled status, authentication requirement class (`none`, `optional`, `required`, `unknown`), supported scope hints, and known caveats. It must not return secret values or raw API keys.

This is the MCP replacement for making agents combine `/config`, prose documentation, and assumptions about engine names.

#### `slopsearx_explain_search_scope`

Dry-run routing preview. This capability does not currently exist as an HTTP endpoint and should be implemented by extracting the selection logic from `server.py` into a reusable service.

Input:

```json
{
  "query": "string, required",
  "intent": "auto | string, default auto",
  "categories": ["string"],
  "engines": ["string"]
}
```

Output:

```json
{
  "selected_engines": ["string"],
  "excluded_engines": [{"engine": "string", "reason": "string"}],
  "routing_rule": "topic match | explicit category | explicit engine | tier-1 fallback | configured fallback",
  "matched_topic": "string | null",
  "warnings": ["string"]
}
```

This tool is important because first-match routing and tier fallback are otherwise invisible. It also gives an agent a chance to correct scope before spending rate limits.

#### `slopsearx_get_service_status`

Agent-facing operational status, derived from `/health` but with semantics corrected in the description.

Output distinguishes:

- server liveness;
- Valkey connectivity and whether fail-closed behavior is active;
- configured/active engine inventory;
- the fact that `/health` does not actively probe external APIs;
- a link or instruction to use search outcomes for passive engine health.

It must not describe every engine as externally healthy merely because `/health` reports `status: ok`.

### 3.3 Tools that should not be exposed as ordinary agent tools

- `/metrics` should remain an operator/Prometheus resource, not a general-purpose agent tool. Raw metrics are high-volume and are not a useful search action.
- Raw `/config` should not be exposed directly. `slopsearx_list_capabilities` is the typed, redacted replacement.
- YAML output should not be an MCP option. MCP already carries structured content; returning YAML would recreate the parsing problem.
- A generic `call_endpoint(path, params)` tool must not exist. It defeats descriptions, validation, safe defaults, and the distinction between search intent and operations.

## 5. Safety boundaries

The MCP layer must not treat every registered adapter as safe for generic discovery or unrestricted search. In particular:

- Sensitive security engines such as HIBP and DeHashed require one explicit
  operator grant (`MCP_TARGETED_SENSITIVE_ALLOWED`). A single shared policy gate
  applies it uniformly across every search path (generic explicit engines,
  targeted, jobs, security, science), so no path — including the generic
  explicit-engine route — can reach them without the grant. Generic routing,
  categories, and intent profiles never select them. Specialist grants
  (`MCP_GRANT_JOBS/SECURITY/SCIENCE`) enable their tools but do not grant
  sensitive access.
- Sensitive searches should not use the current shared cache and audit behavior without an explicit privacy design. Raw queries and client IPs are retained in Valkey audit streams.
- Strict SafeSearch must fail closed when selected engines cannot enforce it. It must never be reported as enforced merely because the HTTP parameter was accepted.
- Explicit engine selection is an advanced operation and should be allowlisted by capability and policy. Generic agents should select intent or source families.
- `read_result` and `read_results` must accept only server-issued handles and cursors, never arbitrary caller URLs. The MCP server is not an SSRF-capable page fetcher.
- Bound query length, fan-out, engine count, result count, wall-clock time, and research-job budgets.
- Never return API keys, raw upstream exception text, private client IPs, or unsanitized URLs.

## 6. MCP resources

Resources provide stable, inspectable context without forcing an agent to spend a tool call for every lookup.

Recommended resources:

- `slopsearx://capabilities` — current generated engine and category catalog, redacted
- `slopsearx://capabilities/{engine}` — one engine's metadata, caveats, and auth class
- `slopsearx://routing-profiles` — intent-to-category/profile definitions and their provenance
- `slopsearx://health/summary` — current server health with the active-health limitation clearly stated
- `slopsearx://metrics/summary` — bounded human-readable aggregate metrics, if an operator-authorized resource is needed; do not expose the unbounded Prometheus text by default

No resource should expose API keys, raw configuration secrets, private client IPs, or unbounded audit records. The existing audit logger is an operational subsystem, not an MCP data source.

## 5. MCP prompts

Prompts are optional but useful for repeatable agent workflows. They should be concise templates that invoke the tools above rather than embed engine knowledge.

- `research_with_source_coverage`: search broadly, inspect capability coverage, identify partial results, and return a source-diverse evidence set.
- `investigate_vulnerability`: search security sources, separate discovery from confirmation, and report missing source families.
- `find_company_jobs`: search ATS boards for a named company, preserve source and update timestamps, and flag when no board was found.
- `compare_package_or_project`: search package registries and developer sources, deduplicate by canonical URL, and report which sources responded.

Prompts should not claim that SlopSearX verifies facts or fetches page bodies. Its current contract returns search results and snippets; downstream retrieval remains a separate operation.

## 6. Error and partial-result contract

Every tool should return a typed result envelope, including successful empty results. The MCP server should map HTTP/internal conditions as follows:

| Condition | MCP behavior |
|---|---|
| Missing query | Structured invalid-input error naming `query` |
| Unknown engine/category | Structured invalid-scope error with valid alternatives from the capability catalog |
| Per-engine timeout, block, or rate limit | Successful partial result when at least one source responds; include engine outcome |
| All selected engines fail | Tool error with the query ID, selected scope, failure outcomes, and retry guidance; do not turn it into an empty authoritative answer |
| Valkey unavailable | Preserve the service's actual fail-open/fail-closed behavior and report cache/rate-limit caveat |
| Malformed upstream data | Per-engine error outcome; do not fail the entire search if other sources respond |
| Sensitive data in upstream exception | Sanitize before entering MCP text, logs, or error fields, reusing `sanitize_url()` and extending its secret patterns as needed |

The distinction between `partial: true`, `results: []` with healthy sources, and total failure must be explicit. Agents use those states differently.

## 7. Architecture recommendation

Do not build the MCP server as an HTTP client pointed back at `/search`. That would duplicate serialization, lose typed internal state, and make routing explanations impossible.

Instead:

1. Extract a `SearchService` from the body of `server.search()`.
2. Make the service accept a normalized `SearchRequest` and return a normalized `SearchResponse` containing results, scope, engine outcomes, cache metadata, and suggestions.
3. Keep FastAPI as an adapter from query parameters to `SearchRequest` and from `SearchResponse` to SearXNG JSON/YAML.
4. Make MCP a second adapter from typed tool arguments to `SearchRequest` and from `SearchResponse` to MCP structured content.
5. Extract engine-selection logic into a `ScopeResolver` that can perform both resolution and dry-run explanation.
6. Build the capability catalog from the runtime registry plus effective configuration, not from README tables.
7. Make intent profiles declarative and validated against active engine/category names at startup.
8. Keep MCP transport and tool registration in a separate package, for example `slopsearx/mcp/`, with no imports from the MCP layer back into FastAPI route code.

Suggested internal contracts:

```python
@dataclass
class SearchRequest:
    query: str
    categories: list[str] | None = None
    engines: list[str] | None = None
    language: str = "en"
    page: int = 1
    time_range: str | None = None
    safesearch: int = 0
    max_results: int | None = None
    include: set[str] = field(default_factory=set)

@dataclass
class ScopeDecision:
    selected_engines: list[str]
    resolved_categories: list[str]
    routing_rule: str
    matched_topic: str | None
    warnings: list[str]

@dataclass
class SearchResponse:
    query: str
    results: list[SearchResult]
    scope: ScopeDecision
    engine_outcomes: list[EngineOutcome]
    suggestions: list[str]
    query_id: str
    cached: bool
    response_time_ms: int
```

These are design targets. They should be introduced through tests and should not be copied verbatim until the current server behavior has been characterized with fixtures.

## 8. Verification plan

Before implementation is called complete:

1. Generate the route inventory from the live FastAPI app and assert the four product endpoints remain present.
2. Generate the engine catalog from the registry and assert it agrees with the MCP capability resource.
3. Add contract tests for every MCP tool with valid, empty, unknown-scope, partial-failure, all-failure, cache-hit, and rate-limit cases.
4. Assert that MCP and HTTP paths produce equivalent selected engines, ranking, deduplication, statuses, query IDs, and partial-failure semantics for the same normalized request.
5. Assert that MCP never emits YAML, API keys, raw client IPs, or unsanitized upstream URLs.
6. Test that explicit engines override categories and that the dry-run scope explanation matches actual dispatch.
7. Test jobs with a company name, without a company name, a missing board, an empty board, and per-ATS rate limiting. The current job adapters intentionally return no results when no company can be extracted.
8. Run the existing suite with the repository virtual environment. The system Python is missing at least `structlog`, so verification must use `./.venv/bin/python` or an explicitly provisioned environment.
9. Reconcile the documented 48-engine count against the live 51-engine registry before shipping generated catalog documentation.
10. Run `graphify update .` after code changes so the repository graph reflects the new MCP architecture.

## 9. Recommended implementation sequence

### Phase 1: seam extraction

- Add normalized request/response types.
- Extract scope resolution and search orchestration without changing HTTP behavior.
- Add characterization tests around the existing server pipeline.

### Phase 2: capability model

- Add runtime capability introspection with secret redaction.
- Define and validate intent profiles.
- Add `ScopeResolver.explain()`.

### Phase 3: MCP read/search surface

- Implement `slopsearx_search`, `slopsearx_search_targeted`, `slopsearx_list_capabilities`, `slopsearx_explain_search_scope`, and `slopsearx_get_service_status`.
- Add the jobs, security, and science tools only after their profiles have explicit output and limitation tests.
- Add resources and prompts after the tool contracts stabilize.

### Phase 4: operational hardening

- Add authentication and authorization at the MCP transport boundary appropriate to deployment.
- Bound result sizes and tool execution time.
- Preserve request IDs and query IDs across HTTP and MCP.
- Add per-tool metrics without exposing raw Prometheus data to agents.
- Verify graceful shutdown, concurrent requests, rate-limit behavior, and no secret leakage.

## 10. Open decisions for stakeholder validation

These are product decisions, not implementation details:

1. Is the MCP server intended to be colocated with SlopSearX or deployed as a remote gateway?
2. Should the initial server expose only search/discovery, or also operator-facing health resources?
3. Are security and medical searches allowed for general agents, or must those tools require an explicit policy grant?
4. Should `max_results` be a presentation limit only, or should the service gain a separate fetch-depth contract?
5. Is company-specific job search the required first specialist workflow, or should jobs remain an intent profile inside the general search tool?
6. Which authentication model and tenant boundary are required for production use?
7. What evidence retention, audit, and privacy guarantees are required for MCP calls?
8. Is a page-number interface sufficient, or is a cursor contract needed before MCP launch?

Until those decisions are answered, the proposed core is the safe default: one well-described general search tool, one explicit-scope search tool, capability and scope explanation, and specialist tools only where the underlying adapters already provide a meaningful semantic contract.
