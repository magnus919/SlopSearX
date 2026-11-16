//! Unit-only pending exchange owner. There is no reply sender or arbitrary transport input.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
#[derive(Default)]
struct Counts {
    started: AtomicUsize,
    dropped: AtomicUsize,
    notify: tokio::sync::Notify,
}
struct PendingOnly(Arc<Counts>);
struct Active(Arc<Counts>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}
impl Transport for PendingOnly {
    fn exchange(&self, _: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let _active = Active(self.0.clone());
            self.0.started.fetch_add(1, Ordering::SeqCst);
            self.0.notify.notify_one();
            std::future::pending().await
        })
    }
}
#[derive(Clone)]
pub(crate) struct PendingObserver {
    counts: Arc<Counts>,
    clock: DeadlineClock,
}
impl PendingObserver {
    pub(crate) fn started(&self) -> usize {
        self.counts.started.load(Ordering::SeqCst)
    }
    pub(crate) fn dropped(&self) -> usize {
        self.counts.dropped.load(Ordering::SeqCst)
    }
    pub(crate) async fn started_signal(&self) {
        loop {
            let notified = self.counts.notify.notified();
            if self.started() != 0 {
                return;
            }
            notified.await;
        }
    }
    pub(crate) fn context(
        &self,
        driver: &monotonic::Driver,
        duration: Duration,
    ) -> ssx_application::SearchContext {
        assert!(
            self.clock.matches_driver(driver),
            "context driver must belong to this owner"
        );
        ssx_application::SearchContext {
            deadline: driver.now().checked_add(duration).unwrap(),
        }
    }
    pub(crate) fn deadlines(&self) -> (Instant, Instant) {
        self.clock.deadlines()
    }
    pub(crate) fn hit(&self) -> Option<(Instant, Instant)> {
        self.clock.hit()
    }
}
pub(super) fn owner(manual: bool) -> (AdapterHttp, PendingObserver, Option<monotonic::Driver>) {
    let (clock, driver) = DeadlineClock::pending_owner(manual);
    let counts = Arc::new(Counts::default());
    let mut owner = AdapterHttp::default();
    owner.clock = clock.clone();
    // Only this closed pending implementation can inhabit this constructor.
    // Public set_transport rejects replacement for the sealed owner.
    owner.state.lock().unwrap().injected = Some(Arc::new(PendingOnly(counts.clone())));
    (owner, PendingObserver { counts, clock }, driver)
}
