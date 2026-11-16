//! Immutable tenant-bound evidence. Reads never invoke a search/provider port.
use crate::{PortFuture, cache::CanonicalResponse};
use ssx_domain::{
    ArtifactRef, QueryId, ResultId, SearchResult, SearchSnapshot, SnapshotId, TenantId,
    WallClockExpiry, WallClockTimestamp,
};
use std::{num::NonZeroU64, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFailure {
    Unavailable,
    Closed,
    Malformed,
    CompletionUnknown,
    InvalidInput,
    Collision,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotWrite {
    Stored,
    Exists,
}
pub trait SnapshotRepository: Send + Sync {
    fn create<'a>(
        &'a self,
        snapshot: &'a SearchSnapshot,
        retention: NonZeroU64,
    ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>>;
    fn read<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a SnapshotId,
    ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>>;
    /// Additive readonly path; existing injected ports retain their read behavior.
    fn read_projected<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a SnapshotId,
    ) -> PortFuture<'a, Result<Option<Box<dyn SnapshotRows>>, SnapshotFailure>> {
        Box::pin(async move {
            Ok(self.read(tenant, id).await?.map(|snapshot| {
                let summary = SnapshotSummary::from_snapshot(&snapshot);
                Box::new(TypedSnapshotRows { snapshot, summary }) as Box<dyn SnapshotRows>
            }))
        })
    }
}
pub use ssx_domain::storage::snapshot::SnapshotSummary;
/// Trusted repository-owned readonly rows. Service checks identity/expiry before
/// publishing this owner; the consumer checks current policy before visitation.
pub trait SnapshotRows: Send + Sync {
    fn summary(&self) -> &SnapshotSummary;
    fn visit_results(
        &self,
        visitor: &mut dyn FnMut(usize, &SearchResult),
    ) -> Result<(), SnapshotFailure>;
}
struct TypedSnapshotRows {
    snapshot: SearchSnapshot,
    summary: SnapshotSummary,
}
impl SnapshotRows for TypedSnapshotRows {
    fn summary(&self) -> &SnapshotSummary {
        &self.summary
    }
    fn visit_results(
        &self,
        visitor: &mut dyn FnMut(usize, &SearchResult),
    ) -> Result<(), SnapshotFailure> {
        for (index, row) in self.snapshot.results().iter().enumerate() {
            visitor(index, row);
        }
        Ok(())
    }
}
impl SnapshotRows for ssx_domain::storage::snapshot::DecodedSnapshotRows {
    fn summary(&self) -> &SnapshotSummary {
        self.summary()
    }
    fn visit_results(
        &self,
        visitor: &mut dyn FnMut(usize, &SearchResult),
    ) -> Result<(), SnapshotFailure> {
        self.visit_results(visitor)
            .map_err(|_| SnapshotFailure::Malformed)
    }
}
pub struct ValidatedSnapshotRead {
    rows: Box<dyn SnapshotRows>,
}
impl ValidatedSnapshotRead {
    pub fn summary(&self) -> &SnapshotSummary {
        self.rows.summary()
    }
    pub fn visit_results(
        &self,
        visitor: &mut dyn FnMut(usize, &SearchResult),
    ) -> Result<(), SnapshotFailure> {
        self.rows.visit_results(visitor)
    }
}
pub enum SnapshotProjectionLookup {
    Found(ValidatedSnapshotRead),
    Missing,
    Expired { expires_at: WallClockExpiry },
    Unavailable,
}
pub trait SnapshotClock: Send + Sync {
    fn now(&self) -> WallClockTimestamp;
}
pub trait SnapshotIds: Send + Sync {
    fn next(&self) -> Result<SnapshotId, SnapshotFailure>;
}
#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotLookup {
    Found(Arc<SearchSnapshot>),
    Missing,
    Expired { expires_at: WallClockExpiry },
    Unavailable,
}
#[derive(Debug, Clone, Copy)]
pub struct SnapshotTtl {
    logical: NonZeroU64,
    retention: NonZeroU64,
}
impl SnapshotTtl {
    pub fn new(seconds: u64) -> Result<Self, SnapshotFailure> {
        if seconds == 0 || seconds > (i64::MAX as u64) - 300 {
            return Err(SnapshotFailure::InvalidInput);
        }
        Ok(Self {
            logical: NonZeroU64::new(seconds).expect("checked"),
            retention: NonZeroU64::new(seconds + 300).expect("checked"),
        })
    }
    pub fn logical_seconds(self) -> u64 {
        self.logical.get()
    }
    pub fn retention(self) -> NonZeroU64 {
        self.retention
    }
}
#[derive(Clone)]
pub struct SnapshotService {
    store: Arc<dyn SnapshotRepository>,
    clock: Arc<dyn SnapshotClock>,
    ids: Arc<dyn SnapshotIds>,
    tenant: TenantId,
    ttl: SnapshotTtl,
}
impl SnapshotService {
    pub fn new(
        store: Arc<dyn SnapshotRepository>,
        clock: Arc<dyn SnapshotClock>,
        ids: Arc<dyn SnapshotIds>,
        tenant: TenantId,
        ttl: SnapshotTtl,
    ) -> Self {
        Self {
            store,
            clock,
            ids,
            tenant,
            ttl,
        }
    }
    pub fn for_tenant(&self, tenant: TenantId) -> Self {
        Self {
            tenant,
            ..self.clone()
        }
    }
    pub async fn create(
        &self,
        canonical: &CanonicalResponse,
        parents: &[ArtifactRef],
    ) -> Result<SnapshotId, SnapshotFailure> {
        let response = canonical.response();
        // Retry known collisions only. Unknown completion never issues a fresh ID.
        for _ in 0..3 {
            let id = self.ids.next()?;
            let created = self.clock.now();
            let expiry =
                WallClockExpiry::new(created.seconds() + self.ttl.logical_seconds() as f64)
                    .map_err(|_| SnapshotFailure::InvalidInput)?;
            let lineage=parents.iter().map(|parent|ssx_json::json!({"from":{"contract":"slopsearx.artifact_ref","version":1,"kind":"snapshot","id":id.as_str()},"relation":"derived_from","to":parent.to_value()})).collect();
            let snapshot = SearchSnapshot::new(
                id.clone(),
                response.query.clone(),
                QueryId::new(response.query_id.clone())
                    .map_err(|_| SnapshotFailure::InvalidInput)?,
                response.results.clone(),
                response.scope.clone(),
                self.tenant.clone(),
                created,
                Some(expiry),
            )
            .map_err(|_| SnapshotFailure::InvalidInput)?
            .with_lineage(response.ranking_explanation.clone(), lineage, true);
            match self.store.create(&snapshot, self.ttl.retention()).await? {
                SnapshotWrite::Stored => return Ok(id),
                SnapshotWrite::Exists => continue,
            }
        }
        Err(SnapshotFailure::Collision)
    }
    pub async fn read_result(&self, id: &ResultId) -> SnapshotResultLookup {
        match self.read(id.snapshot()).await {
            SnapshotLookup::Found(snapshot) => match usize::try_from(id.index())
                .ok()
                .filter(|index| *index < snapshot.total())
            {
                Some(index) => SnapshotResultLookup::Found(SnapshotResult { snapshot, index }),
                None => SnapshotResultLookup::InvalidIndex,
            },
            SnapshotLookup::Missing => SnapshotResultLookup::Missing,
            SnapshotLookup::Expired { expires_at } => SnapshotResultLookup::Expired { expires_at },
            SnapshotLookup::Unavailable => SnapshotResultLookup::Unavailable,
        }
    }
    /// Same repository read and independent F13 authority/expiry mapping. No
    /// visitor can run before all validation and these checks complete.
    pub async fn read_projected(&self, id: &SnapshotId) -> SnapshotProjectionLookup {
        let rows = match self.store.read_projected(&self.tenant, id).await {
            Ok(Some(rows)) => rows,
            Ok(None)
            | Err(
                SnapshotFailure::Malformed
                | SnapshotFailure::InvalidInput
                | SnapshotFailure::Collision,
            ) => return SnapshotProjectionLookup::Missing,
            Err(
                SnapshotFailure::Unavailable
                | SnapshotFailure::Closed
                | SnapshotFailure::CompletionUnknown,
            ) => return SnapshotProjectionLookup::Unavailable,
        };
        let summary = rows.summary();
        if summary.tenant() != &self.tenant || summary.id() != id {
            return SnapshotProjectionLookup::Missing;
        }
        if let Some(expiry) = summary
            .expires_at()
            .filter(|expiry| expiry.expired_at(self.clock.now()))
        {
            return SnapshotProjectionLookup::Expired { expires_at: expiry };
        }
        SnapshotProjectionLookup::Found(ValidatedSnapshotRead { rows })
    }
    pub async fn read(&self, id: &SnapshotId) -> SnapshotLookup {
        let snapshot = match self.store.read(&self.tenant, id).await {
            Ok(Some(s)) => s,
            Ok(None)
            | Err(
                SnapshotFailure::Malformed
                | SnapshotFailure::InvalidInput
                | SnapshotFailure::Collision,
            ) => return SnapshotLookup::Missing,
            Err(
                SnapshotFailure::Unavailable
                | SnapshotFailure::Closed
                | SnapshotFailure::CompletionUnknown,
            ) => return SnapshotLookup::Unavailable,
        };
        // Independent checks prevent a misdirected port from crossing identity.
        if snapshot.tenant() != &self.tenant || snapshot.id() != id {
            return SnapshotLookup::Missing;
        }
        if let Some(expiry) = snapshot
            .expires_at()
            .filter(|expiry| expiry.expired_at(self.clock.now()))
        {
            return SnapshotLookup::Expired { expires_at: expiry };
        }
        SnapshotLookup::Found(Arc::new(snapshot))
    }
}
/// A page references the full immutable snapshot; slicing cannot discard evidence.
#[derive(Debug, Clone)]
pub struct SnapshotPage {
    snapshot: Arc<SearchSnapshot>,
    start: usize,
    end: usize,
    page: u64,
}
impl SnapshotPage {
    pub fn new(
        snapshot: Arc<SearchSnapshot>,
        page: u64,
        page_size: u64,
    ) -> Result<Self, SnapshotFailure> {
        if page == 0 || page_size == 0 {
            return Err(SnapshotFailure::InvalidInput);
        }
        let total = snapshot.results().len();
        let start = (page - 1)
            .checked_mul(page_size)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(usize::MAX)
            .min(total);
        let end = start
            .saturating_add(usize::try_from(page_size).unwrap_or(usize::MAX))
            .min(total);
        Ok(Self {
            snapshot,
            start,
            end,
            page,
        })
    }
    pub fn snapshot(&self) -> &Arc<SearchSnapshot> {
        &self.snapshot
    }
    pub fn page(&self) -> u64 {
        self.page
    }
    pub fn results(&self) -> &[SearchResult] {
        &self.snapshot.results()[self.start..self.end]
    }
    pub fn has_more(&self) -> bool {
        self.end < self.snapshot.total()
    }
    pub fn result_id(&self, offset: usize) -> Option<ResultId> {
        if offset >= self.end - self.start {
            return None;
        }
        Some(ResultId::new(
            self.snapshot.id().clone(),
            (self.start + offset) as u64,
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotResult {
    snapshot: Arc<SearchSnapshot>,
    index: usize,
}
impl SnapshotResult {
    pub fn snapshot(&self) -> &Arc<SearchSnapshot> {
        &self.snapshot
    }
    pub fn result(&self) -> &SearchResult {
        &self.snapshot.results()[self.index]
    }
    pub fn id(&self) -> ResultId {
        ResultId::new(self.snapshot.id().clone(), self.index as u64)
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotResultLookup {
    Found(SnapshotResult),
    Missing,
    InvalidIndex,
    Expired { expires_at: WallClockExpiry },
    Unavailable,
}
