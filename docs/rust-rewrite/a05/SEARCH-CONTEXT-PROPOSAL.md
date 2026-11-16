# Application-owned adapter execution context

Status: additive seam proposed for parent-owned shared review; not implemented.
This supersedes no accepted interface and does not authorize unrelated factories.

Add in ssx-application (no Tokio dependency):

```rust
#[derive(Clone, Copy, Debug)]
pub struct SearchContext {
    pub deadline: std::time::Instant,
}
// Add to SearchEngine; existing implementers remain source-compatible.
fn search_with_context<'a>(
    &'a self,
    request: &'a SearchRequest,
    _context: SearchContext,
) -> PortFuture<'a, AdapterResponse> {
    self.search(request)
}
```

The F14 caller retains the outer timeout and computes its checked binding deadline
under existing policy; pass `min(service_deadline, engine_deadline).into_std()` to
search_with_context and use that same minimum as its execution timeout. Legacy
adapters still use the existing search method via the default. The service's
existing final deadline/response classification remains authoritative; no caller
reconstructs elapsed time from interactive_timeout_ms. Conversion back using
Tokio Instant::from_std preserves the same absolute deadline and controlled-clock
origin. Shared call-site edits and integration are parent reviewed.

A05 overrides both methods through one private run(request, Option<SearchContext>)
path. After successful F10 admission, choose min(context deadline, checked finite
configured operation deadline). A supplied already-expired deadline remains
expired. Source request phase limits and the five-millisecond DDG bootstrap cap
are separately intersected with this chosen total deadline. Positive infinity or
positive finite timers beyond Duration/Instant range add no tighter configured
limit when an externally supplied finite context exists: ordinary success remains
possible within that bound, without silent default substitution. NaN remains a
typed invalid-timer error; nonpositive values expire the native total operation,
including the explicitly approved DDG bootstrap-suppression difference.

Direct search() calls have no external absolute bound. For finite representable
positive timers, use the checked configured timer. For positive infinity or a
positive unrepresentable timer, propose Error with exact sanitized diagnostic
`HTML finite execution deadline required`, before allocation/admission/dispatch.
This is a documented direct-call safety restriction, not a claim of source parity
or replacement by the default timeout. It is subject to parent approval. Negative
infinity should be explicitly classified with the nonpositive expired branch,
matching observed source terminal Timeout while retaining bootstrap difference.
Text uses the source TypeError diagnostic as a no-unhandled-exception correction.

Execution expiry and process cleanup are distinct. F14's timeout may drop A05 at
the same instant the child deadline fires. A05 must not rely on that dropped future
to own kill/reap: approved ProcessPool::exchange_with_lease independently spawns
the supervisor, retaining the actual child and operation Arc/child allowance. The
response sender's closure requests cancellation; run_worker owns kill+wait;
allowance/permit release follows verified reap. Unknown cleanup poisons the pool
and intentionally retains accounting. The outer service can finish its timeout
response while this bounded cleanup supervisor remains live; that response does
not claim OS cleanup has already completed. Backend shutdown before runtime
teardown drains common supervision and propagates cleanup failure rather than
waiting forever on poisoned/leaked accounting. Runtime destruction itself remains
a separate application lifecycle obligation; kill_on_drop is not verified reap.
No adapter-owned detached processing task is added outside this existing pool.

Required concrete controls before acceptance:

- Legacy fake SearchEngine default dispatch still executes exactly once; context
  aware fake receives the actual min of independent service and binding deadlines.
- Queue/admission and formatting elapsed time reduce A05's remaining bound;
  context expiry during F10 is observed after F10 without claiming F10 included.
- Positive infinity + finite context succeeds for fast loopback responses and
  returns Timeout for a stalled primary with bounded actual child kill/reap.
- Positive infinity direct call returns the exact reviewed error before any
  request, while finite huge values exercise checked arithmetic without panic.
- Outer timeout and explicit caller drop race with child expiry; hold cleanup
  deterministically, verify PID ownership and retained allowance while service
  already returned, then verify actual reap before capacity can be reused.
- Google feedback remains between decode/challenge and extraction; DDG terminal
  timeout never gains fallback. Nonpositive source bootstrap difference is named.
- Common shutdown returns cleanup errors under uncertain reap; successful shutdown
  proves no owned child remains, including concurrent expiry/drop races.

The 60 real-socket source controls in TIMEOUT-SCALAR-PROPOSAL.md are retained as
observations, not normalized expected outputs for the intentional restrictions.

Outcome origin and health remain service-owned. F14 must distinguish an adapter
response completed before the limiting service deadline from a response returned
cooperatively at/after that deadline. Passing a deadline alone must not convert
caller truncation into AdapterReturned engine health. At result collection, if
the service limit won and is expired, apply the same existing service synthetic
classification as the outer-deadline branch: interactive started work is
Unavailable/CallerCancelled; full configured deadline uses the existing synthetic
Timeout policy. If only the binding deadline expired, retain the current
binding timeout response/DispatchedSyntheticTimeout behavior. A real adapter
Timeout before either service/binding limit remains AdapterReturned. Equality of
service and binding deadlines must have an explicit deterministic precedence
matching existing outer service semantics, with service expiry taking priority.
No adapter infers health origin from its own configured timer. Concrete tests
must inspect resulting origin and health state, not only returned status.

This is a required shared call-site reconciliation, not a change to the accepted
health formulas or an invitation to add diagnostic-string origin matching.
