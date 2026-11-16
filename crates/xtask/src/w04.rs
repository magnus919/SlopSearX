//! Source-bound native saved execution gate. No oracle/runtime Python execution.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const SOURCE_SHA: &str = "a9089bc14a1827b6f59842e3050ae1db031cdfdd7d753cdf04d8414231c1f366";
const EXPORT_SHA: &str = "718d4591e0d87ccad5f29023ccd3c2e4b568c73c291bdd4f80bf23b8fc99528d";
const CASES: &[&str] = &[
    "first",
    "changes",
    "ranking",
    "outage",
    "cached",
    "partial",
    "deadline",
    "unresponsive",
    "scope",
    "truncated",
    "provenance",
    "ambiguous",
    "utf8_oversize",
    "empty_url",
    "unicode",
    "expired",
    "window",
    "provenance_order",
    "provenance_set_changed",
    "published_date_only",
    "published_date_empty",
];
const APPLICATION: &[&str] = &[
    "exact_source_observations_reports_baselines",
    "report_size_uses_ascii_spaced_bytes_and_retains_prior_baseline_without_panicking",
];
const INFRA: &[&str] = &[
    "shutdown_classified_during_post_acquisition_time_read",
    "shutdown_classified_during_stalled_renewal_io",
    "scheduled_due_batch_preserves_missed_slots_and_unsliced_observation",
    "retained_old_scope_is_revalidated_after_definition_changes",
    "publication_is_atomic_fenced_and_tenant_bound",
    "update_pause_delete_reclaim_and_malformed_key_preflight_prevent_partial_writes",
    "runner_revocation_shutdown_outer_drop_and_uncached_execution",
    "held_loopback_renews_authority_and_lease_loss_joins_only_owned_socket",
    "killed_worker_probe",
    "killed_process_recovery_preserves_slot_and_publishes_once",
    "retained_baseline_and_report_eviction_keep_independent_horizons",
];
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let raw =
        fs::read(root.join("docs/rust-rewrite/w04/source.json")).map_err(|e| e.to_string())?;
    if digest(&raw) != SOURCE_SHA
        || digest(
            &fs::read(root.join("docs/rust-rewrite/w04/capture_source.py"))
                .map_err(|e| e.to_string())?,
        ) != EXPORT_SHA
    {
        return Err("W04 frozen source corpus/export identity changed".into());
    }
    let source: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    if source["baseline"] != "1bad48bbe4e25e2f60d5b86668e92bbce922fdf0"
        || source["cases"].as_array().map(Vec::len) != Some(21)
    {
        return Err("W04 provenance/case inventory mismatch".into());
    }
    let cases: BTreeSet<_> = source["cases"]
        .as_array()
        .ok_or("W04 cases absent")?
        .iter()
        .map(|row| {
            row["name"]
                .as_str()
                .ok_or_else(|| "W04 case identity absent".to_owned())
        })
        .collect::<Result<_>>()?;
    if cases != CASES.iter().copied().collect() {
        return Err("W04 exact source case identity mismatch".into());
    }
    for (path, expected) in source["sources"]
        .as_object()
        .ok_or("W04 source bindings missing")?
    {
        let baseline = command(
            root,
            "git",
            &[
                "rev-parse",
                &format!("1bad48bbe4e25e2f60d5b86668e92bbce922fdf0:{path}"),
            ],
            records,
        )?;
        let current = command(root, "git", &["hash-object", path], records)?;
        if baseline.trim() != current.trim() {
            return Err(format!("W04 baseline/source identity mismatch: {path}"));
        }
        if Some(digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?).as_str())
            != expected.as_str()
        {
            return Err(format!(
                "W04 source drift requires reviewed reconciliation: {path}"
            ));
        }
    }
    let executable =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER").unwrap_or_else(|_| "valkey-server".into());
    if !command(root, &executable, &["--version"], records)?.starts_with("Valkey server v=8.1.10 ")
    {
        return Err("W04 requires private Valkey8.1.10".into());
    }
    for (package, expected) in [("ssx-application", APPLICATION), ("ssx-infra", INFRA)] {
        let listing = command(
            root,
            "cargo",
            &[
                "test", "--locked", "-p", package, "--test", "w04", "--", "--list",
            ],
            records,
        )?;
        let actual: BTreeSet<_> = listing
            .lines()
            .filter_map(|l| l.strip_suffix(": test"))
            .collect();
        if actual != expected.iter().copied().collect() {
            return Err(format!(
                "W04 {package} required semantic test inventory changed"
            ));
        }
        let output = command(
            root,
            "cargo",
            &[
                "test",
                "--locked",
                "-p",
                package,
                "--test",
                "w04",
                "--",
                "--test-threads=1",
            ],
            records,
        )?;
        if !output.lines().any(|l| {
            l.starts_with("test result: ok.")
                && l.contains(&format!("{} passed; 0 failed; 0 ignored", expected.len()))
        }) {
            return Err(format!(
                "W04 {package} missing/failed/ignored required contracts"
            ));
        }
    }
    records.push(json!({"plan_id":"W04","status":"passed","source_sha256":SOURCE_SHA,"provider_calls":0,"authority":"source goldens; private Valkey; held loopback sockets; killed native subprocess","limitations":"No production grant reload or outbox/wire composition; W05/W09 retain those obligations; unknown remote/write completion is not rollback"}));
    Ok(())
}
