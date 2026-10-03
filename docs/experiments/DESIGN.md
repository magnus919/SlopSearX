# Material improvement loop

Scope decision: maintainer, 2026-10-03. This is a repository improvement
process. Operators own deployment, telemetry collection, SLO adoption and
production operations. [SLO.md](../SLO.md) offers optional objective definitions
and documented SLI surfaces. The runner does not configure or send alerts,
operate production SLOs, or require production monitoring before improving code.

## Start with a consequential problem

Resume supported work awaiting delivery before selecting another experiment.
Check current main and pending PRs so an already-fixed blocker does not strand a
useful candidate. Keep measurements frozen; refresh compatibility checks when
the base changes. Delivery includes current-SHA review, CI, merge verification
and the ledger. Deployment is an operator responsibility.

For new work, write the affected user/agent task and an observable before/after
example. Draw candidates from reproducible defects, supplied user/operator
evidence, issues, real service-path bottlenecks and capability gaps. Optional
operator-provided SLI data can inform this choice; its absence is not a blocker.
Rank opportunities by severity, affected tasks/coverage, evidence strength,
likely useful effect and feasibility of an affordable decisive comparison. Do
not give invented numerical impact scores or prioritize tiny edits merely
because their tests are easy. Pick the strongest bounded, measurable candidate.

Before registering, trace the normal HTTP/MCP/portal path and confirm the proposed
mechanism actually runs there. Verify dependencies, evaluation inputs, scoring
and baseline acquisition are available. If not, address the smallest concrete
measurement prerequisite with its own acceptance criterion or record the missing
input. Do not repeatedly run an unchanged blocked experiment or relabel fake
searches as production evidence. Never invent a candidate to fill a daily slot.

## Match evidence to the intended improvement

| Change | Decisive evidence | Claim boundary |
| --- | --- | --- |
| Correctness/recovery | Reproduce a real contract defect through the normal service path; compare fixed failure corpus and healthy controls | Establishes elimination of that defect; production frequency and availability uplift remain unknown without supplied observations |
| Retrieval/routing quality | Fixed representative tasks, independently scored relevance or successful evidence acquisition, family guardrails, held-out confirmation | Routing labels, result counts and synthetic rank fixtures alone are insufficient |
| Agent experience | Frozen task set, scoring rubric, model/configuration and repeated paired task runs; cost/calls per successful task | Metadata correctness or token savings alone do not prove task-success uplift |
| Portal experience | Representative user task with participant evidence for usability claims, plus deterministic contract/browser checks | Scripted browser completion proves the scripted flow, not human usability |
| Efficiency | Paired normal-path replay with equivalent results; warm/cold cohorts and useful absolute as well as relative thresholds | Microbenchmarks of unreachable helpers are not service improvements |
| Measurement prerequisite | A reachable, usable SLI or evaluation surface with a specific coverage/validity criterion | Infrastructure capability is a prerequisite, not an achieved SLO or quality gain |

Register one primary metric, a practical minimum effect justified by the problem,
guardrails, baseline, sample/stopping plan and cost ceiling before candidate work.
Separate operator reports from independent measurements. State what evidence
would disprove the idea. Never lower the useful-effect threshold after seeing a
result. Use fixed exhaustive cases for deterministic claims; use an appropriate
independent sampling unit and uncertainty for noisy task/performance claims.

## Prove, deliver, then account for the outcome

Run one experiment per scheduled cycle, once per America/New_York calendar day
at approximately 09:00. Deduplicate by start date, not finish time or a rolling
24-hour cutoff. Skip an overlapping cycle; do not catch up missed days. Default
to a 45-minute budget, offline replay and zero paid calls. Record actual cost,
requests and elapsed time where available; do not estimate subscription tokens
from wall time. User-authorized one-offs remain possible.

A supported result must clear its registered practical threshold and every
immutable guardrail. Preserve failed trials and exclusions, then open a focused
implementation PR. Follow the standing CI/Droid-or-runner review merge rule;
resolve review findings and verify delivery instead of abandoning supported work
while the next day's runner starts another experiment. A pending PR remains
pending, never counted as delivered. Preserve other outcomes in automatically
merged documentation-only PRs before discarding their implementations.

Readouts distinguish: measured effect and scope, decision, merged implementation,
and operator release status (unknown unless independently verified). A truthful
metadata correction can be supported as a contract improvement while agent
task benefit remains unmeasured. Do not promise quality uplift from that result.

At each cycle, review the most recent seven completed experiments and their
verified delivery status. Summarize supported changes actually merged and their
measured effects, remaining supported candidates, prerequisite-only work, weak
changes rejected and recurring blockers. There is no quota of successful trials:
if cycles repeatedly yield only inconclusive or prerequisite outcomes, improve
candidate selection/evaluation before another speculative implementation.

The ledger preserves learning. The loop's product outcome is useful, evidenced
changes delivered to the repository, with production adoption left to operators.
