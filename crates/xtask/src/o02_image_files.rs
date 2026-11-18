//! Four-file Unix transaction with durable originals and restart recovery.
#[path = "o02_image_transaction.rs"]
mod transaction;
use super::{ApplyError, MAX_PIN_INPUT_BYTES, PROMOTION_FILES, PinText, PromotionPlan};
pub(super) use transaction::{read_guard, recover};
const FILE_BYTES: usize = MAX_PIN_INPUT_BYTES as usize;
const PIN_PATHS: [&str; 4] = PROMOTION_FILES;
use nix::{
    fcntl::{AtFlags, OFlag, openat},
    sys::stat::{Mode, fstatat},
    unistd::{UnlinkatFlags, unlinkat},
};
use std::{
    fs::{self, File, Metadata, Permissions},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
#[derive(Clone, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
    bytes: u64,
    mode: u32,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl Identity {
    fn matches_after_exchange(&self, before: &Self) -> bool {
        // Exchanging a directory entry may change its inode's ctime.
        self.dev == before.dev
            && self.ino == before.ino
            && self.bytes == before.bytes
            && self.mode == before.mode
            && self.modified == before.modified
    }

    fn of(meta: &Metadata) -> Self {
        Self {
            dev: meta.dev(),
            ino: meta.ino(),
            bytes: meta.len(),
            mode: meta.mode(),
            modified: (meta.mtime(), meta.mtime_nsec()),
            changed: (meta.ctime(), meta.ctime_nsec()),
        }
    }
}
struct Source {
    relative: &'static str,
    name: String,
    parent: File,
    identity: Identity,
    bytes: Vec<u8>,
}
pub struct PinSnapshot {
    root_path: PathBuf,
    root: File,
    sources: Vec<Source>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ApplyPoint {
    BeforeStage,
    BeforeReplace,
    ReadyToRename,
    BeforeRename,
    AfterExchange,
    BeforeRollback,
    BeforeCommit,
    AfterCommit,
    InitCreated,
    InitBeforePublish,
    JournalBeforePublish,
    CommitBeforePublish,
    CleanupBeforePublish,
    InitWritten,
    InitSynced,
    InitPublished,
    InitRenamed,
    CommitCreated,
    CommitWritten,
    CommitSynced,
    CommitRenamed,
    CleanupReceiptPublished,
    CleanupReceiptCreated,
    CleanupReceiptWritten,
    CleanupReceiptSynced,
    CleanupMarkersRemoved,
    CleanupDirectoryRemoved,
}
fn err() -> io::Error {
    io::Error::other("fixed pin source identity or bounds changed")
}
fn open(parent: &File, name: &str, flags: OFlag, mode: Mode) -> io::Result<File> {
    openat(
        parent,
        name,
        flags | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK,
        mode,
    )
    .map(File::from)
    .map_err(io::Error::from)
}
fn parent(root: &File, relative: &str) -> io::Result<File> {
    match relative.rsplit_once('/') {
        Some((directory, _)) => open(
            root,
            directory,
            OFlag::O_RDONLY | OFlag::O_DIRECTORY,
            Mode::empty(),
        ),
        None => root.try_clone(),
    }
}
fn read(parent: &File, name: &str) -> io::Result<(Identity, Vec<u8>)> {
    let file = open(parent, name, OFlag::O_RDONLY, Mode::empty())?;
    let before = file.metadata()?;
    if !before.is_file() || before.len() > FILE_BYTES as u64 || before.mode() & 0o7777 != 0o644 {
        return Err(err());
    }
    let identity = Identity::of(&before);
    let mut bytes = Vec::new();
    (&file)
        .take(FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > FILE_BYTES
        || bytes.len() as u64 != identity.bytes
        || Identity::of(&file.metadata()?) != identity
    {
        return Err(err());
    }
    Ok((identity, bytes))
}
impl PinSnapshot {
    pub fn read(root_path: &Path) -> Result<Self, String> {
        let root_path = root_path
            .canonicalize()
            .map_err(|_| "cannot open fixed pin root")?;
        let root = recover(&root_path)?;
        let mut sources = Vec::with_capacity(PROMOTION_FILES.len());
        for relative in PIN_PATHS {
            let dir = parent(&root, relative)
                .map_err(|_| format!("cannot open fixed pin parent: {relative}"))?;
            let name = relative
                .rsplit('/')
                .next()
                .ok_or("fixed pin filename")?
                .to_string();
            let (identity, bytes) = read(&dir, &name)
                .map_err(|_| format!("cannot read bounded regular pin source: {relative}"))?;
            sources.push(Source {
                relative,
                name,
                parent: dir,
                identity,
                bytes,
            });
        }
        let snapshot = Self {
            root_path,
            root,
            sources,
        };
        super::promotion_plan(&snapshot.documents(), None)?;
        snapshot
            .recheck_all()
            .map_err(|_| "pin sources changed during prevalidation")?;
        Ok(snapshot)
    }
    fn documents(&self) -> Vec<PinText> {
        self.sources
            .iter()
            .map(|s| PinText {
                path: s.relative,
                bytes: s.bytes.clone(),
            })
            .collect()
    }
    pub fn plan(&self, latest: &str) -> Result<PromotionPlan, String> {
        super::promotion_plan(&self.documents(), Some(latest))
    }
    fn recheck(&self, source: &Source) -> io::Result<()> {
        let named_root = fs::symlink_metadata(&self.root_path)?;
        let held_root = self.root.metadata()?;
        if !named_root.is_dir()
            || named_root.file_type().is_symlink()
            || named_root.dev() != held_root.dev()
            || named_root.ino() != held_root.ino()
        {
            return Err(err());
        }
        let current_parent = parent(&self.root, source.relative)?;
        let a = current_parent.metadata()?;
        let b = source.parent.metadata()?;
        if !a.is_dir() || a.dev() != b.dev() || a.ino() != b.ino() {
            return Err(err());
        }
        let (identity, bytes) = read(&current_parent, &source.name)?;
        if identity != source.identity || bytes != source.bytes {
            return Err(err());
        }
        Ok(())
    }
    fn recheck_all(&self) -> io::Result<()> {
        for source in &self.sources {
            self.recheck(source)?;
        }
        Ok(())
    }
    pub fn apply(&self, plan: &PromotionPlan) -> Result<Vec<&'static str>, ApplyError> {
        self.apply_inner(plan, |_, _, _, _| Ok(()))
    }
    #[cfg(test)]
    pub(super) fn apply_with_test_hook(
        &self,
        plan: &PromotionPlan,
        hook: impl FnMut(ApplyPoint, usize, &File, &str) -> io::Result<()>,
    ) -> Result<Vec<&'static str>, ApplyError> {
        self.apply_inner(plan, hook)
    }
    fn apply_inner(
        &self,
        plan: &PromotionPlan,
        mut hook: impl FnMut(ApplyPoint, usize, &File, &str) -> io::Result<()>,
    ) -> Result<Vec<&'static str>, ApplyError> {
        let failure = |message, replaced, cleanup_failed| ApplyError {
            message,
            replaced,
            cleanup_failed,
        };
        if plan.originals != self.documents()
            || super::promotion_plan(&plan.originals, None).is_err()
        {
            return Err(failure(
                "promotion plan does not match source snapshot",
                vec![],
                vec![],
            ));
        }
        let verified = super::promotion_plan(&plan.originals, Some(&plan.latest))
            .map_err(|_| failure("invalid promotion plan", vec![], vec![]))?;
        if verified.outputs != plan.outputs || verified.current != plan.current {
            return Err(failure("promotion output proof differs", vec![], vec![]));
        }
        self.recheck_all()
            .map_err(|_| failure("pin sources changed before staging", vec![], vec![]))?;
        if !plan.changed() {
            return Ok(vec![]);
        }
        let mut stages: Vec<Stage> = Vec::with_capacity(PROMOTION_FILES.len());
        let mut replaced = Vec::new();
        let mut journal_started = false;
        let mut commit_uncertain = false;
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| failure("staging clock unavailable", vec![], vec![]))?
            .as_nanos();
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        let stage_names: Vec<String> = (0..PROMOTION_FILES.len())
            .map(|index| {
                format!(
                    ".ssx-image-promotion-{}-{stamp}-{serial}-{index}.tmp",
                    std::process::id()
                )
            })
            .collect();
        let result = (|| -> Result<(), &'static str> {
            journal_started = true;
            transaction::begin(&self.root, plan, &stage_names, |point| {
                hook(point, 0, &self.root, "initialization")
            })
            .map_err(|_| "cannot persist promotion originals")?;
            for (index, (source, output)) in self.sources.iter().zip(&plan.outputs).enumerate() {
                let name = stage_names[index].clone();
                hook(ApplyPoint::BeforeStage, index, &source.parent, &name)
                    .map_err(|_| "staging control failed")?;
                let held_parent = source
                    .parent
                    .try_clone()
                    .map_err(|_| "cannot retain staging directory")?;
                let mut file = open(
                    &source.parent,
                    &name,
                    OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL,
                    Mode::S_IRUSR | Mode::S_IWUSR,
                )
                .map_err(|_| "cannot create owned pin staging file")?;
                stages.push(Stage {
                    parent: held_parent,
                    name,
                    relative: source.relative,
                    dev: 0,
                    ino: 0,
                    identified: false,
                    pending: true,
                });
                let initial = file
                    .metadata()
                    .map_err(|_| "cannot identify pin staging file")?;
                stages[index].dev = initial.dev();
                stages[index].ino = initial.ino();
                stages[index].identified = true;
                file.write_all(&output.bytes)
                    .map_err(|_| "cannot write pin staging output")?;
                file.set_permissions(Permissions::from_mode(source.identity.mode & 0o7777))
                    .map_err(|_| "cannot preserve pin file permissions")?;
                file.flush()
                    .map_err(|_| "cannot flush pin staging output")?;
                file.sync_all()
                    .map_err(|_| "cannot synchronize pin staging output")?;
                drop(file);
                let (identity, bytes) = read(&source.parent, &stages[index].name)
                    .map_err(|_| "cannot verify staged pin output")?;
                if identity.dev != stages[index].dev
                    || identity.ino != stages[index].ino
                    || identity.mode & 0o7777 != source.identity.mode & 0o7777
                    || bytes != output.bytes
                {
                    return Err("staged pin output differs");
                }
            }
            self.recheck_all()
                .map_err(|_| "pin sources changed while staging")?;
            for (index, source) in self.sources.iter().enumerate() {
                let stage = &mut stages[index];
                hook(
                    ApplyPoint::BeforeReplace,
                    index,
                    &source.parent,
                    &stage.name,
                )
                .map_err(|_| "replacement control failed")?;
                self.recheck(source)
                    .map_err(|_| "pin source changed before replacement")?;
                let (identity, bytes) = read(&stage.parent, &stage.name)
                    .map_err(|_| "staged pin output unavailable")?;
                if identity.dev != stage.dev
                    || identity.ino != stage.ino
                    || bytes != plan.outputs[index].bytes
                {
                    return Err("staged pin output changed");
                }
                hook(
                    ApplyPoint::ReadyToRename,
                    index,
                    &source.parent,
                    &stage.name,
                )
                .map_err(|_| "replacement control failed")?;
                self.recheck(source)
                    .map_err(|_| "pin source changed at replacement boundary")?;
                let (identity, bytes) = read(&stage.parent, &stage.name)
                    .map_err(|_| "staged pin output unavailable at replacement boundary")?;
                if identity.dev != stage.dev
                    || identity.ino != stage.ino
                    || identity.mode & 0o7777 != source.identity.mode & 0o7777
                    || bytes != plan.outputs[index].bytes
                {
                    return Err("staged pin output changed at replacement boundary");
                }
                hook(ApplyPoint::BeforeRename, index, &source.parent, &stage.name)
                    .map_err(|_| "replacement control failed")?;
                transaction::exchange(&source.parent, &stage.name, &source.name)
                    .map_err(|_| "atomic pin exchange failed; review partial state before retry")?;
                // This entry now holds the displaced source. Only the durable
                // journal can authorize its recovery or retirement.
                stage.pending = false;
                hook(
                    ApplyPoint::AfterExchange,
                    index,
                    &source.parent,
                    &stage.name,
                )
                .map_err(|_| "post-exchange control failed")?;
                let installed = read(&source.parent, &source.name).is_ok_and(|(after, bytes)| {
                    after.matches_after_exchange(&identity) && bytes == plan.outputs[index].bytes
                });
                let displaced = read(&stage.parent, &stage.name).is_ok_and(|(after, bytes)| {
                    after.matches_after_exchange(&source.identity) && bytes == source.bytes
                });
                if !installed || !displaced {
                    if installed || displaced {
                        // Exchange back without unlinking either participant.
                        let _ = transaction::exchange(&source.parent, &stage.name, &source.name);
                    }
                    let _ = source.parent.sync_all();
                    return Err("pin source or staging file changed during exchange");
                }
                replaced.push(source.relative);
                source.parent.sync_all().map_err(
                    |_| "pin directory synchronization failed; review partial state before retry",
                )?;
            }
            hook(
                ApplyPoint::BeforeCommit,
                PROMOTION_FILES.len(),
                &self.root,
                "journal.json",
            )
            .map_err(|_| "commit control failed")?;
            if let Err(failure) = transaction::commit(&self.root, |point| {
                hook(point, PROMOTION_FILES.len(), &self.root, "commit.pending")
            }) {
                if failure.published {
                    journal_started = false;
                    commit_uncertain = true;
                    return Err(
                        "commit marker published; preserve outputs and run --recover before retry",
                    );
                }
                return Err("commit marker publication failed before commitment");
            }
            journal_started = false;
            // The committed journal is recoverable cleanup evidence. Once the
            // durable commit succeeds, cleanup failure must not report a failed
            // promotion that would misleadingly imply originals were restored.
            let cleanup = hook(
                ApplyPoint::AfterCommit,
                PROMOTION_FILES.len(),
                &self.root,
                "committed",
            )
            .and_then(|()| {
                transaction::complete_with_hook(&self.root, |point| {
                    hook(point, PROMOTION_FILES.len(), &self.root, "cleanup")
                })
            });
            if let Err(error) = cleanup {
                eprintln!(
                    "O02 image pin: committed pins; retaining transaction for cleanup: {error}"
                );
            }
            Ok(())
        })();
        let mut cleanup_failed = Vec::new();
        if commit_uncertain {
            cleanup_failed.push(".ssx-image-promotion (published commit; recovery required)");
        }
        if result.is_err() && journal_started {
            let recovered = transaction::rollback_locked(&self.root, |index| {
                hook(
                    ApplyPoint::BeforeRollback,
                    index,
                    &self.root,
                    "journal.json",
                )
            });
            if recovered.is_ok() {
                replaced.clear();
            } else {
                cleanup_failed.push(".ssx-image-promotion (rollback/recovery required)");
            }
        }

        for stage in &mut stages {
            if stage.cleanup().is_err() {
                cleanup_failed.push(stage.relative);
            }
        }
        match result {
            Ok(()) if cleanup_failed.is_empty() => Ok(replaced),
            Ok(()) => Err(failure(
                "pin staging cleanup failed",
                replaced,
                cleanup_failed,
            )),
            Err(message) => Err(failure(message, replaced, cleanup_failed)),
        }
    }
}
struct Stage {
    parent: File,
    name: String,
    relative: &'static str,
    dev: u64,
    ino: u64,
    identified: bool,
    pending: bool,
}
fn device_id<T: TryInto<u64>>(value: T) -> Option<u64> {
    value.try_into().ok()
}

impl Stage {
    fn cleanup(&mut self) -> io::Result<()> {
        if !self.pending {
            return Ok(());
        }
        if !self.identified {
            return Err(err());
        }
        match fstatat(
            &self.parent,
            self.name.as_str(),
            AtFlags::AT_SYMLINK_NOFOLLOW,
        ) {
            Ok(stat) if device_id(stat.st_dev) == Some(self.dev) && stat.st_ino == self.ino => {
                unlinkat(&self.parent, self.name.as_str(), UnlinkatFlags::NoRemoveDir)
                    .map_err(io::Error::from)?;
                self.pending = false;
                Ok(())
            }
            Err(nix::errno::Errno::ENOENT) => {
                self.pending = false;
                Ok(())
            }
            _ => Err(err()),
        }
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(test)]
mod device_id_tests {
    use super::device_id;

    #[test]
    fn converts_signed_macos_and_unsigned_linux_device_ids_without_casts() {
        assert_eq!(device_id(7_i32), Some(7_u64));
        assert_eq!(device_id(7_u64), Some(7_u64));
        assert_eq!(device_id(-1_i32), None);
    }
}
