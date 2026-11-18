//! Reversible outer source transitions for the optional-Chrono work.
use serde_json::Value;
use sha2::{Digest, Sha256};

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn prior(
    record: &str,
    record_sha: &str,
    path: &str,
    physical: &[u8],
    mode: u32,
) -> Result<Vec<u8>, String> {
    if sha(record.as_bytes()) != record_sha {
        return Err("RMCP source transition record identity differs".into());
    }
    let row: Value = serde_json::from_str(record).map_err(|err| err.to_string())?;
    prior_row(&row, path, physical, mode)
}

fn prior_row(row: &Value, path: &str, physical: &[u8], mode: u32) -> Result<Vec<u8>, String> {
    if row["path"] != path
        || row["current_bytes"] != physical.len()
        || row["current_sha256"] != sha(physical)
        || mode != 0o644
    {
        return Err("RMCP physical source identity differs".into());
    }
    let spans = row["spans"].as_array().ok_or("RMCP source spans missing")?;
    let mut predecessor = Vec::new();
    let mut cursor = 0;
    for span in spans {
        let at = span["at"]
            .as_u64()
            .and_then(|at| usize::try_from(at).ok())
            .ok_or("RMCP source offset missing")?;
        let current = span["current"]
            .as_str()
            .ok_or("RMCP current span missing")?;
        let older = span["prior"].as_str().ok_or("RMCP prior span missing")?;
        let end = at
            .checked_add(current.len())
            .ok_or("RMCP source span overflow")?;
        if physical.get(at..end) != Some(current.as_bytes()) {
            return Err("RMCP physical source span differs".into());
        }
        predecessor.extend_from_slice(
            physical
                .get(cursor..at)
                .ok_or("RMCP source spans overlap")?,
        );
        predecessor.extend_from_slice(older.as_bytes());
        cursor = end;
    }
    predecessor.extend_from_slice(&physical[cursor..]);
    if row["prior_bytes"] != predecessor.len() || row["prior_sha256"] != sha(&predecessor) {
        return Err("RMCP historical source identity differs".into());
    }
    Ok(predecessor)
}

/// Select only the finite post-Chrono runtime sources in the authenticated catalog.
#[allow(
    dead_code,
    reason = "Other source roots share only the single-record inverse"
)]
pub(crate) fn runtime_prior(
    record: &str,
    record_sha: &str,
    path: &str,
    physical: &[u8],
    mode: u32,
) -> Result<Option<Vec<u8>>, String> {
    if sha(record.as_bytes()) != record_sha {
        return Err("RMCP runtime source catalog identity differs".into());
    }
    let rows: Vec<Value> = serde_json::from_str(record).map_err(|err| err.to_string())?;
    let Some(row) = rows.iter().find(|row| row["path"] == path) else {
        return Ok(None);
    };
    prior_row(row, path, physical, mode).map(Some)
}
