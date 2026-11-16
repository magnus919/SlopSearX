//! Exact child of the accepted119a DeHashed source. No historical manifest is repinned.
use serde_json::Value;
use std::path::Path;
pub(super) const SOURCE: &str = "crates/ssx-infra/tests/a13_dehashed.rs";
const CAPSULE: &str = "docs/rust-rewrite/a13/source-amendments/header-layout-successor-v1.json";
const CAPSULE_SHA: &str = "dfcdd0f898f922734abafaca7d8ebc73d2f1ee79461cc4054c3393fa08d55f13";
fn document(raw: &[u8]) -> Result<Value, String> {
    super::need(
        super::digest(raw) == CAPSULE_SHA,
        "A13 header-layout capsule changed",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    super::need(
        doc["schema"] == "A13_EXACT_HEADER_LAYOUT_SUCCESSOR_V1"
            && doc["source_parent"] == "119a4eee9b8e7b7a577f75bcf8f18cbe4c6b46c9"
            && doc["prior"]["path"] == SOURCE
            && doc["current"]["path"] == SOURCE,
        "A13 header-layout closed source scope",
    )?;
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    let path = root.join(CAPSULE);
    super::need(
        super::permissions(&path)? == "0644",
        "A13 header-layout capsule mode",
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
        "A13 header-layout exact119a current caller tuple",
    )?;
    super::material(&doc["prior"], prior, "0644")?;
    super::material(&doc["current"], current, mode)?;
    super::need(
        super::edits(prior, &doc["current_edits"])? == current,
        "A13 header-layout exact current reconstruction",
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
pub(super) fn prove_current(root: &Path, expected: &Value, prior: &[u8]) -> Result<(), String> {
    let doc = load(root)?;
    let current = super::read(root, &doc["current"])?;
    validate(
        &doc,
        expected,
        prior,
        &current,
        &super::permissions(&root.join(SOURCE))?,
    )
}
// shared::compose_scoped has already converted these immutable UTF8 edits to
// byte arrays. Compare that exact caller form; never reinterpret arrays as text.
fn byte_edits(changes: &Value) -> Result<Value, String> {
    let rows = changes.as_array().ok_or("A13 scoped UTF8 edits missing")?;
    Ok(serde_json::json!(
        rows.iter()
            .map(|edit| {
                super::need(
                    edit.as_object().is_some_and(|object| object.len() == 3)
                        && edit["at"].as_u64().is_some(),
                    "A13 scoped closed UTF8 edit fields",
                )?;
                Ok(serde_json::json!({"at":edit["at"],
            "remove":super::text(edit,"remove_utf8")?.as_bytes(),
            "insert":super::text(edit,"insert_utf8")?.as_bytes()}))
            })
            .collect::<Result<Vec<Value>, String>>()?
    ))
}
fn scoped_payload(
    doc: &Value,
    row: &Value,
    historical: &[u8],
    prior: &[u8],
    current: &[u8],
    legacy_edits: &Value,
) -> Result<Value, String> {
    validate(doc, &row["current"], prior, current, "0644")?;
    super::material(&doc["scoped_historical"], historical, "0644")?;
    super::need(
        row["historical191"] == doc["scoped_historical"]
            && row["edits"] == byte_edits(legacy_edits)?
            && super::edits(historical, legacy_edits)? == prior,
        "A13 header-layout exact original scoped caller/proof retained",
    )?;
    super::need(
        super::edits(historical, &doc["scoped_edits"])? == current,
        "A13 header-layout exact scoped current reconstruction",
    )?;
    let mut next = row.clone();
    next["current"] = doc["current"].clone();
    next["edits"] = byte_edits(&doc["scoped_edits"])?;
    Ok(next)
}
pub(super) fn scoped(root: &Path, row: &Value, historical: &[u8]) -> Result<Value, String> {
    let doc = load(root)?;
    let prior = archived_prior(root, &row["current"])?;
    let current = super::read(root, &doc["current"])?;
    let shared = super::load(root)?;
    let original = super::source(&shared, SOURCE)?;
    scoped_payload(
        &doc,
        row,
        historical,
        &prior,
        &current,
        &original["scoped_edits"],
    )
}
fn scoped_controls(root: &Path, doc: &Value, prior: &[u8], current: &[u8]) -> Result<(), String> {
    let shared = super::load(root)?;
    let original = super::source(&shared, SOURCE)?;
    let original_scoped: Value =
        serde_json::from_slice(&super::read(root, &shared["scoped_predecessor"])?)
            .map_err(|e| e.to_string())?;
    let original_row = original_scoped["successors"]
        .as_array()
        .ok_or("A13 original scoped rows missing")?
        .iter()
        .find(|row| row["current"]["path"] == SOURCE)
        .ok_or("A13 exact original scoped source missing")?;
    let historical = super::read(root, &original_row["historical191"])?;
    // This is the full actual caller representation immediately before scoped().
    let mut caller = original_row.clone();
    caller["current"] = original["current"].clone();
    caller["edits"] = byte_edits(&original["scoped_edits"])?;
    let actual = scoped(root, &caller, &historical)?;
    let expected = scoped_payload(
        doc,
        &caller,
        &historical,
        prior,
        current,
        &original["scoped_edits"],
    )?;
    super::need(
        actual == expected
            && actual["current"] == doc["current"]
            && actual["edits"] == byte_edits(&doc["scoped_edits"])?
            && caller["current"] == doc["prior"],
        "A13 complete actual scoped caller-to-byte-output roundtrip",
    )?;
    for mutation in 0..7 {
        let mut bad = caller.clone();
        match mutation {
            0 => bad["edits"] = original["scoped_edits"].clone(),
            1 => {
                bad["edits"].as_array_mut().unwrap().pop();
            }
            2 => {
                let first = bad["edits"][0].clone();
                bad["edits"].as_array_mut().unwrap().insert(0, first);
            }
            3 => {
                bad["edits"][0]["remove"] = serde_json::json!([0]);
            }
            4 => {
                bad["edits"][0]["extra"] = serde_json::json!(true);
            }
            5 => {
                bad["current"]["path"] = serde_json::json!("wrong-caller.rs");
            }
            _ => {
                bad["historical191"]["sha256"] = serde_json::json!("wrong");
            }
        }
        super::need(
            scoped_payload(
                doc,
                &bad,
                &historical,
                prior,
                current,
                &original["scoped_edits"],
            )
            .is_err(),
            "A13 mutated actual scoped caller representation accepted",
        )?;
    }
    super::need(
        scoped_payload(
            doc,
            &caller,
            &historical,
            prior,
            prior,
            &original["scoped_edits"],
        )
        .is_err(),
        "A13 old bytes accepted as scoped current",
    )?;
    Ok(())
}
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    let prior = super::read(root, &doc["archive"])?;
    let current = super::read(root, &doc["current"])?;
    validate(&doc, &doc["prior"], &prior, &current, "0644")?;
    scoped_controls(root, &doc, &prior, &current)?;
    let mut wrong = doc["prior"].clone();
    wrong["path"] = serde_json::json!("wrong-caller.rs");
    super::need(
        validate(&doc, &wrong, &prior, &current, "0644").is_err(),
        "A13 wrong caller accepted",
    )?;
    super::need(
        validate(&doc, &doc["prior"], &prior, &prior, "0644").is_err(),
        "A13 old-as-current accepted",
    )?;
    super::need(
        validate(&doc, &doc["prior"], &prior, &current, "0755").is_err(),
        "A13 changed current mode accepted",
    )?;
    let mut changed = current.clone();
    changed.push(b'\n');
    super::need(
        validate(&doc, &doc["prior"], &prior, &changed, "0644").is_err(),
        "A13 edited current accepted",
    )?;
    let mut changed_prior = prior.clone();
    changed_prior.push(b'\n');
    super::need(
        validate(&doc, &doc["prior"], &changed_prior, &current, "0644").is_err(),
        "A13 edited archive accepted",
    )?;
    let mut changed_doc = doc.clone();
    changed_doc["current"]["sha256"] = serde_json::json!(super::digest(&changed));
    changed_doc["current"]["bytes"] = serde_json::json!(changed.len());
    let raw = serde_json::to_vec(&changed_doc).map_err(|e| e.to_string())?;
    super::need(
        document(&raw).is_err(),
        "A13 rehashed changed current capsule accepted",
    )?;
    let mut changed = std::fs::read(root.join(CAPSULE)).map_err(|e| e.to_string())?;
    changed.push(b'\n');
    super::need(
        document(&changed).is_err(),
        "A13 changed capsule framing accepted",
    )?;
    eprintln!(
        "A13_HEADER_LAYOUT_CONTROLS exact119a_archive_current_and_scoped_proofs_wrong_caller_mode_old_as_current_and_rehashed_edits_rejected=true"
    );
    Ok(())
}
