//! Closed W08 additive generation. Current physical material precedes every
//! inverse; historical operands never masquerade as the current source identity.
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
const CAPSULE: &str = "docs/rust-rewrite/w08/source-successor.json";
const SHA: &str = "d2ec47bd2159466561c8969487eb1fd75f8e939131b2e99be1de1d64ce589871";
const BASELINE: &str = "e9c5911a468b892fe2adef9027374cb867a2535d";
const PATHS: &[&str] = &[
    "crates/ssx-application/src/lib.rs",
    "crates/ssx-infra/src/lib.rs",
    "crates/xtask/src/main.rs",
    "docs/rust-rewrite/a06/source-amendments/shared-successor.rs",
    "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs",
];
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Material {
    sha256: String,
    bytes: usize,
    mode: u32,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    current_offset: usize,
    old: String,
    new: String,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    path: String,
    prior: Material,
    prior_git_blob: String,
    current: Material,
    edits: Vec<Edit>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capsule {
    schema_version: u32,
    plan_id: String,
    baseline: String,
    rows: Vec<Row>,
    caller_inventory: Value,
    frozen_predecessors: Vec<Frozen>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frozen {
    path: String,
    sha256: String,
}
fn need(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn regular(root: &Path, path: &str) -> Result<(Vec<u8>, u32)> {
    need(
        !Path::new(path).is_absolute() && !path.split('/').any(|p| p == ".."),
        "W08 unsafe source path",
    )?;
    let full = root.join(path);
    let metadata = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
    need(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "W08 source must be regular and nonsymlink",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok((
            fs::read(full).map_err(|e| e.to_string())?,
            metadata.permissions().mode() & 0o7777,
        ))
    }
    #[cfg(not(unix))]
    {
        Err("W08 source mode proof requires Unix".into())
    }
}
fn load(root: &Path) -> Result<Capsule> {
    let (raw, mode) = regular(root, CAPSULE)?;
    need(
        mode == 0o644 && digest(&raw) == SHA,
        "W08 exact source-successor capsule changed",
    )?;
    let doc: Capsule = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        doc.schema_version == 1
            && doc.plan_id == "W08"
            && doc.baseline == BASELINE
            && doc
                .rows
                .iter()
                .map(|r| r.path.as_str())
                .eq(PATHS.iter().copied()),
        "W08 closed source membership/baseline differs",
    )?;
    need(
        doc.caller_inventory
            == json!([
                "shared-successor::read_exact exact prior-current tuples",
                "shared-successor::metrics_predecessor exact Metrics caller tuple",
                "w07-t02-joint::parent supplied-physical equality then main inverse",
                "w07::frozen exact prior material tuple",
                "w07::check physical main joint-frame operand"
            ]),
        "W08 caller inventory differs",
    )?;
    Ok(doc)
}
fn material(pin: &Material, raw: &[u8], mode: u32) -> Result<()> {
    need(
        pin.mode == 0o644
            && mode == pin.mode
            && raw.len() == pin.bytes
            && digest(raw) == pin.sha256,
        "W08 complete source material differs",
    )
}
fn inverse(row: &Row, current: &[u8], mode: u32) -> Result<Vec<u8>> {
    // This full afterimage check intentionally precedes edit interpretation.
    material(&row.current, current, mode)?;
    need(!row.edits.is_empty(), "W08 source inverse is empty")?;
    let mut prior = Vec::new();
    let mut end = 0;
    for edit in &row.edits {
        let at = edit.current_offset;
        let stop = at
            .checked_add(edit.new.len())
            .ok_or("W08 inverse offset overflow")?;
        need(
            at >= end && current.get(at..stop) == Some(edit.new.as_bytes()),
            "W08 ordered exact inverse span differs",
        )?;
        prior.extend_from_slice(&current[end..at]);
        prior.extend_from_slice(edit.old.as_bytes());
        end = stop;
    }
    prior.extend_from_slice(&current[end..]);
    material(&row.prior, &prior, 0o644)?;
    Ok(prior)
}
fn git_parent(root: &Path, row: &Row, prior: &[u8]) -> Result<()> {
    let blob = Command::new("git")
        .args(["cat-file", "blob", &row.prior_git_blob])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let tree = Command::new("git")
        .args(["ls-tree", BASELINE, "--", &row.path])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        blob.status.success()
            && blob.stdout == prior
            && tree.status.success()
            && tree.stdout
                == format!("100644 blob {}\t{}\n", row.prior_git_blob, row.path).as_bytes(),
        "W08 exact predecessor Git bytes/mode/blob differ",
    )
}
fn selected<'a>(doc: &'a Capsule, path: &str) -> Result<&'a Row> {
    doc.rows
        .iter()
        .find(|r| r.path == path)
        .ok_or_else(|| "W08 unlisted source path".into())
}
pub(crate) fn parent(root: &Path, path: &str, supplied: &[u8]) -> Result<Vec<u8>> {
    let doc = load(root)?;
    let row = selected(&doc, path)?;
    let (physical, mode) = regular(root, path)?;
    material(&row.current, &physical, mode)?;
    need(
        supplied == physical,
        "W08 supplied material is not the actual physical source",
    )?;
    let prior = inverse(row, &physical, mode)?;
    git_parent(root, row, &prior)?;
    Ok(prior)
}
/// Return the actual current main buffer after verifying its exact prior frame.
pub(crate) fn physical(root: &Path, path: &str, expected_prior: &str) -> Result<Vec<u8>> {
    let doc = load(root)?;
    let row = selected(&doc, path)?;
    need(
        row.prior.sha256 == expected_prior,
        "W08 physical source caller tuple differs",
    )?;
    let (physical, _) = regular(root, path)?;
    parent(root, path, &physical)?;
    Ok(physical)
}
/// Only an exact named prior tuple may receive a proved older operand. No
/// unlisted path, different expected hash, or old physical buffer is forgiven.
pub(crate) fn frozen_operand(
    root: &Path,
    path: &str,
    expected: &str,
    supplied: &[u8],
) -> Result<Option<Vec<u8>>> {
    if !PATHS.contains(&path) {
        return Ok(None);
    }
    let doc = load(root)?;
    let row = selected(&doc, path)?;
    if row.prior.sha256 != expected {
        return Ok(None);
    }
    Ok(Some(parent(root, path, supplied)?))
}
pub(crate) fn controls(root: &Path) -> Result<()> {
    let doc = load(root)?;
    for frozen in &doc.frozen_predecessors {
        let (raw, mode) = regular(root, &frozen.path)?;
        need(
            mode == 0o644 && digest(&raw) == frozen.sha256,
            "W08 changed a frozen predecessor artifact",
        )?;
    }
    for row in &doc.rows {
        let (physical, mode) = regular(root, &row.path)?;
        let prior = parent(root, &row.path, &physical)?;
        need(
            inverse(row, &physical, mode)? == prior,
            "W08 positive same-buffer inverse differs",
        )?;
        need(
            parent(root, &row.path, &prior).is_err(),
            "W08 accepted old bytes as current physical source",
        )?;
        need(
            inverse(row, &physical, 0o755).is_err(),
            "W08 accepted changed source mode",
        )?;
        let mut changed = physical.clone();
        changed[0] ^= 1;
        let mut duplicate = physical.clone();
        duplicate.extend_from_slice(row.edits[0].new.as_bytes());
        let mut moved = physical.clone();
        let first = &row.edits[0];
        if !first.new.is_empty() {
            moved.drain(first.current_offset..first.current_offset + first.new.len());
            moved.splice(0..0, first.new.as_bytes().iter().copied());
        }
        for bad in [changed, duplicate, moved] {
            need(
                inverse(row, &bad, mode).is_err(),
                "W08 accepted changed/duplicate/moved insertion or unrelated body",
            )?;
        }
        let mut missing = row.clone();
        missing.edits.remove(0);
        need(
            inverse(&missing, &physical, mode).is_err(),
            "W08 accepted a missing inverse descriptor",
        )?;
        let mut duplicate_edit = row.clone();
        duplicate_edit.edits.insert(0, row.edits[0].clone());
        need(
            inverse(&duplicate_edit, &physical, mode).is_err(),
            "W08 accepted a duplicate inverse descriptor",
        )?;
        let mut moved_edit = row.clone();
        moved_edit.edits[0].current_offset += 1;
        need(
            inverse(&moved_edit, &physical, mode).is_err(),
            "W08 accepted a relocated inverse descriptor",
        )?;
        let mut wrong = row.clone();

        wrong.prior.sha256 = "0".repeat(64);
        need(
            inverse(&wrong, &physical, mode).is_err(),
            "W08 accepted altered predecessor tuple",
        )?;
    }
    need(
        parent(root, "unlisted.rs", b"").is_err(),
        "W08 accepted unlisted source",
    )?;
    Ok(())
}
