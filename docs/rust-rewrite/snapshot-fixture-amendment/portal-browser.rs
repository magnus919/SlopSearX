//! Exact T03 browser-client source successor for the immutable F01 snapshot.
use serde_json::Value;
use std::path::Path;
pub(super) const SOURCE: &str = "tests/test_native_html_theme_browser.py";
const CAPSULE: &str = "docs/rust-rewrite/snapshot-fixture-amendment/portal-browser-v1.json";
include!("portal-browser-sha.rs");
fn document(root: &Path) -> super::Result<Value> {
    let raw = super::regular(root, CAPSULE)?;
    super::need(
        super::digest(&raw) == SHA,
        "portal browser source capsule digest differs",
    )?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    super::need(
        doc["schema"] == "PORTAL_BROWSER_F01_SOURCE_V1"
            && doc["baseline_commit"] == "e12744511013bd05370173e777daaef471bf9527"
            && doc["source"]["prior"]["path"] == SOURCE
            && doc["source"]["current"]["path"] == SOURCE
            && doc["binding"]["kind"] == "F01"
            && doc["binding"]["container"]["path"] == "crates/xtask/fixtures/F01.json"
            && doc["binding"]["pointer"] == "/sources/443"
            && doc.as_object().is_some_and(|object| object.len() == 4),
        "portal browser source capsule scope differs",
    )?;
    Ok(doc)
}
pub(super) fn f01(root: &Path, claimed: &Value) -> super::Result<()> {
    let doc = document(root)?;
    let binding = &doc["binding"];
    super::need(
        binding["row"] == *claimed
            && binding["row"]["path"] == SOURCE
            && binding["row"]["git_blob"] == "3ae3c76b87e0f4ce5662ec72024965a8d2e80043",
        "portal browser exact F01 caller differs",
    )?;
    let frame = &binding["container"];
    let raw = super::regular_with_bound(root, "crates/xtask/fixtures/F01.json", 1_048_576)?;
    super::material(frame, &raw, "0644")?;
    let fixture: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    super::need(
        fixture.pointer("/sources/443") == Some(&binding["row"]),
        "portal browser F01 source row moved or changed",
    )?;
    verify_source(root, &doc["source"])
}
fn verify_source(root: &Path, source: &Value) -> super::Result<()> {
    let current = super::regular(root, SOURCE)?;
    super::material(&source["current"], &current, "0644")?;
    super::need(
        source["current"]["git_blob"] == super::git_blob(root, &current)?
            && source["prior"]["git_blob"] == "3ae3c76b87e0f4ce5662ec72024965a8d2e80043",
        "portal browser current/prior Git identity differs",
    )?;
    let prior = source["prior"]["source_utf8"]
        .as_str()
        .ok_or("portal browser prior source absent")?
        .as_bytes();
    let replacement = source["current"]["source_utf8"]
        .as_str()
        .ok_or("portal browser current source absent")?
        .as_bytes();
    let prior_text = std::str::from_utf8(prior).map_err(|e| e.to_string())?;
    let current_text = std::str::from_utf8(replacement).map_err(|e| e.to_string())?;
    super::need(
        source["edit"]["offset"] == 0
            && replacement == current.as_slice()
            && !prior.is_empty()
            && source["edit"]["remove_utf8"].as_str() == Some(prior_text)
            && source["edit"]["insert_utf8"].as_str() == Some(current_text),
        "portal browser whole-source transition differs",
    )?;
    super::material(&source["prior"], prior, "0644")?;
    super::need(
        prior.len() as u64 == 5117
            && super::digest(prior)
                == "8f783396281adef6e14699f107b516d7257d54ee16c7170ff05e46cd9fac944b"
            && super::git_blob(root, prior)? == "3ae3c76b87e0f4ce5662ec72024965a8d2e80043",
        "portal browser historical source identity differs",
    )?;
    Ok(())
}
