//! Exact T08 dependency and source successor edges; historical endpoints remain fixed.
#[allow(
    clippy::duplicate_mod,
    reason = "Independent proof roots share this exact inverse"
)]
#[path = "../a11/replay-scheduling-source.rs"]
mod a11_replay_source;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
const BASE: &str = "7d904b9d46f4c4ba8c59dc1ba09828f9e7ea204d";
const CAPSULE: &[u8] = include_bytes!("issue903-config-edge-successor-v1.json");
const CAPSULE_SHA: &str = "3199e5ec4c56ef21659446cd244198240e8d932ec67c180dcdb70f40af402ca2";
const MAX_PHYSICAL_BYTES: u64 = 1_048_576;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capsule {
    schema: String,
    baseline_commit: String,
    rows: Vec<Row>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    path: String,
    prior: Prior,
    current: Current,
    edits: Vec<Edit>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prior {
    bytes: usize,
    sha256: String,
    mode: String,
    git_blob: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Current {
    bytes: usize,
    sha256: String,
    mode: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    offset: usize,
    remove_utf8: String,
    insert_utf8: String,
}
fn need(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("T08 issue903 config successor: {reason}"))
    }
}
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn capsule() -> Result<Capsule> {
    need(sha(CAPSULE) == CAPSULE_SHA, "compiled capsule digest")?;
    let capsule: Capsule = serde_json::from_slice(CAPSULE).map_err(|e| e.to_string())?;
    need(
        capsule.schema == "ISSUE903_CONFIG_EDGE_SUCCESSOR_V1"
            && capsule.baseline_commit == BASE
            && capsule.rows.len() == 10
            && capsule.rows[0].path == "Cargo.lock"
            && capsule.rows[1].path == "crates/ssx-cli-check/Cargo.toml"
            && capsule.rows[2].path == "crates/ssx-mcp/Cargo.toml"
            && capsule.rows[3].path
                == "docs/rust-rewrite/a06/source-amendments/native-search-reader.rs"
            && capsule.rows[4].path == "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs"
            && capsule.rows[5].path == "docs/rust-rewrite/t03/source-successor-v1.rs"
            && capsule.rows[6].path == "crates/ssx-infra/Cargo.toml"
            && capsule.rows[7].path
                == "docs/rust-rewrite/a06/source-amendments/shared-successor.rs"
            && capsule.rows[8].path == "crates/xtask/src/w07.rs"
            && capsule.rows[9].path == "crates/xtask/Cargo.toml",
        "closed ten-path capsule identity/order",
    )?;
    for row in &capsule.rows {
        need(
            row.prior.mode == "0644" && row.current.mode == "0644" && !row.edits.is_empty(),
            "closed prior/current mode, size, and change spans",
        )?;
    }
    Ok(capsule)
}
#[allow(
    dead_code,
    reason = "W07/O02 use this authority; independent source gates also include this module"
)]
pub(crate) fn current_lock_identity() -> Result<(usize, String)> {
    let capsule = capsule()?;
    let row = capsule
        .rows
        .into_iter()
        .next()
        .ok_or("missing Cargo.lock row")?;
    Ok((row.current.bytes, row.current.sha256))
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    need(output.status.success(), "prior Git identity command")?;
    Ok(output.stdout)
}
#[allow(
    dead_code,
    reason = "Used by source-composition gates; the standalone checker also compiles this shared module"
)]
pub(crate) fn owns(path: &str) -> bool {
    matches!(
        path,
        "docs/rust-rewrite/a06/source-amendments/native-search-reader.rs"
            | "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs"
            | "docs/rust-rewrite/t03/source-successor-v1.rs"
            | "docs/rust-rewrite/a06/source-amendments/shared-successor.rs"
            | "crates/xtask/src/w07.rs"
    )
}
fn project_row(row: &Row, current: &[u8]) -> Result<Vec<u8>> {
    let preceding = matches!(
        row.path.as_str(),
        a11_replay_source::NATIVE_READER | a11_replay_source::SHARED_READER
    )
    .then(|| a11_replay_source::prior(&row.path, current))
    .transpose()?;
    let current = preceding.as_deref().unwrap_or(current);
    need(
        current.len() == row.current.bytes,
        &format!("opened current byte length for {}", row.path),
    )?;
    need(
        sha(current) == row.current.sha256,
        &format!("opened current digest for {}", row.path),
    )?;
    let mut prior = Vec::new();
    let mut current_cursor = 0usize;
    let mut prior_cursor = 0usize;
    for edit in &row.edits {
        let remove = edit.remove_utf8.as_bytes();
        let insert = edit.insert_utf8.as_bytes();
        let unchanged = edit
            .offset
            .checked_sub(prior_cursor)
            .ok_or("successor edit order")?;
        let current_end = current_cursor
            .checked_add(unchanged)
            .ok_or("successor current cursor overflow")?;
        prior.extend_from_slice(
            current
                .get(current_cursor..current_end)
                .ok_or("successor unchanged current span")?,
        );
        let inserted_end = current_end
            .checked_add(insert.len())
            .ok_or("successor inserted span overflow")?;
        need(
            current.get(current_end..inserted_end) == Some(insert),
            "exact dependency/source insertion span",
        )?;
        prior.extend_from_slice(remove);
        current_cursor = inserted_end;
        prior_cursor = edit
            .offset
            .checked_add(remove.len())
            .ok_or("successor prior cursor overflow")?;
    }
    prior.extend_from_slice(
        current
            .get(current_cursor..)
            .ok_or("successor current suffix")?,
    );
    need(
        prior.len() == row.prior.bytes && sha(&prior) == row.prior.sha256,
        "exact projected prior bytes",
    )?;
    let mut forward = Vec::new();
    let mut prior_cursor = 0usize;
    let mut current_cursor = 0usize;
    for edit in &row.edits {
        let remove = edit.remove_utf8.as_bytes();
        let insert = edit.insert_utf8.as_bytes();
        let unchanged = edit
            .offset
            .checked_sub(prior_cursor)
            .ok_or("successor forward edit order")?;
        let prior_end = prior_cursor
            .checked_add(unchanged)
            .ok_or("successor prior span overflow")?;
        forward.extend_from_slice(
            prior
                .get(prior_cursor..prior_end)
                .ok_or("successor prior unchanged span")?,
        );
        let removed_end = prior_end
            .checked_add(remove.len())
            .ok_or("successor removed span overflow")?;
        need(
            prior.get(prior_end..removed_end) == Some(remove),
            "exact predecessor source span",
        )?;
        forward.extend_from_slice(insert);
        current_cursor = current_cursor
            .checked_add(unchanged)
            .and_then(|cursor| cursor.checked_add(insert.len()))
            .ok_or("successor current cursor overflow")?;
        prior_cursor = removed_end;
    }
    forward.extend_from_slice(prior.get(prior_cursor..).ok_or("successor prior suffix")?);
    need(
        current_cursor <= current.len() && forward == current,
        "full forward reconstruction",
    )?;
    Ok(prior)
}
/// Pure byte projection for source-frame controls; physical callers use `prior`.
#[allow(
    dead_code,
    reason = "Used by source-composition gates; the standalone checker also compiles this shared module"
)]
pub(crate) fn project_current(path: &str, current: &[u8]) -> Result<Vec<u8>> {
    let capsule = capsule()?;
    let row = capsule
        .rows
        .iter()
        .find(|row| row.path == path)
        .ok_or("unowned issue903 config path")?;
    project_row(row, current)
}

