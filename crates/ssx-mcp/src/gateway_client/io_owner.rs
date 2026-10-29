//! One caller-owned filesystem lane. Cancellation never forgets its JoinHandle.
use super::diagnostics::Diagnostics;
use std::{future::Future, sync::Arc, time::Instant};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoError {
    Closed,
    Cancelled,
    Deadline,
    Worker,
}
#[derive(Default)]
struct State {
    job: Option<JoinHandle<()>>,
}
#[derive(Default)]
pub struct IoOwner {
    pub(super) diagnostics: Arc<Diagnostics>,
    state: tokio::sync::Mutex<State>,
    stop: CancellationToken,
}
impl IoOwner {
    pub fn new() -> Self {
        Self::default()
    }
    pub(super) fn with_diagnostics(diagnostics: Arc<Diagnostics>) -> Self {
        Self {
            diagnostics,
            state: Default::default(),
            stop: CancellationToken::new(),
        }
    }
    pub(super) fn check(
        &self,
        deadline: Instant,
        cancel: &CancellationToken,
    ) -> Result<(), IoError> {
        if self.stop.is_cancelled() {
            return Err(IoError::Closed);
        }
        if cancel.is_cancelled() {
            return Err(IoError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(IoError::Deadline);
        }
        Ok(())
    }
    pub(super) async fn bounded<T>(
        &self,
        deadline: Instant,
        cancel: &CancellationToken,
        future: impl Future<Output = T>,
    ) -> Result<T, IoError> {
        self.check(deadline, cancel)?;
        tokio::select! {biased;
            _=self.stop.cancelled()=>Err(IoError::Closed),
            _=cancel.cancelled()=>Err(IoError::Cancelled),
            _=tokio::time::sleep_until(deadline.into())=>Err(IoError::Deadline),
            result=future=>Ok(result),
        }
    }
    /// The owner is constructed synchronously before startup. Callers borrow it;
    /// startup cancellation leaves any actual job available for explicit drain.
    pub async fn run<T: Send + 'static>(
        &self,
        deadline: Instant,
        cancel: &CancellationToken,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, IoError> {
        let mut state = self.bounded(deadline, cancel, self.state.lock()).await?;
        if let Some(job) = state.job.as_mut() {
            let joined = self.bounded(deadline, cancel, job).await?;
            state.job.take();
            joined.map_err(|_| IoError::Worker)?;
        }
        // Recheck priority before granting the only worker slot.
        if self.stop.is_cancelled() {
            return Err(IoError::Closed);
        }
        if cancel.is_cancelled() {
            return Err(IoError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(IoError::Deadline);
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        state.job = Some(tokio::task::spawn_blocking(move || {
            let _ = tx.send(work());
        }));
        let joined = self
            .bounded(
                deadline,
                cancel,
                state.job.as_mut().expect("owned worker installed"),
            )
            .await?;
        state.job.take();
        joined.map_err(|_| IoError::Worker)?;
        // Worker completion includes the send; no filesystem work survives this
        // success. A cancelled receiver releases its owned bytes in the worker.
        rx.await.map_err(|_| IoError::Worker)
    }
    pub fn close(&self) {
        self.stop.cancel();
    }
    /// Cancellation-safe: the join handle stays in state until Ready. A later
    /// shutdown caller can continue draining if this shutdown future is dropped.
    pub async fn shutdown(&self) -> Result<(), IoError> {
        self.close();
        let mut state = self.state.lock().await;
        let result = if let Some(job) = state.job.as_mut() {
            job.await.map_err(|_| IoError::Worker)
        } else {
            Ok(())
        };
        state.job.take();
        result
    }
}
impl Drop for IoOwner {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    #[tokio::test]
    async fn cancelled_waiter_retains_actual_job_until_shutdown_join() {
        let owner = Arc::new(IoOwner::new());
        let done = Arc::new(AtomicBool::new(false));
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, hold) = std::sync::mpsc::channel();
        let (o, d) = (owner.clone(), done.clone());
        let cancel = CancellationToken::new();
        let worker_cancel = cancel.clone();
        let waiter = tokio::spawn(async move {
            o.run(
                Instant::now() + std::time::Duration::from_secs(10),
                &worker_cancel,
                move || {
                    let _ = started.send(());
                    hold.recv().unwrap();
                    d.store(true, Ordering::SeqCst);
                },
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(3), ready)
            .await
            .unwrap()
            .unwrap();
        cancel.cancel();
        assert_eq!(waiter.await.unwrap(), Err(IoError::Cancelled));
        assert!(!done.load(Ordering::SeqCst));
        let mut drain = Box::pin(owner.shutdown());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), drain.as_mut())
                .await
                .is_err()
        );
        assert!(!done.load(Ordering::SeqCst));
        drop(drain);
        release.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), owner.shutdown())
            .await
            .unwrap()
            .unwrap();
        assert!(done.load(Ordering::SeqCst));
        assert!(owner.state.lock().await.job.is_none());
    }
    #[tokio::test]
    async fn deadline_queue_and_owner_close_do_not_start_second_worker() {
        let owner = Arc::new(IoOwner::new());
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, hold) = std::sync::mpsc::channel();
        let (o, cancel) = (owner.clone(), CancellationToken::new());
        let c = cancel.clone();
        let waiter = tokio::spawn(async move {
            o.run(
                Instant::now() + std::time::Duration::from_secs(10),
                &c,
                move || {
                    let _ = started.send(());
                    hold.recv().unwrap();
                },
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(3), ready)
            .await
            .unwrap()
            .unwrap();
        cancel.cancel();
        assert_eq!(waiter.await.unwrap(), Err(IoError::Cancelled));
        let second = Arc::new(AtomicBool::new(false));
        let b = second.clone();
        assert_eq!(
            owner
                .run(
                    Instant::now() + std::time::Duration::from_millis(10),
                    &CancellationToken::new(),
                    move || b.store(true, Ordering::SeqCst)
                )
                .await,
            Err(IoError::Deadline)
        );
        assert!(!second.load(Ordering::SeqCst));
        owner.close();
        assert_eq!(
            owner
                .run(
                    Instant::now() + std::time::Duration::from_secs(1),
                    &CancellationToken::new(),
                    || ()
                )
                .await,
            Err(IoError::Closed)
        );
        release.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), owner.shutdown())
            .await
            .unwrap()
            .unwrap();
        assert!(!second.load(Ordering::SeqCst));
    }
}
