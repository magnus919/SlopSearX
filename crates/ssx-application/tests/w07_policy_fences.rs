//! Final awaited reads cannot publish artifacts under superseded authority.
use ssx_application::{
    PortFuture,
    lineage::{
        Failure, Resolver, Status,
        resolver::Policy,
        workflows::{Read, Records, WorkflowResolver},
    },
    policy::{PolicyService, SearchPolicy},
    receipts::{
        Cancellation,
        manifest::Sources,
        service::{Clock, ManifestSources},
    },
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
    snapshot::*,
    staged,
};
use ssx_domain::{
    ArtifactKind, ArtifactRef, QueryId, SearchSnapshot, SnapshotId, StagedSearchId, TenantId,
    WallClockExpiry, WallClockTimestamp,
    canonical::composite_artifact_id,
    research::{ResearchJob, ResearchQuery, ResearchQueryAttempt},
    saved::SavedDefinition,
    staged::Record,
};
use ssx_json::{Value, json};
use std::{
    collections::BTreeMap,
    future::{Future, poll_fn},
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
};

// 0 retains authority; 1 revokes the workflow grant; 2 disables the retained
// engine; 3 narrows the coherent report/staged operator bound.
struct Current(AtomicUsize);
impl Policy for Current {
    fn current(&self) -> PolicyService {
        let state = self.0.load(Ordering::SeqCst);
        PolicyService {
            policy: SearchPolicy {
                grants: if state == 1 {
                    Default::default()
                } else {
                    ["saved_searches", "research", "staged_search"]
                        .map(str::to_owned)
                        .into_iter()
                        .collect()
                },
                ..Default::default()
            },
            catalog: CatalogSnapshot::new(vec![EngineCapability {
                name: "brave".into(),
                enabled: state != 2,
                active: true,
                available: true,
                categories: vec!["general".into()],
                media_types: vec![],
                auth_ready: true,
                circuit_open: false,
                cost_class: "free".into(),
                health_status: "healthy".into(),
                health_stale: false,
                enforced_filters: BTreeMap::new(),
            }])
            .unwrap(),
            resolver: ScopeResolver::default(),
        }
    }
    fn current_with_limits(
        &self,
        _: &staged::Limits,
        _: usize,
    ) -> (PolicyService, staged::Limits, usize) {
        let narrowed = self.0.load(Ordering::SeqCst) == 3;
        (
            self.current(),
            staged::Limits {
                granted: true,
                deadline_ms: if narrowed { 1 } else { 30000 },
                engine_calls: 64,
            },
            if narrowed { 1 } else { 2 },
        )
    }
}
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> WallClockTimestamp {
        at(100.)
    }
}
impl SnapshotClock for FixedClock {
    fn now(&self) -> WallClockTimestamp {
        at(100.)
    }
}
fn at(n: f64) -> WallClockTimestamp {
    WallClockTimestamp::new(n).unwrap()
}
fn tenant() -> TenantId {
    TenantId::new("tenant-a").unwrap()
}
struct Gate {
    calls: AtomicUsize,
    hold_at: usize,
    released: AtomicBool,
}
impl Gate {
    fn new(hold_at: usize) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            hold_at,
            released: AtomicBool::new(false),
        }
    }
    async fn reply(&self) {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        poll_fn(|_| {
            if call == self.hold_at && !self.released.load(Ordering::SeqCst) {
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
    }
}
fn definition() -> SavedDefinition {
    ssx_json::from_value(json!({"search_id":"saved-1","tenant":"tenant-a","query":"q","engines":["brave"],
        "interval_seconds":60,"retention_seconds":600,"max_results":25,"max_reports":2,
        "created_at":1,"expires_at":200,"next_due":150,"revision":1,"paused":false,"policy_fingerprint":"fixture",
        "last_slot":0,"latest_run_id":"new","baseline":null,"lease_token":null,"lease_expires_at":0,"deleted":false})).unwrap()
}
struct WorkflowRecords {
    reports: Gate,
    failure: bool,
}
impl Records for WorkflowRecords {
    fn saved<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a str,
        _: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<SavedDefinition>, Failure>> {
        Box::pin(async { Ok(Read::Found(definition())) })
    }
    fn reports<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a SavedDefinition,
        _: WallClockTimestamp,
        limit: usize,
        selected: Option<&'a str>,
        _: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<Vec<Value>>, Failure>> {
        Box::pin(async move {
            assert_eq!(limit, 2);
            assert!(selected.is_none() || selected == Some("old"));
            self.reports.reply().await;
            if self.failure {
                return Err(Failure::Malformed);
            }
            Ok(Read::Found(vec![
                json!({"run_id":"new"}),
                json!({"run_id":"old","finished_at":80,"expires_at":200,"status":"incomparable"}),
            ]))
        })
    }
    fn research<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a str,
        _: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<ResearchJob>, Failure>> {
        Box::pin(async {
            Ok(Read::Found(ResearchJob {
                job_id: "job-1".into(),
                tenant: tenant().as_str().into(),
                question: "private question".into(),
                state: "succeeded".into(),
                queries: vec![ResearchQuery {
                    engines: vec!["brave".into()],
                    attempts: ["snap-first", "snap-last"]
                        .into_iter()
                        .enumerate()
                        .map(|(i, id)| ResearchQueryAttempt {
                            attempt_id: format!("attempt-{i}"),
                            cursor: Some(id.into()),
                            ..Default::default()
                        })
                        .collect(),
                    ..Default::default()
                }],
                ..Default::default()
            }))
        })
    }
    fn staged<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a str,
        _: WallClockTimestamp,
        _: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<Record>, Failure>> {
        Box::pin(async {
            let policy = Current(AtomicUsize::new(0));
            let limits = staged::Limits {
                granted: true,
                ..Default::default()
            };
            let plan = staged::Plan::build(
                &policy.current(),
                &limits,
                &staged::Input {
                    query: "private question".into(),
                    objectives: json!({"deadline_ms":1000,"max_engine_calls":1}),
                    initial_scope: json!({"engines":["brave"]}),
                    fallback_scope: None,
                    allow_scope_expansion: false,
                    filters: None,
                },
            )
            .unwrap();
            let record = plan
                .record(&tenant(), &StagedSearchId::new("stage-1").unwrap(), 1.)
                .unwrap();
            let mut value = record.value().clone();
            value["selected_result_attempt_id"] = json!("attempt-0");
            value["state"] = json!("completed");
            value["stages"][0]["state"] = json!("done");
            value["budget"]["reserved"] = json!(1);
            value["budget"]["observed"] = json!(1);
            let enforcement = value["stages"][0]["enforcement"].clone();
            value["stages"][0]["attempts"] = json!([{"attempt_id":"attempt-0","cursor":"snap-last","state":"done",
                "started_at":1,"finished_at":2,"reserved_engine_calls":1,"observed_engine_calls":1,
                "engine_outcomes":[],"enforcement":enforcement}]);
            Ok(Read::Found(Record::from_value(value).unwrap()))
        })
    }
}
fn saved_case(report: bool, mutation: usize, failure: bool, cancelled: bool) {
    let records = Arc::new(WorkflowRecords {
        reports: Gate::new(if report { 2 } else { 1 }),
        failure,
    });
    // For a report error precedence case, the parent enumeration must succeed.
    assert!(!report || !failure);
    let policy = Arc::new(Current(AtomicUsize::new(0)));
    let resolver = WorkflowResolver {
        records: records.clone(),
        policy: policy.clone(),
        clock: Arc::new(FixedClock),
        staged_limits: staged::Limits::default(),
        saved_max_reports: 2,
    };
    let root = ArtifactRef::new(
        if report {
            ArtifactKind::SavedReport
        } else {
            ArtifactKind::SavedSearch
        },
        if report {
            composite_artifact_id(&["saved-1", "old"]).unwrap()
        } else {
            "saved-1".into()
        },
    )
    .unwrap();
    let tenant = tenant();
    let cancel = Cancellation::default();
    let mut operation = resolver.resolve(&tenant, &root, &cancel);
    let mut cx = Context::from_waker(Waker::noop());
    assert!(
        operation.as_mut().poll(&mut cx).is_pending(),
        "final reports reply was not held"
    );
    assert_eq!(
        records.reports.calls.load(Ordering::SeqCst),
        if report { 2 } else { 1 }
    );
    policy.0.store(mutation, Ordering::SeqCst);
    if cancelled {
        cancel.cancel();
    }
    assert!(
        operation.as_mut().poll(&mut cx).is_pending(),
        "read escaped held reply"
    );
    records.reports.released.store(true, Ordering::SeqCst);
    let Poll::Ready(result) = operation.as_mut().poll(&mut cx) else {
        panic!("released reply did not complete")
    };
    if cancelled {
        assert_eq!(result, Err(Failure::Cancelled));
    } else if failure {
        assert_eq!(result, Err(Failure::Malformed));
    } else if mutation == 0 {
        let result = result.unwrap();
        assert_eq!(result.node.status, Status::Live);
        assert_eq!(result.edges.len(), if report { 1 } else { 2 });
        assert!(result.node.created_at.is_some());
    } else {
        let result = result.unwrap();
        assert_eq!(result.node.status, Status::PolicyDenied);
        assert!(result.edges.is_empty());
        assert!(result.node.created_at.is_none());
        assert!(result.node.expires_at.is_none());
        assert_eq!(
            result.node.details.unwrap(),
            json!({"reason":"current_policy_denied"})
                .as_object()
                .unwrap()
                .clone()
        );
    }
}
#[test]
fn saved_search_final_reply_rechecks_grant() {
    saved_case(false, 1, false, false);
}
#[test]
fn saved_search_final_reply_rechecks_engine() {
    saved_case(false, 2, false, false);
}
#[test]
fn saved_search_final_reply_rechecks_bound() {
    saved_case(false, 3, false, false);
}
#[test]
fn saved_report_final_reply_rechecks_grant() {
    saved_case(true, 1, false, false);
}
#[test]
fn saved_report_final_reply_rechecks_engine() {
    saved_case(true, 2, false, false);
}
#[test]
fn saved_report_final_reply_rejects_bound_shrink_without_reselecting_old_run() {
    saved_case(true, 3, false, false);
}
#[test]
fn unchanged_saved_authority_preserves_both_kinds() {
    saved_case(false, 0, false, false);
    saved_case(true, 0, false, false);
}
#[test]
fn saved_final_reply_preserves_error_and_cancellation_precedence() {
    saved_case(false, 1, true, false);
    saved_case(false, 1, false, true);
}

struct Snapshots(Gate);
impl SnapshotRepository for Snapshots {
    fn create<'a>(
        &'a self,
        _: &'a SearchSnapshot,
        _: NonZeroU64,
    ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
        Box::pin(async { panic!("readonly source must not acquire") })
    }
    fn read<'a>(
        &'a self,
        _: &'a TenantId,
        id: &'a SnapshotId,
    ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
        Box::pin(async move {
            self.0.reply().await;
            Ok(Some(
                SearchSnapshot::new(
                    id.clone(),
                    "private question".into(),
                    QueryId::new("query").unwrap(),
                    vec![],
                    Default::default(),
                    tenant(),
                    at(1.),
                    Some(WallClockExpiry::new(200.).unwrap()),
                )
                .unwrap(),
            ))
        })
    }
}
struct Ids;
impl SnapshotIds for Ids {
    fn next(&self) -> Result<SnapshotId, SnapshotFailure> {
        panic!("readonly source must not allocate")
    }
}
fn manifest_case(research: bool, mutation: usize) {
    let policy = Arc::new(Current(AtomicUsize::new(0)));
    let snapshots = Arc::new(Snapshots(Gate::new(if research { 2 } else { 1 })));
    let source = Sources {
        records: Arc::new(WorkflowRecords {
            reports: Gate::new(0),
            failure: false,
        }),
        snapshots: SnapshotService::new(
            snapshots.clone(),
            Arc::new(FixedClock),
            Arc::new(Ids),
            tenant(),
            SnapshotTtl::new(60).unwrap(),
        ),
        policy: policy.clone(),
        clock: Arc::new(FixedClock),
        staged_limits: staged::Limits::default(),
    };
    let root = ArtifactRef::new(
        if research {
            ArtifactKind::ResearchJob
        } else {
            ArtifactKind::StagedSearch
        },
        if research { "job-1" } else { "stage-1" },
    )
    .unwrap();
    let tenant = tenant();
    let cancel = Cancellation::default();
    let mut operation = source.resolve(&tenant, &root, &cancel);
    let mut cx = Context::from_waker(Waker::noop());
    assert!(
        operation.as_mut().poll(&mut cx).is_pending(),
        "last snapshot was not held"
    );
    assert_eq!(
        snapshots.0.calls.load(Ordering::SeqCst),
        if research { 2 } else { 1 }
    );
    policy.0.store(mutation, Ordering::SeqCst);
    assert!(operation.as_mut().poll(&mut cx).is_pending());
    snapshots.0.released.store(true, Ordering::SeqCst);
    let Poll::Ready(result) = operation.as_mut().poll(&mut cx) else {
        panic!("released snapshot did not complete")
    };
    if mutation == 0 {
        assert!(result.is_ok());
    } else {
        assert_eq!(
            result.err().unwrap(),
            json!({"error":{"code":"source_policy_denied","message":"current policy denies the source artifact"}})
        );
    }
}
#[test]
fn research_second_cursor_final_reply_rechecks_grant() {
    manifest_case(true, 1);
}
#[test]
fn research_second_cursor_final_reply_rechecks_retained_job_scope() {
    manifest_case(true, 2);
}
#[test]
fn staged_final_snapshot_reply_rechecks_grant() {
    manifest_case(false, 1);
}
#[test]
fn staged_final_snapshot_reply_rechecks_coherent_limits() {
    manifest_case(false, 3);
}
#[test]
fn unchanged_manifest_authority_preserves_both_workflows() {
    manifest_case(true, 0);
    manifest_case(false, 0);
}
