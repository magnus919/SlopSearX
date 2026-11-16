# Actual rows helper integration

Status: candidate, not full A05 resource/platform acceptance. Both registered
HTML adapters now use the native child for HTML tree construction, CSS selection
and row extraction. No tree/JSON IPC reaches their parent path. Legacy version1
ParserPool APIs remain for existing compatibility tests; they are not these
adapters' active parse path.

A05H version3 operation2 has a fixed32-byte header: magic/version/operation,
trusted layout, RowLimit tag, exact f64 limitbits, codepoint count and child
allowance. Non-number limit tags require zero payloadbits; invalid source types
retain their four distinct tags and raise only on the first selected row. Raw
query bytes are absent. The parent precharges codepoint payload, fixed header,
lease metadata and reply ceiling; the child allowance and reply ceiling each
reserve half the remaining SAME search root, with no independent256MiB root.
Input framing yields every1024points and retains ownership during cancellation.

The child uses the existing parser/extraction algorithms, passes an empty query
and emits a title-fallback flag for image rows that need the caller query. The
version3 A05R response retains raw URL/img_src and separately carries the existing
storage::build_media sanitized URL/thumbnail/source fields. Optional missing and
present-empty values are distinct. Non-image layouts cannot carry image/media
flags; media requires an image field. Unknown tags/counts/overflow/truncation,
invalidUTF8 and trailingbytes fail before publication. Parse failure diagnostics
are a closed separate A05E stage; native Resource is terminal and cannot become
an ordinary source parse failure or trigger DDG fallback.

The parent validates fields in <=4096-byte UTF8 pieces without unchecked casts,
yielding between pieces/rows. It copies those pieces into charged Strings with
fixed exact capacities. Caller-query fallback duplicates are charged before copy.
The private produced batch retains vector/field charges and the OperationArc
through result consumption until its named AdapterResponse handoff. Partial
batches never publish. G02 owns public response retention, clones and cache/flight
lifetimes after that handoff. This is not a storage-cap-based response bound.

Qualification includes all587 original COMPLETE outcomes/requests, image fields
and longquery cases; strict versioned image optional-field/malformed/oversized
length controls; produced batch live-charge/handoff/refusal/abort controls; and
real deep/wide native parser/CSS deadline/cancellation with PID/permit/root drain.
The decoder's150ms end-to-end deadline may validly expire before consumption or
admission: observed counts are recorded0..1, not asserted as codec-phase entry.
A separate fixture consumes the complete request and holds its process under a
finite absolute deadline, proving held-handoff ownership/reap; it is explicitly
NOT native CPU evidence. Actual native non-emitting codec cancellation remains
separate. No serial-only test workaround or production timeout change is used.

Shared A06 selector module and its224source controls are copied under exact
parent-approved guards. They are NOT activated in this rows slice: current header
selection remains provisional until a raw-header decode frame is integrated.

Open A05 requirements remain concrete in TREE-ALLOCATION-CLOSURE-PROPOSAL.md:
C parser/global allocations, codepoint-width encoding buffer, mutable-wrapper
NodeMap/getter temporaries, Rust flat tree/CSS/dedup collections and media sanitizer
transient heap still escape the child ledger. Native numeric constant-constructor
scratch, F08/header/runtime allocator overhead and actual aggregate RSS/FD/GNU
closure are also not proved by logical field charges. New immutable facade/private
allocator implementation is authorized only as a separately reviewed dependency
candidate before adapter activation. No provider parity or final acceptance follows
from this pipeline's source corpus and process tests.
