//! Exact two-insertion successor of the accepted W07/T02 native gate entrypoint.
//! Preserves old source-chain operands only after proving physical current bytes.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
const CAPSULE: &str = "docs/rust-rewrite/snapshot-fixture-amendment/capsule.json";
#[path = "capsule-sha.rs"]
mod capsule_sha;
use capsule_sha::SHA;
const SOURCE: &str = "crates/xtask/src/main.rs";
const PRIOR_SHA: &str = "abc9435b419cfa6f0013fe1177d27b6661437baabe40d3824fa95185e6da40ee";
fn need(ok: bool, why: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("snapshot main custody: {why}"))
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn material(row: &Value, bytes: &[u8], mode: &str) -> Result<(), String> {
    need(
        row["sha256"] == digest(bytes)
            && row["bytes"].as_u64() == Some(bytes.len() as u64)
            && row["mode"] == mode,
        "exact material",
    )
}
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key].as_str().ok_or_else(|| format!("missing {key}"))
}
#[cfg(unix)]
fn permissions(path: &Path) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    Ok(format!(
        "{:04o}",
        fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o7777
    ))
}
#[cfg(not(unix))]
fn permissions(_: &Path) -> Result<String, String> {
    Err("Unix main mode proof required".into())
}
fn apply_edits(prior: &[u8], changes: &Value) -> Result<Vec<u8>, String> {
    let mut result = vec![];
    let mut end = 0;
    for edit in changes.as_array().ok_or("main edits absent")? {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("main offset")?;
        let remove = text(edit, "remove_utf8")?.as_bytes();
        let next = at.checked_add(remove.len()).ok_or("main span overflow")?;
        need(
            at >= end && prior.get(at..next) == Some(remove),
            "ordered main forward span",
        )?;
        result.extend_from_slice(prior.get(end..at).ok_or("main forward prefix")?);
        result.extend_from_slice(text(edit, "insert_utf8")?.as_bytes());
        end = next;
    }
    result.extend_from_slice(prior.get(end..).ok_or("main forward suffix")?);
    Ok(result)
}
fn regular(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        metadata.is_file()
            && metadata.len() <= 1_048_576
            && !metadata.file_type().is_symlink()
            && permissions(&path)? == "0644",
        "snapshot fixture gate source requires regular0644",
    )?;
    fs::read(path).map_err(|e| e.to_string())
}
fn inverse(doc: &Value, expected: &Value, current: &[u8], mode: &str) -> Result<Vec<u8>, String> {
    let row = &doc["main"];
    need(
        expected["path"] == SOURCE
            && expected["sha256"] == row["prior"]["sha256"]
            && expected["bytes"] == row["prior"]["bytes"]
            && expected["mode"] == row["prior"]["mode"]
            && expected
                .get("git_blob")
                .is_none_or(|blob| *blob == row["prior"]["git_blob"])
            && expected.as_object().is_some_and(|fields| {
                fields.keys().all(|key| {
                    matches!(
                        key.as_str(),
                        "path" | "sha256" | "bytes" | "mode" | "git_blob"
                    )
                })
            }),
        "snapshot fixture exact joint main caller differs",
    )?;
    material(&row["current"], current, mode)?;
    let edits = row["edits"]
        .as_array()
        .ok_or("snapshot fixture main edits absent")?;
    need(
        edits.len() == 2,
        "snapshot fixture main closed edit count differs",
    )?;
    let mut prior = Vec::new();
    let (mut old_cursor, mut cursor) = (0usize, 0usize);
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("snapshot fixture main offset absent")?;
        let insert = text(edit, "insert_utf8")?.as_bytes();
        need(
            edit["remove_utf8"] == "" && !insert.is_empty(),
            "snapshot fixture main only insertions allowed",
        )?;
        let unchanged = at
            .checked_sub(old_cursor)
            .ok_or("snapshot fixture main edit order differs")?;
        let end = cursor
            .checked_add(unchanged)
            .ok_or("snapshot fixture main overflow")?;
        prior.extend_from_slice(
            current
                .get(cursor..end)
                .ok_or("snapshot fixture main prefix absent")?,
        );
        let insert_end = end
            .checked_add(insert.len())
            .ok_or("snapshot fixture main insertion overflow")?;
        need(
            current.get(end..insert_end) == Some(insert),
            "snapshot fixture main exact insertion differs",
        )?;
        cursor = insert_end;
        old_cursor = at;
    }
    prior.extend_from_slice(
        current
            .get(cursor..)
            .ok_or("snapshot fixture main suffix absent")?,
    );
    material(&row["prior"], &prior, "0644")?;
    need(
        apply_edits(&prior, &row["edits"])? == current,
        "snapshot fixture main forward reconstruction differs",
    )?;
    Ok(prior)
}
pub(super) fn joint(root: &Path, supplied: &[u8]) -> Result<Vec<u8>, String> {
    let raw = regular(root, CAPSULE)?;
    need(
        digest(&raw) == SHA,
        "snapshot fixture main capsule identity differs",
    )?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "PORTABLE_SNAPSHOT_FIXTURE_SOURCE_V1"
            && doc["main"]["prior"]["path"] == SOURCE
            && doc["main"]["prior"]["sha256"] == PRIOR_SHA,
        "snapshot fixture main closed source differs",
    )?;
    let current = regular(root, SOURCE)?;
    need(
        supplied == current,
        "snapshot fixture supplied main is not physical current",
    )?;
    let prior = inverse(&doc, &doc["main"]["prior"], &current, "0644")?;
    for (identity, bytes) in [
        (&doc["main"]["prior"], prior.as_slice()),
        (&doc["main"]["current"], current.as_slice()),
    ] {
        need(
            identity["git_blob"] == git_blob(root, bytes)?,
            "snapshot fixture main Git blob differs",
        )?;
    }
    Ok(prior)
}
fn git_blob(root: &Path, bytes: &[u8]) -> Result<String, String> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut child = Command::new("git")
        .args(["hash-object", "--stdin"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let Some(mut input) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("main Git stdin absent".into());
    };
    let written = input.write_all(bytes);
    drop(input);
    if let Err(error) = written {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.to_string());
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    need(
        output.status.success(),
        "snapshot fixture main Git hash failed",
    )?;
    Ok(String::from_utf8(output.stdout)
        .map_err(|e| e.to_string())?
        .trim()
        .into())
}
/// Invoked only by an xtask-owned test; never adds a test to frozen adapter suites.
#[cfg(test)]
#[allow(dead_code)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc: Value = serde_json::from_slice(&regular(root, CAPSULE).unwrap()).unwrap();
    let expected = &doc["main"]["prior"];
    let current = regular(root, SOURCE).unwrap();
    let historical = joint(root, &current).unwrap();
    assert!(joint(root, &historical).is_err());
    super::parent(root, SOURCE, &current, super::Parent::T02)?;
    super::parent(root, SOURCE, &current, super::Parent::W07)?;
    assert!(inverse(&doc, expected, &historical, "0644").is_err());
    for mode in ["0600", "0755", "4644"] {
        assert!(inverse(&doc, expected, &current, mode).is_err());
    }
    let mut bad = expected.clone();
    bad["sha256"] = serde_json::json!("wrong");
    assert!(inverse(&doc, &bad, &current, "0644").is_err());
    bad = expected.clone();
    bad["bytes"] = serde_json::json!(0);
    assert!(inverse(&doc, &bad, &current, "0644").is_err());
    let mut changed = current.clone();
    changed.push(b'\n');
    let mut rehashed = doc.clone();
    rehashed["main"]["current"]["sha256"] = serde_json::json!(digest(&changed));
    rehashed["main"]["current"]["bytes"] = serde_json::json!(changed.len());
    assert!(inverse(&rehashed, expected, &changed, "0644").is_err());
    for mutation in 0..3 {
        let mut wrong = doc.clone();
        let edits = wrong["main"]["edits"].as_array_mut().unwrap();
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
        assert!(inverse(&wrong, expected, &current, "0644").is_err());
    }
    Ok(())
}
