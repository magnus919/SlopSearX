//! Source-bound A11 qualification. Only compiled native tests execute in this gate.
use crate::{Result, command};
use serde::Deserialize;
use serde_json::{Value, json, value::RawValue};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
const COVERAGE_SHA: &str = "69b18d8f2ecb57639972c89bf57b105b83098ea38350bca00695182eda098ff4";
const ORIGINAL: [&str; 4] = ["abuseipdb", "greynoise", "otx", "urlhaus"];
fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn read(root: &Path, path: &str) -> Result<Value> {
    serde_json::from_slice(&fs::read(root.join(path)).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
#[derive(Deserialize)]
struct Report {
    nodeid: String,
    when: String,
    outcome: String,
}
#[derive(Deserialize)]
struct Capture {
    producer_sha256: String,
    real_network_allowed: bool,
    records: Vec<Box<RawValue>>,
    #[serde(default)]
    reports: Vec<Report>,
}
struct Observation {
    id: String,
    source_test: Option<String>,
}
fn capture(root: &Path, path: &str) -> Result<Capture> {
    // Outputs remain opaque: internal surrogate/nonfinite references and large
    // integers are not decoded through serde_json::Value to claim fidelity.
    let value: Capture =
        serde_json::from_slice(&fs::read(root.join(path)).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    require(
        !value.real_network_allowed,
        "A11 original capture allowed network",
    )?;
    Ok(value)
}
fn observations(capture: &Capture) -> Result<Vec<Observation>> {
    let mut ids = BTreeSet::new();
    capture
        .records
        .iter()
        .map(|raw| {
            let fields: BTreeMap<String, Box<RawValue>> =
                serde_json::from_str(raw.get()).map_err(|e| e.to_string())?;
            let id: String =
                serde_json::from_str(fields.get("id").ok_or("A11 source ID missing")?.get())
                    .map_err(|e| e.to_string())?;
            require(
                !id.is_empty() && ids.insert(id.clone()),
                "A11 duplicate/empty source ID",
            )?;
            let source_test = fields
                .get("source_test")
                .map(|value| serde_json::from_str(value.get()).map_err(|e| e.to_string()))
                .transpose()?;
            Ok(Observation { id, source_test })
        })
        .collect()
}
fn strings(value: &Value) -> Result<Vec<String>> {
    value
        .as_array()
        .ok_or("A11 string array missing")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "A11 string missing".into())
        })
        .collect()
}
fn cases(test: &Value) -> Result<BTreeMap<(String, String), usize>> {
    let mut result = BTreeMap::new();
    for case in test["cases"].as_array().ok_or("A11 native cases missing")? {
        let prefix = case["prefix"].as_str().ok_or("A11 marker prefix missing")?;
        let value = case["value"].as_str().ok_or("A11 marker value missing")?;
        require(
            prefix.starts_with("A11_") && prefix.ends_with("_COMPLETE") && !value.is_empty(),
            "A11 invalid marker",
        )?;
        require(
            case["id"] == format!("{prefix}:{value}"),
            "A11 case identity differs from actual marker",
        )?;
        require(
            result.insert((prefix.into(), value.into()), 1).is_none(),
            "A11 duplicate case marker",
        )?;
    }
    require(!result.is_empty(), "A11 caseless native test")?;
    Ok(result)
}
fn completions(output: &str, expected: &BTreeMap<(String, String), usize>) -> Result<()> {
    let mut actual = BTreeMap::new();
    for line in output.lines() {
        // libtest can print its test-name prefix before the first user println.
        if let Some(start) = line.find("A11_")
            && let Some((prefix, value)) = line[start..].split_once(' ')
            && prefix.ends_with("_COMPLETE")
        {
            *actual
                .entry((prefix.to_owned(), value.to_owned()))
                .or_insert(0) += 1;
        }
    }
    require(
        !expected.is_empty() && actual == *expected,
        "A11 actual case completion missing, altered, extra or duplicate",
    )
}
fn execution(output: &str, name: &str) -> Result<()> {
    require(
        output.contains(&format!("test {name} ..."))
            && output.contains("test result: ok. 1 passed; 0 failed; 0 ignored;")
            && !output
                .lines()
                .any(|s| s.starts_with("test ") && s.ends_with(" ... ignored")),
        "A11 exact named native test absent, ignored, caseless or failed",
    )
}
fn original_declarations(root: &Path) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for engine in ORIGINAL {
        let path = format!("tests/test_{engine}.py");
        let source = fs::read_to_string(root.join(&path)).map_err(|e| e.to_string())?;
        let mut class = String::new();
        for line in source.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("class ") {
                class = rest
                    .split(['(', ':'])
                    .next()
                    .ok_or("A11 class identity absent")?
                    .into();
            }
            if let Some(rest) = line
                .strip_prefix("async def ")
                .or_else(|| line.strip_prefix("def "))
            {
                let name = rest
                    .split('(')
                    .next()
                    .ok_or("A11 function identity absent")?;
                if name.starts_with("test_") {
                    require(
                        !class.is_empty() && names.insert(format!("{path}::{class}::{name}")),
                        "A11 duplicate original declaration",
                    )?;
                }
            }
        }
    }
    require(
        names.len() == 32,
        "A11 exact32 original declarations differ",
    )?;
    Ok(names)
}
fn mappings(root: &Path, pack: &Value, all_cases: &BTreeSet<String>) -> Result<()> {
    let source = capture(root, "docs/rust-rewrite/a11/source-regressions-v1.json")?;
    let observations = observations(&source)?;
    let mut phases = BTreeMap::new();
    let mut calls = Vec::new();
    for report in source.reports {
        require(
            report.outcome == "passed",
            "A11 original phase did not pass",
        )?;
        require(
            ["setup", "call", "teardown"].contains(&report.when.as_str()),
            "A11 unknown original phase",
        )?;
        if report.when == "call" {
            calls.push(report.nodeid.clone());
        }
        *phases.entry((report.nodeid, report.when)).or_insert(0) += 1;
    }
    require(
        calls.len() == 41
            && calls.iter().collect::<BTreeSet<_>>().len() == 41
            && phases.len() == 123
            && phases.values().all(|n| *n == 1),
        "A11 exact41 parameterized source calls/123 phases differ",
    )?;
    for node in &calls {
        for phase in ["setup", "call", "teardown"] {
            require(
                phases.get(&(node.clone(), phase.into())) == Some(&1),
                "A11 source call phase missing",
            )?;
        }
    }
    let declarations = original_declarations(root)?;
    let rows = pack["original_mapping"]
        .as_array()
        .ok_or("A11 source mappings missing")?;
    require(
        rows.len() == 32
            && rows
                .iter()
                .filter_map(|r| r["identity"].as_str())
                .collect::<BTreeSet<_>>()
                == declarations.iter().map(String::as_str).collect(),
        "A11 source function mapping differs",
    )?;
    for row in rows {
        let node = row["identity"].as_str().ok_or("A11 mapping ID missing")?;
        let actual_calls: Vec<_> = calls
            .iter()
            .filter(|call| call.split('[').next() == Some(node))
            .cloned()
            .collect();
        require(
            row["actual_parameterized_calls"] == json!(actual_calls),
            "A11 literal parameterized source calls unmapped",
        )?;
        let observed: Vec<_> = observations
            .iter()
            .filter(|o| {
                o.source_test
                    .as_ref()
                    .is_some_and(|s| s.split('[').next() == Some(node))
            })
            .map(|o| o.id.clone())
            .collect();
        require(
            row["source_observation_ids"] == json!(observed),
            "A11 actual source observation mapping differs",
        )?;
        let expected: Vec<_> = if observed.is_empty() {
            require(
                node.ends_with("::test_adapter_registered")
                    || node.ends_with("::test_adapter_categories"),
                "A11 behavior incorrectly mapped to metadata",
            )?;
            vec![format!("A11_SOURCE_TEST_COMPLETE:{node}")]
        } else {
            observed
                .iter()
                .map(|id| format!("A11_OBSERVATION_COMPLETE:baseline58:{id}"))
                .collect()
        };
        require(
            strings(&row["native_cases"])? == expected
                && expected.iter().all(|id| all_cases.contains(id)),
            "A11 source/native semantic binding missing",
        )?;
    }
    Ok(())
}
fn oracle_provenance(root: &Path) -> Result<()> {
    let runs = read(root, "docs/rust-rewrite/a11/oracle-runs-v1.json")?;
    let rows = runs["records"]
        .as_array()
        .ok_or("A11 original run records missing")?;
    require(rows.len() == 6, "A11 exact six original producers missing")?;
    for row in rows {
        let path = row["producer_path"]
            .as_str()
            .ok_or("A11 producer path missing")?;
        require(
            row["current_producer_sha256"]
                == digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?)
                && row["actual_exit_code"] == 0
                && row["optional_manual_only"] == true
                && row["native_gate_never_executes_producer"] == true,
            "A11 source/current producer scope differs",
        )?;
        for (path_key, sha_key) in [
            ("executed_source_archive", "executed_source_archive_sha256"),
            ("actual_receipt_path", "actual_receipt_sha256"),
            ("actual_raw_log_path", "actual_raw_log_sha256"),
            ("capture_path", "capture_sha256"),
        ] {
            let path = row[path_key]
                .as_str()
                .ok_or("A11 oracle artifact path absent")?;
            require(
                row[sha_key] == digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?),
                "A11 immutable original execution artifact changed",
            )?;
        }
        require(
            row["actual_executed_source_sha256"] == row["executed_source_archive_sha256"],
            "A11 historical producer confused with current formatting",
        )?;
        let cap = capture(root, row["capture_path"].as_str().unwrap())?;
        require(
            row["actual_executed_source_sha256"] == cap.producer_sha256,
            "A11 capture not bound to actual executed producer bytes",
        )?;
        let receipt = read(root, row["actual_receipt_path"].as_str().unwrap())?;
        require(
            receipt["terminal"] == true
                && (receipt["actual_exit_code"] == 0 || receipt["exit_code"] == 0),
            "A11 original execution not actually terminal/pass",
        )?;
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let bytes = fs::read(root.join("docs/rust-rewrite/a11/qualification.json"))
        .map_err(|e| e.to_string())?;
    require(
        digest(&bytes) == COVERAGE_SHA,
        "A11 qualification changed; audit and repin",
    )?;
    let pack: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    require(
        pack["plan_id"] == "A11"
            && pack["source_baseline"] == "8b48dc9ea0917230f31dfcc9c44ea2698b612b36"
            && pack["scope"] == json!(ORIGINAL),
        "A11 exact source scope changed",
    )?;
    for row in pack["owned_sources"]
        .as_array()
        .ok_or("A11 source bindings missing")?
    {
        let path = row["path"].as_str().ok_or("A11 source path absent")?;
        crate::a13::check_source_pin(
            root,
            "docs/rust-rewrite/a11/qualification.json",
            &digest(&bytes),
            path,
            row["sha256"].as_str().ok_or("A11 source digest absent")?,
            row["bytes"].as_u64().map(|value| value as usize),
        )
        .map_err(|_| format!("A11 source/fixture differs: {path}"))?;
    }
    let tests = pack["native_tests"]
        .as_array()
        .ok_or("A11 native tests absent")?;
    require(
        tests.len() == 14,
        "A11 exact14 native test identities changed",
    )?;
    let mut all_cases = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for test in tests {
        require(
            identities.insert((test["target"].to_string(), test["name"].to_string())),
            "A11 duplicate native test",
        )?;
        let expected = cases(test)?;
        for (prefix, value) in expected.keys() {
            require(
                all_cases.insert(format!("{prefix}:{value}")),
                "A11 duplicate native case identity",
            )?;
        }
    }
    require(
        all_cases.len() == 419 && pack["actual_cases_total"] == 419,
        "A11 exact419 native case bindings differ",
    )?;
    mappings(root, &pack, &all_cases)?;
    for (profile, path, count) in [
        ("baseline58", "source-regressions-v1.json", 58),
        ("boundary250", "source-boundaries-v2.json", 250),
        ("supplement3", "source-supplement-v1.json", 3),
        ("representation10", "source-representation-v1.json", 10),
    ] {
        let source = capture(root, &format!("docs/rust-rewrite/a11/{path}"))?;
        let rows = observations(&source)?;
        require(
            rows.len() == count
                && rows.iter().all(|row| {
                    all_cases.contains(&format!("A11_OBSERVATION_COMPLETE:{profile}:{}", row.id))
                }),
            "A11 complete source record not bound to actual native case",
        )?;
    }
    let config = capture(root, "docs/rust-rewrite/a11/source-config-entry-v1.json")?;
    let config_rows = observations(&config)?;
    require(
        config_rows.len() == 4
            && config_rows.iter().all(|row| {
                all_cases.contains(&format!(
                    "A11_CONFIG_COMPLETE:original-yaml-rejected-before-factory:{}",
                    row.id
                ))
            }),
        "A11 F06 entry-layer scope missing actual native refusal",
    )?;
    let credential = capture(
        root,
        "docs/rust-rewrite/a11/source-credential-ascii-v1.json",
    )?;
    let credential_rows = observations(&credential)?;
    let expected_credential_ids: Vec<_> = ORIGINAL
        .iter()
        .map(|engine| format!("a11-credential/{engine}/unicode-key-ascii-refusal"))
        .collect();
    require(
        credential_rows
            .iter()
            .map(|row| row.id.clone())
            .collect::<Vec<_>>()
            == expected_credential_ids
            && pack["credential_header_profile"]["records"] == 4
            && credential_rows
                .iter()
                .all(|row| all_cases.contains(&format!("A11_CREDENTIAL_COMPLETE:{}", row.id))),
        "A11 exact four original ASCII credential controls missing actual native completion",
    )?;
    oracle_provenance(root)?;
    let unit = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "--offline",
            "-p",
            "xtask",
            "a11::tests::",
            "--",
            "--nocapture",
        ],
        records,
    )?;
    for name in [
        "a11::tests::missing_ignored_caseless_and_failed_native_execution_is_rejected",
        "a11::tests::exact_case_completions_reject_missing_duplicate_extra_and_altered",
    ] {
        require(
            unit.contains(&format!("test {name} ... ok")),
            "A11 actual gate negative-control test missing",
        )?;
    }
    let mut available = BTreeMap::new();
    for target in ["lib", "a11_replay", "a11_factories"] {
        let mut args = vec!["test", "--locked", "--offline", "-p", "ssx-infra"];
        if target == "lib" {
            args.push("--lib");
        } else {
            args.extend(["--test", target]);
        }
        args.extend(["--", "--list"]);
        let listed = command(root, "cargo", &args, records)?;
        let names: BTreeSet<_> = listed
            .lines()
            .filter_map(|s| s.strip_suffix(": test").map(str::to_owned))
            .collect();
        require(!names.is_empty(), "A11 listed native target empty")?;
        available.insert(target, names);
    }
    for test in tests {
        let target = test["target"].as_str().ok_or("A11 native target missing")?;
        let name = test["name"].as_str().ok_or("A11 native name missing")?;
        require(
            test["package"] == "ssx-infra"
                && available
                    .get(target)
                    .is_some_and(|names| names.contains(name)),
            "A11 actual exact native identity absent",
        )?;
        let mut args = vec!["test", "--locked", "--offline", "-p", "ssx-infra"];
        if target == "lib" {
            args.push("--lib");
        } else {
            args.extend(["--test", target]);
        }
        args.extend([name, "--", "--exact", "--nocapture", "--format", "pretty"]);
        let output = command(root, "cargo", &args, records)?;
        execution(&output, name)?;
        completions(&output, &cases(test)?)?;
    }
    records.push(json!({"a11_complete_original_observations":321,"actual_native_cases":419,"credential_header_original_controls":4,"original_test_functions":32,"actual_original_parameterized_calls":41,"original_raw_yaml_entry_controls":4,"source_outputs_kept_opaque_for_inventory":true,"manual_python_oracles_executed_by_gate":false,"production_composition_claim":false}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_ignored_caseless_and_failed_native_execution_is_rejected() {
        let valid = "test native ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;";
        assert!(execution(valid, "native").is_ok());
        for bad in [
            "",
            "test native ... ignored\ntest result: ok. 1 passed; 0 failed; 0 ignored;",
            "test other ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;",
            "test native ... ok\ntest result: ok. 0 passed; 0 failed; 0 ignored;",
            "test native ... FAILED",
        ] {
            assert!(execution(bad, "native").is_err());
        }
    }
    #[test]
    fn exact_case_completions_reject_missing_duplicate_extra_and_altered() {
        let expected = BTreeMap::from([(("A11_CASE_COMPLETE".into(), "one".into()), 1)]);
        assert!(completions("test native ... A11_CASE_COMPLETE one\n", &expected).is_ok());
        for bad in [
            "",
            "A11_CASE_COMPLETE one\nA11_CASE_COMPLETE one",
            "A11_CASE_COMPLETE two",
            "A11_CASE_COMPLETE one\nA11_CASE_COMPLETE extra",
        ] {
            assert!(completions(bad, &expected).is_err());
        }
        assert!(cases(&json!({"cases":[]})).is_err());
        assert!(
            cases(&json!({"cases":[{"id":"wrong","prefix":"A11_CASE_COMPLETE","value":"one"}]}))
                .is_err()
        );
    }
}
