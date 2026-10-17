//! T02 actual-process, source-bound qualification; never invokes Python.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};
const COVERAGE_SHA: &str = "261ace11e64bd4a1efdf06bc6fce3500cebfd3cb868581ae75a49cc87568560f";
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
                "family" | "date-proof" => require(
                    receipt["body"].is_string()
                        && receipt["status"].as_u64().is_some_and(|s| s >= 200),
                    "actual assertion-family response missing",
                )?,
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
    let tests = output
        .lines()
        .filter_map(|l| l.strip_prefix("test ")?.strip_suffix(" ... ok"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
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
        names == wanted
            && tests.len() == names.len()
            && !output
                .lines()
                .any(|l| l.starts_with("test ") && l.ends_with(" ... ignored")),
        "native test missing, duplicate, extra or ignored",
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
    for artifact in coverage["artifacts"]
        .as_array()
        .ok_or("T02 artifact list missing")?
    {
        let path = artifact["path"]
            .as_str()
            .ok_or("T02 artifact path missing")?;
        require(
            artifact["sha256"]
                == digest(&fs::read(root.join(path)).map_err(|e| format!("{path}:{e}"))?),
            &format!("T02 artifact differs:{path}"),
        )?;
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
        let output = command(root, "cargo", &args, commands)?;
        exact_tests(&output, &coverage["native_tests"][name])?;
        completed.extend(completions(&output)?);
    }
    verify_records(&completed, expected, true)?;
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
