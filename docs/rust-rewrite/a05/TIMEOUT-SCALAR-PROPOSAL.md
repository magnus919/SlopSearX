# Layered timeout scalars and actual transport evidence

Status: source evidence and proposed local projection; exceptional timer policy
is not implemented or accepted. The previously approved post-F10 total operation
deadline remains an intentional native hardening, pending concrete qualification.

`timeout-config-source.json` retains 38 actual layered YAML/environment controls;
`timeout-config-extra-source.json` adds 18 controls without changing those bytes.
Both use original adapters and MockTransport: their request timeout extensions
prove declared arithmetic only. YAML annotations do not validate `EngineEntry`;
F06 retains scalar bool/int/float/text, rejects nonscalars and YAML nonfinite
numbers, but an environment overlay on a float field permits nonfinite values.
Python `timeout_ms / 1000.0` performs bool/integer-to-float arithmetic; text raises
TypeError outside the adapter try block. Native projection must consume the raw
ConfigScalar, never `effective_f64`'s numeric-text coercion. F06-rejected values
must remain separate from reachable scalar behavior. G01 owns real loader through
factory through adapter reconciliation across all batches.

`timeout-loopback-source.json` contains 48 independent original-adapter processes
using real HTTPX sockets and private loopback servers. The separate stalled file
adds 12 processes. The parent process owns the server, captures raw request heads,
response bytes, complete AdapterResponse or stdout/stderr/exit status, and applies
a three-second external watchdog. The original harness did not explicitly override Python ThreadingHTTPServer
daemon_threads=True: its server_close did not join handlers. Those 60 records
remain historical, without a handler-lifetime guarantee. The owned-source and
stalled-owned-source supplements rerun all60 with explicit daemon_threads=False
and block_on_close=True, verify every request completion timestamp precedes
server_close, and join serve_forever before the next case; child kill is followed by communicate/wait. Ports, wall timings and
tracebacks are retained raw and are observational evidence, not deterministic
byte-equality expectations. No provider or proxy endpoint is contacted.

Observed on the recorded macOS/Python3.14.7 runtime:

- Zero, negative and negative infinity return Timeout. Google has no received
  HTTP head. DDG receives its bootstrap GET, whose independent five-millisecond
  timeout is swallowed, then its primary expires before a received head. No lite
  request occurs. A server observation cannot prove a client never began connect.
- Positive infinity succeeds with immediate and 80ms-delayed primary responses.
  DDG's ordinary unusable-primary branch still invokes lite. With a five-second
  stalled primary, the external watchdog kills and reaps both adapters at three
  seconds after the primary request is observed. This is watchdog cancellation,
  not a source timeout classification.
- NaN permits primary request receipt, then this platform's kqueue selector raises
  TypeError during asyncio execution/cleanup; the process exits one, no JSON
  AdapterResponse is emitted, and the full traceback is retained. Other adapters
  have exhibited a fast success plus cleanup diagnostics: this observation is
  not a universal platform-independent source classification.
- Finite1000ms with stalled primary returns Timeout in both engines; DDG does
  not invoke lite after terminal timeout. Slow bootstrap still permits ordinary
  primary success for the positive control.

Proposed dispositions requiring the remaining policy review:

1. Preserve finite positive arithmetic without truncating fractional milliseconds;
   checked Duration and Instant conversion must never panic. Retain exact numeric
   source metadata in tests, including i64 and f64 boundary cases.
2. Text becomes typed Error retaining the source TypeError diagnostic, as an
   explicit correction to the no-unhandled-exception invariant.
3. Finite nonpositive values exhaust the native total deadline. Parent approved
   recording suppressed DDG bootstrap as a deliberate total-deadline difference;
   do not claim source request-count parity for this branch.
4. NaN needs a sanitized typed invalid-timer error under the same invariant;
   acceptance requires exact diagnostic and no request/cleanup ambiguity.
5. Positive infinity and positive finite values too large for native timers must
   preserve success within a supplied finite lifecycle deadline if available.
   A05's historical SearchEngine port has only `search(&SearchRequest)`, without
   an inherited absolute deadline. `interactive_timeout_ms` is optional duration
   data and cannot reconstruct elapsed service time. An accepted service deadline
   seam or an explicitly reviewed finite lifecycle ceiling is needed; silently
   substituting default10000ms, JSON null, or pre-dispatch Error is not proposed.
   Negative infinity's observed source Timeout needs its own explicit disposition.

The live G01 factory criterion does not by itself implement or qualify these local
branches. Full bounded helper cleanup, controlled-clock tests, exact timeout/error
classifications and ordinary 587-outcome regression remain outstanding.

Reproduction (offline migration tooling only; retire under G08):

```
/private/tmp/slopsearx-rust-oracle-py314/bin/python -O docs/rust-rewrite/a05/oracle/timeout_config_source.py --output /private/tmp/timeout-config-source.json
/private/tmp/slopsearx-rust-oracle-py314/bin/python -O docs/rust-rewrite/a05/oracle/timeout_config_source.py --extended-only --output /private/tmp/timeout-config-extra-source.json
/private/tmp/slopsearx-rust-oracle-py314/bin/python -O docs/rust-rewrite/a05/oracle/timeout_loopback_source.py --output /private/tmp/timeout-loopback-source.json
/private/tmp/slopsearx-rust-oracle-py314/bin/python -O docs/rust-rewrite/a05/oracle/timeout_loopback_source.py --stall-only --output /private/tmp/timeout-loopback-stalled-source.json
```

Correction supplement: timeout-loopback-owned-source.json (48) and
timeout-loopback-stalled-owned-source.json (12) retain actual non-daemon
handler policy and completion ordering. All60 preserve the previously observed
status/process-exit/watchdog and method/path-order behavior. The initial60 files
and frozen receipt remain unchanged. `--output` and `--stall-only` commands above
now use the corrected exporter; elapsed telemetry and ports remain nondeterministic.
