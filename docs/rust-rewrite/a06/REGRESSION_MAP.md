# A06 regression mapping in progress

Baseline: `730c37a5f48b9830088594582cefb4c2f10a35e8`.
This is an implementation checklist, not evidence that native coverage exists.
The initial 67 observations remain immutable. Add supplemental actual source
captures for gaps, and replace each pending native entry with an executable test
name and gate before accepting issue #918.

## Direct adapter regressions inspected

| Existing source test | Required native assertion | Initial packet / remaining gap |
| --- | --- | --- |
| `test_openalex_adapter.py`: registration and categories | Factory instantiates OpenAlex; science metadata remains discoverable | Native factory pending |
| `test_search_returns_results`, `test_search_content_from_abstract`, `test_search_doi_fallback_to_id` | Whole result metadata, score/date, ordered abstract reconstruction, fallback URL and payload ID | Success and DOI observations exist; add the original two-work fixture and absent abstract |
| `test_doi_forms` | Preserve HTTPS DOI, HTTP dx.doi.org URL, bare DOI and blank fallback independently | Initial corpus lacks explicit HTTP dx.doi.org and whitespace-only cases |
| `test_search_empty_results`, `test_search_error`, `test_search_timeout` | Empty success versus classified failure; no exception escapes | Initial captures exist; native replay pending |
| `test_elapsed_latency_and_classified_outcomes` | Every success, empty, timeout, 429, 500 and malformed path measures elapsed monotonic time | Exporter omits latency deliberately; require native clock/lifecycle assertions, not corpus count |
| `test_malformed_payload_never_raises` | Whole failure and no partial results for all five malformed nested payloads | Root-shape cases are insufficient; capture original nested abstract/citation/relevance cases |
| `test_search_preserves_upstream_relevance_order` | No added sort parameter; preserve order across at least two works; exact C++ query | Single-work success is insufficient |
| `test_local_rate_limit_is_timed`, `test_rate_limiter_failure_never_raises` | No HTTP call on denied admission or admission failure; classified and timed outcomes | Requires injected admission and real shared F10 composition |
| `test_inclusive_date_filters_and_unproven_dates` | Leap-day inclusive lower, upper and equal bounds; discard absent, year-only and malformed publication dates | Initial four date cases do not cover the six-row source fixture |
| `test_invalid_date_constraints_never_dispatch` | All seven original invalid bounds/time-range cases fail before admission and network | Initial bad/reversed cases cover only a subset |
| `test_arxiv_relevance.py::test_plain_multiword_queries_use_phrase_search` | All ten original query variants; one request, max_results=5, relevance ordering | Add OR, ANDNOT, lowercase and, parentheses and both explicit ti query variants; retain existing Unicode/whitespace captures |
| `test_semanticscholar_cooldown.py::test_retry_after_is_bounded_and_invalid_values_are_safe` | Numeric, HTTP date, past date, malformed, negative and oversized values obey defaults/cap | Initial variants need exact original 45-second, invalid and 999999 cases |
| `test_429_header_variants_use_bounded_cooldown` | Actual adapter publishes bounded cooldown with each original header | Capture alone cannot prove shared-store publication |
| `test_semantic_scholar_429_cools_down_and_recovers` | First 429 publishes cooldown; second request makes no HTTP call; expiry permits recovery | Requires native lifecycle test with counted loopback calls and controlled time |
| `test_upstream_cooldown_is_shared_without_counting_local_strikes`, `test_valkey_publishes_bounded_cooldown_key` | Two clients share cooldown; shorter update never shortens TTL; cap at 300; no local error strike or quota acquisition while cooling down | Reuse accepted F10 interfaces and real isolated Valkey; do not substitute local-only timer |
| `test_service_skips_provider_during_cooldown_and_health_stays_rate_limited` | Real adapter plus F14 service: no second provider request, health remains rate-limited, consecutive errors unchanged | A06 adapter test plus explicit F14/G01 composition handoff |

## Cross-surface inventory still to resolve

