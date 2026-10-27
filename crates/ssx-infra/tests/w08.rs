//! W08 real private Valkey, actual existing research worker and loopback upstream.
#[path = "w07_support/valkey.rs"]
mod private_valkey;
use ssx_application::{
    NoopSink, PortFuture, SearchEngine,
    cache::*,
    composition::{self, Destination, SourceResolver},
    dependency_dossier as dossier,
    lineage::resolver::{Entities, EntityGroup},
    policy::*,
    receipts::{Cancellation, service::Clock},
    scope::*,
    snapshot::*,
};
use ssx_domain::{research::*, *};
use ssx_infra::{
    dependency_dossier::*,
    research_runner::*,
    search::*,
    valkey::{
        self, Valkey,
        research::{JobRead, ResearchStore},
        retrieval::lineage::{
            Authority as RecordsAuthority, AuthoritySnapshot as RecordsAuthoritySnapshot,
            NativeRecords,
        },
        snapshot::{RandomSnapshotIds, SystemSnapshotClock, ValkeySnapshots},
    },
};
use ssx_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Read as _, Write as _},
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Semaphore, watch};
fn tenant() -> TenantId {
    TenantId::new("tenant-a").unwrap()
}
fn now() -> WallClockTimestamp {
    SystemSearchClock.capture().0
}
fn capability(name: &str) -> EngineCapability {
    EngineCapability {
        name: name.into(),
        enabled: true,
        active: true,
        available: true,
        categories: vec![
            match name {
                "pypi" => "packages",
                "github" => "code",
                "nvd" => "security",
                _ => "general",
            }
            .into(),
        ],
        media_types: vec![],
        auth_ready: true,
        circuit_open: false,
        cost_class: "free".into(),
        health_status: "unknown".into(),
        health_stale: true,
        enforced_filters: BTreeMap::new(),
    }
}
fn policy(denied: bool) -> PolicyService {
    PolicyService {
        policy: SearchPolicy {
            grants: if denied {
                Default::default()
            } else {
                [
                    "research",
                    "dependency_dossier",
                    "security",
                    "saved_searches",
                    "staged_search",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect()
            },
            ..Default::default()
        },
        catalog: CatalogSnapshot::new(
            ["pypi", "github", "nvd"]
                .into_iter()
                .map(capability)
                .collect(),
        )
        .unwrap(),
        resolver: ScopeResolver {
            catalog_routing: false,
            ..Default::default()
        },
    }
}
struct Current {
    denied: AtomicBool,
    clock: AtomicU64,
}
impl Clock for Current {
    fn now(&self) -> WallClockTimestamp {
        WallClockTimestamp::new(f64::from_bits(self.clock.load(Ordering::SeqCst))).unwrap()
    }
}
impl SnapshotClock for Current {
    fn now(&self) -> WallClockTimestamp {
        Clock::now(self)
    }
}
impl composition::Authority for Current {
    fn current(&self) -> composition::AuthoritySnapshot {
        composition::AuthoritySnapshot {
            tenant: Some(tenant()),
            policy: policy(self.denied.load(Ordering::SeqCst)),
            staged_limits: ssx_application::staged::Limits {
                granted: true,
                ..Default::default()
            },
            saved_max_reports: 2,
        }
    }
}
impl RecordsAuthority for Current {
    fn current(&self) -> RecordsAuthoritySnapshot {
        RecordsAuthoritySnapshot {
            search: policy(self.denied.load(Ordering::SeqCst)),
            limits: ssx_application::saved::SavedLimits {
                reports: 2,
                ..Default::default()
            },
            auth_configured: ["pypi", "github", "nvd"]
                .into_iter()
                .map(|n| (n.into(), true))
                .collect(),
        }
    }
}
struct Groups;
impl Entities for Groups {
    fn groups(&self, _: &SearchSnapshot, _: u8) -> Vec<EntityGroup> {
        vec![]
    }
}
struct Loopback {
    url: String,
    calls: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Loopback {
    fn new() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let c = calls.clone();
        let s = stop.clone();
        let thread = std::thread::spawn(move || {
            while !s.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        socket
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut bytes = [0u8; 4096];
                        let n = socket.read(&mut bytes).unwrap();
                        assert!(
                            std::str::from_utf8(&bytes[..n])
                                .unwrap()
                                .starts_with("GET /fixture HTTP/1.1")
                        );
                        c.fetch_add(1, Ordering::SeqCst);
                        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1))
                    }
                    Err(e) => panic!("loopback: {e}"),
                }
            }
        });
        Self {
            url,
            calls,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for Loopback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}
