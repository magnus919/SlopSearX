//! Borrowed evidence component graph attached to the existing native MCP owner.
pub mod authority;
pub mod factory;
pub mod tools;
use authority::{EvidenceAuthority, LineagePolicyAdapter, RecordAuthorityAdapter};
use ssx_application::{
    PortFuture,
    lineage::{
        self, Resolver,
        resolver::{Entities, EntityGroup, NativeResolver, ProjectedEntities},
        workflows::{Records, WorkflowResolver},
    },
    receipts::{
        Cancellation, ReceiptRepository,
        manifest::Sources,
        service::{self, Access, Clock, ManifestSources, ResolvedManifestSource},
        source::Snapshots,
    },
    snapshot::{SnapshotService, ValidatedSnapshotRead},
};
use ssx_domain::{ArtifactRef, SearchSnapshot, TenantId, WallClockTimestamp};
use ssx_infra::{
    search::SearchClock,
    valkey::{
        Valkey,
        retrieval::{RandomReceiptIds, ValkeyReceipts, lineage::NativeRecords},
    },
};
use ssx_json::Value;
use std::sync::Arc;

pub struct EvidenceInputs {
    pub backend: Arc<Valkey>,
    pub snapshots: SnapshotService,
    pub authority: Arc<dyn EvidenceAuthority>,
    pub clock: Arc<dyn SearchClock>,
    pub metrics: Arc<dyn ssx_application::Metrics>,
}
pub struct EvidenceAccess {
    pub authority: Arc<dyn EvidenceAuthority>,
    pub backend: Arc<Valkey>,
}
impl Access for EvidenceAccess {
    fn enabled(&self) -> bool {
        self.authority
            .snapshot()
            .search
            .policy
            .grants
            .contains("retrieval_receipts")
    }
    fn connected(&self) -> bool {
        self.authority.snapshot().valkey_connected && !self.backend.is_closed()
    }
}
pub struct EvidenceClock {
    pub clock: Arc<dyn SearchClock>,
}
impl Clock for EvidenceClock {
    fn now(&self) -> WallClockTimestamp {
        self.clock.capture().0
    }
}
/// Both ports feed the exact same collector as read_entities. Projected rows
/// are visited in place; no SearchSnapshot materialization/JSON rehydration.
pub struct EvidenceEntities;
impl Entities for EvidenceEntities {
    fn groups(&self, s: &SearchSnapshot, version: u8) -> Vec<EntityGroup> {
        let mut c = crate::search_surface::entities::Collector::new(s.id().as_str(), version);
        for (i, r) in s.results().iter().enumerate() {
            c.feed(i, r);
        }
        c.lineage_groups()
    }
    fn projected(&self) -> Option<&dyn ProjectedEntities> {
        Some(self)
    }
}
impl ProjectedEntities for EvidenceEntities {
    fn groups(
        &self,
        s: &ValidatedSnapshotRead,
        version: u8,
    ) -> Result<Vec<EntityGroup>, lineage::Failure> {
        let mut c =
            crate::search_surface::entities::Collector::new(s.summary().id().as_str(), version);
        s.visit_results(&mut |i, r| c.feed(i, r))
            .map_err(|e| match e {
                ssx_application::snapshot::SnapshotFailure::Closed => lineage::Failure::Closed,
                ssx_application::snapshot::SnapshotFailure::Unavailable
                | ssx_application::snapshot::SnapshotFailure::CompletionUnknown => {
                    lineage::Failure::Unavailable
                }
                _ => lineage::Failure::Malformed,
            })?;
        Ok(c.lineage_groups())
    }
}
struct LiveWorkflows {
    records: Arc<dyn Records>,
    authority: Arc<dyn EvidenceAuthority>,
    policy: Arc<dyn lineage::resolver::Policy>,
    clock: Arc<dyn Clock>,
}
impl Resolver for LiveWorkflows {
    fn resolve<'a>(
        &'a self,
        tenant: &'a TenantId,
        root: &'a ArtifactRef,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<lineage::Resolved, lineage::Failure>> {
        Box::pin(async move {
            let s = self.authority.snapshot();
            WorkflowResolver {
                records: self.records.clone(),
                policy: self.policy.clone(),
                clock: self.clock.clone(),
                staged_limits: s.staged_limits,
                saved_max_reports: s.saved_limits.reports as usize,
            }
            .resolve(tenant, root, cancel)
            .await
        })
    }
}
struct LiveSources {
    records: Arc<dyn Records>,
    snapshots: SnapshotService,
    authority: Arc<dyn EvidenceAuthority>,
    policy: Arc<dyn lineage::resolver::Policy>,
    clock: Arc<dyn Clock>,
}
impl ManifestSources for LiveSources {
    fn resolve<'a>(
        &'a self,
        tenant: &'a TenantId,
        root: &'a ArtifactRef,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<ResolvedManifestSource, Value>> {
        Box::pin(async move {
            let s = self.authority.snapshot();
            Sources {
                records: self.records.clone(),
                snapshots: self.snapshots.clone(),
                policy: self.policy.clone(),
                clock: self.clock.clone(),
                staged_limits: s.staged_limits,
            }
            .resolve(tenant, root, cancel)
            .await
        })
    }
}
/// No backend close/shutdown capability: the native owner joins handlers before
/// calling its existing SearchService::shutdown exactly once.
pub struct EvidenceRuntime {
    pub service: service::Service,
    pub lineage: Arc<dyn Resolver>,
}
impl EvidenceRuntime {
    pub fn new(inputs: EvidenceInputs) -> Self {
        let repo: Arc<dyn ReceiptRepository> = Arc::new(ValkeyReceipts::new(
            inputs.backend.clone(),
            Arc::new(RandomReceiptIds),
        ));
        let records: Arc<dyn Records> = Arc::new(NativeRecords {
            backend: inputs.backend.clone(),
            authority: Arc::new(RecordAuthorityAdapter {
                authority: inputs.authority.clone(),
            }),
        });
        let policy: Arc<dyn lineage::resolver::Policy> = Arc::new(LineagePolicyAdapter {
            authority: inputs.authority.clone(),
        });
        let clock: Arc<dyn Clock> = Arc::new(EvidenceClock {
            clock: inputs.clock,
        });
        let workflows = Arc::new(LiveWorkflows {
            records: records.clone(),
            authority: inputs.authority.clone(),
            policy: policy.clone(),
            clock: clock.clone(),
        });
        let lineage: Arc<dyn Resolver> = Arc::new(NativeResolver {
            snapshots: inputs.snapshots.clone(),
            receipts: repo.clone(),
            policy: policy.clone(),
            clock: clock.clone(),
            entities: Arc::new(EvidenceEntities),
            workflows,
        });
        let sources = Arc::new(LiveSources {
            records,
            snapshots: inputs.snapshots.clone(),
            authority: inputs.authority.clone(),
            policy,
            clock: clock.clone(),
        });
        let access = Arc::new(EvidenceAccess {
            authority: inputs.authority,
            backend: inputs.backend,
        });
        let service = service::Service::new(
            repo,
            Arc::new(Snapshots(inputs.snapshots)),
            access,
            clock,
            lineage.clone(),
            sources,
            inputs.metrics,
        );
        Self { service, lineage }
    }
}
