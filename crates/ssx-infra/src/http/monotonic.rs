//! One HTTP owner's deadline clock. Production has only the zero-sized System form.
use std::time::Instant;

#[derive(Clone, Default)]
pub(crate) struct Clock {
    #[cfg(test)]
    manual: Option<std::sync::Arc<Manual>>,
    #[cfg(test)]
    trace: Option<std::sync::Arc<Trace>>,
    #[cfg(test)]
    pending_only: bool,
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
                    self.note_hit(deadline);
                    return;
                }
                // This future and its driver retain the sender; closure is a bug.
                now.changed()
                    .await
                    .expect("manual HTTP clock owner retained");
            }
        }
        tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
        self.note_hit(deadline);
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
                trace: None,
                pending_only: false,
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

#[cfg(test)]
#[derive(Default)]
struct Trace {
    hit: std::sync::Mutex<Option<(Instant, Instant)>>,
    registered: std::sync::Mutex<Option<(Instant, Instant)>>,
}
impl Clock {
    pub(crate) fn expired(&self, deadline: Instant) -> bool {
        let expired = self.now() >= deadline;
        if expired {
            self.note_hit(deadline);
        }
        expired
    }
    fn note_hit(&self, deadline: Instant) {
        #[cfg(test)]
        if let Some(trace) = &self.trace {
            let now = self.now();
            if now >= deadline {
                *trace.hit.lock().unwrap() = Some((deadline, now));
            }
        }
        #[cfg(not(test))]
        let _ = deadline;
    }
    #[cfg(test)]
    pub(crate) fn pending_owner(manual: bool) -> (Self, Option<Driver>) {
        let (mut clock, driver) = if manual {
            let (clock, driver) = Self::replay();
            (clock, Some(driver))
        } else {
            (Self::default(), None)
        };
        clock.trace = Some(std::sync::Arc::new(Trace::default()));
        clock.pending_only = true;
        (clock, driver)
    }
    #[cfg(test)]
    pub(crate) fn is_pending_only(&self) -> bool {
        self.pending_only
    }
    #[cfg(test)]
    pub(crate) fn observed(&self) -> bool {
        self.trace.is_some() || self.is_manual()
    }
    #[cfg(test)]
    pub(crate) fn record_deadlines(&self, network: Instant, operation: Instant) {
        if let Some(trace) = &self.trace {
            *trace.registered.lock().unwrap() = Some((network, operation));
        }
    }
    #[cfg(test)]
    pub(crate) fn deadlines(&self) -> (Instant, Instant) {
        self.trace
            .as_ref()
            .unwrap()
            .registered
            .lock()
            .unwrap()
            .unwrap()
    }
    #[cfg(test)]
    pub(crate) fn hit(&self) -> Option<(Instant, Instant)> {
        *self.trace.as_ref().unwrap().hit.lock().unwrap()
    }
    #[cfg(test)]
    pub(crate) fn matches_driver(&self, driver: &Driver) -> bool {
        self.manual
            .as_ref()
            .is_some_and(|manual| std::sync::Arc::ptr_eq(manual, &driver.0))
    }
}
