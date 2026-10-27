//! Exact fixture-only reference-catalog selection; original T06 bindings stay frozen.
//! Shared controls are ordinary functions, never discovered tests in frozen suites.
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
const SOURCE: &str = "crates/ssx-mcp/tests/t06_support/fixture.rs";
const CAPSULE: &str = "docs/rust-rewrite/native-engines/t06-fixture-amendment.json";
const CAPSULE_SHA: &str = "eb5e7eaa2fc530e4bd55552fe9a07a3e769164aaa4d985f6a76b71314c6c98bf";
fn need(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
#[cfg(unix)]
fn regular(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    use std::{
        io::Read,
        os::unix::fs::{MetadataExt, PermissionsExt},
    };
    let path = root.join(relative);
    let before = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        before.is_file() && before.permissions().mode() & 0o7777 == 0o644,
        "T06 catalog fixture requires regular0644 material",
    )?;
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let opened = file.metadata().map_err(|e| e.to_string())?;
    need(
        opened.is_file()
            && opened.permissions().mode() & 0o7777 == 0o644
            && opened.dev() == before.dev()
            && opened.ino() == before.ino(),
        "T06 catalog fixture custody changed",
    )?;
    let mut raw = Vec::new();
    file.take(1_048_577)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    need(
        raw.len() <= 1_048_576 && raw.len() as u64 == opened.len(),
        "T06 catalog fixture bounded read differs",
    )?;
    Ok(raw)
}
#[cfg(not(unix))]
fn regular(_: &Path, _: &str) -> Result<Vec<u8>, String> {
    Err("T06 fixture custody requires Unix".into())
}
fn material(row: &Value, raw: &[u8], mode: &str) -> Result<(), String> {
    need(
        row["mode"] == mode
            && row["bytes"].as_u64() == Some(raw.len() as u64)
            && row["sha256"] == digest(raw),
        "T06 catalog fixture material differs",
    )
}
fn document(raw: &[u8]) -> Result<Value, String> {
    need(
        digest(raw) == CAPSULE_SHA,
        "T06 catalog fixture capsule changed",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "T06_FROZEN_REFERENCE_CATALOG_SUCCESSOR_V1"
            && doc["baseline_commit"] == "e12744511013bd05370173e777daaef471bf9527"
            && doc["container"]["path"] == "docs/rust-rewrite/t06/native_fixture_bindings.json"
            && doc["pointer"] == "/files/1"
            && doc["prior"]["path"] == SOURCE
            && doc["current"]["path"] == SOURCE
            && doc["edits"].as_array().is_some_and(|e| e.len() == 4),
        "T06 catalog fixture closed scope differs",
    )?;
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    document(&regular(root, CAPSULE)?)
}
fn inverse(doc: &Value, current: &[u8], mode: &str) -> Result<Vec<u8>, String> {
    material(&doc["current"], current, mode)?;
    let mut prior = Vec::new();
    let (mut old_cursor, mut cursor) = (0usize, 0usize);
    for edit in doc["edits"]
        .as_array()
        .ok_or("T06 catalog fixture edits absent")?
    {
        need(
            edit.as_object().is_some_and(|e| e.len() == 3),
            "T06 catalog fixture edit shape differs",
        )?;
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("T06 catalog fixture edit offset absent")?;
        let remove = edit["remove_utf8"]
            .as_str()
            .ok_or("T06 catalog fixture preimage absent")?
            .as_bytes();
        let insert = edit["insert_utf8"]
            .as_str()
            .ok_or("T06 catalog fixture insertion absent")?
            .as_bytes();
        need(
            !remove.is_empty() || !insert.is_empty(),
            "T06 catalog fixture empty edit",
        )?;
        let unchanged = at
            .checked_sub(old_cursor)
            .ok_or("T06 catalog fixture edit order differs")?;
        let end = cursor
            .checked_add(unchanged)
            .ok_or("T06 catalog fixture span overflow")?;
        let insert_end = end
            .checked_add(insert.len())
            .ok_or("T06 catalog fixture insertion overflow")?;
        prior.extend_from_slice(
            current
                .get(cursor..end)
                .ok_or("T06 catalog fixture prefix absent")?,
        );
        need(
            current.get(end..insert_end) == Some(insert),
            "T06 catalog fixture exact insertion differs",
        )?;
        prior.extend_from_slice(remove);
        cursor = insert_end;
        old_cursor = at
            .checked_add(remove.len())
            .ok_or("T06 catalog fixture removal overflow")?;
    }
    prior.extend_from_slice(
        current
            .get(cursor..)
            .ok_or("T06 catalog fixture suffix absent")?,
    );
    material(&doc["prior"], &prior, "0644")?;
    // Independently apply the reviewed forward edits after inverse identity proof.
    let mut rebuilt = Vec::new();
    let mut end = 0usize;
    for edit in doc["edits"].as_array().unwrap() {
        let at = edit["at"].as_u64().unwrap() as usize;
        let remove = edit["remove_utf8"].as_str().unwrap().as_bytes();
        need(
            prior.get(at..at + remove.len()) == Some(remove),
            "T06 catalog fixture exact preimage differs",
        )?;
        rebuilt.extend_from_slice(&prior[end..at]);
        rebuilt.extend_from_slice(edit["insert_utf8"].as_str().unwrap().as_bytes());
        end = at + remove.len();
    }
    rebuilt.extend_from_slice(&prior[end..]);
    need(
        rebuilt == current,
        "T06 catalog fixture forward reconstruction differs",
    )?;
    Ok(prior)
}
fn caller(root: &Path, doc: &Value, row: &Value) -> Result<(), String> {
    need(
        *row == doc["caller_row"],
        "T06 catalog fixture original caller tuple differs",
    )?;
    let path = doc["container"]["path"]
        .as_str()
        .ok_or("T06 catalog fixture container absent")?;
    let raw = regular(root, path)?;
    material(&doc["container"], &raw, "0644")?;
    let binding: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        binding.pointer(
            doc["pointer"]
                .as_str()
                .ok_or("T06 catalog fixture pointer absent")?,
        ) == Some(row),
        "T06 catalog fixture frozen caller row changed",
    )
}
pub(crate) fn verify(
    root: &Path,
    row: &Value,
    supplied_current: &[u8],
) -> Option<Result<(), String>> {
    if row["file"] != SOURCE {
        return None;
    }
    Some((|| {
        let doc = load(root)?;
        caller(root, &doc, row)?;
        let physical = regular(root, SOURCE)?;
        need(
            supplied_current == physical,
            "T06 catalog fixture supplied bytes differ from physical current",
        )?;
        inverse(&doc, &physical, "0644")?;
        Ok(())
    })())
}
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    let current = regular(root, SOURCE)?;
    let row = &doc["caller_row"];
    verify(root, row, &current).ok_or("T06 fixture positive not selected")??;
    let prior = inverse(&doc, &current, "0644")?;
    need(
        verify(root, row, &prior).is_some_and(|r| r.is_err()),
        "T06 fixture historical bytes accepted as current",
    )?;
    for mode in ["0600", "0755", "4644"] {
        need(
            inverse(&doc, &current, mode).is_err(),
            "T06 fixture changed mode accepted",
        )?;
    }
    for field in ["sha256", "extra", "bytes", "mode"] {
        let mut bad = row.clone();
        bad[field] = json!("wrong");
        need(
            verify(root, &bad, &current).is_some_and(|r| r.is_err()),
            "T06 fixture wrong full caller row accepted",
        )?;
    }
    let mut missing = row.clone();
    missing.as_object_mut().unwrap().remove("sha256");
    need(
        verify(root, &missing, &current).is_some_and(|r| r.is_err()),
        "T06 fixture missing caller pin accepted",
    )?;
    let mut moved = doc.clone();
    moved["pointer"] = json!("/files/0");
    need(
        caller(root, &moved, row).is_err(),
        "T06 fixture moved caller row accepted",
    )?;
    let mut wrong = doc.clone();
    wrong["container"]["sha256"] = json!("wrong");
    need(
        caller(root, &wrong, row).is_err(),
        "T06 fixture altered caller container accepted",
    )?;
    let mut altered = current.clone();
    altered.push(b'\n');
    let mut rehashed = doc.clone();
    rehashed["current"]["sha256"] = json!(digest(&altered));
    rehashed["current"]["bytes"] = json!(altered.len());
    need(
        inverse(&rehashed, &altered, "0644").is_err(),
        "T06 fixture unrelated rehashed source accepted",
    )?;
    for mutation in 0..4 {
        let mut bad = doc.clone();
        let edits = bad["edits"].as_array_mut().unwrap();
        match mutation {
            0 => {
                edits.pop();
            }
            1 => {
                edits.insert(0, edits[0].clone());
            }
            2 => {
                edits[0]["at"] = json!(0);
            }
            _ => {
                edits[0]["insert_utf8"] = json!("unrelated");
            }
        }
        need(
            inverse(&bad, &current, "0644").is_err(),
            "T06 fixture missing/duplicate/moved/altered edit accepted",
        )?;
    }
    need(
        verify(root, &json!({"file":"unlisted.rs"}), b"wrong").is_none(),
        "T06 fixture unlisted source routing changed",
    )?;
    need(
        verify(Path::new("/missing-t06-fixture-root"), row, &current).is_some_and(|r| r.is_err()),
        "T06 fixture missing physical frame accepted",
    )?;
    let mut raw = regular(root, CAPSULE)?;
    raw.push(b'\n');
    need(
        document(&raw).is_err(),
        "T06 fixture altered capsule accepted",
    )?;
    Ok(())
}
