//! Exact qualification-only receipt-control successor; frozen operands stay intact.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
const CAPSULE: &str = "docs/rust-rewrite/t06/source-amendments/receipt-control-successor.json";
const CAPSULE_SHA: &str = "4bae103d6972ba4aca8acc456ec73d9ee242d2983711f18967b72abafd196d4a";
const BASELINE: &str = "e9c5911a468b892fe2adef9027374cb867a2535d";
const PATHS: &[&str] = &[
    "crates/ssx-mcp/src/bin/search-qualification.rs",
    "crates/ssx-mcp/tests/t06_support/server.rs",
    "crates/xtask/src/t06.rs",
    "crates/ssx-mcp/tests/t06_source_bindings.rs",
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
struct Frozen {
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capsule {
    schema_version: u32,
    baseline: String,
    rows: Vec<Row>,
    frozen_predecessors: Vec<Frozen>,
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
        !Path::new(path).is_absolute() && !path.split('/').any(|part| part == ".."),
        "receipt-control unsafe path",
    )?;
    let path = root.join(path);
    let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        meta.is_file() && !meta.file_type().is_symlink(),
        "receipt-control source must be regular",
    )?;
    use std::os::unix::fs::PermissionsExt;
    Ok((
        fs::read(path).map_err(|e| e.to_string())?,
        meta.permissions().mode() & 0o7777,
    ))
}
fn load(root: &Path) -> Result<Capsule> {
    let (raw, mode) = regular(root, CAPSULE)?;
    need(
        mode == 0o644 && digest(&raw) == CAPSULE_SHA,
        "receipt-control capsule changed",
    )?;
    let capsule: Capsule = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        capsule.schema_version == 1
            && capsule.baseline == BASELINE
            && capsule
                .rows
                .iter()
                .map(|row| row.path.as_str())
                .eq(PATHS.iter().copied()),
        "receipt-control closed membership/baseline differs",
    )?;
    Ok(capsule)
}
fn material(pin: &Material, raw: &[u8], mode: u32) -> Result<()> {
    need(
        pin.mode == 0o644
            && mode == pin.mode
            && raw.len() == pin.bytes
            && digest(raw) == pin.sha256,
        "receipt-control complete source bytes/mode differ",
    )
}
fn inverse(row: &Row, current: &[u8], mode: u32) -> Result<Vec<u8>> {
    material(&row.current, current, mode)?;
    need(!row.edits.is_empty(), "receipt-control empty inverse")?;
    let mut prior = Vec::new();
    let mut end = 0;
    for edit in &row.edits {
        let stop = edit
            .current_offset
            .checked_add(edit.new.len())
            .ok_or("receipt-control offset overflow")?;
        need(
            edit.current_offset >= end
                && current.get(edit.current_offset..stop) == Some(edit.new.as_bytes()),
            "receipt-control exact ordered span differs",
        )?;
        prior.extend_from_slice(&current[end..edit.current_offset]);
        prior.extend_from_slice(edit.old.as_bytes());
        end = stop;
    }
    prior.extend_from_slice(&current[end..]);
    material(&row.prior, &prior, 0o644)?;
    Ok(prior)
}
pub(crate) fn parent(
    root: &Path,
    path: &str,
    expected: &str,
    supplied: &[u8],
) -> Result<Option<Vec<u8>>> {
    if !PATHS.contains(&path) {
        return Ok(None);
    }
    let capsule = load(root)?;
    let row = capsule
        .rows
        .iter()
        .find(|row| row.path == path)
        .ok_or("receipt-control unlisted source")?;
    if row.prior.sha256 != expected {
        return Ok(None);
    }
    let (physical, mode) = regular(root, path)?;
    material(&row.current, &physical, mode)?;
    need(
        supplied == physical,
        "receipt-control supplied buffer is not actual physical source",
    )?;
    let prior = inverse(row, &physical, mode)?;
    let blob = Command::new("git")
        .args(["cat-file", "blob", &row.prior_git_blob])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let tree = Command::new("git")
        .args(["ls-tree", BASELINE, "--", path])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        blob.status.success()
            && blob.stdout == prior
            && tree.status.success()
            && tree.stdout == format!("100644 blob {}\t{path}\n", row.prior_git_blob).as_bytes(),
        "receipt-control exact predecessor Git bytes/mode/blob differ",
    )?;
    Ok(Some(prior))
}
pub(crate) fn reader_parent(
    root: &Path,
    path: &str,
    expected: &str,
    supplied: &[u8],
) -> Result<Option<Vec<u8>>> {
    if !PATHS[2..].contains(&path) {
        return Ok(None);
    }
    parent(root, path, expected, supplied)
}
pub(crate) fn controls(root: &Path) -> Result<()> {
    let capsule = load(root)?;
    for frozen in &capsule.frozen_predecessors {
        let (raw, mode) = regular(root, &frozen.path)?;
        need(
            mode == 0o644 && digest(&raw) == frozen.sha256,
            "receipt-control frozen predecessor changed",
        )?;
    }
    for row in &capsule.rows {
        let (physical, mode) = regular(root, &row.path)?;
        let prior = parent(root, &row.path, &row.prior.sha256, &physical)?
            .ok_or("receipt-control positive tuple absent")?;
        need(
            inverse(row, &physical, mode)? == prior,
            "receipt-control positive inverse differs",
        )?;
        need(
            parent(root, &row.path, &row.prior.sha256, &prior).is_err(),
            "receipt-control old bytes accepted as current",
        )?;
        need(
            inverse(row, &physical, 0o755).is_err(),
            "receipt-control wrong mode accepted",
        )?;
        let mut changed = physical.clone();
        changed[0] ^= 1;
        let mut extra = physical.clone();
        extra.push(b' ');
        for bad in [changed, extra] {
            need(
                inverse(row, &bad, mode).is_err(),
                "receipt-control changed/extra bytes accepted",
            )?;
        }
        let mut missing = row.clone();
        missing.edits.remove(0);
        need(
            inverse(&missing, &physical, mode).is_err(),
            "receipt-control missing inverse accepted",
        )?;
        let mut duplicate = row.clone();
        duplicate.edits.insert(0, row.edits[0].clone());
        need(
            inverse(&duplicate, &physical, mode).is_err(),
            "receipt-control duplicate inverse accepted",
        )?;
        let mut moved = row.clone();
        moved.edits[0].current_offset += 1;
        need(
            inverse(&moved, &physical, mode).is_err(),
            "receipt-control relocated inverse accepted",
        )?;
        need(
            parent(root, &row.path, &"0".repeat(64), &physical)?.is_none(),
            "receipt-control wrong caller hash accepted",
        )?;
    }
    need(
        parent(root, "unlisted.rs", "", b"")?.is_none(),
        "receipt-control unlisted caller accepted",
    )
}
