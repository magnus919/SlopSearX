//! Exact-source, exact-test and completed-case W05 gate; no original Python runtime.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const BINDINGS: &[(&str, &str)] = &[
    (
        "source.json",
        "bfa6a97cc9912f97054e78b56fd3ead683f417d0da293440d5c173f5d579dccf",
    ),
    (
        "inputs.json",
        "09381cd39ecb3dede754b7fd73b0a96a2e7d4a9e672761f4dff5904ce6faa5ed",
    ),
    (
        "case-manifest.json",
        "b86a161a00e1cdf04b11d6dc532ab86644af1ed8a230a3df2e89f4f229091f96",
    ),
    (
        "capture_source.py",
        "da65590a847b3b12b3571900293d123f0e31491e5202799ea0da88256d08af7c",
    ),
];
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn strings(value: &Value) -> Result<BTreeSet<String>> {
    let rows = value.as_array().ok_or("W05 identity array absent")?;
    let result = rows
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "W05 identity string absent".into())
        })
        .collect::<Result<BTreeSet<_>>>()?;
    if result.len() != rows.len() || result.is_empty() {
        return Err("W05 duplicate/empty identity inventory".into());
    }
    Ok(result)
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let dir = root.join("docs/rust-rewrite/w05");
    for (path, sha) in BINDINGS {
        if digest(&fs::read(dir.join(path)).map_err(|e| e.to_string())?) != *sha {
            return Err(format!(
                "W05 frozen source/case/export identity changed: {path}"
            ));
        }
    }
    let source: Value =
        serde_json::from_slice(&fs::read(dir.join("source.json")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if source["baseline"] != "93db215de1c5b59ffdcac035a79ffcbd8e806762" {
        return Err("W05 source baseline changed".into());
    }
    for (path, expected) in source["sources"]
        .as_object()
        .ok_or("W05 source hashes absent")?
    {
        if digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?)
            != expected.as_str().ok_or("W05 source hash absent")?
        {
            return Err(format!(
                "W05 original source drift requires reviewed reconciliation: {path}"
            ));
        }
        let before = command(
            root,
            "git",
            &[
                "rev-parse",
                &format!("93db215de1c5b59ffdcac035a79ffcbd8e806762:{path}"),
            ],
            records,
        )?;
        let current = command(root, "git", &["hash-object", path], records)?;
        if before.trim() != current.trim() {
            return Err(format!("W05 baseline blob differs: {path}"));
        }
    }
    let executable = std::env::var("SLOPSEARX_TEST_VALKEY_SERVER")
        .map_err(|_| "W05 requires explicit private Valkey executable")?;
    if !command(root, &executable, &["--version"], records)?.starts_with("Valkey server v=8.1.10 ")
    {
        return Err("W05 requires private Valkey8.1.10".into());
    }
    let manifest: Value = serde_json::from_slice(
        &fs::read(dir.join("case-manifest.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let expected_cases = strings(&manifest["native_cases"])?;
    let mut completed = BTreeSet::new();
    let mut total = 0;
    for package in ["ssx-infra", "ssx-mcp"] {
        let expected = strings(&manifest["native_tests"][package])?;
        let listing = command(
            root,
            "cargo",
            &[
                "test", "--locked", "-p", package, "--test", "w05", "--", "--list",
            ],
            records,
        )?;
        let actual: BTreeSet<String> = listing
            .lines()
            .filter_map(|line| line.strip_suffix(": test").map(str::to_owned))
            .collect();
        if actual != expected {
            return Err(format!(
                "W05 {package} exact required native test inventory differs"
            ));
        }
        let output = command(
            root,
            "cargo",
            &[
                "test",
                "--locked",
                "-p",
                package,
                "--test",
                "w05",
                "--",
                "--nocapture",
                "--test-threads=1",
            ],
            records,
        )?;
        if !output.lines().any(|line| {
            line.starts_with("test result: ok.")
                && line.contains(&format!("{} passed; 0 failed; 0 ignored", expected.len()))
        }) {
            return Err(format!(
                "W05 {package} required tests missing, ignored or failed"
            ));
        }
        for line in output.lines() {
            if let Some((_, identity)) = line.split_once("W05_CASE_COMPLETE:") {
                let identity = identity.trim();
                total += 1;
                if !completed.insert(identity.to_owned()) {
                    return Err(format!("W05 duplicate completion receipt: {identity}"));
                }
            }
        }
    }
    if completed != expected_cases || total != expected_cases.len() {
        return Err("W05 native PASS without exact completed semantic case receipts".into());
    }
    records.push(json!({"plan_id":"W05","status":"passed","completed_cases":completed,"original_provider_calls":0,"native_provider_calls":0,"authority":"frozen original pure event goldens; private real Valkey; actual native event service and loopback rmcp client","limitations":"typed event observation seam only; O01 exporter and W09 full authenticated production composition remain integration obligations; unknown write completion is not rollback or exactly-once remote execution"}));
    Ok(())
}
