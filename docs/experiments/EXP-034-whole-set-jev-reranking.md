# EXP-034: Whole-set Jev reranking

## Registration

- State: registered, 2026-10-03; maintainer requested; related #504.
- Question: Does scoring every acquired, deduplicated candidate improve relevant-tail access compared with the shipped first-40 policy, without unstable cross-batch judgments or unacceptable completion time?
- Baseline: SlopSearX `6bb683d4a9e154186c9d76af1b55e4faa24cf0e4`; Jev `jev-1.13.0`, existing ten-level relevance rubric and card projections. Actual deployed source identity is separately recorded; reviewed source is not deployment proof.
- Acquire at most eight public searches (four initial, four only if none yield an above-40 set), no changes to services. Queries cover enterprise agentic software engineering, agent evaluation, software supply-chain security and research orchestration. Preserve all returned candidates, available scholarly identifiers, ordering, engine outcomes and ranking explanation. A bounded search result set is not a full web corpus.
- Compare deterministic replay, shipped first-40 semantic policy, whole-set single request where provider bounds permit, and complete-set batches of at most 40 with stable global ordinal-score ordering. Same model/rubric/card projection. Use unchanged retrieved snapshots, not further searches per arm.
- If deployed search returns semantic order, recover the configured deterministic order only from adequate captured source fields/production ranker inputs. Otherwise label first-40 comparison as returned-order replay, not an exact production-incumbent reproduction. Never infer an unobserved pre-rerank pool.
- Freeze assistant-reviewed relevance labels before calls: 0 unrelated, 1 topical context, 2 useful query-specific facet, 3 direct requested evidence. No independent-gold claim. Synthetic tail and distractor controls, if needed, remain separate from real search results.
- Primary: paired nDCG@10 and recall@10 of grades >=2, plus relevant-tail promotion above position40. Minimum useful nDCG gain 0.05; no observed required facet lost. Query-level bootstrap 95% interval seed504; fewer than eight eligible queries is feasibility/exploratory evidence only, not general adoption.
- Batch guardrail: repeated/changed batch placement scores and ordering; record absolute score drift, membership, stable ties and relevant facet retention. A matching ordinal rubric is not calibration proof. Measure at least one reverse/rotated batch replay on a >40 set.
- Feasibility: all-or-nothing membership, configured one-second incumbent deadline versus measured whole-set latency. Report p50/p95, timeout rate, request counts and tokens; up to10s per batch for feasibility, no default change. Candidate >provider/body bound is explicit incomplete/fallback, not truncated whole-set success.
- Stop: maximum8 search requests,64 Jev requests,1 root-lane call in flight; <=500,000 reported input tokens (stop before further calls after crossing),90-minute study. Other independently registered studies count separately. No GPU or model training.
- Provider reference: https://docs.typesafe.ai/models documents64k total tokens and32k state+longestquestion; https://docs.typesafe.ai/model-jaggedness/jev-1.13 warns about distracting state and numeric calibration. Constraints verified2026-10-03; enforce body bounds and record provider errors rather than guessing tokenizer equivalence.
- Evidence: frozen sanitized snapshots, labels, content-free request receipts, summary and inert reproduction harness under `docs/experiments/evidence/EXP-034/`. Keys/private endpoints/deployment metadata are excluded. Public card text may be retained with source attribution and bounded excerpt sizes; no full copyrighted articles.
- Decision: advance only after real >40 pool comparisons, positive primary effect and guardrails; otherwise report feasibility, not-supported, inconclusive or blocked. No production activation/deployment and no modification of the existing reranker in this study.
- Compute: local orchestration CPU-only; remote authorized TypeSafe inference. No GPU workload added.
- Portal impact: none in this study. A later implementation must update the shared HTTP/MCP/CLI/snapshot/portal explanation together.

## Readout

Pending execution. Registration commit will be recorded in the evidence manifest without amending this plan.

## Readout — first screen

The eight registered searches produced 20–45 results; two exceeded the current
shortlist bound. Fourteen Jev requests returned valid exact-membership scores;
168,657 input and 6,496 output tokens were reported. The rerank and scholarly
module hashes match the reviewed source. The deployed shared service hash differs,
and HTTP output does not expose the applied ranking explanation. This is a
returned-order replay, not a claim of exact pre-rerank incumbent reconstruction.

Single requests scored the complete 44/45-result sets in 410/263 ms observed
provider time. Against first-40 replay, nDCG@10 changed by 0.000/-0.0051
(mean -0.0026; exploratory two-query bootstrap 95% interval [-0.0051, 0.000]).
The 0.05 advancement effect was not met, and n=2 is inadequate for generalization.
All first-40 and whole-set top tens contained ten assistant-labeled useful cards.
Whole-set scoring did not promote a tail card into these top tens.

Naive global sorting of separately scored batches promoted useful tail cards,
but harmed the second query's judged nDCG@10 (0.9184 → 0.7507).
Scores changed by up to 2.11 points across whole/batched contexts, and by up to
1.67 when batch placement changed. Do not ship naive cross-batch score fusion.
The observed maximum over all fourteen provider requests was410 ms; this does
not establish a production p95, concurrent deadline behavior or provider bounds
for larger sets. Reference labels are assistant card judgments, not independent
gold, and no answer-quality gain was measured.

**Decision:** whole-set single-request feasibility is demonstrated at44/45;
quality uplift remains inconclusive; naive cross-batch sorting is not supported
on this screen. No runtime change or deployment. Retain frozen cards, labels,
usage, all requests and the inert harness in [evidence/EXP-034](evidence/EXP-034/).
Reproduction uses the pinned source environment, frozen inputs and a supplied
credential-safe proxy; model replay may differ. Provider costs are not inferred
from token telemetry. See `analysis.json` and `summary.json` for complete trials.

A next mechanism study will test documented structured questions: put each card
in its own question and share only the query in state. This removes unrelated
candidate context from each individual judgment while retaining batched transport.
It requires its own preregistration and cannot be promoted from exposed cases
alone. Historical snapshots and this negative result remain intact.


### Reproduction correction

Independent review found that the original token tally counted only receipts ending in `-0.json`, omitting later batches. The observed complete receipt totals remain below the registered 500,000 ceiling, but the original stopping guard did not enforce the protocol correctly. Preserve the executed original as `harness-original.py.txt`; the reproduction harness now counts all JSON receipt usage and uses the registered ceiling. This correction changes no captured judgments or reported results.


### Publication identifier reconciliation

The original signed registration used `EXP-033`. Concurrent upstream work reserved that shared ledger sequence before publication. This study is published under the identifier in this filename; the original signed commit history remains intact. No inputs, judgments, labels, or gates changed during identifier reconciliation.
