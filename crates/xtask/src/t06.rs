//! Source-bound actual T06 search/discovery qualification. Never executes Python.
use crate::{Result, a13::a06_source_successor::native_search, command};
#[path = "../../../docs/rust-rewrite/t06/source-amendments/lock_amendment.rs"]
pub(crate) mod lock_amendment;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const BASELINE: &str = "6c44976444340cc8f343fef61e2e8d723b189c19";
const BINDINGS: &str = "11660374ae3fd544590cb014b532eda4d6f3a95ef8f0bc062b308bd7911fc771";
const NATIVE_BINDINGS: &str = "16edd8d4c89f463a12e9c1bf4863086ed0268a42736c9a31df5a181b421284e2";
const STORAGE_AMENDMENT: &str = "cc5ec0678ef0a54bc21068ad1ea2a230e0283356f3138cc4424b624b0acc6a23";
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn validate(bytes: &[u8]) -> Result<Value> {
    if digest(bytes) != BINDINGS {
        return Err("T06 reviewed source bindings differ".into());
    }
    let v: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if v["baseline"] != BASELINE
        || v["manual_oracle_python"] != "3.14.7"
        || v["source_sdk"]["fastmcp"] != "4.0.11"
        || v["source_sdk"]["mcp"] != "2.3.0"
        || v["unicode"] != "16.0.0"
        || v["sources"].as_array().map(Vec::len) != Some(19)
        || v["artifacts"].as_array().map(Vec::len) != Some(24)
    {
        return Err("T06 incomplete source provenance".into());
    }
    Ok(v)
}
fn validate_native(bytes: &[u8]) -> Result<Value> {
    if digest(bytes) != NATIVE_BINDINGS {
        return Err("T06 reviewed native fixture bindings differ".into());
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if value["schema_version"] != 1 || value["files"].as_array().map(Vec::len) != Some(10) {
        return Err("T06 incomplete native fixture provenance".into());
    }
    Ok(value)
}
fn validate_storage(bytes: &[u8], archive: &[u8]) -> Result<Value> {
    if digest(bytes) != STORAGE_AMENDMENT {
        return Err("T06 reviewed storage amendment differs".into());
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if value["schema_version"] != 1
        || value["file"] != "crates/ssx-mcp/tests/t06_support/server.rs"
        || value["before_sha256"].as_str() != Some(digest(archive).as_str())
    {
        return Err("T06 incomplete storage amendment provenance".into());
    }
    Ok(value)
}
fn validate_native_file(root: &Path, row: &Value, bytes: &[u8], storage: &Value) -> Result<()> {
    if row["file"] == native_search::SOURCE {
        return native_search::verify(
            root,
            row["file"].as_str().ok_or("T06 native fixture path")?,
            row["sha256"].as_str().ok_or("T06 native fixture hash")?,
            bytes,
        );
    }
    let expected = if row["file"] == storage["file"] {
        if row["sha256"] != storage["before_sha256"] {
            return Err("T06 storage amendment preimage differs".into());
        }
        &storage["after_sha256"]
    } else {
        &row["sha256"]
    };
    if Some(digest(bytes).as_str()) != expected.as_str() {
        return Err(format!(
            "T06 reviewed native fixture drift: {}",
            row["file"]
        ));
    }
    Ok(())
}
const NATIVE_FIXTURE_TESTS: &[&str] = &[
    "tests::receipt_write_latency_is_outside_the_ready_fake_engine_future",
    "wall_guard::tests::operation_errors_preserve_error_and_join_watchdog",
    "wall_guard::tests::real_wall_timeout_cancels_and_drains_operation",
    "wall_guard::tests::stuck_cancel_negative_is_failed_cleanup_with_finite_return",
    "wall_guard::tests::watchdog_drop_wakes_and_joins_actual_worker",
];
const LIB_TESTS: &[&str] = &[
    "search_surface::validation::tests::actual_source_argument_matrix_preserves_validation_text_and_coercions",
    "search_surface::entities::tests::frozen_source_entity_grammar_conserves_indices_facts_edges_and_unicode16",
    "search_surface::warning_tests::specialist_previews_preserve_scope_warning_order_and_multiplicity",
];
const T06_DISCOVERY: &[&str] =
    &["live_passive_health_categories_finite_diagnostics_and_secret_redaction"];
const T06_SERVICE: &[&str] = &[
    "actual_legacy_rpc_cancellation_releases_owned_work_and_message_capacity_before_disconnect",
    "actual_stateless_http_disconnect_drains_inflight_optional_request",
    "actual_stdio_eof_during_optional_prepare_and_preview_drains_owned_process",
    "actual_stdio_rpc_cancellation_drains_prepare_and_preview_before_long_optional_timeout",
    "captured_snapshot_is_service_bound_full_include_independent_and_not_reexecuted",
    "pre_cancelled_prepare_and_preview_never_contact_provider_or_dispatch",
    "preview_rejects_sensitive_and_disabled_before_optional_provider_and_never_dispatches",
    "preview_timeout_drop_busy_and_shutdown_drain_optional_http_ownership",
    "runtime_scope_warning_survives_native_mcp_and_snapshot_without_cache_admission",
];
const T06_SOURCE_BINDINGS: &[&str] = &[
    "original_profiles_strategies_tool_surface_and_observations_match_reviewed_source_bindings",
    "lock_amendment::tests::exact_security_lock_successor_retains_historical_profile_and_shared_runtime_disclosure",
    "lock_amendment::tests::changed_entry_sdk_and_unrelated_lock_mutants_cannot_use_the_amendment",
    "lock_amendment::tests::inverse_scope_rejects_missing_duplicate_extra_and_tampered_package_spans",
    "lock_amendment::tests::source_path_row_hash_manifest_git_mode_and_missing_material_controls_fail_closed",
];
const T06_TRANSPORT: &[&str] = &[
    "wall_guard::tests::operation_errors_preserve_error_and_join_watchdog",
    "wall_guard::tests::real_wall_timeout_cancels_and_drains_operation",
    "wall_guard::tests::stuck_cancel_negative_is_failed_cleanup_with_finite_return",
    "wall_guard::tests::watchdog_drop_wakes_and_joins_actual_worker",
    "actual_entity_payloads_survive_search_cache_snapshot_and_member_reads",
    "comparison::comparator_preserves_literal_adapter_fields_and_requires_issued_cursor_identity",
    "comparison::comparator_rejects_dynamic_protocol_type_and_shape_corruption",
    "full_frozen_subset_and_argument_calls_over_authenticated_http",
    "full_frozen_subset_and_argument_calls_over_authenticated_legacy",
    "full_frozen_subset_and_argument_calls_over_native_stdio_process",
    "native_policy_order_sensitive_authorization_precedes_runtime_availability_atomically",
    "oauth_native_snapshots_and_legacy_sessions_follow_stable_client_ownership",
    "outer_scalar_array_prompt_params_are_rejected_before_dispatch_with_explicit_framework_difference",
    "source_specialist_success_and_failure_envelopes_use_actual_pipeline",
];
const T06_WIRE: &[&str] = &[
    "actual_http_native_large_output_and_one_over_profile_fail_closed",
    "actual_legacy_native_large_output_releases_aggregate_budget_on_disconnect",
    "actual_legacy_pump_discards_late_malformed_sdk_generation_before_and_after_external_id_reuse",
    "actual_stdio_native_large_output_with_empty_process_environment",
    "actual_stdio_pump_discards_late_malformed_sdk_generation_before_and_after_external_id_reuse",
    "cancelled_native_generations_retain_capacity_until_drain_and_ignore_late_output",
    "large_deep_native_output_uses_raw_envelope_without_default_document_parser",
    "marker_is_scrubbed_from_every_result_path_and_only_trusted_objects_restore",
    "native_input_codec_is_opt_in_shape_preserving_and_owns_only_existing_pending_ids",
    "native_prompt_input_profile_preserves_other_methods_nooptin_and_original_identity",
    "trusted_admission_extracts_only_raw_name_not_arbitrary_argument_numbers",
    "typed_output_budget_preserves_default_and_rejects_one_byte_over",
    "unknown_result_and_unmarked_metadata_remain_literal",
];
const T06_STDIO_OWNER: &[&str] = &[
    "opaque::tests::owned_stdio_outer_abort_drops_held_sdk_initialization",
    "opaque::tests::owned_stdio_eof_drops_held_sdk_initialization",
    "opaque::tests::owned_stdio_explicit_cancel_drops_held_sdk_initialization",
    "opaque::tests::owned_stdio_normal_eof_and_running_service_cancel_join",
    "opaque::tests::owned_stdio_initialized_outer_abort_releases_idle_sdk_owner",
    "opaque::tests::owned_stdio_pre_cancel_does_not_dispatch_initialize",
];
const T06_OWNER_CLOCK: &[&str] = &[
    "search::deadline_drain_tests::predeadline_completed_outputs_survive_delayed_owner_drain",
    "search::deadline_drain_tests::deadline_abort_drains_pending_without_internal_failure",
    "search::deadline_drain_tests::earlier_binding_timeout_completed_after_service_deadline_is_truncated",
    "search::deadline_drain_tests::cooldown_completed_after_service_deadline_is_truncated",
    "search::deadline_drain_tests::completed_task_panic_is_not_hidden_by_owner_abort",
    "search::deadline_drain_tests::owner_manual_clock_preserves_completion_before_original_one_ms_deadline",
    "search::deadline_drain_tests::owner_manual_clock_rejects_completion_at_original_one_ms_deadline",
    "search::deadline_drain_tests::owner_manual_clock_rejects_completion_after_original_one_ms_deadline",
    "search::deadline_drain_tests::renewed_context_mutant_does_not_match_original_absolute_deadline",
    "search::deadline_drain_tests::manual_owner_clock_does_not_pause_unrelated_real_tokio_timeouts",
];
fn run_suite(
    root: &Path,
    target: &[&str],
    expected: &[&str],
    records: &mut Vec<Value>,
) -> Result<()> {
    run_suite_for(root, "ssx-mcp", target, expected, records)
}
fn run_suite_for(
    root: &Path,
    package: &str,
    target: &[&str],
    expected: &[&str],
    records: &mut Vec<Value>,
) -> Result<()> {
    let mut args = vec!["test", "--locked", "-p", package];
    args.extend_from_slice(target);
    let mut listing = args.clone();
    listing.extend_from_slice(&["--", "--list"]);
    let list = command(root, "cargo", &listing, records)?;
    if list
        .lines()
        .filter_map(|s| s.strip_suffix(": test"))
        .collect::<BTreeSet<_>>()
        != expected.iter().copied().collect()
    {
        return Err(format!(
            "T06 critical test inventory differs: {}",
            target.join(" ")
        ));
    }
    let output = command(root, "cargo", &args, records)?;
    if !output.contains(&format!(
        "test result: ok. {} passed; 0 failed; 0 ignored;",
        expected.len()
    )) {
        return Err(format!(
            "T06 missing/ignored native tests: {}",
            target.join(" ")
        ));
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let raw = fs::read(root.join("docs/rust-rewrite/t06/source_bindings_v2.json"))
        .map_err(|e| e.to_string())?;
    let v = validate(&raw)?;
    let native = validate_native(
        &fs::read(root.join("docs/rust-rewrite/t06/native_fixture_bindings.json"))
            .map_err(|e| e.to_string())?,
    )?;
    let storage_raw = fs::read(
        root.join("docs/rust-rewrite/t06/storage-amendment/current-storage-amendment-v1.json"),
    )
    .map_err(|e| e.to_string())?;
    let storage_archive =
        fs::read(root.join("docs/rust-rewrite/t06/storage-amendment/server.rs.7f8d.frozen"))
            .map_err(|e| e.to_string())?;
    let storage = validate_storage(&storage_raw, &storage_archive)?;
    for row in native["files"].as_array().unwrap() {
        let path = row["file"].as_str().ok_or("T06 native fixture path")?;
        let bytes = fs::read(root.join(path)).map_err(|e| format!("T06 {path}: {e}"))?;
        if path == "crates/ssx-mcp/src/opaque.rs" {
            crate::a13::a06_source_successor::t07_opaque_material(root, row, &bytes)?;
        } else {
            validate_native_file(root, row, &bytes, &storage)?;
        }
    }

    for key in ["sources", "artifacts"] {
        for row in v[key].as_array().unwrap() {
            let path = row["file"].as_str().ok_or("T06 binding path")?;
            let bytes = fs::read(root.join(path)).map_err(|e| format!("T06 {path}: {e}"))?;
            if key == "sources" && path == crate::t06::lock_amendment::SOURCE_PATH {
                crate::t06::lock_amendment::verify(
                    root,
                    path,
                    row["sha256"].as_str().ok_or("T06 lock row hash")?,
                )?;
            } else if Some(digest(&bytes).as_str()) != row["sha256"].as_str() {
                return Err(format!("T06 reviewed source/capture drift: {path}"));
            }
            if key == "sources" {
                let current = command(root, "git", &["hash-object", "--", path], records)?;
                let accepted = command(
                    root,
                    "git",
                    &["rev-parse", &format!("{BASELINE}:{path}")],
                    records,
                )?;
                if path == crate::t06::lock_amendment::SOURCE_PATH {
                    crate::t06::lock_amendment::verify_git_pins(
                        path,
                        accepted.trim(),
                        accepted.trim(),
                        current.trim(),
                    )?;
                } else if current.trim() != accepted.trim() {
                    return Err(format!("T06 original baseline drift: {path}"));
                }
            }
        }
    }
    let binary =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER").unwrap_or_else(|_| "valkey-server".into());
    if !command(root, &binary, &["--version"], records)?.starts_with("Valkey server v=8.1.10 ") {
        return Err("T06 official disposable Valkey8.1.10 required".into());
    }
    run_suite(
        root,
        &["--bin", "search-qualification"],
        NATIVE_FIXTURE_TESTS,
        records,
    )?;
    run_suite(root, &["--lib", "search_surface::"], LIB_TESTS, records)?;
    run_suite(root, &["--test", "t06_discovery"], T06_DISCOVERY, records)?;
    run_suite(root, &["--test", "t06_service"], T06_SERVICE, records)?;
    run_suite(
        root,
        &["--test", "t06_source_bindings"],
        T06_SOURCE_BINDINGS,
        records,
    )?;
    run_suite(root, &["--test", "t06_transport"], T06_TRANSPORT, records)?;
    run_suite(root, &["--test", "t06_wire"], T06_WIRE, records)?;
    run_suite_for(
        root,
        "ssx-infra",
        &["--lib", "search::deadline_drain_tests::"],
        T06_OWNER_CLOCK,
        records,
    )?;
    run_suite(
        root,
        &["--lib", "opaque::tests::owned_stdio_"],
        T06_STDIO_OWNER,
        records,
    )?;
    records.push(json!({"plan_id":"T06","status":"passed","source_bindings_sha256":BINDINGS,"source_baseline":BASELINE,"source_files":19,"frozen_artifacts":24,"native_fixture_files":10,"native_fixture_tests":5,"owner_clock_tests":10,"stdio_owner_tests":6,"native_fixture_bindings_sha256":NATIVE_BINDINGS,"tools":11,"resource_registrations":4,"prompts":5,"provider_calls":0,"scope":"actual native handler/service/snapshots/passive discovery through authenticated HTTP, stdio and legacy; bounded cancellation and lossless input/output; exactly two accepted outer-params diagnostic differences; production startup T02/W09 separate"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn altered_t06_bindings_cannot_self_certify() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        native_search::controls(&root).unwrap();
        let raw = include_bytes!("../../../docs/rust-rewrite/t06/source_bindings_v2.json");
        validate(raw).unwrap();
        let native = include_bytes!("../../../docs/rust-rewrite/t06/native_fixture_bindings.json");
        validate_native(native).unwrap();
        let storage_raw = include_bytes!(
            "../../../docs/rust-rewrite/t06/storage-amendment/current-storage-amendment-v1.json"
        );
        let archive = include_bytes!(
            "../../../docs/rust-rewrite/t06/storage-amendment/server.rs.7f8d.frozen"
        );
        let storage = validate_storage(storage_raw, archive).unwrap();
        assert!(validate_storage(b"{}", archive).is_err());
        assert!(validate_storage(storage_raw, b"changed archive").is_err());
        let approved = validate_native(native).unwrap();
        let server = approved["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["file"] == storage["file"])
            .unwrap();
        let current = include_bytes!("../../ssx-mcp/tests/t06_support/server.rs");
        validate_native_file(&root, server, current, &storage).unwrap();
        assert!(validate_native_file(&root, server, b"changed current source", &storage).is_err());
        assert!(validate_native_file(&root, server, archive, &storage).is_err());
        let mut changed_preimage = server.clone();
        changed_preimage["sha256"] = json!("0".repeat(64));
        assert!(validate_native_file(&root, &changed_preimage, current, &storage).is_err());
        let mut altered: Value = serde_json::from_slice(native).unwrap();
        altered["files"][0]["sha256"] = json!("0".repeat(64));
        assert!(validate_native(&serde_json::to_vec(&altered).unwrap()).is_err());

        let mut v: Value = serde_json::from_slice(raw).unwrap();
        v["artifacts"][0]["sha256"] = json!("0".repeat(64));
        assert!(validate(&serde_json::to_vec(&v).unwrap()).is_err());
        v["sources"] = json!([]);
        assert!(validate(&serde_json::to_vec(&v).unwrap()).is_err());
        assert!(validate(b"{}").is_err());
    }
}
