# W00 state compatibility and intentional differences

## Preserved contract

The baseline is accepted main `66b29c3f8437af45a4f0d28f7c54a0f0727f7fa5`.
Source authority includes saved_models.py, saved_store.py, saved_runner.py and the
actual saved-tool validation/fingerprint functions in mcp/tools.py. The runner
is read to derive schedule semantics, not executed by native qualification.

All existing F07 Lua bytes and generic key/identifier APIs remain unchanged.
Definitions keep the `mcp:saved:v1` namespace, literal tenant components, source
record field names, separate baseline key, definitions/count/due/recent indexes,
global tenant hints, delete tombstones and report cleanup keys. Physical expiry
retains the source 300 second margin; schedule leases last 60 seconds. Recent order
is descending `(created_at, search_id)`, with 200 maintained entries and strict
exclusive page markers. Scan continuation is explicit and is never quota proof.

The decoder retains source scalar rehydration and optional-field defaults within
bounded JSON/finite-number/i64 ranges. Timestamp rehydration produces floats,
which matters to the ASCII-compact SHA256 saved identity. The source window sorts
engines; storage/selection preserves engine order. Slot/missed-slot arithmetic,
first/immediate runs and persisted-recovery run identity are pure source logic.

Claims deliberately retain Valkey cjson re-encoding. It rounds durable timestamp
values; e.g. `1700000000.123456` becomes `1700000000.1235`, changing the run identity.
Native compares the original raw snapshot before mutation, writes the guarded
cjson result and decodes the actual committed bytes. It does not substitute a
bespoke rounding algorithm or hash the pre-claim DTO. Native tests compare exact
committed bytes and run IDs with the accepted source-exact F07 claim script.

Validation preserves raw engine count before deduplication, exact bool/integer
checks, Python whitespace stripping, optional null defaults, retention/lifetime
clamping and strict revision guards. Only supplied numeric update fields are
validated against newly lowered limits; unchanged historical bounds survive
unrelated edits. Scope/interval edits and pause/resume reset schedule/baseline
state as source does. Durable CAS always clears the separate baseline. The saved
grant is required for pure preparation; engine revocation does not prevent
pause/resume control. The separate store remains an internal tenant authority,
not a public authorization surface.

## Reviewed native hardening

- W00 rejects colon-containing saved IDs at its storage boundary, preserving
  generic SavedSearchId and arbitrary supported tenant strings. Generated IDs
  are unchanged. Hand-injected malformed colon IDs require repair/reissue before
  native access; they cannot alias another tenant's record. Every mutation and
  reconciliation binds the payload tenant/ID to its exact canonical key.
- Every additive Lua mutation preflights all fixed/dynamic key types and numeric
  retention arithmetic before writes. Raw-record CAS supplements revision/token
  checks, preventing lost updates under shared revisions or tokens. Wrong types,
  malformed authority and bounded repair exhaustion do not partly publish quota
  admission or repair. Malformed optional baseline expiry is a typed error.
- Valkey TIME at script linearization controls logical expiry, due and quota
  liveness. Caller/prepared timestamps cannot authorize an expired commit. The
  oracle preserves actual source stale-now acceptance as a hazard, rather than
  altering its expected result. Prepared future creation timestamps are refused.
  Claims also validate the cjson-committed authority before publishing ownership.
- Healthy atomic count/index consistency remains a fast path. Inconsistent
  indexes require a complete escaped literal-tenant repair scan, bounded to 64
  SCAN pages, 4096 visited keys, 4 MiB per JSON record and 8 MiB aggregate raw JSON.
  Byte bounds precede decoding; incomplete repair returns RepairIncomplete with
  no repair/admission write. Non-authoritative scans return continuation cursors
  independently. Global scanning can still need multiple pages despite MATCH.
- Repair retains the maximum live expiry horizon, rather than source repair's
  last visited definition TTL, so scan order cannot shorten another live index.
- Transport failures retain F07 Unavailable/CompletionUnknown/partial-Lua-error
  distinctions. Unknown mutating outcomes are not converted into lease loss and
  are never automatically retried. Caller reconciliation must read authority.
  Restart tests reconstruct a connection and read durable state before repair.

Authoritative records are bounded by accepted JSON byte/depth/finite-number
limits and i64 integer representation. Mutation preflight conservatively stays
below Valkey's signed absolute-millisecond expiry overflow. These are corruption
and resource refusals; stored ordinary timestamps and records are not migrated.
Global tenant discovery accepts canonically bound colon-free legacy IDs as well
as generated handles, rather than relying on the source substring heuristic.

## W04 handoff boundary

Claim's private token, exact committed raw snapshot and tenant-bound record are
ownership evidence inside the infra saved subsystem. W04 must reuse those private
fields internally for atomic guarded result/schedule publication; it must not
expose setters, arbitrary mutation closures or wire-deserializable claim tokens.
Renew/release compare current raw state and the lease token, so revised, paused,
deleted, expired and reclaimed owners cannot regain authority. Release preserves
source informational lease fields; the live lease key is authoritative.

A lease proves a valid current owner for guarded publication. It does not promise
exactly-once external execution: a process can crash after a provider request or
lose a commit response. W04 must conserve unknown outcomes, reauthorize before
dispatch and handle interrupted work. W05 must extend the atomic publication
boundary for outbox capacity/events; W00 does not fabricate an event guarantee.

