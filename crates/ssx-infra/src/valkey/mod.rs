//! Shared Valkey authority. Commands are bounded and never retried after dispatch.
//! A failed mutating command may have committed: reconcile before issuing another.
use redis::{Cmd, Value};
use ssx_domain::{LeaseToken, ValkeyQuotaTime, WallClockExpiry, WallClockTimestamp};
use ssx_json::Value as Json;
use std::{
    num::NonZeroU64,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::sync::Mutex;
pub mod keys;
pub mod scripts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    Closed,
    Unavailable,
    /// Response was lost or deadline elapsed after dispatch; do not blindly retry.
    CompletionUnknown,
    /// Lua/WRONGTYPE errors can follow earlier writes; Lua has no rollback.
    ScriptErrorMayHaveWritten,
    Malformed,
    InvalidInput,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Create {
    Stored,
    Exists,
}
#[derive(Debug, PartialEq)]
pub enum Read<T> {
    Found(T),
    Missing,
    Unavailable,
    Malformed,
    Closed,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Ownership {
    Applied,
    Lost,
}
/// Callers inject an unpredictable token source (or deterministic fixture source).
/// Each acquisition must use a fresh token; never reuse a token after release.
pub trait TokenSource<O> {
    fn next_token(&mut self) -> LeaseToken<O>;
}

pub struct Valkey {
    client: redis::Client,
    connection: Mutex<Option<redis::aio::MultiplexedConnection>>,
    closed: AtomicBool,
    budget: Duration,
}
impl Valkey {
    /// Terminal owner state only; this is not transport readiness or a probe.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
    pub fn new(url: &str, budget: Duration) -> Result<Self, Failure> {
        if budget.is_zero() {
            return Err(Failure::InvalidInput);
        }
        Ok(Self {
            client: redis::Client::open(url).map_err(|_| Failure::InvalidInput)?,
            connection: Mutex::new(None),
            closed: AtomicBool::new(false),
            budget,
        })
    }
    /// Close is terminal. Setting the flag first prevents reconnect during shutdown.
    pub async fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.connection.lock().await.take();
    }
    async fn execute(&self, command: Cmd, write: bool) -> Result<Value, Failure> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(Failure::Closed);
        }
        // One budget covers queueing, connection establishment and response.
        let dispatched = AtomicBool::new(false);
        let result = tokio::time::timeout(self.budget, async {
            let mut slot = self.connection.lock().await;
            if self.closed.load(Ordering::SeqCst) {
                return Err(Failure::Closed);
            }
            if slot.is_none() {
                let conn = self
                    .client
                    .get_multiplexed_async_connection()
                    .await
                    .map_err(|_| Failure::Unavailable)?;
                if self.closed.load(Ordering::SeqCst) {
                    return Err(Failure::Closed);
                }
                *slot = Some(conn);
            }
            dispatched.store(true, Ordering::SeqCst);
            let result = command
                .query_async::<Value>(slot.as_mut().expect("connected"))
                .await;
            match result {
                Ok(value) => Ok(value),
                Err(error) => {
                    slot.take();
                    if error.is_io_error() || error.is_timeout() {
                        Err(if write {
                            Failure::CompletionUnknown
                        } else {
                            Failure::Unavailable
                        })
                    } else {
                        Err(if write {
                            Failure::ScriptErrorMayHaveWritten
                        } else {
                            Failure::Malformed
                        })
                    }
                }
            }
        })
        .await;
        result.unwrap_or_else(|_| {
            Err(if write && dispatched.load(Ordering::SeqCst) {
                Failure::CompletionUnknown
            } else {
                Failure::Unavailable
            })
        })
    }
    pub async fn ping(&self) -> Result<(), Failure> {
        match self.execute(redis::cmd("PING"), false).await? {
            Value::SimpleString(v) if v == "PONG" => Ok(()),
            _ => Err(Failure::Malformed),
        }
    }
    pub async fn get(&self, key: &str) -> Read<Vec<u8>> {
        let mut command = redis::cmd("GET");
        command.arg(key);
        match self.execute(command, false).await {
            Ok(Value::Nil) => Read::Missing,
            Ok(Value::BulkString(bytes)) => Read::Found(bytes),
            Err(Failure::Closed) => Read::Closed,
            Err(Failure::Unavailable) => Read::Unavailable,
            _ => Read::Malformed,
        }
    }
    /// Accept object records only; arrays/scalars/null are malformed, not misses.
    pub async fn get_object(&self, key: &str) -> Read<Json> {
        match self.get(key).await {
            Read::Found(raw) => match ssx_json::from_slice::<Json>(&raw) {
                Ok(v) if v.is_object() => Read::Found(v),
                _ => Read::Malformed,
            },
            Read::Missing => Read::Missing,
            Read::Unavailable => Read::Unavailable,
            Read::Malformed => Read::Malformed,
            Read::Closed => Read::Closed,
        }
    }
    /// Single atomic immutable creation. Unknown write outcomes are never retried.
    pub async fn put_if_absent(
        &self,
        key: &str,
        bytes: &[u8],
        ttl: NonZeroU64,
    ) -> Result<Create, Failure> {
        let mut cmd = redis::cmd("SET");
        cmd.arg(key).arg(bytes).arg("EX").arg(ttl.get()).arg("NX");
        match self.execute(cmd, true).await? {
            Value::Okay => Ok(Create::Stored),
            Value::Nil => Ok(Create::Exists),
            _ => Err(Failure::Malformed),
        }
    }
    pub async fn put(&self, key: &str, bytes: &[u8], ttl: NonZeroU64) -> Result<(), Failure> {
        let mut cmd = redis::cmd("SET");
        cmd.arg(key).arg(bytes).arg("EX").arg(ttl.get());
        match self.execute(cmd, true).await? {
            Value::Okay => Ok(()),
            _ => Err(Failure::Malformed),
        }
    }
    pub async fn acquire<O>(
        &self,
        key: &str,
        tokens: &mut impl TokenSource<O>,
        ttl: NonZeroU64,
    ) -> Result<Option<LeaseToken<O>>, Failure> {
        let token = tokens.next_token();
        let mut cmd = redis::cmd("SET");
        cmd.arg(key)
            .arg(token.as_str())
            .arg("NX")
            .arg("EX")
            .arg(ttl.get());
        match self.execute(cmd, true).await? {
            Value::Okay => Ok(Some(token)),
            Value::Nil => Ok(None),
            _ => Err(Failure::Malformed),
        }
    }
    pub async fn renew<O>(
        &self,
        key: &str,
        token: &LeaseToken<O>,
        ttl: NonZeroU64,
    ) -> Result<Ownership, Failure> {
        self.owned(
            scripts::Script::ResearchLeaseRenew,
            &[key],
            &[token.as_str().into(), ttl.to_string()],
        )
        .await
    }
    pub async fn release<O>(&self, key: &str, token: &LeaseToken<O>) -> Result<Ownership, Failure> {
        self.owned(
            scripts::Script::ResearchLeaseRelease,
            &[key],
            &[token.as_str().into()],
        )
        .await
    }
    pub async fn fenced_write<O>(
        &self,
        lease: &str,
        record: &str,
        token: &LeaseToken<O>,
        bytes: &str,
        ttl: NonZeroU64,
    ) -> Result<Ownership, Failure> {
        self.owned(
            scripts::Script::ResearchLeaseSave,
            &[lease, record],
            &[token.as_str().into(), ttl.to_string(), bytes.into()],
        )
        .await
    }
    async fn owned(
        &self,
        script: scripts::Script,
        keys: &[&str],
        args: &[String],
    ) -> Result<Ownership, Failure> {
        match self.run(script, keys, args).await? {
            scripts::Reply::Flag(true) => Ok(Ownership::Applied),
            scripts::Reply::Flag(false) => Ok(Ownership::Lost),
            _ => Err(Failure::Malformed),
        }
    }
    /// Exact workflow-specific scripts; no generic state-transition replacement.
    pub async fn run(
        &self,
        script: scripts::Script,
        keys: &[&str],
        args: &[String],
    ) -> Result<scripts::Reply, Failure> {
        if keys.len() != script.key_count() || args.len() != script.arg_count() {
            return Err(Failure::InvalidInput);
        }
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(script.source()).arg(keys.len()).arg(keys).arg(args);
        script.decode(self.execute(cmd, true).await?)
    }
    pub async fn quota_time(&self) -> Result<ValkeyQuotaTime, Failure> {
        let value = self.execute(redis::cmd("TIME"), false).await?;
        let parts: Vec<String> = redis::from_redis_value(value).map_err(|_| Failure::Malformed)?;
        if parts.len() != 2 {
            return Err(Failure::Malformed);
        }
        let seconds: u64 = parts[0].parse().map_err(|_| Failure::Malformed)?;
        let micros: u64 = parts[1].parse().map_err(|_| Failure::Malformed)?;
        if micros >= 1_000_000 {
            return Err(Failure::Malformed);
        }
        let millis = seconds
            .checked_mul(1000)
            .and_then(|v| v.checked_add(micros / 1000))
            .ok_or(Failure::Malformed)?;
        Ok(ValkeyQuotaTime::from_milliseconds(millis))
    }
}
/// Python int truncates toward zero. Retention is not an authorization horizon.
pub fn retained_ttl(
    expiry: WallClockExpiry,
    now: WallClockTimestamp,
) -> Result<NonZeroU64, Failure> {
    let delta = (expiry.timestamp().seconds() - now.seconds()).trunc();
    if !delta.is_finite() || delta > (i64::MAX - 300) as f64 {
        return Err(Failure::InvalidInput);
    }
    NonZeroU64::new((delta as i64).saturating_add(300).max(1) as u64).ok_or(Failure::InvalidInput)
}

pub mod rate;
pub mod research;

pub mod browser_identity;
pub mod oauth;

pub mod cache;
pub mod snapshot;

pub mod saved;

pub mod staged;

/// Immutable caller-supplied retrieval receipt authority (W07 increment).
pub mod retrieval;
