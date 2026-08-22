# PRD: Full-Strength Agent Access to SlopSearX through MCP

Status: Mission-ready draft
Owner: SlopSearX maintainers
Implementation mission: Factory Droid, with independent verification by Jasper
Primary audience: SlopSearX maintainers and the implementation agent

## 1. Elevator pitch

SlopSearX MCP should give an AI agent the full useful power of SlopSearX without forcing the agent to learn SearXNG-compatible query strings, engine names, transport details, or undocumented caveats. It should preserve the agent-friendly strengths already present, including intent-based routing, source-scope explanation, stable snapshots, provenance, specialist searches, and bounded research, while recovering the evidence, control, diagnostics, and policy guarantees that are currently lost or weakened at the MCP boundary.

This is not a request to mirror every REST parameter. It is a request to make MCP a complete, trustworthy translation of SlopSearX's agent-relevant capability surface. The HTTP API remains the compatibility boundary for existing SearXNG consumers. MCP becomes the high-integrity agent boundary.

## 2. Problem statement

An agent can currently use SlopSearX successfully for ordinary searches, but it cannot reliably use the service at full strength. The MCP server gives it a concise search envelope, yet that concision is sometimes lossy rather than progressive. Answers, corrections, infoboxes, full normalized result content, media fields, result counts, and some diagnostic data exist in the underlying search pipeline but do not consistently reach the agent. The apparent "expand" operation returns provenance and the same short snippet, not a fuller result.

The agent also cannot always trust the boundary's guarantees. Filter parameters may be accepted but unenforced. Cache representation can depend on which request populated the cache. Snapshot serialization can corrupt engine provenance when a set crosses the JSON boundary. Sensitive-engine policy is enforced in the targeted path but not uniformly across explicit-engine paths. These are not merely missing conveniences: they can cause an agent to make a decision from incomplete evidence, believe a filter was applied when it was not, or cross a policy boundary unexpectedly.

The result is an uneven product. MCP is strong enough for a guided search assistant, but not yet strong enough for an autonomous research agent that must select evidence, preserve provenance, inspect gaps, retry intelligently, and explain what happened. Full-strength access means that the MCP surface must be at least as informative and policy-honest as the underlying service, while remaining bounded and agent-oriented.

## 3. Evidence boundary

This PRD is grounded in the current repository and the deployed MCP behavior observed during verification. The following are confirmed repository facts unless explicitly marked as proposed:

- The runtime adapter registry contains 51 engines, including Ashby, Greenhouse, and Lever. `docs/MCP_SERVER_DESIGN.md` notes that some prose documentation still says 48.
- The HTTP product surface includes `/search`, `/health`, `/config`, and `/metrics`, in addition to framework documentation routes.
- `/search` supports query, format, categories, explicit engines, language, page, time range, and SafeSearch inputs.
- The current MCP surface advertises 13 tools, 3 resource families, and 4 workflow prompts.
- The current MCP search result normalizer in `slopsearx/mcp/tools.py:_result_to_dict` emits title, URL, a 300-character snippet, source engines, source count, primary engine, category, publication date, score, position, tier, and a citation.
- The current MCP envelope in `slopsearx/mcp/tools.py:_envelope` emits scope, engine outcomes, metadata, suggestions, and warnings, but does not emit the response's answers, corrections, or infoboxes.
- `SearchResponse` in `slopsearx/service.py` carries suggestions, answers, corrections, and infoboxes.
- `slopsearx_read_result` calls `_result_to_dict`, then adds provenance and a note that the full page was not fetched. It does not currently return complete normalized content.
- `slopsearx_read_results` reads an immutable server-side snapshot and does not rerun the query.
- Research jobs are bounded, Valkey-backed, idempotency-aware, and cancellable on a best-effort basis. In-flight upstream calls are not interrupted.
- Language and time-range filters currently generate explicit warnings that no adapter consumes them. Strict SafeSearch fails closed; moderate SafeSearch warns that no adapter enforces it.
- `slopsearx/service.py:_scope_cache_key` currently includes query, language, SafeSearch, categories, engines, page, and time range, but not the requested include/representation set. This must be verified against the exact target branch before implementation because documentation says cache scoping was repaired while the current source still shows this gap.
- `slopsearx/service.py:search_response_to_payload` uses `dataclasses.asdict`, while `SearchResult.engines` is a set and rehydration assumes an iterable list. The JSON serialization path must be verified and repaired rather than assumed safe.
- `_sensitive_check` is visibly called by targeted search, but the general explicit-engine path must be audited and policy-tested so every route reaches the same boundary.
- The live deployed MCP service was verified to initialize over streamable HTTP, discover 13 tools, and execute `slopsearx_search`. The deployed security grant was intentionally disabled during initial wiring; deployment policy is separate from product completeness.

