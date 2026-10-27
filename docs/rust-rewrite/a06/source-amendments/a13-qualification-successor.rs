//! Exact current A13 qualification successor; the historical JSON remains frozen.
use serde_json::{Value, json};
use std::path::Path;

const QUALIFICATION: &str = "docs/rust-rewrite/a13/qualification-v1.json";
const CAPSULE: &str = "docs/rust-rewrite/a06/source-amendments/a13-qualification-successor-v1.json";
const CAPSULE_SHA: &str = "8720deb26dcefbc0c039622f3013c8cfb40797b9ba76c787f2bf1e28f442fa2a";
const LIBRARY_SUCCESSOR: &str =
    "docs/rust-rewrite/a06/source-amendments/current-library-inventory-successor-v1.json";
const LIBRARY_SUCCESSOR_SHA: &str =
    "dd95b35b0e5441b9f573eb33d60063a0e40bb2e0ffb659942f86e16ad38616f3";
const HISTORICAL_SHA: &str = "e2d9249fbadd0eca06a10215f796defc5845722c63fcf9f409ab9eac8a85f74e";
const CURRENT_SHA: &str = "77661b6d69dc2926a2d01d277e68e22932d7fb1aae258fb272ab1192dff18c6e";
const PRIOR_COMMIT: &str = "b430739d27419dd8eb95681311902168fa47f87b";
const PRIOR_GIT_BLOB: &str = "4101adeaf15dca3361a871a91574013b44f09f7b";
const INVENTORY: &str = "docs/rust-rewrite/a06/source-amendments/current-library-inventory-v1.json";
const INVENTORY_SHA: &str = "f631ecbced0d8ccec249c0d7b83e5155e892730c0597ac452c929216a254a24c";

