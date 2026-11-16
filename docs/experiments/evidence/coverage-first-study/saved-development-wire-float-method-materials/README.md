# Saved-corpus test settings for binary-float score compatibility

This version identifies a bounded parser correction for JSON decimal values that
arrive as binary floats. Validation accepts a nominal hundredth and exactly one
adjacent representable float for reported scores and choice probabilities. The
original wire values and reported scores are preserved; ranking instructions,
cohorts, task-use recipes, budgets, and pass/fail thresholds are unchanged.

The Score probability mass and weighted mean are checked jointly: there must be
a feasible choice within the registered hundredth-rounding intervals whose mass
is exactly one and whose weighted mean explains the reported score. Choice
probability rounding uses the same bounded interval rule and requires exact unit
mass. No epsilon was added to probability mass, and raw values are not rewritten.
A second adjacent float is rejected. Choice ties retain the existing conservative
rule.

The [lock file](material-lock.json) records the exact parser, protocol, cohort,
and companion versions. Only the candidate source hash changes in the protocol;
the companion binds that protocol to the same frozen case groups. Earlier method
folders and results remain intact.

This folder contains locked test settings, not comparison results or stage
authority. Offline fixtures verify compatibility only. A new model-backed
comparison still requires fresh identities and the complete registered checks
before it can establish ranking quality.
