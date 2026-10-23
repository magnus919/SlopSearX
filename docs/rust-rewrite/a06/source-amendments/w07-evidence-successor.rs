//! Exact W07 additive search seam; frozen manifests remain historical contracts.
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
pub(crate) const TEST: &str =
    "search::tests::current_policy_source_is_construction_only_and_preserves_default_generation";
const CAPSULE: &str = "docs/rust-rewrite/a06/source-amendments/w07-evidence-successor-v3.json";
const SHA: &str = "f1d1a1360485feba371aeace873cb5ae60090ea99e57e2ba3ade40f0e42a4be5";
const PREDECESSOR: &str = "docs/rust-rewrite/a06/source-amendments/w07-evidence-successor-v2.json";
const PREDECESSOR_SHA: &str = "8ad6a2eb0e1a574bbc1958ee5b157d47b806ead711e47d77d5e5ac1edf13155b";
const SEARCH: &str = "crates/ssx-infra/src/search.rs";
const PATHS: [&str; 3] = [
    SEARCH,
    "crates/xtask/src/a13.rs",
    "docs/rust-rewrite/a13/source-amendments/validator.rs",
];
fn need(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn raw_mode(mode: u32) -> String {
    format!("{:04o}", mode & 0o7777)
}
#[cfg(unix)]
fn permissions(path: &Path) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    Ok(raw_mode(
        fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode(),
    ))
}
#[cfg(not(unix))]
fn permissions(_: &Path) -> Result<String, String> {
    Err("A06 source mode proof requires Unix".into())
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str, String> {
    v[k].as_str().ok_or_else(|| format!("W07 missing {k}"))
}
fn document(raw: &[u8]) -> Result<Value, String> {
    need(digest(raw) == SHA, "W07 sealed successor capsule changed")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc.as_object().is_some_and(|m| m.len() == 5)
            && doc["schema"] == "W07_EVIDENCE_ADDITIVE_SUCCESSOR_V3"
            && doc["predecessor"]
                == serde_json::json!({"path":PREDECESSOR,"sha256":PREDECESSOR_SHA})
            && doc["test"] == TEST
            && doc["baseline"] == "233c7934f9e2da5c2e94f0c1213945335d786502",
        "W07 exact capsule scope",
    )?;
    let rows = doc["sources"].as_array().ok_or("W07 sources absent")?;
    need(
        rows.len() == 3
            && rows
                .iter()
                .map(|r| r["path"].as_str().unwrap_or(""))
                .eq(PATHS),
        "W07 exact ordered source membership",
    )?;
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    need(
        permissions(&root.join(CAPSULE))? == "0644"
            && permissions(&root.join(PREDECESSOR))? == "0644",
        "W07 capsule modes differ",
    )?;
    let doc = document(&std::fs::read(root.join(CAPSULE)).map_err(|e| e.to_string())?)?;
    let prior_raw = std::fs::read(root.join(PREDECESSOR)).map_err(|e| e.to_string())?;
    need(
        digest(&prior_raw) == PREDECESSOR_SHA,
        "W07 original capsule changed",
    )?;
    let prior: Value = serde_json::from_slice(&prior_raw).map_err(|e| e.to_string())?;
    need(
        prior["schema"] == "W07_EVIDENCE_ADDITIVE_SUCCESSOR_V2"
            && prior["baseline"] == doc["baseline"]
            && prior["test"] == doc["test"],
        "W07 predecessor scope differs",
    )?;
    for (old, next) in prior["sources"]
        .as_array()
        .ok_or("W07 prior sources absent")?
        .iter()
        .zip(doc["sources"].as_array().unwrap())
    {
        need(
            old["path"] == next["path"] && old["prior"] == next["prior"],
            "W07 original baseline changed",
        )?;
        // C adds only cancellation observation to search.rs. All other B
        // transitions, including the prior validator proof, remain byte exact.
        if next["path"] != SEARCH {
            need(old == next, "W07 unrelated source transition changed")?;
        }
    }
    Ok(doc)
}
fn row<'a>(doc: &'a Value, path: &str) -> Result<&'a Value, String> {
    doc["sources"]
        .as_array()
        .and_then(|rs| rs.iter().find(|r| r["path"] == path))
        .ok_or_else(|| "W07 unlisted source".into())
}
fn reconstruct(row: &Value, actual: &[u8], mode: &str) -> Result<Vec<u8>, String> {
    need(
        mode == "0644"
            && row.as_object().is_some_and(|m| m.len() == 4)
            && PATHS.contains(&text(row, "path")?),
        "W07 source mode/closed shape",
    )?;
    need(
        row["current"]["bytes"].as_u64() == Some(actual.len() as u64)
            && row["current"]["sha256"] == digest(actual),
        "W07 actual source differs",
    )?;
    let edits = row["edits"].as_array().ok_or("W07 edits absent")?;
    need(!edits.is_empty(), "W07 empty transition")?;
    let mut prior = Vec::new();
    let mut at = 0usize;
    for edit in edits {
        need(
            edit.as_object().is_some_and(|m| m.len() == 3),
            "W07 closed edit shape",
        )?;
        let start = edit["at"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or("W07 edit position")?;
        let after = text(edit, "after")?.as_bytes();
        let before = text(edit, "before")?.as_bytes();
        let end = start.checked_add(after.len()).ok_or("W07 edit overflow")?;
        need(
            start >= at && end <= actual.len() && actual.get(start..end) == Some(after),
            "W07 ordered exact edit differs",
        )?;
        prior.extend_from_slice(&actual[at..start]);
        prior.extend_from_slice(before);
        at = end;
    }
    prior.extend_from_slice(&actual[at..]);
    need(
        row["prior"]["bytes"].as_u64() == Some(prior.len() as u64)
            && row["prior"]["sha256"] == digest(&prior),
        "W07 historical source reconstruction differs",
    )?;
    Ok(prior)
}
pub(crate) fn prior(root: &Path, path: &str, expected: &Value) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let row = row(&doc, path)?;
    need(
        expected["path"] == path
            && expected["sha256"] == row["prior"]["sha256"]
            && expected["bytes"] == row["prior"]["bytes"]
            && expected["mode"] == "0644",
        "W07 exact predecessor caller",
    )?;
    let file = root.join(path);
    let actual = std::fs::read(&file).map_err(|e| e.to_string())?;
    reconstruct(row, &actual, &permissions(&file)?)
}
pub(crate) fn effective_current(
    root: &Path,
    path: &str,
    expected: &Value,
) -> Result<Value, String> {
    prior(root, path, expected)?;
    let doc = load(root)?;
    let mut current = row(&doc, path)?["current"].clone();
    current["path"] = Value::String(path.into());
    current["mode"] = Value::String("0644".into());
    Ok(current)
}
/// Exactly the two legacy validator contexts and the inventory declaration
/// proof; callers cannot turn an arbitrary current-read request into history.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SearchOperand {
    RuntimePin,
    T06NativeFixture,
    #[cfg(test)]
    AdditiveControl,
    Inventory,
}
pub(crate) fn search_operand(
    root: &Path,
    role: SearchOperand,
    expected: &Value,
    supplied: &[u8],
) -> Result<Vec<u8>, String> {
    let prior = self::prior(root, SEARCH, expected)?;
    let actual = std::fs::read(root.join(SEARCH)).map_err(|e| e.to_string())?;
    need(
        actual == supplied,
        "W07 supplied search operand is not actual source",
    )?;
    eprintln!(
        "W07_SEARCH_OPERAND_PROOF role={role:?} actual_sha256={} prior_sha256={}",
        digest(&actual),
        digest(&prior)
    );
    Ok(prior)
}
/// T06 keeps its frozen native row. Only this exact search predecessor may
/// consume the already sealed W07 transition; supplied bytes must be the actual
/// file, including the capsule's full current hash, length and Unix mode proof.
pub(crate) fn t06_native_search(
    root: &Path,
    frozen: &Value,
    supplied: &[u8],
) -> Result<Vec<u8>, String> {
    const FROZEN_SHA: &str = "fcac2929eb6637960d5e1086c321f0eac62fc0f8446cf73f22d6b43406137b12";
    need(
        frozen.as_object().is_some_and(|m| m.len() == 2)
            && frozen["file"] == SEARCH
            && frozen["sha256"] == FROZEN_SHA,
        "W07 exact T06 native caller differs",
    )?;
    let expected = serde_json::json!({
        "path": SEARCH, "sha256": FROZEN_SHA, "bytes": 86121, "mode": "0644"
    });
    search_operand(root, SearchOperand::T06NativeFixture, &expected, supplied)
}
pub(crate) fn prove_search(root: &Path) -> Result<(), String> {
    let file = root.join(SEARCH);
    let raw = std::fs::read(file).map_err(|e| e.to_string())?;
    let doc = load(root)?;
    let mut expected = row(&doc, SEARCH)?["prior"].clone();
    expected["path"] = Value::String(SEARCH.into());
    expected["mode"] = Value::String("0644".into());
    search_operand(root, SearchOperand::Inventory, &expected, &raw)?;
    let source = std::str::from_utf8(&raw).map_err(|e| e.to_string())?;
    need(
        source
            .matches(
                "fn current_policy_source_is_construction_only_and_preserves_default_generation()",
            )
            .count()
            == 1
            && TEST
                == "search::tests::current_policy_source_is_construction_only_and_preserves_default_generation",
        "W07 exact source-declared additive test identity",
    )
}
#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    prove_search(root)?;
    for path in PATHS {
        let row = row(&doc, path)?;
        let actual = std::fs::read(root.join(path)).map_err(|e| e.to_string())?;
        let prior = reconstruct(row, &actual, "0644")?;
        need(
            reconstruct(row, &actual, "0755").is_err(),
            "W07 wrong source mode accepted",
        )?;
        let mut changed = actual.clone();
        changed.push(0);
        need(
            reconstruct(row, &changed, "0644").is_err(),
            "W07 altered actual source accepted",
        )?;
        need(
            reconstruct(row, &prior, "0644").is_err(),
            "W07 old source accepted as current",
        )?;
        let mut changed = row.clone();
        changed["edits"][0]["before"] = json!("wrong");
        need(
            reconstruct(&changed, &actual, "0644").is_err(),
            "W07 rehashed edit substitution accepted",
        )?;
        let mut changed = row.clone();
        changed["path"] = json!("wrong.rs");
        need(
            reconstruct(&changed, &actual, "0644").is_err(),
            "W07 wrong source scope accepted",
        )?;
        let mut expected = row["prior"].clone();
        expected["path"] = json!(path);
        expected["mode"] = json!("0644");
        need(
            self::prior(root, path, &expected)? == prior,
            "W07 exact prior caller positive",
        )?;
        expected["sha256"] = json!("wrong");
        need(
            self::prior(root, path, &expected).is_err(),
            "W07 wrong prior caller accepted",
        )?;
    }
    let search_raw = std::fs::read(root.join(SEARCH)).map_err(|e| e.to_string())?;
    let mut expected = row(&doc, SEARCH)?["prior"].clone();
    expected["path"] = json!(SEARCH);
    expected["mode"] = json!("0644");
    for role in [
        SearchOperand::RuntimePin,
        SearchOperand::T06NativeFixture,
        SearchOperand::AdditiveControl,
        SearchOperand::Inventory,
    ] {
        let prior = search_operand(root, role, &expected, &search_raw)?;
        let mut bad = search_raw.clone();
        bad.push(0);
        need(
            search_operand(root, role, &expected, &bad).is_err(),
            "W07 changed supplied search operand accepted",
        )?;
        need(
            search_operand(root, role, &expected, &prior).is_err(),
            "W07 historical search supplied as actual",
        )?;
        let mut bad = expected.clone();
        bad["sha256"] = json!("wrong");
        need(
            search_operand(root, role, &bad, &search_raw).is_err(),
            "W07 wrong typed search predecessor accepted",
        )?;
    }
    let frozen = json!({"file":SEARCH,
        "sha256":"fcac2929eb6637960d5e1086c321f0eac62fc0f8446cf73f22d6b43406137b12"});
    let historical = t06_native_search(root, &frozen, &search_raw)?;
    need(
        historical.len() == 86121,
        "W07 T06 historical byte count differs",
    )?;
    for (key, value) in [
        ("file", json!("wrong.rs")),
        ("sha256", json!("wrong")),
        ("unlisted", json!(true)),
    ] {
        let mut bad = frozen.clone();
        bad[key] = value;
        need(
            t06_native_search(root, &bad, &search_raw).is_err(),
            "W07 changed T06 caller accepted",
        )?;
    }
    let mut bad = search_raw.clone();
    bad.push(0);
    need(
        t06_native_search(root, &frozen, &bad).is_err(),
        "W07 changed T06 supplied bytes accepted",
    )?;
    need(
        t06_native_search(root, &frozen, &historical).is_err(),
        "W07 historical T06 supplied as actual",
    )?;
    let raw = std::fs::read(root.join(CAPSULE)).map_err(|e| e.to_string())?;
    let mut changed = doc.clone();
    changed["test"] = json!("observed::unexpected");
    need(
        document(&serde_json::to_vec(&changed).map_err(|e| e.to_string())?).is_err(),
        "W07 substituted additive identity accepted",
    )?;
    let mut changed = raw;
    changed.push(0);
    need(document(&changed).is_err(), "W07 changed capsule accepted")?;
    Ok(())
}
