# Rust rewrite case study and delivery contract

Status: approved destination and implementation backlog; implementation and measured outcomes remain pending.

Maintainer direction, 2026-10-15: plan a fully finished Rust rewrite, leave no Python backend, permit Python only for optional client-side use, and include native client replacements where beneficial. This document and its issue graph are the durable handoff for agents arriving without the originating conversation. Publishing the plan does not authorize live provider spend or production deployment.

## Baseline and evidence

Planning baseline: `2a995f964757df77693747b232aa47738c0c58cc`, fetched from upstream main and verified equal to local HEAD on 2026-10-15. The earlier assessment used upstream `326dc1f05a6dc7a6d0d1406301e0ce58e2b5fb82`; production `slopsearx/` and `engines/` were unchanged between that assessment and this planning baseline. The active checkout originally lagged on a September task branch. PR #877 added mandatory fresh-session synchronization; future agents must follow the current AGENTS.md before using this baseline.

Three specialist assessments inspected core/state, adapter/provider behavior, and protocols/identity/operations, followed by dependency reviews of this plan. Source inventories confirm 51 registered adapters and 38 MCP tools at the baseline. There are five prompts, three fixed resources and an engine-resource template. These are source observations, not passing-test counts. No Rust prototype, live provider campaign, performance benchmark or production rollout has been performed by this planning task. The graph report is navigation; current source and executable contracts remain authoritative.

The primary artifacts are this case study, [the machine-readable plan](plan.json), and [the issue index](ISSUES.md). Every issue links this document and identifies its plan ID, prerequisites, owning lane, source anchors, deliverable and executable acceptance evidence. The JSON graph is finish-to-start and acyclic. Native GitHub blocked-by relationships mirror it; parent/sub-issue links are organization only.

## What finished means

- Rust implements the server, all adapters, shared search service, background workers, all HTTP formats, MCP server/gateway and identity/state behavior. Valkey remains the only shared-state authority.
- Native `ssx` and MCP gateway binaries ship, so ordinary clients need no Python installation. Browser HTML/CSS/JavaScript remain appropriate client assets.
- No Python backend packages, subprocess adapters, embedded CPython/PyO3 bridge, sidecar or remote Python fallback remain in the delivered runtime. No active deployment, scheduled worker or maintenance command executes the old backend.
- Mandatory backend build, test, health, deployment, migration, active acquisition, evidence verification and release commands are native Rust or ordinary shell. Optional Python analysis/wire clients can remain separately packaged and must neither import the old backend nor be a prerequisite for mandatory native release gates.
- Historical evidence records retain their original bytes and schema meaning. Runnable legacy backend source is recoverable from Git history, not retained as an active implementation directory. Historical non-executable captures are explicitly inventoried rather than deleted to satisfy a filename scan.
- Existing capabilities remain available unless the maintainer separately accepts a documented contract change. The rewrite cannot claim completeness by dropping CSV/RSS, SSE, specialist filters, authenticated workflows or less-used adapters.
- Final acceptance uses exact release artifacts after Python removal, deployed process/schedule evidence, full parity/security/browser/state checks, and an explicit remaining-client inventory. A merged port or an empty milestone alone is not completion evidence.

External Jev/search/identity services are dependencies, not source inside this repository to rewrite. External CI service implementation languages are similarly outside scope, but project-owned Python invocations and backend installation steps in CI must be replaced. Protected review publication must keep its provenance and trust boundary; deleting a security job is not migration.

## Feasibility and architectural direction

The rewrite is technically feasible. The costly work is preserving behavior accumulated across providers, distributed state and public protocols. Async fanout itself is not the difficult part. A staged replacement can end in a complete Rust runtime; Python may temporarily serve as a pinned comparison oracle outside Rust execution, with explicit retirement tickets.

