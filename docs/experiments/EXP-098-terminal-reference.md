# EXP-098 terminal outcome: selector input-usage bound

EXP-098 completed its source-reference phase but ended with an incomplete selector run. It establishes no ranking-quality, answer-quality, or production-adoption result. Issue #516 remains open.

## Preserved reference work

All 48 source assessments passed the registered original-response, transcript, citation, and coverage checks: 23 carried assessments and 25 fresh GPT-6.1 Sol assessments. The eight source plans closed with R1 and R2 kept separate. No assessment was repaired or retried. All original reference work remains retained for a prospective continuation, subject to strict provenance verification; retention does not grant a new execution clock or quality credit.

The cumulative grader count is 72: 47 historical submissions plus 25 fresh source submissions. No answer-generation or answer-assessor calls occurred in this run. There were no new searches or source fetches.

## Observed selector stop

The live selector made 12 Jev calls, with known usage of 450,498 input tokens and 47,316 output tokens. The neutral operation, the three d1 variants, and d2 base completed. The d3 base candidate response triggered the registered usage guard; the remaining 16 operations were retained as uninvoked.

| Observed d3 candidate response | Value | Registered bound |
| --- | ---: | ---: |
| HTTP status | 200 | 200 |
| Response complete | Yes | Required |
| Response bytes | 120,041 | 2,000,000 |
| Input tokens | **64,015** | **64,000 per call** |
| Output tokens | 8,534 | 32,000 per call |

Only the input-token value exceeded the checked response/usage bounds, by 15 tokens. Aggregate usage remained below the 3,000,000-input and 160,000-output stage ceilings. A complete HTTP response is not proof that the unparsed candidate response satisfied every semantic or identity requirement.

The runner preserved the entire native order for the failed operation, stopped further selector dispatch, and blocked the answer handoff. The controller was terminalized without retrying the selector, extending its clock, raising its limits, or scoring partial results. Raw responses, grades, identities, configuration, and operational receipts remain private.

## Prospective next step

Investigate a separately registered per-call input-budget correction, including its admission reserve, while keeping the aggregate stage budget, request/response bounds, timing limits, candidate criteria, input representation, fusion policy, numerical quality gates, and untouched-confirmation requirement fixed. Verify all 48 original source references before reuse; do not regrade them or retrospectively credit this stopped run as complete. This terminal outcome itself authorizes no successor stage or live call. A continuation requires separately established authority, prospective registration, qualification and fresh admission. Existing human authorization may satisfy the authority requirement; this report does not create that authorization. Freeze and verify any changed budget before a fresh clock or live call.

The proposed correction is preparation for completing the comparison. It is not evidence that the candidate improves research usefulness, and transport-only delivery does not complete #516.
