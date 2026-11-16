# Daily delivery: 2026-10-16

Cycle start: 2026-10-16 13:03:55 UTC (09:03:55 America/New_York).
No prior same-date cycle start was present. This cycle finishes existing delivery;
no new experiment or retrospective preregistration is claimed.

## Selection and history

Fresh fetch and fast-forward verification established baseline
`730c37a5f48b9830088594582cefb4c2f10a35e8`, HEAD equal to origin/main,
with divergence 0/0. The unrelated untracked inspiration document was preserved.
The separate active Rust rewrite was left intact. No overlapping kaizen cycle was
identified in the current thread inventory.

Review of the seven latest numbered completed records: EXP-123 initialization
timeout, EXP-122 session ownership, and EXP-121 input schemas are merged finite
contract corrections. EXP-120 remained unmerged with an unclosed metadata guard;
EXP-121 subsequently closed that delivery gap. EXP-119 resource selection and
EXP-117 benchmark candidate were rejected; EXP-118 qualified a measurement
prerequisite without proving a product-quality gain. Their historical metrics
were not rerun. No supported pending implementation from those records remained.

Existing PR #874 and issue #873 supplied a consequential ready candidate:
HTTP date_from/date_to controls were silently dropped before the shared OpenAlex
adapter. A successful response could therefore misrepresent a bounded search.
Delivering that already implemented repair took priority over a fresh speculative
trial or further gateway changes. Its evidence predates this daily cycle and is
not relabeled as a preregistered experiment.

## Review and validation

Candidate: `9ab07ac9dbfdcd6d25bc19c472ea4230519bc0ef`.
An isolated detached checkout preserved the original branch and main.
Command, run independently on the candidate and subsequently merged revision:

```text
PYTHONPATH=. /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python -m pytest --no-cov -q tests/test_server.py tests/test_formatter.py tests/test_filter_semantics.py tests/test_mcp_enforcement.py tests/test_mcp_tools.py
```

Both returned exit 0: **319 passed**, four dependency warnings (2.11s candidate;
1.78s merged). These are contract regressions, not a new before/after primary
metric, production sample or retrieval-quality study. No causal effect size or
population confidence interval was estimated today.

Reviewed guards include GET/form POST forwarding, inclusive one-sided/equal and
leap-day bounds, real OpenAlex adapter with mocked HTTP, malformed/missing dates,
pre-dispatch rejection, audited scope, cache separation/proof reacquisition,
JSON/YAML receipts and failure outcomes, MCP grants, and portal continuation.
CSV/RSS reject absolute bounds because they cannot carry the required receipt;
no-date behavior remains covered by the existing suites.

Droid's YAML-proof finding was fixed; its subsequent LGTM preceded the final
CSV/RSS rejection commit. A substantive runner review covered the exact final
head and that delta, with no unresolved finding:
[review](https://github.com/magnus919/SlopSearX/pull/874#issuecomment-6216181679).
The author is magnus919. Every reported current-head check was completed and
SUCCESS or intentionally SKIPPED, including both Python suites, portal/browser,
SearXNG, Valkey, optional Jev, quality, security and build checks. A green reviewer
job alone was not treated as approval. Historical local full-suite environment
limitations remain in the original PR; no new full-suite-green claim is made.

## Verified delivery and limits

[Implementation PR #874](https://github.com/magnus919/SlopSearX/pull/874)
merged normally, exact-head matched, at 2026-10-16 13:08:17 UTC as
`d207d4496129d496fe8a74b040ebe1a71151b6f9`. Issue #873 is CLOSED.
Primary local main fast-forwarded to that merge, preserving unrelated work.
The delivery receipt is persisted separately in a documentation-only PR.
No protection bypass, deployment, alerting or production SLO operation occurred.

Delivered behavior: bounded HTTP searches reach the audited date enforcement
path and expose matching requested/applied proof; unsupported combinations fail
before dispatch instead of silently returning unbounded results. Production
incidence, independent retrieval benefit and release/deployment status remain
unknown. This is completion of existing defect work, not a newly measured uplift.

[Evidence directory](evidence/daily-delivery-2026-10-16/): exact-head check snapshot,
substantive review, both test outputs and SHA-256 manifest. Scientific/provider
requests: 0; provider cost: $0. No new automated review invocation. Subscription
usage is unavailable. Delivery reached merged validation in 4m45s from cycle
start; documentation persistence adds closeout time recorded by its commit/merge.
No candidate code was created or discarded by this cycle.