Where the PRD says "must," it defines the desired product contract. Where it says "proposed," it defines a design direction that Factory Droid must implement only after confirming the source and tests.

## 4. Vision

When this work is complete, an agent can treat SlopSearX as a trustworthy evidence service rather than a search-shaped black box. It can discover the live capability catalog, preview routing, request a bounded search, receive enough evidence to decide whether to go deeper, inspect complete normalized results on demand, understand what was and was not enforced, preserve stable citations across pages and sessions, retry only failed work, and obtain operational explanations without exposing secrets or unsafe control surfaces.

The agent should never have to guess whether a result is a snippet or a full body, whether a requested filter was applied, whether a source failed, whether a score means relevance, whether a result came from a stable snapshot, or whether a specialist tool is disabled by policy. Those facts should be explicit, typed, and cheap to consume.

## 5. Target audience

### Primary users

- AI agents performing web, technical, scientific, job, medical, finance, media, legal, geography, package, or security-oriented research.
- Agent hosts such as Hermes Agent that need a remote, authenticated, discoverable search capability.
- Developers building agent workflows that need evidence provenance and bounded multi-step research.

### Secondary users

- Operators responsible for configuring engine access, policy grants, credentials, rate limits, and service health.
- Human maintainers reviewing agent-generated research and needing to reproduce its search scope and source outcomes.
- Existing SearXNG-compatible clients, which remain served by the HTTP API rather than the MCP contract.

### Not for

- Replacing the HTTP API for existing SearXNG integrations.
- Turning MCP into an unrestricted proxy for arbitrary HTTP fetching.
- Claiming that search snippets are verified facts.
- Giving agents unrestricted access to sensitive intelligence engines or secrets.
- Exposing raw Prometheus, audit, credential, or internal-control surfaces as ordinary agent tools.

## 6. User stories

### Capability discovery and planning

- As a research agent, I want to discover the live engine catalog and each engine's supported evidence types, so that I can choose an appropriate source boundary without memorizing implementation names.
- As a research agent, I want to preview which engines and categories will be used and why, so that I can correct an overly broad or irrelevant plan before spending rate limits.
- As an agent operator, I want capability metadata to distinguish available, configured, credentialed, disabled, sensitive, and degraded engines, so that an agent does not confuse existence with usable access.

### Search and evidence retrieval

- As a research agent, I want to search using a natural research intent, so that I can use SlopSearX without constructing REST query strings.
- As a research agent, I want to request a compact first page, so that I pay only for evidence needed to choose the next action.
- As a research agent, I want to request a complete normalized result when a citation deserves inspection, so that expansion adds information rather than repeating the same snippet.
- As a research agent, I want access to answer boxes, corrections, infoboxes, suggestions, media references, result counts, and engine diagnostics when they exist, so that the MCP surface does not silently discard useful search output.
- As a research agent, I want stable pagination over a captured result set, so that page two does not silently represent a different search than page one.
- As a research agent, I want compact citation identifiers and on-demand source cards, so that I can preserve provenance without carrying every field through the entire reasoning context.

### Scope, filters, and policy

- As a research agent, I want every requested filter to report whether it was enforced, partially enforced, unsupported, or rejected, so that I never treat a warning as proof of filtering.
- As an agent operator, I want sensitive-engine restrictions enforced consistently across every search entry point, so that an explicit-engine loophole cannot bypass policy.
- As a research agent, I want a deliberate advanced-search operation with typed fields, so that I can use legitimate precision without falling back to arbitrary URLs or undocumented parameters.
- As an operator, I want specialist grants and sensitive access to remain fail-closed by default, so that fuller capability does not mean uncontrolled capability.

