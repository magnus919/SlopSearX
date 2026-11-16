# NativeRecords readonly lineage adapter (W07 scope A)

Built on merged PR #1044, main `14b5b9ba612ac975e13b46ea1a53363870f9ff7e`.
This increment adds the concrete readonly `lineage::workflows::Records` adapter.
It does not activate runtime/MCP composition, implement the manifest repository,
or complete issue #913. See the [rewrite case study](../CASE_STUDY.md).
Existing storage/application contracts, receipt body, Lua, and strict Valkey
fixture are preserved. The adapter owns no resources and creates no tasks,
claims, renewals, acquisitions, mutations, provider calls, or retention extensions.

## Store contracts

- Research uses `ResearchStore::load`, including its exact tenant validation,
  record identity and cancellation-flag merge (current source
  `crates/ssx-infra/src/valkey/research.rs:150-203`, error map `:841-848`).
  Found/missing/unavailable/malformed remain distinct; closed is an unavailable
  record observation because the application `Read` enum has no closed variant.
- Staged uses `StagedStore::read` (`staged.rs:108-135`), preserving the explicit
  logical expiry timestamp and tenant/id checks without writes or claims.
- Saved uses `SavedStore::load` (`saved.rs:101-158`). Its authoritative server
  TIME, deletion/expiry decision, baseline validation, and malformed/unavailable
  distinctions remain intact. W00 intentionally maps deleted/expired definitions
  to missing; this adapter does not invent a logical expiry observation.
- Report hints use one tenant-scoped `ZREVRANGE 0 bound-1`, bounded by caller,
  captured definition max_reports and current operator limit, each at most100.
  No hint authorizes disclosure. Each hinted row independently reaches W04
  `SavedStore::recent_report` with a fresh coherent authority snapshot. It
  re-reads the current definition, bounded current membership and report and
  checks binding, expiry and raw-definition stability (`saved.rs:879-982`).
  Selected-run projection happens only after authority/errors for every hinted
  row. An unrelated malformed or denied row cannot be hidden by selection.
  Responses retain run_id only, plus status/finished_at/expires_at for selected
  run, avoiding retention of up to100 full report trees in memory.

## Current authority and credential-presence distinction

`Authority::current` is a composition seam returning `PolicyService`, effective
`SavedLimits`, and boolean-only `auth_configured`. No secret material is carried.
A new snapshot is obtained before hint enumeration and for every report read.
W04 consumes `SavedPolicy.saved_granted`, `search` and `limits.reports`
(`saved.rs:885-917,941-981`). Its grant check precedes definition/member reads,
so this adapter also denies empty hints if the initial grant is absent.

W04 does **not** consume `auth_configured` for retained-report reads and does
not call `SavedPolicy::validate_definition` there. Credential-presence booleans
are supplied for compatibility with `SavedPolicy` (`ssx-application/src/saved.rs:38-44`),
not promoted into a new gate. Current engine eligibility is W04's exact
`retained_engine_rejection`: known/enabled engine and sensitive-scope policy
are authoritative; runtime activity/availability are deliberately excluded
(`ssx-application/src/policy.rs:228-319`). Auth readiness, activity, health and
credential values do not become acquisition authority through a retained read.
The new test reads retained evidence with active=false/auth_ready=false and
boolean auth_configured=false, then proves current grant revocation or engine
disabling still denies despite valid index hints. Effective report limits are
rechecked on every row through W04, including a limit shrinking after hints.

## Cancellation and draining

Every store await is preceded and followed by cancellation checks; report-loop
iterations and completion are checked too. Read errors are not returned before
the post-await cancellation check. Cancellation never races a read via select
or abandons a spawned task: the exact bounded store future completes first.
The backend and its bounded driver/close lifecycle remain externally owned.
The executed cancellation control uses a
private test-only RESP2 UDS relay forwards the exact missing-job GET to the
original disposable Valkey and holds its actual Nil reply. After cancellation,
the same pinned read is explicitly polled before release and must remain Pending;
after release it must return Cancelled. A subsequent missing read must reuse the
same connection. Pre-cancelled calls on all four ports are fenced by a public
same-backend PING; only the explicit PING may appear after the command snapshot.
The relay owns and joins one std thread, closes its UDS streams, and removes only
its own socket/directory before the unchanged fixture retires its original Child.

