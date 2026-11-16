# ADR 003: Rust workspace and contract boundaries

- **Status:** Proposed for F02 maintainer acceptance
- **Scope:** Rust Rewrite milestone, all native backend and client lanes
- **Decision authority:** Maintainer direction in the Rust rewrite case study and issue #881; acceptance is the merged F02 PR
- **Date:** 2026-10-15
- **Predecessor:** Accepted F01 inventory at `20109f9083babd014971951e8c02565ae51c2326`; F02 starts from merged F01 `2e0026b4e471c9309a898665169578333074015e`

## Context

The current Python `SearchService` and `ScopeResolver` own normalized search and scope decisions; HTTP and MCP call them rather than implementing separate pipelines. `SearchResult` is an internal model, while formatters and MCP projections make public representations. The adapter registry, capability catalog, one MCP policy gate, cache, snapshots, and durable workflows constrain where the Rust boundaries may sit. F01 inventories 51 adapters, 38 MCP tools, four resource registrations, five prompts, search formats, and native command replacements. An inventory row is a contract to implement and verify, not proof of native parity.

## Decision

Use one Cargo workspace and one deployable application, with narrowly separated library crates. Crate names are proposed stable API names for F03; moving one before implementation requires an F02 amendment and consumer review.

| Crate | Owns | May depend on | Must not own |
| --- | --- | --- | --- |
| `ssx-json` | Bounded lossless JSON values and pure number/object codecs shared by backend records and public DTOs | `serde`, `serde_json` with `raw_value` only, `ryu` float formatting, `indexmap` insertion-ordered JSON objects | Backend/domain policy, HTTP/MCP, Valkey, provider clients, configuration |
| `ssx-domain` | Internal value types, typed IDs, result/provenance, search request/outcome, explicit missing/expired/unavailable/error distinctions | Standard library, `ssx-json`, pure serialization traits | HTTP, MCP, Valkey, provider clients, operator config |
| `ssx-wire` | Versioned public HTTP/MCP DTOs, explicit codecs and golden contract schemas shared with native clients | `ssx-json` and pure serialization crates only | Domain/application policy, Valkey, backend configuration |
| `ssx-registry` | Engine-local descriptors and registered factories | `ssx-application` search port, standard library and pure serialization traits | Config loading, provider calls or orchestration |
| `ssx-config` | Layered operator configuration and redacted credentials | `ssx-registry`, pure `ssx-domain` codecs, serialization and YAML parsing | Domain-to-config dependencies, provider calls or policy grants |
| `ssx-application` | `SearchService`, scope, shared fail-closed MCP policy with surface-specific inputs, budgets, jobs/workflow commands, canonical response and per-request views | `ssx-domain`, abstract ports | Route parsing, socket/Valkey implementations, provider-specific wire DTOs |
| `ssx-infra` | Valkey stores, leases/scripts, clocks, HTTP client pools, parsers and 51 engine implementations | Domain/application ports | Public route status mapping or changing policy grants |
| `ssx-http` | Axum routes, form/query negotiation, JSON/YAML/HTML/CSV/RSS rendering, portal/OIDC boundary, error/status/security headers | Application and domain contracts; injected infra ports | Independent search, cache or authorization decisions |
| `ssx-mcp` | Tool/resource/prompt registry, stdio/streamable HTTP/SSE transport adapters, MCP OAuth boundary and projections | Application and domain contracts; injected infra ports | Bypassing shared policy or durable state ownership |
| `ssx-cli` | Native `ssx` HTTP client | `ssx-wire` public HTTP DTOs and an injected HTTP client | `ssx-domain`, `ssx-application`, backend internals |
| `ssx-gateway` | Native local stdio MCP proxy to remote streamable HTTP catalog and OAuth client flow | MCP protocol DTOs/client | Reimplementing remote tool logic or hiding remote schemas/errors |
| `ssx-server`, `ssx-worker` | Executable composition roots, config validation, startup/shutdown/readiness | All required libraries | Business decisions in `main` |

`ssx-domain` and `ssx-application` never depend on HTTP/MCP or infrastructure. `ssx-wire` contains only public representations and codecs and has no dependency on backend crates; server surfaces translate from domain outcomes into it. Client binaries import it without importing backend domain or application code. The gateway depends on `ssx-wire` and rmcp client APIs, not on the backend MCP server registry. Cross-crate ports are small traits owned by the application consumer: engine search, engine catalog, canonical cache, durable job/saved/receipt stores, snapshot store, quota/lease primitives, clock, token source, and provider advice. Async trait object use is allowed only at these runtime seams; pure ranking/normalization remains ordinary functions. Constructors receive explicit dependencies and validated configuration. A production composition root wires them once; tests inject fake engines, a deterministic clock/token source, and a faithful in-memory store. Preserve the `state_factory`-equivalent seam so the **real** HTTP/MCP transport, auth, schema, and lifecycle can be tested without live providers or Valkey.

