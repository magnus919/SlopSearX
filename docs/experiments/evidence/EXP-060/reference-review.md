# Bounded blinded reference review

## Method and limits

For reviewers A and B, I inspected the first, middle (`floor(n/2)`), and last shuffled cards for q1–q13 (39 cards per reviewer). For q1–q5 I used the q1–q3 and q4–q5 reference batches; for q6–q8 and q9–q13 I used their corresponding completed batches. I compared only displayed title, URL, snippet, task purpose/facets, and the matching annotation. The EXP-039 annotation protocol governed the check: distinguish reading-lead priority from visible substance, tag only visibly indicated facets, and do not infer linked-page contents or factual correctness.

This is a fixed sample, not a full reference audit or grade adjudication. No scores were retuned. I did not inspect maps, model outputs, the invalid earlier default reference batch, or linked pages.

## Review findings and resolution

- Reviewer A, q1 `q1-r014` (middle): the displayed card says Nx uses the dependency graph to identify affected projects and target builds/tests. The earlier rationale claimed selection “by revision diff,” which was not in the excerpt. Before sealing, it was replaced with a rationale accurately limited to the snippet; lead was set to 2.
- Reviewer A, q2 `q2-r011` (middle): the title is “Testing | OpenRewrite Docs,” while the snippet only redirects Moderne customers to another catalog. The earlier rationale claimed Java 21 migration-validation relevance not visible on the card. Before sealing, the rationale was narrowed, the facet tag removed, uncertainty marked true, and lead set to 1.
- Reviewer A, q8 `q8-r039` (last): the card is a first-person account claiming 58 fixes and 5 quarantines. The rationale calls these “real fixes” as evidence. Consider marking this as author-reported workflow rather than endorsing the outcome as verified.

Other sampled rationales were generally consistent with the visible card and preserved the lead/visible distinction. I found no further concrete unsupported assertion in this sample.

Before sealing, reviewer A qualified q8-r039 as author-reported and unverified; only the rationale changed. The exact change is retained in reference-A-audit.md. All sampled concerns were addressed before the first model call.
