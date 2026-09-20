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

## Readout — supported for contract completeness (2026-09-20)

Registration: `283916f`; baseline runtime `ee2d663`. All 12 pairs (24 real MCP
tool calls) completed with exit 0 using fresh fixture-server processes. The
baseline reported truthful language metadata in 4/12 responses (33.33%); the
candidate did so in 12/12 (100%), a 66.67 percentage-point gain. Both repetitions
agreed. Aggregate normalized response bytes increased 4.2691%, below 15%.
All other fields matched after the registered exclusions; no tool errors.

Decision: **supported** on this exhaustive fixed contract corpus. This proves
that machine-readable language facts become available for default and explicit
English requests while preserving the measured response contract. It does not
prove improved agent task success, relevance, actual language filtering, user
satisfaction or production latency. No population confidence interval applies.
English remains unsupported by these adapters; the change reports that truth.
The prior EXP-001 observation was a default-value omission, not a consequence
of selecting explicit engines.

No implementation has shipped. A separate implementation PR must pass normal
regression and CI gates and remain unmerged for maintainer review. HTTP/HTML
code is unchanged; portal contract validation is still required evidence.

Evidence: [summary](evidence/EXP-003/summary.json), [all pairs](evidence/EXP-003/rows.json),
[reproduction](evidence/EXP-003/reproduce.md), and
[checksums](evidence/EXP-003/SHA256SUMS.txt). Each case directory contains both
raw responses, server logs and exit code. No deviations from the plan occurred.

## Implementation delivery

Evidence was merged in [PR #390](https://github.com/magnus919/SlopSearX/pull/390).
Implementation is [PR #392](https://github.com/magnus919/SlopSearX/pull/392),
addressing [issue #391](https://github.com/magnus919/SlopSearX/issues/391).
It is deliberately **unmerged**; supported evidence is not a deployment claim.
Local full-suite validation passed: 1,957 passed, 54 skipped, four warnings.
Focused pre-commit (lint, types, imports, dead code) passed. All-files hooks
reported pre-existing formatting in historical evidence and an unrelated test;
those automatic edits were restored to preserve evidence hashes and scope.
Graphify AST update completed. CI and automated review were requested on the
implementation PR; use that PR for their current status. No CI/review was
requested for documentation-only publication.
