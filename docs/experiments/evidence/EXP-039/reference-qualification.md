# Reference qualification history

Before any EXP-039 provider calls, root inspected the first reviewer A annotation set after structural validation. It covered all 295 labels but failed semantic spot checks:

- q6-r04: the moved Generative AI conventions page had a snippet describing a documentation index and Markdown availability, yet was labeled visible-detail 3. The card does not visibly provide substantive convention details.
- q7-r00: lead 1 conflicted with the rationale calling the verification-method card a plausible partial lead. The low reading priority was not explained against the stated literature-review task.
- q1-r01: the direct empirical CooperBench paper was ranked below a rehosted benchmark card without a task-relative explanation.

These are annotation-quality objections, not measured Jev outcomes. The first file is retained as `reference-a-provisional.json`, excluded from sealed references and primary analysis. Reviewer A must reassess every card individually; patching only these examples is insufficient. Reviewer B remains independent and is reminded to distinguish sparse source leads from substantive card content. Neither set is trusted solely because it is complete or schema-valid. Freeze and hash only references that pass coverage, schema and substantive review; retain disagreement and uncertainty.

No provider call, search, page fetch or model-score exposure occurred before this check. Corrections remain exposed development annotation work, not independent human ground truth or untouched confirmation.

## Final pre-dispatch qualification

Reviewer A reassessed all 295 cards individually. Root validated coverage and schema and inspected ten varied cards; the corrected file passed those limited semantic checks. Reviewer B completed two full consistency passes. Root inspected eighteen shuffled cards, identified three unsupported facet assignments and one visible-detail grade, and verified their correction. Provisional and intermediate files remain preserved and excluded. The final files are pinned in `label-hashes.json`; assistant judgments remain fallible development references, not human ground truth. Both references must independently satisfy the registered primary gate. No provider outputs were available during these corrections.
