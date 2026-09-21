# EXP-016: Jev source annotations on acquired search cards

## Registration (freeze before measurement)

- State: registered 2026-09-21. Baseline: `ae5cd4c` on `main`. Follow-up to [EXP-015](EXP-015-jev-advisory-calibration.md), whose synthetic holdout was encouraging but too small and did not run the search pipeline.
- Hypothesis: on cards actually emitted by `SearchService`, a calibration-selected, single global Jev threshold improves multi-label source-type micro-F1 by at least 0.10 over a deterministic URL/title/snippet rule, with no response or policy mutation.
- Beneficiary: agents and portal users who need descriptive source-type hints, not authority or factual verification.
- Shared acquisition: the frozen 20 synthetic queries in `evidence/EXP-016/query-set.json` (12 calibration, 8 holdout; five each of package, scholarly, historical, and broad tasks). For each query, dispatch the actual free `duckduckgo` and `google` adapters via `SearchService` with explicit scope, then `npm`, `openalex`, and `internetarchive` with the prewritten specialist query strings. No Brave or other paid search calls. Preserve classified engine outcomes, returned count, response latency, and bounded title/URL/snippet fields. The search pipeline and formatter are production code; the advisory is an experiment-only sidecar. Every search is counted, including failures.
- Card sample: first two general result cards plus the first result from the task's intended specialist family, if available. Broad controls use the third general card. If a card slot is empty, record it; never replace it based on its apparent relevance. This fixed sampling avoids selection after seeing Jev scores. Card is the classification unit; query is the bootstrap cluster.
- Gold: before any Jev scoring, apply a written source-type rubric to saved title/URL/snippet and commit `labels.json`. Labels may overlap: `primary_docs`, `source_repository`, `standard_regulatory`, `tutorial`, `discussion`, `vendor_marketing`. Mark genuinely unjudgeable cards `unknown` and exclude them from F1, reporting coverage. Adjudicate without Jev scores. URL identity alone does not prove the card's claim.
- Baseline: deterministic URL and text rules frozen in the evidence harness before Jev calls. Candidate: one Jev request per card with all six Noul questions batched. Calibration enumerates unique observed scores plus adjacent midpoints and endpoints; select one global threshold maximizing micro-F1 on calibration cards, breaking ties by precision then higher threshold. Save this threshold and its curve before issuing any holdout Jev call. No engine-count or annotation-count cap. Scores are not empirically calibrated correctness probabilities.
- Primary metric: paired held-out micro-F1 difference, Jev minus baseline, across all judged labels. Report precision, recall, per-label confusion, coverage, and a 5,000-replicate paired query-cluster bootstrap 95% interval (seed 42). Claim `supported` only if the interval lower bound exceeds +0.10 and all safety guardrails pass; otherwise `not-supported` if the interval upper bound is below +0.10, or `inconclusive` if it straddles +0.10 or usable sample is too small. This is a minimum useful effect, not a score cutoff.
- Guardrails: at least 16/20 queries return general cards and at least 12 holdout cards are judged; no result order/count/URL/title/content change; no engine routing change from adding the sidecar; no sensitive engine dispatch; Jev p95 <= 1,000 ms; Jev input <= 2 million tokens; no key or provider error produces no annotations and byte-equivalent canonical JSON. These guardrails only support further research, never automatic shipping.
- Stop/budget: at most 40 free general-engine attempts, 60 free specialist attempts, 120 Jev calls total across EXP-016–018 plus three HTTP-529 retries, 2 million Jev input tokens, 45 minutes of active execution. No retries of search requests. Stop on key exposure, unexpected paid API use, or budget breach. Preserve partial evidence; no silent restart.
- Execution phases: (1) commit this registration and corpus; (2) run `/Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python docs/experiments/evidence/EXP-016/acquire.py.txt --output docs/experiments/evidence/EXP-016/acquisition.json`; (3) label and commit acquisition-derived gold without Jev calls; (4) run the same `.venv/bin/python` on `docs/experiments/evidence/EXP-016/evaluate.py.txt --key-file /Volumes/tank01/magnus/git/SlopSearX/.env`; (5) append readout and run relevant deterministic contract checks. All harness files have `.txt` suffix and remain inert documentation evidence.
- Portal impact: none now; no production or browser-visible contract change. Any later product candidate requires portal/API/MCP design and tests, plus a separate implementation decision.

## Readout

Pending. Pre-measurement registration correction: the initial `pypi` candidate
was replaced with `npm` and the five package tasks were rewritten to npm
packages. The PyPI adapter's fallback fetches the entire simple index for
non-package queries and its card does not expose dependencies; npm's bounded
search API is suitable for all three candidate-family comparisons. No search or
Jev call preceded this correction. The revised query corpus and registrations
are frozen in a second commit before acquisition.
The shell's global `python3` lacks the project's `structlog` dependency; the
registered execution commands now use the existing project virtualenv. This
was found by an import check, not a search or Jev measurement.
