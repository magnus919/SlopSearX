//! Test-owned loopback tasks: stopping never assumes that HTTP dispatched.
use std::{future::Future, time::Duration};
use tokio::{sync::oneshot, task::JoinHandle};

pub struct OwnedLoopback<T> {
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Option<T>>>,
}
impl<T: Send + 'static> OwnedLoopback<T> {
    pub fn spawn(work: impl Future<Output = T> + Send + 'static) -> Self {
        let (stop, mut stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            tokio::select! { biased;
                _ = &mut stopped => None,
                result = work => Some(result),
            }
        });
        Self {
            stop: Some(stop),
            task: Some(task),
        }
    }
    fn signal_stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
    async fn join(&mut self) -> Option<T> {
        let joined = match tokio::time::timeout(
            Duration::from_secs(3),
            self.task.as_mut().expect("owned fixture task"),
        )
        .await
        {
            Ok(joined) => joined,
            Err(_) => {
                self.signal_stop();
                self.task.as_ref().expect("owned fixture task").abort();
                let stopped = match tokio::time::timeout(
                    Duration::from_secs(3),
                    self.task.as_mut().expect("owned fixture task"),
                )
                .await
                {
                    Ok(stopped) => stopped,
                    Err(_) => panic!(
                        "loopback fixture abort requested but join not observed within cleanup budget"
                    ),
                };
                self.task.take();
                match stopped {
                    Err(error) if error.is_cancelled() => {
                        panic!("loopback fixture exceeded cleanup budget; abort was joined")
                    }
                    Err(error) => panic!("loopback fixture failed during cleanup: {error}"),
                    Ok(_) => panic!("loopback fixture exceeded cleanup budget before joining"),
                }
            }
        };
        self.task.take();
        joined.expect("loopback fixture task panicked")
    }
    /// Stop accepts, header reads and delayed replies, then observe actual task exit.
    pub async fn stop_and_join(mut self) -> Option<T> {
        self.signal_stop();
        self.join().await
    }
    /// A wire assertion requires the fixture's actual result, never a stopped task.
    pub async fn finish(mut self) -> T {
        self.join()
            .await
            .expect("loopback fixture stopped before its required wire proof")
    }
}
impl<T> Drop for OwnedLoopback<T> {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        // Panic/caller cancellation requests abort. Only explicit normal paths observe join.
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
