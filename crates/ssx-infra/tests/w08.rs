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
        snapshot::{RandomSnapshotIds, ValkeySnapshots},
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
    revoked: AtomicUsize,
    clock: AtomicU64,
    foreign: AtomicBool,
    reports_bound: AtomicUsize,
}
impl Current {
    fn policy(&self) -> PolicyService {
        let mut p = policy(self.denied.load(Ordering::SeqCst));
        match self.revoked.load(Ordering::SeqCst) {
            1 => {
                p.policy.grants.remove("dependency_dossier");
            }
            2 => {
                p.policy.grants.remove("research");
            }
            3 => {
                p.policy.grants.remove("security");
            }
            6 => {
                p.catalog = CatalogSnapshot::new(
                    ["pypi", "github", "nvd"]
                        .into_iter()
                        .map(|name| {
                            let mut c = capability(name);
                            if name == "github" {
                                c.enabled = false;
                            }
                            c
                        })
                        .collect(),
                )
                .unwrap();
            }
            5 => {
                p.policy.grants.remove("saved_searches");
            }
            4 => {
                p.catalog = CatalogSnapshot::new(
                    ["pypi", "github", "nvd"]
                        .into_iter()
                        .map(|name| {
                            let mut c = capability(name);
                            if name == "pypi" {
                                c.enabled = false;
                            }
                            c
                        })
                        .collect(),
                )
                .unwrap();
            }
            _ => {}
        }
        p
    }
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
            tenant: Some(if self.foreign.load(Ordering::SeqCst) {
                TenantId::new("tenant-b").unwrap()
            } else {
                tenant()
            }),
            policy: self.policy(),
            staged_limits: ssx_application::staged::Limits {
                granted: true,
                ..Default::default()
            },
            saved_max_reports: self.reports_bound.load(Ordering::SeqCst),
        }
    }
}
impl RecordsAuthority for Current {
    fn current(&self) -> RecordsAuthoritySnapshot {
        RecordsAuthoritySnapshot {
            search: self.policy(),
            limits: ssx_application::saved::SavedLimits {
                reports: self.reports_bound.load(Ordering::SeqCst) as u64,
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
                        // Darwin inherits O_NONBLOCK from the listener. A timeout
                        // does not clear it; explicitly select bounded blocking I/O.
                        socket.set_nonblocking(false).unwrap();
                        let deadline = std::time::Instant::now() + Duration::from_secs(2);
                        let mut bytes = [0u8; 4096];
                        let mut n = 0;
                        loop {
                            let remaining = deadline
                                .checked_duration_since(std::time::Instant::now())
                                .expect("loopback HTTP header deadline");
                            socket.set_read_timeout(Some(remaining)).unwrap();
                            assert!(n < bytes.len(), "loopback HTTP header exceeds bound");
                            match socket.read(&mut bytes[n..]) {
                                Ok(0) => panic!("loopback closed before complete HTTP header"),
                                Ok(read) => n += read,
                                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                                    continue;
                                }
                                Err(error) => panic!("loopback HTTP header: {error}"),
                            }
                            if bytes[..n].windows(4).any(|part| part == b"\r\n\r\n") {
                                break;
                            }
                        }
                        assert!(bytes[..n].starts_with(b"GET /fixture HTTP/1.1\r\n"));
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
    fn assert_healthy(&self) {
        assert!(
            !self.thread.as_ref().unwrap().is_finished(),
            "loopback fixture thread terminated before cleanup"
        );
    }
}
impl Drop for Loopback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Err(error) = self.thread.take().unwrap().join() {
            // Preserve the original test panic without a second panic/abort.
            // On an ordinary path fixture failure must still fail the test.
            if !std::thread::panicking() {
                std::panic::resume_unwind(error);
            }
        }
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
        assert!(
            server.url().strip_prefix("unix://").is_some_and(|path| {
                std::path::Path::new(path).starts_with(std::env::temp_dir())
            })
        );
        let loopback = Loopback::new();
        let backend = Arc::new(server.store());
        let authority = Arc::new(Current {
            denied: AtomicBool::new(false),
            revoked: AtomicUsize::new(0),
            foreign: AtomicBool::new(false),
            reports_bound: AtomicUsize::new(2),
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
            saved_reports: Arc::new(saved_reports::NativeSavedReports {
                backend: backend.clone(),
                authority: authority.clone(),
                principal: authority.clone(),
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
        let job = tokio::time::timeout(Duration::from_secs(8), runner.run_once(rx))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        self.loopback.assert_healthy();
        job
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
#[tokio::test]
async fn each_destination_grant_and_source_engine_revocation_blocks_unreserved_work() {
    for mode in 1..=4 {
        let h = Harness::new().await;
        let source = h.source("seed-selective").await;
        let service = h.service(Arc::new(h.jobs.clone()));
        service
            .start(
                &tenant(),
                input(Some(&source), None),
                &Cancellation::default(),
            )
            .await
            .unwrap();
        h.authority.revoked.store(mode, Ordering::SeqCst);
        // Selective revocation leaves all other grants intact; this is not an
        // all-disabled fixture that could hide one missing destination requirement.
        let current = h.authority.policy();
        for (index, grant) in ["dependency_dossier", "research", "security"]
            .into_iter()
            .enumerate()
        {
            assert_eq!(current.policy.grants.contains(grant), mode != index + 1);
        }
        let denied = h.tick(service.runner(h.runner.clone())).await;
        assert_eq!(denied.state, "failed", "mode{mode}");
        assert_eq!(
            denied.stop_reason.as_deref(),
            Some("source_rejected: source_policy_denied"),
            "mode{mode}"
        );
        assert_eq!(denied.budget_used["attempts"], 0);
        assert_eq!(denied.workflow["budget"]["used_adapter_calls"], 0);
        assert!(denied.queries.iter().all(|q| q.attempts.is_empty()));
        assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 0);
        h.search.shutdown().await;
    }
}
async fn saved_source_fixture(
    h: &Harness,
) -> (ArtifactRef, ssx_domain::saved::SavedDefinition, Vec<String>) {
    use ssx_application::saved::{SavedPolicy, prepare_create};
    use ssx_infra::valkey::saved::SavedStore;
    let store = SavedStore::new(h.backend.clone(), tenant());
    let current = RecordsAuthority::current(h.authority.as_ref());
    let policy = SavedPolicy {
        search: &current.search,
        saved_granted: true,
        auth_configured: &current.auth_configured,
        limits: &current.limits,
    };
    let prepared=prepare_create(&policy,tenant().as_str(),"saved-source",&json!({"query":"current definition query","engines":["pypi"],"interval_seconds":60,"start_immediately":true,"expires_in_seconds":600,"max_reports":2}),store.now().await.unwrap()).unwrap();
    store.create(&prepared).await.unwrap();
    let definition = store
        .load(&SavedSearchId::new("saved-source").unwrap())
        .await
        .unwrap()
        .unwrap();
    let report = json!({"search_id":"saved-source","run_id":"run-source","status":"changed","finished_at":definition.created_at.get(),"expires_at":definition.expires_at.get(),"revision":1,"incomparable_reasons":[],"observation":{"window":{"query":"historical source query","engines":["pypi"]}}});
    let definition_key =
        valkey::keys::saved("definition", &tenant(), Some("saved-source")).unwrap();
    let index = valkey::keys::saved("reports", &tenant(), Some("saved-source")).unwrap();
    let report_key =
        valkey::keys::saved("report", &tenant(), Some("saved-source:run-source")).unwrap();
    let mut connection = h.server.connection();
    redis::cmd("SETEX")
        .arg(&report_key)
        .arg(600)
        .arg(ssx_json::to_string(&report).unwrap())
        .query::<()>(&mut connection)
        .unwrap();
    redis::cmd("ZADD")
        .arg(&index)
        .arg(definition.created_at.get())
        .arg("run-source")
        .query::<()>(&mut connection)
        .unwrap();
    redis::cmd("EXPIRE")
        .arg(&index)
        .arg(600)
        .query::<()>(&mut connection)
        .unwrap();
    let artifact = ArtifactRef::new(
        ArtifactKind::SavedReport,
        ssx_domain::canonical::composite_artifact_id(&["saved-source", "run-source"]).unwrap(),
    )
    .unwrap();
    (
        artifact,
        definition,
        vec![definition_key, index, report_key],
    )
}
fn source_with_saved(h: &Harness, reports: Arc<dyn composition::SavedReports>) -> SourceResolver {
    SourceResolver {
        snapshots: h.sources.snapshots.clone(),
        records: h.sources.records.clone(),
        saved_reports: reports,
        entities: h.sources.entities.clone(),
        authority: h.sources.authority.clone(),
        clock: h.sources.clock.clone(),
    }
}
struct LegacyLineageProjection {
    records: Arc<NativeRecords>,
}
impl composition::SavedReports for LegacyLineageProjection {
    fn selected<'a>(
        &'a self,
        tenant: &'a TenantId,
        definition: &'a ssx_domain::saved::SavedDefinition,
        run: &'a str,
        limit: usize,
        cancel: &'a Cancellation,
    ) -> PortFuture<
        'a,
        Result<ssx_application::lineage::workflows::Read<Value>, ssx_application::lineage::Failure>,
    > {
        Box::pin(async move {
            use ssx_application::lineage::workflows::{Read, Records};
            match self
                .records
                .reports(tenant, definition, now(), limit, Some(run), cancel)
                .await?
            {
                Read::Found(rows) => Ok(rows
                    .into_iter()
                    .find(|r| r["run_id"].as_str() == Some(run))
                    .map(Read::Found)
                    .unwrap_or(Read::Missing)),
                Read::Missing => Ok(Read::Missing),
                Read::Expired(t) => Ok(Read::Expired(t)),
                Read::Unavailable => Ok(Read::Unavailable),
                Read::PolicyDenied => Ok(Read::PolicyDenied),
            }
        })
    }
}
#[tokio::test]
async fn real_saved_report_composition_uses_full_authority_and_rejects_lossy_lineage_mutant() {
    let h = Harness::new().await;
    let (reference, definition, keys) = saved_source_fixture(&h).await;
    let mut connection = h.server.connection();
    let before = keys
        .iter()
        .map(|key| {
            (
                redis::cmd("DUMP")
                    .arg(key)
                    .query::<Option<Vec<u8>>>(&mut connection)
                    .unwrap(),
                redis::cmd("PTTL")
                    .arg(key)
                    .query::<i64>(&mut connection)
                    .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    for destination in [Destination::Research, Destination::StagedSearch] {
        let source = h
            .sources
            .resolve(&tenant(), &reference, destination, &Cancellation::default())
            .await
            .unwrap();
        assert_eq!(source.query.as_deref(), Some("historical source query"));
        assert_eq!(source.details["revision"], 1);
        assert_ne!(source.query.as_deref(), Some(definition.query.as_str()));
    }
    // This controlled mutant uses the exact accepted NativeRecords projection
    // that d543 mistakenly treated as a complete composition source.
    let mutant = source_with_saved(
        &h,
        Arc::new(LegacyLineageProjection {
            records: Arc::new(NativeRecords {
                backend: h.backend.clone(),
                authority: h.authority.clone(),
            }),
        }),
    );
    assert!(
        mutant
            .resolve(
                &tenant(),
                &reference,
                Destination::Research,
                &Cancellation::default()
            )
            .await
            .is_err()
    );
    for (index, key) in keys.iter().enumerate() {
        let after = redis::cmd("DUMP")
            .arg(key)
            .query::<Option<Vec<u8>>>(&mut connection)
            .unwrap();
        assert_eq!(after, before[index].0);
        let ttl: i64 = redis::cmd("PTTL").arg(key).query(&mut connection).unwrap();
        assert!(ttl > 0 && ttl <= before[index].1);
    }
    assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 0);
    h.search.shutdown().await;
}
struct AfterSavedRead {
    inner: Arc<dyn composition::SavedReports>,
    current: Arc<Current>,
    mode: &'static str,
}
impl composition::SavedReports for AfterSavedRead {
    fn selected<'a>(
        &'a self,
        tenant: &'a TenantId,
        definition: &'a ssx_domain::saved::SavedDefinition,
        run: &'a str,
        limit: usize,
        cancel: &'a Cancellation,
    ) -> PortFuture<
        'a,
        Result<ssx_application::lineage::workflows::Read<Value>, ssx_application::lineage::Failure>,
    > {
        Box::pin(async move {
            let read = self
                .inner
                .selected(tenant, definition, run, limit, cancel)
                .await?;
            match self.mode {
                "tenant" => self.current.foreign.store(true, Ordering::SeqCst),
                "bound" => self.current.reports_bound.store(1, Ordering::SeqCst),
                "policy" => self.current.revoked.store(5, Ordering::SeqCst),
                _ => {}
            }
            Ok(read)
        })
    }
}
#[tokio::test]
async fn real_saved_report_errors_and_awaited_authority_changes_are_readonly() {
    for mode in [
        "malformed",
        "incomparable",
        "truncated",
        "foreign",
        "expired",
        "policy",
        "bound",
        "tenant",
        "unavailable",
    ] {
        let h = Harness::new().await;
        let (mut reference, definition, keys) = saved_source_fixture(&h).await;
        let mut connection = h.server.connection();
        let key = &keys[2];
        let raw: String = redis::cmd("GET").arg(key).query(&mut connection).unwrap();
        let mut report: Value = ssx_json::from_str(&raw).unwrap();
        match mode {
            "malformed" => report["incomparable_reasons"] = json!("bad"),
            "incomparable" => report["status"] = json!("incomparable"),
            "truncated" => {
                report["status"] = json!("incomparable");
                report["incomparable_reasons"] = json!(["observation_truncated"]);
            }
            "expired" => report["expires_at"] = json!(now().seconds() - 1.),
            "foreign" => {
                reference = ArtifactRef::new(
                    ArtifactKind::SavedReport,
                    ssx_domain::canonical::composite_artifact_id(&["foreign-source", "run-source"])
                        .unwrap(),
                )
                .unwrap()
            }
            _ => {}
        }
        if mode == "foreign" {
            let other = TenantId::new("tenant-b").unwrap();
            let mut foreign = definition.clone();
            foreign.tenant = other.as_str().into();
            foreign.search_id = "foreign-source".into();
            let mut foreign_report = report.clone();
            foreign_report["search_id"] = json!("foreign-source");
            redis::cmd("SETEX")
                .arg(valkey::keys::saved("definition", &other, Some("foreign-source")).unwrap())
                .arg(600)
                .arg(ssx_json::to_string(&foreign.stored_payload().unwrap()).unwrap())
                .query::<()>(&mut connection)
                .unwrap();
            redis::cmd("SETEX")
                .arg(
                    valkey::keys::saved("report", &other, Some("foreign-source:run-source"))
                        .unwrap(),
                )
                .arg(600)
                .arg(ssx_json::to_string(&foreign_report).unwrap())
                .query::<()>(&mut connection)
                .unwrap();
            redis::cmd("ZADD")
                .arg(valkey::keys::saved("reports", &other, Some("foreign-source")).unwrap())
                .arg(foreign.created_at.get())
                .arg("run-source")
                .query::<()>(&mut connection)
                .unwrap();
        }
        redis::cmd("SETEX")
            .arg(key)
            .arg(600)
            .arg(ssx_json::to_string(&report).unwrap())
            .query::<()>(&mut connection)
            .unwrap();
        if mode == "unavailable" {
            h.backend.close().await;
        }
        let resolver = if ["policy", "bound", "tenant"].contains(&mode) {
            source_with_saved(
                &h,
                Arc::new(AfterSavedRead {
                    inner: h.sources.saved_reports.clone(),
                    current: h.authority.clone(),
                    mode,
                }),
            )
        } else {
            source_with_saved(&h, h.sources.saved_reports.clone())
        };
        let error = resolver
            .resolve(
                &tenant(),
                &reference,
                Destination::Research,
                &Cancellation::default(),
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.code,
            match mode {
                "malformed" => "source_malformed",
                "incomparable" => "source_incomparable",
                "truncated" => "source_truncated",
                "foreign" | "expired" => "source_not_found",
                "unavailable" => "source_unavailable",
                _ => "source_policy_denied",
            },
            "{mode}"
        );
        if mode == "foreign" {
            h.authority.foreign.store(true, Ordering::SeqCst);
            let allowed = resolver
                .resolve(
                    &TenantId::new("tenant-b").unwrap(),
                    &reference,
                    Destination::Research,
                    &Cancellation::default(),
                )
                .await
                .unwrap();
            assert_eq!(allowed.query.as_deref(), Some("historical source query"));
        }
        let after: String = redis::cmd("GET").arg(key).query(&mut connection).unwrap();
        assert_eq!(after, ssx_json::to_string(&report).unwrap());
        assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 0);
        h.search.shutdown().await;
    }
}
struct ClockAfterSnapshotRead {
    inner: ValkeySnapshots,
    current: Arc<Current>,
    id: String,
    advance: f64,
}
impl SnapshotRepository for ClockAfterSnapshotRead {
    fn create<'a>(
        &'a self,
        _: &'a SearchSnapshot,
        _: NonZeroU64,
    ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
        Box::pin(async { panic!("source read must not create") })
    }
    fn read<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a SnapshotId,
    ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
        Box::pin(async move {
            let read = self.inner.read(tenant, id).await?;
            if id.as_str() == self.id {
                self.current
                    .clock
                    .store(self.advance.to_bits(), Ordering::SeqCst);
            }
            Ok(read)
        })
    }
}
#[tokio::test]
async fn real_staged_expiry_during_selected_snapshot_read_cannot_compose() {
    let h = Harness::new().await;
    let accepted = now().seconds();
    let selected = SearchSnapshot::new(
        SnapshotId::new("stage-selected").unwrap(),
        "staged query".into(),
        QueryId::new("selected-query-id").unwrap(),
        vec![SearchResult {
            engine: "pypi".into(),
            url: "https://pypi.org/project/requests".into(),
            ..Default::default()
        }],
        ScopeDecision {
            selected_engines: vec!["pypi".into()],
            ..Default::default()
        },
        tenant(),
        now(),
        Some(WallClockExpiry::new(accepted + 7200.).unwrap()),
    )
    .unwrap();
    ValkeySnapshots::new(h.backend.clone())
        .create(&selected, NonZeroU64::new(7500).unwrap())
        .await
        .unwrap();
    let limits = ssx_application::staged::Limits {
        granted: true,
        ..Default::default()
    };
    let plan = ssx_application::staged::Plan::build(
        &policy(false),
        &limits,
        &ssx_application::staged::Input {
            query: "staged query".into(),
            objectives: json!({"deadline_ms":1000,"max_engine_calls":1}),
            initial_scope: json!({"engines":["pypi"]}),
            fallback_scope: None,
            allow_scope_expansion: false,
            filters: None,
        },
    )
    .unwrap();
    let record = plan
        .record(
            &tenant(),
            &StagedSearchId::new("stage-expiry").unwrap(),
            accepted,
        )
        .unwrap();
    let mut value = record.value().clone();
    value["state"] = json!("completed");
    value["selected_result_attempt_id"] = json!("selected-attempt");
    value["budget"]["reserved"] = json!(1);
    value["budget"]["observed"] = json!(1);
    let stage = &mut value["stages"].as_array_mut().unwrap()[0];
    stage["state"] = json!("done");
    stage["attempts"] = json!([{"attempt_id":"selected-attempt","cursor":"stage-selected","state":"done","started_at":accepted,"finished_at":accepted+1.,"reserved_engine_calls":1,"observed_engine_calls":1,"engine_outcomes":[],"enforcement":stage["enforcement"]}]);
    let record = ssx_domain::staged::Record::from_value(value).unwrap();
    let stage_key = valkey::keys::staged("record", &tenant(), Some("stage-expiry")).unwrap();
    let snapshot_key = valkey::keys::snapshot(&tenant(), "stage-selected");
    let mut connection = h.server.connection();
    redis::cmd("SETEX")
        .arg(&stage_key)
        .arg(7500)
        .arg(ssx_json::to_string(record.value()).unwrap())
        .query::<()>(&mut connection)
        .unwrap();
    let before: [i64; 2] = [
        redis::cmd("PTTL")
            .arg(&stage_key)
            .query(&mut connection)
            .unwrap(),
        redis::cmd("PTTL")
            .arg(&snapshot_key)
            .query(&mut connection)
            .unwrap(),
    ];
    let mut resolver = source_with_saved(&h, h.sources.saved_reports.clone());
    resolver.snapshots = SnapshotService::new(
        Arc::new(ClockAfterSnapshotRead {
            inner: ValkeySnapshots::new(h.backend.clone()),
            current: h.authority.clone(),
            id: "stage-selected".into(),
            advance: accepted + 3601.,
        }),
        h.authority.clone(),
        Arc::new(RandomSnapshotIds),
        tenant(),
        SnapshotTtl::new(60).unwrap(),
    );
    let reference = ArtifactRef::new(ArtifactKind::StagedSearch, "stage-expiry").unwrap();
    assert_eq!(
        resolver
            .resolve(
                &tenant(),
                &reference,
                Destination::ResearchManifest,
                &Cancellation::default()
            )
            .await
            .unwrap_err()
            .code,
        "source_expired"
    );
    assert!(Clock::now(h.authority.as_ref()).seconds() > record.expires_at());
    assert!(
        selected.expires_at().unwrap().timestamp().seconds()
            > Clock::now(h.authority.as_ref()).seconds()
    );
    let after: [i64; 2] = [
        redis::cmd("PTTL")
            .arg(&stage_key)
            .query(&mut connection)
            .unwrap(),
        redis::cmd("PTTL")
            .arg(&snapshot_key)
            .query(&mut connection)
            .unwrap(),
    ];
    assert!(after.iter().zip(before).all(|(a, b)| *a > 0 && *a <= b));
    assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 0);
    h.search.shutdown().await;
}
#[tokio::test]
async fn dossier_get_preserves_real_missing_expired_unavailable_snapshot_states_without_dispatch() {
    for mode in ["missing", "expired", "unavailable"] {
        let h = Harness::new().await;
        let mut service = h.service(Arc::new(h.jobs.clone()));
        service
            .start(&tenant(), input(None, None), &Cancellation::default())
            .await
            .unwrap();
        let job = h.tick(service.runner(h.runner.clone())).await;
        assert_eq!(job.state, "succeeded");
        let cursor = job.queries[0].cursor.as_deref().unwrap();
        match mode {
            "missing" => {
                redis::cmd("DEL")
                    .arg(valkey::keys::snapshot(&tenant(), cursor))
                    .query::<()>(&mut h.server.connection())
                    .unwrap();
            }
            "expired" => {
                h.authority
                    .clock
                    .store((now().seconds() + 120.).to_bits(), Ordering::SeqCst);
            }
            "unavailable" => {
                let backend = Arc::new(h.server.store());
                backend.close().await;
                let mut resolver = source_with_saved(&h, h.sources.saved_reports.clone());
                resolver.snapshots = SnapshotService::new(
                    Arc::new(ValkeySnapshots::new(backend)),
                    h.authority.clone(),
                    Arc::new(RandomSnapshotIds),
                    tenant(),
                    SnapshotTtl::new(60).unwrap(),
                );
                service.sources = Arc::new(resolver);
            }
            _ => {}
        }
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
        assert_eq!(
            report["sections"]["package_information"]["state"],
            match mode {
                "missing" => "failed",
                "expired" => "expired",
                _ => "unavailable",
            },
            "{mode}"
        );
        assert_eq!(report["partial"], true);
        assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 2);
        h.search.shutdown().await;
    }
}
struct DropWitness(Arc<AtomicBool>);
impl Drop for DropWitness {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
struct CancelTokens;
impl valkey::TokenSource<JobId> for CancelTokens {
    fn next_token(&mut self) -> LeaseToken<JobId> {
        LeaseToken::new("w08-cancel-token").unwrap()
    }
}
struct BlockingSource {
    entered: Semaphore,
    dropped: Arc<AtomicBool>,
}
impl SourceValidator for BlockingSource {
    fn validate<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a ResearchJob,
    ) -> PortFuture<'a, Result<(), composition::Error>> {
        Box::pin(async move {
            let _drop = DropWitness(self.dropped.clone());
            self.entered.add_permits(1);
            std::future::pending::<()>().await;
            unreachable!()
        })
    }
}
#[tokio::test]
async fn blocked_source_validation_observes_shutdown_cancel_deadline_and_owner_loss_without_dispatch()
 {
    for mode in ["shutdown", "cancel", "deadline", "cap", "owner-loss"] {
        let h = Harness::new().await;
        let source = h.source("source-blocked").await;
        let service = h.service(Arc::new(h.jobs.clone()));
        let mut args = input(Some(&source), None);
        if mode == "deadline" {
            args.deadline = WallClockTimestamp::new(now().seconds() + 10.).unwrap();
        }
        if mode == "cap" {
            args.deadline = WallClockTimestamp::new(now().seconds() + 120.).unwrap();
        }
        let requested_deadline = args.deadline;

        let started = service
            .start(&tenant(), args, &Cancellation::default())
            .await
            .unwrap();
        let id = JobId::new(started["job_id"].as_str().unwrap()).unwrap();
        let dropped = Arc::new(AtomicBool::new(false));
        let blocked = Arc::new(BlockingSource {
            entered: Semaphore::new(0),
            dropped: dropped.clone(),
        });
        let runner = h.runner.clone().with_source_validator(blocked.clone());
        let (stop, rx) = watch::channel(false);
        let execute = runner.run_once(rx);
        tokio::pin!(execute);
        tokio::select! {r=&mut execute=>panic!("validation did not block: {r:?}"),p=blocked.entered.acquire()=>p.unwrap().forget()};
        match mode {
            "shutdown" => {
                stop.send_replace(true);
            }
            "cancel" => {
                h.jobs
                    .request_cancel(&id, now(), &mut CancelTokens, || {
                        AttemptId::new("cancel-attempt").unwrap()
                    })
                    .await
                    .unwrap();
            }
            "deadline" | "cap" => {
                let persisted = match h.jobs.load(&id).await {
                    JobRead::Found(job) => job,
                    other => panic!("persisted horizon missing: {other:?}"),
                };
                assert_eq!(persisted.deadline.get(), requested_deadline.seconds());
                tokio::time::pause();
                tokio::time::advance(Duration::from_secs(if mode == "deadline" {
                    11
                } else {
                    31
                }))
                .await;
                tokio::time::resume();
            }

            "owner-loss" => {
                redis::cmd("SETEX")
                    .arg(valkey::keys::research("lease", &tenant(), id.as_str()).unwrap())
                    .arg(30)
                    .arg("replacement-owner-token")
                    .query::<()>(&mut h.server.connection())
                    .unwrap();
            }
            _ => {}
        }
        let result = tokio::time::timeout(
            Duration::from_secs(if mode == "deadline" || mode == "cap" {
                2
            } else {
                8
            }),
            &mut execute,
        )
        .await
        .unwrap();
        assert!(
            dropped.load(Ordering::SeqCst),
            "readonly future was not dropped: {mode}"
        );
        let stored = match h.jobs.load(&id).await {
            JobRead::Found(job) => job,
            other => panic!("job unavailable: {other:?}"),
        };
        assert_eq!(stored.budget_used["attempts"], 0, "{mode}");
        assert_eq!(stored.workflow["budget"]["used_adapter_calls"], 0, "{mode}");
        assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 0, "{mode}");
        match mode {
            "shutdown" | "owner-loss" => assert!(result.is_err(), "{mode}"),
            "cancel" => assert_eq!(result.unwrap().unwrap().state, "cancelled"),
            "deadline" | "cap" => assert!(
                result
                    .unwrap()
                    .unwrap()
                    .stop_reason
                    .as_deref()
                    .unwrap()
                    .contains("source_")
            ),
            _ => {}
        }
        if mode == "owner-loss" {
            let lease: String = redis::cmd("GET")
                .arg(valkey::keys::research("lease", &tenant(), id.as_str()).unwrap())
                .query(&mut h.server.connection())
                .unwrap();
            assert_eq!(lease, "replacement-owner-token");
        }
        h.search.shutdown().await;
    }
}
struct OperatorFull;
impl RecordsAuthority for OperatorFull {
    fn current(&self) -> RecordsAuthoritySnapshot {
        RecordsAuthoritySnapshot {
            search: policy(false),
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
struct OperatorOnlyReportMutant {
    backend: Arc<Valkey>,
}
impl composition::SavedReports for OperatorOnlyReportMutant {
    fn selected<'a>(
        &'a self,
        tenant: &'a TenantId,
        definition: &'a ssx_domain::saved::SavedDefinition,
        run: &'a str,
        limit: usize,
        _: &'a Cancellation,
    ) -> PortFuture<
        'a,
        Result<ssx_application::lineage::workflows::Read<Value>, ssx_application::lineage::Failure>,
    > {
        Box::pin(async move {
            use ssx_application::{
                lineage::{Failure, workflows::Read},
                saved::SavedPolicy,
            };
            use ssx_infra::valkey::saved::{RecentReport, SavedStore};
            let current = RecordsAuthority::current(&OperatorFull);
            let mut limits = current.limits;
            limits.reports = limit as u64;
            let policy = SavedPolicy {
                search: &current.search,
                saved_granted: true,
                auth_configured: &current.auth_configured,
                limits: &limits,
            };
            match SavedStore::new(self.backend.clone(), tenant.clone())
                .recent_report(
                    &SavedSearchId::new(&definition.search_id).unwrap(),
                    run,
                    &policy,
                )
                .await
            {
                Ok(Some(
                    RecentReport::Found(v)
                    | RecentReport::Incomparable(v)
                    | RecentReport::Truncated(v),
                )) => Ok(Read::Found(v)),
                Ok(None) => Ok(Read::Missing),
                Err(_) => Err(Failure::Malformed),
            }
        })
    }
}
struct SwitchSavedDefinition {
    backend: Arc<Valkey>,
    inner: Arc<dyn composition::SavedReports>,
}
impl composition::SavedReports for SwitchSavedDefinition {
    fn selected<'a>(
        &'a self,
        tenant: &'a TenantId,
        definition: &'a ssx_domain::saved::SavedDefinition,
        run: &'a str,
        limit: usize,
        cancel: &'a Cancellation,
    ) -> PortFuture<
        'a,
        Result<ssx_application::lineage::workflows::Read<Value>, ssx_application::lineage::Failure>,
    > {
        Box::pin(async move {
            use ssx_application::{lineage::Failure, saved::SavedPolicy};
            use ssx_infra::valkey::saved::SavedStore;
            let current = RecordsAuthority::current(&OperatorFull);
            let policy = SavedPolicy {
                search: &current.search,
                saved_granted: true,
                auth_configured: &current.auth_configured,
                limits: &current.limits,
            };
            let changed = SavedStore::new(self.backend.clone(), tenant.clone())
                .update(
                    &SavedSearchId::new(&definition.search_id).unwrap(),
                    definition.revision,
                    &json!({"engines":["github"]}),
                    &policy,
                )
                .await
                .map_err(|_| Failure::Malformed)?;
            assert_eq!(changed.engines, vec!["github"]);
            self.inner
                .selected(tenant, definition, run, limit, cancel)
                .await
        })
    }
}
#[tokio::test]
async fn owning_saved_read_uses_principal_operator_intersection_for_changed_definition() {
    for mutant in [false, true] {
        let h = Harness::new().await;
        let (reference, _, _) = saved_source_fixture(&h).await;
        h.authority.revoked.store(6, Ordering::SeqCst);
        let concrete: Arc<dyn composition::SavedReports> =
            Arc::new(saved_reports::NativeSavedReports {
                backend: h.backend.clone(),
                authority: Arc::new(OperatorFull),
                principal: h.authority.clone(),
            });
        let positive = source_with_saved(&h, concrete.clone())
            .resolve(
                &tenant(),
                &reference,
                Destination::Research,
                &Cancellation::default(),
            )
            .await
            .unwrap();
        assert_eq!(positive.query.as_deref(), Some("historical source query"));
        let inner = if mutant {
            Arc::new(OperatorOnlyReportMutant {
                backend: h.backend.clone(),
            }) as Arc<dyn composition::SavedReports>
        } else {
            concrete
        };
        let resolver = source_with_saved(
            &h,
            Arc::new(SwitchSavedDefinition {
                backend: h.backend.clone(),
                inner,
            }),
        );
        let outcome = resolver
            .resolve(
                &tenant(),
                &reference,
                Destination::Research,
                &Cancellation::default(),
            )
            .await;
        if mutant {
            assert_eq!(
                outcome.unwrap().query.as_deref(),
                Some("historical source query")
            );
            eprintln!(
                "W08_CONTROLLED_MUTANT operator_only_policy admitted current github definition despite principal denial; fixed intersection rejects it"
            );
        } else {
            assert_eq!(outcome.unwrap_err().code, "source_policy_denied");
        }
        assert_eq!(h.loopback.calls.load(Ordering::SeqCst), 0);
        h.search.shutdown().await;
    }
}
