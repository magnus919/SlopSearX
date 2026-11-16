//! Source-bound native tests for the npm/crates registry-search pilot.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};

const SOURCE_OBSERVATIONS: [(&str, &str); 8] = [
    ("engines/npm.py", "8ef11a1ad4e71437c1fcb0d88c35b4e4140c7c06"),
    (
        "engines/crates.py",
        "89308bdf1ff037b924003b7c2c6283006744e621",
    ),
    (
        "docs/rust-rewrite/f06/fragments/npm.json",
        "6d7b391ec908ed2e24f6665b060f29ceaabdee7f",
    ),
    (
        "docs/rust-rewrite/f06/fragments/crates.json",
        "de5a8a025731e0b5c9d1bd6d33120d9534704ef8",
    ),
    (
        "tests/test_new_engines.py",
        "4e5293d694bb40332584a9541b6ad41326a01cc0",
    ),
    (
        "tests/test_upstream_contracts.py",
        "38890808971958c1322b502a885d71f5db5d8d64",
    ),
    (
        "tests/fixtures/upstream_contracts/npm.json",
        "76fb9a81b01db87cd001434ef6b93010bb10dbfc",
    ),
    (
        "tests/fixtures/upstream_contracts/crates.json",
        "1409df4adc780242212ea65f17f5abf75d76b878",
    ),
];

const NATIVE_IDENTITY_SOURCES: &[&str] = &[
    "docs/rust-rewrite/a01/README.md",
    "docs/rust-rewrite/a01/cases.json",
    "docs/adr/003-rust-workspace-and-contract-boundaries.md",
    "crates/ssx-application/src/lib.rs",
    "crates/ssx-application/src/rate.rs",
    "crates/ssx-domain/src/lib.rs",
    "crates/ssx-domain/src/storage.rs",
    "crates/ssx-domain/src/records.rs",
    "crates/ssx-domain/src/rate.rs",
    "crates/ssx-registry/Cargo.toml",
    "crates/ssx-registry/src/lib.rs",
    "crates/ssx-infra/Cargo.toml",
    "crates/ssx-infra/src/lib.rs",
    "crates/ssx-infra/src/http.rs",
    "crates/ssx-infra/src/adapters/mod.rs",
    "crates/ssx-infra/src/adapters/npm.rs",
    "crates/ssx-infra/src/adapters/crates_io.rs",
    "crates/ssx-infra/tests/a01_pilots.rs",
    "crates/xtask/src/a01.rs",
    "crates/xtask/src/main.rs",
    ".github/workflows/rust.yml",
    "docs/rust-rewrite/ci/gates.json",
];

// Updated only with review of the complete source-mapped case and inventory files.
const CASES_SHA256: &str = "3c17a05a2aac67c86e5dd72b24d8854f8497fede22b227e2bba08af894a629ef";
const COVERAGE_SHA256: &str = "45c6abe5d280d1ce30c8759ec6eece1d584c69c66794cd1d8a0e9e6545e45a4e";

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_fixture_digest(bytes: &[u8], expected: &str) -> Result<()> {
    if sha256_hex(bytes) != expected {
        return Err("A01 fixture digest differs from reviewed bytes".into());
    }
    Ok(())
}

fn validate_source_inventory(sources: &[Value]) -> Result<()> {
    if sources.len() != SOURCE_OBSERVATIONS.len()
        || sources
            .iter()
            .zip(SOURCE_OBSERVATIONS)
            .any(|(row, (path, blob))| {
                row["path"].as_str() != Some(path) || row["git_blob"].as_str() != Some(blob)
            })
    {
        return Err("A01 source observation inventory differs from reviewed baseline".into());
    }
    Ok(())
}

fn validate_native_inventory(sources: &[Value]) -> Result<Vec<String>> {
    let paths: Vec<_> = sources
        .iter()
        .map(|source| {
            source
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "invalid A01 native identity path".to_owned())
        })
        .collect::<Result<_>>()?;
    if paths
        .iter()
        .map(String::as_str)
        .ne(NATIVE_IDENTITY_SOURCES.iter().copied())
    {
        return Err("A01 native identity inventory differs from reviewed source set".into());
    }
    Ok(paths)
}

