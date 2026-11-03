# T03 public portal: core delivery

This is the first bounded implementation slice for [issue #898](https://github.com/magnus919/SlopSearX/issues/898), under the [Rust rewrite case study](../CASE_STUDY.md). It does **not** complete T03 or authorize closure of that issue.

## Production behavior

`ssx_http::portal::PublicPortal` renders the public portal using the existing Rust HTTP router and its single `SearchService::capture_read` boundary. `HttpState::with_public_portal(theme)` is the composition seam for later O04 server assembly. The synthetic `t03_fixture` example uses this real router and renderer with local engine fixtures; it makes no provider requests.

The renderer supplies query-preserving search/scope forms, exact page navigation, machine-format links, actual filter enforcement, source badges, dates, ranking explanations, retrieval eligibility, shared entity grouping, named partial/unavailable states, suggestions, and escaped error recovery. Its immutable discovery snapshot excludes sensitive engines. Result links accept only absolute HTTP(S); textual fields are escaped and highlights accept only attribute-free `strong`, `em`, and `mark` tokens. CSS and JavaScript are static Rust-included assets. Native forms and links remain usable without JavaScript.

The pure entity collector moved from MCP into `ssx-application::entity_projection`, without changing the grouping algorithm. Both MCP and HTML use that collector. The existing MCP frozen-source entity/Unicode corpus validates the extraction.

Error rendering receives only allowlisted public form fields through the default-compatible `HtmlRenderer::error_with_request` method. Cookies, headers, arbitrary parameters, configuration and credentials are excluded. Invalid fields are escaped; rendering does not dispatch or resolve policy again. The legacy T02 renderer keeps its previous behavior.

## HTTP compatibility

1. HTML receives the enforcement map from the already validated `ScopePreview` captured by the single search authority, on both fresh and cached reads. It does not trigger a second policy/routing pass. JSON and YAML retain their existing metadata contracts; adding their enforcement object requires a sequenced compatibility/proof change.
2. For negotiated **HTML only**, empty optional `date_from` and `date_to` form controls mean no bound. One-sided bounds remain supported. Explicit blank bounds in JSON/YAML requests retain their existing rejection before engine dispatch. This makes ordinary browser form submission work without weakening the machine contract.
3. The new portal has fuller HTML than the frozen legacy T02 shell. It is selected explicitly through the public factory; later production assembly remains O04 work.

Frozen T02 source captures, comparisons and manifests are unchanged. The initial proposed additive machine `meta.enforcement` was removed during independent review because it would invalidate exact-body qualification. A future machine-metadata addition must use reviewed successor comparisons that allow only that field, preserve every existing comparison, and include negative controls. Historical captures must stay immutable.

## Bounded local evidence

`crates/ssx-http/tests/t03.rs` runs a real native HTTP child process and saves whole response/status/header/call-count receipts with startup binary identity and joined shutdown receipts. Its ten cases cover the 16 inherited literal portal assertions, captured browser enforcement without machine-wire changes, landing query/scope submission through both native buttons, and exact large-page state, malicious/optional/empty/partial/unavailable/grouped results, invalid and sensitive scope rejection without dispatch, cached error/429/disabled format recovery, the 4300-digit page limit, cookie noninterference and the 18-request blank-date matrix.

The existing T09 OIDC/TLS flow additionally passes its actually issued session cookie to a 60-request public-route matrix: GET/POST × `/`/`/search` × HTML/JSON/YAML/CSV/RSS × absent/valid/invalid cookie, plus landing. Bodies compare with only query ID and measured response time normalized. Cookie bytes are not stored in receipts. The original T09 assertions remain intact.

`portal_keyboard.test.mjs` executes the production script through Node's built-in test runner. It verifies j/k focuses a real result link, Enter remains native for the focused link/disclosure/buttons, and slash does not steal typing focus. This is a keyboard-handler unit test, not browser layout/accessibility qualification.

Local raw logs, per-command exits, source inventories and storage measurements are retained under `/Volumes/tank01/magnus/rust-builds/slopsearx/issue898-portal-20261103/`; the task ledger is its sibling `storage-ledger/issue898-t03-20261103.json`. Rust1.99 is used explicitly. The pinned toolchain emits a nonfatal `rust-objcopy`/missing `libLLVM.dylib` warning while stripping debug information; raw outputs preserve it.

## Remaining acceptance before T03 completion

- Implement the self-contained `cargo xtask check T03` gate and source-assertion ownership/evidence reconciliation, sequenced after the active shared integration lease.
- Reconcile reviewed successor source identities for changed shared implementation paths; preserve historical bindings. Any future machine enforcement metadata is a separate compatibility change with narrow comparisons and negative controls.
- Run real browser journeys at desktop/narrow widths, 320px/200% reflow, focus/history/theme/storage cases, accessibility checks, available-browser matrix and raw visual artifacts.
- Measure/enforce portal byte, rendering and client performance budgets separately from upstream latency.
- Complete exact-head hosted CI, portal/image acceptance and required independent review before merge. This core slice is not acceptance evidence for O04 production composition or an upstream engine's live behavior.
