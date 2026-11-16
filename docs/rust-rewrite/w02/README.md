# W02 durable research runner

Implementation candidate for issue #906, based on accepted main
`5aa7983aebc2dddbf00727d5a7e6b1192b3dfee0` (W01 and F15 merged).
The integration owner retains acceptance, merge and issue closure.

`ssx_infra::research_runner::ResearchRunner` owns the native claim/renew/run/
settle loop. Composition supplies the shared SearchService, ResearchStore and
SnapshotService, explicit worker limits and research ceilings. `run` supervises a
bounded pool; send true through its watch channel and await it for joined shutdown.
`run_once`, `retry` and `extend` execute through the same fenced path. W09 owns
wire validation/error projection/planning; O04/server composition owns process
configuration and distribution. The foundation ssx-worker executable is not wired
by this library change. No Python process implements execution or mandatory checks.

## Authority and recovery

Valkey ready discovery is authoritative. A local queue is unnecessary. The tenant
comes from the claimed authoritative record, including claims found through the
global tenant index. Renewal, record mutation, cancellation reads and snapshots
all use that tenant. Missing, malformed, unavailable and closed reads never become
terminal success. Store writes are not automatically replayed after uncertain
completion; the next discovery/recovery reconciles the durable record.

Reservations charge before dispatch and return a fresh acknowledged AttemptId.
Settlement carries exactly that identity, never a later last-attempt value.
W01 recovery retains interrupted attempt evidence and uncertain charges. Random
lease/attempt identities use 256 bits from maintained AWS-LC randomness. A kill
fixture terminates an actual compiled worker after its deterministic adapter has
accepted the request, waits for the real lease TTL, and recovers through durable
discovery in a different worker. Fencing prevents stale publication; it cannot
undo remote work or promise exactly-once provider requests.

Renewal runs for the entire claim, including snapshot persistence. Definite lost
ownership or inability to establish authority cancels owned search. Shutdown can
interrupt renewal I/O itself; it does not wait for the store command budget before
signalling search cancellation. SearchCancellation joins the owned flight. The
runner joins renewal and token-releases before returning. Dropping an execution
future signals cancellation and aborts renewal via RAII; because Rust Drop cannot
await, callers requiring joined completion must use and await shutdown. An actual
abort/drop regression verifies socket closure, admission capacity recovery,
unrelated-flight survival and TTL-bounded reclaim with retained charges. A process
kill relies on finite lease expiry and durable recovery.

Cancellation/expiry during a query settles it as cancelled, retains the charged
attempt and returns cancelled or deadline-derived partial/failed job state.
Pre-run claim expiry remains W01's explicit expired state. Completed snapshots
and prior attempts survive retry and extension. No-retryable-work and live-owner
are typed distinctions. Follow-up engine lists retain their order under persisted
ceilings; the full supplied list passes shared policy before any cap.

## Policy and immutable evidence

SearchService remains the only dispatch authority. Dossier execution rechecks
research, security and dependency_dossier grants and every planned query before
any dispatch. Its separate adapter-call budget is charged durably and retained
on unknown outcomes. Present malformed budget values fail closed instead of
falling back to a larger default; captured-result accounting remains separate from research
admission accounting. All guarded publication invokes the existing synchronous
with_revalidated policy gate inside ResearchStore::mutate before attaching evidence.

The cancellation-aware snapshot method preserves the original default-token
wrapper. It captures the complete canonical response and tags lineage with the
acknowledged research attempt. A cancellation during snapshot persistence may
return CompletionUnknown after the write succeeded. Tests retain the resulting
tenant-scoped orphan and prove that the stale worker never links it into the job.
There is no cross-key rollback claim. Snapshot failures settle the query as failed
without refunding dispatched work. Every attempt gets a unique execution isolation
key. Canonical cache identity remains separate; isolation does not disable cache.

Coverage uses actual outcome statuses and trusted required-credential metadata,
with exclusion override and deterministic source order. Empty plus a failed
engine remains failed. Only requested time_range enforcement is persisted by the
current research query contract. Python and Rust ResearchQuery expose no absolute
date fields; this issue adds none. Canonical search capture retains supplied shared
SearchResponse date proof without inventing worker date enforcement.

