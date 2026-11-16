//! A02 source-bound offline package adapters. No Python invocation or providers.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};
const CORPUS_SHA256: &str = "1d6d4a2937dca03dfe8f281c13481c9112b7317c389fc1226e6b78605c16ff45";
const COVERAGE_SHA256: &str = "e7946ca18c68e2b3143977c5a0e564d5d1e23ac2ccfe0bb640752a40e2385818";
const NATIVE_PATHS: &[&str] = &[
    "crates/xtask/fixtures/F01.json",
    "docs/rust-rewrite/inventory/python-disposition.json",
    "docs/rust-rewrite/inventory/python-invocations.json",
    "docs/rust-rewrite/inventory/DRIFT.md",
    "Cargo.toml",
    "Cargo.lock",
    "docs/adr/003-rust-workspace-and-contract-boundaries.md",
    "crates/ssx-application/src/lib.rs",
    "crates/ssx-domain/src/records.rs",
    "crates/ssx-domain/src/storage.rs",
    "crates/ssx-json/src/lib.rs",
    "contracts/unicode/casefold-16.0.0.json",
    "crates/ssx-registry/src/lib.rs",
    "crates/ssx-infra/Cargo.toml",
    "crates/ssx-infra/src/http.rs",
    "crates/ssx-infra/src/adapters/mod.rs",
    "crates/ssx-infra/src/adapters/crates_io.rs",
    "crates/ssx-infra/src/adapters/pypi.rs",
    "crates/ssx-infra/src/adapters/rubygems.rs",
    "crates/ssx-infra/src/adapters/dockerhub.rs",
    "crates/ssx-infra/tests/a02_packages.rs",
    "docs/rust-rewrite/a02/corpus.json",
    "docs/rust-rewrite/a02/corpus-extra.json",
    "docs/rust-rewrite/a02/corpus-review.json",
    "docs/rust-rewrite/a02/corpus-meta.json",
    "docs/rust-rewrite/a02/meta-first-failure.txt",
    "docs/rust-rewrite/a02/empty-base-source.json",
    "docs/rust-rewrite/a02/independent-review.md",
    "docs/rust-rewrite/a02/oracle/export_extra.py",
    "docs/rust-rewrite/a02/coverage.json",
    "docs/rust-rewrite/a02/README.md",
    "docs/rust-rewrite/a02/first-failures.md",
    "docs/rust-rewrite/a02/oracle/export.py",
    "crates/xtask/src/a02.rs",
    "crates/xtask/src/main.rs",
    ".github/workflows/rust.yml",
    "docs/rust-rewrite/ci/gates.json",
];
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn pinned(root: &Path, path: &str, pin: &str) -> Result<Value> {
    let bytes = std::fs::read(root.join(path)).map_err(|e| e.to_string())?;
    if digest(&bytes) != pin {
        return Err(format!("A02 reviewed fixture drift: {path}"));
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn test_inventory(
    root: &Path,
    args: &[&str],
    expected: &BTreeSet<String>,
    records: &mut Vec<Value>,
) -> Result<()> {
    let mut listed_args = args.to_vec();
    listed_args.extend(["--", "--list"]);
    let listed = command(root, "cargo", &listed_args, records)?;
    let actual: BTreeSet<_> = listed
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .map(str::to_owned)
        .collect();
    if actual != *expected || actual.is_empty() {
        return Err("A02 native test inventory differs from frozen manifest".into());
    }
    let output = command(root, "cargo", args, records)?;
    let exact = format!(
        "test result: ok. {} passed; 0 failed; 0 ignored; 0 measured;",
        expected.len()
    );
    if !output.lines().any(|line| line.starts_with(&exact)) {
        return Err("A02 did not execute exact nonignored passing tests".into());
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let corpus = pinned(root, "docs/rust-rewrite/a02/corpus.json", CORPUS_SHA256)?;
    let coverage = pinned(root, "docs/rust-rewrite/a02/coverage.json", COVERAGE_SHA256)?;
    if corpus["plan_id"] != "A02"
        || coverage["plan_id"] != "A02"
        || corpus["source_commit"] != "025ab2f2d50febcb573ba62579eecf7e2d07ef3a"
        || coverage["corpus_sha256"] != CORPUS_SHA256
    {
        return Err("A02 fixture identity changed".into());
    }
    command(
        root,
        "git",
        &[
            "merge-base",
            "--is-ancestor",
            "025ab2f2d50febcb573ba62579eecf7e2d07ef3a",
            "HEAD",
        ],
        records,
    )?;
    let sources = corpus["source_files"]
        .as_array()
        .ok_or("missing A02 source inventory")?;
    if sources.len() != 8 {
        return Err("A02 source inventory incomplete".into());
    }
    for source in sources {
        let path = source["path"].as_str().ok_or("missing source path")?;
        let actual = command(root, "git", &["hash-object", "--", path], records)?;
        if Some(actual.trim()) != source["git_blob"].as_str() {
            return Err(format!("A02 source drift: {path}"));
        }
        let bytes = std::fs::read(root.join(path)).map_err(|e| e.to_string())?;
        if Some(digest(&bytes).as_str()) != source["sha256"].as_str() {
            return Err(format!("A02 source digest drift: {path}"));
        }
    }
    let extra = pinned(
        root,
        "docs/rust-rewrite/a02/corpus-extra.json",
        "5df0064281e5cf633b700e30866de2a1a410b1771a509638a45e01cad061b1d5",
    )?;
    let extra_cases = extra["cases"]
        .as_array()
        .ok_or("missing supplemental cases")?;
    if extra_cases.len() != 13 {
        return Err("A02 supplemental cases incomplete".into());
    }
    let review = pinned(
        root,
        "docs/rust-rewrite/a02/corpus-review.json",
        "4f0afe2be01b901079de9eb90e1e94ffc32817c831b351cf59e5c5cafc3e0fa7",
    )?;
    let review_cases = review["cases"].as_array().ok_or("missing reviewed cases")?;
    if review_cases.len() != 20 {
        return Err("review corpus incomplete".into());
    }
    pinned(
        root,
        "docs/rust-rewrite/a02/empty-base-source.json",
        "7f9028f727ae16cbb4d8887e8ca31bc0e819d56dd1f2982c249b657c62550436",
    )?;
    let meta = pinned(
        root,
        "docs/rust-rewrite/a02/corpus-meta.json",
        "9e3c39711372583d732cd51a3a8f8436e5f483e0d7cf49990335d85455d74331",
    )?;
    let meta_cases = meta["cases"].as_array().ok_or("missing meta cases")?;
    if meta_cases.len() != 3 {
        return Err("meta corpus incomplete".into());
    }
    let cases = corpus["cases"].as_array().ok_or("missing cases")?;
    let ids: BTreeSet<_> = cases.iter().filter_map(|c| c["id"].as_str()).collect();
    if cases.len() != 122 || ids.len() != 122 {
        return Err("A02 case identities incomplete".into());
    }
    let tests: BTreeSet<String> = coverage["native_tests"]
        .as_array()
        .ok_or("missing tests")?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    if tests.len() != 6 {
        return Err("A02 integration test inventory incomplete".into());
    }
    let mappings = coverage["source_tests"]
        .as_array()
        .ok_or("missing source test mapping")?;
    let mut old = BTreeSet::new();
    for row in mappings {
        if !old.insert(row["source_test"].as_str().ok_or("missing source test")?)
            || !tests.contains(
                row["native_test"]
                    .as_str()
                    .ok_or("missing mapped native test")?,
            )
        {
            return Err("A02 source mapping invalid".into());
        }
        for case in row["corpus_cases"]
            .as_array()
            .ok_or("missing mapped corpus cases")?
        {
            if !ids.contains(case.as_str().ok_or("invalid mapped case")?) {
                return Err("A02 source mapping references absent case".into());
            }
        }
    }
    if old.len() != 18 {
        return Err("A02 old regression inventory incomplete".into());
    }
    let mut native = Vec::new();
    for path in NATIVE_PATHS {
        let blob = command(root, "git", &["hash-object", "--", path], records)?;
        native.push(json!({"path":path,"git_blob":blob.trim()}));
    }
    test_inventory(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "a02_packages",
        ],
        &tests,
        records,
    )?;
    let unit: BTreeSet<String> = coverage["native_unit_tests"]
        .as_array()
        .ok_or("missing unit tests")?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let dockerhub_unit = BTreeSet::from([
        "adapters::dockerhub::tests::default_route_scope_and_percent_encoding_match_source"
            .to_owned(),
    ]);
    let pypi_unit = BTreeSet::from([
        "http::pypi_operation_timeout_tests::pypi_uses_fresh_per_operation_timeouts_on_the_caller_pool"
            .to_owned(),
    ]);
    let expected_unit: BTreeSet<String> = dockerhub_unit.union(&pypi_unit).cloned().collect();
    if unit != expected_unit {
        return Err("A02 unit test inventory incomplete".into());
    }
    test_inventory(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "adapters::dockerhub::",
        ],
        &dockerhub_unit,
        records,
    )?;
    test_inventory(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "http::pypi_operation_timeout_tests::pypi_uses_fresh_per_operation_timeouts_on_the_caller_pool",
        ],
        &pypi_unit,
        records,
    )?;
    // F06 registry completeness remains separate from adapter factory availability.
    let registry_tests: BTreeSet<String> = [
        "tests::builtins_are_complete_and_unported_engines_are_unavailable",
        "tests::duplicate_descriptor_and_invalid_specialist_card_fail_closed",
        "tests::fixture_adapter_registers_through_the_engine_owned_seam",
        "tests::filter_consumption_and_enforcement_are_distinct_layered_metadata",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    test_inventory(
        root,
        &["test", "--locked", "-p", "ssx-registry", "--lib"],
        &registry_tests,
        records,
    )?;
    records.push(json!({"contract":"A02","corpus_sha256":CORPUS_SHA256,"coverage_sha256":COVERAGE_SHA256,"cases":ids,"supplemental_cases":extra_cases.iter().map(|c|&c["id"]).collect::<Vec<_>>(),"review_cases":review_cases.iter().map(|c|&c["id"]).collect::<Vec<_>>(),"meta_cases":meta_cases.iter().map(|c|&c["id"]).collect::<Vec<_>>(),"source_test_count":old.len(),"tests":tests,"unit_tests":unit,"native_identity":native,"status":"passed","scope":"offline native named package adapters; no providers, deployment or end-to-end server availability"}));
    Ok(())
}
