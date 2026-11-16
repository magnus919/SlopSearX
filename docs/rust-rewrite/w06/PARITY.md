# W06 source contract and explicit differences

## Plans, validation and replay

The source validates query, exact objective keys, strict integer deadline then
call bounds, paired fallback/consent, filters in SafeSearch/freshness/time-range
order, initial then fallback scopes, overlap, aggregate budget, the shared
engine policy gate and finally strict SafeSearch over the entire plan. Native
source replay compares full envelopes and canonical identity bytes, including
requested and effective objectives and captured routing metadata. A per-stage
policy denial cannot silently replace an earlier aggregate conflict. Scope
engine container/count/item failures preserve the original complete envelope.
Fallback/consent pairing uses Python JSON truthiness before scope validation.
The 16 exact-number controls include signed integers of magnitude 10^400,
signed integer/float zeros and finite signed 1e300 values. Truthiness compares
integers exactly rather than rounding or classifying them through f64.

Replay compares original requested objectives/scopes, normalized default
filters, query after Python whitespace trimming and expansion consent. It
does not recompute identity under a later cap. Replay preserves Python's
equality for equal-valued float/integer/boolean requested objectives, and
ignores nonmap filters while normalizing their defaults. Fresh planning still
requires strict integer objectives. The public-input supplement compares the
complete retained response and record for these branches. Full current policy
validation still rejects a retained operation when limits or captured intent grants have
narrowed. Saved-source resolution occurs before idempotency lookup; query plus
source is rejected before source resolution. A replay cannot hide a missing,
unavailable, truncated, incomparable or currently denied source.

The native typed language field deliberately rejects nonstring language as
`invalid_input` / `filters.language` at planning, before admission. The Python
plan builder accepts that malformed value. Supplemental source captures in
`evidence/language-input-source-outcomes.json` execute the original runner and
full shared SearchService with permissive offline fixture engines: null,
boolean, integer, list and map languages all complete with one fixture engine
call. They do not demonstrate any live provider's acceptance or later failure.
Native rejection is a reviewed input-hardening difference at the typed string
boundary; valid strings retain source behavior.

## Completion order and budgets

The runner passes explicit captured engines, remaining absolute deadline as
the interactive timeout, disabled suggestions, and `operation:attempt`
execution isolation to shared search. Canonical snapshot creation precedes
presentation. A snapshot selects its attempt even when rows are empty or the
response subsequently fails the stage. Nonempty partial/error/deadline results
fail before the success branch while retaining available canonical evidence.

Fallback requires no rows, no cache hit, no partial/deadline, an outcome count
equal to the captured stage engine count and every outcome status `ok`. The
literal source predicate does not compare outcome engine names; missing
outcomes do not authorize empty fallback. It accepts nonempty responses with
missing outcomes when no other failure flag exists. These isolated source
branches are captured alongside actual shared-service acquisition tests;
the capture does not claim shared search can produce impossible outcomes.

Initial nonempty skips later stages with `initial_nonempty`. Clean empty
initial queues fallback; final clean empty completes. Cached empty fails with
unmet fallback `initial_stage_not_clean`; execution failures use
`initial_stage_failed`. Exceptions preserve null per-attempt observed calls
and make the aggregate unknown. Cached calls observe zero; other responses
observe the actual dispatched count. Reservations are never refunded and an
unknown aggregate remains unknown through retries and successful fallback.

The accepted W11 store retains strict `expires_at <= now` logical expiry,
full schema/budget validation, immutable prior attempts and narrow cjson
empty-map normalization. Source memory-store terminal claim behavior is not
used to weaken the accepted server-authoritative deadline/budget claim path.

## Authority and lifecycle hardening

Fresh complete-record authority is checked at admission, replay, reads,
dispatch and publication. It covers all captured stages, including unused
fallback, current intent routing/grants and uniform sensitive-engine policy.
Retained explicit scopes require known, enabled engines and the uniform
sensitive grant. An engine disappearing from the active runtime, missing
credentials, an open circuit or unavailable health does not itself revoke
retained history. Intent scopes are rerouted and compared with their captured
engines. Fresh explicit admission checks current active scope. Missing
credentials or an open circuit may admit a queued record, with no reservation or attempt;
actual shared acquisition applies its scheduler/auth checks. The retained
history check issues no dispatch token. The 13 complete original
policy envelopes are frozen separately under `evidence/retained-policy-*`.
Static native authority is immutable configuration. Mutable fixtures establish
boundary behavior; they do not prove production hot reload.