## A01 amendment: executable registry search port

The application owns `SearchEngine`, the async search contract over domain
`SearchRequest` and classified `AdapterResponse`. The registry re-exports this
port as its factory object type and may depend on application; application
remains independent of registry/config/infra, so the crate graph stays acyclic.
The existing application `Engine::probe` is a foundation readiness seam only;
it is not the search adapter contract. Concrete provider implementations remain
in infrastructure and receive the shared F08 transport through their factories.
The registry's domain dependency is test-only for its fixture implementation;
its runtime dependency remains the application-owned search contract.

Register built-in adapters through one engine-local descriptor per module, composed into a generated compile-time registry by the catalog owner. Adding an engine changes its engine module/descriptor and generated aggregate, never the search orchestrator. The integration owner checks names against F01's machine inventory at build/test time. Each descriptor supplies constructor, categories, audited enforcement layers, Jev metadata, auth/config keys, and routing class. An adapter batch edits only its own descriptor; one catalog owner owns registry generation and shared aggregates. Startup rejects duplicate names, missing descriptors, or invalid metadata. Operator config can deactivate or recategorize a registered engine but cannot dynamically load executable plugins. An engine returns classified `AdapterResponse` values; the service turns panic or task failure into a classified adapter error at the containment boundary. Neither becomes an unhandled request crash.

Use internal typed domain values and separately versioned **wire** and **durable** DTOs. `serde` representation is not itself the contract. HTTP JSON, YAML+Markdown, HTML, CSV, RSS, MCP JSON Schema, OAuth JSON and Valkey records each have explicit serializers/readers and golden fixtures from F01. Preserve absent versus null versus empty, sorted engine provenance, exact IDs/digests, Unicode casefold where required, wall-clock expiry, and the canonical full unsliced response with request-specific views. Never derive durable hashes from generic `serde_json` map ordering or use a lossy `default=str` equivalent. Public DTO changes require a contract amendment and portal impact review. Internal Rust type layout may change without changing any public byte or durable identity.

The shared application command boundary evaluates the authority inputs applicable to each surface before dispatch or mutation. MCP paths use the existing sensitive-engine gate and specialist grants; protected workflows additionally check tenant, current membership/grants, idempotency, expected revision, budget and ownership as applicable. Public anonymous HTTP retains its accepted scope/policy inputs and best-effort cache semantics; an additional HTTP restriction requires an explicit intentional-difference decision. A surface maps validated commands and typed outcomes to its wire representation; it cannot grant authority by constructing a domain type. Revalidate stored engine scope and current tenant/grants where applicable when resuming a workflow. Specialist MCP grants do not imply sensitive-engine grant, and mixed sensitive/non-sensitive explicit MCP scopes fail atomically. Authorization errors are not empty success results. Durable workflow commit additionally requires a current Valkey fence (ADR 004).

## Accepted constraints and alternatives

A Python-backed compatibility layer, PyO3 backend, subprocess adapter, sidecar and remote Python fallback are excluded from the final runtime. A temporary pinned Python comparison oracle is permitted for migration evidence established by F04 and extended by successor parity work, isolated from native execution and final mandatory release gates, with executable retirement at G08. Splitting the application into independent search/MCP/workflow services is rejected for this rewrite because it would add a distributed boundary while existing HTTP/MCP already share one service. Dynamic Rust plugin loading is deferred: it would weaken the complete built-in registration and metadata check without an existing runtime requirement. A single giant crate is rejected because it would let transport and storage types leak into policy and public serialization.

ADR 001's server-rendered same-process portal and ADR 002's separate browser OIDC authority remain in force. The Rust rewrite replaces implementation language, not those accepted user-facing and identity contracts.

## Fitness functions and handoff

F03 creates the workspace and native `cargo xtask check <plan-id>` gate; F04 compares both wire bytes and decoded values against F01 fixtures. F05 establishes DTO readers/writers before durable feature ports. F06 verifies 51 descriptors/metadata; F09 verifies the shared gate on every MCP path and each HTTP path against its accepted surface-specific policy inputs; T02/T03/T06 verify public formats, tools, resources and prompts through real native transports. Architecture checks reject dependency inversion, duplicate search implementations, unregistered adapters and surface imports of concrete Valkey/provider modules. Exact command names and paths become executable evidence when those issues implement them; this ADR is an accepted design boundary, not a passing Rust build.

