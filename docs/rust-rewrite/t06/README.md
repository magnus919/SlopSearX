# T06 native search and discovery

`ssx_mcp::search_surface::SearchSurface` implements the eleven source search/discovery tools, four resource registrations and five prompts. It uses the real shared `SearchService`, snapshot authority and passive runtime discovery. The T01 `Registry` remains a synthetic transport fixture; public native search does not return its fake outputs.

The composition root constructs `SearchState` from the same service, snapshot store, registry/config/catalog/policy and typed diagnostics used by dispatch. HTTP mounts the generic shared router under `NetworkAuth`; stdio and legacy factories bind the transport's existing `WireState`. Legacy additionally binds the verified stable OAuth client identity from the authenticated GET session. No tenant or native-codec authority comes from request arguments or metadata. Injected fixture engines replace only trusted runtime wiring; they retain the real handler, transports and auth.

Search authorizes through the shared fail-closed policy gate. Preview uses the read-only shared preparation seam and cannot grant dispatch authority. Search executes once, then persists the owned unsliced/include-independent capture before deriving the current presentation. Snapshot capture verifies service identity. Cancellation reaches preparation, preview, execution and persistence; completion is published only after owned work returns. The existing bounded per-connection request map retains cancellation ownership until completion, then the pump releases its admission/overlay/legacy permits. Private SDK generations prevent late cancelled output from affecting a reused caller ID.

Native output restoration is independent from native input validation. Only trusted factory catalogs select handlers. Marked successful tool text must be a real object JSON document; the shared overlay restores that exact raw object into `structuredContent` and removes internal metadata on every path. The SDK-facing placeholder contains no duplicate large object. Output-only W05 handlers do not opt into argument replacement. Default input/output limits and pending admission remain the accepted T01 contracts; typed output limits can select the separately qualified 32MiB profile. These are component ownership bounds, not a global RSS bound or gateway claim.

Production Python HTTP uses `slopsearx/mcp/security.py::make_http_app`, explicitly `stateless_http=True, json_response=True`, called by `server.py` and the fixture app helper. The native generic HTTP mount preserves that topology. Historical `capture.WireClient` instead called the SDK's default `http_app(transport="http")`; those stateful reference fixtures establish payloads and validation, not a production HTTP session/cross-request cancellation contract. Actual native disconnect cancellation is tested separately. Stdio and legacy actual RPC cancellation are qualified before EOF/disconnect.

`search-qualification`, `wire-qualification` and `cancel-wire-qualification` are explicit qualification-only processes. The first requires an exclusive owned private Unix socket and optionally an explicit loopback routing fixture. It never chooses fake engines as a public startup default. The last deliberately diverges WireState/Overlay to inject a buffered late SDK-generation fault; that test proves pump ordering, separately from real SearchSurface ownership drain.

Prompt-envelope replay preserves the original errors for null parameters, missing name and non-string name through all three transports. A separate accepted finite framework-diagnostic difference covers invalid outer scalar/array JSONRPC parameters; see [framework-differences.json](framework-differences.json) and [PARITY.md](PARITY.md). Its actual client rejection/auth-before-parse evidence does not imply SDK-wide diagnostic parity.

Complete production HTTP server packaging, all-adapter construction and all-workflow composition remain T02/W09. The handoff is concrete: construct the shared authorities, construct `RuntimeDiscovery` with observed diagnostics, construct `SearchState`, mount `SearchSurface` with the same `NetworkAuth` and shared factory/limits. No complete production startup claim is made here.

The qualification fixtures support the Linux AMD64/ARM64 CI targets and macOS; Windows fixture support is a separate obligation. Native qualification uses a locked Rust toolchain, private Valkey8.1.10 and loopback/subprocess fixtures. `t06_source_bindings` hashes original profiles/strategies/metadata and frozen observations without executing Python. See [PARITY.md](PARITY.md), [G08.md](G08.md) and [coverage.json](coverage.json). Final gate/CI results are recorded by the parent delivery task; development checkpoints are not acceptance.

Reproduce the native surface gate with `cargo xtask check T06 --output target/rust-evidence/T06.json`, using Rust1.99.0 and disposable Valkey8.1.10 on `PATH` or selected by `SLOPSEARX_TEST_VALKEY_SERVER`. The gate validates the pinned original bytes, frozen observations and exact named tests in `crates/xtask/src/t06.rs`; it executes the actual service, authenticated transports, ownership and source-drift assertions without Python or provider calls. The existing T01 gate additionally runs the full MCP package.

The configured `.github/workflows/rust.yml` foundation matrix runs this same T06 command on Linux AMD64 and ARM64. It retains `target/rust-evidence/T06.json` in `rust-foundation-<target>-<github.sha>`. Each native report records the actual head, working tree/diff/file bindings, toolchain, exact commands, statuses and test output. A delivery claim requires those reports on the final candidate; local review receipts supplement the repository-native commands and assertions.

