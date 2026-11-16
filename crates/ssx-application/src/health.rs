//! Redacted observations and the independent source adapter circuit. No active probe.
use ssx_domain::{AdapterResponse, DomainError, EngineStatus, WallClockTimestamp};
use ssx_json::{Value, json};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeOrigin {
    Undispatched,
    CallerCancelled,
    AdapterReturned,
    DispatchedSyntheticTimeout,
}
#[derive(Debug, Clone)]
pub struct HealthState {
    pub consecutive_errors: u64,
    pub circuit_open_until: f64,
    threshold: i64,
    timeout: i64,
    last: Option<Observation>,
}
#[derive(Debug, Clone)]
struct Observation {
    status: EngineStatus,
    at: f64,
    latency: Option<f64>,
    results: u64,
}
#[derive(Debug, Clone)]
pub struct HealthContext {
    pub configured: bool,
    pub auth_class: Option<String>,
    pub auth_configured: Option<bool>,
}
impl Default for HealthState {
    fn default() -> Self {
        Self::new(5, 300)
    }
}
impl HealthState {
    pub fn new(threshold: i64, timeout_seconds: i64) -> Self {
        Self {
            consecutive_errors: 0,
            circuit_open_until: 0.0,
            threshold,
            timeout: timeout_seconds,
            last: None,
        }
    }
    pub fn circuit_allowed(&self, now: WallClockTimestamp) -> bool {
        self.circuit_open_until <= 0.0 || now.seconds() >= self.circuit_open_until
    }
    pub fn circuit_open(&self, now: WallClockTimestamp) -> bool {
        self.circuit_open_until > now.seconds()
    }
    pub fn record_success(&mut self) {
        self.consecutive_errors = 0;
        self.circuit_open_until = 0.0;
    }
    pub fn record_failure(&mut self, now: WallClockTimestamp) {
        self.consecutive_errors = self.consecutive_errors.saturating_add(1);
        if self.threshold <= 0 || self.consecutive_errors >= self.threshold as u64 {
            self.circuit_open_until = now.seconds() + self.timeout as f64;
        }
    }
    pub fn observe(
        &mut self,
        origin: OutcomeOrigin,
        response: &AdapterResponse,
        now: WallClockTimestamp,
    ) {
        if matches!(
            origin,
            OutcomeOrigin::Undispatched | OutcomeOrigin::CallerCancelled
        ) {
            return;
        }
        let synthetic = response.synthetic || origin == OutcomeOrigin::DispatchedSyntheticTimeout;
        let status = if origin == OutcomeOrigin::DispatchedSyntheticTimeout {
            EngineStatus::Timeout
        } else {
            response.status.clone()
        };
        self.last = Some(Observation {
            status: status.clone(),
            at: now.seconds(),
            latency: if synthetic {
                None
            } else {
                response.observed_latency_ms().map(|x| x.get())
            },
            results: response.results.len() as u64,
        });
        match status {
            EngineStatus::Error | EngineStatus::Timeout => self.record_failure(now),
            EngineStatus::Unavailable | EngineStatus::RateLimited => {}
            _ => self.record_success(),
        }
    }
}
/// Configured/auth/circuit are independent signals, never inferred as healthy.
pub fn build_engine_health(
    adapter: Option<&HealthState>,
    context: Option<&HealthContext>,
    now: WallClockTimestamp,
    stale_after: f64,
) -> Result<Value, DomainError> {
    if !stale_after.is_finite() || stale_after <= 0.0 {
        return Err(DomainError("invalid observed health freshness"));
    }
    let observed = adapter.and_then(|s| s.last.as_ref());
    let status = observed
        .map(|o| ssx_json::to_value(&o.status))
        .transpose()?
        .unwrap_or(json!("unknown"));
    let at = observed
        .map(|o| crate::calendar::iso_timestamp(o.at))
        .transpose()?;
    Ok(
        json!({"status":status,"status_at":at,"stale":observed.is_some_and(|o|now.seconds()-o.at>stale_after),"configured":context.map_or(adapter.is_some(),|c|c.configured),"auth_class":context.and_then(|c|c.auth_class.as_deref()),"auth_configured":context.and_then(|c|c.auth_configured),"circuit_open":adapter.is_some_and(|s|s.circuit_open(now)),"circuit_consecutive_errors":adapter.map_or(0,|s|s.consecutive_errors),"last_observed_latency_ms":observed.and_then(|o|o.latency),"last_observed_result_count":observed.map(|o|o.results)}),
    )
}
pub fn stale_seconds(raw: Option<&str>) -> f64 {
    raw.map(|value| value.trim_matches(ssx_json::python_whitespace))
        .and_then(crate::retry_after::python_float)
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(300.0)
}

/// F06 supplies captured raw settings; source integer values (including nonpositive) are retained.
pub fn circuit_settings(threshold: Option<&str>, timeout: Option<&str>) -> (i64, i64) {
    (
        threshold.and_then(crate::retry_after::integer).unwrap_or(5),
        timeout.and_then(crate::retry_after::integer).unwrap_or(300),
    )
}
