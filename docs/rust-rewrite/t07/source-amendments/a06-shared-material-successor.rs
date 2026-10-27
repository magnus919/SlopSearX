//! Exact T07 child of A06's accepted shared materials; historical parents remain intact.
use serde_json::{Value, json};
use std::path::Path;
const CAPSULE: &str =
    "docs/rust-rewrite/t07/source-amendments/a06-shared-material-successor-v1.json";
const CAPSULE_SHA: &str = "f106ec3a76a2c84a31bdfa85330a9538e7a03bd2ce1d69c68f3c27ef4f5cdbec";
const CAPSULE_BYTES: u64 = 22354;
#[path = "gateway-cookie-proxy-lock-successor.rs"]
mod gateway_cookie_proxy_lock_successor;
#[path = "gateway-dns-lock-successor.rs"]
mod gateway_dns_lock_successor;
#[path = "gateway-unit-route-successor.rs"]
mod gateway_unit_route_successor;
#[path = "gateway-xtask-main-successor.rs"]
mod gateway_xtask_main_successor;
#[path = "t06-opaque-successor.rs"]
mod t06_opaque_successor;
const PATHS: [&str; 2] = ["Cargo.toml", "Cargo.lock"];

pub(super) fn selected(path: &str) -> bool {
    PATHS.contains(&path) || gateway_xtask_main_successor::selected(path)
}
fn document(raw: &[u8]) -> Result<Value, String> {
    super::need(
        super::digest(raw) == CAPSULE_SHA,
        "T07 shared capsule digest",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    super::need(
        doc["kind"] == "T07_A06_EXACT_SHARED_MATERIAL_SUCCESSOR_V1"
            && doc["source_parent"] == "51fda0f70e2051e145478783ca9c32e1f4de9596"
            && doc["paths"] == json!(PATHS)
            && doc["transitions"]
                .as_array()
                .is_some_and(|rows| rows.len() == PATHS.len()),
        "T07 shared capsule closed scope",
    )?;
    for (index, path) in PATHS.iter().enumerate() {
        let row = &doc["transitions"][index];
        super::need(
            row["prior"]["path"] == *path
                && row["current"]["path"] == *path
                && row["archive"]["source_path"] == *path,
            "T07 shared capsule exact paths",
        )?;
    }
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    let raw = super::read(
        root,
        &json!({
            "path": CAPSULE, "sha256": CAPSULE_SHA,
            "bytes": CAPSULE_BYTES, "mode": "0644"
        }),
    )?;
    document(&raw)
}
fn row<'a>(doc: &'a Value, expected: &Value) -> Result<&'a Value, String> {
    let path = super::text(expected, "path")?;
    super::need(selected(path), "T07 unlisted shared caller path")?;
    let rows = doc["transitions"]
        .as_array()
        .ok_or("T07 shared transitions absent")?;
    let found = rows
        .iter()
        .filter(|row| row["prior"]["path"] == path)
        .collect::<Vec<_>>();
    super::need(found.len() == 1, "T07 shared caller missing/duplicate")?;
    super::need(
        found[0]["prior"] == *expected,
        "T07 exact A06 caller material",
    )?;
    Ok(found[0])
}
fn validate(
    doc: &Value,
    expected: &Value,
    prior: &[u8],
    actual: &[u8],
    mode: &str,
) -> Result<(), String> {
    let row = row(doc, expected)?;
    super::material(&row["prior"], prior, "0644")?;
    super::material(&row["current"], actual, mode)?;
    super::need(
        super::edits(prior, &row["edits"])? == actual,
        "T07 shared ordered reconstruction differs",
    )
}
fn materials(root: &Path, expected: &Value) -> Result<(Vec<u8>, Vec<u8>, Value), String> {
    let doc = load(root)?;
    let row = row(&doc, expected)?;
    let prior = super::read(root, &row["archive"])?;
    let actual = if expected["path"] == "Cargo.lock" {
        gateway_dns_lock_successor::archived_prior(root, &row["current"])?
    } else {
        let proof = super::main_isolation_current::parent(
            root,
            "Cargo.toml",
            None,
            super::main_isolation_current::Parent::T07,
        )?;
        super::material(&row["current"], &proof, "0644")?;
        proof
    };
    validate(
        &doc,
        expected,
        &prior,
        &actual,
        &super::permissions(&root.join(super::text(expected, "path")?))?,
    )?;
    Ok((prior, actual, row["current"].clone()))
}
pub(super) fn archived_prior(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    if gateway_xtask_main_successor::selected(super::text(expected, "path")?) {
        return gateway_xtask_main_successor::archived_prior(root, expected);
    }
    Ok(materials(root, expected)?.0)
}
pub(super) fn prove_current(
    root: &Path,
    expected: &Value,
    supplied_prior: &[u8],
) -> Result<(), String> {
    if gateway_xtask_main_successor::selected(super::text(expected, "path")?) {
        let prior = gateway_xtask_main_successor::archived_prior(root, expected)?;
        super::need(
            prior == supplied_prior,
            "T07 xtask supplied predecessor differs",
        )?;
        gateway_xtask_main_successor::actual_bytes(root, expected, &prior)?;
        return Ok(());
    }
    let (prior, actual, current) = materials(root, expected)?;
    super::need(
        prior == supplied_prior,
        "T07 supplied A06 predecessor differs",
    )?;
    if expected["path"] == "Cargo.lock" {
        gateway_dns_lock_successor::actual_bytes(root, &current, &actual)?;
    }
    eprintln!(
        "T07_A06_SHARED_CURRENT_PROOF path={} a06_sha256={} current_sha256={}",
        super::text(expected, "path")?,
        super::digest(&prior),
        super::digest(&actual)
    );
    Ok(())
}
pub(super) fn actual_bytes(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    if gateway_xtask_main_successor::selected(super::text(expected, "path")?) {
        let prior = gateway_xtask_main_successor::archived_prior(root, expected)?;
        return gateway_xtask_main_successor::actual_bytes(root, expected, &prior);
    }
    let (_, actual, current) = materials(root, expected)?;
    if expected["path"] == "Cargo.lock" {
        gateway_dns_lock_successor::actual_bytes(root, &current, &actual)
    } else {
        Ok(actual)
    }
}

