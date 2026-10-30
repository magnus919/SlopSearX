//! Finite W08 semantic execution. Missing, ignored or renamed cases fail closed.
pub(crate) use crate::a13::a06_source_successor::w07_t02_joint::w08_source_successor as source_successor;
use crate::{Result, command};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
const REGISTRY: &str = "docs/rust-rewrite/w08/qualification.json";
const REGISTRY_SHA: &str = "34d3a465950796007e91333f23da73550ae786490b3ca923e89fd162f7265451";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Qualification {
    schema_version: u32,
    plan_id: String,
    baseline: String,
    sources: BTreeMap<String, String>,
    suites: Vec<Suite>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Suite {
    package: String,
    target: String,
    tests: Vec<String>,
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn frozen(root: &Path, path: &str, expected: &str) -> Result<Vec<u8>> {
    if Path::new(path).is_absolute() || path.split('/').any(|s| s == "..") {
        return Err("W08 unsafe source path".into());
    }
    let metadata = fs::symlink_metadata(root.join(path)).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!("W08 source must be a regular file: {path}"));
    }
    let raw = fs::read(root.join(path)).map_err(|e| e.to_string())?;
    if digest(&raw) != expected {
        use crate::a13::a06_source_successor::w07_t02_joint::snapshot_helper_amendment;
        let prior = match (path, expected) {
            (
                "crates/xtask/src/w08/source_successor.rs",
                "3a8699c6449e05b6b94f64ff43643af15287ad2ae4d9b5319617d8f5c38f4069",
            ) => snapshot_helper_amendment::w08_reader_source_operand(root, path, expected, &raw)?,
            (
                "crates/xtask/src/w07.rs",
                "64bbd78e5910558eae78de7e449d0488337020a814ec10cb2869e412193582ea",
            ) => snapshot_helper_amendment::w07_source_operand(root, path, expected, &raw)?,
            (
                "docs/rust-rewrite/a06/source-amendments/shared-successor.rs",
                "013f8448379ef92ea059cf2d48981742635d7433931a6e0973f781139bb9d612",
            )
            | (
                "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs",
                "07ffa419b6324171042fdb1bba37b4e3a4827f38ae18e0150c39bf962896a3f6",
            ) => snapshot_helper_amendment::w08_qualification_operand(root, path, expected, &raw)?,
            _ => return Err(format!("W08 source drift requires explicit review: {path}")),
        };
        if digest(&prior) != expected {
            return Err(format!("W08 exact frozen source inverse differs: {path}"));
        }
        return Ok(prior);
    }
    Ok(raw)
}
fn names(output: &str, listing: bool) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for line in output.lines() {
        let name = if listing {
            line.strip_suffix(": test")
        } else if let Some(row) = line.strip_prefix("test ") {
            let Some((name, status)) = row.rsplit_once(" ... ") else {
                continue;
            };
            if status != "ok" {
                return Err(format!("W08 incomplete case {name}: {status}"));
            }
            Some(name)
        } else {
            None
        };
        if let Some(name) = name
            && (name.is_empty() || !names.insert(name.to_owned()))
        {
            return Err("W08 duplicate/empty case identity".into());
        }
    }
    if names.is_empty() {
        return Err("W08 no semantic cases executed".into());
    }
    Ok(names)
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    source_successor::controls(root)?;
    let q: Qualification = serde_json::from_slice(&frozen(root, REGISTRY, REGISTRY_SHA)?)
        .map_err(|e| e.to_string())?;
    if q.schema_version != 1
        || q.plan_id != "W08"
        || q.baseline != "e9c5911a468b892fe2adef9027374cb867a2535d"
        || q.sources.is_empty()
        || q.suites.len() != 2
    {
        return Err("W08 invalid finite qualification registry".into());
    }
    for (path, sha) in &q.sources {
        frozen(root, path, sha)?;
    }
    for suite in q.suites {
        if !["ssx-application", "ssx-infra"].contains(&suite.package.as_str())
            || suite.target != "w08"
            || suite.tests.is_empty()
        {
            return Err("W08 invalid suite".into());
        }
        let expected = suite.tests.iter().cloned().collect::<BTreeSet<_>>();
        if expected.len() != suite.tests.len() {
            return Err("W08 duplicate planned cases".into());
        }
        let args = [
            "test",
            "--locked",
            "-p",
            &suite.package,
            "--test",
            &suite.target,
            "--",
        ];
        let listed = command(
            root,
            "cargo",
            &[args.as_slice(), &["--list"]].concat(),
            records,
        )?;
        if names(&listed, true)? != expected {
            return Err("W08 exact case inventory differs from sealed suite".into());
        }
        let executed = command(
            root,
            "cargo",
            &[args.as_slice(), &["--show-output", "--test-threads=1"]].concat(),
            records,
        )?;
        if names(&executed, false)? != expected {
            return Err("W08 actual case completions differ from sealed suite".into());
        }
        let prefix = format!(
            "test result: ok. {} passed; 0 failed; 0 ignored; 0 measured; ",
            expected.len()
        );
        let summaries = executed
            .lines()
            .filter(|s| s.starts_with("test result: "))
            .collect::<Vec<_>>();
        if summaries.len() != 1 || !summaries[0].starts_with(&prefix) {
            return Err("W08 missing/ignored/partial semantic execution".into());
        }
        for case in expected {
            records.push(json!({"plan_id":"W08","case_id":format!("{}::{}::{case}",suite.package,suite.target),"status":"completed","authority":"actual native semantic assertions; private Valkey/loopback for infrastructure suite"}));
        }
    }
    records.push(json!({"plan_id":"W08","status":"passed","registry_sha256":REGISTRY_SHA,"provider_calls":0,"scope":"native bounded dossier service and readonly composition; existing durable research worker; finite private Valkey and loopback fixtures","limitations":"W09 final workflow transport wiring and W10 protected portal UI remain separate; no live provider, relevance, production or complete rewrite claim"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_successor_requires_actual_afterimages_exact_inverses_and_frozen_predecessors() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        source_successor::controls(&root).unwrap();
    }
    #[test]
    fn semantic_completion_rejects_missing_ignored_failed_and_duplicates() {
        assert!(names("", false).is_err());
        assert!(names("test a ... ignored\n", false).is_err());
        assert!(names("test a ... FAILED\n", false).is_err());
        assert!(names("test a ... ok\ntest a ... ok\n", false).is_err());
        assert_eq!(
            names("test a ... ok\n", false).unwrap(),
            ["a".into()].into_iter().collect()
        );
    }
}
