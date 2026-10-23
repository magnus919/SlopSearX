//! A03 source-bound community adapters: no Python or provider calls at gate time.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};
const CASES_SHA256: &str = "3f91f6fa2acb3a03e6d535d18f548f7451e2697eb1811ff8aef8b38394e048df";
const COVERAGE_SHA256: &str = "70086584c0bac69318a4fb02272439ea361a3be5201586c3607ea2229dba5c63";
const EXPORTER_SHA256: &str = "f8a492111e67eed9bea3733d78b67574c10abd943143cfd5583cb0fd527a9ed7";
const SOURCE: &[(&str, &str)] = &[
    (
        "engines/github.py",
        "58950161475abc9c4d529637ddc998d96cd9c6d4",
    ),
    (
        "engines/huggingface.py",
        "9dbfc93b66a5c46c80109555e78c0ad2515ec8e1",
    ),
    (
        "engines/stackexchange.py",
        "1ea554802e6358ad2f4d62b5711d14309403d6ff",
    ),
    (
        "engines/hackernews.py",
        "9b3384fa85e391f1c827552292603eeb98cf07a7",
    ),
    (
        "slopsearx/adapter.py",
        "664e853d9ab1330596d01899ebe015701de09e88",
    ),
    (
        "slopsearx/payload.py",
        "efa149d48bae401f7a0a9df6e50c8c614adecbb5",
    ),
    (
        "slopsearx/config.py",
        "761db0521d74ec6d955aa8a84fd92a4a62533c43",
    ),
    (
        "tests/test_github.py",
        "46e84d0020f16768414da0fd9badf206697846e4",
    ),
    (
        "tests/test_stackexchange_adapter.py",
        "7a70dd10e66d2c9c95aaf18dd18a38461214df38",
    ),
    (
        "tests/test_adapters.py",
        "788f5747f84438f96bacbec5c6d813dea44ef4cd",
    ),
    (
        "tests/test_tier1_adapters.py",
        "453cb90bfb4a31870f4e1a7370fcd1816dcb2d3c",
    ),
    (
        "tests/test_config.py",
        "ab1b85cb9ee446f99e87dedfcbb3c8babb41686b",
    ),
    (
        "tests/test_capabilities.py",
        "50e9ddccc5e1c8b4eaf54264c0d5e1e2a3b9a94c",
    ),
    ("uv.lock", "24e92a80e8ecc761880fe603a01da41312dea180"),
];
const NATIVE: &[&str] = &[
    "crates/ssx-infra/src/adapters/community.rs",
    "crates/ssx-infra/src/adapters/crates_io.rs",
    "crates/ssx-infra/src/adapters/github.rs",
    "crates/ssx-infra/src/adapters/hackernews.rs",
    "crates/ssx-infra/src/adapters/huggingface.rs",
    "crates/ssx-infra/src/adapters/mod.rs",
    "crates/ssx-infra/src/adapters/npm.rs",
    "crates/ssx-infra/src/adapters/stackexchange.rs",
    "crates/ssx-infra/tests/a03_community.rs",
    "crates/ssx-application/src/calendar.rs",
    "crates/ssx-application/src/lib.rs",
    "crates/ssx-domain/src/records.rs",
    "crates/ssx-domain/src/storage.rs",
    "crates/ssx-domain/src/oauth.rs",
    "crates/ssx-registry/src/lib.rs",
    "crates/ssx-infra/src/http.rs",
    "crates/ssx-infra/src/valkey/rate.rs",
    "crates/ssx-application/src/rate.rs",
    "crates/xtask/src/a03.rs",
    "crates/xtask/src/main.rs",
    "docs/rust-rewrite/a03/README.md",
    "docs/rust-rewrite/a03/PARITY.md",
    "docs/rust-rewrite/a03/cases.json",
    "docs/rust-rewrite/a03/capture.py",
    "docs/rust-rewrite/a03/coverage.json",
    ".github/workflows/rust.yml",
    "docs/rust-rewrite/ci/gates.json",
    "Cargo.lock",
    "crates/ssx-infra/Cargo.toml",
    "docs/rust-rewrite/f06/fragments/github.json",
    "docs/rust-rewrite/f06/fragments/huggingface.json",
    "docs/rust-rewrite/f06/fragments/stackexchange.json",
    "docs/rust-rewrite/f06/fragments/hackernews.json",
];
const TESTS: &[&str] = &[
    "actual_private_valkey_f10_quota_and_cooldown_stop_all_four_before_dispatch",
    "dropping_search_cancels_actual_request_without_retry_or_closing_shared_pool",
    "four_native_factories_keep_source_f06_capabilities_and_jev_cards",
    "operator_numeric_projection_refuses_all_28_captured_invalid_configurations",
    "shared_registry_preserves_pilots_and_caller_transport_after_adapter_drop",
    "source_232_observations_replay_over_actual_f08_requests",
    "typed_config_bounds_zero_deadline_and_secret_redaction_before_dispatch",
    "unavailable_admission_is_fail_closed_before_http_and_auth_is_separate",
];

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn reviewed(bytes: &[u8], expected: &str) -> Result<()> {
    if digest(bytes) != expected {
        return Err("A03 bytes differ from independently reviewed source evidence".into());
    }
    Ok(())
}
fn summary(output: &str, count: usize) -> Result<()> {
    let rows = output
        .lines()
        .filter(|line| line.starts_with("test result:"))
        .collect::<Vec<_>>();
    if rows.len() != 1 || !rows[0].contains(&format!("{count} passed; 0 failed; 0 ignored")) {
        return Err("A03 tests did not execute exact required inventory with zero ignored".into());
    }
    Ok(())
}
fn inventory(output: &str, expected: &[&str]) -> Result<()> {
    let actual: BTreeSet<_> = output
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect();
    if actual != expected.iter().copied().collect() {
        return Err("A03 named test inventory differs".into());
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    command(
        root,
        "git",
        &[
            "merge-base",
            "--is-ancestor",
            "35d97ccf8c9b2d8246373b2024cc8d7a457705a3",
            "HEAD",
        ],
        records,
    )?;
    for (path, sha) in [
        ("cases.json", CASES_SHA256),
        ("coverage.json", COVERAGE_SHA256),
        ("capture.py", EXPORTER_SHA256),
    ] {
        reviewed(
            &std::fs::read(root.join("docs/rust-rewrite/a03").join(path))
                .map_err(|e| e.to_string())?,
            sha,
        )?;
    }
    let coverage: Value = crate::load_json(&root.join("docs/rust-rewrite/a03/coverage.json"))?;
    let native = coverage["native_identity_sources"]
        .as_array()
        .ok_or("missing A03 native inventory")?;
    if native
        .iter()
        .filter_map(Value::as_str)
        .ne(NATIVE.iter().copied())
        || native.len() != NATIVE.len()
    {
        return Err("A03 native source inventory differs".into());
    }
    let sources = coverage["source_observations"]
        .as_array()
        .ok_or("missing A03 source observations")?;
    if sources.len() != SOURCE.len()
        || sources
            .iter()
            .zip(SOURCE)
            .any(|(r, (p, b))| r["path"] != *p || r["git_blob"] != *b)
    {
        return Err("A03 source observation inventory differs".into());
    }
    for (path, expected) in SOURCE {
        let current = command(root, "git", &["hash-object", "--", path], records)?;
        if *path == crate::t06::lock_amendment::SOURCE_PATH {
            crate::t06::lock_amendment::verify(
                root,
                path,
                crate::t06::lock_amendment::HISTORICAL_SHA256,
            )?;
            crate::t06::lock_amendment::verify_git_pins(path, expected, expected, current.trim())?;
        } else if current.trim() != *expected {
            return Err(format!("A03 source drift: {path}"));
        }
    }
    let mut identities = Vec::new();
    for path in NATIVE {
        identities.push(json!({"path":path,"git_blob":command(root,"git", &["hash-object","--",path],records)?.trim()}));
    }
    records.push(json!({"A03_native_sources":identities,"scope":"four engine-local adapters through caller-owned F08/F10, actual private Valkey and loopback fixtures; no live providers or deployment"}));
    let gate = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "xtask", "a03::tests::"],
        records,
    )?;
    summary(&gate, 3)?;
    let listed = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "a03_community",
            "--",
            "--list",
        ],
        records,
    )?;
    inventory(&listed, TESTS)?;
    let tests = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "a03_community",
        ],
        records,
    )?;
    summary(&tests, TESTS.len())?;
    for (package, name) in [
        (
            "ssx-infra",
            "adapters::community::tests::fractional_timeout_and_zero_results_remain_representable",
        ),
        (
            "ssx-infra",
            "adapters::stackexchange::tests::zero_string_slice_precedes_row_interpretation_without_accepting_unsliceable_values",
        ),
        (
            "ssx-config",
            "tests::hf_alias_is_overridden_by_explicit_variable_and_secrets_are_redacted",
        ),
    ] {
        let listed = command(
            root,
            "cargo",
            &[
                "test", "--locked", "-p", package, "--lib", name, "--", "--list", "--exact",
            ],
            records,
        )?;
        inventory(&listed, &[name])?;
        let tests = command(
            root,
            "cargo",
            &[
                "test", "--locked", "-p", package, "--lib", name, "--", "--exact",
            ],
            records,
        )?;
        summary(&tests, 1)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_metadata_cannot_authorize_mutated_oracle() {
        let original = include_bytes!("../../../docs/rust-rewrite/a03/cases.json");
        assert!(reviewed(original, CASES_SHA256).is_ok());
        let mut modified = original.to_vec();
        modified.push(b' ');
        assert!(reviewed(&modified, CASES_SHA256).is_err());
        assert!(reviewed(original, COVERAGE_SHA256).is_err());
    }
    #[test]
    fn removed_or_renamed_execution_is_rejected() {
        assert!(inventory("one: test\ntwo: test", &["one", "two"]).is_ok());
        assert!(inventory("one: test", &["one", "two"]).is_err());
        assert!(inventory("one: test\nother: test", &["one", "two"]).is_err());
    }
    #[test]
    fn ignored_or_zero_execution_is_rejected() {
        assert!(
            summary(
                "test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out",
                8
            )
            .is_ok()
        );
        assert!(
            summary(
                "test result: ok. 7 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out",
                8
            )
            .is_err()
        );
        assert!(
            summary(
                "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out",
                8
            )
            .is_err()
        );
    }
}
