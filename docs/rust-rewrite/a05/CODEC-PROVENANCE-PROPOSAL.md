# A05 native codec provenance proposal — review required

No runtime Python and no new dependency/license policy is adopted by this proposal.

The existing source-codec-policy.json is an immutable observation of Python3.14.7/HTTPX0.28.1/lxml6.1.1. Retain the original455label/117canonical inventory and all errors;108applicable text codecs,7nontext transformations rejected for nonempty HTTPX input, idna/undefined rejected text paths, and empty-body short circuit remain separate. Do not infer equivalence from a codec label shared with encoding_rs.

Proposed generated-data unit: a single manifest containing oracle version, Python build/source identity, stdlib source-file hashes, Unicode database version, generated-file SHA256/Git blobs, canonical sorted labels and aliases, decoder family, mapping origin/license and exact observed invalid-sequence/error policies. Exporter must verify every retained observed row rather than overwrite expectations. Generate once as temporary offline migration tooling, inventory under G08, and execute mandatory Rust gates without Python. Historical corpus files stay byte-exact.

Family1:69 single-byte tables and3simple codecs. For each decoder, capture all256single-byte values, missing mappings and replacement behavior, plus composed byte sequences. Mapundefined entries to the measured replacement semantics, never a generic WHATWGtable. Provenance: pinned CPython encodings modules and factual source-decoder observations, explicitly treated conservatively as PSF-derived data pending review. Do not relabel generated tables as MIT simply because the generator is ours. Native loop implementation may be independently authored Rust but table notices remain.

Family2:9Unicodealgorithms. Prefer independently authored standards-based UTF8/16/32/7 algorithms with exhaustive surrogate/BOM/odd-tail/invalid-prefix source differential fixtures. Native API must represent exact failures as well as Unicode output. SourcePython UTF8SIG and BOM-required UTF16/32 state behavior remain explicit. encoding_rs reuse only after completeinvalid/replacement and alias comparisons; preserve its package license.

Family3:24CJK/multibyte/stateful decoders. Inventory every CPython generated mapping/state machine file and its notices, not merely top-level PSF license. Candidate plan is a native Rust port of pinned state machines/mapping data with notices; no Python CAPI or linking CPython. SeparateJIS2004/JISX0213,CP932/ShiftJIS,CP949/EUCKR,HZ/ISO2022variants instead of assuming commonaliases. Include all state transitions/truncations/resets and source-specific replacement grouping. No finite fixture count alone establishes general correctness; code and state-table review needed.

Family4:punycode,raw_unicode_escape,unicode_escape. Punycode can be standards-based native Rust with source error differential tests. Escape parsers require exact malformed/truncated handling, octal/hex/Unicode limits and \N{name}; source Unicode-name database version/aliases/namedsequences must be pinned. Generated name data requires CPython/Unicode upstream notices and license provenance. Include syntactically validtext that produces noASCIIHTML, not just Caféexamples.

License decision requested: narrowly approve the exact CPython-derived data/algorithm files under their retained PSF-2.0 and any file-specific licenses, after exact source tree/version/checksum and notice inventory. Prefer package-specific cargo-deny clarification/exception for a dedicated codec data component rather than adding a broad blanket license exemption. No license text or generated port has been adopted yet; exact complete source manifest is next evidence, not alreadypassed.

Native gate design: every registered alias resolution; all empty/rejected/unknown-label distinctions; everybyte per singlebytecodec; fullvalidmapping/state machine vectors; invalid/truncated/BOM sequences; streaming chunk partitions where applicable; whole186adaptersearchrequests/outcomes; independently generated additionalmatrix; deliberatealias/table/error mutations must fail. Byteexpansion bounds derive from accepted32MiBF08body plus eachdecoder'smaximumUTF8expansion; no fixed16MiBhelpercap. Some escape/malformed state errors require a Result rather than an assumed total replacement decoder. Measure per-familymemory/CPU and bound helper lifecycle through existingsearchdeadline.

This is a complete implementation strategy with explicit unimplemented families, not a claim that codec porting is complete or that a maintained third-party crate supplies it. Parent review of exactprovenance/licensing precedes copyingderivedtables/code.

## Inventory count reconciliation

The retained inventory has 455 accepted labels/aliases resolving to 117 canonical
codec names. Of those, seven are non-text transformations rejected by HTTPX for
nonempty bodies: base64, bz2, hex, quopri, rot-13, uu and zlib. The remaining 110
are registered text codecs; idna and undefined reject the selected errors=replace
path, leaving 108 applicable text decoder families. The 11 errors observed for
bytes0..255 include these nine rejected paths plus conditional missing-BOM errors
for UTF-16 and UTF-32; those two remain in the 108 applicable codecs. Empty bodies
short-circuit before codec registry use, including all nine rejected paths. These
are different classifications of the same fixed scope, not added decoder scope.
