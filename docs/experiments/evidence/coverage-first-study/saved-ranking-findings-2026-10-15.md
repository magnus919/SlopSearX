# Saved research-ranking findings — 15 October 2026

The tested candidate is not qualified for production. Jev completed all 39 ranking calls, but a test-runner bookkeeping defect stopped the study before answers were generated. Separate offline analysis found promising usefulness scores and a concrete evidence-coverage problem. No confirmation run followed.

## What completed

The development stage `e13bf092-e63a-45bb-beda-8fc4a5d13965` used source `0974b19dbadf80b07f9be9bb20fec1de5ada4b47`. All 53 reference-grading submissions completed and their closure check passed. All 39 serial Jev operations succeeded. Their recorded total usage was 824,816 input tokens and 97,331 output tokens. The slowest recorded whole ranking phase was 467 ms, below the registered 1,000 ms limit.

The runner copied a ranking outcome such as `selected` or `fallback` into a field that required execution state `complete-success`. Its next validation therefore rejected the results. This is a runner defect, not 39 failed Jev requests. The original stage remains stopped and incomplete; correcting the defect cannot retroactively turn it into a passed study.

There were no answer calls, answer assessments, confirmation calls, retries, or new search/capture calls. Development and unused confirmation identities are retired. Deployments were unchanged.

## Correction: distinguish the production baseline from original search order

The first version of this report incorrectly labeled the task manifest's `incumbent_order` as W0. That field is the original search order. The actual registered production baseline is each sealed W0 result's `native_ordered_ids` prefix, followed by the untouched tail from the original search order. All eight reconstructed W0 orders differ from the original search order. The figures and interpretation below correct that error; the first published version is retained in Git history.

Two offline analyses used the saved responses and completed independent reference assessments. Neither made model or search calls. Both included all eight base research cases and used the registered nDCG@10 calculation, 10,000 bootstrap samples, seed 7701, and unchanged quality thresholds.

| Policy | Mean nDCG@10 improvement over actual W0, assessor A / B | Per-case checks passed, A / B |
| --- | --- | --- |
| Original candidate, including its fallbacks | −0.1874 / −0.1465 | 2 of 8 / 2 of 8 |
| Sort by the existing usefulness scores | +0.2026 / +0.2385 | 6 of 8 / 7 of 8 |

For the original candidate, bootstrap intervals were [−0.3395, −0.0566] and [−0.2837, −0.0369]. Seven of eight base cases fell back to the original search order, not W0's reranked order. This candidate therefore underperformed the actual production control.

The single fixed Score-only analysis sorted the original reported scores descending with the existing canonical tie-break, without changing scores, questions, facets, or thresholds. Its intervals against actual W0 were [0.0896, 0.3100] and [0.1611, 0.3160]. It improved the mean but failed the required per-case checks: R01 reduced useful results from ten to nine for both assessors; R05's ranking quality fell by 0.1156 for assessor A. There was no R05 facet loss relative to W0: both had zero useful facets in their top results.

For comparison only, against the original search order, the original candidate gained +0.0447 / +0.0416, and Score-only gained +0.4348 / +0.4266. Those are not improvements over production. Score-only lost R05's two useful facets relative to the original search order. The original published narrative conflated these baselines.

Across all 17 candidate operations, 16 fell back because a lead was unavailable for at least one required facet; only one selected a new order. Offline diagnosis found that all reference-positive R05 card/facet pairs had usefulness scores below the candidate's minimum lead score of five. The facet-profile judgments also missed some evidence. Removing the cutoff or changing the policy has not been qualified by these observations.

These are exploratory findings from exposed development data. They do not establish completed research-task gains, untouched confirmation, navigation, stability, or every finalization requirement. They cannot rescue the stopped primary study.

## One fixed facet-probability revision

A subsequent bounded offline check tested whether the existing profile probabilities could preserve coverage without the whole-question score cutoff. For each facet, it reserved the candidate with the largest summed probability across profiles containing that facet, breaking ties by the original score and canonical key. It deduplicated and score-sorted the reserved leads, then score-sorted the remaining candidates. It introduced no new questions, thresholds, normalization, per-case exceptions, model calls, or searches. The rule was fixed before computing its order on the exposed development data.

Against actual W0, mean nDCG@10 changes were +0.1917 and +0.2319, with bootstrap intervals [0.0704, 0.3021] and [0.1501, 0.3081]. It passed only 6/8 and 7/8 per-case checks. Both assessors still lost one useful result in R01; assessor A's R05 ranking quality fell by 0.1471. It retained useful facets relative to both W0 and original search order, but that does not excuse the other regressions. This revision also fails qualification. No further ordering variants were swept.

Source inspection identified a question-target mismatch worth addressing before another comparison: the independent primary rubric measures whether a result is worth opening, while the candidate's imported Score rubric and facet instructions ask for substantive evidence visible in the card. Useful sparse documentation or publication leads may therefore be penalized. Revising the questions requires new responses and fresh qualification; the saved responses cannot be treated as answers to revised questions. The independent reference rubric and release gates remain unchanged.

## Delivery decision

Keep the production ranking unchanged. Do not ship either tested policy or start confirmation. The remaining product question is specific: can a ranking preserve necessary evidence while retaining the usefulness gains in the existing scores? Any revised policy must be specified before its evaluation and must preserve the failed results above. A development-only improvement is insufficient for release.

Issue #516 still requires a supported policy, complete development and independent confirmation, then shared-service implementation and compatibility verification across API, MCP, portal, cache, and GroktoCrawl forwarding. There should be one shared reranker. No production deployment follows from this record.

## Evidence identity

- Protocol SHA-256: `65b5515eb161c849c54f196c300f03b3965cbdbd4162682131b13ccecd0911a8`.
- Task-input manifest SHA-256: `d23b73091aabc0cba9981d5f04fb763fefd0f83f12e21842450d5bdcc257d002`.
- Closed reference outputs SHA-256: `d949f4c1f971d8c01b861a54e12bfbb211dd03fa4a1d1362712ca530afe64702`.
- Final stage inventory SHA-256: `b1d914293740fe29914cb25f3f9793eaca85e9eb4e7ef662642085c274c32c2b`.
- Closeout receipt SHA-256: `74757486ebd83a5fef646c0334a85caac418074713f89f3b83cc3649bf703ef5`.

The sealed source artifacts, reproducible offline analysis scripts, and detailed readouts are retained privately. This public summary excludes operational paths, credentials, and deployment identifiers.
