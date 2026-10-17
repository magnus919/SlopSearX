//! A12 static-source-bound native component gate; never runs original Python.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const QUALIFICATION_SHA: &str = "61e3d2cd2626962342b1006d0f7269f469dcb5f66eb8d3db8c20166308e54ea3";
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
        require(
            pin["sha256"].as_str()
                == Some(&digest(
                    &fs::read(root.join(p)).map_err(|e| format!("{p}: {e}"))?,
                )),
            "A12 source pin stale",
        )?;
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
        tests.len() == 27
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