/// Reverse exact current source spans after the caller's bounded read.
pub(crate) fn prior(root: &Path, path: &str, current: &[u8], mode: u32) -> Result<Vec<u8>> {
    let capsule = capsule()?;
    let row = capsule
        .rows
        .iter()
        .find(|row| row.path == path)
        .ok_or("unowned issue903 config path")?;
    #[cfg(unix)]
    need(mode == 0o644, "opened Unix current mode")?;
    #[cfg(windows)]
    {
        need(mode == 0, "Windows handle has no Unix mode bits")?;
        let index = git(root, &["ls-files", "--stage", "--", path])?;
        let index = std::str::from_utf8(&index).map_err(|error| error.to_string())?;
        let entries: Vec<_> = index.lines().collect();
        need(
            entries.len() == 1
                && entries[0].starts_with("100644 ")
                && entries[0].ends_with(&format!(" 0\t{path}")),
            "Windows stage-zero index mode",
        )?;
    }
    #[cfg(not(any(unix, windows)))]
    need(false, "opened source mode proof requires Unix or Windows")?;
    let projected_prior = project_row(row, current)?;
    let git_prior = git(root, &["show", &format!("{BASE}:{path}")])?;
    let tree = git(root, &["ls-tree", BASE, "--", path])?;
    need(
        git_prior == projected_prior
            && tree == format!("100644 blob {}\t{path}\n", row.prior.git_blob).as_bytes(),
        "exact baseline Git bytes/mode/blob",
    )?;
    Ok(projected_prior)
}
/// Positive and adversarial controls over the finite config successor.
pub(crate) fn controls(root: &Path) -> Result<()> {
    let capsule = capsule()?;
    for row in &capsule.rows {
        let (current, mode) =
            super::bounded_source_reader::bounded_read(root, &row.path, MAX_PHYSICAL_BYTES)?;
        let projected_prior = prior(root, &row.path, &current, mode)?;
        need(
            projected_prior.len() == row.prior.bytes && sha(&projected_prior) == row.prior.sha256,
            "positive projection control",
        )?;
        need(
            prior(root, &row.path, &current, 0o600).is_err(),
            "wrong opened mode rejected",
        )?;
        let mut altered = current.clone();
        altered[0] ^= 1;
        need(
            prior(root, &row.path, &altered, mode).is_err(),
            "altered current bytes rejected",
        )?;
        need(
            prior(root, &format!("{} ", row.path), &current, mode).is_err(),
            "unowned path rejected",
        )?;
    }
    need(
        capsule.rows[0].edits.len() == 3
            && capsule.rows[0].edits[0].offset == 67341
            && capsule.rows[0].edits[0].remove_utf8.is_empty()
            && capsule.rows[0].edits[0].insert_utf8 == " \"serde\",\n"
            && capsule.rows[0].edits[1].offset == 68937
            && capsule.rows[0].edits[1].remove_utf8.is_empty()
            && capsule.rows[0].edits[1].insert_utf8 == " \"nix\",\n"
            && capsule.rows[0].edits[2].offset == 69483
            && capsule.rows[0].edits[2].remove_utf8.is_empty()
            && capsule.rows[0].edits[2].insert_utf8 == " \"nix\",\n"
            && capsule.rows[1].edits.len() == 2
            && capsule.rows[1].edits[0].offset == 205
            && capsule.rows[1].edits[0].remove_utf8.is_empty()
            && capsule.rows[1].edits[0].insert_utf8 == "serde.workspace = true\n"
            && capsule.rows[1].edits[1].offset == 616
            && capsule.rows[1].edits[1].remove_utf8 == "features = [\"signal\"]\n"
            && capsule.rows[1].edits[1].insert_utf8 == "features = [\"signal\", \"fs\"]\n"
            && capsule.rows[2].edits.len() == 1
            && capsule.rows[2].edits[0].remove_utf8.is_empty()
            && capsule.rows[2].edits[0].insert_utf8
                == "\n[target.'cfg(unix)'.dev-dependencies.nix]\nversion = \"=0.31.3\"\nfeatures = [\"fs\"]\n"
            && !capsule.rows[3].edits.is_empty()
            && capsule.rows[4].edits.len() >= 2
            && !capsule.rows[5].edits.is_empty()
            && capsule.rows[6].edits.len() == 1
            && capsule.rows[6].edits[0].offset == capsule.rows[6].prior.bytes
            && capsule.rows[6].edits[0].insert_utf8
                == "\n[target.'cfg(unix)'.dev-dependencies.nix]\nversion = \"=0.31.3\"\nfeatures = [\"fs\"]\n"
            && !capsule.rows[7].edits.is_empty()
            && !capsule.rows[8].edits.is_empty()
            && capsule.rows[9].edits.len() == 1
            && capsule.rows[9].edits[0].remove_utf8 == "features = [\"signal\"]\n"
            && capsule.rows[9].edits[0].insert_utf8 == "features = [\"signal\", \"fs\"]\n",
        "finite dependency and source successor spans",
    )?;
    Ok(())
}
