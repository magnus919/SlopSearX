# EXP-033: Whole-set Jev reranking

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
- Evidence: frozen sanitized snapshots, labels, content-free request receipts, summary and inert reproduction harness under `docs/experiments/evidence/EXP-033/`. Keys/private endpoints/deployment metadata are excluded. Public card text may be retained with source attribution and bounded excerpt sizes; no full copyrighted articles.
- Decision: advance only after real >40 pool comparisons, positive primary effect and guardrails; otherwise report feasibility, not-supported, inconclusive or blocked. No production activation/deployment and no modification of the existing reranker in this study.
- Compute: local orchestration CPU-only; remote authorized TypeSafe inference. No GPU workload added.
- Portal impact: none in this study. A later implementation must update the shared HTTP/MCP/CLI/snapshot/portal explanation together.

## Readout

Pending execution. Registration commit will be recorded in the evidence manifest without amending this plan.
