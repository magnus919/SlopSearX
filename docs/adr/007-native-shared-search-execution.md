# ADR 007: Native shared execution and client admission ownership

Status: implementation under independent review for F14; production cutover remains G04/G07-gated.

## Context and decision

`ssx-infra::search::SearchService` owns Tokio execution. It composes the accepted application policy, scope, health, ranking, scholarly grouping, cache and snapshot contracts with native F08 transport and F10 Valkey authority. HTTP, MCP and workers must share clones of one service, rather than independently rebuilding its flight registry. Production metric/audit exporters remain O01; semantic provider reranking remains F15.

Every caller revalidates F09 authority and passes its client quota before cache or singleflight. A transport can retain opaque `ValidatedScope` from `prepare_scope` and pass it to `search_prepared`: dispatch revalidation reuses frozen Jev advice and does not repeat disclosure/billing. Runtime roots supply logical adapter generations, changing them when an adapter/configuration changes. Framed dispatch identity includes order, scope, generation, optional advice, interactive/suggestion settings and workflow isolation. `prefer_fresh` always receives a unique flight. Cache and flight identities never use process addresses.

Callers, flights and global engine concurrency have finite configured limits. Excess work returns `Busy` before another task is spawned. A flight owns its engine/suggestion descendants and joins cancellation before completion. Waiter join and final cancellation share one mutex, preventing a new waiter joining work that is being cancelled. A cancelled flight retains its bounded ownership slot until cleanup completes; a new same-key caller can temporarily receive `Busy` during draining. Dropping a caller future signals cancellation; explicit `SearchCancellation` supplies an awaitable final-waiter cleanup boundary. Repeated/concurrent shutdown cancels and joins every retained flight before closing explicitly owned F08/Valkey resources. Borrowed resources stay with their caller.

The execution clock starts at dispatch, preserving source semantics: engine semaphore queue time and all HTTP requests within one adapter consume the same overall budget. Routing, client admission and optional storage use their own accepted operation budgets, plus a configurable finite port backstop. Per-request `Session::set_deadline` cannot extend the outer execution budget. Noninteractive budget preserves the slowest configured engine and source ten-second floor / thirty-second serialized-sum cap. Interactive remaining time also caps reranking and suggestions. Synthetic scheduler/caller outcomes never become measured upstream health.

Capture the epoch and process-local calendar date from one `SystemTime` using the already locked Chrono 0.4.45 clock/std features. Resolve the relative window once per request, and use that anchor for dispatch, local post-filtering and cache identity. No UTC default replaces the source local calendar. Clock injection and subprocess TZ cases cover midnight and DST without mutating process-global environment in a concurrent test runtime. Cargo.lock gains only the direct dependency edge; package versions/checksums stay unchanged.

## Dedicated client quota namespace

F10 `EngineKey` deliberately excludes punctuation such as IPv6 colons. Hashing a client into a valid engine name alone would still collide with a valid custom engine of that name. The opaque native `ClientQuotaKey` therefore hashes exact nonempty client bytes and uses:

```
ratelimit:client:<64 lowercase SHA256 hex>:<Valkey-time bucket>
```

Provider keys remain exactly `ratelimit:<EngineKey>:<bucket>` and cooldown keys/Lua stay unchanged. Because EngineKey cannot contain `:`, the extra framed component makes these namespaces disjoint, including a custom engine named `client-<digest>` or `client`. Same client bytes share a counter; different bytes are independent. IPv6, tenant punctuation and whitespace-only nonempty identities are hashed without normalization. Empty/absent identities skip client admission as in source; direct construction of an empty client key rejects.

Client quotas still use accepted F10 server TIME, atomic increment/expiry, denial charging, exact window retention and explicit fail-open/fail-closed behavior. Closed/invalid authority never fails open. The client namespace is intentionally cold on native cutover and legacy rollback. G04/G07 must stop old writers and drain/expire the old window (or explicitly account for the admission reset); old keys coexist under their original TTLs. No durable workflow record, receipt, snapshot, lease or identifier is rewritten. This is an admission migration, not a claim of completed rehearsal.

## Typed optional suggestions

Valid Brave responses and DDG fallback keep source query/key/ordering behavior; suggestions use the literal source `suggest:<sha256(lower(strip(query)))>` key and 1800-second TTL. Any Brave object member whose `q` exists but is not a string invalidates the whole Brave response and triggers DDG. Valid siblings in that malformed response are deliberately discarded. Actual source captures retain its contrary nullable/nonstring output. Failed fallback yields an empty cached list; suggestions never become dispatched search queries. Invalid optional cache records are misses. Production honors source environment transport settings; controlled fixture endpoints disable ambient proxy discovery.

## Consequences and evidence

PR874 adds a common all-surface absolute-date constraint after malformed-bound validation: exactly explicit OpenAlex, no media or relative time-range value, and both audited upstream date declarations. A closed optional `PublicationDateProof` records fixed provenance with immutable requested/applied bounds. Legacy unbounded storage omits it to preserve accepted bytes; bounded entries without matching current-request proof and admitted scope are misses. Unlike the source's self-consistent corrupted-scope cache acceptance, native execution compares against current authority. The separate actual d207 corpus retains that contrary source result. T02/T06/portal must expose this proof in their meta envelopes; A06 owns actual OpenAlex adapter enforcement.

Typed unavailable outcomes expose only policy-checked previews. They do not mint dispatch authority, and policy failures still precede availability. T02 maps these outcomes to HTTP 503 and T06 to service-unavailable envelopes; empty scope never charges client quota or reads cache.

Full immutable canonical evidence precedes request slicing and optional persistence. Deadline-truncated and rerank-fallback outcomes cannot populate the canonical cache. Oversized/unavailable persistence does not discard fresh results; uncertain snapshot completion never publishes a handle. F15 receives an optional typed stage with skipped/applied/fallback outcomes and remaining deadline; an injected stage test is not acceptance of the future provider implementation.

See [F14 parity and evidence](../rust-rewrite/f14/README.md) and [the state register](../rust-rewrite/architecture/state-compatibility.md). Native gates use pinned immutable source observations, real loopback adapters and private Valkey, with no Python runtime execution. G08 retires the developer-only exporter and legacy-source validation paths after the rollback window while retaining immutable evidence. No provider, deployment, relevance or performance improvement is established here.
