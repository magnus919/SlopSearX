//! Exact native healthcheck image changes projected to the accepted T02 source.
use super::super::{Result, git, need, read, sha};
use serde::Deserialize;
use std::path::Path;

const CAPSULE: &[u8] = include_bytes!("native-health-image-v1.json");
const CAPSULE_PATH: &str = "docs/rust-rewrite/t02/source-amendments/native-health-image-v1.json";
const CAPSULE_SHA: &str = "78e1d99103f3d5eccd0d127fb00e53406ac578d1497ac4d1442f51ec46c469cb";
const BASE: &str = "98aa2e931de1c4c63455ca79a23760fb7581ed38";
pub(super) const FIXTURE: &str = "crates/ssx-http/examples/t02_fixture.rs";
const PATHS: [&str; 3] = [
    "crates/ssx-http/Dockerfile.t02",
    FIXTURE,
    "crates/ssx-http/examples/t02_image_smoke.rs",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    bytes: usize,
    sha256: String,
    blob: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    offset: usize,
    prior: String,
    current: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    path: String,
    prior: Identity,
    current: Identity,
    edits: Vec<Edit>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capsule {
    schema: String,
    base: String,
    rows: Vec<Row>,
}
fn load(root: &Path) -> Result<Capsule> {
    let raw = read(root, CAPSULE_PATH)?;
    need(
        raw.mode == 0o644 && raw.bytes == CAPSULE && sha(CAPSULE) == CAPSULE_SHA,
        "T02 native health image capsule identity",
    )?;
    let capsule: Capsule = serde_json::from_slice(CAPSULE).map_err(|err| err.to_string())?;
    need(
        capsule.schema == "T02_NATIVE_HEALTH_IMAGE_V1"
            && capsule.base == BASE
            && capsule.rows.iter().map(|row| row.path.as_str()).eq(PATHS),
        "T02 native health image capsule scope",
    )?;
    Ok(capsule)
}
fn project(row: &Row, current: &[u8]) -> Result<Vec<u8>> {
    need(
        current.len() == row.current.bytes && sha(current) == row.current.sha256,
        "T02 native health image exact current source",
    )?;
    let mut prior = current.to_vec();
    let mut bound = current.len();
    for edit in row.edits.iter().rev() {
        let end = edit
            .offset
            .checked_add(edit.current.len())
            .ok_or("edit overflow")?;
        need(
            end <= bound
                && prior.get(edit.offset..end) == Some(edit.current.as_bytes())
                && edit.prior != edit.current,
            "T02 native health image finite inverse",
        )?;
        prior.splice(edit.offset..end, edit.prior.bytes());
        bound = edit.offset;
    }
    need(
        prior.len() == row.prior.bytes && sha(&prior) == row.prior.sha256,
        "T02 native health image exact prior source",
    )?;
    Ok(prior)
}
pub(super) fn prior(
    root: &Path,
    path: &str,
    expected: &str,
    expected_blob: Option<&str>,
) -> Result<Vec<u8>> {
    let capsule = load(root)?;
    let row = capsule
        .rows
        .iter()
        .find(|row| row.path == path)
        .ok_or("unlisted T02 health image source")?;
    need(
        row.prior.sha256 == expected && expected_blob.is_none_or(|blob| blob == row.prior.blob),
        "T02 native health image exact predecessor caller",
    )?;
    let current = read(root, path)?;
    need(
        current.mode == 0o644,
        "T02 native health image regular0644 source",
    )?;
    need(
        git(root, &["hash-object", "--", path])? == format!("{}\n", row.current.blob).as_bytes(),
        "T02 native health image current Git blob",
    )?;
    let prior = project(row, &current.bytes)?;
    need(
        git(root, &["show", &format!("{BASE}:{path}")])? == prior
            && git(root, &["ls-tree", BASE, "--", path])?
                == format!("100644 blob {}\t{path}\n", row.prior.blob).as_bytes(),
        "T02 native health image predecessor Git witness",
    )?;
    Ok(prior)
}
pub(super) fn contains(path: &str) -> bool {
    PATHS.contains(&path)
}
pub(super) fn controls(root: &Path) -> Result<()> {
    let capsule = load(root)?;
    for row in capsule.rows {
        prior(root, &row.path, &row.prior.sha256, Some(&row.prior.blob))?;
        need(
            prior(root, &row.path, "wrong", None).is_err(),
            "T02 native health image wrong caller accepted",
        )?;
        let mut mutated = read(root, &row.path)?.bytes;
        mutated[0] ^= 1;
        need(
            project(&row, &mutated).is_err(),
            "T02 native health image mutant accepted",
        )?;
    }
    Ok(())
}
