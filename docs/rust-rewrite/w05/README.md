# W05 native saved event outbox

Candidate implementation for issue910. Native code publishes report, baseline,
definition schedule and stream event in one fenced Valkey script. The existing
six-key W04 commit script remains byte exact; event-enabled publication extends
its accepted ownership, raw-CAS, TTL, type and report-eviction fences. Read and
acknowledgement scripts trim retention independently of publication and preserve
tenant/consumer isolation.

`cargo xtask check W05 --output <new-evidence-path>` verifies frozen source and
case identities, exact native test identities, zero ignored required tests, and
completed semantic case receipts after assertions and resource cleanup. It needs
an explicit `SLOPSEARX_TEST_VALKEY_SERVER` executable reporting Valkey8.1.10. The
fixture server listens only on private Unix sockets. Actual MCP tests bind
127.0.0.1 and build a proxy-disabled client before connecting to that exact URL.
No test calls a real provider, production store or external callback.

The source-bound event corpus records eight original envelope/redaction cases
and17 validation inputs. The separate semantic manifest enumerates native
storage/service/protocol completion identities. Original source function and
parameter mapping is documented in [PARITY.md](PARITY.md). A passing count by
itself is insufficient.

`capture_source.py` is an optional manual original reference oracle, explicitly
inventoried in [oracle-inventory.json](oracle-inventory.json) for G08 retirement.
It refuses execution without `--allow-original-oracle`, pins Python3.14.7 with
Unicode16, disables connection methods before backend imports, and exclusively
creates its requested output. Native runtime and mandatory native gates never
execute Python. Existing captures and immutable inputs are not overwritten.

The first capture attempt failed during stdlib SSL import because replacing the
socket class prevented subclassing. The successful producer blocks connect,
connect_ex and create_connection instead. Capture identity and the failure are
retained in [oracle-runs.json](oracle-runs.json); subsequent formatting does not
relabel the historical producer bytes.

EventObservation is a closed typed observation port with validated finite,
nonnegative backlog age. It emits publication success only after a definite
accepted write and acknowledges advanced/idempotent outcomes from one atomic
reply. SharedEventObserver exposes fifteen fixed counters through the existing
shared ssx_application::Metrics API and forwards the unchanged typed observation,
including validated backlog age. Actual store callbacks prove publication and
ack counters follow committed authority; rejected/duplicate operations cannot
report publication success. NoopEventObserver explicitly means absent downstream
composition. O01 owns rendered/exported metrics and the age gauge; W09 owns
complete authenticated workflow composition. The explicit source metric mapping,
composed W06 qualification and preserved failures are in [HANDOFF.md](HANDOFF.md).
The focused native SavedEventRegistry binds create/read/ack to actual services
and accepts a trusted typed principal from its caller. Its private-loopback
fixture does not certify T05 auth integration or a deployed production surface.

Portal impact: this additive Rust capability does not change the current Python
portal or its browser contract. No HTML route is introduced. Full portal/native
composition and browser qualification remain W09/W10/G01 obligations.

No deployment or live acquisition is authorized. Valkey fencing prevents a
second valid publication; unknown remote execution and unknown write completion
remain unknown and are never described as exactly-once delivery.

## Existing acknowledgement migration and recovery

Native acknowledgement authority uses
`mcp:saved:v2:event-ack:{tenant_utf8_byte_length}:{tenant}:{consumer}`. Streams
and consumer-capacity indexes retain their v1 keys. The accepted tenant and
consumer grammars remain unchanged. UTF8 byte length makes the native key
unambiguous even when both names contain colons.

The original v1 acknowledgement suffix can collide: tenant `a`, consumer `b:c`
and tenant `a:b`, consumer `c` share one key. Actual original/source-exact Lua
and initial-native behavior is preserved in the private characterization under
[evidence](evidence/legacy-collision-characterization-v1.log.json). This is an explicit
correctness correction; the original isolation claim is not treated as evidence.

A valid v2 cursor takes precedence. With no native cursor, a missing v1 cursor
means `0-0`. A present legacy cursor is automatically attributed only when the
entire suffix has exactly one valid tenant/consumer partition. Reading such a
cursor does not write an acknowledgement; the next explicit ack migrates it
monotonically and reuses its existing consumer-capacity slot.

