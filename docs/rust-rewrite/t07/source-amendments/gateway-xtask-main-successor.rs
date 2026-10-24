//! Exact A06 xtask entrypoint successor; historical source reconstruction stays intact.
use super::super::{digest, edits, material, need, permissions, read};
use serde_json::{Value, json};
use std::path::Path;
const CAPSULE: &str =
    "docs/rust-rewrite/t07/source-amendments/gateway-xtask-main-successor-v1.json";
const SHA: &str = "30f1769633cbdf282d01ea74fab690d0071d00f0c028c16e4b501b6448b1c1e6";
const BYTES: u64 = 1866;
pub(super) fn selected(path: &str) -> bool {
    path == "crates/xtask/src/main.rs"
}
fn document(raw: &[u8]) -> Result<Value, String> {
    need(digest(raw) == SHA, "T07 xtask entrypoint capsule digest")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["kind"] == "T07_GATEWAY_XTASK_MAIN_SUCCESSOR_V1"
            && doc["source_parent"] == "233c7934f9e2da5c2e94f0c1213945335d786502"
            && doc["paths"] == json!(["crates/xtask/src/main.rs"])
            && doc["prior"]["path"] == "crates/xtask/src/main.rs"
            && doc["current"]["path"] == "crates/xtask/src/main.rs"
            && doc["archive"]["path"]
                == "docs/rust-rewrite/t07/source-amendments/gateway-xtask-main-predecessor-v1/main.rs"
            && doc["archive"]["source_path"] == "crates/xtask/src/main.rs",
        "T07 xtask entrypoint closed source scope",
    )?;
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    document(&read(
        root,
        &json!({"path":CAPSULE,"sha256":SHA,"bytes":BYTES,"mode":"0644"}),
    )?)
}
fn validate(
    doc: &Value,
    expected: &Value,
    before: &[u8],
    actual: &[u8],
    mode: &str,
) -> Result<(), String> {
    need(
        doc["prior"] == *expected,
        "T07 xtask entrypoint exact predecessor caller",
    )?;
    material(&doc["prior"], before, "0644")?;
    material(&doc["current"], actual, mode)?;
    need(
        edits(before, &doc["edits"])? == actual,
        "T07 xtask entrypoint ordered source reconstruction",
    )
}
pub(super) fn archived_prior(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    need(
        doc["prior"] == *expected,
        "T07 xtask entrypoint exact predecessor caller",
    )?;
    read(root, &doc["archive"])
}
pub(super) fn actual_bytes(
    root: &Path,
    expected: &Value,
    before: &[u8],
) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let actual = super::gateway_unit_route_successor::archived_prior(root, &doc["current"])?;
    validate(
        &doc,
        expected,
        before,
        &actual,
        &permissions(&root.join("crates/xtask/src/main.rs"))?,
    )?;
    super::gateway_unit_route_successor::actual_bytes(root, &doc["current"], &actual)
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    let expected = &doc["prior"];
    let before = read(root, &doc["archive"])?;
    let actual = super::gateway_unit_route_successor::archived_prior(root, &doc["current"])?;
    validate(&doc, expected, &before, &actual, "0644")?;
    let mut wrong = expected.clone();
    wrong["path"] = json!("Cargo.unlisted");
    need(
        validate(&doc, &wrong, &before, &actual, "0644").is_err(),
        "T07 xtask entrypoint wrong caller accepted",
    )?;
    let mut bad = before.clone();
    bad.push(0);
    need(
        validate(&doc, expected, &bad, &actual, "0644").is_err(),
        "T07 xtask entrypoint altered predecessor accepted",
    )?;
    let mut bad = actual.clone();
    bad.push(0);
    need(
        validate(&doc, expected, &before, &bad, "0644").is_err(),
        "T07 xtask entrypoint unrelated source delta accepted",
    )?;
    need(
        validate(&doc, expected, &before, &actual, "0755").is_err(),
        "T07 xtask entrypoint changed mode accepted",
    )?;
    let mut changed = doc.clone();
    changed["current"]["sha256"] = json!(digest(&bad));
    changed["current"]["bytes"] = json!(bad.len());
    need(
        document(&serde_json::to_vec(&changed).map_err(|e| e.to_string())?).is_err(),
        "T07 xtask entrypoint rehashed capsule accepted",
    )?;
    need(
        validate(&doc, expected, &before, &before, "0644").is_err(),
        "T07 xtask historical source accepted as current",
    )?;
    let mut changed = doc.clone();
    changed["edits"] = json!([]);
    need(
        validate(&changed, expected, &before, &actual, "0644").is_err(),
        "T07 xtask omitted edits accepted",
    )?;
    super::gateway_unit_route_successor::controls(root)?;
    Ok(())
}