fn document(raw: &[u8]) -> Result<Value, String> {
    super::need(
        super::digest(raw) == CAPSULE_SHA,
        "A13 qualification successor capsule changed",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    super::need(
        doc.as_object().is_some_and(|v| v.len() == 6)
            && doc["schema"] == "A13_CURRENT_QUALIFICATION_SUCCESSOR_V1"
            && doc["source_parent_commit"] == PRIOR_COMMIT
            && doc["historical"]
                == json!({
                    "path":QUALIFICATION,"bytes":147854,"mode":"0644",
                    "sha256":HISTORICAL_SHA,"git_blob":PRIOR_GIT_BLOB,
                    "archive_path":"docs/rust-rewrite/a06/source-amendments/a13-qualification.e2d924.frozen"
                })
            && doc["current"]
                == json!({"path":QUALIFICATION,"bytes":147854,"mode":"0644","sha256":CURRENT_SHA})
            && doc["source_binding"]
                == json!({
                    "path":"crates/xtask/src/a13.rs",
                    "capsule_path":"docs/rust-rewrite/a06/source-amendments/a13-marker-successor-v2.json",
                    "capsule_sha256":"adb407276f4a570fcc9ad12a11216320899887bbe5c928cfc408d590f6932c63",
                    "qualification_digest_edit_index":0
                }),
        "A13 qualification successor closed scope differs",
    )?;
    Ok(doc)
}

fn collect_changes(old: &Value, new: &Value, path: &str, out: &mut Vec<Value>) {
    match (old, new) {
        (Value::Object(a), Value::Object(b)) if a.keys().eq(b.keys()) => {
            for (key, value) in a {
                let escaped = key.replace('~', "~0").replace('/', "~1");
                collect_changes(value, &b[key], &format!("{path}/{escaped}"), out);
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (index, (left, right)) in a.iter().zip(b).enumerate() {
                collect_changes(left, right, &format!("{path}/{index}"), out);
            }
        }
        _ if old != new => out.push(json!({"pointer":path,"historical":old,"current":new})),
        _ => {}
    }
}

fn check_library_successor(root: &Path, qualification_successor_sha: &str) -> Result<(), String> {
    let raw = super::read(
        root,
        &json!({
            "path":LIBRARY_SUCCESSOR,
            "bytes":757,
            "mode":"0644",
            "sha256":LIBRARY_SUCCESSOR_SHA
        }),
    )?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let predecessor = super::read(
        root,
        &json!({
            "path":INVENTORY,
            "bytes":41041,
            "mode":"0644",
            "sha256":INVENTORY_SHA
        }),
    )?;
    super::need(
        doc.as_object().is_some_and(|v| v.len() == 4)
            && doc["schema"] == "SLOPSEARX_A06_CURRENT_LIBRARY_SUCCESSOR_V1"
            && doc["predecessor"]
                == json!({"path":INVENTORY,"bytes":predecessor.len(),"mode":"0644","sha256":INVENTORY_SHA})
            && super::digest(&predecessor) == INVENTORY_SHA
            && doc["qualification_successor"]
                == json!({
                    "path":CAPSULE,"bytes":7422,"mode":"0644",
                    "sha256":qualification_successor_sha
                })
            && doc["qualification_current"]
                == json!({"path":QUALIFICATION,"bytes":147854,"mode":"0644","sha256":CURRENT_SHA}),
        "A06 current library successor binding differs",
    )
}

fn verify_doc(root: &Path, doc: &Value, expected: &Value) -> Result<(), String> {
    let historical = &doc["historical"];
    let current = &doc["current"];
    super::need(
        expected["path"] == QUALIFICATION
            && expected["sha256"] == HISTORICAL_SHA
            && expected
                .get("bytes")
                .is_none_or(|v| v.as_u64() == Some(147854))
            && expected
                .get("mode")
                .is_none_or(|v| v.as_str() == Some("0644"))
            && historical["path"] == QUALIFICATION
            && historical["sha256"] == HISTORICAL_SHA
            && current["path"] == QUALIFICATION
            && current["sha256"] == CURRENT_SHA,
        "A13 qualification exact caller differs",
    )?;
    let archive_path = historical["archive_path"]
        .as_str()
        .ok_or("A13 qualification archive path absent")?;
    let archive_row =
        json!({"path":archive_path,"bytes":147854,"mode":"0644","sha256":HISTORICAL_SHA});
    let prior = super::read(root, &archive_row)?;
    let now = super::read(root, current)?;
    let (source_prior, source_current) =
        super::a13_marker_successor::qualification_digest_transition(root)?;
    super::need(
        source_prior == HISTORICAL_SHA && source_current == CURRENT_SHA,
        "A13 source does not bind the qualification successor",
    )?;
    let before: Value = serde_json::from_slice(&prior).map_err(|e| e.to_string())?;
    let after: Value = serde_json::from_slice(&now).map_err(|e| e.to_string())?;
    let mut changes = Vec::new();
    collect_changes(&before, &after, "", &mut changes);
    super::need(
        !changes.is_empty() && doc["changes"].as_array() == Some(&changes),
        "A13 qualification finite source transition differs",
    )?;
    Ok(())
}

pub(crate) fn verify(root: &Path, expected: &Value) -> Result<(), String> {
    let raw = super::read(
        root,
        &json!({
            "path":CAPSULE,
            "bytes":7422,
            "mode":"0644",
            "sha256":CAPSULE_SHA
        }),
    )?;
    let doc = document(&raw)?;
    check_library_successor(root, super::digest(&raw).as_str())?;
    verify_doc(root, &doc, expected)
}

#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<(), String> {
    let capsule = std::fs::read(root.join(CAPSULE)).map_err(|e| e.to_string())?;
    let doc = document(&capsule)?;
    verify(root, &doc["historical"])?;
    let mut wrong_hash = doc["historical"].clone();
    wrong_hash["sha256"] = json!(CURRENT_SHA);
    let mut wrong_bytes = doc["historical"].clone();
    wrong_bytes["bytes"] = json!(147853);
    let mut wrong_mode = doc["historical"].clone();
    wrong_mode["mode"] = json!("0755");
    for caller in [&wrong_hash, &wrong_bytes, &wrong_mode] {
        super::need(
            verify_doc(root, &doc, caller).is_err(),
            "A13 qualification wrong caller tuple accepted",
        )?;
    }
    let mut wrong = doc.clone();
    wrong["current"]["sha256"] = json!(HISTORICAL_SHA);
    super::need(
        document(&serde_json::to_vec(&wrong).map_err(|e| e.to_string())?).is_err(),
        "A13 wrong current qualification accepted",
    )?;
    let mut rehashed = doc.clone();
    rehashed["current"]["sha256"] =
        json!("0000000000000000000000000000000000000000000000000000000000000000");
    super::need(
        document(&serde_json::to_vec(&rehashed).map_err(|e| e.to_string())?).is_err(),
        "A13 rehashed qualification sidecar accepted",
    )?;
    let mut extra = doc;
    extra["changes"][0]["pointer"] = json!("/unrelated/path");
    super::need(
        document(&serde_json::to_vec(&extra).map_err(|e| e.to_string())?).is_err(),
        "A13 extra qualification path accepted",
    )?;
    Ok(())
}
