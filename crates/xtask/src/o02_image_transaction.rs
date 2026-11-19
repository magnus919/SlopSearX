//! Owned, bounded recovery journal. Callers hold an exclusive advisory root lock.
//! Readers using this helper never observe our intermediate per-file renames.
use super::super::{PROMOTION_FILES, PinText, PromotionPlan, promotion_plan};
use super::{ApplyPoint, open, parent, read};
use nix::{
    fcntl::{AtFlags, OFlag},
    sys::stat::{Mode, fstatat, mkdirat},
    unistd::{UnlinkatFlags, unlinkat},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, Metadata, Permissions},
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::Path,
};
const DIRECTORY: &str = ".ssx-image-promotion";
const JOURNAL: &str = "journal.json";
const PENDING: &str = "journal.pending";
const COMMITTED: &str = "committed";
const COMMIT_PENDING: &str = "commit.pending";
const CLEANUP: &str = ".ssx-image-promotion-cleanup.json";
const CLEANUP_PENDING: &str = ".ssx-image-promotion-cleanup.pending";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CleanupRecord {
    schema: u32,
    device: u64,
    inode: u64,
    outputs: bool,
    journal: String,
}

const MAX_JOURNAL: u64 = 4 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema: u32,
    latest: String,
    documents: Vec<Document>,
    stages: Vec<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    path: String,
    original: String,
    updated: String,
}
fn named_identity(parent: &File, name: &str, metadata: &std::fs::Metadata) -> io::Result<()> {
    let stat = fstatat(parent, name, AtFlags::AT_SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
    if i128::from(stat.st_dev) != i128::from(metadata.dev())
        || stat.st_ino != metadata.ino()
        || i128::from(stat.st_mode) != i128::from(metadata.mode())
    {
        return Err(fail());
    }
    Ok(())
}

fn fail() -> io::Error {
    io::Error::other(
        "O02 owned promotion journal validation/recovery failed; retain .ssx-image-promotion and restore its exact original operands before retry",
    )
}
fn directory(root: &File) -> io::Result<File> {
    let dir = open(
        root,
        DIRECTORY,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY,
        Mode::empty(),
    )?;
    let meta = dir.metadata()?;
    if meta.mode() & 0o7777 != 0o700 || meta.uid() != unsafe { nix::libc::geteuid() } {
        return Err(fail());
    }
    Ok(dir)
}
/// The root descriptor lock is held until the returned file is dropped.
fn lock_root(path: &Path) -> Result<File, String> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    use std::os::unix::fs::OpenOptionsExt;
    let root = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)
        .map_err(|e| e.to_string())?;
    // CLOEXEC protects launched children, but a concurrent fork can retain
    // this descriptor briefly before exec closes it. Bound that transient
    // contention without waiting indefinitely for another native operation.
    let mut acquired = false;
    for attempt in 0..=50 {
        if unsafe { nix::libc::flock(root.as_raw_fd(), nix::libc::LOCK_EX | nix::libc::LOCK_NB) }
            == 0
        {
            acquired = true;
            break;
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::WouldBlock || attempt == 50 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    if !acquired {
        return Err("O02 image pin: another check/write holds the root lease".into());
    }
    Ok(root)
}

pub(crate) fn read_guard(path: &Path) -> Result<File, String> {
    let root = lock_root(path)?;
    for name in [CLEANUP, CLEANUP_PENDING] {
        match open(&root, name, OFlag::O_RDONLY, Mode::empty()) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            _ => return Err("O02 image pin: pending transaction cleanup; run --recover".into()),
        }
    }
    match directory(&root) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(root),
        _ => Err(
            "O02 image pin: pending transaction; run image-pin-promote --recover before check"
                .into(),
        ),
    }
}

