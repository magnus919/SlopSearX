//! Finite W07 semantic execution. A declaration or test count cannot pass this gate.
use crate::{Result, command};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[path = "../../../docs/rust-rewrite/w07/source-amendments/main_successor.rs"]
mod main_successor;
#[path = "../../../docs/rust-rewrite/w07/source-amendments/test_dependency_successor.rs"]
mod test_dependency_successor;
use main_successor::{
    CURRENT_SHA as MAIN_CURRENT_SHA, PRIOR_SHA as MAIN_SHA, reconstruct as reconstruct_main,
};
const REGISTRY: &str = "docs/rust-rewrite/w07/qualification.json";
const REGISTRY_SHA: &str = "1840c7d33b7548fa89d653a4bcf9d1d999ea29a21f9b46ad46f3c5f5cba72b49";
const MAIN_ARCHIVE: &str = "docs/rust-rewrite/w07/source-amendments/pre-d-main.rs";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Qualification {
    schema_version: u32,
    plan_id: String,
    scope: String,
    suites: Vec<Suite>,
    sources: BTreeMap<String, String>,
    wire_reference: Reference,
    packaging_reference: Reference,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Suite {
    package: String,
    target: String,
    filter: String,
    tests: Vec<String>,
}
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn frozen(root: &Path, path: &str, expected: &str) -> Result<Vec<u8>> {
    if Path::new(path).is_absolute() || path.split('/').any(|s| s == "..") {
        return Err("W07 unsafe source path".into());
    }
    let metadata = fs::symlink_metadata(root.join(path)).map_err(|e| format!("W07 {path}: {e}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!(
            "W07 source leaf must be a regular non-symlink file: {path}"
        ));
    }
    let bytes = fs::read(root.join(path)).map_err(|e| format!("W07 {path}: {e}"))?;
    if sha(&bytes) != expected {
        if path == crate::t06::lock_amendment::SOURCE_PATH
            && expected == crate::t06::lock_amendment::HISTORICAL_SHA256
        {
            crate::t06::lock_amendment::verify(root, path, expected)?;
        } else {
            return Err(format!(
                "W07 source drift requires finite reviewed reconciliation: {path}"
            ));
        }
    }
    Ok(bytes)
}
fn unique<'a>(values: impl IntoIterator<Item = &'a str>) -> Result<BTreeSet<&'a str>> {
    let mut result = BTreeSet::new();
    for name in values {
        if name.is_empty() || !result.insert(name) {
            return Err("W07 empty/duplicate case identity".into());
        }
    }
    if result.is_empty() {
        return Err("W07 empty case inventory".into());
    }
    Ok(result)
}
fn listing(output: &str, expected: &[String]) -> Result<()> {
    let actual = unique(
        output
            .lines()
            .filter_map(|line| line.strip_suffix(": test")),
    )?;
    if actual != unique(expected.iter().map(String::as_str))? {
        return Err("W07 listed case identities differ from sealed semantic suite".into());
    }
    Ok(())
}
fn completion(output: &str, expected: &[String]) -> Result<()> {
    let mut passed = Vec::new();
    for line in output.lines() {
        if let Some(row) = line.strip_prefix("test ")
            && let Some((name, status)) = row.rsplit_once(" ... ")
        {
            if status != "ok" {
                return Err(format!("W07 case not completed: {name}: {status}"));
            }
            passed.push(name);
        }
    }
    if unique(passed)? != unique(expected.iter().map(String::as_str))? {
        return Err("W07 actual case completions incomplete or unexpected".into());
    }
    let summaries: Vec<_> = output
        .lines()
        .filter(|line| line.starts_with("test result: "))
        .collect();
    let prefix = format!(
        "test result: ok. {} passed; 0 failed; 0 ignored; 0 measured; ",
        expected.len()
    );
    if summaries.len() != 1 || !summaries[0].starts_with(&prefix) {
        return Err("W07 missing/partial/ignored execution summary".into());
    }
    Ok(())
}
fn wire_completion(output: &str, expected: &[String]) -> Result<()> {
    let actual = unique(
        output
            .lines()
            .filter_map(|line| line.strip_prefix("W07_WIRE_CASE_COMPLETE ")),
    )?;
    if actual != unique(expected.iter().map(String::as_str))? {
        return Err("W07 measured wire replay case completions incomplete".into());
    }
    Ok(())
}
fn reference(root: &Path, binding: &Reference, packaging: &str) -> Result<Value> {
    let packet: Value = serde_json::from_slice(&frozen(root, &binding.path, &binding.sha256)?)
        .map_err(|e| e.to_string())?;
    if packet["schema_version"] != 1
        || packet["plan_id"] != "W07"
        || packet["kind"] != "measured_python_wire"
        || packet["versions"]
            != json!({"fastmcp":"4.0.11","mcp":"2.3.0","pydantic":"2.13.4","packaging":packaging})
        || packet["provider_calls"] != 0
        || packet["normalization"] != "none"
        || !packet["interpreter"]["version"]
            .as_str()
            .is_some_and(|v| v.starts_with("3.14.7 "))
    {
        return Err("W07 measured reference provenance/profile incomplete".into());
    }
    let controls = packet["sources"]
        .as_object()
        .ok_or("W07 measured source controls missing")?;
    if controls.is_empty() {
        return Err("W07 measured source controls empty".into());
    }
    for (path, expected) in controls {
        frozen(
            root,
            path,
            expected.as_str().ok_or("W07 source hash missing")?,
        )?;
    }
    let projection: Value = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/w07/mcp-tools-source.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if packet["tools"] != projection["tools"] {
        return Err("W07 actual Python wire descriptors differ from source projection; review difference, do not overwrite observation".into());
    }
    let cases = packet["cases"]
        .as_array()
        .ok_or("W07 reference cases missing")?;
    unique(
        cases
            .iter()
            .map(|row| row["id"].as_str().unwrap_or_default()),
    )?;
    for row in cases {
        if row["dispatch_delta"] != 0 || row.get("result").is_none() {
            return Err("W07 incomplete/dispatching reference case".into());
        }
    }
    Ok(packet)
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    test_dependency_successor::controls(root)?;
    let registry = frozen(root, REGISTRY, REGISTRY_SHA)?;
    let qualification: Qualification =
        serde_json::from_slice(&registry).map_err(|e| e.to_string())?;
    if qualification.schema_version != 1
        || qualification.plan_id != "W07"
        || qualification.scope.is_empty()
        || qualification.suites.is_empty()
        || qualification.sources.is_empty()
    {
        return Err("W07 invalid qualification registry".into());
    }
    for (path, digest) in &qualification.sources {
        frozen(root, path, digest)?;
    }
    let original = frozen(root, MAIN_ARCHIVE, MAIN_SHA)?;
    frozen(root, "crates/xtask/src/main.rs", MAIN_CURRENT_SHA)?;
    if reconstruct_main(
        &fs::read(root.join("crates/xtask/src/main.rs")).map_err(|e| e.to_string())?,
    )? != original
    {
        return Err("W07 main source capsule mismatch".into());
    }
    let oracle = reference(root, &qualification.wire_reference, "26.2")?;
    let successor = reference(root, &qualification.packaging_reference, "26.3")?;
    for field in [
        "baseline",
        "sources",
        "producer_sha256",
        "bindings_sha256",
        "interpreter",
        "tools",
        "full_catalog",
        "cases",
        "provider_calls",
        "normalization",
    ] {
        if oracle[field] != successor[field] {
            return Err(format!("W07 packaging comparison changed measured {field}"));
        }
    }
    command(root, "cargo", &["fmt", "--all", "--check"], records)?;
    let packages = [
        "-p",
        "ssx-application",
        "-p",
        "ssx-infra",
        "-p",
        "ssx-mcp",
        "-p",
        "xtask",
        "--all-targets",
    ];
    command(
        root,
        "cargo",
        &[&["check", "--offline", "--locked"][..], &packages].concat(),
        records,
    )?;
    command(
        root,
        "cargo",
        &[
            &["clippy", "--offline", "--locked"][..],
            &packages,
            &["--", "-D", "warnings"],
        ]
        .concat(),
        records,
    )?;
    let mut identities = BTreeSet::new();
    for suite in &qualification.suites {
        unique(suite.tests.iter().map(String::as_str))?;
        for test in &suite.tests {
            if !identities.insert(format!("{}::{}::{test}", suite.package, suite.target)) {
                return Err("W07 duplicate semantic binding".into());
            }
        }
        let mut args = vec!["test", "--offline", "--locked", "-p", &suite.package];
        if suite.target == "lib" {
            args.push("--lib");
        } else if let Some(bin) = suite.target.strip_prefix("bin:") {
            args.extend(["--bin", bin]);
        } else {
            args.extend(["--test", &suite.target]);
        }
        if !suite.filter.is_empty() {
            args.push(&suite.filter);
        }
        args.push("--");
        let listed = command(
            root,
            "cargo",
            &[args.as_slice(), &["--list"]].concat(),
            records,
        )?;
        listing(&listed, &suite.tests)?;
        let executed = command(
            root,
            "cargo",
            &[args.as_slice(), &["--show-output", "--test-threads=1"]].concat(),
            records,
        )?;
        completion(&executed, &suite.tests)?;
        if suite.target == "w07_wire_reference" {
            let expected: Vec<_> = oracle["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["id"].as_str().unwrap().to_owned())
                .collect();
            wire_completion(&executed, &expected)?;
        }
        for test in &suite.tests {
            records.push(json!({"plan_id":"W07","case_id":format!("{}::{}::{test}",suite.package,suite.target),"status":"completed","authority":"actual exact named libtest success; sealed assertion source and fixtures"}));
        }
    }
    records.push(json!({"plan_id":"W07","status":"passed","registry_sha256":REGISTRY_SHA,"wire_reference_sha256":qualification.wire_reference.sha256,"actual_case_identities":identities,"measured_wire_case_identities":oracle["cases"].as_array().unwrap().iter().map(|row|row["id"].clone()).collect::<Vec<_>>(),"provider_calls":0,"scope":qualification.scope,"limitations":"finite scoped semantic/real private storage/protocol qualification only; production startup, portal integration, publication, issue closure and G08 retirement remain separate"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn names() -> Vec<String> {
        vec!["semantic".into()]
    }
    fn output() -> String {
        "test semantic ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n".into()
    }
    #[test]
    fn case_completion_rejects_missing_ignored_duplicate_and_extra_tests() {
        assert!(completion(&output(), &names()).is_ok());
        for bad in [
            String::new(),
            output().replace("test semantic ... ok\n", ""),
            output().replace("... ok", "... ignored"),
            output().replace("1 passed", "0 passed"),
            output().replace("0 ignored", "1 ignored"),
            format!("test semantic ... ok\n{}", output()),
            format!("test other ... ok\n{}", output()),
        ] {
            assert!(completion(&bad, &names()).is_err(), "{bad}");
        }
    }
    #[test]
    fn wire_completion_requires_all_exact_measured_case_identities() {
        assert!(wire_completion("W07_WIRE_CASE_COMPLETE semantic\n", &names()).is_ok());
        for bad in [
            "",
            "W07_WIRE_CASE_COMPLETE other\n",
            "W07_WIRE_CASE_COMPLETE semantic\nW07_WIRE_CASE_COMPLETE semantic\n",
            "W07_WIRE_CASE_COMPLETE semantic\nW07_WIRE_CASE_COMPLETE other\n",
        ] {
            assert!(wire_completion(bad, &names()).is_err());
        }
    }
    #[test]
    fn listing_requires_exact_unique_case_identities() {
        assert!(listing("semantic: test\n", &names()).is_ok());
        for bad in [
            "",
            "other: test\n",
            "semantic: test\nsemantic: test\n",
            "semantic: test\nother: test\n",
        ] {
            assert!(listing(bad, &names()).is_err());
        }
    }
    #[test]
    fn frozen_sources_reject_nonregular_and_symlink_leaves() {
        use std::os::unix::fs::symlink;
        struct Owned(std::path::PathBuf);
        impl Drop for Owned {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Owned(
            std::env::temp_dir().join(format!("w07-identity-{}-{suffix}", std::process::id())),
        );
        fs::create_dir(&root.0).unwrap();
        fs::write(root.0.join("regular"), b"known").unwrap();
        let digest = sha(b"known");
        assert!(frozen(&root.0, "regular", &digest).is_ok());
        symlink("regular", root.0.join("alias")).unwrap();
        assert!(frozen(&root.0, "alias", &digest).is_err());
        fs::create_dir(root.0.join("directory")).unwrap();
        assert!(frozen(&root.0, "directory", &digest).is_err());
    }
    #[test]
    fn test_dependency_successor_requires_exact_lock_manifest_and_modes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        test_dependency_successor::controls(root).unwrap();
    }
    #[test]
    fn main_successor_requires_exact_finite_insertions() {
        let prior =
            include_bytes!("../../../docs/rust-rewrite/w07/source-amendments/pre-d-main.rs");
        assert_eq!(sha(prior), MAIN_SHA);
        let live = include_bytes!("main.rs");
        assert_eq!(reconstruct_main(live).unwrap(), prior);
        let text = String::from_utf8(live.to_vec()).unwrap();
        for bad in [
            text.replace("mod w07;\n", ""),
            format!("mod w07;\n{text}"),
            text.replace("mod a01;", "mod a00;"),
            format!("mod w07;\n{}", text.replace("mod w07;\n", "")),
        ] {
            assert!(reconstruct_main(bad.as_bytes()).is_err());
        }
    }
}
