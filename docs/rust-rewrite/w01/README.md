# W01 research records and conserved accounting

Source baseline: `cfd7a0af61a22cfd040a7fcd41ad950f82005959`, with accepted F05/F07.
Accepted F11 main `fd80e36e6e1868d5c47f3f3d93c78952fcbc20e5` is integrated normally.
Issue: [#905](https://github.com/magnus919/SlopSearX/issues/905).

`ssx_domain::research` owns the durable job/query/attempt/coverage schema and pure
budget transitions. Completed attempts contain their own cursor, query identity,
coverage, enforcement, admitted result IDs and newly seen lead IDs. Mutable
execution ownership lives on the job and is supplied independently as a typed
`LeaseToken<JobId>` to every authoritative mutation. Candidate records can be
edited in memory; the store rejects any rewrite or deletion of completed attempt
history or query planning provenance before an atomic write.

`ResearchJob::from_payload` fills source legacy defaults and normalizes empty/null
collections only at declared schema paths. It retains large integers and nulls in
opaque workflow/enforcement metadata through the F05 lossless JSON codec. Count
and timestamp coercions match the captured source legacy cases. Invalid state,
negative/overflowing counts, malformed collection shapes and duplicate nonempty
attempt IDs are rejected. Legacy empty attempt IDs remain unchanged.

`normalize_legacy_evidence` materializes an implicit completed legacy attempt
before prepared retry/reset or initialization can erase its cursor, query ID,
coverage and enforcement. Empty attempt ID and zero attempted timestamp preserve
the fact that original identity/time is unknown. This is explicit hardening over
the source reset-before-initialization edge; the captured source observations
remain unchanged. The authoritative transition boundary rejects generic callers
that clear this implicit evidence or initialize it away. W02 must call this helper
before applying reset/accounting/reservation to a legacy candidate.

`initialize_budget` only lowers ceilings and infers pre-ledger usage by the source
rules. `reserve_attempt` requires a pending query with no active last attempt,
checks attempt/engine/result capacity, clears current query fields, appends a
fresh attempt and charges attempt/engine slots. Reservation no longer performs implicit recovery. Authoritative claim
and cancellation recovery paths interrupt uncertain attempts while retaining
charges; claim resets running queries to pending, while cancellation remains
terminal. Explicit retry admission uses `reset_retryable_queries`.
`finish_attempt` now requires `&AttemptId` in both domain and application APIs;
it settles only the matching running attempt once, bounds admitted results,
charges duplicates, and deduplicates newly seen lead identities. W02 must capture
the attempt ID from the durable reservation before dispatch and carry that exact
ID into completion, rather than derive it from the latest reloaded query. An
identity mismatch aborts the store mutation before CAS, so response fields set
on its clone cannot leak into durable state. Leases still fence old owners.

This is intentional hardening of Python's raw helper: it can interrupt an active
attempt or reopen a terminal query directly and has no expected-ID argument.
The unchanged source corpus retains both raw-helper cases
`reserve-interrupts-before-budget-rejection` and `result-exhaustion`. Native replay
asserts typed rejection and unchanged native state, compares complete source
payloads permitting only query state/error, stop reason and the prior attempt's
interruption state/error/time where observed, and retains full budget-summary
and roundtrip checks. Both differences must be exercised. Valid recovered
reservation replay explicitly models the claim entrypoint's pending transition.
All clocks and attempt identifiers remain explicit; the source's legacy orphan
reconstruction observation is retained exactly. Existing accounting, recovery,
and retry contracts remain tested through real Valkey without worker/provider
execution.

`ssx_infra::valkey::research::ResearchStore` uses F07 connections, literal tenant
job/lease/cancel keys, typed leases and source-exact ready/recent assets. The F07 legacy idempotency asset remains unchanged; W01 adds a disjoint admission namespace. Its authority
is Valkey alone. `create` is admission-only; `claim` recovers queued/orphaned work;
`claim_prepared` reapplies retry/extension/metadata mutations to an authoritative
record under a fresh lease. A `Some(Claim)` means the caller acquired a fenced
lease and received authoritative state; it is not permission to dispatch work.
Prepared claims can return terminal `cancelled` or `expired` records without
invoking the mutation callback, so callers must inspect state and execution
intent (`execute` and `caller_completed`) before dispatch. They must clear
ownership when appropriate and release the lease on every return path. The W02
runner owns those dispatch and pre-dispatch revalidation decisions, following
the source `run_direct` flow. `mutate` reloads current state, merges cancellation,
validates evidence and accounting deltas, and atomically compares both the lease
and original raw record bytes before writing. Stale owners and concurrent writers
sharing one token cannot overwrite each other's history. The validation boundary
also rejects arbitrary mutation closures that append work/admissions without
charges, increase ceilings, refund counters, or add new work above lowered limits.
Already consumed usage can legitimately exceed a newly lowered ceiling.
Fresh attempt IDs must be valid and unique; introducing an empty ID is allowed
only for a faithful historical legacy snapshot. Existing legacy IDs remain
unchanged. New lead evidence must have same-attempt result admissions with
snapshot cursor/index identities and conserved result charges. The global seen
history extension must exactly match unique canonical new lead evidence; it
cannot grow independently or repeat already known IDs. Imported legacy history
remains the preserved baseline for this extension check.

A reservation is safe to dispatch only after `mutate` returns acknowledged
success. `CompletionUnknown` and Lua partial-write errors retain F07 meanings:
reconcile authoritative state before deciding what happened, and never dispatch
or retry automatically. Losing a lease cannot retract an external request.
W01 does not claim exactly-once external execution or implement the W02 runner,
keepalive supervision, service dispatch, or F15 integration.

Cancellation uses a separate durable key. Live owners observe it on reads and
mutations; callbacks cannot erase an already observed side-key cancellation.
Validation uses the merged observation while CAS atomically compares the lease,
original job bytes and exact cancellation-side-key bytes or absence. A signal
change between observation and commit returns conflict without writing; callbacks
are never retried automatically. This covers mutate, claim, prepared claim and
request_cancel's own prewritten signal. New charged attempts are rejected when
observed or proposed cancellation is true, while existing settlement and
conservative legacy recovery remain allowed. Unowned cancellation reloads under a
fresh lease before finalizing. Commit is the reservation linearization point;
cancellation arriving after that commit remains W02's pre-dispatch revalidation
and cooperative cancellation responsibility. This package does not claim
an atomic exclusion guarantee across external dispatch.
Absolute deadline equality expires at claim/admission, and terminal expired jobs
do not resurrect when a supplied wall clock moves backward. Record retention is
the source 86,400 seconds, distinct from job execution deadline. Reads retain
terminal evidence until physical retention expires. Ready/recent indexes are
derived discovery state; bounded SCAN reconciliation and explicit repair rebuild
them without granting ownership. Literal colon-containing tenants are supported;
job IDs containing colons are rejected to avoid tenant/job key aliasing.

## Native evidence

Run with the official disposable Valkey8.1.10 binary built by the accepted F07
script, never an application store URL:

```sh
SLOPSEARX_TEST_VALKEY_SERVER=/path/to/private/valkey-server cargo xtask check W01 --output target/rust-evidence/W01.json
```

The native gate checks independently pinned exporter/corpus digests and exact
source Git blob/current-file identities, then runs 10 source-bound multistep
record/accounting cases, 56 coverage-classification cases, malformed/immutable
record checks, the accepted F07 real-store gate and W01 private-Valkey contracts.
Missing/corrupt evidence, metadata that declares its own new digest, a missing
server executable, or a server version other than 8.1.10 fails. Rust CI retains
W01 evidence alongside existing foundation gates; no existing gate is removed.

Real-store cases cover tenant/ID/idempotency denials, create collisions, competing
same-token writers, stale owner writes/renew/release, reserve/crash/reclaim,
completed evidence preservation, arbitrary-closure accounting bypass rejection,
prepared direct mutation guards, initialization-plus-work scope/implicit-charge denials,
legacy retry evidence materialization, cancellation, exact deadline expiry, restart and
clock rollback, ready/recent index repair, and an actual timed-out reservation
that subsequently commits. Test processes create random private directories and
Unix sockets with TCP disabled. They ignore application URLs, own cleanup, and
never flush a shared store. The failed initial concurrent-test scheduling attempt
was stopped with its identified private child; the corrected test runs outside
runtime worker threads and passes. No provider requests were made.

The developer-only Python3.14.7 exporter is isolated migration evidence. It checks
pinned source bytes before imports, injects source clock/UUID factories, uses
synthetic records, and never contacts a store or provider. Native gates and runtime
never execute it. Re-capture with:

```sh
/private/tmp/slopsearx-rust-oracle-py314/bin/python -B docs/rust-rewrite/w01/oracle/export.py --source-root /clean/pinned/source --output crates/xtask/fixtures/w01/corpus.json
```

Review both artifacts before changing native digest pins. Record and inspect
failed/negative results; test counts alone do not establish distributed safety.

Portal impact: this package does not wire into the active Python server/portal or
change its browser-visible contract. Later shared-runtime integration owns the
portal contract/browser gate. The existing Python portal remains active.

`ssx_application::research::lead_id(url)` now computes the source lead-v1
SHA256 identity from accepted F11 `normalise_url` and UTF-8 bytes.
`ssx_application::research::finish_attempt` takes the captured expected AttemptId and an optional typed SearchResponse
and settles its actual result URLs through the domain accounting transition.
Use it inside `ResearchStore::mutate` after recording the current query outcome,
snapshot cursor and provenance. Domain remains independent of application.
W02 receives these APIs; this package implements no runner or provider dispatch.

The source normalizer strips its exact tracking-query set and preserves host
case, fragments, nonempty final-segment params and malformed-URL fallbacks.
Those distinctions therefore remain significant to lead identity. Source uses a
truthy cursor: null and empty cursor both produce no admissions; the native
settlement now preserves this case as well. Result metadata and engine do not
participate in the hash. Duplicate URLs still consume each admitted result slot.

A separate source capture `identity.json` records 382 actual source
`research_budget.finish_attempt` lead identities, using every accepted F11 URL
fixture, plus nine full settlement observations (duplicate, cap, prior-seen,
missing/empty cursor, missing/empty response and exhausted/lowered-cap cases).
The original W01 accounting corpus/exporter and all F11 oracle files remain
unchanged. Independent native constants pin both new artifacts; native replay
rejects altered answers even when provenance metadata is repaired. The native
gate replays the original ten multistep cases through actual application identity,
then checks private-store reservation, duplicate charging, history retention and
cap-limited second settlement using actual URL responses.

Reproduce the separate developer-only identity capture with pinned Python3.14.7:

```sh
/private/tmp/slopsearx-rust-oracle-py314/bin/python -B docs/rust-rewrite/w01/oracle/identity.py --source-root /clean/pinned/source --f11-fixtures docs/rust-rewrite/f11 --output crates/xtask/fixtures/w01/identity.json
```

The capture verifies source Git/current-file bytes before imports and records
F11 input-shard hashes. It makes no provider or store calls. Independent source
recapture/review and exact-head CI remain acceptance gates; replay alone does not
establish oracle provenance. G08 retires this executable oracle before backend
removal. Portal remains unchanged because active runtime wiring is deferred.

## Idempotency cutover

Legacy F07 key constructors and Lua bytes are unchanged. Native admission writes
only `mcp:idem-v2:<UTF-8 tenant byte length>:<literal tenant>:<literal idempotency>`.
The disjoint prefix cannot overlap a legacy `mcp:idem:` key, and length framing
separates colon-containing tenants and keys without rewriting job IDs or records.

When v2 is absent, legacy aliases are eligible only when the referenced job's
exact tenant, original idempotency key and job ID match the requested scope.
Atomic creation adopts such an alias using the smaller positive remaining TTL of
legacy mapping and job; it does not extend either legacy TTL or rewrite job bytes.
Expired, immortal, malformed, missing-target or foreign legacy aliases have no
native lookup or admission authority. Both legacy alias and record must have
positive remaining TTL; invalid legacy TTL is never repaired or extended. A malformed v2 alias or mismatched/missing v2 target fails
closed and never falls back. Existing source records and historical aliases stay
readable and unchanged. Empty idempotency keys still mean ordinary admission.

Cutover requires quiescing legacy writers before enabling native writers. The two
namespaces do not coordinate mixed old/new writers, and this package claims no
online mixed-writer migration guarantee. Ongoing cancellation after a committed
reservation remains a W02 dispatch-boundary responsibility. Real-Valkey tests
cover both the verified migration and malformed/expired/colliding aliases.

Scoped ready discovery now takes Valkey TIME and ZRANGEBYSCORE in one atomic W01
helper at the same microsecond precision as the accepted F07 ready scripts.
The prior millisecond truncation could omit a newly due job during the current
millisecond. The helper keeps the 16-item bound and never rounds future scores
into eligibility; actual lease acquisition remains authoritative. A deterministic
real-Valkey test substitutes only the Lua clock value to demonstrate the former
miss, exact due inclusion, future exclusion and the unchanged selection bound.
