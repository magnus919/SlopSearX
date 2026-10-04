# EXP-039: Explicit-purpose source-selection development

## Registration status

Registered 2026-10-04 against main `63735511226e9f06a1fc7a27d24f7d1d05df15e9`, issue #516. The protocol and inputs are frozen before annotation or provider measurement. No Jev calls, Brave searches, page fetches, runtime edits, or deployments have occurred. Before dispatch, freeze both independent EXP-039 annotation sets and hashes and prepare/hash all request bodies. The shared Brave budget is in [evidence/EXP-039/search-budget.json](evidence/EXP-039/search-budget.json): 0 of 10 attempts used, reserved for a later untouched confirmation after candidate selection.

## Question

Does passing an explicit caller research purpose improve ordering of useful sources to read, beyond the current generic W0 scorer, on the same saved q1–q8 result cards? Separately, are any changes explained by richer task text or by a task-relative source-selection rubric? q9 is a previously exposed climate extension and will be reported separately, not pooled into the primary q1–q8 result.

EXP-038 did not support W2 for ordinary-search adoption. Its negative outcome remains immutable. EXP-039 does not revise or retest that result as a fresh holdout: it changes the target from answer-support relevance to reading-lead selection utility, adds constructed caller-purpose context, and requires new blinded two-axis annotations.

## Frozen inputs and limits

- Corpus: the same frozen EXP-038 first-40 projections for EXP-036 q1–q9. Keep each card's ID, title, URL, snippet, order, and bytes fixed across the four arms. q10 cardiac content is excluded. Do not fetch pages or search.
- Queries: original keyword strings and constructed task texts are frozen in [evidence/EXP-039/task-contexts.json](evidence/EXP-039/task-contexts.json). The task texts were composed from the saved query topics/facets after EXP-038; they are not original user utterances, user-intent labels, or fresh queries.
- Labels: use two new independent blinded annotations per card, with the frozen dual-axis protocol in [evidence/EXP-039/annotation-protocol.md](evidence/EXP-039/annotation-protocol.md), its machine-readable [annotation output schema](evidence/EXP-039/annotation-output-schema.json), and the predeclared vocabulary in [evidence/EXP-039/task-facets.json](evidence/EXP-039/task-facets.json). Reading-lead priority is primary; visible substantive detail is a separate diagnostic. Existing EXP-036/038 root and peer grades remain exposed exploratory evidence only. Do not reuse them as primary labels for the new task.
- Model: pinned `jev-1.13.0`. No model/provider change.
- Request shape: all four arms use identical shared state containing one `query` string and the full frozen ordered `candidates` array; each candidate gets one Score question keyed by candidate ID. Same candidate list, request structure, number of questions, candidate order, scoring scale (0–9), stable descending sort, and original-order tie behavior. No card-local transport, singleton requests, fetch Choice, probability threshold, shortlist change, or sibling comparison.
- Factor contracts and exact strings: [evidence/EXP-039/candidate-contracts.md](evidence/EXP-039/candidate-contracts.md). The authoritative baseline is [baseline-contract.json](evidence/EXP-039/baseline-contract.json), pinned to `63735511226e9f06a1fc7a27d24f7d1d05df15e9` and SHA-256 `df518bac899fa457e713201192a65f8ee0c3599419382e5be221d5ac366b479e`.

## Arms and estimands

| Arm | Shared-state query text | Per-candidate rubric |
|---|---|---|
| A | Original keyword query | Frozen generic W0 |
| B | Explicit caller-purpose task text | Frozen generic W0 |
| C | Original keyword query | Source-selection usefulness |
| D | Explicit caller-purpose task text | Source-selection usefulness |

The **primary contrast is D versus A** on q1–q8 using new reading-lead labels. Secondary, descriptive factorial contrasts are B−A (context under W0), D−C (context under source-selection scoring), C−A (rubric under keywords), D−B (rubric under explicit purpose), and the interaction (D−C)−(B−A). Axis B visible-evidence results, q9, and old exposed labels are diagnostic/extension results only. Do not select a winner from secondary contrasts.

## Primary analysis and gates

- Compute per-query paired nDCG@10 with gains (2^{grade}-1) from each independent annotator's 0–3 reading-lead labels. The query is the resampling unit. Report each reference set separately; do not pool annotators, average them into one reference, or promote inter-rater agreement to independent sample size.
- Primary q1–q8 mean D−A must be at least **+0.03 nDCG@10 under each reference**, and each 95% paired query-bootstrap interval (10,000 draws, seed 91939) must have lower bound **greater than 0**. This is a practical development qualification gate for an optional explicit-purpose mode, not an ordinary-search default threshold or proof of general quality.
- Guardrails: D may lose no more than one reading-lead grade ≥2 result in its top ten versus A on any primary query; no facet represented by an A top-ten grade ≥2 card may disappear from D's top ten; no primary query may have fewer than nine grade ≥2 results in D's top ten if A has ten. Report per-query deltas, paired grade changes, facet coverage, top-ten membership, and all disagreements. q9 must be reported separately with no replacement or compensation for primary failure.
- Repeat guard: repeat q1 arms A and D plus q7 arms A and D (four additional requests). Each repeated top ten must overlap its matching original arm by ≥0.8. These repeats are stability checks, not additional query units.
- Result interpretation: primary/gate failure means unsupported for selection; missing or invalid primary responses mean inconclusive, not successful. Passing this exposed development study only selects a candidate for a separate fresh confirmation and later review of whole-set coverage. It does not authorize a production default, runtime change, or implementation adoption.

## Operational bounds

