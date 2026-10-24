//! Finite DNS-only child; preserves A06 and T07 lock reconstruction unchanged.
use super::super::{digest, edits, material, need, permissions, read};
use serde_json::{Value, json};
use std::path::Path;
const CAPSULE: &str = "docs/rust-rewrite/t07/source-amendments/gateway-dns-lock-successor-v1.json";
const SHA: &str = "0ec1e1848d6e770cd54d9e9a49f86699eb5afe54deb7f011232c3b2c3f469833";
const BYTES: u64 = 1532;
fn document(raw: &[u8]) -> Result<Value, String> {
    need(digest(raw) == SHA, "T07 DNS capsule digest")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["kind"] == "T07_GATEWAY_DNS_LOCK_SUCCESSOR_V1"
            && doc["source_parent"] == "757fbc2686db7f0601cfbbd1ecfc42b9f0dbdbd8"
            && doc["paths"] == json!(["Cargo.lock"])
            && doc["prior"]["path"] == "Cargo.lock"
            && doc["current"]["path"] == "Cargo.lock"
            && doc["archive"]["path"]
                == "docs/rust-rewrite/t07/source-amendments/gateway-dns-predecessor-v1/Cargo.lock"
            && doc["archive"]["source_path"] == "Cargo.lock",
        "T07 DNS closed lock scope",
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
        "T07 DNS exact predecessor caller",
    )?;
    material(&doc["prior"], before, "0644")?;
    material(&doc["current"], actual, mode)?;
    need(
        edits(before, &doc["edits"])? == actual,
        "T07 DNS ordered lock reconstruction",
    )
}
pub(super) fn archived_prior(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    need(
        doc["prior"] == *expected,
        "T07 DNS exact predecessor caller",
    )?;
    read(root, &doc["archive"])
}
pub(super) fn actual_bytes(
    root: &Path,
    expected: &Value,
    before: &[u8],
) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let actual = super::gateway_cookie_proxy_lock_successor::archived_prior(root, &doc["current"])?;
    validate(
        &doc,
        expected,
        before,
        &actual,
        &permissions(&root.join("Cargo.lock"))?,
    )?;
    super::gateway_cookie_proxy_lock_successor::actual_bytes(root, &doc["current"], &actual)
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    let expected = &doc["prior"];
    let before = read(root, &doc["archive"])?;
    let actual = super::gateway_cookie_proxy_lock_successor::archived_prior(root, &doc["current"])?;
    validate(&doc, expected, &before, &actual, "0644")?;
    let mut wrong = expected.clone();
    wrong["path"] = json!("Cargo.unlisted");
    need(
        validate(&doc, &wrong, &before, &actual, "0644").is_err(),
        "T07 DNS wrong caller accepted",
    )?;
    let mut bad = before.clone();
    bad.push(0);
    need(
        validate(&doc, expected, &bad, &actual, "0644").is_err(),
        "T07 DNS altered predecessor accepted",
    )?;
    let mut bad = actual.clone();
    bad.push(0);
    need(
        validate(&doc, expected, &before, &bad, "0644").is_err(),
        "T07 DNS unrelated lock delta accepted",
    )?;
    need(
        validate(&doc, expected, &before, &actual, "0755").is_err(),
        "T07 DNS changed mode accepted",
    )?;
    let mut changed = doc.clone();
    changed["current"]["sha256"] = json!(digest(&bad));
    changed["current"]["bytes"] = json!(bad.len());
    need(
        document(&serde_json::to_vec(&changed).map_err(|e| e.to_string())?).is_err(),
        "T07 DNS rehashed capsule accepted",
    )?;
    for (from, to) in [("0.103.16", "0.103.15"), ("1.0.10", "1.0.9")] {
        let rollback = std::str::from_utf8(&actual)
            .map_err(|e| e.to_string())?
            .replacen(from, to, 1);
        need(
            validate(&doc, expected, &before, rollback.as_bytes(), "0644").is_err(),
            "T07 DNS TLS rollback accepted",
        )?;
    }
    super::gateway_cookie_proxy_lock_successor::controls(root)?;
    Ok(())
}
