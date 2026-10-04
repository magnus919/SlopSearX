# EXP-044: relative facet reservations from frozen Jev probabilities

Status: prospective development registration; no candidate implementation or measurement yet. Follow-up to #516. This study tests one deterministic policy using already-published EXP-043 responses; it makes **zero new Jev calls and zero new searches**. The Brave allowance remains 0/10.

## Hypothesis and evidence boundary

EXP-043's absolute 0.80 cutoff produced zero facet insertions even when omitted sources had stronger within-facet probabilities than selected sources. This policy asks whether reserving each requested facet's strongest eligible source improves topic preservation without sacrificing usefulness. It is not a newly calibrated cutoff, and a relative leader is not proof of coverage or truth.

The baseline is `53b008eea91e51a1589861cb69e5f4b9dcc8b3dc`. Freeze the EXP-043 result integrity manifest, runner, source pins, qualified bodies, raw/structured responses, single-attempt ledger, grouping, reference projection and analysis helpers. Reuse exact model `jev-1.13.0`, caller purposes, ordered full pools, cards, facet descriptions and all D/F outputs. Do not rerun or change Jev questions to improve this result. All source/reference rows remain sealed. These cases and labels are exposed development data; this policy cannot establish untouched confirmation.

## One frozen policy

1. Require a complete valid D Score response and a complete valid F Noul response for the same ordered pool, purpose and facet contract. Keep the existing finite 0–9 Score and 0–1 Noul validations and full W0 fallback on any invalid pair. Empty/no-facet/navigation requests bypass this facet policy. Do not turn a failed pair into D-only adoption.
2. Let D be stable descending source Score, with original request order breaking ties. Use k=min(10,N). Every original candidate remains in the final ranking. Preserve the existing Score>=5 floor for **reservation eligibility**; lower-Score candidates remain in D and its tail.
3. For each frozen caller facet, choose the eligible candidate with maximum Noul for that facet, then higher source Score, then earlier original request position. Compare probabilities only within the same facet. Do not impose an absolute Noul cutoff, infer unseen evidence, or call the winner a calibrated yes.
4. Reserve the union of those leaders, reusing a single card when it leads multiple facets. With at most four facets this reserves at most four cards. Protect all leaders before filling slots, including leaders already present in D's top ten. If there are no eligible candidates, retain D unchanged.
5. Fill the remaining k slots with the first D-ranked candidates not already reserved. Return the selected set in its existing D order, then append the complete remaining D order. Thus no original ID is dropped or duplicated, nonreserved ordering stays stable, and at most four cards outside the original top ten can be inserted. There is no facet-order priority, backtracking, new weighting, threshold tuning or case-specific ID rule.

Report each leader's absolute probability, Score, original position, original D top-ten maximum (maximum F Noul and its candidate ID for that facet, before any reservations; use all N cards when N<10; this is a diagnostic, not an eligibility rule), reservation/multi-facet membership, and displaced/gained IDs. A low probability can win a relative comparison; this uncertainty stays visible rather than being relabeled as covered. All-low equal probabilities either pick the same highest eligible D card or have no eligible card, yielding unchanged D. Different low probabilities can trigger a reservation; the unchanged quality gates bound measured regressions on these references; they cannot guarantee that every insertion helps.

## Comparison and populations

Validate the entire published EXP-043 integrity chain and all 65 receipts/history entries before computing rankings. Reconstruct W0 and D from their exact original responses. Produce E44 for every original D/F pair: nine software main pools, four extended main/repeat/rotated pairs, and the original navigation bypass. The request order, 20-card cyclic rotations and full membership stay unchanged. The 79-representative constructed stress pool remains operational-only, never natural relevance. Keep original E43 as a descriptive failed comparator; primary gates use E44 versus W0 and same-response D.

There are no new provider measurements. Historical HTTP/input/output records show acceptance of the unchanged bodies only; do not count them as newly incurred cost, independent samples, or a fresh end-to-end latency test. Reusing the paired outputs isolates the policy change from model variation. Repeat/rotation comparisons retain their original different model responses and measure the new policy's stability on those responses.

## Unchanged conjunctive quality gates

Use the same sealed A/B representative annotations and EXP-042 pure analysis, with exact checked reference/pool transpose. The primary is q1–q8; q9 is separate. Natural cardiac/research/evaluation guard pools are unchanged.

- E44 minus W0 mean nDCG@10 >=+0.03 under **each** reference; paired bootstrap lower95 >0, 10,000 draws, seed 91940.
- E44 minus D mean primary nDCG>=-0.01 under each reference; per-pool loss <=0.03 on the eight primary and three natural guard pools.
- Top-ten useful-source count loses at most one against either comparator; a comparator with ten useful rows requires at least nine. Useful means the same reference's lead>=2.
- Preserve every useful facet found in W0 or D's top ten. Macro coverage uses facets with a tagged lead>=2 representative in the complete pool under that same reference, equal query weights, and reported zero denominators. E44 cannot decrease macro coverage versus D under either reference and must strictly improve it under at least one. No evaluable pools is inconclusive.
- Repeat/rotation top-ten overlap >=0.80 for all four extended pools. All five frozen navigation targets stay top-three under the existing W0/D bypass. Exact membership and response/history integrity must pass.

Report visible-on-card metrics, uncertainty, reviewer disagreement, unsupported facets, every insertion/displacement and per-pool guards. Neither reference is human gold. A passing result only qualifies this single frozen development candidate for the separately registered untouched ten-query confirmation. Failure rejects this policy; do not tune it in the same study or change the annotations.

## Offline qualification before replay

Freeze implementation, fixtures and output plan before analyzing these labels. Test empty/no-facet bypass; Score floor; one-to-four facets and pools through 80 cards; all-equal low probabilities; unequal low-probability uncertainty; same-card multi-facet leaders; ties; leaders already in the top ten; full-tail membership/order; maximum four insertions; strict raw/history tamper rejection; and invalid-pair full-W0 fallback. A complete synthetic replay must reach the actual analysis with both references and all eight primary deltas, explicitly fail neutral quality, and assert all pool/arm inventories.

Publish the qualification milestone before the actual deterministic development replay. Keep EXP-043's failure and all qualification/results untouched. If supported, freeze this exact policy for untouched confirmation before spending any of the 10 authorized Brave attempts. Production API/MCP/portal/cache compatibility, GroktoCrawl X caller-context forwarding, full CI and substantive review remain separate required implementation work; no deployment is authorized here.
