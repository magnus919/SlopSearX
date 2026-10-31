//! Source-bound A08 engine qualification. No interpreter or provider invocation.
use crate::a13::source_amendment::storage_document;
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
pub(crate) mod header_successor;
const COVERAGE_SHA: &str = "4bd65113eded80d605b3fede32f9cb5e97d12b47c4b1756314ed28757f653856";
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
        .ok_or("A08 missing source records")?
    {
        let id = row["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("A08 empty source ID")?;
        require(seen.insert(id.to_owned()), "A08 duplicate source ID")?;
        require(
            row["responses"].is_array() && row["requests"].is_array() && row["output"].is_object(),
            "A08 source request/outcome tuple missing",
        )?;
        ids.push(id.into())
    }
    require(!ids.is_empty(), "A08 empty source pack")?;
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
        "A08 listed tests missing, ignored, or not passed",
    )
}
fn completed_cases(output: &str, expected: &[Value]) -> Result<()> {
    let actual = output
        .lines()
        .filter_map(|line| line.strip_prefix("A08_CASE "))
        .map(|line| serde_json::from_str::<Value>(line).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>>>()?;
    require(
        actual == expected,
        "A08 completed case IDs/kinds missing, extra, duplicate, or reordered",
    )
}
fn binding(value: &Value, available: &BTreeMap<(String, String), BTreeSet<String>>) -> Result<()> {
    let package = value["package"].as_str().ok_or("A08 package missing")?;
    let target = value["target"].as_str().ok_or("A08 target missing")?;
    let test = value["qualified_name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("A08 test name missing")?;
    require(
        available
            .get(&(package.into(), target.into()))
            .is_some_and(|names| names.contains(test)),
        "A08 exact native package/target/test identity missing",
    )
}
const REGISTRATION_NODES: [&str; 5] = [
    "tests/test_fred.py::TestFREDAdapterRegistration::test_adapter_registered",
    "tests/test_new_engines.py::TestOpenFDAAdapter::test_adapter_registered",
    "tests/test_new_engines.py::TestEdgarAdapter::test_adapter_registered",
    "tests/test_new_engines.py::TestFredAdapter::test_adapter_registered",
    "tests/test_new_engines.py::TestOyezAdapter::test_adapter_registered",
];
fn function_node(node: &str) -> &str {
    node.split('[').next().unwrap_or("")
}
fn mapping_semantics(mappings: &[Value], regression: &Value) -> Result<()> {
    let results = regression["results"]
        .as_array()
        .ok_or("A08 original parameter results missing")?;
    let cases = regression["records"]
        .as_array()
        .ok_or("A08 source regression cases missing")?;
    let mut registrations = BTreeSet::new();
    for row in mappings {
        let node = row["node"].as_str().ok_or("A08 mapping node missing")?;
        let registration = REGISTRATION_NODES.contains(&node);
        let registration_label = row["mapping_status"]
            .as_str()
            .unwrap_or("")
            .starts_with("registration-only");
        require(
            !registration_label || registration,
            "A08 registration-only mapping is not a real registration node",
        )?;
        require(
            registration_label == registration,
            "A08 real registration mapping disposition differs",
        )?;
        let parameters: Vec<_> = results
            .iter()
            .filter(|r| function_node(r["node"].as_str().unwrap_or("")) == node)
            .map(|r| r["node"].clone())
            .collect();
        let source_cases: Vec<_> = cases
            .iter()
            .filter(|r| function_node(r["node"].as_str().unwrap_or("")) == node)
            .map(|r| r["id"].clone())
            .collect();
        require(
            !parameters.is_empty() && row["parameter_instances"] == json!(parameters),
            "A08 exact original parameter mapping differs",
        )?;
        require(
            row["source_case_ids"] == json!(source_cases),
            "A08 regression source cases are missing or misbound",
        )?;
        if registration {
            require(
                registrations.insert(node)
                    && source_cases.is_empty()
                    && row["native_cases"] == json!(["factory", "scope"]),
                "A08 registration-only binding differs",
            )?;
        } else {
            require(
                !source_cases.is_empty()
                    && row["native_cases"]
                        .as_array()
                        .is_some_and(|rows| rows.iter().any(|v| v == "replay")),
                "A08 behavioral regression must bind actual source adapter replay",
            )?;
        }
    }
    require(
        registrations.len() == REGISTRATION_NODES.len(),
        "A08 exact five genuine registration nodes missing",
    )?;
    let reject =
        "tests/test_oyez_relevance.py::test_case_url_rejects_untrusted_or_incomplete_identity";
    let empty =
        "tests/test_oyez_relevance.py::test_case_url_does_not_infer_identity_without_provider_url";
    let expected = [
        (reject, "https://evil.example/cases/1954/1"),
        (reject, "https://api.oyez.org/not-cases/1954/1"),
        (reject, "https://api.oyez.org/cases/1954"),
        (reject, "cases/1954/1"),
        (empty, ""),
    ];
    let assertions = regression["helper_assertions"]
        .as_array()
        .ok_or("A08 actual helper assertions missing")?;
    require(
        assertions.len() == expected.len(),
        "A08 exact five helper assertions missing",
    )?;
    for (assertion, (function, input)) in assertions.iter().zip(expected) {
        let node = if function == empty {
            function.to_owned()
        } else {
            format!("{function}[{input}]")
        };
        require(
            *assertion == json!({"node":node,"value":input,"output":""}),
            "A08 original helper input/parameter/result differs",
        )?;
        let case = cases
            .iter()
            .find(|r| r["id"] == format!("{node}#0"))
            .ok_or("A08 helper whole-adapter case missing")?;
        let body: Value = serde_json::from_str(
            case["responses"][0]["wire_body"]
                .as_str()
                .ok_or("A08 helper provider body missing")?,
        )
        .map_err(|e| e.to_string())?;
        require(
            case["node"] == node
                && case["engine"] == "oyez"
                && case["helper_assertion"] == *assertion
                && case["requests"]
                    .as_array()
                    .is_some_and(|rows| rows.len() == 1)
                && case["output"] == json!({"status":"ok","error_message":null,"results":[]})
                && body[0]["search_api_url"] == input
                && body[0]["term"] == "1954"
                && body[0]["field_docket_number"] == "1",
            "A08 helper assertion is not bound to actual whole-adapter input/outcome",
        )?;
    }
    Ok(())
}
pub fn report_scope(records: &[Value]) -> String {
    let completed = records.iter().find_map(|row| {
        Some((
            row["A08"]["actual_adapter_replays"].as_u64()?,
            row["A08"]["typed_loader_supersessions"].as_u64()?,
        ))
    });
    match completed {
        Some((adapters, loaders)) => format!("source-bound EDGAR/FRED/openFDA/Oyez factories;{adapters} adapter controls,{loaders} typed configuration supersessions,44 original functions/47 parameters,real loopback deadlines/drop and compact JSON; no provider,full portal,global RSS or deployment claim"),
        None => "source-bound EDGAR/FRED/openFDA/Oyez factories; qualification incomplete; no completed case totals claimed; no provider,full portal,global RSS or deployment claim".into(),
    }
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    header_successor::verify(root, records)?;
    let coverage_bytes =
        fs::read(root.join("docs/rust-rewrite/a08/coverage.json")).map_err(|e| e.to_string())?;
    require(
        digest(&coverage_bytes) == COVERAGE_SHA,
        "A08 coverage identity changed; audit and repin",
    )?;
    let coverage: Value = serde_json::from_slice(&coverage_bytes).map_err(|e| e.to_string())?;
    require(
        coverage["plan_id"] == "A08"
            && coverage["baseline"] == "2f78b9d02cfe62c9829fcfb38b85cd632a889de0",
        "A08 plan/baseline differs",
    )?;
    for kind in ["source_inventory", "artifacts"] {
        let mut seen = BTreeSet::new();
        for pin in coverage[kind]
            .as_array()
            .ok_or("A08 pin inventory missing")?
        {
            let path = pin["path"].as_str().ok_or("A08 pin path missing")?;
            require(seen.insert(path), "A08 duplicate pinned artifact")?;
            let bytes = fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
            if kind == "artifacts" && path == "docs/rust-rewrite/a08/README.md" {
                header_successor::verify_readme_pin(pin, &bytes)?;
                records.push(json!({"A08_pin":path,"kind":"exact frozen artifact tuple plus archived main-to-pending-UA-document successor","sha256":digest(&bytes)}));
                continue;
            }
            if kind == "source_inventory" && storage_document::is_path(path) {
                storage_document::verify(
                    path,
                    pin["sha256"].as_str().ok_or("A08 malformed source hash")?,
                    pin["bytes"]
                        .as_u64()
                        .map(usize::try_from)
                        .transpose()
                        .map_err(|_| "A08 source length overflow")?,
                    &bytes,
                    &fs::read(root.join(storage_document::archive_path(path)?))
                        .map_err(|error| error.to_string())?,
                )?;
                records.push(json!({"A08_pin":path,"kind":"exact original document archive plus reviewed storage-policy successor","sha256":digest(&bytes)}));
                continue;
            }
            if kind == "source_inventory" && path == "docs/rust-rewrite/CASE_STUDY.md" {
                require(
                    *pin == json!({"path":path,"sha256":"da92206b79b788b468fc8c0ab3b19f68fe6747d86b4f73af7d01ec362ff47a39","git_blob":"d609376fa8a8dba26de1d941a60351915f2b1687"}),
                    "A08 exact Page document caller changed",
                )?;
                crate::a13::a06_source_successor::try_verify(
                    root,
                    "docs/rust-rewrite/a08/coverage.json",
                    COVERAGE_SHA,
                    path,
                    pin["sha256"].as_str().unwrap(),
                    None,
                )
                .ok_or("A08 Page document route absent")??;
                records.push(json!({"A08_pin":path,"kind":"exact frozen source tuple plus authenticated Page inverse","sha256":digest(&bytes)}));
                continue;
            }
            require(
                pin["sha256"].as_str() == Some(digest(&bytes).as_str()),
                &format!("A08 source/artifact changed: {path}"),
            )?;
            records.push(json!({"A08_pin":path,"sha256":digest(&bytes)}));
        }
    }
    let disposition = read(root, "docs/rust-rewrite/inventory/python-disposition.json")?;
    let invocations = read(root, "docs/rust-rewrite/inventory/python-invocations.json")?;
    let receipts = read(root, "docs/rust-rewrite/a08/oracle-runs.json")?;
    let receipt_rows = receipts["records"]
        .as_array()
        .ok_or("A08 actual oracle receipts missing")?;
    require(
        receipt_rows.len() == 4,
        "A08 exactly four temporary producers must be inventoried",
    )?;
    for receipt in receipt_rows {
        let path = receipt["producer_path"]
            .as_str()
            .ok_or("A08 producer path missing")?;
        let hash = digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?);
        require(
            receipt["exit_code"] == 0 && receipt["producer_sha256"] == hash,
            "A08 source capture is not an actual successful exact producer run",
        )?;
        let entry = disposition["files"]
            .as_array()
            .ok_or("F01 dispositions missing")?
            .iter()
            .find(|r| r["path"] == path)
            .ok_or("A08 producer missing global F01 disposition")?;
        require(
            entry["sha256"] == hash
                && entry["migration_owner"] == "A08"
                && entry["migration_owners"]
                    .as_array()
                    .is_some_and(|v| v.iter().any(|x| x == "G08")),
            "A08 producer hash/retirement inventory differs",
        )?;
        require(
            invocations["developer_oracle_invocations"]
                .as_array()
                .is_some_and(|rows| {
                    rows.iter().any(|r| {
                        r["example_path"] == path
                            && r["source_sha256"] == hash
                            && r["retirement_owner"] == "G08"
                    })
                }),
            "A08 actual oracle invocation missing global retirement inventory",
        )?;
        for output in receipt["outputs"]
            .as_array()
            .ok_or("A08 capture outputs missing")?
        {
            let p = output["path"].as_str().ok_or("A08 output path missing")?;
            require(
                output["sha256"] == digest(&fs::read(root.join(p)).map_err(|e| e.to_string())?),
                "A08 capture output no longer matches execution receipt",
            )?;
        }
    }
    let unicode = read(root, "docs/rust-rewrite/a08/case16.json")?;
    require(
        unicode["records"]
            .as_array()
            .is_some_and(|rows| rows.len() == 11468)
            && unicode["unicode"] == "16.0.0",
        "A08 actual whole-string Unicode controls missing",
    )?;
    require(
        unicode["data_sha256"]
            == digest(
                &fs::read(root.join("docs/rust-rewrite/a08/case16-data.json"))
                    .map_err(|e| e.to_string())?,
            ),
        "A08 runtime Unicode data differs from source producer",
    )?;
    let source = read(root, "docs/rust-rewrite/a08/source.json")?;
    let regression = read(root, "docs/rust-rewrite/a08/regressions.json")?;
    let mut ordered = ids(&source)?;
    ordered.extend(ids(&regression)?);
    let unique: BTreeSet<_> = ordered.iter().cloned().collect();
    require(
        unique.len() == 387 && ordered.len() == 387,
        "A08 missing/duplicate primary controls",
    )?;
    require(
        coverage["ordered_case_ids"] == json!(ordered),
        "A08 ordered source controls changed",
    )?;
    let original = regression["results"]
        .as_array()
        .ok_or("A08 original test execution missing")?;
    require(
        original.len() == 47 && original.iter().all(|r| r["outcome"] == "passed"),
        "A08 original 47 parameter instances did not pass",
    )?;
    let functions: BTreeSet<_> = original
        .iter()
        .map(|r| {
            r["node"]
                .as_str()
                .unwrap_or("")
                .split('[')
                .next()
                .unwrap_or("")
        })
        .collect();
    require(
        functions.len() == 44 && !functions.contains(""),
        "A08 original 44 function mapping differs",
    )?;
    let mappings = coverage["regression_mapping"]
        .as_array()
        .ok_or("A08 regression mapping missing")?;
    let mapped: BTreeSet<_> = mappings.iter().filter_map(|r| r["node"].as_str()).collect();
    require(
        mapped == functions,
        "A08 unmapped or invented original function",
    )?;
    mapping_semantics(mappings, &regression)?;
    let differences = read(root, "docs/rust-rewrite/a08/differences.json")?;
    let mut difference_ids = BTreeSet::new();
    let mut superseded = 0;
    for row in differences["records"]
        .as_array()
        .ok_or("A08 differences missing")?
    {
        let id = row["id"].as_str().ok_or("A08 difference ID missing")?;
        require(
            unique.contains(id) && difference_ids.insert(id),
            "A08 unknown/duplicate difference",
        )?;
        require(
            [
                "native-library-diagnostic-wording",
                "finite-scalar-domain-refusal",
                "mock-timer-superseded-by-real-loopback",
                "F06-typed-config-rejection-supersession",
            ]
            .contains(&row["category"].as_str().unwrap_or("")),
            "A08 unreviewed difference category",
        )?;
        if row["category"] == "F06-typed-config-rejection-supersession" {
            superseded += 1;
            require(
                row["native_output"]["adapter_instantiated"] == false,
                "A08 malformed key must stop before instantiation",
            )?;
        }
    }
    require(
        superseded == 2 && difference_ids.len() == 72,
        "A08 exact differences/supersessions changed",
    )?;
    let mut available = BTreeMap::new();
    for (package, target) in [
        ("ssx-domain", "lib"),
        ("ssx-infra", "lib"),
        ("ssx-infra", "a08_regulatory"),
        ("ssx-infra", "a08_factories"),
        ("ssx-mcp", "a08_consumers"),
    ] {
        let mut args = vec!["test", "--offline", "--locked", "-p", package];
        if target == "lib" {
            args.push("--lib")
        } else {
            args.extend(["--test", target])
        }
        args.extend(["--", "--list"]);
        let output = command(root, "cargo", &args, records)?;
        let tests = output
            .lines()
            .filter_map(|line| line.strip_suffix(": test").map(str::to_owned))
            .collect::<BTreeSet<_>>();
        require(!tests.is_empty(), "A08 native test target empty")?;
        available.insert((package.into(), target.into()), tests);
    }
    for mapped in mappings {
        for key in mapped["native_cases"]
            .as_array()
            .ok_or("A08 native mapping missing")?
        {
            let key = key.as_str().ok_or("A08 malformed binding")?;
            binding(
                coverage["native_case_bindings"]
                    .get(key)
                    .ok_or("A08 binding missing")?,
                &available,
            )?;
        }
    }
    for (_, value) in coverage["native_case_bindings"]
        .as_object()
        .ok_or("A08 bindings missing")?
    {
        binding(value, &available)?;
    }
    let rejection_output = command(
        root,
        "cargo",
        &["test", "--offline", "--locked", "-p", "xtask", "a08::tests"],
        records,
    )?;
    execution(
        &rejection_output,
        &BTreeSet::from([
            "a08::tests::behavioral_helper_cannot_be_misclassified_as_registration_only".into(),
            "a08::tests::report_scope_uses_only_completed_case_totals".into(),
            "a08::tests::duplicate_missing_and_reordered_source_id_controls_reject".into(),
            "a08::tests::ignored_missing_and_wrong_exact_native_execution_reject".into(),
            "a08::tests::missing_duplicate_extra_reordered_or_wrong_kind_case_markers_reject"
                .into(),
            "a08::tests::wrong_package_target_and_qualified_name_reject".into(),
        ]),
    )?;
    let expected_cases:Vec<_>=ordered.iter().map(|id|json!({"id":id,"kind":if differences["records"].as_array().unwrap().iter().any(|r|r["id"]==*id&&r["category"]=="F06-typed-config-rejection-supersession"){"loader"}else{"adapter"}})).collect();
    let mut completed_adapter = 0;
    let mut completed_loader = 0;
    for ((package, target), expected) in &available {
        let mut args = vec!["test", "--offline", "--locked", "-p", package.as_str()];
        if target == "lib" {
            args.push("--lib")
        } else {
            args.extend(["--test", target.as_str()])
        }
        args.extend(["--", "--format", "pretty"]);
        if target == "a08_regulatory" {
            args.extend(["--nocapture", "--test-threads=2"]);
        }
        let output = command(root, "cargo", &args, records)?;
        execution(&output, expected)?;
        if target == "a08_regulatory" {
            completed_cases(&output, &expected_cases)?;
            for case in &expected_cases {
                if case["kind"] == "loader" {
                    completed_loader += 1
                } else {
                    completed_adapter += 1
                }
            }
        }
    }
    records.push(json!({"A08":{"source_controls":344,"original_invocations":38,"helper_derived_adapter_replays":5,"actual_adapter_replays":completed_adapter,"typed_loader_supersessions":completed_loader,"original_functions":44,"original_parameter_instances":47,"whole_string_unicode_controls":11468,"network_timeout_controls":12,"python_executed_or_required":false,"boundaries":"caller-owned F08/admission; actual compact JSON only; T03/G01 full portal HTML and T02/T06 public transport remain separate; no global RSS or deployment claim"}}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn behavioral_helper_cannot_be_misclassified_as_registration_only() {
        let coverage: Value =
            serde_json::from_str(include_str!("../../../docs/rust-rewrite/a08/coverage.json"))
                .unwrap();
        let regression: Value = serde_json::from_str(include_str!(
            "../../../docs/rust-rewrite/a08/regressions.json"
        ))
        .unwrap();
        let mappings = coverage["regression_mapping"].as_array().unwrap();
        mapping_semantics(mappings, &regression).unwrap();
        for helper in [
            "tests/test_oyez_relevance.py::test_case_url_rejects_untrusted_or_incomplete_identity",
            "tests/test_oyez_relevance.py::test_case_url_does_not_infer_identity_without_provider_url",
        ] {
            let mut wrong = mappings.clone();
            let row = wrong.iter_mut().find(|r| r["node"] == helper).unwrap();
            row["mapping_status"] = json!(
                "registration-only source node mapped to actual native factories/catalog selection"
            );
            row["source_case_ids"] = json!([]);
            row["native_cases"] = json!(["factory", "scope"]);
            assert_eq!(
                mapping_semantics(&wrong, &regression).unwrap_err(),
                "A08 registration-only mapping is not a real registration node"
            );
        }
        let mut wrong = mappings.clone();
        wrong.iter_mut().find(|r| r["node"] == "tests/test_oyez_relevance.py::test_case_url_does_not_infer_identity_without_provider_url").unwrap()["source_case_ids"] = json!([]);
        assert!(mapping_semantics(&wrong, &regression).is_err());
        let mut missing = regression.clone();
        missing["helper_assertions"].as_array_mut().unwrap().pop();
        assert!(mapping_semantics(mappings, &missing).is_err());
    }
    #[test]
    fn report_scope_uses_only_completed_case_totals() {
        assert!(report_scope(&[]).contains("qualification incomplete"));
        let scope = report_scope(&[
            json!({"A08":{"actual_adapter_replays":385,"typed_loader_supersessions":2}}),
        ]);
        assert!(scope.contains("385 adapter controls,2 typed configuration supersessions"));
        assert!(!scope.contains("285"));
    }
    #[test]
    fn duplicate_missing_and_reordered_source_id_controls_reject() {
        header_successor::verify(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
            &mut Vec::new(),
        )
        .unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for wrong in [
            json!({"path":"Cargo.unlisted","sha256":"d7383efcb512764e820ac778a5fed9b990fbf734ce75d525e077e7e905587dd3","bytes":1984,"mode":420}),
            json!({"path":"Cargo.toml","sha256":"wrong","bytes":1984,"mode":420}),
            json!({"path":"Cargo.toml","sha256":"d7383efcb512764e820ac778a5fed9b990fbf734ce75d525e077e7e905587dd3","bytes":1984,"mode":384}),
        ] {
            assert!(
                crate::a13::a06_source_successor::user_agent_manifest_prior(&root, &wrong).is_err()
            );
        }
        crate::a13::a06_source_successor::native_search::page_controls(&root).unwrap();
        crate::a13::a06_source_successor::page_proof_controls(&root).unwrap();
        let row = json!({"id":"one","responses":[],"requests":[],"output":{}});
        assert!(ids(&json!({"records":[]})).is_err());
        assert!(ids(&json!({"records":[row.clone(),row.clone()]})).is_err());
        assert!(ids(&json!({"records":[{"id":"one"}]})).is_err());
        let a = ids(
            &json!({"records":[row,json!({"id":"two","responses":[],"requests":[],"output":{}})]}),
        )
        .unwrap();
        let mut b = a.clone();
        b.reverse();
        assert!(require(a == b, "A08 ordered source controls changed").is_err());
    }
    #[test]
    fn ignored_missing_and_wrong_exact_native_execution_reject() {
        let expected = BTreeSet::from(["module::required".into()]);
        execution("test module::required ... ok\n", &expected).unwrap();
        for out in [
            "test module::required ... ignored\n",
            "test other::required ... ok\n",
            "test result: ok. 0 passed; 0 failed; 0 ignored\n",
        ] {
            assert!(execution(out, &expected).is_err());
        }
    }
    #[test]
    fn missing_duplicate_extra_reordered_or_wrong_kind_case_markers_reject() {
        let a = json!({"id":"ordinary","kind":"adapter"});
        let b = json!({"id":"raw-key","kind":"loader"});
        let expected = vec![a.clone(), b.clone()];
        completed_cases(&format!("\nA08_CASE {a}\nA08_CASE {b}\n"), &expected).unwrap();
        for actual in [
            vec![a.clone()],
            vec![a.clone(), a.clone(), b.clone()],
            vec![b.clone(), a.clone()],
            vec![a.clone(), json!({"id":"raw-key","kind":"adapter"})],
            vec![a.clone(), b.clone(), json!({"id":"extra","kind":"adapter"})],
        ] {
            let text = actual
                .iter()
                .map(|v| format!("\nA08_CASE {v}\n"))
                .collect::<String>();
            assert!(completed_cases(&text, &expected).is_err());
        }
    }
    #[test]
    fn wrong_package_target_and_qualified_name_reject() {
        let available = BTreeMap::from([(
            ("ssx-infra".into(), "a08_regulatory".into()),
            BTreeSet::from(["all_engine_source_replay".into()]),
        )]);
        for value in [
            json!({"package":"ssx-infra","target":"a08_regulatory","qualified_name":"wrong"}),
            json!({"package":"ssx-mcp","target":"a08_regulatory","qualified_name":"all_engine_source_replay"}),
            json!({"package":"ssx-infra","target":"wrong","qualified_name":"all_engine_source_replay"}),
        ] {
            assert!(binding(&value, &available).is_err());
        }
    }
}
