//! The existing sealed fcac -> a1b search transition, shared by A06 and T06.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
pub(crate) const SOURCE: &str = "crates/ssx-infra/src/search.rs";
const OLD: &str = "fcac2929eb6637960d5e1086c321f0eac62fc0f8446cf73f22d6b43406137b12";
const CURRENT: &str = "a1b310e2ef8692c96109bfd780c0e37d7e50692231e11eceb48e05860e0b5927";
const CAPSULE: &str = "docs/rust-rewrite/a06/source-amendments/post-a06-generation-v1.json";
const CAPSULE_SHA: &str = "5b7ff0786855e613a9da549aeaab63c387061c03657053e2cf4a6eaade03609b";
const ARCHIVE: &str = "docs/rust-rewrite/a06/source-amendments/post-a06-233/search.rs.frozen";
const COMMIT: &str = "233c7934f9e2da5c2e94f0c1213945335d786502";
const BLOB: &str = "793b3a7edbbddead1866f72f97cc3bf5c789f9e7";
fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("T06 exact search successor: {why}"))
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
#[derive(Clone)]
struct Material {
    bytes: Vec<u8>,
    mode: u32,
}
#[derive(Clone)]
struct Inputs {
    archive: Material,
    current: Material,
    git_archive: Vec<u8>,
    git_tree: Vec<u8>,
}
fn read(root: &Path, path: &str) -> Result<Material> {
    let full = root.join(path);
    let meta = fs::symlink_metadata(&full).map_err(|e| format!("{path}: {e}"))?;
    need(
        meta.is_file() && !meta.file_type().is_symlink(),
        "regular non-symlink material",
    )?;
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o7777
    };
    #[cfg(not(unix))]
    return Err("T06 source mode proof requires Unix".into());
    #[cfg(unix)]
    Ok(Material {
        bytes: fs::read(full).map_err(|e| e.to_string())?,
        mode,
    })
}
fn row(raw: &[u8]) -> Result<Value> {
    need(
        raw.len() == 124661 && digest(raw) == CAPSULE_SHA,
        "sealed existing capsule",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "T02_POST_A06_CLOSED_GENERATION_V1"
            && doc["source_parent"] == COMMIT
            && doc["continuation_head"] == "4de0d641ea6e80b398f2cc12b46b2c818608bbac"
            && doc["rows"].as_array().map(Vec::len) == Some(24)
            && doc["callers"].as_array().map(Vec::len) == Some(77),
        "closed capsule membership",
    )?;
    let rows: Vec<_> = doc["rows"]
        .as_array()
        .ok_or("search rows absent")?
        .iter()
        .filter(|r| r["previous"]["path"] == SOURCE && r["previous"]["sha256"] == OLD)
        .collect();
    need(rows.len() == 1, "one exact historical search tuple")?;
    Ok(rows[0].clone())
}
fn reconstruct(prior: &[u8], edits: &Value) -> Result<Vec<u8>> {
    let edits = edits.as_array().ok_or("search edits absent")?;
    need(edits.len() == 6, "six literal search edits")?;
    let mut out = Vec::new();
    let mut end = 0;
    let mut previous = None;
    for e in edits {
        let at = e["at"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or("search edit offset")?;
        let remove = e["remove_utf8"]
            .as_str()
            .ok_or("search edit preimage")?
            .as_bytes();
        let insert = e["insert_utf8"]
            .as_str()
            .ok_or("search edit insertion")?
            .as_bytes();
        let next = at.checked_add(remove.len()).ok_or("search edit overflow")?;
        need(
            at >= end
                && previous.is_none_or(|p| at > p)
                && next <= prior.len()
                && prior[at..next] == *remove
                && (!remove.is_empty() || !insert.is_empty()),
            "ordered literal edit preimages",
        )?;
        out.extend_from_slice(&prior[end..at]);
        out.extend_from_slice(insert);
        end = next;
        previous = Some(at);
    }
    out.extend_from_slice(&prior[end..]);
    Ok(out)
}
fn proof(row: &Value, m: &Inputs) -> Result<()> {
    need(
        row["previous"] == json!({"path":SOURCE,"sha256":OLD,"bytes":86121,"mode":"0644"})
            && row["archive"] == json!({"path":ARCHIVE,"sha256":OLD,"bytes":86121,"mode":"0644"})
            && row["current"]
                == json!({"path":SOURCE,"sha256":CURRENT,"bytes":86610,"mode":"0644"})
            && row["source_commit"] == COMMIT
            && row["source_blob"] == BLOB,
        "exact transition operands",
    )?;
    need(
        m.archive.mode == 0o644 && m.current.mode == 0o644,
        "exact archive/current modes",
    )?;
    need(
        m.archive.bytes.len() == 86121
            && digest(&m.archive.bytes) == OLD
            && m.current.bytes.len() == 86610
            && digest(&m.current.bytes) == CURRENT,
        "exact archive/current bytes",
    )?;
    need(
        m.git_archive == m.archive.bytes
            && m.git_tree == format!("100644 blob {BLOB}\t{SOURCE}\n").as_bytes(),
        "historical Git bytes/tree/blob",
    )?;
    need(
        reconstruct(&m.archive.bytes, &row["edits"])? == m.current.bytes,
        "exact current reconstruction",
    )
}
fn load(root: &Path) -> Result<(Value, Inputs)> {
    let raw = read(root, CAPSULE)?;
    need(raw.mode == 0o644, "capsule mode")?;
    let row = row(&raw.bytes)?;
    let git = |args: &[&str]| -> Result<Vec<u8>> {
        let out = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        need(out.status.success(), "Git proof command")?;
        Ok(out.stdout)
    };
    let m = Inputs {
        archive: read(root, ARCHIVE)?,
        current: read(root, SOURCE)?,
        git_archive: git(&["show", &format!("{COMMIT}:{SOURCE}")])?,
        git_tree: git(&["ls-tree", COMMIT, "--", SOURCE])?,
    };
    proof(&row, &m)?;
    Ok((row, m))
}
/// Used by the original post-A06 historical route after its sealed tuple selection.
pub(crate) fn materials_for_row(root: &Path, expected: &Value) -> Result<(Vec<u8>, Vec<u8>)> {
    let (sealed, m) = load(root)?;
    need(expected == &sealed, "exact supplied transition row")?;
    Ok((m.archive.bytes, m.current.bytes))
}
/// Only this exact T06 historical operand can use the existing physical successor.
pub(crate) fn verify(root: &Path, path: &str, expected: &str, supplied: &[u8]) -> Result<()> {
    need(path == SOURCE && expected == OLD, "exact historical caller")?;
    let raw = read(root, CAPSULE)?;
    need(raw.mode == 0o644, "capsule mode")?;
    let (_, current) = materials_for_row(root, &row(&raw.bytes)?)?;
    need(supplied == current, "supplied current material")
}
/// Additive controls called by the existing T06 identities, without new test names.
#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<()> {
    let (r, good) = load(root)?;
    verify(root, SOURCE, OLD, &good.current.bytes)?;
    need(
        verify(root, "Cargo.lock", OLD, &good.current.bytes).is_err(),
        "wrong path accepted",
    )?;
    need(
        verify(root, SOURCE, CURRENT, &good.current.bytes).is_err(),
        "current hash as historical caller accepted",
    )?;
    need(
        verify(root, SOURCE, "wrong", &good.current.bytes).is_err(),
        "unknown caller accepted",
    )?;
    need(
        verify(root, SOURCE, OLD, &good.archive.bytes).is_err(),
        "historical material as current accepted",
    )?;
    need(row(b"{}").is_err(), "unsealed capsule accepted")?;
    for mode in [0o600, 0o4644] {
        let mut bad = good.clone();
        bad.current.mode = mode;
        need(proof(&r, &bad).is_err(), "current mode accepted")?;
        let mut bad = good.clone();
        bad.archive.mode = mode;
        need(proof(&r, &bad).is_err(), "archive mode accepted")?;
    }
    let mut bad = good.clone();
    bad.current.bytes[0] ^= 1;
    need(proof(&r, &bad).is_err(), "changed current accepted")?;
    let mut bad = good.clone();
    bad.archive.bytes[0] ^= 1;
    need(proof(&r, &bad).is_err(), "changed archive accepted")?;
    let mut bad = good.clone();
    bad.git_archive[0] ^= 1;
    need(proof(&r, &bad).is_err(), "changed Git bytes accepted")?;
    let mut bad = good.clone();
    bad.git_tree[0] ^= 1;
    need(proof(&r, &bad).is_err(), "changed Git mode/blob accepted")?;
    let mut altered_row = r.clone();
    altered_row["source_blob"] = json!("wrong blob");
    need(
        materials_for_row(root, &altered_row).is_err(),
        "altered supplied transition row accepted",
    )?;
    let mut bad = r.clone();
    bad["current"]["sha256"] = json!(OLD);
    need(proof(&bad, &good).is_err(), "changed transition accepted")?;
    let mut bad = r.clone();
    bad["edits"][0]["remove_utf8"] = json!("wrong preimage");
    need(proof(&bad, &good).is_err(), "changed preimage accepted")?;
    let mut bad = r.clone();
    bad["edits"][0]["insert_utf8"] = json!("wrong insertion");
    need(proof(&bad, &good).is_err(), "changed insertion accepted")?;
    let mut bad = r.clone();
    bad["edits"].as_array_mut().unwrap().swap(0, 1);
    need(proof(&bad, &good).is_err(), "unordered edits accepted")?;
    let mut bad = r.clone();
    bad["edits"].as_array_mut().unwrap().pop();
    need(proof(&bad, &good).is_err(), "missing edit accepted")?;
    Ok(())
}
