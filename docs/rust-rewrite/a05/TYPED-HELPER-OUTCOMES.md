# Typed helper outcomes and deterministic bootstrap success

Candidate component, before bounded binary helper adoption. Parent-reviewed
inherited operation deadline starts after F10 admission and includes queue,
formatting, HTTP and child work. This slice does not close aggregate allocation,
no-emission codec CPU, RSS, FD, GNU or full platform qualification.

Operation-facing helper errors distinguish Deadline, Resource, Cancelled,
Shutdown and ordinary Failure(String). Public legacy parser methods retain
String results and distinct `HTML parser shutdown` / `HTML parser cancelled`
diagnostics. Ordinary source failures retain their original message, DDG lite
fallback eligibility and proxy-failure ordering. Deadline is terminal Timeout;
Resource is terminal Error with `HTML resource budget exceeded`; cancellation
and shutdown are terminal Error with their exact native diagnostic. Internal
resource/cancellation outcomes do not penalize DDG's proxy. Google still records
success between decode/challenge and parse, so a subsequent parse deadline keeps
that prior success event and does not add failure feedback. This is explicit
native deadline/resource hardening, not a claim Python has the same total timer.
No additional endpoint or bootstrap request follows a terminal helper outcome.

Both engines retain an Arc<ParserPool> dependency. Production construction uses
the existing shared pool; an engine-private constructor allows owned test pools.
The same F10 admission and operation budget/deadline path applies to injected
pools. Closing an injected pool does not close the unrelated default pool.

The engine-local unit fixture executes ten loopback/real-process scenarios:

| Mode, each DDG and Google | HTTP requests DDG / Google | Worker PID records DDG / Google | Result and feedback |
| --- | --- | --- | --- |
| Deadline | bootstrap+primary (2) / search (1) | 1 / 1 stalled parse PID | Timeout; DDG false after expiry; Google true already observed before blocked parse |
| Resource | 2 / 1 | 1 / 1 | Oversized reply rejected, terminal Error; no proxy feedback |
| Shutdown | 2 / 1 | 1 / 1 | Owned pool closes during blocked decode; terminal cancellation/shutdown Error; no proxy feedback |
| Source failure | bootstrap+primary+lite (3) / search (1) | 2 / 1 | Original failure diagnostic; DDG retries lite and records false,false; Google no feedback before failed decode |
| Quota | 0 / 0 | 0 / 0 | Immediate operation budget refusal; no worker spawn or fallback |

Deadline mode delegates actual decode to the native helper before stalling the
parse invocation. Resource mode consumes the owned request and writes an
out-of-bound binary length. Source mode returns the exact stage-tagged failure
frame. Each fixture owns an exclusively created directory and executable; PID
readers require a complete positive newline record. Shutdown and verified reap
precede final PID/live/status/count assertions and fixture removal. Only signal
zero checks are used against recorded owned PIDs; no broad cleanup occurs. The
quota control proves operation admission refusal, not yet a child decoder's
allocation failure. These fixtures do not qualify hostile/no-emission bounded
codec cancellation or every outer-future-drop path.

The old warm-cookie integration run failed a cookie assertion and is retained at
`/private/tmp/a05-typed-helper-errors-regression-r2.log`. Host load is a possible
cause, not a proven diagnosis. Pausing Tokio alone is insufficient: F08 converts
the HTTP decoder deadline with `into_std()` and compares `std::Instant::now()`.
The warm-cookie test is therefore relocated, retaining all six ordered search
requests, exact cookies/headers/results and two fresh-search isolation checks,
to a private cfg(test)-only two-second successful-bootstrap budget. It has a
response-completion handshake and bounded handler cleanup. Production's
`min(config.bootstrap_timeout_ms, 5.0)/1000.0` clamp/default is unchanged; no
operator configuration or non-test override exists. The separate explicit cold
bootstrap test continues to exercise the unchanged five-millisecond source cap.
The warm test proves cookie/order/isolation given bootstrap success, not cold
five-millisecond wall-time parity. The one warm transport preparation request
is separately asserted before the six search requests, preserving accounting.

The A05 gate now requires twelve integration tests (all587 original complete
outcomes/requests included), plus the exact two engine-local helper unit names.
Source successor mappings explicitly name the relocated cookie test. Empty,
missing, extra or ignored helper tests fail the inventory check. Other pending
codec/provenance gate inventory work remains open; this is not a full gate pass.

Portal impact: existing Timeout/Error status and error-message fields carry the
outcomes through the shared service. No wire schema, formatter, routing or portal
markup changes are introduced. Existing full portal/package gates remain release
requirements; this component's native tests do not claim browser qualification.

Qualification notes: the old helper fixture PID writers originally omitted a
newline. Strengthening only the reader caused five fixture timeouts in retained
r2/r3 integration logs; both writers now emit complete records, liveness checks
trim validated records, and PID-wait timeout awaits owned pool shutdown before
panic. The corrected r4 integration run passed all twelve tests. Every injected
worker also records entry before reading stdin, so quota requires zero entry
records, and all entry PIDs (including the delegated decoder) are checked reaped.
The ten scenario IDs are emitted and required by the gate. Final source has
2/2 helper units, 12/12 integration tests, strict infra library/all-test Clippy
and xtask compile checks passing; the full A05 gate remains pending.
