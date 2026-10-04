# Source-information inventory for the next preregistration

Read-only inventory of public card/provenance fields. This does not rerun ranking or answerability, and does not use reference labels, annotations, provider probabilities, or call outputs to choose data. The earlier note `source-inventory-initial.md` is retained unchanged; this is the corrected scope/count version.

## Frozen acquisition captures versus EXP-046 request pools

The nine EXP-046 main source captures `docs/experiments/evidence/EXP-036/q1-frozen.json` through `q9-frozen.json` contain 295 rows in total (34, 33, 30, 35, 30, 31, 34, 30, 38). Every candidate row has only `id`, `title`, `url`, and `snippet`. The corresponding EXP-046 initial request states under `docs/experiments/evidence/EXP-046/requests/q*-d-choice-step1.json` contain 291 rows: q1 32, q2 33, q3 30, q4 35, q5 30, q6 31, q7 32, q8 30, q9 38. All retained card values match the source snapshot; no new rows appear.

Five source rows are absent from the EXP-046 states: q1 c2 `https://arxiv.org/html/2601.13295v1`, q1 c6 `https://arxiv.org/html/2609.32662`, q7 c1 `https://arxiv.org/html/2605.27710v1`, q7 c13 `https://arxiv.org/html/2511.16198v1`, and research c1 `https://arxiv.org/html/2501.09136v4`. In each case an `arxiv.org/abs/` or `/pdf/` card with the same numeric arXiv base remains in the request. This is consistent with the current arXiv base-version grouping path; it is not evidence about hidden content or independent publication quality. Keep both original membership and current grouped membership explicit in any new capture record.

EXP-040 `docs/experiments/evidence/EXP-040/extended-pools.json` maps natural development pools to prior frozen sources and stores each source SHA-256. Those hashes match:

- cardiac: `EXP-036/q10-frozen.json`, 44 source rows → 44 EXP-046 rows;
- research: `EXP-034/full-research-frozen.json`, 45 source rows → 44 EXP-046 rows;
- evaluation: `EXP-034/full-evaluation-frozen.json`, 44 source rows → 44 EXP-046 rows.

The EXP-040 prepared D states and EXP-046 requests still contain only the same four candidate fields. Across the nine q1–q9 captures plus these three natural captures, the full saved acquisition count is 428; the actual EXP-046 request-state count is 423.

## Identifier and excerpt inventory

Counts below are card rows whose URL matches the current identifier URL recognizers. They are not unique works, identity judgments, or evidence of source authority. A row can have multiple namespace anchors.

| Set | Rows | DOI URLs | arXiv URLs | PMID URLs | PMCID URLs | Rows with any recognized URL ID | Snippets >300 chars | Max snippet |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| EXP-036 q1–q9 source captures | 295 | 82 | 27 | 10 | 4 | 123 | 180 | 848 |
| EXP-046 q1–q9 request states | 291 | 82 | 23 | 10 | 4 | 119 | 178 | 848 |
| EXP-040 cardiac/research/evaluation full sources | 133 | 30 | 13 | 32 | 4 | 79 | 61 | 848 |
| EXP-046 cardiac/research/evaluation requests | 132 | 30 | 12 | 32 | 4 | 78 | 60 | 848 |
| Combined full source captures | 428 | 112 | 40 | 42 | 8 | 202 | 241 | 848 |
| Combined EXP-046 request states | 423 | 112 | 35 | 42 | 8 | 197 | 238 | 848 |

The snapshots have no structured DOI/arXiv/PMID crosswalk, per-card engine attribution, authors, `published_date`, abstract field, publication type, payload, or work-group membership. EXP-036 does retain query-level aggregate `engine_outcomes`; its source capture marks partial engine coverage and the full deployed revision unverified. EXP-034 frozen sources retain candidate/query fields but no per-card engine outcomes. EXP-040 records which frozen file supplied each pool, not the original raw engine responses. Thus these artifacts do not preserve the full normalized search receipts; whether richer records existed elsewhere is unknown.

The archived snippets are bounded excerpts, but the exact capture-time clipping rule is not documented. Many exceed the current MCP compact-card 300-character slice (`slopsearx/mcp/result_serialization.py`); the archives are therefore not identical to that current projection. Do not assume a saved snippet is a complete abstract or source body. The historical registrations exclude full copyrighted articles and refer to bounded card excerpts without documenting one uniform character cap.

## Current runtime schema and what it can retain

`slopsearx/adapter.py` defines `SearchResult` fields for primary and contributing engines, content, `published_date`, structured payload, and optional `work_group`. Science adapter payloads are not uniform: arXiv can report arXiv ID, DOI, authors, journal reference, revision date, and abstract; Semantic Scholar reports DOI/PMID/PMCID/arXiv identifiers and authors; PubMed reports typed IDs, publication types, journal, and full authors; OpenAlex reports DOI/OpenAlex ID and publication date while its abstract is carried in content. These are source-reported adapter fields when present, not guarantees for every result. Payload provenance distinguishes adapter, normalized, and inferred fields.

`slopsearx/service.py` calls `group_publications` before ranking. `slopsearx/scholarly.py` (`scholarly-work-v3`) groups on normalized URLs, recognized DOI/PMID/PMCID/arXiv identifiers and bounded explicit version relationships, not title/date/citation similarity. It supports 64 members and 64 KB per group (128 KB record limit); member records can retain engine, original position, URL/title/content/date, identifiers, and payload. Current URL recognizers cover DOI resolver and selected Nature routes, PubMed/PMC routes, and arXiv abs/pdf/html. Cross-namespace identity needs source metadata or explicit relations; URL-only frozen cards cannot supply those relations. `slopsearx/publication_metadata.py` allowlists fields such as DOI, journal/publisher/type, authors, ISSN/ISBN/tags, and volume. Published date stays on the common result record. Compact MCP cards slice content and may omit larger payloads; the expanded record path carries retained payloads.

## Capture implication

For another prospective study, save the exact public card alongside the pre-projection normalized per-card record: engine contributors, adapter-reported identifiers and typed metadata, publication date, content/abstract clipping status, payload provenance, and the pre-ranking work-group members/representative. Preserve source and post-group membership separately. Missing/conflicting metadata should remain unknown. This lets a study measure what the current whole-pool consumer could actually see without reconstructing provenance from snippets or confusing grouped-work count with acquired-row count. Authority, evidence sufficiency, and downstream usefulness remain separate questions.

No provider, Brave, engine, page-fetch, reviewer, or search calls were made. No repository or deployment files were changed.
