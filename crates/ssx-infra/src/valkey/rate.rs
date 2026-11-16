//! F10 shared authority. Local clock values cannot enter this API or quota Lua.
use super::{Failure, Valkey};
use ssx_application::{
    PortFuture,
    rate::{RateStore, bound_strategy_cooldown, resolve_valkey_url},
};
use ssx_domain::{ValkeyQuotaTime, rate::*};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
pub const QUOTA_SCRIPT: &str = include_str!("rate/quota.lua");
pub const COOLDOWN_SCRIPT: &str = include_str!("rate/cooldown.lua");
/// Client counters have a separate framed namespace. EngineKey cannot contain
/// a colon, so even an engine named `client-<digest>` cannot share this key.
pub struct ClientQuotaKey(String);
impl ClientQuotaKey {
    pub fn new(client: &str) -> Result<Self, RateFailure> {
        use sha2::{Digest, Sha256};
        if client.is_empty() {
            return Err(RateFailure::InvalidInput);
        }
        let digest = Sha256::digest(client.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(Self(digest))
    }
    pub fn digest(&self) -> &str {
        &self.0
    }
}
pub struct DistributedQuota {
    store: Option<Arc<Valkey>>,
    closed: AtomicBool,
    owns_store: bool,
    config: QuotaConfig,
}
impl DistributedQuota {
    pub fn new(store: Arc<Valkey>, config: QuotaConfig) -> Self {
        Self {
            store: Some(store),
            owns_store: false,
            closed: AtomicBool::new(false),
            config,
        }
    }
    /// F06 provides the captured environment value; this module never reads ambient URLs.
    pub fn from_url(
        explicit: &str,
        environment: Option<&str>,
        config: QuotaConfig,
        budget: Duration,
    ) -> Result<Self, RateFailure> {
        let url = resolve_valkey_url(explicit, environment);
        if url.is_empty() {
            return Ok(Self {
                store: None,
                owns_store: false,
                closed: AtomicBool::new(false),
                config,
            });
        }
        let mut quota = Self::new(
            Arc::new(Valkey::new(&url, budget).map_err(failure)?),
            config,
        );
        quota.owns_store = true;
        Ok(quota)
    }
    pub fn configured(&self) -> bool {
        self.store.is_some()
    }
    fn store(&self) -> Result<&Arc<Valkey>, RateFailure> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(RateFailure::Closed);
        }
        self.store.as_ref().ok_or(RateFailure::Unavailable)
    }
    pub async fn acquire(
        &self,
        engine: &EngineKey,
        cost: u64,
    ) -> Result<QuotaReceipt, RateFailure> {
        self.acquire_prefix(format!("ratelimit:{}:", engine.as_str()), cost)
            .await
    }
    pub async fn acquire_client(
        &self,
        client: &ClientQuotaKey,
        cost: u64,
    ) -> Result<QuotaReceipt, RateFailure> {
        self.acquire_prefix(format!("ratelimit:client:{}:", client.0), cost)
            .await
    }
    async fn acquire_prefix(&self, prefix: String, cost: u64) -> Result<QuotaReceipt, RateFailure> {
        if cost > 9_007_199_254_740_991 {
            return Err(RateFailure::InvalidInput);
        }
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(QUOTA_SCRIPT)
            .arg(1)
            .arg(&prefix)
            .arg(self.config.window_ms())
            .arg(self.config.rate())
            .arg(cost);
        let values: Vec<String> =
            redis::from_redis_value(self.store()?.execute(cmd, true).await.map_err(failure)?)
                .map_err(|_| RateFailure::Malformed)?;
        if values.len() != 5 {
            return Err(RateFailure::Malformed);
        }
        let count = values[1].parse().map_err(|_| RateFailure::Malformed)?;
        let now = values[3].parse().map_err(|_| RateFailure::Malformed)?;
        let ttl = values[4]
            .parse::<u64>()
            .map_err(|_| RateFailure::Malformed)?;
        let expected_key = format!("{prefix}{}", now / self.config.window_ms());
        if !["0", "1"].contains(&values[0].as_str())
            || ttl > self.config.retention_ms()
            || count > i64::MAX as u64
            || values[2] != expected_key
            || (values[0] == "1") != (count as f64 <= self.config.rate())
        {
            return Err(RateFailure::Malformed);
        }
        Ok(QuotaReceipt {
            allowed: values[0] == "1",
            count,
            bucket_key: values[2].clone(),
            server_time: ValkeyQuotaTime::from_milliseconds(now),
            remaining_ttl_ms: ttl,
        })
    }
    pub async fn cooldown(&self, engine: &EngineKey) -> Result<SharedCooldown, RateFailure> {
        let mut cmd = redis::cmd("PTTL");
        cmd.arg(format!("ratelimit:upstream-cooldown:{}", engine.as_str()));
        let ttl: i64 =
            redis::from_redis_value(self.store()?.execute(cmd, false).await.map_err(failure)?)
                .map_err(|_| RateFailure::Malformed)?;
        match ttl {
            -2 => Ok(SharedCooldown::Missing),
            0.. => Ok(SharedCooldown::Active {
                // The public port uses whole seconds; never round a live key down.
                seconds: (ttl as u64).div_ceil(1000).max(1),
            }),
            _ => Err(RateFailure::Malformed),
        }
    }
    pub async fn publish_cooldown(
        &self,
        engine: &EngineKey,
        seconds: f64,
    ) -> Result<u64, RateFailure> {
        let seconds = bound_strategy_cooldown(seconds);
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(COOLDOWN_SCRIPT)
            .arg(1)
            .arg(format!("ratelimit:upstream-cooldown:{}", engine.as_str()))
            .arg(seconds);
        redis::from_redis_value(self.store()?.execute(cmd, true).await.map_err(failure)?)
            .map_err(|_| RateFailure::Malformed)
    }
}
impl RateStore for DistributedQuota {
    fn acquire<'a>(
        &'a self,
        engine: &'a EngineKey,
        cost: u64,
    ) -> PortFuture<'a, Result<QuotaReceipt, RateFailure>> {
        Box::pin(self.acquire(engine, cost))
    }
    fn cooldown<'a>(
        &'a self,
        engine: &'a EngineKey,
    ) -> PortFuture<'a, Result<SharedCooldown, RateFailure>> {
        Box::pin(self.cooldown(engine))
    }
    fn publish_cooldown<'a>(
        &'a self,
        engine: &'a EngineKey,
        seconds: f64,
    ) -> PortFuture<'a, Result<u64, RateFailure>> {
        Box::pin(self.publish_cooldown(engine, seconds))
    }
    fn warmup(&self) -> PortFuture<'_, Result<(), RateFailure>> {
        Box::pin(async {
            if self.closed.load(Ordering::SeqCst) {
                return Err(RateFailure::Closed);
            }
            match &self.store {
                Some(store) => store.quota_time().await.map(|_| ()).map_err(failure),
                None => Ok(()),
            }
        })
    }
    fn shutdown(&self) -> PortFuture<'_, ()> {
        Box::pin(async {
            self.closed.store(true, Ordering::SeqCst);
            if self.owns_store
                && let Some(store) = &self.store
            {
                store.close().await;
            }
        })
    }
}
fn failure(f: Failure) -> RateFailure {
    match f {
        Failure::Closed => RateFailure::Closed,
        Failure::Unavailable => RateFailure::Unavailable,
        Failure::CompletionUnknown => RateFailure::CompletionUnknown,
        Failure::ScriptErrorMayHaveWritten => RateFailure::MayHaveWritten,
        Failure::Malformed => RateFailure::Malformed,
        Failure::InvalidInput => RateFailure::InvalidInput,
    }
}
