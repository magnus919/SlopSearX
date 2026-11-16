//! T09 source-bound offline replay and actual TLS/private-Valkey acceptance.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const BASELINE: &str = "1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5";
const CORPUS: &str = "cd4d4794b34272b2004d17b50a44f8c3ae4f15866e37c6a4b9369b4e312dce30";
const EXPORTER: &str = "fbebf885dcda9fef308b9102480593fa3e3f7c5e02d6af03083f0b7b78b76395";
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn validate(corpus: &[u8], exporter: &[u8]) -> Result<Value> {
    if digest(corpus) != CORPUS || digest(exporter) != EXPORTER {
        return Err("T09 independently reviewed capture identity mismatch".into());
    }
    let v: Value = serde_json::from_slice(corpus).map_err(|e| e.to_string())?;
    if v["schema_version"] != 1
        || v["baseline"] != BASELINE
        || v["python"] != "3.14.7"
        || v["provider_calls"] != 0
        || v["exporter_sha256"] != EXPORTER
    {
        return Err("T09 capture provenance mismatch".into());
    }
    for (key, count) in [
        ("sources", 11),
        ("claims", 22),
        ("return_paths", 13),
        ("sessions", 7),
        ("signatures", 2),
        ("directories", 6),
        ("settings", 6),
        ("mutation_digests", 5),
        ("previous_key_drain", 15),
    ] {
        if v[key].as_array().map(Vec::len) != Some(count) {
            return Err(format!("T09 incomplete {key}"));
        }
    }
    Ok(v)
}
fn run(
    root: &Path,
    records: &mut Vec<Value>,
    package: &str,
    selection: &[&str],
    expected: &[&str],
) -> Result<()> {
    let mut args = vec!["test", "--locked", "-p", package];
    args.extend_from_slice(selection);
    let mut list = args.clone();
    list.extend_from_slice(&["--", "--list"]);
    let output = command(root, "cargo", &list, records)?;
    let actual: BTreeSet<_> = output
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect();
    let required: BTreeSet<_> = expected.iter().copied().collect();
    if actual != required || actual.len() != expected.len() {
        return Err(format!(
            "T09 missing/unexpected {package} test inventory: {actual:?}"
        ));
    }
    let output = command(root, "cargo", &args, records)?;
    let result = format!(
        "test result: ok. {} passed; 0 failed; 0 ignored;",
        expected.len()
    );
    if !output.contains(&result) {
        return Err(format!("T09 empty/failed/ignored {package} tests"));
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let bytes =
        fs::read(root.join("crates/xtask/fixtures/t09/source.json")).map_err(|e| e.to_string())?;
    let exporter =
        fs::read(root.join("docs/rust-rewrite/t09/capture.py")).map_err(|e| e.to_string())?;
    let corpus = validate(&bytes, &exporter)?;
    let mut paths = BTreeSet::new();
    for source in corpus["sources"].as_array().ok_or("T09 sources")? {
        let path = source["path"].as_str().ok_or("T09 source path")?;
        if !paths.insert(path) {
            return Err("T09 duplicate source".into());
        }
        let actual = fs::read(root.join(path)).map_err(|e| e.to_string())?;
        if source["sha256"] != digest(&actual) {
            return Err(format!("T09 source drift {path}"));
        }
        let pinned = command(
            root,
            "git",
            &["show", &format!("{BASELINE}:{path}")],
            records,
        )?;
        if pinned.as_bytes() != actual {
            return Err(format!("T09 accepted-source drift {path}"));
        }
    }
    let binary =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER").unwrap_or_else(|_| "valkey-server".into());
    let version = command(root, &binary, &["--version"], records)?;
    if !version.starts_with("Valkey server v=8.1.10 ") {
        return Err("T09 official private Valkey8.1.10 required".into());
    }
    run(
        root,
        records,
        "ssx-http",
        &["--test", "t09"],
        &[
            "actual_oidc_tls_cookie_flow_and_public_search_equivalence",
            "mutations_require_origin_csrf_and_done_replays_do_not_repeat_action",
            "two_tenants_rotate_cookie_and_foreign_objects_are_uniformly_unavailable",
            "signed_invalid_claims_and_replayed_callback_never_create_session",
            "revoked_membership_and_valkey_failure_deny_protected_but_not_public",
            "two_independent_authorities_refresh_rotate_logout_without_session_revival",
            "refresh_between_rotation_read_and_cas_conflicts_without_issuing_handle",
            "decoded_nul_semantic_components_deny_before_claim_or_workflow",
            "stale_exact_record_refresh_and_rotation_never_restore_deleted_state",
            "revision_revocation_is_durable_and_operator_grants_only_intersect",
            "previous_key_drain_is_verification_only_and_expiry_cannot_extend_absolute_lifetime",
            "malformed_wrong_type_oversized_and_duplicate_session_records_deny_without_write",
            "competing_admission_and_stale_finish_fence_actions_and_unknown_failures",
            "callback_state_and_duplicate_inputs_reject_before_token_exchange_and_tampering",
            "host_origin_form_and_rate_boundaries_fail_closed",
            "exact_source_session_transition_projection_replay",
            "stalled_tls_peer_does_not_serialize_other_browser_handshakes",
            "composition_only_accepts_staged_search_kind_and_semantic_replay_ignores_csrf_rotation",
            "semantic_mutation_digest_replays_explicit_defaults_and_rotated_csrf",
            "dropping_listener_closes_owned_pending_tls_peers_promptly",
            "unicode_idempotency_counts_source_characters_not_utf8_bytes",
            "csrf_header_is_supported_but_conflicting_values_deny_and_audit_is_pseudonymous",
            "request_ids_correlate_real_audit_events_without_cross_request_leaks",
            "audit_stream_is_bounded_and_wrong_type_audit_failure_does_not_authorize_read",
            "authority_namespaces_isolate_sessions_quotas_and_closed_store_is_terminal",
            "audit_failure_during_reauthentication_never_issues_unlogged_session_cookie",
            "committed_write_with_lost_reply_is_unknown_and_never_retried",
            "directory_port_cannot_substitute_another_principal_or_verified_subject",
        ],
    )?;
    run(
        root,
        records,
        "ssx-infra",
        &["--lib", "browser_identity::"],
        &[
            "browser_identity::oidc::tests::authentic_claims_reject_finite_type_issuer_audience_nonce_and_time_boundaries",
            "browser_identity::oidc::tests::forged_ambiguous_unsupported_or_oversized_jwt_and_jwk_are_denied",
            "browser_identity::oidc::tests::bounded_json_rejects_nested_escaped_duplicates_depth_and_oversize",
            "browser_identity::oidc::source_replay::exact_source_claim_signature_settings_and_return_corpus_with_named_hardening",
            "browser_identity::sessions::tests::previous_key_deadline_is_exclusive_and_new_keys_use_only_current",
        ],
    )?;
    run(
        root,
        records,
        "ssx-config",
        &["--lib", "browser_identity::"],
        &[
            "browser_identity::tests::operator_settings_disabled_secrets_origin_proxy_and_previous_key_bounds",
            "browser_identity::tests::previous_key_drain_matches_source_integer_grammar",
            "browser_identity::tests::directory_retains_defaults_and_rejects_duplicate_unstable_or_coercible_authority",
            "browser_identity::tests::exact_source_directory_corpus_with_named_unknown_grant_rejection",
        ],
    )?;
    run(
        root,
        records,
        "ssx-http",
        &["--lib", "browser_identity::"],
        &[
            "browser_identity::source_replay::semantic_digest_matches_actual_pinned_source_expression",
        ],
    )?;
    let output = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-domain",
            "--doc",
            "browser_identity::BrowserSecret",
        ],
        records,
    )?;
    if !output.contains("test result: ok. 1 passed; 0 failed; 0 ignored;") {
        return Err("T09 secret compile-fail guard missing/ignored".into());
    }
    records.push(json!({"plan_id":"T09","status":"passed","baseline":BASELINE,"oracle_sha256":CORPUS,"exporter_sha256":EXPORTER,"provider_calls":0,"authority":"real private Valkey8.1.10; independent stores; actual TLS loopback IdP/browser peers; lost committed reply","scope":"browser identity/session/CSRF/tenant/rate/audit/admission boundary; injected W10 operations and public-search stand-ins, no production cutover, machineOAuth or full workflow UI claim"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reviewed_capture_anchor_rejects_mutated_expectation_even_with_forged_metadata() {
        let raw = include_bytes!("../fixtures/t09/source.json");
        let exporter = include_bytes!("../../../docs/rust-rewrite/t09/capture.py");
        validate(raw, exporter).unwrap();
        let mut v: Value = serde_json::from_slice(raw).unwrap();
        v["claims"][0]["source"]["accepted"] = json!(false);
        v["sha256"] = json!(digest(&serde_json::to_vec(&v).unwrap()));
        assert!(validate(&serde_json::to_vec(&v).unwrap(), exporter).is_err());
        v["claims"] = json!([]);
        assert!(validate(&serde_json::to_vec(&v).unwrap(), exporter).is_err());
    }
}
