# EXP-092 — explicit citation-alias representation

Status: terminal inconclusive; complete delivery passed, but a copied citation label was invalid.

[EXP-091](EXP-091-explicit-page-schedule.md) delivered the scheduled input, but the assessor mistook supplied citation aliases for original evidence IDs and requested clarification through a prohibited tool. Deterministic checks confirmed correct encoding: every supplied passage reference belongs to the paired alias binding, no passage reference is an original ID, and the encoded-input hash matches. Missing data and wrong-file wiring are not supported explanations.

This continuation adds explicit representation metadata: the values already present in each supplied passage's `evidence_id` field are the citation labels to copy verbatim into output `evidence_ids`. Their `L-…-S…-R…` form is the alias format; the assessor must not invent an alternate abbreviation, mapping, or original-ID reconstruction. Preserve task, source, facet, chunk and assessor identity fields unchanged. The only allowed tool calls are the prescribed viewer executions; no clarification messages, browsing, file writes or other tools. The exact transcript and citation validators remain unchanged.

All original packet bytes, source context, rubric, requested GPT-6 Luna model, candidate and quality/resource gates remain fixed. Sixteen immutable card assignments are carried unchanged; all 48 source and 32 answer assessments are fresh. Twenty historical grader calls plus eighty fresh assessments total 100 within 112; eighteen answer API calls yield 118 within 130. Failed calls are never refunded. No new search or source capture is allowed, and untouched confirmation remains wholly fresh.

Before qualification, verify alias metadata against the actual binding and replay the exact controller-to-eight-research-case preparation, registry projection, local dispatch and complete prompt path offline. A sterile alias-copy/decode/coverage check establishes transport semantics only, never semantic correctness or research usefulness. Publish reviewed source and qualification before a fresh process, clock and one-shot lease.

The required evidence is a fully delivered, protocol-compliant cited assessment followed by the remaining independent reference collection and the frozen task-use/ranking analysis. An unaccepted or repaired submission cannot satisfy that gap. This addresses the observed representation ambiguity, not another missing-context hypothesis. No production-readiness claim precedes the [acceptance checklist](complete-pool-production-acceptance.md).

The [terminal readout](evidence/EXP-092/completed/readout.md) records one unknown label among 85 copied occurrences and exact source/header identity preservation. No source assessment or quality credit was accepted.
