# Bounded publication dates: native integration and reference map

Related #984. Reference is Python PR #874 merge
`d207d4496129d496fe8a74b040ebe1a71151b6f9`, candidate
`9ab07ac9dbfdcd6d25bc19c472ea4230519bc0ef`. This map was read from the merge
against its first parent. It distinguishes implemented native assertions from
remaining acceptance; neither source presence nor the old Python test count
establishes whole-issue parity.

Native test abbreviations below:

- **HTTP routes**: `ssx-http/tests/science_openalex.rs::native_http_real_openalex_routes_bounds_formats_and_cache_reacquisition`.
- **HTTP denials**: `ssx-http/tests/science_openalex.rs::native_http_date_denials_have_zero_real_acquisition`.
- **HTTP outcomes**: `ssx-http/tests/science_openalex.rs::native_http_real_openalex_failure_and_empty_have_distinct_status_outcomes`.
- **MCP acquisition**: `ssx-mcp/tests/science_openalex.rs::authenticated_science_openalex_dates_preserve_wire_proof_and_valkey_cache`.
- **MCP denials**: `ssx-mcp/tests/science_openalex.rs::science_date_denials_never_reach_real_adapter_or_shared_acquisition`.
- **MCP outcomes**: `ssx-mcp/tests/science_openalex.rs::science_real_openalex_empty_and_http_failure_remain_distinct`.

These six tests use the same actual registered OpenAlex/service/private Valkey
fixture. The HTTP test uses the accepted public router and real peer connection;
MCP uses authenticated SDK transport. No Python runtime, provider request,
production startup or new portal implementation participates.

## Added and changed reference bodies

| Reference body | Native coverage or remaining acceptance |
| --- | --- |
| `test_server.py::test_absolute_dates_forward_and_prove_audited_scope` | HTTP routes checks GET/form POST on `/` and `/search`, lower/upper/equal leap-day/two-sided bounds, JSON/YAML proof and real returned URL/publication-day pairs. Every route/method has cold actual-adapter acquisition; YAML then checks canonical cache readback. |
| `test_server.py::test_absolute_dates_reject_before_dispatch` | HTTP denials checks malformed/empty/invalid leap-day/reversed dates, relative conflict, implicit/category/mixed/unsupported scopes and `date_start`. Both routes and methods assert HTTP 400 and zero adapter/client/cache/upstream/state acquisition. The same table checks the exact reference spellings `fromDate`, `timeRange`, `DateFrom`, `dateFrom`, invalid upper `2024-02-30`, invalid lower `2023-02-29`, and relative `day` alongside its extra native grammar cases. |
| `test_server.py::test_absolute_dates_upstream_failure_is_not_zero_matches` | HTTP outcomes checks actual upstream HTTP 500 becomes HTTP 503/error, distinct from 200/ok empty acquisition, for JSON/YAML. Existing `t02_http::absolute_date_proof_and_forwarding_reach_actual_http_without_claiming_a06` separately retains the synthetic timeout/503 outcome. HTTP 500 is additional actual-adapter evidence, not a claim that this new fixture reproduces timeout. |
| `test_server.py::test_absolute_dates_are_discoverable_in_openapi` | Existing `t02_http::original_assertion_families_use_runtime_policy_config_and_public_discovery` checks GET/POST `/search` date schemas in its `absolute-date-openapi` case. This test was inspected, not rerun in this focused increment. |
| `test_server.py::test_repeated_date_bounds_reject` | HTTP denials checks repeated `date_from` controls, including the reference bad-first/valid-last ordering, for both methods/routes and a query-plus-form duplicate. |
| `test_server.py::test_http_dates_reach_real_openalex_and_preserve_publication_dates` | HTTP routes checks the actual adapter/parser, exact inclusive outbound filter bytes and actual publication-day projection. The unchanged A06 corpus includes adjacent days, missing/bad dates, misleading title/year, timestamp and compact-date forms. |
| `test_server.py::test_portal_keeps_absolute_bounds_in_forms_and_machine_links` | **Open T03 acceptance.** Existing pagination retains dates, but forms lose date/scope on resubmit; truthful enforcement display and machine-format links are absent. No native browser pass is claimed here. |
| `test_server.py::test_bounded_formats_without_proof_reject_before_dispatch` | HTTP denials checks CSV/RSS HTTP 400 and refusal text before actual adapter/client/cache/upstream/state acquisition, for both routes and methods. HTTP routes separately preserves ordinary unbounded CSV/RSS compatibility. |
| `test_filter_semantics.py::TestEnforcementTruthAcrossPaths::test_specialist_date_filters_are_unsupported` (changed to rejection) | MCP denials checks implicit bounded science scope returns `invalid_input` on `engines` before acquisition. |
| `test_filter_semantics.py::TestAuditedOpenAlexDates::test_science_dates_real_adapter_mixed_scope_and_cache` | MCP acquisition checks real OpenAlex, exact date proof/enforcement/outcomes through cache and changed-bound identity; MCP denials rejects mixed scope atomically. |
| `test_filter_semantics.py::TestAuditedOpenAlexDates::test_absolute_and_relative_bounds_intersect` (changed to rejection) | HTTP denials checks the shared absolute-plus-relative policy refusal. Science MCP does not declare `time_range`; MCP denials checks the existing FastMCP-compatible schema refusal instead. |
| `test_filter_semantics.py::test_absolute_date_cache_requires_proof_and_separates_bounds` | HTTP routes and MCP acquisition check real Valkey cache hits, distinct date keys, missing-proof reacquisition, stronger mismatching-proof reacquisition, typed proof/day readback and ordinary unbounded use. MCP additionally checks presentation slicing leaves canonical results intact. |
| `test_filter_semantics.py::test_absolute_date_scope_requires_audited_adapter_not_just_name` | Both denial targets alter only the trusted catalog's audit declarations and require zero acquisition; a registered name alone is insufficient. |
| `test_mcp_enforcement.py::TestAllTools::test_all_five_tools_return_same_report_shape` (science portion changed) | MCP denials checks the changed implicit bounded science rejection. Other tools/report families remain owned by the original T06/F09 inventories. |
| `test_mcp_enforcement.py::TestSpecialistFilterParams::test_science_date_filters_unsupported` (changed to rejection) | MCP denials checks implicit bounded science rejection on `engines`, not a fabricated unsupported-success report. |
| `test_mcp_tools.py::TestSpecialistGrants::test_science_date_range_rejects_unsupported_scope` (renamed from warning case) | MCP denials checks unsupported/implicit scopes and separately preserves science-grant denial; no specialist or sensitive-engine permission is widened. |