The additive `storage-amendment/current-storage-amendment-v1.json` binds the
current T06 server fixture to the retained PR1026 preimage in
`storage-amendment/server.rs.7f8d.frozen`. It preserves the original native
fixture binding container and all source captures; the only fixture change
is to honor the registered root and process temporary directory on macOS.
The gate verifies the archived preimage and exact current source, and keeps
all other native fixture pins unchanged.

The additive [security lock amendment](source-amendments/current-lock-amendment-v1.json)
accepts only the exact `19fa47f4` successor of the original `uv.lock`. The shared
native verifier checks the current full-file SHA, byte count and regular-file
mode, reverses three exact package-entry spans, and requires the reconstructed
bytes to recover the complete historical lock hash. T06's source binding rows,
captures and expected outcomes retain their original hashes. The live lock
checks in T04, T05 and A03 use the same finite verifier; T01's historical Git
provenance and the historical manual-capture guard retain their original scope.

`filelock` 4.1.0 → 4.2.1 and `virtualenv` 21.14.5 → 21.14.10 are outside the normal
runtime dependency closure. `packaging` 26.2 → 26.3 is shared with the runtime:
`slopsearx → fastmcp → fastmcp-slim[server] → packaging`. The static audit follows
mandatory dependencies and selected extras, conservatively retains every marker
branch, and excludes the root development extra. It finds 85 runtime packages;
only `packaging` changes. All 135 other lock package entries are byte-identical,
including FastMCP, MCP, Pydantic and PyYAML. FastMCP 4.0.11 uses `packaging.version`
for component ordering, optional pydocket minimum-version checks, release checks
and prefab form-default support, so an unchanged SDK entry is not by itself a
claim that the runtime profile is unchanged.

The retained [26.2](source-amendments/packaging262-wire.json) and
[26.3](source-amendments/packaging263-wire.json) packets compare the complete
38-tool catalog and 22 negative call results, including six T06 calls, without
normalization. The same CPython 3.14.7, FastMCP 4.0.11, MCP 2.3.0, Pydantic 2.13.4,
producer and measured Python sources were used; the new run selected an isolated,
hash-pinned `packaging` 26.3 overlay. All 18 non-lock T06 source hashes match those
measured sources. The overlay kept the historical source-lock guard; the packet
records the actual imported packaging version separately. The shared verifier
hashes both raw packets and the inert [producer archive](source-amendments/packaging-wire-producer.py.txt),
then compares the full descriptor/result JSON trees and source identities.
The retained [manual commands](source-amendments/packaging-capture-commands.json)
record the actual private environment, arguments, exits and raw output hashes.
These observations establish that finite comparison, with zero engine/provider
dispatch; they do not recapture all T06 outcomes or establish SDK-wide parity.
The original T06 captures remain observations of `packaging` 26.2.

The exact T06 source-binding test inventory adds four named amendment controls
for current/historical identities, individual changed-entry and SDK mutations,
extra/missing/duplicate inverse spans, row/path/hash/mode/manifest/Git controls,
and missing proof material. The existing exact inventory/count checks remain
fail-closed. This proof changes qualification only; no portal request, response,
rendering, policy or engine behavior changes.

## Real OpenAlex science boundary (#984)

The additive `science_openalex` target composes the actual registered OpenAlex
adapter, descriptor-derived policy declarations, shared `SearchService`, real
private Valkey cache/snapshots and authenticated MCP HTTP transport with the
pinned Rust SDK client. It reuses the unchanged A06 actual-date response and
request fixtures, the existing private Valkey server and owned loopback helper.
No fake OpenAlex engine, Python backend, gateway or external upstream is used.

The three source-declared tests require these assertions before completion:

- Lower-only, upper-only, equal leap-day and two-sided science calls preserve
  exact upstream GET filter bytes, publication days, selected source, public
  `meta.date_filter` and layer-qualified `enforcement`. Repeated calls hit the
  real cache without another upstream request; changed bounds use a distinct
  key. The MCP presentation bound leaves the canonical full response intact.
  Missing or mismatched stored proof requires actual OpenAlex reacquisition and
  typed proof/publication-day readback. Structured output must equal tool text.
- Missing/wrong bearer tokens and denied grants, unaudited/unavailable sources,
  invalid/reversed bounds and mixed/unsupported/sensitive scope never reach
  adapter admission, client quota or cache I/O. The undeclared `time_range`
  and `safesearch` arguments are refused at the MCP validation layer;
  no upstream request or shared record is created. The science grant does not
  confer sensitive-engine access.
- Actual successful empty upstream JSON stays empty success, while actual
  HTTP500 stays `all_engines_failed` with the OpenAlex error outcome.

For an explicitly requested inactive OpenAlex adapter, the existing shared
native MCP policy returns `invalid_scope` on `engines` before date-scope
resolution. F09 already asserts this fail-closed active-engine ordering; this
test keeps its zero-admission, zero-I/O and zero-state assertions. Python's
configured-enabled/missing-adapter path instead reaches its later date-scope
check and returns `invalid_input`. This records the existing surface difference,
not whole-#984 parity or a change to policy precedence.

