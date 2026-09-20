# EXP-006: Jev fusion on hard documentation queries

Status: **registered; not yet executed**

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

