//! Frozen source outcomes plus actual native semantic case completions. No Python.
use crate::{Result, command};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

const BASELINE: &str = "9b7d888da1ee3f7e2ed045c2185ea16f419381e9";
const CORPUS_SHA: &str = "98116bc1010da4390d2b4e48775d6b3c8e1df63a41568a344013f4746c2fcbd3";
const EXPORTER_SHA: &str = "e7426b72d8a70a1668a9e5a123f3e821ab34b81c926b8f5f824dbf6be40f2594";
const REGRESSION_MAP_SHA: &str = "1f8033728cef0aaf5b687f88c8ec414ed5e9bdb1133d91c9b5053972a0938af5";
const COVERAGE_SHA: &str = "2a03778871e35de5319b21db722c8a4679d6f0c8744d393f76ee6f89052ac7fb";
const LANGUAGE_SOURCE_SHA: &str =
    "0f4931fb02c53f706b56bdcd6c4d9d76e28b72da63ecad9c42980aa001e3ccd4";
const LANGUAGE_PRODUCER_SHA: &str =
    "46cf840c30c1d36b0a69bcd20d71b96b693a54069cd721ef86f06ca59f179722";
const RETAINED_SOURCE_SHA: &str =
    "108ecf64651cf95adcdaab200577e88cdc662e5ebaaabc1dfdf9e35758a9bd61";
const RETAINED_PRODUCER_SHA: &str =
    "301fcf030f0a582923ca15635894bd637f21481b942318a3809987d267800e3c";
const SAVED_AUTHORITY_SOURCE_SHA: &str =
    "7b3f1375ee478af9fbb7aaa8e975390ee5c5427e8beab976b41955a54922f079";
const SAVED_AUTHORITY_PRODUCER_SHA: &str =
    "b40e1f04bb1101b64d807721d1ae64c25cf99299595347e3d32d7a434cfe0794";
const REVIEW_PUBLIC_SOURCE_SHA: &str =
    "0d43f086c5c98a9bee7bd5339dc80d1ea7801147d7fa6e4be4c71263a5c86a1e";
const REVIEW_PUBLIC_PRODUCER_SHA: &str =
    "9a9e01b55290a741c6795b9eb1cfa09e0cf7b44c79d4156e0ececba71e887fce";
const HUGE_INTEGER_SOURCE_SHA: &str =
    "4bce3df0415a2c49b3df348bf997462864af20efa984d5ca574650ee024c8487";
