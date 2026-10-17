//! Backend-neutral one-request worker ownership. Backend validates its own
//! operation header, payload, output envelope, stages and native resource policy.
use std::{
    fmt,
    num::NonZeroUsize,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Notify, Semaphore, oneshot},
    time::Instant,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessError {
    InvalidCapacity,
    CleanupUnverified,
    Deadline,
    AdmissionClosed,
    Shutdown,
    Cancelled,
    OutputBound,
    Truncated,
    TrailingBytes,
    Allocation,
    Spawn(String),
    Io(String),
    Exit,
    Supervisor,
}
impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ProcessError {}
#[derive(Default)]
struct State {
    closing: bool,
    active: usize,
    cleanup_failed: bool,
    supervisor_failed: bool,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
struct Inner {
    helper: PathBuf,
    #[cfg(test)]
    test_args: Vec<std::ffi::OsString>,
    permits: Arc<Semaphore>,
    state: Mutex<State>,
    drained: Notify,
    shutdown: tokio::sync::watch::Sender<bool>,
    #[cfg(test)]
    cleanup_gate: Mutex<Option<Arc<CleanupGate>>>,
    #[cfg(test)]
    cleanup_error: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    delivery_gate: Mutex<Option<Arc<CleanupGate>>>,
}
#[derive(Clone)]
pub struct ProcessPool {
    inner: Arc<Inner>,
}
struct Active<L> {
    inner: Arc<Inner>,
    completed: bool,
    cleanup_verified: bool,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
    lease: Option<L>,
}
impl<L> Drop for Active<L> {
    fn drop(&mut self) {
        if !self.completed {
            fail_supervisor(&self.inner);
        }
        if self.cleanup_verified {
            let lease = self.lease.take();
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(lease))).is_err() {
                fail_supervisor(&self.inner);
            }
            drop(self.permit.take());
        } else {
            // Reuse is permanently closed on unknown cleanup. Retain caller
            // accounting rather than pretending orphaned resources disappeared.
            if let Some(permit) = self.permit.take() {
                permit.forget();
            }
            if let Some(lease) = self.lease.take() {
                std::mem::forget(lease);
            }
        }
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        state.active -= 1;
        drop(state);
        self.inner.drained.notify_waiters();
    }
}
fn fail_supervisor(inner: &Inner) {
    let mut state = inner.state.lock().unwrap_or_else(|e| e.into_inner());
    state.supervisor_failed = true;
    state.closing = true;
    inner.permits.close();
    inner.shutdown.send_replace(true);
}
impl ProcessPool {
    pub fn new(helper: PathBuf, capacity: NonZeroUsize) -> Result<Self, ProcessError> {
        if capacity.get() > Semaphore::MAX_PERMITS {
            return Err(ProcessError::InvalidCapacity);
        }
        Ok(Self {
            inner: Arc::new(Inner {
                helper,
                #[cfg(test)]
                test_args: Vec::new(),
                permits: Arc::new(Semaphore::new(capacity.get())),
                state: Mutex::new(State::default()),
                drained: Notify::new(),
                shutdown: tokio::sync::watch::channel(false).0,
                #[cfg(test)]
                cleanup_gate: Mutex::new(None),
                #[cfg(test)]
                cleanup_error: std::sync::atomic::AtomicBool::new(false),
                #[cfg(test)]
                delivery_gate: Mutex::new(None),
            }),
        })
    }
    pub fn available_slots(&self) -> usize {
        self.inner.permits.available_permits()
    }
    /// All callers wait for all admitted work to be killed/reaped. Dropping one
    /// shutdown waiter cannot remove another caller's ability to drain.
    /// Must be explicitly awaited before runtime teardown. Runtime task abort
    /// cannot provide the normal asynchronous kill-and-reap guarantee.
    pub async fn shutdown(&self) -> Result<(), ProcessError> {
        {
            let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
            state.closing = true;
            self.inner.permits.close();
            self.inner.shutdown.send_replace(true);
        }
        loop {
            let notified = self.inner.drained.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let tasks = {
                let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
                if state.active == 0 {
                    Some(std::mem::take(&mut state.tasks))
                } else {
                    None
                }
            };
            if let Some(tasks) = tasks {
                for task in tasks {
                    if task.await.is_err() {
                        self.inner
                            .state
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .supervisor_failed = true;
                    }
                }
                let state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
                return if state.cleanup_failed {
                    Err(ProcessError::CleanupUnverified)
                } else if state.supervisor_failed {
                    Err(ProcessError::Supervisor)
                } else {
                    Ok(())
                };
            }
            notified.await;
        }
    }
    /// No child starts after admission closes. The supervisor owns the permit
    /// through kill+wait even when the calling future disappears.
    pub async fn exchange(
        &self,
        deadline: Instant,
        header: Vec<u8>,
        payload: Vec<u8>,
        bound: usize,
    ) -> Result<Vec<u8>, ProcessError> {
        self.exchange_with_lease(deadline, header, payload, bound, ())
            .await
    }
    /// Caller passes a clone of its operation accounting owner. It survives
    /// receiver cancellation until frame resources drop and cleanup is verified.
    /// The caller retains its original owner while consuming returned bytes.
    /// Unknown cleanup/panic permanently poisons admission and retains this lease.
    /// Payload storage is owned beside the lease through verified cleanup; the
    /// IO writer only borrows its bytes. On verified disposal payload drops
    /// before the lease. Audited backend owners must provide nonpanicking
    /// AsRef/Drop; these generic bounds do not prove allocation accounting.
    pub async fn exchange_with_lease<L: Send + 'static, P: AsRef<[u8]> + Send + Sync + 'static>(
        &self,
        deadline: Instant,
        header: Vec<u8>,
        payload: P,
        bound: usize,
        lease: L,
    ) -> Result<Vec<u8>, ProcessError> {
        if Instant::now() >= deadline {
            return Err(ProcessError::Deadline);
        }
        let permit = tokio::time::timeout_at(deadline, self.inner.permits.clone().acquire_owned())
            .await
            .map_err(|_| ProcessError::Deadline)?
            .map_err(|_| ProcessError::AdmissionClosed)?;
        let (sender, receiver) = oneshot::channel();
        let inner = self.inner.clone();
        let mut shutdown = inner.shutdown.subscribe();
        {
            let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.closing {
                return Err(ProcessError::AdmissionClosed);
            }
            state.tasks.retain(|t| !t.is_finished());
            state.active += 1;
            let active = Active {
                inner: inner.clone(),
                completed: false,
                cleanup_verified: false,
                permit: Some(permit),
                // Storage drops before caller accounting on verified cleanup.
                // Unknown cleanup retains both via the existing Active guard.
                lease: Some((payload, lease)),
            };
            let task = tokio::spawn(async move {
                let mut active = active;
                let mut sender = sender;
                let result = run_worker(
                    &inner,
                    &mut sender,
                    &mut shutdown,
                    &active.lease.as_ref().expect("active request owner").0,
                    Request {
                        deadline,
                        header,
                        bound,
                    },
                )
                .await;
                active.cleanup_verified = result != Err(ProcessError::CleanupUnverified);
                if active.cleanup_verified {
                    drop(active.permit.take());
                }
                #[cfg(test)]
                {
                    let gate = inner.delivery_gate.lock().unwrap().clone();
                    if let Some(gate) = gate {
                        gate.entered.add_permits(1);
                        gate.release.acquire().await.unwrap().forget();
                    }
                }
                // Failed delivery returns ownership of the reply. Destroy it
                // before Active drops the accounting lease.
                drop(sender.send(result));
                active.completed = true;
            });
            state.tasks.push(task);
        }
        receiver.await.map_err(|_| ProcessError::Supervisor)?
    }
}
struct Request {
    deadline: Instant,
    header: Vec<u8>,
    bound: usize,
}
async fn run_worker<P: AsRef<[u8]> + Sync>(
    inner: &Inner,
    sender: &mut oneshot::Sender<Result<Vec<u8>, ProcessError>>,
    shutdown: &mut tokio::sync::watch::Receiver<bool>,
    payload: &P,
    request: Request,
) -> Result<Vec<u8>, ProcessError> {
    let Request {
        deadline,
        header,
        bound,
    } = request;
    if sender.is_closed() {
        return Err(ProcessError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(ProcessError::Deadline);
    }
    let mut child = {
        // Actual spawn and closing share this lock; never await while held.
        let state = inner.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closing {
            return Err(ProcessError::Shutdown);
        }
        if sender.is_closed() {
            return Err(ProcessError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(ProcessError::Deadline);
        }
        let mut command = tokio::process::Command::new(&inner.helper);
        #[cfg(test)]
        command.args(&inner.test_args);
        command
            .env_clear()
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ProcessError::Spawn(e.to_string()))?
    };
    let (Some(mut stdin), Some(mut stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.start_kill();
        if child.wait().await.is_err() {
            fail_cleanup(inner);
            return Err(ProcessError::CleanupUnverified);
        }
        return Err(ProcessError::Supervisor);
    };
    let work = async {
        let write = async move {
            stdin.write_all(&header).await.map_err(io)?;
            stdin.write_all(payload.as_ref()).await.map_err(io)?;
            stdin.shutdown().await.map_err(io)?;
            drop(stdin);
            Ok::<(), ProcessError>(())
        };
        let read = async {
            let length = stdout.read_u64().await.map_err(frame_io)?;
            let length = usize::try_from(length).map_err(|_| ProcessError::OutputBound)?;
            if length > bound {
                return Err(ProcessError::OutputBound);
            }
            let mut out = Vec::new();
            out.try_reserve_exact(length)
                .map_err(|_| ProcessError::Allocation)?;
            out.resize(length, 0);
            stdout.read_exact(&mut out).await.map_err(frame_io)?;
            let mut trailing = [0];
            if stdout.read(&mut trailing).await.map_err(io)? != 0 {
                return Err(ProcessError::TrailingBytes);
            }
            Ok(out)
        };
        let (_, out) = tokio::try_join!(write, read)?;
        if !child.wait().await.map_err(io)?.success() {
            return Err(ProcessError::Exit);
        }
        Ok(out)
    };
    let result = tokio::select! {
        result = work => result,
        _ = tokio::time::sleep_until(deadline) => Err(ProcessError::Deadline),
        _ = sender.closed() => Err(ProcessError::Cancelled),
        _ = async {
            if *shutdown.borrow() { return; }
            let _ = shutdown.changed().await;
        } => Err(ProcessError::Shutdown),
    };
    #[cfg(test)]
    {
        let gate = inner.cleanup_gate.lock().unwrap().clone();
        if let Some(gate) = gate {
            gate.observed_pid
                .store(child.id().unwrap_or(0), std::sync::atomic::Ordering::SeqCst);
            eprintln!(
                "private cleanup phase: helper={} pid={:?} status={:?} result={:?}",
                inner.helper.display(),
                child.id(),
                child.try_wait(),
                result.as_ref().map(Vec::len)
            );
            gate.entered.add_permits(1);
            gate.release.acquire().await.unwrap().forget();
        }
    }
    // Even a correct frame cannot leave a wedged child. wait may already have
    // reaped success; repeated wait is supported, kill-on-exit error is benign.
    let _ = child.start_kill();
    let unverified = child.wait().await.is_err();
    #[cfg(test)]
    let unverified = unverified
        | inner
            .cleanup_error
            .swap(false, std::sync::atomic::Ordering::SeqCst);
    if unverified {
        fail_cleanup(inner);
        return Err(ProcessError::CleanupUnverified);
    }
    result
}

fn io(e: std::io::Error) -> ProcessError {
    ProcessError::Io(e.to_string())
}
fn frame_io(e: std::io::Error) -> ProcessError {
    if e.kind() == std::io::ErrorKind::UnexpectedEof {
        ProcessError::Truncated
    } else {
        io(e)
    }
}

fn fail_cleanup(inner: &Inner) {
    let mut state = inner.state.lock().unwrap_or_else(|e| e.into_inner());
    state.cleanup_failed = true;
    state.closing = true;
    inner.permits.close();
    inner.shutdown.send_replace(true);
}

#[cfg(test)]
struct CleanupGate {
    entered: Semaphore,
    release: Semaphore,
    observed_pid: std::sync::atomic::AtomicU32,
}

#[cfg(test)]
mod cleanup_tests {
    use super::*;
    #[tokio::test]
    async fn unknown_cleanup_closes_reuse_and_reports_shutdown_failure() {
        let pool =
            ProcessPool::new(PathBuf::from("unused"), NonZeroUsize::new(1).unwrap()).unwrap();
        fail_cleanup(&pool.inner);
        assert_eq!(
            pool.exchange(
                Instant::now() + std::time::Duration::from_secs(1),
                vec![],
                vec![],
                1
            )
            .await,
            Err(ProcessError::AdmissionClosed)
        );
        assert_eq!(pool.shutdown().await, Err(ProcessError::CleanupUnverified));
    }
    #[tokio::test]
    async fn both_shutdown_waiters_observe_injected_supervisor_panic() {
        let pool =
            ProcessPool::new(PathBuf::from("unused"), NonZeroUsize::new(1).unwrap()).unwrap();
        let accounting = Arc::new(Semaphore::new(1));
        let lease = Arc::new(accounting.clone().acquire_owned().await.unwrap());
        let active = Active {
            inner: pool.inner.clone(),
            completed: false,
            cleanup_verified: false,
            permit: None,
            lease: Some(lease),
        };
        let task = tokio::spawn(async move {
            let _active = active;
            panic!("injected supervisor fault");
        });
        {
            let mut state = pool.inner.state.lock().unwrap();
            state.active = 1;
            state.tasks.push(task);
        }
        let (first, second) = tokio::join!(pool.shutdown(), pool.shutdown());
        assert_eq!(first, Err(ProcessError::Supervisor));
        assert_eq!(second, Err(ProcessError::Supervisor));
        assert_eq!(pool.shutdown().await, Err(ProcessError::Supervisor));
        assert_eq!(
            accounting.available_permits(),
            0,
            "panic must not reopen caller accounting"
        );
    }
    #[cfg(unix)]
    struct ChargedPayload {
        bytes: ssx_owned_budget::OwnedVec<u8>,
        dropped: Arc<std::sync::atomic::AtomicBool>,
    }
    #[cfg(unix)]
    impl AsRef<[u8]> for ChargedPayload {
        fn as_ref(&self) -> &[u8] {
            self.bytes.as_slice()
        }
    }
    #[cfg(unix)]
    impl Drop for ChargedPayload {
        fn drop(&mut self) {
            self.dropped
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    #[cfg(unix)]
    fn charged_payload() -> (
        ChargedPayload,
        ssx_owned_budget::Budget,
        Arc<std::sync::atomic::AtomicBool>,
    ) {
        let budget = ssx_owned_budget::Budget::new(131072).unwrap();
        let mut bytes = ssx_owned_budget::OwnedVec::with_capacity(&budget, 65536).unwrap();
        bytes.try_extend_from_slice(&[7; 65536]).unwrap();
        let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        (
            ChargedPayload {
                bytes,
                dropped: dropped.clone(),
            },
            budget,
            dropped,
        )
    }
    #[cfg(unix)]
    fn fixture(body: &str) -> (PathBuf, PathBuf) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        fixture_at_stamp(body, stamp)
    }
    #[cfg(unix)]
    fn fixture_at_stamp(body: &str, stamp: u128) -> (PathBuf, PathBuf) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        for _ in 0..64 {
            let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "a06-lease-{}-{stamp}-{sequence}",
                std::process::id()
            ));
            match std::fs::create_dir(&directory) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create private fixture directory: {error}"),
            }
            let path = directory.join("worker");
            let pid = directory.join("worker.pid");
            std::fs::write(
                &path,
                format!("#!/bin/sh\necho $$ > '{}'\n{body}\n", pid.display()),
            )
            .unwrap();
            eprintln!(
                "private fixture: helper={} pidfile={}",
                path.display(),
                pid.display()
            );
            return (path, pid);
        }
        panic!("private fixture directory collision limit");
    }
    #[cfg(unix)]
    fn fixture_pool(path: &std::path::Path) -> ProcessPool {
        let mut pool =
            ProcessPool::new(PathBuf::from("/bin/sh"), NonZeroUsize::new(1).unwrap()).unwrap();
        // Configured exactly once before any clone or dispatch; no shared
        // mutation, retry, or production command change.
        Arc::get_mut(&mut pool.inner).unwrap().test_args = vec![path.as_os_str().to_owned()];
        pool
    }
    #[cfg(unix)]
    fn cleanup_fixture(path: PathBuf, pid: PathBuf) {
        let directory = path.parent().unwrap().to_owned();
        let _ = std::fs::remove_file(path.with_extension("ready"));
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(pid);
        let _ = std::fs::remove_dir(directory);
    }
    #[cfg(unix)]
    #[test]
    fn concurrent_fixtures_with_identical_clock_have_exclusive_identities() {
        let threads = (0..16)
            .map(|slot| {
                std::thread::spawn(move || {
                    let body = format!("#fixture-slot-{slot}");
                    let (path, pid) = fixture_at_stamp(&body, 0);
                    (path, pid, body)
                })
            })
            .collect::<Vec<_>>();
        let rows = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();
        let distinct = rows
            .iter()
            .map(|(path, _, _)| path)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(distinct.len(), 16);
        for (path, pid, body) in rows {
            let source = std::fs::read_to_string(&path).unwrap();
            assert!(source.ends_with(&format!("{body}\n")));
            assert!(source.contains(&format!("echo $$ > '{}'", pid.display())));
            cleanup_fixture(path, pid);
        }
    }
    #[cfg(unix)]
    fn complete_pid_record(s: &str) -> Option<&str> {
        let pid = s.trim();
        (s.ends_with('\n') && pid.parse::<u32>().is_ok_and(|pid| pid > 0)).then_some(pid)
    }
    #[cfg(unix)]
    #[test]
    fn incomplete_pid_record_is_not_child_readiness() {
        for raw in ["", "123", "0\n", "-1\n", "bad\n"] {
            assert_eq!(complete_pid_record(raw), None);
        }
        assert_eq!(complete_pid_record("123\n"), Some("123"));
    }
    #[cfg(unix)]
    async fn wait_pid_record(path: &PathBuf) -> Option<String> {
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if let Ok(s) = std::fs::read_to_string(path)
                    && let Some(pid) = complete_pid_record(&s)
                {
                    break pid.to_owned();
                }
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            }
        })
        .await
        .ok()
    }
    #[cfg(unix)]
    async fn actual_pid(path: &PathBuf) -> String {
        wait_pid_record(path)
            .await
            .expect("complete child PID record")
    }
    #[cfg(unix)]
    async fn release_gate_and_shutdown(
        pool: &ProcessPool,
        gate: &CleanupGate,
    ) -> Result<(), String> {
        gate.release.add_permits(1);
        tokio::time::timeout(std::time::Duration::from_secs(2), pool.shutdown())
            .await
            .map_err(|_| "test shutdown deadline".to_owned())?
            .map_err(|error| format!("test shutdown: {error:?}"))
    }
    #[cfg(unix)]
    fn pid_gone(pid: &str) -> bool {
        let observation = std::process::Command::new("/bin/kill")
            .args(["-0", pid])
            .output()
            .unwrap();
        eprintln!(
            "private PID probe: pid={pid} exit={:?} stderr={}",
            observation.status.code(),
            String::from_utf8_lossy(&observation.stderr)
        );
        !observation.status.success()
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn cancelled_caller_lease_survives_blocked_cleanup_until_real_reap() {
        let (path, pidfile) =
            fixture("IFS= read -r first; printf 'ready\\n' > \"$0.ready\"; while :; do :; done");
        let pool = fixture_pool(&path);
        let gate = Arc::new(CleanupGate {
            entered: Semaphore::new(0),
            release: Semaphore::new(0),
            observed_pid: std::sync::atomic::AtomicU32::new(0),
        });
        *pool.inner.cleanup_gate.lock().unwrap() = Some(gate.clone());
        let (payload, payload_budget, payload_dropped) = charged_payload();
        let accounting = Arc::new(Semaphore::new(1));
        let lease = Arc::new(accounting.clone().acquire_owned().await.unwrap());
        let weak = Arc::downgrade(&lease);
        let worker = pool.clone();
        let caller = tokio::spawn(async move {
            worker
                .exchange_with_lease(
                    Instant::now() + std::time::Duration::from_secs(10),
                    b"start\n".to_vec(),
                    payload,
                    2,
                    lease,
                )
                .await
        });
        let Some(pid) = wait_pid_record(&pidfile).await else {
            caller.abort();
            let _ = caller.await;
            let cleanup = release_gate_and_shutdown(&pool, &gate).await;
            panic!("child PID readiness failed; cleanup={cleanup:?}");
        };
        let readyfile = path.with_extension("ready");
        let ready = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if std::fs::read(&readyfile).is_ok_and(|bytes| bytes == b"ready\n") {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;
        if ready.is_err() {
            caller.abort();
            let _ = caller.await;
            let cleanup = release_gate_and_shutdown(&pool, &gate).await;
            panic!("child did not acknowledge consumed request header; cleanup={cleanup:?}");
        }
        caller.abort();
        let _ = caller.await;
        match tokio::time::timeout(std::time::Duration::from_secs(2), gate.entered.acquire()).await
        {
            Ok(Ok(permit)) => permit.forget(),
            error => {
                let cleanup = release_gate_and_shutdown(&pool, &gate).await;
                panic!("cleanup gate entry failed: {error:?}; cleanup={cleanup:?}");
            }
        }
        let held_payload = payload_budget.snapshot();
        let held_drop = payload_dropped.load(std::sync::atomic::Ordering::SeqCst);
        let observed_pid = gate.observed_pid.load(std::sync::atomic::Ordering::SeqCst);
        let blocked = accounting.clone().try_acquire_owned().is_err();
        let alive = !pid_gone(&pid);
        let slots = pool.available_slots();
        release_gate_and_shutdown(&pool, &gate).await.unwrap();
        assert_eq!(held_payload.unwrap().used, 65536);
        assert!(!held_drop);
        assert_eq!(observed_pid, pid.parse::<u32>().unwrap());
        assert_eq!(payload_budget.snapshot().unwrap().used, 0);
        assert!(payload_dropped.load(std::sync::atomic::Ordering::SeqCst));
        assert!(blocked, "caller accounting reopened before cleanup");
        assert!(alive, "gate did not block actual live child cleanup");
        assert_eq!(slots, 0);
        assert_eq!(accounting.available_permits(), 1);
        assert!(weak.upgrade().is_none());
        assert!(pid_gone(&pid));
        cleanup_fixture(path, pidfile);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn missing_request_readiness_releases_gate_before_bounded_shutdown() {
        let (path, pidfile) = fixture("exec /bin/sleep 30");
        let pool = fixture_pool(&path);
        let gate = Arc::new(CleanupGate {
            entered: Semaphore::new(0),
            release: Semaphore::new(0),
            observed_pid: std::sync::atomic::AtomicU32::new(0),
        });
        *pool.inner.cleanup_gate.lock().unwrap() = Some(gate.clone());
        let (payload, budget, dropped) = charged_payload();
        let worker = pool.clone();
        let caller = tokio::spawn(async move {
            worker
                .exchange_with_lease(
                    Instant::now() + std::time::Duration::from_secs(10),
                    vec![],
                    payload,
                    2,
                    (),
                )
                .await
        });
        let pid = wait_pid_record(&pidfile).await;
        let missing = !path.with_extension("ready").exists();
        // Model the readiness-failure branch: release first, then bound drain.
        // No assertion can leave a supervisor wedged behind this gate.
        let cleanup = release_gate_and_shutdown(&pool, &gate).await;
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), caller).await;
        assert!(missing);
        cleanup.unwrap();
        assert_eq!(result.unwrap().unwrap(), Err(ProcessError::Shutdown));
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
        assert!(pid_gone(&pid.expect("complete child PID before shutdown")));
        cleanup_fixture(path, pidfile);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn verified_success_and_protocol_error_release_lease_after_cleanup() {
        for (index, body) in [
            "/bin/cat >/dev/null; printf '\\000\\000\\000\\000\\000\\000\\000\\002ok'",
            "/bin/cat >/dev/null; printf x",
        ]
        .into_iter()
        .enumerate()
        {
            let (path, pidfile) = fixture(body);
            let pool = fixture_pool(&path);
            let (payload, payload_budget, payload_dropped) = charged_payload();
            let accounting = Arc::new(Semaphore::new(1));
            let lease = Arc::new(accounting.clone().acquire_owned().await.unwrap());
            let result = pool
                .exchange_with_lease(
                    Instant::now() + std::time::Duration::from_secs(2),
                    vec![],
                    payload,
                    2,
                    lease,
                )
                .await;
            assert_eq!(
                result,
                if index == 0 {
                    Ok(b"ok".to_vec())
                } else {
                    Err(ProcessError::Truncated)
                }
            );
            pool.shutdown().await.unwrap();
            assert_eq!(payload_budget.snapshot().unwrap().used, 0);
            assert!(payload_dropped.load(std::sync::atomic::Ordering::SeqCst));
            assert_eq!(accounting.available_permits(), 1);
            assert!(pid_gone(&actual_pid(&pidfile).await));
            cleanup_fixture(path, pidfile);
        }
    }
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn held_writer_refuses_direct_exec_but_script_interpreter_preserves_protocol_and_cleanup()
    {
        use std::os::unix::fs::PermissionsExt;
        let (path, pidfile) =
            fixture("/bin/cat >/dev/null; printf '\\000\\000\\000\\000\\000\\000\\000\\002ok'");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let held_writer = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        let direct_error = std::process::Command::new(&path).spawn().unwrap_err();
        assert_eq!(direct_error.raw_os_error(), Some(26));
        let direct = ProcessPool::new(path.clone(), NonZeroUsize::new(1).unwrap()).unwrap();
        assert!(direct.inner.test_args.is_empty());
        assert_eq!(
            direct
                .exchange(
                    Instant::now() + std::time::Duration::from_secs(2),
                    vec![],
                    vec![],
                    2
                )
                .await,
            Err(ProcessError::Spawn(direct_error.to_string()))
        );
        direct.shutdown().await.unwrap();
        assert!(!pidfile.exists());
        let interpreted = fixture_pool(&path);
        let (payload, budget, dropped) = charged_payload();
        let accounting = Arc::new(Semaphore::new(1));
        let lease = Arc::new(accounting.clone().acquire_owned().await.unwrap());
        assert_eq!(
            interpreted
                .exchange_with_lease(
                    Instant::now() + std::time::Duration::from_secs(2),
                    vec![],
                    payload,
                    2,
                    lease
                )
                .await,
            Ok(b"ok".to_vec())
        );
        interpreted.shutdown().await.unwrap();
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(accounting.available_permits(), 1);
        assert_eq!(interpreted.available_slots(), 1);
        assert!(pid_gone(&actual_pid(&pidfile).await));
        drop(held_writer);
        cleanup_fixture(path, pidfile);
        println!(
            "LINUX_ETXTBSY_CONTROL direct-rejected:interpreter-exact-protocol:reaped:released"
        );
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn missing_direct_helper_returns_spawn_error_and_releases_payload_and_lease() {
        let (path, pidfile) = fixture("exit 99");
        let missing = path.with_extension("missing");
        let expected = std::process::Command::new(&missing)
            .spawn()
            .unwrap_err()
            .to_string();
        let pool = ProcessPool::new(missing, NonZeroUsize::new(1).unwrap()).unwrap();
        assert!(pool.inner.test_args.is_empty());
        let (payload, budget, dropped) = charged_payload();
        let accounting = Arc::new(Semaphore::new(1));
        let lease = Arc::new(accounting.clone().acquire_owned().await.unwrap());
        assert_eq!(
            pool.exchange_with_lease(
                Instant::now() + std::time::Duration::from_secs(2),
                vec![],
                payload,
                2,
                lease
            )
            .await,
            Err(ProcessError::Spawn(expected))
        );
        pool.shutdown().await.unwrap();
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(accounting.available_permits(), 1);
        assert!(!pidfile.exists());
        cleanup_fixture(path, pidfile);
        println!("A10_FIXTURE_COMPLETE direct-spawn-error-released");
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn closed_receiver_during_success_delivery_retains_lease_until_reply_drop() {
        let (path, pidfile) =
            fixture("/bin/cat >/dev/null; printf '\\000\\000\\000\\000\\000\\000\\000\\002ok'");
        let pool = fixture_pool(&path);
        let gate = Arc::new(CleanupGate {
            entered: Semaphore::new(0),
            release: Semaphore::new(0),
            observed_pid: std::sync::atomic::AtomicU32::new(0),
        });
        *pool.inner.delivery_gate.lock().unwrap() = Some(gate.clone());
        let (payload, payload_budget, payload_dropped) = charged_payload();
        let accounting = Arc::new(Semaphore::new(1));
        let lease = Arc::new(accounting.clone().acquire_owned().await.unwrap());
        let worker = pool.clone();
        let caller = tokio::spawn(async move {
            worker
                .exchange_with_lease(
                    Instant::now() + std::time::Duration::from_secs(10),
                    vec![],
                    payload,
                    2,
                    lease,
                )
                .await
        });
        match tokio::time::timeout(std::time::Duration::from_secs(2), gate.entered.acquire()).await
        {
            Ok(Ok(permit)) => permit.forget(),
            error => {
                caller.abort();
                let _ = caller.await;
                let cleanup = release_gate_and_shutdown(&pool, &gate).await;
                panic!("delivery gate entry failed: {error:?}; cleanup={cleanup:?}");
            }
        }
        caller.abort();
        let _ = caller.await;
        let held_admission = accounting.available_permits();
        let held_payload = payload_budget.snapshot();
        let held_drop = payload_dropped.load(std::sync::atomic::Ordering::SeqCst);
        release_gate_and_shutdown(&pool, &gate).await.unwrap();
        assert_eq!(held_admission, 0);
        assert_eq!(held_payload.unwrap().used, 65536);
        assert!(!held_drop);
        assert_eq!(payload_budget.snapshot().unwrap().used, 0);
        assert!(payload_dropped.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(accounting.available_permits(), 1);
        assert!(pid_gone(&actual_pid(&pidfile).await));
        cleanup_fixture(path, pidfile);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn charged_payload_survives_supervisor_panic_without_false_drain() {
        let pool =
            ProcessPool::new(PathBuf::from("unused"), NonZeroUsize::new(1).unwrap()).unwrap();
        let (payload, budget, dropped) = charged_payload();
        let active = Active {
            inner: pool.inner.clone(),
            completed: false,
            cleanup_verified: false,
            permit: Some(pool.inner.permits.clone().acquire_owned().await.unwrap()),
            lease: Some((payload, ())),
        };
        {
            let mut state = pool.inner.state.lock().unwrap();
            state.active += 1;
        }
        let task = tokio::spawn(async move {
            let _active = active;
            panic!("injected charged-payload supervisor fault");
        });
        pool.inner.state.lock().unwrap().tasks.push(task);
        assert_eq!(pool.shutdown().await, Err(ProcessError::Supervisor));
        assert_eq!(pool.available_slots(), 0);
        assert_eq!(budget.snapshot().unwrap().used, 65536);
        assert!(!dropped.load(std::sync::atomic::Ordering::SeqCst));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn charged_payload_releases_after_write_and_read_failures() {
        for write_failure in [false, true] {
            let (path, pidfile) = fixture(if write_failure {
                "exec 0<&-; while :; do :; done"
            } else {
                "/bin/cat >/dev/null; printf x"
            });
            let pool = fixture_pool(&path);
            let (payload, budget, dropped) = charged_payload();
            let header_budget = ssx_owned_budget::Budget::new(8 * 1024 * 1024).unwrap();
            let header_charge = header_budget.reserve(4 * 1024 * 1024).unwrap();
            let header = if write_failure {
                vec![7; 4 * 1024 * 1024]
            } else {
                Vec::new()
            };
            let result = pool
                .exchange_with_lease(
                    Instant::now() + std::time::Duration::from_secs(5),
                    header,
                    payload,
                    2,
                    header_charge,
                )
                .await;
            eprintln!("private pipe failure: write={write_failure} result={result:?}");
            if write_failure {
                assert!(matches!(result, Err(ProcessError::Io(_))));
            } else {
                assert_eq!(result, Err(ProcessError::Truncated));
            }
            pool.shutdown().await.unwrap();
            assert_eq!(budget.snapshot().unwrap().used, 0);
            assert_eq!(header_budget.snapshot().unwrap().used, 0);
            assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
            assert!(pid_gone(&actual_pid(&pidfile).await));
            cleanup_fixture(path, pidfile);
        }
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn unknown_cleanup_retains_lease_and_permit_and_reports_failed_drain() {
        let (path, pidfile) =
            fixture("/bin/cat >/dev/null; printf '\\000\\000\\000\\000\\000\\000\\000\\002ok'");
        let pool = fixture_pool(&path);
        pool.inner
            .cleanup_error
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let (payload, payload_budget, payload_dropped) = charged_payload();
        let accounting = Arc::new(Semaphore::new(1));
        let lease = Arc::new(accounting.clone().acquire_owned().await.unwrap());
        let weak = Arc::downgrade(&lease);
        let worker = pool.clone();
        let caller = tokio::spawn(async move {
            worker
                .exchange_with_lease(
                    Instant::now() + std::time::Duration::from_secs(10),
                    vec![],
                    payload,
                    2,
                    lease,
                )
                .await
        });
        let pid = actual_pid(&pidfile).await;
        assert_eq!(caller.await.unwrap(), Err(ProcessError::CleanupUnverified));
        assert_eq!(pool.shutdown().await, Err(ProcessError::CleanupUnverified));
        assert_eq!(pool.available_slots(), 0);
        assert_eq!(payload_budget.snapshot().unwrap().used, 65536);
        assert!(!payload_dropped.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(accounting.available_permits(), 0);
        assert!(weak.upgrade().is_some());
        assert!(
            pid_gone(&pid),
            "injected uncertainty follows actual successful reap"
        );
        cleanup_fixture(path, pidfile);
    }
}
