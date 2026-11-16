# F02 state compatibility register

This register applies [ADR 004](../../adr/004-rust-valkey-authority-and-migration.md) to the accepted F01 [machine inventory](../inventory/core-contracts.json). It is a decision register, not a migrated-data report. Valkey rows stay **pending** until their owner supplies exact legacy bytes, Rust read/write fixtures, reverse-reader results and a real-Valkey interruption test. Process-config inputs use their owning configuration fixtures and validation instead. The F01 manifest is exhaustive; this table groups entries for migration ownership and does not redefine their names or TTLs.

| F01 state IDs | Classification and owner | Window rule and required proof |
| --- | --- | --- |
| `state.research_job`, `state.research_budget`, `state.research_idempotency` | Authoritative execution and attempt history; W01 | Preserve current fields, tenant/ID/digest bytes, request identity, attempt order, consumed budget and unknown-vs-zero outcomes. Rust-created jobs/attempts must be decoded by the pinned Python reader before rollback. Duplicate same-payload admission replays; conflict rejects. |
| `state.research_lease`, `state.research_indexes` | Lease/fence authority versus derived ready/recent hints; W02 | Claim, renew and save-if-owned Lua semantics must match under competing processes. Rebuild ready/recent hints by bounded scan. No old worker remains at Rust handoff. Cancellation/orphan state and expired lease cannot authorize stale commit. |
| `state.saved_definition`, `state.saved_schedule_lease`, `state.saved_report_baseline`, `state.saved_tombstone`, `state.saved_events` | Definitions/reports/baselines/tombstones/events authoritative; due indexes derived; W00/W04/W05 | Preserve revision CAS, schedule lease, event cursor/fence, report and baseline IDs, retention and tombstones. Reconcile authoritative records against bounded indexes and consumer cursors. Rust-created records/events must reverse-read before rollback. |
| `state.staged_record`, `state.staged_admission_index`, `state.staged_lease_budget` | Staged plan and budget authority; repairable admission hints; W11 | Preserve plan digest byte algorithm, admission limits, claimant fence and budget accounting. Validate replay after interrupted claim and reindex from records. |
| `state.snapshot`, `state.retrieval_receipt` | Immutable evidence with logical expiry and retained storage; F13/W07 | Preserve opaque IDs, canonical unsliced payload, receipt idempotency/horizon and missing/expired/unavailable distinction. Neither a view nor a reindex may rewrite prior evidence. |
| `state.oauth_client`, `state.oauth_code`, `state.oauth_access`, `state.oauth_refresh` | MCP authorization-server security authority; T04/T05 | Preserve client/token hash format, code single-use, refresh rotation/replay, expiry and revocation. New Rust-issued values must be accepted by pinned Python rollback readers, or a tested reverse migrator must run before old issuer resumes. Auth store outage fails closed. |
| `state.oauth_client_file` | Local gateway client file, **not** Valkey shared state; T07 | Preserve per-server 0600 file and token/client semantics. Migration of a user's file is separate from server cutover; native gateway must not import a backend module. |
| `state.portal_oidc_transaction`, `state.portal_session`, `state.portal_mutation`, `state.portal_rate_limit`, `state.portal_audit` | Browser OIDC/session and workflow idempotency authority, rate limits and security evidence; T09/W10/O01 | Preserve HMAC handles, session generation, single-use login, membership-revision revalidation, tenant binding and idempotency claim fences. Protected operations fail closed on unavailable authority. Audit retention/redaction survives the handoff. |
| `state.portal_membership` (process-config input; not a Valkey namespace) | Operator-provided membership directory and resolver; T09 | Migrate and validate the configured directory under T09, preserving schema, principal/tenant revisions and fail-closed startup. Keep this separate from the Valkey census and reverse-read tests. Sessions must continue to capture revisions and recheck current membership on resolve. |
| `state.engine_rate_window`, `state.upstream_cooldown` | Shared admission/cooldown authority; F10 | Preserve quota scope and TTL, classify no-cooldown separately from unavailable, and test server-authoritative time correction as an intentional difference before use. No independent Python/Rust counters during handoff. |
| `state.query_audit`, `state.engine_stats` | Observed operational evidence and bounded metrics; O01 | Keep observed/unknown/synthetic distinctions and cardinality limits. Reconcile write horizon; missing stats do not mean healthy providers. |
| `state.suggestion_cache`, `state.canonical_search_cache`, `state.answer_cache` | Recomputable cache, not durable workflow authority; F13 | Canonical search payload stays full and unsliced; request views derive at read. Check legacy `search:v2` and `answer:v2` keys/TTLs and answer-key language/SafeSearch collision before reuse. New cache namespace may be introduced only after proving no durable ID derives from it and behavior under cold cache passes. Best-effort cache outage may degrade as before. |

