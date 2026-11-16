# A05 helper extraction and projection budget proposal

Status: parent approved implementation of the 256 MiB default/max resource
policy and closed protocol; complete accounting/integration remains unfinished. Supervised decoder/parser implementation and existing source captures
remain separate evidence. This proposal does not qualify absolute process RSS,
file-descriptor isolation, or supported-target linking.

## Observed constraints

F08 accepts at most 32 MiB decoded HTTP body bytes. Policy query length defaults
to 500 characters, but `ssx-application/src/policy.rs` explicitly exempts HTTP;
there is no uniform semantic query cap to reuse. Arbitrary adapter config values
also cannot be serialized into a supposedly fixed helper envelope. The shared
`ssx_json` serializer has a 4 MiB cap, so storage serialization is unsuitable for
source-valid helper result rows.

## Closed wire protocol

Each request begins with ASCII `A05H`, version byte 1, operation byte, then only
operation-specific fixed-width integer/tag fields and length-prefixed bytes.
Integers use big endian. Unknown versions/tags, noncanonical codec names,
nonzero reserved fields, integer conversion/checked arithmetic failure,
truncation and trailing bytes fail closed. Child lengths are checked before
allocation. Responses have the shared supervisor's u64 length, strict EOF and
successful-exit requirement. The existing native negative-frame test exercises
wrong version/operation, input oversize and trailing bytes.

Operations: decode/detect returns validated codepoints plus fixed detector bits;
extract takes validated codepoints, a closed engine/layout tag and a reduced
row-limit tag. It returns a simple length-prefixed private row schema. Strings
are valid UTF-8, lengths checked before allocation, and rows carry only A05
fields. No arbitrary JSON/config, selectors, file paths or query bytes enter the
child. Query fallback is a title flag; query is projected by the parent only
where the original source uses it. Invalid max_results types remain deferred
until the first selected row, preserving valid-empty behavior.

Google feedback remains between the decode/detector response and extraction,
matching source ordering even when extraction times out. Each helper operation
uses the original search deadline and shared pool admission. Challenge scans,
CSS traversal, snippets and row selection execute in the killed/reaped child.

## Proposed finite resource policy

Approved: configurable per-search **256 MiB owned-allocation budget**, default
256 MiB, validated range 1..=256 MiB. Larger limits require separate policy
review. Four search-spanning admission permits cap modeled owned allocations
at 1 GiB; this does not establish container or child RSS limits. This is an explicit resource
hardening proposal, not an existing HTTP query-length contract. Reject the whole
adapter with a documented resource-budget error when the checked total exceeds
it. Ordinary long HTTP queries remain accepted when their actual cost fits.
The approved classification is a whole AdapterResponse Error with sanitized
`HTML resource budget exceeded` reason; no query truncation or semantic
query rejection. Admission spans decode, feedback, extraction and projection.

Account before allocation/copy: borrowed query length, encoded outbound request
size, raw body ownership, decoder point backing capacity, IPC request buffer,
response frame, decoded row backing storage, and each actual fallback-title query
copy. Shared borrowed data is charged once; simultaneous copies are each charged.
Use checked sums/products and fallible reserves. Admission reserves the operation's
charged aggregate before materializing queue-owned input; waiting is deadline
owned. Release charges only when their allocation is dropped. A query too large
to fit is rejected before constructing the encoded outbound request. Percent
encoding is counted in a scan before allocation, rather than pessimistically
rejecting every query at a 3x estimate.

The current `input * 256 + 16384` response estimate is provisional and must be
replaced by the admitted remaining budget; it is not a proof of a parser's RSS.
A frame is read incrementally with checked reservations against this remaining
budget. The row decoder checks every string and collection length before reserve,
never uses `ssx_json` or storage codecs, and charges fallback duplication before
projection. The implementation must either transfer ownership or charge both
serialized and decoded buffers while both are live. OS allocator overhead and
libxml2 allocations need separate absolute-child resource qualification; this
budget alone does not establish a total RSS bound.

## Required qualification

Preserve all historical captures byteexact. Add actual-source long-query image
fallback controls, just-below/above aggregate tests, overflow/negative/unknown
frame controls, >4 MiB valid rows without storage cap/panic, detector ordering,
Google feedback-before-extraction-failure, CSS workload deadline/cancellation,
actual PID kill/reap, permit release, saturation without queued payload copies,
and full normalized/request comparisons. Native-only runtime remains mandatory.

Implementation checkpoint: search-spanning admission/query charge and exact
single-buffer request form counting are present. Google config repr, URL copies,
body/IPC/rows/projection lifetime charging and helper extraction are still open;
this checkpoint must not be reported as complete budget enforcement.
