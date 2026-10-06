# EXP-080: adaptive follow-ups

## Registration

State: registered. Date: 2026-10-06. Explicit one-off user request.

Frozen shared [protocol](evidence/QUERY-PLANNING-2026-10-06/protocol.md) and [case manifest](evidence/QUERY-PLANNING-2026-10-06/cases.json) define the hypothesis, current-main baseline, scope, primary metric, threshold, guardrails, sample/stopping rules, commands and limitations.

No candidate plan or evaluation acquisition has run. This is an exposed, caller-authored discovery pilot, not production qualification.

## Readout

State: supported. Registration commit: `42ed87cc0999b44c5d88d452611881b11c221d3f`.

Fixed-corpus paper recovery: 0.2083 → 0.8333; absolute gain 0.6250, adjusted interval [0.4166666666666667, 0.8333333333333334]. All registered gates pass. This supports reading-lead discovery on the exposed corpus only; no automatic planner, answer quality or adoption claim.

[Shared readout](evidence/QUERY-PLANNING-2026-10-06/readout.md), [qualified decisions](evidence/QUERY-PLANNING-2026-10-06/decision-summary.json), [raw receipts](evidence/QUERY-PLANNING-2026-10-06/receipts.jsonl), and [analysis](evidence/QUERY-PLANNING-2026-10-06/analysis-rows.json) retain all outcomes and limitations. No runtime code change or implementation PR. Documentation persistence PR: pending.
