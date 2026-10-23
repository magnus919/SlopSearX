# W07 native MCP evidence boundary

This C increment attaches four evidence tools to the existing native search
surface. It continues B commit `98330f1a94b5f50ad228117ce392601c0ce4dea4`;
that predecessor is a dependency, not merged-main evidence. C is source-only
until the root qualification controller executes the recorded current tests.
No native test, Cargo check, or production activation is claimed here.

The tool order is submission, receipt read, manifest export, and artifact
lineage. Names, parameters, strict integer annotations, defaults, descriptions,
and return annotations are recorded from the current pinned Python AST in
[`mcp-tools-source.json`](mcp-tools-source.json). That file also pins the actual
registry and server sources. Metadata and cleaned schemas follow the frozen
FastMCP 4.0.11 T06 profile. This is a source projection awaiting runtime
comparison, not a new measured Python descriptor capture. Historical EXP-122
metadata is not used as the oracle.

## Shared owner and transport attachment

`EvidenceFactory::new` borrows the caller's existing `SearchState`, backend,
authority, and `SearchClock`. It reuses that state's exact `SnapshotService`
for both search and evidence. It creates no second search service, backend,
runner, scheduler, or shutdown owner. The trusted composition must supply the
same backend and clock as its search dependencies and install
`SearchPolicyAdapter` from the same `EvidenceOwner::authority()` before sharing
the search service. `SearchService::with_policy_source` checks its initial
policy against the existing dependencies and refuses replacement after sharing.
These capabilities are never read from a request.

The owner establishes readiness on the shared backend before accepting work.
The HTTP factory uses `runtime_catalogs()` and requires the existing
`NetworkAuth`. The legacy factory binds each connection's authenticated tenant
and original-request wire state and mounts the same network authorization
around both SSE and message paths. The stdio factory binds the same native
wire state through `stdio_with_factory_owned`; trusted stdio has no network
authorization wrapper. All three factories use the existing transport and
bounded output owners.

An unattached T06 surface keeps its frozen eleven-tool catalog. An attached
surface's runtime catalog and handler both expose the four evidence tools.
The instance catalog is required: static `SearchSurface::catalogs()` intentionally
retains the historical T06 contract. W09 must preserve instance catalogs when
composing additional tool registries.

Shutdown remains with the caller: stop admission through `EvidenceOwner`,
cancel and join the existing transport/handler owners, then call the existing
`SearchService::shutdown`. Evidence never closes the borrowed backend. The
added `SearchCancellation::cancelled` observes the caller's existing signal;
the MCP boundary then signals receipt cancellation and awaits the owned
operation before publishing completion. No detached bridge task is created.

## Contract and policy

Submission, receipt reads, and manifest export require the current
`retrieval_receipts` grant and connected durable state. Lineage has no separate
receipt grant; each artifact resolver rechecks its own current policy. Entity
groups require retained-engine authorization, while receipt nodes require the
receipt grant and retained-engine authorization. The separately committed
application correction `42a0b695bf292632d850cccfd28c096aae8a7056` preserves entity access without that grant and rechecks
result/receipt authorization after their reads. Current engine and sensitive
eligibility remain authoritative. Revoking a receipt grant hides receipt edges
without denying an otherwise eligible result.

The handlers call the existing application receipt service, observation
normalizer, manifest assembly, and lineage traversal. They do not implement
alternate algorithms, fetch supplied URLs, or interpret captured content as
verified. Caller attributes remain inert, with `observations_verified=false`.
The existing tenant-bound snapshot service retains its missing/expired and
malformed-handle behavior. Known malformed receipt repository state and
indeterminate writes have explicit native error mappings recorded in
[`mcp-tools-parity.json`](mcp-tools-parity.json).

Bounds remain those of the original contracts: receipt read limits 1–20,
manifest direct selections at most 25 each, lineage depths 0–5 and node limits
1–100. Receipt storage preserves its 32 KiB cap and 20-entry index. Manifest
assembly preserves its 1 MiB encoded limit. The existing native transports
retain their own complete serialized response limits (HTTP/stdio factory
profile 4 MiB; legacy 16 MiB). Input presentation values never select those
trusted transport limits.

## Source qualification and remaining execution

The current finite C inventory is in
[`mcp-tools-inventory.json`](mcp-tools-inventory.json). Authored tests cover fake
ports, strict/nested argument types, error order, replay/conflict, tenant and
grant isolation, the actual authenticated HTTP/legacy factories, an inert-URL
listener, and cancellation held at an owned receipt read. They have not run.
The private Valkey test fixture requires the root's explicit executable and
SHA-256 environment pins. No ambient application Valkey URL is consumed.

Pinned 1.99.0 rustfmt and source diff checks are source checks only. The exact
additive search-source amendment is V3: frozen V1/V2 and the original A06/A13
source contracts remain unchanged; reconstruction removes only the reviewed
additive spans. The new application/MCP tests must join the current finite
inventory when native qualification resumes. T06's historical tool catalog and
three search-surface library test identities remain intact. Both T06 native
binding consumers use the same sealed W07 reconstruction for the exact frozen
search source row (`fcac2929…7b12`, 86,121 bytes), including actual current
hash/length/mode and inverse edits. The frozen ten-file manifest and existing
storage amendment remain unchanged. This closes the source binding gap; its
positive and negative native controls still require execution.

Before C/D acceptance, execute the current application/MCP tests, compare the
current runtime descriptors against the source contract, run inherited
F03/T01/T06/W07 controls and strict Clippy, review the exact source, and preserve
honest outcomes. The evidence-specific stdio process journey is also authored. It uses the
factory's attached surface and exact owned stdio transport, with a private
observer around the existing handler future. Its ID-bound pending marker is
emitted only after that actual receipt-read future yields; its drained marker
is emitted only after the future returns. The parent holds only its disposable
Valkey, closes stdin after the pending acknowledgement, and requires handler
drain with that actual RequestContext cancellation token set, overall-owner
backend close, and child exit before unpausing the server.
Success and Result-error paths resume the private server and reap the child.
The observer creates no task, backend, shutdown owner, or replacement policy.
This process journey has not run. W07 supplies usable native evidence APIs, storage, fixtures, and these
factories. Full executable startup and workflow registry composition remain
T02/W09 milestone work; this source increment does not close issue #913.

## Portal impact review

This change activates MCP paths only on an explicitly attached native evidence
surface. It does not alter browser request state, HTML fields, canonical search
responses, snapshot identities, or portal routes. The shared search policy gate
is retained, and the cancellation change only observes an existing signal.
No browser-visible contract change or portal documentation change is needed.
The required portal gate remains part of eventual exact-head qualification;
it has not been run for this C source revision.
