//! Optional canonical/answer caches on the borrowed F07 authority.
use super::{Failure, Read, Valkey};
use ssx_application::{
    PortFuture,
    cache::{AnswerRepository, CacheEntry, CacheFailure, CacheRepository, CanonicalResponse},
};
use ssx_domain::storage;
use ssx_json::Value;
use std::{num::NonZeroU64, sync::Arc};
pub struct ValkeyCache {
    store: Arc<Valkey>,
    limits: storage::StorageLimits,
}
impl ValkeyCache {
    pub fn new(store: Arc<Valkey>) -> Self {
        Self::with_limits(store, storage::StorageLimits::default())
    }
    pub fn with_limits(store: Arc<Valkey>, limits: storage::StorageLimits) -> Self {
        Self { store, limits }
    }
}
fn failure(error: Failure) -> CacheFailure {
    match error {
        Failure::Closed => CacheFailure::Closed,
        Failure::Unavailable => CacheFailure::Unavailable,
        Failure::CompletionUnknown | Failure::ScriptErrorMayHaveWritten => {
            CacheFailure::CompletionUnknown
        }
        Failure::Malformed => CacheFailure::Malformed,
        Failure::InvalidInput => CacheFailure::InvalidInput,
    }
}
fn search_key(key: &str) -> bool {
    key.len() <= 4096 && (key.starts_with("search:v2:") || key.starts_with("search:native:v1:"))
}
fn answer_key(key: &str) -> bool {
    key.len() == 74 && key.starts_with("answer:v2:")
}
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => {
            if v.is_f64() {
                v.as_f64().is_some_and(|n| n != 0.0)
            } else {
                v.as_str() != "0"
            }
        }
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}
async fn read(store: &Valkey, key: &str) -> Result<Option<Value>, CacheFailure> {
    match store.get(key).await {
        Read::Found(raw) => {
            let value: Value = ssx_json::from_slice(&raw).map_err(|_| CacheFailure::Malformed)?;
            if !value.is_object() {
                return Err(CacheFailure::Malformed);
            }
            Ok(Some(value))
        }
        Read::Missing => Ok(None),
        Read::Unavailable => Err(CacheFailure::Unavailable),
        Read::Closed => Err(CacheFailure::Closed),
        Read::Malformed => Err(CacheFailure::Malformed),
    }
}
async fn write(
    store: &Valkey,
    key: &str,
    value: &Value,
    ttl: NonZeroU64,
) -> Result<(), CacheFailure> {
    let raw = ssx_json::to_vec(value).map_err(|_| CacheFailure::Malformed)?;
    if raw.len() > ssx_json::MAX_JSON_BYTES {
        return Err(CacheFailure::Malformed);
    }
    store.put(key, &raw, ttl).await.map_err(failure)
}
impl CacheRepository for ValkeyCache {
    fn read<'a>(
        &'a self,
        key: &'a str,
    ) -> PortFuture<'a, Result<Option<CacheEntry>, CacheFailure>> {
        Box::pin(async move {
            if !search_key(key) {
                return Err(CacheFailure::InvalidInput);
            }
            let Some(value) = read(&self.store, key).await? else {
                return Ok(None);
            };
            if value.get("_error").is_some_and(truthy) {
                return Ok(Some(CacheEntry::Negative {
                    timestamp: value
                        .get("timestamp")
                        .and_then(Value::as_f64)
                        .and_then(|v| ssx_domain::WallClockTimestamp::new(v).ok()),
                }));
            }
            let response = storage::decode_response(value).map_err(|_| CacheFailure::Malformed)?;
            Ok(Some(CacheEntry::Canonical(CanonicalResponse::new(
                response,
            ))))
        })
    }
    fn write<'a>(
        &'a self,
        key: &'a str,
        value: &'a CacheEntry,
        ttl: NonZeroU64,
    ) -> PortFuture<'a, Result<(), CacheFailure>> {
        Box::pin(async move {
            if !search_key(key) {
                return Err(CacheFailure::InvalidInput);
            }
            let value = match value {
                CacheEntry::Canonical(v) => {
                    storage::encode_response_with_limits(v.response(), self.limits)
                        .map_err(|_| CacheFailure::Malformed)?
                }
                CacheEntry::Negative { timestamp } => {
                    let timestamp = timestamp.ok_or(CacheFailure::InvalidInput)?.seconds();
                    if !timestamp.is_finite()
                        || !(-9223372036854775808.0..9223372036854775808.0).contains(&timestamp)
                    {
                        return Err(CacheFailure::InvalidInput);
                    }
                    ssx_json::json!({"_error":true,"timestamp":timestamp.trunc() as i64})
                }
            };
            write(&self.store, key, &value, ttl).await
        })
    }
}
impl AnswerRepository for ValkeyCache {
    fn read_answer<'a>(
        &'a self,
        key: &'a str,
    ) -> PortFuture<'a, Result<Option<Value>, CacheFailure>> {
        Box::pin(async move {
            if !answer_key(key) {
                return Err(CacheFailure::InvalidInput);
            }
            read(&self.store, key).await
        })
    }
    fn write_answer<'a>(
        &'a self,
        key: &'a str,
        value: &'a Value,
        ttl: NonZeroU64,
    ) -> PortFuture<'a, Result<(), CacheFailure>> {
        Box::pin(async move {
            if !answer_key(key) || !value.is_object() {
                return Err(CacheFailure::InvalidInput);
            }
            write(&self.store, key, value, ttl).await
        })
    }
}
