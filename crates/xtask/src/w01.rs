//! Source-bound record/accounting replay plus mandatory native disposable Valkey.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
const BASELINE: &str = "cfd7a0af61a22cfd040a7fcd41ad950f82005959";
// Reviewed identities live in native code, independently of artifact metadata.
const CORPUS_SHA256: &str = "c3e5e026e59f84a93017f434042eba929aaa6de269098f642c00ee3eecdb2159";
const EXPORTER_SHA256: &str = "373ff210c0c8512323a52be86d5dc036583f9e2a458563c78e02115068800a0d";
const IDENTITY_SHA256: &str = "c90a37d5521ab94ef9288cbe2a51dbd2a2532c751b00478383daef824ae8b046";
const IDENTITY_EXPORTER_SHA256: &str =
    "f1ebbce92de233479f3417966554dd82b6601ab36d870e85b2c2db1ad64a18ba";
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn validate_artifacts(corpus: &[u8], exporter: &[u8]) -> Result<Value> {
    if digest(corpus) != CORPUS_SHA256 || digest(exporter) != EXPORTER_SHA256 {
        return Err("W01 reviewed oracle artifact identity mismatch".into());
    }
    let value: Value = serde_json::from_slice(corpus).map_err(|e| e.to_string())?;
    if value["schema_version"] != 1
        || value["plan_id"] != "W01"
        || value["baseline"] != BASELINE
        || value["python"] != "3.14.7"
        || value["exporter_sha256"] != EXPORTER_SHA256
    {
        return Err("W01 oracle provenance mismatch".into());
    }
    Ok(value)
}
fn validate_identity(corpus: &[u8], exporter: &[u8]) -> Result<Value> {
    if digest(corpus) != IDENTITY_SHA256 || digest(exporter) != IDENTITY_EXPORTER_SHA256 {
        return Err("W01 reviewed identity artifact mismatch".into());
    }
    let value: Value = serde_json::from_slice(corpus).map_err(|e| e.to_string())?;
    if value["schema_version"] != 1
        || value["baseline"] != BASELINE
        || value["python"] != "3.14.7"
        || value["exporter_sha256"] != IDENTITY_EXPORTER_SHA256
        || value["identities"].as_array().map(Vec::len) != Some(382)
        || value["settlements"].as_array().map(Vec::len) != Some(9)
    {
        return Err("W01 identity capture provenance mismatch".into());
    }
    Ok(value)
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let corpus =
        fs::read(root.join("crates/xtask/fixtures/w01/corpus.json")).map_err(|e| e.to_string())?;
    let exporter =
        fs::read(root.join("docs/rust-rewrite/w01/oracle/export.py")).map_err(|e| e.to_string())?;
    let data = validate_artifacts(&corpus, &exporter)?;
    let identity = fs::read(root.join("crates/xtask/fixtures/w01/identity.json"))
        .map_err(|e| e.to_string())?;
    let identity_exporter = fs::read(root.join("docs/rust-rewrite/w01/oracle/identity.py"))
        .map_err(|e| e.to_string())?;
    validate_identity(&identity, &identity_exporter)?;
    for (package, required) in [
        (
            "ssx-domain",
            &["reservation_admission_rejects_nonpending_and_active_history_without_changes"][..],
        ),
        (
            "ssx-infra",
            &[
                "same_lease_active_dispatch_rejects_a_second_reservation",
                "recovered_attempt_rejects_stale_application_completion_with_current_lease",
                "reservation_crash_reclaim_and_completed_evidence",
            ][..],
        ),
    ] {
        let listed = command(
            root,
            "cargo",
            &[
                "test", "--locked", "-p", package, "--test", "w01", "--", "--list",
            ],
            records,
        )?;
        for name in required {
            if !listed.lines().any(|line| line == format!("{name}: test")) {
                return Err(format!("W01 mandatory attempt contract missing: {name}"));
            }
        }
    }
    command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-application", "--test", "w01"],
        records,
    )?;

    let sources = data["sources"].as_array().ok_or("missing W01 sources")?;
    for source in sources {
        let path = source["path"].as_str().ok_or("missing W01 source path")?;
        let object = format!("{BASELINE}:{path}");
        let git_blob = command(root, "git", &["rev-parse", &object], records)?;
        if Some(git_blob.trim()) != source["git_blob"].as_str() {
            return Err(format!("W01 source blob drift: {path}"));
        }
        let bytes = fs::read(root.join(path)).map_err(|e| e.to_string())?;
        if Some(digest(&bytes).as_str()) != source["sha256"].as_str() {
            return Err(format!(
                "W01 current source drift requires reviewed recapture: {path}"
            ));
        }
    }
    command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-domain", "--test", "w01"],
        records,
    )?;
    // F07 verifies exact accepted substrate scripts and real native fault contracts.
    crate::f07::check(root, records)?;
    if !records.iter().any(|r| {
        r["version"]
            .as_str()
            .is_some_and(|v| v.starts_with("Valkey server v=8.1.10 "))
    }) {
        return Err("W01 requires official disposable Valkey8.1.10".into());
    }
    command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "w01",
            "--",
            "--test-threads=1",
        ],
        records,
    )?;
    records.push(json!({"plan_id":"W01","status":"passed","oracle_sha256":CORPUS_SHA256,"exporter_sha256":EXPORTER_SHA256,"baseline":BASELINE,"case_count":10,"providers":0,"identity_cases":382,"settlement_cases":9,"identity_sha256":IDENTITY_SHA256,"identity_exporter_sha256":IDENTITY_EXPORTER_SHA256,"authority":"native private Valkey8.1.10 processes; source-bound record/accounting replay; no worker execution"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_evidence_cannot_redeclare_expected_answers() {
        let corpus = include_bytes!("../fixtures/w01/identity.json");
        let exporter = include_bytes!("../../../docs/rust-rewrite/w01/oracle/identity.py");
        assert!(validate_identity(corpus, exporter).is_ok());
        assert!(validate_identity(&[], exporter).is_err());
        assert!(validate_identity(corpus, &[]).is_err());
        let mut value: Value = serde_json::from_slice(corpus).unwrap();
        value["identities"][0]["expected"] = json!("lead-v1-wrong");
        value["exporter_sha256"] = json!(digest(exporter));
        assert!(validate_identity(&serde_json::to_vec(&value).unwrap(), exporter).is_err());
    }
    #[test]
    fn missing_corrupt_and_self_redeclared_oracle_evidence_fails() {
        let corpus = include_bytes!("../fixtures/w01/corpus.json");
        let exporter = include_bytes!("../../../docs/rust-rewrite/w01/oracle/export.py");
        assert!(validate_artifacts(corpus, exporter).is_ok());
        assert!(validate_artifacts(&[], exporter).is_err());
        assert!(validate_artifacts(corpus, &[]).is_err());
        let mut altered = corpus.to_vec();
        altered.push(b' ');
        assert!(validate_artifacts(&altered, exporter).is_err());
        let mut value: Value = serde_json::from_slice(corpus).unwrap();
        value["cases"] = json!([]);
        value["exporter_sha256"] = json!(digest(exporter));
        assert!(validate_artifacts(&serde_json::to_vec(&value).unwrap(), exporter).is_err());
    }
}
