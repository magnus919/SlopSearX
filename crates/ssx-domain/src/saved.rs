//! Saved-definition records and deterministic schedule identities. No runner or I/O.
use crate::{
    DomainError, FiniteF64,
    canonical::saved_identity,
    storage::{normalize_python_number_text, python_value_string},
};
use serde::{Deserialize, Serialize};
use ssx_json::Value;

pub const IDENTITY_VERSION: &str = "url-source-window-v1";
pub const RETENTION_MARGIN_SECONDS: u64 = 300;
pub const LEASE_SECONDS: u64 = 60;
pub const MAX_RECENT_DEFINITIONS: usize = 200;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedDefinition {
    pub search_id: String,
    pub tenant: String,
    pub query: String,
    pub engines: Vec<String>,
    pub interval_seconds: u64,
    pub retention_seconds: u64,
    pub max_results: u64,
    pub max_reports: u64,
    pub created_at: FiniteF64,
    pub expires_at: FiniteF64,
    pub next_due: FiniteF64,
    pub revision: u64,
    pub paused: bool,
    pub policy_fingerprint: String,
    pub last_slot: i64,
    pub latest_run_id: Option<String>,
    pub baseline: Option<Value>,
    pub lease_token: Option<String>,
    pub lease_expires_at: FiniteF64,
    pub deleted: bool,
}
impl std::fmt::Debug for SavedDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SavedDefinition")
            .field("search_id", &self.search_id)
            .field("revision", &self.revision)
            .field("paused", &self.paused)
            .field("deleted", &self.deleted)
            .finish_non_exhaustive()
    }
}
/// Exact source slot arithmetic; this does not authorize a claim or execute work.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedSlot {
    pub slot: i64,
    pub missed_slots: i64,
    pub scheduled_at: FiniteF64,
    pub following_due: FiniteF64,
    pub run_id: String,
}
impl SavedDefinition {
    pub fn validate_binding(&self, tenant: &str, id: &str) -> Result<(), DomainError> {
        validate_saved_id(id)?;
        if self.tenant != tenant || self.search_id != id {
            return Err(DomainError("saved definition identity mismatch"));
        }
        Ok(())
    }
    pub fn window(&self) -> Result<Value, DomainError> {
        #[derive(Serialize)]
        struct Window<'a> {
            query: &'a str,
            engines: Vec<&'a str>,
            page: u64,
            max_results: u64,
            revision: u64,
            identity_version: &'static str,
            policy_fingerprint: &'a str,
        }
        let mut engines = self.engines.iter().map(String::as_str).collect::<Vec<_>>();
        engines.sort();
        Ok(ssx_json::to_value(Window {
            query: &self.query,
            engines,
            page: 1,
            max_results: self.max_results,
            revision: self.revision,
            identity_version: IDENTITY_VERSION,
            policy_fingerprint: &self.policy_fingerprint,
        })?)
    }
    pub fn payload(&self) -> Result<Value, DomainError> {
        Ok(ssx_json::to_value(self)?)
    }
    pub fn stored_payload(&self) -> Result<Value, DomainError> {
        let mut value = self.payload()?;
        value["baseline"] = Value::Null;
        Ok(value)
    }
    pub fn physical_ttl(&self, now: f64) -> Result<u64, DomainError> {
        if !now.is_finite() {
            return Err(DomainError("invalid saved clock"));
        }
        let delta = self.expires_at.get() - now;
        if !delta.is_finite() {
            return Err(DomainError("saved retention overflow"));
        }
        let seconds = delta.trunc() + RETENTION_MARGIN_SECONDS as f64;
        if seconds >= u64::MAX as f64 {
            return Err(DomainError("saved retention overflow"));
        }
        Ok(seconds.max(1.0) as u64)
    }
    pub fn slot(&self, now: f64) -> Result<SavedSlot, DomainError> {
        if !now.is_finite() || self.interval_seconds == 0 {
            return Err(DomainError("invalid saved schedule"));
        }
        let difference = now - self.created_at.get();
        if !difference.is_finite() {
            return Err(DomainError("saved slot arithmetic overflow"));
        }
        let interval = self.interval_seconds as f64;
        // CPython float floor division derives the quotient from fmod, then
        // corrects floating-point rounding near an integral quotient.
        let remainder = difference % interval;
        let mut quotient = (difference - remainder) / interval;
        if remainder < 0.0 {
            quotient -= 1.0;
        }
        if !remainder.is_finite() || !quotient.is_finite() {
            return Err(DomainError("saved slot arithmetic overflow"));
        }
        let floor = quotient.floor();
        let value = (if quotient - floor > 0.5 {
            floor + 1.0
        } else {
            floor
        })
        .max(0.0);
        if !value.is_finite() || value >= i64::MAX as f64 {
            return Err(DomainError("saved slot overflow"));
        }
        let slot = value as i64;
        let missed_slots = if self.last_slot >= 0 {
            slot.saturating_sub(self.last_slot).saturating_sub(1).max(0)
        } else {
            slot.saturating_sub(1).max(0)
        };
        let hash = saved_identity(&[
            Value::String(self.search_id.clone()),
            ssx_json::to_value(self.revision)?,
            Value::Number(self.next_due.as_number().clone()),
        ])?;
        Ok(SavedSlot {
            slot,
            missed_slots,
            scheduled_at: self.next_due.clone(),
            following_due: FiniteF64::new(
                self.created_at.get()
                    + ((i128::from(slot) + 1) * i128::from(self.interval_seconds)) as f64,
            )?,
            run_id: format!("run-{}", &hash[..24]),
        })
    }
    /// Source rehydration coerces scalar values before authority validation.
    /// Numbers are finite/bounded; malformed persisted authority is never a default.
    pub fn from_payload(value: &Value) -> Result<Self, DomainError> {
        let object = value
            .as_object()
            .ok_or(DomainError("invalid saved record"))?;
        let required = |name| {
            object
                .get(name)
                .ok_or(DomainError("incomplete saved record"))
        };
        let integer = |name| unsigned(required(name)?);
        let time = |name| number(required(name)?);
        let engines = match object.get("engines") {
            None => Vec::new(),
            Some(Value::Array(items)) => items.iter().map(python_value_string).collect(),
            Some(Value::String(text)) => text.chars().map(|ch| ch.to_string()).collect(),
            Some(Value::Object(items)) => items.keys().cloned().collect(),
            _ => return Err(DomainError("invalid saved engines")),
        };
        Ok(Self {
            search_id: python_value_string(required("search_id")?),
            tenant: python_value_string(required("tenant")?),
            query: python_value_string(required("query")?),
            engines,
            interval_seconds: integer("interval_seconds")?,
            retention_seconds: integer("retention_seconds")?,
            max_results: integer("max_results")?,
            max_reports: integer("max_reports")?,
            created_at: time("created_at")?,
            expires_at: time("expires_at")?,
            next_due: time("next_due")?,
            revision: object
                .get("revision")
                .map(unsigned)
                .transpose()?
                .unwrap_or(1),
            paused: object.get("paused").is_some_and(truthy),
            policy_fingerprint: object
                .get("policy_fingerprint")
                .map(python_value_string)
                .unwrap_or_default(),
            last_slot: object
                .get("last_slot")
                .map(signed)
                .transpose()?
                .unwrap_or(-1),
            latest_run_id: optional_string(object.get("latest_run_id")),
            baseline: object.get("baseline").filter(|v| v.is_object()).cloned(),
            lease_token: optional_string(object.get("lease_token")),
            lease_expires_at: object
                .get("lease_expires_at")
                .map(number)
                .transpose()?
                .unwrap_or_default(),
            deleted: object.get("deleted").is_some_and(truthy),
        })
    }
}
/// Opaque baseline hydration follows the source float expiry check, without
/// interpreting comparison/report content owned by W03.
pub fn baseline_is_live(value: &Value, now: f64) -> Result<bool, DomainError> {
    if !now.is_finite() {
        return Err(DomainError("invalid saved clock"));
    }
    Ok(baseline_expiry(value)? > now)
}
/// Source-coerced opaque-baseline expiry for atomic store preflight.
pub fn baseline_expiry(value: &Value) -> Result<f64, DomainError> {
    let expiry = value
        .as_object()
        .ok_or(DomainError("invalid saved baseline object"))?
        .get("expires_at");
    Ok(expiry.map(number).transpose()?.map_or(0.0, |n| n.get()))
}
pub fn validate_saved_id(id: &str) -> Result<(), DomainError> {
    if id.is_empty()
        || id.len() > 1024
        || id.contains(':')
        || id.chars().any(|ch| ch.is_ascii_control())
    {
        return Err(DomainError("invalid saved storage identity"));
    }
    Ok(())
}
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|n| n != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}
fn optional_string(value: Option<&Value>) -> Option<String> {
    value.filter(|v| truthy(v)).map(python_value_string)
}
fn number(value: &Value) -> Result<FiniteF64, DomainError> {
    let n = match value {
        Value::Bool(v) => f64::from(u8::from(*v)),
        Value::Number(v) => v.as_f64().ok_or(DomainError("invalid saved number"))?,
        Value::String(v) => normalize_python_number_text(v)
            .map_err(|_| DomainError("invalid saved number"))?
            .parse()
            .map_err(|_| DomainError("invalid saved number"))?,
        _ => return Err(DomainError("invalid saved number")),
    };
    FiniteF64::new(n)
}
fn signed(value: &Value) -> Result<i64, DomainError> {
    match value {
        Value::Bool(v) => Ok(i64::from(*v)),
        Value::Number(v) if !v.is_f64() => v.as_i64().ok_or(DomainError("saved integer overflow")),
        Value::String(v) => normalize_python_number_text(v)
            .map_err(|_| DomainError("invalid saved integer"))?
            .parse()
            .map_err(|_| DomainError("invalid saved integer")),
        Value::Number(_) => {
            let n = number(value)?.get().trunc();
            if !(-9223372036854775808.0..9223372036854775808.0).contains(&n) {
                return Err(DomainError("saved integer overflow"));
            }
            Ok(n as i64)
        }
        _ => Err(DomainError("invalid saved integer")),
    }
}
fn unsigned(value: &Value) -> Result<u64, DomainError> {
    u64::try_from(signed(value)?).map_err(|_| DomainError("negative saved integer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_payload_window_schedule_and_rehydration() {
        let corpus: Value =
            ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w00/source.json")).unwrap();
        for row in corpus["observations"].as_array().unwrap() {
            let definition = SavedDefinition::from_payload(&row["input"]).unwrap();
            if let Some(decoded) = row.as_object().unwrap().get("decoded") {
                assert_eq!(definition.payload().unwrap(), *decoded, "{}", row["name"]);
            } else {
                assert_eq!(definition.window().unwrap(), row["window"]);
                let now = row["now"].as_f64().unwrap();
                let slot = definition.slot(now).unwrap();
                assert_eq!(slot.slot, row["slot"].as_i64().unwrap());
                assert_eq!(slot.missed_slots, row["missed_slots"].as_i64().unwrap());
                assert_eq!(
                    slot.following_due.get(),
                    row["following_due"].as_f64().unwrap()
                );
                assert_eq!(
                    definition.physical_ttl(now).unwrap(),
                    row["ttl"].as_u64().unwrap()
                );
            }
            assert_eq!(
                definition.slot(definition.next_due.get()).unwrap().run_id,
                row["run_id"].as_str().unwrap()
            );
        }
    }
    #[test]
    fn storage_identity_is_unambiguous_without_narrowing_tenants() {
        assert!(validate_saved_id("c").is_ok());
        assert!(validate_saved_id("b:c").is_err());
        assert!(validate_saved_id("saved-雪").is_ok());
        assert!(validate_saved_id("bad\0id").is_err());
    }
}
#[cfg(test)]
mod claim_fixture_tests {
    use super::*;
    #[test]
    fn committed_claim_identity_replays_source_precision_and_stale_clock_hazard() {
        let corpus: Value =
            ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w00/source.json")).unwrap();
        for row in corpus["claim_observations"].as_array().unwrap() {
            let before = SavedDefinition::from_payload(&row["input"]).unwrap();
            let after = SavedDefinition::from_payload(&row["committed"]).unwrap();
            assert_eq!(
                before.slot(before.next_due.get()).unwrap().run_id,
                row["run_id_before"].as_str().unwrap()
            );
            assert_eq!(
                after.slot(after.next_due.get()).unwrap().run_id,
                row["run_id_after"].as_str().unwrap()
            );
            assert_eq!(row["lease_seconds"].as_i64(), Some(60));
            assert_eq!(
                row["logical_expired_at_server"].as_bool(),
                Some(row["name"] == "stale_clock")
            );
        }
    }
}
#[cfg(test)]
mod boundary_review_tests {
    use super::*;
    fn definition() -> SavedDefinition {
        let corpus: Value =
            ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w00/source.json")).unwrap();
        SavedDefinition::from_payload(&corpus["observations"][0]["input"]).unwrap()
    }
    #[test]
    fn finite_input_arithmetic_overflow_does_not_manufacture_slot_zero() {
        let mut record = definition();
        record.created_at = FiniteF64::new(-f64::MAX).unwrap();
        assert!(record.slot(f64::MAX).is_err());
    }
    #[test]
    fn oversized_window_is_fallible_without_macro_panic() {
        let mut record = definition();
        record.query = "x".repeat(ssx_json::MAX_JSON_BYTES + 1);
        assert!(std::panic::catch_unwind(|| record.window()).is_ok());
    }
    #[test]
    fn oversized_identity_is_calculated_without_persistence_macro_panic() {
        let mut record = definition();
        record.search_id = "x".repeat(ssx_json::MAX_JSON_BYTES + 1);
        assert!(std::panic::catch_unwind(|| record.slot(1060.0)).is_ok());
    }
}
