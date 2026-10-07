# EXP-072 source-order diagnosis

This is an offline, posthoc diagnosis of the rejected development screen, not a new candidate or qualification. A Luna reviewer independently checked request/response bindings and concrete source moves against both frozen references. Root independently recomputed all 64 requests, raw answers, orders and usage before publication. No extra model calls or searches were needed.

All four A per-case failures concern relative ordering among useful leads. Useful-count and facet retention survived; that alone does not establish better task completion.

| Case | Concrete source move | Interpretation and limit |
|---|---|---|
| q2: Java migration | Exact Java 8-to-21 c9 remains rank 3. Useful intermediate Java 8-to-17 c4 moves 5 to 11. Modernize c3 rises 12 to 4; Recipes c8 rises 13 to 6. | A/B disagree on several promoted source grades: delta -0.081/+0.142. The Java 25 distractor stays outside the top ten, but W0 already excluded it; boundary handling does not explain all ordering losses. |
| q6: compiler/LSP feedback | LSP lead c1 moves 2 to 12 despite both references grading it 3. Compiler-feedback research c4 rises 9 to 2, also grade 3 for both. | Delta -0.035/+0.003. Good topic fit does not reliably preserve every central lead's position. |
| q7: safe schema migration | Direct tool c0 moves 1 to 4; rollback-policy issue c18 moves 9 to 13, both grade 3 under A/B. Compatibility c5 moves 11 to 10, grades A2/B3. | Delta -0.036/0.000; a useful dissenting/absence-of-policy source is displaced, although useful counts/facets remain. |
| q8: flaky-test diagnosis | Diagnosis article c1 moves 2 to 13, grade 3 under both references. Implementation c8 stays near the top (1 to 2); generic c22 remains below ten. | Delta -0.047/+0.033. The central-source miss is real under both references, but other source changes offset it differently. |

The diagnostic also retains all eight primary cases, not only the four failures. In q3, FOSSA guardrail source c1 falls 9 to 14 despite grade 3 under both references, yet A's overall ranking gain is +0.063 and B is nearly unchanged. Aggregate improvement cannot be equated with improving every useful source's position.

There is no single observed hard-boundary error that justifies another wording tweak. The references assess visible cards, not whether opening each source advances the actual task. Their disagreements are evidence of uncertainty about relative reading value, not permission to choose B, rewrite A, or relax quality gates after measurement.

The next evidence stage should establish blinded task-level source-use judgments on untouched tasks and opened public sources before another selector is evaluated. Preserve the existing quality and resource guards; add downstream utility evidence rather than replace failed metrics. Avoid further identical-rubric retuning on this exposed cohort. A new cohort must be explicitly frozen before candidate outputs, and cannot be called final confirmation until a development candidate is qualified. Missing q10/q11 targets remain retrieval misses.

[Hash-bound diagnostic](source-order-diagnostic.json) and [offline script](source-order-diagnostic.py.txt) reproduce exact rank moves and reference grades. From the repository root, use `python3 docs/experiments/evidence/EXP-072/source-order-diagnostic.py.txt --repo . --run-dir docs/experiments/evidence/EXP-072/completed --output DIAGNOSTIC_OUTPUT.json`. The script makes zero provider calls, changes no labels or ranking policy, and assigns no quality/adoption credit. Raw receipt integrity is independently checked by `verify-raw-receipts.py.txt`.
