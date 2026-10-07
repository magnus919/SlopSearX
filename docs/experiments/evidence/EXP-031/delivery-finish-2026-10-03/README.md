# EXP-031 final delivery evidence

Retained supported candidate rebased onto current main without changing the
frozen experiment metric or decision rule. Final candidate:
`e05beed6f34cb5886c2354eb46e0a7d095c33819`.

Validation: 291 contract tests; explicit `PORTAL_BROWSER_SMOKE=1` passes all three
portal browser journeys, including the previously failing workflow confirmation
flow. Full local suite with disposable real Valkey passes 2,356 tests, two skips,
86.75% coverage. Mypy and changed-file hooks pass. All-file hooks find historical
evidence whitespace, restored byte-for-byte. Initial targeted invocation used
a nonexistent browser filename and ran no tests; corrected invocation and
explicit smoke are retained rather than replacing that setup failure.

merge-gate.json retains complete current-SHA CI and positive substantive Droid
review, which explicitly identifies the candidate. merge.json verifies delivery.
The original 50% to 100% warning-truthfulness effect is unchanged and not rerun
under a new hypothesis. New checks establish refreshed compatibility, not agent
task-success or production-SLO uplift. No deployment or production alerting.