| Open risk | Accountable role | Decision gate |
| --- | --- | --- |
| Async trait/ownership shape may add allocation or fail a required target | Foundation owner | F03 compiled ports plus benchmark/profile before freezing APIs |
| Registry descriptors may drift from generated docs/Jev metadata | Catalog owner | F06 51-name completeness and README/portal contract checks |
| Serde/schema generation may alter defaults, optionality or response envelopes | Protocol owner | F04 byte fixtures and T01 real-client descriptor diff |
| One policy gate may be bypassed by workflow resume or a surface route | Search/policy owner | F09 path matrix and G02 denial tests |

## F05 amendment: lossless JSON without global parser mutation

F05 adds the pure `ssx-json` crate for opaque payloads, answers, infoboxes and
canonical numeric values. Both domain/storage and public wire DTOs consume this
serialization-only crate; it has no backend, transport, configuration or network
dependencies. Surface modules continue to own domain-to-public projections.

A reproduced compatibility test showed that enabling `serde_json`'s globally
unified `arbitrary_precision` feature changes the valid user object
`{"$serde_json::private::Number":"123"}` into number `123`. Feature unification
would affect third-party MCP/HTTP deserializers as well as our own parser, so F05
forbids that feature rather than wrapping selected call sites. Default JSON
values also round integers beyond i64/u64 through f64, breaking durable bytes.

`ssx-json` parses bounded raw JSON tokens into its own object/number representation
and serializes number tokens without rounding. JSON objects retain their exact
keys, including serde's private-looking keys. Structured DTOs keep ordinary serde
implementations with opaque `ssx-json::Value` fields. Consumer review covers
`ssx-domain`, `ssx-wire`, surface projections, and the native F05 gate. This adds a
pure serialization dependency and does not change storage authority or public
transport ownership. Parsing is bounded to 4 MiB and 100 nested levels; callers
apply narrower payload and durable-record limits. Existing opaque identifiers
are never parsed as numbers or normalized on read.

The F05 legacy reader retains opaque object insertion order with pinned `indexmap`
inside `ssx-json`: Python `str(dict)` makes that order observable. Canonical
identity profiles still sort keys explicitly. Neither serde_json `preserve_order`
nor `arbitrary_precision` is enabled globally.

## F11 amendment: shared pure publication codecs

F11 extracts the accepted F05 DOI normalizer and publication-field allowlist,
without semantic changes, to `ssx-publication`. This leaf depends only on
`ssx-json`. `ssx-wire` re-exports its existing public functions; application
ranking consumes the same codec directly. Application does not depend on wire,
and wire does not depend on domain. This avoids duplicating Unicode casefold,
DOI validation or publication limits in scholarly grouping. F05 byte/DTO goldens
remain required. The F11 native gate verifies this dependency boundary.


## W01 amendment: application research lead identity

The application may consume the already pinned pure `sha2` codec to derive
source lead-v1 SHA256 identities from F11 `normalise_url` UTF-8 bytes. Research
settlement owns this policy; domain accepts typed `LeadId` values and does not
depend on application. This allowance adds no socket, storage, configuration,
provider or wire dependency. W01 source-captured identities and settlements run
alongside F11 normalization evidence; the F11 dependency guard explicitly permits
this pure hashing dependency while retaining its other boundary checks.


## A05 amendment: native HTML helper and source text codecs

Parent approved the exact vendor/binding direction for implementation after the
A05 adoption packet review; this is not A05 acceptance. Infrastructure owns a
separately packaged `ssx-html-parser` native executable and its supervised IPC
client. The child alone owns libxml2 documents. Application/domain/wire types
never contain C handles, and native CLI/gateway clients do not import this
backend dependency. Cancellation/deadlines and shutdown must kill and reap
children before releasing admission; dropping a future is insufficient.

The pure `ssx-source-codecs` leaf owns source-compatible HTTP text decoding and
pinned generated mapping/name data, consumed by infra. Its first69single-byte
families retain the full Python license, per-source attribution and immutable
source output fixtures. Other108-decoder scope remains unfinished; successful
captured examples do not authorize a reduced charset contract. The package
explicitly declares MIT AND PSF-2.0 with a package-specific PSF policy exception,
not a global license allowlist expansion. Further Unicode/JIS/third-party
mapping notices require exact review before copying. No Python runtime/API,
interpreter, subprocess Python or service fallback is permitted.

See A05 ARCHITECTURE, ADOPTION-PACKET and codec-source-notice-manifest for source,
review authority, known unresolved wrapper/codec/resource qualification and
historical capture identities. Existing186full-output/request replays remain
mandatory; all-target and helper lifecycle/security evidence remain acceptance
work. Native admission bounds must derive from accepted F08 byte limits and
actual decoder/tree expansion; no unexplained16MiBrestriction is accepted.
