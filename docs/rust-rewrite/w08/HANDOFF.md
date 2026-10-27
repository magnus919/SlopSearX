# W08 native dependency dossiers and composition

Issue #914; refreshed baseline `e9c5911a468b892fe2adef9027374cb867a2535d` (2026-10-27). This packet describes source and finite acceptance obligations. Native execution results must be attached after the registered source, target and Cargo-cache leases are admitted; source presence is not a passing result.

## Accepted predecessor reconciliation

W07 merged as PR #1052 at this baseline. Its accepted tree is `c7bbf9582ceb6613c3c47f3ab831e309c88fa823`. The older W07 acceptance addendum's instruction to keep #913 open described its historical checkpoint. The final hosted acceptance packet is `/Volumes/tank01/magnus/rust-builds/slopsearx/w07-publication-draft-20261026/PR-1052-730e-FINAL-HOSTED-ACCEPTANCE.md`, SHA256 `3e17cc5c2a4308fc7040313141dd6d52f13c977e3a2a38b0c393c6fb0cd7bfaa`. Its 29 successful and four expected skipped checks support the native predecessor handoff; the provider assessment remains partial/incomplete and does not establish semantic accuracy or runtime user benefit. Historical receipts remain unchanged.

W08 consumes accepted W07 `NativeRecords`, typed snapshot reads and entity projection ports; W02 `ResearchStore` and `ResearchRunner`; W04 retained saved-report reads; W06 staged record validation; and F15's explicit separation of optional provider advice from retained evidence. It adds no model provider, alternative store or worker.

## Native interfaces

`ssx_application::composition::SourceResolver` resolves the exact allowed source/destination matrix. It has snapshot, retained workflow, entity, current authority and clock ports. It has no dispatch, persistence or TTL-renewal port. `Authority::current` must return the trusted current principal's tenant together with current operator policy and staged/report bounds. Implementations must not derive authority from a tool argument or retained artifact. Policy, tenant, accumulated engine/grant witnesses and relevant bounds are rechecked after every awaited read and before return.

`ssx_application::dependency_dossier` implements package/repository normalization, exact canonical workflow digest, bounded plans, source-bound idempotency comparison, complete admitted-evidence attribution and reports. Report card limits never alter the underlying snapshot or the evidence used for name/version resolution. A supplied native card projection determines presentation; the application does not depend on MCP registry or protocol code.

`ssx_infra::dependency_dossier::DossierService` creates and reads durable jobs. Its `DossierJobs` port delegates production create/load exclusively to the existing `ResearchStore`; the narrow seam permits an exact post-persist/pre-return fixture barrier. `DossierService::runner(existing_runner)` attaches `NativeSourceValidator` to the existing W02 worker. This concrete construction path is exercised by the private-Valkey/loopback suite. Final workflow transports must use this composition instead of constructing a composed-job runner without the validator.

## Source authority at dispatch

Composed work is re-resolved before each new reservation and again after reservation/load/renew awaits immediately before search admission. Missing, null, malformed, tenant-inaccessible, expired, revoked or altered descriptors fail closed. The concrete validator compares the complete stored descriptor, including selected result IDs, query and selected-attempt/cursor details. A runner restarted without a source validator denies composed jobs. Uncomposed jobs retain the existing path.

Pre-reservation denial records failed pending queries and `source_rejected: <code>` with zero reservation and zero adapter requests. A source revoked after an acknowledged reservation records an exact failed attempt, no response/cursor/admitted results, and `source_rejected_before_dispatch: <code>`; actual adapter requests remain zero. W01's already-persisted reservation is conserved. The legacy `workflow.budget.used_adapter_calls` name records reservation exposure and must not be reported as measured actual provider calls. No refund transition was added. Read-only source admission observes worker cancellation, shutdown, the persisted deadline and a 30-second maximum resolution window. Successful validation is a dispatch-admission witness, not a promise to atomically revoke an already in-flight provider request.

## Bounds and unavailable states

Source result IDs are capped at 25; a research destination rejects a truncated seed. Research manifest sources inspect at most 100 retained cursors and reject larger cursor sets explicitly. Dossier query count, adapter reservations, admitted results and per-read card limits remain independent. Package attribution requires schema-v1 package payloads with the matching engine and adapter-supplied (not inferred) name. Repository metadata supplies candidates only; ownership remains false, and advisory applicability remains `not_evaluated`.

Snapshot missing, logical expiry and unavailable states remain distinct. Saved reports use their historical observation window, reject truncation before incomparability, and do not invent an expiry field when the W00 store naturally reports a missing item. Staged sources require the selected attempt and its captured cursor. Research sources retain attempt filtering, unique bounded result IDs and partial/truncated status.

## Portal and downstream boundary

This shared backend addition requires portal review. No HTTP, existing portal request/result contract, HTML formatter, browser asset or production startup is changed. W10 owns protected portal composition UI; W09 owns final workflow schemas/registry/wire integration. Those consumers must reuse these services and the existing public result-card projection. The existing portal CI gate remains required delivery evidence; this packet does not substitute a native service fixture for browser acceptance.

## Validation packet

`cargo xtask check W08` seals the current source/reference inventory, lists the exact semantic case identities, executes both native suites, rejects ignored/renamed/missing/failed cases and requires exact completions. The infrastructure suite requires a pinned disposable Valkey executable and registered `TMPDIR`; it uses private state and actual loopback requests and never global flushes. The Python source is read-only reference material, never a mandatory executable helper.

Required native commands: locked build; `cargo fmt --all --check`; scoped Clippy; both W08 suites; `cargo xtask check W08`; affected predecessor source-reader controls; and focused W02 budget/worker regressions. Record exact source, toolchain, Valkey executable digest, target/cache owners, commands, exits and retained raw outputs. No provider/publication/deployment is authorized by this packet.
