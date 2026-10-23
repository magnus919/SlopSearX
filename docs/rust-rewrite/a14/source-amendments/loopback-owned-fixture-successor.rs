//! Exact test-fixture child of the accepted jobs source; frozen A14 cases stay intact.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
};
const SOURCE: &str = "crates/ssx-infra/src/adapters/jobs.rs";
const FIXTURE: &str = "crates/ssx-infra/tests/support/owned_loopback_fixture.rs";
const CASES: &str = "docs/rust-rewrite/a14/cases.json";
const CAPSULE: &str =
    "docs/rust-rewrite/a14/source-amendments/loopback-owned-fixture-successor-v1.json";
const CAPSULE_SHA: &str = "f1f87761443ca965f32ce1fa62fd40c355b4c7bf2b2e67212fb8b023d43715f5";
fn need(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key]
        .as_str()
        .ok_or_else(|| format!("A14 fixture field {key}"))
}
fn material(row: &Value, raw: &[u8], mode: &str) -> Result<(), String> {
    need(
        row["sha256"] == digest(raw)
            && row["bytes"].as_u64() == Some(raw.len() as u64)
            && row["mode"] == mode
            && mode == "0644",
        "A14 exact fixture material",
    )
}
fn read(root: &Path, row: &Value) -> Result<Vec<u8>, String> {
    let relative = text(row, "path")?;
    need(
        !Path::new(relative).is_absolute() && !relative.split('/').any(|p| p == ".."),
        "A14 fixture path",
    )?;
    let path = root.join(relative);
    let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        meta.is_file() && meta.nlink() == 1 && meta.permissions().mode() & 0o7777 == 0o644,
        "A14 fixture file/mode",
    )?;
    let raw = fs::read(path).map_err(|e| e.to_string())?;
    material(row, &raw, "0644")?;
    Ok(raw)
}
fn document(raw: &[u8]) -> Result<Value, String> {
    need(digest(raw) == CAPSULE_SHA, "A14 fixture capsule changed")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "A14_OWNED_LOOPBACK_FIXTURE_SUCCESSOR_V1"
            && doc["source_parent"] == "7c2bb2213fce490f5b55f4a2d93784a4f62e8ac7"
            && doc["prior"]["path"] == SOURCE
            && doc["current"]["path"] == SOURCE
            && doc["fixture"]["path"] == FIXTURE
            && doc["caller"]["path"] == CASES,
        "A14 fixture closed scope",
    )?;
    Ok(doc)
}
fn reconstruct(prior: &[u8], edits: &Value) -> Result<Vec<u8>, String> {
    let mut at = 0usize;
    let mut out = Vec::new();
    for edit in edits.as_array().ok_or("A14 fixture edits")? {
        let start = edit["at"]
            .as_u64()
            .and_then(|x| usize::try_from(x).ok())
            .ok_or("A14 fixture offset")?;
        let removed = text(edit, "remove_utf8")?.as_bytes();
        let inserted = text(edit, "insert_utf8")?.as_bytes();
        let end = start
            .checked_add(removed.len())
            .ok_or("A14 fixture edit overflow")?;
        need(
            start >= at && end <= prior.len() && &prior[start..end] == removed,
            "A14 fixture exact ordered edit",
        )?;
        out.extend_from_slice(&prior[at..start]);
        out.extend_from_slice(inserted);
        at = end;
    }
    out.extend_from_slice(&prior[at..]);
    Ok(out)
}
fn validate(
    doc: &Value,
    caller_sha: &str,
    expected: &str,
    prior: &[u8],
    current: &[u8],
    fixture: &[u8],
    mode: &str,
) -> Result<(), String> {
    need(
        doc["caller"]["sha256"] == caller_sha && doc["prior"]["sha256"] == expected,
        "A14 fixture exact frozen caller",
    )?;
    material(&doc["prior"], prior, "0644")?;
    material(&doc["current"], current, mode)?;
    material(&doc["fixture"], fixture, "0644")?;
    need(
        reconstruct(prior, &doc["edits"])? == current,
        "A14 fixture reconstruction differs",
    )
}
pub(super) fn verify(root: &Path, caller_sha: &str, expected: &str) -> Result<(), String> {
    let path = root.join(CAPSULE);
    let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        meta.is_file() && meta.nlink() == 1 && meta.permissions().mode() & 0o7777 == 0o644,
        "A14 fixture capsule mode",
    )?;
    let doc = document(&fs::read(path).map_err(|e| e.to_string())?)?;
    read(root, &doc["caller"])?;
    let prior = read(root, &doc["archive"])?;
    let current = read(root, &doc["current"])?;
    let fixture = read(root, &doc["fixture"])?;
    validate(
        &doc, caller_sha, expected, &prior, &current, &fixture, "0644",
    )
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = document(&fs::read(root.join(CAPSULE)).map_err(|e| e.to_string())?)?;
    let prior = read(root, &doc["archive"])?;
    let current = read(root, &doc["current"])?;
    let fixture = read(root, &doc["fixture"])?;
    let caller = text(&doc["caller"], "sha256")?;
    let expected = text(&doc["prior"], "sha256")?;
    verify(root, caller, expected)?;
    for (claim, old, mode) in [
        ("wrong", expected, "0644"),
        (caller, "wrong", "0644"),
        (caller, expected, "0755"),
    ] {
        need(
            validate(&doc, claim, old, &prior, &current, &fixture, mode).is_err(),
            "A14 changed fixture caller/mode accepted",
        )?;
    }
    need(
        validate(&doc, caller, expected, &prior, &prior, &fixture, "0644").is_err(),
        "A14 old fixture accepted as current",
    )?;
    let mut changed = prior.clone();
    changed.push(0);
    need(
        validate(&doc, caller, expected, &changed, &current, &fixture, "0644").is_err(),
        "A14 changed fixture archive accepted",
    )?;
    let mut changed_fixture = fixture.clone();
    changed_fixture.push(0);
    need(
        validate(
            &doc,
            caller,
            expected,
            &prior,
            &current,
            &changed_fixture,
            "0644",
        )
        .is_err(),
        "A14 changed owned fixture accepted",
    )?;
    let mut changed = current.clone();
    changed.push(0);
    let mut rehashed = doc.clone();
    rehashed["current"]["sha256"] = serde_json::json!(digest(&changed));
    rehashed["current"]["bytes"] = serde_json::json!(changed.len());
    need(
        validate(
            &rehashed, caller, expected, &prior, &changed, &fixture, "0644",
        )
        .is_err(),
        "A14 rehashed unrelated fixture accepted",
    )?;
    need(
        document(&serde_json::to_vec(&rehashed).map_err(|e| e.to_string())?).is_err(),
        "A14 changed fixture capsule accepted",
    )?;
    Ok(())
}