- Baseline and issue: SlopSearX issue #516; source baseline commit `6373551`. Reconfirm exact source hash before preparing requests; no runtime edits are in scope.
- Planned runner commands (inert, no secrets/endpoints in tracked files): `python3 replay.py --selftest --root docs/experiments/evidence/EXP-039`, then `python3 replay.py --prepare --root docs/experiments/evidence/EXP-039`, `python3 replay.py --verify --root docs/experiments/evidence/EXP-039`, root-only `python3 replay.py --run --root docs/experiments/evidence/EXP-039 --transport-command-file PRIVATE_TRANSPORT_CONFIG`, and `python3 replay.py --analyze --root docs/experiments/evidence/EXP-039`. All selftests and frozen-label/hash checks must pass before run.

- Planned maximum: 40 serial Jev requests (9 queries × 4 arms = 36, plus q1 A/D and q7 A/D repeats); no retry, replacement, or query substitution. Preserve malformed, timeout, missing, and partial outcomes.
- Before dispatch, freeze every complete request body and its bytes/SHA-256 manifest. Record one durable attempt before each request. Stop on any protocol, request-size, response-size, timeout, or token-budget guard breach; do not retry.
- Preserve EXP-038 limits: complete saved pool of at most 38 cards per query, ≤4,096 query/task UTF-8 bytes, ≤256 title UTF-8 bytes, ≤512 URL UTF-8 bytes, ≤1,200 snippet UTF-8 bytes, ≤128,000 request bytes, ≤2,000,000 response bytes, ≤1,000 ms remote HTTP evaluation deadline excluding separately recorded transport startup. Do not truncate cards to meet a limit. Respect existing 2,000,000 input-token and 100,000 output-token ceilings. Report actual usage and latency; do not invent monetary totals.
- Validate responses with the qualified EXP-038 strict response parser and accepted provider envelope. The score-answer map must contain each expected candidate ID exactly once with no unexpected IDs; each answer must have the expected score type and a finite JSON number in [0,9] (reject booleans, NaN, and infinities), without coercion. Preserve accepted documented answer metadata such as confidence, legend, or probability fields and provider usage fields; do not require the entire provider envelope to contain only candidate IDs. Retain original card order for ties. No request is retried after an invalid response.
- Keep the shared ten-attempt Brave allowance reserved for future confirmation. Development uses saved public cards only and consumes zero Brave attempts.

## Annotation and limitations

The task texts deliberately clarify a source-discovery/read-list objective, but they were written after EXP-038 and from its exposed query topics/facets. The query cards, task texts, and benchmark families are therefore development material. Annotation is blinded to Jev arms, ranking outputs, original pool order, and the historical labels, but annotators cannot be blinded to the query/task itself. A reading-lead grade describes selection priority based on visible card evidence; it does not certify a source's claims, quality, authority, or unseen page contents. A separate visible-evidence axis prevents source-discovery usefulness from being confused with answer sufficiency. These labels remain reviewer judgments, not human-established ground truth.

The four-arm design holds transport, shared context shape, candidate ordering, model, score scale, and sort fixed. It can estimate context and rubric effects on these saved pools, but it does not establish whole-set (>40) behavior, fresh search performance, provider decontamination, product utility, or generalization to all query intents. Exact navigation and ordinary query handling are not the treatment objective in this study; no default path should be changed based on it.

## Before-dispatch qualification clarification

Recorded before the first EXP-039 provider attempt: 2,000,000 input tokens and 100,000 output tokens are **observed-usage qualification gates**, checked against provider-reported usage after every call. Preflight reserves are conservative estimates, not provider-enforced spend guarantees. A crossing or unknown usage stops further dispatch and retains the operational failure; no successful qualification may exceed those totals. The documented [TypeSafe request schema](https://docs.typesafe.ai/api) exposes state, model and questions; no per-call output-token cap was found in that reference. Do not invent an unsupported request parameter or claim a hard spend bound. The numerical gates, 40-call limit, request/response byte bounds, one-second remote deadline and primary comparison remain unchanged. User authorization for the inexpensive public-case Jev work remains applicable; the separate ten-attempt Brave ceiling is counted before dispatch and is untouched.

Independent offline qualification also identified descendant-process cleanup as a runner boundary to fix before calls. The local transport runs in its own process session; termination must clean up its owned process group and reserve cleanup time inside the configured wall allowance. This is an offline runner repair, with no candidate instruction, input, reference rubric or adoption-gate change.

## Timebox

Use a four-hour execution window for this development cycle. At the limit, preserve all partial artifacts and report incomplete status; do not extend, replace failed calls, or consume confirmation-search budget.

## Artifacts

- [Exact candidate contracts](evidence/EXP-039/candidate-contracts.md)
- [Source-pinned W0 baseline](evidence/EXP-039/baseline-contract.json)
- [Frozen task contexts](evidence/EXP-039/task-contexts.json)
- [Blinded annotation protocol](evidence/EXP-039/annotation-protocol.md)
- [Annotation output schema](evidence/EXP-039/annotation-output-schema.json)
- [Frozen facet vocabulary](evidence/EXP-039/task-facets.json)
- [Shared confirmation-attempt budget](evidence/EXP-039/search-budget.json)
- [EXP-039 delivery plan](evidence/EXP-039/delivery-plan.md)

## Outcome

Completed all 40 valid transactions. The explicit-purpose/source-selection combination passed both statistical comparisons but failed the registered useful-source identity-retention guard; it is not selected. See [readout](evidence/EXP-039/readout.md), [analysis](evidence/EXP-039/analysis.json), and preserved receipts. No Brave attempts were used.
