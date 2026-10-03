# Instructions for one improvement cycle

Read AGENTS.md, CONTRIBUTING.md, docs/experiments/README.md, the experiment
ledger, DESIGN.md and relevant project contracts. Inspect current main, open issues and
PRs, and unfinished experiments before choosing work. Preserve unrelated files.

Finish delivery of supported candidates first. For new work, identify a
consequential user/agent problem and an observable before/after example. Rank
opportunities by severity, affected tasks, evidence, useful effect and feasible
measurement; trace the reachable normal service path before selecting one.
Review the latest seven completed experiments and verified delivery outcomes.
Avoid repeating an unchanged blocker or doing tiny changes only because they
are easy to measure. If evaluation prerequisites are missing, address a bounded
prerequisite explicitly; do not claim it as product-quality uplift. Register a falsifiable
hypothesis using TEMPLATE.md and commit the frozen plan before candidate work.
Run one bounded experiment in an isolated worktree using the registered metric,
minimum useful effect, guardrails, sample plan, and stopping rule. Default to
45 minutes, offline inputs, and no paid calls. If valid measurement requires
unavailable data or infrastructure, record the blocker instead of inventing proof.

Preserve reproducible evidence and a readout for every outcome in the repository
experiment log. Open a ready-for-review implementation PR only for a supported
result with all required checks passing. Persist unsuccessful, inconclusive,
and blocked experiments through a documentation-only PR; discard their candidate
implementation only after retaining evidence and reproduction material. Follow
repository sign-off requirements. Automatically merge documentation-only
experiment PRs, including initial process documentation and all ledger updates,
without requesting code review or running CI/pre-commit. Confirm the diff contains
only documentation and inert evidence; use `[skip ci]` in the signed commit.
If necessary, persist supported findings in a separate documentation PR while
implementation review is pending. Verify the merge and safely fast-forward local
main. Report branch-protection blockers without disabling protections. Mixed or implementation PRs retain normal checks and the standing merge
rule below. Do not deploy automatically. Report the
experiment ID, measured effect and uncertainty, guardrails, decision, evidence
location, PR links, and unresolved blockers. Do not describe work as running in
the background unless a scheduler has actually been configured.

## Operator boundary

SLOs are documentation suggestions and SLI surfaces are optional operator tools.
Do not operationalize production SLOs, configure or send alerts, deploy changes,
or treat unavailable production telemetry as a prerequisite for repository work.
Use supplied operator evidence when available and label its origin. Report
measured effect separately from merged delivery and unverified release status.
The schedule stays daily; deduplicate using the Eastern calendar start date,
never finish time or rolling 24-hour elapsed time. Default to one bounded
experiment, 45 minutes, offline replay, zero paid calls.

## Standing implementation merge rule — authorized 2026-10-03

Implementation or mixed PRs authored under `magnus919` are authorized for
merge once all applicable CI checks pass and Droid supplies a positive substantive
code review. A green review job without usable review text is insufficient.
If Droid is unavailable, fails or supplies no usable review, the runner must
perform and record its own substantive review of the current candidate. Fix
findings and rerun affected checks before merging. No human self-approval is
required. Verify the PR author, candidate SHA, review evidence and complete CI suite;
never bypass branch protection. Verify the merge, update delivery in the ledger
and safely fast-forward local main. This authorizes repository merging, not
production deployment. Retarget stacked PRs after prerequisites merge; if
retargeting does not trigger CI, refresh the branch to trigger normal CI.
