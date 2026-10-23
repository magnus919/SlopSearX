//! Exact child of the accepted666 A13 gate; frozen manifests remain immutable.
use serde_json::Value;
use std::path::Path;
pub(super) const SOURCE: &str = "crates/xtask/src/a13.rs";
const CAPSULE: &str =
    "docs/rust-rewrite/a06/source-amendments/a13-json-inventory-successor-v1.json";
const CAPSULE_SHA: &str = "fb2a6bd5578e33fbdf087817162841668631e276a905baad7fc90dd5ecee6cd3";
fn document(raw: &[u8]) -> Result<Value, String> {
    super::need(
        super::digest(raw) == CAPSULE_SHA,
        "A13 JSON inventory capsule changed",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    super::need(
        doc["schema"] == "A13_EXACT_JSON_COMPILED_INVENTORY_SUCCESSOR_V1"
            && doc["source_parent"] == "666dfbfbdf7a94ca6532a629724ec718e557dabd"
            && doc["prior"]["path"] == SOURCE
            && doc["current"]["path"] == SOURCE,
        "A13 JSON inventory closed source scope",
    )?;
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    let path = root.join(CAPSULE);
    super::need(
        super::permissions(&path)? == "0644",
        "A13 JSON inventory capsule mode",
    )?;
    document(&std::fs::read(path).map_err(|e| e.to_string())?)
}
fn validate(
    doc: &Value,
    expected: &Value,
    prior: &[u8],
    current: &[u8],
    mode: &str,
) -> Result<(), String> {
    super::need(
        doc["prior"] == *expected,
        "A13 JSON inventory exact666 caller",
    )?;
    super::material(&doc["prior"], prior, "0644")?;
    super::material(&doc["current"], current, mode)?;
    super::need(
        super::edits(prior, &doc["current_edits"])? == current,
        "A13 JSON inventory exact reconstruction",
    )
}
pub(super) fn archived_prior(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let prior = super::read(root, &doc["archive"])?;
    let current = super::a13_marker_successor::archived_prior(root, &doc["current"])?;
    validate(
        &doc,
        expected,
        &prior,
        &current,
        &super::permissions(&root.join(SOURCE))?,
    )?;
    Ok(prior)
}
pub(super) fn prove_current(root: &Path, expected: &Value, prior: &[u8]) -> Result<(), String> {
    let doc = load(root)?;
    let current = super::a13_marker_successor::archived_prior(root, &doc["current"])?;
    validate(
        &doc,
        expected,
        prior,
        &current,
        &super::permissions(&root.join(SOURCE))?,
    )
}
pub(super) fn effective_current(root: &Path, expected: &Value) -> Result<Value, String> {
    archived_prior(root, expected)?;
    super::a13_marker_successor::effective_current(root, &load(root)?["current"])
}
pub(super) fn library_source(root: &Path, expected: &Value) -> Result<(), String> {
    let doc = load(root)?;
    super::need(
        expected == &serde_json::json!({"path":SOURCE,"sha256":doc["prior"]["sha256"]}),
        "A06 frozen library exact A13 predecessor",
    )?;
    archived_prior(root, &doc["prior"])?;
    Ok(())
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    super::a13_marker_successor::controls(root)?;
    let doc = load(root)?;
    let prior = super::read(root, &doc["archive"])?;
    let current = super::a13_marker_successor::archived_prior(root, &doc["current"])?;
    validate(&doc, &doc["prior"], &prior, &current, "0644")?;
    super::a13_inventory_library_source(
        root,
        &serde_json::json!({"path":SOURCE,"sha256":doc["prior"]["sha256"]}),
    )?;
    let mut wrong = doc["prior"].clone();
    wrong["path"] = serde_json::json!("wrong-caller.rs");
    super::need(
        validate(&doc, &wrong, &prior, &current, "0644").is_err(),
        "A13 wrong inventory caller accepted",
    )?;
    super::need(
        validate(&doc, &doc["prior"], &prior, &prior, "0644").is_err(),
        "A13 old inventory accepted as current",
    )?;
    super::need(
        validate(&doc, &doc["prior"], &prior, &current, "0755").is_err(),
        "A13 changed inventory source mode accepted",
    )?;
    let mut changed = current.clone();
    changed.push(b'\n');
    super::need(
        validate(&doc, &doc["prior"], &prior, &changed, "0644").is_err(),
        "A13 changed inventory source accepted",
    )?;
    let mut changed_prior = prior.clone();
    changed_prior.push(b'\n');
    super::need(
        validate(&doc, &doc["prior"], &changed_prior, &current, "0644").is_err(),
        "A13 edited inventory archive accepted",
    )?;
    let mut changed_doc = doc.clone();
    changed_doc["current"]["sha256"] = serde_json::json!(super::digest(&changed));
    changed_doc["current"]["bytes"] = serde_json::json!(changed.len());
    super::need(
        document(&serde_json::to_vec(&changed_doc).map_err(|e| e.to_string())?).is_err(),
        "A13 rehashed inventory capsule accepted",
    )?;
    super::need(
        library_source(root, &serde_json::json!({"path":SOURCE,"sha256":"wrong"})).is_err(),
        "A06 changed frozen library caller accepted",
    )?;
    Ok(())
}