Use a small Cargo workspace with domain/DTOs, shared application services, infrastructure/adapters, HTTP/MCP surfaces and native executables. F02 owns the final crate/API ADRs and pins. Candidate foundations are Tokio, Axum/Tower, reqwest, Serde/schema tooling, HTML5/XML parsers and the official rmcp SDK. Their availability is not proof of compatibility. See [Tokio lifecycle guidance](https://tokio.rs/tokio/topics/shutdown), [Axum middleware](https://docs.rs/axum/latest/axum/middleware/), [reqwest Client](https://docs.rs/reqwest/latest/reqwest/struct.Client.html), [Serde representations](https://serde.rs/enum-representations.html), and [official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk).

The SDK transport documentation excludes legacy HTTP+SSE, while the current application accepts SSE. F02 must preserve that transport or record a separately approved retirement; T01 must implement the accepted set and cannot quietly narrow it. MCP OAuth client support does not replace the current FastMCP authorization-server integration. T04/T05 own its state and endpoint implementation. OIDC relying-party verification is a different responsibility, owned by T09.

Do not bundle a database replacement, microservice split, frontend-framework rewrite, embedded model runtime or new relevance policy into the language migration. Keep the shared service and policy gate. Rust ownership cannot replace Valkey fencing across replicas or guarantee exactly-once remote provider execution after an ambiguous timeout.

## Contracts that must survive

| Contract family | Important behavior | Work owners |
| --- | --- | --- |
| Models and serialization | Missing/null/empty distinctions; sorted engine provenance; media/domain payloads; bounded internal scholarly work groups; legacy durable readers | F05 |
| Catalog/config | Environment/file/default precedence; opt-in defaults; credential redaction; categories and audited filter layers; Jev metadata | F06 |
| Search authority | One fail-closed sensitive-engine gate; atomic mixed-list denial; strict SafeSearch predispatch rejection; truthful local/upstream enforcement | F09 |
| Execution | Scope precedence, deadlines including queue time, partial outcomes, synthetic versus observed health, singleflight waiter isolation and shutdown | F10–F15 |
| Retrieval quality | Presence/RRF, optional Jev routing/reranking, scholarly identity/version grouping, bounded provider advice, read-boundary advisories | F11, F12, F15 |
| Cached evidence | Full canonical unsliced responses; per-request views; stable snapshots/IDs; logical expiry versus retention; storage unavailable versus unknown | F07, F13 |
| Durable workflows | Conserved budgets, leases and fenced commits; immutable prior attempts; retries/cancellation; saved schedules/baselines/outbox; staged plans/index recovery | W00–W06, W11 |
| Evidence composition | Receipt idempotency/horizons, manifests, lineage and dependency dossiers without claiming search fetched/verified documents | W07–W09 |
| Public surfaces | GET/form POST; JSON/YAML/HTML/CSV/RSS; readiness/config/metrics; complete MCP schemas/resources/prompts/transports | T01–T06, W09 |
| Identity and browser | OAuth PKCE/rotation/revocation; OIDC/session/membership/tenant controls; CSRF/origin/idempotency; public search independent of cookies | T04, T05, T09, W10 |
| Delivery | Native gateway/CLI; monitoring/privacy; platforms/containers; release identity; active tools; deployment/migration/rollback | T07, T08, O01–O09, G01–G09 |

Current source already includes semantic reranking, scholarly grouping and caller-directed research planning. Current gateway fixes preserve full input schemas and own upstream session lifetimes. They are parity requirements, not benefits newly created by Rust. Reranking accepts a permutation of supplied IDs rather than provider-written content; sensitive evidence is excluded. Scholarly grouping distinguishes identity and versions and refuses lossy merges. Planning preserves exact identifiers and requires admitted parent evidence; caller judgment still controls research sufficiency.

Cross-language details deserve explicit fixtures: durable artifact IDs and staged plan digests depend on particular JSON escaping/ordering, not merely equivalent decoded values. Unicode casefold is not lowercase. Persisted wall-clock expiry is not a process-local monotonic deadline. Internal runtime-specific cache fingerprints can be versioned under an ADR; durable references need compatible bytes or an explicit migration.

## Adapter case studies

The 14 batches A01–A14 cover exactly 51 unique adapters; Europe PMC is included and Repology is not. Each batch uses actual Rust parsers against injected HTTP exchanges, not stubs that return expected output. Fixtures assert outbound methods/URLs/headers/auth placement/request order and count, as well as normalized results. Capture provenance distinguishes synthetic regression cases from sanitized upstream recordings. Only three general upstream capture files and two Europe PMC capture files exist at the baseline; other coverage must be exported from inline mocks and regression tests.

- DuckDuckGo demonstrates why connection reuse and isolated cookies must coexist. HTML5 parsing may construct different malformed-page trees than lxml. Bootstrap, fallback and challenge handling are behavior, not just selectors.
- PubMed demonstrates an aggregate deadline across requests, parsing and normalization. Rust CPU work must still observe the budget.
- Europe PMC demonstrates attribution and preprint notices surviving deduplication, compact MCP projection and portal rendering. It remains opt-in and metadata/lite-only.
- Internet Archive demonstrates that a closest-snapshot fallback is not historical completeness. Fallback count, remaining budget and trusted URL validation remain explicit.
- ExploitDB demonstrates that an adapter classified as scraping can primarily consume DataTables JSON. CVE filters, response counts and HTML fallback must be preserved.
- Sensitive intelligence demonstrates that typed scope boundaries complement rather than replace shared authorization and synthetic/private-data-safe fixtures.

F06 provides engine-local metadata fragments or a single integration owner for shared aggregates. A01 proves the end-to-end adapter development/replay workflow with npm/crates before the remaining 13 batches start in parallel. Batch agents own disjoint files; they must not independently edit common transport, registry, Jev or documentation aggregates.

## Improvements deliberately enabled by the rewrite

| Opportunity | Mechanism | Evidence required / owner |
| --- | --- | --- |
| Invalid-state prevention | Typed IDs/lease tokens/authority and mutually exclusive lifecycle enums; internal types separated from DTOs | Compile-time/API boundaries plus malformed/denial fixtures; F05/F09 |
| Less result copying | Immutable reference-counted canonical results with per-caller views rather than payload round-trip cloning | Isolation tests and retained-memory/allocation measurements; F13/F14/G03 |
| Explicit lifecycle ownership | Supervised tasks, bounded queues, cancellation and joined shutdown | Last-waiter/disconnect/crash tests; F03/F14/W02/W04/W06/O01 |
| Authoritative storage errors | Separate best-effort cache interfaces from durable unavailable/missing outcomes | Real-Valkey outage/fault tests; F07/F13/T04 |
| Native distribution | Compiled server/worker/gateway/CLI and runtime without interpreter dependencies | Clean install/image/process inspection; O04/G06/G08 |
| Distributed quota correction | Server-authoritative time plus atomic increment/expiry | Skewed-clock and interrupted-operation tests; F10 |
| Maintained cryptographic verification | Replace hand-written portal RS256 implementation while preserving claims/transaction rules | Controlled IdP and negative security corpus; T09/G02 |
| Atomic OAuth state | Single-use code/refresh transitions and explicit durable outage policy | Competing-instance/replay tests; T04/T05 |

Issue #1049 restores the original positive page domain through the native shared
request, cache and HTTP paths. Its one explicit portal hardening keeps the source
4,300-digit admission limit but disables Next visibly and accessibly when its
exact successor would need 4,301 digits. Python instead fails while formatting
that neighbor. Current/previous values remain exact; no wrapped, clamped or
invalid link is emitted. This compatibility delta requires its own real native
HTTP control and does not count as unchanged source-corpus parity.
The frozen 421-row native inventory keeps its historical three 400/no-dispatch
records. The current qualification layer retires those deviations by requiring
exactly the original source-backed 200/one-dispatch observations for those three
IDs and source hashes; the other 418 rows remain unchanged.

The last three are worthwhile independent of Rust. Record them as intentional hardening changes, not silently accepted parity differences. Existing source suggests risks, not demonstrated incidents. Rust does not by itself improve search relevance, provider quotas, upstream availability, filter enforcement or CAPTCHA handling. Quality changes require their own evidence and must not hide within a port.

## Sequence, readiness and parallel work

The issue graph uses hard finish-to-start prerequisites. A prerequisite is satisfied only when its decision/code is accepted and merged and its promised evidence/handoff is available—not merely when someone closes an issue. A cold agent reads this document, F01 manifests, its source anchors and predecessor handoffs before implementing. If an interface changes, update the graph/contracts and consumers before fan-out. New implementation starts from freshly fetched main per AGENTS.md.

1. **Contract and native foundation:** F01 → F02 → F03 → F04 → F05. Inventory includes all hidden Python paths; ADRs settle crate/storage/transport decisions; the portable oracle must reject deliberate differences.
2. **Shared primitives:** config/catalog, Valkey ownership, HTTP sessions and pure ranking can proceed concurrently. Policy and rate limiting then feed routing/cache/execution. T01 protocol qualification starts as soon as F04 exists, independently of real adapters.
3. **Adapter fan-out:** F06/F08/F04 → A01 pilot → A02–A14 in parallel. Sensitive batch A13 additionally waits for F09. No batch waits for the whole browser UI.
4. **Service and surfaces:** F14/F15 release HTTP/MCP integration. Machine OAuth, browser OIDC, evidence/state stores and observability have distinct lanes. Native gateway and ssx follow working wire boundaries.
5. **Durable features:** saved definitions W00 and staged admission W11 can proceed beside research records W01. Their runners consume the shared service; event publication and composition integrate only after the required stores exist. Workflow MCP and protected portal are later integration points.
6. **Tooling and release work:** O03 starts early to define acquisition/evidence replacements. O07/O08 implement them. O06 adds native CI/review successors; O09 switches mandatory gates only after all replacements exist. Existing Python gates are not removed early.
7. **Qualification and cutover:** G01 full parity and G02 security/fault qualification → G03 measured outcomes; G04 data rehearsal runs alongside these once state producers exist. G05 runbooks consolidate artifacts and migration. G06 staging → G07 authorized canary/full cutover → G08 source/runtime retirement → G09 final acceptance.

Likely schedule drivers are protocol/OAuth compatibility, distributed workflow recovery, and the slowest adapter/tooling batch. The graph identifies mandatory paths, not a calculated calendar critical path: no durations or staffing commitments have been measured. Estimate using the A01 pilot, T01 protocol qualification and first durable recovery slice rather than line counts. There are no invented due dates or user assignments.

Recommended lanes have accountable owner roles: contracts/integration lead, search-core owner, state/workflow owner, protocol/identity owner, portal/client owner, adapter-batch owners and release/quality owner. Roles become named assignees when execution starts. Keep shared interface files single-owned, work in isolated worktrees, and merge small prerequisite PRs before dependent implementation. The tracker shows which issues become ready after each handoff; it does not authorize speculative parallel edits across shared foundations.

## Verification and release gates

F03 supplies documented native `cargo xtask check <plan-id>` entrypoints; F04 defines machine-readable evidence and nonzero mismatch behavior. Implementation issues must fill concrete commands/crate paths as interfaces land. At minimum run scoped tests plus `cargo fmt --check` and the agreed Clippy/locked build checks. A green wrapper that skips missing cases is a failure. Foundation/ADR issues use manifest/decision review until the native runner exists. Every PR updates parity and intentional-difference ledgers and identifies portal impact.

G01 exercises all capability paths through real native transports and exact release artifacts. G02 uses disposable real Valkey and multiple processes, not only in-memory mocks, for claims, rotation, stale-owner writes and recovery. G03 freezes metrics, useful-effect thresholds, sample plan, budgets and stopping rules before measurement; report RSS/CPU/local latency/throughput/startup separately from upstream waiting and retain negative outcomes.

Rollout stages and their durations are defined before execution in G05: offline replay, isolated staging, bounded canary and full traffic, then a declared rollback observation window. Stage windows/thresholds are inputs requiring recorded owner acceptance, not values invented by this plan. Each stage records exact artifacts, failure criteria and recovery commands. G07 requires explicit production authorization for that concrete packet. This planning task does not deploy.

Do not duplicate live provider requests or scheduled mutations for shadow comparisons. Prefer recorded replay; any live campaign has explicit scope/budget. State migration inventories records, indexes, leases, due queues, idempotency, tombstones, cursors, horizons, session/OAuth records and TTL semantics. Choose compatible mixed-version storage or drain-and-migrate. Do not run incompatible owners against one authoritative namespace. Test rollback readers for records created during the Rust canary. Unknown remote request outcomes remain explicit and charged; claim single valid ownership and duplicate-commit prevention, not exactly-once remote execution.

## Risks and decisions that must close

| Risk / decision | Owner | Must resolve before | Evidence / response |
| --- | --- | --- | --- |
| Protocol/SSE and schema mismatch | Protocol owner | T01 completion, T05/T06 start | Pinned-crate real-client descriptor/transport tests; approved F02 amendment for any retirement |
| State record/ID incompatibility | State owner | G04 completion, G06 start | Byte fixtures, namespace inventory, interrupted migration and reverse-read rehearsal |
| Parser/provider behavioral drift | Adapter owners | Batch completion, G01 | Outbound+normalized replay, current-source regression mapping; bounded authorized live qualification if needed |
| Auth or tenant regression | Identity owner | G02/G06 | Multi-instance replay/denial matrix; maintained crypto and preserved policy |
| Hidden Python in tooling/CI | Tooling/release owner | O09/G08/G09 | Per-file/executable inventory and Python-free clean-environment checks with no remote fallback |
| Main changes during migration | Integration lead | Every merge and G09 | Refresh baseline, classify/reconcile additions; freeze a release candidate deliberately |
| Parallel interface conflicts | Foundation owners | Adapter/workflow fan-out | Stable published contracts, engine-local metadata, explicit shared-file ownership |
| Unproven performance benefit | Quality owner | G03/G06 | Preregistered comparison; preserve correctness gates and report inconclusive results honestly |
| External review integration migration | Tooling/security owner | O09 | Keep trust/provenance/permissions; no deletion of protection as a shortcut |

## Completion evidence and case-study updates

Each completed issue appends or links an evidence record containing plan/issue ID, baseline and implementation SHA, artifacts, tested commands/results, intentional differences, failure/negative results and downstream handoff. Keep historical measurements immutable. The final case-study update reports observed benefits, costs, regressions and operational migration results after G09; this initial document does not pretend those outcomes already exist.

The epic remains open until G09 verifies the final native release and every capability/removal requirement. The Rust Rewrite milestone contains all issues created for this plan; prerequisite edges, titles and milestone membership are read back after publication. Publication verification is a planning deliverable, not evidence that implementation is complete.
