//! F13 source-bound cache and immutable snapshot qualification. No Python runtime gate.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
const BASELINE: &str = "35d97ccf8c9b2d8246373b2024cc8d7a457705a3";
// Acceptance anchors live outside mutable fixture metadata. Parent independent
// source recapture is required before publication; self-consistency is not provenance.
const ANCHORS: &[(&str, &str)] = &[
    (
        "crates/xtask/fixtures/f13/source.json",
        "b4867d73f3206c74c440379b08333ff55d2f8f8e861d2fd7dacc594fae10bd24",
    ),
    (
        "crates/xtask/fixtures/f13/reverse.json",
        "10b2f0d3d3a0df0f8c969bbe58036dd554109d4102fb7f3d2a0fc7421d1a1c15",
    ),
    (
        "crates/xtask/fixtures/f13/native-record.json",
        "fbf5c2b453e4c4e4fdd54b5c372be642d036e5cf2db788de0df963e24e0ba203",
    ),
    (
        "docs/rust-rewrite/f13/capture.py",
        "55e9c1d3aece63b5c42df94cca801b6dfdc3412ec74871185b99d99889bb2a31",
    ),
    (
        "crates/xtask/fixtures/f13/native-v1-identity-before.txt",
        "ce5c64d8153cba18829e2d731cee71f94bb24574ef05d041fa99b3e244f47582",
    ),
];
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
fn validate(bytes: &[u8], expected: &str) -> Result<Value> {
    if digest(bytes) != expected {
        return Err("F13 capture acceptance identity mismatch".into());
    }
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}
fn run(
    root: &Path,
    records: &mut Vec<Value>,
    package: &str,
    target: &str,
    expected: &[&str],
) -> Result<()> {
    let listed = command(
        root,
        "cargo",
        &[
            "test", "--locked", "-p", package, "--test", target, "--", "--list",
        ],
        records,
    )?;
    let actual: BTreeSet<_> = listed
        .lines()
        .filter_map(|v| v.strip_suffix(": test"))
        .collect();
    if actual != expected.iter().copied().collect() || actual.len() != expected.len() {
        return Err(format!("F13 exact {package}/{target} test inventory drift"));
    }
    let output = command(
        root,
        "cargo",
        &["test", "--locked", "-p", package, "--test", target],
        records,
    )?;
    if !output.contains(&format!(
        "test result: ok. {} passed; 0 failed; 0 ignored;",
        expected.len()
    )) {
        return Err(format!("F13 {package}/{target} incomplete/ignored tests"));
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let mut loaded = Vec::new();
    for (path, expected) in ANCHORS {
        let raw = fs::read(root.join(path)).map_err(|e| e.to_string())?;
        if digest(&raw) != *expected {
            return Err(format!("F13 reviewed artifact drift {path}"));
        }
        loaded.push(raw);
    }
    let corpus = validate(&loaded[0], ANCHORS[0].1)?;
    if corpus["schema_version"] != 1
        || corpus["plan_id"] != "F13"
        || corpus["source_commit"] != BASELINE
    {
        return Err("F13 provenance profile mismatch".into());
    }
    let cases = corpus["cases"].as_array().ok_or("F13 cases absent")?;
    let mut ids = BTreeSet::new();
    let mut kinds = BTreeMap::<&str, usize>::new();
    for case in cases {
        let id = case["id"].as_str().ok_or("F13 case ID")?;
        if !ids.insert(id) {
            return Err("F13 duplicate case".into());
        }
        *kinds
            .entry(case["kind"].as_str().ok_or("F13 case kind")?)
            .or_default() += 1;
    }
    let required: BTreeMap<_, _> = [
        ("key", 29),
        ("answer-key", 6),
        ("ttl", 15),
        ("view", 6),
        ("snapshot-create", 1),
        ("snapshot-read", 13),
        ("snapshot-integrity", 5),
        ("source-quirk", 2),
        ("routing", 10),
        ("composed-key", 4),
        ("cache-read", 10),
        ("cache-write", 2),
        ("negative-view", 1),
        ("negative-metadata", 5),
        ("answer-nonobject", 5),
        ("storage-bounds", 3),
        ("snapshot-scope", 5),
        ("snapshot-lineage", 8),
    ]
    .into_iter()
    .collect();
    if kinds != required || cases.len() != 130 {
        return Err("F13 finite source inventory mismatch".into());
    }
    let sources = corpus["source_files"].as_object().ok_or("F13 sources")?;
    if sources.len() != 9 {
        return Err("F13 source pin count".into());
    }
    for (path, pin) in sources {
        crate::date_amendment::source(
            root,
            path,
            BASELINE,
            pin["git_blob"].as_str().ok_or("F13 blob")?,
            Some(pin["sha256"].as_str().ok_or("F13 SHA")?),
            records,
        )?;
    }
    let reverse: Value = serde_json::from_slice(&loaded[1]).map_err(|e| e.to_string())?;
    if reverse["cases"] != corpus["cases"]
        || reverse["source_files"] != corpus["source_files"]
        || reverse["reverse_read"]["native_record_sha256"] != digest(&loaded[2])
    {
        return Err("F13 reverse compatibility provenance mismatch".into());
    }
    let binary =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER").unwrap_or_else(|_| "valkey-server".into());
    let version = command(root, &binary, &["--version"], records)?;
    if !version.starts_with("Valkey server v=8.1.10 ") {
        return Err("F13 official private Valkey8.1.10 required".into());
    }
    run(
        root,
        records,
        "ssx-application",
        "f13",
        &[
            "source_base_keys_answers_ttls_and_views_replay_exactly",
            "canonical_nested_views_and_snapshot_pages_cannot_contaminate_evidence",
            "routing_identity_uses_every_source_contributor_and_conditional_health",
            "composed_cache_identity_preserves_source_timeout_and_suggestion_distinctions",
            "native_v1_identity_bytes_survive_tuple_encoding_change",
            "identity_hashing_does_not_apply_optional_storage_bounds_or_panic",
        ],
    )?;
    run(
        root,
        records,
        "ssx-infra",
        "f13",
        &[
            "nx_two_independent_clients_preserve_first_value_and_retention",
            "immutable_cache_views_admission_answer_and_negative_namespaces_are_separate",
            "source_snapshot_legacy_records_integrity_and_error_states_do_not_mutate",
            "logical_equality_real_expiry_retention_and_tenant_isolation",
            "cache_and_snapshot_reconnect_distinguish_missing_malformed_outage_and_closed",
            "lost_or_malformed_create_reply_never_publishes_or_issues_another_id",
            "known_id_collision_retries_only_before_success_without_replacing_evidence",
            "canonical_cache_source_reads_and_source_snapshot_writer_are_replayed",
            "python_reverse_reader_observation_matches_exact_native_created_record",
            "malformed_negative_metadata_is_unknown_and_nonobject_answers_do_not_overwrite",
            "fresh_unsliced_evidence_uses_byte_bounds_without_arbitrary_result_cap",
            "injected_f05_payload_bounds_apply_only_at_persistence_boundary",
            "cache_admission_uses_news_and_partial_retention_without_changing_evidence",
            "result_lookup_keeps_snapshot_identity_and_never_recovers_by_requery",
            "durable_byte_and_depth_refusal_is_nonmutating",
            "snapshot_local_legacy_iterables_preserve_valid_scope_without_rewriting",
        ],
    )?;
    run(
        root,
        records,
        "ssx-application",
        "lossless_cache_key",
        &["source_base_key_preserves_literal_high_page_integers"],
    )?;
    let output = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-application",
            "--doc",
            "cache::CanonicalResponse",
        ],
        records,
    )?;
    if !output.contains("test result: ok. 1 passed; 0 failed; 0 ignored;") {
        return Err("F13 immutable evidence compile-fail guard missing".into());
    }
    records.push(json!({"plan_id":"F13","status":"passed","source_cases":130,"provider_calls":0,"baseline":BASELINE,"scope":"canonical cache and immutable snapshot ports; injected admission/relative dates, no F14 deadline or F15 rerank execution claim"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expected_output_and_mutable_metadata_rehash_cannot_replace_acceptance_anchor() {
        let raw = include_bytes!("../fixtures/f13/source.json");
        let mut value: Value = serde_json::from_slice(raw).unwrap();
        value["cases"][0]["expected"] = json!("forged");
        value["manifest_sha256"] = json!(digest(&serde_json::to_vec(&value["cases"]).unwrap()));
        assert!(validate(&serde_json::to_vec(&value).unwrap(), ANCHORS[0].1).is_err());
        assert!(validate(br#"{"cases":[]}"#, ANCHORS[0].1).is_err());
    }
}
