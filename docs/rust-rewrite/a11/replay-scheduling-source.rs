//! Recover the exact predecessor frames for the A11 replay scheduling repair.
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub(crate) const REPLAY: &str = "crates/ssx-infra/tests/a11_replay.rs";
pub(crate) const NATIVE_READER: &str =
    "docs/rust-rewrite/a06/source-amendments/native-search-reader.rs";
pub(crate) const SHARED_READER: &str =
    "docs/rust-rewrite/a06/source-amendments/shared-successor.rs";
pub(crate) const THREAT_INTEL: &str = "crates/ssx-infra/src/adapters/threat_intel.rs";
pub(crate) const CLOCK_SOURCES: [&str; 4] = [
    "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    "crates/ssx-infra/src/http.rs",
    "crates/ssx-infra/src/http/monotonic.rs",
    THREAT_INTEL,
];
const CAPSULE: &[u8] = include_bytes!("replay-scheduling-source-v1.json");
const CAPSULE_SHA256: &str = "00a39d101d6c05a5152db0163186bea548c4c1a9f1f81c8df902bf9acf12cf01";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    path: String,
    current_bytes: usize,
    current_sha256: String,
    current_blob: String,
    prior_bytes: usize,
    prior_sha256: String,
    inverse: Vec<Edit>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    offset: usize,
    remove: String,
    restore: String,
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn row(path: &str) -> Result<Row, String> {
    if hash(CAPSULE) != CAPSULE_SHA256 {
        return Err("A11 replay scheduling capsule digest".into());
    }
    let rows: [Row; 7] = serde_json::from_slice(CAPSULE).map_err(|error| error.to_string())?;
    let expected = [REPLAY, NATIVE_READER, SHARED_READER]
        .into_iter()
        .chain(CLOCK_SOURCES);
    if !rows.iter().map(|row| row.path.as_str()).eq(expected) {
        return Err("A11 replay scheduling capsule paths".into());
    }
    rows.into_iter()
        .find(|row| row.path == path)
        .ok_or_else(|| "unowned A11 replay scheduling path".into())
}

#[allow(dead_code, reason = "used only by the Git identity consumer")]
pub(crate) fn current_blob(path: &str) -> Result<String, String> {
    Ok(row(path)?.current_blob)
}

/// The caller supplies bounded, mode-checked physical bytes. Only the sealed
/// current source can enter the unchanged historical verification chains.
pub(crate) fn prior(path: &str, current: &[u8]) -> Result<Vec<u8>, String> {
    let row = row(path)?;
    if current.len() != row.current_bytes || hash(current) != row.current_sha256 {
        return Err(format!(
            "A11 replay scheduling current source identity: {path}"
        ));
    }
    let mut prior = Vec::with_capacity(row.prior_bytes);
    let mut cursor = 0;
    for edit in &row.inverse {
        let end = edit
            .offset
            .checked_add(edit.remove.len())
            .ok_or("A11 replay edit overflow")?;
        let prefix = current
            .get(cursor..edit.offset)
            .ok_or("A11 replay edit overlap")?;
        if current.get(edit.offset..end) != Some(edit.remove.as_bytes()) {
            return Err("A11 replay exact current span".into());
        }
        prior.extend_from_slice(prefix);
        prior.extend_from_slice(edit.restore.as_bytes());
        cursor = end;
    }
    prior.extend_from_slice(&current[cursor..]);
    if prior.len() != row.prior_bytes || hash(&prior) != row.prior_sha256 {
        return Err(format!(
            "A11 replay scheduling predecessor identity: {path}"
        ));
    }
    Ok(prior)
}
