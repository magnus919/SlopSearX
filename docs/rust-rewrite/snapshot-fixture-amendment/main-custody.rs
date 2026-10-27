//! Exact two-insertion successor of the e127 native gate entrypoint.
//! Preserves old source-chain operands only after proving physical current bytes.
use serde_json::Value;
use std::{fs, path::Path};
const CAPSULE: &str = "docs/rust-rewrite/snapshot-fixture-amendment/capsule.json";
const SHA: &str = "e8f0b635c6a8a8e9d472587259f54061b6b9f6de345956848571e5b3a12e3256";
const SOURCE: &str = "crates/xtask/src/main.rs";
const PRIOR_SHA: &str = "8755129fbac7454a1ae25982dc44ce1cb92b8ead7ecf650a3901aa2f08d4a23c";
pub(super) fn selected(row: &Value) -> bool {
    row["path"] == SOURCE && row["sha256"] == PRIOR_SHA
}
fn regular(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    super::need(
        metadata.is_file()
            && !metadata.file_type().is_symlink()
            && super::permissions(&path)? == "0644",
        "snapshot fixture gate source requires regular0644",
    )?;
    fs::read(path).map_err(|e| e.to_string())
}
fn inverse(doc: &Value, expected: &Value, current: &[u8], mode: &str) -> Result<Vec<u8>, String> {
    let row = &doc["main"];
    super::need(
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
        "snapshot fixture exact e127 main caller differs",
    )?;
    super::material(&row["current"], current, mode)?;
    let edits = row["edits"]
        .as_array()
        .ok_or("snapshot fixture main edits absent")?;
    super::need(
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
        let insert = super::text(edit, "insert_utf8")?.as_bytes();
        super::need(
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
        super::need(
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
    super::material(&row["prior"], &prior, "0644")?;
    super::need(
        super::edits(&prior, &row["edits"])? == current,
        "snapshot fixture main forward reconstruction differs",
    )?;
    Ok(prior)
}
pub(super) fn prior(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    let raw = regular(root, CAPSULE)?;
    super::need(
        super::digest(&raw) == SHA,
        "snapshot fixture main capsule identity differs",
    )?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    super::need(
        doc["schema"] == "PORTABLE_SNAPSHOT_FIXTURE_SOURCE_V1"
            && doc["main"]["prior"]["path"] == SOURCE
            && doc["main"]["prior"]["sha256"] == PRIOR_SHA,
        "snapshot fixture main closed source differs",
    )?;
    inverse(&doc, expected, &regular(root, SOURCE)?, "0644")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_fixture_amendment_exact_main_custody_controls() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let doc: Value = serde_json::from_slice(&regular(&root, CAPSULE).unwrap()).unwrap();
        let expected = &doc["main"]["prior"];
        let current = regular(&root, SOURCE).unwrap();
        let historical = prior(&root, expected).unwrap();
        assert_eq!(
            super::super::read_exact(&root, expected).unwrap(),
            historical
        );
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
        rehashed["main"]["current"]["sha256"] = serde_json::json!(super::super::digest(&changed));
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
    }
}
