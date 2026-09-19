# Measured improvement loop

SlopSearX experiments improve useful search, human task completion, or agent
task completion while preserving the shared service and compatibility contracts.
The deliverable is evidence and a decision; a code change is optional.

## Run one cycle

1. Read this ledger, relevant project contracts, current issues and open PRs.
   Resume an unfinished experiment before proposing another. Do not duplicate
   rejected work without stating what new evidence makes a retry worthwhile.
2. Select one bounded hypothesis from an observed problem. Prefer the smallest
   change with a useful, affordable measurement. Check accepted design and
   discovery prerequisites before changing portal behavior.
3. Copy [TEMPLATE.md](TEMPLATE.md) to `EXP-NNN-short-name.md`, assign the next
   unused ID, and add it to the ledger. Complete and commit the registration
   **before implementing or measuring the candidate**. Record the baseline SHA,
   primary metric, minimum useful effect, guardrails, sample plan, stopping
   rule, resource limits, and exact commands. Check existing issues and create
   a focused issue before an implementation PR, per CONTRIBUTING.md.
4. Use an isolated branch/worktree from a recorded main SHA. Keep unrelated
   work intact. Compare baseline and candidate on identical inputs through
   the normal service/transport path appropriate to the claim. Pin fixtures,
   dependency versions, configuration, random seeds, and environment details.
5. Run the registered comparison and required regression checks. Preserve raw
   observations, failures, exclusions, commands, and environment metadata in
   `docs/experiments/evidence/EXP-NNN/` or a durable linked artifact with a
   checksum. Remove secrets and personal queries; retain only redistributable
   fixtures. Do not overwrite unsuccessful trials with successful reruns.
6. Append the readout and update the ledger. Use exactly one outcome:
   `supported`, `not-supported`, `inconclusive`, or `blocked`. Record unfinished
   work as `registered` or `running`, never as a completed result.
7. For `supported`, open a focused, ready-for-review implementation PR with the
   experiment, evidence, practical effect, limitations, tests, portal impact,
   and rollback. Follow DCO, pre-commit, CI, and review requirements. Opening
   a PR does not authorize merging, deployment, or changing production defaults.
8. For other outcomes, discard candidate implementation changes after retaining
   the readout and sufficient reproduction material (a small sanitized patch
   or durable candidate commit). Preserve the experiment on a documentation-only
   branch/PR even when the candidate is discarded. Label it explicitly as an
   experiment record, not an improvement. Never delete unrelated work.

## Documentation-only PR delivery

Automatically merge documentation-only experiment PRs, including the initial
process documentation and ledger updates for every outcome. Do not request code
review or run CI/pre-commit for these PRs. Before merging, inspect the changed
paths and diff to confirm they contain only documentation and inert experiment
evidence, with no executable code, configuration, dependencies, or workflows.
Use a DCO-signed documentation commit with `[skip ci]` to suppress workflows
that honor that marker. Do not request automated reviewer comments.

If supported implementation work remains under review, a separate documentation
PR can persist its findings immediately; record the implementation PR as pending,
not shipped. Verify the documentation PR actually merged and update the local
main checkout with a fast-forward when safe, preserving unrelated work.
If branch protection prevents merging without checks or review, report that
specific blocker; do not disable repository protections. Mixed documentation and
implementation PRs retain the normal review and CI requirements.

Experiment records are append-only history: corrections and subsequent trials
must identify the earlier result. Record persistence is required for every
outcome; failed implementation work must not disappear with its branch.

## Decide before seeing results

Each experiment has **one primary outcome** and a predeclared minimum useful
improvement. Secondary metrics explain results; they cannot rescue a failed
primary outcome. Predeclare guardrail tolerances for relevant query families,
errors, latency, cost, accessibility, and correctness. Policy, tenant isolation,
filter truthfulness, serialization, and API/MCP/portal compatibility must pass
regardless of the primary metric.

