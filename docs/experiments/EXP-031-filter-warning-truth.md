# EXP-031: Distinguish filter consumption from enforcement in warnings

## Registration — 2026-10-02

- State: registered. Owner: Codex. Start: 2026-10-02T13:00:20Z.
- Baseline: 26623e64c559c4db284124a941b821f7dcf02d19.
- Observed defect: _filter_warnings gates on unsupported enforcement, but says
  language/time_range is “not consumed by any adapter”. The adapter contract
  explicitly permits supported_filters consumption without audited enforcement.
- Hypothesis: replacing that phrase with “not enforced by selected adapters”
  restores truthful warnings across the permitted adapter declaration cases.
- Candidate: only the two warning phrases and explanatory docstring. Keep
  report, warning gating, dispatch, policy, serialization and results unchanged.
- Primary metric: fraction of emitted language/time_range warnings whose claim
  agrees with fixture declaration/observed parameter acceptance. Unsupported
  consumers contradict “not consumed”; unsupported nonconsumers do not. All
  scopes lack enforcement, so “not enforced” is truthful for both.
- Minimum useful effect: 100% truthful warning entries, >=50 percentage-point
  gain across the fixed balanced corpus. This is exact contract truthfulness,
  not evidence of human comprehension, real agent task success or live retrieval.
- Corpus: one-result Wikipedia and DuckDuckGo fixture adapters; both filters
  requested as language=en, time_range=month. Each adapter either declares
  and records consuming both filters, or declares neither. enforced_filters
  empty in all measured cases. Normal generic and targeted MCP tools with
  explicit same scope, targeted grant on, freshness prefer_fresh. Two arm-order
  repetitions per tool/mode, fresh process/server per pair: eight pairs,16 calls.
- The server transport, policy and shared service are unchanged; inject adapter
  metadata and parameter recording through the fixture runtime seam. Fixtures
  establish the allowed contract counterexample, not current built-in prevalence.
- Baseline expectation from source: 50% truthful warnings on this balanced set;
  measure it before candidate claims. No sampling/population CI for exhaustive
  deterministic cases. Two repeats check reproducibility, not independent users.
- Guards: full normalized response equality except exact intended warning phrase;
  enforcement entries remain unsupported, no fake assurance. Parameter dispatch
  unchanged, no error/policy bypass; <=5% normalized byte growth. Existing enforced
  and partial filter regression checks must pass before implementation promotion.
- Normalize only query_id, cursor, result_id (including embedded references),
  response_time_ms. Fixed inputs, no tuning/exclusions. Stop on guard breach.
- Commands: PYTHONPATH=. project .venv Python /private/tmp/exp031.py orchestrates
  fresh /private/tmp/exp031-worker.py subprocesses with 30-second pair timeout.
  Retain both scripts, raw outputs/errors and analysis in evidence/EXP-031.
- Budget 45 minutes, offline loopback only, no paid/upstream/model calls.
- Supported iff primary threshold and every guard pass; completed failure:
  not-supported; transport unavailable: blocked; incomplete evidence: inconclusive.
- Portal review: prose is MCP-specific; shared service/result/cache unchanged.
  Still run required portal contract gate for tools changes and ordinary checks.
- Supported implementation needs focused issue, regression tests and reviewed PR;
  docs/evidence may merge separately. No automatic code merge or deployment.
- No unfinished runnable experiment. Prior resource blockers remain unchanged.

## Measurement setup correction — before retry

Trial 1 completed all four generic pairs, then stopped at targeted nonconsumer
pair 1: baseline cached=false, candidate cached=true. The targeted signature
lacks freshness; the extra argument did not force a fresh call. The differing
cache state is an arm-order confound, not evidence of a candidate cache change.
Retain all trial-1 raw outputs and assertion failure. Do not score it as a full
comparison. Trial 2 empties all fixture InMemoryStore instances before each arm
so both tool paths start from identical cold stores. Candidate, corpus, warning
rubric, sample size, normalization, thresholds and stopping rules are unchanged.
Run all eight pairs afresh; preserve trial 2 separately. This is a corrected
fixture setup, not relaxed response equality or a removed guard.
