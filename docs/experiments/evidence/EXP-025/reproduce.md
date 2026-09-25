# EXP-025 corpus eligibility audit

Run the inert `audit.py.txt` script with Python 3 from a checkout of baseline
`1cbde5b07e5c546d955d23ca1eb01652e9f353d6`. Copy the script outside that checkout,
then run `python3 /private/tmp/exp025-audit.py`. It inventories tracked fixture
and experiment artifacts, hashes them, and summarizes selected JSON schemas.
It does not run service calls or decide relevance automatically. Original
command exited 0. Inventory checksums pin the reviewed inputs.

## Manual eligibility assessment

| Inputs | Available evidence | Missing prerequisite |
| --- | --- | --- |
| tests/test_rank_fusion.py; RETRIEVAL_QUALITY_EVALUATION.md | Three author-judged synthetic families | Captured confirmation corpus and independent pooled relevance grades |
| EXP-004 | 12 navigation queries, acquisition failures | Useful retrieved candidate pool; broad family coverage |
| EXP-005/006 | 12 Brave navigation queries each, target matchers and rank observations | Multi-engine feeds; complete graded pooled judgments; documented redistributable corpus (methods explicitly disclaim this) |
| EXP-007–012 and EXP-015 | Robustness, stopping, routing and annotation fixtures | These task labels do not grade the full retrieved union for relevance |
| EXP-013/014 | Captured acquisition summaries and specialist routing/utility labels; 30 queries in EXP-014 | Engine-level essential/useful labels are not per-result relevance judgments; prior development exposure |
| EXP-016–021 | 20-query acquisition set; card types, direct-lead and source utility labels | Fewer than 30 queries; selective card labels, not complete 0–3 pooled judgments |
| EXP-022/023 | Reuse of prior exposed routing/planner corpora | No new independent confirmation corpus |
| EXP-001–003/024 and other adapter fixtures | Transport, correctness and latency evidence | No independent captured relevance pool |

The assessment concerns tracked repository artifacts at the pinned SHA, not
all data that might exist elsewhere. Schema inspection alone cannot prove
independent judging or permission to redistribute: associated methods supply
those limitations. No new labels were invented. No candidate ranking was run.

## Next admissible step

Obtain a frozen, reusable corpus with original ordered feeds and independently
judged full candidate pools, satisfying the registered family/count criteria.
Record provenance and judge blinding before either ranker is evaluated. A
future cycle should resume EXP-025 only when these inputs exist; do not repeat
this audit without new data or reinterpret routing labels as relevance grades.
