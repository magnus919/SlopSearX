# EXP-071 outcome: complete invocation, selector rejected

The registered invocation ran on qualified merge `e682bad1545235171591a25ca0894e3f8eecb6ae`. All 21 operations and 43 calls completed; no structural failures or unknown usage occurred. Reported usage was 877,067 input and 58,443 output tokens, total wall time 19.291 seconds, maximum owned HTTP 571.808ms and maximum whole candidate phase 683.530ms. No retries or searches were made.

## Quality against fresh W0 in this invocation

| Reference | Mean candidate minus W0 nDCG@10 | Bootstrap 95% interval | Verdict |
|---|---:|---|---|
| A | -0.03836 | [-0.11451, 0.04064] | quality/retention failed |
| B | -0.03754 | [-0.10711, 0.02132] | quality/retention failed |

Both sample means were negative and neither met the required +0.05/lower-bound-above-zero gate. The intervals include zero; this is not a claim of statistically established general harm. Useful top-ten counts fell in q1/q2/q3/q4/q7/q8 under both references. Case-specific noninferiority failures also occurred. No useful W0 facet was lost. Repeat/rotate overlap stayed 0.8–1.0 and passed all eight comparisons. The three official navigation targets actually present stayed top one; q10/q11 missing acquisition targets remain unmet recall, not ranking failures or recovery credit.

The single-call mechanism resolved sequential latency feasibility and the numerical admission edge, but did not qualify ranking quality. Reject this selector. Do not advance it into untouched confirmation or runtime implementation, and do not change thresholds/references based on this result.

## Evidence and remaining work

The [run](completed/run.json), [analysis](completed/analysis.json), [43-call raw receipt correspondence verification](completed/raw-receipt-verification.json) and [scan](completed/scan.json) retain the evidence. Root independently reconstructed all 43 prepared request bodies, matched raw parent/transport responses and timing bindings, revalidated all typed answers/derived signals and summed every known usage row. Sanitization scanned all 90 JSON files including decoded hexadecimal data before publication, with zero secret/private-pattern hits.

Source-level diagnosis must distinguish direct-support classification errors from relative-band ordering errors before another mechanism is registered. The complete-pool production acceptance checklist remains open: qualifying development and untouched confirmation, shared SlopSearX path and X context propagation, compatibility/security tests, substantive review, required CI and implementation merges. No runtime, default-path, deployment or Hermes changes were made.
