# EXP-035: Card-local judgment with complete-set transport

## Registration — before candidate calls

- Registered2026-10-03; related#504; baselineEXP-034 frozen44/45 public result sets and labels. These cases are exposed; this is a mechanism screen, not held-out product validation.
- Problem: scoring all candidates in shared state changes scores with batch composition. TypeSafe's current API explicitly supports structured question instructions containing question-local data: https://docs.typesafe.ai/api and https://docs.typesafe.ai/advanced/structure . Model documentation warns about unrelated state.
- Candidate: shared state contains only query. Each Score question's structured instructions contain only its candidate plus the unchanged relevance instruction/rubric. IDs map answers; they do not communicate semantic judgment. No source selection or result dropping.
- Compare whole-set and40-sized/rotated batches, repeated whole set, on the same already-frozen candidates. Add one clearly marked constructed80-card mixed-topic stress pool using distinct URLs from existing frozen searches, no new acquisition. Freeze its composition before calls; no quality-effect claim on this constructed pool.
- Primary mechanism metric: maximum score drift for the same card across whole/batch/rotated placement, tolerance0.50; top10 membership overlap >=0.8; exact membership100%. Compare matched EXP-034. Existing labels remain fixed.
- Quality guardrail: no observed nDCG@10 drop over0.05 against shipped first40 replay on the two exposed real queries. Passing is feasibility/robustness only, not proof of new answer usefulness or generalization.
- Fixed stopping rule: <=24 Jev requests,1 root call in flight, <=400k reported input tokens,45minutes, no newsearches. Report latency/timeouts and usage for every call, provider model `jev-1.13.0`; same ten-level rubric and card bounds. Whole request must meet existing128k byte bound; any oversized/failure is explicit incomplete/fallback.
- No automaticretry/no production change/no deployment. Invalid or incomplete answers remain unevaluated. Keyless and sensitive/policy contracts unchanged. Implementation requires supported mechanism plus meaningful larger-set and failure regression coverage; a quality adoption claim requires fresh held-out comparisons.
- Evidence under `evidence/EXP-035`, record label origins, input digests, all receipts and reproduction harness. No credentials/private deployment details.

## Readout

All21 requests returned valid pinned-model scores;304,723 input and11,874 output
tokens were reported. Complete44/45-result requests took399/359ms and the
constructed80-card request took388ms. No transport/provider failure occurred;
maximum observed provider time including repeats was630ms. These observations
are not a production latency percentile or a universal context-size guarantee.

Card-local questions reduced maximum whole/batch score drift from2.11 inEXP-034
to0.41 here; maximum drift including repeated whole sets was0.45. Top-ten overlap
was0.9–1.0 across batch placement/repeat treatments. The drift, membership and
observed latency mechanism checks passed. A relevant tail repository entered
the software-benchmark top ten without displacing a useful card under the fixed
assistant labels.

Quality did not establish superiority: the benchmark query stayed at nDCG@10
1.000; the agentic-RAG query changed from0.9184 incumbent replay to0.8687
(-0.0497), narrowly inside the registered -0.05 tolerance. The latter is a real
warning, not an improvement claim. The80-card mixed-query stress set is constructed
and its placeholder labels are excluded from quality conclusions. Inputs are
exposed and reference labels are best-effort assistant assessments. No fresh
held-out answer-quality validation has occurred.

**Decision:** the card-local complete-set transport mechanism merits fresh
held-out evaluation. Do not substitute naive cross-batch fusion or claim a
quality/default adoption decision from this screen. No runtime implementation or
deployment. [Evidence](evidence/EXP-035/) includes all frozen inputs, receipts,
comparisons and an inert reproduction harness; caller supplies credential-safe
transport. Request receipts preserve model, complete membership, timing, usage
and digests without credentials or private deployment metadata.


### Reproduction correction

Independent review found that the original token tally counted only receipts ending in `-0.json`, omitting later batches. EXP-035 also used 500,000 rather than its registered 400,000 input-token ceiling. The observed complete receipt totals remain below the registered 400,000 ceiling, but the original stopping guard did not enforce the protocol correctly. Preserve the executed original as `harness-original.py.txt`; the reproduction harness now counts all JSON receipt usage and uses the registered ceiling. This correction changes no captured judgments or reported results.


### Publication identifier reconciliation

The original signed registration used `EXP-034`. Concurrent upstream work reserved that shared ledger sequence before publication. This study is published under the identifier in this filename; the original signed commit history remains intact. No inputs, judgments, labels, or gates changed during identifier reconciliation.
