//! Readonly workflow source resolution. This service owns no dispatch, create,
//! lease, TTL-renewal or provider port. Lineage nodes are not source payloads.
use crate::{
    lineage::{
        self,
        resolver::Entities,
        workflows::{Read, Records},
    },
    policy::PolicyService,
    receipts::{Cancellation, service::Clock},
    snapshot::{SnapshotLookup, SnapshotService},
    staged,
};
use ssx_domain::{
    ArtifactKind, ArtifactRef, SearchSnapshot, SnapshotId, TenantId,
    canonical::parse_composite_artifact_id, staged::Record,
};
use ssx_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};
pub const MAX_SOURCE_RESULTS: usize = 25;
pub const MAX_SOURCE_CURSORS: usize = 100;
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
    pub details: Value,
}
impl Error {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: json!({}),
        }
    }
    pub fn value(&self) -> Value {
        json!({"error":{"code":self.code,"message":self.message,"details":self.details}})
    }
}
fn malformed() -> Error {
    Error::new("source_malformed", "source artifact is malformed")
}
fn denied() -> Error {
    Error::new(
        "source_policy_denied",
        "current authority denies the source artifact",
    )
}
fn missing() -> Error {
    Error::new("source_not_found", "source artifact was not found")
}
fn partial() -> Error {
    Error::new(
        "source_partial",
        "source artifact has no captured result snapshot",
    )
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Research,
    DependencyDossier,
    StagedSearch,
    ResearchManifest,
}
impl Destination {
    pub fn allows(self, kind: ArtifactKind) -> bool {
        match self {
            Self::Research => matches!(
                kind,
                ArtifactKind::Snapshot | ArtifactKind::SavedReport | ArtifactKind::StagedSearch
            ),
            Self::DependencyDossier => matches!(
                kind,
                ArtifactKind::Snapshot | ArtifactKind::Result | ArtifactKind::EntityGroup
            ),
            Self::StagedSearch => kind == ArtifactKind::SavedReport,
            Self::ResearchManifest => matches!(
                kind,
                ArtifactKind::StagedSearch
                    | ArtifactKind::ResearchAttempt
                    | ArtifactKind::ResearchJob
            ),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceStatus {
    Live,
    Partial,
    Truncated,
}
impl SourceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Partial => "partial",
            Self::Truncated => "truncated",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedSource {
    pub artifact: ArtifactRef,
    pub status: SourceStatus,
    pub result_ids: Vec<String>,
    pub query: Option<String>,
    pub details: Value,
}
impl ResolvedSource {
    pub fn stored(&self) -> Value {
        json!({"artifact":self.artifact.to_value(),"status":self.status.as_str(),"result_ids":self.result_ids,"query":self.query,"details":self.details})
    }
}
/// Trusted current request principal and current operator policy, sampled together.
/// Implementations must not derive this from a retained artifact or tool argument.
pub struct AuthoritySnapshot {
    pub tenant: Option<TenantId>,
    pub policy: PolicyService,
    pub staged_limits: staged::Limits,
    pub saved_max_reports: usize,
}
pub trait Authority: Send + Sync {
    fn current(&self) -> AuthoritySnapshot;
}
pub struct SourceResolver {
    pub snapshots: SnapshotService,
    pub records: Arc<dyn Records>,
    pub entities: Arc<dyn Entities>,
    pub authority: Arc<dyn Authority>,
    pub clock: Arc<dyn Clock>,
}
#[derive(Default)]
struct Fence {
    engines: BTreeSet<String>,
    grants: BTreeSet<&'static str>,
    reports_bound: usize,
    staged: Option<Record>,
}
impl SourceResolver {
    fn check(
        &self,
        tenant: &TenantId,
        cancel: &Cancellation,
        fence: &Fence,
    ) -> Result<AuthoritySnapshot, Error> {
        if cancel.is_cancelled() {
            return Err(Error::new("cancelled", "source resolution was cancelled"));
        }
        let current = self.authority.current();
        if current.tenant.as_ref() != Some(tenant)
            || fence
                .grants
                .iter()
                .any(|g| !current.policy.policy.grants.contains(*g))
            || current
                .policy
                .retained_engine_rejection(
                    &fence.engines.iter().cloned().collect::<Vec<_>>(),
                    "engines",
                )
                .is_err()
            || (fence.reports_bound > 0 && current.saved_max_reports.min(100) < fence.reports_bound)
            || fence.staged.as_ref().is_some_and(|r| {
                staged::revalidate(&current.policy, &current.staged_limits, r).is_err()
            })
        {
            return Err(denied());
        }
        Ok(current)
    }
    fn read<T>(&self, read: Result<Read<T>, lineage::Failure>) -> Result<T, Error> {
        match read {
            Ok(Read::Found(v)) => Ok(v),
            Ok(Read::Missing) => Err(missing()),
            Ok(Read::Expired(at)) => {
                let mut e = Error::new("source_expired", "source artifact has expired");
                e.details = json!({"expires_at":at.seconds()});
                Err(e)
            }
            Ok(Read::Unavailable)
            | Err(lineage::Failure::Unavailable | lineage::Failure::Closed) => Err(Error::new(
                "source_unavailable",
                "source artifact store is unavailable",
            )),
            Ok(Read::PolicyDenied) => Err(denied()),
            Err(lineage::Failure::Cancelled) => {
                Err(Error::new("cancelled", "source resolution was cancelled"))
            }
            _ => Err(malformed()),
        }
    }
    async fn snapshot(
        &self,
        tenant: &TenantId,
        id: &str,
        cancel: &Cancellation,
        fence: &mut Fence,
    ) -> Result<Arc<SearchSnapshot>, Error> {
        self.check(tenant, cancel, fence)?;
        let id = SnapshotId::new(id).map_err(|_| missing())?;
        let read = self.snapshots.for_tenant(tenant.clone()).read(&id).await;
        self.check(tenant, cancel, fence)?;
        let snapshot = match read {
            SnapshotLookup::Found(v) => v,
            SnapshotLookup::Missing => return Err(missing()),
            SnapshotLookup::Unavailable => {
                return Err(Error::new(
                    "source_unavailable",
                    "source artifact store is unavailable",
                ));
            }
            SnapshotLookup::Expired { expires_at } => {
                let mut e = Error::new("source_expired", "source artifact has expired");
                e.details = json!({"expires_at":expires_at.timestamp().seconds()});
                return Err(e);
            }
        };
        if snapshot.tenant() != tenant || snapshot.id() != &id {
            return Err(malformed());
        }
        fence
            .engines
            .extend(snapshot.scope().selected_engines.clone());
        self.check(tenant, cancel, fence)?;
        Ok(snapshot)
    }
    fn selected(
        snapshot: &SearchSnapshot,
        artifact: &ArtifactRef,
        indices: Vec<usize>,
        details: Value,
    ) -> Result<ResolvedSource, Error> {
        if indices.iter().any(|i| *i >= snapshot.results().len()) {
            return Err(malformed());
        }
        let count = indices.len();
        Ok(ResolvedSource {
            artifact: artifact.clone(),
            status: if count > MAX_SOURCE_RESULTS {
                SourceStatus::Truncated
            } else {
                SourceStatus::Live
            },
            result_ids: indices
                .into_iter()
                .take(MAX_SOURCE_RESULTS)
                .map(|i| format!("{}:{i}", snapshot.id()))
                .collect(),
            query: Some(snapshot.query().into()),
            details,
        })
    }
    async fn snapshot_source(
        &self,
        tenant: &TenantId,
        artifact: &ArtifactRef,
        cancel: &Cancellation,
        fence: &mut Fence,
    ) -> Result<ResolvedSource, Error> {
        let snapshot = self.snapshot(tenant, artifact.id(), cancel, fence).await?;
        let count = snapshot.results().len();
        Self::selected(
            &snapshot,
            artifact,
            (0..count).collect(),
            json!({"query_id":snapshot.query_id().as_str(),"result_count":count,"included_result_count":count.min(MAX_SOURCE_RESULTS)}),
        )
    }
    async fn result_source(
        &self,
        tenant: &TenantId,
        artifact: &ArtifactRef,
        cancel: &Cancellation,
        fence: &mut Fence,
    ) -> Result<ResolvedSource, Error> {
        let (id, raw) = artifact.id().rsplit_once(':').ok_or_else(missing)?;
        let index = raw
            .parse::<usize>()
            .ok()
            .filter(|i| i.to_string() == raw)
            .ok_or_else(missing)?;
        let snapshot = self.snapshot(tenant, id, cancel, fence).await?;
        if index >= snapshot.results().len() {
            return Err(missing());
        }
        Self::selected(
            &snapshot,
            artifact,
            vec![index],
            json!({"query_id":snapshot.query_id().as_str(),"result_index":index}),
        )
    }
    async fn entity_source(
        &self,
        tenant: &TenantId,
        artifact: &ArtifactRef,
        cancel: &Cancellation,
        fence: &mut Fence,
    ) -> Result<ResolvedSource, Error> {
        let parts = parse_composite_artifact_id(artifact.id(), 2).map_err(|_| missing())?;
        let snapshot = self.snapshot(tenant, &parts[0], cancel, fence).await?;
        let groups = self.entities.groups(
            &snapshot,
            if parts[1].starts_with("entity-v2-") {
                2
            } else {
                1
            },
        );
        let group = groups
            .into_iter()
            .find(|g| g.id == parts[1])
            .ok_or_else(missing)?;
        let count = group.result_indices.len();
        Self::selected(
            &snapshot,
            artifact,
            group.result_indices,
            json!({"entity_id":group.id,"member_count":count,"included_result_count":count.min(MAX_SOURCE_RESULTS)}),
        )
    }
    async fn saved_source(
        &self,
        tenant: &TenantId,
        artifact: &ArtifactRef,
        cancel: &Cancellation,
        fence: &mut Fence,
    ) -> Result<ResolvedSource, Error> {
        fence.grants.insert("saved_searches");
        self.check(tenant, cancel, fence)?;
        let parts = parse_composite_artifact_id(artifact.id(), 2).map_err(|_| missing())?;
        let read = self.records.saved(tenant, &parts[0], cancel).await;
        self.check(tenant, cancel, fence)?;
        let definition = self.read(read)?;
        if definition.tenant != tenant.as_str() || definition.search_id != parts[0] {
            return Err(malformed());
        }
        fence.engines.extend(definition.engines.clone());
        let current = self.check(tenant, cancel, fence)?;
        let bound = current.saved_max_reports.min(100);
        if bound == 0 {
            return Err(denied());
        }
        fence.reports_bound = bound;
        let read = self
            .records
            .reports(
                tenant,
                &definition,
                self.clock.now(),
                bound,
                Some(&parts[1]),
                cancel,
            )
            .await;
        self.check(tenant, cancel, fence)?;
        let reports = self.read(read)?;
        let report = reports
            .into_iter()
            .find(|r| r["run_id"].as_str() == Some(&parts[1]))
            .ok_or_else(missing)?;
        if !report.is_object() || report["status"].as_str().is_none_or(|s| s.is_empty()) {
            return Err(malformed());
        }
        let reasons = match report.get("incomparable_reasons") {
            None => vec![],
            Some(value) => value
                .as_array()
                .filter(|a| a.iter().all(|r| r.as_str().is_some()))
                .cloned()
                .ok_or_else(malformed)?,
        };
        if reasons
            .iter()
            .any(|r| r == "observation_truncated" || r == "report_size_exceeded")
        {
            return Err(Error::new(
                "source_truncated",
                "source saved-search report is truncated",
            ));
        }
        if report["status"] == "incomparable" || !reasons.is_empty() {
            return Err(Error::new(
                "source_incomparable",
                "source saved-search report is incomparable",
            ));
        }
        let query = report["observation"]["window"]["query"]
            .as_str()
            .filter(|q| !q.trim().is_empty())
            .ok_or_else(|| {
                Error::new(
                    "source_incomparable",
                    "source saved-search report lacks a complete observation window",
                )
            })?;
        let engines = report["observation"]["window"]["engines"]
            .as_array()
            .filter(|a| !a.is_empty())
            .ok_or_else(|| {
                Error::new(
                    "source_incomparable",
                    "source saved-search report lacks a complete observation window",
                )
            })?;
        for engine in engines {
            fence
                .engines
                .insert(engine.as_str().ok_or_else(malformed)?.to_owned());
        }
        self.check(tenant, cancel, fence)?;
        Ok(ResolvedSource {
            artifact: artifact.clone(),
            status: SourceStatus::Live,
            result_ids: vec![],
            query: Some(query.into()),
            details: json!({"search_id":parts[0],"run_id":parts[1],"report_status":report["status"],"revision":report["revision"]}),
        })
    }
    async fn staged_source(
        &self,
        tenant: &TenantId,
        artifact: &ArtifactRef,
        cancel: &Cancellation,
        fence: &mut Fence,
    ) -> Result<ResolvedSource, Error> {
        fence.grants.insert("staged_search");
        self.check(tenant, cancel, fence)?;
        let read = self
            .records
            .staged(tenant, artifact.id(), self.clock.now(), cancel)
            .await;
        self.check(tenant, cancel, fence)?;
        let record = self.read(read)?;
        if record.tenant() != *tenant || record.id().as_str() != artifact.id() {
            return Err(malformed());
        }
        fence.staged = Some(record.clone());
        self.check(tenant, cancel, fence)?;
        let v = record.value();
        let selected = v["selected_result_attempt_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(partial)?;
        let attempt = v["stages"]
            .as_array()
            .ok_or_else(malformed)?
            .iter()
            .filter_map(|s| s["attempts"].as_array())
            .flatten()
            .find(|a| a["attempt_id"].as_str() == Some(selected))
            .ok_or_else(partial)?;
        let cursor = attempt["cursor"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(partial)?;
        let snapshot_ref =
            ArtifactRef::new(ArtifactKind::Snapshot, cursor).map_err(|_| malformed())?;
        let mut resolved = self
            .snapshot_source(tenant, &snapshot_ref, cancel, fence)
            .await?;
        resolved.artifact = artifact.clone();
        resolved.details["operation_id"] = json!(artifact.id());
        resolved.details["selected_attempt_id"] = json!(selected);
        resolved.details["selected_snapshot"] = json!(cursor);
        Ok(resolved)
    }
    async fn research_source(
        &self,
        tenant: &TenantId,
        artifact: &ArtifactRef,
        cancel: &Cancellation,
        fence: &mut Fence,
    ) -> Result<ResolvedSource, Error> {
        fence.grants.insert("research");
        self.check(tenant, cancel, fence)?;
        let (job_id, attempt_id) = if artifact.kind() == ArtifactKind::ResearchAttempt {
            let p = parse_composite_artifact_id(artifact.id(), 2).map_err(|_| missing())?;
            (p[0].clone(), Some(p[1].clone()))
        } else {
            (artifact.id().into(), None)
        };
        let read = self.records.research(tenant, &job_id, cancel).await;
        self.check(tenant, cancel, fence)?;
        let job = self.read(read)?;
        if job.tenant != tenant.as_str() || job.job_id != job_id {
            return Err(malformed());
        }
        if job.workflow.get("kind") == Some(&json!("dependency_dossier")) {
            return Err(missing());
        }
        fence
            .engines
            .extend(job.queries.iter().flat_map(|q| q.engines.clone()));
        self.check(tenant, cancel, fence)?;
        let attempts = job
            .queries
            .iter()
            .flat_map(|q| &q.attempts)
            .filter(|a| attempt_id.as_ref().is_none_or(|id| id == &a.attempt_id))
            .collect::<Vec<_>>();
        if attempt_id.is_some() && attempts.is_empty() {
            return Err(missing());
        }
        let cursors = attempts
            .iter()
            .filter_map(|a| a.cursor.as_deref())
            .collect::<Vec<_>>();
        if cursors.is_empty() {
            return Err(partial());
        }
        if cursors.len() > MAX_SOURCE_CURSORS {
            return Err(Error::new(
                "source_truncated",
                "source research artifact exceeds the cursor bound",
            ));
        }
        let mut ids = vec![];
        let mut seen = BTreeSet::new();
        let mut total = 0;
        for cursor in cursors {
            let snapshot = self.snapshot(tenant, cursor, cancel, fence).await?;
            total += snapshot.results().len();
            for i in 0..snapshot.results().len().min(MAX_SOURCE_RESULTS) {
                let id = format!("{}:{i}", snapshot.id());
                if seen.insert(id.clone()) && ids.len() < MAX_SOURCE_RESULTS {
                    ids.push(id);
                }
            }
        }
        let count = ids.len();
        Ok(ResolvedSource {
            artifact: artifact.clone(),
            status: if total > count {
                SourceStatus::Truncated
            } else if job.state == "succeeded" {
                SourceStatus::Live
            } else {
                SourceStatus::Partial
            },
            result_ids: ids,
            query: Some(job.question),
            details: json!({"job_id":job.job_id,"job_state":job.state,"result_count":total,"included_result_count":count}),
        })
    }
    pub async fn resolve(
        &self,
        tenant: &TenantId,
        artifact: &ArtifactRef,
        destination: Destination,
        cancel: &Cancellation,
    ) -> Result<ResolvedSource, Error> {
        let mut fence = Fence::default();
        self.check(tenant, cancel, &fence)?;
        if !destination.allows(artifact.kind()) {
            return Err(Error::new(
                "unsupported_transition",
                "source kind cannot be used for this destination",
            ));
        }
        let resolved = match artifact.kind() {
            ArtifactKind::Snapshot => {
                self.snapshot_source(tenant, artifact, cancel, &mut fence)
                    .await
            }
            ArtifactKind::Result => {
                self.result_source(tenant, artifact, cancel, &mut fence)
                    .await
            }
            ArtifactKind::EntityGroup => {
                self.entity_source(tenant, artifact, cancel, &mut fence)
                    .await
            }
            ArtifactKind::SavedReport => {
                self.saved_source(tenant, artifact, cancel, &mut fence)
                    .await
            }
            ArtifactKind::StagedSearch => {
                self.staged_source(tenant, artifact, cancel, &mut fence)
                    .await
            }
            ArtifactKind::ResearchJob | ArtifactKind::ResearchAttempt => {
                self.research_source(tenant, artifact, cancel, &mut fence)
                    .await
            }
            _ => Err(malformed()),
        }?;
        self.check(tenant, cancel, &fence)?;
        if destination == Destination::Research && resolved.status == SourceStatus::Truncated {
            return Err(Error::new(
                "source_truncated",
                "source artifact exceeds the research composition evidence bound",
            ));
        }
        Ok(resolved)
    }
}
