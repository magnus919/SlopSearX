# F14 shared execution qualification

Baseline: `730c37a5f48b9830088594582cefb4c2f10a35e8`. F10–F13 are accepted prerequisites. This lane implements native runtime composition in `ssx-infra`; application policy, ranking, health, cache and snapshot contracts remain their accepted owners' APIs. F15 owns semantic reranking/advisories; O01 owns production metric/audit exporters; T02/T06 own public transport composition. No providers, production state, deployment or performance claim is authorized.

The complete frozen source packet passed independent full review after correction of three demonstrated defects. Signed-off checkpoint `2c7cdf44ada9b4d42c4d69d9279bf12f001957af` was merged normally with freshly fetched upstream `544c830724a322d05521dac7f902dbe93142c4a1` at `04117109063e915188f720955b1992d86326b746`. All 35 reviewed files survived that merge byte-exact. Only the eight independently reviewed PR874 current-source F01 anchors are refreshed; historical source/corpus bindings remain intact. Current-source qualification and parent acceptance remain separate from source-review approval; no F14 milestone acceptance or publication is claimed here.

The independently recaptured historical corpus contains 56 cases and 24 source bindings; its first 48 cases remain byte-identical to the availability freeze (and its initial 46 cases retain the original freeze). A separate 22-case, 11-binding corpus captures PR874 at `d207d4496129d496fe8a74b040ebe1a71151b6f9`. Only query IDs and elapsed response time are normalized. Native tests consume immutable JSON; Python is developer-only reference capture and retires under G08.

Required claims and independent evidence:

| Claim | Evidence |
| --- | --- |
| Authority and per-client admission precede cache/singleflight | Denied mixed-sensitive/strict-SafeSearch, quota/cache-hit and duplicate waiter cases; zero unauthorized engine requests |
| Canonical full results, isolated views/IDs/warnings | Source replay plus two concurrent include/max callers and full snapshot readback |
| Identical work coalesces; fresh/workflow/generation/order isolate | Controlled concurrent execution and exact outbound request counts |
| Finite admission and queue deadlines | Saturated real loopback adapter; bounded callers/flights/global dispatch; no never-started health observation |
| Aggregate budget survives per-request deadline refresh | Real multi-request F08 loopback session refreshing deadlines, service's outer deadline cancels it |
| Cancellation/shutdown own descendants | One waiter cancels without affecting survivor; final explicit cancellation joins; repeated concurrent shutdown joins tasks and closes owned F08/Valkey resources |
| Cache is optional and never stores truncated evidence | Disposable Valkey complete/read/partial/deadline, oversized persistence refusal and outage cases retain full fresh result |
| Suggestions execute natively | Brave primary, DDG fallback, normalized Valkey key/TTL, empty/malformed/outage and cancelled request observations |
| Upstream health is distinct from scheduler/caller cancellation | Accepted HealthState origins with observed versus synthetic metrics; live circuit/cooldown skip |

Source/API inventory expands beyond the issue's original five anchors: `adapter.py`, `cache.py`, `snapshot.py`, `filters.py`, `merger.py`, `scholarly.py`, `router.py`, `capabilities.py`, `ratelimit.py`, adapter observation methods, result serialization, plus deterministic tests for these paths. Existing F09/F10/F11/F12/F13 scoped gates remain authoritative for their primitives; F14 proves their execution composition.

The bounded caller/flight limit deliberately rejects excess native work before spawning another task. Process addresses never identify an adapter: composition supplies a nonempty logical generation and changes it on rewiring. A dropped future signals cancellation by RAII; `search_with_cancellation` provides an awaitable final-waiter cleanup boundary and service shutdown joins every retained owner. Borrowed adapter resources are never closed implicitly; roots explicitly transfer owned transports/stores for shutdown.

Portal impact: shared policy/enforcement, scope, result serialization, cache views, snapshot IDs and partial/deadline fields remain the browser contract. F14 supplies the same service to future HTTP/MCP/worker roots. Native service tests do not establish a finished HTTP portal or browser cutover.

## Unavailable scope and date amendment

