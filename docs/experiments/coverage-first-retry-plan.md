# Retry the research-ranking comparison

**Historical attempt:** this replacement finished during collection without a quality result. The [corrected-input plan](coverage-first-input-correction.md) defines the next separately registered attempt; it retains this failure and the original one.

The latest development attempt stopped during search collection, before any ranking or answer evaluation. This amendment permits one replacement development attempt, followed by one confirmation attempt only if development passes every check. It changes the no-retry restriction for this specific collection failure; it does not relax the quality requirements or authorize production deployment.

## Keep the failed attempt

The [original outcome](evidence/coverage-first-study/completed-2026-10-15-task-use-collection/README.md), its 26 requests, and all retained records remain unchanged. Its run identities stay retired. Do not resume its directories, reuse its execution permissions, or combine its result pools with a replacement attempt. It receives no quality credit.

## Run the same comparison again

Use the same schema-4 candidate, questions, query plans, engine selection, reference rules, thresholds, and model and resource settings from the [registered comparison](evidence/coverage-first-study/prospective-2026-10-15-research-task-use-v4/README.md). The development questions may be reused because the failed attempt produced no grades, candidate scores, or answer outcomes. Keep the separate confirmation questions unchanged and unmeasured until development passes.

Before calls, publish a replacement registration with new development and confirmation identities, exact source and configuration hashes, and a clear link to the failed attempt. Preserve the original registration as history. Bind this narrow retry amendment by its exact file hash in the replacement registration, or publish a new protocol hash carrying the same exception. Do not claim the old no-retry protocol alone authorizes the replacement. All scientific settings remain unchanged. Verify that the active production control still matches the pinned control; if it has changed, record and resolve that mismatch before measurement. Check rate-limit cooldowns and honor any recorded Retry-After requirement before starting. No extra search is needed just to check availability.

Collect every pool again using the unchanged query plans. Freeze the complete replacement collection once; do not select the better result from multiple attempts. Empty pools, missing navigation targets, engine failures, and size-band failures remain visible and retain their existing consequences. No replacement query, extra engine, pool concatenation, trimming, or result substitution is allowed.

## Limits and stopping rules

- At most one replacement development invocation and one conditional confirmation invocation are permitted by this amendment. There are no automatic HTTP retries inside either invocation.
- Development allows at most 40 physical search requests; confirmation allows at most 41. Every failed request counts. Including the original 26 requests, the combined retrieval ceiling for the original attempt and this amendment is 107. Earlier historical studies remain separately recorded.
- Brave usage is zero. The two remaining Brave attempts stay reserved.
- Existing per-stage capture, Jev, answer-model, grader, byte, token, timing, and concurrency limits remain unchanged. They must be copied exactly into the replacement registration, not increased to accommodate failure.
- Stop the replacement development attempt on any failed or inconclusive required check. Do not start confirmation unless all development checks pass. Do not repeat model calls to obtain a better score or answer.
- Stop confirmation on any failed or inconclusive required check. Retain and publish either outcome. This amendment does not permit another replacement attempt automatically.

Both stages must pass before the shared-service implementation and GroktoCrawl X integration proceed. A passing retrieval attempt, fixture, or CI run alone does not establish ranking quality. Production behavior remains unchanged.
