//! Source-bound W11 acceptance. Mandatory execution is entirely native/offline.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
const BASELINE: &str = "1e9dd82d62456fe6f511cc5ade88bae619fac027";
const ARTIFACTS: &[(&str, &str)] = &[
    (
        "crates/xtask/fixtures/w11/corpus.json",
        "a72814e0071a806421b188e73451c91034164438b7942a3055496be4f6562726",
    ),
    (
        "docs/rust-rewrite/w11/oracle/export.py",
        "4f7e3bf96331c12c10f7faf1cfdff3cf370832d410492be59886682f60e43ac3",
    ),
    (
        "crates/ssx-infra/src/valkey/staged_scripts/claim_guard.lua",
        "aec7e11b7ad49c829ed042e41633d724018e8530491f7d0902f4b420b3fc2f4e",
    ),
    (
        "crates/ssx-infra/src/valkey/staged_scripts/save_guard.lua",
        "9701c226cc45db3f5a8eff300ca9b88186a9db4cf5ad344d57a6cb88340e807b",
    ),
    (
        "crates/xtask/fixtures/w11/source-controls.json",
        "d342c77900bba7ce6c892da4b625b07a409d13861feafe7cb50142265cca9d7c",
    ),
];
const DOMAIN: &[&str] = &[
    "source_identity_and_schema_array_normalization",
    "malformed_records_never_panic_or_invent_budget",
];
const INFRA: &[&str] = &[
    "admission_rejects_unbound_query_filters_http_origin_and_revoked_scope",
    "admission_conflict_quota_and_lost_tenant_index",
    "competing_claims_stale_tokens_and_immutable_completion",
    "interruptions_retry_replay_budget_conservation_and_index_repair",
    "fenced_save_storage_loss_and_partial_script_writes_require_reconciliation",
    "two_stage_owned_completion_and_skipped_fallback_keep_immutable_history",
    "stale_prepared_claim_cannot_reuse_attempt_after_complete_retry_cycle",
    "retry_completion_preserves_derivable_observed_total",
    "legacy_index_ids_and_stable_cursor_ordering",
    "completion_unknown_and_durable_restart_reconcile_without_refund",
    "deadline_terminalization_and_live_orphan_recovery_preserve_charge",
];
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
fn validate(bytes: &[u8], expected: &str) -> Result<()> {
    if hash(bytes) != expected {
        return Err("W11 artifact identity mismatch".into());
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    for (path, expected) in ARTIFACTS {
        validate(
            &fs::read(root.join(path)).map_err(|e| e.to_string())?,
            expected,
        )?;
    }
    let corpus: Value =
        serde_json::from_slice(&fs::read(root.join(ARTIFACTS[0].0)).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if corpus["baseline"] != BASELINE || corpus["plan_id"] != "W11" || corpus["schema_version"] != 1
    {
        return Err("W11 source provenance mismatch".into());
    }
    for source in corpus["sources"]
        .as_array()
        .ok_or("missing W11 source pins")?
    {
        let path = source["path"].as_str().ok_or("missing W11 source path")?;
        let git = command(
            root,
            "git",
            &["rev-parse", &format!("{BASELINE}:{path}")],
            records,
        )?;
        if Some(git.trim()) != source["git_blob"].as_str() {
            return Err(format!("W11 pinned source mismatch: {path}"));
        }
        validate(
            &fs::read(root.join(path)).map_err(|e| e.to_string())?,
            source["sha256"].as_str().ok_or("missing W11 source hash")?,
        )?;
    }
    // Binds the accepted exact script bytes and qualified private Valkey identity.
    crate::f07::check(root, records)?;
    for (package, cases) in [("ssx-domain", DOMAIN), ("ssx-infra", INFRA)] {
        let listed = command(
            root,
            "cargo",
            &[
                "test", "--locked", "-p", package, "--test", "w11", "--", "--list",
            ],
            records,
        )?;
        for case in cases {
            if !listed.lines().any(|line| line == format!("{case}: test")) {
                return Err(format!("W11 required regression missing: {case}"));
            }
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
                "w11",
                "--",
                "--include-ignored",
            ],
            records,
        )?;
        for case in cases {
            if !output
                .lines()
                .any(|line| line == format!("test {case} ... ok"))
            {
                return Err(format!(
                    "W11 required regression did not execute successfully: {case}"
                ));
            }
        }
        records.push(json!({"plan_id":"W11","package":package,"executed_case_ids":cases}));
    }
    records.push(json!({"plan_id":"W11","source_baseline":BASELINE,"artifact_hashes":ARTIFACTS,"providers_dispatched":0,"runtime":"native"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn w11_gate_rejects_fixture_mutation() {
        let bytes = include_bytes!("../fixtures/w11/corpus.json");
        assert!(validate(bytes, ARTIFACTS[0].1).is_ok());
        let mut mutated = bytes.to_vec();
        mutated[100] ^= 1;
        assert!(validate(&mutated, ARTIFACTS[0].1).is_err());
        assert!(validate(b"{}", ARTIFACTS[0].1).is_err());
    }
}
