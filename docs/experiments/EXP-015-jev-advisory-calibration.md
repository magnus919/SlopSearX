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
- Execution: commit this registration first; run `python3 docs/experiments/evidence/EXP-015/harness.py.txt --key-file /Volumes/tank01/magnus/git/SlopSearX/.env`; retain fixture, row, summary, and harness evidence under `docs/experiments/evidence/EXP-015/`. The originally registered command omitted the `.txt` extension; this is a documentation typo, not a methodological change.
- Decision: report `supported` only for the research claim of measured holdout improvement with no safety violation; `not-supported` if completed comparisons do not improve; `inconclusive` for too-small/ambiguous estimates or invalid calls; `blocked` for inability to run. A product upgrade requires a separate decision and realistic evaluation.

## Readout

### Trial 1 (aborted before calibration)

The preregistered first run made ten valid annotation calls (`a01`–`a10`) and
then received HTTP 529 on `a11`. It exited before writing raw rows or thresholds;
those ten scores are irretrievable and excluded from all metrics. This is a
harness evidence-retention defect, not a negative model result. No holdout was
queried. Total observed provider requests: 11. No search-engine calls.

### Trial 2 registration (before restart)

Keep the same frozen fixtures, labels, split, question wording, threshold
selection, baselines, primary comparisons, and interpretation limits. Make only
transport/reproducibility changes: persist each successful call immediately,
record non-200 attempt status, wait 0.3 seconds between calls, and retry HTTP
529 at most three times across the entire restarted run. Trial 2 budget: 60
fixture calls plus at most three retry attempts, under two million input tokens
and 45 minutes. Both trials remain in the record; trial 1 is excluded from
calibration and holdout because its scores were not retained. The combined
request ceiling is 74 (11 plus 63). Stop on a fourth 529 or another invalid
response. Register this amendment before trial 2.

### Trial 2 results and decision

Trial 2 completed all 60 valid Jev calls (36 calibration, then 24 holdout)
without a retry. The frozen fixture SHA-256 is
`13084f267386def8cc2c13823abda745d808073fb3a3fd7d5c661f43cd358fcb`.
The run used 27,090 input tokens, estimated at about $0.00114 at the
published $0.042/million input-token price; this excludes the unlogged trial-1
usage. Median Jev call latency was 176 ms and nearest-rank p95 was 269 ms.
There were zero paid search calls and no production changes.

| Function | Held-out baseline micro-F1 | Calibrated per-label Jev micro-F1 | Difference | Paired case-bootstrap 95% interval for difference |
| --- | ---: | ---: | ---: | ---: |
| Multi-label source type | 0.625 | 0.952 | +0.327 | 0.000 to 0.630 |
| Advisory next source | 0.857 | 0.875 | +0.018 | -0.233 to 0.171 |
| Result-card triage | 0.769 | 0.909 | +0.140 | -0.083 to 0.455 |

Intervals resample the eight held-out cases within each function (5,000
replicates, seed 42). They are exploratory uncertainty summaries, not population
guarantees. All include zero; the sample is too small and intentionally
synthetic to certify product benefit.

The exact misses matter more than the aggregate score. In annotations,
`source_repository` scored 0.93 on a GitHub issue, below its calibration-derived
0.97 threshold; the overlapping `discussion` label was retained. In next-source
planning, Jev omitted an archive at 0.80 versus its 0.85 threshold and an
employer jobs source at 0.79 versus 0.85. In triage, it omitted an explicit
instruction attack at 0.89 versus 0.91. Per-label thresholds selected from only
two or three positive calibration examples are evidently brittle. Scores are
model outputs, not factual verification or observed correctness probabilities.

The preregistered lower-complexity *global-threshold diagnostic* was computed
from saved calibration scores, then applied unchanged to holdout. Its selected
cutoffs were 0.69 for annotations, 0.75 for next source, and 0.65 for quality.
Held-out micro-F1 was 1.000, 0.947, and 1.000 respectively. This does **not**
replace the registered per-label primary comparison or authorize a product
threshold: the analysis code was written after the primary readout, and the
tiny, hand-authored cases are easier than real search cards and research traces.
It does show that the high per-label cutoffs, not Jev's basic signal, caused
several misses. The full calibration curves are preserved for inspection.

Outcome: **inconclusive for product adoption**. Multi-label annotation signal
is strong enough to merit a realistic, independently labeled follow-up.
Next-source advice did not materially beat a cheap keyword baseline under the
registered primary rule. Quality triage is promising as an advisory hint, but
the missed attack rules out using it as a security gate. Do not ship any of
these from this trial alone. A subsequent test should freeze a single global
cutoff from a larger calibration corpus, use independent labels and realistic
result cards/traces, and test repeatability and the failure/no-key path before
considering opt-in implementation. Never use Jev to override existing policy
or claim page verification.

Evidence: [fixtures](evidence/EXP-015/fixtures.json),
[harness](evidence/EXP-015/harness.py.txt),
[calibration](evidence/EXP-015/calibration.json),
[all rows](evidence/EXP-015/rows.json),
[primary summary](evidence/EXP-015/summary.json),
[supplementary global-threshold curve](evidence/EXP-015/supplementary_global_threshold.json),
and [provider attempt statuses](evidence/EXP-015/trial2_attempts.json).
The trial-1 console record above is the only retained evidence from its aborted
run; it has no scores. Registration commits: `3910a96`, `473f145`.
