# F05 identity and clock contracts

The pure domain crate separates canonical backend records from public wire DTOs.
Storage conversion follows the pinned Python readers, including free-form exclusion
stages and signed legacy tier/position values. Scores must be finite but may be
negative. Synthetic adapter outcomes cannot contribute observed latency.

Artifact composite IDs retain Python ASCII JSON, UTF-16 surrogate escaping and hex
encoding. Manifest IDs use sorted compact UTF-8 JSON; staged plans and saved IDs
use sorted compact ASCII JSON. Float encoding retains Python's shortest digits,
fixed/scientific cutoffs, exponent padding, and negative zero. These helpers do not
use runtime object identities for cache keys. Future native cache fingerprints
must use a distinct namespace under ADR 004.

`contracts/unicode/casefold-16.0.0.json` is the full default casefold mapping
exported from CPython 3.14.7 (`unicodedata.unidata_version == '16.0.0'`), matching
the deployed Docker baseline. It contains 1557 changed code points, excluding
surrogates. Unicode data is licensed under the Unicode License v3
<https://www.unicode.org/license.txt>. Python's algorithm and bundled Unicode
source are documented at <https://docs.python.org/3.14/library/stdtypes.html#str.casefold>.
The developer 3.12 environment uses Unicode 15 and differs for 27 newly introduced
capital mappings. Existing opaque IDs remain opaque; readers never recase them.
New native normalized identities use the deployed Unicode 16 semantics.

Persisted expiry, Valkey-authoritative quota timestamps, and local monotonic
execution deadlines are distinct types. Monotonic deadlines have no serialization
API. Snapshot fields are private: total is derived from the complete captured
results, expiry cannot precede creation, and reads use mutually exclusive enum
variants. Lease tokens carry their owner's type and cannot be passed as an ID or
as a lease for another owner type.

Native artifact-reference decoding requires JSON integer version `1`; unlike
Python's equality-based check it rejects boolean `true`. Durable references
produced by Python already use integer `1`. Canonical identity writers reject
nesting deeper than 100 to bound untrusted input processing; source-generated
records within this supported bound retain exact bytes. Arbitrary-precision JSON
integers are retained as decimal digits, including values outside i64/u64.

The Unicode table was compared byte-for-byte by mapping against official Unicode
16 `CaseFolding.txt` entries with statuses C/F (full, default fold), excluding S/T.
`provenance.json` records release source digest and reviewed runtime table digest.
The table is immutable runtime data; native acceptance requires its exact digest.

Finite numeric domain fields retain the caller's JSON integer-versus-float token
when constructed, while legacy readers apply the source's explicit coercions.
Their serialization remains representation-sensitive. They expose no ordering
trait: converting a large integer to f64 can round neighboring integers together,
so a later ranking boundary must choose and document that conversion policy.
The public/opaque comparator keeps integer-versus-float types distinct; canonical
JSON is compared as exact bytes, including signed zero.

F05 result projections accept precomputed typed HTTP formatter metadata and MCP
retrieval classifications. They map those inputs and canonical records into
public DTOs. F11 owns the complete retrieval classifier and T02 owns the complete
HTTP URL/date/format derivation. No partial parser or eligibility decision ships
in this DTO boundary, and projection fixtures do not claim those later behaviors.

Legacy readers use Python's JSON-type `str`/container `repr` conventions, including
`True`/`False`/`None`, quote selection, Unicode 16 printable-character escaping,
and dictionary insertion order. The pinned pure `indexmap` dependency retains
that observable order; canonical identity profiles continue to sort explicitly.
`python-text-16.0.0.json` records CPython 3.14.7's 76 decimal-digit ranges and 737
nonprintable ranges. Its generator identity and SHA-256 are in Unicode provenance,
and the native gate checks the reviewed digest. Legacy numeric reads accept
booleans, finite float truncation for integer fields, trimmed numeric strings,
valid digit separators, and Unicode decimal digits; malformed separators and
nonfinite scores remain errors. Media construction applies urllib's literal
TAB/LF/CR and leading-control preprocessing before matching secret query keys,
while retaining the source's original-input no-query and parse-error branches.

Media URL sanitization mirrors the CPython 3.14 urllib splitting/reconstruction
rules used by the source helper: ASCII scheme recognition, relative paths with
colons, empty authorities, path parameters, bracketed IPv6/IPvFuture validation,
and source parse-error fallback. The Unicode table includes all 19 non-ASCII
characters whose Unicode 16 NFKC expansion introduces a netloc delimiter. The
source-derived corpus exercises each character and malformed/valid bracket
families. This constructor codec performs no retrieval eligibility decision.
