//! Exact header-marker child of recorded d448 A13; frozen qualification remains immutable.
use serde_json::Value;
use std::path::Path;
pub(super) const SOURCE: &str = "crates/xtask/src/a13.rs";
const CAPSULE: &str = "docs/rust-rewrite/a06/source-amendments/a13-marker-successor-v1.json";
const CAPSULE_SHA: &str = "f7a17eb80597d6e36e72de3dd693bec6b1a7343c44c2dc1cd421431fdf910690";
fn document(raw: &[u8]) -> Result<Value, String> {
    super::need(
        super::digest(raw) == CAPSULE_SHA,
        "A13 header marker capsule changed",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    super::need(
        doc["schema"] == "A13_EXACT_HEADER_MARKER_SUCCESSOR_V1"
            && doc["source_parent"] == "d44802020d321ba6eb5441dc8f53f3c5cbf286af"
            && doc["prior"]["path"] == SOURCE
            && doc["current"]["path"] == SOURCE,
        "A13 header marker closed source scope",
    )?;
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    let path = root.join(CAPSULE);
    super::need(
        super::permissions(&path)? == "0644",
        "A13 header marker capsule mode",
    )?;
    document(&std::fs::read(path).map_err(|e| e.to_string())?)
}
fn prior_caller() -> Value {
    serde_json::json!({
        "path": SOURCE,
        "bytes": 50563,
        "mode": "0644",
        "sha256": "fde085dcc7d153cf8304e69baa3c4a193e07501175e8f982ad10351f55da7fdc",
        "git_blob": "6329f367feac5befb3e9a45b84e844fbd2fc0949"
    })
}
fn current_caller() -> Value {
    serde_json::json!({
        "path": SOURCE,
        "bytes": 56700,
        "mode": "0644",
        "sha256": "a0396a72539e782ba129f6fb35d51e4097848242fa53dad1d7912c1232eb00bc",
        "git_blob": "963163633fcd35dea9258c0685dd68a5b4002316"
    })
}
fn validate(
    doc: &Value,
    expected: &Value,
    prior: &[u8],
    current: &[u8],
    mode: &str,
) -> Result<(), String> {
    super::need(
        *expected == prior_caller(),
        "A13 header marker exact d448 caller",
    )?;
    super::need(
        doc["prior"]
            == serde_json::json!({
                "path": SOURCE, "bytes": 50563, "mode": "0644",
                "sha256": "fde085dcc7d153cf8304e69baa3c4a193e07501175e8f982ad10351f55da7fdc"
            })
            && doc["current"]
                == serde_json::json!({
                    "path": SOURCE, "bytes": 56700, "mode": "0644",
                    "sha256": "a0396a72539e782ba129f6fb35d51e4097848242fa53dad1d7912c1232eb00bc"
                }),
        "A13 marker exact pinned four-field capsule material",
    )?;
    super::material(&doc["prior"], prior, "0644")?;
    super::material(&doc["current"], current, mode)?;
    super::need(
        super::edits(prior, &doc["current_edits"])? == current,
        "A13 header marker exact reconstruction",
    )
}
pub(super) fn archived_prior(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let prior = super::read(root, &doc["archive"])?;
    let current = super::read(root, &doc["current"])?;
    validate(
        &doc,
        expected,
        &prior,
        &current,
        &super::permissions(&root.join(SOURCE))?,
    )?;
    Ok(prior)
}
pub(super) fn effective_current(root: &Path, expected: &Value) -> Result<Value, String> {
    archived_prior(root, expected)?;
    Ok(current_caller())
}
#[cfg(test)]
pub(super) fn library_source(root: &Path, expected: &Value) -> Result<(), String> {
    let doc = load(root)?;
    super::need(
        expected == &serde_json::json!({"path":SOURCE,"sha256":doc["prior"]["sha256"]}),
        "A13 marker prior exact A13 predecessor",
    )?;
    archived_prior(root, &prior_caller())?;
    Ok(())
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    let prior = super::read(root, &doc["archive"])?;
    let current = super::read(root, &doc["current"])?;
    validate(&doc, &prior_caller(), &prior, &current, "0644")?;
    library_source(
        root,
        &serde_json::json!({"path":SOURCE,"sha256":doc["prior"]["sha256"]}),
    )?;
    let mut wrong = prior_caller();
    wrong["path"] = serde_json::json!("wrong-caller.rs");
    super::need(
        validate(&doc, &wrong, &prior, &current, "0644").is_err(),
        "A13 wrong inventory caller accepted",
    )?;
    super::need(
        validate(&doc, &prior_caller(), &prior, &prior, "0644").is_err(),
        "A13 old inventory accepted as current",
    )?;
    super::need(
        validate(&doc, &prior_caller(), &prior, &current, "0755").is_err(),
        "A13 changed inventory source mode accepted",
    )?;
    let caller = prior_caller();
    let inherited: Value =
        serde_json::from_slice(
            &std::fs::read(root.join(
                "docs/rust-rewrite/a06/source-amendments/a13-json-inventory-successor-v1.json",
            ))
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    super::need(
        inherited["current"] == caller,
        "A13 marker actual JSON caller differs",
    )?;
    archived_prior(root, &inherited["current"])?;
    super::need(
        effective_current(root, &caller)? == current_caller(),
        "A13 marker current Git caller differs",
    )?;
    let mut missing = caller.clone();
    missing
        .as_object_mut()
        .ok_or("A13 caller object")?
        .remove("git_blob");
    let mut wrong_git = caller.clone();
    wrong_git["git_blob"] = serde_json::json!("0000000000000000000000000000000000000000");
    let mut extra = caller.clone();
    extra["extra"] = serde_json::json!(true);
    for bad in [&missing, &wrong_git, &extra] {
        super::need(
            validate(&doc, bad, &prior, &current, "0644").is_err(),
            "A13 marker unclosed historical caller accepted",
        )?;
    }
    let mut expected_current = current_caller();
    expected_current["git_blob"] = serde_json::json!("0000000000000000000000000000000000000000");
    super::need(
        effective_current(root, &caller)? != expected_current,
        "A13 marker wrong current Git identity accepted",
    )?;
    let mut changed = current.clone();
    changed.push(b'\n');
    super::need(
        validate(&doc, &prior_caller(), &prior, &changed, "0644").is_err(),
        "A13 changed inventory source accepted",
    )?;
    let mut changed_prior = prior.clone();
    changed_prior.push(b'\n');
    super::need(
        validate(&doc, &prior_caller(), &changed_prior, &current, "0644").is_err(),
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
