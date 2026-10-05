# EXP-076 prospective design review

This review does not qualify or execute a provider run. No provider or search
calls were made. The mechanism remains subject to exact protocol and source
qualification before execution.

Root refreshed upstream GitHub state on October 5: the latest release remains
[v0.6.0](https://github.com/magnus919/SlopSearX/releases/tag/v0.6.0), published
October 4 at 16:56:32 UTC. The latest eight merged PRs (#655 through #662) are
the recorded experiment registrations, harnesses, outcomes and documentation
repair. The current baseline is #662's merge
`c9f2b48437c65c4ebc17f2c7ee18011ae440c119`; that inspection found no newer
runtime implementation replacing the candidate under study.

The prior-study review distinguishes this proposed canonical full-pool dual
transaction from EXP-075's first-forty query population and complete purpose
population. EXP-034 through EXP-036 already tested whole-pool query scoring
without qualifying quality. EXP-039/040 tested purpose-aware selection and
failed source-retention or facet safeguards. EXP-061 registered canonical
ordering but stopped before any candidate call. None establishes that the new
combination will improve selection.

GPT-6 Luna review identified these required checks:

- Both candidate question families must score every supplied card. Canonical
  state, stable identity mapping and question insertion order must produce
  identical request bytes under original, reverse, rotation and deterministic
  shuffle of the same frozen identities. This is stronger than canonical ties.
- ID-keyed score fixtures with ties and shuffled answer insertion order must
  produce identical complete component permutations and exact Fraction RRF
  results. W0 remains the independent ordinary first-forty comparator with its
  original tail and provenance-sensitive mapping.
- Register 160 questions for the 80-card maximum and explicitly select the new
  capability's aggregate request/state admission bounds. Matching EXP-075's
  numeric envelopes does not guarantee that all otherwise eligible 80-card
  pools fit; oversized complete inputs must fail admission without trimming.
  Offline byte fit is not token-context or
  provider acceptance. The short synthetic maximum does not establish natural
  80-card capacity. Context rejection must remain terminal, without trimming,
  retries or partial output.
- Assemble valid W0 and candidate permutations before the quality evaluator.
  A complete local fixture packet must reach both independent assessments
  through the actual nonsynthetic analysis branch and source-closure checks;
  helper-only or synthetic-short-circuit tests miss EXP-075's lookup failure.
- Carry forward EXP-039's at-most-one useful-source displacement guard as an
  explicit additional prospective requirement. Do not call it an unchanged
  EXP-075 gate or regrade historical outcomes.
- Keep all existing quality, loss, useful-count, facet, stability, navigation,
  membership, one-second durable-phase and usage/failure requirements. Missing
  q10/q11 acquisition targets remain unmet: permutations cannot create results
  absent from the frozen pool.

Root independently inspected the relevant committed protocols, analyzer and
completed readouts. Full production acceptance still requires untouched tasks
and pools, a qualified selector, explicit caller-context propagation, ordinary
compatibility, substantive review, required CI and verified implementation
merges. This review completes none of those acceptance items.

## Independently reproduced offline preview

Root read the credential-free calculator and invoked `calculate()` without
writing its report files. The result exactly matched the committed JSON
(`74a65dedc6372e54a0de476907125c9e4443b437c10799ae2ba25fb4a4e49169`).
All thirteen pinned natural pools had identical candidate request bytes and
mapping hashes under all four input permutations. Their request sizes ranged
from 36,179 to 251,111 bytes. The short synthetic 80-card request was 270,847
bytes with 10,462 bytes of state and 160 questions. These figures supersede the
preliminary projection estimates; exact prompt/ID construction affects size.
This is offline structural evidence only, with zero provider/search calls.

GPT-6 Luna independently reproduced both the JSON and Markdown exactly and
confirmed the input/projection bindings and four-way invariance. Its remaining
design concern is aggregate admission: short synthetic cards do not exercise
every maximum-length field. The protocol must justify its explicitly selected
bounds, reject oversized complete requests, and avoid presenting an 80-card
count bound as an unconditional natural-pool capacity guarantee. Parser/fusion
invariance still requires separate fixed-score/tie tests.

## Final design review disposition

GPT-6 Luna found one material ambiguity in the final draft: design registration
was placed after harness qualification. Root corrected the sequence to committed
immutable design registration, inert harness implementation, exact source
qualification, then gated provider admission. Root also verified all eight input
pins and made the existing 64,000/32,000 per-call reservations explicit. The
review found no other material issue in the complete-pool projection, independent
W0 comparator, native-order fallback, limits or safeguards. No provider call or
production qualification follows from this completed design review.
