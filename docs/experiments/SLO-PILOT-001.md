# SLO-PILOT-001 — one-off product reliability measurement audit

Explicit maintainer-requested pilot on 2026-10-03, separate from today's completed
scheduled cycle. Baseline main ab3168d847186c138751f15c5a4a11c3913d56bc.
This is an instrumentation correctness fix and SLO discovery pass, not an
experiment claiming improved product availability or task success.

## Problem selection and baseline

The proposed loop requires user-boundary failure rates. The normal HTTP path
has no completed-search outcome metric. Existing `server_requests` counts after
some validation returns, so it is not a complete attempt denominator. Engine
errors have a different unit from search requests. An actual HTTP TestClient
reproduction through normal routing, injected deterministic engine and service:

| Attempt | HTTP status | Existing request counter delta |
| --- | --- | --- |
| Successful retrieval | 200 | 1 |
| Total upstream failure | 503 | 1 |
| Invalid page parameter | 400 | 0 |

No paid or live upstream requests; no production traffic or incident prevalence
was measured. Source issue: [#496](https://github.com/magnus919/SlopSearX/issues/496).

## Candidate and acceptance

Add closed-vocabulary completed HTTP search outcome and duration metrics at the
endpoint boundary, export through `/metrics`, preserve all responses and errors.
Cover early rejects, success, total failure, throttling, exceptions/cancellation;
exclude the landing page and nonsearch routes. Retain existing metrics unchanged
for compatibility. Document exclusions and missing production evidence in
[SLO declaration](../SLO.md). Numeric SLO commitments remain unset.

Portal impact: additional telemetry only; no HTML, request semantics, access
policy or results change. Run normal API/portal and metrics regression checks.
Outcome and delivery results will be appended after validation.

## Readout

The new metrics cover six normal HTTP cases (success, total failure, timeout,
invalid page, unsupported format, missing query), with exactly one completed
outcome and duration sample each, exported at `/metrics`. Additional boundary
checks cover 429, 403, exceptions, cancellation and landing/health exclusion.
Existing search response semantics are preserved by the wrapper and regression
suite; no measured retrieval, latency or production availability gain is claimed.

Final full suite: **2,255 passed, 56 skipped**, 85.38% coverage. Final focused
portal/API/metrics suite: **144 passed**. Source-wide mypy: **117 files pass**.
Every hook on changed implementation files passes. The required all-files hook
run still reports pre-existing whitespace in historical EXP-001/003 evidence;
its automatic edits were restored byte-for-byte to preserve evidence hashes.
This is a remaining repository hygiene limitation, not an all-hooks-green claim.
Graphify AST update completed without API calls.

The first full run retained three failures caused by missing `headers` in the
new direct-request test fixture, corrected before final validation. It also
reproduced the existing workflow browser blocker: the fixture grants only
cancellation while the assertion expects two confirmation notices. Corrected
that expectation to one and added a check that unauthorized retry is absent.
No runtime permission or portal behavior change; full browser suite now passes.

Decision: deliver the measurement foundation and fixture correction for normal
implementation review; persist this audit separately as documentation. The pilot
produced useful engineering work, but has **not yet produced a demonstrated
material improvement in user task completion or production SLOs**. It reveals
that the redesigned loop needs measurement readiness as an explicit work stage.

Next action: collect representative user-boundary baseline evidence, approve
numeric targets/deadlines, then rank observed failure classes by user impact.
Offline correctness fixtures cannot supply failure prevalence or error-budget
consumption. The daily automation configuration is unchanged by this pilot.

## Delivery confirmation — 2026-10-03

PR #498 passed all applicable CI and received positive substantive Droid review.
Under the maintainer's new standing merge authorization it merged as
b7d31a4093aec1153f379639bf3540ca706a3fe9. HTTP outcome/duration instrumentation
is integrated into main. Numeric SLO targets remain unset and production
baseline/compliance remains unmeasured. No deployment is inferred.
