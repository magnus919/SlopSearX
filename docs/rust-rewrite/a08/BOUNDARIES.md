# Runtime ownership and remaining boundaries

Actual adapters share F08 sessions, source decoding, SearchAdmission and caller
SearchContext. The accepted 4 MiB SourceJSON input, depth/text/integer limits,
F08 request/body limits and finite native domain apply. One 32 MiB operation
budget covers the reused owned string/vector, payload visitor and ProducedBatch
paths; the actual source DOM, query input, cold immutable case tables, URL parser
scratch, BTree nodes, returned responses and caller clones are separate owners.
A08 does not claim an exact total allocation/RSS bound, platform qualification,
full pending-phase scheduler latency proof or G02 accounting closure. In
particular, EDGAR/Oyez URL parsing and some bounded source predicates use
ordinary temporary allocations. No global bound is inferred from narrow tests.

All four real loopback sockets close after future drop and network timeout.
Caller deadlines also stop pending admission with zero transport requests.
Source FDA sequential requests retain per-request timers in direct calls; a
caller deadline bounds the whole contextual call. Failure or output
representability refusal returns no partial successful rows. Finite/non-scalar
refusals and exact library-error wording differences are individually recorded.

The generic payload envelope preserves source `build_payload` behavior: domain
and type are engine declarations, with no newly invented source vocabulary
validation. Runtime adapters pass only the existing audited financial and
biomedical identifiers. Ordered field omission/provenance and sanitation are
unchanged, including full payload-only text beyond display prefixes.

G01 owns final composition and all adapters/catalog completeness; G02 owns
global/query-input/DOM/retained-response accounting and broad resource testing;
T02/T03/T06 own complete public HTTP, HTML/browser and MCP integration. G08
retires all four executable temporary source producers while preserving their
non-executable captures/provenance. No deployment, live provider certification,
RSS uplift or final milestone acceptance is claimed here.
