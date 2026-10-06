# Query-planning pilot protocol — EXP-079, EXP-080, EXP-081

Registered 2026-10-06 before candidate plans or evaluation retrieval. User explicitly requested an experiment on each of three planning approaches; this authorizes this one-off three-experiment sequence rather than the recurring one-per-day runner. Other unfinished experiments and their cohorts remain untouched.

## Scope and hypothesis

Baseline code is main `17727a71daf2116e2337f293c9261361140eeb52`. One source, OpenAlex, fixed for every arm. Task-neutral availability qualification through SearchService returned three OpenAlex results; arXiv timed out. Neither qualification response is evaluation evidence. No paid search, Jev or generative provider calls, deployment or runtime edits.

Each candidate should increase mean predeclared target recovery by at least 0.15 absolute against a fixed query-text baseline under a three-request ceiling. This measures recovery of known papers as reading leads, not answer support, paper quality or complete recall.

- EXP-079: split the question into two distinct evidence needs; retain original plus two concise facet queries.
- EXP-080: retain original, then use actual returned cards to choose up to two evidence-conditioned follow-ups. Each later query must cite an earlier receipt and a missing evidence need; any new named method must be attributed to an observed card or explicitly marked caller prior knowledge. No silent inferred facts. Query selection occurs before viewing scores and is frozen before dispatch.
- EXP-081: retain original, generate a short hypothetical terminology passage, derive two concise variants that preserve both requested facets. Pseudo-text is never evidence.

The interactive Codex caller authors candidate queries, not a deployed planner. Exact model revision, temperature and seed are not supplied to the harness; preserve actual query bytes and short decision rationales instead. No hidden reasoning traces are collected. Questions and references were constructed by the same caller, so these are exposed, single-trajectory development pilots. No untouched-confirmation or general agent-performance claim is possible.

## Baseline and controlled inputs

Use the shipped `plan_research_queries` first three `counterevidence` query texts: original, original + limitations, original + criticism. Pin source scope to OpenAlex to isolate query-text planning from routing. This is an ablation of an existing fixed-text strategy, not a comparison against unrestricted multi-engine default research. Keep no cache, no Jev, presence ranking, 20 upstream results, first 10 visible cards per request, no date/language/SafeSearch constraint. All arms retain the identical first-pass acquisition for fair evidence starting conditions. Baseline acquires its remaining two queries fresh; candidates acquire their follow-ups fresh. Shared first-pass receipts are charged logically to every arm but only dispatched once.

Execute sequentially. EXP-079 interleaves baseline/candidate later requests using fixed PRNG seed 20261006; EXP-080 and EXP-081 reuse the frozen baseline and first-pass inputs later in the same run. Record timestamps and this temporal confound. No retries, substitution, rescue search or edits to registered references/matchers. Each task remains in the denominator even if empty or unavailable. Exact-title controls preserve the original and may stop after it. Do not exclude unsuccessful cases.

## Dataset, measure and decision

`cases.json`: 12 deliberate compound discovery tasks, four in each of three families, plus three exact-title controls. Two predeclared bibliographic targets per compound task. No training/development tuning or held-out selection; all cases are exposed development. Sample size covers three failure families within the request cap; it is not powered for production adoption. Result identity is normalized exact title (casefold, alphanumeric tokenization, punctuation/whitespace ignored), with no fuzzy matching or post-outcome aliases. Same-title duplicates count once. This matches bibliographic reading leads, not independently verified full-text evidence.

Primary: per-task target recovery = distinct target titles recovered in union of first-ten cards of up to three requests / target count; mean over 12 compound tasks. Secondary: both-target task completion, per-family recall, new target leads after first pass, repeated-result share, requests, service elapsed time and caller-authored query characteristics. Controls excluded from primary and retained separately. Report all target match receipts.

Paired query bootstrap, 50,000 samples, seed 20261006, central 98.3333% intervals for each candidate effect (Bonferroni three-comparison adjustment). This describes resampling sensitivity within this challenge corpus only. Supported requires lower bound >=0.15, at least 90% of issued requests status ok in BOTH compared arms, no family mean loss greater than 0.05, zero exact-title-control loss, and all hard invariants. If acquisition or planner data incomplete: inconclusive (blocked only if no comparison can execute). Otherwise failing gates: not-supported. Even supported means fixed-corpus lead-recovery support, never production usefulness validation. Fresh independent confirmation is mandatory before adopting a planner.

## Hard invariants and budgets

Only `openalex`; pre-dispatch shared `_enforce_policy`; no sensitive engines, private data or credentials; max three logical requests per arm/task; max 500 characters/query; identical first-ten view; stable serialization round-trip. Retain adapter errors as operational failures, not negative relevance judgments. Policy rejection or source/identity violation stops that experiment without further calls. At most 135 fresh OpenAlex requests across acquisition phases (45 baseline plus 30 each candidate), plus two prior task-neutral adapter probes. No retries. Pace requests at least 1 second apart; per-request timeout 15 seconds; total live-stage clock at most 3600 seconds. Each experiment at most 45 minutes including its planner and acquisition; shared qualification excluded. Caller/tool latency preserved where measurable, otherwise unavailable; do not infer end-to-end performance or monetary cost from service timing.

## Execution and evidence

After registration commit, create inert `harness.py.txt`. Run with project venv, source root in PYTHONPATH. Commands:

1. `python /tmp/query-planning-harness.py qualify --root .`
2. `python /tmp/query-planning-harness.py acquire --root . --phase first`
3. `python /tmp/query-planning-harness.py acquire --root . --phase decomposition`
4. Freeze adaptive round 2 queries based on first-pass cards; run `acquire --phase adaptive2`.
5. Freeze adaptive round 3 queries based on rounds 1/2; run `acquire --phase adaptive3`.
6. `python /tmp/query-planning-harness.py acquire --root . --phase expansion`
7. `python /tmp/query-planning-harness.py analyze --root .`

Qualify mechanics offline with deterministic transport fixtures before live evaluation, including budget, title matcher, policy rejection, serialization and empty/failure treatment. Qualification does not measure usefulness. Files: append-only `receipts.jsonl`, `plans-*.json`, qualification/environment/analysis JSON, readout, source hashes and SHA256SUMS in this shared evidence directory. Minimize responses to public bibliographic metadata and bounded snippets (OpenAlex metadata available under CC0), never headers/credentials. Preserve unsuccessful operations; resume skips only receipts already written, never dispatches a second attempt.

Portal impact: no product code or browser-visible behavior changes; no portal gate required for inert evidence. Any later implementation needs portal and research/MCP contract coverage. Commit experiment registration with DCO before candidate work; documentation-only evidence PR, inspect scope, signed skip-ci commit, automatic documentation merge under repository rules. No runtime adoption in this study.