PR #874 did **not** modify `test_formatter.py`. HTTP routes/outcomes independently
check the native JSON/YAML formatter obligations above; they are not an invented
reference formatter-test delta.

## Limits and remaining handoff

This remains a finite integration increment. The HTTP denial table includes all
exact PR #874 denial parameter cells and extends them across both routes/methods;
shared F09 calendar tests are additional component evidence. The positive matrix
also adds an upper bound of `9999-12-31`: this is an explicitly native calendar
boundary check, not an invented PR #874 test delta or recaptured A06 source row.
Its outbound filter is a separate literal expectation and its unchanged upstream
response retains the same three valid URL/day pairs as A06's `both_span` case.
The positive matrix requires twenty cold acquisitions, twenty cached YAML views,
two proof-repair acquisitions and one ordinary unbounded acquisition (23 total).

Complete T03 browser acceptance and final integrated/CI qualification before
closing #984. The exact tested inactive-engine MCP error-order difference remains
documented in the T06 README. OpenAPI coverage is retained under its existing T02
owner; this increment adds neither an OpenAPI implementation nor a new portal.

## Local execution identity

Code revision `24db49cb5ba2c00cd15e7acb36242ae297983975`, based on upstream
`e9c5911a468b892fe2adef9027374cb867a2535d`, passed the two exact three-name test
inventories and all six native tests with pinned private Valkey 8.1.10. Formatting,
strict Clippy for `ssx-http`, `ssx-mcp` and `xtask` across all targets, and the
existing W07 registration control also passed. The additive T06 gate registers
both exact inventories; the required native CI workflow already runs workspace
tests. No hosted CI or full T06 run is claimed by these local results.

An initial MCP compile failed because cached `ssx-domain` metadata contained a
prior `SearchPage` representation while sealed current source declares `u64`.
No source fix was made. The original error was retained; narrowly owned stale
artifacts and then exact library fingerprint markers were retired under all
three exclusive leases. Subsequent compiler JSON records both selected domain profiles with
`fresh=false` and their current manifest/source paths. The selected HTTP/MCP
test images are separately bound to current manifests/source paths and artifact
hashes before all six tests passed again. Registry/download caches were retained.
The local receipt root is
`/Volumes/tank01/magnus/rust-builds/slopsearx/984-http-20261027/`; windows 01, 02
and 03 preserve original commands, raw output, source seal, actual waits,
artifact admission, target measurements and process/lease/TMP retirement.


Required full pre-commit returned 1 solely for the existing unrelated Ruff
format baseline in `tests/test_coverage_saved_method_materials.py`; its exact
original bytes were restored after checking AST equivalence. All eight scoped
changed paths passed. The required AST-only `graphify update .` completed with
source/index unchanged and a 120,557,568-byte local graph. A first graph-update
monitor stopped on a file-replacement `du` ENOENT; the retained retry recognizes
only that scoped condition with bounded attempts. No API extraction or force
option was used. These tooling observations do not extend native/runtime scope.


The subsequent HTTP-only table extension adds the exact reference grammar cells
and maximum-calendar case described above. Its new runtime evidence is pending;
the `24db49cb` results remain observations of the preceding table. Core fixture,
MCP bodies, W07 registrar/pins and dependency files are unchanged by the extension.
