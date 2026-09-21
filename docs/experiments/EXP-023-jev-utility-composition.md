# EXP-023: Utility composition for pre-search specialist routing

## Registration (before measurement)

- State: registered 2026-09-21. Baseline: `b227a64` on `main`.
- Question: on the complete currently eligible specialist catalog, can Jev
  make *separate primary-evidence and secondary-evidence judgments* that code
  combines into a better dispatch decision than the shipped single-Noul,
  `0.65` router? This is distinct from EXP-022's failed post-search
  `min(domain, incremental)` composition. General engines remain the unchanged
  base; sensitive and ineligible specialists are excluded by code first.
- Data: the 30 previously labeled synthetic queries in EXP-014, with five
  each in packages, science/medical, security, structured references, jobs/ML,
  and broad/no-specialist. Freeze first two IDs in each family as 12
  development cases and the other three as 18 evaluation cases. These labels
  and queries are **publicly exposed prior research data**, so the evaluation
  split is diagnostic, not a genuinely blind confirmation.
- Eligibility: use the current built-in adapter registry and capability
  catalog as production does, with the current process environment; exclude
  broad/general, sensitive, disabled, unauthenticated-required, and circuit-
  open engines before asking Jev. A preflight found 32 eligible specialists
  and no missing gold-label engine. Save the exact list. The separate local
  TypeSafe key is used only for experiment calls; it is never supplied as an
  engine credential. No engine or Brave search call is allowed.
- Comparator A: reproduce the *exact production Noul instructions* in
  `slopsearx/jev.py` for each eligible specialist, select every score >=0.65.
  Compare to the ordinary keyless `ScopeResolver` specialist set as a secondary
  deterministic baseline. The primary comparison is A versus B on the same
  cases and catalog.
- Candidate B: two independently evaluated Nouls per specialist. Primary:
  would its original records be essential primary evidence for the query?
  Secondary: if not primary, could it still offer distinct useful secondary
  evidence beyond general web search? State is only the query, exactly as A.
  Let `p` and `s` be these Noul yes-values. Code computes the frozen utility
  proxy `U = 2p + (1-p)s - (1-p)(1-s)` and selects every eligible specialist
  with `U > 0`, with no count cap. This weights essential=+2, useful=+1,
  irrelevant=-1 in the same units as EXP-014's gold utility. It is a heuristic
  unless these conditional model values are empirically calibrated; it is not
  a probability that a search will succeed. No threshold or weight tuning is
  permitted after reading outputs.
- Primary metric: weighted gold utility per evaluation query, using
  `2 * selected essential + selected useful - selected irrelevant`, candidate
  minus shipped-rule comparator. Report 5,000 paired query-bootstrap 95%
  percentiles (seed 42), essential/useful recall, precision, wasted specialist
  requests, three broad-query abstentions, per-family effects, provider
  latency and tokens. The ordinary keyless router is a secondary comparison.
- Exploratory advancement gate: at least +0.25 weighted utility/query over A,
  no more than one extra missed essential engine, all three broad controls
  abstain, and no policy violation. Passing would only register a fresh
  confirmation experiment; this exposed corpus cannot support product change.
  If the gate fails, stop without tuning prompts or thresholds on evaluation
  data. Outcome is `inconclusive` for product adoption regardless of this
  exposed-corpus point estimate; record the narrower tested-candidate verdict.
- Bounds: 60 valid Jev requests (30 per arm), at most three HTTP 529 retries,
  500,000 input tokens, 20 minutes; alternate arm order by query parity.
  Persist all attempts and scores incrementally. Stop on unexpected cost,
  invalid response, or fourth overload. Pin `jev-1.13.0`; zero search calls,
  no personal queries, and no production code or configuration changes.
- Exact command will be frozen with the inert `.py.txt` harness in a signed
  commit before measurement. The key must be read from the ignored project
  `.env` without printing or copying its value to experiment evidence.

## Readout

Pending.
