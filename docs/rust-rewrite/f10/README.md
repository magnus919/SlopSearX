# F10 distributed quotas, cooldowns and observed health

Issue [#889](https://github.com/magnus919/SlopSearX/issues/889). Baseline
`9ab1c7a22de933762c42469145c1f50365220de2`, with accepted F07/F08/F11.
This is native rate/health infrastructure and policy, not F09 eligibility or the
F14 execution runner. No production wiring or provider requests are included.

## Authority and interfaces

`ssx_domain::rate::{EngineKey, QuotaConfig}` validate inputs before commands.
All 51 registered source engine names are covered. Names are bounded ASCII
letters/digits/underscore/hyphen; the reserved `upstream-cooldown` token is
rejected to prevent counter/cooldown namespace aliasing. Costs are unsigned,
zero is valid, and values beyond exact Lua integer range are rejected. Rates
are finite nonnegative values (fractional rates retain integer-count comparison).
Windows must represent whole positive milliseconds, allowing binary floating
roundoff for ordinary decimal millisecond values such as 1.001 seconds. Invalid
values are rejected, never silently clamped or used to refund quota.

`ssx_infra::valkey::rate::DistributedQuota` uses accepted F07 bounded connections
and failure classifications. `new(Arc<Valkey>, config)` borrows shared authority;
shutdown closes only this strategy, never another F07 consumer. `from_url`
creates an owned connection using explicit URL or an injected environment value;
it never reads ambient application URLs. Empty URL warmup is a no-op and
acquisition returns explicit unavailable authority. Owned shutdown is terminal.

Quota acquisition is one Lua operation: Valkey TIME chooses the fixed bucket,
INCRBY charges every request (including denied requests), and PEXPIRE installs
bounded retention atomically. A lost reply may have committed and is never
retried automatically. Receipt decoding verifies engine/bucket/server-time,
count/decision consistency and bounded TTL. The source name “sliding window” is
misleading: the accepted algorithm remains fixed windows, with quota per engine
and no cross-window smoothing or refund. Every replica must use the same policy.

`ssx_application::rate::RateStore` is the abstract port.
`RateLimiter::new(store, fail_closed)` preserves configured fail-open/fail-closed
behavior while returning authority failures separately from quota denial.
Closed/invalid input always denies. Unknown completion is distinct and carries
no automatic retry permission; fail-open still explicitly allows dispatch on
unavailable/unknown authority as the source setting does. `LocalTokenBucket` is
explicit process-local development mode, never an automatic distributed fallback.
It rejects a backward/nonfinite supplied monotonic clock.

Provider cooldowns retain `ratelimit:upstream-cooldown:{engine}`, value `1`,
source EX publishing semantics and the exact set-or-extend Lua body; reads use
the PTTL expiry hardening documented below. Numeric/date
Retry-After parsing preserves source email/obsolete-date forms, Unicode numeric
inputs and bounds 1..300 seconds/default30. The wrapper treats zero/negative
input as default30; the lower strategy clamps finite values to1. Shared cooldown
never adds denial strikes. Missing, persistent malformed and unavailable state
are distinct; the wrapper keeps the source best-effort continuation policy.

The source local-denial marker records30 seconds but is not checked before
acquisition. That behavior is deliberately preserved: three consecutive quota
denials deactivate locally immediately, success resets strikes, and explicit
reactivation clears local state without deleting shared provider cooldowns.

## Truthful health

`ssx_application::health::HealthState` keeps redacted status, timestamp, measured
latency and result count plus the separate source circuit (default5 errors,
300seconds). Error/timeout increments; ok/blocked resets; rate_limited/unavailable
leaves the circuit unchanged. At exact circuit expiry any next request may probe;
there is no new single-probe or distributed circuit algorithm. Source nonpositive
integer threshold/timeout values retain their behavior.

F14 must supply `OutcomeOrigin` accurately. Never-started/locally denied work is
`Undispatched`; caller cancellation is `CallerCancelled`; both leave health and
circuit unchanged. `AdapterReturned` requires actual dispatch evidence.
`DispatchedSyntheticTimeout` records timeout/failure with null measured latency.
An adapter-reported nonzero finite latency is retained, including the source
negative-value edge; default0 and synthetic latencies do not become measurements. The accepted F05 helper now
matches the source service's zero-to-null boundary. No query/content is retained.

`build_engine_health` renders the source vocabulary, UTC timestamp, strict
age>freshness stale boundary, nullable latency/count and independent configured,
auth and circuit fields. Never observed remains unknown, including when configured.
No authentication or registration flag fabricates healthy status. F06 supplies
explicit raw configuration and context; views never reread ambient settings.

## Reviewed corrections and migration boundary

`parity.json` lists each source behavior and intentional correction. Approved
hardening uses server TIME instead of unrelated host monotonic origins and one
atomic increment/expiry instead of an interruptible INCRBY/EXPIRE pair. Fractional
windows retain exact2*window_ms; source EXPIRE int(2*window_seconds) truncates and
can delete sub-0.5second buckets immediately. Captured source expiry arguments and
actual native expiry tests make this difference explicit. Persistent quota
buckets are repaired to bounded TTL. Inputs cannot be negative/nonfinite or
alias another state namespace. Finite-positive freshness is required: source
infinite freshness is captured, while native inf/nan falls back300seconds.

Quota keys keep the literal shape but the bucket meaning changes to server time.
Old/new quota writers must not run concurrently for the same engine during
cutover. G04 must drain admission and expire old windows before exclusive writer
handoff; these transient buckets have no durable workflow IDs. Shared cooldown
bytes/TTL remain source-compatible. This package does not claim completed G04
reverse-reader/migration rehearsal, deployed enforcement, or global observed
health/circuit authority. Those signals remain process-local as in the source.

Portal impact: active Python service/portal contracts are unchanged. Later shared
runtime wiring must exercise portal contracts; this package does not claim it.

## Executable evidence

The isolated Python3.14.7 exporter verifies pinned source bytes before imports and
captures synthetic local observations only. Actual source SearchService calls
cover queued-before-dispatch, returned/synthetic and caller-cancelled outcomes.
Captured URL/fail-setting and timestamp-boundary observations also cover empty
URL fallback, fail-open/closed choices and year/microsecond limits.
Captured cases cover local buckets, zero-cost/zero-rate quota law, provider and
local denial state, circuit boundaries,60 health views,61 Retry-After cases,
40 circuit settings,12 freshness settings, fractional expiry and all51 engines.
Native reviewed constants anchor corpus/exporter/Lua independently of metadata;
forged expected outputs, repaired metadata, missing evidence and changed Lua fail.

```sh
/private/tmp/slopsearx-rust-oracle-py314/bin/python -B docs/rust-rewrite/f10/oracle/export.py --source-root /clean/pinned/source --output crates/xtask/fixtures/f10/corpus.json
SLOPSEARX_TEST_VALKEY_SERVER=/private/disposable/valkey-server cargo xtask check F10 --output target/rust-evidence/F10.json
```

Native qualification requires official Valkey8.1.10 and accepted F07 contracts.
Tests spawn private random Unix sockets with TCP disabled, never an application
store URL or global flush. Two actual child processes with divergent local clock
origins share one quota; a proxy confirms a committed quota reply before its
native waiter is killed, then verifies TTL and physical expiry. A lost
acknowledgment test verifies unknown completion charges once, without retry.
Actual F08 loopback peers prove429 cooldown prevents a second request, expiry
recovers, queued/cancelled requests produce no upstream failure/latency, and a
peer-started timeout records failure with null latency. Test worker processes are
owned and killed on failure. Native gates never execute Python; G08 retires the
developer exporter before backend removal. Independent recapture/review and
exact-head Linux CI remain acceptance requirements.

### Provider cooldown expiry hardening

The source Python reads `TTL`, which rounds a live key with less than 500ms
remaining to zero and can admit a fresh replica early. Native F10 intentionally
reads `PTTL` instead. A present key, including a zero-millisecond observation,
remains active; missing keys permit admission and persistent keys fail closed.
The existing whole-second port rounds positive milliseconds upward, so a local
cooldown can conservatively extend by less than one second. Captured Python
observations and the source-exact publishing Lua remain unchanged.

The loopback HTTP fixture tolerates connections closed before any request bytes
(for example when an expired deadline wins during connection setup), rejects
partial requests, and still requires exactly two complete requests.

### Deterministic peer-started timeout witness

The real-peer timeout control keeps its 150 ms operation budget and 150.0
synthetic latency field. It is a library unit test so it can reuse the existing
private real-pool manual clock without a public clock-selection API. The clock
advances only after the actual loopback peer receives complete request headers;
no response bytes are released. A five-second wall watchdog detects fixture
hangs and does not change the logical timeout. Peer abort is joined and the
HTTP owner is shut down before assertions. F10 requires the exact relocated
`http::f10_peer_timeout_tests::actual_peer_started_timeout_records_failure_with_null_latency`
identity plus the remaining 11 integration controls; the whole workspace gate
executes the library test normally. Historical captures and health outcomes stay
unchanged. Source-pin successor composition and fresh exact-head CI are required.
