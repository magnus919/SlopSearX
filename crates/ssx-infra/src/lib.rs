//! Native infrastructure and isolated foundation fixtures.
pub mod adapters;
pub mod html;
pub mod http;
pub mod jev;
pub mod observability;
pub mod search;
pub mod suggest;
pub mod valkey;
use ssx_application::{
    Audit, Clock, Engine, Metrics, NoopSink, PortFuture, Runtime, Store, Supervisor, TaskEvent,
};
use ssx_domain::{AdapterOutcome, StoreRead};
use std::sync::{Arc, Mutex};
pub struct FakeEngine(pub AdapterOutcome);
impl Engine for FakeEngine {
    fn probe(&self) -> PortFuture<'_, AdapterOutcome> {
        Box::pin(async { self.0.clone() })
    }
}
pub struct MemoryStore(pub StoreRead<()>);
impl Store for MemoryStore {
    fn probe(&self) -> PortFuture<'_, StoreRead<()>> {
        Box::pin(async { self.0.clone() })
    }
}
pub struct FixedClock(pub u64);
impl Clock for FixedClock {
    fn unix_seconds(&self) -> u64 {
        self.0
    }
    fn monotonic_elapsed(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }
}
#[derive(Default)]
pub struct RecordingSink(pub Mutex<Vec<&'static str>>, pub Mutex<Vec<TaskEvent>>);
impl Metrics for RecordingSink {
    fn increment(&self, name: &'static str) {
        self.0.lock().expect("sink mutex").push(name);
    }
}
impl Audit for RecordingSink {
    fn record(&self, event: &'static str) {
        self.0.lock().expect("sink mutex").push(event);
    }
}
impl Supervisor for RecordingSink {
    fn observed(&self, event: TaskEvent) {
        self.1.lock().expect("sink mutex").push(event);
    }
}
pub fn foundation_runtime() -> Runtime {
    Runtime {
        engine: Arc::new(FakeEngine(AdapterOutcome::Success)),
        store: Arc::new(MemoryStore(StoreRead::Found(()))),
        clock: Arc::new(FixedClock(0)),
        metrics: Arc::new(NoopSink),
        audit: Arc::new(NoopSink),
        supervisor: Arc::new(NoopSink),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ssx_application::TaskState;

    #[tokio::test]
    async fn unavailable_is_not_missing_or_ready() {
        let mut runtime = foundation_runtime();
        runtime.store = Arc::new(MemoryStore(StoreRead::Unavailable));
        assert!(!runtime.probe().await);
        runtime.store = Arc::new(MemoryStore(StoreRead::Missing));
        assert!(!runtime.probe().await);
        runtime.store = Arc::new(MemoryStore(StoreRead::Found(())));
        runtime.engine = Arc::new(FakeEngine(AdapterOutcome::InternalError));
        assert!(!runtime.probe().await);
    }
    #[tokio::test]
    async fn injected_sinks_observe_real_readiness() {
        let mut runtime = foundation_runtime();
        let sink = Arc::new(RecordingSink::default());
        runtime.metrics = sink.clone();
        runtime.audit = sink.clone();
        runtime.supervisor = sink.clone();
        assert!(runtime.probe().await);
        assert_eq!(sink.0.lock().unwrap().len(), 2);
        assert_eq!(
            sink.1
                .lock()
                .unwrap()
                .iter()
                .map(|event| event.state)
                .collect::<Vec<_>>(),
            vec![TaskState::Started, TaskState::Completed]
        );
        runtime.engine = Arc::new(FakeEngine(AdapterOutcome::InternalError));
        assert!(!runtime.probe().await);
        assert_eq!(
            sink.1.lock().unwrap().last().unwrap().state,
            TaskState::Failed
        );
        sink.observed(TaskEvent {
            task: "cancelled_worker",
            state: TaskState::Cancelled,
        });
        assert_eq!(
            sink.1.lock().unwrap().last().unwrap().state,
            TaskState::Cancelled
        );
        assert_eq!(runtime.clock.monotonic_elapsed(), std::time::Duration::ZERO);
        assert_eq!(runtime.clock.unix_seconds(), 0);
    }
}

pub mod browser_identity;

pub mod process_pool;

mod html_budget;
pub mod rerank;

pub mod research_runner;

pub mod saved_runner;

pub mod saved_events;
pub mod staged_runner;

pub mod staged_service;

pub mod research_planning;

/// Concrete production composition; foundation fixtures are never selected here.
pub mod composition;
