//! Finite W08 semantic execution. Missing, ignored or renamed cases fail closed.
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
const REGISTRY_SHA: &str = "4bb89aa00950e3782fc992c71eb5cfbd4691bed5e52e6a46a70e8fe2f64d4fb0";
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
        return Err(format!("W08 source drift requires explicit review: {path}"));
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
