# EXP-040 blinded card annotation protocol

## Scope and freeze

Annotate all 133 shuffled cards in `annotation-cards.json`, using each pool's frozen purpose and facet vocabulary. Use title, URL, and snippet only; do not open or fetch linked pages. Do not reveal Jev outputs, any ranking, original pool order, W0/W2 grades, annotation ID map, or another annotator's work. The root prepared shuffled visible cards and a separate ID map; use the embedded frozen facet vocabulary before scoring each card. Preserve annotation aliases in the review file; root alone maps these back to candidate IDs after both reference sets, rationales, uncertainty flags, and hashes are sealed.

Two independent annotators label every card. The unit for each label is one card in the context of its stated task. Annotators must not adjudicate truth, clinical/technical correctness, whether a paper actually supports its abstract, or unobserved page content. Keep disagreements and uncertainty; do not average or manufacture consensus. Freeze annotations before any EXP-040 Jev request.

## Axis A: reading-lead priority (primary)

How high a priority is this visible result to select as a source to read for the stated purpose?

- **0 — No lead:** No plausible reason to read for this task; unrelated or plainly wrong requested target.
- **1 — Low-priority lead:** Incidental/broad topic overlap or generic background with little task-specific reading value.
- **2 — Useful partial lead:** Plausibly useful for at least one material facet or as relevant background, but partial, indirect, or secondary.
- **3 — High-priority lead:** Strong direct source to read for a central question/facet or requested source type, supported by visible card details.

Bibliographic-only or sparse cards may receive 2 or 3 if the visible title, URL, and/or snippet make a direct reading lead clear. A citation by itself is not automatically high priority. Judge the selection value shown by the card, not the unseen document's contents.

## Axis B: substantive information visible on card (diagnostic)

How much task-relevant substantive information is visible in this card itself?

- **0 — None:** No relevant information visible.
- **1 — Topic/source signal:** Topic, title, citation, or resource signal only; no substantive task-relevant detail.
- **2 — Partial detail:** Some relevant substantive details are visible, but central facets remain largely unanswered.
- **3 — Direct detail:** Substantial, specific information addressing at least one central task facet is visible.

Axis B does not judge whether visible claims are true, reliable, or sufficient to answer the task. A card can have high Axis A and low Axis B when it is a good lead whose snippet contains little detail.

## Facets and rationales

Before card labels, use each pool's embedded facet vocabulary from `annotation-cards.json`, also frozen in [task-facets.json](task-facets.json), derived directly from purpose text. For each card, tag only listed facet IDs visibly indicated by its title/URL/snippet; do not invent facet IDs or infer page contents. Retain an uncertainty flag where the indication is weak. Add one brief reason for both axes. Each row in the private annotation JSON must follow [annotation-output-schema.json](annotation-output-schema.json) and have exactly these fields: `label` (the shuffled alias such as `cardiac-r00`), `lead` (integer 0–3), `visible` (integer 0–3), `facets` (array of only that query's frozen facet IDs), `uncertain` (boolean), and `rationale` (one brief string). Keep the reviewers' files separate; do not include the original candidate ID or map. Materially contradictory sources remain relevant and can be useful reading leads when they directly address the task; do not penalize them merely for disagreeing with a premise. A missing or ambiguous snippet does not justify assuming the page's contents.

## Analysis use

Axis A grades are the sole primary relevance labels for nDCG@10 and useful@10. Axis B is a secondary diagnostic. Compare both annotator reference sets separately; no pooled grade is primary. The historical EXP-036 root/peer labels and EXP-038 W0/W2 outcomes are exposed development evidence only and are not substitutes for these blinded labels.

## Additional scope rules

Use every one of the 133 shuffled cards from the three natural pools. The keyword query and prior ranking are intentionally absent from the annotation packet. Do not inspect historical grades, experiment outputs, the ID map, another reviewer's file, or linked pages. Assess each card individually; a templated rationale or keywords-only facet assignment is insufficient. Clinical truth, medical recommendations, and page-level correctness are outside this source-selection rubric. These are assistant development references, not medical expert or human gold labels.
