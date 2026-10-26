//! T02 actual-process, source-bound qualification; never invokes Python.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};
const COVERAGE_SHA: &str = "5ce1ab45a2e2006daa03ebf4a4fcd9ed62d9d59b8490ddc8f5eb82b19fc37f28";
#[path = "../../../docs/rust-rewrite/t02/source-amendments/current-main-two-source-v2.rs"]
mod source_successor;
fn require(ok: bool, why: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(why.into()) }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn load(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(path).map_err(|e| format!("{}:{e}", path.display()))?)
        .map_err(|e| e.to_string())
}
fn key(row: &Value) -> Result<String> {
    Ok(format!(
        "{}:{}",
        row["kind"].as_str().ok_or("missing completion kind")?,
        row["id"].as_str().ok_or("missing completion ID")?
    ))
}
fn completions(output: &str) -> Result<Vec<Value>> {
    output
        .lines()
        .filter_map(|l| l.strip_prefix("T02_BINDING "))
        .map(|s| serde_json::from_str(s).map_err(|e| e.to_string()))
        .collect()
}
// The source producer prefixes family completion IDs with the scenario directory,
// but keeps the raw request ID inside the receipt. Date-proof IDs are unprefixed.
fn verify_assertion_receipt_identity(
    kind: &str,
    id: &str,
    path: &Path,
    receipt: &Value,
) -> Result<()> {
    let raw_id = if kind == "family" {
        let (scenario, raw_id) = id
            .split_once(':')
            .ok_or("family completion source mapping missing")?;
        require(
            !scenario.is_empty() && !raw_id.is_empty(),
            "empty family source mapping",
        )?;
        require(
            path.parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                == Some(scenario),
            "family receipt scenario differs from source mapping",
        )?;
        raw_id
    } else {
        id
    };
    require(
        receipt["id"].as_str() == Some(raw_id),
        "assertion receipt identity differs from source mapping",
    )?;
    require(
        path.file_name().and_then(|name| name.to_str()) == Some(format!("{raw_id}.json").as_str()),
        "assertion receipt filename differs from source mapping",
    )
}
fn verify_records(actual: &[Value], expected: &[Value], read_receipts: bool) -> Result<()> {
    let expected_count = expected.len();
    let expected = expected
        .iter()
        .map(|r| Ok((key(r)?, r)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    require(
        !expected.is_empty() && expected.len() == expected_count,
        "empty or duplicate expected completion inventory",
    )?;
    let mut binaries = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for row in actual {
        let identity = key(row)?;
        require(seen.insert(identity.clone()), "duplicate native completion")?;
        let wanted = expected.get(&identity).ok_or("extra native completion")?;
        for field in ["source_record_sha256", "input", "output"] {
            if let Some(value) = wanted.get(field) {
                require(
                    row.get(field) == Some(value),
                    "native completion source/input/output binding differs",
                )?;
            }
        }
        if read_receipts
            && matches!(
                row["kind"].as_str(),
                Some("http" | "fault" | "review" | "html" | "family" | "date-proof")
            )
        {
            require(
                row["receipt"].is_string() && row["receipt_sha256"].is_string(),
                "wire completion receipt missing",
            )?;
            let path = Path::new(row["receipt"].as_str().ok_or("receipt path is not text")?);
            let raw = fs::read(path).map_err(|e| format!("{}:{e}", path.display()))?;
            require(
                row["receipt_sha256"] == digest(&raw),
                "actual wire receipt digest differs",
            )?;
            let receipt: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
            let start = load(
                &path
                    .parent()
                    .ok_or("receipt parent missing")?
                    .join("process-start.json"),
            )?;
            require(
                start["pid"].as_u64().is_some_and(|pid| pid > 0)
                    && start["startup"]["mode"] == "offline-t02-fixture",
                "actual owned server process identity missing",
            )?;
            let binary = start["binary"]
                .as_str()
                .ok_or("server executable missing")?;
            if !binaries.contains_key(binary) {
                binaries.insert(
                    binary.to_owned(),
                    digest(&fs::read(binary).map_err(|e| e.to_string())?),
                );
            }
            require(
                start["binary_sha256"] == binaries[binary],
                "executed server binary digest differs",
            )?;
            for field in ["status", "engine_call_delta", "engine_calls"] {
                if let Some(value) = wanted.get(field) {
                    require(
                        receipt.get(field) == Some(value),
                        "actual source-bound status/dispatch differs",
                    )?;
                }
            }

            match row["kind"].as_str().unwrap() {
                "http" | "fault" | "review" => {
                    require(
                        receipt["id"] == row["id"]
                            && receipt["differences"].as_array().is_some_and(Vec::is_empty),
                        "actual wire differences/missing identity",
                    )?;
                    require(
                        receipt["body"].is_string() && receipt["headers"].is_object(),
                        "whole wire body/headers missing",
                    )?;
                    for field in ["status", "engine_call_delta", "engine_calls"] {
                        if let Some(value) = wanted.get(field) {
                            require(
                                receipt.get(field) == Some(value),
                                "actual wire status/dispatch differs",
                            )?;
                        }
                    }
                }
                "html" => {
                    require(
                        receipt["requests_completed"] == 5 && receipt["child_exit_success"] == true,
                        "HTML literal requests or process cleanup missing",
                    )?;
                    let raw = fs::read(
                        receipt["raw_http_records"]
                            .as_str()
                            .ok_or("HTML raw stream path missing")?,
                    )
                    .map_err(|e| e.to_string())?;
                    require(
                        receipt["raw_http_records_sha256"] == digest(&raw),
                        "HTML whole wire stream differs",
                    )?;
                    let text = String::from_utf8(raw).map_err(|e| e.to_string())?;
                    let rows = text
                        .lines()
                        .map(|line| serde_json::from_str::<Value>(line).map_err(|e| e.to_string()))
                        .collect::<Result<Vec<_>>>()?;
                    require(
                        rows.len() == 5
                            && rows
                                .iter()
                                .all(|r| r["body"].is_string() && r["headers"].is_object()),
                        "HTML source-owned wire bodies/headers missing",
                    )?;
                }
                "family" | "date-proof" => {
                    verify_assertion_receipt_identity(
                        row["kind"].as_str().unwrap(),
                        wanted["id"]
                            .as_str()
                            .ok_or("source completion ID missing")?,
                        path,
                        &receipt,
                    )?;
                    require(
                        receipt["body"].is_string()
                            && receipt["status"].as_u64().is_some_and(|s| s >= 200),
                        "actual assertion-family response missing",
                    )?;
                }
                _ => return Err("unknown receipt kind".into()),
            }
        }
    }
    require(
        seen == expected.keys().cloned().collect(),
        "missing native completion",
    )
}
fn archive_wire_records(rows: &[Value]) -> Result<Vec<Value>> {
    rows.iter().filter(|r|r.get("receipt").is_some()).map(|binding|{let path=Path::new(binding["receipt"].as_str().ok_or("archive receipt path missing")?);let raw=fs::read(path).map_err(|e|e.to_string())?;let receipt:Value=serde_json::from_slice(&raw).map_err(|e|e.to_string())?;let process=load(&path.parent().ok_or("archive parent missing")?.join("process-start.json"))?;let html=if binding["kind"]=="html"{Some(fs::read_to_string(receipt["raw_http_records"].as_str().ok_or("HTML raw stream missing")?).map_err(|e|e.to_string())?)}else{None};Ok(json!({"binding":binding,"raw_receipt_utf8":String::from_utf8(raw).map_err(|e|e.to_string())?,"process":process,"raw_html_stream_utf8":html}))}).collect()
}
fn exact_tests(output: &str, expected: &Value) -> Result<()> {
    let mut tests = Vec::new();
    for line in output.lines().filter_map(|line| line.strip_prefix("test ")) {
        if let Some((name, status)) = line.rsplit_once(" ... ") {
            require(
                status == "ok",
                "native test missing, duplicate, extra or ignored",
            )?;
            tests.push(name.to_owned());
        } else {
            require(line.starts_with("result:"), "malformed native test result")?;
        }
    }
    let names = tests.iter().cloned().collect::<BTreeSet<_>>();
    let wanted = expected
        .as_array()
        .ok_or("missing native test list")?
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or_else(|| "invalid native test name".to_owned())
                .map(str::to_owned)
        })
        .collect::<Result<BTreeSet<_>>>()?;
    require(
        names == wanted && tests.len() == names.len(),
        "native test missing, duplicate, extra or ignored",
    )
}
const PAGE_HTML_TEST: &str =
    "lossless_large_page_html_links_preserve_filters_and_exact_decimal_neighbors";
