#![allow(dead_code)] // Shared verifier/process support; each uses its own lifetime form.
//! Finite real-wall watchdog for an explicitly owned fixture operation.
//! No Tokio pause, fake timer, runnable spin, detached task, or timeout replacement.
use ssx_mcp::Result;
use std::{future::Future, sync::mpsc, thread, time::Duration};
use tokio_util::sync::CancellationToken;
struct Watchdog {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Watchdog {
    fn start(limit: Duration) -> Result<(Self, tokio::sync::oneshot::Receiver<()>)> {
        let (stop, receiver) = mpsc::channel();
        let (expired, notification) = tokio::sync::oneshot::channel();
        let thread = thread::Builder::new()
            .name("t06-wall-watchdog".into())
            .spawn(move || {
                if matches!(
                    receiver.recv_timeout(limit),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    let _ = expired.send(());
                }
            })?;
        Ok((
            Self {
                stop: Some(stop),
                thread: Some(thread),
            },
            notification,
        ))
    }
    fn finish(&mut self) -> Result<()> {
        self.stop.take(); // Disconnect wakes the watchdog immediately, including unwind/drop.
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| "fixture watchdog thread panicked")?;
        }
        Ok(())
    }
}
impl Drop for Watchdog {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
/// Bound a borrowed verifier operation. Its client/host/service owners remain in
/// the caller, which must execute its existing stop/shutdown path after Err.
pub async fn run<T>(
    future: impl Future<Output = Result<T>>,
    cancel: Option<CancellationToken>,
) -> Result<T> {
    run_with_limit(future, cancel, Duration::from_secs(180)).await
}
pub async fn run_with_limit<T>(
    future: impl Future<Output = Result<T>>,
    cancel: Option<CancellationToken>,
    limit: Duration,
) -> Result<T> {
    let (mut watchdog, mut expired) = Watchdog::start(limit)?;
    tokio::pin!(future);
    let result =
        tokio::select! { biased; _ = &mut expired => None, value = &mut future => Some(value) };
    watchdog.finish()?;
    match result {
        Some(result) => result,
        None => {
            if let Some(cancel) = cancel {
                cancel.cancel();
            }
            // Drop only the borrowed RPC/verifier future, not its client or host.
            // In particular the stdio caller still owns its actual Child handle.
            Err("frozen fixture real-wall watchdog expired".into())
        }
    }
}
/// SDK stdio owns a waiting task. On timeout keep its future through explicit
/// cancellation/drain, with a separate finite System-time cleanup bound.
/// A drain timeout is FAILED cleanup, never evidence of joined SDK/handlers.
pub async fn run_owned<T>(
    future: impl Future<Output = Result<T>>,
    cancel: CancellationToken,
) -> Result<T> {
    run_owned_with_limits(
        future,
        cancel,
        Duration::from_secs(180),
        Duration::from_secs(15),
    )
    .await
}
pub async fn run_owned_with_limits<T>(
    future: impl Future<Output = Result<T>>,
    cancel: CancellationToken,
    limit: Duration,
    drain: Duration,
) -> Result<T> {
    let (mut watchdog, mut expired) = Watchdog::start(limit)?;
    tokio::pin!(future);
    let result =
        tokio::select! { biased; _ = &mut expired => None, value = &mut future => Some(value) };
    watchdog.finish()?;
    match result {
        Some(result) => result,
        None => {
            cancel.cancel();
            match tokio::time::timeout(drain, &mut future).await {
                Ok(_) => {
                    Err("owned fixture real-wall watchdog expired; cancellation returned".into())
                }
                Err(_) => {
                    Err("owned fixture cancellation drain timed out; cleanup unconfirmed".into())
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn operation_errors_preserve_error_and_join_watchdog() {
        let result: Result<()> = run(async { Err("fixture failure".into()) }, None).await;
        assert_eq!(result.unwrap_err().to_string(), "fixture failure");
    }
    #[tokio::test]
    async fn real_wall_timeout_cancels_and_drains_operation() {
        let cancel = CancellationToken::new();
        let observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (c, witness) = (cancel.clone(), observed.clone());
        let operation = async move {
            c.cancelled().await;
            witness.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        };
        let start = std::time::Instant::now();
        let error = run_owned_with_limits(
            operation,
            cancel.clone(),
            Duration::from_millis(10),
            Duration::from_secs(1),
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "owned fixture real-wall watchdog expired; cancellation returned"
        );
        assert!(cancel.is_cancelled());
        assert!(observed.load(std::sync::atomic::Ordering::SeqCst));
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[tokio::test]
    async fn stuck_cancel_negative_is_failed_cleanup_with_finite_return() {
        let cancel = CancellationToken::new();
        let start = std::time::Instant::now();
        let error = run_owned_with_limits(
            std::future::pending::<Result<()>>(),
            cancel.clone(),
            Duration::from_millis(10),
            Duration::from_millis(10),
        )
        .await
        .unwrap_err();
        assert!(cancel.is_cancelled());
        assert_eq!(
            error.to_string(),
            "owned fixture cancellation drain timed out; cleanup unconfirmed"
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn watchdog_drop_wakes_and_joins_actual_worker() {
        let (stop, receiver) = mpsc::channel();
        let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed = finished.clone();
        let thread = thread::spawn(move || {
            let _ = receiver.recv_timeout(Duration::from_secs(180));
            observed.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        drop(Watchdog {
            stop: Some(stop),
            thread: Some(thread),
        });
        assert!(finished.load(std::sync::atomic::Ordering::SeqCst));
    }
}
