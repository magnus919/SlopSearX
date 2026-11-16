# A04: broad API search adapters

This batch ports Brave Search, Wikipedia, and Reddit through the accepted F06
registry and F08 HTTP session. The source-mapped integration tests use real
loopback HTTP sockets and the adapters' actual Rust parsers. The captured
responses and expected Python outputs are in [`cases.json`](cases.json) and the supplementary 333-record [`matrix.json`](matrix.json). The committed offline exporters are [`oracle/export_cases.py`](oracle/export_cases.py) for the original source-bound pack and [`oracle/export_matrix.py`](oracle/export_matrix.py) for added edge observations. The original exporter also writes its complete request/response/config reproduction record to [`source_context.json`](source_context.json).

The capture was produced offline with Python 3.14.7 by invoking the original
adapter `search` methods through `httpx.MockTransport`; it made no provider
requests and used only fake credentials. `coverage.json` pins the exact Python
engine and regression-test blobs, maps every named-engine regression to a
native case or a documented boundary, and pins both observation files and the
exporter. Replays compare every normalized response field and outbound method,
path, query, selected headers, request count, and order. All stable error diagnostics compare exactly, including Brave absent messages,
Reddit status/parser/conversion errors, and Wikipedia stage/status/MIME/shape
messages. Only original case `reddit.500` has an explicit transport diagnostic
boundary: source HTTPX includes its provider URL and HTTP documentation link;
the F08 native adapter returns exactly `Reddit returned an unsuccessful status`.
The full source error remains immutable in cases.json and the native replay
asserts that exact per-case replacement; the only additional decoder diagnostic disposition is documented in PARITY.md.

Run the fail-closed gate with:

```sh
cargo xtask check A04
```

To regenerate the captured oracle files from the repository root with the
pinned offline Python environment, run:

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a04/oracle/export_cases.py --context-output docs/rust-rewrite/a04/source_context.json > docs/rust-rewrite/a04/cases.json
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a04/oracle/export_matrix.py > docs/rust-rewrite/a04/matrix.json
```

The native cases cover Brave web/news/image/video routing, answer boxes, image
and video media, provider parameters, subscription-token placement, absent,
null, empty, and malformed iterable values, empty-string media sources,
status classification, timeout, and missing credentials.
Wikipedia covers both ordered request stages, contact User-Agent, corrections,
thumbnails, infoboxes, no-title success, missing-page IDs, malformed shapes,
HTTP failures, timeout, and cancellation, including stage-specific MIME and
65-item diagnostic shapes. Reddit covers anonymous and bearer
requests, token precedence, default OAuth host selection, custom base URLs,
subreddit routing, list parsing, Python-compatible truthy NSFW filtering,
self-post permalink edge cases, date and thumbnail conversion, whitespace
numeric parsing, Python-compatible booleans and nested container formatting,
out-of-range dates, empty/malformed data, auth/status failures, timeout, and
cancellation. All original records and supplementary matrix records replay.
Default Reddit OAuth host rewriting is checked against captured source paths
using a loopback origin substitution.

`SearchRequest.subreddit` is an optional typed input, consumed only when the
`reddit:subreddit` category is selected. Omitted input maps to `all`; empty and
ordinary strings are preserved. Source observations also retain explicit null
and numeric subreddit values: the Python adapter stringifies those values into
the path, while the typed native API cannot represent them. They are recorded
as an intentional input-boundary difference, not coerced to the missing case.
Brave media dimensions map Python booleans to the native `i64` fields
(`true/false` becomes `1/0`), while null result title/URL fields map to empty
native strings. Raw captures retain their source JSON types.
Accepted-main integration propagates the typed subreddit through validated
shared execution and includes exact Some values, including empty, in canonical
cache and singleflight identity. None retains every legacy key byte. This batch
does not add an HTTP, portal, or MCP parameter.

The native adapter entry point is `register_broad_api`; final registry
composition remains an integration responsibility. The adapters keep caller
ownership of the HTTP client and rate admission. The typed shared-service integration is qualified separately below; public
HTTP/portal/MCP transport composition remains downstream.

Third-review numeric compatibility reuses the accepted domain Unicode16 helpers
for Python decimal digit/underscore normalization and str/repr formatting. JSON
integer and float lexical kinds are preserved; floats render with Python value
spelling rather than their upstream JSON lexemes. The appended source matrix
covers valid and invalid separators, Unicode decimal digits, whitespace, signed
and exponent float forms, int-only timestamp strings, nested nonprintable keys
and values, negative zero and exponent repr, and wide integer diagnostics at
both Wikipedia stages.

Timestamp parity is pinned to this capture's macOS Python 3.14.7 runtime, whose
`time.gmtime` uses signed 32-bit tm_year relative to 1900. The exact lower
accepted timestamp is -67768040609740800 (year -2147481748). CPython's calendar
year conversion imposes the upper bound 67767976233532799 (year 2147483647),
inside libc's upper bound 67768036191676799. Outside libc bounds the source
reports errno 84; inside libc but above CPython's calendar-year bound it reports
`year out of range`; integers outside signed time_t report
`timestamp out of range for platform time_t`. Matrix records capture each side
of those transitions and years 0, 1, 9999 and 10000. This is source-runtime
compatibility evidence, not a portable claim about every Python host's libc.

The separate six-record [nonfinite source fixture](nonfinite_boundaries.json)
and [parity ledger](PARITY.md) document the accepted F05 finite-score boundary.
Its tagged source outcomes are reproduced by a third, separate exporter
invocation (the ordinary matrix command above still emits exactly 333 records):

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a04/oracle/export_matrix.py --nonfinite-output docs/rust-rewrite/a04/nonfinite_boundaries.json
```