### Degradation and operations

- As a research agent, I want per-engine outcomes with machine-readable failure classes, so that I can distinguish no results, timeout, authentication failure, circuit breaking, rate limiting, and malformed upstream data.
- As a research agent, I want to retry only failed or empty source families, so that I do not repeat successful work or waste context.
- As an operator, I want a non-secret health and quality view, so that I can understand whether an apparent research gap is caused by the query or by service degradation.
- As a research agent, I want service version and capability compatibility information from the MCP server itself, so that I do not rely on an inconsistent `/health` version field.

### Multi-query research

- As a research agent, I want to start bounded research with explicit budgets and an idempotency key, so that repeated requests do not create duplicate jobs.
- As a research agent, I want to inspect intermediate coverage and failure gaps, so that I can steer or extend research before synthesis.
- As a research agent, I want to retry failed subqueries or add a bounded follow-up query, so that a partial job can converge without restarting successful work.
- As an operator, I want cancellation to preserve completed evidence while making the remaining state explicit, so that stopping work does not destroy useful results.

## 7. Core concepts

### Search request
A user-intent-bearing request containing the question, desired evidence intent, scope, filters, result detail, and bounded presentation limits. It is not a raw URL query string.

### Evidence scope
The declared set of source families or named engines the agent intends to use, including the reason for selection and any exclusions.

### Capability
A live description of what an engine or source family can provide, including categories, authentication state, sensitivity, supported filters, result fields, caveats, and operational status.

### Enforcement report
A machine-readable statement of which requested constraints were enforced, partially enforced, unsupported, or rejected.

### Search snapshot
An immutable, server-owned capture of a merged result set and its provenance. A snapshot has an opaque handle, a retention boundary, and stable pagination semantics.

### Result card
The compact, token-efficient representation returned in a search page. It contains enough information for triage and a stable identifier for expansion.

### Result record
The fuller normalized representation of one result, including complete available content, media fields, source provenance, ranking explanation, and enforcement context. It does not imply that SlopSearX fetched or verified the linked page.

### Citation
A stable reference to a result, consisting of a human-readable label, URL, source identity, and the snapshot/query context needed to reproduce how it entered the result set.

### Engine outcome
The result of one selected engine attempt, classified separately from the merged result set. It records status, count, latency where available, message, and failure class.

### Research job
A bounded asynchronous collection of planned queries, their snapshots, source coverage, failures, and lifecycle state.

### Research coverage
The relationship between the intended evidence boundary and what actually answered. Coverage must distinguish source families attempted, successful, empty, failed, unavailable, and not selected.

### Specialist grant
An explicit operator permission enabling a source family or tool with elevated privacy, sensitivity, cost, or misuse risk.

### Advanced search
A typed, policy-checked request for precision beyond ordinary intent routing. It is not arbitrary REST passthrough and cannot bypass policy or bounds.

## 8. Product principles

1. **Agent intent over transport parity.** MCP exposes what an agent is trying to accomplish, not a mechanically renamed list of REST parameters.
2. **Progressive disclosure without information loss.** Compact output is the default, but deeper operations must reveal more, never merely repeat the same truncated representation.
3. **Evidence boundaries are explicit.** Every result set identifies selected sources, answering sources, failed sources, unsupported constraints, cache/snapshot state, and ranking semantics.
4. **No silent enforcement claims.** A requested filter is either enforced, partially enforced, unsupported with a structured explanation, or rejected. Warnings alone are insufficient for machine decisions.
5. **Policy is uniform.** Sensitive engines, specialist grants, authentication, rate limits, and output restrictions apply consistently across all tools and alternate paths.
6. **Stable evidence beats repeated work.** Pagination, expansion, retries, and research follow-ups operate on server-owned state where that reduces reruns and context cost.
7. **The server accumulates state; the agent carries decisions.** The agent should send compact commands and receive bounded evidence, not carry every intermediate page through its context window.
8. **Search is not verification.** Snippets, scores, source counts, and cross-engine presence are leads and provenance signals, not truth or relevance confidence.
9. **Least privilege includes observability.** Operational diagnostics expose enough to explain behavior but not credentials, raw audit data, unrestricted metrics, or arbitrary fetch capability.
10. **Degraded results remain honest.** Partial success is useful only when the missing coverage is visible and machine-actionable.
11. **Correctness before parity.** Do not add a new capability on top of corrupted snapshots, ambiguous cache entries, or inconsistent authorization.
12. **The live registry is authoritative.** Capability catalogs derive from runtime registration and effective configuration, not stale prose counts.

