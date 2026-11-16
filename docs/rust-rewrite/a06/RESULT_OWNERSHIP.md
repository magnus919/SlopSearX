# Result ownership and memory scope

This analysis uses accepted main `1e9dd82d` plus the reviewed A06 development
components. It does not establish a global server RSS limit.

The shared allocation ledger owns requested backing storage during one supervised
operation. A parent-reserved child allowance remains charged through verified
reap. Raw transport buffers, request framing, child-local parse and consumer
storage, reply bytes, parent decode and projection need distinct charges for
actual overlapping allocations. The outer operation Arc remains alive through
parent projection and disposal; a supervisor clone covers cancellation cleanup.
A charge must never be released while its covered buffer remains owned.

`AdapterResponse.results` and `SearchResult` in
`crates/ssx-domain/src/records.rs` contain ordinary owned vectors, strings and JSON
values. They do not carry an allocation lease. Returning such values transfers
ownership to the application; an operation guard alone does not continue charging
their lifetime in merger, cache, snapshot or caller-owned clones.

`CanonicalResponse::new` in `crates/ssx-application/src/cache.rs` explicitly accepts
fresh evidence without persistence size/count bounds. Its Arc shares canonical
evidence, while materializing a response view copies the selected response data.
F14 `SearchConfig` defaults limit active callers to 256, flights to 64 and parallel
engine operations to 10. These constrain concurrency, not an aggregate response
byte pool. Completed flight watch data may remain until the next admission prunes
finished flights. A caller's returned response can outlive its search permit.

F05 `StorageLimits` restrict persisted payload/work-group objects (default 16,384
and 128,000 bytes); they are not fresh response/title/content/result-count limits.
The public JSON codec's 4 MiB bound applies to reads and conversion through its
reader. `ssx_json::to_string` and `to_vec` delegate directly to the serializer and
do not impose that byte cap. It would be incorrect to infer a global output cap
from the parser limit or optional persistence refusal.

A06 must therefore report two separate quantities: the supervised operation's
allocation peak, and the finite allocation envelope of each produced batch.
Later application retention and copies are governed by their own owners and
limits; no global memory guarantee follows from the operation ledger. A broad
response-type or pipeline ownership redesign has not been authorized. Any needed
handoff correction must first identify the smallest compatible owner boundary,
with actual caller/flight/cache/snapshot evidence rather than treating a parser
budget as a lifetime quota.

The isolated parser-budget packet is development evidence only. Its opaque root
owns backing reservations, but exposing borrowed `Value` does not prevent callers
from cloning it. Mandatory helper consumers require an explicit audit of copies,
nested scratch and output storage. Conservative root reservations for discarded
duplicate fields and numeric token scratch remain held until root disposal and
can cause earlier operational refusal than a live-allocation-only policy.

The integration owner assigned this boundary explicitly: A06 issue918 closes
per-produced-batch output size, operation peak, cleanup and transfer evidence.
G02 issue937 must qualify integrated retained-result memory, clones and caller
lifetimes under overload. That required gap remains open; it is not waived or
claimed closed by A06's component tests. A06 proceeds without a broad response
schema redesign.
