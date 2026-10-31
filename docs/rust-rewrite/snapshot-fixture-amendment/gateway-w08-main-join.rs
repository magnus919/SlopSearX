//! Closed gateway040/accepted-d9 join. No calls into either historical reader.
#[allow(
    clippy::duplicate_mod,
    reason = "Gateway and CLI/checker proof namespaces share one self-contained stateless physical join leaf"
)]
#[path = "gateway-cli-stacked-join.rs"]
pub(crate) mod stacked;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
pub(crate) const CAPSULE_PATH: &str =
    "docs/rust-rewrite/snapshot-fixture-amendment/gateway-w08-main-join-v1.json";
const CAPSULE_SHA: &str = "9f7b316a40a1aaab0559074c1892541ad0607f7e37dabd245eb1f227aac3ab18";
const GATEWAY_PARENT: &str = "040f504314c805ef11aefcb5bb086875bd7857da";
const MAIN_PARENT: &str = "d9d42277657394e9f62061072a95671345a94f38";
const ROWS: [(&str, usize, usize); 6] = [
    ("crates/xtask/src/main.rs", 3, 5),
    (
        "docs/rust-rewrite/a06/source-amendments/shared-successor.rs",
        6,
        20,
    ),
    (
        "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs",
        15,
        7,
    ),
    ("crates/xtask/src/w07.rs", 3, 26),
    (
        "docs/rust-rewrite/w07/qualification-completion-v1.json",
        2,
        5,
    ),
    (
        "docs/rust-rewrite/t06/source-amendments/page-main-receipt-control-adapter-v1.rs",
        13,
        4,
    ),
];
#[derive(Clone, Copy)]
pub(crate) enum Parent {
    Gateway,
    Main,
}
pub(crate) fn owns(path: &str) -> bool {
    ROWS.iter().any(|row| row.0 == path)
}
fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("Gateway040/d9 join: {why}"))
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
    use std::io::Read;
    let opened = fs::File::open(full).map_err(|e| e.to_string())?;
    let after = opened.metadata().map_err(|e| e.to_string())?;
    need(
        after.is_file() && after.len() <= 1048576,
        "opened regular bounded source",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        need(
            m.dev() == after.dev() && m.ino() == after.ino() && after.mode() & 0o7777 == 0o644,
            "opened source identity/mode",
        )?;
    }
    let mut bytes = Vec::new();
    opened
        .take(1048577)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    need(
        bytes.len() <= 1048576 && bytes.len() as u64 == after.len(),
        "bounded opened source length",
    )?;
    Ok(bytes)
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
fn scope(doc: &Value) -> Result<()> {
    need(
        doc["schema"] == "GATEWAY_040_D9_SOURCE_JOIN_V1"
            && doc.as_object().is_some_and(|fields| fields.len() == 3)
            && doc["parents"] == serde_json::json!({"gateway":GATEWAY_PARENT,"main":MAIN_PARENT})
            && doc["rows"]
                .as_array()
                .is_some_and(|rows| rows.len() == ROWS.len()),
        "closed parents and six-row scope",
    )?;
    for (index, (path, gateway_count, main_count)) in ROWS.iter().enumerate() {
        let row = &doc["rows"][index];
        need(
            row["path"] == *path
                && row.as_object().is_some_and(|fields| fields.len() == 4)
                && row["current"]["path"] == *path
                && row["current"]["mode"] == "0644",
            "exact ordered current path/mode",
        )?;
        for (name, count, commit) in [
            ("gateway", gateway_count, GATEWAY_PARENT),
            ("main", main_count, MAIN_PARENT),
        ] {
            let frame = &row[name];
            need(
                frame["current"] == row["current"]
                    && frame["prior"]["path"] == *path
                    && frame["prior"]["mode"] == "0644"
                    && frame["prior"]["commit"] == commit
                    && frame["edits"]
                        .as_array()
                        .is_some_and(|edits| edits.len() == *count),
                "closed endpoint path/parent/mode/span count",
            )?;
        }
    }
    Ok(())
}
fn capsule(root: &Path) -> Result<Value> {
    source_mode(root, CAPSULE_PATH)?;
    let raw = regular(root, CAPSULE_PATH)?;
    need(
        digest(&raw) == CAPSULE_SHA,
        "compiled outer capsule identity",
    )?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    scope(&doc)?;
    need(
        regular(root, SELF_PATH)? == COMPILED_SELF,
        "compiled join source is not physical current",
    )?;
    source_mode(root, SELF_PATH)?;
    for (path, _, _) in ROWS {
        let physical = regular(root, path)?;
        material(
            &doc["rows"][ROWS.iter().position(|r| r.0 == path).ok_or("row")?]["current"],
            &stacked::project(root, path, &physical, stacked::Parent::Gateway)?,
            source_mode(root, path)?,
        )?;
    }
    Ok(doc)
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
fn interpret(
    doc: &Value,
    path: &str,
    physical: &[u8],
    mode: &str,
    parent: Parent,
) -> Result<Vec<u8>> {
    scope(doc)?;
    let index = ROWS
        .iter()
        .position(|row| row.0 == path)
        .ok_or("unlisted join path")?;
    let row = &doc["rows"][index];
    // Both complete inverses and independent forwards run before either result.
    let cli = inverse(&row["gateway"], physical, mode, ROWS[index].1)?;
    let main = inverse(&row["main"], physical, mode, ROWS[index].2)?;
    Ok(match parent {
        Parent::Gateway => cli,
        Parent::Main => main,
    })
}
/// Exact physical source admission before any logical parent frame is exposed.
pub(crate) fn project(root: &Path, path: &str, supplied: &[u8], parent: Parent) -> Result<Vec<u8>> {
    need(owns(path), "unlisted physical join path")?;
    let mode = source_mode(root, path)?;
    let physical = regular(root, path)?;
    need(
        physical == supplied,
        "join supplied buffer is not physical current",
    )?;
    let logical = stacked::project(root, path, &physical, stacked::Parent::Gateway)?;
    interpret(&capsule(root)?, path, &logical, mode, parent)
}
/// Same controls are reached by owning CLI and W08 composition routes.
pub(crate) fn controls(root: &Path) -> Result<()> {
    stacked::controls(root)?;
    let doc = capsule(root)?;
    for (index, (path, gateway_count, main_count)) in ROWS.iter().enumerate() {
        let physical = regular(root, path)?;
        let logical = stacked::project(root, path, &physical, stacked::Parent::Gateway)?;
        for (name, parent, count, other) in [
            ("gateway", Parent::Gateway, *gateway_count, "main"),
            ("main", Parent::Main, *main_count, "gateway"),
        ] {
            let prior = project(root, path, &physical, parent)?;
            let row = &doc["rows"][index][name];
            need(
                inverse(row, &logical, "0644", count)? == prior,
                "pure outer positive before controls",
            )?;
            let commit = text(&row["prior"], "commit")?;
            let git = Command::new("git")
                .args(["show", &format!("{commit}:{path}")])
                .current_dir(root)
                .output()
                .map_err(|e| e.to_string())?;
            let tree = Command::new("git")
                .args(["ls-tree", commit, "--", path])
                .current_dir(root)
                .output()
                .map_err(|e| e.to_string())?;
            need(
                git.status.success()
                    && git.stdout == prior
                    && tree.status.success()
                    && tree.stdout
                        == format!("100644 blob {}\t{path}\n", text(&row["prior"], "git_blob")?)
                            .as_bytes(),
                "actual exact parent Git bytes/mode/blob",
            )?;
            need(
                prior != physical && project(root, path, &prior, parent).is_err(),
                "parent substituted as physical current",
            )?;
            let mut changed = physical.clone();
            changed.push(0);
            need(
                project(root, path, &changed, parent).is_err(),
                "changed supplied physical bytes",
            )?;
            for mode in ["0600", "0755", "4644"] {
                need(
                    interpret(&doc, path, &logical, mode, parent).is_err(),
                    "outer wrong mode",
                )?;
            }
            for mutation in 0..4 {
                let mut bad = doc.clone();
                let edits = bad["rows"][index][name]["edits"]
                    .as_array_mut()
                    .ok_or("outer edit array")?;
                match mutation {
                    0 => {
                        edits.pop();
                    }
                    1 => {
                        edits.push(edits[0].clone());
                    }
                    2 => edits[0]["at"] = serde_json::json!(usize::MAX),
                    _ => edits[0]["insert_utf8"] = serde_json::json!("changed"),
                }
                need(
                    interpret(&bad, path, &logical, "0644", parent).is_err(),
                    "outer missing/duplicate/moved/changed span",
                )?;
            }
            let mut unselected = doc.clone();
            unselected["rows"][index][other]["prior"]["sha256"] = serde_json::json!("changed");
            need(
                interpret(&unselected, path, &logical, "0644", parent).is_err(),
                "unselected endpoint must also authenticate",
            )?;
        }
    }
    for mutation in 0..4 {
        let mut bad = doc.clone();
        match mutation {
            0 => {
                bad["rows"].as_array_mut().ok_or("rows")?.pop();
            }
            1 => {
                let row = bad["rows"][0].clone();
                bad["rows"].as_array_mut().ok_or("rows")?.push(row);
            }
            2 => bad["rows"][0]["path"] = serde_json::json!("unlisted"),
            _ => bad["parents"]["main"] = serde_json::json!(GATEWAY_PARENT),
        }
        need(
            scope(&bad).is_err(),
            "outer missing/extra/path/parent scope rejected",
        )?;
    }
    need(
        project(root, "unlisted", b"", Parent::Gateway).is_err(),
        "unlisted physical source rejected",
    )
}

const SELF_PATH: &str = "docs/rust-rewrite/snapshot-fixture-amendment/gateway-w08-main-join.rs";
const COMPILED_SELF: &[u8] = include_bytes!("gateway-w08-main-join.rs");
pub(crate) fn validate_current(root: &Path) -> Result<()> {
    capsule(root).map(|_| ())
}
