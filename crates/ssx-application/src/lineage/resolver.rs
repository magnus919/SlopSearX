//! Concrete snapshot/result/receipt lineage resolution. Composition must supply
//! the workflow and entity projections; no lookup acquires or renews ownership.
use super::{
    Edge, Failure, Node, Resolved, Resolver, Status,
    adjacency::{Adjacency, CompactResolved, Members, Results},
    input,
};
use crate::{
    PortFuture,
    policy::PolicyService,
    receipts::{Cancellation, ReceiptRepository, service::Clock, source::result_parts},
    snapshot::{SnapshotLookup, SnapshotProjectionLookup, SnapshotService, ValidatedSnapshotRead},
};
use ssx_domain::{
    ArtifactKind, ArtifactRef, LineageRelation, SearchSnapshot, SnapshotId, TenantId,
    WallClockTimestamp, canonical::composite_artifact_id,
};
use ssx_json::{Value, json};
use std::sync::Arc;

/// A fresh immutable policy snapshot per lookup. The implementation uses the
/// existing shared gate's retained_engine_rejection, never dispatch authority.
pub trait Policy: Send + Sync {
    fn current(&self) -> PolicyService;
    /// Sample policy and operator bounds together at an existing authorization
    /// boundary. Legacy injected ports retain their fixed configured bounds.
    fn current_with_limits(
        &self,
        staged: &crate::staged::Limits,
        reports: usize,
    ) -> (PolicyService, crate::staged::Limits, usize) {
        let mut staged = staged.clone();
        staged.granted = true;
        (self.current(), staged, reports)
    }
}
#[derive(Clone, Debug)]
pub struct EntityGroup {
    pub id: String,
    pub namespace: String,
    pub result_indices: Vec<usize>,
}
pub trait Entities: Send + Sync {
    fn groups(&self, snapshot: &SearchSnapshot, version: u8) -> Vec<EntityGroup>;
    /// Existing injected entity ports keep their full snapshot path unchanged.
    /// Only a trusted streaming-capable implementation opts into F13 visitation.
    fn projected(&self) -> Option<&dyn ProjectedEntities> {
        None
    }
}
pub trait ProjectedEntities: Send + Sync {
    fn groups(
        &self,
        snapshot: &ValidatedSnapshotRead,
        version: u8,
    ) -> Result<Vec<EntityGroup>, Failure>;
}
enum SnapshotView {
    Full(std::sync::Arc<SearchSnapshot>),
    Projected(ValidatedSnapshotRead),
}
impl SnapshotView {
    fn id(&self) -> &SnapshotId {
        match self {
            Self::Full(s) => s.id(),
            Self::Projected(s) => s.summary().id(),
        }
    }
    fn query_id(&self) -> &ssx_domain::QueryId {
        match self {
            Self::Full(s) => s.query_id(),
            Self::Projected(s) => s.summary().query_id(),
        }
    }
    fn scope(&self) -> &ssx_domain::ScopeDecision {
        match self {
            Self::Full(s) => s.scope(),
            Self::Projected(s) => s.summary().scope(),
        }
    }
    fn total(&self) -> usize {
        match self {
            Self::Full(s) => s.total(),
            Self::Projected(s) => s.summary().total(),
        }
    }
    fn created_at(&self) -> WallClockTimestamp {
        match self {
            Self::Full(s) => s.created_at(),
            Self::Projected(s) => s.summary().created_at(),
        }
    }
    fn expires_at(&self) -> Option<ssx_domain::WallClockExpiry> {
        match self {
            Self::Full(s) => s.expires_at(),
            Self::Projected(s) => s.summary().expires_at(),
        }
    }
    fn lineage(&self) -> &[Value] {
        match self {
            Self::Full(s) => s.lineage(),
            Self::Projected(s) => s.summary().lineage(),
        }
    }
    fn lineage_available(&self) -> bool {
        match self {
            Self::Full(s) => s.lineage_available(),
            Self::Projected(s) => s.summary().lineage_available(),
        }
    }
    fn groups(&self, entities: &dyn Entities, version: u8) -> Result<Vec<EntityGroup>, Failure> {
        match self {
            Self::Full(s) => Ok(entities.groups(s, version)),
            Self::Projected(s) => entities
                .projected()
                .ok_or(Failure::Malformed)?
                .groups(s, version),
        }
    }
}
enum SnapshotViewLookup {
    Found(SnapshotView),
    Missing,
    Expired(ssx_domain::WallClockExpiry),
    Unavailable,
}

