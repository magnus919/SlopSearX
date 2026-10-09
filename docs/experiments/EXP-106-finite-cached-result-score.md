# EXP-106 — finite cached score recovery

Original preregistration: [unchanged preserved record](evidence/EXP-106/registration-original.md),
committed before candidate work as `0fcce7e` under the subsequently corrected
EXP-104 label. Existing assessment-carry EXP-104 evidence is untouched.

## Readout — supported and delivered

Registration `0fcce7e`, baseline `e3eb6b4`, candidate
`e14cf3b87668604b94a7bc19c314cbecb9be91e0`. Primary recovery **0/6 → 6/6**
(+100 percentage points), meeting the frozen six-of-six and +3 minimum. Baseline
non-finite score cases all returned HTTP 500; candidate returns HTTP 200 and the
identical fresh results. Every recovery dispatches once and the following request
is a repaired healthy cache hit. All eight EXP-032 corruption regressions and
three cache controls pass. No cases excluded, threshold tuning or primary reruns.

Validation: 234 targeted service/snapshot/API/formatter tests and three explicit
portal browser tests passed. Seven finite legacy controls passed within that
slice. Mypy passed 121 files; all hooks passed; AST graph updated without paid
calls. Existing rejection logs expose only the failure class. Portal review:
shared rehydration, no schema/UI change; portal CI gates passed.

Local limitation: original coverage full run stalled in planning transport and
was interrupted (130). A bounded split run timed out in MCP harness (1). Planning
transport passed alone (1 test); unchanged baseline without coverage passed
2,471 tests, 58 skips and six subtests. A matched uninstrumented candidate run
also timed out (1). These diagnostics are retained and not counted passing;
causality remains unresolved. Normal CI full suites on Python 3.12 and 3.13 and
all other applicable exact-head checks passed. Droid's substantive review found
no actionable correctness/security issues and confirmed cache/snapshot recovery
and finite legacy compatibility.

Delivery: [issue #732](https://github.com/magnus919/SlopSearX/issues/732),
[implementation #733](https://github.com/magnus919/SlopSearX/pull/733), author
magnus919. Verified all checks/review on the exact SHA, then merged without bypass
at 2026-10-09T13:13:23Z as `03bee90c46bef3521cd2779b8a8ade2cb1a9c570`.
[Evidence](evidence/EXP-106/README.md) retains original registration, worker, all
observations, patch, environment, logs, checksums, review and merge records.
No stored-data migration.

This establishes correction of the six reachable cache-score faults, not their
production prevalence, availability uplift, retrieval quality or agent benefit.
Shared snapshot readers reject these malformed scores with existing invalid-handle
behavior. Release status unverified; no deployment, SLO operation or alerts.
Zero upstream/provider/paid calls; subscription usage unavailable. Actual elapsed
accounting is retained; identifier correction changed no scientific criteria.
