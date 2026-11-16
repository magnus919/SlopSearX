//! Single reviewed review-harness pin successor; frozen W03 source oracles stay immutable.
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

pub(super) const SOURCE: &str = "tests/test_disabled_review_workflows.py";
const BYTES: usize = 9976;
const HISTORICAL_SHA: &str = "e349b81f9225bb7a9919dd0c388bfd3716f2cd9a9273f93fef4654f2028af8b8";
const CURRENT_SHA: &str = "a65059c41681137b1bf1532e8f76a4b09c82eb8d328bb35d719203131f2c4c1e";
const OLD_LINE: &[u8] = b"\nHARNESS_SHA = \"04c7fc0b42221872e6c6b611be91c68e462fc679\"\n";
const NEW_LINE: &[u8] = b"\nHARNESS_SHA = \"1c76d40290cdaf9b6603721f899da6c7e8a382b4\"\n";

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn reject() -> String {
    "W03 review-harness pin source is not the exact reviewed successor".into()
}
fn validate(
    path: &str,
    expected: &str,
    actual: &[u8],
    regular: bool,
    mode: u32,
) -> Result<Vec<u8>, String> {
    // Authenticate the whole supplied physical source before any inverse edit.
    if path != SOURCE
        || expected != HISTORICAL_SHA
        || !regular
        || mode != 0o644
        || actual.len() != BYTES
        || digest(actual) != CURRENT_SHA
    {
        return Err(reject());
    }
    let offsets: Vec<_> = actual
        .windows(NEW_LINE.len())
        .enumerate()
        .filter_map(|(at, bytes)| (bytes == NEW_LINE).then_some(at))
        .collect();
    if offsets.len() != 1 || NEW_LINE.len() != OLD_LINE.len() {
        return Err(reject());
    }
    let mut prior = actual.to_vec();
    let at = offsets[0];
    prior[at..at + OLD_LINE.len()].copy_from_slice(OLD_LINE);
    if digest(&prior) != HISTORICAL_SHA {
        return Err(reject());
    }
    Ok(prior)
}

#[cfg(unix)]
fn read_regular(path: &Path) -> Result<Vec<u8>, String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let before = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !before.is_file() || before.permissions().mode() & 0o7777 != 0o644 {
        return Err(reject());
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let opened = file.metadata().map_err(|e| e.to_string())?;
    if !opened.is_file()
        || opened.permissions().mode() & 0o7777 != 0o644
        || opened.dev() != before.dev()
        || opened.ino() != before.ino()
    {
        return Err(reject());
    }
    let mut raw = Vec::new();
    file.take((BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    Ok(raw)
}
#[cfg(not(unix))]
fn read_regular(_path: &Path) -> Result<Vec<u8>, String> {
    Err("W03 review-harness pin custody requires Unix regular 0644 sources".into())
}

pub(super) fn historical(
    root: &Path,
    path: &str,
    expected: &str,
) -> Result<Option<Vec<u8>>, String> {
    if path != SOURCE {
        return Ok(None);
    }
    if expected != HISTORICAL_SHA {
        return Err(reject());
    }
    let actual = read_regular(&root.join(SOURCE))?;
    validate(path, expected, &actual, true, 0o644).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }
    fn actual() -> Vec<u8> {
        fs::read(root().join(SOURCE)).unwrap()
    }
    #[test]
    fn exact_physical_current_has_only_the_reviewed_pin_inverse() {
        let current = actual();
        let prior = validate(SOURCE, HISTORICAL_SHA, &current, true, 0o644).unwrap();
        assert_eq!(prior.len(), current.len());
        assert_eq!(digest(&prior), HISTORICAL_SHA);
        assert_eq!(
            historical(&root(), SOURCE, HISTORICAL_SHA).unwrap(),
            Some(prior)
        );
    }
    #[test]
    fn historical_bytes_cannot_be_supplied_as_current() {
        let prior = validate(SOURCE, HISTORICAL_SHA, &actual(), true, 0o644).unwrap();
        assert!(validate(SOURCE, HISTORICAL_SHA, &prior, true, 0o644).is_err());
    }
    #[test]
    fn wrong_path_or_historical_tuple_cannot_select_the_successor() {
        let current = actual();
        assert!(validate("tests/other.py", HISTORICAL_SHA, &current, true, 0o644).is_err());
        assert!(validate(SOURCE, CURRENT_SHA, &current, true, 0o644).is_err());
        assert!(historical(&root(), SOURCE, CURRENT_SHA).is_err());
        assert_eq!(
            historical(&root(), "tests/other.py", HISTORICAL_SHA).unwrap(),
            None
        );
    }
    #[test]
    fn pin_span_and_unrelated_source_mutations_are_rejected() {
        let current = actual();
        let at = current
            .windows(NEW_LINE.len())
            .position(|bytes| bytes == NEW_LINE)
            .unwrap();
        let mut changed_pin = current.clone();
        changed_pin[at + NEW_LINE.len() - 4] ^= 1;
        let mut outside = current.clone();
        outside[0] ^= 1;
        let mut duplicate = current.clone();
        duplicate.extend_from_slice(NEW_LINE);
        let mut truncated = current;
        truncated.pop();
        for changed in [changed_pin, outside, duplicate, truncated] {
            assert!(validate(SOURCE, HISTORICAL_SHA, &changed, true, 0o644).is_err());
        }
    }
    #[test]
    fn nonregular_and_changed_modes_are_rejected() {
        let current = actual();
        assert!(validate(SOURCE, HISTORICAL_SHA, &current, false, 0o644).is_err());
        for mode in [0o600, 0o664, 0o755, 0o4644] {
            assert!(validate(SOURCE, HISTORICAL_SHA, &current, true, mode).is_err());
        }
    }
    #[cfg(unix)]
    #[test]
    fn physical_missing_symlink_and_mode_drift_are_rejected() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        // Qualification must supply its registered Tank/runner scratch directory.
        let scratch = std::env::var_os("TMPDIR")
            .or_else(|| std::env::var_os("RUNNER_TEMP"))
            .expect("registered TMPDIR or designated RUNNER_TEMP is required");
        let scratch = Path::new(&scratch);
        assert!(scratch.is_absolute());
        let temp = scratch.join(format!(
            "w03-review-pin-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&temp).unwrap(); // Never adopt an existing path.
        struct Owned(std::path::PathBuf);
        impl Drop for Owned {
            fn drop(&mut self) {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }
        let owned = Owned(temp);
        let selected = owned.0.join(SOURCE);
        fs::create_dir(selected.parent().unwrap()).unwrap();
        assert!(historical(&owned.0, SOURCE, HISTORICAL_SHA).is_err());
        symlink(root().join(SOURCE), &selected).unwrap();
        assert!(historical(&owned.0, SOURCE, HISTORICAL_SHA).is_err());
        fs::remove_file(&selected).unwrap();
        fs::write(&selected, actual()).unwrap();
        fs::set_permissions(&selected, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            historical(&owned.0, SOURCE, HISTORICAL_SHA)
                .unwrap()
                .is_some()
        );
        fs::set_permissions(&selected, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(historical(&owned.0, SOURCE, HISTORICAL_SHA).is_err());
    }
}
