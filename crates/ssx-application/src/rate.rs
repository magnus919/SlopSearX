//! Source fixed-window/backpressure policy. F09 owns eligibility; F14 owns dispatch.
use crate::PortFuture;
use ssx_domain::{DomainError, rate::*};
use std::{collections::BTreeMap, sync::Arc};

pub trait RateStore: Send + Sync {
    fn acquire<'a>(
        &'a self,
        engine: &'a EngineKey,
        cost: u64,
    ) -> PortFuture<'a, Result<QuotaReceipt, RateFailure>>;
    fn cooldown<'a>(
        &'a self,
        engine: &'a EngineKey,
    ) -> PortFuture<'a, Result<SharedCooldown, RateFailure>>;
    fn publish_cooldown<'a>(
        &'a self,
        engine: &'a EngineKey,
        seconds: f64,
    ) -> PortFuture<'a, Result<u64, RateFailure>>;
    fn warmup(&self) -> PortFuture<'_, Result<(), RateFailure>>;
    fn shutdown(&self) -> PortFuture<'_, ()>;
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LimiterState {
    pub consecutive_denials: u64,
    pub cooldown_until: f64,
    pub upstream_cooldown_until: f64,
    pub deactivated: bool,
}
pub struct RateLimiter {
    store: Arc<dyn RateStore>,
    fail_closed: bool,
    states: BTreeMap<EngineKey, LimiterState>,
}
impl RateLimiter {
    pub fn new(store: Arc<dyn RateStore>, fail_closed: bool) -> Self {
        Self {
            store,
            fail_closed,
            states: BTreeMap::new(),
        }
    }
    pub fn state(&self, engine: &EngineKey) -> Option<&LimiterState> {
        self.states.get(engine)
    }
    pub async fn acquire(
        &mut self,
        engine: &EngineKey,
        cost: u64,
        monotonic_seconds: f64,
    ) -> Result<RateDecision, DomainError> {
        checked_clock(monotonic_seconds)?;
        if cost > 9_007_199_254_740_991 {
            return Err(DomainError("quota cost exceeds exact range"));
        }
        let state = self.states.entry(engine.clone()).or_default();
        let mut out = RateDecision {
            allowed: false,
            reason: RateReason::Quota,
            authority_failure: None,
            cooldown_failure: None,
        };
        if state.deactivated {
            out.reason = RateReason::Deactivated;
            return Ok(out);
        }
        if state.upstream_cooldown_until > monotonic_seconds {
            out.reason = RateReason::ProviderCooldown;
            return Ok(out);
        }
        match self.store.cooldown(engine).await {
            Ok(SharedCooldown::Active { seconds }) if seconds > 0 => {
                state.upstream_cooldown_until = state
                    .upstream_cooldown_until
                    .max(monotonic_seconds + seconds as f64);
                out.reason = RateReason::ProviderCooldown;
                return Ok(out);
            }
            Err(error) => out.cooldown_failure = Some(error),
            _ => {}
        }
        match self.store.acquire(engine, cost).await {
            Ok(receipt) => out.allowed = receipt.allowed,
            Err(error) => {
                out.allowed = !self.fail_closed
                    && !matches!(error, RateFailure::Closed | RateFailure::InvalidInput);
                out.reason = RateReason::QuotaUnavailable;
                out.authority_failure = Some(error)
            }
        }
        if out.allowed {
            state.consecutive_denials = 0;
            state.cooldown_until = 0.0;
            state.upstream_cooldown_until = 0.0;
        } else {
            state.consecutive_denials = state.consecutive_denials.saturating_add(1);
            state.cooldown_until = monotonic_seconds + 30.0;
            if state.consecutive_denials >= 3 {
                state.deactivated = true;
            }
        }
        // Source records the local 30s marker but does not gate acquisition on it.
        Ok(out)
    }
    pub async fn set_upstream_cooldown(
        &mut self,
        engine: &EngineKey,
        seconds: f64,
        now: f64,
    ) -> Result<(f64, Result<u64, RateFailure>), DomainError> {
        checked_clock(now)?;
        let bounded = bound_provider_cooldown(seconds);
        let state = self.states.entry(engine.clone()).or_default();
        state.upstream_cooldown_until = state.upstream_cooldown_until.max(now + bounded);
        Ok((bounded, self.store.publish_cooldown(engine, bounded).await))
    }
    pub async fn upstream_cooldown_remaining(
        &mut self,
        engine: &EngineKey,
        now: f64,
    ) -> Result<CooldownObservation, DomainError> {
        checked_clock(now)?;
        let shared = self.store.cooldown(engine).await;
        let local = self
            .states
            .get(engine)
            .map_or(0.0, |s| (s.upstream_cooldown_until - now).max(0.0));
        let remote = shared.as_ref().map_or(0.0, |s| s.seconds() as f64);
        if let Some(state) = self.states.get_mut(engine) {
            state.upstream_cooldown_until = state.upstream_cooldown_until.max(now + remote);
        }
        Ok(CooldownObservation {
            remaining_seconds: local.max(remote),
            shared,
        })
    }
    pub fn reactivate(&mut self, engine: &EngineKey) {
        if let Some(state) = self.states.get_mut(engine) {
            *state = LimiterState::default();
        }
    }
    pub fn deactivated_engines(&self) -> Vec<&EngineKey> {
        self.states
            .iter()
            .filter(|(_, s)| s.deactivated)
            .map(|(k, _)| k)
            .collect()
    }
    pub async fn warmup(&self) -> Result<(), RateFailure> {
        self.store.warmup().await
    }
    pub async fn shutdown(&self) {
        self.store.shutdown().await
    }
}
fn checked_clock(now: f64) -> Result<(), DomainError> {
    if now.is_finite() {
        Ok(())
    } else {
        Err(DomainError("invalid monotonic clock"))
    }
}
pub fn bound_provider_cooldown(seconds: f64) -> f64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        30.0
    } else {
        seconds.clamp(1.0, 300.0)
    }
}
/// The lower-level source strategy differs from the wrapper for zero/negative durations.
pub fn bound_strategy_cooldown(seconds: f64) -> f64 {
    if seconds.is_finite() {
        seconds.clamp(1.0, 300.0)
    } else {
        30.0
    }
}

