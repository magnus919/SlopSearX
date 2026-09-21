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

Pending.
