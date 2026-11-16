//! Native event service and publication integration. The observer is a composition
//! port, not an O01 exporter. No outbound callback or provider client exists here.
pub mod codec;
pub mod observations;
use crate::valkey::saved::{
    Claim, Error, SavedStore,
    events::{AckStatus, EventError, EventLimits, Publication},
};
use codec::{ConsumerId, Cursor, Event, EventType, event_id};
use observations::{
    AckOutcome, BacklogAge, Capacity, EventObservation, EventObserver, ReadOutcome,
};
use ssx_application::policy::PolicyService;
use ssx_domain::{DomainError, TenantId};
use ssx_json::{Value, json};
use std::sync::Arc;
/// Trusted composition input. Never deserialized from a tool argument or header.
#[derive(Clone)]
pub struct EventPrincipal {
    tenant: TenantId,
    subject: String,
}
impl EventPrincipal {
    pub fn new(tenant: TenantId, subject: String) -> Result<Self, DomainError> {
        if subject.is_empty() || subject.len() > 1024 || subject.chars().any(char::is_control) {
            return Err(DomainError("invalid event principal"));
        }
        Ok(Self { tenant, subject })
    }
    pub fn tenant(&self) -> &TenantId {
        &self.tenant
    }
    pub fn subject(&self) -> &str {
        &self.subject
    }
}
pub trait EventAuthority: Send + Sync {
    fn events_granted(&self) -> bool;
    fn saved_granted(&self) -> bool;
    fn engines_allowed(&self, engines: &[String]) -> bool;
}
/// Immutable lifetime composition, matching existing native runner authority.
/// Dynamic policy deployments implement EventAuthority from live authority.
pub struct StaticEventAuthority {
    pub search: PolicyService,
    pub events_granted: bool,
    pub saved_granted: bool,
}
impl EventAuthority for StaticEventAuthority {
    fn events_granted(&self) -> bool {
        self.events_granted
    }
    fn saved_granted(&self) -> bool {
        self.saved_granted
    }
    fn engines_allowed(&self, engines: &[String]) -> bool {
        self.search.engine_rejection(engines, "engines").is_ok()
    }
}
#[derive(Clone)]
pub struct EventPublisher {
    pub limits: EventLimits,
    pub authority: Arc<dyn EventAuthority>,
    pub observer: Arc<dyn EventObserver>,
}
impl EventPublisher {
    pub async fn publish(
        &self,
        store: &SavedStore,
        claim: &Claim,
        report: &Value,
        baseline: Option<&Value>,
        next_due: f64,
        fingerprint: &str,
    ) -> Result<bool, Error> {
        if !self.authority.events_granted() {
            return store
                .commit(claim, report, baseline, next_due, fingerprint)
                .await;
        }
        let event = Event::report(claim.definition(), report, &event_id()?)?;
        let result = store
            .commit_event(
                claim,
                report,
                baseline,
                next_due,
                fingerprint,
                Publication {
                    event: &event,
                    limits: self.limits,
                },
            )
            .await;
        if matches!(result, Err(Error::OutboxCapacity)) {
            self.observer
                .observe(EventObservation::Capacity(Capacity::Stream));
        }
        let result = result?;
        if result.committed {
            self.observer.observe(EventObservation::Published(
                event.kind().ok_or(Error::Malformed)?,
            ));
        }
        Ok(result.committed)
    }
    pub async fn publish_expired_page(
        &self,
        store: &SavedStore,
        cursor: u64,
        count: usize,
    ) -> Result<u64, Error> {
        if !self.authority.events_granted() {
            return Ok(cursor);
        }
        let result = store
            .publish_expired_event_page(cursor, count, self.limits)
            .await;
        let progress = result?;
        for _ in 0..progress.published {
            self.observer
                .observe(EventObservation::Published(EventType::DefinitionExpired));
        }
        if matches!(progress.failure, Some(Error::OutboxCapacity)) {
            self.observer
                .observe(EventObservation::Capacity(Capacity::Stream));
        }
        if let Some(error) = progress.failure {
            return Err(error);
        }
        Ok(progress.cursor)
    }
}
#[derive(Clone)]
pub struct EventService {
    store: SavedStore,
    publisher: EventPublisher,
}
impl EventService {
    pub fn new(store: SavedStore, principal: &EventPrincipal, publisher: EventPublisher) -> Self {
        Self {
            store: store.for_tenant(principal.tenant.clone()),
            publisher,
        }
    }
    pub fn store(&self) -> &SavedStore {
        &self.store
    }
    pub fn publisher(&self) -> &EventPublisher {
        &self.publisher
    }
    fn grant(&self) -> Result<(), Value> {
        if self.publisher.authority.events_granted() {
            Ok(())
        } else {
            Err(error(
                "tool_disabled",
                "saved-search events require the event grant (MCP_GRANT_SAVED_SEARCH_EVENTS=1)",
            ))
        }
    }
    pub async fn read(&self, consumer: &str, cursor: Option<&str>, limit: usize) -> Value {
        let result = self.read_inner(consumer, cursor, limit).await;
        match result {
            Ok((value, outcome, age)) => {
                self.publisher.observer.observe(EventObservation::Read {
                    outcome,
                    oldest_age_seconds: age,
                });
                value
            }
            Err(e) => {
                self.publisher.observer.observe(EventObservation::Read {
                    outcome: ReadOutcome::Rejected,
                    oldest_age_seconds: None,
                });
                e
            }
        }
    }
    async fn read_inner(
        &self,
        consumer: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<(Value, ReadOutcome, Option<BacklogAge>), Value> {
        self.grant()?;
        let identity = ConsumerId::new(consumer).map_err(|e| error("invalid_input", e.0))?;
        let cursor = cursor
            .map(Cursor::new)
            .transpose()
            .map_err(|e| error("invalid_input", e.0))?;
        let batch = self
            .store
            .read_events(&identity, cursor.as_ref(), limit, self.publisher.limits)
            .await
            .map_err(map_event_error)?;
        self.grant()?;
        let mut events = Vec::new();
        let mut redacted_count = 0;
        for (cursor, event) in &batch.events {
            let redacted = !self.publisher.authority.saved_granted()
                || event
                    .engines()
                    .is_none_or(|e| !self.publisher.authority.engines_allowed(&e));
            redacted_count += usize::from(redacted);
            events.push(event.public(cursor, redacted));
        }
        let outcome = if batch.gap() {
            ReadOutcome::Gap
        } else if redacted_count > 0 {
            ReadOutcome::Redacted
        } else if events.is_empty() {
            ReadOutcome::Empty
        } else {
            ReadOutcome::Delivered
        };
        let oldest = batch
            .events
            .iter()
            .map(|(_, e)| {
                e.payload()["occurred_at"].as_f64().ok_or_else(|| {
                    error("store_unavailable", "retained event timestamp is malformed")
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .reduce(f64::min)
            .map(|at| BacklogAge::new((batch.now - at).max(0.0)))
            .transpose()
            .map_err(|_| error("store_unavailable", "retained event age is malformed"))?;
        let mut value = batch.metadata();
        value["contract"] = json!("slopsearx.saved_search_event_batch");
        value["contract_version"] = json!(1);
        value["consumer_id"] = json!(consumer);
        value["returned"] = json!(events.len());
        value["redacted"] = json!(redacted_count);
        value["events"] = Value::Array(events);
        value["delivery"] = json!("at_least_once");
        Ok((value, outcome, oldest))
    }
    pub async fn ack(&self, consumer: &str, cursor: &str) -> Value {
        let result = self.ack_inner(consumer, cursor).await;
        match result {
            Ok((value, outcome)) => {
                self.publisher
                    .observer
                    .observe(EventObservation::Ack(outcome));
                value
            }
            Err(e) => {
                self.publisher
                    .observer
                    .observe(EventObservation::Ack(AckOutcome::Rejected));
                e
            }
        }
    }
    async fn ack_inner(&self, consumer: &str, cursor: &str) -> Result<(Value, AckOutcome), Value> {
        self.grant()?;
        let identity = ConsumerId::new(consumer).map_err(|e| error("invalid_input", e.0))?;
        let cursor = Cursor::new(cursor).map_err(|e| error("invalid_input", e.0))?;
        let result = self
            .store
            .acknowledge_event(&identity, &cursor, self.publisher.limits)
            .await;
        if matches!(result, Err(EventError::ConsumerCapacity)) {
            self.publisher
                .observer
                .observe(EventObservation::Capacity(Capacity::Consumer));
        }
        let ack = result.map_err(map_event_error)?;
        Ok((
            json!({"contract":"slopsearx.saved_search_event_ack","contract_version":1,"consumer_id":consumer,"acknowledged_cursor":ack.cursor.as_str(),"idempotent":true}),
            match ack.status {
                AckStatus::Advanced => AckOutcome::Advanced,
                AckStatus::Idempotent => AckOutcome::Idempotent,
            },
        ))
    }
    pub async fn pause(
        &self,
        id: &ssx_domain::SavedSearchId,
        revision: u64,
        paused: bool,
        policy: &ssx_application::saved::SavedPolicy<'_>,
    ) -> Result<ssx_domain::saved::SavedDefinition, Error> {
        if !paused || !self.publisher.authority.events_granted() {
            return self.store.pause(id, revision, paused, policy).await;
        }
        let result = self
            .store
            .pause_event(id, revision, policy, self.publisher.limits)
            .await;
        match result {
            Ok((d, _)) => {
                self.publisher
                    .observer
                    .observe(EventObservation::Published(EventType::DefinitionPaused));
                Ok(d)
            }
            Err(Error::OutboxCapacity) => {
                self.publisher
                    .observer
                    .observe(EventObservation::Capacity(Capacity::Stream));
                Err(Error::OutboxCapacity)
            }
            Err(e) => Err(e),
        }
    }
}
pub fn error(code: &str, message: &str) -> Value {
    json!({"error":{"code":code,"message":message}})
}
fn map_event_error(e: EventError) -> Value {
    match e {
        EventError::Invalid(e) => error("invalid_input", e.0),
        EventError::LegacyAckAmbiguous => error(
            "cursor_conflict",
            "legacy acknowledgement has ambiguous tenant/consumer ownership; read with an explicit cursor and acknowledge a retained event to establish a tenant-bound cursor",
        ),
        EventError::FutureCursor => error("invalid_input", "cursor is ahead of the tenant stream"),
        EventError::CursorExpired => error(
            "cursor_expired",
            "the tenant event stream is unavailable or expired",
        ),
        EventError::ConsumerCapacity => error(
            "resource_limit",
            "saved-search event consumer capacity is exhausted",
        ),
        EventError::Store(_) => error(
            "store_unavailable",
            "saved-search events require connected Valkey",
        ),
    }
}
