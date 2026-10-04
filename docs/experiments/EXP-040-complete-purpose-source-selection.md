# EXP-040 — Complete-pool, purpose-aware source selection

Status: registered; no provider calls or new searches.

EXP-039 improved mean reading-lead ranking under both independent assistant references but failed its source-identity retention rule. That decision is unchanged. A source-selection improvement can legitimately replace useful links; the new study registers guards against losses in useful-source count, named topic coverage, exact navigation and per-pool ranking quality, while retaining source-identity displacement as an explicit diagnostic. This is an informed revision on exposed development cases, not an independent confirmation of EXP-039.

The candidate is the unchanged EXP-039 D wording and fractional-score ordering applied to the complete pool in one request. This study adds the natural 44-card cardiac, 45-card RAG-research and 44-card software-evaluation pools, plus a constructed 80-card stress pool. It separately compares same-rubric first40 versus complete ordering to isolate candidate coverage from prompt changes. No across-batch score fusion is allowed.

All numeric gates, call counts, reference requirements, failure handling and byte/time budgets are frozen in [protocol.json](evidence/EXP-040/protocol.json). The primary comparison uses eight paired software queries and both sealed EXP-039 references independently. The three additional natural pools each require two new shuffled card assessments before provider calls and per-pool non-regression; their few cases establish bounded development evidence, not a medical or cross-domain relevance guarantee. Constructed cases are not additional independent quality units. All 48 calls are serial, with no retries. The 80-card/256 KB upper bound is an explicitly registered change from the current first40/128 KB boundary. Overflow must fall back to the full incumbent order, never truncate or partially rerank.

No new search or page fetch is permitted during this study. Brave remains **0/10 attempts used**. A supported development result permits only the separate prospective confirmation study. Production implementation and merge still require fresh confirmation, real shared-surface tests, all applicable CI and substantive review. This registration authorizes no deployment or Hermes change.

## Before-dispatch schema clarification

The copied annotation-output schema retained EXP-039 wrapper and alias names. Its original bytes remain frozen. The [effective v2 schema](evidence/EXP-040/effective-annotation-schema-v2.json) corrects only packaging and named-pool aliases to match the registered 133-card packet and reviewer assignments; [correction history](evidence/EXP-040/schema-correction.md) explains the mistake. This is fixed and pinned before any provider calls; judgment meanings and numerical gates are unchanged.

## Before-dispatch quality guard scope

An independent tooling review identified ambiguity about how the global useful-count/facet guards combine with the extended nDCG condition. [The scope clarification](evidence/EXP-040/guard-scope-clarification.md) makes the intended intersection explicit before new outputs: primary q1–q8 and all three natural extended pools must meet coverage guards, and extended pools additionally meet their nDCG bound. Constructed80 has no relevance gate. Thresholds and the candidate remain unchanged. The boundary fixture is an executed offline rejecting path, not a claim that production fallback is already implemented.