pub(crate) fn recover(path: &Path) -> Result<File, String> {
    let root = lock_root(path)?;
    if resume_cleanup(&root).map_err(|e| e.to_string())? {
        clean_initializations(&root).map_err(|e| e.to_string())?;
        return Ok(root);
    }
    match directory(&root) {
        Ok(dir) => match journal_plan(&dir, COMMITTED) {
            Ok(_) => complete(&root).map_err(|e| e.to_string())?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                rollback_locked(&root, |_| Ok(())).map_err(|e| e.to_string())?
            }
            Err(e) => return Err(e.to_string()),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    clean_initializations(&root).map_err(|e| e.to_string())?;
    Ok(root)
}
fn owned_record(dir: &File, name: &str) -> io::Result<Vec<u8>> {
    let file = open(dir, name, OFlag::O_RDONLY, Mode::empty())?;
    let meta = file.metadata()?;
    if !meta.is_file()
        || meta.mode() & 0o7777 != 0o600
        || meta.uid() != unsafe { nix::libc::geteuid() }
        || meta.nlink() != 1
        || meta.len() > MAX_JOURNAL
    {
        return Err(fail());
    }
    let mut raw = Vec::new();
    (&file).take(MAX_JOURNAL + 1).read_to_end(&mut raw)?;
    let after = file.metadata()?;
    named_identity(dir, name, &after)?;
    if meta.dev() != after.dev()
        || meta.ino() != after.ino()
        || meta.len() != after.len()
        || meta.mode() != after.mode()
        || meta.uid() != after.uid()
        || meta.nlink() != after.nlink()
        || (
            meta.mtime(),
            meta.mtime_nsec(),
            meta.ctime(),
            meta.ctime_nsec(),
        ) != (
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec(),
        )
    {
        return Err(fail());
    }
    if raw.len() as u64 != meta.len() {
        return Err(fail());
    }
    Ok(raw)
}
fn journal_plan(dir: &File, name: &str) -> io::Result<(PromotionPlan, Vec<String>)> {
    plan_bytes(&owned_record(dir, name)?)
}
fn plan_bytes(raw: &[u8]) -> io::Result<(PromotionPlan, Vec<String>)> {
    let journal: Journal = serde_json::from_slice(raw).map_err(|_| fail())?;
    if journal.schema != 1
        || journal.documents.len() != PROMOTION_FILES.len()
        || journal.stages.len() != PROMOTION_FILES.len()
        || journal.stages.iter().any(|name| {
            !name.starts_with(".ssx-image-promotion-")
                || !name.ends_with(".tmp")
                || name.contains('/')
                || name.contains('\0')
        })
    {
        return Err(fail());
    }
    let mut inputs = Vec::new();
    for (document, path) in journal.documents.iter().zip(PROMOTION_FILES) {
        if document.path != path {
            return Err(fail());
        }
        inputs.push(PinText {
            path,
            bytes: document.original.as_bytes().to_vec(),
        });
    }
    let plan = promotion_plan(&inputs, Some(&journal.latest)).map_err(|_| fail())?;
    if journal
        .documents
        .iter()
        .zip(&plan.outputs)
        .any(|(doc, output)| doc.updated.as_bytes() != output.bytes)
    {
        return Err(fail());
    }
    Ok((plan, journal.stages))
}
fn draft_name(stage: &str) -> String {
    format!("{stage}.draft")
}

pub(super) fn publish_stage(
    root: &File,
    destination: &File,
    stage: &str,
    bytes: &[u8],
    after_create: impl FnOnce(&File, &str) -> io::Result<()>,
) -> io::Result<Metadata> {
    let dir = directory(root)?;
    let draft = draft_name(stage);
    let mut file = open(
        &dir,
        &draft,
        OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL,
        Mode::S_IRUSR | Mode::S_IWUSR,
    )?;
    after_create(&dir, &draft)?;
    file.write_all(bytes)?;
    file.set_permissions(Permissions::from_mode(0o644))?;
    file.sync_all()?;
    let metadata = file.metadata()?;
    named_identity(&dir, &draft, &metadata)?;
    publish_no_replace(&dir, &draft, destination, stage)?;
    destination.sync_all()?;
    dir.sync_all()?;
    Ok(metadata)
}

pub(super) fn begin(
    root: &File,
    plan: &PromotionPlan,
    stages: &[String],
    mut hook: impl FnMut(ApplyPoint) -> io::Result<()>,
) -> io::Result<()> {
    for (index, old) in plan.originals.iter().enumerate() {
        let parent = parent(root, old.path)?;
        match open(&parent, &stages[index], OFlag::O_RDONLY, Mode::empty()) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            _ => return Err(fail()),
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| fail())?
        .as_nanos();
    let initial = format!(".ssx-image-promotion-init-{}-{stamp}", std::process::id());
    mkdirat(root, initial.as_str(), Mode::S_IRWXU).map_err(io::Error::from)?;
    let dir = open(
        root,
        &initial,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY,
        Mode::empty(),
    )?;
    hook(ApplyPoint::InitCreated)?;
    let journal = Journal {
        schema: 1,
        stages: stages.to_vec(),
        latest: plan.latest.clone(),
        documents: plan
            .originals
            .iter()
            .zip(&plan.outputs)
            .map(|(old, new)| {
                Ok(Document {
                    path: old.path.into(),
                    original: String::from_utf8(old.bytes.clone()).map_err(|_| fail())?,
                    updated: String::from_utf8(new.bytes.clone()).map_err(|_| fail())?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?,
    };
    let bytes = serde_json::to_vec(&journal).map_err(|_| fail())?;
    if bytes.len() as u64 > MAX_JOURNAL {
        return Err(fail());
    }
    let mut file = open(
        &dir,
        PENDING,
        OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL,
        Mode::S_IRUSR | Mode::S_IWUSR,
    )?;
    file.write_all(&bytes)?;
    hook(ApplyPoint::InitWritten)?;
    file.sync_all()?;
    hook(ApplyPoint::InitSynced)?;
    hook(ApplyPoint::JournalBeforePublish)?;
    publish_no_replace(&dir, PENDING, &dir, JOURNAL)?;
    dir.sync_all()?;
    root.sync_all()?;
    hook(ApplyPoint::InitBeforePublish)?;
    publish_no_replace(root, &initial, root, DIRECTORY)?;
    hook(ApplyPoint::InitRenamed)?;
    root.sync_all()?;
    hook(ApplyPoint::InitPublished)?;
    Ok(())
}
fn unlink_owned(dir: &File, name: &str, mode: u32) -> io::Result<()> {
    let file = match open(dir, name, OFlag::O_RDONLY, Mode::empty()) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let m = file.metadata()?;
    if !m.is_file()
        || m.uid() != unsafe { nix::libc::geteuid() }
        || m.mode() & 0o7777 != mode
        || m.nlink() != 1
    {
        return Err(fail());
    }
    named_identity(dir, name, &m)?;
    unlinkat(dir, name, UnlinkatFlags::NoRemoveDir).map_err(io::Error::from)
}
// Inspect by descriptor. Unknown entries and invalid known records are never
// removed merely because a directory has the reserved name or caller's UID.
fn duplicate_stream_fd(dir: &File) -> io::Result<i32> {
    let fd = unsafe { nix::libc::fcntl(dir.as_raw_fd(), nix::libc::F_DUPFD_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { nix::libc::fcntl(fd, nix::libc::F_GETFD) } & nix::libc::FD_CLOEXEC == 0 {
        unsafe { nix::libc::close(fd) };
        return Err(fail());
    }
    Ok(fd)
}
fn entries_with_limit(
    dir: &File,
    limit: usize,
) -> io::Result<(std::collections::BTreeSet<String>, bool)> {
    let fd = duplicate_stream_fd(dir)?;
    let stream = unsafe { nix::libc::fdopendir(fd) };
    if stream.is_null() {
        unsafe { nix::libc::close(fd) };
        return Err(io::Error::last_os_error());
    }
    let result = (|| {
        let mut names = std::collections::BTreeSet::new();
        loop {
            let entry = unsafe { nix::libc::readdir(stream) };
            if entry.is_null() {
                break;
            }
            let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_str()
                .map_err(|_| fail())?;
            if name != "." && name != ".." {
                if names.len() == limit {
                    return Ok((names, true));
                }
                names.insert(name.to_string());
            }
        }
        Ok((names, false))
    })();
    unsafe { nix::libc::closedir(stream) };
    result
}
fn entries(dir: &File) -> io::Result<std::collections::BTreeSet<String>> {
    let (names, truncated) = entries_with_limit(dir, 4)?;
    if truncated {
        return Err(fail());
    }
    Ok(names)
}
fn publish_no_replace(
    old_parent: &File,
    old_name: &str,
    new_parent: &File,
    new_name: &str,
) -> io::Result<()> {
    rename_with_flags(old_parent, old_name, new_parent, new_name, false)
}

/// Both names must exist. The displaced entry remains available for verification.
pub(super) fn exchange(parent: &File, stage: &str, source: &str) -> io::Result<()> {
    rename_with_flags(parent, stage, parent, source, true)
}

fn rename_with_flags(
    old_parent: &File,
    old_name: &str,
    new_parent: &File,
    new_name: &str,
    exchange: bool,
) -> io::Result<()> {
    let old = std::ffi::CString::new(old_name).map_err(|_| fail())?;
    let new = std::ffi::CString::new(new_name).map_err(|_| fail())?;
    #[cfg(target_os = "macos")]
    let result = unsafe {
        nix::libc::renameatx_np(
            old_parent.as_raw_fd(),
            old.as_ptr(),
            new_parent.as_raw_fd(),
            new.as_ptr(),
            if exchange {
                nix::libc::RENAME_SWAP
            } else {
                nix::libc::RENAME_EXCL
            },
        )
    };
    #[cfg(target_os = "linux")]
    let result = unsafe {
        nix::libc::syscall(
            nix::libc::SYS_renameat2,
            old_parent.as_raw_fd(),
            old.as_ptr(),
            new_parent.as_raw_fd(),
            new.as_ptr(),
            if exchange {
                nix::libc::RENAME_EXCHANGE
            } else {
                nix::libc::RENAME_NOREPLACE
            },
        )
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "exclusive transaction publication requires Linux or macOS",
    ));
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    if result != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn clean_initializations(root: &File) -> io::Result<()> {
    // This scan only finds cleanup candidates. Recovery of the active
    // journal uses its exact names and must not depend on inert root entries.
    let (names, truncated) = entries_with_limit(root, 4096)?;
    if truncated {
        eprintln!("O02 image pin: initialization cleanup scan reached its entry limit");
    }
    for name in names {
        if !name.starts_with(".ssx-image-promotion-init-") {
            continue;
        }
        let result = (|| -> io::Result<()> {
            let dir = open(
                root,
                &name,
                OFlag::O_RDONLY | OFlag::O_DIRECTORY,
                Mode::empty(),
            )?;
            let meta = dir.metadata()?;
            if meta.mode() & 0o7777 != 0o700 || meta.uid() != unsafe { nix::libc::geteuid() } {
                return Err(fail());
            }
            let names = entries(&dir)?;
            if names.is_empty()
                || names
                    .iter()
                    .any(|n| ![JOURNAL, PENDING].contains(&n.as_str()))
            {
                return Err(fail());
            }
            let mut prior: Option<PromotionPlan> = None;
            for n in &names {
                let (plan, _) = journal_plan(&dir, n)?;
                if let Some(old) = &prior {
                    if old.originals != plan.originals || old.outputs != plan.outputs {
                        return Err(fail());
                    }
                } else {
                    prior = Some(plan);
                }
            }
            for n in names {
                unlink_owned(&dir, &n, 0o600)?;
            }
            dir.sync_all()?;
            named_identity(root, &name, &meta)?;
            unlinkat(root, name.as_str(), UnlinkatFlags::RemoveDir).map_err(io::Error::from)?;
            root.sync_all()
        })();
        if let Err(error) = result {
            eprintln!("O02 image pin: retained inert unpublished initialization {name}: {error}");
        }
    }
    Ok(())
}
fn verify_endpoint(root: &File, plan: &PromotionPlan, outputs: bool) -> io::Result<()> {
    let documents = if outputs {
        &plan.outputs
    } else {
        &plan.originals
    };
    for document in documents {
        let parent = parent(root, document.path)?;
        let name = document.path.rsplit('/').next().ok_or_else(fail)?;
        if read(&parent, name)?.1 != document.bytes {
            return Err(fail());
        }
    }
    Ok(())
}
fn unlink_partial(dir: &File, name: &str, expected: &[u8]) -> io::Result<()> {
    match owned_record(dir, name) {
        Ok(raw) => {
            if !expected.starts_with(&raw) {
                return Err(fail());
            }
            unlink_owned(dir, name, 0o600)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
fn finish_cleanup(
    root: &File,
    record: &CleanupRecord,
    mut hook: impl FnMut(ApplyPoint) -> io::Result<()>,
) -> io::Result<()> {
    if record.schema != 1 {
        return Err(fail());
    }
    let (plan, _) = plan_bytes(record.journal.as_bytes())?;
    verify_endpoint(root, &plan, record.outputs)?;
    match directory(root) {
        Ok(dir) => {
            let meta = dir.metadata()?;
            if meta.dev() != record.device || meta.ino() != record.inode {
                return Err(fail());
            }
            let names = entries(&dir)?;
            if names
                .iter()
                .any(|name| ![JOURNAL, PENDING, COMMITTED, COMMIT_PENDING].contains(&name.as_str()))
            {
                return Err(fail());
            }
            for name in &names {
                if name == COMMIT_PENDING {
                    continue;
                }
                let (other, _) = journal_plan(&dir, name)?;
                if other.originals != plan.originals || other.outputs != plan.outputs {
                    return Err(fail());
                }
            }
            unlink_partial(&dir, COMMIT_PENDING, record.journal.as_bytes())?;
            for name in [JOURNAL, PENDING, COMMITTED] {
                unlink_owned(&dir, name, 0o600)?;
            }
            dir.sync_all()?;
            hook(ApplyPoint::CleanupMarkersRemoved)?;
            named_identity(root, DIRECTORY, &meta)?;
            unlinkat(root, DIRECTORY, UnlinkatFlags::RemoveDir).map_err(io::Error::from)?;
            root.sync_all()?;
            hook(ApplyPoint::CleanupDirectoryRemoved)?;
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    unlink_owned(root, CLEANUP, 0o600)?;
    root.sync_all()
}
fn resume_cleanup(root: &File) -> io::Result<bool> {
    match owned_record(root, CLEANUP) {
        Ok(raw) => {
            let record: CleanupRecord = serde_json::from_slice(&raw).map_err(|_| fail())?;
            finish_cleanup(root, &record, |_| Ok(()))?;
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // A pending cleanup receipt needs the still-valid transaction to
            // reconstruct and authenticate its exact expected prefix.
            if directory(root).is_err() {
                match open(root, CLEANUP_PENDING, OFlag::O_RDONLY, Mode::empty()) {
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    _ => return Err(fail()),
                }
            }
            Ok(false)
        }
        Err(e) => Err(e),
    }
}
pub(super) fn complete(root: &File) -> io::Result<()> {
    complete_with_hook(root, |_| Ok(()))
}
pub(super) fn complete_with_hook(
    root: &File,
    mut hook: impl FnMut(ApplyPoint) -> io::Result<()>,
) -> io::Result<()> {
    let dir = directory(root)?;
    let names = entries(&dir)?;
    if names.is_empty() {
        return Err(fail());
    }
    let name = if names.contains(COMMITTED) {
        COMMITTED
    } else if names.contains(JOURNAL) {
        JOURNAL
    } else {
        PENDING
    };
    let raw = owned_record(&dir, name)?;
    let (plan, stages) = plan_bytes(&raw)?;
    let outputs = name == COMMITTED;
    let mut drafts = Vec::new();
    for name in &names {
        if name == COMMIT_PENDING {
            continue;
        }
        if !outputs && let Some(index) = stages.iter().position(|stage| draft_name(stage) == *name)
        {
            let expected = &plan.outputs[index].bytes;
            if stage_is_owned(&dir, name, expected, StageContent::Draft)? {
                drafts.push(name);
            }
            continue;
        }
        if ![JOURNAL, PENDING, COMMITTED].contains(&name.as_str()) {
            return Err(fail());
        }
        let (other, other_stages) = journal_plan(&dir, name)?;
        if other.originals != plan.originals
            || other.outputs != plan.outputs
            || other_stages != stages
        {
            return Err(fail());
        }
    }
    verify_endpoint(root, &plan, outputs)?;
    for draft in drafts {
        unlinkat(&dir, draft.as_str(), UnlinkatFlags::NoRemoveDir).map_err(io::Error::from)?;
    }
    dir.sync_all()?;
    let documents = if outputs {
        &plan.originals
    } else {
        &plan.outputs
    };
    for (document, stage) in documents.iter().zip(&stages) {
        let parent = parent(root, document.path)?;
        if stage_is_owned(&parent, stage, &document.bytes, StageContent::Exact)? {
            unlinkat(&parent, stage.as_str(), UnlinkatFlags::NoRemoveDir)
                .map_err(io::Error::from)?;
            parent.sync_all()?;
        }
    }
    let meta = dir.metadata()?;
    let record = CleanupRecord {
        schema: 1,
        device: meta.dev(),
        inode: meta.ino(),
        outputs,
        journal: String::from_utf8(raw).map_err(|_| fail())?,
    };
    let bytes = serde_json::to_vec(&record).map_err(|_| fail())?;
    if bytes.len() as u64 > MAX_JOURNAL {
        return Err(fail());
    }
    unlink_partial(root, CLEANUP_PENDING, &bytes)?;
    let mut file = open(
        root,
        CLEANUP_PENDING,
        OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL,
        Mode::S_IRUSR | Mode::S_IWUSR,
    )?;
    hook(ApplyPoint::CleanupReceiptCreated)?;
    file.write_all(&bytes)?;
    hook(ApplyPoint::CleanupReceiptWritten)?;
    file.sync_all()?;
    hook(ApplyPoint::CleanupReceiptSynced)?;
    hook(ApplyPoint::CleanupBeforePublish)?;
    publish_no_replace(root, CLEANUP_PENDING, root, CLEANUP)?;
    root.sync_all()?;
    hook(ApplyPoint::CleanupReceiptPublished)?;
    finish_cleanup(root, &record, hook)
}
/// Restart recovery deliberately returns to originals, including after the last
/// replacement if journal retirement had not completed. Unknown external edits
/// are never overwritten; the journal remains for an explicit repair/retry.
pub(super) fn rollback_locked(
    root: &File,
    mut hook: impl FnMut(usize) -> io::Result<()>,
) -> io::Result<()> {
    let dir = match directory(root) {
        Ok(dir) => dir,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let (plan, stages) = match journal_plan(&dir, JOURNAL) {
        Ok(plan) => plan,
        Err(e) if e.kind() == io::ErrorKind::NotFound => journal_plan(&dir, PENDING)?,
        Err(e) => return Err(e),
    };
    // Validate every pair before starting compensation.
    for ((old, new), stage) in plan.originals.iter().zip(&plan.outputs).zip(&stages) {
        let parent = parent(root, old.path)?;
        let name = old.path.rsplit('/').next().ok_or_else(fail)?;
        let (_, current) = read(&parent, name)?;
        if current == old.bytes {
            stage_is_owned(&parent, stage, &new.bytes, StageContent::Exact)?;
        } else if current != new.bytes || read(&parent, stage)?.1 != old.bytes {
            return Err(fail());
        }
    }
    // Published commit authority is never revoked by compensation.
    match journal_plan(&dir, COMMITTED) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        _ => return Err(fail()),
    }
    for (index, (old, new)) in plan.originals.iter().zip(&plan.outputs).enumerate().rev() {
        let parent = parent(root, old.path)?;
        let name = old.path.rsplit('/').next().ok_or_else(fail)?;
        let (_, current) = read(&parent, name)?;
        if current == old.bytes {
            continue;
        }
        hook(index)?;
        let stage = &stages[index];
        let (installed, current) = read(&parent, name)?;
        let (displaced, original) = read(&parent, stage)?;
        if current != new.bytes || original != old.bytes {
            return Err(fail());
        }
        exchange(&parent, stage, name)?;
        let restored = read(&parent, name).is_ok_and(|(after, bytes)| {
            after.matches_after_exchange(&displaced) && bytes == old.bytes
        });
        let staged = read(&parent, stage).is_ok_and(|(after, bytes)| {
            after.matches_after_exchange(&installed) && bytes == new.bytes
        });
        if !restored || !staged {
            if restored || staged {
                let _ = exchange(&parent, stage, name);
            }
            parent.sync_all()?;
            return Err(fail());
        }
        parent.sync_all()?;
    }
    unlink_partial(
        &dir,
        COMMIT_PENDING,
        &owned_record(&dir, JOURNAL).or_else(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                owned_record(&dir, PENDING)
            } else {
                Err(e)
            }
        })?,
    )?;
    complete(root)
}

pub(super) struct CommitFailure {
    pub(super) published: bool,
}
pub(super) fn commit(
    root: &File,
    mut hook: impl FnMut(ApplyPoint) -> io::Result<()>,
) -> Result<(), CommitFailure> {
    let mut published = false;
    let result = (|| -> io::Result<()> {
        let dir = directory(root)?;
        let (plan, stages) = journal_plan(&dir, JOURNAL)?;
        verify_endpoint(root, &plan, true)?;
        for (old, stage) in plan.originals.iter().zip(&stages) {
            let parent = parent(root, old.path)?;
            if !stage_is_owned(&parent, stage, &old.bytes, StageContent::Exact)? {
                return Err(fail());
            }
        }
        let mut raw = Vec::new();
        open(&dir, JOURNAL, OFlag::O_RDONLY, Mode::empty())?
            .take(MAX_JOURNAL + 1)
            .read_to_end(&mut raw)?;
        if raw.len() as u64 > MAX_JOURNAL {
            return Err(fail());
        }
        let mut file = open(
            &dir,
            COMMIT_PENDING,
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )?;
        hook(ApplyPoint::CommitCreated)?;
        file.write_all(&raw)?;
        hook(ApplyPoint::CommitWritten)?;
        file.sync_all()?;
        hook(ApplyPoint::CommitSynced)?;
        hook(ApplyPoint::CommitBeforePublish)?;
        publish_no_replace(&dir, COMMIT_PENDING, &dir, COMMITTED)?;
        published = true;
        hook(ApplyPoint::CommitRenamed)?;
        dir.sync_all()
    })();
    result.map_err(|_| CommitFailure { published })
}

enum StageContent {
    Exact,
    Draft,
}

fn stage_is_owned(
    parent: &File,
    name: &str,
    expected: &[u8],
    content: StageContent,
) -> io::Result<bool> {
    let file = match open(parent, name, OFlag::O_RDONLY, Mode::empty()) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let before = file.metadata()?;
    if !before.is_file()
        || before.uid() != unsafe { nix::libc::geteuid() }
        || before.nlink() != 1
        || ![0o600, 0o644].contains(&(before.mode() & 0o7777))
        || (before.len() == 0
            && (!matches!(content, StageContent::Draft) || before.mode() & 0o7777 != 0o600))
        || before.len() > expected.len() as u64
    {
        return Err(fail());
    }
    let mut bytes = Vec::new();
    (&file)
        .take(expected.len() as u64 + 1)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if bytes.len() as u64 != before.len()
        || (matches!(content, StageContent::Exact) && bytes != expected)
        || !expected.starts_with(&bytes)
        || before.len() != after.len()
        || before.mode() != after.mode()
        || before.nlink() != after.nlink()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(fail());
    }
    named_identity(parent, name, &after)?;
    Ok(true)
}

#[cfg(test)]
mod lease_controls {
    use super::*;
    use std::os::fd::FromRawFd;
    #[test]
    fn directory_stream_cloexec_prevents_inherited_root_leases() {
        let temp = std::path::PathBuf::from(std::env::var_os("TMPDIR").unwrap());
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = temp.join(format!("o02-lease-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        for close_on_exec in [false, true] {
            let root = lock_root(&path).unwrap();
            let fd = duplicate_stream_fd(&root).unwrap();
            if !close_on_exec {
                assert_eq!(unsafe { nix::libc::fcntl(fd, nix::libc::F_SETFD, 0) }, 0);
            }
            let duplicate = unsafe { File::from_raw_fd(fd) };
            let mut child = std::process::Command::new("sh")
                .args(["-c", "sleep 0.3"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            drop(duplicate);
            drop(root);
            let acquired = lock_root(&path);
            assert_eq!(
                acquired.is_ok(),
                close_on_exec,
                "negative control must reproduce an inherited lease"
            );
            drop(acquired);
            assert!(child.wait().unwrap().success());
            drop(lock_root(&path).unwrap());
        }
        std::fs::remove_dir(&path).unwrap();
    }
}
