# Bounded decoder child integration

Status: candidate for parent review. This integrates the accepted charged body
codec dispatcher into the actual DDG/Google decoder process. It does not close
the subsequent parser phase or the aggregate resource qualification.

The closed A05H version 2 decode request carries only a canonical codec, raw
body length, and a child allowance reserved from the same parent operation
budget. The child validates these fields before allocating body storage. Raw
bytes and decoded codepoints share that child budget. Challenge detection uses
borrowed codepoints and stack KMP scratch. The response streams a versioned
A05D frame; the parent validates every tag, flag, count, codepoint and EOF before
allocating charged output. Surrogates are retained. The response bound is
15 + 4 * raw-body bytes, using the codec consumption bound; it is not an
independent resource allowance.

The parent charges its response-body capacity, copied worker payload, response
frame ceiling, request header and lease metadata concurrently with the reserved
child allowance. The common process supervisor retains payload and lease until
verified reap. Successful child completion releases the child reservation before
parent codepoint projection. Cancellation and uncertain cleanup cannot reopen
that reservation early. Resource and deadline errors remain terminal, and codec
errors retain ordinary source fallback behavior. Cleanup uncertainty is an
internal invariant failure rather than provider failure feedback.

Actual adversarial controls use the rebuilt native helper: a quota that admits
raw bytes but refuses decoded output; repeated ISO-2022 state transitions with
no output followed by a truncated escape under a deadline; and cancellation
while that native operation is live. They assert request consumption, one worker,
PID disappearance, permit release and zero retained logical charge. These prove
end-to-end supervised cleanup, not the exact decoder instruction executing at
the instant of cancellation or a wall-clock bound for individual cooperative
checkpoints. The helper timer covers scans that emit no checkpoints.

Open integration work and decisions:

- Consume A06's reviewed shared raw-header/RFC2231 selector inside this same
  admitted helper; the current adapter header split is provisional. No second
  decoder or selector implementation is intended.
- Replace legacy version 1 tree/JSON IPC with the approved rows wire and child
  CSS extraction while preserving Google feedback between decoding and parsing.
  Legacy tree allocations, C parser allocations, CSS and row projection are not
  bounded by this decoder-phase accounting.
- Consume the reviewed shared counting repr and ProducedBatch interfaces for
  charged configuration formatting and explicit result ownership transfer.
  Current Google repr allocation is not covered by the decoder ledger.
- F08 allocates raw response bytes before the adapter charges their capacity;
  transport decoding/transient buffers and raw headers are excluded here.
- Fixed Rust/Tokio/process overhead, allocator/RSS, descriptors, GNU target
  closure and final native resource/security qualification remain open.
- Per-produced-batch and operation accounting do not establish a global bound
  on retained canonical responses or downstream clones; G02 owns that workload.

The original 587 complete source outcomes and request records remain unchanged.
No provider call or Python runtime is introduced. Earlier component and failed
qualification receipts remain historical evidence rather than being overwritten.
