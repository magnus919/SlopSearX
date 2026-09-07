# Optional interactive search budget

HTTP `/search` and generic MCP `slopsearx_search` accept `interactive_timeout_ms`
(1–30000, omitted by default). This bounds waiting for engine dispatch, semaphore
acquisition, and suggestions. It is not a hard end-to-end SLA: policy checks,
cache access, cancellation cleanup, serialization, and network transport add time.

The ordinary default and specialist/targeted/research tools retain their configured
engine deadlines. An explicit HTTP or generic MCP caller can choose a shorter
budget even with an explicit engine list; doing so explicitly trades coverage for
waiting time without changing selected engines or policy eligibility.

At expiry, available results are returned. `meta.deadline_exceeded` is true and
`meta.partial` is true when some engine succeeded. With no successful engine the
existing all-unresponsive error semantics apply. Engine outcomes identify budget
cancellations as `unavailable`, rather than upstream timeout; they do not alter
observed engine health or trip circuit breakers. Scope warnings persist with MCP
snapshots, whose pagination covers only the captured results.

Budgeted requests have separate cache and coalescing keys for each budget, so
normal searches never inherit their shortened coverage. Deadline-exceeded responses
are not cached. Fully completed responses may be cached within their budget scope.
No background engine work is left running after the budget.

The deterministic regression compares a fast source with a 150ms source: a 20ms
budget returns one result, while the default waits for both. This demonstrates the
latency/coverage tradeoff, not a production benchmark. Existing default behavior is
unchanged; measure representative queries before selecting an interactive budget.
