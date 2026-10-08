# Intent-aware complete-pool ranking: development results

Status: development endpoint calculations complete; production readiness not established.

## What was tested

The frozen development study compared the incumbent order with an intent-aware complete-pool Jev candidate on eight research tasks and five navigation tasks. Two independent card assessors (A/B) evaluated ranking; two independent answer assessors (R1/R2) evaluated research usefulness. Their results remain separate. The study rules, task inputs, and acceptance criteria were fixed before execution.

All 32 answer assessments are structurally complete: 28 accepted assessments carried unchanged and four fresh assessments. Structural closure establishes complete validated records, not answer correctness. The earlier setup failures remain part of the operational record.

## Results

Average nDCG@10 improved by 0.069207 under A and 0.098438 under B. A's bootstrap interval was [0.013531, 0.132219]; B's was [0.054051, 0.145077]. B passes the frozen ranking endpoint. A does not: task d3 fell by 0.030101 against the frozen -0.03 floor, and d4 lost one previously represented facet. These failures are retained; rounding does not change the gate.

Research usefulness did not show consistent improvement. R1 completed one task under each arm; R2 completed zero incumbent tasks and two candidate tasks. Neither passes the frozen task-use endpoint. R1 has four tasks with unknown critical judgments and R2 has five. Both assessors flagged material failures in six candidate tasks. These counts are assessor judgments under the frozen rubric, not independently established facts about each answer. Incumbent answers also had material failures in six tasks under each assessor. R1 counted 10 unsupported-claim and 11 missed-qualification findings for the incumbent versus 8 and 14 for the candidate; R2 counted 7 and 10 versus 7 and 13. These diagnostic finding counts do not change the frozen endpoint and do not establish that ranking caused the failures.

The original compact references were expanded through the original lossless hydrator and validation registry, then supplied to the unchanged endpoint calculator. No grades were repaired, reference sets combined, thresholds relaxed, or new searches/provider calls made during analysis.

## Reliability and audit limits

All eight repeat/rotation comparisons pass: top-ten overlap is 0.9 or 1.0 against the frozen 0.8 floor. All five navigation targets were acquired and ranked first. All eight research pools contained 41–80 cards (45–60 observed). The selector reports 44 calls, 1,404,684 input tokens and 156,040 output tokens, with no unknown usage. Recorded candidate phases range below the 1,000ms ceiling; the maximum is 773.460083ms.

The carried card verifier validates frozen inventories, raw/decoded/expanded assessment joins, and byte hashes. The original selector-artifact validator verifies structural completion and clock binding. The analysis bridge checks complete ordering, separate assessor calculations, navigation, stability, known usage and recorded bounds.

The old selector did not preserve raw provider response bodies. Its parse functions ran during execution, but response hashes alone cannot reproduce parsing independently. Accordingly this report does not claim raw-response replay, full independent timing provenance, or production qualification. The bridge's nine synthetic tests establish calculator integration and rejection behavior, not semantic correctness.

## Decision and next work

Do not promote this candidate to the production default. Ranking gains are promising, but the frozen research-usefulness endpoint is not met. Do not round away d3's failure, drop d4's lost facet, combine assessor sets, or reinterpret uncertainty as success.

Before the next separately preregistered live stage, preserve complete raw selector responses and exact final receipt joins so offline verification is possible. Investigate the answer-level unsupported claims and missed qualifications separately from ordering; the present study does not establish that ranking caused them. Any candidate refinement requires its own frozen comparison; untouched confirmation with new tasks/pools remains required before adoption. Issue #516 remains open until supported product implementation, compatibility and fallback tests, review and merge complete.

## Evidence

The frozen protocol is [EXP-105](evidence/EXP-105/protocol.json). The unchanged calculators are [EXP-077](evidence/EXP-077/decision.py.txt). Sanitized separate-assessor results and offline bridge tests are in [the result packet](evidence/intent-ranking-development-results/results.json). Earlier setup failures and charge accounting remain in the prior experiment records. No deployment changed as part of analysis.
