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

## Readout — supported for contract truthfulness; implementation checks blocked

Corrected trial 2 completed eight pairs (16 MCP search calls). Truthful warning
entries improved **8/16 (50%) → 16/16 (100%)**, +50 percentage points. Every
other normalized response field matched, reports remained unsupported, dispatch
params matched, and normalized byte growth was **0.4928%**, below the 5% guard.
Two arm orders repeated exactly; no population confidence interval applies.
All 281 relevant filter/MCP/portal regression checks pass. The metric establishes
truthful prose on a fixed allowed adapter-contract corpus, not task success or
production prevalence. Built-in language adapters currently declare no consumption.

Trial 1's setup failure and correction are preserved, with no changed decision
rule. See [summary](evidence/EXP-031/trial2/summary.json),
[raw pairs](evidence/EXP-031/trial2/rows.json),
[reproduction](evidence/EXP-031/reproduce.md) and
[validation and blockers](evidence/EXP-031/validation.md).

Focused issue: [#488](https://github.com/magnus919/SlopSearX/issues/488).
Candidate retained in local signed commit f3e9faa7fd9b00a0ebefb4a83651adbfc9ce2c31
and inert patch. No implementation PR: a workflow-browser failure and isolated
hook type-check failures also reproduce on unchanged baseline source, so the
required-check promotion gate is not green. No unrelated fixes or deployment.
Installed-runtime type checking passes; this does not erase the hook failure.

Persist the supported finding through the accompanying docs-only PR now.
Resume delivery when baseline checks are resolved; do not rerun the experiment
or create a new hypothesis merely to bypass those gates.
