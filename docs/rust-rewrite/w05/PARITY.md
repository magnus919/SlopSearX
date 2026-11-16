# W05 original-source mapping and bounded evidence

Original source SHA256 bindings in source.json cover saved_events.py,
saved_store.py, saved_runner.py, MCP tool boundaries and all three event test
files at93db215de1c5b59ffdcac035a79ffcbd8e806762. test_saved_event_metrics.py is
also bound, with exporter assertions explicitly retained as downstream work.
The original23 test functions include six invalid-input parameters and three
report-classification parameters; their execution is not inferred from counts.

| Original regression | Native executable contract / case |
| --- | --- |
| ordered bounded repeated read without ack | ordered_read_ack_restart_tenant_capacity_and_revocation / ordered-bounded-repeat-no-ack |
| idempotent monotonic ack across consumer restart | same test / ack-monotonic-idempotent-restart; actual new MCP client resume |
| tenant and consumer fences | same test / tenant-consumer-fences; cross-tenant claim rejection |
| consumer capacity | consumer-capacity; no additional consumer admitted |
| report+event capacity rejection | competing_claims_fenced_atomic_publication_and_capacity_rejection; report/baseline/definition unchanged |
| pause capacity keeps active revision | pause_expiry_atomic_idempotence_capacity_and_retained_horizons |
| retention gap independent of ack | retention_trim_read_ack_empty_gap_and_exact_large_cursors |
| read trims without another publication | read-trims-without-publisher; actual XLEN decreases |
| ack rejects stale untrimmed cursor | ack-trims-without-read; independent fresh tenant stream |
| revoked policy redacts protected detail | policy-revocation-redaction; saved grant and engine policy independently revoked |
| pause/expiry idempotence | pause-revision-idempotence, expiry-atomic-idempotence; stale revision cannot append another pause event |
| six invalid consumer/cursor parameters | invalid-no-mutation-0..5; actual private Valkey DBSIZE stays0 |
| separate grant blocks read/new events while reports still publish | separate-event-grant; actual-mcp-separate-event-grant-denied waits for committed ordinary report |
| three report classifications | created-complete, incomparable-partial, dispatch-precedence source goldens; operational-error precedence also captured |
| no callback/outbound URL parameters | actual MCP descriptors exclude all four names and actual client call with callback_url rejects |
| competing eight claimants | competing-eight-claims-one-report-event, duplicate-commit-fenced; exactly one report/baseline/event publication |
| real-Valkey capacity report/event atomicity | capacity-report-baseline-definition-event-atomic; trim-only source rejection side effect is explicit |
| real-Valkey ordered read/ack restart and tenant isolation | ordered-bounded-repeat-no-ack, ack-monotonic-idempotent-restart, tenant-consumer-fences |
| real-Valkey pause/expiry atomic/idempotent | pause and expiry capacity rejection leave definition/marker untouched; eventual publication occurs once |
| real-Valkey independent read trim | read-trims-without-publisher |
| real-Valkey independent ack trim | ack-trims-without-read |
| production FastMCP create/read/restart/ack journey | actual native EventService/SavedRunner/SavedStore through real rmcp streamable HTTP and new loopback client; no synthetic Registry response |
| production FastMCP separate grant denial | actual rmcp denial with exact source error envelope |

Additional native cases preserve opaque18-byte event IDs, Unicode ASCII-escaped
64KiB size accounting, null/absence/redaction projection, canonical64-character
cursor grammar, exact adjacent decimal values beyond2^53, oversized wire-ID
components, empty-stream gaps, stream retention versus event timestamp, physical
retention+300s TTL, valid old-timestamp/max-sequence rollover, corrupt extreme
stream ID rejection, stale event-enabled claim fencing and no success observation
on a store error. Source and semantic manifests are immutable gate inputs.

## Explicit integration limits

SharedEventObserver exposes all fifteen finite outcome counters through the
existing shared Metrics API. Its actual private-Valkey recorder observes the
committed report/baseline/definition/event and authoritative ack cursor inside
increment; duplicate/error/capacity controls produce no publication success.
The typed downstream receives the identical observation and validated age.
Three actual causal mutants reject premature publication, premature ack and
omitted downstream forwarding, then restore exact bytes. See [HANDOFF.md](HANDOFF.md)
for every static counter's source metric/outcome mapping, including the extra
retention-gap projection and absent-age empty/gap gauge reset. Source tests for
rendered Counter/Gauge output remain O01 work; this bridge does not establish an
exporter, scraping or production lifecycle. W09 must bind production composition. Static authority composition
does not claim live production grant reload; the mutable native fixture proves
boundary behavior only. The fixed trusted-principal transport fixture proves
actual native handlers and serialization, not full T05 identity resolution or
production deployment. Full release G01/G02/G04/G08 remain required.

## Additional tenant/recovery and lifecycle correction evidence

The v2 migration fixture independently recovers both colliding owners, verifies
UTF8 length-key uniqueness, preserves legacy bytes, reuses existing consumer
capacity, restarts both owners, quarantines ambiguous default reads without
mutation, and refuses malformed native authority even with explicit replay.
Actual MCP calls verify the `cursor_conflict`, recovery object, explicit replay
and explicit acknowledgement wire path. Mixed legacy/native writer drain is a
G04 migration requirement, not an inferred capability.

An actual Valkey WRITE-paused create holds registry admission while close waits;
after release, the same close joins registered work. Late client create calls
fail before adding a definition, and repeated close is idempotent. An actual
expiry-capacity scheduler fixture waits for rejected expiry observations before
asserting zero due-engine calls, frees only the private test stream, and proves
the same runner publishes the expiry/report pair and joins explicit shutdown.

Maximum-batch coverage also executes 100 retained events at exactly 65,536
stored bytes each, including worst quote/backslash and control-heavy strings.
Every delivered event is compared through a RawValue envelope and per-event
lossless parser; no whole large payload is admitted to the default 4 MiB parser.
The original complete-batch parser panic and explicit owned-fixture termination
remain historical failure evidence.

The current focused HTTP mount composes the shared trusted native-handler wire
restoration and typed 32 MiB output profile with an actual NetworkAuth gate. The
permanent maximum batches preserve the original string oracle and independently
check adjacent integers beyond u64, a literal reserved number-tag object and 98
nested arrays, using RawValue envelopes and per-event lossless parsing. The 105
case manifest retains all prior 99 identities. Missing/wrong credentials cause 401
before mutation; separate event grant denial leaves all event/ack keys absent.
The default HTTP 4 MiB input/output limits and 32 pending requests remain
unchanged, as does the legacy 16 MiB output default. This W05 evidence applies to its actual HTTP mount; shared stdio/legacy mechanisms do
not imply W05 mounts or complete production principal resolution.
