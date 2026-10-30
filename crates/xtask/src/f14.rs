//! F14 source-bound executable shared-service qualification; no Python runtime.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
const BASELINE: &str = "730c37a5f48b9830088594582cefb4c2f10a35e8";
const CORPUS: &str = "f3adbefd92e5bc057c1df421a93be3fe3097012af9f56fa4790b1f741b8d7310";
const EXPORTER: &str = "8b41723cfe47b1488fe24cd09948911105f521c8a78809c1b62dd913ef3ea3cd";
const TESTS: &[&str] = &[
    "lossless_high_pages_keep_distinct_valkey_views_and_canonical_snapshots",
    "actual_source_service_replay",
    "actual_source_suggestion_wire_encoding_primary_and_fallback",
    "internal_task_panic_stays_synthetic_without_upstream_health",
    "live_circuit_rechecked_after_awaited_cooldown_before_dispatch",
    "shutdown_cancels_and_joins_preflight_callers_before_returning",
    "typed_unavailable_preserves_policy_precedence_without_side_effects",
    "optional_routing_timeout_falls_back_and_unavailable_implementation_is_typed",
    "actual_amended_source_date_admission_and_proof",
    "real_valkey_amended_date_proof_cache_admission",
    "actual_source_suggestion_parser_and_key_replay",
    "singleflight_views_fresh_isolation_and_final_waiter_join",
    "simultaneous_views_ids_and_bounded_admission",
    "actual_loopback_multirequest_deadline_queue_and_shutdown",
    "actual_registered_npm_adapter_executes_through_service",
    "real_valkey_cache_views_partial_deadline_and_snapshot_composition",
    "real_valkey_optional_failure_and_oversized_evidence_stay_complete",
    "real_valkey_client_authority_precedes_cache_and_denied_dispatch",
    "native_suggestions_loopback_fallback_cache_malformed_and_cancellation",
    "local_calendar_subprocess_probe",
    "actual_source_local_midnight_and_dst_calendar",
    "actual_jev_prepared_scope_reuses_advice_and_source_promotion",
    "actual_configured_slowest_timeout_survives_ten_second_floor",
    "real_cooldown_and_observed_circuit_skip_dispatch",
    "downstream_rerank_stage_cache_admission_and_sensitive_guard",
    "dedicated_client_quota_namespace_exact_keys_and_failures",
    "actual_source_environment_settings_and_explicit_finite_boundaries",
    "local_calendar_is_frozen_for_filter_dispatch_and_cache_identity",
    "native_suggestion_primary_and_failed_fallback_cache_empty",
    "actual_last_waiter_and_shutdown_close_owned_loopback_sockets",
    "coalesced_deadline_warns_each_waiter_once",
    "logical_generation_and_scope_order_bound_cache_identity",
    "context_deadlines_preserve_service_binding_and_adapter_health_origins",
    "context_default_dispatches_legacy_once_and_queue_consumes_service_bound",
    "context_full_service_deadline_retains_synthetic_timeout_health",
];
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
fn validate(bytes: &[u8], expected: &str) -> Result<Value> {
    if digest(bytes) != expected {
        return Err("F14 independently reviewed source-capture identity mismatch".into());
    }
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let raw =
        fs::read(root.join("crates/xtask/fixtures/f14/source.json")).map_err(|e| e.to_string())?;
    let corpus = validate(&raw, CORPUS)?;
    if digest(&fs::read(root.join("docs/rust-rewrite/f14/capture.py")).map_err(|e| e.to_string())?)
        != EXPORTER
    {
        return Err("F14 reviewed exporter identity drift".into());
    }
    if corpus["schema_version"] != 1
        || corpus["plan_id"] != "F14"
        || corpus["source_commit"] != BASELINE
        || corpus["python"] != "3.14.7"
    {
        return Err("F14 provenance profile mismatch".into());
    }
    let cases = corpus["cases"]
        .as_array()
        .ok_or("F14 source cases missing")?;
    let mut ids = BTreeSet::new();
    let mut kinds = BTreeMap::<String, usize>::new();
    for case in cases {
        if !ids.insert(case["id"].as_str().ok_or("F14 case ID")?) {
            return Err("F14 duplicate source case".into());
        }
        *kinds
            .entry(case["kind"].as_str().ok_or("F14 case kind")?.into())
            .or_default() += 1;
    }
    let expected: BTreeMap<_, _> = [
        ("service", 10),
        ("jev-service", 1),
        ("calendar", 8),
        ("settings", 14),
        ("suggest", 8),
        ("suggest-key", 5),
        ("availability", 2),
        ("suggest-wire", 8),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect();
    if kinds != expected || cases.len() != 56 {
        return Err("F14 finite source inventory drift".into());
    }
    let sources = corpus["source_files"]
        .as_object()
        .ok_or("F14 source pins missing")?;
    if sources.len() != 24 {
        return Err("F14 reached source/regression inventory drift".into());
    }
    for (path, pin) in sources {
        crate::date_amendment::source(
            root,
            path,
            BASELINE,
            pin["git_blob"].as_str().ok_or("F14 source blob")?,
            Some(pin["sha256"].as_str().ok_or("F14 source SHA")?),
            records,
        )?;
    }
    crate::date_amendment::corpus(root, records)?;
    let valkey =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER").unwrap_or_else(|_| "valkey-server".into());
    let version = command(root, &valkey, &["--version"], records)?;
    if !version.starts_with("Valkey server v=8.1.10 ") {
        return Err("F14 requires official private Valkey8.1.10".into());
    }
    let listed = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "f14",
            "--",
            "--list",
        ],
        records,
    )?;
    let actual: BTreeSet<_> = listed
        .lines()
        .filter_map(|v| v.strip_suffix(": test"))
        .collect();
    if actual != TESTS.iter().copied().collect() || actual.len() != TESTS.len() {
        return Err("F14 exact native execution test inventory drift".into());
    }
    let output = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-infra", "--test", "f14"],
        records,
    )?;
    if !output.contains(&format!(
        "test result: ok. {} passed; 0 failed; 0 ignored;",
        TESTS.len()
    )) {
        return Err("F14 incomplete/ignored native execution evidence".into());
    }
    let output = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "search::tests::zero_remaining_budget_never_polls_engine",
            "--",
            "--exact",
        ],
        records,
    )?;
    if !output.contains("test result: ok. 1 passed; 0 failed; 0 ignored;") {
        return Err("F14 elapsed execution-budget guard missing".into());
    }
    records.push(json!({"plan_id":"F14","status":"passed","baseline":BASELINE,"source_cases":56,"source_files":24,"native_integration_tests":TESTS.len(),"provider_calls":0,"scope":"shared native execution, source routing/results/views/settings, private F08 loopback and real Valkey; synthetic rerank-port admission is F14 seam evidence only, not F15 provider acceptance; no HTTP/MCP/browser cutover"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mutable_capture_rehash_cannot_replace_reviewed_anchor() {
        let mut value: Value =
            serde_json::from_slice(include_bytes!("../fixtures/f14/source.json")).unwrap();
        value["cases"][0]["output"]["partial"] = json!(true);
        value["manifest_sha256"] = json!(digest(&serde_json::to_vec(&value["cases"]).unwrap()));
        assert!(validate(&serde_json::to_vec(&value).unwrap(), CORPUS).is_err());
    }
}
