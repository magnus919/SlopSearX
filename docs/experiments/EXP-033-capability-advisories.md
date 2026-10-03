# EXP-033 — factual capability advisories

## Registration (before candidate work)

- Maintainer explicitly requested this one-off feature trial, 2026-10-03.
- Baseline: ad03963 (current main); issue #505.
- Intended benefit: disclose relevant configuration limitations and safe recovery
  ownership without asserting a counterfactual improvement in result quality.
- Candidate: bounded read-boundary metadata for relevant public sources that are
  disabled or lack required credentials; optional Jev-routing availability on
  automatic scopes only. No engine dispatch, policy, ranking or cache-key changes.
- Primary: correct advisory classification/ownership on a fixed request/config
  matrix. Useful threshold: 100% of eligible cases correctly disclosed, no false
  source attribution, and >=50 percentage-point increase over baseline visibility.
- Matrix: HTTP and MCP; requested public source absent credentials/disabled;
  category and configured-topic matches; unrelated source; sensitive source;
  enabled/credentialed control; automatic Jev unavailable/available controls;
  cached responses after availability changes. At least 12 cases, exhaustively
  scored against injected runtime declarations, not model preferences.
- Guardrails: no sensitive-source advisory, no source outside declared scope fit,
  operator-owned actions only; expected_quality_gain remains unmeasured; <=3
  advisories, stable sorting, no secrets or arbitrary query text in advisory fields;
  no extra adapter/Jev calls, results/enforcement/policy behavior unchanged.
- Relevant source is defined by explicit engines, explicit categories, media
  type, configured topic routing, or the actual general fallback family. No new
  inferred query classifier. Compute at read time, do not cache advisory text.
- Jev availability is factual runtime presence; do not infer which specialists
  a missing router would have selected or claim that a key improves quality.
- Sample is deterministic contracts; no population confidence interval. This
  does not measure agent task completion, human comprehension or live relevance.
  Behavioral benefit remains unmeasured and needs a separate frozen agent benchmark.
- Preserve matrix observations and regressions. Supported for truthful disclosure
  only if primary and all guardrails pass; otherwise not-supported/inconclusive/
  blocked as appropriate. Do not promote task-success claims from fixtures.
- Budget 45 minutes, offline replay, no paid/live model or provider calls.
- Required portal impact review, HTTP/MCP/cache contracts, full regression/CI,
  signed commits and review before merge under the standing maintainer rule.
