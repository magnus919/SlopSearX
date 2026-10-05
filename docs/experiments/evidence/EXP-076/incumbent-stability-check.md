# Per-request incumbent preservation versus stability

This is an offline feasibility check of a proposed design, not a new candidate
comparison or a change to the registered acceptance criteria. Root and an
independent Luna check reproduced the saved W0 sets and the following bound.

For q4, ordinary W0's base and rotated top tens have zero IDs in common. A
proposed runtime selector retaining at least nine entries from each request's
own W0 top ten can produce candidate sets with at most two shared entries:
each set contains at most one entry outside its own disjoint incumbent set.
Therefore their overlap cannot exceed .2, below the registered .8 stability
requirement. More generally the common-count upper bound is the incumbent
intersection plus two, capped at ten. The calculator retains all four stability
cases and both variants, binds the original run hash, and performs no calls.

EXP-076's actual q4 candidate lists share all ten IDs between base and rotation;
they retain eight and one entries from their respective W0 sets. Its measured
stability is therefore compatible with diverging incumbent membership.

This does **not** establish a contradiction in the original gates. Their useful
source-identity metric is measured on base cases, not a requirement to retain
nine incumbent IDs on every variant or every future request. The check rules
out only the naive per-request anchor proposed as a possible implementation
of source continuity. It must not be promoted to a provider experiment.

A stable anchor or a different product definition of continuity would require
its own semantic justification and prospective registration, while keeping
other acceptance requirements intact. The optimistic grade-based bound in
identity-feasibility-bound.md explicitly ignores stability and remains only
a separate mathematical ceiling. Neither diagnostic supplies a deployable
selector or production readiness. No search, provider or deployment activity
occurred, and the existing rejection is unchanged.
