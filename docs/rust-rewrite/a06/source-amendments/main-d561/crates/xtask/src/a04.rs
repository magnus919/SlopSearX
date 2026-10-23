//! Source-bound offline replay gate for the Brave, Wikipedia, and Reddit A04 batch.
use crate::{Result, command};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};

const SOURCES: [(&str, &str); 12] = [
    (
        "engines/brave.py",
        "8c477b1172fcecd4c02c9e5e2b062767b22bcf3f",
    ),
    (
        "engines/wikipedia.py",
        "09c5402799d10426698f2bfab5bf00baacc99d36",
    ),
    (
        "engines/reddit.py",
        "f46e97fa42eb8c6971a83fb70c9274f871ac375e",
    ),
    (
        "tests/test_adapters.py",
        "788f5747f84438f96bacbec5c6d813dea44ef4cd",
    ),
    (
        "tests/test_reddit.py",
        "38783f488facecd1bac49eaba2e4205e97ea8a34",
    ),
    (
        "tests/test_media_contract.py",
        "37dd3e6e1cca7e648d7fe3f8604f697bed0481ed",
    ),
    (
        "tests/test_tier1_adapters.py",
        "453cb90bfb4a31870f4e1a7370fcd1816dcb2d3c",
    ),
    (
        "tests/test_jev.py",
        "7374e5bfc13166685b5cf5586d2020b22cabda17",
    ),
    (
        "tests/test_service.py",
        "ecfa8043b9e881b24c1bc0c5b63fbe318cc02aaa",
    ),
    (
        "tests/test_filter_semantics.py",
        "7c724c4d3367eb505f4e0f3b4ea01bc046bebccc",
    ),
    (
        "tests/test_routing_service.py",
        "83ab159f1d59f73fe46668651ddd2fc14d14e9aa",
    ),
    (
        "tests/test_misc_polish.py",
        "5844fc6fa5354efeb35f4134c505b3c3be282aa8",
    ),
];

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn ensure(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}

pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let coverage_bytes =
        fs::read(root.join("docs/rust-rewrite/a04/coverage.json")).map_err(|e| e.to_string())?;
    ensure(
        digest(&coverage_bytes)
            == "e4db69bf57868b90a0cc2c8c78d5541a80f5220f19036ccba3d6885925f6ea74",
        "A04 source-to-regression map changed; inspect and repin after regression audit",
    )?;
    let coverage: Value = serde_json::from_slice(&coverage_bytes).map_err(|e| e.to_string())?;
    ensure(
        coverage["schema_version"] == 1
            && coverage["plan_id"] == "A04"
            && coverage["baseline"] == "66b29c3f8437af45a4f0d28f7c54a0f0727f7fa5",
        "A04 coverage identity differs",
    )?;
    ensure(
        coverage["provenance"]
            .as_str()
            .is_some_and(|s| s.contains("offline") && s.contains("No live providers")),
        "A04 fixture provenance is incomplete",
    )?;
    let sources = coverage["source_observations"]
        .as_array()
        .ok_or("missing A04 source inventory")?;
    ensure(
        sources.len() == SOURCES.len(),
        "A04 source inventory is incomplete",
    )?;
    for (row, (path, pin)) in sources.iter().zip(SOURCES) {
        ensure(
            row["path"].as_str() == Some(path) && row["git_blob"].as_str() == Some(pin),
            "A04 source inventory differs from reviewed baseline",
        )?;
        // Historical adapter observations remain pinned; only the accepted
        // PR874 shared filter-contract amendment may replace current bytes.
        crate::date_amendment::source(
            root,
            path,
            "66b29c3f8437af45a4f0d28f7c54a0f0727f7fa5",
            pin,
            None,
            records,
        )?;
    }
    let cases =
        fs::read(root.join("docs/rust-rewrite/a04/cases.json")).map_err(|e| e.to_string())?;
    ensure(
        digest(&cases) == "967e50bd4ebf715767c3c240b75d03b5d6374b674a505d8e2331325d15baae70",
        "A04 captured source-byte pack changed; inspect and repin after source audit",
    )?;
    let case_value: Value = serde_json::from_slice(&cases).map_err(|e| e.to_string())?;
    ensure(
        case_value["schema_version"] == 1 && case_value["plan_id"] == "A04",
        "A04 case pack identity differs",
    )?;
    let case_records = case_value["records"]
        .as_array()
        .ok_or("A04 cases missing records")?;
    ensure(
        case_records.len() >= 30
            && case_records.iter().all(|r| {
                r["id"].as_str().is_some()
                    && r["responses"].as_array().is_some()
                    && r["output"].is_object()
            }),
        "A04 captured case pack is incomplete",
    )?;
    let matrix_path = "docs/rust-rewrite/a04/matrix.json";
    let matrix_bytes = fs::read(root.join(matrix_path)).map_err(|e| e.to_string())?;
    ensure(
        digest(&matrix_bytes) == coverage["oracle"]["matrix_sha256"].as_str().unwrap_or(""),
        "A04 edge matrix changed; inspect and repin after source audit",
    )?;
    let exporter = fs::read(root.join("docs/rust-rewrite/a04/oracle/export_matrix.py"))
        .map_err(|e| e.to_string())?;
    ensure(
        digest(&exporter) == coverage["oracle"]["exporter_sha256"].as_str().unwrap_or(""),
        "A04 source exporter changed; inspect and repin",
    )?;
    for (path, pin) in [
        (
            "docs/rust-rewrite/a04/source_context.json",
            coverage["oracle"]["original_context_sha256"]
                .as_str()
                .unwrap_or(""),
        ),
        (
            "docs/rust-rewrite/a04/oracle/export_cases.py",
            coverage["oracle"]["original_exporter_sha256"]
                .as_str()
                .unwrap_or(""),
        ),
    ] {
        let bytes = fs::read(root.join(path)).map_err(|e| e.to_string())?;
        ensure(
            digest(&bytes) == pin,
            &format!("A04 source artifact changed: {path}"),
        )?;
    }
    let matrix_value: Value = serde_json::from_slice(&matrix_bytes).map_err(|e| e.to_string())?;
    let matrix_records = matrix_value["records"]
        .as_array()
        .ok_or("A04 matrix missing records")?;
    let mapped_matrix = coverage["oracle"]["matrix_cases"]
        .as_array()
        .ok_or("A04 source matrix map missing")?;
    ensure(
        matrix_records.len() == mapped_matrix.len()
            && mapped_matrix
                .iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>()
                .len()
                == mapped_matrix.len()
            && mapped_matrix
                .iter()
                .all(|id| matrix_records.iter().any(|row| row["id"] == *id)),
        "A04 edge matrix mapping is incomplete",
    )?;
    ensure(
        coverage["oracle"]["intentional_typed_boundaries"]
            .as_array()
            .is_some_and(|a| a.len() == 4),
        "A04 typed input boundaries are missing",
    )?;

    let boundary_bytes = fs::read(root.join("docs/rust-rewrite/a04/nonfinite_boundaries.json"))
        .map_err(|e| e.to_string())?;
    ensure(
        digest(&boundary_bytes)
            == coverage["oracle"]["nonfinite_sha256"]
                .as_str()
                .unwrap_or(""),
        "A04 tagged source boundary observations changed",
    )?;
    let boundary: Value = serde_json::from_slice(&boundary_bytes).map_err(|e| e.to_string())?;
    let ids: BTreeSet<_> = boundary["records"]
        .as_array()
        .ok_or("missing tagged boundary records")?
        .iter()
        .filter_map(|r| r["id"].as_str())
        .collect();
    let expected_ids: BTreeSet<_> = coverage["oracle"]["nonfinite_cases"]
        .as_array()
        .ok_or("missing tagged boundary mapping")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    ensure(
        ids.len() == 6 && ids == expected_ids,
        "A04 tagged boundary case mapping incomplete",
    )?;

    let source_json_bytes =
        fs::read(root.join("docs/rust-rewrite/a04/source_json.json")).map_err(|e| e.to_string())?;
    ensure(
        digest(&source_json_bytes)
            == coverage["oracle"]["source_json_sha256"]
                .as_str()
                .unwrap_or_default(),
        "A04 reviewed source JSON observations changed",
    )?;
    let source_json: ssx_json::Value =
        ssx_json::from_slice(&source_json_bytes).map_err(|e| e.to_string())?;
    let source_json_rows = source_json["records"]
        .as_array()
        .ok_or("missing source JSON records")?;
    let source_json_ids: BTreeSet<_> = source_json_rows
        .iter()
        .filter_map(|r| r["id"].as_str())
        .collect();
    let expected_source_json_ids: BTreeSet<_> = coverage["oracle"]["source_json_cases"]
        .as_array()
        .ok_or("missing source JSON mapping")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    ensure(
        source_json_rows.len() == 239
            && source_json_ids.len() == 239
            && source_json_ids == expected_source_json_ids
            && source_json_rows.iter().all(|r| {
                r["source_output"].is_object()
                    && r["decoder_events"]
                        .as_array()
                        .is_some_and(|e| !e.is_empty())
                    && r["native_expectation"].is_object()
            }),
        "A04 source JSON mapping incomplete",
    )?;
    let decoder_tests = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "source_json::tests",
        ],
        records,
    )?;
    ensure(
        decoder_tests
            .lines()
            .any(|l| l.starts_with("test result:") && l.contains("4 passed; 0 failed; 0 ignored")),
        "A04 source JSON grammar, byte/depth/digit limits, and cancellation tests must pass",
    )?;
    let representation_bytes =
        fs::read(root.join("docs/rust-rewrite/a04/representation_boundaries.json"))
            .map_err(|e| e.to_string())?;
    ensure(
        digest(&representation_bytes)
            == coverage["oracle"]["representation_sha256"]
                .as_str()
                .unwrap_or(""),
        "A04 reviewed representation source observations changed",
    )?;
    let representation: ssx_json::Value =
        ssx_json::from_slice(&representation_bytes).map_err(|e| e.to_string())?;
    let representation_rows = representation["records"]
        .as_array()
        .ok_or("missing representation records")?;
    let ids: BTreeSet<_> = representation_rows
        .iter()
        .filter_map(|r| r["id"].as_str())
        .collect();
    let expected_ids: BTreeSet<_> = coverage["oracle"]["representation_cases"]
        .as_array()
        .ok_or("missing representation mapping")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    ensure(
        representation_rows.len() == 126 && ids.len() == 126 && ids == expected_ids,
        "A04 representation mapping incomplete",
    )?;

    let groups = coverage["regression_groups"]
        .as_array()
        .ok_or("missing A04 regression mapping")?;
    let mut mapped_tests = BTreeSet::new();
    let mut mapped_cases = BTreeSet::new();
    for group in groups {
        let tests = group["tests"]
            .as_array()
            .ok_or("A04 regression group missing tests")?;
        let cases = group["native_cases"].as_array();
        let dispositions = group["dispositions"].as_array();
        ensure(
            cases.is_some() || dispositions.is_some(),
            "A04 regression group must map each test to a native case or explicit boundary",
        )?;
        ensure(
            cases.map(|x| x.len()).unwrap_or(0) + dispositions.map(|x| x.len()).unwrap_or(0)
                == tests.len(),
            "A04 regression mapping is incomplete",
        )?;
        for test in tests {
            let name = test.as_str().ok_or("invalid A04 source test name")?;
            ensure(
                mapped_tests.insert(format!(
                    "{}::{name}",
                    group["source"].as_str().ok_or("invalid A04 source group")?
                )),
                "duplicate A04 source regression",
            )?;
        }
        if let Some(cases) = cases {
            for case in cases {
                let id = case.as_str().ok_or("invalid A04 case reference")?;
                ensure(
                    case_records.iter().any(|r| r["id"] == id)
                        || matrix_records.iter().any(|r| r["id"] == id),
                    "A04 regression maps to an unknown captured case",
                )?;
                mapped_cases.insert(id.to_owned());
            }
        }
    }
    ensure(
        mapped_tests.len() == 44 && mapped_cases.len() >= 20,
        "A04 named regression coverage does not meet the reviewed inventory",
    )?;
    ensure(
        coverage["shared_contracts"]
            .as_array()
            .is_some_and(|a| a.len() == 3),
        "A04 shared-contract handoffs are missing",
    )?;

    let listed = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "http::a04_broad_api::",
            "--",
            "--list",
        ],
        records,
    )?;
    let tests: BTreeSet<_> = listed
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .filter_map(|name| name.strip_prefix("http::a04_broad_api::"))
        .collect();
    let expected: BTreeSet<_> = [
        "batch_registration_alone_controls_runtime_availability",
        "replay_headers_accumulate_fragments_and_reject_early_eof_and_oversize",
        "subreddit_scopes_keep_shared_service_dispatch_cache_and_flights_separate",
        "brave_web_answers_and_images_are_parsed_from_loopback_http",
        "wikipedia_two_stages_preserve_request_order_corrections_and_infobox",
        "wikipedia_no_titles_and_missing_page_ids_match_captured_source",
        "reddit_token_precedence_auth_host_and_subreddit_routing_are_explicit",
        "reddit_default_oauth_rewrite_targets_source_host_without_network",
        "wikipedia_deadline_is_per_operation_and_session_is_reused",
        "unrepresentable_reddit_subreddit_values_are_explicit_typed_boundaries",
        "reddit_listing_preserves_source_filter_content_dates_and_thumbnail_rules",
        "reddit_empty_permalink_prefix_and_python_truthiness_follow_captured_source",
        "every_complete_source_record_replays_full_normalized_output_and_wire_contract",
        "actual_source_edge_matrix_replays_full_output_and_ordered_requests",
        "nonfinite_source_records_preserve_full_source_list_and_native_typed_refusal",
        "representation_boundaries_preserve_source_outcomes_and_refuse_whole_native_response",
        "source_json_byte_numeric_and_unicode_contracts_replay_every_record",
        "reddit_subreddit_missing_empty_and_path_special_inputs_match_source_urls",
        "all_three_classify_http_errors_bad_json_and_timeout",
        "timeout_replay_aligns_accumulated_virtual_offset_before_native_request",
        "dropped_search_cancels_outstanding_loopback_request_for_each_adapter",
        "empty_and_wrong_shape_responses_keep_engine_specific_outcomes",
        "missing_brave_credentials_fail_before_dispatch_and_reddit_401_is_unavailable",
    ]
    .into_iter()
    .collect();
    ensure(
        tests == expected,
        "A04 native replay test inventory differs from reviewed list",
    )?;
    let output = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "http::a04_broad_api::",
        ],
        records,
    )?;
    ensure(
        output
            .lines()
            .any(|l| l.starts_with("test result:") && l.contains("23 passed; 0 failed; 0 ignored")),
        "A04 replay must execute exactly twenty-three non-ignored passing tests",
    )?;

    for path in [
        "docs/rust-rewrite/a04/README.md",
        "docs/rust-rewrite/a04/cases.json",
        "docs/rust-rewrite/a04/matrix.json",
        "docs/rust-rewrite/a04/nonfinite_boundaries.json",
        "docs/rust-rewrite/a04/representation_boundaries.json",
        "docs/rust-rewrite/a04/source_json.json",
        "docs/rust-rewrite/a04/PARITY.md",
        "docs/rust-rewrite/a04/oracle/export_matrix.py",
        "docs/rust-rewrite/a04/oracle/export_cases.py",
        "docs/rust-rewrite/a04/source_context.json",
        "docs/rust-rewrite/a04/coverage.json",
        "crates/ssx-domain/src/records.rs",
        "crates/ssx-infra/src/adapters/mod.rs",
        "crates/ssx-infra/src/adapters/brave.rs",
        "crates/ssx-infra/src/adapters/wikipedia.rs",
        "crates/ssx-infra/src/adapters/reddit.rs",
        "crates/ssx-infra/src/adapters/source_json.rs",
        "crates/ssx-infra/tests/support/a04_broad_api.rs",
        "docs/rust-rewrite/a04/source-amendments/191537e-a04_broad_api.rs.frozen",
        "crates/xtask/src/a04.rs",
        "crates/xtask/src/main.rs",
        ".github/workflows/rust.yml",
        "docs/rust-rewrite/ci/gates.json",
    ] {
        let blob = command(root, "git", &["hash-object", "--", path], records)?;
        records
            .push(serde_json::json!({"a04_native_identity":{"path":path,"git_blob":blob.trim()}}));
    }
    Ok(())
}

#[cfg(test)]
mod scoped_successor_controls {
    #[test]
    fn real_nine_bindings_and_distinct_current_history_failures() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        crate::a13::causal_successor::control_bridge(&root).unwrap();
    }
}
