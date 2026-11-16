# A04 parity boundaries

The ordinary 37 source cases and 333 supplementary records compare complete
outputs and exact ordered wire contracts, subject to the four previously
recorded representational boundaries in coverage.json and the exact
`reddit.500` F08 HTTP diagnostic disposition.

## F05 finite score invariant

The accepted F05 `FiniteF64` model cannot represent Python float NaN or either
infinity. This batch preserves that invariant. The source accepts score strings
`nan`, `inf`, and `-inf` and returns OK with those nonfinite score values.
For a listing containing valid/nonfinite/valid posts it returns all three
results, including both valid neighbors. Native Reddit refuses the entire
listing with ERROR, no results, and exactly `Reddit score must be finite`.
There is no partial-result success and no silent replacement with zero.

[`nonfinite_boundaries.json`](nonfinite_boundaries.json) records six genuine
offline source invocations: each spelling singly and in the mixed listing.
Only actual nonfinite output floats are encoded with the explicit
`$source_nonfinite_float` tag; the tag is an observation encoding and is never
a native expected result. Request/response bytes and source status/messages
remain intact. The native test exercises all six IDs, checks source whole-list
outcomes, asserts the exact native whole-error response, and compares every
ordered request/path/query/selected header and credential absence.

Reproduce this separate tagged capture with the same durable exporter:

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a04/oracle/export_matrix.py --nonfinite-output docs/rust-rewrite/a04/nonfinite_boundaries.json
```

The source timestamp portability boundary is documented in README.md: exact
observed macOS Python 3.14.7 behavior is preserved, including non-datetime years.

## Media URL construction

Six additional whole-adapter captures cover image and video URL query sanitization,
blank and duplicate query values, relative URLs, and space encoding. Native Brave
uses the accepted domain media constructor; raw result URL, thumbnail, and image
source fields retain source bytes. Numeric video duration retains integer-versus-
float representation through the domain number constructor. The replay observer
serializes that typed field directly instead of coercing it to a floating value.

## Additional reviewed native representation boundaries

The accepted native finite media duration excludes Python bool and numeric
values that cannot convert to a finite f64. Image width and height are signed
i64. The source retains boolean durations and arbitrarily wide integer media
values. Native refuses the entire response, preserving no partial results,
using exactly these diagnostics:

- `Brave video duration must be a finite non-boolean number`
- `Brave image width must fit a signed 64-bit integer`
- `Brave image height must fit a signed 64-bit integer`

Width is checked before height. Numeric integer durations that fit the finite
domain retain their integer kind, including integers wider than u64; float
durations are first rounded to the source parsed f64 value and emitted through
the accepted Python float formatter. Null and nonnumeric durations remain
absent as in source; boolean dimensions retain the already reviewed 1/0 mapping.

Python also permits a nonempty list selftext to become non-string result content.
Native `SearchResult.content` requires a String, so this exact case refuses the
whole response with `Reddit selftext must be a string when nonempty`. Empty lists
retain source false truthiness and generate the ordinary summary. Scalar and
dictionary selftext preserve their original Python slicing errors and ordering;
they do not use this additional representation diagnostic.

`representation_boundaries.json` retains the actual full source outcomes, plus
separately labeled exact `native_expectation` deltas. The first 14 records capture each of six media variants
and the one content variant singly and between valid neighbors.
Source returns all three mixed-list results; native returns ERROR and zero
results. The gate exercises all 126 distinct IDs with serialized full-output and
ordered wire/header/credential-absence assertions. These are explicit reviewed
typed boundaries, not a generic malformed-input normalization policy.

## F05 String and optional-string output inventory

Python dataclasses also admit source-success JSON field types that the accepted
native String/Option<String> fields cannot represent. Those are explicit native
whole-response refusals with the stable diagnostic format
`<Engine> result <field> must be a string` or, for optional fields,
`<Engine> result <field> must be a string or null`.

| Engine | Required fields | Optional fields and shared uses |
| --- | --- | --- |
| Brave | `url`, `title`, `content` | `thumbnail` (also feeds `img_src` and media thumbnail), `media.url` after source image fallback |
| Wikipedia | Extract processing and title.replace already require source strings and preserve original errors | `thumbnail` (also feeds opaque infobox image data) |
| Reddit | Truthy `title`; nonempty list content retains the specific selftext diagnostic above | A retained truthy, nonsentinel `thumbnail` |

Brave's existing image-only explicit-null title/URL mapping to empty native
strings remains permitted. Missing required fields retain source defaults;
missing/null optional fields remain absent. False Reddit titles are skipped,
and false or sentinel Reddit thumbnails become absent exactly as in source.
Answer and infobox fields remain opaque JSON; they are not coerced into strings.

Source-error precedence is separate from this native output boundary. Every
original parsing operation runs in source order, including all result records
and Brave answer extraction. If any source operation fails, its original whole
ERROR and stable diagnostic win over every pending typed refusal. Only an
otherwise successful source response triggers the first pending refusal. This
also applies to the existing nonfinite-score, media numeric, and list-selftext
boundaries. No partial results or answers are returned.

Within a source-success response, the first rejected result wins. Brave checks
result URL, title, content, thumbnail, selected media URL, width, height, then
duration (only fields used by that source endpoint). Wikipedia checks each
result thumbnail after its source extract/title operations. Reddit checks
truthy title, list selftext, finite score, then retained thumbnail. This order
is stable and does not change original source-error precedence.

The 126-record representation fixture preserves the first 14 reviewed records
and appends 112 genuine field-boundary source observations, each singly and in
a mixed list. The 333 ordinary records include boundary/source-error pairs in
both orders, same-record source errors, and controls for
missing, explicit null, empty strings, false titles and discarded thumbnails.
All ordinary source fields compare exactly; representation records retain
separate full source outcomes and exact native expected deltas.

## Source JSON decoding and public output limits

The additional 239 whole-source observations in source_json.json retain exact
input bytes (or exact large-body recipes), actual Python decoder values or
exception class/text, complete actual source outcomes, and separate complete
native expectations. Its flat typed tree never interprets a user dictionary as
an internal marker. Finite float canonicalization and byte decoding parity are
adapter-owned; global lossless ssx_json and accepted A01–A03 are unchanged.

Python accepts nonfinite JSON constants and overflowing float lexemes anywhere.
Unused values and skipped records remain accepted. Selected typed Reddit scores
and Brave durations preserve their already documented finite refusals. Opaque
Brave answers and Wikipedia infoboxes containing any exposed nonfinite number
produce whole ERROR with respectively `Brave answer must contain only finite
JSON numbers` or `Wikipedia infobox must contain only finite JSON numbers`.
Pending refusals never mask an original source error in any later record or
answer; source errors before the boundary also retain their exact outcome.
Within otherwise-successful parsing the first rejected output wins in source
extraction order. No result is silently dropped or partially returned.

Python strings can contain unpaired surrogates; F05 String fields cannot.
After the original operations (slicing, Python repr, startswith, whitespace
cleanup, source request encoding, and media sanitation), unrepresentable output
fields produce whole ERROR: `<Engine> result <field> must contain only Unicode
scalar values`. Relevant fields are Brave url/title/content/thumbnail/media.url
(and source aliases), Wikipedia url/title/content/thumbnail, and Reddit
url/title/content/category/thumbnail. Wikipedia constructs URL before title,
so a bad rich-page title rejects url first. Opaque diagnostics are `Brave answer
must contain only Unicode scalar strings` and `Wikipedia infobox must contain
only Unicode scalar strings`. These are explicit representation hardening,
not arbitrary-JSON parity. Raw source values remain in observations. A surrogate
removed by source slicing, escaped by nested Python repr, or removed with a
sensitive media query value does not cause refusal. Wikipedia request titles
that fail actual UTF8 encoding retain the original encode error and issue only
the first request. Twenty-four source sanitizer controls include duplicate secret
keys, percent keys, fragments, empty queries/separators, invalid hosts/ports,
controls, kept surrogates, and URL parser fallback.

Only actual Brave JSON syntax failures or invalid byte decoding use the native
message `Brave returned invalid JSON`; source decoder exception class and full
message remain in each record. No other Brave error is normalized. Reddit
retains `Reddit returned invalid JSON` for source ValueError decoding failures;
Wikipedia retains its existing stage/status/MIME/malformed_json diagnostic.
The pinned source runtime's 4300-digit integer decoder limit is observed for
positive and negative integers; 4301-digit integers raise the original
ValueError. Leading zeros remain syntax errors; a decimal/exponent lexeme is
binary64 and does not use the integer digit cap. Brave retains the original
integer-limit message, while Reddit/Wikipedia retain their own ValueError
handling.

The bounded native source decoder retains F05 input limit4194304 bytes and
container depth100. Exact boundary and first-failure tests use valid source
inputs, which Python accepts. Native excess produces distinct whole ERROR:
`<Engine> response exceeds JSON byte limit (4194304 bytes)` or
`<Engine> response exceeds JSON depth limit (100 levels)`. Decoded code points
also have cap4194304 and diagnostic `<Engine> response exceeds JSON decoded-text
limit (4194304 code points)`; the input-byte cap makes a separate decoded-text
excess unreachable with these UTF encodings (each code point consumes at least
one input byte). This is bounded resource hardening, never malformed syntax.
F08 wire/decoded transport caps and cancellation remain unchanged. Decoder
checkpoints yield cooperatively, inherit each request's existing deadline, and
leave no worker running after cancellation. The gate also runs four decoder
unit tests, including depth100/101 without stack overflow and dropped-future
cancellation on a maximum-size input.

## Accepted-main shared-service integration

The pre-integration adapter parser and every historical source observation remain
unchanged. A04's typed optional subreddit now reaches the accepted F14 shared
service through both direct and prepared policy authority. Some(exact string)
adds a versioned, separately framed base-cache identity; None preserves every
historical key byte. Existing composed-cache and singleflight identities bind
that base key. Omitted input therefore still dispatches `/r/all/search.json`,
while explicit empty dispatches `/r//search.json`; neither aliases ordinary
subreddit strings. This closes a demonstrated pre-fix shared-flight collision,
with real held loopback dispatch, eight concurrent callers and four private
Valkey cache entries/hits. It changes no JSON parser behavior and introduces no
HTTP/MCP/portal input. Source null/numeric subreddit values remain outside the
typed API; no coercion or new source-corpus normalization is added.
