//! One HTTP owner's deadline clock. Production has only the zero-sized System form.
use std::time::Instant;

#[derive(Clone, Default)]
pub(crate) struct Clock {
    #[cfg(test)]
    manual: Option<std::sync::Arc<Manual>>,
}
impl Clock {
    pub(crate) fn now(&self) -> Instant {
        #[cfg(test)]
        if let Some(manual) = &self.manual {
            return *manual.now.borrow();
        }
        Instant::now()
    }
    pub(crate) async fn sleep_until(&self, deadline: Instant) {
        #[cfg(test)]
        if let Some(manual) = &self.manual {
            let mut now = manual.now.subscribe();
            loop {
                if *now.borrow_and_update() >= deadline {
                    return;
                }
                // This future and its driver retain the sender; closure is a bug.
                now.changed()
                    .await
                    .expect("manual HTTP clock owner retained");
            }
        }
        tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
    }
    #[cfg(test)]
    pub(crate) fn is_manual(&self) -> bool {
        self.manual.is_some()
    }
    #[cfg(test)]
    pub(crate) fn replay() -> (Self, Driver) {
        let origin = Instant::now();
        let manual = std::sync::Arc::new(Manual {
            origin,
            now: tokio::sync::watch::channel(origin).0,
        });
        (
            Self {
                manual: Some(manual.clone()),
            },
            Driver(manual),
        )
    }
}
#[cfg(test)]
struct Manual {
    origin: Instant,
    now: tokio::sync::watch::Sender<Instant>,
}
/// Only unit tests can name this driver; integration/production builds omit it.
#[cfg(test)]
pub(crate) struct Driver(std::sync::Arc<Manual>);
#[cfg(test)]
impl Driver {
    pub(crate) fn now(&self) -> Instant {
        *self.0.now.borrow()
    }
    pub(crate) fn advance_to(&self, elapsed: std::time::Duration) {
        let next = self
            .0
            .origin
            .checked_add(elapsed)
            .expect("bounded test clock");
        self.0.now.send_if_modified(|current| {
            assert!(next >= *current, "manual clock cannot move backwards");
            if next == *current {
                return false;
            }
            *current = next;
            true
        });
    }
    pub(crate) fn owners(&self) -> usize {
        std::sync::Arc::strong_count(&self.0)
    }
}
