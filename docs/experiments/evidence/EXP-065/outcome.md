# EXP-065 incomplete invocation

The qualified invocation stopped after its first W0 call with `score-type`. Jev returned the expected model, exact answer inventory, finite ordinal scores, and known usage (11,375 input / 429 output tokens). Its answers also contained confidence, legend, and probabilities metadata. The harness incorrectly required exactly two answer fields; the shipped client requires the type and score while permitting metadata.

No shared or local request was sent. No comparative quality result exists. The failed invocation, exact source bindings, pending/raw transport receipts, and decoded-hex sanitization scan are retained. No retry occurred; the one-shot lease remains intact. No new searches occurred, leaving Brave at eight of ten.

The validator correction accepts additional response metadata while retaining exact model/membership, duplicate-key rejection, byte bounds, finite ordinal score/type checks, and known usage. A separate registered invocation is required before further calls; historical failure artifacts remain unchanged.

## Metadata-compatible registered invocation

The separately registered invocation completed all 21 operations and 63 calls, with no unknown usage and no structural failures. Usage was 1,142,347 input and 31,620 output tokens; maximum owned HTTP time was 542.784ms. No retries or new searches occurred. Together with the original failed attempt, EXP-065 used 64 physical calls and 1,153,722 input / 32,049 output tokens.

The local candidate failed quality gates under both references. A mean local-minus-W0 nDCG@10 was +0.00651 (bootstrap95 [-0.05555, +0.07202]); B was +0.04182 ([+0.00601, +0.08422]), below the registered +0.05 threshold. A q2/q7/q8 and B q7 exceeded the allowed per-case loss; A q3 lost one useful top-ten source. No W0 useful facet was lost. Repeat/rotation overlap was 0.9–1.0 for all eight comparisons. Official targets present in q9/q12/q13 remained first; q10/q11 targets were absent from acquisition, so recall remained unmet. Only one natural pool exceeded forty.

Question-local isolation therefore improved neither the required quality nor general tail evidence enough for adoption. Stable, fast, structurally valid calls are not source-selection success. Complete raw/source-bound packets and analysis are under `completed-metadata-compatible/`; the original failure remains unchanged. No production implementation, deployment, or ordinary/default ranking change follows from this result.
