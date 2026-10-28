//! Closed physical-current custody and finite inverse for two W07 source entries.
//! The physical read and the pure inverse remain separate composition stages.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
type Result<T> = std::result::Result<T, String>;
const CAPSULE: &str = "docs/rust-rewrite/snapshot-fixture-amendment/w07-helpers.json";
const SHA: &str = "8889f54d073bc27811febf1bbc6bafdbea1c25303a0f1c0dfff8bff04daaa831";
const JOIN: &str = "docs/rust-rewrite/snapshot-fixture-amendment/gateway-join-v1.json";
const JOIN_SHA: &str = "f826b89eed90510bf15526e243781bfb048dbb60b1d60d753cee683091abac9c";
const CONTAINER: &str = "docs/rust-rewrite/w07/qualification-completion-v1.json";
const CONTAINER_SHA: &str = "5be08978822440ec7bece674b13647c6337eaa4636cc232a1622738a90ac1b64";
const MAIN: &str = "crates/xtask/src/main.rs";
const SOURCES: [&str; 2] = [
    "docs/rust-rewrite/a06/source-amendments/shared-successor.rs",
    "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs",
];
fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("W07 snapshot helper custody: {why}"))
    }
}
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str> {
    row[key].as_str().ok_or_else(|| format!("missing {key}"))
}
fn regular(root: &Path, path: &str) -> Result<Vec<u8>> {
    use std::io::Read;
    let full = root.join(path);
    let before = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
    need(
        before.is_file() && !before.file_type().is_symlink() && before.len() <= 262144,
        "bounded regular leaf",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        need(before.mode() & 0o7777 == 0o644, "regular0644 leaf")?;
        let opened = fs::File::open(full).map_err(|e| e.to_string())?;
        let after = opened.metadata().map_err(|e| e.to_string())?;
        need(
            after.is_file()
                && before.dev() == after.dev()
                && before.ino() == after.ino()
                && after.mode() & 0o7777 == 0o644,
            "opened leaf custody",
        )?;
        let mut bytes = vec![];
        opened
            .take(262145)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        need(
            bytes.len() <= 262144 && bytes.len() as u64 == after.len(),
            "finite opened length",
        )?;
        Ok(bytes)
    }
    #[cfg(not(unix))]
    {
        Err("W07 helper source proof requires Unix".into())
    }
}
fn material(row: &Value, bytes: &[u8], mode: &str) -> Result<()> {
    need(
        row["sha256"] == sha(bytes)
            && row["bytes"].as_u64() == Some(bytes.len() as u64)
            && row["mode"] == mode,
        "exact bytes/length/mode",
    )
}
fn document(root: &Path) -> Result<Value> {
    let raw = regular(root, CAPSULE)?;
    need(sha(&raw) == SHA, "sealed capsule")?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "SNAPSHOT_W07_TWO_HELPER_SUCCESSOR_V1"
            && doc["prior_commit"] == "e9c5911a468b892fe2adef9027374cb867a2535d"
            && doc["container"]["path"] == CONTAINER
            && doc["container"]["sha256"] == CONTAINER_SHA,
        "closed capsule frame",
    )?;
    let rows = doc["rows"].as_array().ok_or("helper rows absent")?;
    need(
        rows.len() == 2
            && rows
                .iter()
                .zip(SOURCES)
                .all(|(r, p)| r["prior"]["path"] == p && r["current"]["path"] == p),
        "closed two-source set",
    )?;
    Ok(doc)
}
fn caller(root: &Path, doc: &Value, row: &Value, path: &str, expected: &str) -> Result<()> {
    need(
        SOURCES.contains(&path)
            && row["prior"]["path"] == path
            && row["prior"]["sha256"] == expected
            && row["caller_entry"] == serde_json::json!({path:expected}),
        "exact caller path/pin/full entry",
    )?;
    let physical = regular(root, CONTAINER)?;
    let raw = composition_parent(root, CONTAINER, &physical, false)?;
    material(&doc["container"], &raw, "0644")?;
    let frozen: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let pointer = format!("/sources/{}", path.replace('~', "~0").replace('/', "~1"));
    need(
        row["pointer"] == pointer
            && frozen.pointer(&pointer) == Some(&Value::String(expected.into())),
        "original pointer entry",
    )
}
// Pure closed inverse: accepts a sealed baseline operand, never claims it was physical.
fn inverse(row: &Value, current: &[u8], mode: &str) -> Result<Vec<u8>> {
    material(&row["current"], current, mode)?;
    let edits = row["edits"].as_array().ok_or("helper edits absent")?;
    let mut prior = vec![];
    let (mut old, mut cursor) = (0usize, 0usize);
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("helper offset")?;
        let remove = text(edit, "remove_utf8")?.as_bytes();
        let insert = text(edit, "insert_utf8")?.as_bytes();
        need(!remove.is_empty() || !insert.is_empty(), "nonvacuous edit")?;
        let end = cursor
            .checked_add(at.checked_sub(old).ok_or("ordered helper edits")?)
            .ok_or("helper span overflow")?;
        prior.extend_from_slice(current.get(cursor..end).ok_or("helper prefix")?);
        let next = end
            .checked_add(insert.len())
            .ok_or("helper insertion overflow")?;
        need(
            current.get(end..next) == Some(insert),
            "exact helper inserted span",
        )?;
        prior.extend_from_slice(remove);
        old = at
            .checked_add(remove.len())
            .ok_or("helper removal overflow")?;
        cursor = next;
    }
    prior.extend_from_slice(current.get(cursor..).ok_or("helper suffix")?);
    material(&row["prior"], &prior, "0644")?;
    let mut forward = vec![];
    let mut end = 0;
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("forward offset")?;
        let remove = text(edit, "remove_utf8")?.as_bytes();
        let next = at
            .checked_add(remove.len())
            .ok_or("forward span overflow")?;
        need(
            at >= end && prior.get(at..next) == Some(remove),
            "exact forward removal",
        )?;
        forward.extend_from_slice(prior.get(end..at).ok_or("forward prefix")?);
        forward.extend_from_slice(text(edit, "insert_utf8")?.as_bytes());
        end = next;
    }
    forward.extend_from_slice(prior.get(end..).ok_or("forward suffix")?);
    need(forward == current, "independent forward proof")?;
    Ok(prior)
}
// Only this reviewed two-helper/current-registry union may precede the original proof.
fn composition_document(root: &Path) -> Result<Value> {
    let raw = regular(root, JOIN)?;
    need(sha(&raw) == JOIN_SHA, "sealed Gateway/snapshot join")?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "SNAPSHOT_GATEWAY_FOUR_OPERAND_JOIN_V1"
            && doc["parents"]
                == serde_json::json!({"gateway":"059de057e7fc023cde8deed210db7b05d0e7d332","snapshot":"8e71a83f9649d58e9a60e34810275b736e44151a"})
            && doc["unchanged_capsules"]
                == serde_json::json!({"gateway":"3e5cb86a53e2060cd9782a812a3834ea23bea96fd0fb4c1f1b5652035a209e82","snapshot":"e16ba1482b12b55a42a4372f3fd4a7116effaf0fa04c79cf5cb13f01930eea4e","helpers":SHA}),
        "closed join parents/capsules",
    )?;
    let rows = doc["helpers"].as_array().ok_or("join helper rows")?;
    need(
        rows.len() == 2
            && rows.iter().zip(SOURCES).all(|(row, path)| {
                row["current"]["path"] == path
                    && ["gateway", "snapshot"].iter().all(|parent| {
                        row[parent]["current"] == row["current"]
                            && row[parent]["prior"]["path"] == path
                    })
            }),
        "closed two-helper join membership",
    )?;
    let main = &doc["main"];
    need(
        main["current"]["path"] == MAIN
            && ["gateway", "snapshot"].iter().all(|parent| {
                main[parent]["current"] == main["current"] && main[parent]["prior"]["path"] == MAIN
            }),
        "closed gateway main join frame",
    )?;
    let row = &doc["registry"];
    need(
        row["current"]["path"] == CONTAINER
            && row["snapshot"]["prior"]["sha256"] == CONTAINER_SHA
            && ["gateway", "snapshot"].iter().all(|parent| {
                row[parent]["current"] == row["current"]
                    && row[parent]["prior"]["path"] == CONTAINER
            }),
        "closed registry join frame",
    )?;
    Ok(doc)
}
fn composition_row<'a>(doc: &'a Value, path: &str) -> Result<&'a Value> {
    if path == CONTAINER {
        return Ok(&doc["registry"]);
    }
    if path == MAIN {
        return Ok(&doc["main"]);
    }
    need(SOURCES.contains(&path), "unlisted join path")?;
    doc["helpers"]
        .as_array()
        .ok_or("join rows")?
        .iter()
        .find(|row| row["current"]["path"] == path)
        .ok_or("join source absent".into())
}
fn composition_parent(root: &Path, path: &str, supplied: &[u8], gateway: bool) -> Result<Vec<u8>> {
    let doc = composition_document(root)?;
    let row = composition_row(&doc, path)?;
    let physical = regular(root, path)?;
    need(
        physical == supplied,
        "join supplied bytes are not physical current",
    )?;
    material(&row["current"], &physical, "0644")?;
    // Both frames must reconstruct before either one is returned to an old reader.
    let gateway_prior = inverse(&row["gateway"], &physical, "0644")?;
    let snapshot_prior = inverse(&row["snapshot"], &physical, "0644")?;
    Ok(if gateway {
        gateway_prior
    } else {
        snapshot_prior
    })
}
// These are the only adapters used outside the two-helper W07 owner.
pub(crate) fn gateway_main(root: &Path, supplied: &[u8]) -> Result<Vec<u8>> {
    composition_parent(root, MAIN, supplied, true)
}
pub(crate) fn snapshot_main(root: &Path, supplied: &[u8]) -> Result<Vec<u8>> {
    composition_parent(root, MAIN, supplied, false)
}
pub(crate) fn verify(
    root: &Path,
    path: &str,
    expected: &str,
    supplied: &[u8],
) -> Option<Result<Vec<u8>>> {
    if !SOURCES.contains(&path) {
        return None;
    }
    Some((|| {
        let doc = document(root)?;
        let row = doc["rows"]
            .as_array()
            .ok_or("rows absent")?
            .iter()
            .find(|r| r["prior"]["path"] == path)
            .ok_or("unlisted helper")?;
        let physical = regular(root, path)?;
        need(
            physical == supplied,
            "supplied differs from physical current",
        )?;
        let join = composition_document(root)?;
        let joined = composition_row(&join, path)?;
        let baseline = composition_parent(root, path, &physical, false)?;
        if joined["current"]["sha256"] == expected {
            let container = regular(root, CONTAINER)?;
            material(&join["registry"]["current"], &container, "0644")?;
            let container: Value = serde_json::from_slice(&container).map_err(|e| e.to_string())?;
            let pointer = format!("/sources/{}", path.replace('~', "~0").replace('/', "~1"));
            need(
                container.pointer(&pointer) == Some(&Value::String(expected.into())),
                "exact union container pointer/current pin",
            )?;
            return Ok(physical);
        }
        caller(root, &doc, row, path, expected)?;
        inverse(row, &baseline, "0644")
    })())
}
/// Ordinary controls, called by an xtask owner; no frozen suite test IDs.
#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<()> {
    let doc = document(root)?;
    for row in doc["rows"].as_array().ok_or("rows absent")? {
        let path = text(&row["prior"], "path")?;
        let expected = text(&row["prior"], "sha256")?;
        let physical = regular(root, path)?;
        let baseline = composition_parent(root, path, &physical, false)?;
        let joined = composition_document(root)?;
        let current_pin = text(&composition_row(&joined, path)?["current"], "sha256")?;
        need(
            verify(root, path, current_pin, &physical).ok_or("current helper absent")?? == physical,
            "closed current helper route positive",
        )?;
        let prior = verify(root, path, expected, &physical).ok_or("selected helper absent")??;
        need(
            inverse(row, &baseline, "0644")? == prior,
            "original pure inverse positive in snapshot frame",
        )?;
        let git = std::process::Command::new("git")
            .args([
                "show",
                &format!("e9c5911a468b892fe2adef9027374cb867a2535d:{path}"),
            ])
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        need(
            git.status.success() && git.stdout == prior,
            "accepted e9 Git bytes",
        )?;
        need(
            verify(root, path, "wrong", &physical).unwrap().is_err()
                && verify(root, path, expected, &prior).unwrap().is_err(),
            "wrong pin/old supplied bytes",
        )?;
        for mode in ["0600", "0755", "4644"] {
            need(inverse(row, &baseline, mode).is_err(), "wrong mode")?;
        }
        let mut bad = row.clone();
        bad["pointer"] = serde_json::json!("/sources/wrong");
        need(
            caller(root, &doc, &bad, path, expected).is_err(),
            "moved caller",
        )?;
        bad = row.clone();
        bad["caller_entry"] = serde_json::json!({"wrong":expected});
        need(
            caller(root, &doc, &bad, path, expected).is_err(),
            "wrong full entry",
        )?;
        let mut wrong_container = doc.clone();
        wrong_container["container"]["sha256"] = serde_json::json!("wrong");
        need(
            caller(root, &wrong_container, row, path, expected).is_err(),
            "wrong sealed container pin",
        )?;
        let mut wrong_length = row.clone();
        wrong_length["current"]["bytes"] = serde_json::json!(0);
        need(
            inverse(&wrong_length, &baseline, "0644").is_err(),
            "wrong source length",
        )?;
        let mut changed = baseline.clone();
        changed.push(b'\n');
        bad = row.clone();
        bad["current"]["sha256"] = serde_json::json!(sha(&changed));
        bad["current"]["bytes"] = serde_json::json!(changed.len());
        need(
            inverse(&bad, &changed, "0644").is_err(),
            "unrelated rehashed source",
        )?;
        for mutation in 0..3 {
            bad = row.clone();
            let edits = bad["edits"].as_array_mut().ok_or("edits absent")?;
            match mutation {
                0 => {
                    edits.pop();
                }
                1 => {
                    edits.push(edits[0].clone());
                }
                _ => {
                    edits[0]["at"] = serde_json::json!(0);
                }
            }
            need(
                inverse(&bad, &baseline, "0644").is_err(),
                "missing/duplicate/moved edit",
            )?;
        }
    }
    need(
        verify(root, "unknown", "wrong", b"").is_none(),
        "unlisted path",
    )?;
    composition_controls(root)?;
    filesystem_controls(root, &doc)
}
#[cfg(test)]
fn composition_controls(root: &Path) -> Result<()> {
    let doc = composition_document(root)?;
    for path in [SOURCES[0], SOURCES[1], MAIN, CONTAINER] {
        let row = composition_row(&doc, path)?;
        let physical = regular(root, path)?;
        for (name, gateway) in [("gateway", true), ("snapshot", false)] {
            let prior = composition_parent(root, path, &physical, gateway)?;
            need(
                inverse(&row[name], &physical, "0644")? == prior,
                "join pure positive frame",
            )?;
            let commit = text(&doc["parents"], name)?;
            let git = std::process::Command::new("git")
                .args(["show", &format!("{commit}:{path}")])
                .current_dir(root)
                .output()
                .map_err(|e| e.to_string())?;
            need(
                git.status.success() && git.stdout == prior,
                "exact Gateway/snapshot parent Git bytes",
            )?;
            let tree = std::process::Command::new("git")
                .args(["ls-tree", commit, "--", path])
                .current_dir(root)
                .output()
                .map_err(|e| e.to_string())?;
            need(
                tree.status.success()
                    && tree.stdout
                        == format!(
                            "100644 blob {}\t{path}\n",
                            text(&row[name]["prior"], "git_blob")?
                        )
                        .as_bytes(),
                "exact join parent Git mode/blob",
            )?;
            need(
                composition_parent(root, path, &prior, gateway).is_err(),
                "either join parent supplied as physical",
            )?;
            for mode in ["0600", "0755", "4644"] {
                need(
                    inverse(&row[name], &physical, mode).is_err(),
                    "join wrong mode",
                )?;
            }
            for mutation in 0..3 {
                let mut bad = row[name].clone();
                let edits = bad["edits"].as_array_mut().ok_or("join changes")?;
                match mutation {
                    0 => {
                        edits.pop();
                    }
                    1 => {
                        edits.push(edits[0].clone());
                    }
                    _ => {
                        edits[0]["at"] = serde_json::json!(0);
                    }
                }
                need(
                    inverse(&bad, &physical, "0644").is_err(),
                    "join missing/duplicate/moved edit",
                )?;
            }
        }
        let mut altered = physical.clone();
        altered.push(0);
        need(
            composition_parent(root, path, &altered, false).is_err(),
            "altered supplied union",
        )?;
    }
    need(
        composition_parent(root, "wrong", b"", false).is_err(),
        "unlisted union path",
    )
}
#[cfg(all(test, unix))]
fn filesystem_controls(root: &Path, doc: &Value) -> Result<()> {
    use std::os::unix::fs::{PermissionsExt, symlink};
    struct Owned(std::path::PathBuf);
    impl Owned {
        fn create(path: std::path::PathBuf) -> Result<Self> {
            fs::create_dir(&path).map_err(|e| e.to_string())?;
            Ok(Self(path))
        }
    }
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let owned = Owned::create(std::env::temp_dir().join(format!(
            "snapshot-w07-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        )))?;
    let collision = owned.0.join("preexisting");
    fs::create_dir(&collision).map_err(|e| e.to_string())?;
    let sentinel = collision.join("sentinel");
    fs::write(&sentinel, b"preexisting ownership").map_err(|e| e.to_string())?;
    need(
        Owned::create(collision).is_err(),
        "preexisting directory accepted as newly owned",
    )?;
    need(
        fs::read(sentinel).map_err(|e| e.to_string())? == b"preexisting ownership",
        "failed acquisition deleted preexisting contents",
    )?;
    for path in [CAPSULE, JOIN, MAIN, CONTAINER, SOURCES[0], SOURCES[1]] {
        let target = owned.0.join(path);
        fs::create_dir_all(target.parent().ok_or("copy parent")?).map_err(|e| e.to_string())?;
        fs::copy(root.join(path), &target).map_err(|e| e.to_string())?;
        fs::set_permissions(target, fs::Permissions::from_mode(0o644))
            .map_err(|e| e.to_string())?;
    }
    let oversized = owned.0.join("over-bound");
    fs::write(&oversized, vec![0u8; 262145]).map_err(|e| e.to_string())?;
    fs::set_permissions(&oversized, fs::Permissions::from_mode(0o644))
        .map_err(|e| e.to_string())?;
    need(
        regular(&owned.0, "over-bound").is_err(),
        "finite source read bound",
    )?;
    for row in doc["rows"].as_array().ok_or("rows absent")? {
        let path = text(&row["prior"], "path")?;
        let expected = text(&row["prior"], "sha256")?;
        let physical = regular(root, path)?;
        let target = owned.0.join(path);
        verify(&owned.0, path, expected, &physical).ok_or("selected helper absent")??;
        for mode in [0o600, 0o755, 0o4644] {
            fs::set_permissions(&target, fs::Permissions::from_mode(mode))
                .map_err(|e| e.to_string())?;
            need(
                verify(&owned.0, path, expected, &physical)
                    .unwrap()
                    .is_err(),
                "physical wrong mode",
            )?;
        }
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
            .map_err(|e| e.to_string())?;
        let backup = target.with_extension("backup");
        fs::rename(&target, &backup).map_err(|e| e.to_string())?;
        need(
            verify(&owned.0, path, expected, &physical)
                .unwrap()
                .is_err(),
            "missing source",
        )?;
        symlink(&backup, &target).map_err(|e| e.to_string())?;
        need(
            verify(&owned.0, path, expected, &physical)
                .unwrap()
                .is_err(),
            "symlink source",
        )?;
        fs::remove_file(&target).map_err(|e| e.to_string())?;
        fs::create_dir(&target).map_err(|e| e.to_string())?;
        need(
            verify(&owned.0, path, expected, &physical)
                .unwrap()
                .is_err(),
            "nonregular source",
        )?;
        fs::remove_dir(&target).map_err(|e| e.to_string())?;
        fs::rename(&backup, &target).map_err(|e| e.to_string())?;
        for gateway in [true, false] {
            let parent = composition_parent(root, path, &physical, gateway)?;
            fs::write(&target, &parent).map_err(|e| e.to_string())?;
            need(
                verify(&owned.0, path, expected, &parent).unwrap().is_err(),
                "either parent physically substituted",
            )?;
            let joined = composition_document(root)?;
            let current_pin = text(&composition_row(&joined, path)?["current"], "sha256")?;
            need(
                verify(&owned.0, path, current_pin, &parent)
                    .unwrap()
                    .is_err(),
                "old parent with current pin",
            )?;
        }
        let baseline = composition_parent(root, path, &physical, false)?;
        let prior = inverse(row, &baseline, "0644")?;
        fs::write(&target, &prior).map_err(|e| e.to_string())?;
        need(
            verify(&owned.0, path, expected, &prior).unwrap().is_err(),
            "old physically substituted source",
        )?;
        fs::write(target, physical).map_err(|e| e.to_string())?;
    }
    // Registry custody is independent of which source pin the caller presents.
    let joined = composition_document(root)?;
    let source = SOURCES[0];
    let physical = regular(root, source)?;
    let original_pin = text(&doc["rows"][0]["prior"], "sha256")?;
    let current_pin = text(&composition_row(&joined, source)?["current"], "sha256")?;
    let container = regular(root, CONTAINER)?;
    let target = owned.0.join(CONTAINER);
    for pin in [original_pin, current_pin] {
        verify(&owned.0, source, pin, &physical).ok_or("registry positive route")??;
        for gateway in [true, false] {
            let parent = composition_parent(root, CONTAINER, &container, gateway)?;
            fs::write(&target, &parent).map_err(|e| e.to_string())?;
            need(
                verify(&owned.0, source, pin, &physical).unwrap().is_err(),
                "old registry physically substituted",
            )?;
            fs::write(&target, &container).map_err(|e| e.to_string())?;
        }
        for mode in [0o600, 0o755, 0o4644] {
            fs::set_permissions(&target, fs::Permissions::from_mode(mode))
                .map_err(|e| e.to_string())?;
            need(
                verify(&owned.0, source, pin, &physical).unwrap().is_err(),
                "physical registry wrong mode",
            )?;
        }
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
            .map_err(|e| e.to_string())?;
    }
    let physical_main = regular(root, MAIN)?;
    let main_target = owned.0.join(MAIN);
    gateway_main(&owned.0, &physical_main)?;
    snapshot_main(&owned.0, &physical_main)?;
    for gateway in [true, false] {
        let prior = composition_parent(root, MAIN, &physical_main, gateway)?;
        fs::write(&main_target, &prior).map_err(|e| e.to_string())?;
        need(
            gateway_main(&owned.0, &prior).is_err() && snapshot_main(&owned.0, &prior).is_err(),
            "either main parent physically substituted",
        )?;
    }
    fs::write(&main_target, &physical_main).map_err(|e| e.to_string())?;
    for mode in [0o600, 0o755, 0o4644] {
        fs::set_permissions(&main_target, fs::Permissions::from_mode(mode))
            .map_err(|e| e.to_string())?;
        need(
            gateway_main(&owned.0, &physical_main).is_err()
                && snapshot_main(&owned.0, &physical_main).is_err(),
            "physical gateway main wrong mode",
        )?;
    }
    fs::set_permissions(&main_target, fs::Permissions::from_mode(0o644))
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(all(test, not(unix)))]
fn filesystem_controls(_: &Path, _: &Value) -> Result<()> {
    Err("Unix required".into())
}
