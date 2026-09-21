# EXP-015: Calibrate three additive Jev advisory functions

## Registration (frozen before measurement)

- State: registered.
- Date: 2026-09-21. Baseline: `b4ee77b` on `main`.
- Scope: research only; no search engines, production code, ranking, policy, or user data.
- Question: can Jev improve (A) multi-label source-type annotations, (B) next-source advice after a partial research result, and (C) result-card quality triage, without mistaking scores for facts or permissions?
- Existing evidence: EXP-010's single-label annotation rule confused overlapping primary-doc and standard types; EXP-009 tested initial plans, not next-step advice; EXP-007 tested relevance robustness, not explicit answer-likelihood and instruction-attack flags.
- Inputs: 60 synthetic, hand-labeled cases: 20 per function, 12 calibration and 8 held out. Each case is one independent Jev call with all applicable Noul questions batched. IDs, labels, and split are fixed before inference. No real result card, personal query, Brave call, or other search call. Source URLs in synthetic cards are illustrative, not fetched or verified.
- Baselines: (A) exclusive highest-label source typing at 0.5 with 0.10 lead, mirroring EXP-010; (B) deterministic family advice from task keywords, without interpreting prior results; (C) keyword rules for answer-likelihood and prompt-like instruction detection. Compare on identical holdout fixtures.
- Calibration: for each binary label, enumerate all unique observed Jev scores plus endpoints on the 12 calibration cases; choose the threshold maximizing case-level F1, breaking ties toward higher precision and then the higher threshold. For annotation, also show a single global threshold as a lower-complexity alternative. Freeze all thresholds before scoring the 8 holdout cases. Scores are not calibrated probabilities. Report full calibration precision/recall trade-offs and held-out confusion, F1, baseline delta, latency, and token usage; no arbitrary pass cutoff.
- Primary comparison: held-out micro-F1 of Jev-derived labels versus baseline, separately for A, B, and C. Diagnostic: precision, recall, per-label errors, calibration-to-holdout drift, and estimated usage cost. With only 8 held-out cases per function, do not claim product readiness or population-level significance. Do not reselect thresholds on holdout.
- Safety: advice and annotations are additive only. No false claim of authority, page verification, filter enforcement, factual correctness, policy eligibility, or permission to dispatch sensitive engines. A Jev error means no advisory label; core search remains unchanged.
- Stop rule: one call per fixture, at most 60 Jev calls plus three transport retries, 2 million input tokens, 45 minutes, and no paid search calls. Stop on secret exposure, unexpected charges, or repeated invalid provider responses. The user authorized Jev usage and synthetic testing.
- Execution: commit this registration first; run `python3 docs/experiments/evidence/EXP-015/harness.py --key-file /Volumes/tank01/magnus/git/SlopSearX/.env`; retain fixture, row, summary, and harness evidence under `docs/experiments/evidence/EXP-015/`.
- Decision: report `supported` only for the research claim of measured holdout improvement with no safety violation; `not-supported` if completed comparisons do not improve; `inconclusive` for too-small/ambiguous estimates or invalid calls; `blocked` for inability to run. A product upgrade requires a separate decision and realistic evaluation.

## Readout

Pending.
