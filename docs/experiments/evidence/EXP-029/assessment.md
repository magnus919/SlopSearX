# EXP-029 prerequisite assessment

Baseline cb46b990a9df2bed5bd620808146db2efd5bd0c7; registration 5420464.
Audit completed once. reference-audit.json preserves tracked-file reference
matches and hashes, including the newly registered protocol. To reproduce the
search from this registration checkout, run `git ls-files docs tests`, inspect
text files for participant/usability/recovery references, then inspect the
portal evidence record and experiment protocol at the indicated line numbers.
No keyword search alone proves the absence of resources outside the repository.

Reviewed evidence:

- PORTAL_UX_SPEC.md:153 describes recording representative participant checks,
  but does not provide a cohort, consent/scoring records or recovery completion
  observations. Its first-release proxy target is an acceptance target.
- PORTAL_ACCEPTANCE.md Evidence record documents local maintainer visual review,
  accessibility-tree review and scripted Chromium/smoke checks. These do not
  supply measured participant recovery outcomes for either arm.
- tests/test_portal_browser.py and portal tests provide deterministic journeys,
  useful for regressions, not independent human behavior observations.
- Other search matches concern research job recovery, database leases, auth,
  annotation labels, or statements that usability remains unmeasured.
- The current thread authorizes bounded offline experiments but has no available
  cohort or scope for recruiting/contacting participants. No one was contacted.

The registered evidence gate therefore cannot pass with available resources.
Baseline and candidate completion rates, effect and interval are unmeasured.
No candidate prototype, browser journey, participant trial, service replay or
regression suite was run. No candidate implementation exists to discard.
This is a missing-resource blocker; it is not evidence against the proposed hint.

Resume when an authorized participant cohort, consent/observation plan and frozen
matched scenarios are available. Do not re-audit this same absence every day,
substitute agent clicks for participants, or claim a faster human recovery from
DOM structure or passing tests.
