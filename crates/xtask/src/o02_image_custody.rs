//! Digest-slot successor of the exact builder-patch-v2 deployment oracle.
//! Only the 64 lowercase hex characters at byte 3032 may vary. The normalized
//! full-source hash authenticates every other byte; no source is executed.
use crate::Result;
use sha2::{Digest, Sha256};
pub(super) const SOURCE_BYTES: usize = 13732;
pub(super) const CURRENT_BYTES: usize = 13810;
const CURRENT_SHA: &str = "ecdf9ce063dc0086287e7d9bc22dfc85e5b4f1d523c6914b799191041d71c8dd";
const PRIOR_ASSERTION: &str =
    "    assert \"scripts/promote_image_digest.py --write\" in step_text\n";
const CURRENT_ASSERTION: &str = "    assert \"cargo xtask ci-helper image-pin-promote --write\" in step_text\n    assert \"scripts/promote_image_digest.py --write\" not in step_text\n";
const SLOT: usize = 3032;
const SOURCE_SHA: &str = "e9d53620f211325296cea8f3723c11a013621a8eb7fc642e077f06bf235035b8";
const ORIGINAL: &[u8] = b"91194d146d205b1cf4688c1989da8f5f6b599a9627be23fd1ee7a4e488fda5b7";

pub(crate) fn normalize(raw: &[u8]) -> Result<Vec<u8>> {
    if raw.len() != CURRENT_BYTES {
        return Err("O02 digest custody: exact deployment source size".into());
    }
    let slot = &raw[SLOT..SLOT + 64];
    if !slot
        .iter()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
    {
        return Err("O02 digest custody: slot requires 64 lowercase hex characters".into());
    }
    let mut normalized = raw.to_vec();
    normalized[SLOT..SLOT + 64].copy_from_slice(ORIGINAL);
    if Sha256::digest(&normalized)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        != CURRENT_SHA
    {
        return Err("O02 digest custody: non-digest source bytes changed".into());
    }
    let current = std::str::from_utf8(&normalized).map_err(|e| e.to_string())?;
    if current.matches(CURRENT_ASSERTION).count() != 1 {
        return Err("O02 digest custody: exact native workflow assertion span".into());
    }
    let prior = current
        .replacen(CURRENT_ASSERTION, PRIOR_ASSERTION, 1)
        .into_bytes();
    let sha = Sha256::digest(&prior)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    if prior.len() != SOURCE_BYTES || sha != SOURCE_SHA {
        return Err("O02 digest custody: exact prior assertion reconstruction".into());
    }
    Ok(prior)
}

pub(crate) fn current_digest(raw: &[u8]) -> Result<String> {
    normalize(raw)?;
    Ok(format!(
        "sha256:{}",
        std::str::from_utf8(&raw[SLOT..SLOT + 64]).map_err(|e| e.to_string())?
    ))
}

pub(crate) fn replace(raw: &[u8], digest: &str) -> Result<Vec<u8>> {
    normalize(raw)?;
    let hex = digest
        .strip_prefix("sha256:")
        .ok_or("O02 digest custody: SHA-256 required")?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("O02 digest custody: invalid target digest".into());
    }
    let mut updated = raw.to_vec();
    updated[SLOT..SLOT + 64].copy_from_slice(hex.as_bytes());
    normalize(&updated)?;
    Ok(updated)
}
