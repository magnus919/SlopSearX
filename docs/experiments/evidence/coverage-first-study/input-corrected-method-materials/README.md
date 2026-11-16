# Method for the corrected-input comparison

These files freeze the two query changes described in the [input-correction plan](../../../coverage-first-input-correction.md). They preserve the previous questions, confirmation cases, ranking candidate, control and quality thresholds. This is a method definition, not an execution registration or a quality result.

`input-correction-audit.json` records the changes. Deterministic tests restore the two original query values and compare the full cohorts, verify that confirmation is unchanged, check the material hashes, and exercise the calculator with both valid and mismatched registrations.

A separate execution registration must bind a clean source revision, fresh identifiers, configurations, budgets and the plan before any live calls. Failed collections remain published separately. No production service is changed by these files.