This post-dispatch control covers one missing-research-job GET. A found job's
second cancellation-record GET and the other three ports' post-dispatch paths
are not covered by that barrier. The explicit test backend uses a 30-second
owner budget under five-second phase guards; this is neither the accepted
fixture's 300 ms default nor default-performance evidence. Reply release is an
explicit channel action, never an elapsed-time threshold.

## Focused test map and qualification limits

All four tests are in `crates/ssx-infra/tests/w07_lineage_records.rs`. They use
`#[path = "w07_support/valkey.rs"]` to reuse the accepted strict pinned fixture
unaltered (SHA256 `ed4e2e544ff40856f46810532a17cf5fb9a3b393eb35d09a75907c4585b6b699`).
The fixture retains its original Child through retirement, exact binary digest,
Tank scratch requirement and private socket; no fallback server or owner exists.
The tests reuse the existing W11 corpus without editing its source identity.

| New test | Intended observable control |
| --- | --- |
| typed_stores_tenant_binding_expiry_missing_and_readonly_ttls | Actual typed stores; foreign tenant keys missing; forged tenant data malformed; staged explicit expiry; closed backend unavailable; DUMP bytes identical and PTTL never extended |
| hints_cannot_bypass_live_policy_and_projection_preserves_error_order | Compact projection; report/definition/index bytes and TTL unchanged; second-row grant revocation discards partial output; current engine disabling denies; malformed nonselected row errors |
| fresh_report_bound_and_authoritative_definition_override_stale_hints | Current report bound shrinks after initial hint read; current stored scope overrides stale caller snapshot; missing authoritative row suppresses hint; closed index backend unavailable |
| cancellation_before_reads_and_after_dispatched_read_is_drained | Fenced pre-cancellation on all four ports; exact missing-job GET forwarded and real Nil reply held; manual post-cancel Pending polls before explicit release; Cancelled, same-connection reuse, joined relay cleanup and terminal close |

## Executed focused proof

On the reviewed four-file delta over merged main, pinned Rust 1.99.0 formatting
and strict scoped Clippy passed. All four causal tests and all eleven merged
receipt regressions passed with zero ignored or filtered tests. Both finite
negative controls were compiled and executed: each returned native exit 101
with the exact named test failing on its registered assertion, not a compile
failure. Each reported zero passed, one failed, and three filtered tests.
The production module was restored byte-for-byte after each mutant; all four
causal tests then passed again. Its restored SHA256 is
`6c97e49dd6de263a04b40be4b67438fb1b21fff44979d27bc68e6f6070e6531a`.

The initial strict Clippy attempt stopped on a nested cleanup `if`. The only
successor change was an edition-2024 let-chain with identical short-circuit,
cleanup, panic and logging behavior. That failed attempt remains recorded.
The successful runner was original handle 47616, exit 0. Its immutable result is
`w07-native-records-focused-20261023/resume-clippy/RESULT.json`, SHA256
`9dd75159b9f196678dd9bcdb533ef114e696ba5b557b5065907df1d85fa9e064`.
Each native process had two clean retirement observations. The private Valkey
scratch was empty and removed, all leases were released, and target growth was
266,907,648 bytes. No providers or production services were contacted.

This is focused storage-adapter evidence, not full-workspace, production,
transport/MCP composition, default-performance, or completed-W07 evidence.
The existing HTTP/portal behavior is unchanged by this disconnected adapter;
portal impact must be reviewed when runtime composition is added.
AST graph update was run after application; source and exact-head delivery
checks remain separate from these focused native results.

## Prepared negative sources and bounded claim

The owned source capsule contains two separate afterimages of only lineage.rs:
`pre_io_guard_removed` removes the research pre-I/O cancellation check while
retaining its post-await check; `premature_cancel_return` manually polls that
owning read but returns Cancelled as soon as cancellation is observed. The first
must fail the same-stream PING-fenced command suffix; the second must fail the
first explicit Pending assertion while the real Nil reply remains held. Both mutants were executed and rejected by their registered assertions, then
the exact production module was restored. They are not production branches.

The four original test identities are preserved. Relative to the preserved causal
successor, the report-policy test now explicitly closes its three independently
created backend owners before the fixture retires. The other three test bodies
are unchanged. These explicit owner closes were exercised by the successful native tests.
Production Authority remains an injected coherent-snapshot port: this candidate
contains no live operator authority implementation or runtime/MCP activation.


