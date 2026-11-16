//! Native saved definitions/claims acceptance; developer Python never executes.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, io::Read, path::Path};
const BASELINE: &str = "66b29c3f8437af45a4f0d28f7c54a0f0727f7fa5";
const CORPUS_SHA256: &str = "e73df07686f402a080cb7d57666eb084e4c2db9ea9f9a1204b90f77bdb831ffb";
const EXPORTER_SHA256: &str = "6c319ddb761296ec08aca8f786820f85fb80e8fcb4038b4c2e635177273dad09";
const COVERAGE_SHA256: &str = "db13d76f5ac125213bb4b2329631b5ca95c829e6dbc367d3e3eae342ac80ec69";
// Accepted PR874 changes unrelated date-bound search functions, not saved helpers.
// Preserve historical corpus/source pins and validate the narrow current mapping.
const CURRENT_MCP_PIN: &str = "d06ecd475b9b523addf79781d2762eddd92f4911";
const INTEGRATION_SHA256: &str = "c500069182b9975539910e29683531759e0df375e049e8daea5b8664aa10aad3";
const SOURCE_PINS: &[(&str, &str)] = &[
    (
        "slopsearx/saved_models.py",
        "f9f3e893e06a317f604357ee05030dfe277d7a27",
    ),
    (
        "slopsearx/saved_store.py",
        "5999e14a71539529fb410fe27715c2a71fc3ebd7",
    ),
    (
        "slopsearx/saved_runner.py",
        "b313cc7983afa1629ced33e379672c6cd7fdc1ee",
    ),
    (
        "slopsearx/mcp/tools.py",
        "061593ce45d3e2440e818703af6b50d6e6d99cf1",
    ),
    (
        "slopsearx/capabilities.py",
        "5ef6f90750352fdadbc877561d3520c1fe774fdb",
    ),
    (
        "tests/test_research_retry_followup.py",
        "e5700d40e73d18debbae41108e778efcc2512fca",
    ),
    (
        "tests/test_saved_models.py",
        "a8e47d6b8cca8aac7b7b5b44478e8e3a147298f7",
    ),
    (
        "tests/test_saved_valkey.py",
        "6f1d0690110763e4436d09aaa331c2356dbcb2ed",
    ),
    (
        "tests/test_saved_searches.py",
        "8d870d43bbfdf45e9026f549b3e2cdee5827fea2",
    ),
    (
        "tests/test_saved_transport.py",
        "d6fe845787bcd34a4b82073cfe411abcf94b57ae",
    ),
    (
        "tests/test_workflow_recent_indexes.py",
        "0cd352d9c80cd52ca6aa41d929654dea714b4221",
    ),
];
const NATIVE_SOURCES: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "crates/ssx-domain/Cargo.toml",
    "crates/ssx-domain/src/lib.rs",
    "crates/ssx-domain/src/saved.rs",
    "crates/ssx-domain/src/canonical.rs",
    "crates/ssx-domain/src/storage.rs",
    "crates/ssx-domain/src/identity.rs",
    "crates/ssx-domain/src/records.rs",
    "crates/ssx-application/Cargo.toml",
    "crates/ssx-application/src/lib.rs",
    "crates/ssx-application/src/saved.rs",
    "crates/ssx-application/src/policy.rs",
    "crates/ssx-application/src/scope.rs",
    "crates/ssx-infra/Cargo.toml",
    "crates/ssx-infra/src/valkey/mod.rs",
    "crates/ssx-infra/src/valkey/keys.rs",
    "crates/ssx-infra/src/valkey/scripts.rs",
    "crates/ssx-infra/src/valkey/scripts/saved_claim.lua",
    "crates/ssx-infra/src/valkey/saved.rs",
    "crates/ssx-infra/src/valkey/saved_scripts/hints.lua",
    "crates/ssx-infra/src/valkey/saved_scripts/prune_hint.lua",
    "crates/ssx-infra/src/valkey/saved_scripts/prune_tenants.lua",
    "crates/ssx-infra/src/valkey/saved_scripts/repair_tenant.lua",
    "crates/ssx-infra/src/valkey/saved_scripts/transition.lua",
    "crates/ssx-infra/tests/w00.rs",
    "crates/xtask/src/main.rs",
    "crates/xtask/src/w00.rs",
    "docs/rust-rewrite/w00/source.json",
    "docs/rust-rewrite/w00/integration-source.json",
    "docs/rust-rewrite/w00/capture_source.py",
    "docs/rust-rewrite/w00/coverage.json",
    "docs/rust-rewrite/w00/README.md",
    "docs/rust-rewrite/w00/PARITY.md",
    ".github/workflows/rust.yml",
    "docs/rust-rewrite/ci/gates.json",
    "docs/rust-rewrite/w00/evidence/README.md",
    "docs/rust-rewrite/w00/evidence/source-before-claim-rehydration.json",
    "docs/rust-rewrite/w00/evidence/domain-boundary-red.txt",
    "docs/rust-rewrite/w00/evidence/fingerprint-boundary-red.txt",
    "docs/rust-rewrite/w00/evidence/review-correction-red.txt",
    "docs/rust-rewrite/w00/evidence/final-review-two-red.txt",
];
const INFRA_TESTS: &[&str] = &[
    "independent_max_revision_delete_and_repair",
    "independent_nonobject_baseline_refuses_before_claim",
    "max_revision_full_lifecycle_repair_quota_and_true_overflow",
    "repair_exact_proof_replacement_or_new_boundary_conflicts_without_writes",
    "baseline_object_shape_matches_load_before_any_lease",
    "maximum_revision_repair_proof_keeps_exact_byte_budgets",
    "probe_claim_hydrates_live_baseline",
    "probe_large_revision_increment",
    "probe_configured_query_limit",
    "probe_query_limit_does_not_revalidate_untouched_query",
    "claim_baseline_snapshot_variants_races_and_fencing",
    "claim_read_failure_and_unknown_commit_preserve_authority",
    "revision_extremes_and_competing_updates_are_exact",
    "supplied_query_bounds_and_unchanged_query_keep_engine_authority",
    "atomic_revision_claim_pause_delete_and_tombstone",
    "quota_repair_tenant_binding_and_full_preflight",
    "server_time_expiry_future_preparation_and_stale_owner_fail_closed",
    "restart_reconcile_due_recent_and_tenant_continuation",
    "repair_byte_boundaries_are_atomic_and_preserve_existing_records",
    "revoked_engine_control_and_reduced_untouched_limits_match_source",
    "concurrent_quota_and_claim_have_one_owner_and_no_lost_definition",
    "repair_preserves_longest_expiry_and_rejects_overflow_before_writes",
    "claimed_cjson_bytes_and_run_ids_match_accepted_source_script",
    "unknown_create_completion_is_not_retried_and_reconciles_authority",
    "authoritative_tenant_enumeration_continues_and_escapes_literal_prefixes",
    "exact_colon_collision_is_rejected_and_tenants_admit_independently",
    "recent_index_is_bounded_and_fills_around_stale_records",
    "authoritative_repair_key_budget_refuses_partial_publication",
    "due_cleanup_is_bounded_and_logical_expiry_preserves_retention_record",
    "deletion_with_missing_index_forces_authoritative_quota_repair",
];
const DOMAIN_TESTS: &[&str] = &[
    "saved::boundary_review_tests::finite_input_arithmetic_overflow_does_not_manufacture_slot_zero",
    "saved::boundary_review_tests::oversized_window_is_fallible_without_macro_panic",
    "saved::boundary_review_tests::oversized_identity_is_calculated_without_persistence_macro_panic",
    "saved::tests::source_payload_window_schedule_and_rehydration",
    "saved::tests::storage_identity_is_unambiguous_without_narrowing_tenants",
    "saved::claim_fixture_tests::committed_claim_identity_replays_source_precision_and_stale_clock_hazard",
];
const APPLICATION_TESTS: &[&str] = &[
    "saved::fingerprint_boundary_tests::large_policy_metadata_identity_does_not_use_persistence_macro_bounds",
    "saved::tests::configured_auth_fingerprint_is_source_exact_and_health_independent",
    "saved::tests::strict_inputs_and_sensitive_scope_reject_before_preparation",
];
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn read(root: &Path, path: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(root.join(path))
        .map_err(|e| e.to_string())?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("W00 fixture exceeds bounded input".into());
    }
    Ok(bytes)
}
fn artifacts(corpus: &[u8], exporter: &[u8], coverage: &[u8]) -> Result<(Value, Value)> {
    if digest(corpus) != CORPUS_SHA256
        || digest(exporter) != EXPORTER_SHA256
        || digest(coverage) != COVERAGE_SHA256
    {
        return Err("W00 reviewed source-oracle/coverage identity mismatch".into());
    }
    let data: Value = serde_json::from_slice(corpus).map_err(|e| e.to_string())?;
    let mapping: Value = serde_json::from_slice(coverage).map_err(|e| e.to_string())?;
    if data["schema_version"] != 1
        || data["plan_id"] != "W00"
        || data["baseline"] != BASELINE
        || data["python"] != "3.14.7"
        || data["valkey_version"] != "8.1.10"
        || data["exporter_sha256"] != EXPORTER_SHA256
        || mapping["schema_version"] != 1
        || mapping["plan_id"] != "W00"
        || mapping["baseline"] != BASELINE
    {
        return Err("W00 oracle provenance mismatch".into());
    }
    for (key, count) in [
        ("observations", 9),
        ("policy_observations", 3),
        ("strict_interval_observations", 10),
        ("control_observations", 2),
        ("claim_observations", 6),
        ("review_observations", 13),
        ("final_review_observations", 9),
    ] {
        if data[key].as_array().map(Vec::len) != Some(count) {
            return Err(format!("W00 source observation inventory changed: {key}"));
        }
    }
    if mapping["source_test_mappings"].as_array().map(Vec::len) != Some(37) {
        return Err("W00 source test mapping inventory incomplete".into());
    }
    let expected: Vec<_> = SOURCE_PINS
        .iter()
        .map(|(path, blob)| json!({"path":path,"git_blob":blob}))
        .collect();
    if mapping["source_observations"] != json!(expected)
        || mapping["native_identity_sources"] != json!(NATIVE_SOURCES)
    {
        return Err("W00 finite source identity inventory changed".into());
    }
    Ok((data, mapping))
}
fn integration_artifact(historical: &Value, captured: &[u8], current_mcp: &[u8]) -> Result<()> {
    if digest(captured) != INTEGRATION_SHA256 {
        return Err("W00 reviewed integration recapture changed".into());
    }
    let captured: Value = serde_json::from_slice(captured).map_err(|e| e.to_string())?;
    let mut expected = historical.clone();
    // Exactly this named provenance field may differ; every observation and
    // all other metadata must remain byte-semantically identical.
    expected["sources"]["slopsearx/mcp/tools.py"] = json!(digest(current_mcp));
    if captured != expected {
        return Err("W00 integration changed historical saved observations or provenance".into());
    }
    Ok(())
}
fn inventory(output: &str, expected: &[&str]) -> Result<()> {
    let actual: BTreeSet<_> = output
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect();
    if actual != expected.iter().copied().collect() {
        return Err("W00 named native test inventory changed".into());
    }
    Ok(())
}
fn summary(output: &str, count: usize) -> Result<()> {
    let lines: Vec<_> = output
        .lines()
        .filter(|line| line.starts_with("test result:"))
        .collect();
    if lines.len() != 1
        || !lines[0].starts_with(&format!(
            "test result: ok. {count} passed; 0 failed; 0 ignored;"
        ))
    {
        return Err("W00 mandatory tests missing, ignored or failed".into());
    }
    Ok(())
}
fn suite(
    root: &Path,
    package: &str,
    target: &[&str],
    filter: Option<&str>,
    expected: &[&str],
    records: &mut Vec<Value>,
) -> Result<()> {
    let mut args = vec!["test", "--locked", "-p", package];
    args.extend_from_slice(target);
    if let Some(filter) = filter {
        args.push(filter)
    }
    let mut listed = args.clone();
    listed.extend_from_slice(&["--", "--list"]);
    inventory(&command(root, "cargo", &listed, records)?, expected)?;
    summary(&command(root, "cargo", &args, records)?, expected.len())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let corpus = read(root, "docs/rust-rewrite/w00/source.json")?;
    let exporter = read(root, "docs/rust-rewrite/w00/capture_source.py")?;
    let coverage = read(root, "docs/rust-rewrite/w00/coverage.json")?;
    let (data, mapping) = artifacts(&corpus, &exporter, &coverage)?;
    integration_artifact(
        &data,
        &read(root, "docs/rust-rewrite/w00/integration-source.json")?,
        &read(root, "slopsearx/mcp/tools.py")?,
    )?;
    command(
        root,
        "git",
        &["merge-base", "--is-ancestor", BASELINE, "HEAD"],
        records,
    )?;
    for (path, pin) in SOURCE_PINS {
        let expected_pin = if *path == "slopsearx/mcp/tools.py" {
            let historical = format!("{BASELINE}:{path}");
            if command(root, "git", &["rev-parse", &historical], records)?.trim() != *pin {
                return Err("W00 historical saved source pin changed".into());
            }
            CURRENT_MCP_PIN
        } else {
            *pin
        };
        if command(root, "git", &["hash-object", "--", path], records)?.trim() != expected_pin {
            return Err(format!("W00 source drift: {path}"));
        }
    }
    let known: BTreeSet<_> = INFRA_TESTS
        .iter()
        .chain(DOMAIN_TESTS.iter())
        .chain(APPLICATION_TESTS.iter())
        .map(|name| name.rsplit("::").next().unwrap())
        .collect();
    for row in mapping["source_test_mappings"]
        .as_array()
        .ok_or("missing source mappings")?
    {
        let tests = row["native_tests"]
            .as_array()
            .ok_or("missing native mapping")?;
        if tests
            .iter()
            .any(|name| name.as_str().is_none_or(|name| !known.contains(name)))
        {
            return Err("W00 source mapping references missing native test".into());
        }
        if tests.is_empty()
            && !row["remaining_owner"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
        {
            return Err("W00 unported source test has no explicit owner".into());
        }
    }
    let mut native = Vec::new();
    for path in NATIVE_SOURCES {
        native.push(json!({"path":path,"git_blob":command(root,"git",&["hash-object","--",path],records)?.trim()}))
    }
    let binary =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER").unwrap_or_else(|_| "valkey-server".into());
    let version = command(root, &binary, &["--version"], records)?;
    if !version.contains("v=8.1.10") {
        return Err("W00 Valkey version differs from accepted8.1.10".into());
    }
    suite(
        root,
        "ssx-domain",
        &["--lib"],
        Some("saved::"),
        DOMAIN_TESTS,
        records,
    )?;
    suite(
        root,
        "ssx-application",
        &["--lib"],
        Some("saved::"),
        APPLICATION_TESTS,
        records,
    )?;
    suite(
        root,
        "ssx-infra",
        &["--test", "w00"],
        None,
        INFRA_TESTS,
        records,
    )?;
    let docs = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--doc",
            "valkey::saved::Claim",
        ],
        records,
    )?;
    summary(&docs, 1)?;
    records.push(json!({"native_saved_acceptance":{"runtime":"Rust","backend":"private disposable Valkey8.1.10","source_observations":55,"source_test_mappings":37,"domain_tests":DOMAIN_TESTS.len(),"application_tests":APPLICATION_TESTS.len(),"store_tests":INFRA_TESTS.len(),"private_claim_compile_fail":1,"provider_calls":0,"native_identity_sources":native,"scope":"definitions, revisions, quotas, indexes and schedule ownership; W04/W05 publication and W09/W10 public surfaces remain downstream"}}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oracle_or_mapping_mutation_is_not_self_authorizing() {
        let corpus = include_bytes!("../../../docs/rust-rewrite/w00/source.json");
        let exporter = include_bytes!("../../../docs/rust-rewrite/w00/capture_source.py");
        let coverage = include_bytes!("../../../docs/rust-rewrite/w00/coverage.json");
        assert!(artifacts(corpus, exporter, coverage).is_ok());
        let (historical, _) = artifacts(corpus, exporter, coverage).unwrap();
        let integration = include_bytes!("../../../docs/rust-rewrite/w00/integration-source.json");
        let current = include_bytes!("../../../slopsearx/mcp/tools.py");
        assert!(integration_artifact(&historical, integration, current).is_ok());
        let mut changed = integration.to_vec();
        changed.push(b' ');
        assert!(integration_artifact(&historical, &changed, current).is_err());
        let mut changed = current.to_vec();
        changed.push(b' ');
        assert!(integration_artifact(&historical, integration, &changed).is_err());
        let mut changed = historical.clone();
        changed["observations"][0]["slot"] = json!(999);
        assert!(integration_artifact(&changed, integration, current).is_err());
        for (index, mut bytes) in [corpus.to_vec(), exporter.to_vec(), coverage.to_vec()]
            .into_iter()
            .enumerate()
        {
            bytes.push(b' ');
            let parts = match index {
                0 => artifacts(&bytes, exporter, coverage),
                1 => artifacts(corpus, &bytes, coverage),
                _ => artifacts(corpus, exporter, &bytes),
            };
            assert!(parts.is_err())
        }
    }
    #[test]
    fn mandatory_native_inventory_and_nonignored_execution_are_required() {
        assert!(inventory("alpha: test\nbeta: test\n", &["alpha", "beta"]).is_ok());
        assert!(inventory("alpha: test\n", &["alpha", "beta"]).is_err());
        assert!(inventory("alpha: test\nother: test\n", &["alpha", "beta"]).is_err());
        assert!(
            summary(
                "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured;",
                2
            )
            .is_ok()
        );
        for output in [
            "test result: ok. 1 passed; 0 failed; 1 ignored;",
            "test result: ok. 0 passed; 0 failed; 0 ignored;",
            "test result: ok. 2 passed; 0 failed; 0 ignored;\ntest result: ok. 0 passed; 0 failed; 0 ignored;",
        ] {
            assert!(summary(output, 2).is_err())
        }
    }
}
