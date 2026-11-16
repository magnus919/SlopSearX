//! Exact PR874 source-freshness bridge, separate from behavioral qualification.
const MAP: &[(&str, &str, &str)] = &[
    (
        "slopsearx/formatter.py",
        "a703e3fbeaaff5e85230caff9b23d994fcaed250",
        "8fa2bb950dc341adace7b9524b5755e98a4da406",
    ),
    (
        "slopsearx/mcp/tools.py",
        "061593ce45d3e2440e818703af6b50d6e6d99cf1",
        "d06ecd475b9b523addf79781d2762eddd92f4911",
    ),
    (
        "slopsearx/server.py",
        "f60c03fe48ce837f3b2ad65c58d0336ad3b72983",
        "412ad973735f0199be9a32da4763437f55a584bb",
    ),
    (
        "slopsearx/service.py",
        "8b45ce71efc3c7cc4a72b962968e146a63e135be",
        "82eb29d50c4ca98a237949d4d8994cfe3cbeca9f",
    ),
    (
        "tests/test_filter_semantics.py",
        "7c724c4d3367eb505f4e0f3b4ea01bc046bebccc",
        "6475f5fff325f8476976d664dbb798f42296bafa",
    ),
    (
        "tests/test_mcp_enforcement.py",
        "e422d794e7db1bf0f29ab58edff6c89aa87fe533",
        "aa2a9be0b47e3e36d106c5af69610dec87f048d2",
    ),
    (
        "tests/test_mcp_tools.py",
        "90495a0038f43ca1140140576f28eec1015b1183",
        "9804b31ace1575fc1253dd82549d4f3a17bcebda",
    ),
    (
        "tests/test_server.py",
        "128af281562581b96d26111f3e2bfa81501c69d1",
        "ec95d1c7d79e3cbdee3eac7251cd8a23914912fe",
    ),
];
use crate::{Result, command};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
pub const REVISION: &str = "d207d4496129d496fe8a74b040ebe1a71151b6f9";
pub const CORPUS: &str = "4a09305db9779b96edff181dc64b0d3d10e65aa7b9af3073588b539e7a32f4c6";
pub const EXPORTER: &str = "57f06bde66186e763b83de56fa4c05e10af9d4a36b9c350f909ed843af05ba89";
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn accepts(path: &str, old: &str, current: &str) -> bool {
    old == current
        || MAP
            .iter()
            .any(|(p, a, b)| *p == path && *a == old && *b == current)
}
pub fn corpus(root: &Path, records: &mut Vec<Value>) -> Result<Value> {
    let bytes =
        fs::read(root.join("crates/xtask/fixtures/f14/dates.json")).map_err(|e| e.to_string())?;
    if digest(&bytes) != CORPUS
        || digest(
            &fs::read(root.join("docs/rust-rewrite/f14/capture_dates.py"))
                .map_err(|e| e.to_string())?,
        ) != EXPORTER
    {
        return Err("date amendment reviewed corpus/exporter drift".into());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if value["source_commit"] != REVISION
        || value["cases"].as_array().map(Vec::len) != Some(22)
        || value["source_files"].as_object().map(|v| v.len()) != Some(11)
    {
        return Err("date amendment finite provenance drift".into());
    }
    for (path, pin) in value["source_files"]
        .as_object()
        .ok_or("date amendment bindings")?
    {
        let blob = command(
            root,
            "git",
            &["rev-parse", &format!("{REVISION}:{path}")],
            records,
        )?;
        let bytes = command(
            root,
            "git",
            &["show", &format!("{REVISION}:{path}")],
            records,
        )?;
        if Some(blob.trim()) != pin["git_blob"].as_str()
            || Some(digest(bytes.as_bytes()).as_str()) != pin["sha256"].as_str()
        {
            return Err(format!(
                "date amendment original source binding drift: {path}"
            ));
        }
    }
    Ok(value)
}
/// Verify the original source independently before considering only the exact
/// reviewed current-byte amendment. This does not assert behavioral parity.
pub fn source(
    root: &Path,
    path: &str,
    baseline: &str,
    old_blob: &str,
    old_sha: Option<&str>,
    records: &mut Vec<Value>,
) -> Result<()> {
    let old = command(
        root,
        "git",
        &["rev-parse", &format!("{baseline}:{path}")],
        records,
    )?;
    let historical = command(
        root,
        "git",
        &["show", &format!("{baseline}:{path}")],
        records,
    )?;
    if old.trim() != old_blob || old_sha.is_some_and(|sha| sha != digest(historical.as_bytes())) {
        return Err(format!("historical source binding drift: {path}"));
    }
    let current = command(root, "git", &["hash-object", "--", path], records)?;
    if !accepts(path, old_blob, current.trim()) {
        return Err(format!("unreviewed current source drift: {path}"));
    }
    if current.trim() != old_blob {
        let proof = corpus(root, records)?;
        let pin = &proof["source_files"][path];
        let bytes = fs::read(root.join(path)).map_err(|e| e.to_string())?;
        if Some(current.trim()) != pin["git_blob"].as_str()
            || Some(digest(&bytes).as_str()) != pin["sha256"].as_str()
        {
            return Err(format!("current date amendment binding drift: {path}"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_current_upstream_amendment_has_reviewed_bridge() {
        for (path, old, new) in MAP {
            assert!(accepts(path, old, new), "{path}");
        }
    }
    #[test]
    fn unknown_blob_wrong_path_and_wrong_old_identity_fail_closed() {
        for (path, old, new) in MAP {
            assert!(!accepts(
                path,
                old,
                "0000000000000000000000000000000000000000"
            ));
            assert!(!accepts("unrelated.py", old, new));
            assert!(!accepts(
                path,
                "0000000000000000000000000000000000000000",
                new
            ));
        }
    }
    #[test]
    fn corrupted_date_corpus_or_exporter_cannot_activate_bridge() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp = std::env::temp_dir().join(format!("ssx-date-negative-{}", std::process::id()));
        fs::create_dir_all(temp.join("crates/xtask/fixtures/f14")).unwrap();
        fs::create_dir_all(temp.join("docs/rust-rewrite/f14")).unwrap();
        let fixture = fs::read(root.join("crates/xtask/fixtures/f14/dates.json")).unwrap();
        let exporter = fs::read(root.join("docs/rust-rewrite/f14/capture_dates.py")).unwrap();
        fs::write(temp.join("crates/xtask/fixtures/f14/dates.json"), b"{}").unwrap();
        fs::write(
            temp.join("docs/rust-rewrite/f14/capture_dates.py"),
            &exporter,
        )
        .unwrap();
        assert!(corpus(&temp, &mut vec![]).is_err());
        fs::write(temp.join("crates/xtask/fixtures/f14/dates.json"), fixture).unwrap();
        fs::write(
            temp.join("docs/rust-rewrite/f14/capture_dates.py"),
            b"# changed",
        )
        .unwrap();
        assert!(corpus(&temp, &mut vec![]).is_err());
        fs::remove_dir_all(temp).unwrap();
    }
}
