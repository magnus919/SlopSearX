# EXP-039 development readout

The explicit-purpose and source-selection combination improved mean reading-lead nDCG@10 against both sealed assistant references, but did not meet all preregistered selection requirements. The decision remains **not supported for selection**. This is exposed development evidence, not untouched confirmation or production qualification.

| Reference | Mean D minus A | Paired-query bootstrap 95% interval |
| --- | ---: | --- |
| A | +0.214504 | +0.128244 to +0.314868 |
| B | +0.075292 | +0.014565 to +0.129668 |

Both passed the registered statistical gate. The useful-source identity-retention guard failed: up to four baseline grade-2-or-better cards left the top ten in individual queries, exceeding the permitted one. Useful-source counts did not decrease and no registered useful facet disappeared in either reference. This distinction motivates inspecting replacements; it does not authorize weakening the existing gate or calling the candidate selected.

All four repeat checks passed, with top-ten overlap of 0.8 to 1.0. All 40 requests were valid, with no retries, 655,388 input tokens and 19,500 output tokens. Maximum remote HTTP time was 471.458 ms, within the registered one-second allowance. All frozen bodies, input hashes, reference hashes and receipts independently verified after execution.

**Brave attempts: 0 of the authorized maximum 10.** The complete run reused saved public result cards. No deployment, default ranking change or new search occurred. Preserve the remaining search budget for independently registered fresh confirmation after selecting a complete candidate.

Next: inspect the displaced useful sources and their replacements, preregister any justified change to future guard design, and qualify whole-pool ranking on the saved 44-card cardiac corpus. Keep EXP-039's outcome unchanged. Any follow-up is another exposed development study, not a rescue analysis of this result.
