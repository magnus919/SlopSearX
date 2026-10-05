# Optimistic source-identity feasibility bound

Offline mathematical diagnosis only; no candidate was proposed or tested.
The registered EXP-076 rejection and all production gates remain unchanged.

For each primary case and each separate reference, enumerate retaining all
useful W0 top-ten IDs or all except one. Fill the remaining top-ten slots with
the highest-graded eligible pool entries and sort the selected ten by grade.
The highest resulting nDCG is an optimistic upper bound under the source-identity
constraint. Every feasible top ten is included in one of those retention
scenarios; selecting the highest remaining grades and sorting them optimally
cannot reduce its nDCG. The calculator verifies the baseline against the
original analysis and binds source hashes.

| Reference | Optimistic mean gain ceiling |
|---|---:|
| A | +.077625 |
| B | +.071784 |

Root and an independent Luna calculation reproduced both means. The +.05
quality threshold is therefore not shown mathematically impossible under
identity retention alone. This is not proof that a real selector can attain it.
The calculation uses reference grades unavailable at runtime and gives each
reference its own optimal selection. It does not prove a single selection can
satisfy both, and ignores facets, useful-count, stability, navigation, resource
bounds, provider attainability and downstream task completion.

Do not use these grade-selected lists as a deployable candidate, fit fusion
weights to them, alter thresholds or award qualification/adoption credit. The
remaining task is a justified prospective mechanism with independent evidence,
not an optimistic oracle. No provider, search or deployment activity occurred.