## 9. Desired capability surface

The existing tools remain the base surface. The following describes the complete destination, not a requirement to preserve every current field name unchanged.

### 9.1 Core search

`slopsearx_search` remains the primary entry point. It must support:

- Natural query and intent.
- Automatic routing or declared source scope.
- Bounded result count.
- Compact result cards.
- Explicit detail selection.
- Filter enforcement reporting.
- Snapshot/cursor issuance.
- Suggestions when requested and available.
- Answers, corrections, infoboxes, and result-level metadata when available.
- Honest partial and cache state.

### 9.2 Deliberate scope search

`slopsearx_search_targeted` remains the explicit evidence-boundary operation. It must apply the same policy checks as every other tool, including sensitive-engine checks, grant checks, engine validation, credential state, and result bounds.

### 9.3 Specialist searches

Jobs, science, security, and any future specialist tool must have:

- An explicit enabled/disabled state.
- A declared evidence boundary.
- Source coverage and limitation metadata.
- Consistent filter semantics.
- No implicit privilege escalation through generic search.

Security access may remain disabled in a deployment. Product completeness means the boundary is correct and discoverable, not that every deployment must grant it.

### 9.4 Capability catalog and resources

The live capability catalog should expose, per engine or source family:

- Stable name and display name.
- Category and intent families.
- Enabled/disabled state.
- Credential required/configured/usable state without revealing credentials.
- Sensitive classification.
- Supported filters: language, time range, SafeSearch, pagination.
- Supported result types: text, answers, corrections, infoboxes, media, structured fields.
- Typical failure classes and known caveats.
- Rate-limit or cost class where policy permits disclosure.
- Last-known operational state and freshness.

Resources should remain read-only and cacheable. They must not expose raw environment variables, secrets, arbitrary internal configuration, or unrestricted audit records.

### 9.5 Full normalized result access

The product must provide a real expansion path. A result record should contain, when available:

- Complete normalized content, not only the 300-character triage snippet.
- Title, URL, primary engine, all contributing engines, category, publication date, tier, position, and score.
- Thumbnail, image source, and other safe media fields.
- Source-specific structured metadata where it can be normalized safely.
- Answers, corrections, or infobox membership when the result came from those structures.
- Citation and snapshot context.
- A clear statement that SlopSearX did not fetch or verify the linked page.

If full content is unavailable because an adapter only returns a snippet, the record must say so explicitly.

### 9.6 Advanced search

Add a typed advanced operation only if the existing tools cannot express a legitimate agent need. It should support a bounded schema for result-affecting choices, such as:

- Query.
- Named engines or source families.
- Categories.
- Freshness/time range.
- Language.
- SafeSearch mode.
- Requested output fields.
- Detail level.
- Maximum results.
- Requirements such as `requires_answers`, `requires_media`, or `requires_source_type` where the catalog can evaluate them.

It must reject unknown fields, arbitrary URLs, unbounded values, and policy-inconsistent engine selections. It must report which requested capabilities were met.

### 9.7 Stable snapshots and citations

Snapshot behavior must include:

- Opaque cursor and result IDs.
- TTL and expiration status.
- Stable page reads.
- Stable result expansion.
- Explicit snapshot identity in citations.
- Correct JSON-safe serialization of all fields, including engine collections.
- A documented behavior when Valkey is unavailable: either fail closed for snapshot-dependent operations or return a clearly marked non-persistent mode that does not pretend stable pagination exists.

### 9.8 Research jobs

Research jobs should preserve the existing bounded model and add:

- Per-query and per-engine coverage summaries.
- Structured failure classes.
- Retry of failed or empty subqueries without rerunning successful queries.
- A bounded follow-up or extension operation.
- Intermediate progress/events sufficient for an agent to decide whether to continue.
- Idempotent creation.
- Preserved completed evidence after cancellation.
- Explicit expiration and cleanup state.

