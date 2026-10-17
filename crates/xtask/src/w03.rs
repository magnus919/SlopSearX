//! Source-bound caller-directed planning qualification. Never starts Python.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const BASELINE: &str = "87af8096d46359241421ac747ed6aa1ad07f38ec";
const EVIDENCE: &[(&str, &str)] = &[
    (
        "docs/rust-rewrite/w03/error-envelope-map.json",
        "8703828b8f68312f96b455403636da880e56cc0c91ee640403c3cd4f2cd92edb",
    ),
    (
        "docs/rust-rewrite/w03/deadline-local-stdlib-observation-001.json",
        "86019990f526d1080d5e5ac010f5f0af40b53efd4359150caf79a7b13e6f080e",
    ),
    (
        "docs/rust-rewrite/w03/capture_contract_map.py.frozen",
        "64bc20910dfe15656568215bec125d44aef016da73a0c6ad5cf94b855617b611",
    ),
    (
        "docs/rust-rewrite/w03/source-observation-001.json",
        "2fa8b498a07dd5bd923df1354dd776036dd29ec2aa49c512a950202c01d18e5b",
    ),
    (
        "docs/rust-rewrite/w03/template-observation-001.json",
        "9ed3d6a584d1e610a569b6f2065ce301659cb474acd9e63550744e9e1922bbe0",
    ),
    (
        "docs/rust-rewrite/w03/deadline-observation-001.json",
        "6a9a5896ab736758d6c16533bea5259b10628cf27a0e41ed6bf553571baae399",
    ),
    (
        "docs/rust-rewrite/w03/oracle-pins.json",
        "20ccdc5ce88182c601030a3c884e9248a8869b85c480f98a7f90ceb53573d0f4",
    ),
    (
        "docs/rust-rewrite/w03/template-oracle-pins.json",
        "7cbda8a4ebfce49da29352cc7e0f987f1a83f8177f666c9ca33d544eb9a5b1ce",
    ),
    (
        "docs/rust-rewrite/w03/deadline-oracle-pins.json",
        "d80d1227ea36138ddc0dc62f449b04baf1ddcc367169f19bdaf7a4294d3b16ca",
    ),
    (
        "docs/rust-rewrite/w03/oracle-receipt.json",
        "e905aa1d84968b0883fe8237656379dc778aa509a7bba918aa2aea8ad9d1c74a",
    ),
    (
        "docs/rust-rewrite/w03/regression-map.json",
        "553b4061d33466f980b7df36ed3e2b7233d8bbe20d64aa2908885cdc37aa2ca4",
    ),
    (
        "docs/rust-rewrite/w03/native-tests.json",
        "83518f20eedf04d3cb1184f8239197291ad8f926aee772e84daf07f14a74ff86",
    ),
    (
        "docs/rust-rewrite/w03/capture_source.py.frozen",
        "601060feda97d852cf4e9d3a5ead688b66c4efd24a7c38fd1a521785c7323fc4",
    ),
    (
        "docs/rust-rewrite/w03/capture_templates.py.frozen",
        "2964020598aed26efd79b1a9fa956810d2a1a6c6175de32108cc8367ebb44ce7",
    ),
    (
        "docs/rust-rewrite/w03/capture_deadlines.py.frozen",
        "a622a0192d858197d0603665d03a76e0e8d4e7a29469019d5eace42d86cb9513",
    ),
    (
        "docs/rust-rewrite/w03/archival-location.json",
        "1b0b5d75aa978ca291c64df2db7ee18d4622dc08f90932372fa65c1bc4de95a8",
    ),
    (
        "docs/rust-rewrite/w03/ownership.json",
        "5bc7b842d39f0beb1532b60e95b1e5123e6f5539df8cf234520b86673b61d48e",
    ),
    (
        "docs/rust-rewrite/w03/differences.json",
        "244826a3dd34cec98088670e7866a1e82626ec8e0b00a6e8779afb9276716c4f",
    ),
    (
        "docs/rust-rewrite/w03/evidence/guard-log-manifest.json",
        "67bcc7e5820ed7606e78a0bc4c65a72f354ffce3251986a37ad6cb0c5c840f04",
    ),
];

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
fn array<'a>(v: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    v[field]
        .as_array()
        .ok_or_else(|| format!("W03 missing {field}"))
}
fn validate(map: &Value, inventory: &Value) -> Result<()> {
    if map["plan_id"] != "W03" || map["baseline"] != BASELINE || inventory["plan_id"] != "W03" {
        return Err("W03 provenance mismatch".into());
    }
    let mut names = BTreeSet::new();
    let mut suites = BTreeSet::new();
    for suite in array(inventory, "suites")? {
        let package = suite["package"].as_str().ok_or("W03 package absent")?;
        let target = suite["target"].as_str().ok_or("W03 target absent")?;
        if !suites.insert((package, target)) {
            return Err("W03 duplicate suite".into());
        }
        for name in array(suite, "tests")? {
            let name = name.as_str().ok_or("W03 malformed native case")?;
            if !names.insert(format!("{package}/{target}/{name}")) {
                return Err("W03 duplicate native case".into());
            }
        }
    }
    if suites.len() != 7 {
        return Err("W03 missing required suite".into());
    }
    let tests = array(map, "tests")?;
    let functions = array(map, "functions")?;
    if tests.len() != 52 || functions.len() != 25 || array(map, "sources")?.len() != 14 {
        return Err("W03 incomplete source inventory".into());
    }
    let mut identities = BTreeSet::new();
    let mut assertions = BTreeSet::new();
    for row in tests {
        let path = row["path"].as_str().ok_or("W03 source path absent")?;
        let name = row["function"].as_str().ok_or("W03 function absent")?;
        if !identities.insert((path, name)) {
            return Err("W03 duplicate source function".into());
        }
        let cases = array(row, "native_cases")?;
        let differences = array(row, "intentional_differences")?;
        if cases.is_empty()
            && !(name == "test_rate_limit_error_remains_a_compatibility_export"
                && differences == &vec![json!("W03-D04")])
        {
            return Err("W03 unmapped original behavior".into());
        }
        for case in cases {
            if !case.as_str().is_some_and(|s| names.contains(s)) {
                return Err("W03 native binding absent from executed inventory".into());
            }
        }
        for assertion in array(row, "assertion_families")? {
            let hash = assertion["ast_sha256"]
                .as_str()
                .ok_or("W03 assertion identity absent")?;
            let line = assertion["line"]
                .as_u64()
                .ok_or("W03 assertion location absent")?;
            if hash.len() != 64 || !assertions.insert((path, line, hash)) {
                return Err("W03 duplicate/malformed assertion family".into());
            }
        }
        array(row, "parameterizations")?;
    }
    if assertions.len() != 314 {
        return Err("W03 original assertion family missing".into());
    }
    Ok(())
}
fn validate_envelopes(map: &Value, inventory: &Value) -> Result<()> {
    if map["plan_id"] != "W03"
        || array(map, "functions")?.len() != 30
        || array(map, "source_pins")?.len() != 5
    {
        return Err("W03 incomplete business-envelope source map".into());
    }
    let names: BTreeSet<_> = array(inventory, "suites")?
        .iter()
        .flat_map(|s| s["tests"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    let check_names = |row: &Value| -> Result<()> {
        let tests = array(row, "native_tests")?;
        if tests.is_empty()
            || tests
                .iter()
                .any(|v| v.as_str().is_none_or(|n| !names.contains(n)))
        {
            return Err("W03 unbound business-envelope control".into());
        }
        Ok(())
    };
    let mut functions = BTreeSet::new();
    let mut sites = BTreeSet::new();
    let mut strings = 0;
    for row in array(map, "functions")? {
        let path = row["path"].as_str().ok_or("W03 envelope path absent")?;
        let name = row["function"]
            .as_str()
            .ok_or("W03 envelope function absent")?;
        if !functions.insert((path, name))
            || row["function_ast_sha256"]
                .as_str()
                .is_none_or(|s| s.len() != 64)
        {
            return Err("W03 duplicate/unidentified envelope source function".into());
        }
        check_names(row)?;
        for site in array(row, "sites")? {
            let line = site["line"]
                .as_u64()
                .ok_or("W03 envelope source location absent")?;
            if site["id"] != format!("{name}:{line}")
                || !sites.insert((path, line))
                || site["source"].as_str().is_none_or(str::is_empty)
                || site["qualification"].as_str().is_none_or(str::is_empty)
            {
                return Err("W03 missing/duplicate envelope branch identity".into());
            }
            check_names(site)?;
        }
        if let Some(refusals) = row["string_refusals"].as_array() {
            for refusal in refusals {
                check_names(refusal)?;
                strings += 1;
            }
        }
    }
    if sites.len() != 125 || strings != 9 {
        return Err("W03 envelope source branch missing".into());
    }
    Ok(())
}
fn validate_listing(output: &str, expected: &[Value]) -> Result<()> {
    let actual: Vec<_> = output
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .collect();
    let expected: Vec<_> = expected.iter().map(|v| v.as_str().unwrap_or("")).collect();
    let distinct: BTreeSet<_> = actual.iter().copied().collect();
    if distinct.len() != actual.len()
        || distinct != expected.iter().copied().collect()
        || expected.is_empty()
    {
        return Err("W03 test inventory missing, duplicate or changed".into());
    }
    Ok(())
}
fn freeze(root: &Path, inventory: &Value) -> Result<Vec<(String, String)>> {
    let mut paths: BTreeSet<String> = array(inventory, "suites")?
        .iter()
        .map(|v| v["path"].as_str().unwrap().into())
        .collect();
    // Freeze original source identities as well as native execution inputs:
    // source parity cannot remain qualified if a backend input drifts mid-run.
    let originals = read(root, "docs/rust-rewrite/w03/oracle-pins.json")?;
    for row in array(&originals, "sources")? {
        paths.insert(
            row["path"]
                .as_str()
                .ok_or("W03 original path absent")?
                .to_owned(),
        );
    }
    paths.extend(EVIDENCE.iter().map(|(path, _)| (*path).to_owned()));
    paths.extend(
        [
            "Cargo.lock",
            "crates/ssx-infra/src/research_planning.rs",
            "crates/ssx-infra/src/search.rs",
            "crates/ssx-application/src/policy.rs",
            "crates/ssx-domain/src/research.rs",
            "crates/ssx-infra/src/research_runner.rs",
            "crates/ssx-infra/src/valkey/research.rs",
            "crates/ssx-infra/src/valkey/scripts.rs",
            "crates/ssx-mcp/src/research_planning.rs",
            "crates/ssx-mcp/src/bin/planning-qualification.rs",
            "crates/ssx-mcp/tests/w03_support/mod.rs",
            "crates/xtask/src/w03.rs",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    paths
        .into_iter()
        .map(|p| {
            fs::read(root.join(&p))
                .map(|raw| (p, digest(&raw)))
                .map_err(|e| e.to_string())
        })
        .collect()
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    for (path, expected) in EVIDENCE {
        if digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?) != *expected {
            return Err(format!("W03 frozen evidence changed: {path}"));
        }
    }
    let directory = "docs/rust-rewrite/w03";
    let map = read(root, &format!("{directory}/regression-map.json"))?;
    let inventory = read(root, &format!("{directory}/native-tests.json"))?;
    validate(&map, &inventory)?;
    let envelopes = read(root, &format!("{directory}/error-envelope-map.json"))?;
    validate_envelopes(&envelopes, &inventory)?;
    let mut finite_negative_controls = 0usize;
    let mut missing_envelope = envelopes.clone();
    missing_envelope["functions"].as_array_mut().unwrap().pop();
    let mut duplicate_envelope = envelopes.clone();
    duplicate_envelope["functions"][0] = envelopes["functions"][1].clone();
    let mut unbound_envelope = envelopes.clone();
    unbound_envelope["functions"][0]["native_tests"] = json!(["not_a_test"]);
    for bad in [missing_envelope, duplicate_envelope, unbound_envelope] {
        if validate_envelopes(&bad, &inventory).is_ok() {
            return Err("W03 negative envelope-binding control accepted".into());
        }
        finite_negative_controls += 1;
    }
    for source in array(&envelopes, "source_pins")? {
        let path = source["path"]
            .as_str()
            .ok_or("W03 envelope pin path absent")?;
        if Some(digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?).as_str())
            != source["sha256"].as_str()
        {
            return Err(format!("W03 envelope source changed: {path}"));
        }
    }
    // Finite negative controls run against the same validator, without bypassing identity checks on disk.
    let mut missing = map.clone();
    missing["tests"].as_array_mut().unwrap().pop();
    let mut duplicate = map.clone();
    duplicate["tests"].as_array_mut().unwrap()[1] = map["tests"][0].clone();
    let mut unbound = map.clone();
    unbound["tests"].as_array_mut().unwrap()[1]["native_cases"] =
        json!(["ssx-infra/w03/not_a_test"]);
    let mut missing_assertion = map.clone();
    missing_assertion["tests"].as_array_mut().unwrap()[1]["assertion_families"]
        .as_array_mut()
        .unwrap()
        .pop();
    for bad in [missing, duplicate, unbound, missing_assertion] {
        if validate(&bad, &inventory).is_ok() {
            return Err("W03 negative binding control was accepted".into());
        }
        finite_negative_controls += 1;
    }
    let example = array(&inventory, "suites")?[0]["tests"].as_array().unwrap();
    if validate_listing("", example).is_ok()
        || validate_listing("one: test\none: test", &[json!("one")]).is_ok()
    {
        return Err("W03 missing/duplicate test control accepted".into());
    }
    finite_negative_controls += 2;
    let pins = read(root, &format!("{directory}/oracle-pins.json"))?;
    if pins["baseline"] != BASELINE || array(&pins, "sources")?.len() != 315 {
        return Err("W03 oracle source provenance mismatch".into());
    }
    let mut pinned = BTreeSet::new();
    for row in array(&pins, "sources")? {
        let path = row["path"].as_str().ok_or("W03 pin path absent")?;
        if !pinned.insert(path)
            || Some(digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?).as_str())
                != row["sha256"].as_str()
        {
            return Err(format!("W03 original source drift: {path}"));
        }
    }
    for row in array(&map, "sources")? {
        let path = row["path"].as_str().ok_or("W03 path absent")?;
        let blob = command(
            root,
            "git",
            &["rev-parse", &format!("{BASELINE}:{path}")],
            records,
        )?;
        if Some(blob.trim()) != row["git_blob"].as_str()
            || Some(digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?).as_str())
                != row["sha256"].as_str()
        {
            return Err(format!("W03 source binding drift: {path}"));
        }
    }
    let executable =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER").unwrap_or_else(|_| "valkey-server".into());
    if !command(root, &executable, &["--version"], records)?.starts_with("Valkey server v=8.1.10 ")
    {
        return Err("W03 requires private pinned Valkey8.1.10".into());
    }
    let before = freeze(root, &inventory)?;
    for suite in array(&inventory, "suites")? {
        let package = suite["package"].as_str().unwrap();
        let target = suite["target"].as_str().unwrap();
        let tests = array(suite, "tests")?;
        let listing = command(
            root,
            "env",
            &[
                "TZ=America/New_York",
                "cargo",
                "test",
                "--locked",
                "-p",
                package,
                "--test",
                target,
                "--",
                "--list",
            ],
            records,
        )?;
        validate_listing(&listing, tests)?;
        let output = command(
            root,
            "env",
            &[
                "TZ=America/New_York",
                "cargo",
                "test",
                "--locked",
                "-p",
                package,
                "--test",
                target,
                "--",
                "--test-threads=1",
            ],
            records,
        )?;
        let marker = format!(
            "test result: ok. {} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;",
            tests.len()
        );
        if !output.lines().any(|l| l.starts_with(&marker))
            || tests.iter().any(|n| {
                !output
                    .lines()
                    .any(|l| l == format!("test {} ... ok", n.as_str().unwrap()))
            })
        {
            return Err(format!("W03 incomplete execution: {package}/{target}"));
        }
    }
    if before != freeze(root, &inventory)? {
        return Err("W03 candidate source changed during qualification".into());
    }
    let guards = read(
        root,
        &format!("{directory}/evidence/guard-log-manifest.json"),
    )?;
    let mut guard_paths = BTreeSet::new();
    for row in array(&guards, "logs")? {
        let path = row["artifact"]
            .as_str()
            .ok_or("W03 guard log path absent")?;
        if !guard_paths.insert(path) {
            return Err("W03 duplicate guard log".into());
        }
        let raw = fs::read(root.join(path)).map_err(|e| e.to_string())?;
        let content = std::str::from_utf8(&raw).map_err(|e| e.to_string())?;
        let refusal = if path.contains("-missing.log") {
            "explicit --allow-original-oracle required before imports"
        } else {
            "refusing existing output before imports"
        };
        if Some(digest(&raw).as_str()) != row["sha256"].as_str()
            || row["bytes"].as_u64() != Some(raw.len() as u64)
            || !content.contains(refusal)
        {
            return Err(format!("W03 actual guard log mismatch: {path}"));
        }
    }
    if guard_paths.len() != 12 {
        return Err("W03 actual guard logs missing".into());
    }
    let preview = read(root, &format!("{directory}/source-observation-001.json"))?;
    if preview["provider_calls"] != 0
        || preview["writes"] != 0
        || array(&preview, "tools")?.len() != 9
        || array(&preview, "observations")?.len() != 21
    {
        return Err("W03 oracle accounting mismatch".into());
    }
    records.push(json!({"plan_id":"W03","status":"passed","baseline":BASELINE,"source_functions":array(&map,"functions")?.len(),"original_test_functions":array(&map,"tests")?.len(),"assertion_families":array(&map,"tests")?.iter().map(|t|t["assertion_families"].as_array().map_or(0,Vec::len)).sum::<usize>(),"suites":array(&inventory,"suites")?.len(),"native_tests":array(&inventory,"suites")?.iter().map(|s|s["tests"].as_array().map_or(0,Vec::len)).sum::<usize>(),"finite_negative_controls":finite_negative_controls,"error_envelope_source_functions":array(&envelopes,"functions")?.len(),"error_construction_and_forwarding_sites":array(&envelopes,"functions")?.iter().map(|f|f["sites"].as_array().map_or(0,Vec::len)).sum::<usize>(),"dispatch_string_refusals":array(&envelopes,"functions")?.iter().map(|f|f["string_refusals"].as_array().map_or(0,Vec::len)).sum::<usize>(),"historical_actual_preimport_refusals_verified":12,"additional_stdlib_only_timezone_primitives":2,"source_freeze":before,"timezone":"America/New_York","authority":"actual shared service, private pinned Valkey, native HTTP/SSE/stdio transports","limitations":"start.source artifact composition and final production registry integration remain explicit W09 dependency; no Python runtime or autonomous stopping"}));
    Ok(())
}
