//! Digest-slot successor of the exact builder-patch-v2 deployment oracle.
//! Only the 64 lowercase hex characters at byte 3032 may vary. The normalized
//! full-source hash authenticates every other byte; no source is executed.
use crate::Result;
use sha2::{Digest, Sha256};
pub(super) const SOURCE_BYTES: usize = 13732;
pub(super) const CURRENT_BYTES: usize = 14124;
const CURRENT_SHA: &str = "658c9cfe717e63f962793b0fcfe97a7c855460d23044118912201c28ea06a092";
const PROMOTION_BYTES: usize = 13810;
const PROMOTION_SHA: &str = "ecdf9ce063dc0086287e7d9bc22dfc85e5b4f1d523c6914b799191041d71c8dd";
const PRIOR_PUBLICATION_ASSERTIONS: &str = "    assert '\"$(cat digests/amd64)\" \"$(cat digests/arm64)\"' in command\n    assert 'assert platforms == {(\"linux\", \"amd64\"), (\"linux\", \"arm64\")}' in command\n";
const CURRENT_PUBLICATION_ASSERTIONS: &str = "    assert \"amd64=$(cat digests/amd64)\" in command\n    assert \"arm64=$(cat digests/arm64)\" in command\n    assert 'docker buildx imagetools create \"${tags[@]}\" \"$amd64\" \"$arm64\"' in command\n    verifier = 'target/debug/xtask ci-helper manifest manifest.json \"$amd64_digest\" \"$arm64_digest\"'\n    assert command.count(verifier) == 2\n    assert command.index(\"--dry-run\") < command.index(verifier) < command.index('create \"${tags[@]}\"')\n    assert \"python3\" not in command\n";
const PRIOR_ASSERTION: &str =
    "    assert \"scripts/promote_image_digest.py --write\" in step_text\n";
const CURRENT_ASSERTION: &str = "    assert \"cargo xtask ci-helper image-pin-promote --write\" in step_text\n    assert \"scripts/promote_image_digest.py --write\" not in step_text\n";
const SLOT: usize = 3032;
const SOURCE_SHA: &str = "e9d53620f211325296cea8f3723c11a013621a8eb7fc642e077f06bf235035b8";
const ORIGINAL: &[u8] = b"91194d146d205b1cf4688c1989da8f5f6b599a9627be23fd1ee7a4e488fda5b7";

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

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
    if digest(&normalized) != CURRENT_SHA {
        return Err("O02 digest custody: non-digest source bytes changed".into());
    }
    let current = std::str::from_utf8(&normalized).map_err(|e| e.to_string())?;
    if current.matches(CURRENT_PUBLICATION_ASSERTIONS).count() != 1 {
        return Err("O02 digest custody: exact native publication assertion span".into());
    }
    let current = current.replacen(
        CURRENT_PUBLICATION_ASSERTIONS,
        PRIOR_PUBLICATION_ASSERTIONS,
        1,
    );
    if current.len() != PROMOTION_BYTES || digest(current.as_bytes()) != PROMOTION_SHA {
        return Err("O02 digest custody: exact promotion assertion reconstruction".into());
    }
    if current.matches(CURRENT_ASSERTION).count() != 1 {
        return Err("O02 digest custody: exact native workflow assertion span".into());
    }
    let prior = current
        .replacen(CURRENT_ASSERTION, PRIOR_ASSERTION, 1)
        .into_bytes();
    if prior.len() != SOURCE_BYTES || digest(&prior) != SOURCE_SHA {
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
