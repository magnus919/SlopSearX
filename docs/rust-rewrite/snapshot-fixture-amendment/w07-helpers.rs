//! Closed physical-current custody and finite inverse for two W07 source entries.
//! The physical read and the pure inverse remain separate composition stages.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
type Result<T> = std::result::Result<T, String>;
const CAPSULE: &str = "docs/rust-rewrite/snapshot-fixture-amendment/w07-helpers.json";
const SHA: &str = "8889f54d073bc27811febf1bbc6bafdbea1c25303a0f1c0dfff8bff04daaa831";
const PAGE_JOIN: &str = "docs/rust-rewrite/snapshot-fixture-amendment/page-join-v1.json";
const PAGE_JOIN_SHA: &str = "fc345b654e18cbc81f79802e70c363621a3050476b3936be85dfee20b0147704";
const CONTAINER: &str = "docs/rust-rewrite/w07/qualification-completion-v1.json";
const CONTAINER_SHA: &str = "5be08978822440ec7bece674b13647c6337eaa4636cc232a1622738a90ac1b64";
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
    let raw = common_snapshot(root, CONTAINER, &physical)?;
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
fn page_composition_document(root: &Path) -> Result<Value> {
    let raw = regular(root, PAGE_JOIN)?;
    need(sha(&raw) == PAGE_JOIN_SHA, "sealed Page/snapshot join")?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "SNAPSHOT_PAGE_TWO_HELPER_JOIN_V1"
            && doc["parents"]
                == serde_json::json!({"page":"ef9c5c93e4a4a57d9b835fb291e8d87410a232a8","snapshot":"8e71a83f9649d58e9a60e34810275b736e44151a"})
            && doc["unchanged_capsules"]
                == serde_json::json!({"page":"dfd7494b0ee42ac6a9ca702020640aaf65454b6a57f99991ae761c571dd96fcf","snapshot":"e16ba1482b12b55a42a4372f3fd4a7116effaf0fa04c79cf5cb13f01930eea4e","helpers":SHA}),
        "closed join parents/capsules",
    )?;
    let rows = doc["helpers"].as_array().ok_or("join helper rows")?;
    need(
        rows.len() == 2
            && rows.iter().zip(SOURCES).all(|(row, path)| {
                row["current"]["path"] == path
                    && ["page", "snapshot"].iter().all(|parent| {
                        row[parent]["current"] == row["current"]
                            && row[parent]["prior"]["path"] == path
                    })
            }),
        "closed two-helper join membership",
    )?;
    let row = &doc["registry"];
    need(
        row["current"]["path"] == CONTAINER
            && row["snapshot"]["prior"]["sha256"] == CONTAINER_SHA
            && ["page", "snapshot"].iter().all(|parent| {
                row[parent]["current"] == row["current"]
                    && row[parent]["prior"]["path"] == CONTAINER
            }),
        "closed registry join frame",
    )?;
    Ok(doc)
}
fn page_composition_row<'a>(doc: &'a Value, path: &str) -> Result<&'a Value> {
    if path == CONTAINER {
        return Ok(&doc["registry"]);
    }
    need(SOURCES.contains(&path), "unlisted join path")?;
    doc["helpers"]
        .as_array()
        .ok_or("join rows")?
        .iter()
        .find(|row| row["current"]["path"] == path)
        .ok_or("join source absent".into())
}
fn page_composition_parent(
    root: &Path,
    path: &str,
    supplied: &[u8],
    page: bool,
) -> Result<Vec<u8>> {
    let doc = page_composition_document(root)?;
    let row = page_composition_row(&doc, path)?;
    // Private sealed old-frame interpretation, after outer physical admission.
    let physical = supplied.to_vec();
    material(&row["current"], &physical, "0644")?;
    // Both frames must reconstruct before either one is returned to an old reader.
    let page_prior = inverse(&row["page"], &physical, "0644")?;
    let snapshot_prior = inverse(&row["snapshot"], &physical, "0644")?;
    Ok(if page { page_prior } else { snapshot_prior })
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
            .find(|row| row["prior"]["path"] == path)
            .ok_or("unlisted helper")?;
        let physical = regular(root, path)?;
        need(
            physical == supplied,
            "supplied differs from physical current",
        )?;
        let baseline = common_snapshot(root, path, &physical)?;
        let outer = outer_document(root)?;
        let current = &outer_row(&outer, path)?["current"];
        if current["sha256"] == expected {
            let container = regular(root, CONTAINER)?;
            outer_endpoint(root, CONTAINER, &container, "page")?;
            let value: Value =
                serde_json::from_slice(&container).map_err(|error| error.to_string())?;
            let pointer = format!("/sources/{}", path.replace('~', "~0").replace('/', "~1"));
            need(
                value.pointer(&pointer) == Some(&Value::String(expected.into())),
                "exact combined current container pointer",
            )?;
            return Ok(physical);
        }
        let page_doc = page_composition_document(root)?;
        if page_composition_row(&page_doc, path)?["current"]["sha256"] == expected {
            let raw = regular(root, CONTAINER)?;
            let page_container = outer_endpoint(root, CONTAINER, &raw, "page")?;
            material(&page_doc["registry"]["current"], &page_container, "0644")?;
            let value: Value =
                serde_json::from_slice(&page_container).map_err(|error| error.to_string())?;
            let pointer = format!("/sources/{}", path.replace('~', "~0").replace('/', "~1"));
            need(
                value.pointer(&pointer) == Some(&Value::String(expected.into())),
                "exact accepted Page caller pointer",
            )?;
            return outer_endpoint(root, path, &physical, "page");
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
        let baseline = common_snapshot(root, path, &physical)?;
        let joined = outer_document(root)?;
        let current_pin = text(&outer_row(&joined, path)?["current"], "sha256")?;
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
fn page_composition_controls(root: &Path) -> Result<()> {
    let doc = page_composition_document(root)?;
    for path in [SOURCES[0], SOURCES[1], CONTAINER] {
        let row = page_composition_row(&doc, path)?;
        let current = regular(root, path)?;
        let physical = outer_endpoint(root, path, &current, "page")?;
        for (name, page) in [("page", true), ("snapshot", false)] {
            let prior = page_composition_parent(root, path, &physical, page)?;
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
                "exact Page/snapshot parent Git bytes",
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
                page_composition_parent(root, path, &prior, page).is_err(),
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
            page_composition_parent(root, path, &altered, false).is_err(),
            "altered supplied union",
        )?;
    }
    need(
        page_composition_parent(root, "wrong", b"", false).is_err(),
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
    for path in [
        CAPSULE, PAGE_JOIN, W08_JOIN, OUTER, MAIN, CONTAINER, SOURCES[0], SOURCES[1],
    ] {
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
        for page in [true, false] {
            let operand = outer_endpoint(root, path, &physical, "page")?;
            let parent = page_composition_parent(root, path, &operand, page)?;
            fs::write(&target, &parent).map_err(|e| e.to_string())?;
            need(
                verify(&owned.0, path, expected, &parent).unwrap().is_err(),
                "either parent physically substituted",
            )?;
            let joined = outer_document(root)?;
            let current_pin = text(&outer_row(&joined, path)?["current"], "sha256")?;
            need(
                verify(&owned.0, path, current_pin, &parent)
                    .unwrap()
                    .is_err(),
                "old parent with current pin",
            )?;
        }
        for endpoint in ["w08", "page"] {
            let parent = outer_endpoint(root, path, &physical, endpoint)?;
            fs::write(&target, &parent).map_err(|error| error.to_string())?;
            need(
                verify(&owned.0, path, expected, &parent).unwrap().is_err(),
                "outer endpoint physically substituted",
            )?;
            let outer = outer_document(root)?;
            let pin = text(&outer_row(&outer, path)?["current"], "sha256")?;
            need(
                verify(&owned.0, path, pin, &parent).unwrap().is_err(),
                "outer endpoint substituted with current pin",
            )?;
        }
        let baseline = common_snapshot(root, path, &physical)?;
        let prior = inverse(row, &baseline, "0644")?;
        fs::write(&target, &prior).map_err(|e| e.to_string())?;
        need(
            verify(&owned.0, path, expected, &prior).unwrap().is_err(),
            "old physically substituted source",
        )?;
        fs::write(target, physical).map_err(|e| e.to_string())?;
    }
    // Registry custody is independent of which source pin the caller presents.
    let joined = outer_document(root)?;
    let source = SOURCES[0];
    let physical = regular(root, source)?;
    let original_pin = text(&doc["rows"][0]["prior"], "sha256")?;
    let current_pin = text(&outer_row(&joined, source)?["current"], "sha256")?;
    let container = regular(root, CONTAINER)?;
    let target = owned.0.join(CONTAINER);
    for pin in [original_pin, current_pin] {
        verify(&owned.0, source, pin, &physical).ok_or("registry positive route")??;
        for page in [true, false] {
            let operand = outer_endpoint(root, CONTAINER, &container, "page")?;
            let parent = page_composition_parent(root, CONTAINER, &operand, page)?;
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
    Ok(())
}
#[cfg(all(test, not(unix)))]
fn filesystem_controls(_: &Path, _: &Value) -> Result<()> {
    Err("Unix required".into())
}

fn w08_composition_document(root: &Path) -> Result<Value> {
    let raw = regular(root, W08_JOIN)?;
    need(sha(&raw) == W08_JOIN_SHA, "sealed W08/snapshot join")?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "SNAPSHOT_W08_THREE_OPERAND_JOIN_V1"
            && doc["parents"]
                == serde_json::json!({"w08":"3ce03032bd07e57ef42d9ed4620cd940da209072","snapshot":"30270aaa7c459cec6404f4f015fa17077704c2a0"})
            && doc["unchanged_capsules"]
                == serde_json::json!({"w08":"a12f8ebaf9fe78bbf6b2647562b7af3d6e55f18d53e7caf99ead3b36080ba712","snapshot":"e16ba1482b12b55a42a4372f3fd4a7116effaf0fa04c79cf5cb13f01930eea4e","helpers":SHA}),
        "closed W08/snapshot parents and unchanged capsules",
    )?;
    let rows = doc["rows"].as_array().ok_or("join rows absent")?;
    need(
        rows.len() == 3
            && rows
                .iter()
                .zip([MAIN, SOURCES[0], SOURCES[1]])
                .all(|(row, path)| {
                    row["current"]["path"] == path
                        && ["w08", "snapshot"].iter().all(|name| {
                            row[name]["current"] == row["current"]
                                && row[name]["prior"]["path"] == path
                        })
                }),
        "closed ordered three-operand membership",
    )?;
    Ok(doc)
}
fn w08_composition_parent(root: &Path, path: &str, supplied: &[u8], w08: bool) -> Result<Vec<u8>> {
    let doc = w08_composition_document(root)?;
    let row = doc["rows"]
        .as_array()
        .ok_or("join rows absent")?
        .iter()
        .find(|row| row["current"]["path"] == path)
        .ok_or("unlisted W08/snapshot operand")?;
    // Private sealed old-frame interpretation, after outer physical admission.
    let physical = supplied.to_vec();
    material(&row["current"], &physical, "0644")?;
    let w08_prior = inverse(&row["w08"], &physical, "0644")?;
    let snapshot_prior = inverse(&row["snapshot"], &physical, "0644")?;
    Ok(if w08 { w08_prior } else { snapshot_prior })
}
pub(crate) fn w08_composition_controls(root: &Path) -> Result<()> {
    let doc = w08_composition_document(root)?;
    for row in doc["rows"].as_array().ok_or("join rows absent")? {
        let path = text(&row["current"], "path")?;
        let current = regular(root, path)?;
        let physical = outer_endpoint(root, path, &current, "w08")?;
        for (name, w08) in [("w08", true), ("snapshot", false)] {
            let prior = w08_composition_parent(root, path, &physical, w08)?;
            need(
                inverse(&row[name], &physical, "0644")? == prior,
                "join pure positive before mutants",
            )?;
            let commit = text(&doc["parents"], name)?;
            let git = std::process::Command::new("git")
                .args(["show", &format!("{commit}:{path}")])
                .current_dir(root)
                .output()
                .map_err(|e| e.to_string())?;
            need(
                git.status.success() && git.stdout == prior,
                "exact W08/snapshot Git bytes",
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
                "exact W08/snapshot Git mode/blob",
            )?;
            need(
                w08_composition_parent(root, path, &prior, w08).is_err(),
                "either parent supplied as physical current",
            )?;
            for mode in ["0600", "0755", "4644"] {
                need(
                    inverse(&row[name], &physical, mode).is_err(),
                    "join wrong mode",
                )?;
            }
            for mutation in 0..4 {
                let mut bad = row[name].clone();
                let edits = bad["edits"].as_array_mut().ok_or("join edits absent")?;
                match mutation {
                    0 => {
                        edits.pop();
                    }
                    1 => edits.push(edits[0].clone()),
                    2 => edits[0]["at"] = serde_json::json!(0),
                    _ => edits[0]["insert_utf8"] = serde_json::json!("changed"),
                }
                need(
                    inverse(&bad, &physical, "0644").is_err(),
                    "join missing/duplicate/moved/changed descriptor",
                )?;
            }
            let mut changed = physical.clone();
            changed.push(0);
            let mut bad = row[name].clone();
            bad["current"]["sha256"] = serde_json::json!(sha(&changed));
            bad["current"]["bytes"] = serde_json::json!(changed.len());
            need(
                inverse(&bad, &changed, "0644").is_err(),
                "unrelated rehashed union body",
            )?;
        }
    }
    need(
        w08_composition_parent(root, "unlisted.rs", b"", true).is_err(),
        "unlisted join operand",
    )
}

const MAIN: &str = "crates/xtask/src/main.rs";
const W08_JOIN: &str = "docs/rust-rewrite/snapshot-fixture-amendment/w08-join-v1.json";
const W08_JOIN_SHA: &str = "11e80b07b1c40fd7b0c3f37b5c1dfbe95981ab68306bebf83112b443f088309e";
const OUTER: &str = "docs/rust-rewrite/snapshot-fixture-amendment/w08-page-join-v1.json";
const OUTER_SHA: &str = "2201b3d459bc0e386d3fb847a02a23f5f30fcf4b7e82ca85fd056bead5567847";
const OUTER_PATHS: [&str; 4] = [MAIN, SOURCES[0], SOURCES[1], CONTAINER];
fn outer_document(root: &Path) -> Result<Value> {
    let raw = regular(root, OUTER)?;
    need(
        sha(&raw) == OUTER_SHA,
        "sealed W08/Page physical composition",
    )?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|error| error.to_string())?;
    need(
        doc["schema"] == "W08_PAGE_FOUR_OPERAND_JOIN_V1"
            && doc["parents"]
                == serde_json::json!({"w08":"8c12d02ae5e7741410e41de4795a235700fc71c2","page":"72987c50283eb70f70ff6d339c5d9f4ed8e00efd"})
            && doc["unchanged_capsules"]
                == serde_json::json!({"w08_join":W08_JOIN_SHA,"page_join":PAGE_JOIN_SHA,"helpers":SHA}),
        "closed composition parents and capsules",
    )?;
    let rows = doc["rows"].as_array().ok_or("composition rows absent")?;
    need(
        rows.len() == OUTER_PATHS.len()
            && rows.iter().zip(OUTER_PATHS).all(|(row, path)| {
                row["current"]["path"] == path
                    && ["w08", "page"].iter().all(|parent| {
                        row[*parent]["current"] == row["current"]
                            && row[*parent]["prior"]["path"] == path
                    })
            }),
        "closed four-operand composition",
    )?;
    for (path, expected) in [
        (W08_JOIN, W08_JOIN_SHA),
        (PAGE_JOIN, PAGE_JOIN_SHA),
        (CAPSULE, SHA),
    ] {
        need(
            sha(&regular(root, path)?) == expected,
            "unchanged original composition asset",
        )?;
    }
    Ok(doc)
}
fn outer_row<'a>(doc: &'a Value, path: &str) -> Result<&'a Value> {
    need(OUTER_PATHS.contains(&path), "unlisted composition path")?;
    doc["rows"]
        .as_array()
        .ok_or("composition rows absent")?
        .iter()
        .find(|row| row["current"]["path"] == path)
        .ok_or("composition source absent".into())
}
fn outer_endpoint(root: &Path, path: &str, supplied: &[u8], parent: &str) -> Result<Vec<u8>> {
    let doc = outer_document(root)?;
    let row = outer_row(&doc, path)?;
    let physical = regular(root, path)?;
    need(
        physical == supplied,
        "composition supplied bytes are not physical current",
    )?;
    material(&row["current"], &physical, "0644")?;
    // Both exact endpoint inverses run before either old reader receives an operand.
    let w08 = inverse(&row["w08"], &physical, "0644")?;
    let page = inverse(&row["page"], &physical, "0644")?;
    match parent {
        "w08" => Ok(w08),
        "page" => Ok(page),
        _ => Err("unlisted composition parent".into()),
    }
}
fn common_snapshot(root: &Path, path: &str, physical: &[u8]) -> Result<Vec<u8>> {
    let w08 = outer_endpoint(root, path, physical, "w08")?;
    let page = outer_endpoint(root, path, physical, "page")?;
    let a = if path == CONTAINER {
        need(sha(&w08) == CONTAINER_SHA, "exact original W08 registry")?;
        w08
    } else {
        w08_composition_parent(root, path, &w08, false)?
    };
    let b = if path == MAIN {
        page
    } else {
        page_composition_parent(root, path, &page, false)?
    };
    need(
        a == b,
        "W08 and Page joins must yield the same exact snapshot frame",
    )?;
    Ok(a)
}
pub(crate) fn w08_operand(root: &Path, path: &str, supplied: &[u8]) -> Result<Vec<u8>> {
    common_snapshot(root, path, supplied)?;
    let operand = outer_endpoint(root, path, supplied, "w08")?;
    w08_composition_parent(root, path, &operand, true)
}
pub(crate) fn snapshot_main(root: &Path, supplied: &[u8]) -> Result<Vec<u8>> {
    common_snapshot(root, MAIN, supplied)
}
fn outer_controls(root: &Path) -> Result<()> {
    let doc = outer_document(root)?;
    for row in doc["rows"].as_array().ok_or("composition rows absent")? {
        let path = text(&row["current"], "path")?;
        let physical = regular(root, path)?;
        common_snapshot(root, path, &physical)?;
        for name in ["w08", "page"] {
            let prior = outer_endpoint(root, path, &physical, name)?;
            need(
                inverse(&row[name], &physical, "0644")? == prior,
                "composition pure positive before controls",
            )?;
            let commit = text(&doc["parents"], name)?;
            let git = std::process::Command::new("git")
                .args(["show", &format!("{commit}:{path}")])
                .current_dir(root)
                .output()
                .map_err(|error| error.to_string())?;
            need(
                git.status.success() && git.stdout == prior,
                "composition exact Git parent bytes",
            )?;
            let tree = std::process::Command::new("git")
                .args(["ls-tree", commit, "--", path])
                .current_dir(root)
                .output()
                .map_err(|error| error.to_string())?;
            need(
                tree.status.success()
                    && tree.stdout
                        == format!(
                            "100644 blob {}\t{path}\n",
                            text(&row[name]["prior"], "git_blob")?
                        )
                        .as_bytes(),
                "composition exact Git parent mode/blob",
            )?;
            if prior != physical {
                need(
                    outer_endpoint(root, path, &prior, name).is_err(),
                    "historical operand supplied as physical",
                )?;
            }
            for mode in ["0600", "0755", "4644"] {
                need(
                    inverse(&row[name], &physical, mode).is_err(),
                    "composition changed mode",
                )?;
            }
            let mut bad = row[name].clone();
            bad["prior"]["sha256"] = serde_json::json!("changed");
            need(
                inverse(&bad, &physical, "0644").is_err(),
                "composition changed prior identity",
            )?;
            if !row[name]["edits"]
                .as_array()
                .ok_or("composition edits")?
                .is_empty()
            {
                for mutation in 0..4 {
                    let mut bad = row[name].clone();
                    let edits = bad["edits"].as_array_mut().ok_or("composition edits")?;
                    match mutation {
                        0 => {
                            edits.pop();
                        }
                        1 => edits.push(edits[0].clone()),
                        2 => {
                            let at = edits[0]["at"].as_u64().ok_or("composition offset")?;
                            edits[0]["at"] = serde_json::json!(at + 1);
                        }
                        _ => edits[0]["insert_utf8"] = serde_json::json!("changed"),
                    }
                    need(
                        inverse(&bad, &physical, "0644").is_err(),
                        "composition missing/duplicate/moved/changed descriptor",
                    )?;
                }
            }
        }
        let mut altered = physical.clone();
        altered.push(0);
        need(
            outer_endpoint(root, path, &altered, "w08").is_err(),
            "composition changed supplied bytes",
        )?;
    }
    need(
        outer_endpoint(root, "unlisted.rs", b"", "w08").is_err(),
        "unlisted physical operand",
    )
}
pub(crate) fn verify_current_registry(root: &Path, supplied: &[u8]) -> Result<()> {
    let doc = outer_document(root)?;
    let physical = regular(root, CONTAINER)?;
    common_snapshot(root, CONTAINER, supplied)?;
    need(
        physical == supplied,
        "current registry supplied bytes differ",
    )?;
    let registry: Value = serde_json::from_slice(&physical).map_err(|error| error.to_string())?;
    for path in SOURCES {
        let current = &outer_row(&doc, path)?["current"];
        material(current, &regular(root, path)?, "0644")?;
        let pointer = format!("/sources/{}", path.replace('~', "~0").replace('/', "~1"));
        need(
            registry.pointer(&pointer) == Some(&current["sha256"]),
            "exact current registry source pointer differs",
        )?;
    }
    Ok(())
}

// One closed source-only admission for W08's immutable W07 source binding.
const W07_SOURCE_PATH: &str = "crates/xtask/src/w07.rs";
const W07_SOURCE_PRIOR: &str = "64bbd78e5910558eae78de7e449d0488337020a814ec10cb2869e412193582ea";
const W07_SOURCE_CAPSULE: &str = "docs/rust-rewrite/w08/w07-page-source-admission-v1.json";
const W07_SOURCE_CAPSULE_SHA: &str =
    "7db2d39f8c2e0fe49211a3cfdc1ef4e35c755a4b66b8d893f717643677f28b57";
fn w07_source_scope(doc: &Value) -> Result<()> {
    need(
        doc["schema"] == "W08_W07_PAGE_SOURCE_ADMISSION_V1"
            && doc["parent"] == "8c12d02ae5e7741410e41de4795a235700fc71c2"
            && doc["caller"]
                == serde_json::json!({"path":W07_SOURCE_PATH,"expected_sha256":W07_SOURCE_PRIOR})
            && doc.as_object().is_some_and(|fields| fields.len() == 4)
            && doc["rows"].as_array().is_some_and(|rows| rows.len() == 1),
        "closed W07 source caller/parent/membership",
    )?;
    let row = &doc["rows"][0];
    need(
        row["current"]["path"] == W07_SOURCE_PATH
            && row["prior"]["path"] == W07_SOURCE_PATH
            && row["prior"]["sha256"] == W07_SOURCE_PRIOR
            && row["current"]["mode"] == "0644"
            && row["prior"]["mode"] == "0644",
        "closed W07 source path/prior/modes",
    )
}
fn w07_source_document(root: &Path) -> Result<Value> {
    let raw = regular(root, W07_SOURCE_CAPSULE)?;
    need(
        sha(&raw) == W07_SOURCE_CAPSULE_SHA,
        "sealed W07 source admission",
    )?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    w07_source_scope(&doc)?;
    Ok(doc)
}
pub(crate) fn w07_source_operand(
    root: &Path,
    path: &str,
    expected: &str,
    supplied: &[u8],
) -> Result<Vec<u8>> {
    need(
        path == W07_SOURCE_PATH && expected == W07_SOURCE_PRIOR,
        "exact W07 frozen caller",
    )?;
    let doc = w07_source_document(root)?;
    let physical = regular(root, W07_SOURCE_PATH)?;
    need(
        physical == supplied,
        "W07 supplied bytes are not physical current",
    )?;
    let row = &doc["rows"][0];
    let prior = inverse(row, supplied, "0644")?;
    let git = std::process::Command::new("git")
        .args(["cat-file", "blob", text(&row["prior"], "git_blob")?])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let tree = std::process::Command::new("git")
        .args([
            "ls-tree",
            "8c12d02ae5e7741410e41de4795a235700fc71c2",
            "--",
            W07_SOURCE_PATH,
        ])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        git.status.success()
            && git.stdout == prior
            && tree.status.success()
            && tree.stdout
                == format!(
                    "100644 blob {}\t{}\n",
                    text(&row["prior"], "git_blob")?,
                    W07_SOURCE_PATH
                )
                .as_bytes(),
        "W07 exact historical Git bytes/mode/blob",
    )?;
    Ok(prior)
}
fn w07_source_controls(root: &Path) -> Result<()> {
    let doc = w07_source_document(root)?;
    let row = &doc["rows"][0];
    let physical = regular(root, W07_SOURCE_PATH)?;
    let prior = w07_source_operand(root, W07_SOURCE_PATH, W07_SOURCE_PRIOR, &physical)?;
    need(
        sha(&prior) == W07_SOURCE_PRIOR && prior != physical,
        "nonvacuous W07 source positive",
    )?;
    need(
        w07_source_operand(root, W07_SOURCE_PATH, W07_SOURCE_PRIOR, &prior).is_err(),
        "old W07 bytes supplied as physical",
    )?;
    need(
        w07_source_operand(root, "unlisted.rs", W07_SOURCE_PRIOR, &physical).is_err(),
        "wrong W07 source path",
    )?;
    need(
        w07_source_operand(root, W07_SOURCE_PATH, "wrong", &physical).is_err(),
        "wrong W07 expected caller",
    )?;
    let mut changed = physical.clone();
    changed[0] ^= 1;
    let mut extra = physical.clone();
    extra.push(b' ');
    for bad in [changed, extra] {
        need(
            w07_source_operand(root, W07_SOURCE_PATH, W07_SOURCE_PRIOR, &bad).is_err(),
            "changed W07 supplied source",
        )?;
    }
    for mode in ["0600", "0755", "4644"] {
        need(
            inverse(row, &physical, mode).is_err(),
            "changed W07 source mode",
        )?;
    }
    let mut missing = doc.clone();
    missing["rows"] = serde_json::json!([]);
    let mut duplicate = doc.clone();
    duplicate["rows"].as_array_mut().unwrap().push(row.clone());
    let mut moved = doc.clone();
    moved["rows"][0]["current"]["path"] = Value::String("unlisted.rs".into());
    let mut mode = doc.clone();
    mode["rows"][0]["current"]["mode"] = Value::String("0755".into());
    for bad in [missing, duplicate, moved, mode] {
        need(
            w07_source_scope(&bad).is_err(),
            "changed W07 admission scope",
        )?;
    }
    let mut missing = row.clone();
    missing["edits"].as_array_mut().unwrap().remove(0);
    let mut duplicate = row.clone();
    let edit = duplicate["edits"][0].clone();
    duplicate["edits"].as_array_mut().unwrap().insert(0, edit);
    let mut moved = row.clone();
    moved["edits"][0]["at"] = serde_json::json!(row["edits"][0]["at"].as_u64().unwrap() + 1);
    let mut altered = row.clone();
    altered["edits"][0]["insert_utf8"] = Value::String("changed".into());
    for bad in [missing, duplicate, moved, altered] {
        need(
            inverse(&bad, &physical, "0644").is_err(),
            "changed W07 inverse spans",
        )?;
    }
    Ok(())
}

pub(crate) fn composition_controls(root: &Path) -> Result<()> {
    w07_source_controls(root)?;
    outer_controls(root)?;
    verify_current_registry(root, &regular(root, CONTAINER)?)?;
    w08_composition_controls(root)?;
    page_composition_controls(root)
}