### 9.9 Operational diagnostics

Expose a safe operational summary rather than raw `/metrics`. It may include:

- Service and MCP contract version.
- Valkey availability.
- Effective engine count.
- Enabled specialist grants by name, without secret values.
- Recent aggregate engine health by status class.
- Cache and snapshot availability.
- Current policy bounds.
- Degradation summary and freshness timestamp.

The exact metric fields are an implementation decision after inspecting the existing metrics contract. Do not expose an unbounded metrics dump as an ordinary agent response.

## 10. Success criteria

### Functional must-haves

- [ ] The MCP surface documents the complete agent-relevant SlopSearX capability model and distinguishes current deployment grants from product capability.
- [ ] Generic, targeted, specialist, advanced, and research paths share one auditable policy decision boundary.
- [ ] Sensitive engines cannot be reached through a path that bypasses the configured grant.
- [ ] Search responses expose all useful currently available normalized evidence or explicitly identify why a field is unavailable.
- [ ] Result expansion returns more than the compact triage card when more source data exists.
- [ ] Answers, corrections, infoboxes, suggestions, result counts, and engine outcomes have explicit MCP semantics.
- [ ] Every result-affecting filter reports enforcement status.
- [ ] Snapshot pagination and expansion survive serialization and preserve engine provenance.
- [ ] Cache entries cannot return a representation inconsistent with the request's requested fields or detail level.
- [ ] Advanced search, if needed, is typed, bounded, and policy-checked rather than arbitrary REST passthrough.
- [ ] Research jobs expose coverage gaps and can retry failed work without duplicating successful work.
- [ ] Safe operational diagnostics are available without leaking secrets or unrestricted internal data.
- [ ] Capability discovery reflects the runtime registry and effective configuration.

### Agent-native quality gates

- [ ] The default search response remains compact enough for triage and does not require the agent to receive full page content for every result.
- [ ] A complete result can be requested by stable ID without rerunning the search.
- [ ] Pagination uses compact commands and stable server state rather than requiring the agent to replay a query.
- [ ] A partial search clearly states which sources answered, failed, returned empty, were unavailable, or were not selected.
- [ ] A filter that was not enforced cannot be mistaken for an enforced filter by a structured consumer.
- [ ] The response schema distinguishes search result content, provenance, diagnostics, and service state.
- [ ] The agent can discover available capabilities without reading repository documentation or reverse-engineering REST routes.
- [ ] Research jobs have an explicit budget, deadline, idempotency behavior, cancellation behavior, and evidence-retention behavior.

### Verification gates

- [ ] Unit tests cover every new response field and every negative policy path.
- [ ] Integration tests run against deterministic local fixtures for cache, Valkey, snapshots, engine failures, and specialist grants.
- [ ] MCP protocol tests cover initialize, tool discovery, resource discovery, prompt discovery, authenticated calls, malformed input, and expired handles.
- [ ] Contract tests compare the MCP result model with the underlying `SearchResponse` so fields are not silently dropped.
- [ ] Serialization round trips are tested for sets, lists, optional fields, nested metadata, and empty values.
- [ ] Exact-head tests verify that the final implementation, not an earlier commit, satisfies the acceptance criteria.
- [ ] The full repository test suite passes, including the coverage gate.
- [ ] The deployed MCP endpoint is tested through the actual configured transport and authentication mode.
- [ ] No claim of full-strength access is made while known correctness or policy defects remain unresolved.

## 11. UX sketch

### Ordinary search

```text
Agent: Find current evidence about Python agent memory.

MCP: slopsearx_explain_search_scope
     intent: science + developer
     selected sources: arxiv, Semantic Scholar, OpenAlex, GitHub, Stack Exchange
     excluded: security sources, unavailable credentialed sources
     filters:
       time_range: requested=month, enforced=false, reason=adapter support unavailable

Agent: Proceed with the plan.

MCP: slopsearx_search
     summary cards: 10
     snapshot: snap_abc
     cursor: cur_abc
     coverage: 4 answered, 1 empty, 1 timeout
     detail available: full normalized records for 10 results

Agent: Expand result 3.

MCP: slopsearx_read_result
     complete normalized content: available
     source engines: arxiv, OpenAlex
     citation: stable
     page verification: not performed by SlopSearX
```

