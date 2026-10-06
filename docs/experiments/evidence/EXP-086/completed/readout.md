# EXP-086 terminal outcome

Status: assessment-inconclusive. Health passed with the pinned model and revision, and the complete input corpus was frozen. Five fresh card responses were emitted: four passed validation, and the fifth failed because it repeated a same-card reference ID. The run stopped before source assessments, Jev calls, answer generation or ranking measurements. No search or source-fetch request occurred.

The failed response remains unchanged and is not reference truth. A structural diagnostic found a mismatch: the supplied output schema and original EXP-077 literal-anchor verifier permit repeated anchors, while the newer ID adapter adds a rejection. Repeated known citations do not add a new source or change a grade.

EXP-087 prospectively aligns the adapter with the original contract: retain repeated known same-card IDs in their emitted order and expand each separately, without deduplication or score changes. Unknown/cross-card references, duplicate facet labels, coverage failures and all existing semantic/gate requirements remain invalid. Fresh assessments are required; none of these outcomes is repaired, reclassified or used as gold.

Two agent-thread allocations failed before inference; the eventual assessments ran sequentially. Five logical card model calls occurred, with billing/tokens unavailable through this interface. Neither allocation failure caused a repeated model assessment.
