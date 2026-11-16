//! Source-pinned, offline native regressions for the ATS adapters.
use crate::{Result, command};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};

#[path = "../../../docs/rust-rewrite/a14/current-helper-amendment.rs"]
mod helper_amendment;
#[path = "../../../docs/rust-rewrite/a14/source-amendments/loopback-owned-fixture-successor.rs"]
mod loopback_owned_fixture_successor;

const SOURCE_BASELINE: &str = "a8a7b3e05ccb7ee2e536c0651163c429384f8738";
const SOURCES: &[(&str, &str)] = &[
    (
        "engines/ashby.py",
        "dbaa14b4f850342a3ba9dea012f13972c29b29fe",
    ),
    (
        "engines/greenhouse.py",
        "10c4ded5b7d612b5a6751f96298c4c4a1cf8a591",
    ),
    (
        "engines/lever.py",
        "284e2d570f266eb18df4c7396df87fabf5d6e684",
    ),
    (
        "slopsearx/jobs_utils.py",
        "d33a18497ed868f32b8724bc24c5807dd1fc6421",
    ),
    (
        "tests/test_jobs_adapters.py",
        "739c82162d70eb9c88706380bd3f3192f1153306",
    ),
    (
        "tests/test_payload.py",
        "4b5bc1a0940ad6c1214eb55764fb4aea57188de9",
    ),
    (
        "docs/rust-rewrite/f06/fragments/ashby.json",
        "6470c358b096701f1e2907a60cfac43574da5eb3",
    ),
    (
        "docs/rust-rewrite/f06/fragments/greenhouse.json",
        "0ff9b9c0db79920d12748fc2ac9aaed8883dc914",
    ),
    (
        "docs/rust-rewrite/f06/fragments/lever.json",
        "457c2156e6a70069fa29e5eb40a65479792778c0",
    ),
    (
        "crates/ssx-source-codecs/provenance/idna/inputs/UnicodeData-16.0.0.txt",
        "64258a37395359c2f8875fdf3432817c1c459c31",
    ),
    (
        "crates/ssx-source-codecs/provenance/idna/inputs/SpecialCasing-16.0.0.txt",
        "e83aba432f43ced82db088f18fa22b77da7d68e8",
    ),
];
const CASES_SHA: &str = "4d7e4d4208a514c0f2b7b9b137651d07c9953fdaa3836d8c3a30490b6f0cd5c5";
const SUBCASE_COUNT: usize = 178;
const NATIVE_CASES: &[&str] = &[
    "adapters::jobs::tests::system_proxy_guard_rejects_every_enabled_system_flag: test",
    "adapters::jobs::tests::resolved_extreme_numeric_timeouts_use_the_finite_caller_lifecycle: test",
    "adapters::jobs::tests::ashby_uses_graphql_post_and_normalizes_job_postings: test",
    "adapters::jobs::tests::company_scope_rejects_before_dispatch_and_404_is_successful_empty: test",
    "adapters::jobs::tests::configuration_errors_keep_source_http_and_slice_order: test",
    "adapters::jobs::tests::configured_paths_and_source_graphql_document_are_exact: test",
    "adapters::jobs::tests::every_provider_timeout_drop_and_owner_shutdown_close_and_join_exchange: test",
    "adapters::jobs::tests::greenhouse_builds_content_query_and_job_payload_with_absent_id_preserved: test",
    "adapters::jobs::tests::lever_binary64_quotient_then_cpython_half_even_dates: test",
    "adapters::jobs::tests::lever_epoch_milliseconds_match_utc_iso: test",
    "adapters::jobs::tests::lever_sends_mode_json_and_normalizes_categories_epoch_and_limit: test",
    "adapters::jobs::tests::malformed_payloads_and_http_failures_are_classified: test",
    "adapters::jobs::tests::original_python_inline_fixtures_match_every_asserted_field: test",
    "adapters::jobs::tests::provider_timeout_and_dropped_search_cancel_the_loopback_exchange: test",
    "adapters::jobs::tests::resolved_f06_factories_preserve_slice_kind_and_signed_limits: test",
    "adapters::jobs::tests::source_company_cases_are_preserved: test",
    "adapters::jobs::tests::source_iterables_slice_before_row_access_and_atomic_failure_precedence: test",
    "adapters::jobs::tests::source_response_bytes_decode_before_only_retained_output_projection: test",
];

fn validate_sources(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    command(
        root,
        "git",
        &["merge-base", "--is-ancestor", SOURCE_BASELINE, "HEAD"],
        records,
    )?;
    for (path, expected) in SOURCES {
        let actual = command(root, "git", &["hash-object", "--", path], records)?;
        if actual.trim() != *expected {
            return Err(format!("A14 pinned source drift: {path}"));
        }
    }
    Ok(())
}

fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn reviewed_cases(root: &Path) -> Result<Value> {
    let bytes =
        std::fs::read(root.join("docs/rust-rewrite/a14/cases.json")).map_err(|e| e.to_string())?;
    require(
        digest(&bytes) == CASES_SHA,
        "A14 reviewed source/subcase map changed",
    )?;
    let cases: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    require(
        cases["issue"] == 933 && cases["batch"] == "A14",
        "A14 source map identity changed",
    )?;
    for (path, blob) in SOURCES {
        require(
            cases["source_blobs"][*path] == *blob,
            "A14 source pin mismatch",
        )?;
    }
    require(
        cases["native_tests"]
            .as_array()
            .is_some_and(|a| a.len() == NATIVE_CASES.len()),
        "A14 function inventory count changed",
    )?;
    let tests = cases["native_tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap_or(""))
        .collect::<BTreeSet<_>>();
    require(
        tests
            == NATIVE_CASES
                .iter()
                .map(|s| s.strip_suffix(": test").unwrap())
                .collect(),
        "A14 reviewed function identities changed",
    )?;
    require(
        cases["native_subcases"]
            .as_array()
            .is_some_and(|a| a.len() == SUBCASE_COUNT),
        "A14 subcase inventory count changed",
    )?;
    require(
        cases["qualification_status"] == "pending_root_code_and_map_review"
            || cases["qualification_status"] == "approved_for_native_qualification",
        "A14 invalid qualification state",
    )?;
    Ok(cases)
}
fn execution(output: &str, expected: &BTreeSet<String>) -> Result<()> {
    let actual = output
        .lines()
        .filter_map(|s| s.strip_prefix("test ")?.strip_suffix(" ... ok"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let set = actual.iter().cloned().collect::<BTreeSet<_>>();
    require(
        actual.len() == expected.len() && set == *expected,
        "A14 exact test identities missing, duplicate, ignored, or failed",
    )?;
    let prefix = format!(
        "test result: ok. {} passed; 0 failed; 0 ignored; 0 measured; ",
        expected.len()
    );
    require(
        output
            .lines()
            .filter(|line| line.starts_with("test result:"))
            .count()
            == 1
            && output.lines().any(|line| {
                line.strip_prefix(&prefix)
                    .and_then(|s| s.split_once(" filtered out; finished in "))
                    .is_some_and(|(count, elapsed)| {
                        count.parse::<usize>().is_ok()
                            && elapsed
                                .strip_suffix('s')
                                .and_then(|value| value.parse::<f64>().ok())
                                .is_some_and(|seconds| seconds.is_finite() && seconds >= 0.)
                    })
            }),
        "A14 exact passed/failed/ignored execution counts changed",
    )
}
fn subcases(output: &str, expected: &[Value]) -> Result<()> {
    let expected = expected
        .iter()
        .map(|v| v.as_str().ok_or("A14 nontext subcase").map(str::to_owned))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let actual = output
        .lines()
        .filter_map(|line| line.strip_prefix("A14_CASE "))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let set = actual.iter().cloned().collect::<BTreeSet<_>>();
    let expected_set = expected.iter().cloned().collect::<BTreeSet<_>>();
    require(
        expected.len() == expected_set.len()
            && actual.len() == expected.len()
            && set == expected_set,
        "A14 completed subcase identities missing, extra, duplicate, or skipped",
    )
}
fn source_tree(root: &Path, records: &mut Vec<Value>) -> Result<String> {
    let paths = command(
        root,
        "git",
        &[
            "ls-files",
            "-co",
            "-z",
            "--exclude-standard",
            "--",
            "crates",
            "vendor",
            "engines",
            "slopsearx",
            "tests",
            "docs/rust-rewrite/f06",
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            ".cargo",
            "docs/rust-rewrite/a14",
            "docs/rust-rewrite/a13",
        ],
        records,
    )?;
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    for path in paths
        .split('\0')
        .filter(|path| !path.is_empty())
        .collect::<BTreeSet<_>>()
    {
        let data = std::fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
        hash.update((path.len() as u64).to_le_bytes());
        hash.update(path.as_bytes());
        hash.update((data.len() as u64).to_le_bytes());
        hash.update(&data);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
fn verify_test_inventory(root: &Path, cases: &Value, records: &mut Vec<Value>) -> Result<()> {
    let args = [
        "test",
        "--locked",
        "--offline",
        "-p",
        "ssx-infra",
        "jobs::tests",
        "--lib",
    ];
    let mut list_args = args.to_vec();
    list_args.extend(["--", "--list"]);
    let listed = command(root, "cargo", &list_args, records)?;
    let actual = listed
        .lines()
        .filter(|s| s.ends_with(": test"))
        .collect::<BTreeSet<_>>();
    let expected = NATIVE_CASES.iter().copied().collect::<BTreeSet<_>>();
    require(
        actual == expected && actual.len() == NATIVE_CASES.len(),
        "A14 listed native test inventory differs from reviewed map",
    )?;
    require(
        listed
            .lines()
            .any(|s| s == format!("{} tests, 0 benchmarks", NATIVE_CASES.len())),
        "A14 listed count differs",
    )?;
    let mut run_args = args.to_vec();
    run_args.extend(["--", "--show-output", "--test-threads=1"]);
    let output = command(root, "cargo", &run_args, records)?;
    let expected = NATIVE_CASES
        .iter()
        .map(|s| s.strip_suffix(": test").unwrap().to_owned())
        .collect();
    execution(&output, &expected)?;
    subcases(
        &output,
        cases["native_subcases"]
            .as_array()
            .ok_or("A14 subcases missing")?,
    )
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let cases = reviewed_cases(root)?;
    require(
        cases["qualification_status"] == "approved_for_native_qualification",
        "A14 root code/map approval is pending; qualification denied",
    )?;
    validate_sources(root, records)?;
    for (path, expected) in cases["accepted_helper_blobs"]
        .as_object()
        .ok_or("A14 accepted helper pins missing")?
    {
        let actual = command(root, "git", &["hash-object", "--", path], records)?;
        if let Some(result) = crate::a13::a06_source_successor::try_git(
            root,
            "docs/rust-rewrite/a14/cases.json",
            CASES_SHA,
            path,
            expected.as_str().unwrap_or(""),
            actual.trim(),
            |archive| command(root, "git", &["hash-object", "--", archive], records),
        ) {
            result?;
        } else if path == helper_amendment::SOURCE_PATH {
            helper_amendment::verify(
                root,
                path,
                expected.as_str().unwrap_or(""),
                actual.trim(),
                records,
            )?;
        } else {
            require(
                actual.trim() == expected.as_str().unwrap_or(""),
                "A14 accepted source helper changed",
            )?;
        }
    }

    loopback_owned_fixture_successor::verify(
        root,
        CASES_SHA,
        cases["owned_source_sha256"]["crates/ssx-infra/src/adapters/jobs.rs"]
            .as_str()
            .unwrap_or(""),
    )?;
    let before = source_tree(root, records)?;
    verify_test_inventory(root, &cases, records)?;
    let controls = command(
        root,
        "cargo",
        &[
            "test",
            "--offline",
            "--locked",
            "-p",
            "xtask",
            "a14::",
            "--",
            "--test-threads=1",
        ],
        records,
    )?;
    execution(
        &controls,
        &BTreeSet::from([
            "a14::tests::exact_execution_rejects_missing_ignored_duplicate_and_wrong_counts".to_owned(),
            "a14::tests::completed_subcase_guard_rejects_omission_duplicate_extra_and_skip".to_owned(),
            "a14::helper_amendment::tests::original_cases_archive_current_and_unrelated_helper_mutations_are_rejected".to_owned(),
            "a14::helper_amendment::tests::equal_but_changed_timeout_reference_sections_are_rejected".to_owned(),
        ]),
    )?;
    let after = source_tree(root, records)?;
    require(
        before == after,
        "A14 source tree changed during qualification",
    )?;
    records.push(json!({"plan_id":"A14","status":"passed","source_baseline":SOURCE_BASELINE,"native_function_count":NATIVE_CASES.len(),"completed_subcase_count":SUBCASE_COUNT,"reviewed_cases_sha256":CASES_SHA,"frozen_whole_source_tree_sha256":before,"provider_calls":0,"python_executed":false,"network":"private loopback fixtures only","higher_level_contracts":cases["integration_source_tests"]}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_execution_rejects_missing_ignored_duplicate_and_wrong_counts() {
        let expected = BTreeSet::from(["module::test".to_owned()]);
        let valid = "test module::test ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
        execution(valid, &expected).unwrap();
        for invalid in [
            valid.replace("module::test", "module::other"),
            valid.replace(" ... ok", " ... ignored"),
            valid.replace("1 passed", "0 passed"),
            valid.replace("0 ignored", "1 ignored"),
            format!("test module::test ... ok\n{valid}"),
        ] {
            assert!(execution(&invalid, &expected).is_err(), "{invalid}")
        }
    }
    #[test]
    fn completed_subcase_guard_rejects_omission_duplicate_extra_and_skip() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        loopback_owned_fixture_successor::controls(&root).unwrap();
        let expected = vec![json!("a"), json!("b")];
        subcases("A14_CASE a\nA14_CASE b\n", &expected).unwrap();
        for invalid in [
            "A14_CASE a\n",
            "A14_CASE b\n",
            "A14_CASE a\nA14_CASE a\nA14_CASE b\n",
            "A14_CASE a\nA14_CASE b\nA14_CASE extra\n",
            "A14_CASE a\nignored A14_CASE b\n",
        ] {
            assert!(subcases(invalid, &expected).is_err(), "{invalid}")
        }
    }
}
