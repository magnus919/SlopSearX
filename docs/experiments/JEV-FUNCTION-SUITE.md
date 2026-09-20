# Jev bounded function experiments (EXP-006 through EXP-010)

Status: **registered; not yet executed**

Registered: 2026-09-20

Parent research spike: [#389](https://github.com/magnus919/SlopSearX/issues/389)

These five experiments test distinct hypothetical SlopSearX functions in this
fixed sequence:

1. [EXP-006](EXP-006-jev-hard-query-fusion.md): hard-query ranking fusion;
2. [EXP-007](EXP-007-jev-robustness.md): calibration and adversarial robustness;
3. [EXP-008](EXP-008-jev-research-stopping.md): research sufficiency/stopping;
4. [EXP-009](EXP-009-jev-engine-plan-selection.md): engine-plan selection;
5. [EXP-010](EXP-010-jev-result-annotations.md): additive result annotations.

The exact fixture sets, prompts, decision rules, and implementation are frozen
in [the suite harness](evidence/JEV-FUNCTION-SUITE/harness.py.txt). Each
experiment runs once without retries. Later experiments are not modified in
response to earlier results.

Every call pins `jev-1.13.0`. A valid call is HTTP 200, reports that resolved
model, contains every requested finite Noul probability in `[0, 1]`, and
includes input-token usage. A failed validity or coverage prerequisite makes
that experiment inconclusive. Other failed gates produce `not-supported`.

This suite is research only. No result authorizes implementation, production
traffic, policy changes, or external transmission of user data. API keys,
authorization headers, raw responses, and live Brave snippets are never
retained. Costs use the registered `$0.042 / 1M input tokens` rate.