Science's existing six-parameter API is `query`, `source_types`, `date_from`,
`date_to`, `engines`, and `max_results`. The frozen T06 descriptor and Python
signature do not declare `time_range` or `safesearch`. Their two negative calls
must therefore return `isError=true`, one exact FastMCP validation text block
(`unexpected_keyword_argument`, with the actual field/value), and no
`structuredContent`. They do not exercise application relative-date or strict
SafeSearch policy. The other ten cases retain `isError=false`, equal structured
content and JSON tool text, and exact expected error codes/fields. All twelve
retain both bearer-token refusals and zero admission, quota, cache I/O, upstream
requests and shared keys. Case-labelled full-envelope diagnostics aggregate
contract mismatches after each fixture is closed; successful calls retain the
original strict helper. This changes tests and documentation only.

Only the mixed OpenAlex/Brave and unsupported Brave date-scope cases register
and instantiate the existing real Brave adapter, add its actual engine binding,
and derive its eligibility from that registry. A synthetic fixture key and the
owned loopback base keep configuration local. This lets those cases reach the
explicit-OpenAlex date gate rather than an earlier inactive-engine refusal.
Their exact expected envelope remains `invalid_input` on `engines` with
`absolute dates require explicit engines=['openalex']`. Other fixture modes,
including positive OpenAlex calls, do not register Brave. If a regression
attempts Brave dispatch, the existing admission counter increments and denies
it before the adapter creates an HTTP session; the zero-acquisition assertion
then fails. All twelve authentication, I/O, upstream and state guards remain.

The existing T06 gate runs this separate three-name target in addition to every
original source binding, fixture and transport test. Frozen JSON, historic test
identities and policy defaults are unchanged. Portal impact: these are additive
contract tests, with no production request/response or browser-visible change.
This science MCP composition is extended by the native HTTP slice below.
T03 portal/date links, gateway and production activation remain separate work.
Source presence is not qualification: record exact-head native commands/results
and cleanup under the normal resource grant before claiming acceptance.


## Native HTTP to actual OpenAlex (#984)

`ssx-http/tests/science_openalex.rs` calls the accepted public native router
through a real Rust HTTP client on a loopback socket. Its trusted composition
uses the same registered OpenAlex adapter, descriptor-derived catalog, shared
service and private Valkey as the three MCP tests. The neutral test fixture is
`ssx-infra/tests/support/science_openalex.rs`; the original MCP test bodies and
three selector names are unchanged. No production module, dependency manifest,
lockfile, policy grant or frozen source inventory changes in this increment.

The HTTP target adds three named tests to the T06 gate. Its inventory is listed
and compared exactly before execution, alongside the unchanged MCP inventory.
The required Rust CI workflow also runs `cargo test --locked --workspace` and
strict Clippy over all targets, which includes this HTTP target.

- `native_http_real_openalex_routes_bounds_formats_and_cache_reacquisition`
  requires cold acquisition for GET and form POST on both `/` and `/search`
  for lower-only, upper-only, equal leap-day, two-sided and maximum-calendar
  upper bounds. The fixture checks twenty exact outbound request lines and compares returned URL/day
  pairs with the unchanged A06 date corpus. JSON and cached YAML must expose
  identical requested/applied proof, audited `upstream:openalex` provenance and
  OpenAlex status/identity. Separate bound keys remain in private Valkey;
  missing and mismatched proof each require another real acquisition and typed
  storage readback. An unbounded acquisition preserves JSON/YAML/CSV/RSS use.
- `native_http_date_denials_have_zero_real_acquisition` requires the reference
  HTTP 400 error/field behavior for malformed, invalid leap-day, empty or
  reversed dates, absolute plus relative constraints, implicit/category/mixed/
  unsupported scopes, unavailable and unaudited adapters, unknown/repeated
  controls and bounded CSV/RSS. URI and form controls share duplicate detection.
  Every denial must leave adapter admission, client admission, cache reads and
  writes at zero, with no upstream requests or private shared records.
- `native_http_real_openalex_failure_and_empty_have_distinct_status_outcomes`
  requires actual upstream empty JSON to remain HTTP 200 with status `ok`,
  while HTTP 500 becomes HTTP 503 with status `error`. Both public JSON and
  YAML retain bounded proof and engine outcomes.

Portal impact: this increment adds tests of accepted native HTTP behavior;
there is no browser-visible implementation change. Full T03 browser acceptance
is still open: the current form loses date/scope controls on resubmission, the
renderer does not display date enforcement, and machine-format links remain
unimplemented. Existing pagination date retention does not satisfy that full
journey. These finite fixtures do not close #984 or integrated parity, measure
retrieval quality or establish provider/production availability. Exact-head
commands, results and cleanup must be recorded before claiming local execution.

The [reference regression map and local execution identity](DATE_FILTER_INTEGRATION.md)
records covered assertions, original failures and remaining T03 acceptance.
