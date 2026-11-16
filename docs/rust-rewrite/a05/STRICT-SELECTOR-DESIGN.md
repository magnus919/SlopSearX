# Narrow RFC2231 strict selector capability

Status: private native strict capability implemented and transferred in parent-reviewed development donor v3. The exact Unicode3.2 profile data, Unicode16 label classifier, independent ICU replacements and alias dictionary boundaries received scoped parent review; complete provenance/resource/package acceptance remains pending. A06 owns the shared header/email selector, and A05 owns codec implementation. Body replacement/incremental mode remains separate.

The paragraphs below retain the original design investigation and source rationale. Their pre-adoption wording describes that historical decision point, not the current implementation status. Current types/ownership are in SHARED-HTTP-TEXT-PROPOSAL.md; immutable donor v3 retains the original proposal text. Native qualification at that freeze ran14 unit and16 integration tests with no ignored/filtered tests plus strict all-target Clippy. Actual whole HTTPX selector integration and charged allocation implementation remain open.

The operation must return strict ASCII success, strict non-ASCII success, Unicode/lookup failure (email falls back to the original parameter text), or an exact propagated non-Unicode/non-lookup failure. It must stop at the first source error. U+FFFD is ordinary data, never an error marker. UTF16/32 BOMless native-endian nonincremental and UTF8SIG final-prefix handling need separate controls from body decoding.

IDNA cannot simply map to original text or UTF8. The actual HTTPX controls use body byte80: `utf8%00.xn--caf-dma` under RFC2231 IDNA succeeds with non-ASCII and selects UTF8, whereas `utf8%00.xn--a` falls back to original NUL and raises ValueError `embedded null character`. A later invalid ACE label undoes an earlier valid non-ASCII label: both valid-then-invalid and invalid-then-valid fall back to the whole original parameter. Empty body precedence remains a separate caller concern.

The proposed private IDNA capability validates all byte labels in order, preserving the source fast path (ASCII without ACE), strict punycode failures, ToUnicode's 1024-byte label check, ToASCII's source profile, and case-insensitive roundtrip. It need only report whether the final decoded text is ASCII; no public general IDNA string API is required. It must not return early on the first valid non-ASCII label because later errors change email fallback.

A decoded-label NamePrep fixedpoint shortcut is rejected. Actual `xn--CAF-dma` succeeds even though NamePrep changes its basic ASCII case; the source compares encoded roundtrip bytes after lowercasing. Implement the actual private encode/compare or prove an equivalent algorithm against canonical/noncanonical, uppercase basic/extended, multiple-label and failure-precedence controls. Punycode injectivity alone is insufficient.

Existing stringprep0.1.5 public nameprep is not directly suitable: it rejects Unicode3.2-unassigned characters; CPython's profile explicitly allows them. Actual `utf8%00.xn--6la` (U0221), `xn--tma` (U0237), and `xn--e28h` (U1F600) select UTF8. Rejecting these would expose the original NUL ValueError. Its normalization/bidi dependencies also need Unicode3.2 qualification. Existing idna1.1 UTS46 is not a drop-in IDNA2003 profile.

Preferred assessment is a private profile over existing native punycode, source B1/B2 mappings and exact Unicode3.2 normalization/bidi/prohibition data. Before copying data, produce a per-file source/notice manifest and generator identity. CPython3.14.7 already supplies the versioned UCD machinery; importing all current Unicode16 normalization tables would not establish source compatibility. No new tables are approved by this proposal.

Pinned source archive SHA256: 3b48dac8fb59f62eaa67ac83c1eb12bda1b7a08406dd286e252c11a66be27f81. Local root `/private/tmp/a05-cpython-provenance/Python-3.14.7`:

| File | SHA256 |
| --- | --- |
| Lib/encodings/idna.py | ea8d7f01422af0ad138e4bf58d4ae63112f3ceb68833c486d6c4d18252610fd8 |
| Lib/stringprep.py | 60b6c83581093029312efb6670b11c540090b3f78bcf72264467b494f02f21a5 |
| Modules/unicodedata.c | d69558ad96a38932da9e9d596bea8cd91d54d9801e56f2cea77a6ecfccb47a6c |
| Modules/unicodedata_db.h | 00a5ed9e602bb55e9f080333ac8cbc360352eea9de960eec69efdb0c3d68b110 |
| Tools/unicode/makeunicodedata.py | 3a83416f717bab94983baba84537dc17f4d997f82d653aa944e98a710e8c7078 |

Temporary source exporter: `oracle/idna_selector.py`, pinned CPython3.14.7 and immutable source-codec policy input. Current 488 observations: `/private/tmp/a05-idna-selector-observations-v4.json`, SHA256 edf887c2be153b76beb9cecf19d9cc332b019d7f7497a93167f731ced58717f4. Includes all455 registered labels and33 controls, complete raw header/body, selected charset/encoding/canonical decoder, codepoints or exact exception. Register exporter/invocation for F01/G08 retirement before final handoff. These are source observations, not native parity results.
