# Ranking strategies

Presence ranking remains the default. Operators can opt into rank-aware fusion
through the existing YAML configuration:

```yaml
ranking:
  strategy: reciprocal_rank_fusion
```

Restart the service after changing ranking configuration. Unknown legacy strategy
names continue to fall back to presence ranking.

Reciprocal rank fusion sums `1 / (60 + rank)` across engines for each normalized
URL. Rank is the one-based position in each engine's returned list, not its
possibly incomparable numeric score. An engine contributes only once per URL,
using its first occurrence. Tracking-parameter normalization matches presence
ranking. Tier 1 always precedes Tier 2, even when Tier 2 has a larger fusion score;
duplicates preserve the higher-priority tier. Equal scores use normalized URL as
a deterministic tie-breaker. The engine with the alphabetically first name
supplies duplicate display fields. Inputs and provenance sets are not mutated.

## Reproducible evaluation and decision

Run `python -m pytest tests/test_rank_fusion.py --no-cov -q -s`.
The test contains three **synthetic**, author-judged intent/feed pairs. Grades are
3 for a direct answer, 1 for related material, and 0 for irrelevant material:

| Intent | Grade 3 | Grade 1 | Grade 0 | Presence nDCG@3 | Fusion nDCG@3 |
|---|---|---|---|---:|---:|
| Why the sky is blue | Rayleigh explanation | Weather information | Shop | 0.5413 | 0.9828 |
| TaskGroup cancellation | API documentation | Tutorial | Release page | 0.7098 | 1.0000 |
| Trial evidence for intervention X | Trial | Review | Advertisement | 1.0000 | 0.6443 |

These labels describe constructed documents, not inspected live pages. The fixtures
exercise feed-order sensitivity, including a regression when an irrelevant item
ranks highly in another engine. The science regression also shows that arbitrary
URL tie-breaking can affect relevance. They are regression evidence for the
algorithm, **not evidence of a production relevance improvement**. No live engine
outputs, independent assessors, confidence intervals, or latency measurements are
included. Therefore changing the default is not justified.

Before enabling broadly, collect representative general/code/science queries with
captured engine responses, judge pooled results independently, and compare paired
nDCG@10 and coverage within the same tier policy. Include disagreement and sparse
feeds, audit per-domain regressions, and measure end-to-end latency separately.

Search metadata reports the effective ranking algorithm, including fallback to
presence for unknown legacy names. Cache entries and snapshots persist the
explanation used to produce their results. Reading a snapshot after configuration
changes preserves that historical explanation; legacy snapshots lacking the new
field retain the presence default. Direct searches and research subquery captures
use the same provenance field (issue #254).
