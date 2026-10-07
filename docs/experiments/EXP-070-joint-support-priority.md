# EXP-070: joint task-support and reading-priority classification

## Hypothesis and boundary

EXP-069 stopped on the 59-card case when sequential support and comparative exchanges exceeded the shared one-second phase. EXP-068 scope-first Score and EXP-067 comparative Score failed quality gates. A single five-option Choice per card tests a distinct joint classification: unsupported, or one of four supported relative-priority bands. It is not two independent judgments or a calibrated substitute for Noul.

Every eligible card in the complete pool is judged in one exchange. Direct support uses the candidate's own visible fields; comparison uses sibling visible fields, with unsupported siblings unable to outrank direct support. The count denominator stays the entire pool minus one. No source/reference grades enter the requests.

Code validates the complete distributions, accepts raw sums within 0.01 using Decimal arithmetic (rounding already observed in EXP-046), normalizes, and places supported mass strictly above 0.5 first. Within that bucket it orders by descending conditional expected supported band. The second bucket uses canonical order only. No probability-magnitude ranking, dropped tail, chunks, or posthoc threshold changes are allowed. Canonical unsupported ordering may lose useful contextual leads; the fixed retention gates must expose that risk.

## Frozen experiment

The [protocol](evidence/EXP-070/protocol.json) is authoritative: exact pinned EXP-060 inputs and prospective EXP-064 A-v2/B-v3 references; unchanged W0 production request; same 21 operations including repeat/rotate and navigation; one neutral maximum-card request; 43 total calls, serial, no retries or searches. The [offline envelope](evidence/EXP-070/request-envelope-check.json) checks all frozen pools and 80-card neutral without provider calls. Both one-second owned HTTP and one-second whole candidate phase apply.

Both independent reference means must improve nDCG@10 by at least 0.05 with bootstrap lower bound above zero; each case loss cannot exceed 0.03; useful-count/facet retention, overlap at least 0.8, official-target navigation, exact membership, and all resource gates remain unchanged. Missing acquisition targets are still unmet recall, never ranking recovery. The single natural pool above 40 cannot establish general long-pool benefit.

## Qualification and acceptance

Luna design review found the five classes exhaustive/disjoint and required explicit sibling visibility/support semantics; those corrections are frozen here. Implementation must independently test distribution validation/normalization, threshold and integer-count boundaries, zero-support policy, atomic full-W0 fallback, durable receipts, admission pins and deadline enforcement before a live invocation. No runner exists at registration. Development must pass before untouched fresh confirmation; compatibility-preserving SlopSearX and X implementation, substantive review and required CI remain open under [production acceptance](complete-pool-production-acceptance.md).

Provider shape refreshed from [Choice documentation](https://docs.typesafe.ai/primitives/choice) and [HTTP API](https://docs.typesafe.ai/api). Observed EXP-046 Choice answer records have keys type/choice/probabilities/confidence; raw decimal sums can differ from one by 0.01. Valid shape does not establish judgment accuracy.
