use super::protocol::{self, SourceResolveError, SourceResolveStage};
use super::runtime::locked;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
    os::unix::{
        fs::MetadataExt,
        process::{CommandExt, ExitStatusExt},
    },
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{ChildStderr, ChildStdin, ChildStdout},
    sync::watch,
};
pub(super) type OwnedChild = Child;
pub(super) struct Control {
    group: Arc<AtomicU32>,
    signalable: Arc<Mutex<bool>>,
}
impl Control {
    pub(super) fn new() -> Self {
        Self {
            group: Arc::new(AtomicU32::new(0)),
            signalable: Arc::new(Mutex::new(true)),
        }
    }
    pub(super) fn identity(&self) -> u32 {
        self.group.load(Ordering::Acquire)
    }
    pub(super) fn clear_identity(&self) {
        self.group.store(0, Ordering::Release);
    }
    pub(super) fn cancel_fallback(&self) {
        let gate = locked(&self.signalable);
        if *gate {
            signal(self.identity(), nix::sys::signal::Signal::SIGKILL);
        }
    }
}
/// Explicit trusted installation identity. Digest is verified against an opened
/// file at construction; metadata is rechecked before each path-based exec.
/// The installation directory must prevent replacement: path exec has a TOCTOU
/// window and this API does not claim fd-based/cryptographic execution binding.
#[derive(Clone)]
pub struct SourceResolverHelper {
    path: PathBuf,
    identity: FileIdentity,
    #[cfg(feature = "gateway-dns-fixtures")]
    fail_pipe_conversion: bool,
}
#[derive(Clone, Eq, PartialEq)]
struct FileIdentity {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: i64,
    mtime_nsec: i64,
}
impl FileIdentity {
    fn of(metadata: &std::fs::Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            len: metadata.len(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
        }
    }
}
impl std::fmt::Debug for SourceResolverHelper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceResolverHelper")
            .finish_non_exhaustive()
    }
}
impl SourceResolverHelper {
    pub fn new(path: PathBuf, expected_sha256: [u8; 32]) -> io::Result<Self> {
        if !path.is_absolute() || std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut file = File::open(&path)?;
        let before = file.metadata()?;
        if !before.is_file() || before.mode() & 0o111 == 0 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let identity = FileIdentity::of(&before);
        let mut hash = Sha256::new();
        let mut block = [0; 65536];
        loop {
            let n = file.read(&mut block)?;
            if n == 0 {
                break;
            }
            hash.update(&block[..n]);
        }
        let actual: [u8; 32] = hash.finalize().into();
        if actual != expected_sha256 || identity != FileIdentity::of(&file.metadata()?) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(Self {
            path,
            identity,
            #[cfg(feature = "gateway-dns-fixtures")]
            fail_pipe_conversion: false,
        })
    }
    #[cfg(feature = "gateway-dns-fixtures")]
    #[doc(hidden)]
    pub fn inject_pipe_conversion_failure_for_test(mut self) -> Self {
        self.fail_pipe_conversion = true;
        self
    }
    fn command(&self) -> io::Result<Command> {
        let metadata = std::fs::symlink_metadata(&self.path)?;
        if !metadata.is_file() || self.identity != FileIdentity::of(&metadata) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let mut command = Command::new(&self.path);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        Ok(command)
    }
}

