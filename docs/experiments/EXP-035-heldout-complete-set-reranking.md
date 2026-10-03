# EXP-035: Fresh task-specific complete-set reranking

## Registration

Registered2026-10-03 before acquisition/labels/model calls; followsEXP-033/034
and#504. This fresh screen evaluates the card-local mechanism on eight specified
user-oriented research queries. It does not train a scorer or alter runtime.

- Queries: coding-agent collaboration/conflict benchmarks; enterprise approval/audit governance; durable execution/replay/external effects; SWE-bench leakage/time-split evaluation; SLSA verifier threats; OpenTelemetry agent tracing conventions; claim/passage support and contradictions; research stopping/evidence gaps.
- Acquire one current shared-service search per query (8 total), explicit same non-sensitive engines where active: brave,duckduckgo,wikipedia,openalex,semanticscholar,arxiv,pubmed,github. Prefer fresh; disable suggestions. A separate read-only service process may capture the deterministic pre-rerank pool by wrapping the configured ranker's result return, without changing server code/configuration. Record exact component hashes and final applied explanation; never infer full deployed revision from module hashes.
- Freeze complete deduplicated candidate pools and best-effort relevance labels before candidate calls. Labels0unrelated,1context,2usefulfacet,3directtaskevidence. Root and Luna reviewer assess independently before seeing candidate scores; document disagreements and resolution. No independent-gold claim. Queries with<=40results are controls, not proof of tail benefit; fewer than4above40 cases yields an inconclusive tail-benefit screen.
- Compare current first40 shared-state Score replay and whole-set card-local Score on identical captured deterministic pools, pinnedjev-1.13.0 and same card text bounds/rubric. No newly searched input per arm. Complete-set requests exceeding128k bytes are explicit unevaluated fallback; no silent truncation. No naive cross-batch pooling in this screen.
- Primary: query-paired nDCG@10, useful@10 and useful tail promotion. Minimum meaningful nDCG uplift0.05; no claim of quality support unless mean>=0.05 and query-bootstrap95% interval excludes0. Completeness feasibility is separate. Per-query quality guard: delta>=-0.05 and no loss of a prelabelled required facet from top10. Report ceiling effects and uncertainty rather than changing a failed gate after results.
- Stability guard: one repeated complete-set request on two predesignated queries1/3, membership100%, top10overlap>=0.8; no observed provider timeout in complete requests under the incumbent1s deadline. Measure all observed times and do not call8queries productionp95.
- Budget:8searches; candidate<=20Jevrequests plus atmost8 configured search rerank requests;1rootlaneinflight; <=500k reported candidate inputtokens,60minutes. Labels/public research cases only; no credentials/private endpoints published. Actual search rerank usage may be unavailable and must remain distinct, notzero.
- Fixed analysis: query bootstrap10,000 resamples seed504; report all exclusions and strata. Invalid transport/output unevaluated, deterministic replay comparison available; no retries except identified setup repair with preserved attempts and unchanged labels/questions.
- Evidenceunder`evidence/EXP-035`, frozen inputs/labels/digests, source hashes, all calls/usage, inert harness and readout. No deployment/default change. Fresh measured benefit and guardrails are required before an implementation recommendation; a negative screen remains useful evidence.


## User-directed breadth extension (registered before additional acquisition)

After the original eight searches returned only 30–35 cards each, Magnus suggested topics with abundant general and academic coverage. Add two separately reported breadth probes: `climate change impacts mitigation adaptation research` and `cardiac health cardiovascular disease prevention research`. This is a prospective user-directed extension, not an unmodified preregistered eight-query result or post-hoc selection of favorable rankings. No medical or climate guidance is being assessed.

Expanded ceiling: ten total search acquisitions, at most ten configured search-rerank requests, at most 22 candidate Jev requests, and 600,000 reported candidate input tokens. Permit 60 additional minutes for these two probes; the original eight-query deadline and quality gates remain unchanged. Freeze cards and assistant labels before candidate calls. Report original eight and extra two separately; no change to the requirement for at least four natural pools above 40 before a tail-benefit conclusion. All text bounds, membership, source coverage, stability, no-deployment, and no-silent-truncation rules remain.


### Reference-label disagreement handling (before candidate calls)

Independent assistant reviewers differ on many 2-versus-3 relevance judgments, and a few larger differences require source-card rechecks. Retain both original label sets and any pre-call correction audit. Do not manufacture a single gold label from those differences. Evaluate quality and per-query regression guards under both frozen label sets; any positive recommendation must meet the unchanged quality gate under both. Facet coverage uses the frozen reviewer tags after root checks their grounding in the supplied cards. These are exploratory assistant references, not human gold. This prospective change replaces forced single-label adjudication in the outline; it does not relax quality gates or promote disagreement into correctness evidence.
