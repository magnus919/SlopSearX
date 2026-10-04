# Provider contract snapshot — 2026-10-04

Refreshed from the primary [API reference](https://docs.typesafe.ai/api), [Noul documentation](https://docs.typesafe.ai/primitives/noul), and [model listing](https://docs.typesafe.ai/models).

The evaluation endpoint is `POST https://api.typesafe.ai/v1/systemone`. A request contains `model`, `state` and named `questions`. The question field is `instructions` (plural). Noul supports optional true/false criteria and returns a yes probability under the matching question ID. IDs are response keys, not semantic question text. Responses contain the resolved model, typed answers and input/output token usage; Noul has no separate confidence field.

Pin `jev-1.13.0`. The published limits are 64,000 tokens for state plus all questions, and 32,000 for state plus the longest question. Bytes do not establish token fit. No exact tokenizer preflight is assumed. Documented input price is $0.042 per million tokens, with output tokens free; the three-million-input study budget therefore corresponds to at most $0.126 of known input usage. Unknown failed-call usage is not zero. Acceptance, actual usage and latency still require provider observation.

The prototype previously used singular `instruction`. That wire-contract defect was corrected before any new provider attempt. The fixture now rejects the singular field before invoking its opener. Existing prototype packets remain historical synthetic evidence; they never established API acceptance. The registered question semantics and .5 action boundaries are unchanged.
