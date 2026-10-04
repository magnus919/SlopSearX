# EXP-044 result integrity and gate review

I independently read the registered policy and result artifacts at post-fix SlopSearX commit `f411cc1` (merge `7fcd…` was reported by the owner). Runner SHA-256: `aa7e0e38490da87ee9603eed20a87af831184f4b2bbe283c83b878c32705aba5`; saved `analysis.json` SHA-256: `a77cbb93d94e855f261649a5ee6187cb18266de608e1ae817867e636134d2d5b`.

## Reproducibility check

With no external/provider/search calls, I invoked only the runner's pure `compute_report()` and compared its in-memory JSON result to the published `analysis.json`: exact equality. The pinned EXP-043 verifier accepted all 219 result-integrity files, all 65 response receipts, and all 65 `attempt_begun`/`attempt_finished` history entries. Reconstructed identities contain 18 W0, 26 D, and 21 F records; all 21 D/F pairs have the same candidate order and task and the registered empty-D/nonempty-F facet boundary. The reconstructed grouped corpus has 13 analysis pools (nine software, three natural guards, one constructed stress pool) with `constructed80` grouped to 79 representatives. Analysis contains both A/B references and all eight primary paired deltas for each; navigation bypass passed all five targets. The comparison decision remains `unsupported_for_selection`.

This reused frozen EXP-043 judgments and labels. It is a deterministic policy replay over exposed development data, not new provider measurements, independent quality confirmation, or production-readiness evidence. The first post-merge invocation remains preserved as a zero-call failure before analysis due to the now-fixed reference-shape mismatch; the successful artifact was recomputed through the exact pinned A/B reference adapter. Nothing indicates a new Jev or search attempt in this study.

## Registered gates

- Membership, navigation, repeat/rotation stability, and macro-coverage gates pass. Coverage is not the blocker: under A, D* and E are both 1.0 macro coverage; under B, E is 1.0 versus D* 0.9773, a +0.0227 improvement. B's strict gain is in the research guard pool.
- The primary E−W0 mean gain clears +0.03 for both references, but the bootstrap lower bound fails for B: mean +0.04925, lower95 −0.00743 (10,000 draws, seed 91940). A passes with mean +0.20825 and lower95 +0.12587. The primary gate is conjunctive, so A's pass cannot offset B's failure.
- Ordinal noninferiority E−D* fails for B: mean −0.01847 against the −0.01 floor. A passes at −0.00749.
- The registered per-pool nDCG guard also fails: E−D* is −0.05219 for A/q3, −0.06638 for A/research, −0.03635 for A/cardiac; for B it is −0.03927/q3, −0.04485/q5, −0.06362/q7, and −0.03635/cardiac, all below the −0.03 limit. Useful-card count and useful-facet-preservation diagnostics themselves pass: the worst count loss is one and no useful facet is lost.

Thus the overall rejection follows directly from frozen statistical and noninferiority/per-pool gates, while coverage and stability pass. Do not describe the policy as “coverage failed”; do not selectively waive the B reference or per-pool limits.

## Descriptive mechanism, with the evidence limits

The research guard gives a concrete example of why the relative reservation policy was explored. Its `evidence_grounding` facet leader is c6 with Noul 0.72 and Score 6.02; the best E43/D* top-ten Noul diagnostic was c3 at 0.61. c6's frozen visible snippet says the “Research Paper Agent” identifies research gaps, compares sources, generates citations, and grounds answers in retrieved text. The facet's tag-based coverage is 0.75 for B/D* and 1.0 for B/E; A already has the facet covered in both D* and E. This is an observed agreement among one model judgment, snippets, and one reference annotation, not evidence that c6 is true, independently useful, or that the policy generalizes. It cannot override the failed quality gates.

For q5, relative reservations also illustrate the cost side: c14 (“SLSA and Provenance…”, ostering.com) is selected as the attestations facet leader (Noul 0.71, Score 5.49), entering E's top ten and displacing c5 (OpenSSF's “Mini Shai-Hulud: Where SLSA’s Boundaries Fall”). The result retains an absolute low leader probability diagnostically; the policy's relative maximum is not a calibrated relevance guarantee. This is descriptive card-level evidence only, and no labels, thresholds, facets, or cases should be changed in this study.

## Recommendation

Keep EXP-044 rejected for selection. Do not adopt these results in the shared service or change the public/API/MCP/portal behavior. The research c6 example may motivate a distinct, newly preregistered hypothesis on untouched queries, but should not be used to tune the failed policy or reinterpret the frozen outcome. Any future confirmation is separate work with fresh provenance and unchanged gates; it would establish only that new study's evidence, not broad production readiness.

No external network, Jev/provider, or search calls were made in this review.