pub struct NativeResolver {
    pub snapshots: SnapshotService,
    pub receipts: Arc<dyn ReceiptRepository>,
    pub policy: Arc<dyn Policy>,
    pub clock: Arc<dyn Clock>,
    pub entities: Arc<dyn Entities>,
    /// Mandatory real workflow resolver, rather than synthetic missing records.
    pub workflows: Arc<dyn Resolver>,
}
fn reference(kind: ArtifactKind, id: &str) -> Result<ArtifactRef, Failure> {
    ArtifactRef::new(kind, id).map_err(|_| Failure::Malformed)
}
fn edge(from: ArtifactRef, relation: LineageRelation, to: ArtifactRef) -> Edge {
    Edge { from, relation, to }
}
fn empty(reference: &ArtifactRef, status: Status) -> Resolved {
    Resolved {
        node: Node::missing(reference.clone(), status),
        edges: vec![],
    }
}
fn parent_error(reference: &ArtifactRef, parent: &Node) -> Resolved {
    if parent.status == Status::PolicyDenied {
        return Resolved {
            node: Node::missing(reference.clone(), Status::PolicyDenied),
            edges: vec![],
        };
    }
    let mut result = empty(reference, parent.status);
    result.node.expires_at = parent.expires_at;
    result
}
fn map_failure(f: crate::receipts::Failure) -> Failure {
    match f {
        crate::receipts::Failure::Closed => Failure::Closed,
        crate::receipts::Failure::Cancelled => Failure::Cancelled,
        crate::receipts::Failure::Unavailable | crate::receipts::Failure::CompletionUnknown => {
            Failure::Unavailable
        }
        _ => Failure::Malformed,
    }
}
impl NativeResolver {
    async fn read_snapshot(&self, tenant: &TenantId, id: &SnapshotId) -> SnapshotViewLookup {
        let service = self.snapshots.for_tenant(tenant.clone());
        if self.entities.projected().is_some() {
            match service.read_projected(id).await {
                SnapshotProjectionLookup::Found(owner) => {
                    SnapshotViewLookup::Found(SnapshotView::Projected(owner))
                }
                SnapshotProjectionLookup::Missing => SnapshotViewLookup::Missing,
                SnapshotProjectionLookup::Expired { expires_at } => {
                    SnapshotViewLookup::Expired(expires_at)
                }
                SnapshotProjectionLookup::Unavailable => SnapshotViewLookup::Unavailable,
            }
        } else {
            match service.read(id).await {
                SnapshotLookup::Found(owner) => {
                    SnapshotViewLookup::Found(SnapshotView::Full(owner))
                }
                SnapshotLookup::Missing => SnapshotViewLookup::Missing,
                SnapshotLookup::Expired { expires_at } => SnapshotViewLookup::Expired(expires_at),
                SnapshotLookup::Unavailable => SnapshotViewLookup::Unavailable,
            }
        }
    }
    async fn snapshot(
        &self,
        tenant: &TenantId,
        root: &ArtifactRef,
        cancel: &Cancellation,
    ) -> Result<CompactResolved, Failure> {
        let Ok(id) = SnapshotId::new(root.id()) else {
            return Ok(empty(root, Status::Missing).into());
        };
        let read = self.read_snapshot(tenant, &id).await;
        if cancel.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        let snapshot = match read {
            SnapshotViewLookup::Found(v) => v,
            SnapshotViewLookup::Missing => return Ok(empty(root, Status::Missing).into()),
            SnapshotViewLookup::Unavailable => return Ok(empty(root, Status::Unavailable).into()),
            SnapshotViewLookup::Expired(expires_at) => {
                let mut r = empty(root, Status::Expired);
                r.node.expires_at = Some(expires_at.timestamp());
                return Ok(r.into());
            }
        };
        if self
            .policy
            .current()
            .retained_engine_rejection(&snapshot.scope().selected_engines, "engines")
            .is_err()
        {
            return Ok(Resolved {
                node: Node::policy_denied(root.clone()),
                edges: vec![],
            }
            .into());
        }
        let mut edges = snapshot
            .lineage()
            .iter()
            .filter_map(Edge::from_value)
            .collect::<Vec<_>>();
        let results = Results::new(root.clone(), snapshot.total())?;
        for group in snapshot.groups(self.entities.as_ref(), 1)? {
            if !group.id.is_empty() {
                let id = composite_artifact_id(&[snapshot.id().as_str(), &group.id])
                    .map_err(|_| Failure::Malformed)?;
                edges.push(edge(
                    reference(ArtifactKind::EntityGroup, &id)?,
                    LineageRelation::DerivedFrom,
                    root.clone(),
                ));
            }
        }
        Ok(CompactResolved{node:Node{artifact:root.clone(),status:Status::Live,created_at:Some(snapshot.created_at()),expires_at:snapshot.expires_at().map(|v|v.timestamp()),details:Some(json!({"query_id":snapshot.query_id().as_str(),"result_count":snapshot.total(),"lineage_status":if snapshot.lineage_available(){"available"}else{"lineage_unavailable"}}).as_object().unwrap().clone())},edges:Adjacency::new(edges,Some(results))})
    }
    async fn result(
        &self,
        tenant: &TenantId,
        root: &ArtifactRef,
        cancel: &Cancellation,
    ) -> Result<Resolved, Failure> {
        let Some((id, index)) = result_parts(root.id()) else {
            return Ok(empty(root, Status::Missing));
        };
        let Ok(parent) = reference(ArtifactKind::Snapshot, id) else {
            return Ok(empty(root, Status::Missing));
        };
        let status = self.snapshot(tenant, &parent, cancel).await?;
        if status.node.status != Status::Live {
            return Ok(parent_error(root, &status.node));
        }
        let read = self
            .read_snapshot(
                tenant,
                &SnapshotId::new(id).map_err(|_| Failure::Malformed)?,
            )
            .await;
        if cancel.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        let SnapshotViewLookup::Found(snapshot) = read else {
            return Ok(empty(root, Status::Missing));
        };
        let Some(index) = index.filter(|i| *i < snapshot.total()) else {
            return Ok(empty(root, Status::Missing));
        };
        let policy = self.policy.current();
        if policy
            .retained_engine_rejection(&snapshot.scope().selected_engines, "engines")
            .is_err()
        {
            return Ok(Resolved {
                node: Node::policy_denied(root.clone()),
                edges: vec![],
            });
        }
        let mut edges = vec![edge(root.clone(), LineageRelation::DerivedFrom, parent)];
        if policy.policy.grants.contains("retrieval_receipts") {
            let retained = self
                .receipts
                .read(tenant, root.id(), self.clock.now(), 20, cancel)
                .await;
            if cancel.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            let current = self.policy.current();
            if current
                .retained_engine_rejection(&snapshot.scope().selected_engines, "engines")
                .is_err()
            {
                return Ok(Resolved {
                    node: Node::policy_denied(root.clone()),
                    edges: vec![],
                });
            }
            // Results remain visible without a receipt grant, but no receipt
            // relationship may escape a grant revoked during this read.
            if current.policy.grants.contains("retrieval_receipts") {
                match retained {
                    Ok(retained) => {
                        for receipt in retained.receipts {
                            if let Some(id) = receipt
                                .get("receipt_id")
                                .and_then(Value::as_str)
                                .filter(|s| !s.is_empty())
                            {
                                let composite = composite_artifact_id(&[root.id(), id])
                                    .map_err(|_| Failure::Malformed)?;
                                edges.push(edge(
                                    reference(ArtifactKind::RetrievalReceipt, &composite)?,
                                    LineageRelation::RetrievalOf,
                                    root.clone(),
                                ));
                            }
                        }
                    }
                    // Source store.read reports no receipts when disconnected.
                    Err(
                        crate::receipts::Failure::Unavailable | crate::receipts::Failure::Closed,
                    ) => {}
                    Err(e) => return Err(map_failure(e)),
                }
            }
        }
        Ok(Resolved {
            node: Node {
                artifact: root.clone(),
                status: Status::Live,
                created_at: Some(snapshot.created_at()),
                expires_at: snapshot.expires_at().map(|v| v.timestamp()),
                details: Some(json!({"index":index}).as_object().unwrap().clone()),
            },
            edges,
        })
    }
    async fn entity(
        &self,
        tenant: &TenantId,
        root: &ArtifactRef,
        cancel: &Cancellation,
    ) -> Result<CompactResolved, Failure> {
        let Ok(parts) = input::composite(root.id(), 2) else {
            return Ok(empty(root, Status::Missing).into());
        };
        let Ok(parent) = reference(ArtifactKind::Snapshot, &parts[0]) else {
            return Ok(empty(root, Status::Missing).into());
        };
        let status = self.snapshot(tenant, &parent, cancel).await?;
        if status.node.status != Status::Live {
            return Ok(parent_error(root, &status.node).into());
        }
        let Ok(id) = SnapshotId::new(&parts[0]) else {
            return Ok(empty(root, Status::Missing).into());
        };
        let read = self.read_snapshot(tenant, &id).await;
        if cancel.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        let SnapshotViewLookup::Found(snapshot) = read else {
            return Ok(empty(root, Status::Missing).into());
        };
        let policy = self.policy.current();
        if policy
            .retained_engine_rejection(&snapshot.scope().selected_engines, "engines")
            .is_err()
        {
            return Ok(Resolved {
                node: Node::policy_denied(root.clone()),
                edges: vec![],
            }
            .into());
        }
        let version = if parts[1].starts_with("entity-v2-") {
            2
        } else {
            1
        };
        let Some(group) = snapshot
            .groups(self.entities.as_ref(), version)?
            .into_iter()
            .find(|g| g.id == parts[1])
        else {
            return Ok(empty(root, Status::Missing).into());
        };
        let member_count = group.result_indices.len();
        let edges = Adjacency::with_members(
            vec![edge(root.clone(), LineageRelation::DerivedFrom, parent)],
            Members::new(root.clone(), snapshot.id().as_str(), group.result_indices)?,
        );
        Ok(CompactResolved {
            node: Node {
                artifact: root.clone(),
                status: Status::Live,
                created_at: Some(snapshot.created_at()),
                expires_at: snapshot.expires_at().map(|v| v.timestamp()),
                details: Some(
                    json!({"namespace":group.namespace,"member_count":member_count})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
            },
            edges,
        })
    }
    async fn receipt(
        &self,
        tenant: &TenantId,
        root: &ArtifactRef,
        cancel: &Cancellation,
    ) -> Result<Resolved, Failure> {
        if !self
            .policy
            .current()
            .policy
            .grants
            .contains("retrieval_receipts")
        {
            return Ok(Resolved {
                node: Node::policy_denied(root.clone()),
                edges: vec![],
            });
        }
        let Ok(parts) = input::composite(root.id(), 2) else {
            return Ok(empty(root, Status::Missing));
        };
        let read = self
            .receipts
            .read(tenant, &parts[0], self.clock.now(), 20, cancel)
            .await;
        if cancel.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        let policy = self.policy.current();
        if !policy.policy.grants.contains("retrieval_receipts") {
            return Ok(Resolved {
                node: Node::policy_denied(root.clone()),
                edges: vec![],
            });
        }
        let retained = match read {
            Ok(v) => v,
            Err(crate::receipts::Failure::Unavailable | crate::receipts::Failure::Closed) => {
                return Ok(empty(root, Status::Unavailable));
            }
            Err(e) => return Err(map_failure(e)),
        };
        if cancel.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        let Some(receipt) = retained
            .receipts
            .into_iter()
            .find(|r| r.get("receipt_id").and_then(Value::as_str) == Some(&parts[1]))
        else {
            return Ok(empty(root, Status::Missing));
        };
        let engines = receipt
            .get("discovery")
            .and_then(|v| v.get("source_engines"));
        let allowed = engines.and_then(Value::as_array).is_some_and(|a| {
            let names = a
                .iter()
                .map(|v| v.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>();
            names.is_some_and(|names| policy.retained_engine_rejection(&names, "engines").is_ok())
        });
        if !allowed {
            return Ok(Resolved {
                node: Node::policy_denied(root.clone()),
                edges: vec![],
            });
        }
        let timestamp = |field: &str| -> Result<Option<WallClockTimestamp>, Failure> {
            receipt
                .get(field)
                .filter(|v| !v.is_null())
                .map(|v| {
                    v.as_f64()
                        .and_then(|n| WallClockTimestamp::new(n).ok())
                        .ok_or(Failure::Malformed)
                })
                .transpose()
        };
        Ok(Resolved {
            node: Node {
                artifact: root.clone(),
                status: Status::Live,
                created_at: timestamp("submitted_at")?,
                expires_at: timestamp("expires_at")?,
                details: Some(
                    json!({"status":receipt["observation"]["status"]})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
            },
            edges: vec![edge(
                root.clone(),
                LineageRelation::RetrievalOf,
                reference(ArtifactKind::Result, &parts[0])?,
            )],
        })
    }
}
impl Resolver for NativeResolver {
    fn resolve<'a>(
        &'a self,
        tenant: &'a TenantId,
        root: &'a ArtifactRef,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Resolved, Failure>> {
        Box::pin(async move {
            self.resolve_compact(tenant, root, cancel)
                .await
                .map(CompactResolved::materialize)
        })
    }
    fn resolve_compact<'a>(
        &'a self,
        tenant: &'a TenantId,
        root: &'a ArtifactRef,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<CompactResolved, Failure>> {
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            match root.kind() {
                ArtifactKind::Snapshot => self.snapshot(tenant, root, cancel).await,
                ArtifactKind::Result => self.result(tenant, root, cancel).await.map(Into::into),
                ArtifactKind::EntityGroup => self.entity(tenant, root, cancel).await,
                ArtifactKind::RetrievalReceipt => {
                    self.receipt(tenant, root, cancel).await.map(Into::into)
                }
                ArtifactKind::ResearchManifest => Ok(empty(root, Status::Unavailable).into()),
                _ => self.workflows.resolve_compact(tenant, root, cancel).await,
            }
        })
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::{
        PortFuture,
        policy::{PolicyService, SearchPolicy},
        receipts::{Failure as ReceiptFailure, Insert, Replay, Retained, Submission},
        scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
        snapshot::{
            SnapshotClock, SnapshotFailure, SnapshotIds, SnapshotRepository, SnapshotTtl,
            SnapshotWrite,
        },
    };
    use ssx_domain::SearchResult;
    use std::{
        collections::BTreeSet,
        future::Future,
        num::NonZeroU64,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    struct CurrentPolicy(bool);
    impl Policy for CurrentPolicy {
        fn current(&self) -> PolicyService {
            PolicyService {
                policy: SearchPolicy {
                    grants: if self.0 {
                        ["retrieval_receipts".to_owned()].into_iter().collect()
                    } else {
                        BTreeSet::new()
                    },
                    ..Default::default()
                },
                catalog: CatalogSnapshot::new(vec![EngineCapability {
                    name: "brave".into(),
                    enabled: true,
                    active: true,
                    available: true,
                    categories: vec![],
                    media_types: vec![],
                    auth_ready: true,
                    circuit_open: false,
                    cost_class: "free".into(),
                    health_status: "ok".into(),
                    health_stale: false,
                    enforced_filters: Default::default(),
                }])
                .unwrap(),
                resolver: ScopeResolver::default(),
            }
        }
    }
    struct TestClock;
    impl Clock for TestClock {
        fn now(&self) -> WallClockTimestamp {
            WallClockTimestamp::new(100.).unwrap()
        }
    }
    impl SnapshotClock for TestClock {
        fn now(&self) -> WallClockTimestamp {
            WallClockTimestamp::new(100.).unwrap()
        }
    }
    struct Ids;
    impl SnapshotIds for Ids {
        fn next(&self) -> Result<SnapshotId, SnapshotFailure> {
            panic!("readonly resolver requested an id")
        }
    }
    struct EmptySnapshots;
    impl SnapshotRepository for EmptySnapshots {
        fn create<'a>(
            &'a self,
            _: &'a SearchSnapshot,
            _: NonZeroU64,
        ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
            Box::pin(async { panic!("readonly resolver created a snapshot") })
        }
        fn read<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a SnapshotId,
        ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
            Box::pin(async { panic!("receipt resolution must not read a snapshot") })
        }
    }
    struct Receipts {
        calls: AtomicUsize,
        rows: Vec<Value>,
        trace: Arc<Mutex<Vec<String>>>,
    }
    impl ReceiptRepository for Receipts {
        fn replay<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a str,
            _: &'a str,
            _: &'a str,
            _: WallClockTimestamp,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Replay, ReceiptFailure>> {
            Box::pin(async { Ok(Replay::Missing) })
        }
        fn submit<'a>(
            &'a self,
            _: Insert<'a>,
        ) -> PortFuture<'a, Result<Submission, ReceiptFailure>> {
            Box::pin(async { panic!("resolver submitted a receipt") })
        }
        fn read<'a>(
            &'a self,
            tenant: &'a TenantId,
            result: &'a str,
            _: WallClockTimestamp,
            _: usize,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Retained, ReceiptFailure>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.trace
                .lock()
                .unwrap()
                .push(format!("{}:{result}", tenant.as_str()));
            let rows = self.rows.clone();
            Box::pin(async move {
                Ok(Retained {
                    total: rows.len(),
                    receipts: rows,
                })
            })
        }
    }
    struct NoEntities;
    impl Entities for NoEntities {
        fn groups(&self, _: &SearchSnapshot, _: u8) -> Vec<EntityGroup> {
            vec![]
        }
    }
    struct NoWorkflows;
    impl Resolver for NoWorkflows {
        fn resolve<'a>(
            &'a self,
            _: &'a TenantId,
            artifact: &'a ArtifactRef,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Resolved, Failure>> {
            let node = Node::missing(artifact.clone(), Status::Missing);
            Box::pin(async move {
                Ok(Resolved {
                    node,
                    edges: vec![],
                })
            })
        }
    }
    fn tenant(name: &str) -> TenantId {
        TenantId::new(name).unwrap()
    }
    fn resolver(policy: bool, receipts: Arc<Receipts>) -> NativeResolver {
        let snapshots = SnapshotService::new(
            Arc::new(EmptySnapshots),
            Arc::new(TestClock),
            Arc::new(Ids),
            tenant("tenant-a"),
            SnapshotTtl::new(60).unwrap(),
        );
        NativeResolver {
            snapshots,
            receipts,
            policy: Arc::new(CurrentPolicy(policy)),
            clock: Arc::new(TestClock),
            entities: Arc::new(NoEntities),
            workflows: Arc::new(NoWorkflows),
        }
    }
    fn run<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("fixture unexpectedly yielded"),
        }
    }
    fn receipt(source_engines: Value) -> Value {
        json!({"receipt_id":"receipt-1","observation":{"status":"succeeded"},"discovery":{"source_engines":source_engines}})
    }

    #[test]
    fn receipt_lineage_requires_current_grant_and_historical_engine_allowance() {
        let rows = vec![receipt(json!(["brave"]))];
        let trace = Arc::new(Mutex::new(vec![]));
        let repo = Arc::new(Receipts {
            calls: AtomicUsize::new(0),
            rows: rows.clone(),
            trace: trace.clone(),
        });
        let composite =
            ssx_domain::canonical::composite_artifact_id(&["result-1", "receipt-1"]).unwrap();
        let root = ArtifactRef::new(ArtifactKind::RetrievalReceipt, composite).unwrap();
        let denied = run(resolver(false, repo.clone()).resolve(
            &tenant("tenant-a"),
            &root,
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(denied.node.status, Status::PolicyDenied);
        // Frozen primary behavior: slopsearx/mcp/lineage_tools.py `_node`
        // emits this reason-only detail for policy denial; it does not expose
        // the receipt's observation status or any adjacency.
        assert_eq!(
            denied
                .node
                .details
                .as_ref()
                .unwrap()
                .get("reason")
                .and_then(Value::as_str),
            Some("current_policy_denied")
        );
        assert!(
            denied
                .node
                .details
                .as_ref()
                .unwrap()
                .get("status")
                .is_none()
        );
        assert!(denied.edges.is_empty());
        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);

        let allowed = run(resolver(true, repo.clone()).resolve(
            &tenant("tenant-a"),
            &root,
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(allowed.node.status, Status::Live);
        assert_eq!(
            allowed.node.details.as_ref().unwrap()["status"],
            "succeeded"
        );
        assert_eq!(allowed.edges.len(), 1);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 1);
        assert_eq!(*trace.lock().unwrap(), vec!["tenant-a:result-1"]);

        let denied_repo = Arc::new(Receipts {
            calls: AtomicUsize::new(0),
            rows: vec![receipt(json!(["unlisted-engine"]))],
            trace: Arc::new(Mutex::new(vec![])),
        });
        let denied = run(resolver(true, denied_repo.clone()).resolve(
            &tenant("tenant-a"),
            &root,
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(denied.node.status, Status::PolicyDenied);
        assert_eq!(
            denied
                .node
                .details
                .as_ref()
                .unwrap()
                .get("reason")
                .and_then(Value::as_str),
            Some("current_policy_denied")
        );
        assert!(
            denied
                .node
                .details
                .as_ref()
                .unwrap()
                .get("status")
                .is_none()
        );
        assert!(denied.edges.is_empty());
        assert_eq!(denied_repo.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn receipt_identity_is_tenant_scoped_and_wrong_receipt_id_is_missing() {
        let rows = vec![receipt(Value::Array(vec![]))];
        let repo = Arc::new(Receipts {
            calls: AtomicUsize::new(0),
            rows,
            trace: Arc::new(Mutex::new(vec![])),
        });
        let composite =
            ssx_domain::canonical::composite_artifact_id(&["result-1", "other-receipt"]).unwrap();
        let root = ArtifactRef::new(ArtifactKind::RetrievalReceipt, composite).unwrap();
        let value = run(resolver(true, repo.clone()).resolve(
            &tenant("tenant-b"),
            &root,
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(value.node.status, Status::Missing);
        assert!(value.edges.is_empty());
        assert_eq!(repo.calls.load(Ordering::SeqCst), 1);
        assert_eq!(*repo.trace.lock().unwrap(), vec!["tenant-b:result-1"]);
    }
    struct PublishingSnapshots {
        policy: Arc<Mutex<PolicyService>>,
        next: PolicyService,
        reads: AtomicUsize,
        snapshot: SearchSnapshot,
    }
    impl SnapshotRepository for PublishingSnapshots {
        fn create<'a>(
            &'a self,
            _: &'a SearchSnapshot,
            _: NonZeroU64,
        ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
            Box::pin(async { panic!("readonly source") })
        }
        fn read<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a SnapshotId,
        ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
            Box::pin(async move {
                if self.reads.fetch_add(1, Ordering::SeqCst) == 1 {
                    *self.policy.lock().unwrap() = self.next.clone();
                }
                Ok(Some(self.snapshot.clone()))
            })
        }
    }
    struct PublishedPolicy(Arc<Mutex<PolicyService>>);
    impl Policy for PublishedPolicy {
        fn current(&self) -> PolicyService {
            self.0.lock().unwrap().clone()
        }
    }
    struct CountingProjected(AtomicUsize);
    impl Entities for CountingProjected {
        fn groups(&self, _: &SearchSnapshot, _: u8) -> Vec<EntityGroup> {
            panic!("projected path must not rehydrate")
        }
        fn projected(&self) -> Option<&dyn ProjectedEntities> {
            Some(self)
        }
    }
    impl ProjectedEntities for CountingProjected {
        fn groups(
            &self,
            snapshot: &ValidatedSnapshotRead,
            version: u8,
        ) -> Result<Vec<EntityGroup>, Failure> {
            self.0.fetch_add(1, Ordering::SeqCst);
            snapshot.visit_results(&mut |_, _| {}).unwrap();
            Ok(if version == 2 {
                vec![EntityGroup {
                    id: "entity-v2-fixture".into(),
                    namespace: "fixture".into(),
                    result_indices: vec![0],
                }]
            } else {
                vec![]
            })
        }
    }
    #[test]
    fn entity_second_read_rechecks_live_policy_before_projected_visitation() {
        for mutation in ["absent", "grant", "engine", "sensitive"] {
            let mut initial = CurrentPolicy(mutation != "absent").current();
            initial.policy.sensitive_engines.insert("brave".into());
            initial.policy.sensitive_allowed = true;
            let mut next = initial.clone();
            match mutation {
                "grant" => {
                    next.policy.grants.remove("retrieval_receipts");
                }
                "engine" => {
                    let mut engines = next.catalog.engines().to_vec();
                    engines[0].enabled = false;
                    next.catalog = CatalogSnapshot::new(engines).unwrap();
                }
                "sensitive" => next.policy.sensitive_allowed = false,
                _ => {}
            }
            let policy = Arc::new(Mutex::new(initial));
            let mut scope = ssx_domain::ScopeDecision::default();
            scope.selected_engines = vec!["brave".into()];
            let snapshot = SearchSnapshot::new(
                SnapshotId::new("snapshot-a").unwrap(),
                "q".into(),
                ssx_domain::QueryId::new("query-a").unwrap(),
                vec![SearchResult::default()],
                scope,
                tenant("tenant-a"),
                WallClockTimestamp::new(50.).unwrap(),
                None,
            )
            .unwrap();
            let store = Arc::new(PublishingSnapshots {
                policy: policy.clone(),
                next,
                reads: AtomicUsize::new(0),
                snapshot,
            });
            let entities = Arc::new(CountingProjected(AtomicUsize::new(0)));
            let receipts = Arc::new(Receipts {
                calls: AtomicUsize::new(0),
                rows: vec![],
                trace: Arc::new(Mutex::new(vec![])),
            });
            let resolver = NativeResolver {
                snapshots: SnapshotService::new(
                    store.clone(),
                    Arc::new(TestClock),
                    Arc::new(Ids),
                    tenant("tenant-a"),
                    SnapshotTtl::new(60).unwrap(),
                ),
                receipts: receipts.clone(),
                policy: Arc::new(PublishedPolicy(policy)),
                clock: Arc::new(TestClock),
                entities: entities.clone(),
                workflows: Arc::new(NoWorkflows),
            };
            let id = composite_artifact_id(&["snapshot-a", "entity-v2-fixture"]).unwrap();
            let result = run(resolver.resolve(
                &tenant("tenant-a"),
                &ArtifactRef::new(ArtifactKind::EntityGroup, id).unwrap(),
                &Cancellation::default(),
            ))
            .unwrap();
            let allowed = matches!(mutation, "absent" | "grant");
            assert_eq!(
                result.node.status,
                if allowed {
                    Status::Live
                } else {
                    Status::PolicyDenied
                },
                "{mutation}"
            );
            assert_eq!(result.edges.len(), if allowed { 2 } else { 0 });
            assert_eq!(store.reads.load(Ordering::SeqCst), 2);
            assert_eq!(
                entities.0.load(Ordering::SeqCst),
                if allowed { 2 } else { 1 },
                "only authorized rows may be visited"
            );
            assert_eq!(receipts.calls.load(Ordering::SeqCst), 0);
        }
    }

    // Manual polling holds the actual repository future at an await boundary.
    // Policy changes only after the lookup is pending, before its rows return.
    #[derive(Default)]
    struct ReadGate {
        entered: std::sync::atomic::AtomicBool,
        released: std::sync::atomic::AtomicBool,
    }
    impl ReadGate {
        async fn wait(&self) {
            std::future::poll_fn(|_| {
                self.entered.store(true, Ordering::SeqCst);
                if self.released.load(Ordering::SeqCst) {
                    std::task::Poll::Ready(())
                } else {
                    std::task::Poll::Pending
                }
            })
            .await;
        }
    }
    struct HeldSnapshots {
        snapshot: SearchSnapshot,
        reads: AtomicUsize,
        hold_at: usize,
        gate: Arc<ReadGate>,
    }
    impl SnapshotRepository for HeldSnapshots {
        fn create<'a>(
            &'a self,
            _: &'a SearchSnapshot,
            _: NonZeroU64,
        ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
            Box::pin(async { panic!("readonly lineage created a snapshot") })
        }
        fn read<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a SnapshotId,
        ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
            Box::pin(async move {
                if self.reads.fetch_add(1, Ordering::SeqCst) == self.hold_at {
                    self.gate.wait().await;
                }
                Ok(Some(self.snapshot.clone()))
            })
        }
    }
    struct HeldReceipts {
        rows: Arc<Receipts>,
        gate: Arc<ReadGate>,
    }
    impl ReceiptRepository for HeldReceipts {
        fn replay<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a str,
            _: &'a str,
            _: &'a str,
            _: WallClockTimestamp,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Replay, ReceiptFailure>> {
            Box::pin(async { panic!("readonly lineage replayed a receipt") })
        }
        fn submit<'a>(
            &'a self,
            _: Insert<'a>,
        ) -> PortFuture<'a, Result<Submission, ReceiptFailure>> {
            Box::pin(async { panic!("readonly lineage submitted a receipt") })
        }
        fn read<'a>(
            &'a self,
            tenant: &'a TenantId,
            result: &'a str,
            now: WallClockTimestamp,
            limit: usize,
            cancel: &'a Cancellation,
        ) -> PortFuture<'a, Result<Retained, ReceiptFailure>> {
            Box::pin(async move {
                self.gate.wait().await;
                self.rows.read(tenant, result, now, limit, cancel).await
            })
        }
    }
    fn revoke(policy: &mut PolicyService, mutation: &str) {
        match mutation {
            "grant" => {
                policy.policy.grants.remove("retrieval_receipts");
            }
            "engine" => {
                let mut engines = policy.catalog.engines().to_vec();
                engines[0].enabled = false;
                policy.catalog = CatalogSnapshot::new(engines).unwrap();
            }
            "sensitive" => policy.policy.sensitive_allowed = false,
            _ => panic!("unknown policy mutation"),
        }
    }
    fn assert_denied(value: &Resolved) {
        assert_eq!(value.node.status, Status::PolicyDenied);
        assert!(value.node.created_at.is_none());
        assert!(value.node.expires_at.is_none());
        assert_eq!(
            value.node.details.as_ref().unwrap(),
            json!({"reason":"current_policy_denied"})
                .as_object()
                .unwrap()
        );
        assert!(value.edges.is_empty());
    }
    #[test]
    fn held_lineage_reads_revalidate_policy_before_publishing_metadata_or_edges() {
        for projected in [false, true] {
            for boundary in ["result_snapshot", "result_receipts", "receipt"] {
                for mutation in ["engine", "sensitive", "grant"] {
                    let mut initial = CurrentPolicy(true).current();
                    initial.policy.sensitive_engines.insert("brave".into());
                    initial.policy.sensitive_allowed = true;
                    let policy = Arc::new(Mutex::new(initial));
                    let gate = Arc::new(ReadGate::default());
                    let mut scope = ssx_domain::ScopeDecision::default();
                    scope.selected_engines = vec!["brave".into()];
                    let snapshots = Arc::new(HeldSnapshots {
                        snapshot: SearchSnapshot::new(
                            SnapshotId::new("snapshot-held").unwrap(),
                            "private query".into(),
                            ssx_domain::QueryId::new("query-held").unwrap(),
                            vec![SearchResult::default()],
                            scope,
                            tenant("tenant-a"),
                            WallClockTimestamp::new(50.).unwrap(),
                            None,
                        )
                        .unwrap(),
                        reads: AtomicUsize::new(0),
                        hold_at: if boundary == "result_snapshot" {
                            1
                        } else {
                            usize::MAX
                        },
                        gate: gate.clone(),
                    });
                    let rows = Arc::new(Receipts {
                        calls: AtomicUsize::new(0),
                        rows: vec![receipt(json!(["brave"]))],
                        trace: Arc::new(Mutex::new(vec![])),
                    });
                    let receipts: Arc<dyn ReceiptRepository> = if boundary == "result_snapshot" {
                        rows.clone()
                    } else {
                        Arc::new(HeldReceipts {
                            rows: rows.clone(),
                            gate: gate.clone(),
                        })
                    };
                    let resolver = NativeResolver {
                        snapshots: SnapshotService::new(
                            snapshots.clone(),
                            Arc::new(TestClock),
                            Arc::new(Ids),
                            tenant("tenant-a"),
                            SnapshotTtl::new(60).unwrap(),
                        ),
                        receipts,
                        policy: Arc::new(PublishedPolicy(policy.clone())),
                        clock: Arc::new(TestClock),
                        entities: if projected {
                            Arc::new(CountingProjected(AtomicUsize::new(0)))
                        } else {
                            Arc::new(NoEntities)
                        },
                        workflows: Arc::new(NoWorkflows),
                    };
                    let root = if boundary == "receipt" {
                        ArtifactRef::new(
                            ArtifactKind::RetrievalReceipt,
                            composite_artifact_id(&["snapshot-held:0", "receipt-1"]).unwrap(),
                        )
                        .unwrap()
                    } else {
                        ArtifactRef::new(ArtifactKind::Result, "snapshot-held:0").unwrap()
                    };
                    let tenant = tenant("tenant-a");
                    let cancel = Cancellation::default();
                    let mut operation = std::pin::pin!(resolver.resolve(&tenant, &root, &cancel));
                    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
                    assert!(operation.as_mut().poll(&mut context).is_pending());
                    assert!(gate.entered.load(Ordering::SeqCst));
                    revoke(&mut policy.lock().unwrap(), mutation);
                    assert!(operation.as_mut().poll(&mut context).is_pending());
                    gate.released.store(true, Ordering::SeqCst);
                    let value = run(operation).unwrap();
                    if mutation == "grant" && boundary != "receipt" {
                        assert_eq!(value.node.status, Status::Live);
                        assert_eq!(value.edges.len(), 1);
                        assert_eq!(value.edges[0].relation, LineageRelation::DerivedFrom);
                        assert_eq!(value.edges[0].to.kind(), ArtifactKind::Snapshot);
                    } else {
                        assert_denied(&value);
                    }
                    assert_eq!(
                        snapshots.reads.load(Ordering::SeqCst),
                        if boundary == "receipt" { 0 } else { 2 }
                    );
                    if boundary == "result_snapshot" && mutation != "grant" {
                        assert_eq!(rows.calls.load(Ordering::SeqCst), 0);
                    }
                    if boundary != "result_snapshot" {
                        assert_eq!(rows.calls.load(Ordering::SeqCst), 1);
                    }
                }
            }
        }
    }
}
