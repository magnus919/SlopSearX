#![cfg(unix)]
use ssx_infra::process_pool::{ProcessError, ProcessPool};
use std::{
    fs,
    num::NonZeroUsize,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::Instant;
static ID: AtomicU64 = AtomicU64::new(0);
// Keep writable script-fixture setup separate from other tests' fork/exec.
// Each test still exercises its own concurrent exchanges and shutdowns.
static FIXTURE_EXECUTION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
struct Helper {
    path: PathBuf,
    pid: PathBuf,
}
impl Helper {
    fn new(body: &str) -> Self {
        let id = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            ID.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(format!("a06-worker-{id}"));
        let pid = path.with_extension("pid");
        fs::write(
            &path,
            format!("#!/bin/sh\necho $$ > '{}'\n{body}\n", pid.display()),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self { path, pid }
    }
    fn pool(&self) -> ProcessPool {
        ProcessPool::new(self.path.clone(), NonZeroUsize::new(1).unwrap()).unwrap()
    }
    async fn pid(&self) -> String {
        for _ in 0..200 {
            if let Ok(s) = fs::read_to_string(&self.pid) {
                return s.trim().into();
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        panic!("no helper PID")
    }
}
impl Drop for Helper {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(&self.pid);
    }
}
fn gone(pid: &str) -> bool {
    !std::process::Command::new("/bin/kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success()
}
async fn slots(pool: &ProcessPool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while pool.available_slots() != 1 {
            tokio::time::sleep(Duration::from_millis(2)).await
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn exact_frame_eof_and_nonzero_exit() {
    let _fixture_execution = FIXTURE_EXECUTION.lock().await;
    for (body, expected) in [
        (
            "/bin/cat >/dev/null; printf '\\000\\000\\000\\000\\000\\000\\000\\002ok'",
            Ok(b"ok".to_vec()),
        ),
        (
            "/bin/cat >/dev/null; printf '\\000\\000\\000\\000\\000\\000\\000\\002okx'",
            Err(ProcessError::TrailingBytes),
        ),
        (
            "/bin/cat >/dev/null; printf '\\000\\000\\000\\000\\000\\000\\000\\002o'",
            Err(ProcessError::Truncated),
        ),
        (
            "/bin/cat >/dev/null; printf '\\000\\000\\000\\000\\000\\000\\000\\003xxx'",
            Err(ProcessError::OutputBound),
        ),
        (
            "/bin/cat >/dev/null; printf '\\000\\000\\000\\000\\000\\000\\000\\002ok'; exit 7",
            Err(ProcessError::Exit),
        ),
    ] {
        let h = Helper::new(body);
        let p = h.pool();
        assert_eq!(
            p.exchange(
                Instant::now() + Duration::from_secs(2),
                vec![1],
                vec![2; 65536],
                2
            )
            .await,
            expected
        );
        assert!(gone(&h.pid().await));
        p.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn deadline_cancel_saturation_and_concurrent_shutdown_reap() {
    let _fixture_execution = FIXTURE_EXECUTION.lock().await;
    let h = Helper::new("exec /bin/sleep 30");
    let p = h.pool();
    let p1 = p.clone();
    let call = tokio::spawn(async move {
        p1.exchange(Instant::now() + Duration::from_secs(20), vec![], vec![], 1)
            .await
    });
    let pid = h.pid().await;
    assert_eq!(
        p.exchange(
            Instant::now() + Duration::from_millis(20),
            vec![],
            vec![],
            1
        )
        .await,
        Err(ProcessError::Deadline)
    );
    call.abort();
    let _ = call.await;
    slots(&p).await;
    assert!(gone(&pid));
    let p1 = p.clone();
    let call = tokio::spawn(async move {
        p1.exchange(Instant::now() + Duration::from_secs(20), vec![], vec![], 1)
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while p.available_slots() != 0 {
            tokio::task::yield_now().await
        }
    })
    .await
    .unwrap();
    let second_pid = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let current = h.pid().await;
            if current != pid {
                break current;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    let (first, second) = tokio::join!(p.shutdown(), p.shutdown());
    first.unwrap();
    second.unwrap();
    assert_eq!(call.await.unwrap(), Err(ProcessError::Shutdown));
    assert!(gone(&second_pid));
    assert_eq!(
        p.exchange(Instant::now() + Duration::from_secs(1), vec![], vec![], 1)
            .await,
        Err(ProcessError::AdmissionClosed)
    );
    let h = Helper::new("exec /bin/sleep 30");
    let p = h.pool();
    assert_eq!(
        p.exchange(
            Instant::now() + Duration::from_millis(250),
            vec![],
            vec![],
            1
        )
        .await,
        Err(ProcessError::Deadline)
    );
    assert!(gone(&h.pid().await));
    p.shutdown().await.unwrap();
}

#[tokio::test]
async fn impossible_allocation_is_typed_and_child_reaped() {
    let _fixture_execution = FIXTURE_EXECUTION.lock().await;
    let h = Helper::new("/bin/cat >/dev/null; printf '\\377\\377\\377\\377\\377\\377\\377\\377'");
    let p = h.pool();
    assert_eq!(
        p.exchange(
            Instant::now() + Duration::from_secs(2),
            vec![],
            vec![],
            usize::MAX
        )
        .await,
        Err(ProcessError::Allocation)
    );
    assert!(gone(&h.pid().await));
    assert_eq!(p.available_slots(), 1);
    p.shutdown().await.unwrap();
}

#[tokio::test]
async fn delayed_supervisor_cannot_spawn_after_shutdown() {
    let _fixture_execution = FIXTURE_EXECUTION.lock().await;
    use std::{
        future::{Future, poll_fn},
        task::Poll,
    };
    let h = Helper::new("exec /bin/sleep 30");
    let p = h.pool();
    let mut request =
        Box::pin(p.exchange(Instant::now() + Duration::from_secs(5), vec![], vec![], 1));
    // Default current-thread runtime: poll admission and enqueue supervisor,
    // then close without yielding to that supervisor first.
    poll_fn(|cx| {
        assert!(request.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    p.shutdown().await.unwrap();
    assert_eq!(request.await, Err(ProcessError::Shutdown));
    assert!(!h.pid.exists(), "post-close worker executed");
}

#[tokio::test]
async fn duplex_backpressure_reads_output_before_input_is_consumed() {
    let _fixture_execution = FIXTURE_EXECUTION.lock().await;
    let h = Helper::new(
        "printf '\\000\\000\\000\\000\\000\\004\\000\\000'; /bin/dd if=/dev/zero bs=65536 count=4 2>/dev/null; /bin/cat >/dev/null",
    );
    let p = h.pool();
    let bytes = p
        .exchange(
            Instant::now() + Duration::from_secs(5),
            vec![],
            vec![7; 1024 * 1024],
            262144,
        )
        .await
        .unwrap();
    assert_eq!(bytes, vec![0; 262144]);
    assert!(gone(&h.pid().await));
    p.shutdown().await.unwrap();
}

#[test]
fn oversized_capacity_is_typed_instead_of_panicking() {
    assert!(matches!(
        ProcessPool::new(
            PathBuf::from("unused"),
            NonZeroUsize::new(tokio::sync::Semaphore::MAX_PERMITS + 1).unwrap()
        ),
        Err(ProcessError::InvalidCapacity)
    ));
}
