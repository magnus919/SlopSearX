# EXP-025: Captured-response ranking comparison

## Registration — 2026-09-25

- State: registered. Owner: Codex on behalf of the maintainer.
- Baseline: `1cbde5b07e5c546d955d23ca1eb01652e9f353d6`, presence ranking.
- Problem: the retrieval evaluation documents a science regression for RRF on
  synthetic inputs; these cannot establish useful retrieval improvement.
  Later EXP-004 through EXP-023 captured data may provide better inputs.
  This is a prerequisite audit for a captured-data comparison, not a rerun
  of the already rejected synthetic default-ranking proposal.
- Hypothesis: the existing opt-in RRF strategy improves mean nDCG@10 by at
  least 0.05 absolute over presence on an eligible captured multi-engine corpus.
- Candidate scope: existing `ranking_strategy="rrf"`; no new algorithm,
  default change, model call, upstream call, or production mutation.
- Primary metric: query-level paired nDCG@10 difference, relevance grades 0–3,
  gain 2**grade-1, logarithmic discount. Larger is better.
- Minimum useful effect: 0.05 absolute, to justify ranking behavior investigation.
- Eligibility: at least 30 distinct captured queries, at least 10 each general,
  code and science, with original ordered per-engine feeds, documented reuse
  provenance, and independently assigned pool judgments made without seeing
  either experimental ranking. All candidate union results must be judged.
  Synthetic, author-tuned, already-exposed development cases are not confirmation.
- Baseline numeric value: not yet measurable until eligibility is established;
  historical synthetic scores are explicitly not the baseline for this claim.
- Sampling: use every eligible query in the frozen repository snapshot; query
  is the sampling unit, no repeated query masquerading as extra samples.
  One candidate, no tuning. Paired query bootstrap, 10,000 replicates,
  seed 20260925, stratified by family; equal family weight, percentile 95% CI.
- Guardrails: each family mean difference >= -0.02; exact candidate membership,
  tiers, source provenance, status and filter metadata preservation; no policy,
  cache, snapshot or serialization changes. Only rank/score may differ.
- Measurement path if eligible: replay original captured adapter responses
  through `SearchService.search` with identical explicit scope and cache off,
  once for each existing strategy. Never score a hand-constructed replacement.
- Fixed stopping rule: audit tracked tests/fixtures and docs/experiments/evidence
  once; stop blocked before candidate replay if eligibility cannot be proven.
  Otherwise freeze eligible paths and replay commands in an additive registration
  commit before measurement. Total budget 45 minutes; offline; zero paid calls.
- Audit command: `rg --files tests/fixtures docs/experiments/evidence`, then
  inspect capture schemas and associated experiment methods for feeds, judgments,
  family counts and provenance. Retain exact audit script/output as inert evidence.
- Decision: supported only if lower 95% bound >0.05 and all guardrails pass;
  completed comparison failing gates is not-supported; unresolved uncertainty
  is inconclusive; missing eligible corpus is blocked, never a negative effect.
- Evidence: `docs/experiments/evidence/EXP-025/`.
- Portal impact: ranking would affect displayed order; no product change in
  this audit. Any later implementation requires portal contract coverage.
- No implementation PR unless the full registered comparison supports it.

## Readout — blocked (2026-09-25)

Registration committed as `dcb52f8` before the eligibility audit. The baseline
and proposed candidate remain the same source SHA; no implementation was made.
The audit found no corpus meeting the registered prerequisites in the tracked
fixture/evidence collection. See [assessment and reproduction](evidence/EXP-025/reproduce.md),
[artifact inventory](evidence/EXP-025/inventory.json) and
[schema observations](evidence/EXP-025/audit.json).

Baseline nDCG, candidate nDCG, effect and confidence interval are **unmeasured**.
This is missing evidence, not a zero effect or a failed ranker. Existing routing,
source-type and target-match labels cannot silently become independently judged
full-pool relevance grades. No service replay, tests, upstream requests or paid
calls ran. Statistical and response-equivalence guardrails were not evaluated;
no runtime, portal, policy, cache or API behavior changed.

The registered prerequisite stopping rule was followed without deviations.
Only documentation and inert audit evidence are retained; there is no candidate
implementation to discard and no implementation PR. Documentation is persisted
through the accompanying automatically merged experiment PR. Resume only after
a suitable new corpus exists; do not spend daily cycles repeating this blocker.
