# Shared HTTP charset selector design

A06 owns the prospective additive selector; A05 owns the codec leaf. Neither
current DDG split-on-semicolon code nor the candidate donor implies accepted
source label selection. Full implementation and qualification remain pending.
`source-charset.json` contains45 actual HTTPX/email selector observations and
complete arXiv outcomes from private fixtures. Existing source corpora stay intact.

The required pipeline follows inspected pinned HTTPX and CPython source:

1. Determine one Headers encoding over every raw header key/value: ASCII if all
   fit, otherwise UTF8 if all fit, otherwise Latin1. Decode all Content-Type
   duplicates with that encoding and join them with comma-space in original order.
2. Parse only the source email parameter path: semicolon splitting respects its
   double-quote/backslash rule; names/values use Python whitespace rules; bare
   parameters have empty value. The first parameter is preserved specially.
3. Decode RFC2231 continuations, preserving ordinary-parameter precedence,
   original continuation-name case, numeric ordering, duplicate indices and the
   rule that numbered zero suppresses an unnumbered segment. Irrelevant matching
   continuation parameters can still raise the source4300-digit integer error.
4. The first charset parameter wins. Quoted/angle values use source unquoting.
   Extended values percent-decode Latin1 octets, split charset/language/value,
   encode via raw-unicode-escape, then invoke NONINCREMENTAL STRICT decoding.
   LookupError/UnicodeError fall back to original parameter text. Other errors
   propagate. A successful non-ASCII final charset is rejected.
5. Lower ASCII charset, check registered encoding with source lookup behavior,
   then unknown/missing/empty labels fall back to UTF8. Embedded NUL lookup can
   raise ValueError and must not be discarded by alias normalization. Empty
   response bodies short-circuit text decoding before evaluating this pipeline.

The already locked maintained `mime`0.3.17 was tested before proposing custom
compatibility code. It rejects source-valid `garbage; charset=utf-16` and bare
charset parameters, and does not resolve charset* continuations into charset.
The isolated proof is retained in local review evidence; no dependency was added.
The candidate is a narrow selector, not a general MIME/email parser.

The HTTP body facade cannot provide step4 by substituting errors=replace.
The captured `charset*=utf-7''utf-7%2B%21` must yield original ASCII `utf-7+!`
after a strict failure. Replace decoding produces U+FFFD and would choose UTF8.
Furthermore strict nonincremental UTF16/32 permit native-endian BOMless input,
while body incremental decoding requires BOM. UTF8SIG incomplete input, strict
IDNA2003/NamePrep and the seven nontext codecs need separate source outcomes.
A05 is assessing reusable decoder error-mode tracking and a strict facade; no
error inference from U+FFFD is valid. This obligation is not permanently excluded.

Proposed strict seam distinguishes decoded codepoints, UnicodeFailure,
LookupFailure and other source exceptions. The selector needs complete decoded
ASCII only on success, but must not turn valid-nonASCII and failure into one
state because failure can select original ASCII. Codec ownership stays with A05;
no parallel decoder edits or Python runtime fallback are permitted.

A bounded selector must preflight aggregate header bytes/work before copying,
check length arithmetic and continuation count before reserve, and yield during
large scans. F08 currently bounds bodies/requests but does not expose a specific
header byte policy in Bounds; this gap must be resolved explicitly with the
integration owner. A post-allocation selector check alone cannot prove transport
header allocation bounds. No arbitrary smaller header capacity is claimed as
source-derived or silently imposed by this design.

## Measured HTTP/1 transport buffering

The pinned source httpcore HTTP/1 reader requests at most65,536bytes and passes
`max_incomplete_event_size=102,400` to h11. This is an incomplete-event threshold,
not a fixed total-header cap. Actual controlled reads accepted131,072bytes with
two65,536byte reads, while131,073bytes failed before a third read. A deliberately
fragmented65,536+36,864+65,536 sequence accepted a complete167,936byte event;
167,937bytes failed before another read. Direct h11 accepted a complete1MiB event
supplied in one call, confirming that treating102,400as a whole-event limit would
misstate the source. These are HTTP/1 controls, not HTTP/2 qualification.

The source implementation identities are httpcore `_async/http11.py`
`faa33d6d5ecf8d2405e6fc6cdfbf9e5173881706c88cf7196cd962b97f53b41c`
and h11 `_connection.py`
`93d61155fea4a19a9bb6d056dfac5259a26959d6706bec50554f401c4a3d0ee2`.
The paired actual-source probe/result remain in the review packet at
`/private/tmp/a06-header-limits.py` and
`/private/tmp/a06-header-limits-source.json`.

Locked native reqwest0.13.5/hyper1.12.0 has a417,792byte default HTTP/1 buffer
threshold and100header slots. Real F08 loopback requests accepted500,000bytes
but rejected524,288bytes and101fields. Complete parsing precedes the incomplete
buffer threshold check, and the read path can expose the full BytesMut spare
capacity. Consequently417,792must not be reported as an absolute allocation cap.
The paired native probe/log are `/private/tmp/a06-native-header-probe.rs` and
`/private/tmp/a06-native-header-probe.log`. Derivation of retained capacity across
connection reuse and selector/frame allocation accounting remains open; these
measurements do not establish a final memory bound or authorize a smaller cap.
