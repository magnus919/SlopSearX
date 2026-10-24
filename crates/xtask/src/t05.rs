//! Source-bound T05 wire replay and private-Valkey network authorization.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const BASELINE: &str = "403dfd3720e969fa829f426136bf0f05b0360c56";
const CORPUS: &str = "b623b0ada55441176841f2875128070d049696fcbe15602c5f83af70fd8cc33c";
const EXPORTER: &str = "7dbde64b36b2f90bf8cefccd01f2674b74c6ab713963914c6f1b0eebefafcba7";
fn digest(v: &[u8]) -> String {
    Sha256::digest(v)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn validate(raw: &[u8], exporter: &[u8]) -> Result<Value> {
    if digest(raw) != CORPUS || digest(exporter) != EXPORTER {
        return Err("T05 reviewed source artifacts differ".into());
    }
    let v: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    if v["schema_version"] != 1
        || v["plan_id"] != "T05"
        || v["baseline"] != BASELINE
        || v["python"] != "3.14.7"
        || v["python_hash_seed"] != "0"
        || v["mcp"] != "2.3.0"
        || v["fastmcp"] != "4.0.11"
        || v["cases"].as_array().map(Vec::len) != Some(80)
    {
        return Err("T05 incomplete source provenance".into());
    }
    let names = v["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["case"].as_str().ok_or("T05 case name"))
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    if names.len() != 80 {
        return Err("T05 duplicate source cases".into());
    }
    Ok(v)
}
const TESTS: &[&str] = &[
    "corrupt_code_only_fields_are_rejected_without_consumption",
    "stored_grant_defaults_and_corruption_have_structured_nonconsuming_responses",
    "request_shape_errors_preserve_existing_code_before_valid_redemption",
    "pinned_source_wire_corpus_replays_with_only_named_security_corrections",
    "authoritative_clock_units_and_failure_never_fall_back_to_process_time",
    "bounded_forms_reject_ambiguity_oversize_and_truncated_multipart_before_authority",
    "independent_rmcp_clients_authenticate_and_revocation_blocks_existing_sessions",
    "static_bearer_cli_precedence_and_every_registered_network_method_are_guarded",
    "oauth_guards_legacy_sse_and_message_suffix_after_token_revocation",
    "concurrent_refresh_has_one_winner_and_expired_secret_never_issues_tokens",
    "committed_registration_unknown_outcome_returns_no_credentials_and_never_retries",
    "endpoint_configuration_accepts_only_https_or_actual_loopback_and_exact_resource",
    "corrupt_persisted_client_authority_denies_without_consuming_code_and_recovers_exactly",
];
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let raw =
        fs::read(root.join("crates/xtask/fixtures/t05/source.json")).map_err(|e| e.to_string())?;
    let exporter =
        fs::read(root.join("docs/rust-rewrite/t05/capture.py")).map_err(|e| e.to_string())?;
    let v = validate(&raw, &exporter)?;
    let sources = v["sources"].as_object().ok_or("T05 source inventory")?;
    let expected = BTreeSet::from([
        "slopsearx/mcp/oauth.py",
        "slopsearx/mcp/security.py",
        "slopsearx/mcp/server.py",
        "tests/test_mcp_oauth.py",
        "tests/test_mcp_security.py",
        "uv.lock",
    ]);
    if sources.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return Err("T05 source inventory differs".into());
    }
    for (path, pin) in sources {
        let current = command(root, "git", &["hash-object", "--", path], records)?;
        let accepted = command(
            root,
            "git",
            &["rev-parse", &format!("{BASELINE}:{path}")],
            records,
        )?;
        if path == crate::t06::lock_amendment::SOURCE_PATH {
            crate::t06::lock_amendment::verify(
                root,
                path,
                crate::t06::lock_amendment::HISTORICAL_SHA256,
            )?;
            crate::t06::lock_amendment::verify_git_pins(
                path,
                pin.as_str().ok_or("T05 lock row hash")?,
                accepted.trim(),
                current.trim(),
            )?;
        } else if Some(current.trim()) != pin.as_str() || current.trim() != accepted.trim() {
            return Err(format!("T05 accepted source drift:{path}"));
        }
    }
    let binary =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER").unwrap_or_else(|_| "valkey-server".into());
    if !command(root, &binary, &["--version"], records)?.starts_with("Valkey server v=8.1.10 ") {
        return Err("T05 official disposable Valkey8.1.10 required".into());
    }
    let list = command(
        root,
        "cargo",
        &[
            "test", "--locked", "-p", "ssx-mcp", "--test", "t05", "--", "--list",
        ],
        records,
    )?;
    if list
        .lines()
        .filter_map(|s| s.strip_suffix(": test"))
        .collect::<BTreeSet<_>>()
        != TESTS.iter().copied().collect()
    {
        return Err("T05 critical test inventory differs".into());
    }
    let output = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-mcp", "--test", "t05"],
        records,
    )?;
    if !output.contains(&format!(
        "test result: ok. {} passed; 0 failed; 0 ignored;",
        TESTS.len()
    )) {
        return Err("T05 missing/ignored native tests".into());
    }
    records.push(json!({"plan_id":"T05","status":"passed","oracle_sha256":CORPUS,"exporter_sha256":EXPORTER,"source_cases":80,"native_tests":TESTS.len(),"provider_calls":0,"scope":"actual loopback OAuth endpoints and T01 network authentication using private Valkey; no production composition or browser identity policy claim"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn altered_expectations_cannot_self_certify_through_metadata() {
        let raw = include_bytes!("../fixtures/t05/source.json");
        let exporter = include_bytes!("../../../docs/rust-rewrite/t05/capture.py");
        validate(raw, exporter).unwrap();
        let mut v: Value = serde_json::from_slice(raw).unwrap();
        v["cases"][0]["response"]["status"] = json!(403);
        v["sha256"] = json!(digest(&serde_json::to_vec(&v).unwrap()));
        assert!(validate(&serde_json::to_vec(&v).unwrap(), exporter).is_err());
        assert!(validate(raw, b"forged exporter").is_err());
    }
}