fn require_test_summary(output: &str, expected: usize, label: &str) -> Result<()> {
    let summaries: Vec<_> = output
        .lines()
        .filter(|line| line.starts_with("test result:"))
        .collect();
    if !summaries
        .iter()
        .any(|line| line.contains(&format!("{expected} passed; 0 failed; 0 ignored")))
        || summaries.iter().any(|line| !line.contains("0 ignored"))
    {
        return Err(format!(
            "{label} did not execute exactly {expected} non-ignored passing tests"
        ));
    }
    Ok(())
}

pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let coverage_bytes = std::fs::read(root.join("docs/rust-rewrite/a01/coverage.json"))
        .map_err(|e| e.to_string())?;
    validate_fixture_digest(&coverage_bytes, COVERAGE_SHA256)
        .map_err(|_| "A01 coverage manifest digest differs from reviewed fixture")?;
    let coverage: Value = serde_json::from_slice(&coverage_bytes).map_err(|e| e.to_string())?;
    if coverage["schema_version"] != 1
        || coverage["plan_id"] != "A01"
        || coverage["baseline"] != "1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5"
        || !coverage["provenance"]
            .as_str()
            .is_some_and(|value| value.contains("no live provider calls"))
    {
        return Err("A01 coverage identity or no-provider provenance changed".into());
    }
    command(
        root,
        "git",
        &[
            "merge-base",
            "--is-ancestor",
            "1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5",
            "HEAD",
        ],
        records,
    )?;

    let sources = coverage["source_observations"]
        .as_array()
        .ok_or("missing A01 immutable source observations")?;
    validate_source_inventory(sources)?;
    let mut paths = BTreeSet::new();
    for source in sources {
        let path = source["path"].as_str().ok_or("missing A01 source path")?;
        let pin = source["git_blob"]
            .as_str()
            .ok_or("missing A01 source blob")?;
        if path.is_empty()
            || Path::new(path).is_absolute()
            || path.split('/').any(|part| part == "..")
            || !paths.insert(path)
        {
            return Err("invalid or duplicate A01 source path".into());
        }
        let actual = command(root, "git", &["hash-object", "--", path], records)?;
        if actual.trim() != pin {
            return Err(format!("A01 source drift: {path}"));
        }
    }
    if sources.len() != 8 {
        return Err("A01 source observation inventory is incomplete".into());
    }

    let native_sources = coverage["native_identity_sources"]
        .as_array()
        .ok_or("missing A01 native identity source paths")?;
    let expected_native_paths = validate_native_inventory(native_sources)?;
    let mut native_paths = BTreeSet::new();
    let mut native_identity = Vec::new();
    for path in &expected_native_paths {
        if path.is_empty()
            || Path::new(path).is_absolute()
            || path.split('/').any(|part| part == "..")
            || !native_paths.insert(path)
        {
            return Err("invalid or duplicate A01 native identity path".into());
        }
        let blob = command(root, "git", &["hash-object", "--", path], records)?;
        native_identity.push(json!({"path":path,"git_blob":blob.trim()}));
    }

    let cases_bytes =
        std::fs::read(root.join("docs/rust-rewrite/a01/cases.json")).map_err(|e| e.to_string())?;
    validate_fixture_digest(&cases_bytes, CASES_SHA256)
        .map_err(|_| "A01 case manifest digest differs from reviewed fixture")?;
    let cases: Value = serde_json::from_slice(&cases_bytes).map_err(|e| e.to_string())?;
    if cases["schema_version"] != 1
        || cases["plan_id"] != "A01"
        || !cases["provenance"]
            .as_str()
            .is_some_and(|value| value.contains("no live provider calls"))
    {
        return Err("A01 case manifest identity/provenance mismatch".into());
    }
    let case_rows = cases["cases"].as_array().ok_or("missing A01 cases")?;
    let observations = cases["source_observations"]
        .as_array()
        .ok_or("missing offline Python source observations")?;
    if observations.len() != 47
        || observations.iter().any(|row| {
            !matches!(
                row["captured_by"].as_str(),
                Some("engines/crates.py via offline MockTransport")
                    | Some(
                        "engines/crates.py via offline httpx.MockTransport; CratesAdapter.search"
                    )
                    | Some("engines/npm.py via offline httpx.MockTransport; NpmAdapter.search")
            ) || row["case"].as_str().is_none_or(str::is_empty)
                || !matches!(row["output"]["status"].as_str(), Some("ok" | "error"))
        })
    {
        return Err("invalid or incomplete offline crates source observations".into());
    }
    let mut ids = BTreeSet::new();
    let mut expected_tests = BTreeSet::new();
    let mut source_tests = BTreeSet::new();
    for row in case_rows {
        let id = row["id"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or("invalid A01 case id")?;
        let test = row["test"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or("invalid A01 test name")?;
        if !ids.insert(id.to_owned()) {
            return Err(format!("duplicate A01 case: {id}"));
        }
        expected_tests.insert(test.to_owned());
        if let Some(source_test) = row["source_test"].as_str()
            && !source_tests.insert(source_test.to_owned())
        {
            return Err(format!("duplicate A01 source test mapping: {source_test}"));
        }
    }
    let expected_source_tests: BTreeSet<_> = [
        "TestNpmAdapter.test_search_returns_results",
        "TestNpmAdapter.test_search_empty",
        "TestNpmAdapter.test_non_mapping_package_links_are_ignored",
        "TestNpmAdapter.test_search_rate_limited",
        "TestNpmAdapter.test_adapter_registered",
        "TestCratesAdapter.test_search_returns_results",
        "TestCratesAdapter.test_search_empty",
        "TestCratesAdapter.test_adapter_registered",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if ids.len() != 78 || source_tests != expected_source_tests {
        return Err("A01 case manifest is incomplete".into());
    }
    let required_cases: BTreeSet<_> = [
        "npm_query_replacement",
        "crates_query_replacement",
        "configured_max_results_and_ignored_page",
        "npm_payload_omissions_and_provenance",
        "crates_payload_absent",
        "npm_valid_empty_array",
        "crates_valid_empty_array",
        "npm_wrong_item_shape_error",
        "crates_wrong_item_shape_error",
        "npm_http_429_rate_limited",
        "crates_http_429_rate_limited",
        "npm_http_404_error",
        "crates_http_404_error",
        "npm_http_503_error",
        "crates_http_503_error",
        "npm_timeout",
        "crates_timeout",
        "npm_invalid_base_url",
        "crates_invalid_base_url",
        "npm_drop_cancels_f08_socket",
        "crates_drop_cancels_f08_socket",
        "crates_download_format_observations",
        "crates_large_integer_exact_display",
        "crates_integer_above_u64_exact_format",
        "crates_float_zero_is_falsey",
        "npm_publisher_float_zero_is_falsey",
        "crates_raw_json_float_trailing_zero",
        "crates_raw_json_float_rounding",
        "crates_raw_json_float_underflow_zero",
        "crates_raw_json_negative_float_zero",
        "npm_nested_python_container_stringification",
        "crates_integral_float_preserves_decimal",
        "crates_fractional_float_grouping",
        "crates_numeric_string_format_error",
        "crates_null_without_recent_error",
        "crates_null_with_recent_success",
        "crates_true_boolean_format",
        "crates_false_boolean_score",
        "crates_recent_numeric_string_score",
        "crates_recent_null_download_fallback",
        "npm_publisher_falsey_username_omitted",
        "npm_empty_container_descriptions",
        "npm_empty_container_popularity_zero",
        "npm_popularity_python_float_string_grammar",
        "npm_popularity_invalid_digit_separators_rejected",
        "crates_empty_container_descriptions",
        "crates_recent_float_whitespace_underscores_and_unicode",
        "crates_recent_float_invalid_digit_separators",
        "crates_float_scientific_large_1e16",
        "crates_float_scientific_large_1e20",
        "crates_float_scientific_small_1e5",
        "crates_float_decimal_boundary_1e4",
        "crates_float_decimal_boundary_1e15",
        "f10_composition_shared_clock_no_dispatch",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if !required_cases.iter().all(|case| ids.contains(case)) {
        return Err("A01 source-to-native contract matrix is incomplete".into());
    }

    let gate_tests = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "xtask", "a01::tests::"],
        records,
    )?;
    require_test_summary(&gate_tests, 3, "A01 gate validation tests")?;

    let listed = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "a01_pilots",
            "--",
            "--list",
        ],
        records,
    )?;
    let actual_tests: BTreeSet<_> = listed
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .map(str::to_owned)
        .collect();
    if actual_tests != expected_tests {
        return Err("A01 native test inventory differs from source mapping".into());
    }
    let native_output = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "a01_pilots",
        ],
        records,
    )?;
    require_test_summary(&native_output, expected_tests.len(), "A01 adapter tests")?;

    let registry_list = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-registry", "--", "--list"],
        records,
    )?;
    let actual_registry_tests: BTreeSet<_> = registry_list
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .map(str::to_owned)
        .collect();
    let expected_registry_tests: BTreeSet<_> = [
        "tests::builtins_are_complete_and_unported_engines_are_unavailable",
        "tests::duplicate_descriptor_and_invalid_specialist_card_fail_closed",
        "tests::fixture_adapter_registers_through_the_engine_owned_seam",
        "tests::filter_consumption_and_enforcement_are_distinct_layered_metadata",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if actual_registry_tests != expected_registry_tests {
        return Err("registry fixture test inventory changed".into());
    }
    let registry_output = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-registry"],
        records,
    )?;
    require_test_summary(
        &registry_output,
        expected_registry_tests.len(),
        "registry tests",
    )?;

    let metadata = command(
        root,
        "cargo",
        &["metadata", "--locked", "--no-deps", "--format-version", "1"],
        records,
    )?;
    let metadata: Value = serde_json::from_str(&metadata).map_err(|e| e.to_string())?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("cargo metadata packages missing")?;
    let dependencies = |name: &str| -> Result<BTreeSet<String>> {
        let package = packages
            .iter()
            .find(|package| package["name"] == name)
            .ok_or_else(|| format!("missing {name} package"))?;
        Ok(package["dependencies"]
            .as_array()
            .ok_or("package dependencies missing")?
            .iter()
            .filter_map(|dependency| dependency["name"].as_str().map(str::to_owned))
            .collect())
    };
    let app = dependencies("ssx-application")?;
    let registry = dependencies("ssx-registry")?;
    let infra = dependencies("ssx-infra")?;
    if app.contains("ssx-registry")
        || app.contains("ssx-config")
        || !registry.contains("ssx-application")
        || !infra.contains("ssx-registry")
        || !infra.contains("ssx-application")
    {
        return Err("A01 application-owned port dependency direction is invalid".into());
    }
    records.push(json!({"contract":"A01","coverage":"docs/rust-rewrite/a01/coverage.json","source_observations":sources.len(),"native_identity":native_identity,"cases":ids.len(),"tests":actual_tests,"status":"passed","scope":"offline source-mapped npm/crates adapters via registry and F08 loopback; no live provider calls"}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_digest_rejects_any_manifest_edit() {
        assert!(validate_fixture_digest(b"reviewed", &sha256_hex(b"reviewed")).is_ok());
        assert!(validate_fixture_digest(b"edited", &sha256_hex(b"reviewed")).is_err());
    }

    #[test]
    fn source_inventory_rejects_empty_shortened_or_replaced_inputs() {
        let full: Vec<_> = SOURCE_OBSERVATIONS
            .iter()
            .map(|(path, blob)| json!({"path":path,"git_blob":blob}))
            .collect();
        assert!(validate_source_inventory(&full).is_ok());
        assert!(validate_source_inventory(&[]).is_err());
        assert!(validate_source_inventory(&full[..full.len() - 1]).is_err());
        let mut replaced = full.clone();
        replaced[0]["path"] = json!("tests/other.py");
        assert!(validate_source_inventory(&replaced).is_err());
        replaced = full.clone();
        replaced[0]["git_blob"] = json!("0000000000000000000000000000000000000000");
        assert!(validate_source_inventory(&replaced).is_err());
    }

    #[test]
    fn native_identity_rejects_empty_or_modified_inventory() {
        let full: Vec<_> = NATIVE_IDENTITY_SOURCES
            .iter()
            .map(|path| json!(path))
            .collect();
        assert_eq!(
            validate_native_inventory(&full).unwrap().len(),
            NATIVE_IDENTITY_SOURCES.len()
        );
        assert!(validate_native_inventory(&[]).is_err());
        assert!(validate_native_inventory(&full[..full.len() - 1]).is_err());
        let mut changed = full;
        changed[0] = json!("docs/empty.md");
        assert!(validate_native_inventory(&changed).is_err());
    }
}
