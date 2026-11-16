# Raw Atom same-helper integration candidate

This slice activates raw-header charset selection, bounded replacement decoding,
Expat extraction and shared ProducedBatch projection through the actual existing
`ssx-atom-parser` binary. Actual adapter HTTP, registry factories, OpenAlex issue
984, transport header capacity and GNU qualification remain pending.

`AtomPool::begin_raw(existing_budget, absolute_deadline)` acquires the existing
single-operation admission before the future adapter HTTP call. `RawOperation`
retains that operation through framing, helper exchange and parent projection.
Its budget accessor lets the caller retain its own raw HTTP buffer charges; this
slice does not establish transport preallocation or HeaderMap capacity bounds.
There is no new process pool, concurrency increase or independent codec quota.
The old decoded-text parse seam remains for legacy regression comparison only;
actual native adapter wiring must select the new raw operation.

The existing helper performs worker OS preparation once, reads a four-byte stack
prefix and dispatches SSXS v2 to the new handler, preserving the old u64-length
protocol for legacy tests. The new handler validates the complete frame before
selection, then borrows header pairs without reconstructing a vector. One child
Budget mirrors the parent's reserved 256 MiB allowance. Raw frame, selector scratch,
codepoint decode output, scalar UTF8 conversion and unchanged 64 MiB Expat/native+
retained quota are charged simultaneously when they overlap. The Expat reserve
precedes parse and reduces to its reported retained capacity only after native
parse returns. Decoded/input buffers drop before reply construction. Retained
entries remain charged while the legacy Atom wire Vec's entire upper bound is
reserved and allocated; entries drop before that output is written.

Parent ProcessPool receives a payload owner plus a lease containing the operation,
mirrored child allowance and shared output reservation. Unknown disposal retains
that lease through the existing ProcessPool failure path. Verified reap releases
the child mirror; the caller keeps output reservation through parent wire decode.
A 64 MiB entry reservation precedes parent decoding while reply bytes remain live.
After output disposal, direct borrowed Entry fields and charged author descriptors
feed shared `publication_budgeted` and ProducedBatch. No intermediate SourceText
or SourceValue copies are created. Temporary normalized abstract/URL/snippet
buffers remain charged while output copies are built. The existing named
`into_adapter_response` boundary moves owned results without clones; public
response retention and subsequent copies remain the explicit G02 obligation.

Closed SA01 replies distinguish resource/deadline/defect stages and selector
ValueError/RuntimeError/OverflowError, body Codec errors and UTF8 UnicodeEncodeError.
Error payloads are UTF8-validated and limited to 512 bytes before parent copying.
Whole-empty bodies bypass selection. No surrogate is replaced or sanitized:
conversion reports the actual source UnicodeEncodeError span/message. Eight
new complete offline arXiv+HTTPX captures bind those cases. The temporary oracle
is developer evidence only and must be inventoried and retired at G08.

The helper drains finite parent-owned input with fixed stack scratch after early
refusal so BrokenPipe cannot hide a typed result. It imposes no guessed header
size policy. Parent absolute ProcessPool supervision remains authoritative for
blocking reads/non-emitting native scans. Helper checkpoints follow the accepted
supervised decoder seam; their no-op callback does not claim a child wall-clock
check or native CPU proof. Actual OS/RSS/transport and GNU envelopes stay open.

Qualification on installed Rust 1.99 using existing target and offline cache:

- 269 complete source outcomes through the actual raw helper: charset 45,
 Unicode boundaries 8, XML 24, initial 16, regressions 10, shared 1, grammar 165.
 Two previously approved Expat upgrade declaration rejections stay explicit.
- 7 actual native tests pass: source replay, pre-HTTP admission, pre-spawn quota,
 actual PID cancellation/reap, actual post-reap parent projection cancellation,
 parent Tokio deadline with actual PID and a 5 MiB produced abstract.
- 8 existing legacy helper tests pass, including 216 historical parser outcomes
 and existing resource/deep/large/cancellation controls.
- 18 scholarly unit tests pass, including closed reply stages, same-budget
 parse/output overlap and source-surrogate errors; strict all-target Clippy passes.
- 5 MiB abstract test reports parent requested-reservation peak 351273354 bytes
 under 536870912, including 10485976 bytes of caller input capacity. This includes
 the conservative child mirror and is not RSS or summed parent+child allocation.

The paused-Tokio test advances only the parent's Tokio supervisor after an actual
PID handshake. Its frame std::Instant remains future; it is not evidence of child
std::Instant expiry or native CPU timing. Existing cancellation/projection tests
use real clocks. No production timer or dependency feature changed.

Initial source test mistakenly selected an unreached declared 200 after a refused
redirect. Its failure log is preserved; final selection uses the actual captured
request count. All source observations remain unchanged. No provider/production
calls, push, merge or issue closure occurred for this slice.
