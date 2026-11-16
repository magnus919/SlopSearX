# Fixed Unicode conversion and NUL compatibility proposal

Status: parent approved implementation of the bounded candidate; not acceptance.
Workspace now enables platform-only ICONV and the exact NUL compatibility patch.
No provider qualification or A05 acceptance is claimed.

## Evidence

The pinned lxml parser.pxi:1074–1140 selects ISO-8859-1, native UTF-16, or
native UTF-32 from Python storage kind; it does not pass UTF-8 for every string.
Both libxml2 2.14.6 and 2.15.4 omit UTF-32 handlers when ICONV and ICU are OFF.
The pinned lxml build reports iconv enabled. Reconstructing minimal Python kind
is supported by CPython Include/cpython/unicodeobject.h:124–145 and
Objects/unicodeobject.c:747–762, not merely empirical maximum-codepoint tests.

The isolated builds with ICONV_ON produce identical raw tree/text/attribute
observations for all 44 retained surrogate cases. The comparison stores UTF-8
hex directly from C nodes; it does not compare serializer output or normalize
values. `surrogate-iconv-version-comparison.json` retains every observation.
`oracle/libxml-compare.c` is the native comparison harness.

The 180 source/NUL cases in `nul-version-observations.json` span three adapter
modes, four storage widths, and 15 positions/encoding controls. There are 56
C-tree differences between 2.14.6 and 2.15.4, concentrated in normal-data NUL
handling. All meta charset controls give equal trees across versions, including
UTF-8, Latin-1, UTF-16LE, UTF-32LE and unknown labels with fixed input encoding.
These observations do not establish every possible declaration behavior.

## Proposed narrow amendments

Enable ICONV only, retaining the pinned vendored libxml2 archive and all other
feature settings, loader denial, NONET and no-system-libxml fallback. On macOS,
the actual pkg-config closure is `-lxml2 -liconv -lm`; otool confirms only the
platform libiconv.2.dylib and libSystem dependencies. On Linux GNU the intended
converter is glibc iconv, with CMake proving built-in availability. GNU x86_64
and aarch64 builds, actual symbol/link closure and converter behavior remain
mandatory CI qualification; no system-library or alternative converter fallback
is proposed. The platform SDK/libc is the accepted toolchain/runtime substrate,
not a newly downloaded iconv implementation. The build must reject an unexpected
external iconv prefix/library and retain actual CMake feature receipts.

Restore only HTMLparser.c:3235–3254 normal-data NUL replacement to U+FFFD,
matching 2.14.6. In 2.15.4 this branch explicitly strips NUL to follow HTML5;
non-normal modes already replace it. A dependency-internal compatibility patch
would preserve original storage selection, attribute/comment processing and all
other 2.15.4 fixes. No global NUL preprocessing, input-kind rewriting, or blanket
Document-empty success conversion is proposed. Expanded raw-text/tag/doctype
position probes and exact patch review are required before adoption.

## Pending

Exact full patch review, platform feature/symbol receipts,
complete parser corpus replay, negative external-resource tests, and all-target
helper cancellation/resource qualification. Both changes are implemented but qualification remains incomplete.

Exact compatibility patch SHA256: `a0817f232192bc74082dd40c39334312e0102c5a20d956e654e6eab5dba35b68`.