Text references were found in `test_adapters.py`, `test_tier1_adapters.py`,
`test_filter_semantics.py`, `test_mcp_enforcement.py`, `test_service.py`,
`test_service_engine_config.py`, capability/catalog, payload/entity/formatter,
Jev/routing/advisory, scholarly grouping, research and coverage-evaluation suites.
A text match is a discovery pointer, not proof of an A06 responsibility.
Inspect individual assertions and assign each to A06 or its owning successor;
retain provenance through G01 and O05 rather than dropping shared regressions.

In particular, `test_scholarly_arxiv_html_identity.py` covers grouping of HTML,
abstract and PDF URLs, version preservation, deceptive hosts, payload conflicts,
provenance and cache policy identity. Those are grouping/integration contracts;
A06 must preserve the adapter's raw identifiers and revision metadata needed by
the accepted grouping implementation. It must not replace grouping tests with an
adapter URL-normalization assertion.

The initial packet and this map are incomplete until every relevant shared
assertion has an owner and runnable successor. Parser/codec decisions identified
in A05 remain explicit dependencies for arXiv response-text fidelity.

## Supplemental capture receipt

The source-input additions requested by the direct-test rows above are now
captured in `source-regressions.json` (38 records, SHA256
`d060b8a53f77eaeb04c62cfa043bbfc65585a542edb2bbb44a592cdb8d1ccb12`).
This supplements rather than rewrites the initial packet. Reproduction uses
`export_source.py --regressions`; default mode still reproduces all initial
bytes. Native tests and the shared-test ownership audit remain pending.

## Shared-test audit and ownership (2026-10-16)

Inspected the actual assertions rather than assigning ownership by engine-name
matches. The following allocations are obligations, not claims of passing native
successors. Adapter source and original shared tests below are pinned to the A06
baseline. Date integration observations were additionally inspected on current
main `544c830724a322d05521dac7f902dbe93142c4a1` after PR874.

| Inspected shared regression | A06 obligation | Integration owner / remaining evidence |
| --- | --- | --- |
| `test_adapters.py::TestArxivAdapter` | Default HTTPS endpoint; same-host HTTP-to-HTTPS redirect succeeds; external host rejected; redirected 429 classified; two ordered feed results and empty feed | A06 native loopback must observe both requests, including source omission of original query parameters on request2. Do not replace this with transport-only unit coverage. |
| `test_adapters.py::TestSemanticScholarAdapter` | Citation score/date, author list, DOI/arXiv payload, arXiv content, non-mapping externalIds ignored, configured x-api-key, empty/429 | A06 full-output and request assertions; shared source capture added below. Initial packet does not replace original two-paper order coverage. |
| `test_tier1_adapters.py::TestOpenAlexAdapter` | None/empty/simple/repeated-position abstract, registration, science/reference categories and absence of general | A06 abstract captures added; native factory/catalog completeness still pending. |
| `test_service_engine_config.py` | Semantic Scholar configured timeout/key must retain adapter default endpoint when blank base URL is omitted | F06 owns normalization; A06 must prove real factory dispatch consumes normalized configuration. New capture covers adapter-side endpoint/key/timeout only and does not prove service normalization. |
| `test_filter_semantics.py` mock arXiv enforcement rows | Preserve audited metadata; never infer that real arXiv implements time-range/SafeSearch because a mock declares them | F09 owns generic enforcement and scope semantics; T06 owns MCP projection. A06 must not change real adapter declarations to match mocks. |
| `TestAuditedOpenAlexDates::test_science_dates_real_adapter_mixed_scope_and_cache` | Actual OpenAlex emits upstream date filter and preserves publication dates | F14 owns exact scope/proof/cache and bounds identity; T06 owns science grant/enforcement wire response; G01 combines real A06 factory with both. Count two upstream calls across first/hit/changed-bounds/mixed-rejection. |
| `test_real_openalex_relative_window_drops_missing_and_old_dates` | Retain supplied publication dates and audited local enforcement metadata | F09/F14 own fixed inclusive relative window and postfilter; T06/G01 prove real adapter plus wire layer drops absent/malformed/old dates. |
| `test_absolute_and_relative_bounds_intersect` | No special adapter relaxation | Despite its historical name, current PR874 test now requires rejection of combined absolute/relative bounds. F14 owns this amendment, before dispatch. Do not port the obsolete test name as behavioral permission. |
| `test_relative_window_is_frozen_across_midnight` | Do not independently resample request-local date anchor | F14 owns single request instant, separate next-day cache identity, and caller-input immutability; G01 real-adapter composition. |
| `test_absolute_date_cache_requires_proof_and_separates_bounds`, `test_absolute_date_scope_requires_audited_adapter_not_just_name` | Register actual upstream date enforcement, not name-only eligibility | F14 owns proof validation/missing-proof miss/independent bounds keys and fail-closed eligibility; G01 requires real A06 implementation. |
| `test_scholarly.py`, `test_scholarly_arxiv_html_identity.py` | Preserve raw IDs, revision strings, author/DOI metadata needed by grouping | F11 owns grouping/ranking semantics (accepted 751-case corpus); G01 must run actual A06 outputs through F11. Membership/provenance/conflicts/revision choice are not adapter URL-normalization responsibilities. |

