# EXP-073 outcome: incomplete at the capacity deadline

The qualified harness made two fresh provider attempts, then stopped. The synthetic 80-card capacity operation obtained a valid first-40 query-only response in **238.17 ms** of reported HTTP time. The complete-purpose request timed out before the shared one-second phase could finish; the retained phase lasted **1012.38 ms**, including termination/failure handling. The other 21 operations were not invoked.

Known usage is 12,297 input and 594 output tokens from the first response. Usage for the timed-out second attempt is unknown; it is not counted as zero or estimated. The study was not retried. No searches or Brave calls were made, so the remaining Brave budget stays at two.

Independent raw verification accepts the incomplete evidence: both attempted request bodies are bound to registered inputs, the successful response and its retained timing envelope match their hashes, observed usage is recomputed, and the timeout, stop and uninvoked operations match the saved analysis. The analyzer reports `incomplete` and performs no quality analysis. Expected structural errors identify the failed capacity phase and uninvoked operations; they do not represent measured ranking regressions.

This is a resource-bound failure, **not a ranking-quality result**. The combined design is not qualified for implementation under the unchanged contract. The evidence does not establish whether its research rankings would improve. Before another registered study, inspect transport overhead without provider calls and compare the harness path with the intended persistent service path; do not subtract overhead from this measurement, relax its deadline, reuse its model responses, or rerun it.

The public packet includes the raw call receipts, failure record, run, analysis and verifier report. Its scan checks decoded request/response/telemetry bytes for authorization material, the configured credential value and private deployment details. Private launch files are excluded. No deployment, default, mainline GroktoCrawl or Hermes changes were made.
