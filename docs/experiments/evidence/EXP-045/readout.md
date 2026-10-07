# EXP-045 development result

Decision: inconclusive due to a provider-response contract failure. The first registered operation made one request and stopped. The failed operation returned full W0 fallback; the remaining 20 operations were not attempted. No retries or searches occurred. Including the separately published synthetic acceptance, this study used two provider attempts.

The response contained all expected questions/options and pinned model identity. HTTP time was 285.326 ms, reported usage 15,855 input / 1,638 output tokens. One conditional distribution summed to 0.99 rather than the registered 1.00 within 0.000001. The strict whole-batch contract therefore rejected it, even though the selected victim branch chose KEEP and the malformed distribution was a different branch. This is the frozen policy, not a retrospective judgment to rescue the response.

This result does not estimate quality improvement or rejection of the replacement idea: only one operation was attempted, and the complete population was not evaluated. Any numerical fallback analysis is diagnostic, not a reduced-sample primary result or evidence for adoption. Probability rounding is a hypothesis to investigate; do not normalize or alter the stored raw response, change this protocol, or rerun it. A revised policy requires a new prospective registration and qualification. Brave usage remains 0/10.

The current [TypeSafe Choice documentation](https://docs.typesafe.ai/primitives/choice) says the full option distribution sums to one. It does not specify a rounding tolerance. The observed 0.99 total contradicts that exact claim; rounding remains an explanation to investigate rather than a verified provider guarantee.
