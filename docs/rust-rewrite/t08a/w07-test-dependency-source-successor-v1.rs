//! Exact outer source projection for the updated W07 test-dependency admission.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
const BASE: &str = "cebf0f8c701d2fde2c8b22ae3da39386e874bcae";
const PATH: &str = "docs/rust-rewrite/w07/source-amendments/test_dependency_successor.rs";
const CAPSULE: &[u8] = include_bytes!("w07-test-dependency-source-successor-v1.json");
const CAPSULE_SHA: &str = "dd824f65bb5809f5481b2a1906830771ef0be0f7f98a1455618157bbd831b1c2";
const CURRENT_BYTES: usize = 13215;
const CURRENT_SHA: &str = "fb8725bff7cbe82719db5076fae73979bf7f81ab4445ae8a5354b98db7e0b664";
const PRIOR_BYTES: usize = 12158;
const PRIOR_SHA: &str = "5dd1fc5bd429e413a9dac90402ec322d5d1cb4894ea3b8d63f958074b76aaf15";
const PRIOR_BLOB: &str = "2fe4c3acf212bcffb6546219e9d102f88c4a4e3a";
const EDITS: usize = 10;
fn need(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("W07 test-dependency outer source: {message}"))
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn keys(value: &Value, expected: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
    })
}
fn decode() -> Result<Value> {
    need(
        digest(CAPSULE) == CAPSULE_SHA,
        "compiled exact capsule digest",
    )?;
    let doc: Value = serde_json::from_slice(CAPSULE).map_err(|error| error.to_string())?;
    need(
        keys(&doc, &["schema", "baseline_commit", "row"])
            && doc["schema"] == "T08A_W07_TEST_DEPENDENCY_SOURCE_SUCCESSOR_V1"
            && doc["baseline_commit"] == BASE,
        "closed capsule header",
    )?;
    let row = &doc["row"];
    need(
        keys(row, &["path", "current", "prior", "edits"])
            && row["path"] == PATH
            && keys(&row["current"], &["bytes", "sha256", "mode"])
            && row["current"]["bytes"] == CURRENT_BYTES
            && row["current"]["sha256"] == CURRENT_SHA
            && row["current"]["mode"] == "0644"
            && keys(&row["prior"], &["bytes", "sha256", "mode", "git_blob"])
            && row["prior"]["bytes"] == PRIOR_BYTES
            && row["prior"]["sha256"] == PRIOR_SHA
            && row["prior"]["mode"] == "0644"
            && row["prior"]["git_blob"] == PRIOR_BLOB
            && row["edits"]
                .as_array()
                .is_some_and(|edits| edits.len() == EDITS),
        "closed exact W07 source row",
    )?;
    Ok(doc)
}
pub(crate) fn owns(path: &str) -> bool {
    path == PATH
}
pub(crate) fn project_current(path: &str, current: &[u8]) -> Result<Vec<u8>> {
    need(owns(path), "unowned path")?;
    let doc = decode()?;
    let row = &doc["row"];
    need(
        current.len() == CURRENT_BYTES && digest(current) == CURRENT_SHA,
        "exact current source bytes/hash",
    )?;
    let edits = row["edits"].as_array().ok_or("source edits missing")?;
    let mut prior = Vec::with_capacity(PRIOR_BYTES);
    let (mut current_cursor, mut prior_cursor) = (0usize, 0usize);
    for edit in edits {
        need(
            keys(edit, &["offset", "remove_utf8", "insert_utf8"]),
            "closed edit shape",
        )?;
        let offset = edit["offset"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("edit offset")?;
        let remove = edit["remove_utf8"]
            .as_str()
            .ok_or("removed source span")?
            .as_bytes();
        let insert = edit["insert_utf8"]
            .as_str()
            .ok_or("inserted source span")?
            .as_bytes();
        let unchanged = offset.checked_sub(prior_cursor).ok_or("edit order")?;
        let end = current_cursor
            .checked_add(unchanged)
            .ok_or("current cursor overflow")?;
        prior.extend_from_slice(
            current
                .get(current_cursor..end)
                .ok_or("unchanged current span")?,
        );
        let inserted_end = end
            .checked_add(insert.len())
            .ok_or("inserted span overflow")?;
        need(
            current.get(end..inserted_end) == Some(insert),
            "exact inserted UTF-8 span",
        )?;
        prior.extend_from_slice(remove);
        current_cursor = inserted_end;
        prior_cursor = offset
            .checked_add(remove.len())
            .ok_or("prior cursor overflow")?;
    }
    prior.extend_from_slice(current.get(current_cursor..).ok_or("current suffix")?);
    need(
        prior.len() == PRIOR_BYTES && digest(&prior) == PRIOR_SHA,
        "exact historical source bytes/hash",
    )?;
    let mut forward = Vec::with_capacity(CURRENT_BYTES);
    let mut prior_cursor = 0usize;
    for edit in edits {
        let offset = edit["offset"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("forward offset")?;
        let remove = edit["remove_utf8"]
            .as_str()
            .ok_or("forward removed span")?
            .as_bytes();
        let insert = edit["insert_utf8"]
            .as_str()
            .ok_or("forward inserted span")?
            .as_bytes();
        let end = offset
            .checked_add(remove.len())
            .ok_or("forward span overflow")?;
        need(
            offset >= prior_cursor && prior.get(offset..end) == Some(remove),
            "exact prior span",
        )?;
        forward.extend_from_slice(&prior[prior_cursor..offset]);
        forward.extend_from_slice(insert);
        prior_cursor = end;
    }
    forward.extend_from_slice(prior.get(prior_cursor..).ok_or("prior suffix")?);
    need(forward == current, "complete forward reconstruction")?;
    Ok(prior)
}
fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    need(output.status.success(), "Git identity command")?;
    Ok(output.stdout)
}
#[cfg(any(test, windows))]
fn windows_stage_zero_index(index: &str, path: &str) -> bool {
    let mut lines = index.lines();
    lines
        .next()
        .is_some_and(|line| line.starts_with("100644 ") && line.ends_with(&format!(" 0\t{path}")))
        && lines.next().is_none()
}
pub(crate) fn prior(root: &Path, path: &str, current: &[u8], opened_mode: u32) -> Result<Vec<u8>> {
    need(owns(path), "unowned prior path")?;
    #[cfg(unix)]
    need(opened_mode == 0o644, "opened source mode0644")?;
    #[cfg(windows)]
    {
        need(opened_mode == 0, "Windows handle mode")?;
        let index = git(root, &["ls-files", "--stage", "--", path])?;
        let index = std::str::from_utf8(&index).map_err(|error| error.to_string())?;
        need(
            windows_stage_zero_index(index, path),
            "Windows stage-zero index mode",
        )?;
    }
    #[cfg(not(any(unix, windows)))]
    need(false, "source mode proof requires Unix or Windows")?;
    let prior = project_current(path, current)?;
    let spec = format!("{BASE}:{path}");
    need(
        git(root, &["show", &spec])? == prior,
        "exact baseline Git bytes",
    )?;
    let tree = git(root, &["ls-tree", BASE, "--", path])?;
    need(
        tree == format!("100644 blob {PRIOR_BLOB}\t{path}\n").as_bytes(),
        "exact baseline Git tree/blob/mode",
    )?;
    Ok(prior)
}
pub(crate) fn controls(root: &Path) -> Result<()> {
    #[cfg(test)]
    {
        let entry = format!("100644 {PRIOR_BLOB} 0\t{PATH}\n");
        need(
            windows_stage_zero_index(&entry, PATH),
            "stage-zero record with Git newline",
        )?;
        need(
            !windows_stage_zero_index(&entry.repeat(2), PATH),
            "duplicate index record rejected",
        )?;
        need(
            !windows_stage_zero_index(&entry.replace(" 0\t", " 1\t"), PATH),
            "nonzero stage rejected",
        )?;
    }
    let (current, opened_mode) = super::regular_material(root, PATH)?;
    let projected_prior = prior(root, PATH, &current, opened_mode)?;
    need(
        projected_prior.len() == PRIOR_BYTES && digest(&projected_prior) == PRIOR_SHA,
        "positive source projection",
    )?;
    need(
        prior(root, "unowned.rs", &current, opened_mode).is_err(),
        "unowned path rejected",
    )?;
    need(
        prior(root, PATH, &projected_prior, opened_mode).is_err(),
        "historical input rejected as current",
    )?;
    let mut changed = current.clone();
    changed.push(b' ');
    need(
        prior(root, PATH, &changed, opened_mode).is_err(),
        "mutated current rejected",
    )?;
    #[cfg(unix)]
    need(
        prior(root, PATH, &current, 0o600).is_err(),
        "changed physical mode rejected",
    )?;
    #[cfg(windows)]
    need(
        prior(root, PATH, &current, 0o644).is_err(),
        "fabricated Windows mode rejected",
    )?;
    Ok(())
}
