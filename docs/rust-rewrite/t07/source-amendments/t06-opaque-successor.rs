//! Exact current T07 opaque bytes compose with the immutable T06 native row.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
const CAPSULE: &str = "docs/rust-rewrite/t07/source-amendments/t06-opaque-successor-v1.json";
const SHA: &str = "924bd720bf302560345ca264629e3aa4ce80b52df89897e9164b4cc5bc33f1c1";
const BYTES: u64 = 32211;
const CURRENT_CAPSULE: &str =
    "docs/rust-rewrite/t07/source-amendments/t06-opaque-current-operand-v1.json";
const CURRENT_SHA: &str = "900f9c32eb17a791cdba1d4a780878bf03ef003aeeefff98127ee7e986ee7e44";
const CURRENT_BYTES: u64 = 9684;
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
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key]
        .as_str()
        .ok_or_else(|| format!("A06 missing {key}"))
}
fn material(row: &Value, raw: &[u8], mode: &str) -> Result<(), String> {
    need(
        row["sha256"] == digest(raw)
            && row["bytes"].as_u64() == Some(raw.len() as u64)
            && row["mode"] == mode,
        "A06 exact source material differs",
    )
}
fn read(root: &Path, row: &Value) -> Result<Vec<u8>, String> {
    let path = root.join(text(row, "path")?);
    let stat = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        stat.is_file() && !stat.file_type().is_symlink(),
        "A06 source must be regular, not symlink",
    )?;
    let raw = fs::read(&path).map_err(|e| e.to_string())?;
    material(row, &raw, &permissions(&path)?)?;
    Ok(raw)
}
fn edits(before: &[u8], changes: &Value) -> Result<Vec<u8>, String> {
    let rows = changes.as_array().ok_or("A06 ordered edits absent")?;
    need(!rows.is_empty(), "A06 ordered edits absent")?;
    let mut output = Vec::new();
    let mut end = 0;
    for row in rows {
        let at = row["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("A06 edit offset")?;
        let remove = text(row, "remove_utf8")?.as_bytes();
        let insert = text(row, "insert_utf8")?.as_bytes();
        let next = at.checked_add(remove.len()).ok_or("A06 edit overflow")?;
        need(
            at >= end
                && next <= before.len()
                && &before[at..next] == remove
                && (!remove.is_empty() || !insert.is_empty()),
            "A06 edit preimage/order differs",
        )?;
        output.extend_from_slice(&before[end..at]);
        output.extend_from_slice(insert);
        end = next;
    }
    output.extend_from_slice(&before[end..]);
    Ok(output)
}
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
// This is a finite current operand for the existing immutable T06 proof, not
// a replacement for its archive/capsule or a generic source-reader fallback.
fn current_row(index: usize) -> Value {
    match index {
        0 => {
            json!({"path":"crates/ssx-mcp/src/opaque.rs","sha256":"bcd6a423ff2b917b7253718401e7618231099901cf0d178bf1ee04b2a8d82d3f","bytes":104941,"mode":"0644","git_commit":"0a44833af3d19f46e2dab006ea2fcb69732a7ba4","git_blob":"2176565525f764611f2cdd3bb702e7dfce621b59"})
        }
        1 => {
            json!({"path":"crates/ssx-mcp/src/opaque.rs","sha256":"3fe5f2d3f32ea3de1a5da9479631bd5eff4a5253931ca0d764814a80e4628f64","bytes":104612,"mode":"0644","git_commit":"11ee12000d642cca1d1388499155b9f818af17a6","git_blob":"9de9692eb9fecc5a5445b4fd0ff931c8b94e9c2a"})
        }
        2 => {
            json!({"path":"crates/ssx-mcp/src/opaque.rs","sha256":"f46db8ce513e3c399169f3560324982c19562f05cc7c5b4658be581c81bc14ab","bytes":103482,"mode":"0644","git_commit":"c23d3148ec028e1c230593f6877506a152ab3271","git_blob":"1cc2f92ad584f92c98aa516e144bd586dc6d2fe4"})
        }
        _ => Value::Null,
    }
}
fn current_shape(doc: &Value) -> Result<(), String> {
    need(
        doc["schema_version"] == 1
            && doc["kind"] == "T07_T06_EXACT_OPAQUE_CURRENT_OPERAND_V1"
            && doc["original_capsule_sha256"] == SHA
            && doc["current"] == current_row(0)
            && doc["prior"] == current_row(2)
            && doc["default_owner"]
                == json!({"bytes":4554,"sha256":"2782b71431ef1260541b240a4930d29e0e2c107f9cf8111e98b5d5c8f0605f19"}),
        "T06 opaque current closed lineage differs",
    )?;
    let stages = doc["inverse_stages"]
        .as_array()
        .ok_or("T06 opaque current stages absent")?;
    need(stages.len() == 2, "T06 opaque current stage count differs")?;
    for (index, stage) in stages.iter().enumerate() {
        need(
            stage["from"] == current_row(index) && stage["to"] == current_row(index + 1),
            "T06 opaque current stage identity differs",
        )?;
    }
    Ok(())
}
fn current_document(raw: &[u8]) -> Result<Value, String> {
    need(
        digest(raw) == CURRENT_SHA && raw.len() as u64 == CURRENT_BYTES,
        "T06 opaque current capsule identity",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    current_shape(&doc)?;
    Ok(doc)
}
fn load_current(root: &Path) -> Result<Value, String> {
    current_document(&read(
        root,
        &json!({"path":CURRENT_CAPSULE,"sha256":CURRENT_SHA,"bytes":CURRENT_BYTES,"mode":"0644"}),
    )?)
}
fn git_current_material(root: &Path, row: &Value, raw: &[u8]) -> Result<(), String> {
    let commit = text(row, "git_commit")?;
    let path = text(row, "path")?;
    let blob = text(row, "git_blob")?;
    let tree = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-tree", "-z", commit, "--", path])
        .output()
        .map_err(|e| e.to_string())?;
    need(
        tree.status.success() && tree.stdout == format!("100644 blob {blob}\t{path}\0").as_bytes(),
        "T06 opaque current Git tree identity differs",
    )?;
    let content = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "blob", blob])
        .output()
        .map_err(|e| e.to_string())?;
    need(
        content.status.success() && content.stdout == raw,
        "T06 opaque current Git blob differs",
    )
}
fn unchanged_current_owner(actual: &[u8], prior: &[u8]) -> Result<(), String> {
    let current_owner = owner(actual)?;
    need(
        current_owner.len() == 4554
            && digest(current_owner)
                == "2782b71431ef1260541b240a4930d29e0e2c107f9cf8111e98b5d5c8f0605f19"
            && current_owner == owner(prior)?,
        "T06 opaque current changed default owner",
    )
}
fn validate_current(
    root: &Path,
    doc: &Value,
    expected: &Value,
    actual: &[u8],
    mode: &str,
) -> Result<Vec<u8>, String> {
    current_shape(doc)?;
    need(
        *expected
            == json!({"file":"crates/ssx-mcp/src/opaque.rs","sha256":"c89e508d9d4d2c86f6d0f0f8cbf8f35690e83e63da94a4ca342c28da60526d41"}),
        "T06 opaque current historical caller differs",
    )?;
    material(&doc["current"], actual, mode)?;
    git_current_material(root, &doc["current"], actual)?;
    let mut proof = actual.to_vec();
    for stage in doc["inverse_stages"]
        .as_array()
        .ok_or("T06 opaque current stages absent")?
    {
        material(&stage["from"], &proof, "0644")?;
        proof = edits(&proof, &stage["inverse"])?;
        material(&stage["to"], &proof, "0644")?;
        git_current_material(root, &stage["to"], &proof)?;
        unchanged_current_owner(actual, &proof)?;
    }
    material(&doc["prior"], &proof, "0644")?;
    Ok(proof)
}
// The client-only boundary adds one cfg to a server-fixture test. The exact
// physical afterimage is authenticated before removing that one annotation;
// the complete accepted Git/current/owner proof below remains unchanged.
fn isolation_prior(physical: &[u8], source_mode: &str) -> Result<Vec<u8>, String> {
    need(
        source_mode == "0644"
            && physical.len() == 104972
            && digest(physical)
                == "eb37f197c6b3b5f4af4c5e01f8b75a26fa735da397d70f7c74a45842634f4f7f",
        "T06 isolation physical identity",
    )?;
    let at = 98689;
    let annotation = b"    #[cfg(feature = \"server\")]\n";
    need(
        physical.get(at..at + annotation.len()) == Some(annotation),
        "T06 isolation exact cfg span",
    )?;
    let mut accepted = physical.to_vec();
    accepted.drain(at..at + annotation.len());
    material(&current_row(0), &accepted, "0644")?;
    Ok(accepted)
}
fn current_operand(
    root: &Path,
    expected: &Value,
    supplied: Option<&[u8]>,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let doc = load_current(root)?;
    // One authenticated physical buffer is compared with the supplied bytes and
    // then inverted. A recovered historical proof operand is never physical input.
    let path = root.join("crates/ssx-mcp/src/opaque.rs");
    let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "T06 isolation source must be regular",
    )?;
    let physical = fs::read(&path).map_err(|e| e.to_string())?;
    let accepted = isolation_prior(&physical, &permissions(&path)?)?;
    if let Some(supplied) = supplied {
        need(supplied == physical, "T06 supplied current opaque differs")?;
    }
    material(&doc["current"], &accepted, "0644")?;
    let prior = validate_current(root, &doc, expected, &accepted, "0644")?;
    Ok((physical, prior))
}

