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
