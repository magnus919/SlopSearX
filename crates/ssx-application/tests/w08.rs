//! Finite native W08 behavior cases. No providers, storage writes or runtime.
use ssx_application::{
    PortFuture,
    composition::*,
    dependency_dossier as dossier,
    lineage::{
        Failure,
        resolver::{Entities, EntityGroup},
        workflows::{Read, Records},
    },
    policy::{PolicyService, SearchPolicy},
    receipts::{Cancellation, service::Clock},
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
    snapshot::*,
};
use ssx_domain::{research::*, saved::SavedDefinition, staged::Record, *};
use ssx_json::{Value, json};
use std::{
    collections::BTreeMap,
    future::Future,
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
};
fn run<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::noop();
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut Context::from_waker(waker)) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("fixture unexpectedly needs a runtime"),
    }
}
fn at(n: f64) -> WallClockTimestamp {
    WallClockTimestamp::new(n).unwrap()
}
fn tenant() -> TenantId {
    TenantId::new("tenant-a").unwrap()
}
fn cap(name: &str) -> EngineCapability {
    EngineCapability {
        name: name.into(),
        enabled: true,
        active: true,
        available: true,
        categories: vec!["general".into()],
        media_types: vec![],
        auth_ready: true,
        circuit_open: false,
        cost_class: "free".into(),
        health_status: "unknown".into(),
        health_stale: true,
        enforced_filters: BTreeMap::new(),
    }
}
fn policy(deny: bool) -> PolicyService {
    PolicyService {
        policy: SearchPolicy {
            grants: [
                "research",
                "dependency_dossier",
                "security",
                "saved_searches",
                "staged_search",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            ..Default::default()
        },
        catalog: CatalogSnapshot::new(if deny {
            vec![]
        } else {
            ["pypi", "npm", "github", "nvd", "brave"]
                .into_iter()
                .map(cap)
                .collect()
        })
        .unwrap(),
        resolver: ScopeResolver {
            catalog_routing: false,
            ..Default::default()
        },
    }
}
struct AuthorityPort {
    deny: AtomicBool,
    foreign: AtomicBool,
}
impl Authority for AuthorityPort {
    fn current(&self) -> AuthoritySnapshot {
        AuthoritySnapshot {
            tenant: Some(if self.foreign.load(Ordering::SeqCst) {
                TenantId::new("tenant-b").unwrap()
            } else {
                tenant()
            }),
            policy: policy(self.deny.load(Ordering::SeqCst)),
            staged_limits: ssx_application::staged::Limits {
                granted: true,
                ..Default::default()
            },
            saved_max_reports: 2,
        }
    }
}
struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> WallClockTimestamp {
        at(100.)
    }
}
impl SnapshotClock for Fixed {
    fn now(&self) -> WallClockTimestamp {
        at(100.)
    }
}
impl SnapshotIds for Fixed {
    fn next(&self) -> Result<SnapshotId, SnapshotFailure> {
        panic!("preview cannot create a snapshot")
    }
}
struct Snapshots {
    snapshot: Option<SearchSnapshot>,
    failure: Option<SnapshotFailure>,
    reads: AtomicUsize,
    revoke: Option<Arc<AuthorityPort>>,
}
impl SnapshotRepository for Snapshots {
    fn create<'a>(
        &'a self,
        _: &'a SearchSnapshot,
        _: NonZeroU64,
    ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
        Box::pin(async { panic!("preview must not persist or renew") })
    }
    fn read<'a>(
        &'a self,
        t: &'a TenantId,
        id: &'a SnapshotId,
    ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
        Box::pin(async move {
            assert_eq!(t, &tenant());
            self.reads.fetch_add(1, Ordering::SeqCst);
            if let Some(a) = &self.revoke {
                a.deny.store(true, Ordering::SeqCst);
            }
            if let Some(e) = self.failure {
                return Err(e);
            }
            Ok(self.snapshot.as_ref().filter(|s| s.id() == id).cloned())
        })
    }
}
struct Groups;
impl Entities for Groups {
    fn groups(&self, s: &SearchSnapshot, _: u8) -> Vec<EntityGroup> {
        vec![EntityGroup {
            id: "entity-1".into(),
            namespace: "fixture".into(),
            result_indices: (0..s.results().len()).collect(),
        }]
    }
}
// Explicit groups remain authoritative even beyond the displayed prefix.
struct ExplicitGroups(Vec<usize>);
impl Entities for ExplicitGroups {
    fn groups(&self, _: &SearchSnapshot, _: u8) -> Vec<EntityGroup> {
        vec![EntityGroup {
            id: "entity-1".into(),
            namespace: "fixture".into(),
            result_indices: self.0.clone(),
        }]
    }
}
struct WorkflowRecords {
    job: Read<ResearchJob>,
    staged: Read<Record>,
    definition: Read<SavedDefinition>,
    reports: Vec<Value>,
    reads: AtomicUsize,
}
impl Records for WorkflowRecords {
    fn research<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a str,
        _: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<ResearchJob>, Failure>> {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(self.job.clone())
        })
    }
    fn staged<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a str,
        _: WallClockTimestamp,
        _: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<Record>, Failure>> {
        Box::pin(async move { Ok(self.staged.clone()) })
    }
    fn saved<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a str,
        _: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<SavedDefinition>, Failure>> {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(self.definition.clone())
        })
    }
    fn reports<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a SavedDefinition,
        _: WallClockTimestamp,
        limit: usize,
        _: Option<&'a str>,
        _: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<Vec<Value>>, Failure>> {
        Box::pin(async move {
            assert_eq!(limit, 2);
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(Read::Found(self.reports.clone()))
        })
    }
}
impl SavedReports for WorkflowRecords {
    fn selected<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a SavedDefinition,
        run: &'a str,
        limit: usize,
        _: &'a Cancellation,
    ) -> PortFuture<'a, Result<Read<Value>, Failure>> {
        Box::pin(async move {
            assert_eq!(limit, 2);
            Ok(self
                .reports
                .iter()
                .find(|r| r["run_id"].as_str() == Some(run))
                .cloned()
                .map(Read::Found)
                .unwrap_or(Read::Missing))
        })
    }
}
fn records() -> WorkflowRecords {
    WorkflowRecords {
        job: Read::Missing,
        staged: Read::Missing,
        definition: Read::Missing,
        reports: vec![],
        reads: AtomicUsize::new(0),
    }
}
fn snapshot(count: usize, expires: f64) -> SearchSnapshot {
    SearchSnapshot::new(
        SnapshotId::new("snapshot-a").unwrap(),
        "retained query".into(),
        QueryId::new("query-a").unwrap(),
        (0..count)
            .map(|i| SearchResult {
                url: format!("https://example.test/{i}"),
                engine: "pypi".into(),
                ..Default::default()
            })
            .collect(),
        ScopeDecision {
            selected_engines: vec!["pypi".into()],
            ..Default::default()
        },
        tenant(),
        at(1.),
        Some(WallClockExpiry::new(expires).unwrap()),
    )
    .unwrap()
}
fn resolver(
    snapshot: Option<SearchSnapshot>,
    failure: Option<SnapshotFailure>,
    revoke: bool,
    records: WorkflowRecords,
) -> (SourceResolver, Arc<Snapshots>, Arc<AuthorityPort>) {
    let authority = Arc::new(AuthorityPort {
        deny: AtomicBool::new(false),
        foreign: AtomicBool::new(false),
    });
    let records = Arc::new(records);
    let store = Arc::new(Snapshots {
        snapshot,
        failure,
        reads: AtomicUsize::new(0),
        revoke: revoke.then(|| authority.clone()),
    });
    (
        SourceResolver {
            snapshots: SnapshotService::new(
                store.clone(),
                Arc::new(Fixed),
                Arc::new(Fixed),
                tenant(),
                SnapshotTtl::new(200).unwrap(),
            ),
            records: records.clone(),
            saved_reports: records,
            entities: Arc::new(Groups),
            authority: authority.clone(),
            clock: Arc::new(Fixed),
        },
        store,
        authority,
    )
}
fn artifact(kind: ArtifactKind, id: &str) -> ArtifactRef {
    ArtifactRef::new(kind, id).unwrap()
}
fn package(name: &str, version: &str) -> SearchResult {
    SearchResult {
        engine: "pypi".into(),
        payload: Some(
            json!({"domain":"packages","type":"package","schema_version":1,"provenance":{"engine":"pypi","adapter_fields":["name"],"inferred_fields":[]},"data":{"name":name,"version":version,"repository_url":"https://github.com/psf/requests.git"}}),
        ),
        ..Default::default()
    }
}
#[test]
fn identity_normalization_and_canonical_digest_are_exact() {
    assert_eq!(
        dossier::normalize_package("pypi", " Foo__Bar.-BAZ ").unwrap(),
        "foo-bar-baz"
    );
    assert_eq!(
        dossier::normalize_package("npm", "@scope/a~b").unwrap(),
        "@scope/a~b"
    );
    for value in ["Upper", "@scope", "@scope/a/b", "@/pkg", "../pkg"] {
        assert!(dossier::normalize_package("npm", value).is_err(), "{value}");
    }
    for value in [
        "https://evil.test/a/b",
        "https://github.com:443/a/b",
        "https://a@github.com/a/b",
        "https://github.com/a/b/issues",
        "https://github.com/a/b?q=1",
        "a/b#fragment",
    ] {
        assert!(
            dossier::normalize_repository(Some(value)).is_err(),
            "{value}"
        );
    }
    let identity = dossier::Identity::new(
        "pypi",
        "Foo_Bar",
        Some(" 1.0 "),
        Some("https://github.com/a/b.git"),
        None,
    )
    .unwrap();
    assert_eq!(
        identity.value,
        json!({"ecosystem":"pypi","package":"foo-bar","repository":"a/b","requested_deadline":null,"requested_version":"1.0"})
    );
    // Independent native SHA over the exact Python sorted compact ASCII bytes.
    use sha2::{Digest, Sha256};
    let expected=Sha256::digest(b"{\"ecosystem\":\"pypi\",\"package\":\"foo-bar\",\"repository\":\"a/b\",\"requested_deadline\":null,\"requested_version\":\"1.0\"}");
    assert_eq!(
        identity.digest,
        expected
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
}
#[test]
fn exact_attribution_preserves_ambiguity_and_never_infers_ownership() {
    let mut forged = package("requests", "1");
    forged.payload.as_mut().unwrap()["provenance"]["inferred_fields"] = json!(["name"]);
    let r = dossier::resolve_package_results("pypi", "requests", &[forged]);
    assert_eq!(r["status"], "unresolved");
    let r = dossier::resolve_package_results(
        "pypi",
        "requests",
        &[
            package("Requests", "1"),
            package("requests", "2"),
            package("other", "9"),
        ],
    );
    assert_eq!(r["status"], "ambiguous");
    assert_eq!(r["matches"].as_array().unwrap().len(), 2);
    assert_eq!(r["repository_candidates"], json!(["psf/requests"]));
    for mode in [
        "schema",
        "provenance",
        "adapter_fields",
        "name",
        "repository",
    ] {
        let mut malformed = package("requests", "1");
        let payload = malformed.payload.as_mut().unwrap();
        match mode {
            "schema" => payload["schema_version"] = json!(2),
            "provenance" => payload["provenance"] = json!("bad"),
            "adapter_fields" => payload["provenance"]["adapter_fields"] = json!("name"),
            "name" => payload["data"]["name"] = json!({"bad":true}),
            "repository" => payload["data"]["repository_url"] = json!("psf/requests"),
            _ => unreachable!(),
        }
        let result = dossier::resolve_package_results("pypi", "requests", &[malformed]);
        if mode == "repository" {
            assert_eq!(result["status"], "resolved");
            assert_eq!(result["repository_candidates"], json!([]));
        } else {
            assert_eq!(result["status"], "unresolved", "{mode}");
        }
    }
    let mut wrong = package("requests", "1");

    wrong.engine = "npm".into();
    assert_eq!(
        dossier::resolve_package_results("pypi", "requests", &[wrong])["status"],
        "unresolved"
    );
}
#[test]
fn plan_is_bounded_and_idempotency_binds_identity_and_source() {
    let limits = dossier::Limits {
        queries: 6,
        results: 50,
        max_results: 5,
    };
    let identity =
        || dossier::Identity::new("pypi", "requests", Some("1"), Some("a/b"), None).unwrap();
    let source = ResolvedSource {
        artifact: artifact(ArtifactKind::Snapshot, "snapshot-a"),
        status: SourceStatus::Live,
        result_ids: vec!["snapshot-a:0".into()],
        query: Some("q".into()),
        details: json!({}),
    };
    let job = dossier::plan(
        identity(),
        &tenant(),
        "job-a",
        at(100.),
        at(200.),
        Some("idem"),
        Some(&source),
        &limits,
    )
    .unwrap();
    assert_eq!(
        job.queries
            .iter()
            .map(|q| q.query.as_str())
            .collect::<Vec<_>>(),
        vec!["requests", "repo:a/b", "pypi requests 1"]
    );
    assert_eq!(job.workflow["budget"]["max_adapter_calls"], 6);
    assert_eq!(job.workflow["composition"]["source"], source.stored());
    assert!(dossier::same_request(&job, &job));
    let mut changed = job.clone();
    changed.workflow.get_mut("composition").unwrap()["source"]["artifact"]["id"] = json!("other");
    assert!(!dossier::same_request(&job, &changed));
    assert!(
        dossier::plan(
            identity(),
            &tenant(),
            "job-a",
            at(100.),
            at(200.),
            None,
            None,
            &dossier::Limits {
                queries: 2,
                ..limits
            }
        )
        .is_err()
    );
}
#[test]
fn preview_bounds_snapshot_result_entity_without_persistence_or_dispatch() {
    let (resolver, store, _) = resolver(Some(snapshot(26, 200.)), None, false, records());
    let cancel = Cancellation::default();
    let source = run(resolver.resolve(
        &tenant(),
        &artifact(ArtifactKind::Snapshot, "snapshot-a"),
        Destination::DependencyDossier,
        &cancel,
    ))
    .unwrap();
    assert_eq!(source.status, SourceStatus::Truncated);
    assert_eq!(source.result_ids.len(), 25);
    assert_eq!(source.details["result_count"], 26);
    assert_eq!(
        run(resolver.resolve(
            &tenant(),
            &artifact(ArtifactKind::Snapshot, "snapshot-a"),
            Destination::Research,
            &cancel
        ))
        .unwrap_err()
        .code,
        "source_truncated"
    );
    let selected = run(resolver.resolve(
        &tenant(),
        &artifact(ArtifactKind::Result, "snapshot-a:25"),
        Destination::DependencyDossier,
        &cancel,
    ))
    .unwrap();
    assert_eq!(selected.result_ids, vec!["snapshot-a:25"]);
    for id in ["snapshot-a:01", "snapshot-a:-1", "snapshot-a:26"] {
        assert_eq!(
            run(resolver.resolve(
                &tenant(),
                &artifact(ArtifactKind::Result, id),
                Destination::DependencyDossier,
                &cancel
            ))
            .unwrap_err()
            .code,
            "source_not_found"
        );
    }
    let group = ssx_domain::canonical::composite_artifact_id(&["snapshot-a", "entity-1"]).unwrap();
    assert_eq!(
        run(resolver.resolve(
            &tenant(),
            &artifact(ArtifactKind::EntityGroup, &group),
            Destination::DependencyDossier,
            &cancel
        ))
        .unwrap()
        .result_ids
        .len(),
        25
    );
    assert_eq!(store.reads.load(Ordering::SeqCst), 5);

    // A large retained source still exposes only the ordered first 25 IDs,
    // while its complete selection count controls status and metadata.
    let first_25 = (0..25)
        .map(|index| format!("snapshot-a:{index}"))
        .collect::<Vec<_>>();
    for count in [0, 25, 26, 4096] {
        let (resolver, store, _) =
            self::resolver(Some(snapshot(count, 200.)), None, false, records());
        let requested = artifact(ArtifactKind::Snapshot, "snapshot-a");
        let source = run(resolver.resolve(
            &tenant(),
            &requested,
            Destination::DependencyDossier,
            &cancel,
        ))
        .unwrap();
        assert_eq!(source.artifact, requested);
        assert_eq!(source.query.as_deref(), Some("retained query"));
        assert_eq!(source.details["query_id"], "query-a");
        assert_eq!(source.details["result_count"], count);
        assert_eq!(source.details["included_result_count"], count.min(25));
        assert_eq!(
            source.status,
            if count > 25 {
                SourceStatus::Truncated
            } else {
                SourceStatus::Live
            }
        );
        assert_eq!(
            source.result_ids,
            if count == 0 { vec![] } else { first_25.clone() }
        );
        assert_eq!(store.reads.load(Ordering::SeqCst), 1);
        assert_eq!(store.snapshot.as_ref().unwrap().results().len(), count);
    }

    let (mut explicit, _, _) = self::resolver(Some(snapshot(26, 200.)), None, false, records());
    explicit.entities = Arc::new(ExplicitGroups((0..26).rev().collect()));
    let requested = artifact(ArtifactKind::EntityGroup, &group);
    let source = run(explicit.resolve(
        &tenant(),
        &requested,
        Destination::DependencyDossier,
        &cancel,
    ))
    .unwrap();
    assert_eq!(source.artifact, requested);
    assert_eq!(source.status, SourceStatus::Truncated);
    assert_eq!(
        source.result_ids,
        (1..26)
            .rev()
            .map(|index| format!("snapshot-a:{index}"))
            .collect::<Vec<_>>()
    );
    assert_eq!(source.details["member_count"], 26);
    assert_eq!(source.details["included_result_count"], 25);

    // The first 25 members are valid: only complete validation can reject
    // the out-of-range final member, which would not be displayed.
    explicit.entities = Arc::new(ExplicitGroups((0..27).collect()));
    assert_eq!(
        run(explicit.resolve(
            &tenant(),
            &requested,
            Destination::DependencyDossier,
            &cancel,
        ))
        .unwrap_err()
        .code,
        "source_malformed"
    );
}
#[test]
fn foreign_expired_missing_unavailable_revoked_and_cancelled_sources_fail_closed() {
    for (snap, failure, code) in [
        (Some(snapshot(1, 99.)), None, "source_expired"),
        (None, None, "source_not_found"),
        (
            None,
            Some(SnapshotFailure::Unavailable),
            "source_unavailable",
        ),
    ] {
        let (resolver, _, _) = resolver(snap, failure, false, records());
        assert_eq!(
            run(resolver.resolve(
                &tenant(),
                &artifact(ArtifactKind::Snapshot, "snapshot-a"),
                Destination::Research,
                &Cancellation::default()
            ))
            .unwrap_err()
            .code,
            code
        );
    }
    let (resolver, store, auth) = resolver(Some(snapshot(1, 200.)), None, true, records());
    assert_eq!(
        run(resolver.resolve(
            &tenant(),
            &artifact(ArtifactKind::Snapshot, "snapshot-a"),
            Destination::Research,
            &Cancellation::default()
        ))
        .unwrap_err()
        .code,
        "source_policy_denied"
    );
    auth.foreign.store(true, Ordering::SeqCst);
    assert!(
        run(resolver.resolve(
            &tenant(),
            &artifact(ArtifactKind::Snapshot, "snapshot-a"),
            Destination::Research,
            &Cancellation::default()
        ))
        .is_err()
    );
    assert_eq!(store.reads.load(Ordering::SeqCst), 1);
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(
        run(resolver.resolve(
            &tenant(),
            &artifact(ArtifactKind::Snapshot, "snapshot-a"),
            Destination::Research,
            &cancel
        ))
        .unwrap_err()
        .code,
        "cancelled"
    );
}
#[test]
fn unsupported_transitions_read_nothing_and_research_missing_cursors_are_partial() {
    let (resolver, store, _) = resolver(Some(snapshot(1, 200.)), None, false, records());
    assert_eq!(
        run(resolver.resolve(
            &tenant(),
            &artifact(ArtifactKind::Result, "snapshot-a:0"),
            Destination::Research,
            &Cancellation::default()
        ))
        .unwrap_err()
        .code,
        "unsupported_transition"
    );
    assert_eq!(store.reads.load(Ordering::SeqCst), 0);
    let mut rec = records();
    rec.job = Read::Found(ResearchJob {
        job_id: "job-a".into(),
        tenant: tenant().as_str().into(),
        question: "research question".into(),
        queries: vec![ResearchQuery {
            engines: vec!["pypi".into()],
            ..Default::default()
        }],
        ..Default::default()
    });
    let (resolver, _, _) = self::resolver(Some(snapshot(1, 200.)), None, false, rec);
    assert_eq!(
        run(resolver.resolve(
            &tenant(),
            &artifact(ArtifactKind::ResearchJob, "job-a"),
            Destination::ResearchManifest,
            &Cancellation::default()
        ))
        .unwrap_err()
        .code,
        "source_partial"
    );
}
#[test]
fn research_attempts_stream_valid_retained_cursor_sets() {
    let make = |n| {
        let mut rec = records();
        rec.job = Read::Found(ResearchJob {
            job_id: "job-a".into(),
            tenant: tenant().as_str().into(),
            state: "partial".into(),
            question: "q".into(),
            queries: vec![ResearchQuery {
                engines: vec!["pypi".into()],
                attempts: (0..n)
                    .map(|i| ResearchQueryAttempt {
                        attempt_id: format!("attempt-{i}"),
                        cursor: Some("snapshot-a".into()),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        });
        rec
    };
    let (resolver, _, _) = resolver(Some(snapshot(2, 200.)), None, false, make(1));
    let source = run(resolver.resolve(
        &tenant(),
        &artifact(ArtifactKind::ResearchJob, "job-a"),
        Destination::ResearchManifest,
        &Cancellation::default(),
    ))
    .unwrap();
    assert_eq!(source.status, SourceStatus::Partial);
    assert_eq!(source.result_ids.len(), 2);
    let (resolver, store, _) = self::resolver(Some(snapshot(2, 200.)), None, false, make(101));
    let source = run(resolver.resolve(
        &tenant(),
        &artifact(ArtifactKind::ResearchJob, "job-a"),
        Destination::ResearchManifest,
        &Cancellation::default(),
    ))
    .unwrap();
    assert_eq!(source.status, SourceStatus::Truncated);
    assert_eq!(source.result_ids.len(), 2);
    assert_eq!(source.details["result_count"], 202);
    assert_eq!(store.reads.load(Ordering::SeqCst), 101);
}
#[test]
fn report_attribution_uses_all_admitted_rows_while_cards_remain_sliced() {
    let limits = dossier::Limits {
        queries: 4,
        results: 50,
        max_results: 1,
    };
    let mut job = dossier::plan(
        dossier::Identity::new("pypi", "requests", Some("2"), None, None).unwrap(),
        &tenant(),
        "job-a",
        at(100.),
        at(200.),
        None,
        None,
        &limits,
    )
    .unwrap();
    job.state = "succeeded".into();
    job.queries[0].state = "done".into();
    job.queries[0].cursor = Some("snapshot-a".into());
    let s = SearchSnapshot::new(
        SnapshotId::new("snapshot-a").unwrap(),
        "q".into(),
        QueryId::new("query-a").unwrap(),
        vec![package("requests", "1"), package("requests", "2")],
        Default::default(),
        tenant(),
        at(1.),
        None,
    )
    .unwrap();
    let section = dossier::section_from_snapshot(
        &job,
        0,
        &s,
        1,
        None,
        &|r, id| json!({"result_id":id,"title":r.title}),
    )
    .unwrap();
    assert_eq!(section.value["results"].as_array().unwrap().len(), 1);
    let report = dossier::report(
        &job,
        [
            section,
            dossier::section_without_snapshot(None),
            dossier::section_without_snapshot(Some(&job.queries[1])),
        ],
    )
    .unwrap();
    assert_eq!(report["resolved_package_identity"]["status"], "ambiguous");
    assert_eq!(
        report["resolved_package_identity"]["version_match"],
        "exact"
    );
    assert_eq!(
        report["repository_identity"]["proven_package_ownership"],
        false
    );
    assert_eq!(
        report["sections"]["advisory_leads"]["applicability"]["status"],
        "not_evaluated"
    );
    assert_eq!(s.results().len(), 2);
    let failed = ResearchQuery {
        state: "failed".into(),
        engine_coverage: vec![EngineCoverage {
            engine: "nvd".into(),
            bucket: "failed".into(),
            failure_class: Some("error".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let section = dossier::section_without_snapshot(Some(&failed));
    assert_eq!(section.value["state"], "failed");
    assert_eq!(section.value["error"], "upstream_error");
    let unavailable = ResearchQuery {
        engine_coverage: vec![EngineCoverage {
            engine: "nvd".into(),
            bucket: "unavailable".into(),
            failure_class: Some("unavailable".into()),
            ..Default::default()
        }],
        ..failed
    };
    let section = dossier::section_without_snapshot(Some(&unavailable));
    assert_eq!(section.value["state"], "unavailable");
    assert_eq!(section.value["error"], "unavailable");
}
fn saved_definition() -> SavedDefinition {
    ssx_json::from_value(json!({"search_id":"saved-a","tenant":"tenant-a","query":"current definition query","engines":["pypi"],"interval_seconds":60,"retention_seconds":600,"max_results":25,"max_reports":2,"created_at":1,"expires_at":200,"next_due":150,"revision":1,"paused":false,"policy_fingerprint":"fixture","last_slot":0,"latest_run_id":"run-a","baseline":null,"lease_token":null,"lease_expires_at":0,"deleted":false})).unwrap()
}
#[test]
fn saved_report_uses_historical_window_and_rejects_incomparable_truncated_malformed() {
    let reference = artifact(
        ArtifactKind::SavedReport,
        &ssx_domain::canonical::composite_artifact_id(&["saved-a", "run-a"]).unwrap(),
    );
    for mode in [
        "live",
        "truncated",
        "incomparable",
        "malformed",
        "missing-window",
        "denied-window",
    ] {
        let mut rec = records();
        rec.definition = Read::Found(saved_definition());
        let mut report = json!({"search_id":"saved-a","run_id":"run-a","expires_at":200,"status":"changed","revision":1,"incomparable_reasons":[],"observation":{"window":{"query":"historical window query","engines":["pypi"]}}});
        match mode {
            "truncated" => report["incomparable_reasons"] = json!(["observation_truncated"]),
            "incomparable" => report["status"] = json!("incomparable"),
            "malformed" => report["incomparable_reasons"] = json!("not-an-array"),
            "missing-window" => report["observation"] = json!({}),
            "denied-window" => report["observation"]["window"]["engines"] = json!(["hibp"]),
            _ => {}
        }
        rec.reports.push(report);
        let (resolver, store, _) = resolver(None, None, false, rec);
        let result = run(resolver.resolve(
            &tenant(),
            &reference,
            Destination::Research,
            &Cancellation::default(),
        ));
        if mode == "live" {
            let source = result.unwrap();
            assert_eq!(source.query.as_deref(), Some("historical window query"));
            assert!(source.result_ids.is_empty());
        } else {
            assert_eq!(
                result.unwrap_err().code,
                match mode {
                    "truncated" => "source_truncated",
                    "incomparable" | "missing-window" => "source_incomparable",
                    "denied-window" => "source_policy_denied",
                    _ => "source_malformed",
                },
                "{mode}"
            );
        }
        assert_eq!(store.reads.load(Ordering::SeqCst), 0);
    }
}
fn staged_record(selected: bool) -> Record {
    let limits = ssx_application::staged::Limits {
        granted: true,
        ..Default::default()
    };
    let plan = ssx_application::staged::Plan::build(
        &policy(false),
        &limits,
        &ssx_application::staged::Input {
            query: "stage question".into(),
            objectives: json!({"deadline_ms":1000,"max_engine_calls":1}),
            initial_scope: json!({"engines":["pypi"]}),
            fallback_scope: None,
            allow_scope_expansion: false,
            filters: None,
        },
    )
    .unwrap();
    let record = plan
        .record(&tenant(), &StagedSearchId::new("stage-a").unwrap(), 100.)
        .unwrap();
    if !selected {
        return record;
    }
    let mut v = record.value().clone();
    v["state"] = json!("completed");
    v["selected_result_attempt_id"] = json!("attempt-a");
    v["budget"]["reserved"] = json!(1);
    v["budget"]["observed"] = json!(1);
    let stage = &mut v["stages"].as_array_mut().unwrap()[0];
    stage["state"] = json!("done");
    stage["attempts"] = json!([{"attempt_id":"attempt-a","cursor":"snapshot-a","state":"done","started_at":100,"finished_at":101,"reserved_engine_calls":1,"observed_engine_calls":1,"engine_outcomes":[],"enforcement":stage["enforcement"]}]);
    Record::from_value(v).unwrap()
}
#[test]
fn staged_source_requires_selected_attempt_and_captured_cursor() {
    for selected in [false, true] {
        let mut rec = records();
        rec.staged = Read::Found(staged_record(selected));
        let (resolver, store, _) = resolver(Some(snapshot(1, 200.)), None, false, rec);
        let source = run(resolver.resolve(
            &tenant(),
            &artifact(ArtifactKind::StagedSearch, "stage-a"),
            Destination::ResearchManifest,
            &Cancellation::default(),
        ));
        if selected {
            let source = source.unwrap();
            assert_eq!(source.details["selected_attempt_id"], "attempt-a");
            assert_eq!(source.details["selected_snapshot"], "snapshot-a");
            assert_eq!(source.result_ids, vec!["snapshot-a:0"]);
            assert_eq!(store.reads.load(Ordering::SeqCst), 1);
        } else {
            assert_eq!(source.unwrap_err().code, "source_partial");
            assert_eq!(store.reads.load(Ordering::SeqCst), 0);
        }
    }
}
#[test]
fn repository_parser_preserves_literal_host_port_and_empty_credentials_semantics() {
    for value in [
        "https://ｇithub.com/a/b",
        "https://github．com/a/b",
        "https://%67ithub.com/a/b",
        "https://github.com:443/a/b",
        "https://name@github.com/a/b",
        "https://:secret@github.com/a/b",
        "https://github.com/a/../b/c",
        "https://github.com/a/b;",
    ] {
        assert!(
            dossier::normalize_repository(Some(value)).is_err(),
            "{value}"
        );
    }
    for value in [
        "https://github.com:0/a/b",
        "https://github.com:/a/b",
        "https://@github.com/a/b",
        "https://:@github.com/a/b",
        "HTTPS://GitHub.Com/a/b.git",
        "https://github.com/a/b?",
        "https://github.com/a/b#",
    ] {
        assert_eq!(
            dossier::normalize_repository(Some(value))
                .unwrap()
                .as_deref(),
            Some("a/b"),
            "{value}"
        );
    }
}
struct MovingClock(std::sync::atomic::AtomicU64);
impl Clock for MovingClock {
    fn now(&self) -> WallClockTimestamp {
        at(f64::from_bits(self.0.load(Ordering::SeqCst)))
    }
}
impl SnapshotClock for MovingClock {
    fn now(&self) -> WallClockTimestamp {
        Clock::now(self)
    }
}
struct AdvancingSnapshots {
    rows: BTreeMap<String, SearchSnapshot>,
    clock: Arc<MovingClock>,
    advance_on: String,
    advance_to: f64,
    reads: AtomicUsize,
}
impl SnapshotRepository for AdvancingSnapshots {
    fn create<'a>(
        &'a self,
        _: &'a SearchSnapshot,
        _: NonZeroU64,
    ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
        Box::pin(async { panic!("expiry witness resolution must not persist") })
    }
    fn read<'a>(
        &'a self,
        _: &'a TenantId,
        id: &'a SnapshotId,
    ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if id.as_str() == self.advance_on {
                self.clock
                    .0
                    .store(self.advance_to.to_bits(), Ordering::SeqCst);
            }
            Ok(self.rows.get(id.as_str()).cloned())
        })
    }
}
#[test]
fn earlier_snapshot_expiry_is_rechecked_after_later_cursor_read_and_attempt_refs_are_exact() {
    let clock = Arc::new(MovingClock(std::sync::atomic::AtomicU64::new(
        100f64.to_bits(),
    )));
    let mut b = snapshot(1, 5000.);
    b = SearchSnapshot::new(
        SnapshotId::new("snapshot-b").unwrap(),
        b.query().into(),
        b.query_id().clone(),
        b.results().to_vec(),
        b.scope().clone(),
        tenant(),
        at(1.),
        b.expires_at(),
    )
    .unwrap();
    let store = Arc::new(AdvancingSnapshots {
        rows: [
            ("snapshot-a".into(), snapshot(1, 200.)),
            ("snapshot-b".into(), b),
        ]
        .into_iter()
        .collect(),
        clock: clock.clone(),
        advance_on: "snapshot-b".into(),
        advance_to: 201.,
        reads: AtomicUsize::new(0),
    });
    let mut rec = records();
    rec.job = Read::Found(ResearchJob {
        job_id: "job-attempts".into(),
        tenant: tenant().as_str().into(),
        state: "succeeded".into(),
        question: "q".into(),
        queries: vec![ResearchQuery {
            engines: vec!["pypi".into()],
            attempts: vec![
                ResearchQueryAttempt {
                    attempt_id: "attempt-a".into(),
                    cursor: Some("snapshot-a".into()),
                    ..Default::default()
                },
                ResearchQueryAttempt {
                    attempt_id: "attempt-b".into(),
                    cursor: Some("snapshot-b".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    });
    let (mut resolver, _, _) = resolver(None, None, false, rec);
    resolver.snapshots = SnapshotService::new(
        store.clone(),
        clock.clone(),
        Arc::new(Fixed),
        tenant(),
        SnapshotTtl::new(60).unwrap(),
    );
    resolver.clock = clock.clone();
    let attempt = artifact(
        ArtifactKind::ResearchAttempt,
        &ssx_domain::canonical::composite_artifact_id(&["job-attempts", "attempt-a"]).unwrap(),
    );
    let source = run(resolver.resolve(
        &tenant(),
        &attempt,
        Destination::ResearchManifest,
        &Cancellation::default(),
    ))
    .unwrap();
    assert_eq!(source.result_ids, vec!["snapshot-a:0"]);
    assert_eq!(store.reads.load(Ordering::SeqCst), 1);
    let missing = artifact(
        ArtifactKind::ResearchAttempt,
        &ssx_domain::canonical::composite_artifact_id(&["job-attempts", "missing"]).unwrap(),
    );
    assert_eq!(
        run(resolver.resolve(
            &tenant(),
            &missing,
            Destination::ResearchManifest,
            &Cancellation::default()
        ))
        .unwrap_err()
        .code,
        "source_not_found"
    );
    assert_eq!(store.reads.load(Ordering::SeqCst), 1);
    assert_eq!(
        run(resolver.resolve(
            &tenant(),
            &artifact(ArtifactKind::ResearchJob, "job-attempts"),
            Destination::ResearchManifest,
            &Cancellation::default()
        ))
        .unwrap_err()
        .code,
        "source_expired"
    );
    assert_eq!(Clock::now(clock.as_ref()).seconds(), 201.);
    assert_eq!(store.reads.load(Ordering::SeqCst), 3);
}
#[test]
fn staged_expiry_is_rechecked_after_selected_snapshot_read() {
    let clock = Arc::new(MovingClock(std::sync::atomic::AtomicU64::new(
        100f64.to_bits(),
    )));
    let store = Arc::new(AdvancingSnapshots {
        rows: [("snapshot-a".into(), snapshot(1, 5000.))]
            .into_iter()
            .collect(),
        clock: clock.clone(),
        advance_on: "snapshot-a".into(),
        advance_to: 3701.,
        reads: AtomicUsize::new(0),
    });
    let mut rec = records();
    let staged = staged_record(true);
    assert_eq!(staged.expires_at(), 3700.);
    rec.staged = Read::Found(staged);
    let (mut resolver, _, _) = resolver(None, None, false, rec);
    resolver.snapshots = SnapshotService::new(
        store.clone(),
        clock.clone(),
        Arc::new(Fixed),
        tenant(),
        SnapshotTtl::new(60).unwrap(),
    );
    resolver.clock = clock;
    assert_eq!(
        run(resolver.resolve(
            &tenant(),
            &artifact(ArtifactKind::StagedSearch, "stage-a"),
            Destination::ResearchManifest,
            &Cancellation::default()
        ))
        .unwrap_err()
        .code,
        "source_expired"
    );
    assert_eq!(store.reads.load(Ordering::SeqCst), 1);
}
