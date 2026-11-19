//! Shared descriptor-bound bounded source reads. Included by independent Rust gates.
use std::{fs, io::Read, path::Path};
type Result<T> = std::result::Result<T, String>;
fn need(ok: bool, why: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(why.into()) }
}
#[cfg(windows)]
fn windows_directory_chain(root: &Path) -> Result<Vec<std::path::PathBuf>> {
    use std::path::{Component, PathBuf};
    let mut cursor = PathBuf::new();
    let mut saw_root = false;
    let mut chain = Vec::new();
    for component in root.components() {
        match component {
            Component::Prefix(_) => cursor.push(component.as_os_str()),
            Component::RootDir => {
                cursor.push(component.as_os_str());
                saw_root = true;
                chain.push(cursor.clone());
            }
            Component::Normal(_) if saw_root => {
                cursor.push(component.as_os_str());
                chain.push(cursor.clone());
            }
            Component::CurDir if saw_root => {}
            Component::ParentDir if chain.len() > 1 => {
                chain.pop();
                cursor = chain.last().ok_or("missing Windows root anchor")?.clone();
            }
            _ => return Err("Windows root is not an anchored absolute path".into()),
        }
    }
    need(
        saw_root && !chain.is_empty(),
        "Windows root has no complete anchor",
    )?;
    Ok(chain)
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const _: () = {
    assert!(nix::fcntl::OFlag::O_DIRECTORY.bits() == 0x4000);
    assert!(nix::fcntl::OFlag::O_NOFOLLOW.bits() == 0x8000);
};

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unix_root(root: &Path) -> Result<fs::File> {
    use nix::{
        fcntl::{OFlag, open, openat},
        sys::stat::Mode,
    };
    use std::path::Component;

    let flags = OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC;
    let anchor = Path::new(if root.is_absolute() { "/" } else { "." });
    let anchor = fs::File::from(open(anchor, flags, Mode::empty()).map_err(|e| e.to_string())?);
    let mut held = vec![anchor];
    for part in root.components() {
        match part {
            Component::RootDir | Component::CurDir => {}
            Component::ParentDir if held.len() > 1 => {
                held.pop();
            }
            Component::Normal(_) | Component::ParentDir => {
                let parent = held.last().ok_or("missing source root anchor")?;
                let directory = fs::File::from(
                    openat(parent, part.as_os_str(), flags, Mode::empty())
                        .map_err(|e| e.to_string())?,
                );
                if part == Component::ParentDir {
                    held[0] = directory;
                } else {
                    held.push(directory);
                }
            }
            Component::Prefix(_) => return Err("unexpected source root prefix".into()),
        }
    }
    held.pop()
        .ok_or_else(|| "missing source root anchor".into())
}

pub(crate) fn bounded_read(root: &Path, path: &str, limit: u64) -> Result<(Vec<u8>, u32)> {
    bounded_read_inner(root, path, limit, || {})
}
fn bounded_read_inner<F: FnOnce()>(
    root: &Path,
    path: &str,
    limit: u64,
    after_open: F,
) -> Result<(Vec<u8>, u32)> {
    bounded_read_with_hooks(root, path, limit, after_open, || {})
}

fn bounded_read_with_hooks(
    root: &Path,
    path: &str,
    limit: u64,
    after_open: impl FnOnce(),
    after_read: impl FnOnce(),
) -> Result<(Vec<u8>, u32)> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use nix::{
            fcntl::{OFlag, openat},
            sys::stat::Mode,
        };
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        need(
            !Path::new(path).is_absolute()
                && Path::new(path)
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_))),
            "source path must be a relative normal path",
        )?;
        let root_dir = unix_root(root)?;
        let root_identity = root_dir.metadata().map_err(|e| e.to_string())?;
        let mut parent = root_dir.try_clone().map_err(|e| e.to_string())?;
        let parts: Vec<_> = Path::new(path).components().collect();
        for part in &parts[..parts.len().saturating_sub(1)] {
            parent = fs::File::from(
                openat(
                    &parent,
                    part.as_os_str(),
                    OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|e| e.to_string())?,
            );
        }
        let leaf = parts.last().ok_or("empty source path")?;
        let file = fs::File::from(
            openat(
                &parent,
                leaf.as_os_str(),
                OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|e| e.to_string())?,
        );
        let opened = file.metadata().map_err(|e| e.to_string())?;
        let mode = opened.permissions().mode() & 0o7777;
        need(
            opened.is_file() && opened.len() <= limit,
            "opened source must be regular and within byte limit",
        )?;
        after_open();
        let mut bytes = Vec::new();
        (&file)
            .take(limit.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        after_read();
        let after = file.metadata().map_err(|e| e.to_string())?;
        // Re-resolve from the held root after reading. This catches a parent
        // directory that was renamed/replaced during the read, without
        // following a parent symlink during the verification walk.
        let mut verify_parent = root_dir;
        for part in &parts[..parts.len().saturating_sub(1)] {
            verify_parent = fs::File::from(
                openat(
                    &verify_parent,
                    part.as_os_str(),
                    OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|e| e.to_string())?,
            );
        }
        let named_file = fs::File::from(
            openat(
                &verify_parent,
                leaf.as_os_str(),
                OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|e| e.to_string())?,
        );
        let times = |meta: &fs::Metadata| {
            (
                meta.mtime(),
                meta.mtime_nsec(),
                meta.ctime(),
                meta.ctime_nsec(),
            )
        };
        let matches_opened = |meta: &fs::Metadata| {
            meta.is_file()
                && meta.len() == opened.len()
                && meta.dev() == opened.dev()
                && meta.ino() == opened.ino()
                && meta.permissions().mode() & 0o7777 == mode
                && times(meta) == times(&opened)
        };
        need(
            matches_opened(&named_file.metadata().map_err(|e| e.to_string())?),
            "reopened source identity or size changed before verification read",
        )?;
        let mut current = Vec::new();
        (&named_file)
            .take(limit.saturating_add(1))
            .read_to_end(&mut current)
            .map_err(|e| e.to_string())?;
        let named_meta = named_file.metadata().map_err(|e| e.to_string())?;
        let named_root = unix_root(root)?.metadata().map_err(|e| e.to_string())?;
        need(
            root_identity.dev() == named_root.dev() && root_identity.ino() == named_root.ino(),
            "source root identity changed during bounded read",
        )?;
        need(
            bytes.len() as u64 == opened.len()
                && current == bytes
                && matches_opened(&after)
                && matches_opened(&named_meta),
            "source identity or size changed during bounded read",
        )?;
        Ok((current, mode))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        #[cfg(windows)]
        {
            use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
            need(
                !Path::new(path).is_absolute()
                    && Path::new(path)
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_))),
                "source path must be a relative normal path",
            )?;
            const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
            const BACKUP_SEMANTICS: u32 = 0x0200_0000;
            const FILE_SHARE_READ: u32 = 0x0000_0001;
            const REPARSE_POINT: u32 = 0x0000_0400;
            // Resolve lexical components first; each actual directory is opened
            // without resolving reparse points and held against replacement.
            let absolute_root = std::path::absolute(root).map_err(|e| e.to_string())?;
            let mut held_directories = Vec::new();
            let root_chain = windows_directory_chain(&absolute_root)?;
            let held_root_path = root_chain
                .last()
                .ok_or("missing Windows root anchor")?
                .clone();
            for directory_path in root_chain {
                let directory = fs::OpenOptions::new()
                    .read(true)
                    .share_mode(FILE_SHARE_READ)
                    .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS)
                    .open(&directory_path)
                    .map_err(|e| e.to_string())?;
                let metadata = directory.metadata().map_err(|e| e.to_string())?;
                need(
                    metadata.is_dir() && metadata.file_attributes() & REPARSE_POINT == 0,
                    "source root traversal encountered a reparse point or non-directory",
                )?;
                held_directories.push(directory);
            }
            let mut parent_path = held_root_path;
            let full = parent_path.join(path);
            let components: Vec<_> = Path::new(path).components().collect();
            for part in &components[..components.len().saturating_sub(1)] {
                parent_path.push(part.as_os_str());
                let directory = fs::OpenOptions::new()
                    .read(true)
                    .share_mode(FILE_SHARE_READ)
                    .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS)
                    .open(&parent_path)
                    .map_err(|e| e.to_string())?;
                let metadata = directory.metadata().map_err(|e| e.to_string())?;
                need(
                    metadata.is_dir() && metadata.file_attributes() & REPARSE_POINT == 0,
                    "source parent traversal encountered a reparse point or non-directory",
                )?;
                held_directories.push(directory);
            }
            let file = fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .custom_flags(OPEN_REPARSE_POINT)
                .open(&full)
                .map_err(|e| e.to_string())?;
            let opened = file.metadata().map_err(|e| e.to_string())?;
            need(
                opened.is_file()
                    && opened.file_attributes() & REPARSE_POINT == 0
                    && opened.len() <= limit,
                "opened source must be regular and within byte limit",
            )?;
            after_open();
            let mut bytes = Vec::new();
            (&file)
                .take(limit.saturating_add(1))
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            after_read();
            let after = file.metadata().map_err(|e| e.to_string())?;
            need(
                bytes.len() as u64 <= limit
                    && after.is_file()
                    && after.file_attributes() & REPARSE_POINT == 0
                    && after.len() == opened.len(),
                "source identity or size changed during bounded read",
            )?;
            drop(held_directories);
            Ok((bytes, 0))
        }
        #[cfg(not(windows))]
        {
            let _ = (root, path, limit);
            let _ = (after_open, after_read);
            Err("descriptor-bound source read requires a supported host".into())
        }
    }
}