## Per-namespace evidence record

Each owner adds a machine-verifiable record with: F01 ID/key pattern and owner, old and new serializer versions, original and Rust-written byte fixtures, missing/null/empty/error cases, old→new and new→old reader verdicts, TTL measured at creation and after interruption, referenced IDs/indexes/leases, real-Valkey contention result where relevant, interrupted forward and reverse migration result, rollback image SHA, and disposition (`compatible`, `reverse-migrate`, `recompute`, or `blocked`). `compatible` requires a Python reverse-read of Rust-created records, not merely Rust reading old data. A `recompute` disposition is allowed only for proven derived state. The release owner records cutover generation, admitted traffic and the compatibility-window close event. Any `blocked` authoritative row blocks G04 and G07.

## Gate sequence

1. F05 establishes wire/durable DTO byte fixtures and distinguishes legacy readers from native types. F07/F10/F13, W00–W11 and T04/T09 own their rows and atomic transitions.
2. G04 runs an isolated rehearsal with sanitized representative records, real Valkey, at least two processes, interrupted migration/lease renewal and reverse-read of records produced by Rust after canary start. Reconcile completeness, decoded accuracy, TTL/timestamp timeliness and index/lease consistency. Any discrepancy is a stop condition.
3. G05 states the bounded admission interruption, RTO/RPO, backup/restore drill, rollback observation window and traffic thresholds. G06 verifies exact image/binary digests. G07 records exclusive writer handoff and canary results. G08 may remove the Python backend only after the rollback window is explicitly closed and a roll-forward/recovery path is accepted.

A pre-cutover snapshot is recovery material; restoring it after accepting Rust writes discards those writes. That is a separately approved **restore with data-loss accounting**, not a code rollback. External provider calls already made cannot be undone. When their outcome is unknown, retain the charged attempt and reconcile without claiming exactly-once remote execution.

## T09 browser authority exception (cutover remains G04/G07-gated)

T09 native browser keys use `portal:native:v1:<authority digest>:<kind>:<keyed id>`.
This is an explicit drain/relogin design, not a same-namespace rollback claim.
The module is not wired into production by default. G04/G07 must stop legacy
admission/writers, drain and reconcile workflow commands and pending browser
admissions, expire old login/session authority and require fresh native sign-in.
Rollback stops native admission, reconciles results and requires fresh legacy
sign-in; neither reader adopts the other's cookie records. Old audit evidence
remains retained/read-only under its original keys; G04 preserves/exports it with
original retention. No migration rehearsal, reverse compatibility, completed
workflow rollback or production cutover is claimed by the T09 implementation.
Membership directory remains validated operator process configuration. Browser
admission expiry never proves a prior workflow operation did not happen; W10
retains durable command idempotency beyond the 600-second browser guard horizon.


## F13 cache and snapshot compatibility evidence

F13 retains legacy snapshot keys and JSON shape, with source-to-native reads and
actual native-created bytes reverse-read by the pinned Python reader. Creation
now requires atomic SET EX NX success; unknown completion does not publish a
cursor or retry with another ID. This is an authority correction, not proof of
a completed G04/G07 rollout. Opaque legacy handles remain readable, subject to
accepted F05 invariants and independently checked tenant/handle equality.
Logical expiry remains wall epoch with strict now>expiry and physical TTL+300.

