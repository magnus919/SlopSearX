# Instructions for one improvement cycle

Read AGENTS.md, CONTRIBUTING.md, docs/experiments/README.md, the experiment
ledger and relevant project contracts. Inspect current main, open issues and
PRs, and unfinished experiments before choosing work. Preserve unrelated files.

Resume an unfinished experiment, or select one evidence-based improvement to
search usefulness, human experience, or agent experience. Register a falsifiable
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
main. Report branch-protection blockers without disabling protections. Mixed or
implementation PRs retain normal checks and review; do not auto-merge them or
deploy. Report the
experiment ID, measured effect and uncertainty, guardrails, decision, evidence
location, PR links, and unresolved blockers. Do not describe work as running in
the background unless a scheduler has actually been configured.