// Exercise the shared reader in its existing owning test. Registering tests here
// would change the frozen inventory of the other binaries that include this file.
#[cfg(any(all(test, unix), windows))]
#[allow(
    dead_code,
    reason = "Only the T08A test owner calls these controls; other suites include the reader by path"
)]
pub(super) fn controls(root: &std::path::Path) {
    #[cfg(unix)]
    bounded_read_tests::controls(root);
    #[cfg(windows)]
    windows_handle_tests::controls(root);
}

#[cfg(windows)]
#[allow(
    dead_code,
    reason = "Only the T08A test owner calls these controls; other suites include the reader by path"
)]
mod windows_handle_tests {
    use super::{bounded_read_inner, windows_directory_chain};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    pub(super) fn controls(root: &std::path::Path) {
        drive_and_unc_roots_start_at_complete_anchors();
        held_leaf_handle_blocks_write_delete_and_replacement_until_read_returns(root);
        parent_reparse_point_is_rejected_when_symlink_creation_is_available(root);
        held_parent_handle_blocks_rename_during_leaf_read(root);
    }

    fn drive_and_unc_roots_start_at_complete_anchors() {
        use std::path::Path;
        let drive = windows_directory_chain(Path::new(r"C:\repo\child")).unwrap();
        assert_eq!(drive.first().unwrap(), Path::new(r"C:\"));
        let unc = windows_directory_chain(Path::new(r"\\server\share\repo")).unwrap();
        assert_eq!(unc.first().unwrap(), Path::new(r"\\server\share\"));
        let verbatim = Path::new(r"\\?\C:\repo\crates\xtask\..\..");
        let normalized = windows_directory_chain(verbatim).unwrap();
        assert_eq!(normalized.last().unwrap(), Path::new(r"\\?\C:\repo"));
        assert_eq!(normalized.len(), 2);
    }

    fn held_leaf_handle_blocks_write_delete_and_replacement_until_read_returns(
        root: &std::path::Path,
    ) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = root.join(format!("ssx-bounded-leaf-{nonce}"));
        let parent = base.join("parent");
        fs::create_dir_all(&parent).unwrap();
        let leaf = parent.join("leaf");
        let replacement = parent.join("replacement");
        fs::write(&leaf, b"original bytes").unwrap();
        fs::write(&replacement, b"replacement bytes").unwrap();

        let result = bounded_read_inner(&base, "parent/leaf", 64, || {
            assert!(
                fs::write(&leaf, b"mutated bytes").is_err(),
                "held read handle must deny a concurrent write open"
            );
            assert!(
                fs::remove_file(&leaf).is_err(),
                "held read handle must deny leaf deletion"
            );
            assert!(
                fs::rename(&leaf, parent.join("renamed-leaf")).is_err(),
                "held read handle must deny leaf rename"
            );
            assert!(
                fs::rename(&replacement, &leaf).is_err(),
                "held read handle must deny leaf replacement"
            );
        })
        .unwrap();
        assert_eq!(result.0, b"original bytes");

        fs::write(&leaf, b"released").expect("write is permitted after handle release");
        fs::remove_file(&leaf).expect("delete is permitted after handle release");
        fs::rename(&replacement, &leaf).expect("replacement is permitted after handle release");
        assert_eq!(fs::read(&leaf).unwrap(), b"replacement bytes");
        fs::remove_dir_all(base).unwrap();
    }

    fn parent_reparse_point_is_rejected_when_symlink_creation_is_available(root: &std::path::Path) {
        use std::os::windows::fs::symlink_dir;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = root.join(format!("ssx-bounded-reparse-{nonce}"));
        let outside = base.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("leaf"), b"outside bytes").unwrap();
        if symlink_dir(&outside, base.join("linked-parent")).is_ok() {
            assert!(bounded_read_inner(&base, "linked-parent/leaf", 64, || {}).is_err());
        }
        fs::remove_dir_all(base).unwrap();
    }

    fn held_parent_handle_blocks_rename_during_leaf_read(root: &std::path::Path) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = root.join(format!("ssx-bounded-parent-{nonce}"));
        let parent = base.join("parent");
        fs::create_dir_all(&parent).unwrap();
        fs::write(parent.join("leaf"), b"bounded windows bytes").unwrap();
        let moved_root = base.with_extension("moved");
        let result = bounded_read_inner(&base, "parent/leaf", 64, || {
            assert!(
                fs::rename(&base, &moved_root).is_err(),
                "held root handle must block root rename while leaf is open"
            );
            assert!(
                fs::rename(&parent, base.join("renamed-parent")).is_err(),
                "read-only sharing handle must block parent rename while leaf is open"
            );
        })
        .unwrap();
        assert_eq!(result.0, b"bounded windows bytes");
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(all(test, unix))]
#[allow(
    dead_code,
    reason = "Only the T08A test owner calls these controls; other suites include the reader by path"
)]
mod bounded_read_tests {
    use super::{bounded_read_inner, bounded_read_with_hooks};
    use std::{
        fs,
        os::unix::fs::symlink,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn fixture(root: &std::path::Path) -> PathBuf {
        let dir = root.join(format!(
            "ssx-bound-read-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).expect("fresh bounded-read fixture directory");
        dir
    }

    pub(super) fn controls(root: &std::path::Path) {
        let dir = fixture(root);
        let valid = dir.join("valid");
        fs::write(&valid, b"source bytes").unwrap();
        assert_eq!(
            bounded_read_inner(&dir, "valid", 12, || {}).unwrap().0,
            b"source bytes"
        );

        let large = dir.join("large");
        fs::write(&large, b"larger than bound").unwrap();
        assert!(
            bounded_read_inner(&dir, "large", 3, || {}).is_err(),
            "oversize source rejected"
        );

        symlink(&valid, dir.join("link")).unwrap();
        assert!(
            bounded_read_inner(&dir, "link", 64, || {}).is_err(),
            "leaf symlink rejected"
        );
        symlink(&dir, dir.join("parent-link")).unwrap();
        assert!(
            bounded_read_inner(&dir, "parent-link/valid", 64, || {}).is_err(),
            "symlinked parent rejected"
        );

        let replaced = dir.join("replaced");
        let alternate = dir.join("alternate");
        fs::write(&replaced, b"original").unwrap();
        fs::write(&alternate, b"different").unwrap();
        assert!(
            bounded_read_inner(&dir, "replaced", 64, || {
                fs::rename(&alternate, dir.join("replacement")).unwrap();
                fs::rename(dir.join("replacement"), &replaced).unwrap();
            })
            .is_err(),
            "replacement after open rejected by name/descriptor identity check"
        );

        let mutated = dir.join("mutated");
        fs::write(&mutated, b"original bytes").unwrap();
        assert!(
            bounded_read_inner(&dir, "mutated", 64, || {
                fs::write(&mutated, b"short").unwrap();
            })
            .is_err(),
            "in-place mutation after open rejected by post-read metadata check"
        );

        let equal_size = dir.join("equal-size");
        fs::write(&equal_size, b"before").unwrap();
        assert!(
            bounded_read_with_hooks(
                &dir,
                "equal-size",
                64,
                || {},
                || {
                    fs::write(&equal_size, b"rewrit").unwrap();
                }
            )
            .is_err(),
            "equal-size rewrite after the first read must invalidate custody"
        );

        let parent = dir.join("parent");
        let outside = dir.join("outside");
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(parent.join("leaf"), b"parent bytes").unwrap();
        fs::write(outside.join("leaf"), b"outside bytes").unwrap();
        assert!(
            bounded_read_inner(&dir, "parent/leaf", 64, || {
                fs::rename(&parent, dir.join("moved-parent")).unwrap();
                symlink(&outside, &parent).unwrap();
            })
            .is_err(),
            "parent replacement after open rejected by no-follow root-relative recheck"
        );

        let reopened = dir.join("reopened");
        let next = dir.join("next-parent");
        fs::create_dir(&reopened).unwrap();
        fs::create_dir(&next).unwrap();
        fs::write(reopened.join("leaf"), b"same bytes").unwrap();
        fs::write(next.join("leaf"), b"same bytes").unwrap();
        let error = bounded_read_inner(&dir, "reopened/leaf", 64, || {
            fs::rename(&reopened, dir.join("old-parent")).unwrap();
            fs::rename(&next, &reopened).unwrap();
        })
        .unwrap_err();
        assert_eq!(
            error,
            "reopened source identity or size changed before verification read"
        );

        let checkout = dir.join("checkout");
        let replacement = dir.join("next-checkout");
        fs::create_dir(&checkout).unwrap();
        fs::create_dir(&replacement).unwrap();
        fs::write(checkout.join("leaf"), b"admitted checkout").unwrap();
        fs::write(replacement.join("leaf"), b"unintended bytes").unwrap();
        symlink(&checkout, dir.join("linked-checkout")).unwrap();
        assert!(bounded_read_inner(&dir.join("linked-checkout"), "leaf", 64, || {}).is_err());
        let linked_ancestor = dir.join("linked-ancestor");
        symlink(&dir, &linked_ancestor).unwrap();
        assert!(bounded_read_inner(&linked_ancestor.join("checkout"), "leaf", 64, || {}).is_err());
        assert_eq!(
            bounded_read_inner(&checkout.join("../checkout"), "leaf", 64, || {})
                .unwrap()
                .0,
            b"admitted checkout"
        );
        let error = bounded_read_inner(&checkout, "leaf", 64, || {
            fs::rename(&checkout, dir.join("old-checkout")).unwrap();
            fs::rename(&replacement, &checkout).unwrap();
        })
        .unwrap_err();
        assert_eq!(error, "source root identity changed during bounded read");

        let mode_changed = dir.join("mode-changed");
        fs::write(&mode_changed, b"same length").unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&mode_changed, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            bounded_read_inner(&dir, "mode-changed", 64, || {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&mode_changed, fs::Permissions::from_mode(0o600)).unwrap();
            })
            .is_err(),
            "mode mutation after open rejected by post-read metadata check"
        );

        fs::remove_dir_all(&dir).unwrap();
    }
}
