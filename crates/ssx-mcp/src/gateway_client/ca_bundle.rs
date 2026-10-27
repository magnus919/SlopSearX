//! Optional operator trust extends platform verification; it never replaces it.
use super::io_owner::IoOwner;
use crate::Result;
use std::{io::Read, path::PathBuf, time::Instant};
use tokio_util::sync::CancellationToken;
const MAX_BUNDLE_BYTES: u64 = 4 * 1024 * 1024;
fn read(path: &std::path::Path) -> Result<Vec<reqwest::Certificate>> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|_| "gateway CA bundle unavailable")?;
    if !file
        .metadata()
        .map_err(|_| "gateway CA bundle unavailable")?
        .is_file()
    {
        return Err("gateway CA bundle must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BUNDLE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "gateway CA bundle unavailable")?;
    if bytes.len() as u64 > MAX_BUNDLE_BYTES {
        return Err("gateway CA bundle exceeds operational byte bound".into());
    }
    let certificates =
        reqwest::Certificate::from_pem_bundle(&bytes).map_err(|_| "gateway CA bundle malformed")?;
    if certificates.is_empty() {
        return Err("gateway CA bundle has no certificates".into());
    }
    Ok(certificates)
}
pub(super) async fn load(
    path: Option<PathBuf>,
    io: &IoOwner,
    end: Instant,
    cancel: &CancellationToken,
) -> Result<Vec<reqwest::Certificate>> {
    let certificates = match path {
        None => Vec::new(),
        Some(path) => io
            .run(end, cancel, move || read(&path))
            .await
            .map_err(|_| "gateway CA bundle I/O cancelled or failed")??,
    };
    io.check(end, cancel)
        .map_err(|_| "gateway CA bundle admission expired")?;
    Ok(certificates)
}