### Filter honesty

```text
Agent: Search only results from January 2026 and enforce strict SafeSearch.

MCP:
  rejected: strict SafeSearch cannot be enforced by the selected adapters
  or, for a non-strict request:
  filter report:
    time_range: unsupported
    language: enforced by 2 of 6 selected engines
    safesearch: enforced by 0 of 6 selected engines
```

### Partial search and retry

```text
MCP search result:
  successful: arxiv, OpenAlex, GitHub
  empty: Semantic Scholar
  timeout: Brave
  credential_missing: NVD

Agent: Retry only timeout and empty sources.

MCP: retry_search_work
  reused successful snapshot evidence
  new attempts: Semantic Scholar, Brave
  merged coverage and provenance preserved
```

### Bounded research

```text
Agent: Investigate whether claim X is supported, including counterevidence.

MCP: start_research(strategy=counterevidence, budget=bounded)
  job: job_123
  queries: 5
  deadline: explicit

MCP: get_job(job_123)
  completed: 3/5
  coverage gap: no primary-source results yet
  failed: 1 timeout
  cursors: [cur_1, cur_2, cur_3]

Agent: Retry the failed query and add one primary-source query within the remaining budget.
```

## 12. Non-goals

- One-to-one parity with every SearXNG or SlopSearX HTTP parameter.
- Arbitrary HTTP requests, URL fetching, page crawling, or SSRF-capable tools.
- Automatic truth verification of snippets or linked pages.
- Relevance-confidence claims derived from the current cross-engine presence score.
- Exposing engine API keys, raw audit records, or unrestricted Prometheus metrics.
- Making all specialist tools available in every deployment.
- Replacing source-specific adapters with an MCP-only abstraction.
- Adding a general-purpose web browser to the MCP server.
- Silently rewriting or normalizing away source limitations.
- A broad rewrite of the HTTP compatibility API unless a shared correctness defect requires it.
- A new persistent research knowledge base beyond the bounded snapshot and job lifecycle needed for this product.

## 13. Open questions

1. What is the canonical normalized full-content contract when different adapters provide different amounts and structures of content?
2. Should complete result content be returned inline, stored behind a second handle, or both under a strict token budget?
3. Which answer, correction, infobox, and suggestion structures are stable enough for the MCP contract?
4. Should media fields be returned by default in result cards, only in result records, or only through a media-specific expansion?
5. What is the precise retention policy for snapshots, result records, research jobs, and citations?
6. Should expired citations return a durable tombstone with metadata, or only an `expired_handle` error?
7. What failure taxonomy is sufficient for agents without exposing sensitive upstream details?
8. Which filter capabilities can be implemented consistently across adapters, and which must remain explicit unsupported states?
9. Is a retry operation better modeled as a new snapshot linked to the old one or as a revision of the existing snapshot?
10. Should advanced search be added immediately, or should richer `include`/`detail` semantics first prove that existing tools cover the need?
11. What non-secret operational metrics are safe and useful to expose to a remote agent?
12. Should capability metadata include estimated cost, latency, or rate-limit class, and who owns those values?
13. Does the current authorization model need tenant identity beyond a single bearer token before the service is exposed outside the trusted network?
14. Which specialist grants require separate audit events or human approval in production deployments?
15. What is the supported compatibility/version negotiation story when the MCP server and Hermes client evolve independently?
16. Which existing documentation claims, especially the 48-versus-51 engine count and cache-scoping statements, need correction as part of this work?

## 14. Ecosystem map

### SlopSearX HTTP API

The HTTP API remains the SearXNG-compatible boundary for existing clients. It is optimized for compatibility and returns JSON or YAML+Markdown. MCP should reuse its domain behavior where correct but should not inherit ambiguous representation, cache, or failure semantics.

### SlopSearX CLI

The `ssx` CLI provides search, engine listing, health, and config operations. It is useful for human operators and local inspection. MCP should provide typed equivalents for agents, with clearer scope and enforcement semantics.

### SlopSearX MCP server

MCP is the agent-native boundary. It should add orchestration, progressive disclosure, stable evidence handles, research lifecycle, capability discovery, and policy-aware translation.

