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

The native `cargo xtask check T03` gate is now implemented in
`crates/xtask/src/t03.rs` and mirrored by the `native-t03-browser` CI job. It
validates the HTTP and synthetic browser receipts, exact source and binary
identities, loopback-only traffic, joined cleanup, and browser budgets. Each of
the five browser scenarios runs on Chromium, Firefox and WebKit from the same
lock-matched `playwright-core` client; the receipt identity must name the
requested engine, and the suite inventory rejects missing, duplicated or
engine-substituted rows. Only the Chromium contract run carries the
CDP-throttled performance-budget profile; Firefox and WebKit execute the same
journeys without CDP and do not claim those timing budgets. Playwright WebKit is
the WebKit engine on Linux, not Apple Safari, and does not substitute for the
manual Safari gap. The CLI's top-level T03
summary string stays at its custody-frozen historical wording; the authoritative
engine coverage is the gate's own browser-scenario records and this note. Its
CLS budget checks both the landing document and populated results document
after a 250 ms settle interval; it reports the larger value and preserves both
samples. The Chromium interaction budget applies the declared CDP profile to
every measured page, including the separate storage-blocked context; a guard
rejects any budget sample from an unprofiled page. Firefox and WebKit journeys
remain unthrottled and do not claim this budget. This is an automation slice,
not completion of #898.

`crates/ssx-http/tests/t03.rs` runs a real native HTTP child process and saves whole response/status/header/call-count receipts with startup binary identity and joined shutdown receipts. Its eleven cases cover the 16 inherited literal portal assertions, captured browser enforcement without machine-wire changes, landing query/scope submission through both native buttons, and exact large-page state, malicious/optional/empty/partial/unavailable/grouped results, invalid and sensitive scope rejection without dispatch, cached error/429/disabled format recovery, the 4300-digit page limit, cookie noninterference and the 18-request blank-date matrix.

The existing T09 OIDC/TLS flow additionally passes its actually issued session cookie to a 60-request public-route matrix: GET/POST × `/`/`/search` × HTML/JSON/YAML/CSV/RSS × absent/valid/invalid cookie, plus landing. Bodies compare with only query ID and measured response time normalized. Cookie bytes are not stored in receipts. The original T09 assertions remain intact.

`portal_keyboard.test.mjs` executes the production script through Node's built-in test runner. It verifies j/k focuses a real result link, Enter remains native for the focused link/disclosure/buttons, and slash does not steal typing focus. This is a keyboard-handler unit test, not browser layout/accessibility qualification.

Local raw logs, per-command exits, source inventories and storage measurements are retained under `/Volumes/tank01/magnus/rust-builds/slopsearx/issue898-portal-20261103/`; the task ledger is its sibling `storage-ledger/issue898-t03-20261103.json`. Rust1.99 is used explicitly. The pinned toolchain emits a nonfatal `rust-objcopy`/missing `libLLVM.dylib` warning while stripping debug information; raw outputs preserve it.

## Remaining acceptance before T03 completion

- Complete source-assertion ownership/evidence reconciliation and review the new CI job against the protected workflow contract.
- Reconcile reviewed successor source identities for changed shared implementation paths; preserve historical bindings. Any future machine enforcement metadata is a separate compatibility change with narrow comparisons and negative controls.
- Chromium, Firefox and Playwright WebKit run the full deterministic journey set in the T03 gate. Real Apple Safari, 200% browser zoom, manual visual review and screen-reader (VoiceOver/NVDA) execution remain recorded validation gaps, not passes.
- Measure/enforce portal byte, rendering and client performance budgets separately from upstream latency.
- Complete exact-head hosted CI, portal/image acceptance and required independent review before merge. This core slice is not acceptance evidence for O04 production composition or an upstream engine's live behavior.

## Bounded publication-date proof presentation (#984)

The selected `PublicPortal` renders the closed `SearchResponse.date_filter` proof used by JSON/YAML. It shows requested and applied inclusive bounds, the selected `openalex` engine and `upstream:openalex` provenance. The request or scope-enforcement preview cannot fabricate that proof. A separate source-outcome message distinguishes returned results, a valid empty response, an unavailable upstream and an unknown acquisition outcome; enforcing constraints does not establish that results were acquired.

