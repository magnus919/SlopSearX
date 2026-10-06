# Three arXiv-grounded improvement candidates

Research question: Which research-backed changes could improve SlopSearX beyond its current implementation?
Date and source access: 2026-10-06. Depth: moderate, targeted scan, not a systematic literature review.
Inspected checkout: `a39f795129313d3355d8ed1a0e4d6bcc7ed33ec6`.

## Summary

Prioritize a caller-directed query-expansion experiment, then bounded query-aware reranking, then retrieval-supervised specialist routing. These are hypotheses, not measured improvements. The checkout already implements optional RRF, deterministic cost/coverage routing, Jev specialist augmentation, and caller-directed adaptive research. Existing experiments are important counterevidence: routing classification gains have not consistently translated into useful retrieved results. No implementation, model calls, benchmark reproduction, or live search-engine acquisition was performed for this research note.

## Source evidence records

### S1 — Query2doc

Wang, Yang and Wei, *Query2doc: Query Expansion with Large Language Models*, EMNLP 2023, [arXiv:2303.07678v2](https://arxiv.org/html/2303.07678v2).

- Method: generate a pseudo-document from the query, concatenate it with the original query, and retrieve using sparse or dense retrieval. Sparse retrieval repeats the original query to preserve its weight.
- Evidence: Table 1 reports BM25 TREC DL 2019 nDCG@10 increasing from 51.2 to 66.2. This is 15.0 points on the paper's displayed scale, not a predicted SlopSearX gain.
- Limitation: Table 2 contains out-of-domain regressions for some dense retrievers, including SciFact. Model quality matters. The experiments use controlled indexes rather than heterogeneous upstream search APIs.
- Relationship: motivates candidate C1; translating generated passages into short engine-specific queries is our adaptation, not the evaluated paper method.

### S2 — RankGPT

Sun et al., *Is ChatGPT Good at Search? Investigating Large Language Models as Re-Ranking Agents*, EMNLP 2023, [arXiv:2304.09542v3](https://arxiv.org/html/2304.09542v3).

- Method: give passages stable identifiers and request a relevance-ordered permutation. Sliding windows handle larger pools; permutation distillation transfers teacher rankings to smaller models.
- Evidence: the authors evaluate ranking on IR benchmarks and introduce NovelEval to examine ranking of newer knowledge. They report competitive ranking and successful small-model distillation.
- Limitation: the paper explicitly identifies expensive inference, latency and unstable generation. Passage results do not establish effectiveness for short search snippets or this project's engines.
- Relationship: motivates candidate C2. A snippets-only, tier-preserving caller view is a constrained adaptation.

### S3 — ReSLLM

Wang, Zhuang, Koopman and Zuccon, *ReSLLM: Large Language Models are Strong Resource Selectors for Federated Search*, [arXiv:2401.17645v1](https://arxiv.org/html/2401.17645v1).

- Method: zero-shot resource selection from resource representations; SLAT labels logged query/result snippets with an LLM, aggregates judgments by resource, and fine-tunes a selector.
- Evidence: evaluated on TREC FedWeb 2013/2014. The authors report competitive zero-shot selection and stronger selection after SLAT fine-tuning.
- Counterevidence: section 5.4 finds that adding similar snippets to resource representations after fine-tuning substantially decreases effectiveness. More context is not automatically better.
- Limitation: historical FedWeb collections and resource-selection metrics do not establish useful modern SlopSearX results, latency savings, or cost savings.
- Relationship: motivates candidate C3; human auditing and marginal-value labels are proposed controls beyond the paper's synthetic supervision.

## Candidate proposals and validation

### C1 — Engine-aware query expansion through adaptive research

Observation: `plan_research_queries` in `slopsearx/research.py:167` uses fixed strategies and suffixes. `docs/ADAPTIVE_RESEARCH.md` already supports caller-supplied plans and follow-ups, so this does not require an autonomous model inside SlopSearX.

Proposal: have a caller generate a pseudo-document, extract one or two concise terminology-rich query variants, retain the original query, and submit the variants through existing research continuation. Begin with descriptive science and technical questions. Preserve exact CVE/DOI/package identifiers; generated text is a query aid, never evidence. Each dispatch retains current policy checks and durable attempt budgets.

Experiment: compare original-query retrieval against original plus expansion on held-out ambiguous questions and exact-identifier controls. Judge pooled real results independently. Measure relevant unique leads, recall within a judged pool, top-ten relevance, adapter calls, elapsed time and query drift. Use a same-call-budget alternative as well as the single-query baseline to distinguish better queries from simply doing more searches.

Confidence: medium for a useful experiment; unknown for actual uplift. This is the lowest implementation burden because the continuation seam exists.

### C2 — Bounded query-aware reranking as an optional view

Observation: `PresenceRanker` and `ReciprocalRankFusionRanker` in `slopsearx/merger.py:18` and `:86` use presence or feed order rather than query/content relevance. `_promote_jev_specialists` in `slopsearx/service.py:1366` already adds a specialist visibility rule, so a reranking evaluation must include that effective behavior. The retrieval-quality document explicitly rejects adding an LLM dependency to core ranking at this stage.

Proposal: first evaluate a caller-side permutation of at most 20 captured result cards. Preserve every supplied ID, URL and provenance; reject invented, duplicate or missing IDs. Keep mandatory tier ordering and compare against the actual specialist-promotion baseline. Treat result text as untrusted data. An optional versioned view could follow only if the experiment succeeds; distillation is a later cost-reduction option.

Experiment: hold candidate membership fixed and compare presence, RRF, current promotion and reranking using independently judged nDCG@10 and first-useful-result rank. Stratify general, code and science queries; shuffle input order to detect order sensitivity; measure invalid permutations, latency and cost. Include real acquisition coverage: EXP-004 was inconclusive because canonical targets were absent.

Confidence: medium for mechanism fit; unknown uplift. Reranking cannot recover missing candidates.

### C3 — Train specialist routing from observed retrieval value

Observation: `JevSpecialistRouter.route` in `slopsearx/jev.py:207` uses static routing cards and thresholded judgments. Cost/coverage routing already exists. EXP-013 improved offline routing but gave limited end-to-end acquisition gains; EXP-023's composition candidate failed its advance gate.

Proposal: use bounded query/engine response captures to build a training corpus, aggregate result relevance into engine utility, and test an offline-trained selector against the shipped router. Start with simple resource descriptions; ablate richer representations separately. Audit synthetic labels with independent human judgments and include marginal useful evidence beyond the general-search baseline. Any later runtime integration retains explicit scopes, capability/auth/health/cost eligibility, sensitive-engine policy and deterministic fallback.

Experiment: hold out query families and paraphrases. Measure relevant evidence actually visible in the top ten, essential-source misses, unnecessary adapter calls, and total routing-plus-search latency/cost. Compare at equal budgets; do not promote a candidate on routing F1 alone. Persist shared runtime state only through Valkey.

Confidence: medium for research fit, lower for near-term delivery due to corpus and training requirements.

## Research log, exclusions and preservation

Searches used web search restricted by query to arXiv: query2doc/query expansion; RankGPT/permutation reranking; federated resource selection. Then directly inspected the three retained arXiv abstracts and full HTML methods, results and limitations. Secondary search hits were excluded as evidence when the primary paper was accessible. *Resource Selection for Federated Search on the Web* (arXiv:1609.04556) was considered as historical context, but not retained as a separate recommendation because ReSLLM more directly addresses the existing model-assisted routing seam. Other query-expansion search hits were excluded as redundant for this bounded three-candidate scan.

Project evidence: graph report hubs; current merger, Jev router, research planner and specialist promotion; `docs/ADAPTIVE_RESEARCH.md`, `docs/COST_COVERAGE_ROUTING.md`, `docs/RETRIEVAL_QUALITY_EVALUATION.md`, and the current experiment ledger. Historical memory helped locate prior experiments; current checkout evidence governed the conclusions. No claim about a remembered experiment absent from this checkout is made.

Preservation inventory: this project document contains separate source records S1–S3, linked synthesis C1–C3, proposed measurements, limitations and search/exclusion decisions. Every retained paper is represented. Benchmark findings remain author-reported and were not reproduced. No essential finding is left only in chat.

Open questions: Will expansions help each upstream query parser? Are returned snippets sufficient for relevance ranking? Can independently audited routing labels predict incremental visible evidence? A licensed/reusable captured corpus and preregistered, query-level evaluation are prerequisites to answering these.

## Query-planning follow-up

The user specifically asked about improved query planning. Prioritize the following three planning experiments; they refine C1 and can initially operate through caller-directed plans rather than changing core execution.

### S4 / P1 — Decompose by evidence need

Source accessed 2026-10-06: Khot et al., *Decomposed Prompting: A Modular Approach for Solving Complex Tasks*, ICLR 2023, [arXiv:2210.02406v2](https://arxiv.org/abs/2210.02406v2). Abstract-level inspection: the paper decomposes tasks into modular subtasks and reports benefits for textual multi-hop QA, including incorporation of symbolic retrieval. Its results do not validate a SlopSearX planner.

Proposal: create distinct subquestions, identify which can be searched immediately and which require earlier facts, and assign each a source intent. Use existing `subquestions`, `initial_plan` and `subquestion_id`. Keep dependency scheduling in the caller initially; the current subquestion model is not a dependency-graph executor. Record brief evidence requirements and rationale, not private reasoning traces.

Evaluation: compare fixed strategy templates with decomposed plans at equal adapter-call budgets. Measure independently judged subquestion evidence coverage, omitted requirements, redundant searches and final answer support.

### S5 / P2 — Plan the next query from retrieved evidence

Source accessed 2026-10-06: Trivedi et al., *Interleaving Retrieval with Chain-of-Thought Reasoning for Knowledge-Intensive Multi-Step Questions*, ACL 2023, [arXiv:2212.10509v2](https://arxiv.org/abs/2212.10509v2). Abstract-level inspection: IRCoT alternates retrieval and reasoning because later retrieval needs depend on earlier findings; evaluated on HotpotQA, 2WikiMultihopQA, MuSiQue and IIRC. Author-reported benchmark gains are not reproduced here and do not establish performance across live heterogeneous engines.

Proposal: after each attempt, have the caller extract source-attributed entities, unresolved evidence needs and contradictions, then submit a targeted follow-up using `parent_attempt_id` and `subquestion_id`. Do not assume the model's intermediate answer is true. Stop when the caller's evidence requirements are met or durable budgets are exhausted. Example: identify a project's actual successor name before searching that successor's maintenance and compatibility.

Evaluation: compare an up-front decomposed plan with evidence-conditioned follow-ups at identical budgets. Measure successful dependent retrieval, unsupported entity propagation, final answer support, duplicate work and latency. This is the preferred first planning experiment because it tests a distinct failure mode and uses the existing continuation contract.

### P3 — Diversify query wording while preserving intent

Grounding: S1 (Query2doc). Generate concise variants using domain terminology while retaining the original query, exact identifiers and constraints. This targets vocabulary mismatch rather than creating additional evidence needs. Short-query extraction is an adaptation beyond the paper's direct pseudo-document concatenation.

Evaluation: compare expansion with both the original query and an equal-budget decomposition baseline. Score newly retrieved relevant evidence and query drift separately. Do not count more URLs as better evidence.

Follow-up search log: searched arXiv for interleaved retrieval and decomposition; retained S4 and S5 after opening primary abstracts. Least-to-Most Prompting and Successive Prompting were considered but not retained because S4 more directly includes retrieval and S5 addresses evidence-dependent follow-ups. These two sources were inspected only at abstract level; no full-method reproduction or latest-literature completeness claim is made. Source records and proposals are preserved here alongside S1–S3.

Portal impact review: no browser-visible contract changes in this research-only note. Future scope/ranking/view changes need portal contract/browser coverage and docs, plus truthful enforcement, canonical cache/view derivation and snapshot consistency. No implementation is authorized by this note.
