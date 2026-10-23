use super::protocol::{self, SourceResolveError, SourceResolveStage};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
    net::{IpAddr, SocketAddr},
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
    sync::{oneshot, watch},
    task::JoinSet,
};

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

/// Failed cleanup is sticky across close retries; semantic GAI/socket failures
/// do not make close fail when every process was reaped and group was absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCloseError {
    pub failed_supervisors: usize,
    pub unretired_rows: usize,
}
impl std::fmt::Display for SourceCloseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "source resolver close: {} failed supervisors, {} unretired rows",
            self.failed_supervisors, self.unretired_rows
        )
    }
}
impl std::error::Error for SourceCloseError {}
struct Row {
    cancel: watch::Sender<bool>,
    group: Arc<AtomicU32>,
    ready: Arc<AtomicBool>,
    // Monotonic true -> false under the signal gate before FIRST try_wait.
    // A false value can never authorize another group signal.
    signalable: Arc<Mutex<bool>>,
    residual: Option<Child>,
}
struct State {
    closing: bool,
    next: u64,
    rows: BTreeMap<u64, Row>,
    tasks: JoinSet<(u64, Option<Child>)>,
    failures: usize,
}
impl State {
    fn completed(&mut self, result: Result<(u64, Option<Child>), tokio::task::JoinError>) {
        match result {
            Ok((id, None)) => {
                self.rows.remove(&id);
            }
            Ok((id, Some(child))) => {
                self.failures += 1;
                // Keep ownership of a child whose reaping was not established.
                // The next explicit close retries its retirement.
                if let Some(row) = self.rows.get_mut(&id) {
                    row.residual = Some(child);
                }
            }
            Err(_) => {
                self.failures += 1;
            }
        }
    }
    fn reap_ready(&mut self) {
        while let Some(result) = self.tasks.try_join_next() {
            self.completed(result);
        }
    }
}
struct Inner {
    helper: SourceResolverHelper,
    state: Mutex<State>,
    closer: tokio::sync::Mutex<()>,
}
fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}
impl Drop for Inner {
    fn drop(&mut self) {
        // Best effort only; application owner MUST await close. JoinSet/Child Drop
        // and this signal cannot prove reaping or group absence.
        let state = self
            .state
            .get_mut()
            .unwrap_or_else(|error| error.into_inner());
        for row in state.rows.values() {
            row.cancel.send_replace(true);
            let signalable = locked(&row.signalable);
            if *signalable {
                signal(
                    row.group.load(Ordering::Acquire),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
        }
    }
}
/// Application lifetime owner. Its outer server owner retains a clone across
/// request aborts and must await close before retiring the application's IO.
#[derive(Clone)]
pub struct SourceRuntime {
    inner: Arc<Inner>,
}
impl std::fmt::Debug for SourceRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceRuntime").finish_non_exhaustive()
    }
}
struct CancelOnDrop(watch::Sender<bool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}
impl SourceRuntime {
    pub fn new(helper: SourceResolverHelper) -> Self {
        Self {
            inner: Arc::new(Inner {
                helper,
                state: Mutex::new(State {
                    closing: false,
                    next: 0,
                    rows: BTreeMap::new(),
                    tasks: JoinSet::new(),
                    failures: 0,
                }),
                closer: tokio::sync::Mutex::new(()),
            }),
        }
    }
    /// Reqwest's ordinary hostname hook supplies no service. Preserve the
    /// native sockaddr order/scope/flow/port; the existing connector applies its
    /// ordinary explicit/default destination-port rules after this hook.
    pub async fn resolve_dns(
        &self,
        host: &str,
        lookup_budget: Duration,
    ) -> Result<Vec<SocketAddr>, SourceResolveError> {
        self.lookup(host, "0", lookup_budget, true).await
    }
    /// Retained source endpoint protocol. Raw gateway URL adoption is separate.
    pub async fn resolve(
        &self,
        host: &str,
        decimal: &str,
        lookup_budget: Duration,
    ) -> Result<Vec<SocketAddr>, SourceResolveError> {
        self.lookup(host, decimal, lookup_budget, false).await
    }
    async fn lookup(
        &self,
        host: &str,
        decimal: &str,
        lookup_budget: Duration,
        native_sockaddrs: bool,
    ) -> Result<Vec<SocketAddr>, SourceResolveError> {
        if locked(&self.inner.state).closing {
            return Err(SourceResolveError::new(SourceResolveStage::Closed));
        }
        // Validate the trusted raw endpoint contract before literal handling.
        let _ = protocol::request(host, decimal)?;
        // AnyIO ip_address accepts scoped IPv6; asyncio _ipaddr_info explicitly
        // declines %. Unscoped numeric addresses never call getaddrinfo.
        if !host.contains('%') {
            if let Ok(ip) = host.parse::<IpAddr>() {
                return protocol::socket_port(decimal).map(|port| vec![SocketAddr::new(ip, port)]);
            }
        }
        let scoped_host = scoped_literal(host);
        let scoped = scoped_host.is_some();
        // AnyIO passes IPv6Address.compressed, preserving the scope string.
        // CPython3.14 uses dotted IPv4 for mapped IPv6, as Rust Display does.
        let input = protocol::request(scoped_host.as_deref().unwrap_or(host), decimal)?;
        let stage = if scoped {
            SourceResolveStage::AsyncIoGetAddrInfo
        } else {
            SourceResolveStage::AnyIoGetAddrInfo
        };
        let (send, receive) = oneshot::channel();
        let (cancel, _) = watch::channel(false);
        let request_guard = CancelOnDrop(cancel.clone());
        {
            // One lock commits closing/admission/row+task ownership atomically.
            // The cancellation row exists before the task can first be polled.
            let mut state = locked(&self.inner.state);
            state.reap_ready();
            if state.closing {
                return Err(SourceResolveError::new(SourceResolveStage::Closed));
            }
            let id = state.next;
            state.next = state
                .next
                .checked_add(1)
                .ok_or_else(|| SourceResolveError::new(SourceResolveStage::OperationalLimit))?;
            let group = Arc::new(AtomicU32::new(0));
            let ready = Arc::new(AtomicBool::new(false));
            let signalable = Arc::new(Mutex::new(true));
            state.rows.insert(
                id,
                Row {
                    cancel: cancel.clone(),
                    group: group.clone(),
                    ready: ready.clone(),
                    signalable: signalable.clone(),
                    residual: None,
                },
            );
            let helper = self.inner.helper.clone();
            let original_decimal = decimal.to_owned();
            state.tasks.spawn(async move {
                let (result, residual) = supervise(
                    helper,
                    input,
                    cancel.subscribe(),
                    group,
                    ready,
                    signalable,
                    lookup_budget,
                )
                .await;
                let result = result.and_then(|bytes| {
                    let addrs = protocol::decode_response(&bytes, stage)?;
                    if addrs.is_empty() {
                        return Err(SourceResolveError::new(SourceResolveStage::EmptyAddresses));
                    }
                    if scoped || native_sockaddrs {
                        return Ok(addrs);
                    }
                    // AnyIO ignores GAI returned ports; each asyncio numeric
                    // shortcut retains original integer. All non-u16 outcomes
                    // fail at socket conversion before TCP attempts can start.
                    let port = protocol::socket_port(&original_decimal)?;
                    Ok(addrs
                        .into_iter()
                        .map(|addr| SocketAddr::new(addr.ip(), port))
                        .collect())
                });
                let _ = send.send(result);
                (id, residual)
            });
        }
        let result = receive
            .await
            .unwrap_or_else(|_| Err(SourceResolveError::new(SourceResolveStage::Helper)));
        drop(request_guard);
        result
    }
    /// Close is cancellation safe: dropping this future leaves tasks and rows
    /// registered, closing stays true, and the next close resumes joining them.
    /// Concurrent callers serialize join polling; no overwritten JoinSet waker.
    pub async fn close(&self) -> Result<(), SourceCloseError> {
        {
            let mut state = locked(&self.inner.state);
            state.closing = true;
            for row in state.rows.values() {
                row.cancel.send_replace(true);
            }
        }
        let _closer = self.inner.closer.lock().await;
        {
            let mut state = locked(&self.inner.state);
            let retries: Vec<_> = state
                .rows
                .iter_mut()
                .filter_map(|(id, row)| {
                    row.residual
                        .take()
                        .map(|child| (*id, child, row.group.clone(), row.signalable.clone()))
                })
                .collect();
            for (id, mut child, group, signalable) in retries {
                state.tasks.spawn(async move {
                    // Residual rows are reaping/observation only. The leader may
                    // already have been reaped; its PGID is never signalled again.
                    let outcome = retire(
                        &mut child,
                        group.load(Ordering::Acquire),
                        &signalable,
                        false,
                    )
                    .await;
                    if outcome.group_absent {
                        group.store(0, Ordering::Release);
                    }
                    if outcome.retired() {
                        (id, None)
                    } else {
                        (id, Some(child))
                    }
                });
            }
        }
        loop {
            let result = std::future::poll_fn(|cx| {
                let mut state = locked(&self.inner.state);
                state.tasks.poll_join_next(cx)
            })
            .await;
            match result {
                Some(result) => locked(&self.inner.state).completed(result),
                None => break,
            }
        }
        let state = locked(&self.inner.state);
        if state.failures == 0 && state.rows.is_empty() {
            Ok(())
        } else {
            Err(SourceCloseError {
                failed_supervisors: state.failures,
                unretired_rows: state.rows.len(),
            })
        }
    }
    #[doc(hidden)]
    pub fn ready_process_groups(&self) -> Vec<u32> {
        let state = locked(&self.inner.state);
        state
            .rows
            .values()
            .filter(|row| row.ready.load(Ordering::Acquire))
            .map(|row| row.group.load(Ordering::Acquire))
            .filter(|id| *id != 0)
            .collect()
    }
    #[doc(hidden)]
    pub fn ownership_snapshot(&self) -> (bool, usize, Vec<u32>) {
        let mut state = locked(&self.inner.state);
        state.reap_ready();
        (
            state.closing,
            state.rows.len(),
            state
                .rows
                .values()
                .map(|row| row.group.load(Ordering::Acquire))
                .filter(|id| *id != 0)
                .collect(),
        )
    }
}
fn scoped_literal(host: &str) -> Option<String> {
    let (address, scope) = host.split_once('%')?;
    if scope.is_empty() || scope.contains('%') {
        return None;
    }
    let address = address.parse::<std::net::Ipv6Addr>().ok()?;
    Some(format!("{address}%{scope}"))
}
#[cfg(test)]
mod tests {
    #[test]
    fn anyio_scoped_literal_compression_keeps_source_scope() {
        assert_eq!(
            super::scoped_literal("0:0:0:0:0:0:0:1%lo0").as_deref(),
            Some("::1%lo0")
        );
        assert_eq!(
            super::scoped_literal("0:0:0:0:0:ffff:c000:201%3").as_deref(),
            Some("::ffff:192.0.2.1%3")
        );
        assert_eq!(super::scoped_literal("example.invalid%scope"), None);
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
struct Retirement {
    status: Option<std::process::ExitStatus>,
    group_absent: bool,
}
impl Retirement {
    fn retired(&self) -> bool {
        self.status.is_some() && self.group_absent
    }
}
async fn retire(
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
async fn supervise(
    helper: SourceResolverHelper,
    input: Vec<u8>,
    mut cancel: watch::Receiver<bool>,
    group: Arc<AtomicU32>,
    ready: Arc<AtomicBool>,
    signalable: Arc<Mutex<bool>>,
    lookup_budget: Duration,
) -> (Result<Vec<u8>, SourceResolveError>, Option<Child>) {
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
    let outcome = retire(&mut child, pid, &signalable, completed_io).await;
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