For noisy measurements, specify the independent sampling unit, sample-size
rationale, fixed sample/stopping rule, estimator, and uncertainty calculation.
Use paired inputs and randomized/interleaved baseline/candidate runs where
appropriate. A supported result needs its uncertainty bound to clear the
registered useful-effect threshold and all guardrails. If the budget cannot
resolve that effect, report `inconclusive`. Repeated timings of one query are
not independent users or queries. Do not stop when a result first looks good.
Account for testing multiple candidates; use fresh confirmation data for the
selected candidate. Do not tune on held-out evaluation inputs.

For exhaustive deterministic claims, report exact differences across the fixed
corpus and repeatability; statistical inference may be inapplicable. Limit the
claim to that corpus and behavior. Passing tests proves correctness, not a user
benefit. Synthetic relevance fixtures do not establish production relevance.
Automated browser actions measure scripted flows, not human usability. Human
usability claims require actual participant evidence. Agent task-success claims
require a frozen task set, scoring rubric, model/configuration, and repeated
runs; token reduction alone is insufficient if task completion regresses.

Use `not-supported` when the completed comparison fails the decision rule or a
guardrail. It means this candidate lacks support under these conditions, not
that the idea can never work. Missing data, underpowered comparisons, and
infrastructure failures are `inconclusive` or `blocked`, not negative evidence.

## Useful measurement seams

| Objective | Primary outcome examples | Existing starting point | Limits |
| --- | --- | --- | --- |
| Search usefulness | independently judged nDCG@K or task success | [Retrieval evaluation](../RETRIEVAL_QUALITY_EVALUATION.md), `tests/test_rank_fusion.py` | Current synthetic fixtures are not production-quality evidence. |
| Routing | correct scope selection across a fixed labeled set | `tests/test_routing_eval.py` | Scope accuracy alone does not establish relevant results. |
| Agent experience | task success, calls or tokens per successful task | `tests/test_mcp_harness.py`, `slopsearx/mcp/harness.py` | Deterministic fake engines test contracts, not live availability. |
| Human experience | completion rate, time to find a useful result | [Portal acceptance](../PORTAL_ACCEPTANCE.md) | Visual inspection and accessibility checks remain necessary. |
| Efficiency | latency distribution, CPU or memory per equivalent response | paired replay through shared service | Protect result equivalence; separate warm/cold cache and upstream variation. |

## Execution limits

Default to one active experiment and one candidate per cycle, offline replay,
no paid API calls, and a 45-minute execution budget. Register tighter or justified
alternative limits before running. Stop on guardrail breach or budget exhaustion,
record partial evidence, and resume only under an explicit updated plan. Live
traffic, production writes, new spend, or participant recruitment require scope
that the user has authorized. Never change the decision rule after seeing data;
a revised rule is a new experiment linked to its predecessor.

A recurring runner should read [RUNNER.md](RUNNER.md). The scheduler determines
cadence; this document does not itself start background work. If a previous
cycle is still active or its documentation is not persisted, resume it instead
of opening a competing experiment. Automatically merge documentation-only
experiment PRs as described below; do not automatically merge implementation
PRs or deploy changes.

## Ledger

| ID | Hypothesis / question | State | Decision / evidence | Implementation PR |
| --- | --- | --- | --- | --- |
| [EXP-001](EXP-001-results-only.md) | Can results-only MCP output save >=10% bytes without losing interpretation facts? | not-supported | Completed isolated retry: 6.97% smaller (target 10%); explicit engine outcome facts lost. All eight pairs completed. | None |
| Historical reference | Would RRF improve all three synthetic query families? | not-supported | [Existing evaluation](../RETRIEVAL_QUALITY_EVALUATION.md): two improve, science regresses. Retrospective reference, not a preregistered experiment or a new run. | None from this loop |

## Candidate backlog (not yet tested by this loop)

These are investigation leads, not accepted hypotheses or promised benefits.

- Identify repeated response-normalization work; test whether eliminating it
  reduces CPU/latency while preserving complete response semantics.
- Measure whether a bounded MCP presentation reduces tokens per successful
  agent task without reducing task completion or provenance usability.
- Investigate portal explanations of empty/partial results against the accepted
  UX specification; measure a defined recovery task before proposing changes.

Promote a lead into a registered experiment only after inspecting its real
production path and establishing a measurable baseline.
