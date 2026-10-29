//! Token document policy, distinct from publicly readable package integrity.
//! The current user, SYSTEM and Administrators remain the trusted principals.
use crate::trust::{
    Identity, create_private_directory, create_private_file, identity, opened_confidential,
    opened_confidential_parent,
};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub enum ConfidentialRead {
    Missing,
    Bytes(Vec<u8>),
    TooLarge,
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn absolute(path: &Path) -> io::Result<PathBuf> {
    // Keep relative caller paths relative to the actual filesystem worker CWD.
    std::path::absolute(path)
}
fn parent(path: &Path) -> io::Result<&Path> {
    path.parent()
        .ok_or_else(|| io::ErrorKind::InvalidInput.into())
}
fn prepare_directory(path: &Path) -> io::Result<File> {
    match opened_confidential_parent(path) {
        Ok(file) => Ok(file),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let _parent = prepare_directory(parent(path)?)?;
            create_private_directory(path)?;
            opened_confidential_parent(path)
        }
        Err(error) => Err(error),
    }
}
pub fn check_confidential_file(path: &Path) -> io::Result<()> {
    opened_confidential(&absolute(path)?).map(drop)
}
pub fn read_confidential_file(path: &Path, limit: usize) -> io::Result<ConfidentialRead> {
    let path = absolute(path)?;
    // Missing storage is not a grant to create directories during a read.
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(ConfidentialRead::Missing);
        }
        Err(error) => return Err(error),
        Ok(_) => (),
    }
    read_admitted(admit(&path)?, limit)
}
struct AdmittedRead {
    file: File,
    _parent: File,
    before: Identity,
}
fn admit(path: &Path) -> io::Result<AdmittedRead> {
    let parent = opened_confidential_parent(parent(path)?)?;
    let file = opened_confidential(path)?;
    let before = identity(&file, false)?;
    Ok(AdmittedRead {
        file,
        _parent: parent,
        before,
    })
}
fn read_admitted(mut admitted: AdmittedRead, limit: usize) -> io::Result<ConfidentialRead> {
    let bound = limit.checked_add(1).ok_or(io::ErrorKind::InvalidInput)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut admitted.file)
        .take(bound as u64)
        .read_to_end(&mut bytes)?;
    if identity(&admitted.file, false)? != admitted.before {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(if bytes.len() > limit {
        ConfidentialRead::TooLarge
    } else {
        ConfidentialRead::Bytes(bytes)
    })
}
/// A deterministic fixture barrier after the exact production read admission.
/// Owns the original handles; exposes no raw handle or new file access policy.
#[cfg(feature = "fixtures")]
pub struct FixtureConfidentialRead(AdmittedRead);
#[cfg(feature = "fixtures")]
impl FixtureConfidentialRead {
    pub fn open(path: &Path) -> io::Result<Self> {
        Ok(Self(admit(&absolute(path)?)?))
    }
    pub fn read(self, limit: usize) -> io::Result<ConfidentialRead> {
        read_admitted(self.0, limit)
    }
}
pub fn write_confidential_file(path: &Path, bytes: &[u8], nonce: [u8; 16]) -> io::Result<()> {
    let path = absolute(path)?;
    // Retain the validated parent against replacement through publication. A
    // private file cannot repair an untrusted-writer parent directory.
    let _parent = prepare_directory(parent(&path)?)?;
    let _previous = match opened_confidential(&path) {
        Ok(file) => Some(file),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let suffix: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
    let temporary_path = parent(&path)?.join(format!(".slopsearx-oauth-{suffix}.tmp"));
    let mut file = create_private_file(&temporary_path)?;
    let temporary = Temporary(temporary_path);
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    // Same-parent rename preserves complete publication and the newly created
    // file's protected DACL. No copy-across-volume or delete-then-write fallback.
    fs::rename(&temporary.0, &path)?;
    check_confidential_file(&path)
}
