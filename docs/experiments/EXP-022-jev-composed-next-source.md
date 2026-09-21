# EXP-022: Decomposed Jev decisions for next-source advice

## Registration (before new Jev calls)

- State: registered 2026-09-21. Baseline: `1a84802` on `main`.
- Question: with exactly the same first-pass search state, does composing two
  atomic Jev judgments in code choose useful specialist follow-ups better than
  one compound Jev judgment? This tests the TypeSafe workflow pattern, not a
  product implementation. The general-search base is never replaced.
- Why retry: EXP-020's Noul asked conceptual fit but its primary outcome counted
  realized yield. The production router's question also blends retrieval fit,
  distinctiveness, and usefulness. This design separates those judgments and
  keeps live engine availability in deterministic code. Prior holdout cases are
  **exposed exploratory data**, never fresh confirmation.
- Fixed replay data: all 20 synthetic planner states in
  `evidence/EXP-016/selected-cases.json`, their pre-Jev gold labels in
  `labels.json`, and captured specialist outcomes. Use the frozen 12 calibration
  and eight holdout IDs; do not relabel or reassign them. Candidates are exactly
  `npm`, `openalex`, and `internetarchive`, because all three were acquired for
  every replay task. No paid or free search calls in the replay.
- Same-state comparator: one Noul per candidate asking whether that source
  would add the stated missing evidence beyond the first-pass cards, assuming
  it is available. Atomic candidate: two independent Nouls per source asking
  (A) whether the source's retrieval domain contains the kind of record needed
  and (B) whether a matching record would add evidence absent from the shown
  first-pass cards. All questions receive the same structured state (`task`,
  `missing_evidence`, first three general cards, general engine outcomes), and
  factual routing-card purpose/use-when fields are frozen in the harness.
  Existing EXP-020 scores and keyword rule are diagnostic comparators only.
- Composition, frozen before calls: score each source as
  `min(domain_fit_probability, incremental_evidence_probability)`. This is an
  AND-like rule; the score is *not* a calibrated probability of successful
  search. Select every source at or above one shared threshold, with no count
  cap. The compound comparator likewise selects every source above one shared
  threshold. Enumerate all distinct calibration scores, midpoints, and
  endpoints, choosing the threshold with maximal calibration realized net
  utility `(useful selected - useless selected) / queries`; tie-break by useful
  recall, fewer requests, then higher threshold. Freeze both thresholds and
  their complete curves before querying the eight holdout states.
- Primary replay outcome: paired difference in realized net utility per held-out
  query, atomic minus same-state compound. Unit of resampling is a query, not
  an engine; 5,000 paired query-bootstrap replicates, seed 42. Report the
  difference and 95% percentile interval, per-source confusion, missed useful
  sources, wasted dispatches, conceptual fit separately from actual yield,
  latency and token cost. The exploratory advancement gate is a point gain of
  at least +0.20/query with no additional useful-source miss and no policy
  violation; it is **not** a product-support gate. Failing it ends this trial
  without searching for a friendlier threshold.
- Fresh confirmation, only if the replay gate passes: register a separate
  fixed 24-task synthetic corpus and gold before any Jev calls, balanced across
  package, scholarship, history, and broad/no-specialist tasks. Acquire through
  `SearchService` using free general and the same three free specialists, at
  most 72 specialist attempts and 48 free general attempts; zero Brave and
  zero other paid searches. If free general engines provide no first-pass cards,
  stop and report inconclusive rather than purchasing replacements. Fresh
  confirmation requires a paired 95% interval lower bound above +0.20/query,
  no extra useful-source misses, and valid policy/no-key behavior. Fresh labels
  must be judged from bounded cards before Jev scores, with ambiguity recorded.
- Limits: at most 40 valid replay Jev requests (20 per arm) plus three HTTP 529
  retries total, 500,000 Jev input tokens, 20 minutes for replay. Alternate
  arm order by query ID parity. Save every provider attempt and successful row
  immediately, including failures. A fourth overload or invalid answer stops
  the run. Never log the API key. Use pinned `jev-1.13.0`. The keyless existing
  router is unchanged; this is an inert research harness, not application code.
- Run from the isolated experiment worktree using
  `/Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python docs/experiments/evidence/EXP-022/replay.py.txt --repo . --output docs/experiments/evidence/EXP-022/trial1 --registration-commit 78b73f9 --key-file /Volumes/tank01/magnus/git/SlopSearX/.env`.
  The key file is read but its contents must never be committed or printed.
- Outcome: `supported` is reserved for a completed *fresh* confirmation that
  passes the registered bound and guardrails. Otherwise `not-supported` if a
  completed comparison rules out the minimum effect, `inconclusive` for
  exposed-only or underpowered evidence, and `blocked` for inability to run.
  Record failures and corrections append-only. No product change is authorized.

## Readout

Outcome: **inconclusive for product value; the registered exploratory advance
gate failed**. The candidate does not advance to fresh confirmation. All 40
valid Jev requests completed in 40 provider attempts, with 41,894 input tokens
and zero searches (paid or free). Both arms saw the same saved first-pass cards
and source metadata. The calibration file was written before the holdout calls.

The 12 calibration cases selected `0.79` for the compound Noul and `0.72` for
the atomic `min(domain, incremental)` score. Both selected the same seven
sources: four actually useful, three not useful, and zero useful sources
missed. Calibrated realized net utility was `+0.0833/query` for both.

| Eight exposed holdout queries | Compound | Atomic composition |
| --- | ---: | ---: |
| Actually useful requests selected | 0 | 0 |
| Useless requests selected | 2 | 2 |
| Actually useful requests missed | 2 | 2 |
| Net utility/query | -0.25 | -0.25 |
| Median Jev latency | 182 ms | 174 ms |
| Jev p95 latency | 222 ms | 211 ms |

The paired difference is **0.00 utility/query**, with a query-bootstrap 95%
interval of `[0.00, 0.00]` because the two arms selected exactly the same
sources on every holdout query—not because uncertainty about wider use is
zero. Both selected Internet Archive on `h04` and `h05`, where archive results
were unavailable in the captured acquisition, while omitting the useful npm
follow-ups on `p04` and `p05`. Archive was conceptually appropriate in those
cases; availability is a separate deterministic gate. The `min` composition
lowered npm's held-out scores to 0.51/0.59, below its calibration-selected
0.72 cutoff. The compound scores were 0.68/0.78, below 0.79. This replay
therefore provides no observed evidence that the tested decomposition improves
the decision.

The previous EXP-020 single-question run selected one useful and one useless
specialist on these same eight cases (net 0.00/query), but that is diagnostic
only: it used a different prompt and threshold-selection run. The current
comparison isolates the two newly frozen question shapes on identical state.
The holdout had already been exposed in earlier research; labels came from one
adjudicator, only three specialist families were represented, and the
Internet Archive availability failure dominates realized value. It cannot
falsify all composed-workflow designs or support replacing the shipped router.
Under the preregistered stop rule, no new synthetic corpus, Brave search, free
search, or production modification was attempted.

Raw evidence: [harness](evidence/EXP-022/replay.py.txt),
[provider attempts](evidence/EXP-022/trial1/attempts.json),
[pre-holdout calibration](evidence/EXP-022/trial1/calibration.json),
[all scores](evidence/EXP-022/trial1/rows.json), and
[summary](evidence/EXP-022/trial1/summary.json). The incremental
[`rows-partial.json`](evidence/EXP-022/trial1/rows-partial.json) preserves the
write-as-you-go record. Registration was committed as `78b73f9`; frozen
harness and exact command as `5e6834f` before measurement.
