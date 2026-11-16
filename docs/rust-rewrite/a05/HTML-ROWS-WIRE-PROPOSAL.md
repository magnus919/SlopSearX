# Private HTML row wire proposal

Status: concrete next-version schema; implementation and qualification pending.
This supersedes the provisional serde JSON worker output envelope when adopted.
It does not alter SearchResult, shared JSON or storage serialization limits.

Retain outer common ProcessPool framing: u64BE payload length, exactly that many
bytes, exact EOF and successful child exit. Existing supervisor ownership,
deadline, sender-drop kill/reap and uncertain-cleanup failure remain unchanged.
A child allocation allowance is reserved from the original parent operation
ledger through verified reap; simultaneously live parent input/frame/output
charges are additional charges in that same ledger, never another256MiB grant.

Inner request begins ASCII A05H, version2, operation tag. Operation1 is raw HTTP
text decoding/detection with ordered raw header pairs using A06's shared selector.
The whole-body empty shortcut occurs before selector/lookup. Operation2 is
parse-and-extract: engine/layout tag (DDG HTML, DDG lite, DDG image, Google), closed
RowLimit tag and u32BE codepoints. Number row limit carries f64 bits; Unlimited
has no payload; invalid type tag is one of None/string/list/dict. Invalid type is
not rejected at decode: the existing source comparison raises only upon the
first selected row. Preserve that timing even if the row later lacks a valid URL.
All count*width and aggregate arithmetic is checked before allocation/conversion;
all codepoints must be <=10FFFF. The helper receives no query string.

Response tags distinguish decoded text, rows, and source-stage failure. Failures
retain Decode/Parse/Extract stage plus exact supported diagnostic; resource
failure has a separate tag and cannot become Unicode/unknown-label fallback.
Malformed helper frames are transport/protocol errors, not valid empty results.
Detector flags have explicit defined bits; unknown flag bits/tags/versions fail
closed. Google success feedback remains after successful decode/challenge checks
and before parse/extraction. The original search lease spans both exchanges.

Rows response carries operation/layout tag then u64BE row count. Each row has:

1. i64BE position, preserving source candidate index versus DDG-lite accepted index.
2. one flags byte: bit0 means DDG-image query-title fallback; bit1 means img_src present.
3. u64BE length plus UTF8 bytes for URL, title and content, in that order.
4. optional u64BE length plus UTF8 img_src bytes when bit1 is set.

Only those two flags are defined. Query fallback must be false outside DDG-image
layout. Parent supplies engine/category from the trusted operation tag, applies
query title only when bit0 is set, and builds media through accepted build_media
while retaining raw URL/img_src. Empty title cannot substitute for the fallback
flag: a genuine empty source title has different semantics. All fields are
ordinary post-lxml UTF8, not normalized/trimmed again by the protocol.

Provide allocation-free encoded-size counting with checked additions, then write
into one precharged exact buffer. The receiver first validates count/field sizes
against actual remaining frame bytes and its reserved allowance; it must not
reserve row_count Vec elements merely because the frame announces that count.
A borrowed row iterator validates UTF8/flags and exact end before projection.
Projection allocations and any query fallback duplication acquire charges before
copying and retain those charges with returned rows. A malformed later row fails
the entire response rather than yielding a prefix. Row fields do not pass through
the storage serializer, avoiding its unrelated4MiB cap and panic path.

Qualification requires all587 existing complete outcomes/requests, selected-row
invalid-type timing, malformed/truncated/trailing/count-overflow/UTF8/flag cases,
long query fallback and empty-title controls, exact source feedback ordering,
and actual expensive CSS/row extraction cancellation/deadline with PID reap and
permit/charge release. No structural multiplier or result count alone proves
these properties.
