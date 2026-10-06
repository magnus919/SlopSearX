# EXP-088 — short evidence labels with exact reference validation

Status: development in progress. All sixteen fresh card assessments passed exact-reference validation; [captured-page assessment and the quality comparisons remain](evidence/EXP-088/in-progress/readout.md). No ranking or task-use result yet.

EXP-087 stopped when its twelfth card assessment selected an unknown long evidence ID. Its [terminal outcome](evidence/EXP-087/completed/readout.md) remains invalid and receives no ranking-quality credit. This study tests a narrower interface change: short, closed evidence labels that expand privately to the original exact references.

The labels change only selected evidence-reference IDs. Passage text, reference windows, presentation order, task instructions, rubric, facets, candidate and numerical acceptance gates remain unchanged. A source passage keeps the same label across roles and answer arms. Each role still accepts only its originally permitted references. Decoding preserves order and multiplicity, then applies the original validators. Unknown, malformed, cross-scope or altered bindings remain terminal failures; there is no closest-match substitution or response repair.

Card fingerprints and source, packet and claim identities retain their original schema fields. This study does not remove copying requirements for those record identifiers or claim to eliminate every possible transcription failure. Prompt changes distinguish evidence-label selection from the unchanged record identities.

Private bindings pin the original catalogs, encoded inputs and exact reference mapping. Qualification must exercise the real frozen packet constructors and answer runner, including its neutral readiness requests, before admission. All assessments are fresh. Prior assessment outputs are excluded from the reusable input bundle.

The study reuses the frozen public-document pools and completed 442-source inventory without new searches or source fetches. The original call, byte, concurrency and elapsed-time limits remain binding. Shorter labels do not establish research usefulness: complete independent assessments, the fixed ranking and task-use comparisons, and untouched confirmation are still required by the [production acceptance checklist](complete-pool-production-acceptance.md).

See the [protocol](evidence/EXP-088/protocol.json). This registration does not implement a production ranker.
