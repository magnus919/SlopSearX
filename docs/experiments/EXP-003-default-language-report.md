# EXP-003: Report default-language enforcement

## Registration (2026-09-20)

Baseline main: `ee2d663`. EXP-001 and EXP-002 are complete. Current source
`_core_filter_enforcement` excludes language `en`, whereas MCP_CONTRACT.md
states language is unsupported. EXP-001's empty report was caused by this
value check, not explicit-engine routing. Correct that earlier hypothesis.

Candidate: change `if language and language != "en"` to `if language` in the
shared MCP reporting helper. Preserve the resolver, dispatch and policy gate.
The helper also drives honest warnings, so a default-English warning is an
expected additive output. No adapter or actual enforcement changes.

Primary metric: fraction of successful fixture search responses containing a
language record with the exact requested/effective language, status unsupported,
nonempty reason and empty enforced_by. Require 100% on candidate, a gain of at
least 50 percentage points over baseline, and unchanged results and scope.

Fixed cases: generic search with omitted language, explicit en, and de, each
with automatic scope and explicit wikipedia/duckduckgo scope (six cases).
Use real MCP streamable HTTP, FakeEngineSpec with three results per engine,
in-memory store, two repetitions per case, fresh process per case/pair. Reverse
arm order on the second repetition. Omitted language means effective en.
No live engines, paid calls or actual agent tasks. Measure information
availability and contract compliance, not agent success or human usability.

Guardrails: identical full envelope after excluding only query_id, cursor,
result_id/artifact references, response_time_ms, and the intended language
entry/warning. Preserve every other result, scope, enforcement, status and
pagination field. No tool errors. Mean normalized JSON size increase <=15%.
No timing-benefit claim. Exact fixed-corpus arithmetic, no statistical inference
or population confidence interval; both repetitions must agree on facts.

Stop after 12 paired cases, first infrastructure/correctness failure, or
45 minutes. Candidate supported only if primary and guardrails pass;
not-supported for a completed comparison failing criteria; blocked for
infrastructure; inconclusive for unstable facts. No tuning or new candidates.

Before production PR: run enforcement, MCP transport, shared service and portal
contract checks plus repository pre-commit and required CI. Portal impact is
limited to shared metadata expectations: HTTP/HTML implementation is unchanged;
MCP language reporting and warning docs must be updated. Keep any supported
implementation unmerged; merge documentation evidence separately.

Reproduction: extract evidence/EXP-003/reproduce.md code blocks into the named
`/private/tmp` scripts; run its recorded command with project .venv Python and
PYTHONPATH pointed at this isolated checkout. Candidate may be injected into
its module only inside the experiment process; no source edit before measurement.
Retain all payloads, exit codes, environment and SHA256 manifest in
`docs/experiments/evidence/EXP-003/`. Commit this registration first.
