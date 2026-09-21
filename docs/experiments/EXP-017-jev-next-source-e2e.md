# EXP-017: Jev next-source advice after an actual search

## Registration (freeze before measurement)

- State: registered 2026-09-21. Baseline: `ae5cd4c`. Follow-up to [EXP-015](EXP-015-jev-advisory-calibration.md). Shares the 20-query, no-Brave acquisition and fixed budget in [EXP-016](EXP-016-jev-source-annotations-e2e.md); this is a separate hypothesis with its own primary outcome.
- Hypothesis: after seeing the general search response, Jev selects useful follow-up families (`npm`, `openalex`, `internetarchive`) with at least +0.20 more net evidence utility per held-out query than a deterministic keyword router, without dispatching an irrelevant family at a higher rate.
- State shown to Jev: synthetic user task, missing-evidence requirement, first three general cards and general engine outcomes. It never sees the specialist acquisition results, gold labels, future cards, or holdout thresholds. All three candidate families are independently scored in one batched Noul call per query. This advises a *subsequent* research step; it never replaces or removes the general base.
- Gold: after capture, before Jev calls, label a specialist family `useful` only when its real top-five result contains direct evidence satisfying the recorded missing-evidence requirement that the first three general cards lack. Mark availability separately: an unavailable/rate-limited specialist is not a successful source, even if conceptually apt. Two reviewers are not available; use a strict written rubric and mark ambiguous cases unknown. Save case-level rationale and commit before Jev scoring.
- Baseline: frozen keyword rules using only task and missing-evidence text, not specialist outcomes. Candidate: one global Jev threshold chosen from the calibration queries by maximizing net utility `true useful selections - false selections` per query; tie-break to higher useful recall, fewer requests, then higher threshold. Save complete utility curve and frozen threshold before holdout calls. No arbitrary cap on how many of the three eligible families may be advised.
- Primary metric: held-out paired difference in mean net evidence utility per query against baseline. Also report useful recall, precision, wasted requests, missing useful sources, and realized result yield. Estimate a 5,000-replicate paired query bootstrap 95% interval (seed 42). `supported` requires interval lower bound above +0.20, at least 8 judged holdout queries, and guardrails; `not-supported` when upper bound below +0.20; otherwise `inconclusive`. This is a value criterion, not a preselected Jev score cutoff.
- Guardrails: no sensitive/ineligible candidate, no changes to first-pass search results or existing policy; no more than three candidate specialist requests because precisely three families are under study (not a general product engine cap). Jev failure/no key means no advice, not a failed search. Existing no-key default remains unchanged. Jev p95 <= 1,000 ms and shared budgets apply.
- Execution: follow EXP-016's frozen phases and shared evidence. A separate result file and readout will be linked here; no production candidate is authorized by this research.

## Readout

Outcome: **inconclusive (shared acquisition gate failed)**. All 40 general
DDG/Google attempts in EXP-016 were classified `blocked`. Jev had no first-pass
general response to inspect, so next-source advice, calibration, and holdout
were not run. The 60 specialist outcomes are preserved in
[EXP-016 acquisition evidence](evidence/EXP-016/acquisition.json), but they
cannot by themselves establish the value of an *incremental* next source. Zero
Jev and zero paid search calls in this trial. A separate registered retry is
required; do not repurpose these unavailable general results as negatives.
