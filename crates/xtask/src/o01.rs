//! Native API and private-backend O01 qualification; never invokes an interpreter.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const COVERAGE_SHA: &str = "add7293c5ee3baf651536ec8eabe572145a3775a88e2277252f4824505c6ec30";
fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn records(output: &str, prefix: &str) -> Result<Vec<Value>> {
    output
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(|line| serde_json::from_str(line).map_err(|e| format!("O01 completion JSON: {e}")))
        .collect()
}
fn exact_records(actual: &[Value], expected: &[Value], identity: &str) -> Result<()> {
    let canonical = |rows: &[Value]| -> Vec<String> {
        let mut rows = rows.iter().map(Value::to_string).collect::<Vec<_>>();
        rows.sort();
        rows
    };
    let ids = actual
        .iter()
        .map(|row| row[identity].as_str().unwrap_or(""))
        .collect::<BTreeSet<_>>();
    require(
        !expected.is_empty()
            && !ids.contains("")
            && ids.len() == actual.len()
            && canonical(actual) == canonical(expected),
        "O01 completed source records missing, duplicate, extra, or different",
    )
}
fn execution(output: &str, expected: &[Value]) -> Result<()> {
    let actual = output
        .lines()
        .filter_map(|line| line.strip_prefix("test ")?.strip_suffix(" ... ok"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let names = actual.iter().cloned().collect::<BTreeSet<_>>();
    let wanted = expected
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    require(
        !wanted.is_empty()
            && names == wanted
            && actual.len() == names.len()
            && !output
                .lines()
                .any(|line| line.starts_with("test ") && line.ends_with(" ... ignored")),
        "O01 exact native tests missing, duplicated, extra, or ignored",
    )
}
pub fn check(root: &Path, commands: &mut Vec<Value>) -> Result<()> {
    let bytes =
        fs::read(root.join("docs/rust-rewrite/o01/coverage.json")).map_err(|e| e.to_string())?;
    require(
        digest(&bytes) == COVERAGE_SHA,
        "O01 coverage digest differs",
    )?;
    let coverage: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    require(coverage["plan_id"] == "O01", "O01 plan identity differs")?;
    for artifact in coverage["artifacts"]
        .as_array()
        .ok_or("O01 artifacts missing")?
    {
        let path = artifact["path"]
            .as_str()
            .ok_or("O01 artifact path missing")?;
        let bytes = fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
        require(
            artifact["sha256"] == digest(&bytes),
            &format!("O01 artifact differs: {path}"),
        )?;
    }
    let source: Value = serde_json::from_slice(
        &fs::read(
            root.join(
                coverage["original_source_execution"]
                    .as_str()
                    .ok_or("O01 source execution path missing")?,
            ),
        )
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let collected = source["collected"]
        .as_array()
        .ok_or("O01 source collected nodes missing")?;
    let original_expected = coverage["completed_original"]
        .as_array()
        .ok_or("O01 original records missing")?;
    let nodes = original_expected
        .iter()
        .filter_map(|r| r["node"].as_str())
        .collect::<BTreeSet<_>>();
    require(
        source["exit"] == 0
            && source["network_attempts"]
                .as_array()
                .is_some_and(Vec::is_empty)
            && collected.len() == 53
            && nodes.len() == 53
            && nodes == collected.iter().filter_map(Value::as_str).collect()
            && nodes
                .iter()
                .map(|n| n.split('[').next().unwrap_or(""))
                .collect::<BTreeSet<_>>()
                .len()
                == 42
            && source["deselected"] == coverage["downstream_http_handoffs"],
        "O01 original source node/function/handoff set differs",
    )?;
    for phase in ["setup", "call", "teardown"] {
        let rows = source["reports"]
            .as_array()
            .ok_or("O01 source phase reports missing")?
            .iter()
            .filter(|r| r["when"] == phase)
            .collect::<Vec<_>>();
        require(
            rows.len() == 53
                && rows.iter().all(|r| r["outcome"] == "passed")
                && rows
                    .iter()
                    .filter_map(|r| r["nodeid"].as_str())
                    .collect::<BTreeSet<_>>()
                    == nodes,
            "O01 original source phase missing or failed",
        )?;
    }
    let bindings = coverage["original_bindings"]
        .as_array()
        .ok_or("O01 original bindings missing")?;
    require(bindings.len() == 53, "O01 original binding count differs")?;
    for row in original_expected {
        let matches = bindings
            .iter()
            .filter(|b| b["source_node"] == row["node"] && b["consumer"] == row["consumer"])
            .collect::<Vec<_>>();
        require(
            matches.len() == 1,
            "O01 original consumer binding missing or duplicate",
        )?;
        let binding = matches[0];
        require(
            coverage["native_targets"]
                .as_array()
                .is_some_and(|targets| {
                    targets.iter().any(|t| {
                        t["package"] == binding["package"]
                            && t["target"] == binding["target"]
                            && t["tests"]
                                .as_array()
                                .is_some_and(|tests| tests.contains(&binding["qualified_name"]))
                    })
                }),
            "O01 original consumer not in executed target",
        )?;
    }
    let self_output = command(
        root,
        "cargo",
        &[
            "test",
            "--offline",
            "--locked",
            "-p",
            "xtask",
            "o01::tests::",
            "--",
            "--test-threads=1",
            "--show-output",
        ],
        commands,
    )?;
    execution(
        &self_output,
        &[json!(
            "o01::tests::successful_test_status_cannot_replace_exact_source_completions"
        )],
    )?;
    let mut original = Vec::new();
    let mut supplement = Vec::new();

    for target in coverage["native_targets"]
        .as_array()
        .ok_or("O01 targets missing")?
    {
        let package = target["package"].as_str().ok_or("O01 package missing")?;
        let name = target["target"].as_str().ok_or("O01 target missing")?;
        let filter = target["filter"].as_str().ok_or("O01 filter missing")?;
        let tests = target["tests"].as_array().ok_or("O01 tests missing")?;
        let mut args = vec!["test", "--offline", "--locked", "-p", package];
        if name == "lib" {
            args.push("--lib");
        } else {
            args.extend(["--test", name]);
        }
        if !filter.is_empty() {
            args.push(filter);
        }
        let mut list_args = args.clone();
        list_args.extend(["--", "--list"]);
        let listed = command(root, "cargo", &list_args, commands)?;
        let available = listed
            .lines()
            .filter_map(|line| line.strip_suffix(": test"))
            .collect::<BTreeSet<_>>();
        require(
            available == tests.iter().filter_map(Value::as_str).collect(),
            "O01 compiled identities differ",
        )?;
        args.extend(["--", "--test-threads=1", "--show-output"]);
        let output = command(root, "cargo", &args, commands)?;
        execution(&output, tests)?;
        original.extend(records(&output, "O01_ORIGINAL_COMPLETE:")?);
        supplement.extend(records(&output, "O01_CASE_COMPLETE:")?);
    }
    exact_records(
        &original,
        coverage["completed_original"]
            .as_array()
            .ok_or("O01 original records missing")?,
        "node",
    )?;
    exact_records(
        &supplement,
        coverage["completed_supplement"]
            .as_array()
            .ok_or("O01 supplemental records missing")?,
        "id",
    )?;
    commands.push(json!({"O01":{"original_parameter_completions":original.len(),"supplemental_completions":supplement.len(),"python_executed":false,"provider_calls":false,"scope":coverage["scope"],"downstream_http_handoffs":coverage["downstream_http_handoffs"]}}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn successful_test_status_cannot_replace_exact_source_completions() {
        let expected = vec![
            json!({"node":"a","class":"exact"}),
            json!({"node":"b","class":"reviewed"}),
        ];
        execution("test native ... ok\n", &[json!("native")]).unwrap();
        exact_records(&expected, &expected, "node").unwrap();
        for bad in [
            vec![expected[0].clone()],
            vec![expected[0].clone(), expected[0].clone()],
            vec![expected[0].clone(), json!({"node":"b","class":"exact"})],
            vec![
                expected[0].clone(),
                expected[1].clone(),
                json!({"node":"c"}),
            ],
        ] {
            assert!(exact_records(&bad, &expected, "node").is_err());
        }
        assert!(records("O01_ORIGINAL_COMPLETE:{invalid", "O01_ORIGINAL_COMPLETE:").is_err());
        assert!(execution("test native ... ignored", &[json!("native")]).is_err());
    }
}