`SearchError::Unavailable` carries a policy-checked preview and a typed reason, never dispatch authority. Both direct execution and prepared scope preserve policy failures first and return without quota, cache or engine work for an empty scope. T02 maps this outcome to the existing HTTP 503 availability response; T06 maps it to its service-unavailable tool envelope using the preview. Invalid explicit scope, denied grants and strict-filter rejection remain policy errors. Neither successor may manufacture a `ValidatedScope` from the preview.

Absolute publication bounds require exactly explicit `engines=["openalex"]`, no media constraint and no `time_range` value (including an empty string). The common F09 preview validates malformed dates first and checks both audited upstream date declarations before optional Jev. The closed `PublicationDateProof` supplies exact requested/applied bounds and fixed provenance; it is evidence, not authority. F14 compares a bounded cached response against both the current admitted proof and current scope. Source PR874 accepts a corrupt cache whose scope and proof engines are both changed to Brave; the captured `cache-scope-self-consistent` case records that contrary behavior. Native code deliberately rejects it. Legacy unbounded records omit the optional proof, preserving accepted fixture bytes; bounded pre-contract entries miss. T02 JSON/YAML/portal meta and T06 tool envelopes must expose the proof, and HTTP query validation must preserve the current source rejection contract. A06 still owns the actual OpenAlex adapter: F14's proof tests establish composition and audited declarations, not upstream filtering by an unaccepted adapter.

F09's 15 historical native tests remain unchanged and pass; pure bound/filter observations retain their meaning. F13's legacy codecs retain omitted-proof behavior. Neither old corpus establishes the new bounded service contract. The separate amended corpus and named F14 tests provide that evidence. The exact source freshness bridge preserves original baseline Git/SHA bindings and accepts only the reviewed PR874 current bytes, with no behavioral skipping or historical repinning.

Developer reproduction (not a runtime or gate dependency):

```sh
/private/tmp/slopsearx-rust-oracle-py314/bin/python -B docs/rust-rewrite/f14/capture.py --source-root /clean/730c/source --output crates/xtask/fixtures/f14/source.json
/private/tmp/slopsearx-rust-oracle-py314/bin/python -B docs/rust-rewrite/f14/capture_dates.py --source-root /clean/d207/source --output crates/xtask/fixtures/f14/dates.json
```

## Independent review corrections

The first full-source review demonstrated three uncovered defects. Durable native regressions now suspend cooldown admission while another search opens the circuit; the circuit is checked after cooldown and semaphore awaits immediately before engine invocation. An internal engine-task panic remains a visible synthetic error, with a separate internal-failure metric and no upstream origin, health observation, measured latency or circuit charge. Six repeated deliberately contract-violating panics prove that separation. Eight additive actual HTTPX source captures and real Brave/DDG loopback requests prove Python quote_plus bytes, including `*`, `~`, UTF-8, spaces, percent/reserved characters and controls. All historical 48 source objects and 24 bindings remain exact; the date corpus and freshness bridge are unchanged.

## #984 actual OpenAlex core integration

`real_registered_openalex_dates_survive_service_valkey_cache_and_reacquisition`
in `crates/ssx-infra/tests/f14.rs` joins the registered A06 OpenAlex adapter,
the shared SearchService, and the existing private F14 HTTP/Valkey owners. It
reuses the unchanged A06 equal-leap capture: exact outbound filter bytes and
publication-day defense, audited enforcement and typed date proof, a cache hit
without another upstream request, distinct date-bound cache identities, full
canonical results despite a bounded view, and real reacquisition after missing
or structurally valid but mismatched cached proof.

This is the independent core slice of [#984](https://github.com/magnus919/SlopSearX/issues/984#issuecomment-6379736916),
corresponding to #874's `test_absolute_date_cache_requires_proof_and_separates_bounds`
and the shared-service portion of
`TestAuditedOpenAlexDates::test_science_dates_real_adapter_mixed_scope_and_cache`.
It does not replace the existing A06 adapter or F14 policy/cache tests. Its
source is prepared; execution evidence must be recorded separately. It does
not qualify HTTP GET/form POST, MCP transport, or portal/browser behavior;
those public-boundary obligations remain open in #984 after accepted T02.
