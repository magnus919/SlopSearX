//! Real Valkey receipt authority. No URLs are fetched and no command is retried.
//! Fixed receipt workflow with preflighted collision/corruption hardening; no generic CAS.
use super::{Failure as StoreFailure, Read, Valkey};
use base64::Engine as _;
use sha2::{Digest, Sha256};
use ssx_application::{PortFuture, receipts::*};
use ssx_domain::{TenantId, WallClockTimestamp, canonical::python_float};
use ssx_json::Value;
use std::sync::Arc;

const INSERT: &str = include_str!("retrieval_scripts/insert.lua");
pub struct RandomReceiptIds;
impl ReceiptIds for RandomReceiptIds {
    fn next(&self) -> Result<String, Failure> {
        let mut bytes = [0; 18];
        aws_lc_rs::rand::fill(&mut bytes).map_err(|_| Failure::Unavailable)?;
        Ok(format!(
            "receipt-{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
        ))
    }
}
#[derive(Clone)]
pub struct ValkeyReceipts {
    backend: Arc<Valkey>,
    ids: Arc<dyn ReceiptIds>,
}
impl ValkeyReceipts {
    pub fn new(backend: Arc<Valkey>, ids: Arc<dyn ReceiptIds>) -> Self {
        Self { backend, ids }
    }
    fn key(tenant: &TenantId, result_id: &str, kind: &str) -> String {
        let digest = Sha256::digest(result_id.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        format!("mcp:retrieval-receipt:v1:{kind}:{tenant}:{digest}")
    }
    fn prefix(tenant: &TenantId, result_id: &str) -> String {
        format!("{}:", Self::key(tenant, result_id, "receipt"))
    }
    fn idem(tenant: &TenantId, result_id: &str, key: &str) -> String {
        let digest = Sha256::digest(key.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        format!("{}:{digest}", Self::key(tenant, result_id, "idem"))
    }
    async fn object(&self, key: &str, cap: usize) -> Result<Option<Value>, Failure> {
        match self.backend.get(key).await {
            Read::Missing => Ok(None),
            Read::Found(raw) if raw.len() <= cap => {
                let value: Value = ssx_json::from_slice(&raw).map_err(|_| Failure::Malformed)?;
                if value.is_object() {
                    Ok(Some(value))
                } else {
                    Err(Failure::Malformed)
                }
            }
            Read::Closed => Err(Failure::Closed),
            Read::Unavailable => Err(Failure::Unavailable),
            Read::Found(_) | Read::Malformed => Err(Failure::Malformed),
        }
    }
    fn expires(value: &Value) -> Result<f64, Failure> {
        value["expires_at"]
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or(Failure::Malformed)
    }
    fn input(result_id: &str) -> Result<(), Failure> {
        // Full source artifact IDs are at most 1024 Unicode characters. Permit
        // their UTF-8 representation plus the canonical result-index suffix.
        if result_id.is_empty() || result_id.len() > 8192 {
            Err(Failure::InvalidInput)
        } else {
            Ok(())
        }
    }
}
fn failure(error: StoreFailure) -> Failure {
    match error {
        StoreFailure::Closed => Failure::Closed,
        StoreFailure::Unavailable => Failure::Unavailable,
        StoreFailure::CompletionUnknown => Failure::CompletionUnknown,
        StoreFailure::ScriptErrorMayHaveWritten => Failure::ScriptErrorMayHaveWritten,
        StoreFailure::Malformed => Failure::Malformed,
        StoreFailure::InvalidInput => Failure::InvalidInput,
    }
}
impl ReceiptRepository for ValkeyReceipts {
    fn replay<'a>(
        &'a self,
        tenant: &'a TenantId,
        result_id: &'a str,
        idempotency_key: &'a str,
        digest: &'a str,
        now: WallClockTimestamp,
        cancellation: &'a Cancellation,
    ) -> PortFuture<'a, Result<Replay, Failure>> {
        Box::pin(async move {
            cancellation.check()?;
            Self::input(result_id)?;
            if idempotency_key.len() > 512 || digest.len() > 256 {
                return Err(Failure::InvalidInput);
            }
            let Some(prior) = self
                .object(&Self::idem(tenant, result_id, idempotency_key), 2048)
                .await?
            else {
                return Ok(Replay::Missing);
            };
            if Self::expires(&prior)? <= now.seconds() {
                return Ok(Replay::Missing);
            }
            let prior_digest = prior["digest"].as_str().ok_or(Failure::Malformed)?;
            if prior_digest != digest {
                return Ok(Replay::Conflict);
            }
            let id = prior["receipt_id"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 1024)
                .ok_or(Failure::Malformed)?;
            cancellation.check()?;
            let Some(receipt) = self
                .object(
                    &format!("{}{id}", Self::prefix(tenant, result_id)),
                    MAX_RECEIPT_BYTES,
                )
                .await?
            else {
                return Ok(Replay::Missing);
            };
            if receipt["receipt_id"].as_str() != Some(id) {
                return Err(Failure::Malformed);
            }
            Ok(Replay::Replayed(receipt))
        })
    }
    fn submit<'a>(&'a self, insert: Insert<'a>) -> PortFuture<'a, Result<Submission, Failure>> {
        Box::pin(async move {
            insert.cancellation.check()?;
            Self::input(insert.result_id)?;
            if !insert.receipt.is_object()
                || insert.idempotency_key.len() > 512
                || insert.digest.len() > 256
                || !((insert.now.seconds() + RETENTION_SECONDS as f64).is_finite())
            {
                return Err(Failure::InvalidInput);
            }
            let id = self.ids.next()?;
            if !id.starts_with("receipt-")
                || id.len() <= 8
                || id.len() > 1024
                || id.bytes().any(|b| b < 32 || b == 127)
            {
                return Err(Failure::InvalidInput);
            }
            let mut receipt = insert.receipt.clone();
            receipt["receipt_id"] = Value::String(id.clone());
            receipt["submitted_at"] = ssx_json::json!(insert.now.seconds());
            let raw = ssx_json::to_string(&receipt).map_err(|_| Failure::InvalidInput)?;
            // The original script checks the complete encoded record after its
            // expires_at assignment. Do not preempt its replay/capacity order.
            let now = python_float(insert.now.seconds()).map_err(|_| Failure::InvalidInput)?;
            let prefix = Self::prefix(insert.tenant, insert.result_id);
            let keys = [
                Self::key(insert.tenant, insert.result_id, "horizon"),
                Self::key(insert.tenant, insert.result_id, "index"),
                format!("{prefix}{id}"),
                Self::idem(insert.tenant, insert.result_id, insert.idempotency_key),
            ];
            let args = [
                now.clone(),
                insert.digest.to_owned(),
                prefix,
                RETENTION_SECONDS.to_string(),
                STORE_MARGIN_SECONDS.to_string(),
                MAX_RECEIPTS_PER_RESULT.to_string(),
                raw,
                now,
                id,
                MAX_RECEIPT_BYTES.to_string(),
            ];
            let mut command = redis::cmd("EVAL");
            command.arg(INSERT).arg(keys.len()).arg(&keys).arg(&args);
            insert.cancellation.check()?;
            let reply = self.backend.execute(command, true).await.map_err(failure)?;
            let raw: Vec<u8> = redis::from_redis_value(reply).map_err(|_| Failure::Malformed)?;
            if raw.len() > MAX_RECEIPT_BYTES + 1024 {
                return Err(Failure::Malformed);
            }
            let value: Value = ssx_json::from_slice(&raw).map_err(|_| Failure::Malformed)?;
            match value["status"].as_str() {
                Some("created") if value["receipt"].is_object() => {
                    Ok(Submission::Created(value["receipt"].clone()))
                }
                Some("replayed") if value["receipt"].is_object() => {
                    Ok(Submission::Replayed(value["receipt"].clone()))
                }
                Some("conflict") => Ok(Submission::Conflict),
                Some("capacity") => Ok(Submission::Capacity),
                Some("size") => Ok(Submission::Size),
                Some("collision") => Err(Failure::Collision),
                Some("corrupt") => Err(Failure::Malformed),
                _ => Err(Failure::Malformed),
            }
        })
    }
    fn read<'a>(
        &'a self,
        tenant: &'a TenantId,
        result_id: &'a str,
        now: WallClockTimestamp,
        limit: usize,
        cancellation: &'a Cancellation,
    ) -> PortFuture<'a, Result<Retained, Failure>> {
        Box::pin(async move {
            cancellation.check()?;
            Self::input(result_id)?;
            if !(1..=MAX_RECEIPTS_PER_RESULT).contains(&limit) {
                return Err(Failure::InvalidInput);
            }
            let Some(horizon) = self
                .object(&Self::key(tenant, result_id, "horizon"), 2048)
                .await?
            else {
                return Ok(Retained {
                    receipts: vec![],
                    total: 0,
                });
            };
            if Self::expires(&horizon)? <= now.seconds() {
                return Ok(Retained {
                    receipts: vec![],
                    total: 0,
                });
            }
            let index = Self::key(tenant, result_id, "index");
            cancellation.check()?;
            let mut card = redis::cmd("ZCARD");
            card.arg(&index);
            let total: usize =
                redis::from_redis_value(self.backend.execute(card, false).await.map_err(failure)?)
                    .map_err(|_| Failure::Malformed)?;
            if total > MAX_RECEIPTS_PER_RESULT {
                return Err(Failure::Malformed);
            }
            cancellation.check()?;
            let mut range = redis::cmd("ZREVRANGE");
            range.arg(index).arg(0).arg(limit - 1);
            let ids: Vec<String> =
                redis::from_redis_value(self.backend.execute(range, false).await.map_err(failure)?)
                    .map_err(|_| Failure::Malformed)?;
            if ids.len() > limit || ids.iter().any(|id| id.is_empty() || id.len() > 1024) {
                return Err(Failure::Malformed);
            }
            let mut receipts = Vec::with_capacity(ids.len());
            for id in ids {
                cancellation.check()?;
                if let Some(receipt) = self
                    .object(
                        &format!("{}{id}", Self::prefix(tenant, result_id)),
                        MAX_RECEIPT_BYTES,
                    )
                    .await?
                {
                    if receipt["receipt_id"].as_str() != Some(id.as_str()) {
                        return Err(Failure::Malformed);
                    }
                    receipts.push(receipt);
                }
            }
            Ok(Retained { receipts, total })
        })
    }
}

/// Readonly workflow records; receipt persistence above is unchanged.
pub mod lineage;
