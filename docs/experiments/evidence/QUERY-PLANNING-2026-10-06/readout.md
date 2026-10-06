# Query-planning pilot readout — 2026-10-06

## Decision

Decomposition (EXP-079) and evidence-conditioned follow-ups (EXP-080) pass the registered **fixed-corpus reading-lead recovery** gate. Terminology expansion (EXP-081) is **inconclusive** because acquisition rate-limited and stopped. None qualifies a deployed automatic planner or establishes answer quality. No runtime implementation or production-default change follows.

| Arm | Predeclared papers recovered | Complete two-paper tasks | Absolute recovery gain | Bonferroni-adjusted 98.3333% paired query-bootstrap interval | Outcome |
| --- | --- | --- | --- | --- | --- |
| Fixed counterevidence query texts | 5/24 (20.83%) | 0/12 | Reference | Reference | Baseline |
| Decomposition | 18/24 (75.00%) | 7/12 | +54.17 percentage points | +29.17 to +79.17 points | supported within this pilot |
| Evidence-conditioned follow-ups | 20/24 (83.33%) | 8/12 | +62.50 percentage points | +41.67 to +83.33 points | supported within this pilot |
| Terminology expansion | Not estimable | Not estimable | Not estimable | Not estimable | inconclusive; rate-limited |

Primary denominator: twelve compound questions and their 24 frozen paper titles. This is target discovery among first-ten cards per query, not exhaustive relevant-evidence recall or successful completion of the substantive comparison. Each arm has a three-request ceiling. All arms share exactly the same original first-pass receipts; only query text changes. Baseline uses actual shipped counterevidence text templates, with source scope controlled to OpenAlex. This is not a claim that the full default multi-engine planner recovers only 20.83%.

## Qualification, execution and resource use

Baseline code: `17727a71daf2116e2337f293c9261361140eeb52`; registration commit: `42ed87cc0999b44c5d88d452611881b11c221d3f`. Same Python virtualenv and dependencies are retained in `environment.json`. Candidate plans, actual request bytes, parents, outcomes, cards and phase source hashes are retained. Runtime source did not change.

Thirteen offline qualification checks passed before live acquisition. All acquisition and analysis commands in the protocol executed with exit status 0. The expansion command exited normally after its registered-guardrail stop. Qualification was repeated before the operational-stop harness version; the only code difference is retained in `availability-stop-amendment.md`. Both harness versions are inert `.py.txt` records.

97 fresh study requests: 15 shared originals, 30 baseline suffix queries, 24 decomposition, 24 adaptive, 4 expansion. Logical per-arm calls, including shared originals and controls: baseline 45, decomposition 39, adaptive 39, expansion 19 issued (39 maximum planned). Candidates preserve exact-title controls with their first pass and omit six unnecessary suffix searches. Separate task-neutral probes: one successful OpenAlex call and one arXiv timeout. Zero paid search, Jev or separate model-provider calls; the interactive caller's subscription cost is not measured. Stage wall-clock span about 382.7 seconds, including planning/tool gaps and cooldown. Service timings exclude planner/tool latency and cannot establish end-to-end speed gains.

91 study requests returned ok, six rate_limited. Baseline and decomposition availability: 100%; adaptive: 37/39 = 94.87%, above the registered 90% threshold. The last two adaptive requests were not retried. All four expansion requests rate-limited; four failures make the best possible full-arm availability 35/39 = 89.74%, so the arm stopped with 20 operations uninvoked. No substitutions or rescue searches occurred. Expansion's absence of new recovered titles is missing acquisition evidence, not negative evidence about the planning method. See `analysis-validity.md` for why raw partial zero-effect arithmetic is suppressed in `decision-summary.json`.

## Guardrails and slices

All completed arms preserve policy, allowed source, request/query budgets and serialization identity. No sensitive source, credentials, private query, source substitution, storage/tenant mutation or production change. Queries stayed under 500 characters; every result's engines round-tripped as the OpenAlex provenance set. Reservations align one-to-one with all 97 retained receipts. No duplicated attempts or unrecorded retries. Evidence verification is retained in `evidence-verification.json`.

Decomposition family gains: retrieval/planning +62.5 points; model methods +25 points; retrieval infrastructure +75 points. Adaptive: +75, +50, +62.5 points respectively. No registered family nonregression gate failed. Exact-title controls have zero candidate regression; Query2doc and MTEB are recovered, but LoRA is missing in baseline and candidate. An unchanged miss is not proof of reliable exact-title retrieval.

Retain the unsuccessful tasks: decomposition does not recover either original Transformer/BERT target; it misses the listwise reranking target and recovers only one target each for tool use, corrective/self-reflective retrieval and low-rank adaptation. Adaptive improves the Transformer/BERT and listwise cases but still misses ReAct, Self-RAG and original LoRA, and its BEIR follow-up rate-limits. Per-case records and matching IDs are in `analysis-rows.json`.

## Interpretation and validity limits

Concise facet queries help locate foundational papers that verbose compound/suffix queries miss in this selected OpenAlex corpus. Evidence-conditioned follow-ups can choose a missing facet based on observed cards; all new named methods not present in a card are explicitly marked caller prior knowledge. This is a useful development signal for query planning.

The same interactive caller authored the questions, knew the references, and wrote the candidate plans. The corpus is exposed, deliberately challenging, scholarly and small; it is not an untouched test set or representative user sample. No deployed planner/model version or repeated stochastic runs were tested, and no independent answer-quality judgments were collected. Some adaptive gains use caller-known method names or an exact paper title; the experiment does not isolate evidence-conditioning from prior knowledge or shortening/name specificity. The adaptive arm uses its final discovery slot even when both facets already have leads; adaptive stopping was not tested. Two adaptive acquisitions fail and expansion has no usable follow-up data. Sequential arm execution and later OpenAlex quota pressure are confounds. No registered head-to-head significance test compares the two completed candidates, so the larger adaptive point estimate alone does not establish superiority.

## Next decision

Advance decomposition and evidence-conditioned planning to a fresh, independent confirmation design with unseen questions, fixed planner model/prompts, independent evidence-support grading and an equal-budget concise-query control. Freeze scorer references outside planner context. Test uncertainty/error propagation and stopping separately. Run expansion only as a newly registered trial with adequate acquisition capacity; do not reuse this interrupted attempt as a successful or negative quality test. No automatic background run, implementation, merge of runtime changes or deployment is initiated.

Portal impact: documentation and inert evidence only; browser-visible contracts unchanged. Under the repository experiment guide, no CI/pre-commit or reviewer requests are made for the documentation PR. Registration, plans, raw observations, all failures, analysis validity, qualified decisions and readout are retained with checksums.

Documentation persistence: [PR #678](https://github.com/magnus919/SlopSearX/pull/678). Registration remains a separate signed commit before measurement.
