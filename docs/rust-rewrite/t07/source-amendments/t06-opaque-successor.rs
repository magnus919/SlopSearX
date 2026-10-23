//! Exact current T07 opaque bytes compose with the immutable T06 native row.
use super::super::{digest, edits, need, read};
use serde_json::{Value, json};
use std::path::Path;
const CAPSULE: &str = "docs/rust-rewrite/t07/source-amendments/t06-opaque-successor-v1.json";
const SHA: &str = "924bd720bf302560345ca264629e3aa4ce80b52df89897e9164b4cc5bc33f1c1";
const BYTES: u64 = 32211;
fn document(raw: &[u8]) -> Result<Value, String> {
    need(digest(raw) == SHA, "T06 opaque capsule identity")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["kind"] == "T07_T06_EXACT_OPAQUE_SUCCESSOR_V1"
            && doc["source_parent"] == "14b5b9ba612ac975e13b46ea1a53363870f9ff7e"
            && doc["archive"]["path"]
                == "docs/rust-rewrite/t07/source-amendments/t06-opaque-predecessor-v1/opaque.rs"
            && doc["current"]["path"] == "crates/ssx-mcp/src/opaque.rs",
        "T06 opaque closed source scope",
    )?;
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    document(&read(
        root,
        &json!({"path":CAPSULE,"sha256":SHA,"bytes":BYTES,"mode":"0644"}),
    )?)
}
fn owner(raw: &[u8]) -> Result<&[u8], String> {
    let text = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
    let start = text
        .find("pub async fn stdio_with_factory_owned")
        .ok_or("T06 owner start")?;
    let end = start
        + text[start..]
            .find("fn validate_reply")
            .ok_or("T06 owner end")?;
    Ok(&raw[start..end])
}
fn validate(doc: &Value, expected: &Value, prior: &[u8], actual: &[u8]) -> Result<(), String> {
    need(
        *expected
            == json!({"file":"crates/ssx-mcp/src/opaque.rs","sha256":"c89e508d9d4d2c86f6d0f0f8cbf8f35690e83e63da94a4ca342c28da60526d41"})
            && doc["original_native_caller"] == *expected,
        "T06 opaque exact historical caller",
    )?;
    need(
        digest(prior) == expected["sha256"],
        "T06 opaque predecessor identity",
    )?;
    need(
        edits(prior, &doc["edits"])? == actual && digest(actual) == doc["current"]["sha256"],
        "T06 opaque ordered current reconstruction",
    )?;
    let before = owner(prior)?;
    let after = owner(actual)?;
    need(
        before == after
            && after.len() == 4554
            && digest(after) == "2782b71431ef1260541b240a4930d29e0e2c107f9cf8111e98b5d5c8f0605f19",
        "T06 exact default owner changed",
    )
}
pub(super) fn prove(root: &Path, expected: &Value, supplied_current: &[u8]) -> Result<(), String> {
    let doc = load(root)?;
    let prior = read(root, &doc["archive"])?;
    let actual = read(root, &doc["current"])?;
    need(
        supplied_current == actual,
        "T06 supplied current opaque differs",
    )?;
    validate(&doc, expected, &prior, &actual)
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    let prior = read(root, &doc["archive"])?;
    let actual = read(root, &doc["current"])?;
    let expected = &doc["original_native_caller"];
    prove(root, expected, &actual)?;
    let mut wrong = expected.clone();
    wrong["file"] = json!("unlisted.rs");
    need(
        validate(&doc, &wrong, &prior, &actual).is_err(),
        "T06 unlisted opaque caller accepted",
    )?;
    let mut wrong = prior.clone();
    wrong.push(0);
    need(
        validate(&doc, expected, &wrong, &actual).is_err(),
        "T06 changed opaque predecessor accepted",
    )?;
    let mut wrong = actual.clone();
    wrong.push(0);
    need(
        prove(root, expected, &wrong).is_err(),
        "T06 changed opaque current accepted",
    )?;
    need(
        validate(&doc, expected, &prior, &prior).is_err(),
        "T06 historical opaque accepted as current",
    )?;
    let mut wrong = doc.clone();
    wrong["edits"] = json!([]);
    need(
        validate(&wrong, expected, &prior, &actual).is_err(),
        "T06 omitted opaque edits accepted",
    )?;
    let mut wrong = doc.clone();
    wrong["current"]["sha256"] = json!(digest(&prior));
    need(
        document(&serde_json::to_vec(&wrong).map_err(|e| e.to_string())?).is_err(),
        "T06 rehashed opaque capsule accepted",
    )?;
    Ok(())
}
