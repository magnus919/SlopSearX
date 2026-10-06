# EXP-078 — proposed development-only acquisition repair

**State: acquisition amendment approved by the maintainer on 2026-10-06.** Approval covers the two exceptions below and execution of the bounded study after separate registration, source qualification and one-shot admission checks. Those checks have not yet passed; no live stage has started.

EXP-077 remains terminal and inconclusive. Its development acquisition retained eight ungraded research pools (45–60 cards each), four target-acquired navigation pools, and all 37 HTTP receipts. Wikipedia returned blocked outcomes across the eight research tasks. The original Sigstore/cosign navigation search retained 20 results but missed its frozen target. No grading, selector, capture, answer, or comparison calls followed. The exact retained bundle is represented by [hash-only pins](reuse-input-pins.json); raw requests, responses, URLs, and private captures are not copied here. The original receipts and readout remain unchanged.

## Requested scoped exceptions

The release decision says to retain every fixed case, failed/empty/overflow acquisition, and missing target, with no rescue searches or substitutions. This proposal requests one explicit exception: one keyless GitHub search with the exact query `cosign` for `https://github.com/sigstore/cosign`. The new response must be retained as a distinct receipt; the original 20-result miss remains visible and immutable. The new response alone becomes the effective d12 pool; do not union it with or overwrite the original results. No Brave call, retry, query reformulation, engine substitution, or other acquisition is proposed.

The existing admission path also required all planned research-engine outcomes to be `ok`. This proposal requests a second explicit exception: permit evaluation on the eight complete received pools while retaining each Wikipedia blocked outcome as unavailable. It does not recast those acquisitions as all-engine successes. Whether these fixed gates can be meaningfully evaluated under partial engine availability is part of the approval decision; if the registered evaluator treats any such failure as disqualifying, the study stops inconclusive without repair or gate changes.

## Frozen scope if approved

Reuse the exact eight ungraded, unselected, uncaptured research pools and four target-acquired navigation pools, bound by the attached per-file hashes. Preserve every original result, engine outcome, missing target, and receipt. Append exactly one new GitHub acquisition receipt for the same frozen navigation target. The candidate, prompts, scores, pool handling, exact Fraction RRF with `k=60`, analysis code, and all numerical acceptance gates remain exactly those frozen by EXP-077 protocol SHA-256 `04fed87ccaa12272c73fcb6e6af5ef7da619c77488a62ae947d7cb647f251992` and EXP-076 protocol SHA-256 `5dc07f3a59428210489e851e60d69fe5db1a8409c70440b3bf04f5aa82497e32`. No tuning, regrading, or retrospective credit is allowed. The confirmation cohort and its five targets remain untouched and are not used for development. Any later confirmation must prospectively use and disclose the same three research engines (arXiv, OpenAlex, GitHub), excluding unavailable Wikipedia for both development and confirmation; results are not comparable to prior four-engine studies. No confirmation run is proposed or authorized here.

This would be a new development-only study with disclosed reuse and partial-engine scope, not a rerun or repair of EXP-077. It cannot establish untouched-confirmation success or authorize production adoption. Any missing-target or other fixed gate failure remains a failure; do not substitute another target.

## Admission boundary

EXP-077's stage clock and live runner are not reusable as an EXP-078 authorization: their source closure and all-engine preflight bind the old stage. If this proposal is approved, a separate, genuine one-shot EXP-078 admission must first bind the exact reused-input pin file, the appended-query bytes, the unchanged candidate/evaluator closure, a new stage manifest/clock, and a qualified runner/source revision. The runner must retain both original and appended receipts and report partial engine outcomes explicitly. Until that binding is separately reviewed and issued, there must be no external call. No EXP-077 guard may be bypassed or modified to simulate admission.

## Decision requested

Approve or reject the two acquisition exceptions above. Approval authorizes the separate EXP-078 registration and qualification work and, once its source, input, and one-shot admission checks pass, the bounded study including its one corrective query; no further confirmation is needed for those mechanical steps. Rejection leaves EXP-077 terminal and unchanged.
