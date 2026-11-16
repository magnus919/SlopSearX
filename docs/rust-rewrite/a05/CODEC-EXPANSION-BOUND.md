# Native HTTP text codepoint expansion bound

Implementation argument for review, not an RSS/CPU or complete compatibility
claim. The native gate separately checks all117 canonical policy entries and
455 labels. Of117 entries, seven nontext transforms and idna/undefined reject
HTTPX's errors=replace interface;108 applicable text decoders remain. Empty
response bodies short-circuit before any rejected decoder is entered.

For every applicable decoder and body of B bytes, the completed decoded string
has at most B codepoints. Surrogates count individually until the lxml boundary.
This is a consumption argument over decoder actions, not an inference from the
largest captured example. Unicode replacement characters count as one point.
Discarded signatures/shift sequences consume bytes without increasing output.

| Family | Canonical count | Consumption argument |
| --- | ---: | --- |
| Generated single-byte tables |69|Each consumed byte emits exactly one mapped point or U+FFFD. No table entry is a sequence.|
| ASCII, charmap, ISO8859-1 |3|One point per byte, including ASCII errors=replace.|
| UTF8, UTF8-SIG |2|Each scalar consumes1–4 bytes; every invalid byte/consumed error span emits at most one replacement. Signature removal and incomplete signature flush add no points.|
| UTF16/LE/BE |3|Each retained unit consumes two bytes; a pair emits one point. Errors consume at least one byte per replacement, including final odd byte and high-surrogate plus odd-tail consumption. Missing required BOM raises instead of emitting.|
| UTF32/LE/BE |3|A scalar or invalid full unit consumes four bytes; a trailing error consumes1–3 bytes for one replacement. Missing required BOM raises.|
| UTF7 |1|Direct bytes emit at most one point each. Within a shifted component, m base64 bytes provide6m bits and emit at most floor(6m/16) UTF16 units; pairing reduces count. A residual malformed-unit replacement is charged to its opening plus byte, so emitted units plus replacement are at most m+1. A terminating byte reprocessed in direct mode is charged to that distinct byte. Plus-minus emits one point from two bytes; lone opening plus at final flush emits none.|
| Stateless CJK, including GB18030 |16|Single-byte actions emit one point. Valid pairs emit at most two points from two bytes (JIS/HKSCS decompositions included); valid triples, quadruples and Korean eight-byte makeup emit at most two points. An error action emits one replacement for at least one consumed byte; reprocessed lookahead is not charged until consumed. GB18030 range synthesis emits one scalar, with no normalization expansion.|
| ISO2022 variants |7|A designation consumes3/4/6 bytes and emits0 points when accepted or1 replacement when rejected. Shift consumes1/emits0. ASCII/control/through actions consume1/emit1. Roman/kana consume1/emit1. Double-byte tables consume2/emit at most2 (actual table maxima inventoried). SS2 consumes3/emits1. Incomplete/error actions consume at least1/emit1. Conditional RuntimeError emits no completed result.|
| HZ |1|Direct, shifted pair, escape and error actions emit at most one point per consumed byte; reprocessed second bytes are distinct subsequent consumption.|
| raw_unicode_escape |1|Ordinary bytes emit one point; recognized escapes emit one point from6/10 bytes; unknown escapes preserve at most their consumed bytes. Malformed escape replacement consumes at least one byte.|
| unicode_escape |1|Ordinary bytes emit one point. Fixed/octal escapes emit at most one point from at least two bytes. Named escapes resolve to a single codepoint; named sequences are rejected by the source API. Unknown escapes preserve their two bytes; malformed replacement consumes at least one byte.|
| punycode |1|The basic prefix copies each source byte once. Every insertion in the encoded suffix requires at least one new generalized-integer byte and inserts one codepoint; large/invalid values change that point or terminate decoding, never insert a sequence.|
| Total |108|The cases partition applicable canonical codecs.|

The implementation must retain this progress invariant when changing any decoder;
a test passing for finite inputs does not replace review of consumption actions.
The ports retain source files/hashes in package attribution; the source oracle
also exercises full pair/state spaces and malformed/stateful transitions.

Consequences: accepted F08 body limit B<=32MiB permits at most32MiB u32 values.
Fixed-endian IPC and the widest lxml representation need at most4B=128MiB bytes.
All frame counts/conversions and count*4 use checked arithmetic before reserve
or C conversion.128MiB is below source lxml's INT_MAX buffer-length guard. Python
PEP393 minimal-kind invariants select width1/2/4 from the maximum retained point;
they do not imply a UTF8 shortcut for scalar-only non-ASCII strings. Embedded
NUL, XML declarations and surrogates remain in the representation selection.

This bound covers decoded point backing storage and encoded width only. Input,
IPC, decoded vectors, C trees, row strings and serialization can coexist; each
requires its own charge in the approved256MiB operation budget. It does not
justify the old input*256 response multiplier or bound libxml2 allocations.
Native checked-frame/negative tests, supervised cancellation and OS resource
qualification remain independent requirements.

[Canonical/native-file/table inventory](codec-family-bound-evidence.json) ties
every applicable canonical name and all its policy aliases to current native file
hashes and actual table record maxima. The offline bound_inventory.py generator
checks all69 table lengths/ranges and every CJK/ISO pair/triple record, failing
on output length greater than consumed width. Algorithm-action source review
remains necessary; this inventory is not a claim that counting labels proves it.
