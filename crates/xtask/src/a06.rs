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
const ADMISSION_SUCCESSOR: &str =
    "docs/rust-rewrite/a06/source-amendments/shared-admission-successor-v2.json";
const ADMISSION_SUCCESSOR_SHA: &str =
    "e9acae3d1d7cf7841d1dba2a3ccee5452a14d1b1d11734e669cff933f678a948";
const ADMISSION_PATHS: &[&str] = &[
    "crates/ssx-infra/src/adapters/arxiv_expat.rs",
    "crates/ssx-infra/src/adapters/arxiv_runtime.rs",
    "crates/ssx-infra/src/adapters/openalex_runtime.rs",
    "crates/ssx-infra/src/adapters/semantic_runtime.rs",
    "crates/ssx-infra/tests/a06_admission.rs",
    "crates/ssx-infra/tests/support/a06_timeout_clock.rs",
];
fn admission_bytes(raw: &[u8], expected: &str, row: &Value) -> Result<()> {
    if row.as_object().is_none_or(|v| v.len() != 6)
        || row["historical_sha256"] != expected
        || row["current_sha256"] != digest(raw)
        || row["current_bytes"].as_u64() != Some(raw.len() as u64)
    {
        return Err("A06 shared admission exact current/caller differs".into());
    }
    let edits = row["edits"]
        .as_array()
        .ok_or("A06 admission edits absent")?;
    if edits.is_empty() {
        return Err("A06 admission edits empty".into());
    }
    let mut original = Vec::new();
    let mut cursor = 0usize;
    let mut old_cursor = 0usize;
    for edit in edits {
        if edit.as_object().is_none_or(|v| v.len() != 3) {
            return Err("A06 admission edit shape differs".into());
        }
        let at = edit["at"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or("A06 admission edit offset invalid")?;
        let remove = edit["remove"]
            .as_str()
            .ok_or("A06 admission removal invalid")?
            .as_bytes();
        let insert = edit["insert"]
            .as_str()
            .ok_or("A06 admission insertion invalid")?
            .as_bytes();
        let unchanged = at
            .checked_sub(old_cursor)
            .ok_or("A06 admission edits overlap")?;
        let start = cursor
            .checked_add(unchanged)
            .ok_or("A06 admission offset overflow")?;
        let end = start
            .checked_add(insert.len())
            .ok_or("A06 admission offset overflow")?;
        if raw.get(start..end) != Some(insert) {
            return Err("A06 admission exact inserted bytes differ".into());
        }
        original.extend_from_slice(
            raw.get(cursor..start)
                .ok_or("A06 admission prefix absent")?,
        );
        original.extend_from_slice(remove);
        cursor = end;
        old_cursor = at
            .checked_add(remove.len())
            .ok_or("A06 admission offset overflow")?;
    }
    original.extend_from_slice(raw.get(cursor..).ok_or("A06 admission tail absent")?);
    if row["historical_bytes"].as_u64() != Some(original.len() as u64)
        || digest(&original) != expected
    {
        return Err("A06 admission edits do not reconstruct historical source".into());
    }
    Ok(())
}
fn admission_capsule(root: &Path) -> Result<Value> {
    let raw = fs::read(root.join(ADMISSION_SUCCESSOR)).map_err(|e| e.to_string())?;
    if digest(&raw) != ADMISSION_SUCCESSOR_SHA
        || digest(
            &fs::read(root.join("docs/rust-rewrite/a06/coverage.json"))
                .map_err(|e| e.to_string())?,
        ) != COVERAGE_SHA256
    {
        return Err("A06 admission capsule/frozen coverage changed".into());
    }
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let rows = doc["sources"]
        .as_array()
        .ok_or("A06 admission sources absent")?;
    let paths: BTreeSet<_> = rows.iter().filter_map(|r| r["path"].as_str()).collect();
    if doc.as_object().is_none_or(|v| v.len() != 3)
        || doc["schema"] != "A06_SHARED_ADMISSION_SUCCESSOR_V1"
        || doc["coverage_sha256"] != COVERAGE_SHA256
        || rows.len() != ADMISSION_PATHS.len()
        || paths != ADMISSION_PATHS.iter().copied().collect()
    {
        return Err("A06 admission closed source selection differs".into());
    }
    Ok(doc)
}

// One mutable cancellation fixture; published A06 coverage and old admission
// capsules remain unchanged. The existing inverse checker proves its old bytes.
const NATIVE_CANCEL_SOURCE: &str = "crates/ssx-infra/tests/a06_expat_candidate.rs";
const NATIVE_CANCEL_PRIOR: &str =
    "39bcb04f308a2556627f4105a7d2150522fffb65e803d043df6e1ab29fdc8f70";
const NATIVE_CANCEL_CAPSULE: &str =
    "docs/rust-rewrite/a06/source-amendments/native-cancel-fixture-v1.json";
const NATIVE_CANCEL_CAPSULE_SHA: &str =
    "863fcb998aae337c6246adbf619d8e8d65046c65ddb53796bec430861b9c689c";
fn native_cancel_material(root: &Path, path: &str, bound: usize) -> Result<Vec<u8>> {
    use std::io::Read as _;
    let path = root.join(path);
    let before = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !before.is_file() {
        return Err("A06 native cancellation requires regular source".into());
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let opened = file.metadata().map_err(|e| e.to_string())?;
    if !opened.is_file() {
        return Err("A06 native cancellation source changed kind".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if before.dev() != opened.dev()
            || before.ino() != opened.ino()
            || before.permissions().mode() & 0o7777 != 0o644
            || opened.permissions().mode() & 0o7777 != 0o644
        {
            return Err("A06 native cancellation source custody differs".into());
        }
    }
    let mut raw = Vec::new();
    file.take((bound + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    if raw.len() > bound || opened.len() != raw.len() as u64 {
        return Err("A06 native cancellation bounded source changed".into());
    }
    Ok(raw)
}
fn native_cancel_capsule(root: &Path) -> Result<Value> {
    let raw = native_cancel_material(root, NATIVE_CANCEL_CAPSULE, 9811)?;
    if digest(&raw) != NATIVE_CANCEL_CAPSULE_SHA {
        return Err("A06 native cancellation capsule changed".into());
    }
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    if doc["path"] != NATIVE_CANCEL_SOURCE || doc["historical_sha256"] != NATIVE_CANCEL_PRIOR {
        return Err("A06 native cancellation closed scope changed".into());
    }
    Ok(doc)
}
fn verify_artifact(root: &Path, row: &Value) -> Result<()> {
    let path = row["path"].as_str().ok_or("A06 artifact path missing")?;
    let expected = row["sha256"].as_str().ok_or("A06 artifact hash missing")?;
    if path == NATIVE_CANCEL_SOURCE {
        if expected != NATIVE_CANCEL_PRIOR {
            return Err("A06 native cancellation frozen caller differs".into());
        }
        let capsule = native_cancel_capsule(root)?;
        let raw = native_cancel_material(root, path, 18095)?;
        return admission_bytes(&raw, expected, &capsule);
    }
    if crate::snapshot_fixture_amendment::a06(root, row)?.is_some() {
        return Ok(());
    }
    if path == "docs/rust-rewrite/a13/qualification-v1.json" {
        return crate::a13::a06_source_successor::verify_current_qualification(root, row);
    }
    if ADMISSION_PATHS.contains(&path) {
        let doc = admission_capsule(root)?;
        let selected = doc["sources"]
            .as_array()
            .ok_or("A06 admission sources absent")?
            .iter()
            .find(|r| r["path"] == path)
            .ok_or("A06 admission source absent")?;
        return admission_bytes(
            &fs::read(root.join(path)).map_err(|e| e.to_string())?,
            expected,
            selected,
        );
    }
    if let Some(result) = crate::a13::a06_source_successor::try_verify(
        root,
        "docs/rust-rewrite/a06/coverage.json",
        COVERAGE_SHA256,
        path,
        expected,
        None,
    ) {
        return result;
    }
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

// Preserve historical A06 coverage and compose only the exact current library inventory.
const CURRENT_LIBRARY: &str =
    "docs/rust-rewrite/a06/source-amendments/current-library-inventory-v1.json";
const CURRENT_LIBRARY_SHA256: &str =
    "f631ecbced0d8ccec249c0d7b83e5155e892730c0597ac452c929216a254a24c";
const CURRENT_LIBRARY_NAMES_SHA256: &str =
    "373449cec9c40115ba54055f6852bc65ed75756dfd0d219f144cc189b3221839";
fn library_names(doc: &Value, key: &str) -> Result<BTreeSet<String>> {
    let rows = doc[key].as_array().ok_or("A06 library names missing")?;
    let mut names = BTreeSet::new();
    for row in rows {
        let name = row
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("A06 library name malformed")?;
        if !names.insert(name.to_owned()) {
            return Err("A06 duplicate library name".into());
        }
    }
    Ok(names)
}
fn current_library_inventory(root: &Path) -> Result<Value> {
    let raw = fs::read(root.join(CURRENT_LIBRARY)).map_err(|e| e.to_string())?;
    if digest(&raw) != CURRENT_LIBRARY_SHA256 {
        return Err("A06 current library sidecar changed".into());
    }
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    if doc["qualification"]["path"] != "docs/rust-rewrite/a13/qualification-v1.json"
        || doc["a13_source"]["path"] != "crates/xtask/src/a13.rs"
    {
        return Err("A06 library derivation source path changed".into());
    }
    verify_artifact(root, &doc["qualification"])?;
    crate::a13::a06_source_successor::a13_inventory_library_source(root, &doc["a13_source"])?;
    crate::a13::a06_source_successor::w07_evidence_successor::prove_search(root)?;
    Ok(doc)
}
fn current_tests_for_platform(group: &Value, doc: &Value, linux: bool) -> Result<BTreeSet<String>> {
    let mut expected = expected_tests_for_platform(group, linux)?;
    if group["package"] != "ssx-infra" || group["target"] != "lib" {
        return Ok(expected);
    }
    if doc["schema"] != "SLOPSEARX_A06_CURRENT_LIBRARY_INVENTORY_V1"
        || doc["frozen_coverage_sha256"] != COVERAGE_SHA256
        || doc["original_common"] != group["tests"]
        || doc["linux_extra"] != group["linux_tests"]
        || doc["current_common_count"] != 383
        || doc["current_common_sha256"] != CURRENT_LIBRARY_NAMES_SHA256
    {
        return Err("A06 current library predecessor/identity differs".into());
    }
    let mut common = expected_tests_for_platform(group, false)?;
    let added = library_names(doc, "additive_common")?;
    if common.len() != 295 || added.len() != 88 || !common.is_disjoint(&added) {
        return Err("A06 current library partition differs".into());
    }
    common.extend(added.iter().cloned());
    if digest(&serde_json::to_vec(&common).map_err(|e| e.to_string())?)
        != CURRENT_LIBRARY_NAMES_SHA256
    {
        return Err("A06 current library exact closed names differ".into());
    }
    expected.extend(added);
    if expected.len() != 383 + usize::from(linux) {
        return Err("A06 current library platform cardinality differs".into());
    }
    if !expected.insert(crate::a13::a06_source_successor::w07_evidence_successor::TEST.into()) {
        return Err("W07 additive library test duplicates historical inventory".into());
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
    let current_library = current_library_inventory(root)?;
    let mut bindings = BTreeSet::new();
    let mut completed_output = String::new();
    for group in coverage["native_groups"]
        .as_array()
        .ok_or("A06 native groups missing")?
    {
        let package = group["package"].as_str().ok_or("A06 package missing")?;
        let target = group["target"].as_str().ok_or("A06 target missing")?;
        let expected =
            current_tests_for_platform(group, &current_library, cfg!(target_os = "linux"))?;
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
    records.push(json!({"A06_current_library_inventory":{"sidecar":CURRENT_LIBRARY,"sha256":CURRENT_LIBRARY_SHA256,"historical_common":295,"additive_common":88,"current_common":384,"linux_extra":1,"w07_additive_test":crate::a13::a06_source_successor::w07_evidence_successor::TEST,"scope":"exact closed list and execution, not arbitrary superset admission"}}));
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

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let doc = current_library_inventory(&root).unwrap();
        let coverage: Value = serde_json::from_slice(
            &fs::read(root.join("docs/rust-rewrite/a06/coverage.json")).unwrap(),
        )
        .unwrap();
        let group = coverage["native_groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["package"] == "ssx-infra" && g["target"] == "lib")
            .unwrap();
        let common = current_tests_for_platform(group, &doc, false).unwrap();
        let linux = current_tests_for_platform(group, &doc, true).unwrap();
        assert_eq!(common.len(), 384);
        assert_eq!(linux.len(), 385);
        let mut historical = common.clone();
        assert!(historical.remove(crate::a13::a06_source_successor::w07_evidence_successor::TEST));
        assert_eq!(historical.len(), 383);
        assert_eq!(
            linux.difference(&common).cloned().collect::<BTreeSet<_>>(),
            library_names(&doc, "linux_extra").unwrap()
        );
        let text = common
            .iter()
            .map(|name| format!("test {name} ... ok\n"))
            .collect::<String>();
        verify_executed(&text, &common).unwrap();
        let first = common.first().unwrap();
        for bad in [
            text.replacen(&format!("test {first} ... ok\n"), "", 1),
            format!("{text}test unexpected::extra ... ok\n"),
            format!("{text}test {first} ... ok\n"),
        ] {
            assert!(verify_executed(&bad, &common).is_err());
        }
        for kind in [
            "missing",
            "duplicate",
            "substitution",
            "overlap",
            "predecessor",
            "linux",
            "schema",
        ] {
            let mut bad = doc.clone();
            match kind {
                "missing" => {
                    bad["additive_common"].as_array_mut().unwrap().pop();
                }
                "duplicate" => {
                    let name = bad["additive_common"][0].clone();
                    bad["additive_common"].as_array_mut().unwrap().push(name);
                }
                "substitution" => bad["additive_common"][0] = json!("unexpected::substitute"),
                "overlap" => bad["additive_common"][0] = group["tests"][0].clone(),
                "predecessor" => bad["original_common"][0] = json!("unexpected::predecessor"),
                "linux" => bad["linux_extra"][0] = json!("unexpected::linux"),
                "schema" => bad["schema"] = json!("another-inventory"),
                _ => unreachable!(),
            }
            for is_linux in [false, true] {
                assert!(
                    current_tests_for_platform(group, &bad, is_linux).is_err(),
                    "{kind}"
                );
            }
        }
        for (key, value) in [("package", "other"), ("target", "other")] {
            let mut other = group.clone();
            other[key] = json!(value);
            assert_eq!(
                current_tests_for_platform(&other, &doc, false).unwrap(),
                expected_tests_for_platform(&other, false).unwrap()
            );
        }
        println!(
            "A06_CURRENT_LIBRARY_INVENTORY_CONTROLS frozen295_add88_exact383_linux384_missing_extra_duplicate_substitution_and_partition_rejected=true"
        );
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
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        crate::a13::a06_source_successor::artifact_controls(&root).unwrap();
        let current = native_cancel_material(&root, NATIVE_CANCEL_SOURCE, 18095).unwrap();
        let capsule = native_cancel_capsule(&root).unwrap();
        let caller = json!({"path":NATIVE_CANCEL_SOURCE,"sha256":NATIVE_CANCEL_PRIOR});
        verify_artifact(&root, &caller).unwrap();
        assert!(
            verify_artifact(
                &root,
                &json!({"path":NATIVE_CANCEL_SOURCE,"sha256":"wrong"})
            )
            .is_err()
        );
        let mut changed = current.clone();
        changed.push(0);
        assert!(admission_bytes(&changed, NATIVE_CANCEL_PRIOR, &capsule).is_err());
        let mut rehashed = capsule.clone();
        rehashed["current_sha256"] = json!(digest(&changed));
        rehashed["current_bytes"] = json!(changed.len());
        assert!(admission_bytes(&changed, NATIVE_CANCEL_PRIOR, &rehashed).is_err());
        let mut bad_edit = capsule.clone();
        bad_edit["edits"][0]["at"] = json!(usize::MAX);
        assert!(admission_bytes(&current, NATIVE_CANCEL_PRIOR, &bad_edit).is_err());
        let isolated =
            std::env::temp_dir().join(format!("a06-native-capsule-{}", std::process::id()));
        fs::create_dir(&isolated).unwrap();
        let capsule_path = isolated.join(NATIVE_CANCEL_CAPSULE);
        fs::create_dir_all(capsule_path.parent().unwrap()).unwrap();
        assert!(native_cancel_capsule(&isolated).is_err());
        fs::write(
            &capsule_path,
            fs::read(root.join(NATIVE_CANCEL_CAPSULE)).unwrap(),
        )
        .unwrap();
        native_cancel_capsule(&isolated).unwrap();
        fs::write(&capsule_path, b"tampered").unwrap();
        assert!(native_cancel_capsule(&isolated).is_err());
        fs::remove_dir_all(isolated).unwrap();
        let doc = admission_capsule(&root).unwrap();
        for row in doc["sources"].as_array().unwrap() {
            let raw = fs::read(root.join(row["path"].as_str().unwrap())).unwrap();
            let expected = row["historical_sha256"].as_str().unwrap();
            admission_bytes(&raw, expected, row).unwrap();
            assert!(admission_bytes(&raw, "wrong-caller", row).is_err());
            let mut changed = raw.clone();
            changed.push(0);
            assert!(admission_bytes(&changed, expected, row).is_err());
            let mut edits = row.clone();
            edits["edits"][0]["at"] = json!(usize::MAX);
            assert!(admission_bytes(&raw, expected, &edits).is_err());
            let mut rehashed = row.clone();
            rehashed["current_sha256"] = json!(digest(&changed));
            rehashed["current_bytes"] = json!(changed.len());
            assert!(admission_bytes(&changed, expected, &rehashed).is_err());
        }
    }
    #[test]
    fn snapshot_fixture_amendment_actual_artifact_consumer() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let coverage: Value = serde_json::from_slice(
            &fs::read(root.join("docs/rust-rewrite/a06/coverage.json")).unwrap(),
        )
        .unwrap();
        let row = &coverage["artifacts"][139];
        assert_eq!(row["path"], json!("tests/test_coverage_pipeline_inputs.py"));
        verify_artifact(&root, row).unwrap();
        let mut wrong = row.clone();
        wrong["sha256"] = json!("wrong");
        assert!(verify_artifact(&root, &wrong).is_err());
        wrong = row.clone();
        wrong["extra"] = json!(true);
        assert!(verify_artifact(&root, &wrong).is_err());
    }
    #[test]
    fn snapshot_fixture_amendment_exact_main_custody_controls() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        crate::a13::a06_source_successor::snapshot_fixture_main_controls(&root).unwrap();
        crate::a13::a06_source_successor::w07_t02_joint::controls(&root).unwrap();
        crate::w07::snapshot_fixture_main_controls(&root).unwrap();
        for (container, pointer) in [
            ("docs/rust-rewrite/a09/coverage.json", "/artifacts/88"),
            ("docs/rust-rewrite/a12/qualification-v1.json", "/sources/20"),
        ] {
            let raw = fs::read(root.join(container)).unwrap();
            let doc: Value = serde_json::from_slice(&raw).unwrap();
            let row = doc.pointer(pointer).unwrap();
            let sha = digest(&raw);
            assert_eq!(row["path"], "crates/xtask/src/main.rs");
            assert_eq!(
                row["sha256"],
                "169f4ac8bed8bda10bdee0aa15e888eb06b924954acf304b2e4bf372cebff16c"
            );
            crate::a13::check_source_pin(
                &root,
                container,
                &sha,
                row["path"].as_str().unwrap(),
                row["sha256"].as_str().unwrap(),
                None,
            )
            .unwrap();
            assert!(
                crate::a13::check_source_pin(
                    &root,
                    container,
                    &sha,
                    row["path"].as_str().unwrap(),
                    row["sha256"].as_str().unwrap(),
                    Some(39725)
                )
                .is_err()
            );
        }
    }
}
