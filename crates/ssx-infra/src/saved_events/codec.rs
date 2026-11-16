//! Version-one privacy-bounded event envelopes. No provider or storage access.
use base64::Engine as _;
use ssx_domain::{DomainError, saved::SavedDefinition};
use ssx_json::{
    Value,
    canonical::{self, JsonProfile},
    json,
};
use std::cmp::Ordering;
pub const MAX_EVENT_BYTES: usize = 64 * 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor(String);
impl Cursor {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let valid = value.len() <= 64
            && value.split_once('-').is_some_and(|(a, b)| {
                [a, b].into_iter().all(|p| {
                    !p.is_empty()
                        && p.bytes().all(|c| c.is_ascii_digit())
                        && (p == "0" || !p.starts_with('0'))
                })
            });
        if !valid {
            return Err(DomainError(
                "cursor must use the '<milliseconds>-<sequence>' form",
            ));
        }
        Ok(Self(value))
    }
    pub fn zero() -> Self {
        Self("0-0".into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl Ord for Cursor {
    fn cmp(&self, other: &Self) -> Ordering {
        let (a, b) = self.0.split_once('-').expect("validated cursor");
        let (c, d) = other.0.split_once('-').expect("validated cursor");
        (a.len(), a, b.len(), b).cmp(&(c.len(), c, d.len(), d))
    }
}
impl PartialOrd for Cursor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerId(String);
impl ConsumerId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if !(1..=128).contains(&value.len())
            || !value.as_bytes()[0].is_ascii_alphanumeric()
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
        {
            return Err(DomainError(
                "consumer_id must be 1-128 safe identifier characters",
            ));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    ReportCreated,
    RunIncomparable,
    RunFailed,
    DefinitionPaused,
    DefinitionExpired,
}
impl EventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReportCreated => "report_created",
            Self::RunIncomparable => "run_incomparable",
            Self::RunFailed => "run_failed",
            Self::DefinitionPaused => "definition_paused",
            Self::DefinitionExpired => "definition_expired",
        }
    }
}
#[derive(Debug, Clone)]
pub struct Event(Value);
fn artifact(kind: &str, id: &str) -> Value {
    json!({"contract":"slopsearx.artifact_ref","version":1,"kind":kind,"id":id})
}
pub fn event_id() -> Result<String, DomainError> {
    let mut bytes = [0; 18];
    aws_lc_rs::rand::fill(&mut bytes).map_err(|_| DomainError("event randomness unavailable"))?;
    Ok(format!(
        "event-{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    ))
}
impl Event {
    fn envelope(
        d: &SavedDefinition,
        kind: EventType,
        at: f64,
        identity: &str,
    ) -> Result<Value, DomainError> {
        if !at.is_finite()
            || identity.len() > 128
            || !identity.starts_with("event-")
            || !identity
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        {
            return Err(DomainError("invalid saved event envelope"));
        }
        let mut engines = d.engines.clone();
        engines.sort();
        Ok(
            json!({"contract":"slopsearx.saved_search_event","contract_version":1,"event_id":identity,"event_type":kind.as_str(),"occurred_at":at,"saved_search_id":d.search_id,"run_id":null,"saved_search":artifact("saved_search",&d.search_id),"run":null,"summary":null,"coverage_state":"unknown","reason_code":"none","_policy_engines":engines}),
        )
    }
    pub fn report(
        d: &SavedDefinition,
        report: &Value,
        identity: &str,
    ) -> Result<Self, DomainError> {
        let status = report["status"].as_str().unwrap_or("incomparable");
        let failed = truthy(&report["observation"]["operational_error"])
            || report["incomparable_reasons"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v.as_str() == Some("dispatch_failed")));
        let (kind, reason) = if failed {
            (EventType::RunFailed, "dispatch_failed")
        } else if status == "incomparable" {
            (EventType::RunIncomparable, "observation_incomparable")
        } else {
            (EventType::ReportCreated, "none")
        };
        let at = report["finished_at"]
            .as_f64()
            .ok_or(DomainError("event report has no finished_at"))?;
        let run = report["run_id"]
            .as_str()
            .ok_or(DomainError("event report has no run_id"))?;
        let mut value = Self::envelope(d, kind, at, identity)?;
        let composite = ssx_domain::canonical::composite_artifact_id(&[&d.search_id, run])?;
        value["run_id"] = json!(run);
        value["run"] = artifact("saved_report", &composite);
        let count = |name| {
            report["events"].as_array().map_or(0, |a| {
                a.iter()
                    .filter(|v| v["kind"].as_str() == Some(name))
                    .count()
            })
        };
        value["summary"] = json!({"status":status,"added_count":count("added"),"changed_count":count("source_field_changed"),"not_observed_count":count("not_observed_in_latest_run"),"discovered_count":report["observation"]["discovered_count"].as_i64().unwrap_or(0),"missed_slots":report["missed_slots"].as_i64().unwrap_or(0)});
        let coverage = report["observation"]["coverage"].as_object();
        value["coverage_state"] = json!(match coverage {
            Some(c) if !c.is_empty() && c.values().all(|v| v.as_str() == Some("ok")) => "complete",
            Some(c) if !c.is_empty() => "partial",
            _ => "unknown",
        });
        value["reason_code"] = json!(reason);
        Self::from_payload(value)
    }
    pub fn definition(
        d: &SavedDefinition,
        kind: EventType,
        at: f64,
        identity: &str,
    ) -> Result<Self, DomainError> {
        let reason = match kind {
            EventType::DefinitionPaused => "operator_paused",
            EventType::DefinitionExpired => "lifetime_elapsed",
            _ => return Err(DomainError("unsupported definition event type")),
        };
        let mut value = Self::envelope(d, kind, at, identity)?;
        value["reason_code"] = json!(reason);
        Self::from_payload(value)
    }
    /// Historic retained payloads may have additional public fields; preserve them.
    pub fn from_payload(value: Value) -> Result<Self, DomainError> {
        if !value.is_object()
            || canonical::canonical_json(&value, JsonProfile::AsciiCompact)
                .map_err(|_| DomainError("invalid saved event JSON"))?
                .len()
                > MAX_EVENT_BYTES
        {
            return Err(DomainError("saved-search event exceeds the storage bound"));
        }
        Ok(Self(value))
    }
    pub fn payload(&self) -> &Value {
        &self.0
    }
    pub fn bytes(&self) -> Result<Vec<u8>, DomainError> {
        canonical::canonical_json(&self.0, JsonProfile::AsciiCompact)
            .map_err(|_| DomainError("invalid saved event JSON"))
    }
    pub fn kind(&self) -> Option<EventType> {
        match self.0["event_type"].as_str()? {
            "report_created" => Some(EventType::ReportCreated),
            "run_incomparable" => Some(EventType::RunIncomparable),
            "run_failed" => Some(EventType::RunFailed),
            "definition_paused" => Some(EventType::DefinitionPaused),
            "definition_expired" => Some(EventType::DefinitionExpired),
            _ => None,
        }
    }
    pub fn engines(&self) -> Option<Vec<String>> {
        self.0["_policy_engines"]
            .as_array()?
            .iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect()
    }
    pub fn public(&self, cursor: &Cursor, redacted: bool) -> Value {
        let mut value = Value::Object(
            self.0
                .as_object()
                .expect("event object")
                .iter()
                .filter(|(k, _)| !k.starts_with('_'))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );
        value["cursor"] = json!(cursor.as_str());
        value["policy_state"] = json!(if redacted { "redacted" } else { "current" });
        if redacted {
            value["summary"] = Value::Null;
            value["coverage_state"] = json!("redacted");
            value["reason_code"] = json!("policy_redacted");
        }
        value
    }
}
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
    }
}
