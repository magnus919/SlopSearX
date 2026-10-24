//! Closed portable-unit/CI child of the already accepted main and workflow bytes.
use super::super::{digest, edits, material, need, permissions, read, text};
use serde_json::{Value, json};
use std::path::Path;
const CAPSULE: &str =
    "docs/rust-rewrite/t07/source-amendments/gateway-unit-route-successor-v1.json";
const SHA: &str = "4fabaf5668fbe765a315e1399d54138db4d51e0342fc16331e0e791cdc6cacf2";
const BYTES: u64 = 17885;
const PATHS: [&str; 3] = [
    "crates/xtask/src/main.rs",
    ".github/workflows/rust.yml",
    ".pre-commit-config.yaml",
];
fn document(raw: &[u8]) -> Result<Value, String> {
    need(digest(raw) == SHA, "T07 unit-route capsule digest")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["kind"] == "T07_PORTABLE_UNIT_ROUTE_SUCCESSOR_V1"
            && doc["source_parent"] == "0f77197409ced9454e9930521f147f76fdd3c557"
            && doc["paths"] == json!(PATHS),
        "T07 unit-route exact closed paths",
    )?;
    let rows = doc["transitions"]
        .as_array()
        .ok_or("T07 unit-route transitions")?;
    need(
        rows.len() == PATHS.len(),
        "T07 unit-route closed transition count",
    )?;
    for (row, path) in rows.iter().zip(PATHS) {
        need(
            row["prior"]["path"] == path
                && row["current"]["path"] == path
                && row["archive"]["source_path"] == path
                && row["archive"]["path"]
                    == format!(
                        "docs/rust-rewrite/t07/source-amendments/gateway-unit-route-predecessor-v1/{path}"
                    ),
            "T07 unit-route exact material paths",
        )?;
    }
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    document(&read(
        root,
        &json!({"path":CAPSULE,"sha256":SHA,"bytes":BYTES,"mode":"0644"}),
    )?)
}
fn row<'a>(doc: &'a Value, expected: &Value) -> Result<&'a Value, String> {
    let path = text(expected, "path")?;
    need(PATHS.contains(&path), "T07 unit-route unlisted caller")?;
    let matches: Vec<_> = doc["transitions"]
        .as_array()
        .ok_or("T07 unit-route rows")?
        .iter()
        .filter(|r| r["prior"]["path"] == path)
        .collect();
    need(
        matches.len() == 1 && matches[0]["prior"] == *expected,
        "T07 unit-route exact predecessor caller",
    )?;
    Ok(matches[0])
}
fn validate(row: &Value, before: &[u8], actual: &[u8], mode: &str) -> Result<(), String> {
    material(&row["prior"], before, "0644")?;
    material(&row["current"], actual, mode)?;
    need(
        edits(before, &row["edits"])? == actual,
        "T07 unit-route exact ordered reconstruction",
    )
}
pub(super) fn archived_prior(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    read(root, &row(&doc, expected)?["archive"])
}
pub(super) fn actual_bytes(
    root: &Path,
    expected: &Value,
    before: &[u8],
) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let selected = row(&doc, expected)?;
    let actual = read(root, &selected["current"])?;
    validate(
        selected,
        before,
        &actual,
        &permissions(&root.join(text(expected, "path")?))?,
    )?;
    for row in doc["transitions"].as_array().ok_or("T07 unit-route rows")? {
        let before = read(root, &row["archive"])?;
        let actual = read(root, &row["current"])?;
        validate(
            row,
            &before,
            &actual,
            &permissions(&root.join(text(&row["current"], "path")?))?,
        )?;
    }
    Ok(actual)
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    for selected in doc["transitions"].as_array().ok_or("T07 unit-route rows")? {
        let expected = &selected["prior"];
        let before = read(root, &selected["archive"])?;
        let actual = actual_bytes(root, expected, &before)?;
        let mut wrong = expected.clone();
        wrong["sha256"] = json!("wrong");
        need(
            archived_prior(root, &wrong).is_err(),
            "T07 unit-route wrong predecessor accepted",
        )?;
        let mut bad = actual.clone();
        bad.push(0);
        need(
            validate(selected, &before, &bad, "0644").is_err(),
            "T07 unit-route extra delta accepted",
        )?;
        need(
            validate(selected, &before, &actual, "0755").is_err(),
            "T07 unit-route mode changed",
        )?;
        let mut missing = selected.clone();
        let _ = missing["edits"]
            .as_array_mut()
            .ok_or("T07 unit-route edits")?
            .pop();
        need(
            validate(&missing, &before, &actual, "0644").is_err(),
            "T07 unit-route missing span accepted",
        )?;
        let mut duplicate = selected.clone();
        let first = duplicate["edits"][0].clone();
        duplicate["edits"]
            .as_array_mut()
            .ok_or("T07 unit-route edits")?
            .insert(0, first);
        need(
            validate(&duplicate, &before, &actual, "0644").is_err(),
            "T07 unit-route duplicate span accepted",
        )?;
        let mut span = selected.clone();
        span["edits"][0]["at"] = json!(
            span["edits"][0]["at"]
                .as_u64()
                .ok_or("T07 unit-route offset")?
                + 1
        );
        need(
            validate(&span, &before, &actual, "0644").is_err(),
            "T07 unit-route changed span accepted",
        )?;
        let mut changed = doc.clone();
        changed["transitions"][0]["archive"]["source_path"] = json!("unlisted");
        need(
            document(&serde_json::to_vec(&changed).map_err(|e| e.to_string())?).is_err(),
            "T07 unit-route rehashed/wrong source document accepted",
        )?;
    }
    Ok(())
}
