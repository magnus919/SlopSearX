# EXP-065 incomplete invocation

The qualified invocation stopped after its first W0 call with `score-type`. Jev returned the expected model, exact answer inventory, finite ordinal scores, and known usage (11,375 input / 429 output tokens). Its answers also contained confidence, legend, and probabilities metadata. The harness incorrectly required exactly two answer fields; the shipped client requires the type and score while permitting metadata.

No shared or local request was sent. No comparative quality result exists. The failed invocation, exact source bindings, pending/raw transport receipts, and decoded-hex sanitization scan are retained. No retry occurred; the one-shot lease remains intact. No new searches occurred, leaving Brave at eight of ten.

The validator correction accepts additional response metadata while retaining exact model/membership, duplicate-key rejection, byte bounds, finite ordinal score/type checks, and known usage. A separate registered invocation is required before further calls; historical failure artifacts remain unchanged.