const YAML_SCALAR_TEST: &str = "successful_yaml_break_scalars_match_pinned_emitter_on_actual_http";
const YAML_SCALAR_REFERENCE: &str = "docs/rust-rewrite/t02/yaml-break-source-controls-v1.json";
const YAML_SCALAR_REFERENCE_SHA: &str =
    "5e00f07a6b613a8d2bca60010dac90499cc1d09698a764d3c466abcc4223051c";
const FROZEN_HTTP_TESTS: [&str; 8] = [
    "original_literal_corpus_through_a_real_native_http_process",
    "original_fault_bodies_and_private_valkey_cache_match_real_http",
    "original_owned_html_assertions_use_real_native_renderer",
    "original_assertion_families_use_runtime_policy_config_and_public_discovery",
    "absolute_date_proof_and_forwarding_reach_actual_http_without_claiming_a06",
    "reviewed_offsets_category_labels_and_duplicate_queries_match_original_http",
    "unobserved_metric_outcomes_require_joint_complete_absence",
    "yaml_invalid_date_field_escapes_nonprintable_unicode_before_dispatch",
];
fn current_tests(frozen: &Value, family: &str) -> Result<Value> {
    let mut expected = frozen.clone();
    match family {
        "http" => {
            require(
                *frozen == json!(FROZEN_HTTP_TESTS),
                "T02 frozen eight HTTP tests differ",
            )?;
            expected
                .as_array_mut()
                .ok_or("HTTP test inventory missing")?
                .extend([json!(YAML_SCALAR_TEST), json!(PAGE_HTML_TEST)]);
        }
        "formatter" | "library" => {}
        _ => return Err("unknown T02 native test family".into()),
    }
    Ok(expected)
}
fn yaml_scalar_roots() -> Result<BTreeSet<PathBuf>> {
    let paths = if let Some(base) = std::env::var_os("T02_HTTP_RECEIPTS") {
        let p = PathBuf::from(base).join("yaml-break-scalars");
        if p.exists() { vec![p] } else { vec![] }
    } else {
        fs::read_dir(std::env::temp_dir())
            .map_err(|e| e.to_string())?
            .map(|entry| entry.map(|e| e.path()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.starts_with("t02-http-yaml-break-scalars-"))
            })
            .collect()
    };
    for path in &paths {
        let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        require(
            m.is_dir() && !m.file_type().is_symlink(),
            "YAML scalar receipt root is not an owned directory",
        )?;
    }
    Ok(paths.into_iter().collect())
}
fn yaml_scalar_cases(
    reference: &[Value],
    wires: &[Value],
    calls: &[Value],
    terminal: &Value,
) -> Result<()> {
    require(
        reference.len() == 18 && wires.len() == 18 && calls.len() == 18,
        "YAML scalar eighteen-case inventory differs",
    )?;
    require(
        terminal["expected"] == 18
            && terminal["completed"] == 18
            && terminal["engine_calls"] == 18
            && terminal["child_joined"] == true
            && terminal["differences"]
                .as_array()
                .is_some_and(Vec::is_empty),
        "YAML scalar completion or owned-child join differs",
    )?;
    for ((case, wire), call) in reference.iter().zip(wires).zip(calls) {
        require(
            wire["id"] == case["id"]
                && wire["status"] == 200
                && wire["media"] == "text/vnd.yaml+markdown; charset=utf-8"
                && wire["mismatches"].as_array().is_some_and(Vec::is_empty)
                && call["query"] == case["query"],
            "YAML scalar source case/status/dispatch differs",
        )?;
        let body = wire["body"].as_str().ok_or("YAML scalar body missing")?;
        let document = body
            .split_once("---\n## Results Summary\n")
            .ok_or("YAML scalar document separator missing")?
            .0;
        let fields = case["fragments"]
            .as_object()
            .ok_or("YAML scalar reference fields missing")?;
        require(
            fields.len() == 4
                && fields
                    .values()
                    .all(|v| v.as_str().is_some_and(|v| document.contains(v))),
            "YAML scalar measured field bytes differ",
        )?;
    }
    Ok(())
}
fn yaml_scalar_witness(root: &Path, before: &BTreeSet<PathBuf>) -> Result<Value> {
    let after = yaml_scalar_roots()?;
    let added = after.difference(before).collect::<Vec<_>>();
    require(
        added.len() == 1,
        "new YAML scalar receipt root missing or duplicated",
    )?;
    let directory = added[0];
    let read = |name: &str| -> Result<Vec<u8>> {
        let path = directory.join(name);
        let m = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        require(
            m.is_file() && !m.file_type().is_symlink(),
            "YAML scalar receipt is not regular",
        )?;
        fs::read(path).map_err(|e| e.to_string())
    };
    let raw_reference = fs::read(root.join(YAML_SCALAR_REFERENCE)).map_err(|e| e.to_string())?;
    require(
        digest(&raw_reference) == YAML_SCALAR_REFERENCE_SHA,
        "YAML scalar reference identity differs",
    )?;
    let reference: Value = serde_json::from_slice(&raw_reference).map_err(|e| e.to_string())?;
    let reference = reference["records"]
        .as_array()
        .ok_or("YAML scalar reference cases missing")?;
    let mut wires = Vec::new();
    let mut archived = Vec::new();
    let mut expected = BTreeSet::from([
        "process-start.json".to_owned(),
        "engine-calls.jsonl".to_owned(),
        "server-stderr.log".to_owned(),
        "terminal.json".to_owned(),
    ]);
    for index in 0..18 {
        let name = format!("{index:02}.json");
        let raw = read(&name)?;
        let wire: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        archived.push(json!({"id":wire["id"],"receipt":directory.join(&name),"receipt_sha256":digest(&raw),"raw_receipt_utf8":String::from_utf8(raw).map_err(|e|e.to_string())?}));
        wires.push(wire);
        expected.insert(name);
    }
    let names = fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .map(|p| {
            p.map(|p| p.file_name().to_string_lossy().into_owned())
                .map_err(|e| e.to_string())
        })
        .collect::<Result<BTreeSet<_>>>()?;
    require(
        names == expected,
        "YAML scalar receipt file inventory differs",
    )?;
    let terminal_raw = read("terminal.json")?;
    let terminal: Value = serde_json::from_slice(&terminal_raw).map_err(|e| e.to_string())?;
    let calls_raw = read("engine-calls.jsonl")?;
    let calls = std::str::from_utf8(&calls_raw)
        .map_err(|e| e.to_string())?
        .lines()
        .map(|l| serde_json::from_str(l).map_err(|e| e.to_string()))
        .collect::<Result<Vec<Value>>>()?;
    yaml_scalar_cases(reference, &wires, &calls, &terminal)?;
    let start_raw = read("process-start.json")?;
    let start: Value = serde_json::from_slice(&start_raw).map_err(|e| e.to_string())?;
    require(
        start["pid"].as_u64().is_some_and(|p| p > 0)
            && start["startup"]["mode"] == "offline-t02-fixture",
        "YAML scalar actual process identity missing",
    )?;
    let binary = Path::new(
        start["binary"]
            .as_str()
            .ok_or("YAML scalar executable missing")?,
    );
    let m = fs::symlink_metadata(binary).map_err(|e| e.to_string())?;
    require(
        m.is_file()
            && !m.file_type().is_symlink()
            && start["binary_sha256"] == digest(&fs::read(binary).map_err(|e| e.to_string())?),
        "YAML scalar executed binary identity differs",
    )?;
    Ok(
        json!({"T02_completed_yaml_scalar_records":18,"scope":"current additive real HTTP scalar witness; separate from frozen421 bindings","native_test":YAML_SCALAR_TEST,"reference_sha256":YAML_SCALAR_REFERENCE_SHA,"receipt_root":directory,"process":start,"terminal":terminal,"terminal_sha256":digest(&terminal_raw),"engine_calls_sha256":digest(&calls_raw),"records":archived}),
    )
}
fn original_execution(root: &Path, coverage: &Value) -> Result<()> {
    let original = load(
        &root.join(
            coverage["original_execution"]
                .as_str()
                .ok_or("missing original execution")?,
        ),
    )?;
    let nodes = original["collected"]
        .as_array()
        .ok_or("missing original collected nodes")?;
    let wanted = coverage["original_nodes"]
        .as_array()
        .ok_or("missing pinned original node list")?;
    require(
        nodes == wanted
            && nodes.len() == 162
            && nodes
                .iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>()
                .len()
                == 162,
        "original collected node identities differ",
    )?;
    require(
        original["terminal"]["exit_code"] == 0
            && original["terminal"]["failed_phases"] == 0
            && original["terminal"]["skipped_phases"] == 0
            && original["terminal"]["source_unchanged"] == true,
        "original source execution failed",
    )?;
    let reports = original["events"]
        .as_array()
        .ok_or("original phase events missing")?;
    for phase in ["started", "setup", "call", "teardown"] {
        let actual = reports
            .iter()
            .filter(|r| r["phase"] == phase)
            .collect::<Vec<_>>();
        let identities = actual
            .iter()
            .filter_map(|r| r["nodeid"].as_str())
            .collect::<BTreeSet<_>>();
        require(
            actual.len() == 162
                && identities == nodes.iter().filter_map(Value::as_str).collect()
                && actual
                    .iter()
                    .all(|r| phase == "started" || r["outcome"] == "passed"),
            "original started/setup/call/teardown missing, duplicate, skipped or failed",
        )?;
    }
    let bindings = load(
        &root.join(
            coverage["original_bindings"]
                .as_str()
                .ok_or("missing original assertion binding map")?,
        ),
    )?;
    let rows = bindings["bindings"]
        .as_array()
        .ok_or("missing assertion binding rows")?;
    require(
        rows.len() == 162
            && rows
                .iter()
                .filter_map(|r| r["source_node"].as_str())
                .collect::<BTreeSet<_>>()
                == nodes.iter().filter_map(Value::as_str).collect(),
        "original assertion node mapping incomplete",
    )?;
    require(
        rows.iter().all(|r| {
            r["assertions"].as_array().is_some_and(|a| !a.is_empty())
                && r["owner"].as_str().is_some_and(|s| !s.is_empty())
                && r["native_evidence"]
                    .as_array()
                    .is_some_and(|a| !a.is_empty())
        }),
        "empty original assertion ownership/evidence binding",
    )
}
pub fn check(root: &Path, commands: &mut Vec<Value>) -> Result<()> {
    let bytes =
        fs::read(root.join("docs/rust-rewrite/t02/coverage.json")).map_err(|e| e.to_string())?;
    require(
        digest(&bytes) == COVERAGE_SHA,
        "T02 coverage digest differs",
    )?;
    let coverage: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    require(coverage["plan_id"] == "T02", "T02 plan identity differs")?;
    source_successor::controls(root)?;
    for artifact in coverage["artifacts"]
        .as_array()
        .ok_or("T02 artifact list missing")?
    {
        let path = artifact["path"]
            .as_str()
            .ok_or("T02 artifact path missing")?;
        let pin = artifact["sha256"]
            .as_str()
            .ok_or("T02 artifact digest missing")?;
        let page_prior = source_successor::page_original(root, path)?;
        if let Some(proof) = source_successor::verify(root, path, pin, COVERAGE_SHA) {
            proof?;
        } else {
            require(
                artifact["sha256"]
                    == digest(&match page_prior {
                        Some(prior) => prior,
                        None => fs::read(root.join(path)).map_err(|e| format!("{path}:{e}"))?,
                    }),
                &format!("T02 artifact differs:{path}"),
            )?;
        }
    }
    let chunks = load(&root.join("docs/rust-rewrite/t02/oracle/source-http-chunks.json"))?;
    let mut source_bytes = Vec::new();
    for part in chunks["parts"]
        .as_array()
        .ok_or("source HTTP chunk inventory missing")?
    {
        let path = part["path"].as_str().ok_or("source chunk path missing")?;
        let bytes = fs::read(root.join("docs/rust-rewrite/t02/oracle").join(path))
            .map_err(|e| e.to_string())?;
        require(
            part["sha256"] == digest(&bytes),
            "source HTTP chunk differs",
        )?;
        source_bytes.extend(bytes);
    }
    require(
        chunks["original_sha256"] == digest(&source_bytes)
            && source_bytes.len() as u64
                == chunks["original_bytes"]
                    .as_u64()
                    .ok_or("original byte count missing")?
            && String::from_utf8_lossy(&source_bytes).lines().count() == 119,
        "original119 source byte/record binding differs",
    )?;
    original_execution(root, &coverage)?;
    for (path, pin) in coverage["original_source_sha256"]
        .as_object()
        .ok_or("original source pins missing")?
    {
        require(
            *pin == digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?),
            &format!("original source drift:{path}"),
        )?;
    }
    let expected = coverage["native_records"]
        .as_array()
        .ok_or("native completion inventory missing")?;
    require(
        expected.len() == 421,
        "native completion inventory count differs",
    )?;
    // Independent verifier controls reject missing, duplicate, extra and changed source bindings.
    let controls = vec![
        json!({"kind":"control","id":"a","source_record_sha256":"source-a"}),
        json!({"kind":"control","id":"b","source_record_sha256":"source-b"}),
    ];
    verify_records(&controls, &controls, false)?;
    require(
        verify_records(&controls[..1], &controls, false).is_err(),
        "missing completion accepted",
    )?;
    let mut duplicate = controls.clone();
    duplicate.push(controls[0].clone());
    require(
        verify_records(&duplicate, &controls, false).is_err(),
        "duplicate completion accepted",
    )?;
    let mut extra = controls.clone();
    extra.push(json!({"kind":"control","id":"extra","source_record_sha256":"source-extra"}));
    require(
        verify_records(&extra, &controls, false).is_err(),
        "extra completion accepted",
    )?;
    let mut changed = controls.clone();
    changed[0]["source_record_sha256"] = json!("wrong");
    require(
        verify_records(&changed, &controls, false).is_err(),
        "changed source binding accepted",
    )?;
    command(
        root,
        "cargo",
        &[
            "build",
            "--locked",
            "-p",
            "ssx-http",
            "--example",
            "t02_fixture",
        ],
        commands,
    )?;
    let mut completed = Vec::new();
    for (name, args) in [
        (
            "http",
            vec![
                "test",
                "--locked",
                "-p",
                "ssx-http",
                "--test",
                "t02_http",
                "--",
                "--show-output",
                "--test-threads=1",
            ],
        ),
        (
            "formatter",
            vec![
                "test",
                "--locked",
                "-p",
                "ssx-http",
                "--test",
                "t02_formatter",
                "--",
                "--show-output",
            ],
        ),
        (
            "library",
            vec![
                "test",
                "--locked",
                "-p",
                "ssx-http",
                "--lib",
                "native::source_grammar_tests",
                "--",
                "--show-output",
            ],
        ),
    ] {
        let expected_tests = current_tests(&coverage["native_tests"][name], name)?;
        let before = if name == "http" {
            Some(yaml_scalar_roots()?)
        } else {
            None
        };
        let output = command(root, "cargo", &args, commands)?;
        exact_tests(&output, &expected_tests)?;
        if let Some(before) = before {
            commands.push(yaml_scalar_witness(root, &before)?);
        }
        completed.extend(completions(&output)?);
    }
    let mut current_expected = expected.clone();
    current_expected.push(json!({"kind":"html","id":"lossless-pages","source_record_sha256":null}));
    verify_records(&completed, &current_expected, true)?;
    commands.push(json!({"T02_actual_wire_receipts":archive_wire_records(&completed)?}));
    // Actually omit one pinned source case in a fresh server run. The test must fail and clean up.
    let skipped = coverage["skip_control_id"]
        .as_str()
        .ok_or("skip control source ID missing")?;
    let args = [
        "test",
        "--locked",
        "-p",
        "ssx-http",
        "--test",
        "t02_http",
        "original_literal_corpus_through_a_real_native_http_process",
        "--",
        "--show-output",
        "--test-threads=1",
    ];
    let result = Command::new("cargo")
        .args(args)
        .env("T02_SKIP_SOURCE_CASE", skipped)
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&result.stdout);
    commands.push(json!({"program":"cargo","args":args,"environment":{"T02_SKIP_SOURCE_CASE":skipped},"exit_code":result.status.code(),"stdout":stdout,"stderr":String::from_utf8_lossy(&result.stderr)}));
    require(
        !result.status.success()
            && stdout.contains(&format!("T02_INTENTIONAL_SKIP {skipped}"))
            && stdout.contains("\"graceful_signal_and_child_exit_success\":true"),
        "actual omitted-case control failed to reject or clean up",
    )?;
    let negative_rows = completions(&stdout)?;
    let wanted = expected
        .iter()
        .filter(|r| r["kind"] == "http" && r["id"] != skipped)
        .cloned()
        .collect::<Vec<_>>();
    require(
        wanted.len() == 269,
        "negative control expected source identities differ",
    )?;
    verify_records(&negative_rows, &wanted, true)?;
    commands
        .push(json!({"T02_omitted_control_wire_receipts":archive_wire_records(&negative_rows)?}));
    require(
        stdout.contains("\"completed\":269") && stdout.contains("\"failures\":[]"),
        "omitted-case control had unrelated failures",
    )?;
    commands.push(json!({"T02_completed_native_records":completed.len(),"original_nodes_each_phase":162,"native_skip_control":"rejected and server joined","scope":"offline native boundary; image execution is separate CI; no production assembly or #984 closure"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_duplicate_extra_and_source_drift_fail_closed() {
        source_successor::controls(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
        // Current discovery retains the frozen eight, the reviewed YAML test, and the additive lossless-page control.
        let frozen = json!(FROZEN_HTTP_TESTS);
        let current = current_tests(&frozen, "http").unwrap();
        let names = current.as_array().unwrap();
        assert_eq!(names.len(), 10);
        let output = names
            .iter()
            .map(|n| format!("test {} ... ok\n", n.as_str().unwrap()))
            .collect::<String>();
        assert!(exact_tests(&output, &current).is_ok());
        for name in names {
            let line = format!("test {} ... ok\n", name.as_str().unwrap());
            assert!(exact_tests(&output.replace(&line, ""), &current).is_err());
            assert!(exact_tests(&format!("{output}{line}"), &current).is_err());
            assert!(
                exact_tests(
                    &output.replace(&line, &line.replace(" ... ok", " ... ignored")),
                    &current
                )
                .is_err()
            );
        }
        for status in ["ignored", "ignored, reason", "FAILED", "unexpected"] {
            for name in names {
                let line = format!("test {} ... ok\n", name.as_str().unwrap());
                let ignored = format!("test {} ... {status}\n", name.as_str().unwrap());
                assert!(exact_tests(&output.replace(&line, &ignored), &current).is_err());
                assert!(exact_tests(&format!("{output}{ignored}"), &current).is_err());
            }
            assert!(
                exact_tests(&format!("{output}test unlisted ... {status}\n"), &current).is_err()
            );
        }
        assert!(exact_tests(&format!("{output}test unlisted ... ok\n"), &current).is_err());
        assert!(exact_tests(&format!("{output}test malformed\n"), &current).is_err());
        assert!(exact_tests(&format!("{output}test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"), &current).is_ok());
        assert!(current_tests(&json!([]), "http").is_err());
        let mut wrong = frozen.clone();
        wrong[0] = json!(YAML_SCALAR_TEST);
        assert!(current_tests(&wrong, "http").is_err());
        // Synthetic verifier controls; real HTTP acceptance still requires the subprocess receipts.
        let reference: Value = serde_json::from_str(include_str!(
            "../../../docs/rust-rewrite/t02/yaml-break-source-controls-v1.json"
        ))
        .unwrap();
        let reference = reference["records"].as_array().unwrap();
        let wires=reference.iter().map(|case|json!({"id":case["id"],"status":200,"media":"text/vnd.yaml+markdown; charset=utf-8","mismatches":[],"body":format!("{}---\n## Results Summary\n",case["fragments"].as_object().unwrap().values().map(|v|v.as_str().unwrap()).collect::<String>())})).collect::<Vec<_>>();
        let calls = reference
            .iter()
            .map(|case| json!({"query":case["query"]}))
            .collect::<Vec<_>>();
        let terminal = json!({"expected":18,"completed":18,"engine_calls":18,"child_joined":true,"differences":[]});
        assert!(yaml_scalar_cases(reference, &wires, &calls, &terminal).is_ok());
        for case in 0..12 {
            let mut bad = wires.clone();
            let mut bad_calls = calls.clone();
            let mut bad_terminal = terminal.clone();
            match case {
                0 => {
                    bad.pop();
                }
                1 => bad.push(bad[0].clone()),
                2 => bad.swap(0, 1),
                3 => bad[0]["id"] = json!("unlisted"),
                4 => bad[0]["mismatches"] = json!(["query"]),
                5 => bad[0]["body"] = json!("query: wrong\n---\n## Results Summary\n"),
                6 => bad[0]["status"] = json!(503),
                7 => bad_calls[0]["query"] = json!("wrong"),
                8 => bad_terminal["child_joined"] = json!(false),
                9 => bad_terminal["engine_calls"] = json!(17),
                10 => bad_terminal["completed"] = json!(17),
                _ => bad_terminal["differences"] = json!(["query"]),
            }
            assert!(yaml_scalar_cases(reference, &bad, &bad_calls, &bad_terminal).is_err());
        }
        let expected = vec![json!({"kind":"http","id":"a","source_record_sha256":"a"})];
        assert!(verify_records(&expected, &expected, false).is_ok());
        assert!(verify_records(&[], &expected, false).is_err());
        let mut duplicate = expected.clone();
        duplicate.push(expected[0].clone());
        assert!(verify_records(&duplicate, &expected, false).is_err());
        let mut extra = expected.clone();
        extra[0]["id"] = json!("extra");
        assert!(verify_records(&extra, &expected, false).is_err());
        let mut drift = expected.clone();
        drift[0]["source_record_sha256"] = json!("changed");
        assert!(verify_records(&drift, &expected, false).is_err());
    }
}

#[cfg(test)]
mod receipt_tests {
    use super::*;
    #[test]
    fn assertion_receipt_identity_rejects_swaps_and_preserves_family_prefix() {
        let family = json!({"id":"health"});
        assert!(
            verify_assertion_receipt_identity(
                "family",
                "mock3:health",
                Path::new("/owned/mock3/health.json"),
                &family
            )
            .is_ok()
        );
        // Same status/body/dispatch do not make another scenario/request equivalent.
        for (id, path, receipt) in [
            ("mock3:health", "/owned/grants/health.json", family.clone()),
            (
                "mock3:health",
                "/owned/mock3/other.json",
                json!({"id":"other"}),
            ),
            (
                "mock3:health",
                "/owned/mock3/health.json",
                json!({"id":"other"}),
            ),
            ("mock3:health", "/owned/mock3/other.json", family.clone()),
            ("health", "/owned/mock3/health.json", family.clone()),
            ("mock3:", "/owned/mock3/.json", json!({"id":""})),
        ] {
            assert!(
                verify_assertion_receipt_identity("family", id, Path::new(path), &receipt).is_err()
            );
        }
        let proof = json!({"id":"date-proof-00"});
        assert!(
            verify_assertion_receipt_identity(
                "date-proof",
                "date-proof-00",
                Path::new("/owned/date-proof/date-proof-00.json"),
                &proof
            )
            .is_ok()
        );
        assert!(
            verify_assertion_receipt_identity(
                "date-proof",
                "date-proof-01",
                Path::new("/owned/date-proof/date-proof-00.json"),
                &proof
            )
            .is_err()
        );
        assert!(
            verify_assertion_receipt_identity(
                "date-proof",
                "date-proof-00",
                Path::new("/owned/date-proof/date-proof-01.json"),
                &proof
            )
            .is_err()
        );
    }
    #[test]
    fn missing_wire_receipt_fails_closed() {
        for kind in ["http", "fault", "review", "html", "family", "date-proof"] {
            let rows = vec![json!({"kind":kind,"id":"required","source_record_sha256":"source"})];
            assert!(
                verify_records(&rows, &rows, true).is_err(),
                "{kind} accepted an absent actual receipt"
            );
        }
        for kind in ["formatter", "accept", "formatter-review"] {
            let rows = vec![json!({"kind":kind,"id":"pure","input":"x","output":"y"})];
            assert!(verify_records(&rows, &rows, true).is_ok());
        }
    }
}