The source renews around acquisition but stops its renewal task before
snapshot/save, and a lost renewal alone does not cancel the acquisition. Native
renewal continues across ownership-dependent acquisition, snapshot and fenced
save phases. Lease loss/shutdown/revoked authority cancel and join owned work,
including blocked I/O, while preserving unrelated shared-service flights.
This is explicit lifecycle hardening, not literal source parity.

After acquisition, revocation detected at the post-acquisition or immediately
pre-save check publishes no new result reference, terminal record or event.
The reservation remains conservatively charged for lease-expiry interruption
recovery. The authority trait has no change notification and supplies no
atomic policy fence with Valkey. A policy change after SAVE was sent can race
an acknowledged commit; that commit remains a commit. Unknown remote requests
or writes are not rolled back. The actual final pre-save test permits the
post-acquisition check, then flips the grant; a RESP trace proves that final validation sends no SAVE. Original
source controls instead persist a completed record when authority flips after
snapshot persistence. These are documented reference controls, not equivalent
input executions. Snapshot orphaning is possible; a stale owner cannot publish
its reference. No exactly-once provider execution claim is made.

The additive acknowledged interruption script preserves the accepted original
script/protocol and wrappers. Recovery and transition callbacks arise only
from definite acknowledgments. They are nondurable observation seams, not an
outbox; O01/W09 own concrete metric/exporter composition. Reconstructing an
event from a later racy record read does not establish an acknowledgment.
The queued-to-running callback carries the exact acknowledged claim record;
an unknown claim reply emits no inferred event even when Valkey committed it.

## Saved sources and result views

Saved source resolution checks saved grant, availability, current definition
policy, membership under the current bounded report listing, report truncation
reasons before generic incomparable status/reasons, a complete observation
window, and original report engines. The historical report query is
authoritative after definition edits. The stored source preserves search ID,
run ID, status and revision; derived-from lineage uses the exact artifact.
The additive bounded `recent_report` path checks current known/enabled/
sensitive authority for the definition and historical report. It does not
apply creation fingerprint, credential, category or runtime-active restrictions
that the original composition resolver omits. The established W04 `report()`
API retains its own behavior. The 13 source authority envelopes exercise this
through the real native saved store. Original saved-source absence is
`source_not_found`; the source resolver does
not invent an expired-report envelope where it only sees a missing report.

Reads find the selected attempt across all histories and resolve its tenant
snapshot. Missing/expired handles differ from available-empty snapshots.
`include` and `max_results` affect presentation; full snapshots remain intact.
Invalid include values preserve the exact public envelope; an empty include
list selects the default engine_status/results view. Malformed artifact
shape/type/kind/id/contract diagnostics stay distinct; a valid wrong kind
retains destination/source_kind on unsupported_transition. A structurally
valid composite run component containing a colon that is absent from the
bounded report list returns source_not_found. Required-input checks, trimmed
key bounds, source conflicts, plan conflicts and store-unavailable errors
preserve their original precedence, including a real lookup/admission race.
Remaining budget is limit minus reserved spend. Wire registration, tool grants,
MCP schemas, prompts, public response transport and concrete observability
remain W09 integration obligations; downstream source manifests/dossiers are
W08/W09. They are not established by a function-to-test mapping.

Portal impact: W06 adds native application/store/runner services and a native
projection seam; it does not activate portal routes. W09/W10 must exercise the
composed workflow through real protocol/browser paths and the required portal
gate before cutover. Existing Python portal contracts remain the baseline.

## Explicit operational diagnostic and comparison differences

The native unexpected typed shared-acquisition error uses the reviewed public
message `shared search acquisition failed`. An actual closed native service
test compares the full acknowledged terminal record/event with reserved spend
retained and observed spend unknown. The separate original shared-rate-denial
control raises a real RateLimitExceededError whose str is empty; its runner
persists `execution_failed` with an empty message. These operational triggers
differ and do not form an identical-input parity claim. A renewal-ack deadline
control confirms the original exact message `deadline expired before dispatch`.

Fresh native public-admission comparisons bind one opaque operation identity
to the deterministic source ID and augment only the separately accepted F09
pagination-enforcement entry. All other complete output/stored-record fields
are compared. Seeded replay records retain their exact captured shape. Original
runner full-record comparisons remove only the memory fallback's cleared owner
token; durable W11 Lua retains its acknowledged token. The four supplemental
runner controls remain reference-only; they are excluded from executed source
IDs by the qualification manifest and gate.
