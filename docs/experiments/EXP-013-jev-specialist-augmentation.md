# EXP-013: Jev specialist augmentation

**Status:** preregistered; no measurements collected

## Decision

Determine whether Jev should augment a deterministic general-search base with
every narrowly specialized engine that clears a relevance threshold. The specialists are additive:
they do not displace general engines. Jev never judges general or broad-support
engines in this experiment.

## Architecture under test

- General base, in fixed preference order: Brave, Google, DuckDuckGo.
- Broad supporting sources excluded from Jev decisions: Wikipedia, Stack
  Exchange, Reddit, and Hacker News.
- Specialist candidates: all other enabled, non-sensitive, authenticated,
  circuit-closed engines.
- Candidate dispatch retains all available engines in the three-engine general
  base and appends every specialist whose Jev score clears the frozen threshold.
  There is no count-based truncation.
- Zero specialists is a valid and desirable answer for broad queries.
- Existing policy, auth, health, and cost gates remain authoritative. Jev
  cannot make an ineligible engine eligible.
- Missing or invalid Jev output uses the intact general base.

## Frozen design

- Model: `jev-1.13.0`.
- Cards: EXP-011 `engine-routing-cards-v1.yaml`, unchanged.
- Fresh synthetic corpus: `evidence/EXP-013/development-v1.json` and
  `evidence/EXP-013/evaluation-v1.json`.
- Development: 12 queries. Held-out: 36 queries, six in each family.
- Families: packages/code, science/medical, security, structured reference,
  economy/jobs/media, and general/no-specialist.
- Candidate thresholds: 0.45, 0.55, 0.65, 0.75; no specialist-count limit.
- Each Noul explicitly names and describes one specialist engine.
- Jev input ceiling: 5,000,000 tokens.

Development selects a threshold by highest specialist macro-family recall,
then F1, abstention accuracy, lowest mean specialist count, and higher
threshold. The selection and schema checksum are committed before held-out
evaluation.

## Offline metrics and gates

Labels distinguish required specialist engines from an empty specialist set.
Broad engines are not counted as false positives because they are outside
Jev's decision surface.

The candidate advances only if:

- specialist recall >= 0.85;
- specialist precision >= 0.75;
- specialist macro-family recall >= 0.80 across positive families;
- specialist F1 improves by >= 0.20 over specialists selected by the current
  production scope;
- at least 5/6 general queries correctly abstain;
- mean Jaccard agreement of specialist selections under paraphrase >= 0.85;
- zero ineligible or sensitive selections;
- at least 35/36 query pairs are valid.

## Acquisition, only after offline success

Use a frozen 30-query subset: the first five IDs in each held-out family. Call
the union of current production scope and the general-plus-specialist candidate
once per query and replay identical feeds through the production presence
merger.

- Brave: 30 expected, 36 maximum; retry only transport failure or empty data.
- Other engines: 600 calls total, 30 per engine; no retries. This is an
  operational ceiling, not a relevance-based truncation rule.
- No per-query union-engine ceiling.

For the 25 specialist-positive acquisition rows, compare top-ten presence and
reciprocal rank of results contributed by a labeled specialist. The five broad
rows are a guardrail: candidate result availability may not lose more than one
row versus baseline.

Recommend a product implementation experiment only if specialist coverage
improves by at least 3/25, there are at least five unique candidate wins across
three positive families, no family loses more than one target-covered query,
at least 30% of incremental specialist dispatches contribute a labeled target
within the top ten, at least 29/30 rows are usable, Jev p95 latency is <= 600
ms, and all budgets pass.

## Interpretation

Passing supports further product work, not immediate delivery. A failed
offline gate prevents all search acquisition. Results must distinguish the
registered advancement verdict from broader research value.
