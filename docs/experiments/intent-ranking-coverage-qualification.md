# Coverage-first ranking: prospective qualification checklist

Status: **NO-CALL DESIGN CHECKPOINT / NOT REGISTERED / NOT ADMITTED**.
Related to [#516](https://github.com/magnus919/SlopSearX/issues/516).

The maintainer approved the [coverage-first candidate amendment](coverage-first-release-decision.md)
on 2026-10-09. This resolves the candidate-decision dependency below; exact
registration, source qualification and admission remain outstanding. This
historical no-call checkpoint is not itself an invocation permit.

The previous complete-pool development comparison did not qualify its candidate.
Its [published results](intent-ranking-development-results.md) remain unchanged.
The [coverage prototype](intent-ranking-coverage-prototype.md) is a different
hypothesis: reserving a relevant lead for each caller-declared research need may
help an answerer use a broader range of evidence. It has no measured quality
result. This checkpoint authorizes no search, capture, provider or model call.

## Candidate to qualify

The proposed source baseline is merged commit
`6818b658f8a701ab826582fc2dd496240378f39b`:

| Component | SHA-256 |
| --- | --- |
| `scripts/intent_ranking_coverage.py` | `806746b4fa5c497f269933008db76650e2c1113dbd35a08b011d86c4d81f11ca` |
| `scripts/intent_ranking_receipts.py` | `3aa43da78719596424387339a8202b03699f840264ca83b12efc91b56e580df9` |

Freeze the prototype's Score + Choice instructions, model, representation,
probability validation, score floor, tie rule, whole-order fallback and bounds
without tuning after registration. Requested facet definitions must be authored
from the caller's task before looking at reference grades or candidate outputs.
Frozen evaluator labels, source assessments and answer grades are never ranking
inputs. Model-predicted card coverage is an annotation of visible text, not
proof that a document supports a claim.

## Inputs and controls

The existing eight development pools and their captures are exposed historical
evidence, not eligible primary cases for this proposed study. The approved bar
requires eight new development tasks/pools and eight distinct untouched
confirmation tasks/pools. Register both cohorts, task/facet definitions,
acquisition and capture plans, input lineage and assessor assignments before
invocation. Retain every failed acquisition and complete pool; no replacement,
synthetic concatenation or trimming. Freeze source availability and independent
references before selector/answer outputs. The remaining Brave allowance is two
attempts; this checkpoint uses zero and reserves both.

Define W0 explicitly before measurement. A comparison against current production
must execute its query-only ranking on identical frozen pools with the exact
production source and configuration pinned; historical orders are diagnostic
unless equivalence is established. Keep source opening, context, answerer,
time/token budgets and anonymous assessor packet construction equal between
arms. Navigation and tasks without valid caller facets remain on W0, but their
present-target top-one gate still applies. Freeze new, distinct confirmation
tasks and an acquisition plan before development; do not open confirmation
outputs unless development passes.

## Gates retained

The [approved release bar](research-usefulness-release-decision.md) remains the
quality bar, not permission to replace its frozen candidate automatically:

- Eight research tasks per stage; at least four natural pools of 41–80 cards.
- Each independent A/B reference: mean nDCG@10 gain at least .02, positive
  bootstrap 95% lower bound (10,000 draws, seed 7701), every per-case delta at
  least -.03, no useful-count loss or useful-facet loss.
- Each independent R1/R2 assessor: at least two additional completed tasks,
  no incumbent completion lost, no material unsupported claim, omitted critical
  contradiction/qualification, target/version error or unearned completion.
  Every critical check must have supporting, applicable evidence delivered and
  correctly used; bibliographic/topical leads alone do not satisfy this gate.
  Abstention is separate from completion; unknown required endpoints remain
  inconclusive. Do not average assessor disagreement into a pass.
- Five acquired exact navigation targets stay top one. Repeat/rotation top-ten
  overlap is at least .8. Every result remains in the complete permutation.
- Retain the one-second candidate phase gate, including request construction,
  exchange, parsing, selection and final receipt fsync. Retain all declared
  byte/token, physical-call and total usage ceilings in the eventual protocol;
  do not silently copy obsolete historical limits.

Both development and untouched confirmation must pass before optional shared
service/API/MCP/portal implementation and GroktoCrawl X propagation. Existing
query-only defaults and SearXNG compatibility remain unchanged. A documentation
merge, passing fixture or transport probe does not establish product readiness.

## Runner and independent admission requirements

1. Register an exact stage manifest: tasks, facet definitions, complete pools,
   control, source pins, assignments, numeric gates, model identities, total
   call/token budgets and one-shot stop conditions. The old candidate's failure
   does not grant a replacement invocation; record explicit authorization for
   this new candidate separately from approval of its offline prototype.
2. Implement a no-call preparation command that verifies every pin, input and
   complete operation inventory before loading provider credentials. Give the
   new stage its own identifier; historical leases and clocks confer no authority.
3. Bind every physical response slot to stage ID, operation ID, exact request
   hash and qualified source revision. Preserve exact response bytes privately
   before parsing, including malformed responses and transport-failure status.
   Seal receipt hashes independently; replay those bytes through the exact
   candidate parser and require identical annotations, order and fallback reason.
   The current receipt utility's replay path accepts only complete successful
   HTTP slots. Define a separate terminal inventory for failed/partial slots
   and later uninvoked operations; preserve failure bytes without claiming that
   those slots passed successful replay.
4. Test wrong bindings, missing/extra slots, tampering, interrupted writes,
   non-UTF-8 and malformed JSON, probability/score inconsistencies, size limits,
   exact budget reservations and terminal stop behavior. No repair, silent retry
   or missing-usage assumption is permitted. Raw bodies and credentials are not
   publication artifacts.
5. Independently review the exact runner and data separation, pass repository
   checks and merge registration/qualification artifacts. Publish an explicit
   admission receipt before a bounded readiness probe or scientific invocation.
   Provider acceptance and token fit require observed receipts, not fixture claims.

The executable runner, fully pinned manifests, exact physical-call budget,
untouched confirmation acquisition plan and admission receipt are outstanding.
There is deliberately no invocation command in this checkpoint. Issue #516
remains open.

## Five blockers before preregistration or invocation

1. **Resolved by the 2026-10-09 amendment.** The approved 2026-10-05 decision
   designated EXP-076 equal-weight RRF as its sole candidate. The linked
   maintainer amendment now satisfies the candidate-decision prerequisite for
   coverage-first; registration and admission still require the other gates.
2. The exposed eight development pools are historical evidence. Qualifying
   development requires eight new tasks/pools plus eight distinct untouched
   confirmation tasks/pools. Existing pools may be used only for exploratory,
   nonqualifying analysis unless an explicit amendment changes this requirement.
3. First-40 behavior and broader 41–80-result behavior need separately declared
   qualification. Whole-pool score stability alone does not establish either
   production scope. Freeze both scopes and their decision rules before calls.
4. Every critical check must be supported, applicable, delivered and correctly
   used. Bibliographic or topical-only leads are not substantive visible evidence
   for factual-answer tasks. Keep fetch desirability separate from substantive
   evidence; a likely useful document is not an observed supporting passage.
5. Fresh blind independent source/card references must be frozen before any Jev
   score or selector output in both stages. Historical exposed references do not
   replace this requirement. Freeze the exact budget and invocation only after
   the candidate amendment and complete cohort/runner contract are reviewable.
