# EXP-051 protocol review

Review scope: read-only review of the current EXP-051 protocol, review contract, and delivery design. No repository files were changed.

## Blockers

None in the reviewed drafts.

The current review contract resolves the earlier rubric concerns. It uses typed `reading_change` values, requires case evidence pointers for distinct lead changes, defines deterministic precedence for the overall relation, and propagates uncertainty in either affected card's discovery label to `reading_change=uncertain`. It requires comparison against the retained selected set, so a redundant card or prose-only assertion cannot establish a distinct contribution.

The abstract rule is consistent: the exact bounded excerpt and count/clipping data remain in private packets, per-abstract digests are omitted, and qualification checks against the pinned projection. Registration precedes implementation; the complete 232-card, 12-case, six-pool corpus and unknown lanes remain fixed. Metadata stays separate from truth, authority, and original engine provenance. Selected-union card judgments are collected once per pool, and set coverage is derived from those labels.
