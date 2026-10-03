# EXP-032 — recover search completion from corrupt cache entries

## Registration (before candidate work)

- Explicit maintainer-requested one-off, October 3, 2026; not another scheduled cycle.
- Baseline 1402e287348b965fb106264cf075c73ca13225c3, the pending SLO instrumentation
  PR #498. Implementation will be stacked on that PR, not auto-merged or deployed.
- Problem: normal HTTP search reads real Valkey JSON. The cache assumes decoded
  JSON is an object; response reconstruction trusts nested types. Corrupt entries
  can prevent a healthy adapter's result from reaching the caller.
- Issue: https://github.com/magnus919/SlopSearX/issues/499.
- Beneficiary: API/portal search callers during cache corruption or incompatible
  cached data. Production prevalence is unknown; no error-budget gain is inferred.
- Hypothesis: rejecting nonobject JSON and treating known reconstruction errors
  as cache misses restores 100% of affected searches without changing good hits.
- Primary metric: correct successful HTTP responses / corrupt-entry attempts.
  Useful effect: 100% recovery and at least 50 percentage-point gain on fixed corpus.
- Eight frozen corruption cases: JSON list, number, string; object with string
  scope; result list containing a string; invalid response_time_ms;
  invalid dispatched_engine_count; scope.jev_scores containing an invalid number.
- Two repetitions per case per arm, fixed seeded case order, baseline before
  candidate. Independent processes/instances per arm. Repetitions establish
  repeatability, not independent users or population confidence intervals.
- Start each case by deleting only keys belonging to the disposable Valkey,
  populate through normal /search, inject the corrupt value at its actual key,
  then execute the same request. One fixed result adapter, no upstream calls.
- Guards: results identical to fresh response; recovered response uncached and
  dispatches once; next request is a healthy cache hit without new dispatch;
  healthy cache control remains a hit; intentional negative-cache control
  remains 503 without dispatch. Negative and malformed JSON handling unchanged.
- Measurement boundary: real FastAPI route + shared SearchService + real SearchCache
  + disposable Valkey on a private Unix socket. Fixed adapter makes fault recovery
  reproducible; does not prove retrieval relevance or production availability.
- Candidate scope: cache decoded-object validation and narrowly classified
  reconstruction failures; log no raw payload/query. Preserve cancellation.
- Stop on any guard breach, 45 minutes, no paid/live calls. Missing infrastructure:
  blocked; incomplete evidence: inconclusive; failed threshold/guard: not-supported.
- Supported requires all declared recovery thresholds/guards and normal regression
  validation; preserve both arms and reproduction. Do not relax after outcomes.
- Commands: PYTHONPATH=. .venv/bin/python evidence worker (retained under evidence);
  .venv/bin/pytest --cov=slopsearx --cov=engines; changed-file hooks and full mypy;
  normal cache integration and portal contracts. Retain existing historical
  all-files hook whitespace limitation; do not edit historical experiment hashes.

## Readout — supported for cache fault recovery

Frozen registration commit 9ce2043 preceded candidate work and measurement.
Candidate af399bbbb90ffb0203241a971f5140b081863987.
All 16 baseline fault attempts returned HTTP 500; all 16 candidate attempts
returned HTTP 200 with the correct fresh result: **0% → 100%, +100 percentage
points**. Every recovered attempt dispatches once, replaces the bad entry, and
has a subsequent healthy cache hit without further dispatch. All three controls
pass in both arms. No exclusions, altered cases, or threshold changes.

This is exact observed recovery on eight repeated fault classes through a real
Valkey-backed HTTP path. It does not quantify production frequency, overall SLO
gain, search relevance or human/agent task success. No population CI applies.

Full validation with real Valkey enabled: **2,319 passed, 2 skipped**, 86.70%
coverage. Focused cache/service/API/portal slice: **277 passed**. Changed-file
hooks and source-wide mypy (117 files) pass. All-files hooks still flag historical
experiment whitespace; automatic changes were restored without altering hashes.
Graphify AST update completed. The first integration run found an unrelated
lineage test missing required SearchResult.content; supplied empty content to
exercise its original assertions. Retain the failed and final run summaries.

Ready-for-review implementation: [PR #500](https://github.com/magnus919/SlopSearX/pull/500),
stacked on [#498](https://github.com/magnus919/SlopSearX/pull/498). Neither is
merged or deployed by this experiment. Evidence: [reproduction](evidence/EXP-032/reproduce.md),
[baseline](evidence/EXP-032/baseline/summary.json),
[candidate](evidence/EXP-032/candidate/summary.json).

Persist this supported finding through a documentation-only PR separately.
