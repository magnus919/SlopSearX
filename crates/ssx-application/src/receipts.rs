//! Consumer-owned receipt storage boundary. Supplied capture data is inert.
//! No engine, HTTP client, scheduler or lease-acquisition port is available here.
pub mod captured_manifest;
pub(crate) mod encoding;
pub mod manifest;
pub mod memory;
pub mod observation;
pub mod service;
pub mod source;
use crate::PortFuture;
use ssx_domain::{TenantId, WallClockTimestamp};
use ssx_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub const CONTRACT: &str = "slopsearx.retrieval_receipt";
pub const VERSION: u8 = 1;
pub const RETENTION_SECONDS: u64 = 86_400;
pub const STORE_MARGIN_SECONDS: u64 = 300;
pub const MAX_RECEIPTS_PER_RESULT: usize = 20;
pub const MAX_RECEIPT_BYTES: usize = 32 * 1024;
pub const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
pub const MAX_MANIFEST_RESULTS: usize = 25;

/// Preserve indeterminate writes: an unknown completion is not a failed insert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Unavailable,
    Closed,
    Malformed,
    InvalidInput,
    CompletionUnknown,
    ScriptErrorMayHaveWritten,
    Collision,
    Cancelled,
}
/// Cooperative cancellation never abandons a dispatched store mutation. The
/// caller awaits the current bounded operation before publishing completion.
#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    pub fn check(&self) -> Result<(), Failure> {
        if self.is_cancelled() {
            Err(Failure::Cancelled)
        } else {
            Ok(())
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum Replay {
    Missing,
    Conflict,
    Replayed(Value),
}
#[derive(Debug, Clone, PartialEq)]
pub enum Submission {
    Created(Value),
    Replayed(Value),
    Conflict,
    Capacity,
    Size,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Retained {
    pub receipts: Vec<Value>,
    /// Source index cardinality; missing individual values do not change it.
    pub total: usize,
}
/// Authority comes from the composition/auth boundary, never an observation.
/// Keys, observation digest and policy authorization are distinct inputs.
pub struct Insert<'a> {
    pub tenant: &'a TenantId,
    pub result_id: &'a str,
    pub idempotency_key: &'a str,
    pub digest: &'a str,
    pub receipt: &'a Value,
    pub now: WallClockTimestamp,
    pub cancellation: &'a Cancellation,
}
pub trait ReceiptRepository: Send + Sync {
    fn replay<'a>(
        &'a self,
        tenant: &'a TenantId,
        result_id: &'a str,
        idempotency_key: &'a str,
        digest: &'a str,
        now: WallClockTimestamp,
        cancellation: &'a Cancellation,
    ) -> PortFuture<'a, Result<Replay, Failure>>;
    fn submit<'a>(&'a self, insert: Insert<'a>) -> PortFuture<'a, Result<Submission, Failure>>;
    fn read<'a>(
        &'a self,
        tenant: &'a TenantId,
        result_id: &'a str,
        now: WallClockTimestamp,
        limit: usize,
        cancellation: &'a Cancellation,
    ) -> PortFuture<'a, Result<Retained, Failure>>;
}
/// Production supplies unpredictable IDs; deterministic fixtures can exercise
/// collisions without weakening the live source of authority.
pub trait ReceiptIds: Send + Sync {
    fn next(&self) -> Result<String, Failure>;
}