The full composed cache uses search:native:v1 and is recomputable derived state.
Source search:v2 base-key distinctions and query-only answer:v2 remain explicit.
Snapshot issuance generates independent random IDs; result IDs derive only from
snapshot ID/index. No durable identifier depends on a cache fingerprint. Thus
the composed namespace causes a cold cache on native rollout and legacy rollback,
without retagging evidence or requiring flushdb. Existing byte/depth/payload
bounds apply only at persistence; fresh canonical evidence stays complete.
G04/G07 must still rehearse mixed-version admission, rollback and retention.

## F14 client admission namespace and composed execution

F14 adds the dedicated typed `ratelimit:client:<sha256 of exact client bytes>:<server-time bucket>` namespace. Provider `ratelimit:<EngineKey>:<bucket>` bytes and cooldown keys/Lua are unchanged. EngineKey forbids colons, so this additional framed component is disjoint even from an engine named `client-<digest>`. The accepted F10 TIME/increment/expiry algorithm, denial charging and exact window retention apply to client counters. Empty/absent clients skip admission; IPv6, punctuation and whitespace-only nonempty identifiers have stable separate digests with no raw identities in keys.

This is a deliberate cold ephemeral client quota transition, not a durable record migration. G04/G07 must stop legacy writers and drain/expire prior windows or explicitly account for the admission reset. Old and native counters coexist under their original TTLs; rollback likewise starts legacy client accounting cold. Provider quota writer handoff remains the existing F10 obligation. Real private-Valkey tests verify exact key bytes, independent counts/TTLs, custom engine noncollision, unavailable policy and terminal-close denial. No production reset, drain or rollback rehearsal is claimed.

F14 composes canonical cache admission from actual execution outcomes: deadline-truncated evidence and downstream rerank fallback are skipped, while full fresh results survive oversized/failed optional writes. Source suggestion keys/1800s TTL stay literal; malformed typed suggestion data is a cache miss or provider fallback. New logical adapter generation/scope-order routing extensions cold the recomputable search cache without changing any durable snapshot/result IDs. [ADR 007](../../adr/007-native-shared-search-execution.md) records the lifecycle, local-calendar and malformed-Brave boundaries.

PR874 date evidence is an additive optional closed field. Unbounded native serialization omits it, retaining accepted legacy fixture bytes; the new Python writer emits null, and both readers treat missing/null as no proof. Bounded legacy cache records miss until fresh retrieval supplies exact evidence. Current scope and request bounds remain authoritative over persisted scope/proof. Wrong provenance or malformed proof rejects optional storage without losing fresh complete results. Snapshot evidence carries a valid proof through the existing bounded codec; no identifiers or durable state keys change.

## A04 subreddit cache identity integration

`SearchRequest.subreddit = None` retains every existing `search:v2:<digest>` base
key byte. Some(exact string), including empty, uses the disjoint base namespace
`search:v2:subreddit:<sha256(JSON([legacy-base-key, exact-subreddit]))>`. The
accepted `search:native:v1` composed key and singleflight bind this base key;
validated dispatch retains the identical optional value. Explicit empty is
separate from omission (`/r//search.json` versus `/r/all/search.json`). General
categories may ignore subreddit at the provider but remain conservatively
separated in cache identity. This is additive recomputable cache state, not a
durable migration: old omitted-field caches remain valid, newly scoped entries
start cold and expire normally, and snapshot/result identifiers stay unchanged.
No public wire parameter or production rollout is introduced.

## F15 ephemeral reranking identity and read-boundary notes

The concrete native host automatically binds the provider/model/rubric/limits/parser/host-ordering identity into cache and singleflight identity. Explicit manual request/config routing identity cannot mask that concrete identity. This intentionally creates a cold ephemeral namespace for the native host; prior cache keys coexist and expire under their existing TTL. No durable state schema, migration or rewrite is introduced. Per-caller advisories remain outside canonical cache and snapshot evidence and derive from the current trusted policy/catalog and full canonical count. Shared strict-JSON decimal normalization corrects finite IEEE conversion for source-valid long numeric tokens; SourceJSON's existing nonfinite policy remains caller-owned, and unrelated Python float-string consumers are not declared fixed by this change.
