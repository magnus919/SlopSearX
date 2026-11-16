# Bounded strict parameter candidate

Status: proposed additive interface and owned implementation slice; not selector
or helper adoption. Body dispatcher acceptance does not cover this path.

Add `strict_parameter_ascii_bounded(bytes, label, &Budget, &checkpoint) ->
Result<BoundedStrictAsciiOutcome, BoundedParameterError>`. Existing legacy strict
API remains unchanged. BoundedParameterError variants are LookupFailure,
ValueError(&'static str), RuntimeError(&'static str), OverflowError(&'static str),
Output(BoundedTextError), RegistryInvariant. Static source diagnostics avoid an
uncovered error String. Resource and Deadline never become UnicodeFailure or
email original-label fallback.

Preserve NUL-label validation before empty-parameter short circuit, then charged
registry lookup and source strict mode. Whole-response empty body still bypasses
this selector entirely in its caller. Keep strict UTF16/32's optional BOM/native
endianness and exact invalid-stream behavior separate from replacement body mode.
Unknown/nontext LookupFailure, undefined UnicodeFailure, runtime ISO and propagated
Punycode/IDNA overflow stay distinct. Validate every IDNA label before NonAscii.

A private ASCII collector keeps total decoded length separately from retained
ASCII bytes, accepts fallible emission and checked insertion, and reserves finite
input-derived capacity under the same Budget. On first nonASCII point it can drop
its ASCII buffer but continues complete source validation. A successful ASCII
value moves its charged OwnedVec<u8> into BudgetAscii; no bare Vec or uncharged
String escapes. Change only BudgetAscii's private storage representation from
OwnedString to OwnedVec<u8>, keeping public borrowed string/charge methods and
source ASCII validation. BoundedText/ASCII ownership contract remains intact.

IDNA's mapping, decomposition, canonical ordering/composition, bidi checks and
roundtrip encoding share the existing algorithm through private legacy/charged
Scratch<T:Copy> buffers backed by Vec/OwnedVec. Legacy tests use legacy mode;
bounded mode never extracts a Vec. All growth, simultaneous mapped/decomposed
buffers, returned label bytes, Punycode decoded points and numeric clones retain
independent charges, including old+new reallocation overlap. Private source
1024-byte label rule and verified B2max4/NFKDmax18 (72-point upper bound) remain
source rules/evidence, not permission to allocate outside the shared root.

Add callbacks in per-input, decomposition, ordering swap, composition, flags/bidi,
Punycode and roundtrip encoding loops. Bulk fixed shifts/static table lookup still
require supervised process expiry. The public scalar-output facade remains
minimal; no general strict Python codec API or new Unicode data is introduced.

Qualification: existing488+all1114112 IDNA ACE outcomes, all normalization source
sequences and2255 overflow/order cases; complete strict mode source matrices,
ASCII contents and NonAscii/UnicodeFailure classifications; scarce-budget overlap
and returned charge lifetime; prefix/lookup/NUL/empty precedence; deterministic
long-loop deadline refusal and actual supervised hostile/noemission cleanup in
the integrated helper. Shared root child allowance/header ownership, process
RSS/FD/GNU and final source-to-artifact provenance stay separate open criteria.
