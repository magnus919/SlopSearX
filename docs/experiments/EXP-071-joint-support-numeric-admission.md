# EXP-071: joint classification with bounded numeric argmax admission

EXP-070 remains incomplete: q1-repeat selected supported_1 at 0.33999999999999997 while another option had 0.34, a one-binary64-ULP discrepancy. Its retained outcome must never be reclassified as a pass. This registration corrects only selected-label numeric admission for a separately identified invocation.

## Exact correction

After the unchanged strict finite, nonboolean, range, option/answer/model and sum checks, define max_raw=max(raw probabilities), chosen_raw=raw[choice], gap=Decimal(str(max_raw))-Decimal(str(chosen_raw)), and tolerance=Decimal(str(4*max(math.ulp(float(max_raw)),math.ulp(float(chosen_raw))))). Accept selected-label argmax only when 0<=gap<=tolerance, using Decimal precision100. Preserve raw probabilities; do not snap values, change normalized mass, reinterpret confidence, or use the selected label as a ranking key.

The [protocol](evidence/EXP-071/protocol.json) otherwise retains byte-identical EXP-070 requests, model, five classes, normalized support threshold strictly above 0.5, conditional expected supported-band ranking, canonical lower-bucket ordering, complete eligible pool, all six inputs/references, 21 operations/43 calls, neutral80, one-second whole phase, quality/retention/stability/navigation gates, resource bounds, serial execution and zero retries/searches. This is a numeric admission correction, not a new quality/calibration claim or a relaxed relevance threshold.

## Qualification before a separate invocation

Independent Luna design review supports this narrow correction. Tests must accept exact ties and the observed pair, accept four-ULP and reject five-ULP gaps constructed with nextafter, reject a meaningful 0.001 wrong argmax, retain malformed/bool/nonfinite/missing/extra-key rejection, and show unchanged raw-distribution-derived signals/order. Reuse the guarded full prebuild, exact committed admission pins, one-shot lease, pending/raw/known-usage durable receipts, remaining child deadline, whole-W0 fallback and fail-stop. No runner exists at registration; qualification must merge before any live calls.

The [offline envelope](evidence/EXP-071/request-envelope-check.json) inherits exact EXP-070 request sizes because no request fields change. Existing exposed development cannot replace untouched fresh confirmation. All compatibility-preserving runtime implementation, substantive review, required CI and merge requirements remain open in [production acceptance](complete-pool-production-acceptance.md). No default path, deployment or Hermes changes are authorized by this registration.
