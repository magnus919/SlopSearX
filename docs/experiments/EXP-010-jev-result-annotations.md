# EXP-010: Jev additive result annotations

Status: **registered; not yet executed**

## Hypothesis

Jev can assign useful source-type annotations without changing result order.

## Frozen method and gates

The harness contains 18 independently labeled result fixtures across six
classes: `primary_docs`, `source_repository`, `standard_regulatory`, `tutorial`,
`discussion`, and `vendor_marketing`. Jev scores all six labels with identical
Noul questions. The highest score is emitted only when it is at least `0.5` and
beats the runner-up by at least `0.10`; otherwise the result abstains.

Supported-for-further-research requires: 18/18 valid calls; non-abstained
coverage at least `0.75`; accuracy on all fixtures (abstentions count wrong) at
least `0.80`; macro F1 at least `0.75`; p95 at most 1,500 ms; and cost below
`$0.01`.

Annotations are descriptive hints, never factual verification, authority,
SafeSearch enforcement, or permission to reorder results.
