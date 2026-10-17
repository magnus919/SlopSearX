# Shared supervised HTTP charset selector

Component candidate; not full A05/A06 or issue 984 acceptance.

`ssx_infra::adapters::http_charset::select_http_encoding_bounded` accepts a
cloneable iterator of borrowed ordered `(name_bytes, value_bytes)` pairs,
`body_empty`, the existing operation `Budget`, and the existing checkpoint.
A05 can pass `pairs.iter().copied()`; A06 can adapt validated frame views without
allocating a pair vector. Validate the complete helper frame before calling.
The return is a closed static canonical codec label. No owned label, decoder
worker, or independent allowance escapes or is created.

The caller retains and charges raw transport/header/frame storage. All selector
scratch (joined decoded header, borrowed-span descriptor arrays, continuation
sort storage, unquoting, percent decoding, raw-unicode encoding, label scratch)
is charge-owning and drops before the static label returns. The same budget is
passed to A05's bounded lookup and strict parameter facade. No smaller header
policy is inferred; actual transport retained-capacity accounting remains a
separate integration obligation. Standard UTF8 validation, trimming, bounded
numeric-key comparison and in-place sort execute synchronously in the already
supervised helper; callback checks alone are not a hard process deadline.

Whole-empty-body bypass precedes headers and checkpoint evaluation. Header
encoding considers all raw names and values, then joins Content-Type duplicates
in order. Ordinary parameter precedence, quoted and angle unquoting, stable
numeric RFC2231 continuation order, zero suppressing unnumbered segments, and
irrelevant continuation integer-limit errors follow the source path. ASCII
name comparison is sufficient for the target `charset` and ASCII continuation
regex; Unicode header values are retained losslessly. No surrogate conversion
is performed. Strict Unicode/lookup failure retains original parameter text;
successful non-ASCII decoding selects UTF8. All other errors propagate.

`SelectorError::source_exception()` returns exact source exception classes;
its Display preserves source messages, including counted integer-limit digits.
`output_error()` exposes Resource/Deadline even when nested in the strict
facade. These are terminal operational failures and must never trigger an email
fallback or another HTTP attempt. Every remaining invariant error is fatal.
The helper's selector stage must remain distinguishable from subsequent body
codec errors when mapping these values to its parent protocol.

HTTPX's `Response.encoding` retains a recognized original alias. This API
intentionally returns that alias's canonical static registry identity after
source selection. Golden tests compare canonical identity only on source
success; source errors compare exact type/message separately and are never
canonicalized into success. Existing 45 captures stay unchanged. The additional
179 offline source controls retain raw header bytes, body-empty state, selected
alias or exact error, source registry name, Python version and source hashes.
The temporary Python oracle is evidence-generation tooling only, must enter the
shared temporary-source inventory, and retires at G08. There is no runtime
Python dependency.

Five deterministic component tests cover all 224 source controls, empty-before-
NUL/deadline precedence, allocation release, caller-retained charge, every
reached checkpoint forced to Deadline, and every scarce budget below observed
peak on strict-success and strict-UnicodeFailure-fallback paths. This does not
establish actual helper kill/reap, process resource containment, transport
header capacity, scholarly HTTP/factory activation, or publication-date proof.
