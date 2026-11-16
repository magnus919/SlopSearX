# F13 source parity and intentional differences

Pinned source is commit `35d97ccf8c9b2d8246373b2024cc8d7a457705a3`. The corpus binds nine exact source/regression files. It contains 130 offline observations; the initial 96 remain unchanged. Python source executions use isolated fixed time/UUID and fake store/network clients, with environment cleared. Native qualification uses private actual Valkey and independent clients. No provider traffic is evidence here.

| Boundary | Preserved behavior and native evidence |
| --- | --- |
| Source base keys | 29 query/scope/date/media key cases, sorting and duplicate distinction; six query-only answer key cases; `source_base_keys_answers_ttls_and_views_replay_exactly` |
| TTL/views | 15 configuration/TTL cases, six full/include/slice cases and current-request negative view; immutable nested evidence and all admission classes; actual news/partial PTTL assertions |
| Snapshot payload | Actual source writer to native store; 13 legacy reader cases; full scope writer/legacy reader projection; JSON-safe F05 engines and payloads |
| Legacy iterable projection | Five actual scope iterable/falsy/noniterable observations and eight lineage cases, compared against stored bytes without rewriting; snapshot-local compatibility leaves general F05 unchanged |
| Durable identity | Source opaque handles, tenant equality, stable unsliced pages and result IDs; actual native bytes reverse-read by actual source reader |
| Time | Exact equality remains live; expired retained records, physical expiry, backward jump and independent fresh service instance retain wall-epoch semantics |
| Authority | Missing, malformed, wrong type, unavailable, reconnect and terminal close are distinguished internally; no requery port exists |
| Atomic creation | Two clients contend on SET NX; first bytes and TTL unchanged; actual committed lost reply produces CompletionUnknown; malformed reply is distinct; no new ID after uncertain write |
| Optional storage bounds | Source count 10,000/10,001 observations retained; both native persist when under 4 MiB. Source >4 MiB write observation retained; native declines optional persistence and preserves complete fresh evidence |
| Native-v1 correction compatibility | 100 independently captured pre-correction native identity triples plus explicit null/empty/order differential checks; separate from actual Python source evidence. Oversized query/routing identities hash without persistence limits; aggregate oversized snapshots refuse without cursor or mutation |
| Routing | Ten source helper contributor observations compared by equality effect; native fingerprint also varies every explicit contributor, timeout and suggestions. Full composed bytes are versioned, not claimed source-identical |

Intentional differences are narrow and source-observed:

* Python SnapshotStore publishes an ID even after a failed best-effort SET, and connected read failure collapses to missing. Native authoritative creation returns failure/unknown without cursor; unavailable reads stay unavailable. This is the issue's authority correction, not a source-parity claim.
* Python cache startup waits five local seconds after a failed connect; native reuses accepted F07 reconnect-on-next-operation and its aggregate budget/terminal close. No separate backoff authority is introduced. Source disconnected negative writes are no-ops; native writes attempt the bounded F07 connection and report the actual outcome instead.
* A payload under one handle may contain a different ID in Python. Native rejects it as malformed without rewriting; public lookup is missing. Omitted empty snapshot/query IDs, inconsistent total and expiry-before-creation are rejected by existing accepted F05 constructors. Five captured integrity cases explicitly retain the differing source outcome.
* Python writes non-object answer values and later reads them as misses. Native object-typed answer writes reject before replacing an existing good entry; the five source cases and native preservation assertions retain this distinction.
* Negative timestamp metadata is ignored by source presentation. Native preserves the same error outcome and keeps malformed timestamp metadata unknown. It does not manufacture an observed timestamp.
* Native uses existing bounded JSON/payload/work-group storage policy. Fresh canonical construction is unrestricted by optional persistence; no result-count threshold is added.
* Native positive typed TTLs reject nonpositive negative-cache retention rather than defer failure to Valkey SETEX. Source normal/partial TTLs already validate positivity; source helper-only partial=0 without partial mode is recorded separately from SearchCache constructor validation.
* Full composed cache identity uses a cold versioned namespace. It preserves source input distinctions, not old hash bytes. No durable handle derives from a cache fingerprint. Rollout/rollback remains G04/G07-owned.

Regression mapping: `tests/test_cache.py` covers base/answer keys, TTL, store lifecycle and serialization; `tests/test_cache_integration.py` canonical full-response reuse and include/max views; `tests/test_mcp_snapshots.py` creation/lookup/tenant/expiry/pagination/result references. Native tests reproduce the shared boundary and error outcomes. Transport policy/enforcement belongs to accepted F09 plus future T02/T03; actual pipeline admission belongs to F14/F15. Offline helper observations do not establish those routes or execution integrations.
