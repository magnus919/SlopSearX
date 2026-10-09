# Selector resource collection

The resource collector now consumes the selector executor's terminal inventory, result receipts and response archives. It checks the externally supplied terminal digest against both the supplied bytes and the private file, then verifies stage, protocol, source revision, source closure, operation order and every result/archive binding. W0 operations require the original V1 parser and successful original-V1 status; candidate operations require the coverage parser with consistent selection status.

Known token usage must also match parsing of the exact archived response. Caller-provided operation rows cannot supply token counts, elapsed time or serialization evidence. Missing terminal proof leaves these measurements unknown; a changed or misbound artifact stops collection. A mock regression re-seals altered usage claims while retaining the original response and confirms rejection.

The collected selector duration is the maximum measured per-call time through its result/correction fsync. Dispatch, retry and concurrency observations cover the registered schedule on one study ledger. Stage-wide elapsed time remains unknown pending a separate observation taken after the final coordinator inventory fsync; this change does not invent that measurement or claim an admission.

Validation: 260 coverage/intent-ranking tests and 9 subtests pass locally. The consumer fixture exercises both frozen original-V1 replay and candidate coverage execution through mocked HTTP responses. The coordinator ignores fabricated token rows when archived terminal proof is absent. No provider calls, source acquisition, scientific assessment, deployment or quality claim are part of this preparation.
