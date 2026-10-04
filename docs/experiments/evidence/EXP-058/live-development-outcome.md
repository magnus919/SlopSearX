# EXP-058 live development outcome: reject for production

The one-shot study completed every one of twenty-one operations with valid full permutations and no failure. The fixed budget-admission guard prevented another call from exceeding the remote budget; seventeen operations stopped explicitly for that budget, including one constructed case with zero calls. The maximum cumulative historical D/F-plus-new-HTTP duration was 1385.405 ms. This is mixed development timing, not fresh end-to-end production latency.

Twenty-six Jev attempts (one neutral plus twenty-five case calls) consumed 831,487 known input and 46,682 output tokens. No unknown usage, search or Brave call occurred. All five exact navigation controls and repeat/rotation stability gates passed. Exact original requests, responses, process receipts, ranking traces and complete analysis are preserved in live-run.json and live-attempts.json.

## Quality decision

Under reference A, primary E-W0 mean nDCG@10 gain was +0.197017 with lower95 +0.108697. Under B it was +0.030112 with lower95 -0.020600; B's strict-positive confidence gate failed. E-D primary means were -0.018718 (A) and -0.037605 (B), failing the unchanged -0.01 noninferiority floor. Several software pools also exceeded the permitted per-pool loss. Macro facet coverage stayed at 1.0 under A and 0.977273 under B, unchanged from D, so the required strict gain was absent. All original labels, references and numeric gates remain unchanged.

Reject this candidate for production and do not spend the untouched Brave allowance on its confirmation. Purpose-only scoring remains a useful comparator; these additional swaps did not establish a reliable improvement over it. The result supports reconsidering whether proposed novelty addresses a needed evidence gap, rather than assuming any distinct lead improves the user's selected set. Any successor must be separately registered and face the same quality gates. No deployment/default/Hermes change or production integration follows. Full production acceptance under #516 remains open. Brave usage remains 0/10.
