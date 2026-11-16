//! A12 static-source-bound native component gate; never runs original Python.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const QUALIFICATION_SHA: &str = "f55413a595d927c7450b942bc4b545b8db8401e8a9f730e9321680d63bf5ca39";
fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn one_pass(stdout: &str, name: &str) -> bool {
    let lines = stdout.lines().collect::<Vec<_>>();
    lines.iter().filter(|l| **l == "running 1 test").count() == 1
        && lines
            .iter()
            .filter(|l| **l == format!("test {name} ... ok"))
            .count()
            == 1
        && lines
            .iter()
            .filter(|l| l.starts_with("test ") && !l.starts_with("test result:"))
            .count()
            == 1
        && lines
            .iter()
            .filter(|l| l.starts_with("test result:"))
            .count()
            == 1
        && lines
            .iter()
            .any(|l| l.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "))
}
const DEFAULT_IDENTITY_TEST_PATH: &str = "crates/ssx-infra/tests/a12_security.rs";
const DEFAULT_IDENTITY_OLD_SHA: &str =
    "601e6e9c1260dc1550820be5057db2dea37e41e81d9ba7d030f02dddfa2446db";
const DEFAULT_IDENTITY_CURRENT_SHA: &str =
    "ee1339e68e04241029c78e17711fbff5b1a7b2e6033f5f6816de80bf46ef87ec";
const DEFAULT_IDENTITY_INSERT: &str = "    headers.insert(\n        reqwest::header::USER_AGENT,\n        reqwest::header::HeaderValue::from_static(http::DEFAULT_USER_AGENT),\n    );\n";
fn default_identity_source_historical(
    raw: &[u8],
    expected_sha: &str,
    expected_bytes: Option<usize>,
) -> Result<()> {
    require(
        expected_sha == DEFAULT_IDENTITY_OLD_SHA && expected_bytes.is_none(),
        "A12 exact historical default-identity source caller changed",
    )?;
    require(
        digest(raw) == DEFAULT_IDENTITY_CURRENT_SHA,
        "A12 current default-identity test source changed",
    )?;
    let current = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
    require(
        current.matches(DEFAULT_IDENTITY_INSERT).count() == 1,
        "A12 exact one default-identity insertion required",
    )?;
    let historical = current.replacen(DEFAULT_IDENTITY_INSERT, "", 1);
    require(
        digest(historical.as_bytes()) == DEFAULT_IDENTITY_OLD_SHA,
        "A12 default-identity insertion changes other original assertions",
    )
}
// The legacy UA proof receives e127 bytes only after physical Page authentication.
fn default_identity_current(
    root: &Path,
    raw: &[u8],
    expected_sha: &str,
    expected_bytes: Option<usize>,
) -> Result<Vec<u8>> {
    require(
        expected_sha == DEFAULT_IDENTITY_OLD_SHA && expected_bytes.is_none(),
        "A12 exact historical default-identity source caller changed",
    )?;
    let frame = json!({"path":DEFAULT_IDENTITY_TEST_PATH,"sha256":DEFAULT_IDENTITY_CURRENT_SHA,"bytes":60225,"mode":"0644"});
    crate::a13::a06_source_successor::native_search::page_prior(root, &frame, Some(raw))?
        .ok_or("A12 default-identity page operand not selected".into())
}
fn default_identity_source(
    root: &Path,
    raw: &[u8],
    expected_sha: &str,
    expected_bytes: Option<usize>,
) -> Result<()> {
    let current = default_identity_current(root, raw, expected_sha, expected_bytes)?;
    default_identity_source_historical(&current, expected_sha, expected_bytes)
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let bytes = fs::read(root.join("docs/rust-rewrite/a12/qualification-v1.json"))
        .map_err(|e| e.to_string())?;
    require(
        digest(&bytes) == QUALIFICATION_SHA,
        "A12 qualification bytes changed; review before repin",
    )?;
    let q: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    require(
        q["plan_id"] == "A12"
            && q["evidence_kind"] == "STATIC_SOURCE_DERIVATION_AND_NATIVE_COMPONENT",
        "A12 evidence kind changed",
    )?;
    let mut pins = BTreeSet::new();
    for pin in q["sources"].as_array().ok_or("A12 source pins absent")? {
        let p = pin["path"].as_str().ok_or("A12 path absent")?;
        require(
            !Path::new(p).is_absolute() && !p.split('/').any(|v| v == "..") && pins.insert(p),
            "A12 duplicate/unsafe path",
        )?;
        let expected_sha = pin["sha256"].as_str().ok_or("A12 source digest absent")?;
        let expected_bytes = pin["bytes"].as_u64().map(|value| value as usize);
        if p == DEFAULT_IDENTITY_TEST_PATH {
            default_identity_source(
                root,
                &fs::read(root.join(p)).map_err(|e| e.to_string())?,
                expected_sha,
                expected_bytes,
            )?;
        } else {
            crate::a13::check_source_pin(
                root,
                "docs/rust-rewrite/a12/qualification-v1.json",
                &digest(&bytes),
                p,
                expected_sha,
                expected_bytes,
            )
            .map_err(|_| "A12 source pin stale".to_owned())?;
        }
    }
    require(pins.len() >= 20, "A12 source coverage absent")?;
    let map: Value = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/a12/source-test-map-v1.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    require(
        map["source_runs"].as_array().is_some_and(Vec::is_empty)
            && map["python_executed"] == false
            && map["live_provider_calls"] == 0,
        "A12 static evidence mislabeled as source execution",
    )?;
    let tests = q["native_tests"]
        .as_array()
        .ok_or("A12 native test identities absent")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "A12 test identity invalid".to_owned())
        })
        .collect::<Result<BTreeSet<_>>>()?;
    require(
        tests.len() == 28
            && q["native_tests"]
                .as_array()
                .is_some_and(|v| v.len() == tests.len()),
        "A12 native test identity set changed or duplicated",
    )?;
    let mut originals = BTreeSet::new();
    for row in map["original_tests"]
        .as_array()
        .ok_or("A12 original map absent")?
    {
        let node = row["node"]
            .as_str()
            .ok_or("A12 original identity missing")?;
        let test = row["native_test"]
            .as_str()
            .ok_or("A12 original native binding missing")?;
        require(
            originals.insert(node) && tests.contains(test) && row["source_executed"] == false,
            "A12 duplicate/unbound original test",
        )?;
        let path = node
            .split("::")
            .next()
            .ok_or("A12 original source path missing")?;
        let text = fs::read_to_string(root.join(path)).map_err(|e| e.to_string())?;
        require(
            text.contains(
                row["original_test_source_excerpt"]
                    .as_str()
                    .ok_or("A12 source excerpt missing")?,
            ),
            "A12 original assertion excerpt stale",
        )?;
    }
    require(originals.len() == 33, "A12 original test coverage changed")?;
    let listed = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "a12_security",
            "--",
            "--list",
        ],
        records,
    )?;
    let actual = listed
        .lines()
        .filter_map(|l| l.strip_suffix(": test").map(str::to_owned))
        .collect::<BTreeSet<_>>();
    require(
        actual == tests && listed.lines().filter(|l| l.ends_with(": test")).count() == actual.len(),
        "A12 compiled test identity set differs or duplicates",
    )?;
    for test in tests {
        let stdout = command(
            root,
            "cargo",
            &[
                "test",
                "--locked",
                "-p",
                "ssx-infra",
                "--test",
                "a12_security",
                &test,
                "--",
                "--exact",
            ],
            records,
        )?;
        require(
            one_pass(&stdout, &test),
            "A12 exact test missing, ignored, caseless or changed",
        )?;
        records.push(
            json!({"A12_native_completion":test,"original_execution":false,"provider_calls":0}),
        );
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::one_pass;
    #[test]
    fn exact_positive_completion_is_required() {
        let root = &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let current = include_bytes!("../../ssx-infra/tests/a12_security.rs");
        assert!(
            super::default_identity_source(root, current, super::DEFAULT_IDENTITY_OLD_SHA, None)
                .is_ok()
        );
        assert!(super::default_identity_source(root, current, "wrong", None).is_err());
        assert!(
            super::default_identity_source(
                root,
                current,
                super::DEFAULT_IDENTITY_OLD_SHA,
                Some(current.len())
            )
            .is_err()
        );
        let logical =
            super::default_identity_current(root, current, super::DEFAULT_IDENTITY_OLD_SHA, None)
                .unwrap();
        assert!(
            super::default_identity_source(root, &logical, super::DEFAULT_IDENTITY_OLD_SHA, None)
                .is_err()
        );
        let mut altered = current.to_vec();
        altered.push(0);
        assert!(
            super::default_identity_source(root, &altered, super::DEFAULT_IDENTITY_OLD_SHA, None)
                .is_err()
        );
        super::default_identity_source_historical(&logical, super::DEFAULT_IDENTITY_OLD_SHA, None)
            .unwrap();
        let current = std::str::from_utf8(&logical).unwrap();
        for mutated in [
            current.replacen(super::DEFAULT_IDENTITY_INSERT, "", 1),
            current.replace("http::DEFAULT_USER_AGENT", "\"wrong-agent\""),
            current.replace(
                super::DEFAULT_IDENTITY_INSERT,
                &format!(
                    "{}{}",
                    super::DEFAULT_IDENTITY_INSERT,
                    super::DEFAULT_IDENTITY_INSERT
                ),
            ),
            current.replace("synthetic-secret", "changed-secret"),
        ] {
            assert!(
                super::default_identity_source_historical(
                    mutated.as_bytes(),
                    super::DEFAULT_IDENTITY_OLD_SHA,
                    None
                )
                .is_err()
            );
        }
        let valid = "\nrunning 1 test\ntest selected ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out; finished in 0.01s\n";
        assert!(one_pass(valid, "selected"));
        for invalid in [
            valid.replace("selected", "different"),
            valid.replace("running 1 test", "running 0 tests"),
            valid.replace("... ok", "... ignored"),
            valid.replace("0 ignored", "1 ignored"),
            valid.replace("1 passed", "0 passed"),
            valid.replace(
                "test selected ... ok",
                "test selected ... ok\ntest extra ... ok",
            ),
            valid.replace("test result: ok.", "test result: FAILED."),
        ] {
            assert!(!one_pass(&invalid, "selected"));
        }
    }
    #[test]
    fn duplicate_or_missing_completion_is_rejected() {
        assert!(!one_pass("", "selected"));
        assert!(!one_pass(
            "running 1 test\ntest selected ... FAILED\n",
            "selected"
        ));
        let valid = "running 1 test\ntest selected ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out; finished in 0.01s\n";
        assert!(!one_pass(&format!("{valid}{valid}"), "selected"));
    }
}