struct FixtureEngine {
    url: String,
    client: reqwest::Client,
    authority: Arc<Current>,
    revoke_after_call: AtomicBool,
}
impl SearchEngine for FixtureEngine {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            let reply = self.client.get(&self.url).send().await.unwrap();
            assert_eq!(reply.status(), 200);
            let _ = reply.bytes().await.unwrap();
            if self.revoke_after_call.load(Ordering::SeqCst) {
                self.authority.denied.store(true, Ordering::SeqCst);
            }
            let package = request
                .engines
                .as_ref()
                .is_some_and(|e| e.iter().any(|n| n == "pypi"));
            AdapterResponse{results:vec![SearchResult{url:if package{"https://pypi.org/project/requests".into()}else{"https://example.test/advisory".into()},title:request.query.clone(),engine:if package{"pypi"}else{"nvd"}.into(),payload:package.then(||json!({"domain":"packages","type":"package","schema_version":1,"provenance":{"engine":"pypi","adapter_fields":["name"],"inferred_fields":[]},"data":{"name":"requests","version":"2.0","repository_url":"https://github.com/psf/requests"}})),..Default::default()}],..Default::default()}
        })
    }
}
struct Harness {
    server: private_valkey::Server,
    loopback: Loopback,
    backend: Arc<Valkey>,
    jobs: ResearchStore,
    sources: Arc<SourceResolver>,
    authority: Arc<Current>,
    engine: Arc<FixtureEngine>,
    search: SearchService,
    runner: ResearchRunner,
}
impl Harness {
    async fn new() -> Self {
        let server = private_valkey::Server::start();
        let loopback = Loopback::new();
        let backend = Arc::new(server.store());
        let authority = Arc::new(Current {
            denied: AtomicBool::new(false),
            clock: AtomicU64::new(now().seconds().to_bits()),
        });
        let snapshots = SnapshotService::new(
            Arc::new(ValkeySnapshots::new(backend.clone())),
            authority.clone(),
            Arc::new(RandomSnapshotIds),
            tenant(),
            SnapshotTtl::new(60).unwrap(),
        );
        let sources = Arc::new(SourceResolver {
            snapshots: snapshots.clone(),
            records: Arc::new(NativeRecords {
                backend: backend.clone(),
                authority: authority.clone(),
            }),
            entities: Arc::new(Groups),
            authority: authority.clone(),
            clock: authority.clone(),
        });
        let jobs = ResearchStore::new(backend.clone(), tenant());
        let engine = Arc::new(FixtureEngine {
            url: loopback.url.clone(),
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            authority: authority.clone(),
            revoke_after_call: AtomicBool::new(false),
        });
        let search = SearchService::new(
            SearchConfig::default(),
            dependencies(vec![
                ("pypi", engine.clone()),
                ("nvd", engine.clone()),
                ("github", engine.clone()),
            ]),
        )
        .unwrap();
        let runner = ResearchRunner::new(
            jobs.clone(),
            search.clone(),
            snapshots,
            RunnerConfig {
                owner: "w08-worker".into(),
                lease_seconds: NonZeroU64::new(3).unwrap(),
                poll_interval: Duration::from_millis(10),
                workers: NonZeroU64::new(1).unwrap(),
                ceilings: ResearchCeilings {
                    queries: 6,
                    engines_per_query: 3,
                    results: 20,
                },
            },
        )
        .unwrap();
        Self {
            server,
            loopback,
            backend,
            jobs,
            sources,
            authority,
            engine,
            search,
            runner,
        }
    }
    fn service(&self, jobs: Arc<dyn DossierJobs>) -> DossierService {
        DossierService {
            jobs,
            sources: self.sources.clone(),
            clock: self.authority.clone(),
            ids: Arc::new(RandomJobIds),
            limits: dossier::Limits {
                queries: 6,
                results: 20,
                max_results: 2,
            },
        }
    }
    async fn source(&self, id: &str) -> ArtifactRef {
        let snapshot = SearchSnapshot::new(
            SnapshotId::new(id).unwrap(),
            "retained seed".into(),
            QueryId::new(format!("query-{id}")).unwrap(),
            vec![SearchResult {
                url: "https://pypi.org/project/requests".into(),
                engine: "pypi".into(),
                ..Default::default()
            }],
            ScopeDecision {
                selected_engines: vec!["pypi".into()],
                ..Default::default()
            },
            tenant(),
            now(),
            Some(WallClockExpiry::new(now().seconds() + 60.).unwrap()),
        )
        .unwrap();
        ValkeySnapshots::new(self.backend.clone())
            .create(&snapshot, NonZeroU64::new(360).unwrap())
            .await
            .unwrap();
        ArtifactRef::new(ArtifactKind::Snapshot, id).unwrap()
    }
    async fn tick(&self, runner: ResearchRunner) -> ResearchJob {
        let (_tx, rx) = watch::channel(false);
        tokio::time::timeout(Duration::from_secs(8), runner.run_once(rx))
            .await
            .unwrap()
            .unwrap()
            .unwrap()
    }
}
fn input<'a>(source: Option<&'a ArtifactRef>, idem: Option<&'a str>) -> Input<'a> {
    Input {
        ecosystem: "pypi",
        package: "Requests",
        version: Some("2.0"),
        repository: None,
        requested_deadline: None,
        deadline: WallClockTimestamp::new(now().seconds() + 30.).unwrap(),
        idempotency_key: idem,
        source,
    }
}
fn card(r: &SearchResult, id: &str) -> Value {
    json!({"result_id":id,"title":r.title,"url":r.url,"payload":r.payload})
}
#[tokio::test]
async fn native_service_executes_existing_worker_and_reads_without_new_calls_or_ttl_renewal() {
    let h = Harness::new().await;
    let source = h.source("seed-a").await;
    let service = h.service(Arc::new(h.jobs.clone()));
    let mut connection = h.server.connection();
    let key = valkey::keys::snapshot(&tenant(), source.id());
    let ttl_before: i64 = redis::cmd("PTTL").arg(&key).query(&mut connection).unwrap();
    let preview = h
        .sources
        .resolve(
            &tenant(),
            &source,
            Destination::DependencyDossier,
            &Cancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(preview.result_ids, vec!["seed-a:0"]);
    assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 0);
    let started = service
        .start(
            &tenant(),
            input(Some(&source), Some("idem-native")),
            &Cancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(started["replay"], false);
    let replay = service
        .start(
            &tenant(),
            input(Some(&source), Some("idem-native")),
            &Cancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(replay["job_id"], started["job_id"]);
    assert_eq!(replay["replay"], true);
    let job = h.tick(service.runner(h.runner.clone())).await;
    assert_eq!(job.state, "succeeded");
    assert_eq!(job.workflow["budget"]["used_adapter_calls"], 2);
    assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 2);
    let report = service
        .get(
            &tenant(),
            &JobId::new(&job.job_id).unwrap(),
            Some(1),
            &card,
            &Cancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(report["resolved_package_identity"]["status"], "resolved");
    assert_eq!(
        report["repository_identity"]["proven_package_ownership"],
        false
    );
    assert_eq!(
        report["sections"]["advisory_leads"]["applicability"]["status"],
        "not_evaluated"
    );
    assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 2);
    let ttl_after: i64 = redis::cmd("PTTL").arg(&key).query(&mut connection).unwrap();
    assert!(ttl_after > 0 && ttl_after <= ttl_before);
    h.search.shutdown().await;
}
struct BarrierJobs {
    jobs: ResearchStore,
    persisted: Semaphore,
    release: Semaphore,
}
impl DossierJobs for BarrierJobs {
    fn create<'a>(
        &'a self,
        tenant: &'a TenantId,
        candidate: &'a ResearchJob,
    ) -> PortFuture<'a, Result<(ResearchJob, bool), composition::Error>> {
        Box::pin(async move {
            let admitted =
                <ResearchStore as DossierJobs>::create(&self.jobs, tenant, candidate).await?;
            self.persisted.add_permits(1);
            self.release.acquire().await.unwrap().forget();
            Ok(admitted)
        })
    }
    fn load<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a JobId,
    ) -> PortFuture<'a, Result<Option<ResearchJob>, composition::Error>> {
        <ResearchStore as DossierJobs>::load(&self.jobs, tenant, id)
    }
}
#[tokio::test]
async fn expiry_during_atomic_create_is_persisted_but_worker_denies_without_charge() {
    let h = Harness::new().await;
    let source = h.source("seed-expiry").await;
    let barrier = Arc::new(BarrierJobs {
        jobs: h.jobs.clone(),
        persisted: Semaphore::new(0),
        release: Semaphore::new(0),
    });
    let service = h.service(barrier.clone());
    let cancel = Cancellation::default();
    let t = tenant();
    let start = service.start(&t, input(Some(&source), Some("expiry-race")), &cancel);
    tokio::pin!(start);
    tokio::select! {r=&mut start=>panic!("admission bypassed barrier: {r:?}"),p=barrier.persisted.acquire()=>p.unwrap().forget()};
    let persisted = match h.jobs.find_by_idempotency("expiry-race").await {
        JobRead::Found(job) => job,
        other => panic!("atomic create missing: {other:?}"),
    };
    assert_eq!(persisted.state, "queued");
    h.authority
        .clock
        .store((now().seconds() + 120.).to_bits(), Ordering::SeqCst);
    barrier.release.add_permits(1);
    assert_eq!(start.await.unwrap_err().code, "source_expired");
    let denied = h.tick(service.runner(h.runner.clone())).await;
    assert_eq!(denied.state, "failed");
    assert_eq!(
        denied.stop_reason.as_deref(),
        Some("source_rejected: source_expired")
    );
    assert_eq!(denied.budget_used["attempts"], 0);
    assert_eq!(denied.workflow["budget"]["used_adapter_calls"], 0);
    assert!(denied.queries.iter().all(|q| q.attempts.is_empty()));
    assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 0);
    h.search.shutdown().await;
}
#[tokio::test]
async fn source_revocation_after_first_query_blocks_next_query() {
    let h = Harness::new().await;
    let source = h.source("seed-revocation").await;
    let service = h.service(Arc::new(h.jobs.clone()));
    service
        .start(
            &tenant(),
            input(Some(&source), None),
            &Cancellation::default(),
        )
        .await
        .unwrap();
    h.engine.revoke_after_call.store(true, Ordering::SeqCst);
    let job = h.tick(service.runner(h.runner.clone())).await;
    assert_eq!(job.state, "partial");
    assert_eq!(
        job.stop_reason.as_deref(),
        Some("source_rejected: source_policy_denied")
    );
    assert_eq!(job.budget_used["attempts"], 1);
    assert_eq!(job.workflow["budget"]["used_adapter_calls"], 1);
    assert!(job.queries[1].attempts.is_empty());
    assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 1);
    h.search.shutdown().await;
}
#[tokio::test]
async fn restart_without_validator_and_malformed_or_tampered_binding_fail_closed() {
    for mode in [
        "unwired",
        "null",
        "details",
        "result_ids",
        "query",
        "foreign",
    ] {
        let h = Harness::new().await;
        let source = h.source("seed-binding").await;
        let resolved = h
            .sources
            .resolve(
                &tenant(),
                &source,
                Destination::DependencyDossier,
                &Cancellation::default(),
            )
            .await
            .unwrap();
        let mut candidate = dossier::plan(
            dossier::Identity::new("pypi", "requests", None, None, None).unwrap(),
            &tenant(),
            "job-binding",
            now(),
            WallClockTimestamp::new(now().seconds() + 30.).unwrap(),
            None,
            Some(&resolved),
            &dossier::Limits {
                queries: 6,
                results: 20,
                max_results: 2,
            },
        )
        .unwrap();
        match mode {
            "null" => {
                candidate.workflow.insert("composition".into(), Value::Null);
            }
            "details" => {
                candidate.workflow.get_mut("composition").unwrap()["source"]["details"]["query_id"] =
                    json!("forged")
            }
            "result_ids" => {
                candidate.workflow.get_mut("composition").unwrap()["source"]["result_ids"] =
                    json!([])
            }
            "query" => {
                candidate.workflow.get_mut("composition").unwrap()["source"]["query"] =
                    json!("forged")
            }
            "foreign" => {
                candidate.tenant = "tenant-b".into();
            }
            _ => {}
        }
        h.jobs
            .for_tenant(TenantId::new(&candidate.tenant).unwrap())
            .create(&candidate)
            .await
            .unwrap();
        let service = h.service(Arc::new(h.jobs.clone()));
        let runner = if mode == "unwired" {
            h.runner.clone()
        } else {
            service.runner(h.runner.clone())
        };
        let denied = h.tick(runner).await;
        assert_eq!(denied.state, "failed", "{mode}");
        assert!(
            denied
                .stop_reason
                .as_deref()
                .unwrap()
                .starts_with("source_rejected:"),
            "{mode}"
        );
        assert_eq!(denied.budget_used["attempts"], 0, "{mode}");
        assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 0, "{mode}");
        h.search.shutdown().await;
    }
}
#[tokio::test]
async fn concurrent_idempotency_with_different_source_never_adopts_other_binding() {
    let h = Harness::new().await;
    let a = h.source("seed-race-a").await;
    let b = h.source("seed-race-b").await;
    let service = h.service(Arc::new(h.jobs.clone()));
    let cancel = Cancellation::default();
    let t = tenant();
    let (a, b) = tokio::join!(
        service.start(&t, input(Some(&a), Some("same-key")), &cancel),
        service.start(&t, input(Some(&b), Some("same-key")), &cancel)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let error = if let Err(e) = a { e } else { b.unwrap_err() };
    assert_eq!(error.code, "idempotency_conflict");
    assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 0);
    h.search.shutdown().await;
}
fn dependencies(engines: Vec<(&str, Arc<dyn SearchEngine>)>) -> SearchDependencies {
    SearchDependencies {
        policy: PolicyService {
            policy: policy(false).policy,
            catalog: CatalogSnapshot::new(engines.iter().map(|(n, _)| capability(n)).collect())
                .unwrap(),
            resolver: ScopeResolver {
                catalog_routing: false,
                ..Default::default()
            },
        },
        engines: engines
            .into_iter()
            .map(|(n, engine)| {
                (
                    n.into(),
                    EngineBinding {
                        engine,
                        generation: "fixture-v1".into(),
                        timeout: Duration::from_secs(3),
                        scrape: false,
                    },
                )
            })
            .collect(),
        routing_identity: RoutingIdentity {
            catalog: None,
            sensitive: ["hibp", "dehashed"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            tier1: ScopeResolver::default().tier1,
            budget: None,
            ranking_strategy: "presence".into(),
            work_group_version: "scholarly-work-v3".into(),
            rerank_identity: None,
            routing_extension: None,
        },
        cache: None,
        client_admission: None,
        suggestions: None,
        jev: None,
        routing_cache: None,
        metrics: Arc::new(NoopSink),
        supervisor: Arc::new(NoopSink),
        audit: Arc::new(NoopSink),
        cooldowns: None,
        owned_http: vec![],
        owned_stores: vec![],
        clock: Arc::new(SystemSearchClock),
        reranker: None,
    }
}
struct DispatchBarrier {
    native: NativeSourceValidator,
    calls: AtomicUsize,
    entered: Semaphore,
    release: Semaphore,
}
impl SourceValidator for DispatchBarrier {
    fn validate<'a>(
        &'a self,
        tenant: &'a TenantId,
        job: &'a ResearchJob,
    ) -> PortFuture<'a, Result<(), composition::Error>> {
        Box::pin(async move {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if call == 1 {
                self.entered.add_permits(1);
                self.release.acquire().await.unwrap().forget();
            }
            self.native.validate(tenant, job).await
        })
    }
}
#[tokio::test]
async fn revocation_in_reserved_renewal_gap_blocks_actual_dispatch_and_conserves_reservation() {
    let h = Harness::new().await;
    let source = h.source("seed-dispatch").await;
    let service = h.service(Arc::new(h.jobs.clone()));
    let started = service
        .start(
            &tenant(),
            input(Some(&source), None),
            &Cancellation::default(),
        )
        .await
        .unwrap();
    let id = JobId::new(started["job_id"].as_str().unwrap()).unwrap();
    let barrier = Arc::new(DispatchBarrier {
        native: NativeSourceValidator {
            sources: h.sources.clone(),
        },
        calls: AtomicUsize::new(0),
        entered: Semaphore::new(0),
        release: Semaphore::new(0),
    });
    let runner = h.runner.clone().with_source_validator(barrier.clone());
    let (_tx, rx) = watch::channel(false);
    let execute = runner.run_once(rx);
    tokio::pin!(execute);
    tokio::select! {r=&mut execute=>panic!("worker bypassed dispatch barrier: {r:?}"),p=barrier.entered.acquire()=>p.unwrap().forget()};
    let reserved = match h.jobs.load(&id).await {
        JobRead::Found(job) => job,
        other => panic!("reservation missing: {other:?}"),
    };
    assert_eq!(reserved.queries[0].state, "running");
    assert_eq!(reserved.budget_used["attempts"], 1);
    assert_eq!(reserved.workflow["budget"]["used_adapter_calls"], 1);
    assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 0);
    h.authority.denied.store(true, Ordering::SeqCst);
    barrier.release.add_permits(1);
    let job = execute.await.unwrap().unwrap();
    assert_eq!(job.state, "failed");
    assert_eq!(
        job.stop_reason.as_deref(),
        Some("source_rejected_before_dispatch: source_policy_denied")
    );
    assert_eq!(job.budget_used["attempts"], 1);
    assert_eq!(job.workflow["budget"]["used_adapter_calls"], 1);
    assert_eq!(job.queries[0].attempts[0].state, "failed");
    assert!(job.queries[0].attempts[0].cursor.is_none());
    assert!(job.queries[0].attempts[0].admitted_result_ids.is_empty());
    assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 0);
    h.search.shutdown().await;
}
