# EXP-006: Jev fusion on hard documentation queries

Status: **completed; not supported**

## Hypothesis

A fixed `0.70 * reciprocal Brave rank + 0.30 * Jev probability` fusion improves
canonical-target MRR@20 on 12 indirect, task-shaped documentation queries that
offer more ranking headroom than EXP-005.

## Frozen method and gates

The harness contains the 12 queries and canonical URL matchers. Each query gets
one Brave web request for at most 20 candidates and one Jev request. No retries.
The prompt and tie-breaks are identical in intent to EXP-005.

Supported-for-further-research requires: target coverage at least 8/12; valid
Jev calls at least 11/12; absolute MRR gain at least `0.05`; no more than two
target regressions and no drop greater than three ranks; Jev p95 at most 1,500
ms; and estimated cost below `$0.01`. Coverage/validity failure is
inconclusive. Jev-only ranking is diagnostic.

The live Brave corpus is not a redistributable frozen corpus, so even a
supported result cannot close the provenance/rights or user-value gaps.

## Result

Coverage was 11/12 and all 12 Jev calls were valid. Brave MRR@20 was
`0.6845`; fusion reached `0.6952`, an absolute gain of only `+0.0107` versus
the required `+0.05`. There were no regressions. The run is **not supported**.

See the [suite results](JEV-FUNCTION-SUITE-RESULTS.md) and
[evidence](evidence/EXP-006/summary.json).
