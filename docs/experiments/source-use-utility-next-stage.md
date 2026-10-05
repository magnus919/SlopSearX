# Next evidence stage: does opening the source advance the task?

Status: execution plan, not a registered ranking comparison or a qualified selector. EXP-072 remains rejected. This plan adds task-use evidence; it does not replace or relax the existing two-reference quality, retention, stability, navigation, full-membership, natural-large-pool or resource gates. The full production acceptance checklist remains the final delivery requirement.

## First small pilot

Use q6 (compiler/LSP-assisted refactoring) and q8 (flaky-test diagnosis) from the existing frozen EXP-060 pools. Their exposure and the EXP-072 outcome are disclosed. Freeze a task manifest with the original purpose, all three original facets/target checks, input hashes and the complete W0/EXP-072 orders before opening sources or generating answers. The rejected order is a diagnostic contrast only; this pilot cannot qualify it or become final confirmation.

Target the actual **grounded Q&A keyword path with `num_sources=5`**, inspected at experimental X revision `3612affba3b51e7240a0e60d079499739052c0b1`: `AnswerRequest` defaults to five successful sources (range 1–20); `_run_answer_discover_and_scrape` requests `2*num_sources` results; keyword mode does not add the local semantic reranker. `_scrape_answer_sources` deduplicates, handles refusal/video ordering and returns up to five artifacts. Thus freeze each arm's first ten presented candidates, share the same captured source availability/content, and preserve failed openings separately from the successful-source quota. Render contexts through the pinned `SourceArtifact.to_document` contract (default 8,000 characters per source). The controlled no-retry capture must record attempted URL/order, refusals, video deprioritization and successful-artifact counts separately; it is not an exact end-to-end replay of ordinary Q&A recovery attempts. Freeze answerer request identity, exact prompts, context bytes, token limits, acquisition/answer timeouts and no-retry policy before generation. Record the returned model/version where exposed; an alias alone is not a pinned checkpoint, and any unobservable revision must remain explicit rather than implying production equivalence.

Do not infer that unrestricted agent research opens only five or ten sources. At the inspected X revision it discovers with `limit=None` and admits all distinct URLs unless explicit `max_credits` bounds novel attempts. This bounded pilot is not evidence for all agent paths; those remain in the production acceptance scope.

## Freeze this utility/target-check rubric before source capture

For every original task facet, retain the expected target/boundary and mark the source/answer state separately:

1. Not acquired: blocked, failed, absent or unavailable; no invented source content.
2. Opened but no usable task evidence: topical lead, wrong target, insufficient detail, or unresolved provenance/date/version.
3. Evidence available: a captured passage directly supports, contradicts or qualifies a required task facet, with exact source hash and span.
4. Evidence actually used: the generated answer applies that passage to the facet with a traceable claim-to-source link, correct target and explicit limitations.

Opening, citing, high model confidence, engine rank or a plausible answer is not task completion. A successful task must address every declared critical facet using available source evidence, preserve relevant contradictions/qualifications, and avoid unsupported material claims. Missing evidence yields an explicit incomplete/abstaining result. Record found-but-unused evidence and answer-generation errors separately from retrieval/source insufficiency.

Use two independent Luna assessors, blind to ranking labels/scores and each other's judgments. Source/target assessments precede generated answers; later answer assessments use anonymized arm names. Preserve individual ratings, passage links, disagreements and abstentions rather than manufacturing gold. Apply deterministic version/identifier/span checks where possible. The pilot validates this rubric and attribution process only; two exposed tasks cannot establish ranking uplift or downstream generalization. No human mechanical grading is required.

## Then return to selector qualification

Inventory non-Brave acquisition paths before spending the remaining two Brave attempts. The [source inventory](evidence/source-use-next-stage/non-brave-source-inventory.json) is code-level availability evidence only: it does not establish live engine health, credential availability, pool sizes or fresh-source adequacy. Any new acquisition/model comparison needs a frozen registration and per-attempt budget first. Keep all actual failures and partial coverage; do not concatenate synthetic pools and call them natural search output.

Use the pilot's observed attribution gaps to design a materially different, justified development investigation. Do not tune the rejected rubric to the exposed A/B labels. A development-qualified contract must still precede final confirmation. Final confirmation needs entirely new tasks **and** new source pools, frozen task targets/references before selector outputs, the original guards and additional task-use evidence. New questions over old pools are not untouched. Reserve the two remaining Brave calls for that later stage if non-Brave sources prove inadequate.

After both qualification stages, implement the full shared SlopSearX contract and X propagation, compatibility/security tests, substantive review, required CI and implementation PR merges. Neither this plan nor a diagnostic pilot is production delivery. No deployment, Hermes or default changes are part of this stage.

One bounded independent Luna design review found no blocking corrections. No source-opening, answer-generation, model or search calls have been performed for this stage. The exact task manifest and request/acquisition budgets must still be frozen before execution.
