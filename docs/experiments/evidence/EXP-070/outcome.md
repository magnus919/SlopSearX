# EXP-070 outcome: incomplete on Choice argmax roundoff

The live invocation on qualified merge `35c63d2b73a35b443dc11f9ccf1a87dfd2c78b98` stopped after 5 of 43 attempts. At q1-repeat, candidate c9 returned chosen label supported_1 with raw probability 0.33999999999999997 while unsupported had probability 0.34. The discrepancy is about 5.55e-17. Exact raw-maximum validation rejected it as choice-argmax. This is an observed provider serialization/numeric boundary, not evidence of semantic label error or ranking quality.

Known reported usage was 121,195 input and 9,125 output tokens; usage was complete for all five attempts. Total wall time was 2.159 seconds. The 80-card neutral passed (owned HTTP 482.429ms; candidate phase 565.875ms). The failed operation had owned HTTP 344.565ms and phase 425.535ms; deadline was not the cause. The runner preserved fresh W0 and made no later calls, retries or searches.

The analyzer correctly declined quality analysis and adoption. No complete quality improvement/regression conclusion is available. The [run](incomplete/run.json), [raw receipts](incomplete/receipts/call-04.json), [analysis](incomplete/analysis.json) and [scan](incomplete/scan.json) retain this invocation. Any corrected numeric admission must be preregistered and qualified before a separately identified invocation; do not reinterpret this frozen run as passed.

Full production acceptance remains open. No runtime, default-path or deployment changes were made.
