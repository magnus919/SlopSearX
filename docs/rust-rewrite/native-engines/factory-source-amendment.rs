//! Exact native factory insertion after the accepted A06 shared-current source.
//! The historical caller row remains immutable; actual regular0644 source is
//! authenticated and inverted before it can supply an old proof operand.
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::{fs, path::Path};
pub(super) const SOURCE: &str = "crates/ssx-infra/src/adapters/mod.rs";
const CAPSULE: &str = "docs/rust-rewrite/native-engines/factory-source-amendment.json";
const CAPSULE_SHA: &str = "72eb86dd437e7c6148fe964b381dc59d4597bb6e1015db7aef5439810b8e9710";
const PRIOR_SHA: &str = "ce4442703459ec6a1cf4da1961e99203d8c1207a58d823f084ed30f4d7df12ad";
fn regular(root: &Path, relative: &str) -> Result<(Vec<u8>, String), String> {
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    super::need(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "DeepSeek factory source must be regular, not symlink",
    )?;
    Ok((
        fs::read(&path).map_err(|e| e.to_string())?,
        super::permissions(&path)?,
    ))
}
fn load(root: &Path) -> Result<Value, String> {
    let (raw, mode) = regular(root, CAPSULE)?;
    super::need(
        mode == "0644" && super::digest(&raw) == CAPSULE_SHA,
        "DeepSeek factory capsule identity differs",
    )?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    super::need(
        doc["schema"] == "DEEPSEEK_FACTORY_EXACT_INSERTION_V1"
            && doc["baseline_commit"] == "e12744511013bd05370173e777daaef471bf9527"
            && doc["prior"]["path"] == SOURCE
            && doc["current"]["path"] == SOURCE
            && doc["prior"]["sha256"] == PRIOR_SHA,
        "DeepSeek factory capsule closed scope differs",
    )?;
    Ok(doc)
}
pub(super) fn selected(row: &Value) -> bool {
    row["path"] == SOURCE && row["sha256"] == PRIOR_SHA
}
fn inverse(doc: &Value, expected: &Value, physical: &[u8], mode: &str) -> Result<Vec<u8>, String> {
    let prior = &doc["prior"];
    super::need(
        expected["path"] == prior["path"]
            && expected["sha256"] == prior["sha256"]
            && expected["bytes"] == prior["bytes"]
            && expected["mode"] == prior["mode"]
            && expected
                .get("git_blob")
                .is_none_or(|blob| *blob == prior["git_blob"]),
        "DeepSeek factory exact historical caller tuple differs",
    )?;
    super::material(&doc["current"], physical, mode)?;
    let at = doc["insertion"]["at"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("DeepSeek factory insertion offset absent")?;
    let insertion = super::text(&doc["insertion"], "utf8")?.as_bytes();
    let end = at
        .checked_add(insertion.len())
        .ok_or("DeepSeek factory insertion overflow")?;
    super::need(
        !insertion.is_empty() && physical.get(at..end) == Some(insertion),
        "DeepSeek factory exact insertion differs",
    )?;
    let mut recovered = physical[..at].to_vec();
    recovered.extend_from_slice(&physical[end..]);
    super::material(prior, &recovered, "0644")?;
    let mut reconstructed = recovered[..at].to_vec();
    reconstructed.extend_from_slice(insertion);
    reconstructed.extend_from_slice(&recovered[at..]);
    super::need(
        reconstructed == physical,
        "DeepSeek factory forward reconstruction differs",
    )?;
    Ok(recovered)
}
fn materials(root: &Path, expected: &Value) -> Result<(Vec<u8>, Vec<u8>), String> {
    let doc = load(root)?;
    let (physical, mode) = regular(root, SOURCE)?;
    let prior = inverse(&doc, expected, &physical, &mode)?;
    Ok((prior, physical))
}
pub(super) fn prior(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    Ok(materials(root, expected)?.0)
}
#[cfg(test)]
pub(super) fn current(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    Ok(materials(root, expected)?.1)
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    let expected = &doc["prior"];
    let (prior, physical) = materials(root, expected)?;
    super::need(
        super::read_exact(root, expected)? == prior && current(root, expected)? == physical,
        "DeepSeek factory real historical reader/current positive differs",
    )?;
    super::need(
        inverse(&doc, expected, &prior, "0644").is_err(),
        "DeepSeek factory historical bytes accepted as actual current",
    )?;
    for mode in ["0600", "0755", "4644"] {
        super::need(
            inverse(&doc, expected, &physical, mode).is_err(),
            "DeepSeek factory mode mutant accepted",
        )?;
    }
    for field in ["path", "sha256", "bytes", "mode", "git_blob"] {
        let mut wrong = expected.clone();
        wrong[field] = json!("wrong");
        super::need(
            inverse(&doc, &wrong, &physical, "0644").is_err(),
            "DeepSeek factory wrong caller accepted",
        )?;
        if field != "git_blob" {
            wrong = expected.clone();
            wrong.as_object_mut().unwrap().remove(field);
            super::need(
                inverse(&doc, &wrong, &physical, "0644").is_err(),
                "DeepSeek factory incomplete caller accepted",
            )?;
        }
    }
    let at = doc["insertion"]["at"].as_u64().unwrap() as usize;
    let insertion = super::text(&doc["insertion"], "utf8")?.as_bytes();
    for mutation in 0..5 {
        let mut changed = physical.clone();
        match mutation {
            0 => {
                changed.drain(at..at + insertion.len());
            }
            1 => {
                changed.splice(at..at, insertion.iter().copied());
            }
            2 => {
                changed.drain(at..at + insertion.len());
                changed.extend_from_slice(insertion);
            }
            3 => {
                changed[at] ^= 1;
            }
            _ => {
                changed.push(b'\n');
            }
        }
        // Rehash only the proposed afterimage to reach the exact reconstruction
        // guard: a self-consistent unrelated hash is still not an amendment.
        let mut claim = doc.clone();
        claim["current"]["sha256"] = json!(super::digest(&changed));
        claim["current"]["bytes"] = json!(changed.len());
        super::need(
            inverse(&claim, expected, &changed, "0644").is_err(),
            "DeepSeek factory missing/duplicate/moved/altered/unrelated insertion accepted",
        )?;
    }
    super::need(
        self::prior(Path::new("/missing-deepseek-factory-root"), expected).is_err(),
        "DeepSeek factory missing physical source accepted",
    )?;
    let mut unrelated = expected.clone();
    unrelated["path"] = json!("unlisted.rs");
    super::need(
        !selected(&unrelated),
        "DeepSeek factory changed unlisted source routing",
    )?;
    Ok(())
}
