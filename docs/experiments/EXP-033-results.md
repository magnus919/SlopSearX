# EXP-033 readout: capability advisories

Outcome: supported for truthful capability disclosure. Behavior or retrieval
uplift is unmeasured. [Registration](EXP-033-capability-advisories.md).

The 22-case paired offline HTTP/MCP matrix yielded 0/14 correct eligible
disclosures at baseline ad03963 and 14/14 at candidate
f4ab41d486f77725304aa1b58dac9c03fb5ae0f3: +100 percentage points, exceeding the
registered 50-point threshold. All eight quiet controls passed. Adapter-call
counts and normalized non-advisory payloads were identical. The fixed matrix
has no population confidence interval and establishes no agent task benefit.

Jev specialist planning and reranking recommendations attribute production
gains to the maintainer's report. No raw production measurements were provided
and no paid/model/provider calls were made. Reranking is recommended above five
canonical results regardless of presentation slicing; configured or sensitive
scopes suppress that recommendation. Disclosure never guarantees query uplift.

The first measured candidate missed one MCP early validation error (13/14);
subsequent trials repaired it. Initial full regression had three failures because
sensitive-policy rejections gained metadata; the final candidate preserves their
original metadata-free contract. The repaired policy slice passed 53 tests.
The final full local rerun stalled in MCP transport setup and was interrupted;
CI full regression passes on Python 3.12 and 3.13. Portal browser/contracts,
SearXNG compatibility, Valkey integration, optional Jev modes and quality gates
pass. Mypy passes 118 source files. Focused search/formatter contracts passed
222 tests before the final cached-runtime refresh test (26 advisory tests pass).
Changed-file hooks pass. All-file hooks find pre-existing whitespace in immutable
EXP-001/003 records, restored byte-for-byte. AST graph updated.

[Evidence](evidence/EXP-033/README.md) retains raw observations, unsuccessful
trials, worker, comparison exclusions and validation logs.

Implementation [PR #506](https://github.com/magnus919/SlopSearX/pull/506) was pending
review/merge at this initial readout; final delivery is recorded below. Current-SHA runner review is recorded at
https://github.com/magnus919/SlopSearX/pull/506#issuecomment-5973162265.
No production deployment. Rollback is reverting the advisory source changes;
canonical cached data and dispatch behavior are unchanged.

## Review correction and final delivery

Droid identified two real boundary gaps after the initial record: HTTP omitted
the operator-configured sensitive-engine set, and media-constrained requests
could advise sources without media support. Both were fixed and have dedicated
regressions. The corrected contract slice passed 253 tests and the paired matrix
remained 14/14 eligible and 8/8 quiet controls. The runner's earlier review missed
these findings; retain that review as history, not proof of these boundaries.

A subsequent Python 3.13 CI failure revealed the new HTTP test's full lifespan
startup polluted global engine state, allowing later tests to dispatch live free
searches. The fixture now injects runtime state without starting the application.
No production code changed in that final delta. The affected sequence passes
69 tests. Final local full regression again stalled in MCP transport setup and
was interrupted; its trace is retained, not counted as passing evidence.

Droid's substantive follow-up reports no remaining actionable issues and confirms
both earlier fixes. The runner reviewed the final test-only delta and current
candidate at
https://github.com/magnus919/SlopSearX/pull/506#issuecomment-5973255032.
All applicable CI passed on final SHA
`6c590affdee8a9ecfb203be413547136e48cabc1`, including full Python 3.12/3.13,
portal browser/contract, Valkey, SearXNG, optional Jev, quality and builds.

Implementation PR #506 merged at 2026-10-03T20:37:47Z as `8653dff1e660370577947e1f764837cbc0d31014`. No deployment.
