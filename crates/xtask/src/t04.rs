//! T04 finite native state qualification; no Python reference execution here.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const BASELINE: &str = "1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5";
const CORPUS: &str = "4bdf404d45476fbe7c9d0ffaabb80ef5b36d30bece8475a436d2399303fadf13";
const EXPORTER: &str = "4198363d4ee5f150e747e3fed0f93e05ecfb104677f0de7a6f55097604150ecf";
const LUA: &str = "6298de4653e58da715f46ef42d96d12b874193445e1d6138264b7e8aafeca864";
fn digest(v: &[u8]) -> String {
    Sha256::digest(v)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
fn validate(raw: &[u8], exporter: &[u8], lua: &[u8]) -> Result<Value> {
    if digest(raw) != CORPUS || digest(exporter) != EXPORTER || digest(lua) != LUA {
        return Err("T04 reviewed artifact identity mismatch".into());
    };
    let v: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    if v["schema_version"] != 1
        || v["baseline"] != BASELINE
        || v["clients"].as_array().map(Vec::len) != Some(4)
        || v["settings"].as_array().map(Vec::len) != Some(9)
        || v["versions"]["mcp"] != "2.3.0"
        || v["versions"]["fastmcp"] != "4.0.11"
        || !v["python"]
            .as_str()
            .is_some_and(|s| s.starts_with("3.14.7 "))
    {
        return Err("T04 source observation provenance mismatch".into());
    };
    Ok(v)
}
const TESTS: &[&str] = &[
    "captured_source_flow_replays_binding_lifetimes_rotation_and_revocation",
    "credentials_debug_and_durable_grants_do_not_expose_tokens",
    "empty_existing_records_are_collisions_and_preflight_is_nonmutating",
    "expiry_is_valkey_authoritative_and_outage_never_falls_back",
    "independent_instances_single_use_and_exact_binding",
    "lost_write_response_is_unknown_and_never_retried",
    "malformed_asymmetric_links_never_delete_unrelated_pair",
    "malformed_schema_and_oversize_records_fail_without_writes",
    "natural_asymmetric_expiry_preserves_refresh_and_reverse_revocation",
    "narrowed_scope_bound_rejects_without_consuming_live_pair",
    "optional_empty_subject_resource_and_scope_narrowing_remain_exact",
    "revoke_rotate_race_never_revives_revoked_or_old_pair",
    "source_registration_model_dump_roundtrips_complete_nullable_metadata",
    "wrong_type_and_invalid_binding_are_nonmutating_and_authorities_isolate",
];
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let read = |p: &str| fs::read(root.join(p)).map_err(|e| e.to_string());
    let corpus = validate(
        &read("crates/xtask/fixtures/t04/source.json")?,
        &read("docs/rust-rewrite/t04/capture.py")?,
        &read("crates/ssx-infra/src/valkey/oauth/transition.lua")?,
    )?;
    let sources = corpus["sources"]
        .as_object()
        .ok_or("T04 missing source identity")?;
    let expected = BTreeSet::from([
        "slopsearx/mcp/oauth.py",
        "slopsearx/mcp/security.py",
        "tests/test_mcp_oauth.py",
        "uv.lock",
    ]);
    if sources.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return Err("T04 source inventory differs".into());
    };
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
                pin.as_str().ok_or("T04 lock row hash")?,
                accepted.trim(),
                current.trim(),
            )?;
        } else if Some(current.trim()) != pin.as_str() || current.trim() != accepted.trim() {
            return Err(format!("T04 source drift: {path}"));
        }
    }
    let list = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "t04",
            "--",
            "--list",
        ],
        records,
    )?;
    let names = list
        .lines()
        .filter_map(|s| s.strip_suffix(": test"))
        .collect::<BTreeSet<_>>();
    if names != TESTS.iter().copied().collect() {
        return Err("T04 critical test inventory differs".into());
    }
    let output = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-infra", "--test", "t04"],
        records,
    )?;
    if !output.contains("14 passed; 0 failed; 0 ignored") {
        return Err("T04 native integration tests missing or ignored".into());
    }
    let domain_list = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-domain",
            "--lib",
            "oauth::tests",
            "--",
            "--list",
        ],
        records,
    )?;
    let selected: BTreeSet<_> = domain_list
        .lines()
        .filter_map(|s| s.strip_suffix(": test"))
        .collect();
    if selected
        != BTreeSet::from(["oauth::tests::configured_modes_and_issuer_have_source_contract"])
    {
        return Err("T04 settings test inventory differs".into());
    }
    let domain_result = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-domain",
            "--lib",
            "oauth::tests",
        ],
        records,
    )?;
    if !domain_result.contains("1 passed; 0 failed; 0 ignored") {
        return Err("T04 settings tests missing or ignored".into());
    }
    let docs = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-domain", "--doc", "oauth"],
        records,
    )?;
    if !docs.contains("1 passed; 0 failed; 0 ignored") {
        return Err("T04 credential serialization negative missing".into());
    }
    records.push(json!({"kind":"t04_authority","source_baseline":BASELINE,"corpus_sha256":CORPUS,"private_valkey_tests":13,"scope":"state transitions only; no endpoint, provider or deployment claim"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modified_answers_cannot_redeclare_their_identity() {
        let raw = include_bytes!("../fixtures/t04/source.json");
        let exporter = include_bytes!("../../../docs/rust-rewrite/t04/capture.py");
        let lua = include_bytes!("../../ssx-infra/src/valkey/oauth/transition.lua");
        assert!(validate(raw, exporter, lua).is_ok());
        let mut value: Value = serde_json::from_slice(raw).unwrap();
        value["flow"]["access"]["resource"] = json!("forged");
        value["corpus_sha256"] = json!(digest(&serde_json::to_vec(&value).unwrap()));
        assert!(validate(&serde_json::to_vec(&value).unwrap(), exporter, lua).is_err());
        assert!(validate(&[], exporter, lua).is_err());
    }
}
