# EXP-007: Jev calibration and adversarial robustness

Status: **completed; supported for further research**

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

## Result

All 16 calls were valid. Balanced accuracy was `0.9375`, adversarial
false-positive rate was zero, and positive false-negative rate was `0.125`.
The only miss was a relevant PromQL result with no snippet. The run is
**supported for further research**.

See the [suite results](JEV-FUNCTION-SUITE-RESULTS.md) and
[evidence](evidence/EXP-007/summary.json).