The actual accepted service keeps grants immutable for its configured lifetime.
Current policy is checked at dispatch and publication, but this is not proof of
hot grant revocation. A negative fixture showed that opening the real circuit
during snapshot persistence still authorizes an explicit research scope under the
shared resolver; the runner preserves that behavior. A separate fixture prepares
authority under one configuration and refuses it under a denied configuration,
including a real-Valkey mutation whose proposed publication leaves the record
unchanged. Cross-surface live policy lifecycle disposition remains an integration
requirement; no synthetic circuit-as-grant-revocation claim is made.

Denied workflow authority records an immutable failed predispatch audit by
reserving and immediately settling the acknowledged attempt. This charges the
conservative research attempt/engine reservation, preserves not-selected coverage
and denial diagnostics across retry, and does not charge the dossier actual-call
counter or dispatch providers. Exhausted reservation appends no fabricated attempt.
The source appends this audit without charged reservation; W01 requires the native
accounting difference. A restored-configuration retry fixture verifies prior
success and denial attempt history remain byte-identical. Successful dossiers at
the query ceiling retain source `plan_executed`, while ordinary research uses
`query_budget_exhausted`.

## Evidence and portal impact

`sources.json` pins eight current source/regression files to Git blobs and SHA256.
The native gate independently pins that binding file, verifies historical and
current source identity, requires official Valkey 8.1.10, verifies the exact test
inventory, and rejects missing, failed or ignored tests. The subprocess probe is
invoked by the parent kill test against its private Valkey; its ordinary discovery
invocation is a no-op rather than a separate process-kill claim.

```sh
SLOPSEARX_TEST_VALKEY_SERVER=/path/to/private/valkey-server cargo xtask check W02
cargo fmt --all --check
cargo clippy --locked -p ssx-infra --test w02 -p xtask -- -D warnings
cargo test --locked -p ssx-infra --test f14 --test f15 --test w01
```

All adapter exchanges are deterministic fixtures or loopback sockets. No live,
paid, scientific or production request is part of qualification. The gate is
native; source Python remains pinned historical comparison material, not invoked
by W02. W01 owns raw store/legacy/accounting regression coverage; W09 owns MCP
errors, caller identity extraction, planning and start/extend wire validation.

Portal review: shared search cancellation is additive; the existing snapshot
wrapper and public request/response/rendering contracts remain unchanged. No
browser-visible behavior changes in this unwired runner library. Required portal
CI remains an integration gate; this package does not claim browser cutover.

Large finite persisted deadlines use checked Duration/Instant conversion without
clamping the stored horizon; engine limits continue to bound real work. Lease
and poll settings that cannot be represented by signed Valkey expiry or Tokio
Instant are rejected before workers start. Real-store tests exercise deadlines
1e300, 1e20 and 1e12; constructor tests cover TTL1 and rejected u64::MAX.

## Retained negative results

- Initial fixture used unknown research intent `general`: correctly produced zero
  dispatch. It was corrected to source intent `web` and held waits were bounded.
- The first circuit-revocation expectation failed: current explicit-scope authority
  permits publication. The regression now records accepted shared behavior.
- A first native check ran while source was still being extended. All 20 tests
  passed, but the gate correctly failed source-stability verification. That run is
  not acceptance evidence; qualification is rerun after freezing source.
- A temporary coverage refactor produced a parse error before any acceptance run;
  the loop was repaired and scoped compilation/tests rerun. No such intermediate
  source is a delivery candidate.

## Candidate qualification before upstream integration

The frozen native gate passed 25 tests with zero failed or ignored, including
`large_finite_persisted_deadlines_execute_without_duration_or_instant_panic`,
`runner_rejects_unrepresentable_lease_and_poll_configuration` and
`malformed_workflow_budget_never_defaults_into_more_spend`. Native F01, F02 and
O06 also passed. Shared native regressions passed F14's31, F15's15 and W01's28
cases. Scoped Clippy with denied warnings and workspace formatting passed.
Graphify AST update completed without provider/API calls.

The required full pre-commit run passed every hook except ruff-format, which
reformatted the unrelated, previously tracked
`tests/test_coverage_saved_method_materials.py`. That hook-created cosmetic diff
was inspected and restored byte-exactly; no Python file is part of W02. Repeating
all-files hooks with only that known baseline formatter exception passed all
remaining hooks, including JSON/YAML validation, Ruff, import-linter and mypy.
The formatter exception is retained as a limitation, not reported as a complete
all-hooks pass. Native W02 qualification invokes no Python backend.
