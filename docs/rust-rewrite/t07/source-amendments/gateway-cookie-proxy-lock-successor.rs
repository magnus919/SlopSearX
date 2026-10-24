//! Finite six-edge CookieJar/proxy child; preserves the exact historical DNS lock hop.
use super::super::{digest, edits, material, need, permissions, read};
use serde_json::{Value, json};
use std::path::Path;
const CAPSULE: &str =
    "docs/rust-rewrite/t07/source-amendments/gateway-cookie-proxy-lock-successor-v1.json";
const SHA: &str = "5f263afdf0766e00208850fe94d999654ce02a7288d76c3d2da822fe1aeb1f80";
const BYTES: u64 = 1544;
fn document(raw: &[u8]) -> Result<Value, String> {
    need(digest(raw) == SHA, "T07 cookie/proxy capsule digest")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["kind"] == "T07_GATEWAY_COOKIE_PROXY_LOCK_SUCCESSOR_V1"
            && doc["source_parent"] == "c6147891422f56e02ca36195ca866776d3a50f3e"
            && doc["paths"] == json!(["Cargo.lock"])
            && doc["prior"]["path"] == "Cargo.lock"
            && doc["current"]["path"] == "Cargo.lock"
            && doc["archive"]["path"]
                == "docs/rust-rewrite/t07/source-amendments/gateway-cookie-proxy-predecessor-v1/Cargo.lock"
            && doc["archive"]["source_path"] == "Cargo.lock",
        "T07 cookie/proxy closed lock scope",
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
        "T07 cookie/proxy exact predecessor caller",
    )?;
    material(&doc["prior"], before, "0644")?;
    material(&doc["current"], actual, mode)?;
    need(
        edits(before, &doc["edits"])? == actual,
        "T07 cookie/proxy ordered lock reconstruction",
    )
}
pub(super) fn archived_prior(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    need(
        doc["prior"] == *expected,
        "T07 cookie/proxy exact predecessor caller",
    )?;
    read(root, &doc["archive"])
}
pub(super) fn actual_bytes(
    root: &Path,
    expected: &Value,
    before: &[u8],
) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let actual = read(root, &doc["current"])?;
    validate(
        &doc,
        expected,
        before,
        &actual,
        &permissions(&root.join("Cargo.lock"))?,
    )?;
    Ok(actual)
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    let expected = &doc["prior"];
    let before = read(root, &doc["archive"])?;
    let actual = actual_bytes(root, expected, &before)?;
    let mut wrong = expected.clone();
    wrong["path"] = json!("Cargo.unlisted");
    need(
        validate(&doc, &wrong, &before, &actual, "0644").is_err(),
        "T07 cookie/proxy wrong caller accepted",
    )?;
    let mut wrong_row = expected.clone();
    wrong_row["sha256"] = json!("changed");
    need(
        validate(&doc, &wrong_row, &before, &actual, "0644").is_err(),
        "T07 cookie/proxy wrong predecessor row accepted",
    )?;
    let mut bad = before.clone();
    bad.push(0);
    need(
        validate(&doc, expected, &bad, &actual, "0644").is_err(),
        "T07 cookie/proxy altered predecessor accepted",
    )?;
    let mut bad = actual.clone();
    bad.push(0);
    need(
        validate(&doc, expected, &before, &bad, "0644").is_err(),
        "T07 cookie/proxy unrelated lock delta accepted",
    )?;
    need(
        validate(&doc, expected, &before, &actual, "0755").is_err(),
        "T07 cookie/proxy changed mode accepted",
    )?;
    let mut changed = doc.clone();
    changed["current"]["sha256"] = json!(digest(&bad));
    changed["current"]["bytes"] = json!(bad.len());
    need(
        document(&serde_json::to_vec(&changed).map_err(|e| e.to_string())?).is_err(),
        "T07 cookie/proxy rehashed capsule accepted",
    )?;
    let mut missing = doc.clone();
    missing["edits"]
        .as_array_mut()
        .ok_or("T07 cookie/proxy edits")?
        .pop();
    need(
        validate(&missing, expected, &before, &actual, "0644").is_err(),
        "T07 cookie/proxy missing edge accepted",
    )?;
    let mut duplicate = doc.clone();
    let first = duplicate["edits"][0].clone();
    duplicate["edits"]
        .as_array_mut()
        .ok_or("T07 cookie/proxy edits")?
        .insert(0, first);
    need(
        validate(&duplicate, expected, &before, &actual, "0644").is_err(),
        "T07 cookie/proxy duplicate edge accepted",
    )?;
    let mut span = doc.clone();
    span["edits"][0]["at"] = json!(
        span["edits"][0]["at"]
            .as_u64()
            .ok_or("T07 cookie/proxy span")?
            + 1
    );
    need(
        validate(&span, expected, &before, &actual, "0644").is_err(),
        "T07 cookie/proxy tampered span accepted",
    )?;
    for (field, value) in [
        ("source_path", "Cargo.unlisted"),
        ("path", "unlisted/Cargo.lock"),
    ] {
        let mut source = doc.clone();
        source["archive"][field] = json!(value);
        need(
            document(&serde_json::to_vec(&source).map_err(|e| e.to_string())?).is_err(),
            "T07 cookie/proxy changed archive source accepted",
        )?;
    }
    for (from, to) in [("0.103.16", "0.103.15"), ("1.0.10", "1.0.9")] {
        let rollback = std::str::from_utf8(&actual)
            .map_err(|e| e.to_string())?
            .replacen(from, to, 1);
        need(
            validate(&doc, expected, &before, rollback.as_bytes(), "0644").is_err(),
            "T07 cookie/proxy TLS rollback accepted",
        )?;
    }
    Ok(())
}