The pure slot API rejects nonfinite subtraction/division intermediates even when
both input timestamps are finite; it does not mask NaN into slot zero. Window
projection serializes a typed DTO fallibly under the existing JSON boundary.
Identity and policy hashing construct values directly, so a pure hash is not
accidentally subject to persistence-size limits or a macro unwrap panic. Compact
actual-source observations cover these boundaries without committing giant
fixture strings. An early stale-clock fixture incorrectly hashed cjson's raw
integer timestamp instead of the source decoder's float; the retained failure
and corrected source rehydration are explicit evidence, not a native hash change.

## Independent-review corrections

A successful claim carries the separately stored live opaque baseline, matching
source SavedSearchStore.claim. Native reads and validates bounded baseline bytes
before acquisition; Lua compares their exact bytes/absence with that snapshot
alongside the definition CAS and uses its server TIME for baseline liveness.
A changed baseline conflicts without acquiring a lease. Read errors or malformed
baseline expiry likewise prevent acquisition. No post-commit baseline GET can
turn an acquired lease into a false missing claim. A timed-out mutating request
remains CompletionUnknown; callers read authority instead of retrying it.
Hydration changes the read-only claim view, never its exact committed raw CAS
snapshot. W04 owns interpretation of the opaque comparison data.

Revision updates use a private Rust exact-integer proof: decode the original raw
snapshot, bind tenant/ID, checked-add one within the existing i64 model, and verify
the prepared next revision. Lua's exact raw CAS binds that pair at commit. Lua
therefore does not perform double-precision revision addition. Retained-byte
lifecycle paths use exact raw proofs, including the bounded repair protocol below.
The cjson-reencoding claim boundary is unchanged. Source observations include successful MAX-1 to MAX and
MAX to MAX+1; the latter is deliberately refused natively before mutation under
the existing i64 representation limit. Concurrent stale snapshots still conflict.

Supplied query text uses the current configured max_query_length before Python
whitespace stripping. An unrelated numeric edit does not revalidate a historical
query against a newly lowered length limit. Shared engine active/enabled/sensitive
checks, saved grant and complete configuration fingerprint remain mandatory;
this exemption supplies no dispatch authority. Re-supplying that overlong query
is rejected. The original33 observations are unchanged;13 full-source store/tool
observations were appended after the independent four-counterexample review.

Valid i64MAX records remain repairable and deletable, with exact decimal revision
tombstones and subsequent quota admission. Delete/renew/release use exact typed
raw proof and CAS without reencoding record bytes. Ordinary healthy creation
keeps its existing fast path. Only a repair scan encountering rounded-boundary
revisions adds a proof roundtrip: its first bounded script response explicitly
precedes all writes; Rust validates returned exact key/raw pairs losslessly and
allows one retry. The second atomic scan verifies every proof snapshot and fails
Conflict on replacement, deletion or new unmatched boundary records. No transport
failure or unknown completion authorizes a retry. Each scan remains bounded to
64 pages/4096 visited keys/4 MiB per JSON/8 MiB aggregate raw bytes. Across both
attempts the maximum is 128 pages/8192 visited keys/16 MiB scanned raw; proof input
and returned raw are independently bounded to 4096 records/8 MiB. True MAX+1 is
rejected by Rust before any index/admission write. Claim's deliberate cjson
precision boundary is unchanged, rather than widened by lifecycle repair.

Baseline JSON must be an object consistently for load and pre-claim validation.
An actual absent key and an object with missing expiry both yield no live
baseline and may be claimed. Arrays/scalars are malformed before acquiring a
lease, preserving source dictionary expectations. Actual source also treats a
stored JSON null as absent; native intentionally rejects this present nonobject
under its existing object read boundary. That source observation is preserved
alongside the native refusal and unchanged-record/no-lease proof. Nine actual
whole-source observations were appended; all prior 46 observation fields remain
unchanged. The final native inventory includes 30 tests.

## Accepted-main integration provenance

The historical `source.json` (SHA256 `e73df07686f402a080cb7d57666eb084e4c2db9ea9f9a1204b90f77bdb831ffb`),
exporter and historical source pins are immutable. Normal integration of accepted
main `544c830724a322d05521dac7f902dbe93142c4a1` brings PR #874's bounded-date HTTP
contract. MCP's whole-file blob changes from
`061593ce45d3e2440e818703af6b50d6e6d99cf1` to
`d06ecd475b9b523addf79781d2762eddd92f4911`: `_run_search` adds date-filter metadata
and the science tool requires explicit OpenAlex scope for absolute dates. Saved
helpers and their shared query/engine/policy validators are unchanged. The related
service/server/formatter changes add date validation, canonical response proof and
HTTP/portal rendering, not saved-definition or claim logic.

`integration-source.json` is a separate actual offline recapture against those
accepted files. Its reviewed SHA256 is
`c500069182b9975539910e29683531759e0df375e049e8daea5b8664aa10aad3`.
All 55 observations and every metadata field equal the historical capture except
exactly `sources["slopsearx/mcp/tools.py"]`. The native gate independently pins the
current MCP blob, verifies the historical blob at the original baseline, and
checks that precise equivalence. It does not ignore an arbitrary metadata group,
replace historical outcomes or execute Python. This integration adds no saved
runtime behavior; W00 domain/application/store/scripts/tests remain byte-identical
to the independently approved final55 checkpoint.
