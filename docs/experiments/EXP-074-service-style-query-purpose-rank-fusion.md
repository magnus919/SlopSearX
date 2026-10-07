# EXP-074: service-style transport for fixed query/purpose fusion

This is a separately registered system-development comparison, not a retry of the terminal EXP-073 run. The 24-pair offline diagnostic met its frozen motivation rule: the median subprocess-minus-in-process fixture difference was 180.033 ms. That does not estimate provider latency, subtract overhead from EXP-073, or establish a ranking improvement.

The candidate retains the exact EXP-073 request semantics, two serial fresh calls, equal-weight exact RRF k=60, stable canonical ties, full eligible-pool membership, budgets, stop/fallback behavior and both independent-reference quality gates. The only mechanism change is an in-process asynchronous HTTP transport, with a new `httpx.AsyncClient` per call, no redirects, environment proxy inheritance, retries or cross-call client reuse. The whole operation retains its one-second deadline from before the W0 pending receipt through both calls, parsing, fusion and the durable final candidate receipt.

The synthetic 80-card operation is capacity-only. If it succeeds, the frozen 21 development operations follow; only a complete schedule permits ranking analysis. A pass cannot establish general tail benefit or final adoption: these tasks and pools are exposed, and untouched confirmation plus every cross-service acceptance requirement remain. Missing navigation targets remain acquisition gaps.

The [protocol](evidence/EXP-074/protocol.json) is frozen before calls. Implementation and dependency pins, independent offline qualification, full receipt verification and a new one-shot study identity are required before dispatch. The old EXP-073 identity remains permanently stopped. No search, runtime/default/deployment/Hermes changes are authorized by this registration.
