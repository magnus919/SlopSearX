# Post-confirmation failure diagnosis

Read-only GPT-6 Luna reviews inspected the sealed run, references and acquisition; no labels or thresholds changed and no new calls were made. These are now exposed development observations, not fresh confirmation.

## Order-sensitive D, then marginal E

On q4, base/repeat D and E top-ten overlap was 0.9. Base/rotate overlap was 0.7 for D and 0.6 for E. Same-order D score maximum change was 0.17 (mean absolute 0.058); rotating identical cards produced maximum 0.94 (mean absolute 0.347). No exact score tie occurred at the top-ten cutoff: this is scoring sensitivity, not a tie-breaking defect.

E made one rotation-only swap, c10 for c18, with sufficiency 0.22, addition 0.81 and loss 0.49. Both sources are useful; c10 covers shrinking and c18 oracle limitations. The swap did not lose useful-count or supported-facet coverage but further reduced stability. c10 is acquisition position eleven, not a beyond-forty tail promotion. c40 (position forty-one) remained in every q4 D/E top ten; rotated runs also retained c52 and c58.

Primary q4 E equals D. The A/B relevance disagreement (+0.0397 / -0.0775 against W0) therefore arises before refinement. Canonical request ordering is a plausible mechanism to test; it must be measured with fresh same-byte repeated calls, not claimed correct because permutations produce identical request bytes.

## Redundant refinement and inherited regressions

q2 replaced c3 with c11 under facet q2_f2, with addition 0.63 and loss 0.49. c3 is a Java migration catalog listing Java 21; c11 concerns Java 8 to 11. A assigns both lead 2; B assigns c3 lead 3 and c11 lead 2. The B E-D nDCG loss is -0.0448. Existing selected c9 already covers a Java 8-to-21 case and both references tag the same facet. The probability gate admitted a swap without a measured coverage gain.

q7 and q8 E equals D and actions are empty. Their W0 regressions are therefore not E swap failures. On q7, a rollback-strategy source moves from W0 rank ten to D/E rank eleven while a broader design-pattern source enters rank eight. Coverage remains complete; relevance can regress despite unchanged facet coverage.

## Next mechanism boundaries

Test canonical full-card request ordering independently from judgment changes. Keep original purpose and the complete pool; reverse-map output IDs and preserve the full incumbent fallback. Hold model, question wording, references, budgets and scoring fixed in the ordering ablation. Record D scores/ranks, F nominees and E actions across repeated and rotated inputs.

Separately test whether refinement's usefulness/loss proposition describes source-selection value for the caller's task, rather than only visible snippet contribution. Near-0.5 probabilities are not calibrated correctness evidence. Do not choose a new cutoff from these few observed failures or relax quality gates. A changed contract requires registration and independent review before calls; exposed diagnostic gains still require genuinely fresh confirmation.

Missing navigation targets are acquisition recall failures, not ranking demotions. Future fresh acquisition must preregister recall and natural-pool measurement; no repair of the frozen cohort or extra Brave attempts is implied. Production integration remains gated on qualifying evidence across the full original scope.
