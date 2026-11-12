//! One reviewed Rust builder patch projected into the unchanged O02 operands.
//! Mutable manifests never supply expected hashes; the outer identity is compiled.
use serde_json::Value;
use sha2::{Digest, Sha256};
type Result<T> = std::result::Result<T, String>;
pub(super) const CAPSULE: &str = "docs/rust-rewrite/o02/builder-patch-v1/capsule.json";
pub(super) const CAPSULE_SHA: &str =
    "dfec3598f0142f7d6d4839d2779e42a4923ab0e40b5f6c817e82670f7937011b";
pub(super) const CAPSULE_RAW: &[u8] =
    include_bytes!("../../../docs/rust-rewrite/o02/builder-patch-v1/capsule.json");
const ROWS: [(&str, usize); 3] = [
    ("Dockerfile", 1),
    ("scripts/verify_docker_base_digest.py", 1),
    ("tests/test_deployment_artifacts.py", 3),
];
fn need(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("O02 builder patch: {reason}"))
    }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn capsule(raw: &[u8]) -> Result<Value> {
    need(digest(raw) == CAPSULE_SHA, "compiled capsule identity")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "O02_BUILDER_PATCH_1_99_1_V1"
            && doc["parent"] == "011dbfd7eedc47cf60d7d27569d90cee1587da21"
            && doc.as_object().is_some_and(|r| r.len() == 3)
            && doc["rows"]
                .as_array()
                .is_some_and(|r| r.len() == ROWS.len()),
        "closed capsule scope",
    )?;
    for (i, (path, count)) in ROWS.iter().enumerate() {
        let row = &doc["rows"][i];
        need(
            row["path"] == *path
                && row.as_object().is_some_and(|r| r.len() == 4)
                && row["edits"].as_array().is_some_and(|r| r.len() == *count)
                && row["prior"]["mode"] == "0644"
                && row["current"]["mode"] == "0644",
            "ordered paths, modes and edit counts",
        )?;
    }
    Ok(doc)
}
fn size(row: &Value) -> Result<usize> {
    row["bytes"]
        .as_u64()
        .and_then(|n| n.try_into().ok())
        .ok_or("O02 builder patch: size".into())
}
fn sha(row: &Value) -> Result<&str> {
    row["sha256"]
        .as_str()
        .ok_or("O02 builder patch: sha256".into())
}
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str> {
    row[key]
        .as_str()
        .ok_or_else(|| format!("O02 builder patch: {key}"))
}
fn endpoint(row: &Value, raw: &[u8]) -> Result<()> {
    need(
        raw.len() == size(row)? && digest(raw) == sha(row)?,
        "exact endpoint bytes",
    )
}
pub(super) struct Projection(Value);
/// Select only the original expected tuple. No caller can declare new authority.
pub(super) fn projection(
    path: &str,
    bytes: usize,
    expected_sha: &str,
) -> Result<Option<Projection>> {
    let Some(index) = ROWS.iter().position(|(owned, _)| *owned == path) else {
        return Ok(None);
    };
    let doc = capsule(CAPSULE_RAW)?;
    let row = &doc["rows"][index];
    need(
        size(&row["prior"])? == bytes && sha(&row["prior"])? == expected_sha,
        "original caller operand",
    )?;
    Ok(Some(Projection(row.clone())))
}
impl Projection {
    pub(super) fn current(&self) -> Result<(usize, &str)> {
        Ok((size(&self.0["current"])?, sha(&self.0["current"])?))
    }
    pub(super) fn prior(&self, raw: &[u8]) -> Result<Vec<u8>> {
        endpoint(&self.0["current"], raw)?;
        let edits = self.0["edits"]
            .as_array()
            .ok_or("O02 builder patch: edits")?;
        let (mut old_cursor, mut cursor) = (0usize, 0usize);
        let mut prior = Vec::new();
        for edit in edits {
            let at = edit["at"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or("edit offset")?;
            let remove = text(edit, "remove_utf8")?.as_bytes();
            let insert = text(edit, "insert_utf8")?.as_bytes();
            need(remove != insert, "no no-op edits")?;
            let unchanged = at
                .checked_sub(old_cursor)
                .ok_or("overlapping prior spans")?;
            let end = cursor.checked_add(unchanged).ok_or("prefix overflow")?;
            prior.extend_from_slice(raw.get(cursor..end).ok_or("current prefix")?);
            let next = end.checked_add(insert.len()).ok_or("insert overflow")?;
            need(raw.get(end..next) == Some(insert), "exact inserted span")?;
            prior.extend_from_slice(remove);
            old_cursor = at.checked_add(remove.len()).ok_or("prior overflow")?;
            cursor = next;
        }
        prior.extend_from_slice(raw.get(cursor..).ok_or("current suffix")?);
        endpoint(&self.0["prior"], &prior)?;
        let mut forward = Vec::new();
        let mut cursor = 0;
        for edit in edits {
            let at = edit["at"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or("edit offset")?;
            let remove = text(edit, "remove_utf8")?.as_bytes();
            let end = at.checked_add(remove.len()).ok_or("forward overflow")?;
            need(
                at >= cursor && prior.get(at..end) == Some(remove),
                "ordered forward span",
            )?;
            forward.extend_from_slice(&prior[cursor..at]);
            forward.extend_from_slice(text(edit, "insert_utf8")?.as_bytes());
            cursor = end;
        }
        forward.extend_from_slice(&prior[cursor..]);
        need(forward == raw, "complete forward reconstruction")?;
        Ok(prior)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};
    #[test]
    fn actual_three_sources_reconstruct_original_operands() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let doc = capsule(CAPSULE_RAW).unwrap();
        for row in doc["rows"].as_array().unwrap() {
            let path = row["path"].as_str().unwrap();
            let projection = projection(
                path,
                size(&row["prior"]).unwrap(),
                sha(&row["prior"]).unwrap(),
            )
            .unwrap()
            .unwrap();
            let current = fs::read(root.join(path)).unwrap();
            let prior = projection.prior(&current).unwrap();
            endpoint(&row["prior"], &prior).unwrap();
            assert!(projection.prior(&prior).is_err());
            for mutant in [vec![], [current.clone(), current.clone()].concat(), {
                let mut changed = current.clone();
                changed[0] ^= 1;
                changed
            }] {
                assert!(projection.prior(&mutant).is_err(), "{path}");
            }
            assert!(super::projection(path, current.len(), &digest(&current)).is_err());
        }
        assert!(projection("unowned", 0, "wrong").unwrap().is_none());
    }
    #[test]
    fn mutable_capsule_cannot_rehash_or_change_scope() {
        let good = capsule(CAPSULE_RAW).unwrap();
        for kind in 0..7 {
            let mut forged = good.clone();
            let rows = forged["rows"].as_array_mut().unwrap();
            match kind {
                0 => {
                    rows.pop();
                }
                1 => rows.push(rows[0].clone()),
                2 => rows.swap(0, 1),
                3 => rows[0]["current"]["sha256"] = digest(b"forged").into(),
                4 => {
                    rows[0]["edits"].as_array_mut().unwrap().pop();
                }
                5 => {
                    let edit = rows[0]["edits"][0].clone();
                    rows[0]["edits"].as_array_mut().unwrap().push(edit);
                }
                _ => rows[0]["edits"][0]["insert_utf8"] = "FROM scratch\n".into(),
            }
            let raw = serde_json::to_vec(&forged).unwrap();
            assert!(capsule(&raw).is_err());
            assert_ne!(digest(&raw), CAPSULE_SHA);
        }
    }
}
