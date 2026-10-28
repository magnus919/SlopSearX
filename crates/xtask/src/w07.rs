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

use crate::a13::a06_source_successor::w07_t02_joint::snapshot_helper_amendment;
use crate::a13::a06_source_successor::{
    w07_main_successor as main_successor, w07_t02_joint,
    w07_test_dependency as test_dependency_successor,
};
use main_successor::{PRIOR_SHA as MAIN_SHA, reconstruct as reconstruct_main};
const REGISTRY: &str = "docs/rust-rewrite/w07/qualification-completion-v1.json";
const ORIGINAL_REGISTRY: &str = "docs/rust-rewrite/w07/qualification.json";
const ORIGINAL_REGISTRY_SHA: &str =
    "6669d06edbe317078e815f103d8172316575183b29fb0b42be4fd240df85a4a7";
const REGISTRY_SHA: &str = "5be08978822440ec7bece674b13647c6337eaa4636cc232a1622738a90ac1b64";
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
    if path == "crates/xtask/src/main.rs" && expected == w07_t02_joint::MAIN_SHA {
        w07_t02_joint::parent(root, path, &bytes, w07_t02_joint::Parent::W07)?;
        return Ok(bytes);
    }
    if let Some(proven) = snapshot_helper_amendment::verify(root, path, expected, &bytes) {
        return proven;
    }
    if sha(&bytes) != expected {
        if let Some(operand) =
            crate::w08::source_successor::frozen_operand(root, path, expected, &bytes)?
        {
            return Ok(operand);
        }

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
/// Called by the xtask-owned amendment test without adding W07 suite identities.
#[cfg(test)]
pub(crate) fn snapshot_fixture_main_controls(root: &Path) -> Result<()> {
    let path = "crates/xtask/src/main.rs";
    let physical = fs::read(root.join(path)).map_err(|e| e.to_string())?;
    let proven = frozen(root, path, w07_t02_joint::MAIN_SHA)?;
    if proven != physical {
        return Err("W07 main seam did not retain physical bytes".into());
    }
    let parent = w07_t02_joint::parent(root, path, &proven, w07_t02_joint::Parent::W07)?;
    reconstruct_main(&parent)?;
    if frozen(root, path, &"0".repeat(64)).is_ok() {
        return Err("W07 wrong main caller accepted".into());
    }
    let registry: Value =
        serde_json::from_slice(&fs::read(root.join(REGISTRY)).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    for path in [
        "docs/rust-rewrite/a06/source-amendments/shared-successor.rs",
        "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs",
    ] {
        let expected = registry["sources"][path]
            .as_str()
            .ok_or("W07 helper source absent")?;
        let prior = frozen(root, path, expected)?;
        if sha(&prior) != expected {
            return Err("W07 helper seam did not recover accepted operand".into());
        }
        if frozen(root, path, "wrong").is_ok() {
            return Err("W07 wrong helper caller accepted".into());
        }
    }
    snapshot_helper_amendment::controls(root)?;
    Ok(())
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
    expected["suites"][0]["tests"].as_array_mut().ok_or("W07 original suite identity missing")?.extend(serde_json::from_str::<Vec<Value>>("[\"receipts::service::service_contract_tests::receipt_telemetry_preserves_source_branch_order_and_terminal_status\", \"receipts::service::service_contract_tests::receipt_telemetry_never_invents_acceptance_for_refusal_or_unknown_completion\", \"receipts::service::service_contract_tests::receipt_read_expiry_metrics_require_expired_source_and_empty_retained_receipts\"]").map_err(|e|e.to_string())?);
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
    expected["suites"][10]["tests"]
        .as_array_mut()
        .ok_or("W07 original evidence-tools suite identity missing")?
        .push(Value::String("evidence::tools::tests::raw_any_artifacts_preserve_source_semantic_precedence_without_storage".into()));
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
        Value::String("294df04b4cab9dd64ccf48e85534b1bb7f592200dbbb1723675e4d7186c7725e".into());
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
        Value::String("01bb4fbbdc3ee06f777cb2232a608b85799c8da4b700c0faf6d5e4d250e47760".into());
    if expected["sources"]["crates/ssx-mcp/tests/w07_composition.rs"]
        != "d44e46295e812839e0fd43571c29942df47e16fd51513a8c496f8993faa32064"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-mcp/tests/w07_composition.rs"] =
        Value::String("1f952da6a11d01234edfe05ff17f0c26690151489245bafeb079d585a82865ca".into());
    if expected["sources"]["crates/ssx-mcp/tests/w07_mcp.rs"]
        != "9c234756fac4aefaffe9bcee8325f8ff6d1d8e2e23949b7ca870395ccaca622e"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-mcp/tests/w07_mcp.rs"] =
        Value::String("1cab294c93e03a933c77764edb1628b2727fde9e0dfc5cff34d721fcf1e37084".into());
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
        Value::String("7825c39c9d264b11170e3ab749249b266e5a530450d3cdcdf4ef076e4545a3e2".into());
    // Normal W07/CI join: preserve the original pin and admit only the reviewed
    // merged whole-file operand, including W07's existing joint module import.
    if expected["sources"]["docs/rust-rewrite/a06/source-amendments/post-a06-generation.rs"]
        != "92ee65c35762283c69445cae87ea7d901e036078de6638d10ec0015906373109"
    {
        return Err("W07 original post-generation source pin changed".into());
    }
    expected["sources"]["docs/rust-rewrite/a06/source-amendments/post-a06-generation.rs"] =
        Value::String("6c0c9128694f6d84daa85bc97f18b0137db2b985859502dca367439d924fe80c".into());
    if expected["sources"]["crates/ssx-mcp/src/search_surface/validation.rs"]
        != "7df42259622e459f1efa28257a6898dbf794f6cc360686be9cb10a25ec40ab0b"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-mcp/src/search_surface/validation.rs"] =
        Value::String("4163a01d3c879132e60e611f4085cd493f3310a5341bbff54fee86d90fb1af36".into());
    if expected["sources"]["crates/ssx-mcp/src/evidence/tools.rs"]
        != "c4504b7dffbaf11420350bce226c9e18559ea382316f57fc55a7469f4c12943f"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-mcp/src/evidence/tools.rs"] =
        Value::String("705362f0e083cf1a65b5b1149430231b7f0329517e53311317bc2ff91432f9b9".into());
    if expected["sources"]["crates/ssx-mcp/src/search_surface/mod.rs"]
        != "db27a4c64584f32eeabfecb7953148bbe7666f90568a3fb00ba9f3cdec4cb091"
    {
        return Err("W07 original whole-file source pin changed".into());
    }
    expected["sources"]["crates/ssx-mcp/src/search_surface/mod.rs"] =
        Value::String("1de423862041d7988ce020f801af73abd25db3f9dcdae83a3e362db58428496f".into());
    if expected["sources"]
        .get("crates/ssx-application/src/lib.rs")
        .is_some()
    {
        return Err("W07 new causal source was already present".into());
    }
    expected["sources"]["crates/ssx-application/src/lib.rs"] =
        Value::String("e328c4b5a4217acbe79a186d1c85d772fbe78ff901bad81ad07aaa458e0ffa8b".into());
    if expected["sources"]
        .get("crates/ssx-infra/src/observability/mod.rs")
        .is_some()
    {
        return Err("W07 new causal source was already present".into());
    }
    expected["sources"]["crates/ssx-infra/src/observability/mod.rs"] =
        Value::String("e32ec1a5d7b0fa487335a2fd55c142bfbf271634a87a2f1e8edcdafa53c4f032".into());
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
    let current = crate::w08::source_successor::physical(
        root,
        "crates/xtask/src/main.rs",
        w07_t02_joint::MAIN_SHA,
    )?;
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
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = "crates/ssx-application/src/lib.rs";
        let expected = "7d3b8126448059633426e964a1ecc9cc3c2b1fd0ba2cdc7bd75917862cf0cee4";
        let actual = include_bytes!("../../ssx-application/src/lib.rs");
        let predecessor = metrics_predecessor(&root, path, expected, actual).unwrap();
        assert_eq!(sha(&predecessor), expected);
        assert_eq!(predecessor.len(), 5893);
        assert!(metrics_predecessor(&root, path, expected, &predecessor).is_err());
        assert!(metrics_predecessor(&root, "unlisted.rs", expected, actual).is_err());
        assert!(metrics_predecessor(&root, path, &"0".repeat(64), actual).is_err());
        let at = 2805;
        let insertion = b"    fn workflow_accepted(&self, _: &'static str, _: &'static str) {}\n    fn workflow_rejected(&self, _: &'static str, _: &'static str) {}\n    fn workflow_terminal(&self, _: &'static str, _: &'static str) {}\n    fn workflow_expired(&self, _: &'static str, _: &'static str) {}\n";
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
            assert!(metrics_predecessor(&root, path, expected, &bad).is_err());
        }
        // Original Metrics-frame controls remain nonvacuous: prove its positive
        // operand, then mutate that frame rather than only W08's outer buffer.
        use crate::a13::a06_source_successor::metrics_frame_predecessor;
        let frame = crate::w08::source_successor::parent(&root, path, actual).unwrap();
        assert_eq!(
            metrics_frame_predecessor(path, expected, &frame).unwrap(),
            predecessor
        );
        let mut duplicate = frame.clone();
        duplicate.splice(at..at, insertion.iter().copied());
        let mut altered = frame.clone();
        altered[at] ^= 1;
        let mut unrelated = frame.clone();
        unrelated[0] ^= 1;
        let mut moved = frame.clone();
        moved.drain(at..at + insertion.len());
        moved.splice(0..0, insertion.iter().copied());
        let mut extra = frame.clone();
        extra.push(b' ');
        for bad in [duplicate, altered, unrelated, moved, extra] {
            assert!(metrics_frame_predecessor(path, expected, &bad).is_err());
        }
        assert!(metrics_frame_predecessor("unlisted.rs", expected, &frame).is_err());
        assert!(metrics_frame_predecessor(path, &"0".repeat(64), &frame).is_err());
        // This is O01's real frozen caller; every mutated physical buffer must fail.

        use crate::a13::a06_source_successor::o01_metrics_predecessor;
        let caller = include_bytes!("../../../docs/rust-rewrite/o01/coverage.json");
        let current = include_bytes!("../../ssx-infra/src/observability/mod.rs");
        let claim = (
            "docs/rust-rewrite/o01/coverage.json",
            "32da35318cb2b2e369b10975bb985aa55469bcc1a39c1549425d1723bd5dcbce",
            "crates/ssx-infra/src/observability/mod.rs",
            "a2cff0ef1848fcac8c6c7630406217e824d9a6a3b860235843fe69074c5733ae",
            None,
        );
        let prior = o01_metrics_predecessor(claim, caller, current, "0644").unwrap();
        assert_eq!(prior.len(), 20291);
        assert_eq!(sha(&prior), claim.3);
        let at = 10996;
        let end = at + 830;
        let mut duplicate = current.to_vec();
        duplicate.splice(at..at, current[at..end].iter().copied());
        let mut altered = current.to_vec();
        altered[at] ^= 1;
        let mut unrelated = current.to_vec();
        unrelated[0] ^= 1;
        let mut moved = current.to_vec();
        let span = moved.drain(at..end).collect::<Vec<_>>();
        moved.splice(0..0, span);
        for bad in [prior, duplicate, altered, unrelated, moved] {
            assert!(o01_metrics_predecessor(claim, caller, &bad, "0644").is_err());
        }
        let mut bad_caller = caller.to_vec();
        bad_caller.push(b' ');
        assert!(o01_metrics_predecessor(claim, &bad_caller, current, "0644").is_err());
        assert!(o01_metrics_predecessor(claim, caller, current, "0755").is_err());
        for bad in [
            ("unlisted.json", claim.1, claim.2, claim.3, claim.4),
            (claim.0, "wrong", claim.2, claim.3, claim.4),
            (claim.0, claim.1, "unlisted.rs", claim.3, claim.4),
            (claim.0, claim.1, claim.2, "wrong", claim.4),
            (claim.0, claim.1, claim.2, claim.3, Some(20291)),
        ] {
            assert!(o01_metrics_predecessor(bad, caller, current, "0644").is_err());
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
        // Every changed raw-artifact implementation pin remains mandatory and exact.
        for source in [
            "crates/ssx-mcp/src/search_surface/validation.rs",
            "crates/ssx-mcp/src/evidence/tools.rs",
            "crates/ssx-mcp/src/search_surface/mod.rs",
            "crates/ssx-mcp/src/evidence/tools/tests.rs",
            "crates/ssx-mcp/tests/w07_mcp.rs",
        ] {
            let mut missing = current.clone();
            missing["sources"].as_object_mut().unwrap().remove(source);
            assert!(preserve_registry(&original, &missing).is_err());
            let mut changed = current.clone();
            changed["sources"][source] = Value::String("0".repeat(64));
            assert!(preserve_registry(&original, &changed).is_err());
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
