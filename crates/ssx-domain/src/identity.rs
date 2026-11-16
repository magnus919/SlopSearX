//! Authority-bearing identifiers and distinct clock domains.
use crate::{DomainError, ScopeDecision, SearchResult};
use std::{
    marker::PhantomData,
    time::{Duration, Instant},
};
macro_rules! identifier {
    ($($name:ident),+ $(,)?) => {$ (
        #[derive(Debug,Clone,PartialEq,Eq,PartialOrd,Ord,Hash)]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self,DomainError> {
                let value=value.into();
                if value.is_empty() || value.chars().count()>1024 || value.chars().any(|c|c.is_ascii_control()) {
                    return Err(DomainError("identifier must contain 1..1024 characters without ASCII controls"));
                }
                Ok(Self(value))
            }
            pub fn as_str(&self)->&str {&self.0}
        }
        impl std::fmt::Display for $name {fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str(&self.0)}}
    )+};
}
identifier!(
    SnapshotId,
    QueryId,
    TenantId,
    JobId,
    AttemptId,
    SavedSearchId,
    StagedSearchId,
    ReceiptId
);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultId {
    snapshot: SnapshotId,
    index: u64,
}
impl ResultId {
    pub fn new(snapshot: SnapshotId, index: u64) -> Self {
        Self { snapshot, index }
    }
    pub fn snapshot(&self) -> &SnapshotId {
        &self.snapshot
    }
    pub fn index(&self) -> u64 {
        self.index
    }
}
impl std::fmt::Display for ResultId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.snapshot, self.index)
    }
}
/// A lease cannot be substituted for an identifier, nor used for another owner's type.
/// ```compile_fail
/// use ssx_domain::{LeaseToken,JobId,AttemptId};
/// fn claim(_: LeaseToken<JobId>) {}
/// let attempt = LeaseToken::<AttemptId>::new("token").unwrap();
/// claim(attempt);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseToken<Owner> {
    value: String,
    owner: PhantomData<Owner>,
}
impl<Owner> LeaseToken<Owner> {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = JobId::new(value)?;
        Ok(Self {
            value: value.0,
            owner: PhantomData,
        })
    }
    pub fn as_str(&self) -> &str {
        &self.value
    }
}
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct WallClockTimestamp(f64);
impl WallClockTimestamp {
    pub fn new(seconds: f64) -> Result<Self, DomainError> {
        if seconds.is_finite() {
            Ok(Self(seconds))
        } else {
            Err(DomainError("wall time must be finite"))
        }
    }
    pub fn seconds(self) -> f64 {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct WallClockExpiry(WallClockTimestamp);
impl WallClockExpiry {
    pub fn new(seconds: f64) -> Result<Self, DomainError> {
        WallClockTimestamp::new(seconds).map(Self)
    }
    pub fn timestamp(self) -> WallClockTimestamp {
        self.0
    }
    pub fn expired_at(self, now: WallClockTimestamp) -> bool {
        now > self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValkeyQuotaTime(u64);
impl ValkeyQuotaTime {
    pub fn from_milliseconds(value: u64) -> Self {
        Self(value)
    }
    pub fn milliseconds(self) -> u64 {
        self.0
    }
}
/// Process-local deadlines deliberately have no durable serialization implementation.
#[derive(Debug, Clone, Copy)]
pub struct MonotonicDeadline(Instant);
impl MonotonicDeadline {
    pub fn after(now: Instant, budget: Duration) -> Result<Self, DomainError> {
        now.checked_add(budget)
            .map(Self)
            .ok_or(DomainError("deadline overflow"))
    }
    pub fn remaining(self, now: Instant) -> Duration {
        self.0.saturating_duration_since(now)
    }
    pub fn elapsed(self, now: Instant) -> bool {
        now >= self.0
    }
}
pub trait Clock {
    fn wall_time(&self) -> WallClockTimestamp;
    fn monotonic_time(&self) -> Instant;
}
pub trait IdGenerator {
    fn next_query_id(&mut self) -> QueryId;
    fn next_snapshot_id(&mut self) -> SnapshotId;
}
/// Complete captured pages. Construction prevents impossible totals and expiry order.
/// External callers cannot construct or mutate an inconsistent captured page.
/// ```compile_fail
/// use ssx_domain::*;
/// let snapshot = SearchSnapshot {
///     id: SnapshotId::new("s").unwrap(), query: "q".into(),
///     query_id: QueryId::new("q").unwrap(), results: vec![],
///     scope: ScopeDecision::default(), tenant: TenantId::new("t").unwrap(),
///     created_at: WallClockTimestamp::new(2.0).unwrap(),
///     expires_at: Some(WallClockExpiry::new(1.0).unwrap()),
///     ranking_explanation: "rank".into(), lineage: vec![], lineage_available: true,
/// };
/// ```

#[derive(Debug, Clone, PartialEq)]
pub struct SearchSnapshot {
    id: SnapshotId,
    query: String,
    query_id: QueryId,
    results: Vec<SearchResult>,
    scope: ScopeDecision,
    tenant: TenantId,
    created_at: WallClockTimestamp,
    expires_at: Option<WallClockExpiry>,
    ranking_explanation: String,
    lineage: Vec<ssx_json::Value>,
    lineage_available: bool,
}
impl SearchSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: SnapshotId,
        query: String,
        query_id: QueryId,
        results: Vec<SearchResult>,
        scope: ScopeDecision,
        tenant: TenantId,
        created_at: WallClockTimestamp,
        expires_at: Option<WallClockExpiry>,
    ) -> Result<Self, DomainError> {
        validate_snapshot_expiry(created_at, expires_at)?;
        Ok(Self {
            id,
            query,
            query_id,
            results,
            scope,
            tenant,
            created_at,
            expires_at,
            ranking_explanation: "tier_then_cross_engine_presence".into(),
            lineage: vec![],
            lineage_available: true,
        })
    }
    pub fn with_lineage(
        mut self,
        ranking_explanation: String,
        lineage: Vec<ssx_json::Value>,
        lineage_available: bool,
    ) -> Self {
        self.ranking_explanation = ranking_explanation;
        self.lineage = lineage;
        self.lineage_available = lineage_available;
        self
    }
    pub fn ranking_explanation(&self) -> &str {
        &self.ranking_explanation
    }
    pub fn lineage(&self) -> &[ssx_json::Value] {
        &self.lineage
    }
    pub fn lineage_available(&self) -> bool {
        self.lineage_available
    }
    pub fn id(&self) -> &SnapshotId {
        &self.id
    }
    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn query_id(&self) -> &QueryId {
        &self.query_id
    }
    pub fn results(&self) -> &[SearchResult] {
        &self.results
    }
    pub fn total(&self) -> usize {
        self.results.len()
    }
    pub fn scope(&self) -> &ScopeDecision {
        &self.scope
    }
    pub fn tenant(&self) -> &TenantId {
        &self.tenant
    }
    pub fn created_at(&self) -> WallClockTimestamp {
        self.created_at
    }
    pub fn expires_at(&self) -> Option<WallClockExpiry> {
        self.expires_at
    }
}
/// Shared construction invariant for full and readonly projected snapshots.
pub(crate) fn validate_snapshot_expiry(
    created_at: WallClockTimestamp,
    expires_at: Option<WallClockExpiry>,
) -> Result<(), DomainError> {
    if expires_at.is_some_and(|expiry| expiry.timestamp() < created_at) {
        return Err(DomainError("snapshot expiry precedes creation"));
    }
    Ok(())
}
#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotRead {
    Found(Box<SearchSnapshot>),
    Missing,
    Expired { expires_at: WallClockExpiry },
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Snapshot,
    Result,
    EntityGroup,
    ResearchJob,
    ResearchAttempt,
    StagedSearch,
    SavedSearch,
    SavedReport,
    RetrievalReceipt,
    ResearchManifest,
    DependencyDossier,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageRelation {
    Contains,
    DerivedFrom,
    Selected,
    Observed,
    RetrievalOf,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRef {
    kind: ArtifactKind,
    id: String,
}
impl ArtifactRef {
    pub fn new(kind: ArtifactKind, id: impl Into<String>) -> Result<Self, DomainError> {
        Ok(Self {
            kind,
            id: SnapshotId::new(id)?.0,
        })
    }
    pub fn kind(&self) -> ArtifactKind {
        self.kind
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn to_value(&self) -> ssx_json::Value {
        ssx_json::json!({"contract":"slopsearx.artifact_ref","version":1,"kind":self.kind,"id":self.id})
    }
    pub fn from_value(value: &ssx_json::Value) -> Result<Self, DomainError> {
        let object = value
            .as_object()
            .ok_or(DomainError("artifact must be object"))?;
        if object.len() != 4
            || object.get("contract") != Some(&ssx_json::json!("slopsearx.artifact_ref"))
            || object.get("version") != Some(&ssx_json::json!(1))
        {
            return Err(DomainError("invalid artifact contract"));
        }
        let kind = ssx_json::from_value(
            object
                .get("kind")
                .cloned()
                .ok_or(DomainError("missing artifact kind"))?,
        )
        .map_err(|_| DomainError("invalid artifact kind"))?;
        Self::new(
            kind,
            object
                .get("id")
                .and_then(ssx_json::Value::as_str)
                .ok_or(DomainError("invalid artifact id"))?,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_authority_and_lifecycle() {
        assert!(SnapshotId::new("").is_err());
        assert!(JobId::new("a\n").is_err());
        assert!(SnapshotId::new("a".repeat(1024)).is_ok());
        assert!(SnapshotId::new("a".repeat(1025)).is_err());
        let expiry = WallClockExpiry::new(2.0).unwrap();
        assert!(!expiry.expired_at(WallClockTimestamp::new(2.0).unwrap()));
        assert!(expiry.expired_at(WallClockTimestamp::new(2.001).unwrap()));

        assert!(WallClockTimestamp::new(f64::INFINITY).is_err());
        let snapshot = SearchSnapshot::new(
            SnapshotId::new("s").unwrap(),
            "q".into(),
            QueryId::new("q").unwrap(),
            vec![],
            ScopeDecision::default(),
            TenantId::new("t").unwrap(),
            WallClockTimestamp::new(2.0).unwrap(),
            Some(WallClockExpiry::new(1.0).unwrap()),
        );
        assert!(snapshot.is_err());
        let deadline = MonotonicDeadline::after(Instant::now(), Duration::ZERO).unwrap();
        assert!(deadline.elapsed(Instant::now()));
    }
    #[test]
    fn artifact_shape_closed() {
        let reference = ArtifactRef::new(ArtifactKind::ResearchAttempt, "id:雪").unwrap();
        assert_eq!(
            ArtifactRef::from_value(&reference.to_value()).unwrap(),
            reference
        );
        let mut invalid = reference.to_value();
        invalid["extra"] = true.into();
        assert!(ArtifactRef::from_value(&invalid).is_err());
        assert!(ArtifactRef::from_value(&ssx_json::json!({"contract":"slopsearx.artifact_ref","version":true,"kind":"snapshot","id":"a"})).is_err());
    }
    #[test]
    fn synthetic_latency_is_not_observed() {
        let mut response = crate::AdapterResponse::default();
        assert_eq!(response.observed_latency_ms(), None);
        response.latency_ms = crate::FiniteF64::new(2.0).unwrap();
        assert_eq!(response.observed_latency_ms().unwrap().get(), 2.0);
        response.synthetic = true;
        assert_eq!(response.observed_latency_ms(), None);
    }
}
