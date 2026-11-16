//! Manual monotonic clock private to the explicit fake-engine qualification fixture.
//! Tokio runtime, transport timers, shared-store timers and calendar time stay System.
use ssx_application::PortFuture;
use ssx_infra::search::SearchDeadlineClock;
use std::sync::Arc;
use tokio::{sync::watch, time::Instant};
pub struct ManualDeadlineClock {
    now: watch::Sender<Instant>,
}
impl ManualDeadlineClock {
    pub fn frozen() -> Arc<Self> {
        Arc::new(Self {
            now: watch::channel(Instant::now()).0,
        })
    }
}
impl SearchDeadlineClock for ManualDeadlineClock {
    fn now(&self) -> Instant {
        *self.now.borrow()
    }
    fn sleep_until(&self, deadline: Instant) -> PortFuture<'_, ()> {
        Box::pin(async move {
            let mut now = self.now.subscribe();
            loop {
                if *now.borrow_and_update() >= deadline {
                    return;
                }
                now.changed().await.expect("manual clock retained by owner");
            }
        })
    }
}
