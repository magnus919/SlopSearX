//! Explicit trusted current gate root; validates proof material before executing it.
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
type Result<T> = std::result::Result<T, String>;
const FILES: &[(&str, &str, usize)] = &[
    (
        "docs/rust-rewrite/t06/source-amendments/receipt-control-successor.rs",
        "792876948f3885663a72373a733e7b6c312a45de0b3c2cda431fbf9fae1cbef1",
        8372,
    ),
    (
        "docs/rust-rewrite/t06/source-amendments/receipt-control-successor.json",
        "d6fb5b119b7a20b810a5cf2f45bb523d882bce928329ec9b417ae804aeb728b0",
        9067,
    ),
];
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn validate(path: &str, raw: &[u8], mode: u32) -> Result<()> {
    let (_, expected, bytes) = FILES
        .iter()
        .find(|(name, _, _)| *name == path)
        .ok_or("receipt-control closure unlisted material")?;
    if mode != 0o644 || raw.len() != *bytes || digest(raw) != *expected {
        return Err(format!(
            "receipt-control expected current proof material differs: {path}"
        ));
    }
    Ok(())
}
pub(crate) fn verify(root: &Path) -> Result<()> {
    for (path, _, _) in FILES {
        let full = root.join(path);
        let metadata = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("receipt-control proof material must be regular and nonsymlink".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            validate(
                path,
                &fs::read(full).map_err(|e| e.to_string())?,
                metadata.permissions().mode() & 0o7777,
            )?;
        }
        #[cfg(not(unix))]
        {
            return Err("receipt-control proof mode requires Unix".into());
        }
    }
    Ok(())
}
pub(crate) fn controls(root: &Path) -> Result<()> {
    verify(root)?;
    for (path, _, _) in FILES {
        let raw = fs::read(root.join(path)).map_err(|e| e.to_string())?;
        let mut changed = raw.clone();
        changed[0] ^= 1;
        let mut extra = raw.clone();
        extra.push(b' ');
        if validate(path, &changed, 0o644).is_ok()
            || validate(path, &extra, 0o644).is_ok()
            || validate(path, &raw, 0o755).is_ok()
            || validate("unlisted.rs", &raw, 0o644).is_ok()
        {
            return Err(
                "receipt-control closure accepted changed/moved/mode/unlisted material".into(),
            );
        }
    }
    Ok(())
}
