# Native observability and supervision

O01 provides native metrics, audit/stat delivery, operational diagnostics and
bounded telemetry shutdown APIs. `cargo xtask check O01` executes native tests
with private Valkey and loopback endpoints; it never executes Python or calls a
provider. The gate binds complete source-case records and exact compiled tests.

The original five test files contain44 functions. Native assertions cover42
owned functions/53parameter cases, plus39supplemental source controls.
`original44-map.json` lists every function and its consumers; the two actual
HTTP `/metrics`/request-normalization cases are explicit T02 obligations.
`coverage.json` classifies older captures and reference-only rows separately.
Passing O01 APIs does not establish server/worker bootstrap activation.

`fixed-metric-families.json` lists the35 fixed names, help, kinds and labels.
Engine/category catalogs allow at most256entries of128bytes, including input
work on duplicates. Unknown names/values and duplicate/missing labels fail;
series retention is bounded at16384. Histogram registry profiles are immutable
13-boundary latency and9-boundary result profiles. The internal helper accepts
up to64finite nonnegative strictly increasing boundaries, an explicit bound
relative to the original uncapped helper. Rejected profiles/values do not retain
series. Actual exact source render comparisons include all35families.

`Telemetry::new`/`new_with_sentry` return an owned `Arc<Telemetry>` implementing
the application Metrics/Audit/Supervisor ports. Search emits classified cache
observations, engine outcomes and completed queries through these ports.
Passive engine health remains F14's observed state: unknown, stale and synthetic
results do not become successful probes. Request/job RAII spans and finite task
state callbacks expose lifecycle outcomes without identifier labels.

Audit is enabled by default and redacts query and client identifiers before
writing to the configured private Valkey authority. `QueryPrivacy::RetainRaw`
is an explicit opt-in for operators who require source-compatible raw audit
fields. Logs and metrics do not receive raw query/client values. Audit stream
and per-engine stats retain90days; audit stream trimming uses approximate
MAXLEN10000. Stats keys use the local calendar day. Queue capacity defaults256
and is validated1..65536; operations default1second and shutdown2seconds.
Delivery failures are nonfatal and are not retried. Missing authority is a no-op;
actual connection refusal is a separate failed-delivery control. Result-count
overflow refuses a whole audit record before allocation. Source Python integers
are unbounded; native i64 totals are explicit. Midnight scheduling determinism
and global memory/RSS bounds are not inferred from finite queues.

Reviewed differences remain in `crates/xtask/fixtures/o01/native-differences.json`:
Duration cannot encode source negative-zero latency; three absent Python cache
shapes project to native optional authority; real refused connections differ
from mock execute failures; native callbacks enqueue an owned asynchronous
worker instead of exposing Python coroutine objects. Other captured latency
strings and exact command/field assertions remain unchanged. Search's inherited
integer elapsed-time policy is not changed by O01.

Structured logging uses closed event names and validated correlation IDs.
`LogRecord::write` accepts an explicit writer/level/format; O01 does not install
a global logger. Sentry configuration selects explicit nonempty DSN before the
supplied environment value. O01 does not read process environment implicitly.
Unset DSN/sample-zero creates no exporter worker/socket. Enabled monitoring
uses the shared F08 HTTP owner, bounded queue/POST and explicit shutdown with no
redirect/retry. Native `slopsearx.rust/<version>` identity and allowlisted error
messages intentionally differ from Python exception/tracing payloads. Public
`Telemetry::report_operational_error` is exercised through an actual private
POST. No external Sentry event is sent during qualification.

The W05 `saved_event_observer` uses accepted `SharedEventObserver`: its fifteen
static counter callbacks increment exactly once; the downstream adapter projects
only validated backlog age. None preserves age except the source-defined
empty/gap reset. No-success/unknown outcomes remain distinct. Real private
publish/read/ack/capacity/failure controls qualify the concrete bridge.

Transport/worker owners stop intake and finish their owned search/job work,
then await `Telemetry::shutdown`. Shutdown retains worker join ownership across
caller cancellation, uses the existing finite deadline and drains/aborts/joins
telemetry/exporter work. Drop aborts but is not an externally awaited join;
process/runtime termination has no universal OS cancellation guarantee.

T02/W09 must compose the APIs into actual service startup/configuration,
`/metrics` HTTP routing, concrete workflow observers, passive-health responses
and final shutdown order. T02 depends on O01's APIs; those integration obligations
do not become fake O01 route completions. The temporary offline source oracles
and original Python backend/tests retire under G08; immutable non-executable
provenance and captured native JSON may remain. No benchmark or deployment claim
is made by these tests.
