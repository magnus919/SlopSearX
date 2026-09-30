# EXP-029: Guidance for recovery from an empty portal search

## Registration — 2026-09-30

- State: registered. Owner: Codex. Cycle start: 2026-09-30T13:01:09Z.
- Baseline: cb46b990a9df2bed5bd620808146db2efd5bd0c7.
- Observation: formatter.py empty-result body echoes the query and shows
  supplied suggestions, but without suggestions has no scope-recovery hint.
  PORTAL_UX_SPEC.md lists edit query or broaden scope as the empty-state action.
  This is a design opportunity, not evidence that users currently fail.
- Hypothesis: adding the text “Try editing your query or choosing more sources.”
  immediately after the empty-result explanation increases successful recovery
  within 120 seconds by >=20 percentage points, without policy or access errors.
- Candidate scope: that single hint on a genuine zero-result response with no
  suggestions and at least one successful source; not all-source failures,
  restricted scopes, strict-filter rejection, partial success or rate limiting.
  Retain existing controls, query, URL and source diagnostics.
- Primary metric: participant-level completion fraction difference, candidate
  minus baseline. Completion means finding the scenario's predefined relevant
  result after an empty initial response within 120 seconds, without facilitator
  assistance. Exceeding limit, abandoning or wrong result counts as failure.
- Minimum practical effect: +0.20 absolute, to justify a visible copy change
  intended to help recovery. Secondary time-to-completion cannot rescue failure.
- Offline evaluation: normal portal /search and shared SearchService with frozen
  local adapters/scenarios, no external results or paid/model calls. Both arms
  use identical source behavior. A result becomes available through the existing
  query/scope controls; the hint supplies no answer or special shortcut.
- Participant plan: 30 consenting representative users, each does one baseline
  and one candidate task on different matched scenarios. Counterbalance arm
  order and task mapping with seed 20260930. Include 15 desktop and 15 narrow
  viewport users; no developer/script actions counted as participant evidence.
- Evidence gate before UI implementation: a documented available participant
  cohort and approved participant-study scope, plus an observation/scoring plan
  with consent and no personal query capture. Recruitment, compensation and
  contacting people are outside this automated offline cycle. If unavailable,
  stop blocked, preserve protocol; do not substitute simulated human completion.
- Analysis: paired completion difference; 10,000 participant-cluster bootstrap
  replicates, seed 20260930, percentile 95% interval. Supported only when lower
  bound exceeds +0.20; pilot may be underpowered, in which case inconclusive.
  No multiple candidates, tuning, exclusions or post-hoc sample expansion.
- Guardrails: zero policy bypass, inaccurate source/filter claims, query loss,
  unsafe links, facilitator help scored as success, or user-data exposure;
  keyboard controls and status announcements retained. Both viewport strata
  must have nonnegative point differences. Portal contract/browser checks and
  visual review required for supported implementation, not proof of usability.
- Baseline completion rate is currently unknown; acceptance walkthroughs are
  not a measured participant baseline. Do not invent a numeric rate.
- Stop: audit tracked docs/tests/evidence for eligible study resources; absent
  resources stop before candidate rendering. Otherwise freeze exact scenarios
  and available authorized cohort in additive registration before implementation.
  One bounded cycle, 45 minutes, offline, zero spend. Record partial work if
  the budget cannot complete an authorized study; do not start live recruitment.
- Audit command: git ls-files docs tests; inspect participant/recovery/usability
  references in portal contracts and experiment records. Save exact audit
  commands, relevant excerpts and hashes in evidence/EXP-029/.
- Decision: supported if practical effect and all guards pass; complete valid
  data failing guards/threshold: not-supported; unresolved interval/incomplete
  comparison: inconclusive; absent prerequisite evidence/resources: blocked.
- Any implementation remains a reviewed PR, never automatic merge or deploy.
- EXP-025 remains blocked; EXP-026–028 complete. No unfinished runnable work.

## Readout — blocked

Registration 5420464 froze the plan before the prerequisite audit. The available
tracked portal evidence provides maintainer walkthroughs and scripted contracts,
not an available authorized participant cohort or measured recovery observations.
See [assessment](evidence/EXP-029/assessment.md) and
[reference audit](evidence/EXP-029/reference-audit.json).

Baseline rate, candidate rate, absolute effect and interval are **unmeasured**.
No comparison or guardrail performance result is claimed. The gate stopped this
cycle before candidate rendering or code changes; no recruitment, participant
contact, service calls, paid calls, tests, CI or pre-commit ran. No exclusions,
reruns or criteria changes. There is no candidate implementation to discard.

Persist this blocked protocol through the accompanying documentation-only PR.
Resume after the resources described in the assessment exist; avoid repeating
an identical audit on subsequent daily cycles. Scope-recovery guidance remains
a hypothesis, not a proven user benefit or a deployed behavior.