A present ambiguous legacy cursor produces `cursor_conflict` for a default
read before any mutation. A client can explicitly read from `0-0` (or another
known retained cursor), process the replay, and explicitly acknowledge a
retained event. The replay reports native `acknowledged_cursor: "0-0"` and
`acknowledgement_recovery: {state: "legacy_ambiguous_ignored", action:
"acknowledge_a_retained_event"}`. This establishes only that client's tenant and
consumer v2 authority; it never attributes, overwrites or deletes ambiguous v1
state. Malformed native authority fails closed even with an explicit cursor.

This adds a machine-readable MCP recovery error/state. Current Python portal
routes and browser behavior are unchanged; W09/W10 native portal consumers must
preserve and explain these recovery instructions. G04 must drain legacy
acknowledgement writers before migration. Native readers never follow subsequent
v1 writes after v2 exists; mixed legacy/native acknowledgement writers are not a
qualified deployment configuration.

The actual registry lifecycle also fences invocation admission against close.
Already-admitted create calls can finish registration; close then signals and
joins their owned work. Calls admitted after close return an explicit closed
error before store mutation. A private Valkey WRITE pause makes the concurrent
create/close assertion causal, and repeated close is idempotent.

## Lossless historic payload transport

The original SDK defect remains immutable in
[evidence](evidence/numeric-wire-defect-v1.json): structuredContent rounded two
adjacent integers above u64 while text preserved them. The current focused HTTP
mount uses the shared trusted native-handler wire restoration. The handler
supplies exact object text and a structured placeholder; only the three admitted
saved tools can restore that text, and the internal marker is removed.

Actual HTTP bytes are decoded through RawValue envelopes and checked one event
at a time with ssx_json. All 300 exact-size retained events across three batches
preserve distinct 18446744073709551617/18446744073709551618, a literal
`$serde_json::private::Number` object, 98 nested arrays and every original public
field. The typed SDK client is not the numeric oracle. The full W05 gate requires
105 exact completed cases, preserving all prior 99.

The focused HTTP helper requires an explicit NetworkAuth mount and retains its
managed session cleanup. Missing and wrong bearer credentials return 401 before
any definition or engine dispatch. Event-grant denial leaves event/consumer/ack
keys absent while the ordinary saved report still completes. This fixture uses
a fixed trusted principal; it does not certify complete production identity,
live grant reload, W05 stdio/legacy mounts, or gateway composition. W09 owns
those production integration obligations.

## Maximum retained event batches

A read may return 100 events of up to 64 KiB each. The service constructs the
typed event array directly; reparsing this complete batch through the default
4 MiB JSON parser panicked. The original panic, pending HTTP request, explicit
owned-process cleanup and exact test-source restoration are preserved in
[evidence](evidence/max-batch-initial-panic-v1.json).

The permanent actual-HTTP regression reads all 100 exact-size events for plain,
quote/backslash and control-heavy unknown fields. It compares every event and
its original string through RawValue envelopes and per-event ssx_json parsing.
The shared-profile batches measure 13,116,620, 19,625,420 and 16,371,020 HTTP
bytes with the historic numeric/nesting fields included. Historical measurements
without those fields remain in the earlier evidence. Whole-batch parsing is
never used as the oracle; default ssx_json document limits remain unchanged.

The focused HTTP composition opts into a typed 32 MiB output profile. Every input
remains bounded at 4 MiB with 32 pending requests, and the saved handler retains
its 64 KiB argument limit. Other HTTP mounts keep their 4 MiB output
default; the legacy transport retains its 16 MiB output default. For 100 events, allowing 256 extra public bytes each and 4096 envelope
bytes gives N <= 100*(65536+256)+4096 = 6,583,296. JSON text escaping plus the raw
structured copy is bounded by 3*N+4096 = 19,753,984 < 32 MiB. The real quote,
backslash and control-heavy batches qualify this bound. Shared regressions
separately reject one byte beyond the configured output profile.

The separate [nesting capture](evidence/nesting-characterization-v1.json) also
proves that a valid retained event with 98 nested arrays reaches both HTTP
payloads exactly. Wrapping it in the event batch exceeds the default whole-value
depth bound even though each event remains valid. The actual shared HTTP
profile now preserves this boundary in every maximum-size batch; per-event input
limits remain unchanged.