pub(crate) fn prove(root: &Path, expected: &Value, supplied_current: &[u8]) -> Result<(), String> {
    let doc = load(root)?;
    let prior = read(root, &doc["archive"])?;
    let (_, actual) = current_operand(root, expected, Some(supplied_current))?;
    // The original frozen ordered reconstruction/default-owner proof is unchanged.
    validate(&doc, expected, &prior, &actual)
}
#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    let prior = read(root, &doc["archive"])?;
    let expected = &doc["original_native_caller"];
    let (physical, actual) = current_operand(root, expected, None)?;
    prove(root, expected, &physical)?;
    current_controls(root, expected, &physical, &actual)?;
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

#[cfg(test)]
fn current_controls(
    root: &Path,
    expected: &Value,
    physical: &[u8],
    proof_prior: &[u8],
) -> Result<(), String> {
    let doc = load_current(root)?;
    let accepted = isolation_prior(physical, "0644")?;
    need(
        isolation_prior(&accepted, "0644").is_err(),
        "T06 accepted parent supplied as isolation current",
    )?;
    need(
        isolation_prior(physical, "0600").is_err(),
        "T06 isolation physical mode",
    )?;
    need(
        prove(root, expected, proof_prior).is_err(),
        "T06 old proof preimage accepted as physical current",
    )?;
    let mut altered = physical.to_vec();
    altered[0] ^= 1;
    need(
        prove(root, expected, &altered).is_err(),
        "T06 altered supplied physical accepted",
    )?;
    need(
        validate_current(root, &doc, expected, &accepted, "0600").is_err(),
        "T06 wrong physical mode accepted",
    )?;
    for key in ["path", "sha256", "mode", "git_commit", "git_blob"] {
        let mut wrong = doc.clone();
        wrong["current"][key] = json!("unlisted");
        need(
            validate_current(root, &wrong, expected, &accepted, "0644").is_err(),
            "T06 wrong current row accepted",
        )?;
    }
    let mut wrong_caller = expected.clone();
    wrong_caller["file"] = json!("unlisted.rs");
    need(
        prove(root, &wrong_caller, physical).is_err(),
        "T06 wrong current caller accepted",
    )?;
    let mut wrong_caller_hash = expected.clone();
    wrong_caller_hash["sha256"] = json!("unlisted");
    need(
        prove(root, &wrong_caller_hash, physical).is_err(),
        "T06 wrong current caller hash accepted",
    )?;
    need(
        validate_current(root, &doc, expected, &altered, "0644").is_err(),
        "T06 altered physical hash accepted",
    )?;
    let intermediate = edits(&accepted, &doc["inverse_stages"][0]["inverse"])?;
    need(
        prove(root, expected, &intermediate).is_err(),
        "T06 intermediate proof frame accepted as physical",
    )?;
    let mut omitted_stage = doc.clone();
    omitted_stage["inverse_stages"]
        .as_array_mut()
        .unwrap()
        .pop();
    need(
        validate_current(root, &omitted_stage, expected, &accepted, "0644").is_err(),
        "T06 missing inverse stage accepted",
    )?;
    for stage in 0..2 {
        let rows = doc["inverse_stages"][stage]["inverse"]
            .as_array()
            .ok_or("T06 current inverse absent")?;
        for index in 0..rows.len() {
            let mut missing = doc.clone();
            missing["inverse_stages"][stage]["inverse"]
                .as_array_mut()
                .unwrap()
                .remove(index);
            need(
                validate_current(root, &missing, expected, &accepted, "0644").is_err(),
                "T06 missing current inverse span accepted",
            )?;
            let mut duplicate = doc.clone();
            duplicate["inverse_stages"][stage]["inverse"]
                .as_array_mut()
                .unwrap()
                .insert(index, rows[index].clone());
            need(
                validate_current(root, &duplicate, expected, &accepted, "0644").is_err(),
                "T06 duplicate current inverse span accepted",
            )?;
            for field in ["remove_utf8", "insert_utf8"] {
                let mut altered = doc.clone();
                let old =
                    text(&altered["inverse_stages"][stage]["inverse"][index], field)?.to_owned();
                altered["inverse_stages"][stage]["inverse"][index][field] =
                    json!(format!("{old}x"));
                need(
                    validate_current(root, &altered, expected, &accepted, "0644").is_err(),
                    "T06 altered current inverse span accepted",
                )?;
            }
        }
    }
    let mut altered_owner = physical.to_vec();
    let owner_slice = owner(physical)?;
    let at = physical
        .windows(owner_slice.len())
        .position(|part| part == owner_slice)
        .ok_or("T06 owner position")?;
    altered_owner[at + 100] ^= 1;
    need(
        unchanged_current_owner(&altered_owner, proof_prior).is_err(),
        "T06 altered actual default owner accepted",
    )?;
    let mut altered_prior = proof_prior.to_vec();
    let owner_slice = owner(proof_prior)?;
    let at = proof_prior
        .windows(owner_slice.len())
        .position(|part| part == owner_slice)
        .ok_or("T06 prior owner position")?;
    altered_prior[at + 100] ^= 1;
    need(
        unchanged_current_owner(physical, &altered_prior).is_err(),
        "T06 altered prior default owner accepted",
    )?;
    let mut changed_doc = doc.clone();
    changed_doc["scope"] = json!("forged");
    need(
        current_document(&serde_json::to_vec(&changed_doc).map_err(|e| e.to_string())?).is_err(),
        "T06 changed current capsule accepted",
    )?;
    Ok(())
}