### Hermes Agent

Hermes is a remote MCP consumer and SearXNG provider consumer. It needs compact structured outputs, authenticated transport, discoverable tools, and reliable semantics for partial results and evidence provenance.

### Engine adapters

Adapters are the authoritative source of actual engine behavior and limitations. MCP capability claims must derive from adapter behavior and tests, not only names or categories.

### Valkey

Valkey supports caching, snapshots, rate limiting, and research-job state. Its availability is part of the product behavior for stateful MCP operations and must be reported honestly.

## 15. Risks and mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---:|---:|---|
| Richer output overwhelms agent context | Medium | High | Progressive disclosure, result cards, requested fields, bounded pages, stable IDs |
| Full-content expansion is mistaken for page verification | Medium | High | Explicit provenance and non-verification note in every expanded record |
| Sensitive engine access bypasses policy | Medium | Critical | One shared policy gate, negative tests across every tool and explicit-engine path |
| Cache returns representation from a different request shape | Medium | High | Include representation/detail fields in identity or cache richest canonical response and derive views |
| Snapshot serialization corrupts provenance | Medium | High | JSON-safe canonical serialization and round-trip tests with sets and nested fields |
| Unsupported filters are treated as applied | High | High | Structured enforcement reports and rejection for mandatory constraints |
| More tools create routing ambiguity | Medium | Medium | Capability descriptions, plan-before-execute, clear entry-point rules, prompt examples |
| Research jobs consume unbounded resources | Low/Medium | High | Query, engine, result, deadline, concurrency, and total-cost bounds |
| Retry duplicates successful work | Medium | Medium | Immutable query evidence, retry only failed work, linked snapshots |
| Operational diagnostics leak private data | Medium | High | Curated non-secret schema, redaction tests, no raw audit or environment output |
| Runtime registry and docs drift again | High | Medium | Generate catalog from registry; contract test count and metadata; update docs from source |
| HTTP and MCP semantics diverge | Medium | High | Shared service model, cross-surface contract tests, explicit divergence documentation |
| Remote transport token is exposed | Medium | Critical | TLS/reverse proxy guidance, firewall restriction, secret-backed client config, rotation procedure |
| Agent treats source count or score as truth | Medium | High | Stable ranking explanation and explicit search-is-not-verification language |
| Implementation expands into an unrelated HTTP rewrite | Medium | Medium | Keep shared correctness fixes narrow; preserve HTTP compatibility contract |

## 16. Factory Droid mission brief

Implement the product described in this document in the SlopSearX repository.

### Mission objective

Make the SlopSearX MCP server a complete, trustworthy, agent-oriented translation of the service's useful search and research capabilities. Preserve the existing intent-level architecture. Do not turn MCP into a thin REST wrapper. Do not claim completion until the implementation and exact-head verification prove that the agent receives the evidence, policy semantics, stable state, and diagnostics described here.

### Required execution order

1. Inspect the current branch, source, tests, `docs/MCP_SERVER.md`, and `docs/MCP_SERVER_DESIGN.md`. Confirm every stated gap against the current source; do not blindly implement a stale diagnosis.
2. Establish a small contract inventory mapping each underlying `SearchResponse` field and relevant engine capability to its MCP representation, omission rationale, or explicit unsupported state.
3. Fix correctness and policy defects first: serialization, cache representation identity, uniform sensitive-engine enforcement, and filter/enforcement semantics.
4. Add the progressive-disclosure result contract and recover currently available evidence fields.
5. Add or justify the typed advanced-search capability only after measuring whether richer existing search/detail semantics cover the need.
6. Improve snapshot and research-job lifecycle behavior, including coverage gaps and selective retry where the source model supports it.
7. Add safe operational diagnostics and capability feature metadata without exposing secrets or arbitrary internal surfaces.
8. Update operator and agent documentation with authoritative current behavior, deployment differences, policy grants, and examples.
9. Run focused tests, integration tests, the full suite, coverage, graphify update, and exact-head verification. Report any remaining open question rather than silently choosing a high-impact policy.

### Implementation constraints

