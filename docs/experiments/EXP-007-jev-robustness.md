# EXP-007: Jev calibration and adversarial robustness

Status: **registered; not yet executed**

## Hypothesis

Jev relevance probabilities distinguish frozen relevant and irrelevant result
fixtures despite missing snippets, misleading titles, prompt-like text in
results, and multilingual text.

## Frozen method and gates

The harness contains 16 independently labeled fixtures. Each makes one Jev
Noul call and uses `p >= 0.5` as relevant. Cases tagged `adversarial` contain
prompt-like or misleading result text; that text is data, never an instruction.

Supported-for-further-research requires: 16/16 valid calls; balanced accuracy
at least `0.80`; adversarial false-positive rate at most `0.25`; positive
false-negative rate at most `0.25`; p95 at most 1,500 ms; and cost below
`$0.01`. Calibration error and Brier score are diagnostic because 16 fixtures
are too few to establish calibration.

