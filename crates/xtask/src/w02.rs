//! Source-bound native research worker qualification; no Python runtime.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const BASELINE: &str = "5aa7983aebc2dddbf00727d5a7e6b1192b3dfee0";
const SOURCES: &str = "ec5e8e7a26032612d7571e7d1ead77262c1fb83bbb36b6c6c239bdae2b495ddf";
const TESTS: &[&str] = &[
    "denied_workflow_audit_survives_restored_authority_retry_without_actual_call_charge",
    "exhausted_denial_reservation_never_fabricates_attempt_or_refunds_counts",
    "source_success_at_query_ceiling_distinguishes_dossier_stop_reason",
    "malformed_workflow_budget_never_defaults_into_more_spend",
    "large_finite_persisted_deadlines_execute_without_duration_or_instant_panic",
    "runner_rejects_unrepresentable_lease_and_poll_configuration",
    "dropped_caller_releases_owned_socket_capacity_and_lease_without_harming_other_flight",
    "partial_retry_preserves_success_and_failed_snapshot_bytes",
    "supervised_pool_shutdown_joins_all_owned_searches_and_retains_charges",
    "shutdown_interrupts_renewal_io_before_store_timeout",
    "source_coverage_required_optional_excluded_absent_and_order",
    "competing_workers_settle_once_in_claimed_tenant_and_snapshot",
    "held_execution_renews_past_ttl_and_shutdown_joins_without_refund",
    "durable_cancel_interrupts_owned_capture_and_settles_truthfully",
    "retry_extension_retain_successful_evidence_and_history",
    "deadline_during_held_query_is_failed_with_conserved_attempt",
    "pre_cancelled_snapshot_has_zero_dispatch",
    "lease_loss_joins_held_loopback_socket_and_preserves_reclaimer",
    "worker_process_probe",
    "process_kill_restart_discovers_orphan_and_conserves_unknown_spend",
    "workflow_grant_matrix_denies_atomically_without_provider_calls",
    "workflow_mixed_sensitive_plan_denies_before_first_dispatch",
    "snapshot_write_cancel_can_leave_tenant_orphan_but_never_stale_job_link",
    "snapshot_write_cancellation_reports_completion_unknown",
    "concurrent_retry_holds_authority_and_preserves_empty_attempt_snapshot",
    "cancellation_affects_only_owned_isolated_attempt",
    "actual_runtime_circuit_change_preserves_authorized_explicit_capture",
    "invalid_delayed_authority_never_runs_guarded_publication",
];
const COMPLETION_TESTS: &[&str] = &[
    "research_runner::completion_tests::initial_completion_joins_actual_monitor_before_own_clear",
    "research_runner::completion_tests::terminal_early_return_joins_actual_monitor_before_own_clear",
    "research_runner::completion_tests::reclaimer_after_monitor_join_before_clear_preserves_replacement_lease_and_evidence",
];
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let raw =
        fs::read(root.join("docs/rust-rewrite/w02/sources.json")).map_err(|e| e.to_string())?;
    if digest(&raw) != SOURCES {
        return Err("W02 reviewed source bindings changed".into());
    }
    let sources: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    if sources["plan_id"] != "W02"
        || sources["baseline"] != BASELINE
        || sources["sources"].as_array().map(Vec::len) != Some(8)
    {
        return Err("W02 provenance mismatch".into());
    }
    for row in sources["sources"]
        .as_array()
        .ok_or("W02 source rows absent")?
    {
        let path = row["path"].as_str().ok_or("W02 source path absent")?;
        let baseline = command(
            root,
            "git",
            &["rev-parse", &format!("{BASELINE}:{path}")],
            records,
        )?;
        let current = command(root, "git", &["hash-object", path], records)?;
        if Some(baseline.trim()) != row["git_blob"].as_str()
            || current.trim() != baseline.trim()
            || Some(digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?).as_str())
                != row["sha256"].as_str()
        {
            return Err(format!("W02 source drift requires review: {path}"));
        }
    }
    let executable =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER").unwrap_or_else(|_| "valkey-server".into());
    let version = command(root, &executable, &["--version"], records)?;
    if !version.starts_with("Valkey server v=8.1.10 ") {
        return Err("W02 requires official private Valkey8.1.10".into());
    }
    let listing = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "w02",
            "--",
            "--list",
        ],
        records,
    )?;
    let actual: BTreeSet<_> = listing
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .collect();
    let expected: BTreeSet<_> = TESTS.iter().copied().collect();
    if actual != expected {
        return Err("W02 required test inventory changed".into());
    }
    let output = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "w02",
            "--",
            "--test-threads=1",
        ],
        records,
    )?;
    if !output.lines().any(|l| {
        l.starts_with("test result: ok.")
            && l.contains(&format!("{} passed; 0 failed; 0 ignored", TESTS.len()))
    }) {
        return Err("W02 tests missing, failed, or ignored".into());
    }
    let completion_listing = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "research_runner::completion_tests::",
            "--",
            "--list",
        ],
        records,
    )?;
    let listed: Vec<_> = completion_listing
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect();
    let actual: BTreeSet<_> = listed.iter().copied().collect();
    let expected: BTreeSet<_> = COMPLETION_TESTS.iter().copied().collect();
    if actual != expected || listed.len() != COMPLETION_TESTS.len() {
        return Err("W02 completion-retirement test inventory changed".into());
    }
    let completion_output = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "research_runner::completion_tests::",
            "--",
            "--test-threads=1",
        ],
        records,
    )?;
    let executed: Vec<_> = completion_output
        .lines()
        .filter_map(|line| {
            line.strip_prefix("test ")
                .and_then(|line| line.strip_suffix(" ... ok"))
        })
        .collect();
    if executed.len() != COMPLETION_TESTS.len()
        || executed.iter().copied().collect::<BTreeSet<_>>() != expected
        || !completion_output.lines().any(|line| {
            line.starts_with("test result: ok.") && line.contains("3 passed; 0 failed; 0 ignored")
        })
    {
        return Err(
            "W02 completion-retirement tests missing, failed, duplicated or ignored".into(),
        );
    }
    records.push(json!({"plan_id":"W02","status":"passed","baseline":BASELINE,"sources_sha256":SOURCES,"native_tests":TESTS.len()+COMPLETION_TESTS.len(),"source_replay_tests":TESTS.len(),"completion_retirement_tests":COMPLETION_TESTS.len(),"provider_calls":0,"authority":"private pinned Valkey plus killed native worker subprocess and joined loopback sockets","limitations":"No runtime grant reload; explicit-engine circuit semantics preserved; W09 wire mapping and production composition remain downstream"}));
    Ok(())
}
