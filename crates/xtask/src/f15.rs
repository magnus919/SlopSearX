//! F15 source-bound offline native reranking and read-boundary evidence.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const CORPUS: &str = "33656cc13ff8f4e15016b6655ad8db5d0bea6011aa5c59e642b8ad61a66865a5";
const EXPORTER: &str = "363fc7bb8f299e4f88a3b384d4eb23af75681ce8911aa541c423007143bb529a";
const BASELINE: &str = "4dfc0f5691b6a5a1c039a2837372b1977422ec27";
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn ensure(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn pinned(root: &Path, path: &str, expected: &str) -> Result<Vec<u8>> {
    let bytes = fs::read(root.join(path)).map_err(|e| e.to_string())?;
    ensure(
        digest(&bytes) == expected,
        &format!("F15 frozen bytes changed: {path}"),
    )?;
    Ok(bytes)
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    pinned(
        root,
        "docs/rust-rewrite/f15/headers.json",
        "db18cc1167000765a86a99c15354ea65159a030f1cbfb35e1e23e71e159d9c11",
    )?;
    pinned(
        root,
        "docs/rust-rewrite/f15/capture_headers.py",
        "47357904bbbce9fd790eee6b4857ad2f32533d30fedfd7aad148ad6b333947eb",
    )?;
    let coverage_bytes = pinned(
        root,
        "docs/rust-rewrite/f15/coverage.json",
        "99446b40b5bca38ada67c8207e80899ec56cd2e693aa3be34bbf0427d2e1a0a7",
    )?;
    let coverage: Value = serde_json::from_slice(&coverage_bytes).map_err(|e| e.to_string())?;
    ensure(
        coverage["plan_id"] == "F15"
            && coverage["baseline"] == BASELINE
            && coverage["source_parameterized_case_count"] == 77
            && coverage["obligations"]
                .as_array()
                .is_some_and(|r| r.len() == 42),
        "F15 source assertion inventory incomplete",
    )?;
    let bytes = pinned(root, "docs/rust-rewrite/f15/cases.json", CORPUS)?;
    pinned(root, "docs/rust-rewrite/f15/capture.py", EXPORTER)?;
    let corpus: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    ensure(
        corpus["plan"] == "F15" && corpus["source_baseline"] == BASELINE,
        "F15 source identity changed",
    )?;
    let sources = corpus["sources"].as_array().ok_or("F15 sources missing")?;
    ensure(
        sources.len() == 13,
        "F15 historical source inventory incomplete",
    )?;
    for row in sources {
        let path = row["path"].as_str().ok_or("F15 source path missing")?;
        let blob = row["git_blob"].as_str().ok_or("F15 source blob missing")?;
        let sha = row["sha256"].as_str().ok_or("F15 source digest missing")?;
        let historical = command(
            root,
            "git",
            &["rev-parse", &format!("{BASELINE}:{path}")],
            records,
        )?;
        ensure(
            historical.trim() == blob,
            "F15 historical source identity differs",
        )?;
        let current = command(root, "git", &["hash-object", "--", path], records)?;
        ensure(current.trim() == blob, "F15 unknown current source drift")?;
        pinned(root, path, sha)?;
    }
    ensure(
        corpus["cases"].as_array().is_some_and(|c| c.len() == 150),
        "F15 frozen case inventory incomplete",
    )?;
    let listing = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "f15",
            "--",
            "--list",
        ],
        records,
    )?;
    let names: BTreeSet<_> = listing
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .collect();
    let expected: BTreeSet<_> = [
        "read_capture_preserves_full_canonical_per_caller_views_and_snapshot_semantics",
        "provider_strict_answers_stable_ties_usage_and_input_bounds_execute",
        "cached_read_uses_current_trusted_availability_without_new_execution_or_authority",
        "actual_source_provider_cases_replay_exact_wire_identity_usage_and_decisions",
        "actual_source_projection_and_advisory_cases_replay_full_objects",
        "actual_source_host_cases_preserve_whole_results_tier_crossing_and_fixed_tail",
        "real_socket_provider_bounds_gzip_redirect_free_wire_and_cancellation_release_slots",
        "expired_host_deadline_never_polls_or_dispatches_provider",
        "composed_native_provider_cache_admission_and_sensitive_no_send",
        "composed_native_cancellation_and_concurrent_shutdown_join_transport",
        "composed_native_identity_overrides_manual_identity_and_waiters_share_execution",
        "real_native_oversized_identity_header_rejects_before_held_body_and_closes_socket",
        "keyless_ranking_and_credential_config_factory_execute",
        "configured_native_stage_suppresses_specialist_promotion_for_every_outcome",
        "actual_native_service_remaining_interactive_budget_cancels_provider_socket",
    ]
    .into_iter()
    .collect();
    ensure(names == expected, "F15 executable test inventory changed")?;
    let output = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-infra", "--test", "f15"],
        records,
    )?;
    ensure(
        output
            .lines()
            .any(|l| l.starts_with("test result:") && l.contains("15 passed; 0 failed; 0 ignored")),
        "F15 tests did not all execute",
    )?;
    let json_output = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-json"],
        records,
    )?;
    for name in [
        "float::tests::direct_api_rejects_malformed_incomplete_and_bounds_without_panics",
        "float::tests::signed_zero_extreme_exponents_subnormal_and_rounding_are_preserved",
    ] {
        ensure(
            json_output
                .lines()
                .any(|l| l == format!("test {name} ... ok")),
            "F15 shared numeric test missing or ignored",
        )?;
    }

    let critical = [
        "rerank::f15_provider_tests::native_queue_wait_expires_without_dispatch_and_releases_admission",
        "search::f15_sensitive_tests::sensitive_post_rank_engine_and_provenance_are_excluded",
        "http::f15_header_tests::opt_in_identity_length_limit_preserves_encoding_and_python_integer_rules",
        "http::f15_header_tests::actual_source_streamed_header_cases_replay_early_admission",
        "rerank::strict_json::tests::strict_json_preserves_python_encodings_numbers_and_surrogates",
        "rerank::strict_json::tests::strict_json_deep_success_error_and_destruction_use_flat_storage",
        "rerank::strict_json::tests::cancelled_large_arena_is_dropped_and_joined_without_detached_work",
        "rerank::strict_json::tests::deep_parser_yields_to_current_thread_heartbeat_and_deadline",
        "rerank::strict_json::tests::near_limit_finite_float_keeps_heartbeat_and_never_accepts_after_deadline",
        "rerank::strict_json::tests::long_significand_conversion_checks_deadline_without_reparsing_scores",
    ];
    let listing = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "--",
            "--list",
        ],
        records,
    )?;
    let names: BTreeSet<_> = listing
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .collect();
    ensure(
        critical.iter().all(|n| names.contains(n)),
        "F15 critical library test removed",
    )?;
    let output = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-infra", "--lib"],
        records,
    )?;
    ensure(
        critical
            .iter()
            .all(|n| output.lines().any(|l| l == format!("test {n} ... ok"))),
        "F15 critical library test ignored or not executed",
    )?;

    // The shared parser has one implementation. Its complete former infra
    // unit suite now executes in the shared crate, including the F15 control.
    let source_output = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-source-json", "--lib"],
        records,
    )?;
    ensure(
        source_output.lines().any(|line| line == "test source_json::f15_numeric_tests::shared_float_normalizer_preserves_long_finite_and_nonfinite_source_policy ... ok"),
        "F15 relocated shared source JSON control missing or ignored",
    )?;

    for path in [
        "crates/ssx-application/src/rerank.rs",
        "crates/ssx-application/src/advisories.rs",
        "crates/ssx-infra/src/rerank.rs",
        "crates/ssx-infra/src/http.rs",
        "crates/ssx-infra/src/rerank/strict_json.rs",
        "crates/ssx-infra/src/search.rs",
        "crates/ssx-infra/src/adapters/source_json.rs",
        "crates/ssx-json/src/float.rs",
        "crates/ssx-json/src/lib.rs",
        "crates/ssx-infra/tests/f15.rs",
        "docs/rust-rewrite/f15/cases.json",
        "docs/rust-rewrite/f15/capture.py",
        "docs/rust-rewrite/f15/capture_headers.py",
        "docs/rust-rewrite/f15/headers.json",
        "docs/rust-rewrite/f15/coverage.json",
        "docs/rust-rewrite/f15/README.md",
        "docs/rust-rewrite/f15/PARITY.md",
        "crates/xtask/src/f15.rs",
        "crates/xtask/src/main.rs",
    ] {
        let blob = command(root, "git", &["hash-object", "--", path], records)?;
        records.push(json!({"f15_native_identity":{"path":path,"git_blob":blob.trim()}}));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_byte_checks_reject_corrupt_corpus_and_exporter() {
        let root = std::env::temp_dir().join(format!("ssx-f15-pin-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        for pin in [CORPUS, EXPORTER] {
            fs::write(root.join("changed"), b"corrupt").unwrap();
            assert!(pinned(&root, "changed", pin).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