- Keep the MCP surface intent-level and model/provider agnostic.
- Reuse the shared service layer rather than making in-process HTTP calls.
- Preserve the HTTP compatibility boundary unless a narrowly scoped shared defect requires correction.
- Do not add arbitrary URL fetching or SSRF-capable behavior.
- Keep sensitive access fail-closed by default.
- Do not expose credentials, raw audit streams, or unrestricted metrics.
- Use deterministic local fixtures for engine, cache, Valkey, snapshot, and research tests. Do not make tests depend on live external engines.
- Preserve backward compatibility for existing tool names and reasonable response consumers, or document a versioned migration if a breaking response change is unavoidable.
- Do not commit, push, merge, or create a pull request. Return the implementation diff and verification evidence to the delegating agent.

### Required acceptance evidence

Factory Droid must return:

- A changed-file summary.
- A field-level MCP contract mapping.
- A list of correctness and policy defects confirmed and fixed.
- Tests for compact search, full result expansion, answers/corrections/infoboxes/suggestions, media, partial outcomes, unsupported filters, sensitive-engine bypasses, cache representation identity, snapshot round trips, expired handles, research retries, and diagnostics redaction.
- The exact commands run and their real outputs.
- The exact commit SHA or working-tree state tested, without implying a commit was created if none was authorized.
- Any unresolved open questions and the conservative behavior chosen for each.

### Definition of done

The mission is complete only when:

- An agent can discover the live capability surface.
- An agent can plan and execute ordinary and deliberate-scope searches.
- An agent can receive compact cards and genuinely expand a result.
- Useful underlying evidence is not silently discarded.
- Filter and policy enforcement are explicit and uniform.
- Stable snapshots preserve provenance across serialization and pagination.
- Cache behavior is independent of request history.
- Partial results and retryable failures are machine-actionable.
- Research jobs are bounded, inspectable, and preserve completed evidence.
- Safe operational diagnostics explain service behavior.
- The full test and quality gates pass at the exact tested head.
- Documentation matches the implementation and deployment controls.

## 17. Traceability matrix

| Gap | Product consequence | Required outcome | Primary evidence location |
|---|---|---|---|
| Compact result drops full content | Agent cannot deepen evidence without leaving MCP | Full normalized result expansion or explicit unavailable state | `slopsearx/mcp/tools.py:_result_to_dict`, `slopsearx/adapter.py` |
| Envelope drops answers/corrections/infoboxes | Agent loses useful search semantics | Typed response fields and tests | `slopsearx/service.py:SearchResponse`, `slopsearx/mcp/tools.py:_envelope` |
| Media fields omitted | Agent cannot use available visual/source context | Safe media fields in result record | `slopsearx/formatter.py`, adapter model |
| Filter enforcement weak | Agent may make invalid evidence claims | Enforcement report or fail-closed rejection | `slopsearx/mcp/tools.py` filter helpers, adapters |
| Sensitive check path inconsistency | Policy bypass | One shared policy gate | `slopsearx/mcp/tools.py`, policy tests |
| Snapshot set serialization | Provenance corruption | JSON-safe round trip | `slopsearx/service.py`, `slopsearx/snapshot.py`, cache tests |
| Cache representation identity incomplete | Response depends on request history | Canonical cache or complete identity | `slopsearx/service.py:_scope_cache_key`, cache tests |
| Research cancellation/retry coarse | Wasted work and poor steering | Selective retry/follow-up and coverage | `slopsearx/research.py`, MCP research tools |
| Catalog lacks feature semantics | Agent cannot choose engines intelligently | Live feature matrix | `slopsearx/capabilities.py`, adapters |
| No safe diagnostics | Agent cannot distinguish query gap from service gap | Curated operational summary | `slopsearx/server.py`, metrics/health code |

## 18. Final product judgment

The current MCP server has a sound foundation and a valuable agent-oriented shape. The problem is not that it lacks a REST endpoint for every feature. The problem is that the translation boundary currently loses some useful evidence and does not yet make every correctness and policy guarantee strong enough for autonomous use.

The target is therefore not "more tools" in the abstract. The target is a coherent contract in which every search decision has enough evidence, every limitation is explicit, every stateful handle is trustworthy, and every elevated capability is uniformly governed. That is what full-strength SlopSearX access means for an agent.
