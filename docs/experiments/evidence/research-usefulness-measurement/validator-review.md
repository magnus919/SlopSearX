# Offline validator review

Status: draft findings pending stable-source fixes and root verification.
No model comparison, study admission or semantic qualification is claimed.

Root's bounded review of the initial prototype identified:

1. Source evidence dispositions must bind each reference to that disposition's
   own source ID, not merely any source in the frozen pool.
2. Nested source scope/entailment/applicability judgments marked not-assessable
   must propagate to the conservative assessment state even when another
   disposition field says evidence is available.
3. A captured unusable/nonapplicable passage may validly anchor a negative
   assessment. Forbidding every such anchor confuses non-support with absence.
   Not-acquired sources cannot supply a content reference.
4. Packet ID, assessor assignment and kind must match the frozen expected
   assignment, rather than accepting any nonempty label.
5. Non-string enum inputs must fail through the controlled integrity error.

The source owner is adding focused regressions. The initial ten synthetic
tests were owner-reported draft evidence, not independent qualification.

## Model versus integrity envelope

The expanded quote/hash/offset representation is an integrity envelope. It is
not a requirement that future models copy hashes, offsets or whole passages.
The previous ID ablation supports using selected catalog IDs and deterministic
resolution. A future registered hydrator must preserve original model output
and resolve only known IDs, without semantic repair or replacing invalid IDs.
It must be qualified separately before any model run.

Registry coverage and empty arrays do not establish exhaustive semantic
extraction, absence of contradictions or truth. The prototype verifies only
closed supplied identity/shape relationships; blind semantic judgments and
their uncertainty remain separate.

## Stable checkpoint disposition

Root integrated the corrected prototype at `5e68b0f` and independently passed
all 15 synthetic tests. The five findings above have focused coverage: source
binding, nested unknown states, negative-evidence anchors, frozen independent
assignment and controlled invalid enums. The earlier ten-test owner draft is
not the final verification checkpoint.

These passes establish bounded integrity behavior only. No live admission,
semantic entailment/coverage, model-packet hydration or task-completion result is
qualified. The deterministic evidence-ID hydrator and its envelope integration
remain under construction before any prospective model study.

## Evidence-ID integration checkpoint

Root integrated the deterministic hydrator at `7efb06f` and independently
passed six synthetic hydration tests, including actual validator integration.
The earlier 15 validator tests pass on the integrated source. This finishes
the assigned offline prototype scope; no new validator expansion is needed
before the next study design.

The helper resolves one explicit frozen catalog/assignment per invocation.
A study must predeclare per-task/per-arm assessment packet partitioning and
complete cohort coverage; these fixtures do not qualify that orchestration.
Raw model bytes are hashed separately, non-reference semantic fields are
preserved, and semantic completion is expressly not claimed.
