# EXP-044 reference-shape correction review

Compared the post-merge runner against published `33753d7` in the clean main checkout. Current runner SHA is `aa7e0e38490da87ee9603eed20a87af831184f4b2bbe283c83b878c32705aba5`; current fixture SHA is `7217687cf32027dbc5ad26296cf0c0c8256b5cdbe15bdccc5d447ec112f6cdf8`.

The runtime correction is narrowly scoped: `transpose_references()` is replaced by `analysis_references()`, which consumes the exact A/B→pool shape returned by pinned `EXP043.validate_refs()`, checks A/B keys, full expected pool sets and each reference's representative IDs, and passes that result to the unchanged EXP-042 analysis helper. `compute_report()` now calls this same adapter, so the earlier double-transpose failure is corrected along the real analysis path. Diff shows no changes to selector, body construction, facets, task/purpose, pools, labels, gates, or source pins. No replay or calls were made.

## One qualification blocker

The modified fake-65 fixture (`fixture_tests.py.txt` around lines 198–223) now sends fake rankings through the real pinned reference adapter, which is useful and fixes the exact analysis-shape integration gap. But it removed the previous all-neutral synthetic reference rows (all leads 0, no facet tags), while retaining `assertFalse(development_screen_passed, "neutral synthetic references must fail selection")`. The fixture now uses the real sealed EXP-039/040 labels; that assertion no longer demonstrates the preregistered requirement that a neutral synthetic analysis explicitly fail. It can pass for an unrelated gate or happenstance of exposed labels.

Keep this real reference-adapter case to prove end-to-end shape, and add a second call to the actual analysis with explicit neutral synthetic references, asserting its expected neutral failure. That restores the registration's synthetic neutrality guard without weakening the check that the actual adapter path is exercised.

No other issue found in the correction diff. Root-reported fixture pass does not close this missing assertion because the current assertion message overstates what its input establishes.

## Recheck of root's final bytes

Root's current copies match runner SHA `aa7e0e38490da87ee9603eed20a87af831184f4b2bbe283c83b878c32705aba5` and fixture SHA `a82c522ed4723d7174be4365c68730347a57bbf1464ab89cab750797313414eb`. The fixture now calls the true analysis helper twice: first with neutral synthetic reference rows and asserts failure, then with the actual references through `analysis_references()` and checks A/B primary deltas and membership. This resolves the sole qualification blocker above. No behavioral/policy/body/facet/label change was introduced by the correction.

Only a stale diagnostic assertion message remains at fixture line 225: it labels the actual-reference analysis as “neutral synthetic references.” The prior explicit neutral check at line 218 is accurate and sufficient; update line 225's message to describe fake receipts/rankings against exposed references if convenient. This is not a substantive blocker. Final review: no blockers found. I did not run replay or call providers/search.
