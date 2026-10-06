# EXP-085 terminal outcome

Status: setup-inconclusive. The fresh admitted run made one health request and stopped before preparing assessments or calling any model. No search, source capture, Jev comparison, answer, grade repair or retry occurred.

The controller looked for top-level `model` and `revision`, while the actual API supplies `runtime.model` and `runtime.revision`. A read-only diagnostic of that already-returned response confirmed the nested values matched the pinned model and revision. That diagnostic does not turn the rejected health gate into a pass or authorize resumption.

The 28 offline tests and input/source qualification were valid interface checks, but their flat health fixture missed the real API shape. EXP-086 prospectively corrects only that parser and fixture under fresh qualification, admission and clock. Ranking usefulness remains unmeasured; production acceptance is unchanged.
