//! Immutable snapshot persistence; uncertain creation never publishes a cursor.
use super::{Create, Failure, Read, Valkey, keys};
use ssx_application::{
    PortFuture,
    snapshot::{SnapshotClock, SnapshotFailure, SnapshotIds, SnapshotRepository, SnapshotWrite},
};
use ssx_domain::{SearchSnapshot, SnapshotId, TenantId, WallClockTimestamp, storage};
use std::{
    num::NonZeroU64,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
pub struct ValkeySnapshots {
    store: Arc<Valkey>,
    limits: storage::StorageLimits,
}
impl ValkeySnapshots {
    pub fn new(store: Arc<Valkey>) -> Self {
        Self::with_limits(store, storage::StorageLimits::default())
    }
    pub fn with_limits(store: Arc<Valkey>, limits: storage::StorageLimits) -> Self {
        Self { store, limits }
    }
}
fn failure(error: Failure) -> SnapshotFailure {
    match error {
        Failure::Closed => SnapshotFailure::Closed,
        Failure::Unavailable => SnapshotFailure::Unavailable,
        Failure::CompletionUnknown | Failure::ScriptErrorMayHaveWritten => {
            SnapshotFailure::CompletionUnknown
        }
        Failure::Malformed => SnapshotFailure::Malformed,
        Failure::InvalidInput => SnapshotFailure::InvalidInput,
    }
}
impl SnapshotRepository for ValkeySnapshots {
    fn create<'a>(
        &'a self,
        snapshot: &'a SearchSnapshot,
        retention: NonZeroU64,
    ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
        Box::pin(async move {
            let value = storage::snapshot::encode_with_limits(snapshot, self.limits)
                .map_err(|_| SnapshotFailure::Malformed)?;
            let raw = ssx_json::to_vec(&value).map_err(|_| SnapshotFailure::Malformed)?;
            if raw.len() > ssx_json::MAX_JSON_BYTES {
                return Err(SnapshotFailure::Malformed);
            }
            match self
                .store
                .put_if_absent(
                    &keys::snapshot(snapshot.tenant(), snapshot.id().as_str()),
                    &raw,
                    retention,
                )
                .await
                .map_err(failure)?
            {
                Create::Stored => Ok(SnapshotWrite::Stored),
                Create::Exists => Ok(SnapshotWrite::Exists),
            }
        })
    }
    fn read<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a SnapshotId,
    ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
        Box::pin(async move {
            let raw = match self.store.get(&keys::snapshot(tenant, id.as_str())).await {
                Read::Found(raw) => raw,
                Read::Missing => return Ok(None),
                Read::Unavailable => return Err(SnapshotFailure::Unavailable),
                Read::Closed => return Err(SnapshotFailure::Closed),
                Read::Malformed => return Err(SnapshotFailure::Malformed),
            };
            let value = ssx_json::from_slice(&raw).map_err(|_| SnapshotFailure::Malformed)?;
            let snapshot =
                storage::snapshot::decode(value).map_err(|_| SnapshotFailure::Malformed)?;
            if snapshot.id() != id || snapshot.tenant() != tenant {
                return Err(SnapshotFailure::Malformed);
            }
            Ok(Some(snapshot))
        })
    }
}
/// Maintained AWS-LC randomness; native shape remains source snap- plus12hex.
pub struct RandomSnapshotIds;
impl SnapshotIds for RandomSnapshotIds {
    fn next(&self) -> Result<SnapshotId, SnapshotFailure> {
        let mut bytes = [0u8; 6];
        aws_lc_rs::rand::fill(&mut bytes).map_err(|_| SnapshotFailure::Unavailable)?;
        let suffix: String = bytes.iter().map(|v| format!("{v:02x}")).collect();
        SnapshotId::new(format!("snap-{suffix}")).map_err(|_| SnapshotFailure::InvalidInput)
    }
}
pub struct SystemSnapshotClock;
impl SnapshotClock for SystemSnapshotClock {
    fn now(&self) -> WallClockTimestamp {
        let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(v) => v.as_secs_f64(),
            Err(v) => -v.duration().as_secs_f64(),
        };
        WallClockTimestamp::new(now).expect("finite system epoch")
    }
}
