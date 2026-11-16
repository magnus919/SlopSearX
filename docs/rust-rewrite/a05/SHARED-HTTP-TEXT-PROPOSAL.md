# Shared native HTTP text decoding proposal

Status: concrete interface/ownership proposal for parent review. Codec algorithms
and package provenance remain under qualification; this is not an accepted shared
API or a claim of complete codec parity.

Reuse the `ssx-source-codecs` leaf inside each existing supervised native helper.
A06 should decode raw HTTP bytes inside its Atom helper, then perform its own
source-qualified XML boundary conversion and parsing. It need not call A05's
HTML helper, transfer decoded codepoints back to the parent, or acquire a second
A05-specific search permit. Decoder CPU then shares Atom cancellation/deadline,
kill/reap, descriptor and memory policy. No interpreter/backend fallback exists.

Proposed public leaf facade:

```rust
pub enum DecodeError {
    Codec(String),          // exact native source-compatible codec diagnostic
    RegistryInvariant,      // applicable registry entry missing implementation
}
pub fn decode_http(body: &[u8], label: &str) -> Result<Text, DecodeError>;
impl Text {
    pub fn codepoints(&self) -> &[u32];
    pub fn into_codepoints(self) -> Vec<u32>; // ownership move, no copy
}
```

The facade wraps the existing qualified `decode_implemented` dispatcher and
converts its internal None into RegistryInvariant, never a fallback. Existing
empty-body short circuit, unknown-label UTF8 fallback and known rejected-codec
errors remain unchanged. Aliases resolve through the shared canonical registry;
helper IPC accepts only a closed canonical label, validated before allocation.
The Option dispatcher can remain during migration for existing gate/test users.

`Text` owns validated codepoints <=10FFFF, including lone surrogates. It is not a
RustString or a JSON string and does not normalize/replace surrogate pairs.
A05 reconstructs CPython minimal-kind lxml input. A06 must qualify the actual
ElementTree/source Unicode boundary independently: converting to scalar UTF8
must explicitly reject surrogate units with source-compatible stage/outcome,
or preserve any source behavior its observations require. No generic lossy
conversion method is proposed. A06 must not assume A05's lxml UTF16/32 behavior
applies to XML/Expat.

Ownership contract: caller acquires its search/operation admission before any
queue-owned copy, retains an original Arc through response projection, and clones
the guard into approved ProcessPool::exchange_with_lease. Prefer transferring an
owned raw response Vec into the helper request instead of cloning it. Account
actual capacities and all simultaneous raw-body, IPC, decoded-u32, scalar-UTF8,
parser, output-frame and projection buffers. Existing F08 body limits do not
by themselves account those overlaps. Codepoint count <= body bytes has a
108-family derivation, but decoded UTF8 may require four bytes per point.

Caller headers/config remain outside the closed worker operation except the
validated codec/layout/count tags needed by that backend. Decoder failures carry
a Decode stage, before source XML/HTML errors and before any backend-specific
feedback ordering. Common worker lease persists until payload/response futures
have dropped and kill/reap is verified. Cleanup uncertainty/panic fails closed;
shutdown propagates that error before waiting on accounting deliberately retained
by the poisoned worker. No successful drainage may be claimed in that case.

Each backend still owns supported-target/resource/FD qualification, the finite
aggregate admission policy and raw response allocation preflight. This proposal
shares codec logic and process ownership, not unreviewed A05 budget estimates.

## Strict selector development amendment

The private selector capability is implemented in the same leaf, for reuse inside
an already admitted backend helper. Its current public seam is:

```rust
pub enum StrictAsciiOutcome { Ascii(String), NonAscii, UnicodeFailure }
pub enum ParameterError {
    LookupFailure,
    ValueError(&'static str),
    RuntimeError(String),
    OverflowError(&'static str),
    RegistryInvariant,
}
pub fn strict_parameter_ascii(bytes: &[u8], label: &str)
    -> Result<StrictAsciiOutcome, ParameterError>;
pub fn lookup_http_encoding(label: &str)
    -> Result<Option<&'static str>, ParameterError>;
```

`lookup_http_encoding` preserves embedded-NUL ValueError separately from unknown
labels. `strict_parameter_ascii` checks NUL first, then returns empty ASCII for
empty parameter bytes before codec lookup. Whole HTTPX empty-response handling
must bypass the entire selector, including lookup. Body decoding remains the
replacement-mode `decode_http`; strict failure cannot be inferred from U+FFFD.
IDNA validates every label and case-insensitive roundtrip before returning
NonAscii. Fatal RuntimeError/OverflowError/ValueError propagate; callers must not
turn them into unknown-label or Unicode fallback.

A06 owns the shared HTTPX/email charset selector. The earlier paragraph excluding
raw caller headers from worker operations is superseded for that selector:
ordered raw headers and body belong inside the existing admitted helper, with
checked framing and all lifetimes accounted. A05 will consume the reviewed
selector; this donor does not yet implement or qualify the complete selector.

The Unicode16 label classifier, dictionary-only dotted fallback, strict state
modes and private Unicode3.2 IDNA profile have source observations and native
regression coverage. This supports development transfer, not complete package
acceptance. CODEC-ALLOCATION-INVENTORY.md records remaining transient allocation
work. Process memory/FD/GNU qualification, full provenance reconciliation,
F01/G08 inventory and integration with backend resource limits remain open.
