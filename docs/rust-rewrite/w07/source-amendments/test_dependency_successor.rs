//! One direct test-only AWS-LC edge over the accepted D Cargo source pair.
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
type Result<T> = std::result::Result<T, String>;
pub const LOCK_PRIOR_SHA: &str = "92ee06e2da26d90c014ffabe812d52af88b084b0edfc4b362c9c33716e0ecd7d";
pub const LOCK_CURRENT_SHA: &str =
    "9b2ab4a8dfb4a6c41c0af3575a6f77c0bbaf139f7efe83f40e60b9380974fd58";
const MANIFEST_PRIOR_SHA: &str = "40f3d08fff5e359fab748b9d3979322bad29452b2c61512c6b07f58466db7dfc";
const MANIFEST_CURRENT_SHA: &str =
    "9a077866f6c72e8513d727c4f81928777ee985ae5742ace152bc61875775727b";
const LOCK_AT: usize = 62615;
const LOCK_EDGE: &[u8] = b" \"aws-lc-rs\",\n";
const MANIFEST_AT: usize = 1129;
const DEV_LINE: &[u8] = b"aws-lc-rs.workspace = true\n";
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn need(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn inverse(
    bytes: &[u8],
    prior: &str,
    current: &str,
    at: usize,
    insertion: &[u8],
) -> Result<Vec<u8>> {
    if sha(bytes) == prior {
        return Ok(bytes.to_vec());
    }
    need(
        sha(bytes) == current,
        "W07 test dependency is not the exact reviewed afterimage",
    )?;
    let end = at
        .checked_add(insertion.len())
        .ok_or("W07 dependency span overflow")?;
    need(
        bytes.get(at..end) == Some(insertion),
        "W07 dependency insertion moved/missing/changed",
    )?;
    let mut old = bytes[..at].to_vec();
    old.extend_from_slice(&bytes[end..]);
    need(
        sha(&old) == prior,
        "W07 dependency changed unrelated predecessor bytes",
    )?;
    Ok(old)
}
pub fn reconstruct_lock(bytes: &[u8]) -> Result<Vec<u8>> {
    inverse(bytes, LOCK_PRIOR_SHA, LOCK_CURRENT_SHA, LOCK_AT, LOCK_EDGE)
}
fn reconstruct_manifest(bytes: &[u8]) -> Result<Vec<u8>> {
    inverse(
        bytes,
        MANIFEST_PRIOR_SHA,
        MANIFEST_CURRENT_SHA,
        MANIFEST_AT,
        DEV_LINE,
    )
}
#[cfg(unix)]
fn mode(stat: &fs::Metadata) -> Result<u32> {
    use std::os::unix::fs::PermissionsExt;
    Ok(stat.permissions().mode() & 0o777)
}
#[cfg(not(unix))]
fn mode(_: &fs::Metadata) -> Result<u32> {
    Err("W07 dependency source mode proof requires Unix".into())
}
fn read(root: &Path, path: &str) -> Result<Vec<u8>> {
    let path = root.join(path);
    let stat = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        stat.is_file() && !stat.file_type().is_symlink() && mode(&stat)? == 0o644,
        "W07 dependency source must be a regular non-symlink0644 file",
    )?;
    fs::read(path).map_err(|e| e.to_string())
}
fn pair(lock: &[u8], manifest: &[u8]) -> Result<Vec<u8>> {
    let old_lock = reconstruct_lock(lock)?;
    reconstruct_manifest(manifest)?;
    need(
        (sha(lock) == LOCK_PRIOR_SHA && sha(manifest) == MANIFEST_PRIOR_SHA)
            || (sha(lock) == LOCK_CURRENT_SHA && sha(manifest) == MANIFEST_CURRENT_SHA),
        "W07 dependency lock and manifest must use the same exact source profile",
    )?;
    Ok(old_lock)
}
/// Historical readers receive exactly their unchanged A06 predecessor bytes.
/// Both actual source leaves and their paired profile are checked first.
pub fn historical_lock(root: &Path, supplied: &[u8]) -> Result<Vec<u8>> {
    let lock = read(root, "Cargo.lock")?;
    let manifest = read(root, "crates/ssx-mcp/Cargo.toml")?;
    let old = pair(&lock, &manifest)?;
    need(
        supplied == lock || supplied == old,
        "W07 supplied dependency lock differs from its exact source pair",
    )?;
    Ok(old)
}
/// W07 current admission requires the actual new direct dev edge, never old pins alone.
pub fn verify_current(root: &Path) -> Result<()> {
    let lock = read(root, "Cargo.lock")?;
    let manifest = read(root, "crates/ssx-mcp/Cargo.toml")?;
    need(
        sha(&lock) == LOCK_CURRENT_SHA && sha(&manifest) == MANIFEST_CURRENT_SHA,
        "W07 current test dependency edge and manifest are required",
    )?;
    pair(&lock, &manifest).map(|_| ())
}
/// Called by the W07 named unit and every shared aggregate proof consumer.
pub fn controls(root: &Path) -> Result<()> {
    verify_current(root)?;
    let lock = read(root, "Cargo.lock")?;
    let manifest = read(root, "crates/ssx-mcp/Cargo.toml")?;
    let old_lock = reconstruct_lock(&lock)?;
    let old_manifest = reconstruct_manifest(&manifest)?;
    need(
        pair(&old_lock, &old_manifest).is_ok(),
        "W07 historical dependency pair rejected",
    )?;
    need(
        pair(&old_lock, &manifest).is_err() && pair(&lock, &old_manifest).is_err(),
        "W07 mixed dependency profiles accepted",
    )?;
    for (bytes, prior, current, at, insertion) in [
        (
            &lock[..],
            LOCK_PRIOR_SHA,
            LOCK_CURRENT_SHA,
            LOCK_AT,
            LOCK_EDGE,
        ),
        (
            &manifest[..],
            MANIFEST_PRIOR_SHA,
            MANIFEST_CURRENT_SHA,
            MANIFEST_AT,
            DEV_LINE,
        ),
    ] {
        let old = inverse(bytes, prior, current, at, insertion)?;
        let mut duplicate = bytes.to_vec();
        duplicate.splice(at..at, insertion.iter().copied());
        let mut relocated = old.clone();
        relocated.extend_from_slice(insertion);
        let mut unrelated = bytes.to_vec();
        unrelated.push(b' ');
        let mut changed = bytes.to_vec();
        changed[at] ^= 1;
        for bad in [duplicate, relocated, unrelated, changed] {
            need(
                inverse(&bad, prior, current, at, insertion).is_err(),
                "W07 duplicate/relocated/unrelated/tampered dependency delta accepted",
            )?;
        }
    }
    let text = String::from_utf8(lock.clone()).map_err(|e| e.to_string())?;
    for bad in [
        text.replacen("version = \"1.18.2\"", "version = \"1.18.3\"", 1),
        text.replacen("name = \"ssx-mcp\"", "name = \"other-package\"", 1),
        text.replacen(" \"axum\",", " \"unexpected-sdk\",", 1),
    ] {
        need(bad != text, "W07 dependency mutation control was vacuous")?;
        need(
            reconstruct_lock(bad.as_bytes()).is_err(),
            "W07 SDK/other-package lock delta accepted",
        )?;
    }
    filesystem_controls(&lock, &manifest, &old_lock, &old_manifest)
}
#[cfg(not(unix))]
fn filesystem_controls(_: &[u8], _: &[u8], _: &[u8], _: &[u8]) -> Result<()> {
    Err("W07 dependency filesystem controls require Unix".into())
}
#[cfg(unix)]
fn filesystem_controls(
    lock: &[u8],
    manifest: &[u8],
    old_lock: &[u8],
    old_manifest: &[u8],
) -> Result<()> {
    use std::os::unix::fs::{PermissionsExt, symlink};
    struct Owned(std::path::PathBuf);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("w07-dev-{}-{suffix}", std::process::id()));
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let owned = Owned(directory);
    fs::create_dir_all(owned.0.join("crates/ssx-mcp")).map_err(|e| e.to_string())?;
    let target = owned.0.join("Cargo.lock");
    let manifest_path = owned.0.join("crates/ssx-mcp/Cargo.toml");
    fs::write(&target, lock).map_err(|e| e.to_string())?;
    fs::write(&manifest_path, manifest).map_err(|e| e.to_string())?;
    for path in [&target, &manifest_path] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
    }
    verify_current(&owned.0)?;
    for path in [&target, &manifest_path] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        need(
            verify_current(&owned.0).is_err(),
            "W07 changed dependency source mode accepted",
        )?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
        let backup = path.with_extension("owned-original");
        fs::rename(path, &backup).map_err(|e| e.to_string())?;
        need(
            verify_current(&owned.0).is_err(),
            "W07 missing dependency source accepted",
        )?;
        symlink(&backup, path).map_err(|e| e.to_string())?;
        need(
            verify_current(&owned.0).is_err(),
            "W07 symlink dependency source accepted",
        )?;
        fs::remove_file(path).map_err(|e| e.to_string())?;
        fs::rename(backup, path).map_err(|e| e.to_string())?;
    }
    fs::write(&target, old_lock).map_err(|e| e.to_string())?;
    fs::write(&manifest_path, old_manifest).map_err(|e| e.to_string())?;
    need(
        historical_lock(&owned.0, old_lock)?.as_slice() == old_lock,
        "W07 exact historical dependency read rejected",
    )?;
    need(
        verify_current(&owned.0).is_err() && historical_lock(&owned.0, lock).is_err(),
        "W07 historical pair authorized a current dependency afterimage",
    )?;
    fs::write(&target, lock).map_err(|e| e.to_string())?;
    fs::write(&manifest_path, manifest).map_err(|e| e.to_string())?;
    verify_current(&owned.0)
}
