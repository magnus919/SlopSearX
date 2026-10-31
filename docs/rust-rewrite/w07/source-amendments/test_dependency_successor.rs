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
fn original_historical_lock(root: &Path, supplied: &[u8]) -> Result<Vec<u8>> {
    let lock = read(root, "Cargo.lock")?;
    let manifest = read(root, "crates/ssx-mcp/Cargo.toml")?;
    if sha(&lock) == LOCK_PRIOR_SHA {
        let old = pair(&lock, &manifest)?;
        need(supplied == lock, "W07 supplied historical lock differs")?;
        return Ok(old);
    }
    let parent = super::w07_t02_joint::original_parent(
        root,
        "Cargo.lock",
        supplied,
        super::w07_t02_joint::Parent::W07,
    )?;
    pair(&parent, &manifest)
}
/// W07 current admission requires the actual new direct dev edge, never old pins alone.
fn original_verify_current(root: &Path) -> Result<()> {
    let lock = read(root, "Cargo.lock")?;
    let manifest = read(root, "crates/ssx-mcp/Cargo.toml")?;
    need(
        sha(&lock) == super::w07_t02_joint::LOCK_SHA && sha(&manifest) == MANIFEST_CURRENT_SHA,
        "W07 current test dependency edge and manifest are required",
    )?;
    let parent = super::w07_t02_joint::original_parent(
        root,
        "Cargo.lock",
        &lock,
        super::w07_t02_joint::Parent::W07,
    )?;
    pair(&parent, &manifest).map(|_| ())
}
/// Current physical lock and manifest first prove the accepted normal-main
/// frame; the original paired aws-lc inverse remains the only historic read.
pub fn historical_lock(root: &Path, supplied: &[u8]) -> Result<Vec<u8>> {
    let lock = read(root, "Cargo.lock")?;
    need(supplied == lock, "W07 supplied physical lock differs")?;
    let manifest = super::main_isolation_current::parent(
        root,
        "crates/ssx-mcp/Cargo.toml",
        None,
        super::main_isolation_current::Parent::Main,
    )?;
    let accepted = super::w07_t02_joint::parent(
        root,
        "Cargo.lock",
        supplied,
        super::w07_t02_joint::Parent::W07,
    )?;
    pair(&accepted, &manifest)
}
pub fn verify_current(root: &Path) -> Result<()> {
    let lock = read(root, "Cargo.lock")?;
    let main = super::main_isolation_current::parent(
        root,
        "Cargo.lock",
        Some(&lock),
        super::main_isolation_current::Parent::Main,
    )?;
    let manifest = super::main_isolation_current::parent(
        root,
        "crates/ssx-mcp/Cargo.toml",
        None,
        super::main_isolation_current::Parent::Main,
    )?;
    let cli = super::w07_t02_joint::cli_source_custody::prior(root, "Cargo.lock", &lock)?;
    need(
        main == cli,
        "gateway and CLI current lock agree on original joint frame",
    )?;
    need(
        sha(&main) == super::w07_t02_joint::LOCK_SHA && sha(&manifest) == MANIFEST_CURRENT_SHA,
        "W07 accepted main dependency pair required",
    )?;
    let accepted =
        super::w07_t02_joint::parent(root, "Cargo.lock", &lock, super::w07_t02_joint::Parent::W07)?;
    pair(&accepted, &manifest).map(|_| ())
}
/// Called by the W07 named unit and every shared aggregate proof consumer.
pub fn controls(root: &Path) -> Result<()> {
    verify_current(root)?;
    super::w07_t02_joint::controls(root)?;
    let physical_lock = read(root, "Cargo.lock")?;
    let lock = super::w07_t02_joint::parent(
        root,
        "Cargo.lock",
        &physical_lock,
        super::w07_t02_joint::Parent::W07,
    )?;
    let manifest = super::main_isolation_current::parent(
        root,
        "crates/ssx-mcp/Cargo.toml",
        None,
        super::main_isolation_current::Parent::Main,
    )?;
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
    let main_lock = super::main_isolation_current::parent(
        root,
        "Cargo.lock",
        Some(&physical_lock),
        super::main_isolation_current::Parent::Main,
    )?;
    current_filesystem_controls(root, &physical_lock)?;
    filesystem_controls(&main_lock, &manifest, &old_lock, &old_manifest)
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
    original_verify_current(&owned.0)?;
    for path in [&target, &manifest_path] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        need(
            original_verify_current(&owned.0).is_err(),
            "W07 changed dependency source mode accepted",
        )?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
        let backup = path.with_extension("owned-original");
        fs::rename(path, &backup).map_err(|e| e.to_string())?;
        need(
            original_verify_current(&owned.0).is_err(),
            "W07 missing dependency source accepted",
        )?;
        symlink(&backup, path).map_err(|e| e.to_string())?;
        need(
            original_verify_current(&owned.0).is_err(),
            "W07 symlink dependency source accepted",
        )?;
        fs::remove_file(path).map_err(|e| e.to_string())?;
        fs::rename(backup, path).map_err(|e| e.to_string())?;
    }
    fs::write(&target, old_lock).map_err(|e| e.to_string())?;
    fs::write(&manifest_path, old_manifest).map_err(|e| e.to_string())?;
    need(
        original_historical_lock(&owned.0, old_lock)?.as_slice() == old_lock,
        "W07 exact historical dependency read rejected",
    )?;
    need(
        original_verify_current(&owned.0).is_err()
            && original_historical_lock(&owned.0, lock).is_err(),
        "W07 historical pair authorized a current dependency afterimage",
    )?;
    fs::write(&target, lock).map_err(|e| e.to_string())?;
    fs::write(&manifest_path, manifest).map_err(|e| e.to_string())?;
    original_verify_current(&owned.0)
}

