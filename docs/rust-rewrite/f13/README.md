# F13 canonical cache and immutable snapshots

F13 implements native cache and snapshot boundaries for issue #892. It uses accepted F05 result/response serialization, F07 shared Valkey ownership and explicit F09-derived identity inputs. No provider is called by these modules or their tests. F14 must supply actual execution admission, timeout and resolved calendar-window facts; F15 must supply actual rerank identity/fallback facts. Injected facts in these tests do not establish those future integrations.

`ssx_application::cache::CanonicalResponse` owns an immutable `Arc<SearchResponse>`. Construction is independent of optional persistence bounds. Views borrow the same full evidence and apply include/max-results only when presented. An owned view may be modified without affecting another request. Snapshots always capture the full canonical result list, preserving absolute `snapshot:index` result IDs across page sizes. Their lookup port has no search/requery capability. Scope fields are evidence, never authorization.

`CanonicalCache` is best effort: missing, malformed, unavailable and closed reads are misses; `prefer_fresh` bypasses reading. Writes return Stored, Skipped or NotStored with a classified reason. Complete/partial are caller-supplied admission classes. All-unresponsive, deadline-truncated, rerank-fallback and rejected outcomes are not cached. General/news/partial TTLs preserve source rules, including case-insensitive substring `news` and the 300-second cap. Configuration is explicit, with accepted F05 Python integer normalization and positive retention; the application never reads environment variables.

`SnapshotService` exposes found, missing, expired and unavailable outcomes. Wall-clock epochs are injected, expiry is strictly `now > expires_at`, and physical retention is logical TTL plus 300 seconds. Process restart does not reset a timestamp. A backward wall-clock jump can make a retained snapshot live again, matching source semantics. F07's separate quota time authority is unchanged. Missing/expired/wrong-tenant/malformed/unavailable reads never invoke upstream engines. Public integration must preserve these distinctions rather than turn storage outages into requery.

`ValkeySnapshots` uses the legacy `mcp:snapshot:<tenant>:<opaque id>` key and source payload shape. It validates tenant and handle independently. Cryptographic IDs retain `snap-` plus 12 lowercase hexadecimal characters. Creation uses one atomic `SET EX NX`; known collisions may retry at most three IDs. Unknown completion is explicit and never publishes a cursor or retries with a new ID. The borrowed Valkey client is not closed by repositories. Accepted F07 budgets, reconnect-on-next-operation and terminal close semantics apply. There is no duplicate local cache reconnect timer or flushdb operation.

Durable input uses the existing 4 MiB JSON byte limit, depth limit and F05 payload/work-group limits. There is no additional result-count cap: the 10,000 and 10,001 source cases both persist and read in full when bytes fit. Oversized fresh evidence remains complete and serializable for presentation while cache/snapshot persistence refuses explicitly. No cursor is returned on refusal. Malformed stored records are never rewritten during reads.

Pure identity tuples serialize without the optional durable byte limit. The snapshot writer uses a fallible typed record conversion, so a large aggregate of individually small results returns NotStored/no cursor rather than panicking. A separate 100-row pre-correction native-v1 compatibility fixture proves unchanged ordinary base/routing/composed bytes; it is native compatibility evidence, distinct from the 130 actual Python source observations. Explicit null/empty/order cases and all three oversized regressions are mandatory native tests.

## Identity and migration

`source_base_key` exactly preserves source `search:v2` query, sorted scope, language, SafeSearch, page, media and publication-window distinctions. `answer:v2` intentionally remains query-only, including the source language/SafeSearch collision; it cannot satisfy canonical search reads. Negative errors remain separate typed entries and derive current-request query identity at the presentation boundary. A malformed optional negative timestamp stays unknown, never fabricated as epoch zero.

Full composed fingerprints use `search:native:v1`. They capture active/enabled catalog, auth/configuration, cost, circuit, conditional observed health, sensitive/tier-one sets, budget, ranking, work-group and rerank identities, explicit routing extension, timeout and suggestions. Freeze these inputs once per operation. Full bytes are deliberately versioned rather than pretending Python helper dynamic objects are native F09 types. The pure source base key and source contributor equality effects are replayed independently.

The composed namespace change is a cold-cache transition only. Snapshot IDs originate independently in random issuance; result IDs derive from snapshot ID/index, not cache keys. No durable record is retagged. Legacy snapshots are read natively and an actual native-created record is reverse-read by the pinned Python reader. Native canonical response writes use the accepted F05 profile. G04/G07 still own deployment rehearsal, exclusive writer/cutover and rollback validation; no production migration is claimed. On rollback the Python composed cache is cold, legacy snapshot keys remain readable, and no cache flush is required.

## Evidence and downstream handoff

`cargo xtask check F13 --output target/rust-evidence/F13.json` is native-only and requires official private Valkey 8.1.10. It pins source/exporter/corpus/reverse record outside mutable fixture metadata, enumerates exact tests and rejects ignored/empty selections. Both native CI architectures retain the report. Source capture is optional migration tooling, not a runtime dependency:

```
/private/tmp/slopsearx-rust-oracle-py314/bin/python -B docs/rust-rewrite/f13/capture.py --source-root /clean/pinned/source --output crates/xtask/fixtures/f13/source.json
/private/tmp/slopsearx-rust-oracle-py314/bin/python -B docs/rust-rewrite/f13/capture.py --source-root /clean/pinned/source --output crates/xtask/fixtures/f13/reverse.json --reverse-record crates/xtask/fixtures/f13/native-record.json
```

G08 retires this exporter after the rollback window; immutable observations and native tests remain. The original 96 observations were retained before appending boundary cases; all source file pins remain unchanged. Parent independent recapture is required before acceptance. Fixtures alone do not prove provenance.

Portal impact: immutable include/max views and tenant-bound stable snapshot navigation are covered at these shared boundaries. T02/T03/W07 must wire the same services into actual HTTP/MCP/workflow presentation; this module does not claim those unfinished routes or browser cutover. F14 owns execution admission/deadline/coalescing, F15 rerank integration. Consumers must treat CompletionUnknown as uncertain, never as proof of no prior write.
