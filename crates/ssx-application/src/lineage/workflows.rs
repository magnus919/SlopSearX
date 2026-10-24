//! Readonly retained workflow lineage. F07/F13/W01/W04/W06 own persistence and
//! lifecycle; this layer has no execution, cancellation-write or lease port.
use super::{Edge, Failure, Node, Resolved, Resolver, Status, input, resolver::Policy};
use crate::{
    PortFuture,
    receipts::{Cancellation, service::Clock},
    staged,
};
use ssx_domain::{
    ArtifactKind, ArtifactRef, LineageRelation, TenantId, WallClockTimestamp,
    canonical::composite_artifact_id, research::ResearchJob, saved::SavedDefinition,
    staged::Record,
};
use ssx_json::{Value, json};
use std::sync::Arc;
#[derive(Clone, Debug)]
pub enum Read<T> {
    Found(T),
    Missing,
    Expired(WallClockTimestamp),
    Unavailable,
    PolicyDenied,
}
pub trait Records: Send + Sync {
    fn research<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a str,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<ResearchJob>, Failure>>;
    fn staged<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a str,
        now: WallClockTimestamp,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<Record>, Failure>>;
    fn saved<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a str,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<SavedDefinition>, Failure>>;
    fn reports<'a>(
        &'a self,
        tenant: &'a TenantId,
        definition: &'a SavedDefinition,
        now: WallClockTimestamp,
        limit: usize,
        selected_run: Option<&'a str>,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<Vec<Value>>, Failure>>;
}
pub struct WorkflowResolver {
    pub records: Arc<dyn Records>,
    pub policy: Arc<dyn Policy>,
    pub clock: Arc<dyn Clock>,
    /// Trusted effective current operator limits, never supplied by a tool.
    pub staged_limits: staged::Limits,
    pub saved_max_reports: usize,
}
fn reference(kind: ArtifactKind, id: &str) -> Result<ArtifactRef, Failure> {
    ArtifactRef::new(kind, id).map_err(|_| Failure::Malformed)
}
fn edge(from: ArtifactRef, relation: LineageRelation, to: ArtifactRef) -> Edge {
    Edge { from, relation, to }
}
fn empty(root: &ArtifactRef, status: Status) -> Resolved {
    Resolved {
        node: Node::missing(root.clone(), status),
        edges: vec![],
    }
}
fn denied(root: &ArtifactRef) -> Resolved {
    Resolved {
        node: Node::policy_denied(root.clone()),
        edges: vec![],
    }
}
fn timestamp(n: f64) -> Result<WallClockTimestamp, Failure> {
    WallClockTimestamp::new(n).map_err(|_| Failure::Malformed)
}
fn missing<T>(read: Read<T>, root: &ArtifactRef) -> Result<T, Box<Resolved>> {
    match read {
        Read::Found(v) => Ok(v),
        Read::Missing => Err(Box::new(empty(root, Status::Missing))),
        Read::Unavailable => Err(Box::new(empty(root, Status::Unavailable))),
        Read::PolicyDenied => Err(Box::new(denied(root))),
        Read::Expired(at) => {
            let mut r = empty(root, Status::Expired);
            r.node.expires_at = Some(at);
            Err(Box::new(r))
        }
    }
}
fn inherited(root: &ArtifactRef, parent: &Node) -> Resolved {
    if parent.status == Status::PolicyDenied {
        empty(root, Status::PolicyDenied)
    } else {
        let mut r = empty(root, parent.status);
        r.node.expires_at = parent.expires_at;
        r
    }
}
fn details(
    root: &ArtifactRef,
    created: Option<WallClockTimestamp>,
    expires: Option<WallClockTimestamp>,
    details: Value,
    edges: Vec<Edge>,
) -> Resolved {
    Resolved {
        node: Node {
            artifact: root.clone(),
            status: Status::Live,
            created_at: created,
            expires_at: expires,
            details: Some(details.as_object().expect("fixed details object").clone()),
        },
        edges,
    }
}
impl WorkflowResolver {
    fn research_allowed(&self, job: &ResearchJob) -> bool {
        let policy = self.policy.current();
        policy.policy.grants.contains("research")
            && policy
                .retained_engine_rejection(
                    &job.queries
                        .iter()
                        .flat_map(|q| q.engines.clone())
                        .collect::<Vec<_>>(),
                    "engines",
                )
                .is_ok()
            && policy.validate_research_workflow(job).is_ok()
    }
    async fn job(
        &self,
        tenant: &TenantId,
        root: &ArtifactRef,
        cancel: &Cancellation,
        dossier: bool,
    ) -> Result<Resolved, Failure> {
        let policy = self.policy.current();
        if !policy.policy.grants.contains("research")
            || dossier
                && ["dependency_dossier", "security"]
                    .iter()
                    .any(|g| !policy.policy.grants.contains(*g))
        {
            return Ok(denied(root));
        }
        let job = match missing(
            self.records.research(tenant, root.id(), cancel).await?,
            root,
        ) {
            Ok(v) => v,
            Err(e) => return Ok(*e),
        };
        if job.tenant != tenant.as_str() || job.job_id != root.id() {
            return Err(Failure::Malformed);
        }
        if dossier && job.workflow.get("kind").and_then(Value::as_str) != Some("dependency_dossier")
        {
            return Ok(empty(root, Status::Missing));
        }
        if !self.research_allowed(&job) {
            return Ok(denied(root));
        }
        let mut edges = job
            .workflow
            .get("lineage")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Edge::from_value)
            .collect::<Vec<_>>();
        for query in &job.queries {
            for attempt in &query.attempts {
                if dossier {
                    if let Some(cursor) = &attempt.cursor {
                        edges.push(edge(
                            reference(ArtifactKind::Snapshot, cursor)?,
                            LineageRelation::DerivedFrom,
                            root.clone(),
                        ));
                    }
                } else {
                    let id = composite_artifact_id(&[&job.job_id, &attempt.attempt_id])
                        .map_err(|_| Failure::Malformed)?;
                    let attempt_ref = reference(ArtifactKind::ResearchAttempt, &id)?;
                    edges.push(edge(
                        attempt_ref.clone(),
                        LineageRelation::DerivedFrom,
                        root.clone(),
                    ));
                    if let Some(cursor) = &attempt.cursor {
                        edges.push(edge(
                            reference(ArtifactKind::Snapshot, cursor)?,
                            LineageRelation::DerivedFrom,
                            attempt_ref,
                        ));
                    }
                }
            }
        }
        Ok(details(
            root,
            Some(timestamp(job.created_at.get())?),
            Some(timestamp(job.created_at.get() + 86400.)?),
            json!({"state":job.state}),
            edges,
        ))
    }
    async fn attempt(
        &self,
        tenant: &TenantId,
        root: &ArtifactRef,
        cancel: &Cancellation,
    ) -> Result<Resolved, Failure> {
        let Ok(parts) = input::composite(root.id(), 2) else {
            return Ok(empty(root, Status::Missing));
        };
        let parent = reference(ArtifactKind::ResearchJob, &parts[0])?;
        let status = self.job(tenant, &parent, cancel, false).await?;
        if status.node.status != Status::Live {
            return Ok(inherited(root, &status.node));
        }
        let job = match missing(
            self.records.research(tenant, &parts[0], cancel).await?,
            root,
        ) {
            Ok(v) => v,
            Err(e) => return Ok(*e),
        };
        if job.tenant != tenant.as_str() || job.job_id != parts[0] {
            return Err(Failure::Malformed);
        }
        if !self.research_allowed(&job) {
            return Ok(denied(root));
        }
        let Some(attempt) = job
            .queries
            .iter()
            .flat_map(|q| &q.attempts)
            .find(|a| a.attempt_id == parts[1])
        else {
            return Ok(empty(root, Status::Missing));
        };
        let mut edges = vec![edge(root.clone(), LineageRelation::DerivedFrom, parent)];
        if let Some(cursor) = &attempt.cursor {
            edges.push(edge(
                reference(ArtifactKind::Snapshot, cursor)?,
                LineageRelation::DerivedFrom,
                root.clone(),
            ));
        }
        Ok(details(
            root,
            Some(timestamp(attempt.attempted_at.get())?),
            status.node.expires_at,
            json!({"state":attempt.state}),
            edges,
        ))
    }
    async fn staged(
        &self,
        tenant: &TenantId,
        root: &ArtifactRef,
        cancel: &Cancellation,
    ) -> Result<Resolved, Failure> {
        let policy = self.policy.current();
        if !policy.policy.grants.contains("staged_search") {
            return Ok(denied(root));
        }
        let record = match missing(
            self.records
                .staged(tenant, root.id(), self.clock.now(), cancel)
                .await?,
            root,
        ) {
            Ok(v) => v,
            Err(e) => return Ok(*e),
        };
        if record.tenant() != *tenant || record.id().as_str() != root.id() {
            return Err(Failure::Malformed);
        }
        let (policy, limits, _) = self.policy.current_with_limits(&self.staged_limits, 0);
        if !policy.policy.grants.contains("staged_search")
            || staged::revalidate(&policy, &limits, &record).is_err()
        {
            return Ok(denied(root));
        }
        let v = record.value();
        let mut edges = v
            .get("lineage")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Edge::from_value)
            .collect::<Vec<_>>();
        for stage in v["stages"].as_array().ok_or(Failure::Malformed)? {
            for attempt in stage["attempts"].as_array().ok_or(Failure::Malformed)? {
                if let Some(cursor) = attempt["cursor"].as_str().filter(|s| !s.is_empty()) {
                    let snapshot = reference(ArtifactKind::Snapshot, cursor)?;
                    edges.push(edge(
                        snapshot.clone(),
                        LineageRelation::DerivedFrom,
                        root.clone(),
                    ));
                    if attempt["attempt_id"] == v["selected_result_attempt_id"] {
                        edges.push(edge(root.clone(), LineageRelation::Selected, snapshot));
                    }
                }
            }
        }
        Ok(details(
            root,
            v["accepted_at"].as_f64().map(timestamp).transpose()?,
            Some(timestamp(record.expires_at())?),
            json!({"state":record.state()}),
            edges,
        ))
    }
    fn saved_read_allowed(&self, definition: &SavedDefinition, used_bound: usize) -> bool {
        let (policy, _, current_bound) = self
            .policy
            .current_with_limits(&self.staged_limits, self.saved_max_reports);
        policy.policy.grants.contains("saved_searches")
            && policy
                .retained_engine_rejection(&definition.engines, "engines")
                .is_ok()
            && current_bound.min(100) >= used_bound
    }
    async fn saved(
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
            .contains("saved_searches")
        {
            return Ok(denied(root));
        }
        let definition = match missing(self.records.saved(tenant, root.id(), cancel).await?, root) {
            Ok(v) => v,
            Err(e) => return Ok(*e),
        };
        if definition.tenant != tenant.as_str() || definition.search_id != root.id() {
            return Err(Failure::Malformed);
        }
        let (policy, _, reports_limit) = self
            .policy
            .current_with_limits(&self.staged_limits, self.saved_max_reports);
        if !policy.policy.grants.contains("saved_searches")
            || policy
                .retained_engine_rejection(&definition.engines, "engines")
                .is_err()
        {
            return Ok(denied(root));
        }
        if reports_limit == 0 {
            return Err(Failure::Malformed);
        }
        let reports = match missing(
            self.records
                .reports(
                    tenant,
                    &definition,
                    self.clock.now(),
                    reports_limit.min(100),
                    None,
                    cancel,
                )
                .await?,
            root,
        ) {
            Ok(v) => v,
            Err(e) => return Ok(*e),
        };
        if !self.saved_read_allowed(&definition, reports_limit.min(100)) {
            return Ok(denied(root));
        }
        let mut edges = vec![];
        for report in reports {
            if let Some(run) = report["run_id"].as_str().filter(|s| !s.is_empty()) {
                let id = composite_artifact_id(&[&definition.search_id, run])
                    .map_err(|_| Failure::Malformed)?;
                edges.push(edge(
                    reference(ArtifactKind::SavedReport, &id)?,
                    LineageRelation::DerivedFrom,
                    root.clone(),
                ));
            }
        }
        Ok(details(
            root,
            Some(timestamp(definition.created_at.get())?),
            Some(timestamp(definition.expires_at.get())?),
            json!({"paused":definition.paused}),
            edges,
        ))
    }
    async fn report(
        &self,
        tenant: &TenantId,
        root: &ArtifactRef,
        cancel: &Cancellation,
    ) -> Result<Resolved, Failure> {
        let Ok(parts) = input::composite(root.id(), 2) else {
            return Ok(empty(root, Status::Missing));
        };
        let parent = reference(ArtifactKind::SavedSearch, &parts[0])?;
        let status = self.saved(tenant, &parent, cancel).await?;
        if status.node.status != Status::Live {
            return Ok(inherited(root, &status.node));
        }
        let definition = match missing(self.records.saved(tenant, &parts[0], cancel).await?, root) {
            Ok(v) => v,
            Err(e) => return Ok(*e),
        };
        if definition.tenant != tenant.as_str() || definition.search_id != parts[0] {
            return Err(Failure::Malformed);
        }
        let (policy, _, reports_limit) = self
            .policy
            .current_with_limits(&self.staged_limits, self.saved_max_reports);
        if !policy.policy.grants.contains("saved_searches")
            || policy
                .retained_engine_rejection(&definition.engines, "engines")
                .is_err()
        {
            return Ok(denied(root));
        }
        if reports_limit == 0 {
            return Err(Failure::Malformed);
        }
        let reports = match missing(
            self.records
                .reports(
                    tenant,
                    &definition,
                    self.clock.now(),
                    reports_limit.min(100),
                    Some(&parts[1]),
                    cancel,
                )
                .await?,
            root,
        ) {
            Ok(v) => v,
            Err(e) => return Ok(*e),
        };
        if !self.saved_read_allowed(&definition, reports_limit.min(100)) {
            return Ok(denied(root));
        }
        let Some(report) = reports
            .into_iter()
            .find(|r| r["run_id"].as_str() == Some(&parts[1]))
        else {
            return Ok(empty(root, Status::Missing));
        };
        Ok(details(
            root,
            report["finished_at"].as_f64().map(timestamp).transpose()?,
            report["expires_at"].as_f64().map(timestamp).transpose()?,
            json!({"status":report["status"]}),
            vec![edge(root.clone(), LineageRelation::DerivedFrom, parent)],
        ))
    }
}
impl Resolver for WorkflowResolver {
    fn resolve<'a>(
        &'a self,
        tenant: &'a TenantId,
        root: &'a ArtifactRef,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Resolved, Failure>> {
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            let result = match root.kind() {
                ArtifactKind::ResearchJob => self.job(tenant, root, cancel, false).await,
                ArtifactKind::ResearchAttempt => self.attempt(tenant, root, cancel).await,
                ArtifactKind::SavedSearch => self.saved(tenant, root, cancel).await,
                ArtifactKind::SavedReport => self.report(tenant, root, cancel).await,
                ArtifactKind::StagedSearch => self.staged(tenant, root, cancel).await,
                ArtifactKind::DependencyDossier => self.job(tenant, root, cancel, true).await,
                _ => Err(Failure::Malformed),
            };
            if cancel.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            result
        })
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::{
        policy::{PolicyService, SearchPolicy},
        scope::{CatalogSnapshot, ScopeResolver},
    };
    use std::{
        collections::BTreeSet,
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct CurrentPolicy(bool);
    impl Policy for CurrentPolicy {
        fn current(&self) -> PolicyService {
            PolicyService {
                policy: SearchPolicy {
                    grants: if self.0 {
                        ["research".to_owned()].into_iter().collect()
                    } else {
                        BTreeSet::new()
                    },
                    ..Default::default()
                },
                catalog: CatalogSnapshot::new(vec![]).unwrap(),
                resolver: ScopeResolver::default(),
            }
        }
    }
    struct FixedClock;
    impl Clock for FixedClock {
        fn now(&self) -> WallClockTimestamp {
            WallClockTimestamp::new(100.).unwrap()
        }
    }
    enum RecordResult {
        ForeignTenant,
        Expired,
    }
    struct FixtureRecords {
        outcome: RecordResult,
        calls: AtomicUsize,
    }
    impl Records for FixtureRecords {
        fn research<'a>(
            &'a self,
            tenant: &'a TenantId,
            id: &'a str,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Read<ResearchJob>, Failure>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(tenant.as_str(), "tenant-a");
            let outcome = match self.outcome {
                RecordResult::Expired => Read::Expired(WallClockTimestamp::new(50.).unwrap()),
                RecordResult::ForeignTenant => {
                    let mut job = ResearchJob {
                        job_id: id.into(),
                        tenant: "tenant-b".into(),
                        ..Default::default()
                    };
                    job.workflow.insert("kind".into(), json!("research"));
                    Read::Found(job)
                }
            };
            Box::pin(async move { Ok(outcome) })
        }
        fn staged<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a str,
            _: WallClockTimestamp,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Read<Record>, Failure>> {
            Box::pin(async { panic!("unexpected staged lookup") })
        }
        fn saved<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a str,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Read<SavedDefinition>, Failure>> {
            Box::pin(async { panic!("unexpected saved lookup") })
        }
        fn reports<'a>(
            &'a self,
            _: &'a TenantId,
            _: &'a SavedDefinition,
            _: WallClockTimestamp,
            _: usize,
            _: Option<&'a str>,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Read<Vec<Value>>, Failure>> {
            Box::pin(async { panic!("unexpected report lookup") })
        }
    }
    fn tenant() -> TenantId {
        TenantId::new("tenant-a").unwrap()
    }
    fn root() -> ArtifactRef {
        ArtifactRef::new(ArtifactKind::ResearchJob, "job-1").unwrap()
    }
    fn run<F: std::future::Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("fixture unexpectedly yielded"),
        }
    }
    fn resolver(records: Arc<FixtureRecords>, grant: bool) -> WorkflowResolver {
        WorkflowResolver {
            records,
            policy: Arc::new(CurrentPolicy(grant)),
            clock: Arc::new(FixedClock),
            staged_limits: staged::Limits::default(),
            saved_max_reports: 10,
        }
    }

    #[test]
    fn current_research_grant_gates_lookup_and_cross_tenant_rows_fail_closed() {
        let records = Arc::new(FixtureRecords {
            outcome: RecordResult::ForeignTenant,
            calls: AtomicUsize::new(0),
        });
        let denied = run(resolver(records.clone(), false).resolve(
            &tenant(),
            &root(),
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(denied.node.status, Status::PolicyDenied);
        assert!(denied.edges.is_empty());
        assert_eq!(records.calls.load(Ordering::SeqCst), 0);

        assert_eq!(
            run(resolver(records.clone(), true).resolve(
                &tenant(),
                &root(),
                &Cancellation::default()
            )),
            Err(Failure::Malformed)
        );
        assert_eq!(records.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn expired_parent_record_reports_status_and_timestamp_without_additional_reads() {
        let records = Arc::new(FixtureRecords {
            outcome: RecordResult::Expired,
            calls: AtomicUsize::new(0),
        });
        let result = run(resolver(records.clone(), true).resolve(
            &tenant(),
            &root(),
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(result.node.status, Status::Expired);
        assert_eq!(result.node.expires_at.unwrap().seconds(), 50.);
        assert!(result.edges.is_empty());
        assert_eq!(records.calls.load(Ordering::SeqCst), 1);
    }
}