## Shared evidence composition (W07 scope B)

Scope B composes borrowed references to the same `Arc<ValkeyBackend>`,
`SnapshotService`, and `SearchClock`, with an injected current-policy owner.
Construction is explicit and does not activate a new MCP tool, HTTP route,
portal response, provider dispatch, or gateway behavior. The existing runtime
owner remains responsible for startup, bounded PING readiness, cancellation,
and shutdown. The component owns no replacement backend or background task.
The portal impact is therefore limited to a construction-only seam: existing
browser-visible behavior remains unchanged. Dispatch integration and end-to-end
portal acceptance belong to the remaining delivery scopes.

Every authority publication is validated atomically. Post-read authorization
boundaries resample current grants, retained-engine eligibility, and the
corresponding staged/report limits from one coherent publication. Revocation
remains effective after each await; a policy snapshot is not frozen for an
entire operation. Legacy injected application ports retain fixed numeric-limit
defaults while explicitly rechecking current grants. Entity collection shares
the full/projected implementation, including the frozen F13 corpus.

Focused native qualification passed on integrated source
`637dcc99a04a718c35cfdb186f60f777511615fb` (8,171 source files, unchanged
through the run). All 119 xtask controls and 60 focused business tests passed,
with no ignored tests: eight authority controls, one frozen entity corpus,
14 lineage boundaries, 18 receipt regressions, one search-construction control,
three real shared-component tests, and 15 existing record/receipt regressions.
The controls include publication during staged, saved-definition, selected
report, research and second entity reads; denial prevents downstream report or
projected visitor work. Strict scoped library, component, record and xtask
all-target Clippy checks passed. Formatting passed. Earlier lint and finite
source-proof wiring failures remain preserved rather than relabeled as passes.

Immutable local receipts under the registered Rust build root:

- `w07-evidence-composition-focused-20261023/resume-integrated/RESULT.json`,
  SHA256 `f00746d3f4cd5e8312f9ce1f24f2594be1c7a615068d09e38393e8c8ccc0d136`.
- `w07-evidence-composition-focused-20261023/strict-xtask/RESULT.json`,
  SHA256 `5e56334105c1ac0138455dcdcbf91316bf6a8eb98621b57caa77506ad00c160f`.

Both native owners retired, all three exclusive leases were released, and the
private Valkey scratch was empty and removed. Shared target/cache reuse used
profile debug=0 and a finite package provenance cleanup after the prior owner.
The source successor preserves frozen inventories and historical negative
controls, proving the exact three predecessor sources before accepting the one
new construction-test identity (383 to 384 current A06 library tests).

This is focused component evidence, not full-workspace qualification,
production activation, transport performance, or complete W07 delivery.
Issue #913 remains open for scopes C/D. See the [case study](../CASE_STUDY.md).

## Final authoritative reply fence (source repair; qualification pending)

The final-read repair retains a typed cumulative witness of every engine set and
W04 membership bound actually authorized before an awaited report read. After
that read, and again before returning an enumeration (including zero hints), a
fresh authority snapshot must preserve all those permissions. An engine enabled
only after the initial hint read is included in this witness. A report bound
narrowed below any membership bound used by W04 discards the complete projection;
it never selects another run or truncates the previously selected result. The
stored definition remains W04's authority. Malformed/backend error and
cancellation precedence, original read ownership and readonly TTL behavior stay
intact. Held real final-CAS reply controls are authored for this boundary; actual
native qualification of this source repair is pending.

The witness deliberately rejects conservatively: it includes all retained-allowed
catalog engines in every policy sample used, and the maximum actual W04
membership bound, even when the outer index hint selected fewer rows. Revoking
an otherwise unused previously allowed engine or reducing that membership bound
can therefore deny the whole in-flight projection. Concurrent widening never
erases an earlier bound, and a later narrowing cannot reuse the original hint
bound as disclosure authority. This avoids reauthorizing from an untrusted caller
snapshot and adds no epoch, retry or additional asynchronous store operation.
The repair changes the existing readonly composition seam; it activates no new
HTTP, MCP or portal route and changes no browser-visible Python portal contract.