#[cfg(not(unix))]
fn current_filesystem_controls(_: &Path, _: &[u8]) -> Result<()> {
    Err("W07 current dependency filesystem controls require Unix".into())
}
#[cfg(unix)]
fn current_filesystem_controls(source_root: &Path, current_lock: &[u8]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    struct Owned(std::path::PathBuf);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("w07-stacked-dev-{}-{stamp}", std::process::id()));
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let owned = Owned(directory);
    let stacked = &owned.0;
    super::w07_t02_joint::snapshot_helper_amendment::gateway_w08_main_join::stacked::copy_fixture_inputs(source_root, stacked)?;
    for path in [
        "crates/ssx-mcp/Cargo.toml",
        "docs/rust-rewrite/t07/source-amendments/main-isolation-current-operands-v1.json",
        "docs/rust-rewrite/t08a/shared-source-successor.json",
        "docs/rust-rewrite/t08a/accepted-w08-main-join-v1.json",
    ] {
        let target = stacked.join(path);
        fs::create_dir_all(target.parent().ok_or("current fixture parent")?)
            .map_err(|e| e.to_string())?;
        fs::copy(source_root.join(path), &target).map_err(|e| e.to_string())?;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
            .map_err(|e| e.to_string())?;
    }
    verify_current(stacked)?;
    let capsule = stacked.join(super::w07_t02_joint::snapshot_helper_amendment::gateway_w08_main_join::stacked::CAPSULE_PATH);
    let raw = fs::read(&capsule).map_err(|e| e.to_string())?;
    let mut changed = raw.clone();
    changed.push(0);
    fs::write(&capsule, changed).map_err(|e| e.to_string())?;
    need(
        verify_current(stacked).is_err(),
        "changed stacked lock capsule accepted",
    )?;
    fs::remove_file(&capsule).map_err(|e| e.to_string())?;
    need(
        verify_current(stacked).is_err(),
        "missing stacked lock capsule accepted",
    )?;
    fs::write(&capsule, &raw).map_err(|e| e.to_string())?;
    fs::set_permissions(&capsule, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
    let target = stacked.join("Cargo.lock");
    let old =
        super::w07_t02_joint::cli_source_custody::prior(source_root, "Cargo.lock", current_lock)?;
    fs::write(&target, old).map_err(|e| e.to_string())?;
    need(
        verify_current(stacked).is_err(),
        "old joint lock substituted for stacked current",
    )?;
    fs::write(&target, current_lock).map_err(|e| e.to_string())?;
    let manifest = stacked.join("crates/ssx-mcp/Cargo.toml");
    let raw = fs::read(&manifest).map_err(|e| e.to_string())?;
    let mut changed = raw.clone();
    changed[0] ^= 1;
    fs::write(&manifest, changed).map_err(|e| e.to_string())?;
    need(
        verify_current(stacked).is_err(),
        "changed paired stacked MCP manifest accepted",
    )?;
    fs::write(&manifest, raw).map_err(|e| e.to_string())?;
    verify_current(stacked)
}
