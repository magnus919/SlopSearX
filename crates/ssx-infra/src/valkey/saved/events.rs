//! Native tenant-bound outbox. Real Valkey scripts own every mutation and clock.
use super::*;
use crate::saved_events::codec::{ConsumerId, Cursor, Event};
use ssx_json::{Value, json};
#[derive(Debug, Clone, Copy)]
pub struct EventLimits {
    capacity: u32,
    retention_seconds: u32,
    consumers: u32,
}
impl EventLimits {
    pub fn new(capacity: u32, retention_seconds: u32, consumers: u32) -> Result<Self, DomainError> {
        if !(1..=100000).contains(&capacity)
            || !(1..=31536000).contains(&retention_seconds)
            || !(1..=100000).contains(&consumers)
        {
            return Err(DomainError("invalid saved event limits"));
        }
        Ok(Self {
            capacity,
            retention_seconds,
            consumers,
        })
    }
    pub fn capacity(self) -> u32 {
        self.capacity
    }
    pub fn retention_seconds(self) -> u32 {
        self.retention_seconds
    }
}
impl Default for EventLimits {
    fn default() -> Self {
        Self {
            capacity: 1000,
            retention_seconds: 604800,
            consumers: 100,
        }
    }
}
pub struct Publication<'a> {
    pub event: &'a Event,
    pub limits: EventLimits,
}
#[derive(Debug)]
pub struct CommitOutcome {
    pub committed: bool,
    pub cursor: Option<Cursor>,
}
impl CommitOutcome {
    pub(super) fn lost() -> Self {
        Self {
            committed: false,
            cursor: None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckStatus {
    Advanced,
    Idempotent,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventError {
    Store(Error),
    Invalid(DomainError),
    CursorExpired,
    FutureCursor,
    ConsumerCapacity,
    LegacyAckAmbiguous,
}
impl From<Error> for EventError {
    fn from(e: Error) -> Self {
        Self::Store(e)
    }
}
impl From<DomainError> for EventError {
    fn from(e: DomainError) -> Self {
        Self::Invalid(e)
    }
}
impl From<Failure> for EventError {
    fn from(e: Failure) -> Self {
        Self::Store(Error::Store(e))
    }
}
pub struct Ack {
    pub status: AckStatus,
    pub cursor: Cursor,
}
pub struct Batch {
    pub requested: Cursor,
    pub acknowledged: Cursor,
    pub first: Option<Cursor>,
    pub events: Vec<(Cursor, Event)>,
    pub now: f64,
    pub legacy_recovery: bool,
}
impl Batch {
    pub fn gap(&self) -> bool {
        self.requested != Cursor::zero() && self.first.as_ref().is_none_or(|c| &self.requested < c)
    }
    pub fn metadata(&self) -> Value {
        let mut value = json!({"requested_cursor":self.requested.as_str(),"acknowledged_cursor":self.acknowledged.as_str(),"next_cursor":self.events.last().map_or(self.requested.as_str(),|(c,_)|c.as_str()),"gap":{"detected":self.gap(),"reason":if self.gap(){json!("retention_expired")}else{Value::Null},"first_available_cursor":self.first.as_ref().map(|c|c.as_str())}});
        if self.legacy_recovery {
            value["acknowledgement_recovery"] =
                json!({"state":"legacy_ambiguous_ignored","action":"acknowledge_a_retained_event"});
        }
        value
    }
}
const READ: &str = concat!(
    include_str!("../saved_scripts/events/common.lua"),
    include_str!("../saved_scripts/events/read.lua")
);
const ACK: &str = concat!(
    include_str!("../saved_scripts/events/common.lua"),
    include_str!("../saved_scripts/events/ack.lua")
);
/// Unambiguous native authority, independent of delimiters inside accepted IDs.
pub fn acknowledgement_key(tenant: &TenantId, consumer: &ConsumerId) -> String {
    format!(
        "mcp:saved:v2:event-ack:{}:{}:{}",
        tenant.as_str().len(),
        tenant.as_str(),
        consumer.as_str()
    )
}
/// A legacy cursor is attributed only when its complete suffix admits exactly
/// one partition under both accepted grammars. Index hints are not ownership.
pub fn legacy_acknowledgement_unambiguous(tenant: &TenantId, consumer: &ConsumerId) -> bool {
    let suffix = format!("{}:{}", tenant.as_str(), consumer.as_str());
    suffix
        .match_indices(':')
        .filter(|(at, _)| {
            TenantId::new(&suffix[..*at]).is_ok() && ConsumerId::new(&suffix[*at + 1..]).is_ok()
        })
        .take(2)
        .count()
        == 1
}
impl SavedStore {
    pub async fn commit_event(
        &self,
        claim: &Claim,
        report: &Value,
        replacement: Option<&Value>,
        next_due: f64,
        fingerprint: &str,
        publication: Publication<'_>,
    ) -> Result<CommitOutcome, Error> {
        let event = publication.event;
        // Bind the generated payload to this exact validated report and claim.
        let expected = Event::report(
            claim.definition(),
            report,
            event.payload()["event_id"]
                .as_str()
                .ok_or(Error::Malformed)?,
        )?;
        if expected.payload() != event.payload() {
            return Err(Error::Malformed);
        }
        self.commit_publication(
            claim,
            report,
            replacement,
            next_due,
            fingerprint,
            Some(publication),
        )
        .await
    }
    pub async fn read_events(
        &self,
        consumer: &ConsumerId,
        cursor: Option<&Cursor>,
        limit: usize,
        limits: EventLimits,
    ) -> Result<Batch, EventError> {
        if !(1..=100).contains(&limit) {
            return Err(DomainError("limit must be an integer between 1 and 100").into());
        }
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(READ)
            .arg(3)
            .arg(self.index("events"))
            .arg(acknowledgement_key(&self.tenant, consumer))
            .arg(self.key("event-ack", consumer.as_str()))
            .arg(cursor.map_or("", Cursor::as_str))
            .arg(limit)
            .arg(limits.retention_seconds)
            .arg(
                if legacy_acknowledgement_unambiguous(&self.tenant, consumer) {
                    "1"
                } else {
                    "0"
                },
            );
        type Row = (String, Vec<Vec<u8>>);
        let (status, requested, ack, first, rows, now, recovery): (
            i64,
            String,
            String,
            String,
            Vec<Row>,
            String,
            String,
        ) = redis::from_redis_value(self.backend.execute(cmd, true).await?)
            .map_err(|_| Error::Malformed)?;
        if status == -7 {
            return Err(EventError::LegacyAckAmbiguous);
        }
        if status != 1 || !matches!(recovery.as_str(), "0" | "1") {
            return Err(Error::Malformed.into());
        }
        let mut events = Vec::with_capacity(rows.len());
        for (cursor, fields) in rows {
            if fields.len() % 2 != 0 {
                return Err(Error::Malformed.into());
            }
            let mut payload = None;
            for pair in fields.as_chunks::<2>().0 {
                if pair[0] == b"payload" && payload.replace(pair[1].as_slice()).is_some() {
                    return Err(Error::Malformed.into());
                }
            }
            let bytes = payload.ok_or(Error::Malformed)?;
            if bytes.len() > crate::saved_events::codec::MAX_EVENT_BYTES {
                return Err(Error::Malformed.into());
            }
            let value = ssx_json::from_slice(bytes).map_err(|_| Error::Malformed)?;
            events.push((Cursor::new(cursor)?, Event::from_payload(value)?));
        }
        let now = now.parse::<f64>().map_err(|_| Error::Malformed)?;
        if !now.is_finite() {
            return Err(Error::Malformed.into());
        }
        Ok(Batch {
            requested: Cursor::new(requested)?,
            acknowledged: Cursor::new(ack)?,
            first: if first.is_empty() {
                None
            } else {
                Some(Cursor::new(first)?)
            },
            events,
            now,
            legacy_recovery: recovery == "1",
        })
    }
    pub async fn acknowledge_event(
        &self,
        consumer: &ConsumerId,
        cursor: &Cursor,
        limits: EventLimits,
    ) -> Result<Ack, EventError> {
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(ACK)
            .arg(4)
            .arg(acknowledgement_key(&self.tenant, consumer))
            .arg(self.key("event-ack", consumer.as_str()))
            .arg(self.index("event-consumers"))
            .arg(self.index("events"))
            .arg(cursor.as_str())
            .arg(consumer.as_str())
            .arg(limits.retention_seconds)
            .arg(limits.consumers)
            .arg(
                if legacy_acknowledgement_unambiguous(&self.tenant, consumer) {
                    "1"
                } else {
                    "0"
                },
            );
        let (status, ack): (i64, String) =
            redis::from_redis_value(self.backend.execute(cmd, true).await?)
                .map_err(|_| Error::Malformed)?;
        let status = match status {
            1 => AckStatus::Advanced,
            0 => AckStatus::Idempotent,
            -1 => return Err(EventError::ConsumerCapacity),
            -2 => return Err(EventError::FutureCursor),
            -3 | -6 => return Err(EventError::CursorExpired),
            _ => return Err(Error::Malformed.into()),
        };
        Ok(Ack {
            status,
            cursor: Cursor::new(ack)?,
        })
    }
}
const PAUSE: &str = concat!(
    include_str!("../saved_scripts/events/common.lua"),
    include_str!("../saved_scripts/events/pause.lua")
);
const EXPIRY: &str = concat!(
    include_str!("../saved_scripts/events/common.lua"),
    include_str!("../saved_scripts/events/expiry.lua")
);
#[derive(Debug)]
pub struct ExpiryProgress {
    pub cursor: u64,
    pub published: usize,
    pub failure: Option<Error>,
}
impl SavedStore {
    pub async fn pause_event(
        &self,
        id: &SavedSearchId,
        revision: u64,
        policy: &SavedPolicy<'_>,
        limits: EventLimits,
    ) -> Result<(SavedDefinition, Cursor), Error> {
        let (raw, current) = self.raw(id).await?;
        if current.revision != revision {
            return Err(Error::Conflict);
        }
        let now = self.now().await?;
        if current.deleted || current.expires_at.get() <= now {
            return Err(Error::Missing);
        }
        let next = prepare_pause(policy, &current, true, now)?;
        let event = Event::definition(
            &next,
            crate::saved_events::codec::EventType::DefinitionPaused,
            now,
            &crate::saved_events::codec::event_id()?,
        )?;
        let bytes = ssx_json::to_vec(&next.stored_payload()?).map_err(|_| Error::Malformed)?;
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(PAUSE)
            .arg(6)
            .arg(self.key("definition", id.as_str()))
            .arg(self.index("due"))
            .arg(self.key("lease", id.as_str()))
            .arg(self.key("baseline", id.as_str()))
            .arg(self.index("recent"))
            .arg(self.index("events"))
            .arg(raw)
            .arg(bytes)
            .arg(next.expires_at.get())
            .arg(next.created_at.get())
            .arg(next.next_due.get())
            .arg(id.as_str())
            .arg(event.bytes()?)
            .arg(limits.retention_seconds)
            .arg(limits.capacity);
        let (status, cursor): (i64, String) =
            redis::from_redis_value(self.backend.execute(cmd, true).await?)
                .map_err(|_| Error::Malformed)?;
        match status {
            1 => Ok((next, Cursor::new(cursor)?)),
            -1 => Err(Error::Missing),
            -2 => Err(Error::Conflict),
            -5 => Err(Error::OutboxCapacity),
            _ => Err(Error::Malformed),
        }
    }
    pub async fn publish_expired_event_page(
        &self,
        cursor: u64,
        count: usize,
        limits: EventLimits,
    ) -> Result<ExpiryProgress, Error> {
        let (next, keys) = self
            .scan(cursor, "mcp:saved:v1:definition:*", count)
            .await?;
        let mut published = 0;
        for key in keys {
            let attempt = async {
                let (tenant, id) = key
                    .strip_prefix("mcp:saved:v1:definition:")
                    .and_then(|s| s.rsplit_once(':'))
                    .ok_or(Error::Malformed)?;
                let target = self.for_tenant(TenantId::new(tenant)?);
                let id = SavedSearchId::new(id)?;
                let (raw, d) = match target.raw(&id).await {
                    Err(Error::Missing) => return Ok(false),
                    r => r?,
                };
                if d.expires_at.get() > target.now().await? {
                    return Ok(false);
                }
                let event = Event::definition(
                    &d,
                    crate::saved_events::codec::EventType::DefinitionExpired,
                    d.expires_at.get(),
                    &crate::saved_events::codec::event_id()?,
                )?;
                let mut cmd = redis::cmd("EVAL");
                cmd.arg(EXPIRY)
                    .arg(3)
                    .arg(&key)
                    .arg(target.key("event-expired", id.as_str()))
                    .arg(target.index("events"))
                    .arg(raw)
                    .arg(d.expires_at.get())
                    .arg(event.bytes()?)
                    .arg(limits.retention_seconds)
                    .arg(limits.capacity);
                let (status, _): (i64, String) =
                    redis::from_redis_value(target.backend.execute(cmd, true).await?)
                        .map_err(|_| Error::Malformed)?;
                match status {
                    1 => Ok(true),
                    0 => Ok(false),
                    -5 => Err(Error::OutboxCapacity),
                    _ => Err(Error::Malformed),
                }
            }
            .await;
            match attempt {
                Ok(true) => published += 1,
                Ok(false) => {}
                Err(error) => {
                    return Ok(ExpiryProgress {
                        cursor,
                        published,
                        failure: Some(error),
                    });
                }
            }
        }
        Ok(ExpiryProgress {
            cursor: next,
            published,
            failure: None,
        })
    }
}