The fourth full-source review is covered by 333 ordinary matrix observations,
including missing/null/type controls and mixed valid/malformed records. A04
uses one local Python quote_plus encoder for every parameter at all three
engines and both Wikipedia stages. Source whitespace applies only where Python
str.strip/regex whitespace is actually called; raw result text stays raw.
Integer truthiness is independent of float convertibility.

The separately retained [representation boundaries](representation_boundaries.json)
contain 126 genuine whole-source invocations and their reviewed native deltas.
See [PARITY.md](PARITY.md) for exact diagnostics and single/mixed whole-response
behavior. Reproduce this fourth, separate developer invocation with:

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a04/oracle/export_matrix.py --representation-output docs/rust-rewrite/a04/representation_boundaries.json
```

The typed-output policy covers every source-success value that the accepted
native result/media String fields cannot represent. Refusals wait until all
source operations succeed; a later original parser/conversion error takes
priority. The existing image-only null title/URL and boolean-dimension mappings
remain explicit exceptions. See the field inventory and deterministic refusal
order in PARITY.md. The matrix includes both boundary-before-source-error and
source-error-before-boundary cases, plus missing/null/falsy controls.

## Adapter-owned source JSON boundary

`ssx_infra::adapters::source_json::from_slice(bytes, deadline)` is the reusable
async source decoder. The deadline is the existing HTTP operation's absolute
`std::time::Instant`, never a fresh timeout. The future owns no background work;
byte decoding/token scanning yields at most every 4096 input code points, and
checks the inherited deadline before and after each yield and before returning.
Dropping the future stops decoding. Four native unit tests cover grammar,
encoding, depth/byte/digit boundaries, finite projection, and cancellation.

The internal `Value` holds exact arbitrary integers, rounded binary64 floats
including NaN/infinities, source text including unpaired surrogates, arrays, and
ordered objects. Duplicate object keys retain first position and last value,
as Python does. `Text::as_str` returns scalar text only; source operations must
use the source text/code points when it returns None. `Value::project` explicitly
requires finite numbers and Unicode scalar strings. Consumers must finish all
original source operations before returning any output representation refusal.
The API does not replace the accepted public `ssx_json` codec or storage types;
other adapter batches must independently audit their consumption and output
projection rules before reusing it.

All four decoder stages (Brave, Reddit, Wikipedia opensearch and rich query)
accept Python byte encoding detection: UTF8 with/without BOM, UTF16/32 in either
byte order with/without BOM, strict byte validity with surrogatepass, JSON
escapes, and Python NaN/Infinity constants. Finite floating lexemes round before
any source operation or opaque output; integers remain exact. Unused nonfinite
values and skipped Reddit records are retained without body-wide refusal.

[`source_json.json`](source_json.json) adds 239 actual whole-source captures
with complete native output and ordered request assertions. Source outcomes and
actual decoder exceptions are stored separately from explicit native deltas.
Source observations are flat fully typed trees (node references, tagged Python
types, UTF8-surrogatepass/base64 string leaves), so exceptional values and
user objects resembling tags stay unambiguous and source depth does not inflate
the finite fixture reader's nesting. The two large byte-limit inputs and long
integer bodies have exact byte recipes instead of repeated megabyte literals.
The original 37/context/333/126/6 files remain immutable. Reproduce the new corpus
with this separate offline developer invocation:

```sh
PYTHONPATH=. /private/tmp/slopsearx-rust-oracle-py314/bin/python docs/rust-rewrite/a04/oracle/export_matrix.py --source-json-output docs/rust-rewrite/a04/source_json.json
```

Representation and resource restrictions, precise diagnostics, source error
priority, and the narrow Brave malformed-byte/syntax diagnostic contract are
listed in [`PARITY.md`](PARITY.md). Native tests exercise every corpus record;
the gate compares its exact ID mapping and reviewed digest.

## Accepted-main integration

The approved A04 checkpoint is normally merged with accepted F14 main
`576d43b03c0378ffe113a1386b1ec88eaa4323df`. Adapter/parser implementations and all
37/context/333/126/6/239 source observations remain immutable. Both A03 community
and A04 broad registration APIs coexist; F06 activation remains a composition
successor. The shared filter-semantics source observation retains its historical
blob, with only the accepted exact PR874 old/new bridge admitting current bytes.
The bridge independently binds the existing 22-case date corpus and rejects any
unrelated drift; F09/F13/F14 supply current native date admission/cache evidence.
No adapter output or malformed JSON rule changes in this integration.

The additive typed `SearchRequest.subreddit` defaults to None. Policy retains the
exact request through direct and prepared authorization, and shared execution
forwards it unchanged. Canonical keys use a separate `search:v2:subreddit:` base
namespace for Some(exact string), which the composed cache and flight identity
inherit. None uses every historical source-key byte unchanged; empty stays
separate from omission. A real registered Reddit adapter, held loopback HTTP and
private Valkey regression runs eight simultaneous callers across four scopes,
proves one flight per scope, distinct results/query IDs and four subsequent
exact-scope cache hits. The mandatory A04 inventory now executes21 integration
tests plus the four existing decoder unit tests. All historical capture records
remain unchanged. T02/T06 and portal successors still own public request/output
composition; no new wire parameter is introduced here.

Current-main qualification and independent integration review precede parent
acceptance and publication; the historical source review alone does not accept
this combined tree.

The final current-main preparation also integrates accepted W00 main
`950071404c61eabc1201730362baccd01b3f2295`. Saved modules and gate wiring are
additive; the qualified A04 adapter/parser and shared-service cache/flight
semantics remain byte-identical to checkpoint `ca7c6209`. The final inventory
union is384 source records and25 developer invocations, preserving complete
parent rows. Acceptance still requires independent integrated-source review.

## Coordinated loopback timeout replay

Arm64 CI exposed a fixture race in the original `brave.timeout` and
`reddit.timeout` captures: a 10ms deadline could cancel an accepted TCP
connection before request bytes were written, and the fixture recorded its one
empty read as a complete request. The replay reader now accumulates complete
headers under a 64KiB bound and reports case-labelled EOF/read failures. A
mandatory regression covers fragmented headers, empty/partial EOF and overflow.
No observed request is skipped or accepted as partial wire evidence.

For timeout replays, the current-thread test clock is held while real request
headers arrive. A cooperative runnable task prevents idle clock auto-advance;
only then do explicit ticks exercise the unchanged native 10ms timeout before
the unchanged 60ms delayed response. Keeper cleanup joins and resumes time.
Full byte/request/count/output assertions and immutable source captures remain
unchanged. A04 now executes 22 integration tests plus four decoder unit tests;
this is a fixture scheduling correction, not an adapter behavior change.

## Repeated virtual-clock replay correction during F15 qualification

A full workspace run exposed an additional test-only clock relationship: explicit Tokio advances survive `resume()`, while native adapters construct `std::time::Instant` deadlines. Repeated timeout cases could therefore begin with virtual time ahead of the real deadline, completing before a request was recorded. A controlled 40ms prior-case advance reproduced this failure with a measured 39.940208ms offset.

The replay clock now pauses with its existing cooperative keepalive, then yields under a five-second real guard until the real clock catches the frozen virtual instant. Only afterward is the native search created. The adapter remains configured at 10ms, the delayed response remains 60ms, and complete real headers must still be observed before explicit deadline ticks. The guard owns panic/cancellation cleanup and resumes the test clock. There is no adapter runtime or source-capture change. The durable offset regression joins all 22 existing tests; the exact A04 gate now requires 23 passing, non-ignored integration tests.
