//! Source-bound A09 engine qualification. No interpreter or provider invocation.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
const COVERAGE_SHA: &str = "f9b049e85c64f147351a8b36df69571fe9799010dd6b89a6b2dd7d5d6322b20f";
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn read(root: &Path, path: &str) -> Result<Value> {
    serde_json::from_slice(&fs::read(root.join(path)).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn ids(pack: &Value) -> Result<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut ids = vec![];
    for row in pack["records"]
        .as_array()
        .ok_or("A09 missing source records")?
    {
        let id = row["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("A09 empty source ID")?;
        require(seen.insert(id.to_owned()), "A09 duplicate source ID")?;
        require(
            row["responses"].is_array() && row["requests"].is_array() && row["output"].is_object(),
            "A09 source request/outcome tuple missing",
        )?;
        ids.push(id.into())
    }
    require(!ids.is_empty(), "A09 empty source pack")?;
    Ok(ids)
}
fn execution(output: &str, expected: &BTreeSet<String>) -> Result<()> {
    let passed: BTreeSet<_> = output
        .lines()
        .filter_map(|s| {
            s.strip_prefix("test ")?
                .strip_suffix(" ... ok")
                .map(str::to_owned)
        })
        .collect();
    require(
        passed == *expected
            && !output
                .lines()
                .any(|s| s.starts_with("test ") && s.ends_with(" ... ignored")),
        "A09 listed tests missing, ignored, or not passed",
    )
}
fn completed_cases(output: &str, expected: &[Value]) -> Result<()> {
    let actual = output
        .lines()
        .filter_map(|line| line.strip_prefix("A09_CASE "))
        .map(|line| serde_json::from_str::<Value>(line).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>>>()?;
    require(
        actual == expected,
        "A09 completed case IDs/kinds missing, extra, duplicate, or reordered",
    )
}
fn completion_set(output: &str, prefix: &str, expected: &[String]) -> Result<()> {
    let actual = output
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(|line| serde_json::from_str::<String>(line).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>>>()?;
    let set: BTreeSet<_> = actual.iter().cloned().collect();
    require(
        actual.len() == set.len()
            && actual.len() == expected.len()
            && set == expected.iter().cloned().collect(),
        "A09 owned/timer completion IDs missing, extra, duplicate or invalid JSON string",
    )
}
fn binding(value: &Value, available: &BTreeMap<(String, String), BTreeSet<String>>) -> Result<()> {
    let package = value["package"].as_str().ok_or("A09 package missing")?;
    let target = value["target"].as_str().ok_or("A09 target missing")?;
    let test = value["qualified_name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("A09 test name missing")?;
    require(
        available
            .get(&(package.into(), target.into()))
            .is_some_and(|names| names.contains(test)),
        "A09 exact native package/target/test identity missing",
    )
}
fn mapping_semantics(coverage: &Value, regression: &Value) -> Result<()> {
    let original = regression["results"]
        .as_array()
        .ok_or("A09 original test results missing")?;
    require(
        original.len() == 56 && original.iter().all(|r| r["outcome"] == "passed"),
        "A09 original exact 56 parameter results not passed",
    )?;
    let functions: BTreeSet<_> = original
        .iter()
        .filter_map(|r| {
            r["node"]
                .as_str()
                .map(|n| n.split('[').next().unwrap_or(""))
        })
        .collect();
    let mappings = coverage["regression_mapping"]
        .as_array()
        .ok_or("A09 original mapping missing")?;
    require(
        functions.len() == 51
            && functions == mappings.iter().filter_map(|r| r["node"].as_str()).collect(),
        "A09 original 51 function set differs",
    )?;
    let cases = regression["records"]
        .as_array()
        .ok_or("A09 original actual adapter invocations missing")?;
    for mapping in mappings {
        let node = mapping["node"].as_str().ok_or("A09 mapping node missing")?;
        let observed: Vec<_> = cases
            .iter()
            .filter(|c| {
                c["node"]
                    .as_str()
                    .is_some_and(|n| n.split('[').next() == Some(node))
            })
            .map(|c| c["id"].clone())
            .collect();
        require(
            mapping["source_case_ids"] == json!(observed),
            "A09 mapping is not bound to actual exact original invocation IDs",
        )?;
        if observed.is_empty() {
            require(
                node.ends_with("::test_adapter_registered")
                    || node.ends_with("::test_adapter_categories_excludes_general")
                    || node.ends_with("::test_adapter_default_disabled"),
                "A09 behavioral function mislabeled registration-only",
            )?;
            require(
                mapping["kind"] == "registration-metadata-default",
                "A09 source registration node must map actual catalog/default assertion",
            )?;
        } else {
            require(
                mapping["kind"] == "actual-source-adapter-replay",
                "A09 behavior node requires actual source adapter case",
            )?;
        }
        require(
            mapping["native_cases"]
                .as_array()
                .is_some_and(|r| !r.is_empty()),
            "A09 native case binding empty",
        )?;
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let bytes =
        fs::read(root.join("docs/rust-rewrite/a09/coverage.json")).map_err(|e| e.to_string())?;
    require(
        digest(&bytes) == COVERAGE_SHA,
        "A09 coverage changed; audit and repin",
    )?;
    let coverage: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    require(
        coverage["baseline"] == "9fae1639a57b9e3497c04c3f5fe304651d077ad5"
            && coverage["plan_id"] == "A09",
        "A09 source baseline/plan drift",
    )?;
    for kind in ["source_inventory", "artifacts"] {
        for pin in coverage[kind]
            .as_array()
            .ok_or("A09 artifact pins missing")?
        {
            let path = pin["path"].as_str().ok_or("A09 pin path missing")?;
            require(
                pin["sha256"]
                    == digest(&fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))?),
                &format!("A09 source/artifact identity differs: {path}"),
            )?;
        }
    }
    let source = read(root, "docs/rust-rewrite/a09/source.json")?;
    let regression = read(root, "docs/rust-rewrite/a09/regressions.json")?;
    let mut ordered = ids(&source)?;
    ordered.extend(ids(&regression)?);
    require(
        ordered.len() == 395
            && ordered.iter().collect::<BTreeSet<_>>().len() == 395
            && coverage["ordered_case_ids"] == json!(ordered),
        "A09 exact ordered source cases missing/duplicated/reordered",
    )?;
    mapping_semantics(&coverage, &regression)?;
    let owned_ids = source["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["id"].as_str().is_some_and(|id| id.contains("-atomic-")))
        .map(|r| r["id"].clone())
        .collect::<Vec<_>>();
    let timers = read(root, "docs/rust-rewrite/a09/timeouts.json")?;
    let timer_ids = timers["records"]
        .as_array()
        .ok_or("A09 timer source records missing")?
        .iter()
        .map(|r| r["id"].clone())
        .collect::<Vec<_>>();
    require(
        coverage["owned_case_ids"] == json!(owned_ids)
            && coverage["timer_case_ids"] == json!(timer_ids),
        "A09 ownership/timer inventory not actual source IDs",
    )?;
    for (key, expected) in [
        ("owned_case_bindings", &owned_ids),
        ("timer_case_bindings", &timer_ids),
    ] {
        let rows = coverage[key]
            .as_array()
            .ok_or("A09 completion binding missing")?;
        require(
            rows.iter().map(|r| r["id"].clone()).collect::<Vec<_>>() == *expected,
            "A09 case not individually bound to native execution",
        )?;
    }

    let receipts = read(root, "docs/rust-rewrite/a09/oracle-runs.json")?;
    require(
        receipts["records"].as_array().is_some_and(|r| r.len() == 4),
        "A09 exact four source producer receipts missing",
    )?;
    let disposition = read(root, "docs/rust-rewrite/inventory/python-disposition.json")?;
    let invocations = read(root, "docs/rust-rewrite/inventory/python-invocations.json")?;
    for receipt in receipts["records"]
        .as_array()
        .ok_or("A09 oracle execution receipts missing")?
    {
        let path = receipt["producer_path"]
            .as_str()
            .ok_or("A09 producer path missing")?;
        let sha = digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?);
        require(
            receipt["exit_code"] == 0 && receipt["producer_sha256"] == sha,
            "A09 source producer execution identity differs",
        )?;
        require(
            disposition["files"].as_array().is_some_and(|r| {
                r.iter().any(|row| {
                    row["path"] == path
                        && row["sha256"] == sha
                        && row["migration_owner"] == "A09"
                        && row["migration_owners"]
                            .as_array()
                            .is_some_and(|x| x.iter().any(|v| v == "G08"))
                })
            }),
            "A09 temporary producer missing A09/G08 exact retirement inventory",
        )?;
        require(
            invocations["developer_oracle_invocations"]
                .as_array()
                .is_some_and(|r| {
                    r.iter().any(|row| {
                        row["example_path"] == path
                            && row["source_sha256"] == sha
                            && row["retirement_owner"] == "G08"
                    })
                }),
            "A09 temporary oracle invocation not inventoried",
        )?;
        let stdout_path = receipt["stdout_path"]
            .as_str()
            .ok_or("A09 source stdout missing")?;
        let stdout = fs::read(root.join(stdout_path)).map_err(|e| e.to_string())?;
        let wrapped: Value = serde_json::from_slice(&stdout).map_err(|e| e.to_string())?;
        let raw = wrapped["utf8"]
            .as_str()
            .ok_or("A09 raw source stdout missing")?;
        require(
            receipt["stdout_sha256"] == digest(&stdout)
                && receipt["raw_stdout_sha256"] == digest(raw.as_bytes())
                && wrapped["sha256"] == receipt["raw_stdout_sha256"]
                && receipt["command"]
                    .as_array()
                    .is_some_and(|args| args.iter().any(|arg| arg == "-O")),
            "A09 optimized process stdout receipt differs",
        )?;
        for output in receipt["outputs"]
            .as_array()
            .ok_or("A09 source capture output identity missing")?
        {
            let p = output["path"].as_str().ok_or("A09 output path missing")?;
            require(
                output["sha256"] == digest(&fs::read(root.join(p)).map_err(|e| e.to_string())?),
                "A09 output differs from actual producer run",
            )?;
        }
    }
    let mut targets = BTreeMap::new();
    for target in coverage["native_targets"]
        .as_array()
        .ok_or("A09 targets missing")?
    {
        let package = target["package"].as_str().ok_or("A09 package missing")?;
        let name = target["target"].as_str().ok_or("A09 target missing")?;
        let mut args = vec!["test", "--offline", "--locked", "-p", package];
        if name == "lib" {
            args.push("--lib")
        } else {
            args.extend(["--test", name])
        };
        args.extend(["--", "--list"]);
        let output = command(root, "cargo", &args, records)?;
        let available = output
            .lines()
            .filter_map(|l| l.strip_suffix(": test").map(str::to_owned))
            .collect::<BTreeSet<_>>();
        require(!available.is_empty(), "A09 empty native target")?;
        targets.insert((package.into(), name.into()), available);
    }
    for (_, test) in coverage["native_case_bindings"]
        .as_object()
        .ok_or("A09 native bindings missing")?
    {
        binding(test, &targets)?;
    }
    for mapping in coverage["regression_mapping"].as_array().unwrap() {
        for key in mapping["native_cases"]
            .as_array()
            .ok_or("A09 missing exact mapped native cases")?
        {
            let key = key.as_str().ok_or("A09 invalid native case key")?;
            binding(
                coverage["native_case_bindings"]
                    .get(key)
                    .ok_or("A09 mapped native case binding absent")?,
                &targets,
            )?;
        }
    }
    let gate = command(
        root,
        "cargo",
        &[
            "test",
            "--offline",
            "--locked",
            "-p",
            "xtask",
            "a09::tests",
            "--",
            "--format",
            "pretty",
        ],
        records,
    )?;
    execution(
        &gate,
        &BTreeSet::from([
            "a09::tests::exact_execution_and_completed_markers_reject_missing_ignored_or_reordered"
                .into(),
            "a09::tests::behavior_cannot_map_registration_only".into(),
        ]),
    )?;
    let mut owned_output = String::new();
    for target in coverage["native_targets"].as_array().unwrap() {
        let package = target["package"].as_str().unwrap();
        let name = target["target"].as_str().unwrap();
        let expected: BTreeSet<String> = target["tests"]
            .as_array()
            .ok_or("A09 exact test identity list missing")?
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect();
        require(
            expected
                .iter()
                .all(|n| targets[&(package.into(), name.into())].contains(n)),
            "A09 native exact inventory changed",
        )?;
        if name == "lib" {
            for test in &expected {
                let output = command(
                    root,
                    "cargo",
                    &[
                        "test",
                        "--offline",
                        "--locked",
                        "-p",
                        package,
                        "--lib",
                        test,
                        "--",
                        "--exact",
                        "--format",
                        "pretty",
                        "--test-threads=2",
                        "--nocapture",
                    ],
                    records,
                )?;
                execution(&output, &BTreeSet::from([test.clone()]))?;
                owned_output.push_str(&output);
                owned_output.push('\n');
            }
        } else {
            let output = command(
                root,
                "cargo",
                &[
                    "test",
                    "--offline",
                    "--locked",
                    "-p",
                    package,
                    "--test",
                    name,
                    "--",
                    "--format",
                    "pretty",
                    "--nocapture",
                    "--test-threads=2",
                ],
                records,
            )?;
            execution(&output, &expected)?;
            if name == "a09_factories" {
                let expected = coverage["timer_case_ids"]
                    .as_array()
                    .ok_or("A09 timer inventory missing")?
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>();
                completion_set(&output, "A09_TIMER_CASE ", &expected)?;
            }
            if name == "a09_catalog_media" {
                completed_cases(
                    &output,
                    &ordered.iter().map(|id| json!(id)).collect::<Vec<_>>(),
                )?;
            }
        }
    }
    let owned = coverage["owned_case_ids"]
        .as_array()
        .ok_or("A09 owned inventory missing")?
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    require(
        owned.len() == 9
            && coverage["timer_case_ids"]
                .as_array()
                .is_some_and(|v| v.len() == 12),
        "A09 actual owned/timer source inventory differs",
    )?;
    completion_set(&owned_output, "A09_OWNED_CASE ", &owned)?;
    for key in ["owned_case_bindings", "timer_case_bindings"] {
        for row in coverage[key]
            .as_array()
            .ok_or("A09 source case binding missing")?
        {
            binding(&row["native"], &targets)?;
        }
    }
    records.push(json!({"A09":{"actual_adapter_case_completions":ordered.len(),"owned_case_completions":9,"timer_case_completions":12,"original_functions":51,"original_parameters":56,"python_executed_or_required":false,"provider_calls_in_gate":false,"scope":"native named-engine request/parser/config/owned-operation and guarded private-loopback lifecycle; actual rank/six-component HTTP DTO; full portal/transport/global RSS remain downstream"}}));
    Ok(())
}
/// Verify an actual native mutation transcript with the same validators as the gate.
pub fn verify_transcript(root: &Path, path: &Path) -> Result<()> {
    let bytes =
        fs::read(root.join("docs/rust-rewrite/a09/coverage.json")).map_err(|e| e.to_string())?;
    require(
        digest(&bytes) == COVERAGE_SHA,
        "A09 transcript expected inventory changed",
    )?;
    let coverage: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let output = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let target = coverage["native_targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["target"] == "a09_catalog_media")
        .ok_or("A09 source native target missing")?;
    let expected = target["tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    execution(&output, &expected)?;
    completed_cases(
        &output,
        coverage["ordered_case_ids"]
            .as_array()
            .ok_or("A09 expected IDs missing")?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_execution_and_completed_markers_reject_missing_ignored_or_reordered() {
        let tests = BTreeSet::from(["module::exact".into()]);
        execution("test module::exact ... ok\n", &tests).unwrap();
        for output in [
            "test module::exact ... ignored\n",
            "test module::other ... ok\n",
            "test result: ok. 0 passed; 0 failed; 0 ignored\n",
        ] {
            assert!(execution(output, &tests).is_err());
        }
        let expected_ids = vec!["a".to_owned(), "b".to_owned()];
        completion_set(
            "A09_OWNED_CASE \"b\"\nA09_OWNED_CASE \"a\"\n",
            "A09_OWNED_CASE ",
            &expected_ids,
        )
        .unwrap();
        for text in [
            "A09_OWNED_CASE \"a\"\n",
            "A09_OWNED_CASE a\nA09_OWNED_CASE b\n",
            "A09_OWNED_CASE \"a\"\nA09_OWNED_CASE \"a\"\nA09_OWNED_CASE \"b\"\n",
        ] {
            assert!(completion_set(text, "A09_OWNED_CASE ", &expected_ids).is_err());
        }
        let expected = vec![json!("a"), json!("b")];
        completed_cases("A09_CASE \"a\"\nA09_CASE \"b\"\n", &expected).unwrap();
        for text in [
            "A09_CASE \"a\"\n",
            "A09_CASE \"b\"\nA09_CASE \"a\"\n",
            "A09_CASE \"a\"\nA09_CASE \"a\"\nA09_CASE \"b\"\n",
        ] {
            assert!(completed_cases(text, &expected).is_err());
        }
    }
    #[test]
    fn behavior_cannot_map_registration_only() {
        let c: Value =
            serde_json::from_str(include_str!("../../../docs/rust-rewrite/a09/coverage.json"))
                .unwrap();
        let r: Value = serde_json::from_str(include_str!(
            "../../../docs/rust-rewrite/a09/regressions.json"
        ))
        .unwrap();
        mapping_semantics(&c, &r).unwrap();
        let mut wrong = c.clone();
        let row=wrong["regression_mapping"].as_array_mut().unwrap().iter_mut().find(|r|r["node"]=="tests/test_openlibrary_url_contract.py::test_distinct_urls_survive_merger_and_json_formatting").unwrap();
        row["source_case_ids"] = json!([]);
        row["kind"] = json!("registration-metadata-default");
        assert!(mapping_semantics(&wrong, &r).is_err());
    }
}