Ten further whole-source observations are in `source-shared.json`, SHA256
`d47263c381f7259e960cb86143f2f88b6b28aa75e4ff2845dfac19afb221d591`.
Reproduce with `PYTHONPATH=. python docs/rust-rewrite/a06/oracle/export_shared.py`
using the pinned offline oracle environment. All HTTP is mocked. Redirect cases
actually observe 2/2/1 requests and OK/rate-limited/error respectively. Nine source
file digests include the three inspected shared test files. The original 67 and
supplemental 38 record files are unchanged. This new temporary exporter and its
invocation must be added to F01 before A06 acceptance and removed by G08.

Remaining audit: shared capabilities, payload/entity/formatter, Jev/advisory,
research and coverage-evaluation assertions still require explicit owners and
successor names. Shared ownership assignments above do not waive those cases or
prove their successor gates pass.

### Additional inspected shared assertions

| Existing regression | Ownership and required successor evidence |
| --- | --- |
| `test_payload.py::TestSciencePayload::test_arxiv_payload` | A06: exact science/publication payload, engine provenance, versioned `publication_id`, normalized multiline abstract; authors/journal remain absent for this fixture because it contains no such nodes. The test comment claiming the adapter does not parse them is stale: current `_parse_feed` extracts author/name and arxiv:journal_ref when present. Preserve presence-sensitive extraction; do not delete populated fields or fabricate missing ones. Initial full-output captures include these payload boundaries; native replay must assert complete payloads, not only titles. |
| `test_payload.py::test_provenance_distinguishes_field_kinds` and persistence/snapshot oversize cases | F05 owns generic payload provenance/size and F13 persistence round trip; G01 exercises actual A06 payloads through both. A06 must construct the source field classifications before those layers. Adapter success is not proof of persistence fidelity. |
| `test_formatter.py::test_payload_between_inline_and_persist_bounds_omitted` | T02/T03/T06 own presentation bounds; F05/F13 own persistence. The synthetic arXiv payload is not an adapter parser requirement. G01 must retain consistent fresh/cached behavior across actual scholarly results. |
| `test_entity_projection.py::test_v2_scholarly_identifiers_normalize_without_merging_namespaces` and invalid-identifier cases | T06 owns entity projection: distinct DOI/PMID/PMCID/OpenAlex identities, explicit alias relationships, no false merges or deceptive-host IDs. A06 preserves raw payload IDs and must not pre-normalize them to make this assertion pass. G01 wires actual scholarly output into the projection. |
| `test_capabilities.py::test_representative_families_report_distinct_capabilities`, OpenAlex failure-class and filter assertions; `test_capability_catalog.py` science rows | A06 owns engine-local truthful metadata: free source classification; text results; OpenAlex rate_limited/error/timeout; supported time_range/date_from/date_to with local/upstream/upstream audited enforcement. F06 owns catalog serialization, G01 completeness. Real arXiv/Semantic Scholar must not inherit mock filter declarations. |
| `test_jev.py` specialist metadata, scoring/cache, explicit-scope bypass, addition and transport projection cases | A06 preserves engine-local routing role/card. F12 owns routing provider/cache selection, F14 search composition, T02/T03 wire/portal exposure; G01 real adapter integration. Mock arXiv score/dispatch expectations are not evidence that A06 parser is implemented. |
| `test_advisories.py::test_jev_only_on_automatic_scopes_and_available_state`, `test_bounded_sorted_and_derived_from_current_catalog` | F15 owns advisories from live catalog and presentation-independent counts; A06 provides truthful metadata. Never copy cached advisories or enable an unfinished factory merely to suppress an unavailable-source notice. |
| `test_research_coverage.py::test_empty_subquery_distinct_from_failed`, `test_mixed_empty_and_failed_is_never_clean_empty` and coverage round-trip | W01/W02 own stored coverage and worker aggregation; A06 must distinguish valid-empty OK from timeout/error. G01 verifies real scholarly response classifications through the runner. A mixed empty+timeout result must never become clean empty. |
| `test_research_retry_followup.py` scope widening with mock arXiv | W02 owns retry/follow-up policy and budgeting; A06 cannot authorize broader scope itself. G01 integration preserves engine outcomes and attempt accounting. |

