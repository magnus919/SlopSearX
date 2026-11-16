//! Typed quota authority inputs and outcomes; no local clock can name a shared bucket.
use crate::{DomainError, ValkeyQuotaTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct EngineKey(String);
impl EngineKey {
    pub fn new(name: &str) -> Result<Self, DomainError> {
        if name.is_empty()
            || name == "upstream-cooldown"
            || name.len() > 128
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(DomainError("invalid rate engine identifier"));
        }
        Ok(Self(name.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, Copy)]
pub struct QuotaConfig {
    rate: f64,
    window_ms: u64,
}
impl QuotaConfig {
    pub fn new(rate: f64, window_seconds: f64) -> Result<Self, DomainError> {
        let millis = window_seconds * 1000.0;
        if !rate.is_finite()
            || !(0.0..=9_007_199_254_740_991.0).contains(&rate)
            || !millis.is_finite()
            || !(1.0..=4_503_599_627_370_495.0).contains(&millis)
            || (millis - millis.round()).abs() > f64::EPSILON * millis.abs().max(1.0) * 2.0
        {
            return Err(DomainError("invalid exact quota rate/window"));
        }
        Ok(Self {
            rate,
            window_ms: millis.round() as u64,
        })
    }
    pub fn rate(self) -> f64 {
        self.rate
    }
    pub fn window_ms(self) -> u64 {
        self.window_ms
    }
    pub fn retention_ms(self) -> u64 {
        self.window_ms * 2
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateFailure {
    Closed,
    Unavailable,
    CompletionUnknown,
    MayHaveWritten,
    Malformed,
    InvalidInput,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaReceipt {
    pub allowed: bool,
    pub count: u64,
    pub bucket_key: String,
    pub server_time: ValkeyQuotaTime,
    pub remaining_ttl_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedCooldown {
    Missing,
    Active { seconds: u64 },
}
impl SharedCooldown {
    pub fn seconds(&self) -> u64 {
        match self {
            Self::Missing => 0,
            Self::Active { seconds } => *seconds,
        }
    }
}
/// Strategy accepts a caller-selected fallback policy without erasing authority failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateDecision {
    pub allowed: bool,
    pub reason: RateReason,
    pub authority_failure: Option<RateFailure>,
    pub cooldown_failure: Option<RateFailure>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateReason {
    Quota,
    QuotaUnavailable,
    ProviderCooldown,
    Deactivated,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CooldownObservation {
    pub remaining_seconds: f64,
    pub shared: Result<SharedCooldown, RateFailure>,
}
