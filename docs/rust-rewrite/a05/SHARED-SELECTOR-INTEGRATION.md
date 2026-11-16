# Same-helper charset selector requirements

Status: implemented A05 candidate using parent-accepted A06 selector component
70a664c5. This document retains the original interface requirements below; the
following implementation receipt is not whole-adapter/resource acceptance.

Actual registered adapters send A05H version4/operation1: a fixed38-byte header
(count, serialized header bytes, body bytes, child allowance; all u64BE), followed
by length-prefixed raw name/value pairs and body bytes. Parent preflights checked
lengths before allocation, copies in4096-byte cooperative chunks, and reserves
frame, response, metadata and child allowance from one Operation root. The child
validates counts, lengths and strict EOF before a borrowed Clone iterator feeds
the unchanged A06 selector. No pair Vec, second worker or independent quota exists.

The F08 HeaderMap boundary preserves each name's duplicate value order; original
wire name casing and inter-name ordering are already discarded there. This
candidate does not reconstruct them or infer opaque HeaderValue backing capacity.
The shared transport-envelope gap is specified separately in
F08-TRANSPORT-ENVELOPE-PROPOSAL.md.

A05D tag5 distinguishes selector-stage NUL ValueError, internal codec RuntimeError,
C ssize_t OverflowError and integer-limit ValueError with checked digit count.
Legacy body-codec tag2 remains separate. Unknown codes/messages fail invariant;
Resource/Deadline stay operational and never source fallback. Parent rejects
selector codes without a header envelope and rejects impossible integer counts.
Google feedback remains after successful decode/challenge classification; DDG
ordinary selector exceptions retain source retry and failed-proxy feedback.

Qualification adds26 whole-original-adapter cases (complete outcomes and requests)
to the unchanged587 cases,179 source-selector boundary compositions, closed frame
negative controls and a12MiB header request cancellation/reap control. Captured
worker PID plus request-consumed handshake proves a live owned request, not exact
selector CPU phase entry; no injected hold is mislabeled native CPU work.
Parent and child header/frame/output capacity charging is scoped evidence only;
C-tree/getter/sanitizer/F08 backing and platform RSS/FD/GNU proof remain open.

The new offline oracle http_charset_pipeline.py is manual temporary evidence,
never invoked by native mandatory gates; F01 registration and G08 retirement
remain required in the final inventory union.

A05 needs one synchronous, charged, shared selector callable inside its existing
supervised helper before bounded body decoding. Its input is borrowed ordered
raw header name/value byte pairs and the body-empty predicate. The transport
HeaderMap and its original byte capacities remain owned/charged by the parent;
a closed length-prefixed helper frame carries all pairs with checked counts and
lengths. The child must validate the complete frame before selecting or decoding.
Header storage, decoded parameter scratch and body storage share the reserved
child allowance. No second worker or independent quota is introduced.

The selector should return a static canonical codec label or a typed fatal error.
It must use the shared bounded lookup and strict-to-ASCII parameter facade;
Resource/Deadline are never email fallback. Preserve whole-empty-body bypass,
ordered duplicate/header semantics, quoted parameters, RFC2231 continuations,
original-label strict-decode fallback and NUL ValueError precedence according to
the actual HTTPX source corpus. Unknown final labels select UTF8; successful
non-ASCII parameter decoding has source rejection behavior. The public interface
must not expose an uncharged owned String or convert lone surrogates lossily.

A05 retains the existing operation lease across selection, decode, feedback,
parse and projection. Google feedback still occurs after successful decode and
challenge classification, before HTML parsing. A selector error must preserve
its exact stage, diagnostic and ordinary-versus-terminal disposition; the codec
error tags currently used by A05D are not a license to collapse new selector
ValueError/RuntimeError/OverflowError into one category.

No global header-size restriction is inferred from the HTTP request query
policy. Actual F08 accepted header count/bytes and parent/frame overlap require
separate measured accounting. Any new finite resource refusal uses the approved
whole-operation resource error, not a semantic header truncation policy.
