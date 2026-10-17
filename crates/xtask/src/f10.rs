//! Independently anchored source replay plus real native private authority/peer tests.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const BASELINE: &str = "9ab1c7a22de933762c42469145c1f50365220de2";
const CORPUS_SHA256: &str = "29cccf4f3c4652e38e2f532323edfbe5bd84bf0ecfbb1ac5d79d1a7468050e24";
const EXPORTER_SHA256: &str = "137fa18301e8f800317cd574495b52d277c2c5106e33227f39100459c655fc31";
const QUOTA_SHA256: &str = "04c1c84c21ae7dd22ecd97a280ab53016d0a6387bfa0a38885eb17f01e6c5a82";
const COOLDOWN_SHA256: &str = "7b58717441e0553eb7594f395f0b6fc98f0cd88282fd03b414017517c4072b7b";
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn validate(corpus: &[u8], exporter: &[u8], quota: &[u8], cooldown: &[u8]) -> Result<Value> {
    for (bytes, anchor) in [
        (corpus, CORPUS_SHA256),
        (exporter, EXPORTER_SHA256),
        (quota, QUOTA_SHA256),
        (cooldown, COOLDOWN_SHA256),
    ] {
        if digest(bytes) != anchor {
            return Err("F10 reviewed artifact identity mismatch".into());
        }
    }
    let v: Value = serde_json::from_slice(corpus).map_err(|e| e.to_string())?;
    if v["schema_version"] != 1
        || v["baseline"] != BASELINE
        || v["python"] != "3.14.7"
        || v["exporter_sha256"] != EXPORTER_SHA256
    {
        return Err("F10 source provenance mismatch".into());
    }
    for (key, count) in [
        ("sources", 9),
        ("local", 3),
        ("sequences", 4),
        ("quota_law", 4),
        ("ttl_law", 4),
        ("settings", 40),
        ("freshness", 12),
        ("health", 21),
        ("views", 60),
        ("circuit", 4),
        ("retry_after", 61),
        ("bounds", 7),
        ("registry", 51),
        ("url_resolution", 6),
        ("fail_settings", 2),
        ("timestamp_limits", 11),
    ] {
        if v[key].as_array().map(Vec::len) != Some(count) {
            return Err(format!("F10 missing/incomplete {key} observations"));
        }
    }
    let script = std::str::from_utf8(cooldown).map_err(|e| e.to_string())?;
    if v["cooldown_script_sha256"] != digest(script.trim().as_bytes()) {
        return Err("F10 cooldown Lua differs from source".into());
    }
    Ok(v)
}
fn inventory(
    root: &Path,
    records: &mut Vec<Value>,
    package: &str,
    expected: &[&str],
) -> Result<()> {
    let listed = command(
        root,
        "cargo",
        &[
            "test", "--locked", "-p", package, "--test", "f10", "--", "--list",
        ],
        records,
    )?;
    let actual: BTreeSet<_> = listed
        .lines()
        .filter_map(|s| s.strip_suffix(": test"))
        .collect();
    if actual != expected.iter().copied().collect() {
        return Err(format!("F10 {package} critical test inventory differs"));
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let read = |path: &str| fs::read(root.join(path)).map_err(|e| e.to_string());
    let data = validate(
        &read("crates/xtask/fixtures/f10/corpus.json")?,
        &read("docs/rust-rewrite/f10/oracle/export.py")?,
        &read("crates/ssx-infra/src/valkey/rate/quota.lua")?,
        &read("crates/ssx-infra/src/valkey/rate/cooldown.lua")?,
    )?;
    for source in data["sources"]
        .as_array()
        .ok_or("missing F10 source inventory")?
    {
        let path = source["path"].as_str().ok_or("invalid F10 source path")?;
        if !path.starts_with("slopsearx/")
            && !path.starts_with("engines/")
            && !path.starts_with("tests/")
        {
            return Err("invalid F10 source namespace".into());
        }
        crate::date_amendment::source(
            root,
            path,
            BASELINE,
            source["git_blob"].as_str().ok_or("F10 source blob")?,
            Some(source["sha256"].as_str().ok_or("F10 source SHA")?),
            records,
        )?;
    }
    inventory(
        root,
        records,
        "ssx-application",
        &[
            "decimal_millisecond_configuration_does_not_reject_binary_roundoff",
            "invalid_input_and_unavailable_are_explicit_and_never_healthy",
            "source_url_fail_settings_and_timestamp_limits",
            "source_raw_configuration_and_documented_fractional_expiry_hardening",
            "source_local_bucket_backpressure_and_registry_contracts",
            "source_actual_service_health_and_freshness_views",
            "source_circuit_boundary_and_retry_after_codec",
        ],
    )?;
    inventory(
        root,
        records,
        "ssx-infra",
        &[
            "a06_optional_adapter_cooldown_feedback_reuses_shared_authority_and_epoch",
            "absent_url_warmup_is_noop_and_fail_setting_is_explicit",
            "actual_f08_429_cooldown_prevents_redispatch_and_health_stays_observed",
            "actual_peer_started_timeout_records_failure_with_null_latency",
            "actual_quota_law_zero_cost_fractional_rate_and_expiry",
            "explicit_fail_open_is_separate_from_authority_and_terminal_close",
            "invalid_requests_do_not_write_and_persistent_counter_is_repaired",
            "killed_native_waiter_after_commit_cannot_leave_an_immortal_bucket",
            "lost_acknowledgment_is_unknown_and_does_not_retry_increment",
            "quota_process_worker",
            "shared_cooldown_extension_recovery_and_unavailable_distinction",
            "shutdown_does_not_close_borrowed_f07_authority",
            "two_native_processes_with_divergent_local_clock_origins_share_one_quota",
        ],
    )?;
    command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-application", "--test", "f10"],
        records,
    )?;
    crate::f07::check(root, records)?;
    if !records.iter().any(|r| {
        r["version"]
            .as_str()
            .is_some_and(|s| s.starts_with("Valkey server v=8.1.10 "))
    }) {
        return Err("F10 requires official private Valkey8.1.10".into());
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
            "f10",
            "--",
            "--test-threads=1",
            "--nocapture",
        ],
        records,
    )?;
    records.push(json!({"plan_id":"F10","status":"passed","baseline":BASELINE,"oracle_sha256":CORPUS_SHA256,"exporter_sha256":EXPORTER_SHA256,"quota_script_sha256":QUOTA_SHA256,"cooldown_script_sha256":COOLDOWN_SHA256,"providers":0,"authority":"private native Valkey8.1.10; two child processes with divergent local clocks; killed postcommit waiter; F08 actual loopback429 and timeout peers","scope":"no F09 eligibility or F14 runner/production wiring"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_corrupt_forged_expected_and_script_evidence_fails() {
        let corpus = include_bytes!("../fixtures/f10/corpus.json");
        let exporter = include_bytes!("../../../docs/rust-rewrite/f10/oracle/export.py");
        let quota = include_bytes!("../../ssx-infra/src/valkey/rate/quota.lua");
        let cooldown = include_bytes!("../../ssx-infra/src/valkey/rate/cooldown.lua");
        assert!(validate(corpus, exporter, quota, cooldown).is_ok());
        assert!(validate(&[], exporter, quota, cooldown).is_err());
        assert!(validate(corpus, &[], quota, cooldown).is_err());
        assert!(validate(corpus, exporter, &[], cooldown).is_err());
        let mut v: Value = serde_json::from_slice(corpus).unwrap();
        v["health"][0]["expected"]["status"] = json!("error");
        v["exporter_sha256"] = json!(digest(exporter));
        assert!(validate(&serde_json::to_vec(&v).unwrap(), exporter, quota, cooldown).is_err());
        let mut v: Value = serde_json::from_slice(corpus).unwrap();
        v["health"] = json!([]);
        assert!(validate(&serde_json::to_vec(&v).unwrap(), exporter, quota, cooldown).is_err());
        let mut altered = quota.to_vec();
        altered.push(b' ');
        assert!(validate(corpus, exporter, &altered, cooldown).is_err());
    }
}
