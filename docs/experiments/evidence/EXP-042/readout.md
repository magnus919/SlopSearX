# EXP-042 — terminal result

**Outcome: not-supported.** The registered mixed Score/Noul candidate did not complete its operational screen and must not be integrated into production. The failed run was not retried.

## What happened

Thirty-five requests completed with valid typed responses. The thirty-sixth attempt, `constructed80-e`, received HTTP 400. That request contained 79 post-group candidates, 395 questions and 351,588 serialized bytes; it fit our declared byte cap. The remote error wrapper did not retain the provider's error explanation, so the precise rejection reason is unconfirmed. The runner stopped immediately, preserved the complete 79-ID W0 fallback order, and issued no subsequent requests. Stress repeats/rotation and all five navigation pairs remain unmeasured.

Known usage from the 35 valid receipts was 973,929 input tokens and 85,832 output tokens; failed-call usage is unavailable, not zero. The maximum valid remote latency was below 600 ms. New searches, Brave attempts, retries, deployment changes and Hermes changes: zero.

TypeSafe's [current model documentation](https://docs.typesafe.ai/models) specifies a 64K combined request context and 32K for state plus the longest question. These are token limits; fitting a serialized byte cap does not prove either one. Request-context overflow is a plausible explanation for this HTTP 400, but no actual token count or error body proves that diagnosis. Future qualification must explicitly address those provider limits before dispatch.

## Partial quality findings

Completed q1–q8 and the three natural pools permit descriptive analysis only. Under reference A, E−W0 mean nDCG@10 was +0.204 with bootstrap lower95 +0.113. Under reference B, mean was +0.058 with lower95 −0.0035, failing the registered development uncertainty gate.

Facet coverage did not improve versus the same-batch ordinal D* control under either reference: A remained 1.000 and B remained 0.977. Research still lost the reference-B grounding facet compared with W0. The broad “worthwhile source for this facet” judgments did not cause the intended recovery. Completing the stress group or shortening its request would not turn those observed quality failures into a supported candidate.

These are reused assistant labels and exposed development cases, not independent human gold or held-out evidence. Partial gains cannot override the terminal operational failure.

## Reporting correction

The frozen runner returns references as pool→reference→candidate, while its pure analysis module expects reference→pool→candidate. Neutral module tests supplied the correct shape, and fake transport tests did not invoke receipt-to-analysis integration; the pre-call qualification missed this mismatch.

The first [partial diagnostic](partial-quality-diagnostic.json) is retained as **invalid** because it used the untransposed mapping. The [corrected partial diagnostic](partial-quality-diagnostic-corrected.json) changes only the mapping shape; every annotation row, grade, facet, threshold, model answer and ranking remains unchanged. Its [reproduction script](partial_diagnostic.py.txt) reproduces the corrected report exactly without dispatching a request. An independent Luna reviewer verified that correction and its limited scope. The frozen qualified runner and manifest have not been altered.

Future runners need a real fake-receipt-to-analysis integration test asserting both references, all eight primary deltas and representative-only membership. This defect does not rescue the experiment; the HTTP 400 and observed quality failures stand independently.

## Disposition

Retain the registration, qualified inputs, all 36 attempts, raw responses, receipts, terminal stop and corrected diagnostic. Reject this candidate. Do not acquire fresh confirmation searches for it. A follow-up must explain new evidence and register a different, provider-aware decision contract before implementation or measurements; it must retain whole-pool comparison, reference separation, honest fallback and compatibility requirements. Follow-up #516 remains open. Brave usage remains 0/10.
