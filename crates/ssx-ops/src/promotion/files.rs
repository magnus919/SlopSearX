//! Fixed-three-data-file Unix application. Renames are atomic individually, not as a set.
use super::{ApplyError, FILE_BYTES, PIN_PATHS, PinText, PromotionPlan};
use nix::{
    fcntl::{AtFlags, OFlag, openat, renameat},
    sys::stat::{Mode, fstatat},
    unistd::{UnlinkatFlags, unlinkat},
};
use std::{
    fs::{self, File, Metadata, Permissions},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
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
    if !before.is_file() || before.len() > FILE_BYTES as u64 {
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
        let root = fs::OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(&root_path)
            .map_err(|_| "cannot open fixed pin root")?;
        let mut sources = Vec::with_capacity(3);
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
        super::source_state(&snapshot.documents())?;
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
        super::plan(&self.documents(), latest)
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
        if plan.originals != self.documents() || super::source_state(&plan.originals).is_err() {
            return Err(failure(
                "promotion plan does not match source snapshot",
                vec![],
                vec![],
            ));
        }
        let verified = super::plan(&plan.originals, &plan.latest)
            .map_err(|_| failure("invalid promotion plan", vec![], vec![]))?;
        if verified.outputs != plan.outputs
            || verified.counts != plan.counts
            || verified.current != plan.current
        {
            return Err(failure("promotion output proof differs", vec![], vec![]));
        }
        self.recheck_all()
            .map_err(|_| failure("pin sources changed before staging", vec![], vec![]))?;
        if !plan.changed() {
            return Ok(vec![]);
        }
        let mut stages: Vec<Stage> = Vec::with_capacity(3);
        let mut replaced = Vec::new();
        let result = (|| -> Result<(), &'static str> {
            for (index, (source, output)) in self.sources.iter().zip(&plan.outputs).enumerate() {
                static NEXT: AtomicU64 = AtomicU64::new(0);
                let serial = NEXT.fetch_add(1, Ordering::Relaxed);
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| "staging clock unavailable")?
                    .as_nanos();
                let name = format!(
                    ".ssx-image-promotion-{}-{stamp}-{serial}.tmp",
                    std::process::id()
                );
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
                renameat(
                    &stage.parent,
                    stage.name.as_str(),
                    &source.parent,
                    source.name.as_str(),
                )
                .map_err(|_| "atomic pin replacement failed; review partial state before retry")?;
                stage.pending = false;
                replaced.push(source.relative);
                source.parent.sync_all().map_err(
                    |_| "pin directory synchronization failed; review partial state before retry",
                )?;
            }
            Ok(())
        })();
        let mut cleanup_failed = Vec::new();
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
            Ok(stat) if stat.st_dev as u64 == self.dev && stat.st_ino as u64 == self.ino => {
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
