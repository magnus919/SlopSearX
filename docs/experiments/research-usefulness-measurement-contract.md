# Research-usefulness measurement contract — offline design

Status: measurement design and inert prototype work; no registered model
comparison, candidate qualification or invocation. Follows the maintainer's
[prospective decision](research-usefulness-acceptance.md). Exact task cohort,
numerical uplift/sample requirements, budgets, source pins and execution
qualification must be registered before any new outputs or acquisition.

## Estimand and stages

The selector chooses sources worth opening; task usefulness measures whether
the bounded consumer receives and correctly applies evidence to the original
question. Sparse but promising primary sources may deserve opening without
substantive evidence visible in their search cards. Do not replace reading
priority with card-only factual support or confuse either with source truth.

For each declared task/critical facet, preserve an auditable progression:

| State | Interpretation |
|---|---|
| Not acquired | source/target absent, failed or blocked; preserve reason |
| Acquired unusable | source opened but lacks applicable evidence |
| Available, not delivered | qualified evidence in the frozen pool omitted from the budgeted context |
| Delivered, unused | applicable evidence delivered but not applied |
| Used with support | correct application to facet with valid attribution and reviewed entailment/scope |
| Used invalidly | misattributed, unsupported, wrong target/version or unearned assurance |

These are per-evidence/task-use dispositions. Do not force a whole source into
a single state when it supports one facet and is unusable for another. Keep
acquisition and delivery records separate from reviewer semantic judgments.
Unavailable evidence remains in denominators; it does not become a repaired
ranking case or disappear through sample replacement.

## Frozen inputs and fair comparison

Before acquisition, define task IDs, critical checks, targets, date/version/
assumption boundaries, expected valid evidence forms and abstention rules.
Use one captured availability/content snapshot for both arms, the same
first-ten/successful-source/context projection or another prospectively justified
consumer budget, exact prompts/model identity where observable, token/time
bounds and stop/no-retry rules. Retain alias/checkpoint uncertainty.

Freeze two independent source assessments before selector output and answer
dispatch. Keep discovery desirability separate from full-text support,
contradiction and qualification. Grouped/copied publications are not independent
corroboration; scope differences are not automatically contradictory evidence.

## Attribution and closed assessment packets

Use immutable source and evidence IDs, UTF8 content hashes and **Python Unicode
character offsets in the exact delivered context**, matching the existing
evidence catalog. Check bounds, text/hash/ID identity and arm delivery
deterministically. Offsets establish attribution only, never entailment,
applicability, independence or factual truth.

Future model outputs should select immutable catalog evidence IDs. A separately
frozen deterministic hydrator resolves known IDs into expanded integrity
records; models need not copy hashes, offsets or whole passages. Preserve raw
output and reject unknown IDs without semantic repair. The inert validator
prototype checks expanded envelopes, not a qualified model-facing adapter.

Require closed schemas and exactly the registered task/arm/facet rows for each
independent assessor. Reject duplicate, extra or missing identities and fields.
`unsupported_claim_indices` and other required arrays must appear explicitly,
even when empty; missing is not an empty list. Unknown required judgments use
explicit `not_assessable` with a reason. Do not impute from another assessor,
repair an output or reprompt after seeing its scores. Preserve invalid raw
packets and valid partial observations; a structurally invalid or unassessable
endpoint cannot pass its comparison gate.

For each critical facet require semantic judgments of support/application,
correct target/scope, material unsupported claims, contradictions/qualifications
retained or omitted, and whether the required observable check is complete.
These are assessor judgments bound to evidence, not answerer self-certification.
For claims of performed actions require the declared execution/artifact record;
a documented procedure is not evidence that an action occurred.

## Completion and paired comparison

A task is complete only when every critical check is supported and correctly
applied using evidence delivered to that arm, with no material unsupported
claim, missed contradiction/qualification, target/version error or unearned
completion assurance. Partial or missing critical checks are incomplete.
Appropriate abstention is a separate safety outcome, not task completion.

Report each assessor's paired binary completion, context-coverage changes and
unsupported-claim/contradiction results separately. Never average away a valid
disagreement to manufacture a pass. Freeze sample count, practical uplift margin,
uncertainty/non-regression and unavailable/invalid endpoint rules before a run.
No numerical task-use gate is chosen by this design. Existing ranking/resource/
stability/navigation and untouched-confirmation requirements still apply.

## Prototype boundary and evidence

Synthetic validator fixtures qualify only deterministic integrity behavior. They
receive no quality/task-use/adoption credit and make no calls. No prototype
changes runtime search, inference, Hermes or deployment.

This design responds to [pilot attribution failures](evidence/source-use-next-stage/pilot-outcome.md),
[missing assessment fields](evidence/evidence-id-ablation/readout.md), and
[observed task-check ambiguities](evidence/selector-failure-diagnostic/prospective-task-checks.json).
All historical results remain unchanged.
