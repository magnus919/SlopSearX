//! Two-path CLI successor. Physical current bytes precede historical interpretation.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
const CAPSULE: &str = "docs/rust-rewrite/t08a/shared-source-successor.json";
const CAPSULE_SHA: &str = "5cb976646459d885bf056889d24cec52ad9a671dd54b1a7b6b85c0131e9de6cc";
const ROWS: [(&str, usize); 2] = [("crates/xtask/src/main.rs", 6), ("Cargo.lock", 23)];
fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("T08A shared source custody: {why}"))
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn regular(root: &Path, path: &str) -> Result<Vec<u8>> {
    let full = root.join(path);
    let m = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
    need(
        m.is_file() && !m.file_type().is_symlink() && m.len() <= 1048576,
        "regular bounded source leaf",
    )?;
    need(
        full.canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(root.canonicalize().map_err(|e| e.to_string())?),
        "source stays within root",
    )?;
    fs::read(full).map_err(|e| e.to_string())
}
fn source_mode(root: &Path, path: &str) -> Result<&'static str> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        need(
            fs::symlink_metadata(root.join(path))
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o7777
                == 0o644,
            "physical source requires regular0644",
        )?;
    }
    #[cfg(not(unix))]
    {
        // Windows does not expose Unix permission bits. Preserve the committed
        // source mode through the current index and independently reject links.
        let out = Command::new("git")
            .args(["ls-files", "--stage", "--", path])
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        need(
            out.status.success() && String::from_utf8_lossy(&out.stdout).starts_with("100644 "),
            "Windows current source index requires100644",
        )?;
    }
    Ok("0644")
}
fn capsule(root: &Path) -> Result<Value> {
    let raw = regular(root, CAPSULE)?;
    need(digest(&raw) == CAPSULE_SHA, "compiled capsule identity")?;
    let value: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        value["schema"] == "T08A_SHARED_MAIN_LOCK_V1"
            && value["rows"].as_array().is_some_and(|r| r.len() == 2),
        "closed two-path capsule",
    )?;
    for (index, (path, count)) in ROWS.iter().enumerate() {
        let row = &value["rows"][index];
        need(
            row["path"] == *path
                && row["edits"].as_array().is_some_and(|r| r.len() == *count)
                && row["prior"]["mode"] == "0644"
                && row["current"]["mode"] == "0644",
            "closed path/mode/edit count",
        )?;
    }
    Ok(value)
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key].as_str().ok_or_else(|| format!("missing {key}"))
}
fn material(row: &Value, bytes: &[u8], mode: &str) -> Result<()> {
    need(
        row["sha256"] == digest(bytes)
            && row["bytes"].as_u64() == Some(bytes.len() as u64)
            && row["mode"] == mode,
        "exact bytes/length/mode",
    )
}
fn inverse(row: &Value, current: &[u8], mode: &str, count: usize) -> Result<Vec<u8>> {
    material(&row["current"], current, mode)?;
    let edits = row["edits"].as_array().ok_or("ordered edits missing")?;
    need(edits.len() == count, "finite edit count")?;
    let mut prior = Vec::new();
    let (mut old_cursor, mut cursor) = (0usize, 0usize);
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("edit offset")?;
        let remove = text(edit, "remove_utf8")?.as_bytes();
        let insert = text(edit, "insert_utf8")?.as_bytes();
        need(remove != insert, "no no-op edit")?;
        let unchanged = at
            .checked_sub(old_cursor)
            .ok_or("overlapping prior spans")?;
        let end = cursor.checked_add(unchanged).ok_or("prefix overflow")?;
        prior.extend_from_slice(current.get(cursor..end).ok_or("current prefix")?);
        let next = end.checked_add(insert.len()).ok_or("insert overflow")?;
        need(
            current.get(end..next) == Some(insert),
            "exact inserted span",
        )?;
        prior.extend_from_slice(remove);
        old_cursor = at
            .checked_add(remove.len())
            .ok_or("prior offset overflow")?;
        cursor = next;
    }
    prior.extend_from_slice(current.get(cursor..).ok_or("current suffix")?);
    material(&row["prior"], &prior, mode)?;
    let mut forward = Vec::new();
    let mut cursor = 0;
    for edit in edits {
        let at = edit["at"].as_u64().unwrap() as usize;
        let remove = text(edit, "remove_utf8")?.as_bytes();
        let next = at.checked_add(remove.len()).ok_or("forward overflow")?;
        need(
            at >= cursor && prior.get(at..next) == Some(remove),
            "forward ordered span",
        )?;
        forward.extend_from_slice(&prior[cursor..at]);
        forward.extend_from_slice(text(edit, "insert_utf8")?.as_bytes());
        cursor = next;
    }
    forward.extend_from_slice(&prior[cursor..]);
    need(forward == current, "full forward reconstruction")?;
    Ok(prior)
}
/// Returns one exact pre-CLI frame only after authenticating physical current.
/// Historical bytes supplied as current are rejected, even if the caller owns them.
pub(crate) fn prior(root: &Path, path: &str, supplied: &[u8]) -> Result<Vec<u8>> {
    let index = ROWS
        .iter()
        .position(|r| r.0 == path)
        .ok_or("unowned CLI successor path")?;
    let current = regular(root, path)?;
    need(
        current == supplied,
        "supplied bytes are not physical current",
    )?;
    let doc = capsule(root)?;
    inverse(
        &doc["rows"][index],
        &current,
        source_mode(root, path)?,
        ROWS[index].1,
    )
}
/// Portable byte/Git controls. Unix also proves physical permission bits;
/// Windows proves current Git index mode, without claiming POSIX chmod semantics.
fn controls_for(
    root: &Path,
    rows: &[(&str, usize)],
    doc: &Value,
    normalize: fn(&Path, &str, &[u8]) -> Result<Vec<u8>>,
) -> Result<()> {
    for (index, (path, count)) in rows.iter().enumerate() {
        let row = &doc["rows"][index];
        let actual = regular(root, path)?;
        let old = normalize(root, path, &actual)?;
        let commit = text(&row["prior"], "commit")?;
        let git = Command::new("git")
            .args(["show", &format!("{commit}:{path}")])
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        need(
            git.status.success() && git.stdout == old,
            "exact pre-CLI Git bytes",
        )?;
        let tree = Command::new("git")
            .args(["ls-tree", commit, "--", path])
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        need(
            tree.status.success()
                && tree.stdout
                    == format!("100644 blob {}\t{path}\n", text(&row["prior"], "git_blob")?)
                        .as_bytes(),
            "pre-CLI Git mode/blob",
        )?;
        need(
            normalize(root, path, &old).is_err(),
            "historical frame rejected as supplied current",
        )?;
        let mut changed = actual.clone();
        changed.push(b'\n');
        need(
            normalize(root, path, &changed).is_err(),
            "changed supplied current",
        )?;
        for mode in ["0600", "0755", "4644"] {
            need(
                inverse(row, &actual, mode, *count).is_err(),
                "mode substitution",
            )?;
        }
        for mutation in 0..3 {
            let mut wrong = row.clone();
            let edits = wrong["edits"].as_array_mut().unwrap();
            match mutation {
                0 => {
                    edits.pop();
                }
                1 => {
                    edits.push(edits[0].clone());
                }
                _ => {
                    edits[0]["at"] = serde_json::json!(usize::MAX);
                }
            }
            need(
                inverse(&wrong, &actual, "0644", *count).is_err(),
                "missing/duplicate/overflow span",
            )?;
        }
    }
    Ok(())
}