`ssx-http/tests/science_openalex.rs` extends the actual registered OpenAlex/shared-service/private-Valkey portal composition with one-sided leap-day and two-sided bounds through GET/form POST on `/` and `/search`. It resubmits both rendered forms, follows JSON/YAML and pagination links, checks exact upstream filter bytes and publication days, and preserves missing-proof reacquisition. Separate actual-upstream controls distinguish HTTP 200/empty from HTTP 503/error. A pure rendering negative control refuses proof inferred only from request or metadata.

This slice changes HTML presentation only. Shared validation, explicit audited OpenAlex-only scope, machine metadata and cache policy remain authoritative. These non-browser checks do not establish real Safari, VoiceOver, visual, performance or production-composition acceptance; current-head hosted portal acceptance and source-binding reconciliation remain required.

The source assertion inventory now derives its classification totals from all 98
rows and requires both summary count declarations to agree (47 unresolved,
33 candidate counterparts, 18 retained raw mapping gaps). The T03 gate also
rejects duplicate or missing row identities, unknown classifications and accepted
parity claims. These checks establish inventory consistency only; the source
assertions still require their original acceptance evidence. Current-main date
roundtrip and profiling controls remain alongside the empty-date-control journey
and source/media evidence in this candidate.

The HTTP receipt validator applies 17 frozen formatter literal predicates to
named native responses for landing, result, malformed-content and error paths.
The landing-form fixture starts with the darker theme and checks that the
response declares that default. Each result includes its original source row and current HTTP receipt
binding under `retained_formatter_assertions`. The check covers the entire
malformed-content response for the forbidden URL scheme and rejects missing,
duplicate or mismatched evidence. Source-owner review of fixture equivalence
remains pending. The historical inventory and its acceptance states are
preserved. The compact-results budget uses the original query and single ordinary result
in a direct native formatter fixture. The native renderer derives grouping
from the result. The gate measures that receipt’s exact UTF-8 bytes and its
level-nine gzip representation against the original 50 KiB and 15 KiB caps.
The browser landing-page budgets run separately.

The fast and slow trusted-click controls first record a bounded pre-input readiness
phase: the target is brought forward, a trial click proves Playwright actionability
without dispatching input, and snapshots taken before and during two sampled browser
animation frames confirm bounds within 0.5 CSS pixels of the initial sample,
visibility and hit testing. A controlled one-frame move-and-return must be rejected.
This is bounded sampling, not proof of continuous stability or physical paint.
Preparation measurements stay separate from the interaction samples. Ordinary
readiness has a 16,000 ms aggregate acceptance ceiling: up to 5,000 ms each for
bringing the page forward and trial actionability, plus the 6,000 ms observation
watchdog. The synthetic transient-movement control has a 21,000 ms ceiling because
it adds a bounded 5,000 ms setup hook. These are receipt acceptance budgets;
individual timers are soft deadlines and can overrun under host suspension. The
100 ms interaction budget, 1500 ms missing-input/state guard and slow, absent and
untrusted negative controls remain unchanged. The trusted click itself is also
bounded by the 1500 ms guard. Transient-control restoration has a separate 1,000 ms
bound; a restoration failure rejects the receipt but retains captured snapshots and
a separate diagnostic. A page-close failure is also recorded separately and cannot
replace the original measurement error.

## Formatter pagination context

`PublicPortal::search_with_context` accepts a typed `PortalRenderContext` with
optional `has_more`. It preserves the frozen Python formatter contract: unknown
availability offers “Try next page” only for nonempty results; explicit false
omits next navigation; explicit true offers “Next page” with its definite
accessible name, including when the current result list is empty. Previous-page
and exact query/filter/page serialization remain independent of this option.

The native HTTP `HtmlRenderer::search` path always uses the default unknown
context. Neither `SearchResponse` nor the HTTP server supplies an authoritative
upstream `has_more`; no availability is inferred from query parameters, response
metadata, result count, or snapshot remainder. Machine formats and their schemas
are unchanged. Pure renderer cases execute within the existing static formatter
test; they remain candidate evidence for F:L680/L681/L688/L689 and do not establish
HTTP upstream availability, hosted browser, human, or full T03 acceptance.