async fn cancelled(cancel: &mut watch::Receiver<bool>) {
    loop {
        if *cancel.borrow_and_update() {
            return;
        }
        if cancel.changed().await.is_err() {
            return;
        }
    }
}
async fn bounded_read(
    reader: impl AsyncRead + Unpin,
    limit: usize,
) -> Result<Vec<u8>, SourceResolveError> {
    let mut bytes = Vec::new();
    reader
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| SourceResolveError::new(SourceResolveStage::Helper))?;
    if bytes.len() > limit {
        return Err(SourceResolveError::new(
            SourceResolveStage::OperationalLimit,
        ));
    }
    Ok(bytes)
}
struct GroupGuard {
    group: u32,
    signalable: Arc<Mutex<bool>>,
}
impl Drop for GroupGuard {
    fn drop(&mut self) {
        // The gate is disarmed before any reaping poll. Drop must never
        // signal an integer PGID after child reaping has become possible.
        let signalable = locked(&self.signalable);
        if *signalable {
            signal(self.group, nix::sys::signal::Signal::SIGKILL);
        }
    }
}
fn signal(group: u32, kind: nix::sys::signal::Signal) {
    if group > 1 && group <= i32::MAX as u32 {
        let _ = nix::sys::signal::killpg(nix::unistd::Pid::from_raw(group as i32), kind);
    }
}
fn group_absent(group: u32) -> bool {
    if group == 0 {
        return true;
    } // only stored after established group absence
    if group == 1 || group > i32::MAX as u32 {
        return false;
    }
    matches!(
        nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(group as i32),
            None::<nix::sys::signal::Signal>
        ),
        Err(nix::errno::Errno::ESRCH)
    )
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObservedExit {
    code: Option<i32>,
    signal: Option<i32>,
    core_dumped: bool,
}
impl ObservedExit {
    fn of(status: std::process::ExitStatus) -> Self {
        Self {
            code: status.code(),
            signal: status.signal(),
            core_dumped: status.core_dumped(),
        }
    }
    fn success(self) -> bool {
        self.code == Some(0)
    }
    fn matches(self, status: std::process::ExitStatus) -> bool {
        self == Self::of(status)
    }
}
// Safe process-exit observation must NOT reap the direct leader. It is armed
// before helper input can permit GAI work or a normal successful exit.
struct ExitObserver {
    pid: nix::unistd::Pid,
    #[cfg(target_vendor = "apple")]
    queue: nix::sys::event::Kqueue,
}
impl ExitObserver {
    fn new(pid: u32) -> Result<Self, SourceResolveError> {
        let pid = nix::unistd::Pid::from_raw(pid as i32);
        #[cfg(target_vendor = "apple")]
        {
            use nix::sys::event::{EvFlags, EventFilter, FilterFlag, KEvent, Kqueue};
            let queue =
                Kqueue::new().map_err(|_| SourceResolveError::new(SourceResolveStage::Helper))?;
            let event = KEvent::new(
                pid.as_raw() as usize,
                EventFilter::EVFILT_PROC,
                EvFlags::EV_ADD | EvFlags::EV_ENABLE | EvFlags::EV_ONESHOT,
                FilterFlag::NOTE_EXIT | FilterFlag::NOTE_EXITSTATUS,
                0,
                0,
            );
            queue
                .kevent(
                    &[event],
                    &mut [],
                    Some(libc::timespec {
                        tv_sec: 0,
                        tv_nsec: 0,
                    }),
                )
                .map_err(|_| SourceResolveError::new(SourceResolveStage::Helper))?;
            Ok(Self { pid, queue })
        }
        #[cfg(not(target_vendor = "apple"))]
        {
            Ok(Self { pid })
        }
    }
    async fn exited(&mut self) -> Result<ObservedExit, SourceResolveError> {
        loop {
            if let Some(status) = self.peek()? {
                return Ok(status);
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
    fn peek(&mut self) -> Result<Option<ObservedExit>, SourceResolveError> {
        let failure = || SourceResolveError::new(SourceResolveStage::Helper);
        #[cfg(target_vendor = "apple")]
        {
            use nix::sys::event::{EvFlags, EventFilter, FilterFlag, KEvent};
            let mut events = [KEvent::new(
                0,
                EventFilter::EVFILT_PROC,
                EvFlags::empty(),
                FilterFlag::empty(),
                0,
                0,
            )];
            let count = match self.queue.kevent(
                &[],
                &mut events,
                Some(libc::timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                }),
            ) {
                Ok(count) => count,
                Err(nix::errno::Errno::EINTR) => return Ok(None),
                Err(_) => return Err(failure()),
            };
            if count == 0 {
                return Ok(None);
            }
            let event = events[0];
            if event.ident() != self.pid.as_raw() as usize
                || event.flags().contains(EvFlags::EV_ERROR)
                || !event
                    .fflags()
                    .contains(FilterFlag::NOTE_EXIT | FilterFlag::NOTE_EXITSTATUS)
            {
                return Err(failure());
            }
            // Apple NOTE_EXITSTATUS data carries the child wait status in the
            // low status bits. We requested no NOTE_EXIT_DETAIL additions.
            Ok(Some(ObservedExit::of(std::process::ExitStatus::from_raw(
                (event.data() & 0xffff) as i32,
            ))))
        }
        #[cfg(not(target_vendor = "apple"))]
        {
            use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid};
            let status = match waitid(
                Id::Pid(self.pid),
                WaitPidFlag::WEXITED | WaitPidFlag::WNOWAIT | WaitPidFlag::WNOHANG,
            ) {
                Ok(status) => status,
                Err(nix::errno::Errno::EINTR) => return Ok(None),
                Err(_) => return Err(failure()),
            };
            match status {
                WaitStatus::StillAlive => Ok(None),
                WaitStatus::Exited(_, code) => Ok(Some(ObservedExit {
                    code: Some(code),
                    signal: None,
                    core_dumped: false,
                })),
                WaitStatus::Signaled(_, signal, core_dumped) => Ok(Some(ObservedExit {
                    code: None,
                    signal: Some(signal as i32),
                    core_dumped,
                })),
                _ => Err(failure()),
            }
        }
    }
}
pub(super) struct Retirement {
    status: Option<std::process::ExitStatus>,
    pub(super) group_absent: bool,
}
impl Retirement {
    pub(super) fn retired(&self) -> bool {
        self.status.is_some() && self.group_absent
    }
}
async fn retire_raw(
    child: &mut Child,
    group: u32,
    signalable: &Mutex<bool>,
    completed_io: bool,
) -> Retirement {
    let sent_term = {
        let signalable = locked(signalable);
        if *signalable {
            signal(group, nix::sys::signal::Signal::SIGTERM);
            true
        } else {
            false
        }
    };
    if sent_term && !completed_io {
        // Incomplete IO includes held lookup, timeout, overflow and cancel.
        // completed_io also requires original non-reaping exit observation;
        // pipe EOF by itself never establishes successful exit.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    {
        // Every Drop/retirement signal takes this SAME gate. Final KILL and
        // disarm are atomic relative to those signals; no stale check/send can
        // race the first reaping poll on another runtime worker.
        let mut signalable = locked(signalable);
        if *signalable {
            signal(group, nix::sys::signal::Signal::SIGKILL);
            *signalable = false;
        }
    }
    let status = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => tokio::time::sleep(Duration::from_millis(5)).await,
                Err(_) => break None,
            }
        }
    })
    .await
    .ok()
    .flatten();
    // Observation only after reaping becomes possible. A reused PGID may cause
    // conservative failed retirement, but can never receive one of our signals.
    let group_absent = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if group_absent(group) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .is_ok();
    Retirement {
        status,
        group_absent,
    }
}
fn fail<T>(stage: SourceResolveStage) -> Result<T, SourceResolveError> {
    Err(SourceResolveError::new(stage))
}
pub(super) async fn supervise(
    helper: SourceResolverHelper,
    input: Vec<u8>,
    mut cancel: watch::Receiver<bool>,
    control: Arc<Control>,
    ready: Arc<AtomicBool>,
    lookup_budget: Duration,
) -> (Result<Vec<u8>, SourceResolveError>, Option<Child>) {
    let group = control.group.clone();
    let signalable = control.signalable.clone();
    if *cancel.borrow() {
        return (fail(SourceResolveStage::Cancelled), None);
    }
    let mut command = match helper.command() {
        Ok(command) => command,
        Err(_) => return (fail(SourceResolveStage::Helper), None),
    };
    if *cancel.borrow() {
        return (fail(SourceResolveStage::Cancelled), None);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return (fail(SourceResolveStage::Helper), None),
    };
    // std::process::Child exposes the exact owned identity immediately; async
    // pipe conversion below can fail without discarding the child handle.
    let pid = child.id();
    group.store(pid, Ordering::Release);
    let guard = GroupGuard {
        group: pid,
        signalable: signalable.clone(),
    };
    // Child is blocked on stdin until after registered cancellation is checked.
    let result = if *cancel.borrow() {
        fail(SourceResolveStage::Cancelled)
    } else {
        let observer = ExitObserver::new(pid);
        #[cfg(feature = "gateway-dns-fixtures")]
        let observer = if helper.fail_pipe_conversion {
            Err(SourceResolveError::new(SourceResolveStage::Helper))
        } else {
            observer
        };
        let stdin = child.stdin.take().map(ChildStdin::from_std).transpose();
        let stdout = child.stdout.take().map(ChildStdout::from_std).transpose();
        let stderr = child.stderr.take().map(ChildStderr::from_std).transpose();
        if let (Ok(Some(mut stdin)), Ok(Some(mut stdout)), Ok(Some(stderr)), Ok(mut observer)) =
            (stdin, stdout, stderr, observer)
        {
            let io_cancel = cancel.clone();
            let io = async {
                let mut handshake = [0; 16];
                stdout
                    .read_exact(&mut handshake)
                    .await
                    .map_err(|_| SourceResolveError::new(SourceResolveStage::Helper))?;
                if &handshake != protocol::MAGIC {
                    return fail(SourceResolveStage::Helper);
                }
                ready.store(true, Ordering::Release);
                if *io_cancel.borrow() {
                    return fail(SourceResolveStage::Cancelled);
                }
                let send = async {
                    stdin
                        .write_all(&input)
                        .await
                        .map_err(|_| SourceResolveError::new(SourceResolveStage::Helper))?;
                    stdin
                        .shutdown()
                        .await
                        .map_err(|_| SourceResolveError::new(SourceResolveStage::Helper))?;
                    drop(stdin);
                    Ok(())
                };
                // Never wait/poll/try_wait the child in this IO future.
                // Even crash and full pipe EOF leave the leader unreaped until
                // the retirement owner has sent its final process-group signal.
                let ((), out, _err, original_exit) = tokio::try_join!(
                    send,
                    bounded_read(stdout, protocol::FRAME_LIMIT),
                    bounded_read(stderr, protocol::STDERR_LIMIT),
                    observer.exited()
                )?;
                Ok((out, original_exit))
            };
            tokio::select! { biased;
                _=cancelled(&mut cancel)=>fail(SourceResolveStage::Cancelled),
                result=tokio::time::timeout(lookup_budget,io)=>match result { Ok(result)=>result, Err(_)=>fail(SourceResolveStage::TimedOut) },
            }
        } else {
            fail(SourceResolveStage::Helper)
        }
    };
    let completed_io = result.is_ok();
    let outcome = retire_raw(&mut child, pid, &signalable, completed_io).await;
    // guard takes the disarmed gate and cannot send a stale PGID signal.
    drop(guard);
    if outcome.group_absent {
        group.store(0, Ordering::Release);
    }
    let retired = outcome.retired();
    let result = if !retired {
        Err(SourceResolveError::new(SourceResolveStage::Helper))
    } else {
        result.and_then(|(bytes, original_exit)| {
            if original_exit.success()
                && outcome
                    .status
                    .is_some_and(|status| original_exit.matches(status))
            {
                Ok(bytes)
            } else {
                Err(SourceResolveError::new(SourceResolveStage::Helper))
            }
        })
    };
    (result, if retired { None } else { Some(child) })
}

pub(super) async fn retire(child: &mut Child, control: &Control, completed_io: bool) -> Retirement {
    retire_raw(child, control.identity(), &control.signalable, completed_io).await
}
