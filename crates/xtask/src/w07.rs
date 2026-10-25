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

use crate::a13::a06_source_successor::{
    w07_main_successor as main_successor, w07_t02_joint,
    w07_test_dependency as test_dependency_successor,
};
use main_successor::{PRIOR_SHA as MAIN_SHA, reconstruct as reconstruct_main};
const REGISTRY: &str = "docs/rust-rewrite/w07/qualification-completion-v1.json";
const ORIGINAL_REGISTRY: &str = "docs/rust-rewrite/w07/qualification.json";
const ORIGINAL_REGISTRY_SHA: &str =
    "6669d06edbe317078e815f103d8172316575183b29fb0b42be4fd240df85a4a7";
const REGISTRY_SHA: &str = "f69e8c33053cc1c70acbc0fd9b64dfc8d6b0f7d5800ca36e0c44c4de5b42c8a0";
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
fn preserve_registry(original: &Value, current: &Value) -> Result<()> {
    let mut expected = original.clone();
    expected["suites"][0]["tests"].as_array_mut().ok_or("W07 original suite identity missing")?.extend(serde_json::from_str::<Vec<Value>>("[\"receipts::service::service_contract_tests::receipt_telemetry_preserves_source_branch_order_and_terminal_status\", \"receipts::service::service_contract_tests::receipt_telemetry_never_invents_acceptance_for_refusal_or_unknown_completion\"]").map_err(|e|e.to_string())?);
    expected["suites"][1]["tests"].as_array_mut().ok_or("W07 original suite identity missing")?.extend(serde_json::from_str::<Vec<Value>>("[\"lineage::source_cycle_contract::original_cycle_duplicate_edges_and_expired_nodes_are_exact_and_deterministic\"]").map_err(|e|e.to_string())?);
    expected["suites"][6]["tests"].as_array_mut().ok_or("W07 original suite identity missing")?.extend(serde_json::from_str::<Vec<Value>>("[\"retained_snapshot_original_bounds_foreign_legacy_and_source_expiry_contracts\"]").map_err(|e|e.to_string())?);
    expected["suites"][7]["tests"].as_array_mut().ok_or("W07 original suite identity missing")?.extend(serde_json::from_str::<Vec<Value>>("[\"authenticated_receipts_emit_source_workflow_counters_on_the_shared_collector\"]").map_err(|e|e.to_string())?);
    expected["suites"][11]["tests"]
        .as_array_mut()
        .ok_or("W07 original suite identity missing")?
        .extend(
            serde_json::from_str::<Vec<Value>>(
                "[\"w07::tests::metrics_successor_requires_exact_afterimage_and_inverse\"]",
            )
            .map_err(|e| e.to_string())?,
        );
    if expected["sources"]["crates/ssx-application/src/lineage.rs"]
        != "f087610a7dce527e6f86a44e64c91815041e2af91fa93656a8321efa1cb29762"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-application/src/lineage.rs"] =
        Value::String("61ab285842203a4fd10f083c65fca788a7e5dc0a10c6c2064da69657ee8c5d07".into());
    if expected["sources"]["crates/ssx-application/src/receipts/service.rs"]
        != "c67ed8a85bf2893642e3d7c3633ba8fe75b3c57c4800627d9a5885bd5b8265a4"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-application/src/receipts/service.rs"] =
        Value::String("b2ab0d86f190ae85ae8d2b47b7f930ace5cd7650a9873c3cff6fc4422ba5141d".into());
    if expected["sources"]["crates/ssx-mcp/src/evidence/factory.rs"]
        != "3c77f1a7a3b470f72e324f33fbbf006ff256c6a707f2d9415119f520edad8b96"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-mcp/src/evidence/factory.rs"] =
        Value::String("f409c156d5a9ab76afb57f584f94c5d7eabdf47149a6a93b6474c81bf956a384".into());
    if expected["sources"]["crates/ssx-mcp/src/evidence/mod.rs"]
        != "31ca14935818a50fa3a59f8561c9e9b71caa512f165cdd7a7ab5b52c465064e6"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-mcp/src/evidence/mod.rs"] =
        Value::String("2481244dc37c4518554a76672df7eb8a45c963bd59419a4c3c5574f2b1cb35eb".into());
    if expected["sources"]["crates/ssx-mcp/src/evidence/tools/tests.rs"]
        != "e59db600245871647443c6af8995e80a0f947705224e56e2278ad435384bc98e"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-mcp/src/evidence/tools/tests.rs"] =
        Value::String("04ebdd2551b9644122357514eba5e438525441a2f3c30d9a74cadf6d88ac37bb".into());
    if expected["sources"]["crates/ssx-mcp/tests/w07_composition.rs"]
        != "d44e46295e812839e0fd43571c29942df47e16fd51513a8c496f8993faa32064"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-mcp/tests/w07_composition.rs"] =
        Value::String("d9fc3328472f8fde9804d49d369dcc0686e9a524c0c128de3d751a12bbd8127b".into());
    if expected["sources"]["crates/ssx-mcp/tests/w07_mcp.rs"]
        != "9c234756fac4aefaffe9bcee8325f8ff6d1d8e2e23949b7ca870395ccaca622e"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-mcp/tests/w07_mcp.rs"] =
        Value::String("248ee74f84deba26519770c2cdd5384da2dbf5a35d8020dbe4f41b27f4301c38".into());
    if expected["sources"]["crates/ssx-mcp/tests/w07_support/mcp.rs"]
        != "244e3509015eb7394b9fb2fc682b9cd8240c6fddb0b2a2b422ae5fa715608c2c"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-mcp/tests/w07_support/mcp.rs"] =
        Value::String("e0b6368a32e9e1c2070e5944b022d55b37b12304e373eea52051170ba5680545".into());
    if expected["sources"]["docs/rust-rewrite/a06/source-amendments/shared-successor.rs"]
        != "c0da01213b78ba25e928e6682c510ac8daea5458bedf68bb376d570501bb320e"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["docs/rust-rewrite/a06/source-amendments/shared-successor.rs"] =
        Value::String("f248a1bb993975ca268a2bfbd1fb52bbd815651a852f4045f648fb8151277045".into());
    // Normal W07/CI join: preserve the original pin and admit only the reviewed
    // merged whole-file operand, including W07's existing joint module import.
    if expected["sources"]["docs/rust-rewrite/a06/source-amendments/post-a06-generation.rs"]
        != "92ee65c35762283c69445cae87ea7d901e036078de6638d10ec0015906373109"
    {
        return Err("W07 original post-generation source pin changed".into());
    }
    expected["sources"]["docs/rust-rewrite/a06/source-amendments/post-a06-generation.rs"] =
        Value::String("6c0c9128694f6d84daa85bc97f18b0137db2b985859502dca367439d924fe80c".into());
    if expected["sources"]
        .get("crates/ssx-application/src/lib.rs")
        .is_some()
    {
        return Err("W07 new causal source was already present".into());
    }
    expected["sources"]["crates/ssx-application/src/lib.rs"] =
        Value::String("1c979b776d4f3952316b6b262813784a851d6b841f2d9d95f079e76e038d5a84".into());
    if expected["sources"]
        .get("crates/ssx-infra/src/observability/mod.rs")
        .is_some()
    {
        return Err("W07 new causal source was already present".into());
    }
    expected["sources"]["crates/ssx-infra/src/observability/mod.rs"] =
        Value::String("39dcefcdb94bb441fde39263365d25758c6262cca0bb2b4d0078be05f3ca87f9".into());
    if expected["sources"]
        .get("crates/ssx-infra/src/observability/metrics.rs")
        .is_some()
    {
        return Err("W07 new causal source was already present".into());
    }
    expected["sources"]["crates/ssx-infra/src/observability/metrics.rs"] =
        Value::String("cccca54dc139061f691d183c0b073cb479229be0c5c3990752686ce3457bc460".into());
    if expected != *current {
        return Err(
            "W07 completion inventory differs from the exact additive source/test delta".into(),
        );
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    test_dependency_successor::controls(root)?;
    // Frozen original inventory and all measured wire observations remain intact.
    let original = frozen(root, ORIGINAL_REGISTRY, ORIGINAL_REGISTRY_SHA)?;
    let registry = frozen(root, REGISTRY, REGISTRY_SHA)?;
    preserve_registry(
        &serde_json::from_slice(&original).map_err(|e| e.to_string())?,
        &serde_json::from_slice(&registry).map_err(|e| e.to_string())?,
    )?;
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
    let current = frozen(root, "crates/xtask/src/main.rs", w07_t02_joint::MAIN_SHA)?;
    let parent = w07_t02_joint::parent(
        root,
        "crates/xtask/src/main.rs",
        &current,
        w07_t02_joint::Parent::W07,
    )?;
    if reconstruct_main(&parent)? != original {
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
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let live = include_bytes!("main.rs");
        let parent = w07_t02_joint::parent(
            &root,
            "crates/xtask/src/main.rs",
            live,
            w07_t02_joint::Parent::W07,
        )
        .unwrap();
        assert_eq!(reconstruct_main(&parent).unwrap(), prior);
        let text = String::from_utf8(parent).unwrap();
        for bad in [
            text.replace("mod w07;\n", ""),
            format!("mod w07;\n{text}"),
            text.replace("mod a01;", "mod a00;"),
            format!("mod w07;\n{}", text.replace("mod w07;\n", "")),
        ] {
            assert!(reconstruct_main(bad.as_bytes()).is_err());
        }
    }
    #[test]
    fn metrics_successor_requires_exact_afterimage_and_inverse() {
        use crate::a13::a06_source_successor::metrics_predecessor;
        let path = "crates/ssx-application/src/lib.rs";
        let expected = "7d3b8126448059633426e964a1ecc9cc3c2b1fd0ba2cdc7bd75917862cf0cee4";
        let actual = include_bytes!("../../ssx-application/src/lib.rs");
        let predecessor = metrics_predecessor(path, expected, actual).unwrap();
        assert_eq!(sha(&predecessor), expected);
        assert_eq!(predecessor.len(), 5893);
        assert!(metrics_predecessor(path, expected, &predecessor).is_err());
        assert!(metrics_predecessor("unlisted.rs", expected, actual).is_err());
        assert!(metrics_predecessor(path, &"0".repeat(64), actual).is_err());
        let at = 2805;
        let insertion = b"    fn workflow_accepted(&self, _: &'static str, _: &'static str) {}\n    fn workflow_rejected(&self, _: &'static str, _: &'static str) {}\n    fn workflow_terminal(&self, _: &'static str, _: &'static str) {}\n";
        let mut duplicate = actual.to_vec();
        duplicate.splice(at..at, insertion.iter().copied());
        let mut altered = actual.to_vec();
        altered[at] ^= 1;
        let mut unrelated = actual.to_vec();
        unrelated[0] ^= 1;
        let mut moved = actual.to_vec();
        moved.drain(at..at + insertion.len());
        moved.splice(0..0, insertion.iter().copied());
        let mut extra = actual.to_vec();
        extra.push(b' ');
        for bad in [duplicate, altered, unrelated, moved, extra] {
            assert!(metrics_predecessor(path, expected, &bad).is_err());
        }
        // Assert the successor preserves every original identity and oracle.
        let original: Value = serde_json::from_slice(include_bytes!(
            "../../../docs/rust-rewrite/w07/qualification.json"
        ))
        .unwrap();
        let current: Value = serde_json::from_slice(include_bytes!(
            "../../../docs/rust-rewrite/w07/qualification-completion-v1.json"
        ))
        .unwrap();
        preserve_registry(&original, &current).unwrap();
        let mut removed = current.clone();
        removed["suites"][0]["tests"]
            .as_array_mut()
            .unwrap()
            .remove(0);
        let mut unknown = current.clone();
        unknown["sources"]["unlisted.rs"] = Value::String("0".repeat(64));
        let mut changed = current.clone();
        changed["wire_reference"]["sha256"] = Value::String("0".repeat(64));
        let mut weakened = current.clone();
        weakened["sources"]["crates/ssx-infra/src/observability/mod.rs"] =
            Value::String("0".repeat(64));
        let mut removed_metrics = current.clone();
        removed_metrics["sources"]
            .as_object_mut()
            .unwrap()
            .remove("crates/ssx-infra/src/observability/metrics.rs");
        let mut weakened_metrics = current.clone();
        weakened_metrics["sources"]["crates/ssx-infra/src/observability/metrics.rs"] =
            Value::String("0".repeat(64));
        let mut removed_post_generation = current.clone();
        removed_post_generation["sources"]
            .as_object_mut()
            .unwrap()
            .remove("docs/rust-rewrite/a06/source-amendments/post-a06-generation.rs");
        let mut weakened_post_generation = current.clone();
        weakened_post_generation["sources"]["docs/rust-rewrite/a06/source-amendments/post-a06-generation.rs"] =
            Value::String("0".repeat(64));
        for bad in [
            removed,
            unknown,
            changed,
            weakened,
            removed_metrics,
            weakened_metrics,
            removed_post_generation,
            weakened_post_generation,
        ] {
            assert!(preserve_registry(&original, &bad).is_err());
        }
        for field in [
            "schema_version",
            "plan_id",
            "scope",
            "wire_reference",
            "packaging_reference",
        ] {
            assert_eq!(original[field], current[field]);
        }
        for old in original["suites"].as_array().unwrap() {
            let matches: Vec<_> = current["suites"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| {
                    s["package"] == old["package"]
                        && s["target"] == old["target"]
                        && s["filter"] == old["filter"]
                })
                .collect();
            assert_eq!(matches.len(), 1);
            let expected = old["tests"].as_array().unwrap();
            let live = matches[0]["tests"].as_array().unwrap();
            assert!(live.starts_with(expected));
            assert_eq!(
                live.iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<BTreeSet<_>>()
                    .len(),
                live.len()
            );
        }
    }
}
