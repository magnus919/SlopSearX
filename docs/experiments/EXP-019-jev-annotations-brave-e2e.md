# EXP-019: Jev source annotations on Brave-acquired cards

## Registration (before Brave or Jev calls)

- State: registered 2026-09-21. Baseline: `ae5cd4c`. This is a separately registered retry of [EXP-016](EXP-016-jev-source-annotations-e2e.md), whose free general engines were blocked on 20/20 queries. Preserve that failed trial and reuse its frozen [synthetic query set](evidence/EXP-016/query-set.json) and already captured specialist responses without making those calls again.
- Hypothesis and primary outcome: the same as EXP-016—calibration-selected Jev multi-label source annotations improve held-out micro-F1 by at least +0.10 over frozen deterministic URL/title/snippet rules on actual `SearchService` result cards. This retry changes only the general acquisition source to Brave and the associated cost bound.
- Acquisition: exactly one `BraveAdapter` search through `SearchService` per query, explicit engine scope, first five normalized cards retained, no retry. The `ENGINE_BRAVE_API_KEY` is read from the project `.env` but never printed or persisted. At most 20 new paid Brave searches; zero if key missing. Reuse EXP-016's 60 already captured specialist outcomes. Save every Brave outcome incrementally. No personal query or production write.
- Fixed card sampling: first two Brave cards plus the first card from the task's predefined specialist-focus engine if available; broad controls use the third Brave card. Empty slots stay empty. Adjudicate all selected cards by the six-label rubric in EXP-016, record `unknown` explicitly, and commit labels before any Jev call. Query is the bootstrap cluster. No Jev score may influence card selection or labeling.
- Baseline: frozen deterministic URL/title/snippet rules. Candidate: six batched Noul questions per card; choose one *global* threshold from observed calibration scores only by highest micro-F1, then precision, then higher threshold. Save threshold, full curve and calibration scores before holdout calls. No arbitrary count cap or fixed 0.5 threshold. Do not tune prompts on holdout.
- Primary estimator: held-out paired micro-F1 difference and 5,000 query-cluster bootstrap replicates (seed 42). `supported` for further product research only when interval lower bound > +0.10, at least 16/20 usable Brave queries and 12 judged holdout cards, zero canonical response mutation, and Jev p95 <= 1,000 ms. `not-supported` when interval upper bound < +0.10; otherwise `inconclusive`. Report per-label confusion and abstention/unknown coverage.
- Guardrails and stop: no sensitive engine, no ranking/filter/policy change; no-key/provider failure leaves canonical JSON identical and produces no advisory. Maximum 20 Brave requests, no retries, 120 Jev calls shared with EXP-020–021 plus three bounded HTTP-529 retries, 2 million Jev input tokens, 45 active minutes. Stop on secret exposure, unexpected expense, or budget breach. The expense limit is an acquisition budget, not a decision threshold.
- Execution: commit this registration first, then run the inert `.txt` Brave acquisition harness using `/Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python`; save raw evidence. Commit blinded card and source-utility labels. Then run Jev calibration and frozen holdout. No runtime code is changed; the experimental sidecar is not a feature release. Portal impact is none until a separate implementation decision.

## Readout

Outcome: **inconclusive under the frozen decision rule, with an observed
regression**. All 20 Brave calls and 99 shared Jev calls completed; 22 held-out
source cards were judged. The zero-cost deterministic baseline reached
micro-F1 0.9565 (11 TP, one FP, no FN). Jev at the calibration-selected global
threshold 0.79 reached 0.9000 (nine TP, no FP, two FN), a -0.0565 difference.
The paired query-bootstrap 95% interval was -0.1765 to +0.1667, crossing the
registered +0.10 minimum useful effect. Jev missed `primary_docs` on h05:B1
(score 0.69) and `tutorial` on b05:B2 (0.63). The baseline had one extra
`tutorial` label on h04:B1. There is no measured case for shipping Jev source
typing from this sample, and the taxonomy misses scholarly/registry types.

See the [consolidated readout](JEV-ADVISORY-E2E-RESULTS.md) and
[raw score summary](evidence/EXP-016/jev-summary.json). The calibration curve
was saved before holdout. No product code was changed.
