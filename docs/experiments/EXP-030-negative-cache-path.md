# EXP-030: Negative-cache lifetime robustness

## Registration — 2026-10-01

- State: registered. Owner: Codex. Start: 2026-10-01T13:01:44Z.
- Baseline: 058575daa037ed20737a9ca0ecfdf4f2149838ae.
- Observation: normal/partial cache TTLs use the positive-integer validator;
  SEARCH_CACHE_NEGATIVE_TTL_SECONDS uses bare int conversion. Zero/negative
  values can reach set_error, whose storage exceptions are caught.
- Hypothesis: positive-integer startup validation prevents all nonpositive
  negative-cache TTL misconfigurations on an active service error-cache path,
  preserving behavior for valid values. Intended benefit: avoid repeated failed
  search dispatch when an invalid lifetime silently prevents negative caching.
- Candidate: use existing _cache_ttl_setting for _negative_ttl. No new cache
  wiring, disabled-feature activation, policy or source changes.
- Primary metric: fraction of the fixed invalid settings {0, -1, -60} rejected
  before normal service dispatch, from 0 to 100% (minimum +100 percentage points).
  Require demonstrable error-cache writer reachability on the current normal
  application path; constructor-only assertions cannot establish service benefit.
- Guardrails: valid values {unset, 1, 60, 120} retain effective lifetime and
  service startup; decoded cache behavior, key identity and error semantics
  unchanged. No claimed production frequency or latency effect.
- Prerequisite: identify a current production caller of SearchCache.set_error
  from HTTP/MCP/shared SearchService before candidate work. If no writer exists,
  stop blocked: there is no normal-path mechanism for the intended benefit.
  Adding a new writer is a different hypothesis and outside this candidate.
- Frozen path audit: AST parse all tracked Python files under slopsearx and
  engines; inventory set_error definitions/calls and SearchCache construction.
  Save source hashes and exact audit code as inert evidence. Review call-site
  inventory for actual dispatch path; tests/fixtures are not production callers.
- If gate passes, register the exact offline error-response replay seam before
  implementation; use local adapters/store, fixed invalid/valid matrix twice,
  baseline/candidate alternating order. Exhaustive deterministic outcomes;
  no population CI, random model judgment, participant or paid/upstream calls.
- Stop on missing prerequisite or contract failure. Default budget 45 minutes.
  Supported only if normal-path evidence, primary threshold and guards pass.
  Complete failing comparison: not-supported; no path: blocked; insufficient
  observation: inconclusive. Baseline rate/effect remain unmeasured until replay.
- Evidence: docs/experiments/evidence/EXP-030. Audit command: python3
  /private/tmp/exp030-audit.py, retained there as audit.py.txt.
- Portal impact: startup/cache semantics affect all interfaces; any supported
  implementation needs ordinary portal and cache checks. No candidate code
  is written before the path gate, no automatic implementation merge/deploy.
- Earlier EXP-025/029 remain blocked with unchanged resources; completed
  serialization experiments will not be repeated on their existing corpus.

## Readout — blocked

Registration 5e31742 preceded the frozen AST audit. The tracked production
source contains one set_error definition and **zero calls or attribute
references** to it. SearchCache construction is reachable at service startup,
but the intended negative-write benefit has no identified current service path.
Normal service cache writes use cache.set; all-unresponsive writes are skipped.

The path prerequisite failed, so no constructor/candidate comparison was run.
Primary rejection rate, effect, statistical uncertainty, valid-setting guards
and real dispatch savings are **unmeasured**. This is absent mechanism evidence,
not proof that a validator could never be useful to an external consumer.
No candidate source changes, runtime replay, tests, paid calls or CI/pre-commit.
No retries, exclusions or criterion changes. No candidate implementation exists
to discard, and no implementation PR is justified.

[Audit](evidence/EXP-030/audit.json) and [reproduction](evidence/EXP-030/reproduce.md)
preserve the inventory and limitations. Persist through the documentation-only
PR. Resume only with new writer/failure evidence; do not activate a dormant
policy or tighten startup configuration solely to manufacture a measurable win.
