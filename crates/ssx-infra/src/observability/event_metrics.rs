//! Concrete O01 gauge projection for the accepted W05 static-counter bridge.
use super::Telemetry;
use crate::saved_events::observations::{
    EventObservation, EventObserver, ReadOutcome, SharedEventObserver,
};
use std::sync::Arc;
struct GaugeObserver(Arc<Telemetry>);
impl EventObserver for GaugeObserver {
    fn observe(&self, event: EventObservation) {
        if let EventObservation::Read {
            outcome,
            oldest_age_seconds,
        } = event
        {
            let _ = self.0.metrics.saved_event_age(
                matches!(outcome, ReadOutcome::Empty | ReadOutcome::Gap),
                oldest_age_seconds.map(|age| age.seconds()),
            );
        }
    }
}
impl Telemetry {
    /// Counters use W05's finite shared Metrics vocabulary exactly once; the
    /// private downstream projects only the unchanged validated backlog age.
    pub fn saved_event_observer(self: &Arc<Self>) -> Arc<dyn EventObserver> {
        Arc::new(SharedEventObserver::new(
            self.clone(),
            Arc::new(GaugeObserver(self.clone())),
        ))
    }
}
