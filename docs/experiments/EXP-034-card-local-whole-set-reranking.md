# EXP-034: Card-local judgment with complete-set transport

## Registration — before candidate calls

- Registered2026-10-03; related#504; baselineEXP-033 frozen44/45 public result sets and labels. These cases are exposed; this is a mechanism screen, not held-out product validation.
- Problem: scoring all candidates in shared state changes scores with batch composition. TypeSafe's current API explicitly supports structured question instructions containing question-local data: https://docs.typesafe.ai/api and https://docs.typesafe.ai/advanced/structure . Model documentation warns about unrelated state.
- Candidate: shared state contains only query. Each Score question's structured instructions contain only its candidate plus the unchanged relevance instruction/rubric. IDs map answers; they do not communicate semantic judgment. No source selection or result dropping.
- Compare whole-set and40-sized/rotated batches, repeated whole set, on the same already-frozen candidates. Add one clearly marked constructed80-card mixed-topic stress pool using distinct URLs from existing frozen searches, no new acquisition. Freeze its composition before calls; no quality-effect claim on this constructed pool.
- Primary mechanism metric: maximum score drift for the same card across whole/batch/rotated placement, tolerance0.50; top10 membership overlap >=0.8; exact membership100%. Compare matched EXP-033. Existing labels remain fixed.
- Quality guardrail: no observed nDCG@10 drop over0.05 against shipped first40 replay on the two exposed real queries. Passing is feasibility/robustness only, not proof of new answer usefulness or generalization.
- Fixed stopping rule: <=24 Jev requests,1 root call in flight, <=400k reported input tokens,45minutes, no newsearches. Report latency/timeouts and usage for every call, provider model `jev-1.13.0`; same ten-level rubric and card bounds. Whole request must meet existing128k byte bound; any oversized/failure is explicit incomplete/fallback.
- No automaticretry/no production change/no deployment. Invalid or incomplete answers remain unevaluated. Keyless and sensitive/policy contracts unchanged. Implementation requires supported mechanism plus meaningful larger-set and failure regression coverage; a quality adoption claim requires fresh held-out comparisons.
- Evidence under `evidence/EXP-034`, record label origins, input digests, all receipts and reproduction harness. No credentials/private deployment details.
