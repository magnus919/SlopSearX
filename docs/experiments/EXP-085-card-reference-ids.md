# EXP-085 — card references using bound IDs

Status: terminal setup-inconclusive. The [retained outcome](evidence/EXP-085/completed/readout.md) records one rejected health request and zero model calls; no ranking result or production adoption claim.

EXP-084 captured 359 usable pages from the existing 442-source inventory. Its first two card assessments stopped the study because some model-written quotations did not exactly match the supplied cards. Those failed outputs remain unchanged in the [terminal record](evidence/EXP-084/completed/readout.md); they are not reference truth for this study.

This study keeps the saved search results and captured pages. It makes no new searches and does not fetch the pages again. Instead of copying quotations, each assessor selects IDs from a deterministic catalog attached to the original card. Code expands those IDs into exact source text and applies the existing validator. Unknown or cross-card IDs stop the study. There is no repair or retry.

Both independent assessors receive the complete original title, URL and snippet, plus identical catalogs. Catalog windows preserve every character and contain at most 256 UTF-8 bytes. Adding windows changes the presentation and citation granularity, so these are fresh measurements, not a paired comparison with the failed EXP-084 assessments. The rubric, facets, ranking candidate and release thresholds remain unchanged.

The candidate remains the exact EXP-076 complete-pool query/purpose scores with equal-weight RRF at k=60. Valid independent card and source references must precede ranking and task-use comparisons. A fresh qualified source closure, admission, clock and one-shot health check are required before calls. Historical clocks and outputs provide no authority or time credit.

See the [protocol](evidence/EXP-085/protocol.json), [card reference adapter](evidence/EXP-085/card-reference-id-adapter.py.txt), and [input preparation](evidence/EXP-085/prepare-assessments.py.txt). Offline checks establish interface integrity only. Untouched confirmation and the [production acceptance checklist](complete-pool-production-acceptance.md) remain required before shipping.
