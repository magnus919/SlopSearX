# Item-level diagnosis of EXP-072 regressions

This retrospective diagnostic joins the complete base-operation card membership and rankings for q2/q6/q7/q8 to both frozen reference judgments. It does not rerank, call providers, amend references or qualify a selector. The builder binds all three source files by SHA-256 and retains card hashes, original scores, full ranks and individual reference rationales. Reproduce with `python3 docs/experiments/evidence/selector-failure-diagnostic/build.py.txt --repo . --output NEW_OUTPUT.json`.

Reference lead grades differ on 5/23 q2 cards, 5/40 q6 cards, 9/39 q7 cards and 19/40 q8 cards. Disagreement must remain explicit, but is not a complete explanation for loss: both references grade several W0 top-ten cards at the highest usefulness level, yet the candidate excludes them from its top ten.

| Case | Agreed highly useful excluded cards | Observed topic from reference rationales |
|---|---|---|
| q6 | c1, c18 | Cross-file LSP operations; feeding diagnostics back after edits |
| q7 | c14, c18 | Compatible deployment ordering; reversibility/rollback policy |
| q8 | c1, c4, c20 | Reproduction before repair; backtesting quarantine; validating that repair commands exercise the bug |

These are card-level advisory judgments, not verified source truth. Candidate scores on these rows are approximately 2.10–2.48 on its four-class scale. Control scores use a different ten-level scale and are not numerically comparable. The table does not establish why Jev gave those scores; the response supplies probabilities/scores, not explanatory reasoning.

No new rule is justified merely by preserving these particular IDs, fitting their reference grades, or interpolating saved scores until the gates pass. A subsequent mechanism must state a prospective semantic distinction, retain complete eligible-pool membership and the fixed two-reference/resource gates, and later pass new-task/new-pool confirmation. The source-use pilot's lack of task-completion gain and attribution failures remain separate evidence.

Unconstrained per-reference ideal nDCG gives mean primary improvement headroom of 0.14021/A and 0.11389/B over W0. These independent upper bounds ignore joint compatibility, retention, resource costs and attainable model behavior; they show neither impossibility nor a qualifying solution. They must not be used as a selector or as evidence that the required joint gate will pass.

An independent Luna inspection of the item table and original cards found a tentative operational-versus-broad coverage pattern in q2/q6, but no consistent rule across q7/q8. This is hypothesis generation only. A prospective investigation should measure task-stage usefulness against observable evidence requirements, without altering exposed reference grades or rewarding named documents.