These allocations come from inspected source assertions, but runnable successor
names remain an acceptance obligation. Coverage-evaluation suites still need the
O03/O07/O08 inventory cross-check; this document does not claim the complete
repository-wide traceability audit is finished.

## Atom parser observations

`source-xml.json` adds 24 actual whole-adapter captures, SHA256
`2b34a88df34ed59b5b374a2161f13f3a6405b05dcfc37629b3fca0131cc56f16`;
reproduce with the pinned environment and
`PYTHONPATH=. python docs/rust-rewrite/a06/oracle/export_xml.py`.
All HTTP is mocked. This is the third temporary exporter file and fourth invocation (the first
exporter has two modes); all three files and four invocations need F01
registration and G08 retirement. No historical observation file changed.

These observations constrain the native XML implementation:

- Invalid/empty XML, multiple roots, illegal characters and unresolved/external
  entities produce OK with zero results, not an adapter error.
- Internal document entities resolve; comments and processing instructions are
  skipped while adjacent text joins. CDATA and character references decode.
- Entry lookup is direct-child and namespace-aware, but the root name itself is
  not validated. A wrong root name with an Atom entry still produces a result.
- First matching child only; ElementTree `.text` excludes descendant content
  and tails. A leading nested element can therefore give an empty title.
- Empty metadata and absent metadata differ. Present authors/journal are parsed;
  the older payload-test comment claiming otherwise is stale. Author text trims
  and ignores blank names; other `findtext` metadata preserves its source spaces.
- Summary whitespace follows Python Unicode rules, and the 300-character snippet
  bound counts Unicode characters, not UTF-8 bytes.
- Source currently extracts an arXiv ID with `rsplit('/', 1)`, losing a legacy
  category prefix such as `hep-th/` from old-style IDs. The capture retains that
  actual behavior. Any repair must be a separately documented intentional change
  with current identifier/grouping compatibility reviewed; do not silently
  rewrite the reference output or claim this observation proves correct links.

The already-locked native `quick-xml`0.42.0 is an implementation candidate, not
proof of ElementTree-equivalent entity/namespace/mixed-content semantics. Input
text still depends on A05's complete HTTPX-compatible codec work. XML resource
bounds, cancellation and hostile entity expansion need native tests in addition
to these source captures. No external file/network entity loading is required.


## Acquisition and evidence tooling cross-check

The accepted `inventory/o03-active-tooling.json` assigns
`coverage_engine_replay.py`, `coverage_live_acquire.py` and
`coverage_pipeline_inputs.py` to O07; `coverage_resource_evidence.py` and
`coverage_stage_finalization.py` belong to O08. These assignments were checked
against the actual inventory, not inferred from adapter names in fixtures.