const HUGE_INTEGER_PRODUCER_SHA: &str =
    "4b3a5e6fb340d3be8f71fab2b45904092482928c8daf6c81d102948aa68d5624";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Qualification {
    schema_version: u32,
    plan_id: String,
    source_cases: Vec<String>,
    native_cases: Vec<String>,
    reference_controls: Vec<String>,
    owned_sources: BTreeMap<String, String>,
    targets: Vec<Target>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    package: String,
    target: String,
    tests: Vec<Test>,
    worker_probes: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Test {
    name: String,
    cases: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Completion {
    case_id: String,
    test: String,
}
// Only identity metadata is consumed here. Deserializing ignored input fields
// directly into serde_json::Value would reject exact integers beyond f64 range.
#[derive(Deserialize, Serialize)]
struct SourceIdentityPacket {
    schema_version: u32,
    plan_id: String,
    baseline: String,
    source_controls: BTreeMap<String, String>,
    provider_calls: u64,
    cases: Vec<SourceCaseIdentity>,
}
#[derive(Deserialize, Serialize)]
struct SourceCaseIdentity {
    id: String,
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
fn frozen(root: &Path, path: &str, expected: &str) -> Result<Vec<u8>> {
    let bytes = fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
    if digest(&bytes) != expected {
        return Err(format!("W06 frozen identity changed: {path}"));
    }
    Ok(bytes)
}
fn unique<'a>(
    values: impl IntoIterator<Item = &'a str>,
    description: &str,
) -> Result<BTreeSet<&'a str>> {
    let mut result = BTreeSet::new();
    for value in values {
        if value.is_empty() || !result.insert(value) {
            return Err(format!("W06 empty/duplicate {description}: {value}"));
        }
    }
    Ok(result)
}
fn case_ids<'a>(packet: &'a Value, role: Option<&str>) -> Result<Vec<&'a str>> {
    let mut ids = Vec::new();
    for row in packet["cases"]
        .as_array()
        .ok_or("W06 source cases missing")?
    {
        if let Some(role) = role {
            let actual = row["qualification_role"]
                .as_str()
                .ok_or("W06 review source role missing")?;
            if !matches!(actual, "source_case" | "reference_control") {
                return Err("W06 unexpected review source role".into());
            }
            if actual != role {
                continue;
            }
        }
        ids.push(
            row["id"]
                .as_str()
                .ok_or("W06 source case identity missing")?,
        );
    }
    Ok(ids)
}
fn execution(output: &str, test: &Test) -> Result<()> {
    if !output
        .lines()
        .any(|line| line == format!("test {} ... ok", test.name))
        || !output
            .lines()
            .any(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
    {
        return Err(format!(
            "W06 required test missing, failed or ignored: {}",
            test.name
        ));
    }
    let expected = unique(test.cases.iter().map(String::as_str), "test case")?;
    let mut actual = BTreeSet::new();
    for line in output.lines() {
        if let Some(payload) = line.strip_prefix("W06_CASE_COMPLETE ") {
            let row: Completion = serde_json::from_str(payload)
                .map_err(|e| format!("W06 malformed completion receipt: {e}"))?;
            if row.test != test.name || !expected.contains(row.case_id.as_str()) {
                return Err(format!(
                    "W06 unexpected completion {} from {}",
                    row.case_id, row.test
                ));
            }
            if !actual.insert(row.case_id) {
                return Err("W06 duplicate case completion receipt".into());
            }
        }
    }
    if actual.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return Err(format!(
            "W06 incomplete semantic case receipts for {}",
            test.name
        ));
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let bytes = frozen(root, "docs/rust-rewrite/w06/source.json", CORPUS_SHA)?;
    frozen(
        root,
        "docs/rust-rewrite/w06/capture_source.py",
        EXPORTER_SHA,
    )?;
    frozen(
        root,
        "docs/rust-rewrite/w06/regression-map.json",
        REGRESSION_MAP_SHA,
    )?;
    let coverage = frozen(
        root,
        "docs/rust-rewrite/w06/qualification.json",
        COVERAGE_SHA,
    )?;
    let qualification: Qualification =
        serde_json::from_slice(&coverage).map_err(|e| e.to_string())?;
    let corpus: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let language: Value = serde_json::from_slice(&frozen(
        root,
        "docs/rust-rewrite/w06/evidence/language-input-source-outcomes.json",
        LANGUAGE_SOURCE_SHA,
    )?)
    .map_err(|e| e.to_string())?;
    frozen(
        root,
        "docs/rust-rewrite/w06/evidence/language-source-producer.source.txt",
        LANGUAGE_PRODUCER_SHA,
    )?;
    let retained: Value = serde_json::from_slice(&frozen(
        root,
        "docs/rust-rewrite/w06/evidence/retained-policy-source-outcomes.json",
        RETAINED_SOURCE_SHA,
    )?)
    .map_err(|e| e.to_string())?;
    frozen(
        root,
        "docs/rust-rewrite/w06/evidence/retained-policy-source-producer.source.txt",
        RETAINED_PRODUCER_SHA,
    )?;
    let saved_authority: Value = serde_json::from_slice(&frozen(
        root,
        "docs/rust-rewrite/w06/evidence/saved-source-authority-outcomes.json",
        SAVED_AUTHORITY_SOURCE_SHA,
    )?)
    .map_err(|e| e.to_string())?;
    frozen(
        root,
        "docs/rust-rewrite/w06/evidence/saved-source-authority-producer.source.txt",
        SAVED_AUTHORITY_PRODUCER_SHA,
    )?;
    let review_public: Value = serde_json::from_slice(&frozen(
        root,
        "docs/rust-rewrite/w06/evidence/review-public-source-outcomes.json",
        REVIEW_PUBLIC_SOURCE_SHA,
    )?)
    .map_err(|e| e.to_string())?;
    frozen(
        root,
        "docs/rust-rewrite/w06/evidence/review-public-source-producer.source.txt",
        REVIEW_PUBLIC_PRODUCER_SHA,
    )?;
    let huge_integer: SourceIdentityPacket = serde_json::from_slice(&frozen(
        root,
        "docs/rust-rewrite/w06/evidence/huge-integer-truthiness-source-outcomes.json",
        HUGE_INTEGER_SOURCE_SHA,
    )?)
    .map_err(|e| e.to_string())?;
    let huge_integer = serde_json::to_value(huge_integer).map_err(|e| e.to_string())?;
    frozen(
        root,
        "docs/rust-rewrite/w06/evidence/huge-integer-truthiness-producer.source.txt",
        HUGE_INTEGER_PRODUCER_SHA,
    )?;
    for (path, expected) in &qualification.owned_sources {
        if let Some(result) = crate::a13::a06_source_successor::try_verify(
            root,
            "docs/rust-rewrite/w06/qualification.json",
            COVERAGE_SHA,
            path,
            expected,
            None,
        ) {
            result?;
        } else if path == "crates/ssx-application/src/lib.rs" {
            super::w06_source_amendment::verify(root, expected, records)?;
        } else if path == "Cargo.lock" {
            crate::a13::check_source_pin(
                root,
                "docs/rust-rewrite/w06/qualification.json",
                COVERAGE_SHA,
                path,
                expected,
                None,
            )
            .map_err(|error| format!("W06 Cargo.lock source provenance: {error}"))?;
        } else {
            frozen(root, path, expected)?;
        }
    }
    if qualification.schema_version != 2
        || qualification.plan_id != "W06"
        || corpus["schema_version"] != 1
        || corpus["plan_id"] != "W06"
        || corpus["baseline"] != BASELINE
        || corpus["provider_calls"] != 0
    {
        return Err("W06 provenance mismatch".into());
    }
    for packet in [&saved_authority, &review_public, &huge_integer] {
        if packet["schema_version"] != 1
            || packet["plan_id"] != "W06"
            || packet["baseline"] != BASELINE
            || packet["provider_calls"] != 0
        {
            return Err("W06 review supplement provenance mismatch".into());
        }
    }
    if review_public["network_calls"] != 0 {
        return Err("W06 public review supplement network provenance mismatch".into());
    }
    // Reference controls record actual original behavior, but are not executed
    // source parity cases when a native test uses different operational inputs.
    let reference_controls = unique(
        case_ids(&review_public, Some("reference_control"))?,
        "source reference control",
    )?;
    if reference_controls
        != unique(
            qualification.reference_controls.iter().map(String::as_str),
            "qualified source reference control",
        )?
    {
        return Err("W06 exact source reference controls changed".into());
    }
    let source_ids = unique(
        case_ids(&corpus, None)?
            .into_iter()
            .chain(case_ids(&retained, None)?)
            .chain(case_ids(&saved_authority, None)?)
            .chain(case_ids(&review_public, Some("source_case"))?)
            .chain(case_ids(&huge_integer, None)?),
        "source case",
    )?;
    if source_ids
        != unique(
            qualification.source_cases.iter().map(String::as_str),
            "qualified source case",
        )?
    {
        return Err("W06 exact source case identities changed".into());
    }
    let mut source_bindings = BTreeMap::new();
    for bindings in [
        &corpus["sources"],
        &language["source_controls"],
        &retained["source_controls"],
        &saved_authority["source_controls"],
        &review_public["source_controls"],
        &huge_integer["source_controls"],
    ] {
        for (path, expected) in bindings.as_object().ok_or("W06 source bindings missing")? {
            if let Some(previous) = source_bindings.insert(path, expected)
                && previous != expected
            {
                return Err(format!("W06 conflicting original source bindings: {path}"));
            }
        }
    }
    for (path, expected) in source_bindings {
        let source = fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
        if Some(digest(&source).as_str()) != expected.as_str() {
            return Err(format!(
                "W06 source drift requires reviewed reconciliation: {path}"
            ));
        }
        let original = command(
            root,
            "git",
            &["rev-parse", &format!("{BASELINE}:{path}")],
            records,
        )?;
        let actual = command(root, "git", &["hash-object", path], records)?;
        if original.trim() != actual.trim() {
            return Err(format!(
                "W06 source no longer matches frozen baseline: {path}"
            ));
        }
    }
    let controls: Value = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/w06/source-controls.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if controls != corpus["sources"] {
        return Err("W06 oracle source guards differ from frozen source bindings".into());
    }
    let mut all_ids = source_ids;
    for id in &qualification.native_cases {
        if !all_ids.insert(id) {
            return Err(format!("W06 duplicate native/source case identity: {id}"));
        }
    }
    if !all_ids.is_disjoint(&reference_controls) {
        return Err("W06 source-only reference control bound as executed case".into());
    }
    let bindings = qualification
        .targets
        .iter()
        .flat_map(|target| &target.tests)
        .flat_map(|test| test.cases.iter().map(String::as_str));
    if unique(bindings, "case binding")? != all_ids {
        return Err(
            "W06 semantic bindings do not cover exact source/native case identities".into(),
        );
    }
    let executable =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER").unwrap_or_else(|_| "valkey-server".into());
    if !command(root, &executable, &["--version"], records)?.starts_with("Valkey server v=8.1.10 ")
    {
        return Err("W06 requires private Valkey8.1.10".into());
    }
    for target in &qualification.targets {
        let expected = unique(
            target
                .tests
                .iter()
                .map(|test| test.name.as_str())
                .chain(target.worker_probes.iter().map(String::as_str)),
            "native test",
        )?;
        let prefix = [
            "test",
            "--locked",
            "-p",
            &target.package,
            "--test",
            &target.target,
            "--",
        ];
        let listing = command(
            root,
            "cargo",
            &[prefix.as_slice(), &["--list"]].concat(),
            records,
        )?;
        let listed = unique(
            listing
                .lines()
                .filter_map(|line| line.strip_suffix(": test")),
            "listed native test",
        )?;
        if listed != expected {
            return Err(format!(
                "W06 exact native inventory changed: {} {}",
                target.package, target.target
            ));
        }
        for test in &target.tests {
            // Successful output is printed after the test, keeping status/receipts distinct.
            let output = command(
                root,
                "cargo",
                &[
                    prefix.as_slice(),
                    &["--exact", &test.name, "--show-output", "--test-threads=1"],
                ]
                .concat(),
                records,
            )?;
            execution(&output, test)?;
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
        &[&["check", "--locked"][..], &packages].concat(),
        records,
    )?;
    command(
        root,
        "cargo",
        &[
            &["clippy", "--locked"][..],
            &packages,
            &["--", "-D", "warnings"],
        ]
        .concat(),
        records,
    )?;
    records.push(json!({"plan_id":"W06","status":"passed","source_sha256":CORPUS_SHA,"qualification_sha256":COVERAGE_SHA,"provider_calls":0,"source_cases":qualification.source_cases,"native_cases":qualification.native_cases,"source_only_reference_controls":qualification.reference_controls,"owned_sources":qualification.owned_sources,"owned_source_amendments":[{"source_path":"crates/ssx-application/src/lib.rs","historical_pin_sha256":"a7102f8c75244e74680cf8773c527d21fb2dcd177a29f0bd27abeef2beeb880e","successor_sha256":"1ff0647013d66e6711587bbd103659959ab860bd3bd2d2b42d10b7a640a5544e","allowed_append":"pub mod lineage; + pub mod receipts;","negative_controls":5}],"authority":"full source outcomes and canonical bytes; actual native semantic assertions; private Valkey; loopback; killed native worker","limitations":"No production/hot reload/transport composition or exactly-once remote dispatch claim; source differences require explicit review"}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn contract() -> Test {
        Test {
            name: "semantic".into(),
            cases: vec!["source.case".into()],
        }
    }
    fn output(receipt: &str) -> String {
        format!(
            "test semantic ... ok\n{receipt}\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
        )
    }
    #[test]
    fn receipts_require_actual_pass_and_exact_post_assertion_case_identity() {
        let good = output("W06_CASE_COMPLETE {\"case_id\":\"source.case\",\"test\":\"semantic\"}");
        execution(&good, &contract()).unwrap();
        for bad in [
            good.replace("... ok", "... ignored"),
            good.replace(
                "1 passed; 0 failed; 0 ignored",
                "0 passed; 0 failed; 1 ignored",
            ),
            output(""),
            output("W06_CASE_COMPLETE {\"case_id\":\"source.other\",\"test\":\"semantic\"}"),
            good.replace("\"test\":\"semantic\"", "\"test\":\"other\""),
            output(
                "W06_CASE_COMPLETE {\"case_id\":\"source.case\",\"test\":\"semantic\"}\nW06_CASE_COMPLETE {\"case_id\":\"source.case\",\"test\":\"semantic\"}",
            ),
        ] {
            assert!(
                execution(&bad, &contract()).is_err(),
                "accepted malformed actual execution: {bad}"
            );
        }
    }
    #[test]
    fn review_reference_controls_require_explicit_roles_separate_from_parity_cases() {
        let packet = json!({"cases":[
            {"id":"review.plan.actual","qualification_role":"source_case"},
            {"id":"review.runner.control","qualification_role":"reference_control"}
        ]});
        assert_eq!(
            case_ids(&packet, Some("source_case")).unwrap(),
            ["review.plan.actual"]
        );
        assert_eq!(
            case_ids(&packet, Some("reference_control")).unwrap(),
            ["review.runner.control"]
        );
        for bad in [
            json!({"cases":[{"id":"review.runner.control"}]}),
            json!({"cases":[{"id":"review.runner.control","qualification_role":"executed"}]}),
            json!({"cases":[{"qualification_role":"source_case"}]}),
        ] {
            assert!(case_ids(&bad, Some("source_case")).is_err());
        }
    }
    #[test]
    fn source_identity_reader_accepts_exact_integers_beyond_float_range() {
        let bytes = include_bytes!(
            "../../../docs/rust-rewrite/w06/evidence/huge-integer-truthiness-source-outcomes.json"
        );
        // The metadata reader must skip exact numeric inputs, not convert them.
        assert!(serde_json::from_slice::<Value>(bytes).is_err());
        let packet: SourceIdentityPacket = serde_json::from_slice(bytes).unwrap();
        let ids = packet
            .cases
            .iter()
            .map(|row| row.id.as_str())
            .collect::<BTreeSet<_>>();
        assert!(ids.contains("huge_truthiness.positive_huge_integer.false"));
        assert!(ids.contains("huge_truthiness.negative_huge_integer.true"));
        assert_eq!(
            packet.source_controls["slopsearx/mcp/staged_tools.py"],
            "ac49b6942567aaab2c8bb56c15bb325f6a87d68f6c65a75a4bc912a74310a2ff"
        );
    }
}
