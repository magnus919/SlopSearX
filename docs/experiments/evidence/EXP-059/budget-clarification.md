# Pre-measurement budget semantics

These budget semantics were established before EXP-056 provider measurement and are inherited by EXP-059. EXP-059 explicitly changes the per-request question cap to 84; the spending guards and quality thresholds remain unchanged.

The three-million input budget is an admission guard: reserve the documented maximum 64,000 input tokens before each call. Actual known usage is charged to the study. Unknown failed-call usage stops further calls and is never represented as zero. The one-neutral/84-case attempt ceilings remain hard admission guards.

The 100,000 output-token limit is an observed fail-and-stop guard. The current public HTTP API documents no request-side output-token parameter, so the controller cannot promise provider-side output suppression or an absolute no-overrun guarantee. If a completed call reports cumulative output above 100,000, retain and charge that usage, reject the operation and study, and make no further call. This is the existing implemented overrun behavior, not permission to count an over-budget study as passing or to discard a failed call.

This distinction concerns an agent-authored evaluation budget, not a new spending authorization. Magnus explicitly authorized multiple cheap Jev tests; the provider documents output tokens as free. Paid input remains bounded by the admission rule under the published provider contract. No fabricated per-Noul output upper bound or unsupported API parameter is introduced. The [API reference](https://docs.typesafe.ai/api) and [model documentation](https://docs.typesafe.ai/models) were refreshed on 2026-10-04.

All provider context limits, response byte caps, operation deadlines, conjunctive quality gates and no-retry rules still apply. Any output overrun is an unsuccessful experiment outcome, never a production-readiness result.