| Inspected assertion | Native owner and required composition |
| --- | --- |
| `test_coverage_engine_replay.py` successful four-adapter scenario and classified outcome parameterization | A06 must consume the recorded arXiv/OpenAlex requests and preserve output/status. O07 owns replay matching, exhaustion and receipt generation: five exchanges for the four adapters, no unused scenario IDs, and `quality_credit=false` for synthetic scenarios. G01 must compose real factories with this transport. |
| `test_arxiv_redirect_dispatches_are_physically_paced_within_request_deadline` | O07 owns physical dispatch pacing and receipts. A06 must expose both redirect requests through the supplied transport. Preserve three actual requests, a first redirect marked allowed, offsets 0/3000001/10000001 microseconds under the controlled clock, and per-query versus per-physical-request pacing. A successful two-request adapter capture alone cannot satisfy this assertion. |
| `test_arxiv_physical_wait_consumes_the_per_request_deadline` | O07/F08 composition must count pacing wait against the deadline. The second request times out before dispatch: one physical call total, a receipt with `request_deadline_before_dispatch`, and `dispatched=false`. The test calls the bounded transport directly; do not misdescribe it as whole-adapter coverage. |
| `test_complete_v2_snapshots_flow_through_pipeline_late_materials_and_selector_map` | O07 owns acquisition and downstream snapshot consumers; O08 owns evidence verification. Preserve all 13 complete operations, physical-count equality with observed requests, stage budget, 40/80 pool sizes, and exact stage/source/profile/hash bindings. A06 contributes actual adapter outputs without replacing these end-to-end checks with parser fixtures. |
| `test_saved_capture_accounting_accepts_only_explicit_historical_producer_pin` and stage-finalization acquisition profile fields | O08 owns schema-aware historical producer identity and acceptance/rejection, including acquisition and arXiv pacing profile bindings. A06 must not rewrite historical producer identities when replacing the live backend. Runnable native verifier and negative controls remain required under O08. |

This cross-check covers the named assertions above. Other coverage suites found
by reference search still need assertion-level disposition. O07/O08 tests and
native command mappings do not yet exist for this packet; these ownership rows
are implementation obligations, not qualification evidence or permission for
live acquisition.

### Further acquisition assertions inspected

- O07's `test_two_engine_pool_keeps_complete_canonical_results_and_paces`
  retains all 40 canonical results and exactly two physical requests, source
  engine order, per-exchange timeout and request/response hashes. The four-engine
  companion preserves 41–80 results, eight actual requests versus twelve
  reserved worst-case requests, and the controlled inter-operation wait.
  A06 must not implement presentation slicing inside its canonical output.
- `test_research_band_engine_count_mismatch_fails_before_any_dispatch` and
  `test_failures_are_retained_without_retry_or_rescue` belong to O07 admission
  and execution: no request on malformed band selection; two requests and an
  inconclusive operation/stage on the OpenAlex timeout, without rescue/retry.
- `test_complete_640_card_high_band_pools_are_retained_without_truncation`
  preserves 640 cards/sources, 80 per task and the full native order. The fresh
  frozen-pool companion preserves partial availability: 21/41-card task pools,
  inconclusive acquisition and arXiv's explicit `rate_limited`, zero-result
  sidecar row, with synthetic `quality_credit=false`. O07 owns these downstream
  consumers; A06 must preserve the originating classification and results.
- O08's resource-observation tests deliberately leave retries and pacing
  **unknown** for mock/fake-clock evidence. Even
  `test_acquisition_collector_derives_arxiv_physical_gap_from_dispatch_receipts`
  currently asserts two arXiv calls but null pacing and observed-gap fields.
  Its name must not be treated as permission to promote configured pacing into
  observed enforcement. Uniform timeout observations are reported separately.
- `test_actual_collector_resource_accounting_preserves_unknown_live_controls`
  counts actual exchanges/bytes and retains unknown retry/pacing controls. An
  OpenAlex 429 produces one physical call and an inconclusive operation even
  while pool eligibility is complete. O08 must preserve that distinction and
  reject tampered archived responses or a changed summary after verification.

These are reviewed source expectations, not completed native assertions.

## Shared SourceJSON repr expansion follow-up

`source-repr-boundary.json` records actual pinned CPython json.loads(bytes)/repr
input and full output hashes. The native regression
`adapters::source_json::tests::accepted_utf16_expansion_repr_matches_complete_source_hash`
accepts exactly4MiB UTF16LE JSON that expands to6,291,450 UTF8 bytes and checks the
complete6,291,452-byte representation hash. The prior JSON-macro scalar conversion
panicked at the unrelated public JSON byte limit. Direct typed Value construction
now invokes the existing Unicode repr formatter without another serialization/
parse. Global JSON bounds and all original A04/A06 source corpora are unchanged.
This is a shared boundary correction, not full A06 adapter acceptance.
