# EXP-045 actual result independent review

Read-only verification of the saved EXP-045 attempt. No retry, provider call, search, label change, or tolerance change was made.

## Receipt and analysis

The single real attempt is `q1-d-choice-step1`. Its saved request hash is `5b559a03518f284b1ccc3057f7441a564b1531d1289f8c6ee075c177d33926e5`; the receipt and raw response hashes match the saved request/response bytes (`b5bc7e77…f6cc3` and `398f3738…5cc25`, respectively). The request contains 11 questions and 151 offered options for this 32-card pool. The response reports pinned model `jev-1.13.0`, 285.326 ms, 15,855 input tokens, and 1,638 output tokens.

The global answer selected victim `c11`, and that victim's conditional answer selected `KEEP`. However, the required, unselected `replacement_for_c19` answer has 14 finite probabilities summing to `0.9900000000000001`; the other question distributions sum to 1. Re-running the frozen validator rejects the complete batch as `Choice probabilities invalid: replacement_for_c19`. This is a 0.01 mass shortfall, far larger than the registered `1e-6` tolerance or ordinary binary floating-point summation noise. The registered rule validates every required answer, including branches the global answer does not consume, so the c11 `KEEP` cannot be applied from this invalid batch.

The saved operation result therefore correctly returns full W0 for q1 and marks the remaining 20 operations `not_attempted_terminal_stop`. The terminal reason is `invalid_choice_response`; recorded usage includes the one case attempt and neutral acceptance. I independently recomputed `compute_report()` from the saved receipts and obtained the same analysis JSON and `inconclusive_protocol_or_incomplete` decision. Membership accounting passes, but the complete quality screen cannot pass from this interrupted run. No E rankings or quality/adoption conclusion are supported.

Hashes for the saved analysis, operation results, and attempt ledger are `ca50905c0df2aa9c1601a53f9c490c8ff6a192632635d6262b8a0da305465cec`, `9fadb52a26d6f2f7e3c10a1ed89ac9cf18324dcd26d79ac44310dab5df678a93`, and `756f30270755899d33471ac9dae8bca2214da379c3186b589edeb2315ec8b1aa`.

## Prospective handling only

The current TypeSafe Choice and API references define `choice` as the highest-probability option and describe `probabilities` as the full distribution whose values sum to 1. They do not specify a decimal rounding quantum or permission to renormalize a short-summing distribution ([Choice reference](https://docs.typesafe.ai/primitives/choice), [API reference](https://docs.typesafe.ai/api)). The current strict rejection therefore follows the published contract.

For a separately registered future protocol, the principled options are:

1. Keep the strict distribution requirement and treat any similar provider response as a protocol failure, as this run did.
2. First obtain a documented provider guarantee about probability serialization precision. If it confirms fixed rounding, preregister a residual bound derived from that precision and the maximum option count; retain the raw probabilities, validate the exact option set and finite range, verify `choice` against the raw argmax, and specify whether/where normalization is used. Qualify boundary fixtures and the exact maximum batch before any new case calls. The existing docs alone do not justify a 0.01 tolerance.
3. Define a new response policy that acts on the discrete `choice` field and treats probabilities as diagnostic only. This would materially change validation/selection semantics and needs its own frozen contract and evaluation; it cannot repair this attempt retroactively.

These are future design choices, not a reason to alter EXP-045's frozen tolerance, reuse the c11 answer, or resume the stopped attempt.
