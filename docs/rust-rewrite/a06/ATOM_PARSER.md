# arXiv Atom parser candidate

This is parser progress for issue918, not A06 acceptance. The candidate API is
`ssx_infra::adapters::arxiv_expat::AtomPool::parse(&str, deadline)`. It accepts
already source-compatible decoded Unicode scalar text. No engine factory, HTTP
request, redirect, decoder or production service is activated. The original
`_parse_feed` query argument is unused. The earlier quick-xml implementation is
retained only as [historical comparison](ATOM_PARSER_COMPARISON.md), never as a
runtime fallback. Its provisional resource claims are superseded here.

## Maintained native dependency and source behavior

The offline build checks the exact Expat2.9.0 archive SHA256
`1e6371862cc31999b368c3b89b49994f0677e1bab5f1b2b85ae3741f5d803051`.
The archive and MIT notice live in `vendor/expat-source`; no signature verification
is claimed. `vendor/expat` is the narrow unsafe boundary. Native C is explicit:
this candidate has no Python backend, but is not a pure Rust parser.

Expat DTD/general-entity/namespace support is enabled. UTF8 input and namespace
separator `}` are fixed; parameter entities use NEVER. No external resolver,
external-parser creation, unknown-encoding handler, parser reset/re-entry or
suspend/resume API is exposed. Runtime version, feature flags and native default
amplification/allocation-amplification policies are checked. This replaces
bespoke XML/DTD grammar and Expat accounting reconstruction.

The wrapper streams only fields used by the source adapter: direct Atom entries,
first direct fields, all direct author/name children, `.text` before the first
element child, ignored comments/PI, and source namespace/default-attribute rules.
It validates the entire document before exposing results. XML syntax errors
return source-style OK-empty. The known legacy ID `rsplit('/')` bug remains.
Payload field order, author order, empty-versus-absent fields, provenance, Python
whitespace trimming/collapse and 300-code-point snippet are preserved.

The actual source runtime is Expat2.7.4. Upgrading rejects two captured malformed
XML declarations accepted by that runtime: version `2.0` and `1.`. Their original
successes remain in `source-xml-grammar.json`; native tests assert exact OK-empty
outputs for those IDs. This is an approved parser-upgrade correction, not fixture
normalization. Other captured outputs are compared completely.

## Ownership, cancellation and operational bounds

One operation is admitted before any text scan/copy and holds admission through
worker cleanup, reply decode and projection. The constructor rejects capacity
other than1; increasing it requires aggregate application/container review.
The backend keeps an original Arc lease through returned-byte consumption and
passes a clone to common ProcessPool. Cancellation cannot free accounting while
the supervisor still owns frame/reply allocations. Common shutdown failure is
propagated before waiting for accounting deliberately retained on unknown cleanup.
The shared component is separately reviewed; it owns lifecycle/framing only.
Shutdown must be explicitly awaited before runtime teardown.

A single64MiB requested-capacity ledger covers native Expat allocations plus
retained Rust fields/vectors, with headers and conservative old/new growth overlap.
Allocation overflow, failed realloc preserving the old allocation, free and
cross-language charges are tested. This is not allocator bookkeeping or RSS.
A retained-field refusal drops retained data and continues XML validation: later
syntax failure wins as source OK-empty. Native allocator exhaustion is an
operational ERROR. No independent depth/node/entity-chain ceiling is imposed.

These are explicit operational-hardening differences: source-valid expansion to
a64MiB title refuses with whole ERROR `arXiv native XML resource limit exceeded`;
the same input followed by malformed XML remains OK-empty. Source observations
and deterministic generators are retained in `source-xml-stress.json`. There is
no field truncation and no claim the quota is source-equivalent.

The input ceiling is128MiB UTF8 and32Mi Unicode scalar values, conditional on the
accepted F08 body32MiB plus eventual HTTPX-compatible decoder seam. This covers
replacement-character expansion without replacing32MiB with an arbitrary4MiB
parser ceiling. Complete HTTP decoding/ownership integration remains unfinished.
A closed `AX01` binary backend payload has checked counts/lengths before reserve,
nullable fields and closed status tags. Unknown versions/tags, UTF8 errors,
overflows, truncation and trailing bytes refuse. Decoder retained capacity is
charged before allocation; reply length is at most64MiB+13 bytes. Parent drops
reply bytes before projection. No global ssx_json policy changes.

Projection has a separate128MiB checked quota derived from actual result/value
sizes, closed map cardinalities8/5/4, pinned IndexMap/hashbrown reservations, known
keys, bounded snippet bytes, author slots and conservative live string copies.
Direct known payload construction avoids generic JSON reparse limits; an actual
source-valid5MiB abstract now passes after a reproduced pre-fix interpolation
panic. The source summary and all other fields remain captured.

For N1, tracked parent payload phases are bounded by320MiB: caller text128MiB,
frame128MiB and reply64MiB during exchange; caller128MiB+reply64MiB+decoded64MiB
while decoding; caller128MiB+decoded64MiB+projection128MiB while projecting.
These exclude allocator overhead, surrounding decoder/raw HTTP buffers and whole
service state. They are not a whole-server RSS bound. Final adapter integration
must account those additional owners. Cancellation checkpoints occur during
input scanning/copying, binary decode/UTF8 validation and projection scans/copies;
near64MiB reply and8MiB field timer probes run on a single-thread runtime.

GNU helpers apply hard1GiB address space, CPU300s, CORE/FSIZE0 and NOFILE3 before
reading input. Existing request deadlines normally kill sooner. Inherited FD
closure preserves stdio and the enumeration directory FD, then closes that FD;
real file/socket sentinel checks verify close and peer EOF. macOS developer mode
adds1GiB virtual headroom to its enormous startup mapping; this is explicitly not
an absolute RSS guarantee. Supported production qualification remains GNU amd64
and arm64, matching ADR005. CI matrix qualification is prepared but has not run.

Expat2.9.0 retains upstream unresolved security reports [#1160](https://github.com/libexpat/libexpat/issues/1160)
and [#1076](https://github.com/libexpat/libexpat/issues/1076). In particular a
reported roughly2MiB CPU denial-of-service trigger fits this input budget; its
undisclosed details cannot be declared unreachable. Isolated processes, resource
limits and kill/reap mitigate exposure; they do not fix the library or establish
that unresolved vulnerabilities are harmless. This is resource containment,
not a filesystem/network sandbox.

## Evidence and remaining acceptance

Run native candidate tests with `cargo test --locked -p ssx-infra --test
 a06_expat_candidate`, common tests with `--test a06_process_pool`, binary and
projection units with `--lib arxiv_expat`, and native allocator units with
`cargo test --manifest-path vendor/expat/Cargo.toml --target-dir target`.
The complete replay inventory is216 parser-applicable records, including the two
explicit version deltas. The original139 whole-adapter source records remain
unchanged. Stress supplement contains five complete outputs and three large-field
summaries, not eight complete outputs. Counts are bounded evidence, not source
completeness or adapter acceptance.

Remaining work includes final native safety/resource review, actual both-GNU-arch
CI, temporary oracle inventory/retirement registration, full adapter HTTP/codec/
redirect/deadline integration, OpenAlex/Semantic Scholar, portal impact and exact
current-main qualification. No deployment, factory or issue completion is claimed.
