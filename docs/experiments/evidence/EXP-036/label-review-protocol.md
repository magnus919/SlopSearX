# EXP-036 reviewer prep (scratch, not preregistration edits)

## Proposed exact frozen search queries

Use one shared-service acquisition per query, all eight as new acquisitions in the preregistered order, no follow-up searches or suggestions:

| ID | Exact query | Facets a useful result may address |
|---|---|---|
| Q1 collaboration | `multi-agent coding agent collaboration benchmark coordination conflict resolution software engineering` | Benchmark/task suite; coordination or conflict behaviors; evaluation metrics/comparability. |
| Q2 governance | `enterprise AI agent governance human approval audit logging policy controls` | Human approval/authorization; auditability/traceability; enterprise policy/governance controls. |
| Q3 durable execution | `durable workflow execution replay recovery idempotency external side effects` | Persisted/recoverable execution; replay semantics; idempotency or duplicate effects; external-effect handling. |
| Q4 SWE-bench | `SWE-bench evaluation data leakage contamination temporal split benchmark` | SWE-bench-specific evaluation; contamination/leakage; temporal split or test-set integrity. |
| Q5 SLSA verifier | `SLSA provenance verifier threat model verification builder identity attestation` | SLSA verifier requirements; provenance authenticity/builder identity; threat/attack limits. |
| Q6 OpenTelemetry | `OpenTelemetry semantic conventions AI agents GenAI traces tool calls spans` | Official conventions; agent/tool span structure; GenAI trace attributes/context propagation. |
| Q7 claim support | `claim passage evidence support contradiction citation verification benchmark` | Claim-evidence entailment; contradiction/insufficiency; citation or passage-level evaluation methods. |
| Q8 research stopping | `research agent search stopping criteria evidence gaps uncertainty coverage` | Search/research stopping rules; evidence coverage/gaps; uncertainty or saturation. |

## Public-card labeling rubric

For each query, label every candidate in the frozen deduplicated pool against that query, independently and before seeing any Jev/candidate scores or reranked order. Use only the card-local evidence actually available in the captured result (`title`, `content` snippet, and `url`); do not open pages or search again. Hide rank, score, engine, and reranking explanations during first-pass labels. Preserve stable candidate ID and query ID. Multiple facets may be tagged; facets are query-specific as in the table.

- **3 — direct task evidence:** the card explicitly presents a benchmark, primary specification, method, result, or operational design that directly answers the central question or one of its core requested facets.
- **2 — useful facet:** directly useful evidence for at least one named facet, but partial, adjacent, broad, or not enough to answer the central question alone.
- **1 — context:** relevant background or terminology, but no concrete evidence for a named facet.
- **0 — unrelated:** no material help for the query; lexical overlap alone is insufficient.

Do not use relevance labels to represent truth, source authority, novelty, safety, or whether the linked full page would be useful. A source title/domain alone cannot justify 2/3 when the snippet does not establish the facet. Conversely, do not downscore a relevant card merely because the publisher is secondary or unfamiliar. Note uncertainty separately rather than silently guessing. If a result has too little card-local text to judge, retain it and mark `label_status=uncertain` with a short reason; do not drop it or call it unrelated by default.

For each candidate record: `query_id`, immutable pool item ID, grade 0–3, zero or more facet IDs, status (`labeled`/`uncertain`), concise rationale tied to visible card text, reviewer ID, and disagreement flag. Root and Luna label independently, blind to one another and all candidate outputs; adjudicate disagreements before unblinding, retaining both original labels and the resolution. Report agreement and counts by grade/facet. These assistant labels are best-effort references, not human gold.

At analysis, preserve query-paired comparisons and original pool membership/order. Report all pool sizes, including <=40 controls, and count queries with >40; do not exclude small pools or select only high-scoring / attractive results. Tail promotion counts require a prelabelled grade >=2 candidate initially outside top 40 entering top 10. The preregistered required-facet guard should be assessed from facet tags among grade >=2 candidates; do not redefine facets after seeing model outputs.
