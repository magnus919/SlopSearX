//! Closed observer vocabulary; composition adapters must preserve these outcomes.
use super::codec::EventType;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOutcome {
    Delivered,
    Empty,
    Gap,
    Redacted,
    Rejected,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckOutcome {
    Advanced,
    Idempotent,
    Rejected,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capacity {
    Stream,
    Consumer,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BacklogAge(f64);
impl BacklogAge {
    pub fn new(seconds: f64) -> Result<Self, ssx_domain::DomainError> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(ssx_domain::DomainError("invalid event backlog age"));
        }
        Ok(Self(seconds))
    }
    pub fn seconds(self) -> f64 {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum EventObservation {
    Published(EventType),
    Read {
        outcome: ReadOutcome,
        oldest_age_seconds: Option<BacklogAge>,
    },
    Ack(AckOutcome),
    Capacity(Capacity),
}
pub trait EventObserver: Send + Sync {
    fn observe(&self, event: EventObservation);
}
/// Explicit absence of an exporter; this does not establish O01 metric parity.
#[derive(Default)]
pub struct NoopEventObserver;
impl EventObserver for NoopEventObserver {
    fn observe(&self, _: EventObservation) {}
}

/// Bridge finite event counters into the existing shared Metrics API. The typed
/// downstream receives the unchanged observation, including validated age. This
/// adapter does not render an exporter or synthesize a gauge from increments.
pub struct SharedEventObserver {
    metrics: std::sync::Arc<dyn ssx_application::Metrics>,
    downstream: std::sync::Arc<dyn EventObserver>,
}
impl SharedEventObserver {
    pub fn new(
        metrics: std::sync::Arc<dyn ssx_application::Metrics>,
        downstream: std::sync::Arc<dyn EventObserver>,
    ) -> Self {
        Self {
            metrics,
            downstream,
        }
    }
}
impl EventObserver for SharedEventObserver {
    fn observe(&self, event: EventObservation) {
        let counter = match &event {
            EventObservation::Published(EventType::ReportCreated) => {
                "saved_event_published_report_created"
            }
            EventObservation::Published(EventType::RunIncomparable) => {
                "saved_event_published_run_incomparable"
            }
            EventObservation::Published(EventType::RunFailed) => "saved_event_published_run_failed",
            EventObservation::Published(EventType::DefinitionPaused) => {
                "saved_event_published_definition_paused"
            }
            EventObservation::Published(EventType::DefinitionExpired) => {
                "saved_event_published_definition_expired"
            }
            EventObservation::Read {
                outcome: ReadOutcome::Delivered,
                ..
            } => "saved_event_read_delivered",
            EventObservation::Read {
                outcome: ReadOutcome::Empty,
                ..
            } => "saved_event_read_empty",
            EventObservation::Read {
                outcome: ReadOutcome::Gap,
                ..
            } => "saved_event_read_gap",
            EventObservation::Read {
                outcome: ReadOutcome::Redacted,
                ..
            } => "saved_event_read_redacted",
            EventObservation::Read {
                outcome: ReadOutcome::Rejected,
                ..
            } => "saved_event_read_rejected",
            EventObservation::Ack(AckOutcome::Advanced) => "saved_event_ack_advanced",
            EventObservation::Ack(AckOutcome::Idempotent) => "saved_event_ack_idempotent",
            EventObservation::Ack(AckOutcome::Rejected) => "saved_event_ack_rejected",
            EventObservation::Capacity(Capacity::Stream) => "saved_event_capacity_stream",
            EventObservation::Capacity(Capacity::Consumer) => "saved_event_capacity_consumer",
        };
        self.metrics.increment(counter);
        self.downstream.observe(event);
    }
}
