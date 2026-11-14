//! Portable Cargo.lock reader used by W07's Windows-specific admission path.
//!
//! This leaf intentionally depends only on `std` so its exact source can be
//! compiled for the native Windows target without building the full xtask
//! dependency graph.
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
    process::Command,
};

const MAX_SOURCE_BYTES: u64 = 1_048_576;

pub fn physical_current_cargo_lock(
    root: &Path,
    expected_bytes: usize,
) -> Result<(Vec<u8>, &'static str), String> {
    let full = root.join("Cargo.lock");
    let root_canonical = root.canonicalize().map_err(|error| error.to_string())?;
    let before = fs::symlink_metadata(&full).map_err(|error| error.to_string())?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.len() != expected_bytes as u64
        || before.len() > MAX_SOURCE_BYTES
        || !full
            .canonicalize()
            .map_err(|error| error.to_string())?
            .starts_with(&root_canonical)
    {
        return Err("W07 current Cargo.lock must be the exact regular source".into());
    }

    let index = Command::new("git")
        .args(["ls-files", "--stage", "--", "Cargo.lock"])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    if !index.status.success() {
        return Err("W07 Git index query failed".into());
    }
    let index = String::from_utf8(index.stdout).map_err(|error| error.to_string())?;
    let index_lines = index.lines().collect::<Vec<_>>();
    if index_lines.len() != 1
        || !index_lines[0].starts_with("100644 ")
        || !index_lines[0].ends_with(" 0\tCargo.lock")
    {
        return Err("W07 Cargo.lock requires the stage-zero Git mode 100644".into());
    }

    let file = File::open(&full).map_err(|error| error.to_string())?;
    let opened = file.metadata().map_err(|error| error.to_string())?;
    if !opened.is_file() || opened.len() != before.len() {
        return Err("W07 current Cargo.lock descriptor identity changed".into());
    }
    let mut bytes = Vec::with_capacity(expected_bytes);
    (&file)
        .take(expected_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let after = file.metadata().map_err(|error| error.to_string())?;
    let named = fs::symlink_metadata(&full).map_err(|error| error.to_string())?;
    if !after.is_file()
        || after.len() != before.len()
        || !named.is_file()
        || named.file_type().is_symlink()
        || named.len() != after.len()
        || bytes.len() != expected_bytes
    {
        return Err("W07 current Cargo.lock changed during bounded source read".into());
    }
    Ok((bytes, "0644"))
}