#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    for row in doc["transitions"].as_array().ok_or("T07 controls scope")? {
        let expected = &row["prior"];
        let prior = super::read(root, &row["archive"])?;
        let (_, actual, _) = materials(root, expected)?;
        validate(&doc, expected, &prior, &actual, "0644")?;
        let mut wrong = expected.clone();
        wrong["path"] = json!("Cargo.unlisted");
        super::need(
            validate(&doc, &wrong, &prior, &actual, "0644").is_err(),
            "T07 unlisted caller accepted",
        )?;
        let mut wrong = expected.clone();
        wrong["sha256"] = json!("changed");
        super::need(
            validate(&doc, &wrong, &prior, &actual, "0644").is_err(),
            "T07 changed caller digest accepted",
        )?;
        let mut bad_prior = prior.clone();
        bad_prior.push(0);
        super::need(
            validate(&doc, expected, &bad_prior, &actual, "0644").is_err(),
            "T07 changed A06 archive accepted",
        )?;
        let mut bad_actual = actual.clone();
        bad_actual.push(0);
        super::need(
            validate(&doc, expected, &prior, &bad_actual, "0644").is_err(),
            "T07 unrelated current delta accepted",
        )?;
        super::need(
            validate(&doc, expected, &prior, &actual, "0755").is_err(),
            "T07 changed current mode accepted",
        )?;
        let mut wrong = doc.clone();
        let changed = wrong["transitions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|item| item["prior"] == *expected)
            .unwrap();
        changed["current"]["sha256"] = json!(super::digest(&bad_actual));
        changed["current"]["bytes"] = json!(bad_actual.len());
        let rehashed = serde_json::to_vec_pretty(&wrong).map_err(|e| e.to_string())?;
        super::need(
            document(&rehashed).is_err(),
            "T07 self-rehashed unapproved capsule accepted",
        )?;
        let mut wrong = doc.clone();
        wrong["transitions"].as_array_mut().unwrap().pop();
        super::need(
            document(&serde_json::to_vec(&wrong).map_err(|e| e.to_string())?).is_err(),
            "T07 omitted material accepted",
        )?;
        let mut wrong = doc.clone();
        wrong["transitions"][0]["edits"] = json!([]);
        super::need(
            document(&serde_json::to_vec(&wrong).map_err(|e| e.to_string())?).is_err(),
            "T07 changed ordered edit policy accepted",
        )?;
        if expected["path"] == "Cargo.lock" {
            for (from, to) in [("0.103.16", "0.103.15"), ("1.0.10", "1.0.9")] {
                let rolled_back = std::str::from_utf8(&actual)
                    .map_err(|e| e.to_string())?
                    .replacen(from, to, 1);
                super::need(
                    validate(&doc, expected, &prior, rolled_back.as_bytes(), "0644").is_err(),
                    "T07 TLS rollback accepted",
                )?;
            }
        }
    }
    gateway_dns_lock_successor::controls(root)?;
    gateway_xtask_main_successor::controls(root)?;
    t06_opaque_successor::controls(root)?;
    Ok(())
}

// Called by the shared xtask T06 bridge; standalone A13 includes this module too.
#[allow(dead_code)]
pub(super) fn t06_opaque_material(
    root: &Path,
    expected: &Value,
    actual: &[u8],
) -> Result<(), String> {
    t06_opaque_successor::prove(root, expected, actual)
}