/// Explicit local development mode: never a distributed fallback or production authority.
pub struct LocalTokenBucket {
    rate: f64,
    burst: f64,
    cells: BTreeMap<EngineKey, (f64, f64)>,
}
impl LocalTokenBucket {
    pub fn new(rate: f64, burst: f64) -> Result<Self, DomainError> {
        if !rate.is_finite() || !burst.is_finite() || rate < 0.0 || burst < 0.0 {
            return Err(DomainError("invalid local token bucket"));
        }
        Ok(Self {
            rate,
            burst,
            cells: BTreeMap::new(),
        })
    }
    pub fn acquire(
        &mut self,
        engine: &EngineKey,
        cost: u64,
        now: f64,
    ) -> Result<bool, DomainError> {
        checked_clock(now)?;
        let (tokens, last) = self
            .cells
            .entry(engine.clone())
            .or_insert((self.burst, now));
        if now < *last {
            return Err(DomainError("local monotonic clock moved backward"));
        }
        *tokens = self.burst.min(*tokens + (now - *last) * self.rate);
        *last = now;
        let allowed = *tokens >= cost as f64;
        if allowed {
            *tokens -= cost as f64;
        }
        Ok(allowed)
    }
}
/// Explicit-empty URL follows the source fallback expression; no ambient env access.
pub fn resolve_valkey_url(explicit: &str, environment: Option<&str>) -> String {
    if explicit.is_empty() {
        environment.unwrap_or("").into()
    } else {
        explicit.into()
    }
}