// Separate closed legacy reader admission; the original main/lock ROWS remain2.
pub(crate) const LEGACY_CAPSULE_PATH: &str = "docs/rust-rewrite/t08a/legacy-input-successor.json";
const LEGACY_CAPSULE_SHA: &str = "0baa1d08654a179f2c5446ac9060b7bf9c91b39d6a765720e28358096336f507";
const LEGACY_ROWS: [(&str, usize); 4] = [
    ("Cargo.toml", 1),
    (
        "docs/rust-rewrite/a06/source-amendments/shared-successor.rs",
        1,
    ),
    (
        "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs",
        5,
    ),
    ("crates/xtask/src/a13.rs", 1),
];
fn legacy_capsule(root: &Path) -> Result<Value> {
    let raw = regular(root, LEGACY_CAPSULE_PATH)?;
    need(
        digest(&raw) == LEGACY_CAPSULE_SHA,
        "compiled legacy capsule identity",
    )?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "T08A_FOUR_LEGACY_INPUTS_V1"
            && doc["rows"].as_array().is_some_and(|rows| rows.len() == 4),
        "closed legacy four-path schema",
    )?;
    for (index, (path, count)) in LEGACY_ROWS.iter().enumerate() {
        let row = &doc["rows"][index];
        need(
            row["path"] == *path
                && row["prior"]["mode"] == "0644"
                && row["current"]["mode"] == "0644"
                && row["edits"]
                    .as_array()
                    .is_some_and(|edits| edits.len() == *count),
            "exact legacy path/modes/edit count",
        )?;
    }
    Ok(doc)
}
pub(crate) fn legacy_prior(root: &Path, path: &str, supplied: &[u8]) -> Result<Vec<u8>> {
    let index = LEGACY_ROWS
        .iter()
        .position(|row| row.0 == path)
        .ok_or("unowned legacy successor path")?;
    let physical = regular(root, path)?;
    need(
        physical == supplied,
        "legacy supplied is not physical current",
    )?;
    let doc = legacy_capsule(root)?;
    inverse(
        &doc["rows"][index],
        &physical,
        source_mode(root, path)?,
        LEGACY_ROWS[index].1,
    )
}
pub(crate) fn controls(root: &Path) -> Result<()> {
    controls_for(root, &ROWS, &capsule(root)?, prior)?;
    controls_for(root, &LEGACY_ROWS, &legacy_capsule(root)?, legacy_prior)
}
