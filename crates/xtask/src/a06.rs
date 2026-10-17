//! Source-bound scholarly qualification. Source producers are never executed.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const COVERAGE_SHA256: &str = "92686ed70979a698b0dab3414cad813198b720fe586421062db8a4cef5102d9b";
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn verify_executed(output: &str, expected: &BTreeSet<String>) -> Result<()> {
    let passed: Vec<_> = output
        .lines()
        .filter_map(|line| {
            line.strip_prefix("test ")?
                .strip_suffix(" ... ok")
                .map(str::to_owned)
        })
        .collect();
    let unique: BTreeSet<_> = passed.iter().cloned().collect();
    if passed.len() != unique.len() || unique != *expected || expected.is_empty() {
        return Err(
            "A06 exact native identities were missing, duplicated, ignored or failed".into(),
        );
    }
    Ok(())
}
fn verify_source_cases(output: &str, expected: &BTreeSet<String>) -> Result<()> {
    let mut actual = BTreeSet::new();
    for line in output.lines() {
        if let Some(raw) = line.strip_prefix("A06_COMPLETED_SOURCE ") {
            let row: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
            let object = row.as_object().ok_or("A06 case marker is not an object")?;
            if object.len() != 5
                || ["consumer", "corpus", "id", "variant", "kind"]
                    .iter()
                    .any(|key| !row[*key].as_str().is_some_and(|s| !s.is_empty()))
            {
                return Err("A06 malformed source completion".into());
            }
            let identity = serde_json::to_string(&row).map_err(|e| e.to_string())?;
            if !actual.insert(identity) {
                return Err("A06 duplicate source completion".into());
            }
        }
    }
    if actual != *expected {
        return Err(format!(
            "A06 source completions differ: missing {:?}; extra {:?}",
            expected.difference(&actual).collect::<Vec<_>>(),
            actual.difference(expected).collect::<Vec<_>>()
        ));
    }
    Ok(())
}
/// Diagnostic/mutation checker uses the same exact matcher as the full gate.
pub fn check_source_log(root: &Path, path: &Path) -> Result<()> {
    let inventory: Value = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/a06/completed-source-cases.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let expected = inventory["completed_cases"]
        .as_array()
        .ok_or("A06 case inventory missing")?
        .iter()
        .map(|row| serde_json::to_string(row).map_err(|e| e.to_string()))
        .collect::<Result<BTreeSet<_>>>()?;
    verify_source_cases(
        &fs::read_to_string(path).map_err(|e| e.to_string())?,
        &expected,
    )
}
fn verify_artifact(root: &Path, row: &Value) -> Result<()> {
    let path = row["path"].as_str().ok_or("A06 artifact path missing")?;
    let expected = row["sha256"].as_str().ok_or("A06 artifact hash missing")?;
    if digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?) != expected {
        return Err(format!("A06 artifact changed: {path}"));
    }
    Ok(())
}
fn expected_tests_for_platform(group: &Value, linux_enabled: bool) -> Result<BTreeSet<String>> {
    let common = group["tests"]
        .as_array()
        .ok_or("A06 native tests missing")?;
    let linux = match group.get("linux_tests") {
        Some(value) => value
            .as_array()
            .ok_or("A06 Linux tests malformed")?
            .as_slice(),
        None => &[],
    };
    let mut all = BTreeSet::new();
    let mut expected = BTreeSet::new();
    for (include, tests) in [(true, common.as_slice()), (linux_enabled, linux)] {
        for test in tests {
            let name = test
                .as_str()
                .filter(|name| !name.is_empty())
                .ok_or("A06 test identity malformed")?;
            if !all.insert(name.to_owned()) {
                return Err("A06 duplicate platform test identity".into());
            }
            if include {
                expected.insert(name.to_owned());
            }
        }
    }
    if expected.is_empty() {
        return Err("A06 native test inventory empty".into());
    }
    Ok(expected)
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let bytes =
        fs::read(root.join("docs/rust-rewrite/a06/coverage.json")).map_err(|e| e.to_string())?;
    if digest(&bytes) != COVERAGE_SHA256 {
        return Err("A06 frozen source coverage changed".into());
    }
    let coverage: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    for row in coverage["artifacts"]
        .as_array()
        .ok_or("A06 artifact bindings missing")?
    {
        verify_artifact(root, row)?;
    }
    let mappings = coverage["regression_mapping"]
        .as_array()
        .ok_or("A06 source regression mapping missing")?;
    if mappings.is_empty() {
        return Err("A06 source regression inventory empty".into());
    }
    let mut mapped = BTreeSet::new();
    for row in mappings {
        let id = row["source_identity"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("A06 source identity missing")?;
        if !mapped.insert(id) {
            return Err("A06 duplicate original regression identity".into());
        }
        if row["native_bindings"]
            .as_array()
            .is_none_or(|v| v.is_empty())
            || row["status"] != "qualified local"
        {
            return Err(format!("A06 original regression not qualified: {id}"));
        }
    }
    command(
        root,
        "cargo",
        &[
            "test",
            "--offline",
            "--locked",
            "-p",
            "xtask",
            "a06",
            "--",
            "--format",
            "pretty",
        ],
        records,
    )?;
    // Qualify native containment independently before the longer consumer suite.
    crate::a06_resource::check(root, records)?;
    // Library ownership controls launch both real parser binaries before any
    // integration target can implicitly build them. Qualify from a cold target.
    command(
        root,
        "cargo",
        &[
            "build",
            "--offline",
            "--locked",
            "-p",
            "ssx-infra",
            "--bins",
        ],
        records,
    )?;
    let mut bindings = BTreeSet::new();
    let mut completed_output = String::new();
    for group in coverage["native_groups"]
        .as_array()
        .ok_or("A06 native groups missing")?
    {
        let package = group["package"].as_str().ok_or("A06 package missing")?;
        let target = group["target"].as_str().ok_or("A06 target missing")?;
        let expected = expected_tests_for_platform(group, cfg!(target_os = "linux"))?;
        for name in &expected {
            if !bindings.insert(format!("{package}/{target}/{name}")) {
                return Err("A06 duplicate native binding".into());
            }
        }
        let mut args = vec!["test", "--offline", "--locked", "-p", package];
        if target == "lib" {
            args.push("--lib");
        } else {
            args.extend(["--test", target]);
        }
        let mut list = args.clone();
        list.extend(["--", "--list"]);
        let listed = command(root, "cargo", &list, records)?;
        let available: BTreeSet<_> = listed
            .lines()
            .filter_map(|s| s.strip_suffix(": test").map(str::to_owned))
            .collect();
        if group["whole_binary"] == false {
            if !available.is_superset(&expected) {
                return Err(format!(
                    "A06 selected native identity absent: {package}/{target}"
                ));
            }
            for name in &expected {
                let mut exact = args.clone();
                exact.extend([
                    name,
                    "--",
                    "--exact",
                    "--format",
                    "pretty",
                    "--test-threads=1",
                    "--show-output",
                ]);
                let output = command(root, "cargo", &exact, records)?;
                verify_executed(&output, &BTreeSet::from([name.clone()]))?;
                completed_output.push_str(&output);
            }
        } else {
            if available != expected {
                return Err(format!("A06 native test set changed: {package}/{target}"));
            }
            args.extend([
                "--",
                "--format",
                "pretty",
                "--test-threads=1",
                "--show-output",
            ]);
            let output = command(root, "cargo", &args, records)?;
            verify_executed(&output, &expected)?;
            completed_output.push_str(&output);
        }
    }
    let expected_cases: BTreeSet<String> = coverage["completed_source_cases"]
        .as_array()
        .ok_or("A06 completed source inventory missing")?
        .iter()
        .map(|row| serde_json::to_string(row).map_err(|e| e.to_string()))
        .collect::<Result<_>>()?;
    if expected_cases.is_empty() {
        return Err("A06 completed source inventory empty".into());
    }
    verify_source_cases(&completed_output, &expected_cases)?;
    for row in mappings {
        for binding in row["native_bindings"]
            .as_array()
            .ok_or("A06 mapping malformed")?
        {
            if !binding.as_str().is_some_and(|s| bindings.contains(s)) {
                return Err("A06 regression binding was not executed".into());
            }
        }
    }
    records.push(json!({"A06":{"original_regressions":mapped.len(),"completed_native_identities":bindings,"qualification":"local source-bound scholarly factories/helper semantics/lifecycle; reviewed differences retained","temporary_python":"not executed or required","remaining":"G02/global ownership, final platform/exact-head and issue984 cross-surface acceptance remain separate"}}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platform_inventory_retains_common_and_only_adds_linux_cases() {
        let group = json!({"tests":["common::case"],"linux_tests":["linux::case"]});
        assert_eq!(
            expected_tests_for_platform(&group, false).unwrap(),
            BTreeSet::from(["common::case".to_owned()])
        );
        assert_eq!(
            expected_tests_for_platform(&group, true).unwrap(),
            BTreeSet::from(["common::case".to_owned(), "linux::case".to_owned()])
        );
        for group in [
            json!({"tests":["common::case","common::case"]}),
            json!({"tests":["common::case"],"linux_tests":["common::case"]}),
            json!({"tests":["common::case"],"linux_tests":[null]}),
            json!({"tests":[]}),
        ] {
            for linux in [false, true] {
                assert!(expected_tests_for_platform(&group, linux).is_err());
            }
        }
    }
    #[test]
    fn missing_ignored_duplicate_and_wrong_completed_identities_fail_closed() {
        let ids = BTreeSet::from(["real::case".to_owned()]);
        verify_executed("test real::case ... ok\n", &ids).unwrap();
        for text in [
            "",
            "test real::case ... ignored\n",
            "test other ... ok\n",
            "test real::case ... ok\ntest real::case ... ok\n",
            "test result: ok. 1 passed; 0 ignored",
        ] {
            assert!(verify_executed(text, &ids).is_err());
        }
        assert!(verify_executed("", &BTreeSet::new()).is_err());
    }
    #[test]
    fn missing_duplicate_extra_and_malformed_source_cases_fail_even_when_test_passes() {
        let row = json!({"consumer":"real/test/case","corpus":"source.json","id":"first","variant":"default","kind":"bounded_helper"});
        let marker = format!("A06_COMPLETED_SOURCE {row}\n");
        let expected = BTreeSet::from([serde_json::to_string(&row).unwrap()]);
        verify_source_cases(&marker, &expected).unwrap();
        for output in [
            "test real ... ok\n".to_owned(),
            format!("{marker}{marker}"),
            format!("{marker}A06_COMPLETED_SOURCE {{}}\n"),
            marker.replace("first", "extra"),
        ] {
            assert!(verify_source_cases(&output, &expected).is_err());
        }
    }
    #[test]
    fn absent_and_tampered_artifact_fail_closed() {
        let dir = std::env::temp_dir().join(format!("ssx-a06-artifact-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let row = json!({"path":"fixture","sha256":digest(b"fixed")});
        assert!(verify_artifact(&dir, &row).is_err());
        fs::write(dir.join("fixture"), b"wrong").unwrap();
        assert!(verify_artifact(&dir, &row).is_err());
        fs::write(dir.join("fixture"), b"fixed").unwrap();
        verify_artifact(&dir, &row).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}
