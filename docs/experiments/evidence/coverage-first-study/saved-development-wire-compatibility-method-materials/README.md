# Saved-corpus test settings for the corrected Jev parser

These settings identify the corrected response parser. The questions, saved
corpus, ranking instructions, request schedule, budgets and pass/fail thresholds
stay the same. The previous settings and results remain unchanged.

The parser accepts a small score discrepancy only when rounding the displayed
probabilities and score could explain it. It keeps the reported score. A choice
separated from the highest probability by one adjacent floating-point value is
treated as uncertain, so it cannot claim a research facet. The provider's [Score reference](https://docs.typesafe.ai/primitives/score)
does not promise a rounding precision; this is an explicit, bounded compatibility
rule tested against an archived synthetic response.

The [lock file](material-lock.json) records the exact parser, protocol, cohort
and companion versions. Only the candidate source hash changes in the protocol;
the companion identifies that updated protocol and keeps the same case groups.

This folder contains test settings. There are no new comparison results. Offline
fixtures verify the software path; a new model-backed comparison still needs
fresh identities and its complete checks before it can establish ranking quality.
