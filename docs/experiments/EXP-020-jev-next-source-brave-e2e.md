# EXP-020: Jev next-source advice after Brave-backed search

## Registration (before Brave or Jev calls)

- State: registered 2026-09-21. Baseline: `ae5cd4c`. Separately retries [EXP-017](EXP-017-jev-next-source-e2e.md) after DDG and Google returned only blocked outcomes. Shares EXP-019's 20 Brave searches, frozen query set and earlier captured npm/OpenAlex/Internet Archive specialist responses; no duplicate search calls.
- Hypothesis: after seeing each actual first-pass Brave response, Jev improves held-out mean net evidence utility per query by at least +0.20 over a frozen deterministic keyword planner. This is advice for an additional step; Brave stays in place and there is no arbitrary specialist-count cap.
- Inputs to Jev: synthetic task, missing-evidence requirement, first three Brave cards and Brave status. Independently score the three named specialists in one Jev call per query, with explicit family descriptions. Do not expose the specialist results or gold labels to Jev. A non-OK specialist is still reported as an acquisition failure, never as a useful result.
- Gold before Jev: using only the captured Brave top three and specialist top five, record whether each specialist adds a direct source for the missing requirement, with a one-sentence rationale and ambiguity exclusions. For historical tasks, a timed-out archive yields no *realized* value even if conceptually apt; report conceptual fit separately. Commit labels before Jev scores. Baseline keyword rules see the same task/general state, not specialist results.
- Calibration: enumerate observed Jev scores and midpoints on the 12 calibration queries; choose a single threshold maximizing mean realized net utility `+1 useful source selected -1 useless source selected` per query, breaking ties by useful-source recall, fewer requests, then higher threshold. Freeze and save it before the 8 holdout calls. No magic fixed cutoff and no holdout retuning.
- Primary metric: held-out paired mean realized net-utility difference, Jev minus keyword baseline, with 5,000 paired query-bootstrap replicates (seed 42). `supported` for further research only when interval lower bound > +0.20, at least 8 judged holdout queries, Jev p95 <= 1,000 ms, and all shared safety/cost guardrails. `not-supported` when interval upper bound < +0.20; otherwise `inconclusive`. Report useful recall/precision, wasted requests, availability and conceptual-fit discordance separately.
- Safety: no Jev selection can make an ineligible/sensitive engine eligible. This experiment only scores three non-sensitive free specialists. No-key/provider failure produces no additional advice and leaves the first-pass JSON byte-equivalent. At most 20 shared Brave calls, no search retries, and the shared Jev budget of EXP-019 apply. No production dispatch or portal change is authorized.

## Readout

Outcome: **inconclusive**. On eight held-out queries, keyword advice selected
six specialists (two useful, four useless), net utility -0.25/query. Jev at
its calibration-selected 0.86 threshold selected two (one useful, one useless),
net 0.00/query. The +0.25/query difference had a paired-bootstrap 95% interval
of -0.25 to +0.75, which does not clear the registered +0.20 useful-effect
bound. Jev missed the useful npm verification on p04 (0.84), and suggested
Internet Archive on h04 (0.88) despite its observed timeout. All five
historical archive requests failed; conceptual suitability and realized yield
must be kept separate. The prompt asked for conceptual fit while the primary
metric counted realized yield, limiting causal interpretation of the model's
score. This is not permission to dispatch an unhealthy or sensitive engine.

See the [consolidated readout](JEV-ADVISORY-E2E-RESULTS.md) and
[per-case scores](evidence/EXP-016/jev-rows.json). No product code was changed.
