//! W02: actual native execution against private disposable Valkey, no providers.
use ssx_application::{
    NoopSink, PortFuture, SearchEngine, cache::*, policy::*, scope::*, snapshot::*,
};
use ssx_domain::{research::*, *};
use ssx_infra::{
    research_runner::*,
    search::*,
    valkey::{research::*, snapshot::*, *},
};
use std::{
    collections::BTreeMap,
    fs,
    io::Read as _,
    num::NonZeroU64,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Notify, watch};
struct Server {
    child: Child,
    dir: PathBuf,
    url: String,
}
impl Server {
    fn start() -> Self {
        let mut bytes = [0; 16];
        fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut bytes)
            .unwrap();
        bytes[6] = (bytes[6] & 15) | 64;
        bytes[8] = (bytes[8] & 63) | 128;
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("ssx-w01-{id}"));
        fs::create_dir(&dir).unwrap();
        let url = format!("unix://{}", dir.join("s.sock").display());
        let mut s = Self {
            child: Self::spawn(&dir),
            dir,
            url,
        };
        s.wait_ready();
        s
    }
    fn spawn(dir: &std::path::Path) -> Child {
        Command::new(
            std::env::var("SLOPSEARX_TEST_VALKEY_SERVER")
                .unwrap_or_else(|_| "valkey-server".into()),
        )
        .args([
            "--port",
            "0",
            "--bind",
            "127.0.0.1",
            "--save",
            "",
            "--appendonly",
            "no",
            "--daemonize",
            "no",
            "--unixsocketperm",
            "700",
            "--dir",
        ])
        .arg(dir)
        .arg("--unixsocket")
        .arg(dir.join("s.sock"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("disposable valkey-server required")
    }
    fn wait_ready(&mut self) {
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "Valkey startup failed"
            );
            if redis::Client::open(self.url.as_str())
                .unwrap()
                .get_connection()
                .is_ok()
            {
                break;
            }
            assert!(Instant::now() < until, "Valkey startup deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn connection(&self) -> redis::Connection {
        redis::Client::open(self.url.as_str())
            .unwrap()
            .get_connection()
            .unwrap()
    }
    fn store(&self) -> Valkey {
        Valkey::new(&self.url, Duration::from_millis(300)).unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}
fn capability(name: &str) -> EngineCapability {
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

fn dependencies(engines: Vec<(&str, Arc<dyn SearchEngine>)>) -> SearchDependencies {
    SearchDependencies {
        policy: PolicyService {
            policy: SearchPolicy::default(),
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

struct FixtureEngine {
    calls: AtomicUsize,
    drops: AtomicUsize,
    held: bool,
    entered: Notify,
    release: Notify,
}
struct Guard<'a>(&'a AtomicUsize);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
impl SearchEngine for FixtureEngine {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let _guard = Guard(&self.drops);
            self.entered.notify_one();
            if self.held {
                self.release.notified().await;
            }
            AdapterResponse {
                results: vec![SearchResult {
                    url: format!("https://example.test/{}", request.query),
                    title: request.query.clone(),
                    ..Default::default()
                }],
                ..Default::default()
            }
        })
    }
}
fn engine(held: bool) -> Arc<FixtureEngine> {
    Arc::new(FixtureEngine {
        calls: AtomicUsize::new(0),
        drops: AtomicUsize::new(0),
        held,
        entered: Notify::new(),
        release: Notify::new(),
    })
}
fn tenant(s: &str) -> TenantId {
    TenantId::new(s).unwrap()
}
fn id() -> JobId {
    JobId::new("job-test").unwrap()
}
fn now() -> WallClockTimestamp {
    SystemSearchClock.capture().0
}
fn job(tenant: &str) -> ResearchJob {
    ResearchJob {
        job_id: id().as_str().into(),
        tenant: tenant.into(),
        question: "question".into(),
        deadline: FiniteF64::new(now().seconds() + 30.0).unwrap(),
        created_at: FiniteF64::new(now().seconds()).unwrap(),
        queries: vec![ResearchQuery {
            query: "question".into(),
            intent: "web".into(),
            engines: vec!["fake".into()],
            requires_intent_grant: false,
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn setup(
    server: &Server,
    engine: Arc<dyn SearchEngine>,
) -> (
    ResearchRunner,
    ResearchStore,
    SearchService,
    SnapshotService,
) {
    setup_store(server.store(), engine)
}
fn setup_store(
    valkey: Valkey,
    engine: Arc<dyn SearchEngine>,
) -> (
    ResearchRunner,
    ResearchStore,
    SearchService,
    SnapshotService,
) {
    setup_policy(valkey, engine, &["research"])
}
fn setup_policy(
    valkey: Valkey,
    engine: Arc<dyn SearchEngine>,
    grants: &[&str],
) -> (
    ResearchRunner,
    ResearchStore,
    SearchService,
    SnapshotService,
) {
    let store = Arc::new(valkey);
    let jobs = ResearchStore::new(store.clone(), tenant("poller"));
    let mut deps = dependencies(vec![("fake", engine)]);
    deps.policy.policy.grants = grants.iter().map(|s| (*s).to_owned()).collect();
    let search = SearchService::new(SearchConfig::default(), deps).unwrap();
    let snapshots = SnapshotService::new(
        Arc::new(ValkeySnapshots::new(store)),
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        tenant("poller"),
        SnapshotTtl::new(60).unwrap(),
    );
    let runner = ResearchRunner::new(
        jobs.clone(),
        search.clone(),
        snapshots.clone(),
        RunnerConfig {
            owner: "worker-1".into(),
            lease_seconds: NonZeroU64::new(1).unwrap(),
            workers: NonZeroU64::new(2).unwrap(),
            poll_interval: Duration::from_millis(20),
            ceilings: ResearchCeilings {
                queries: 8,
                engines_per_query: 4,
                results: 100,
            },
        },
    )
    .unwrap();
    (runner, jobs, search, snapshots)
}
async fn found(store: &ResearchStore) -> ResearchJob {
    match store.load(&id()).await {
        JobRead::Found(job) => *job,
        other => panic!("authoritative read: {other:?}"),
    }
}
fn shutdown() -> (watch::Sender<bool>, watch::Receiver<bool>) {
    watch::channel(false)
}
#[tokio::test]
async fn competing_workers_settle_once_in_claimed_tenant_and_snapshot() {
    let server = Server::start();
    let e = engine(false);
    let (runner, jobs, search, snapshots) = setup(&server, e.clone());
    let scoped = jobs.for_tenant(tenant("org:other"));
    scoped.create(&job("org:other")).await.unwrap();
    let (_stop, rx) = shutdown();
    let (a, b) = tokio::join!(runner.run_once(rx.clone()), runner.run_once(rx));
    assert!(a.is_ok() && b.is_ok(), "{a:?} {b:?}");
    assert_eq!(e.calls.load(Ordering::SeqCst), 1, "{a:?} {b:?}");
    let j = found(&scoped).await;
    assert_eq!(j.state, "succeeded");
    assert_eq!(j.budget_used["attempts"], 1);
    assert_eq!(j.budget_used["results"], 1);
    assert!(j.lease_token.is_none());
    let snap = SnapshotId::new(j.queries[0].cursor.clone().unwrap()).unwrap();
    assert!(matches!(
        snapshots.read(&snap).await,
        SnapshotLookup::Missing
    ));
    assert!(matches!(
        snapshots.for_tenant(tenant("org:other")).read(&snap).await,
        SnapshotLookup::Found(_)
    ));
    search.shutdown().await;
}
#[tokio::test]
async fn held_execution_renews_past_ttl_and_shutdown_joins_without_refund() {
    let server = Server::start();
    let e = engine(true);
    let (runner, jobs, search, _) = setup(&server, e.clone());
    let scoped = jobs.for_tenant(tenant("org:other"));
    scoped.create(&job("org:other")).await.unwrap();
    let (stop, rx) = shutdown();
    let task = tokio::spawn(async move { runner.run_once(rx).await });
    tokio::time::timeout(Duration::from_secs(3), e.entered.notified())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    struct Tokens;
    impl TokenSource<JobId> for Tokens {
        fn next_token(&mut self) -> LeaseToken<JobId> {
            LeaseToken::new("competing-token").unwrap()
        }
    }
    assert!(
        scoped
            .claim(
                &id(),
                "competitor",
                NonZeroU64::new(1).unwrap(),
                &mut Tokens,
                now(),
                || AttemptId::new("recovery").unwrap()
            )
            .await
            .unwrap()
            .is_none()
    );
    stop.send_replace(true);
    assert!(matches!(task.await.unwrap(), Err(RunnerError::Shutdown)));
    assert_eq!(e.drops.load(Ordering::SeqCst), 1);
    assert_eq!(e.calls.load(Ordering::SeqCst), 1);
    assert_eq!(found(&scoped).await.budget_used["attempts"], 1);
    search.shutdown().await;
}
#[tokio::test]
async fn durable_cancel_interrupts_owned_capture_and_settles_truthfully() {
    let server = Server::start();
    let e = engine(true);
    let (runner, jobs, search, _) = setup(&server, e.clone());
    let scoped = jobs.for_tenant(tenant("org:other"));
    scoped.create(&job("org:other")).await.unwrap();
    let (_stop, rx) = shutdown();
    let task = tokio::spawn(async move { runner.run_once(rx).await });
    tokio::time::timeout(Duration::from_secs(3), e.entered.notified())
        .await
        .unwrap();
    scoped
        .request_cancel(&id(), now(), &mut TestTokens, || {
            AttemptId::new("cancel-attempt").unwrap()
        })
        .await
        .unwrap();
    let j = task.await.unwrap().unwrap().unwrap();
    assert_eq!(j.state, "cancelled");
    assert_eq!(j.queries[0].attempts[0].state, "cancelled");
    assert_eq!(j.budget_used["attempts"], 1);
    assert_eq!(e.drops.load(Ordering::SeqCst), 1);
    search.shutdown().await;
}

struct TestTokens;
impl TokenSource<JobId> for TestTokens {
    fn next_token(&mut self) -> LeaseToken<JobId> {
        LeaseToken::new("test-fresh-token").unwrap()
    }
}

#[tokio::test]
async fn retry_extension_retain_successful_evidence_and_history() {
    let server = Server::start();
    let e = engine(false);
    let (runner, jobs, search, _) = setup(&server, e.clone());
    let scoped = jobs.for_tenant(tenant("org:other"));
    let mut j = job("org:other");
    j.queries.push(ResearchQuery {
        index: 1,
        query: "other".into(),
        intent: "web".into(),
        engines: vec!["fake".into()],
        ..Default::default()
    });
    scoped.create(&j).await.unwrap();
    let (_stop, rx) = shutdown();
    let j = runner.run_once(rx.clone()).await.unwrap().unwrap();
    assert_eq!(j.state, "succeeded");
    let prior = j.queries.clone();
    assert!(matches!(
        runner.retry(tenant("org:other"), &id(), rx.clone()).await,
        Err(RunnerError::NoRetryableWork)
    ));
    let q = ResearchQuery {
        query: "followup".into(),
        intent: "web".into(),
        engines: vec!["fake".into()],
        ..Default::default()
    };
    let j = runner
        .extend(tenant("org:other"), &id(), q, rx)
        .await
        .unwrap();
    assert_eq!(&j.queries[..2], prior.as_slice());
    assert_eq!(j.queries[2].index, 2);
    assert_eq!(j.budget_used["attempts"], 3);
    search.shutdown().await;
}
#[tokio::test]
async fn deadline_during_held_query_is_failed_with_conserved_attempt() {
    let server = Server::start();
    let e = engine(true);
    let (runner, jobs, search, _) = setup(&server, e.clone());
    let scoped = jobs.for_tenant(tenant("org:other"));
    let mut j = job("org:other");
    j.deadline = FiniteF64::new(now().seconds() + 0.2).unwrap();
    scoped.create(&j).await.unwrap();
    let (_stop, rx) = shutdown();
    let j = runner.run_once(rx).await.unwrap().unwrap();
    assert_eq!(j.state, "failed");
    assert_eq!(j.stop_reason.as_deref(), Some("deadline_expired"));
    assert_eq!(j.budget_used["attempts"], 1);
    assert_eq!(e.drops.load(Ordering::SeqCst), 1);
    search.shutdown().await;
}
#[tokio::test]
async fn pre_cancelled_snapshot_has_zero_dispatch() {
    let server = Server::start();
    let e = engine(false);
    let (_, _, search, snapshots) = setup(&server, e.clone());
    let cancel = SearchCancellation::default();
    cancel.cancel();
    let p = ScopeProposal::generic(SearchRequest {
        query: "q".into(),
        engines: Some(vec!["fake".into()]),
        ..Default::default()
    });
    assert!(matches!(
        search
            .search_snapshot_with_cancellation(p, &snapshots, &[], cancel)
            .await,
        Err(SearchError::Cancelled)
    ));
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    search.shutdown().await;
}
struct SocketEngine(std::net::SocketAddr);
impl SearchEngine for SocketEngine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut socket = tokio::net::TcpStream::connect(self.0).await.unwrap();
            socket
                .write_all(b"GET /held HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .await
                .unwrap();
            let mut b = [0u8; 1];
            let _ = socket.read(&mut b).await;
            AdapterResponse::default()
        })
    }
}
#[tokio::test]
async fn lease_loss_joins_held_loopback_socket_and_preserves_reclaimer() {
    use tokio::io::AsyncReadExt;
    let server = Server::start();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (runner, jobs, search, _) = setup(
        &server,
        Arc::new(SocketEngine(listener.local_addr().unwrap())),
    );
    let scoped = jobs.for_tenant(tenant("org:other"));
    scoped.create(&job("org:other")).await.unwrap();
    let (_stop, rx) = shutdown();
    let task = tokio::spawn(async move { runner.run_once(rx).await });
    let (mut peer, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let mut request = [0u8; 256];
    assert!(peer.read(&mut request).await.unwrap() > 0);
    let old = found(&scoped).await;
    redis::cmd("DEL")
        .arg(keys::research("lease", &tenant("org:other"), id().as_str()).unwrap())
        .query::<u64>(&mut server.connection())
        .unwrap();
    let new = scoped
        .claim(
            &id(),
            "replacement",
            NonZeroU64::new(5).unwrap(),
            &mut TestTokens,
            now(),
            || AttemptId::new("orphan-recovery").unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_ne!(old.lease_token.as_deref(), Some(new.token.as_str()));
    assert!(task.await.unwrap().is_err());
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), peer.read(&mut request))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    let j = found(&scoped).await;
    assert_eq!(j.lease_token.as_deref(), Some(new.token.as_str()));
    assert!(j.queries[0].cursor.is_none());
    assert_eq!(j.budget_used["attempts"], 1);
    assert_eq!(j.queries[0].attempts[0].state, "interrupted");
    search.shutdown().await;
}

struct CrashEngine(PathBuf);
impl SearchEngine for CrashEngine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            fs::write(&self.0, b"request accepted by deterministic fixture").unwrap();
            std::future::pending::<AdapterResponse>().await
        })
    }
}
#[test]
fn worker_process_probe() {
    let Ok(url) = std::env::var("SSX_W02_CHILD_VALKEY") else {
        return;
    };
    let marker = PathBuf::from(std::env::var("SSX_W02_CHILD_MARKER").unwrap());
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let (runner, _, _, _) = setup_store(
            Valkey::new(&url, Duration::from_millis(300)).unwrap(),
            Arc::new(CrashEngine(marker)),
        );
        let (_stop, rx) = shutdown();
        runner.run_once(rx).await.unwrap();
    });
}
#[tokio::test]
async fn process_kill_restart_discovers_orphan_and_conserves_unknown_spend() {
    let server = Server::start();
    let e = engine(false);
    let (runner, jobs, search, _) = setup(&server, e.clone());
    let scoped = jobs.for_tenant(tenant("org:other"));
    scoped.create(&job("org:other")).await.unwrap();
    let marker = server.dir.join("accepted");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "worker_process_probe", "--nocapture"])
        .env("SSX_W02_CHILD_VALKEY", &server.url)
        .env("SSX_W02_CHILD_MARKER", &marker)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while !marker.exists() {
        assert!(
            child.try_wait().unwrap().is_none(),
            "worker unexpectedly exited"
        );
        assert!(Instant::now() < until, "child never dispatched");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let before = found(&scoped).await;
    assert_eq!(before.budget_used["attempts"], 1);
    assert_eq!(before.queries[0].attempts[0].state, "running");
    child.kill().unwrap();
    child.wait().unwrap();
    tokio::time::sleep(Duration::from_millis(1250)).await;
    let (_stop, rx) = shutdown();
    let recovered = runner.run_once(rx).await.unwrap().unwrap();
    assert_eq!(recovered.state, "succeeded");
    assert_eq!(recovered.budget_used["attempts"], 2);
    assert_eq!(recovered.queries[0].attempts.len(), 2);
    assert_eq!(
        recovered.queries[0].attempts[0].attempt_id,
        before.queries[0].attempts[0].attempt_id
    );
    assert_eq!(recovered.queries[0].attempts[0].state, "interrupted");
    assert_eq!(e.calls.load(Ordering::SeqCst), 1);
    search.shutdown().await;
}

#[tokio::test]
async fn workflow_grant_matrix_denies_atomically_without_provider_calls() {
    for mask in 0..8 {
        let server = Server::start();
        let e = engine(false);
        let all = ["research", "security", "dependency_dossier"];
        let grants = all
            .iter()
            .enumerate()
            .filter(|(bit, _)| mask & (1 << bit) != 0)
            .map(|(_, s)| *s)
            .collect::<Vec<_>>();
        let (runner, jobs, search, _) = setup_policy(server.store(), e.clone(), &grants);
        let scoped = jobs.for_tenant(tenant("org:other"));
        let mut j = job("org:other");
        j.workflow
            .insert("kind".into(), ssx_json::json!("dependency_dossier"));
        j.workflow.insert(
            "budget".into(),
            ssx_json::json!({"max_adapter_calls":1,"max_results":1}),
        );
        scoped.create(&j).await.unwrap();
        let (_stop, rx) = shutdown();
        let j = runner.run_once(rx).await.unwrap().unwrap();
        assert_eq!(
            e.calls.load(Ordering::SeqCst),
            usize::from(mask == 7),
            "mask {mask}: {j:?}"
        );
        if mask == 7 {
            assert_eq!(
                j.workflow["budget"]["used_adapter_calls"],
                ssx_json::json!(1)
            );
            assert_eq!(j.workflow["budget"]["captured_results"], ssx_json::json!(1));
        } else {
            assert_eq!(j.state, "failed");
            assert_eq!(j.budget_used["attempts"], 1);
        }
        search.shutdown().await;
    }
}
#[tokio::test]
async fn workflow_mixed_sensitive_plan_denies_before_first_dispatch() {
    let server = Server::start();
    let e = engine(false);
    let (runner, jobs, search, _) = setup_policy(
        server.store(),
        e.clone(),
        &["research", "security", "dependency_dossier"],
    );
    let scoped = jobs.for_tenant(tenant("org:other"));
    let mut j = job("org:other");
    j.workflow
        .insert("kind".into(), ssx_json::json!("dependency_dossier"));
    j.queries.push(ResearchQuery {
        index: 1,
        query: "sensitive".into(),
        intent: "security".into(),
        engines: vec!["fake".into(), "hibp".into()],
        ..Default::default()
    });
    scoped.create(&j).await.unwrap();
    let (_stop, rx) = shutdown();
    let j = runner.run_once(rx).await.unwrap().unwrap();
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    assert_eq!(j.budget_used["attempts"], 2);
    assert!(j.queries.iter().all(|q| {
        q.attempts.len() == 1
            && q.attempts[0].state == "failed"
            && q.attempts[0]
                .error
                .as_deref()
                .unwrap()
                .contains("policy_rejected")
    }));
    assert_eq!(j.state, "failed");
    search.shutdown().await;
}
struct HeldSnapshot {
    inner: ValkeySnapshots,
    written: Notify,
    release: Notify,
}
impl SnapshotRepository for HeldSnapshot {
    fn create<'a>(
        &'a self,
        snapshot: &'a SearchSnapshot,
        ttl: NonZeroU64,
    ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
        Box::pin(async move {
            self.inner.create(snapshot, ttl).await?;
            self.written.notify_one();
            self.release.notified().await;
            Ok(SnapshotWrite::Stored)
        })
    }
    fn read<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a SnapshotId,
    ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
        self.inner.read(tenant, id)
    }
}
struct FixedSnapshot;
impl SnapshotIds for FixedSnapshot {
    fn next(&self) -> Result<SnapshotId, SnapshotFailure> {
        Ok(SnapshotId::new("snap-orphan").unwrap())
    }
}
#[tokio::test]
async fn snapshot_write_cancel_can_leave_tenant_orphan_but_never_stale_job_link() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, _) = setup(&server, e);
    let scoped = jobs.for_tenant(tenant("org:other"));
    scoped.create(&job("org:other")).await.unwrap();
    let repo = Arc::new(HeldSnapshot {
        inner: ValkeySnapshots::new(Arc::new(server.store())),
        written: Notify::new(),
        release: Notify::new(),
    });
    let snapshots = SnapshotService::new(
        repo.clone(),
        Arc::new(SystemSnapshotClock),
        Arc::new(FixedSnapshot),
        tenant("org:other"),
        SnapshotTtl::new(60).unwrap(),
    );
    let runner = ResearchRunner::new(
        jobs.clone(),
        search.clone(),
        snapshots.clone(),
        RunnerConfig {
            owner: "original".into(),
            lease_seconds: NonZeroU64::new(1).unwrap(),
            poll_interval: Duration::from_millis(20),
            workers: NonZeroU64::new(1).unwrap(),
            ceilings: ResearchCeilings {
                queries: 4,
                engines_per_query: 2,
                results: 20,
            },
        },
    )
    .unwrap();
    let (_stop, rx) = shutdown();
    let task = tokio::spawn(async move { runner.run_once(rx).await });
    tokio::time::timeout(Duration::from_secs(3), repo.written.notified())
        .await
        .unwrap();
    redis::cmd("DEL")
        .arg(keys::research("lease", &tenant("org:other"), id().as_str()).unwrap())
        .query::<u64>(&mut server.connection())
        .unwrap();
    let replacement = scoped
        .claim(
            &id(),
            "replacement",
            NonZeroU64::new(5).unwrap(),
            &mut TestTokens,
            now(),
            || AttemptId::new("recovered").unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(task.await.unwrap().is_err());
    let j = found(&scoped).await;
    assert_eq!(j.lease_token.as_deref(), Some(replacement.token.as_str()));
    assert!(j.queries[0].cursor.is_none());
    let sid = SnapshotId::new("snap-orphan").unwrap();
    assert!(matches!(
        snapshots.read(&sid).await,
        SnapshotLookup::Found(_)
    ));
    assert!(matches!(
        snapshots.for_tenant(tenant("other")).read(&sid).await,
        SnapshotLookup::Missing
    ));
    search.shutdown().await;
}
#[tokio::test]
async fn snapshot_write_cancellation_reports_completion_unknown() {
    let server = Server::start();
    let (_, _, search, _) = setup(&server, engine(false));
    let repo = Arc::new(HeldSnapshot {
        inner: ValkeySnapshots::new(Arc::new(server.store())),
        written: Notify::new(),
        release: Notify::new(),
    });
    let snapshots = SnapshotService::new(
        repo.clone(),
        Arc::new(SystemSnapshotClock),
        Arc::new(FixedSnapshot),
        tenant("org:other"),
        SnapshotTtl::new(60).unwrap(),
    );
    let cancellation = SearchCancellation::default();
    let cancel = cancellation.clone();
    let service = search.clone();
    let store = snapshots.clone();
    let task = tokio::spawn(async move {
        service
            .search_snapshot_with_cancellation(
                ScopeProposal::generic(SearchRequest {
                    query: "q".into(),
                    engines: Some(vec!["fake".into()]),
                    ..Default::default()
                }),
                &store,
                &[],
                cancel,
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), repo.written.notified())
        .await
        .unwrap();
    cancellation.cancel();
    let captured = task.await.unwrap().unwrap();
    assert_eq!(captured.snapshot, Err(SnapshotFailure::CompletionUnknown));
    assert_eq!(captured.canonical.response().results.len(), 1);
    search.shutdown().await;
}
struct RetryEngine {
    calls: AtomicUsize,
    entered: Notify,
    release: Notify,
}
impl SearchEngine for RetryEngine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                return AdapterResponse::default();
            }
            self.entered.notify_one();
            self.release.notified().await;
            AdapterResponse {
                results: vec![SearchResult {
                    url: "https://example.test/retry".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }
        })
    }
}
#[tokio::test]
async fn concurrent_retry_holds_authority_and_preserves_empty_attempt_snapshot() {
    let server = Server::start();
    let e = Arc::new(RetryEngine {
        calls: AtomicUsize::new(0),
        entered: Notify::new(),
        release: Notify::new(),
    });
    let (runner, jobs, search, _) = setup(&server, e.clone());
    let scoped = jobs.for_tenant(tenant("org:other"));
    scoped.create(&job("org:other")).await.unwrap();
    let (_stop, rx) = shutdown();
    let original = runner.run_once(rx.clone()).await.unwrap().unwrap();
    assert_eq!(original.state, "succeeded");
    assert_eq!(original.queries[0].result_count, 0);
    let original_attempt = original.queries[0].attempts[0].clone();
    let r = runner.clone();
    let rx2 = rx.clone();
    let task = tokio::spawn(async move { r.retry(tenant("org:other"), &id(), rx2).await });
    tokio::time::timeout(Duration::from_secs(3), e.entered.notified())
        .await
        .unwrap();
    assert!(matches!(
        runner.retry(tenant("org:other"), &id(), rx).await,
        Err(RunnerError::LiveOwner)
    ));
    e.release.notify_one();
    let j = task.await.unwrap().unwrap();
    assert_eq!(j.queries[0].attempts[0], original_attempt);
    assert_eq!(j.queries[0].attempts.len(), 2);
    assert_eq!(j.budget_used["attempts"], 2);
    assert_eq!(j.budget_used["results"], 1);
    search.shutdown().await;
}
#[tokio::test]
async fn cancellation_affects_only_owned_isolated_attempt() {
    let server = Server::start();
    let e = engine(true);
    let (_, _, search, snapshots) = setup(&server, e.clone());
    let token = SearchCancellation::default();
    let proposal = |key: &str| {
        ScopeProposal::generic(SearchRequest {
            query: "identical".into(),
            engines: Some(vec!["fake".into()]),
            execution_isolation_key: Some(key.into()),
            ..Default::default()
        })
    };
    let owned = proposal("attempt-one");
    let svc = search.clone();
    let snaps = snapshots.clone();
    let ct = token.clone();
    let a = tokio::spawn(async move {
        svc.search_snapshot_with_cancellation(owned, &snaps, &[], ct)
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), e.entered.notified())
        .await
        .unwrap();
    let other = proposal("interactive-two");
    let svc = search.clone();
    let b = tokio::spawn(async move { svc.search(other).await });
    tokio::time::timeout(Duration::from_secs(3), e.entered.notified())
        .await
        .unwrap();
    token.cancel();
    assert!(matches!(a.await.unwrap(), Err(SearchError::Cancelled)));
    assert!(!b.is_finished());
    e.release.notify_one();
    assert_eq!(b.await.unwrap().unwrap().results.len(), 1);
    assert_eq!(e.calls.load(Ordering::SeqCst), 2);
    search.shutdown().await;
}
struct ChangingHealth;
impl SearchEngine for ChangingHealth {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            if request.query.starts_with("fail-") {
                AdapterResponse {
                    status: EngineStatus::Error,
                    ..Default::default()
                }
            } else {
                AdapterResponse {
                    results: vec![SearchResult {
                        url: "https://example.test/evidence".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }
            }
        })
    }
}
#[tokio::test]
async fn actual_runtime_circuit_change_preserves_authorized_explicit_capture() {
    let server = Server::start();
    let store = Arc::new(server.store());
    let jobs = ResearchStore::new(store.clone(), tenant("poller"));
    let scoped = jobs.for_tenant(tenant("org:other"));
    scoped.create(&job("org:other")).await.unwrap();
    let mut deps = dependencies(vec![("fake", Arc::new(ChangingHealth))]);
    deps.policy.policy.grants.insert("research".into());
    deps.policy.resolver.catalog_routing = true;
    let search = SearchService::new(SearchConfig::default(), deps).unwrap();
    let repo = Arc::new(HeldSnapshot {
        inner: ValkeySnapshots::new(store),
        written: Notify::new(),
        release: Notify::new(),
    });
    let snapshots = SnapshotService::new(
        repo.clone(),
        Arc::new(SystemSnapshotClock),
        Arc::new(FixedSnapshot),
        tenant("poller"),
        SnapshotTtl::new(60).unwrap(),
    );
    let runner = ResearchRunner::new(
        jobs,
        search.clone(),
        snapshots,
        RunnerConfig {
            owner: "worker".into(),
            lease_seconds: NonZeroU64::new(1).unwrap(),
            workers: NonZeroU64::new(1).unwrap(),
            poll_interval: Duration::from_millis(20),
            ceilings: ResearchCeilings {
                queries: 4,
                engines_per_query: 2,
                results: 20,
            },
        },
    )
    .unwrap();
    let (_stop, rx) = shutdown();
    let task = tokio::spawn(async move { runner.run_once(rx).await });
    tokio::time::timeout(Duration::from_secs(3), repo.written.notified())
        .await
        .unwrap();
    for i in 0..5 {
        search
            .search(ScopeProposal::generic(SearchRequest {
                query: format!("fail-{i}"),
                engines: Some(vec!["fake".into()]),
                ..Default::default()
            }))
            .await
            .unwrap();
    }
    assert!(search.health()["fake"].circuit_open(now()));
    repo.release.notify_one();
    let j = task.await.unwrap().unwrap().unwrap();
    assert_eq!(j.state, "succeeded");
    assert!(j.queries[0].cursor.is_some());
    assert_eq!(j.budget_used["attempts"], 1);
    assert_eq!(j.budget_used["results"], 1);
    search.shutdown().await;
}
#[tokio::test]
async fn invalid_delayed_authority_never_runs_guarded_publication() {
    let server = Server::start();
    let (_, _, allowed, _) = setup(&server, engine(false));
    let (_, _, denied, _) = setup_policy(server.store(), engine(false), &[]);
    let authority = allowed
        .prepare_scope(&ScopeProposal::from_research_query(
            &job("org:other").queries[0],
        ))
        .await
        .unwrap();
    let mut committed = false;
    let result = denied.revalidate_research_scope(&authority).map(|()| {
        committed = true;
    });
    assert!(result.is_err());
    assert!(!committed);
    let store = ResearchStore::new(Arc::new(server.store()), tenant("org:other"));
    store.create(&job("org:other")).await.unwrap();
    let claim = store
        .claim(
            &id(),
            "owner",
            NonZeroU64::new(5).unwrap(),
            &mut TestTokens,
            now(),
            || AttemptId::new("unused").unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    let reserved = store
        .mutate(&id(), &claim.token, |j| {
            j.initialize_budget(ResearchCeilings {
                queries: 4,
                engines_per_query: 2,
                results: 20,
            })?;
            j.reserve_attempt(0, AttemptId::new("reserved").unwrap(), now())?;
            Ok(())
        })
        .await
        .unwrap();
    let denied_write = store
        .mutate(&id(), &claim.token, |j| {
            denied
                .revalidate_research_scope(&authority)
                .map_err(|_| DomainError("policy rejected publication"))?;
            j.queries[0].cursor = Some("snap-forbidden".into());
            Ok(())
        })
        .await;
    assert!(denied_write.is_err());
    assert_eq!(found(&store).await, reserved);
    store.release(&id(), &claim.token).await.unwrap();
    allowed.shutdown().await;
    denied.shutdown().await;
}
#[tokio::test]
async fn source_coverage_required_optional_excluded_absent_and_order() {
    let mut deps = dependencies(vec![("brave", engine(false)), ("github", engine(false))]);
    let mut required = capability("brave");
    required.auth_ready = false;
    let optional = capability("github");
    deps.policy.catalog = CatalogSnapshot::new(vec![required, optional]).unwrap();
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let q = ResearchQuery {
        engines: vec!["github".into(), "brave".into(), "absent".into()],
        ..Default::default()
    };
    let response = SearchResponse {
        engine_outcomes: vec![
            EngineOutcome {
                engine: "brave".into(),
                status: "unavailable".into(),
                result_count: 0,
                latency_ms: None,
                message: None,
            },
            EngineOutcome {
                engine: "github".into(),
                status: "ok".into(),
                result_count: 0,
                latency_ms: None,
                message: None,
            },
        ],
        ..Default::default()
    };
    let coverage = service.research_coverage(&q, &response).unwrap();
    assert_eq!(
        coverage
            .iter()
            .map(|c| c.engine.as_str())
            .collect::<Vec<_>>(),
        vec!["github", "brave", "absent"]
    );
    assert_eq!(coverage[0].bucket, "empty");
    assert_eq!(coverage[1].bucket, "unavailable");
    assert_eq!(coverage[1].failure_class.as_deref(), Some("auth_required"));
    assert_eq!(coverage[2].bucket, "not-selected");
    let mut response = response;
    response.scope.excluded_engines = vec![
        EngineExclusion {
            engine: "brave".into(),
            reason: "excluded".into(),
            stage: "auth".into(),
        },
        EngineExclusion {
            engine: "extra".into(),
            reason: "excluded".into(),
            stage: "scope".into(),
        },
    ];
    let coverage = service.research_coverage(&q, &response).unwrap();
    assert_eq!(coverage[1].bucket, "not-selected");
    assert_eq!(coverage[3].engine, "extra");
    service.shutdown().await;
}
#[tokio::test]
async fn supervised_pool_shutdown_joins_all_owned_searches_and_retains_charges() {
    let server = Server::start();
    let e = engine(true);
    let (runner, jobs, search, _) = setup(&server, e.clone());
    for name in ["org:first", "org:second"] {
        jobs.for_tenant(tenant(name))
            .create(&job(name))
            .await
            .unwrap();
    }
    let (stop, rx) = shutdown();
    let task = tokio::spawn(async move { runner.run(rx).await });
    let until = Instant::now() + Duration::from_secs(3);
    while e.calls.load(Ordering::SeqCst) != 2 {
        assert!(Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    stop.send_replace(true);
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(e.drops.load(Ordering::SeqCst), 2);
    for name in ["org:first", "org:second"] {
        assert_eq!(
            found(&jobs.for_tenant(tenant(name))).await.budget_used["attempts"],
            1
        );
    }
    search.shutdown().await;
}
#[tokio::test]
async fn shutdown_interrupts_renewal_io_before_store_timeout() {
    let server = Server::start();
    let e = engine(true);
    let (runner, jobs, search, _) = setup_store(
        Valkey::new(&server.url, Duration::from_secs(3)).unwrap(),
        e.clone(),
    );
    jobs.for_tenant(tenant("org:other"))
        .create(&job("org:other"))
        .await
        .unwrap();
    let (stop, rx) = shutdown();
    let task = tokio::spawn(async move { runner.run_once(rx).await });
    tokio::time::timeout(Duration::from_secs(3), e.entered.notified())
        .await
        .unwrap();
    redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(1800)
        .arg("ALL")
        .query::<()>(&mut server.connection())
        .unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    stop.send_replace(true);
    let until = Instant::now() + Duration::from_millis(600);
    while e.drops.load(Ordering::SeqCst) != 1 {
        assert!(
            Instant::now() < until,
            "owned search remained blocked behind renewal I/O"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(4), task)
            .await
            .unwrap()
            .unwrap(),
        Err(RunnerError::Shutdown)
    ));
    search.shutdown().await;
}
struct PartialEngine {
    success_calls: AtomicUsize,
    failed_calls: AtomicUsize,
}
impl SearchEngine for PartialEngine {
    fn search<'a>(&'a self, r: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            if r.query == "failure" {
                if self.failed_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    return AdapterResponse {
                        status: EngineStatus::Error,
                        ..Default::default()
                    };
                }
            } else {
                self.success_calls.fetch_add(1, Ordering::SeqCst);
            }
            AdapterResponse {
                results: vec![SearchResult {
                    url: format!("https://example.test/{}", r.query),
                    ..Default::default()
                }],
                ..Default::default()
            }
        })
    }
}
#[tokio::test]
async fn partial_retry_preserves_success_and_failed_snapshot_bytes() {
    let server = Server::start();
    let e = Arc::new(PartialEngine {
        success_calls: AtomicUsize::new(0),
        failed_calls: AtomicUsize::new(0),
    });
    let (runner, jobs, search, snapshots) = setup(&server, e.clone());
    let scoped = jobs.for_tenant(tenant("org:other"));
    let mut j = job("org:other");
    j.queries.push(ResearchQuery {
        index: 1,
        query: "failure".into(),
        intent: "web".into(),
        engines: vec!["fake".into()],
        ..Default::default()
    });
    scoped.create(&j).await.unwrap();
    let (_stop, rx) = shutdown();
    let initial = runner.run_once(rx.clone()).await.unwrap().unwrap();
    assert_eq!(initial.state, "partial");
    let prior = initial.queries[0].clone();
    let failed = initial.queries[1].attempts[0].clone();
    let snapshots = snapshots.for_tenant(tenant("org:other"));
    let cursor = SnapshotId::new(failed.cursor.clone().unwrap()).unwrap();
    let before = match snapshots.read(&cursor).await {
        SnapshotLookup::Found(s) => ssx_domain::storage::snapshot::encode(&s).unwrap(),
        other => panic!("{other:?}"),
    };
    let retried = runner.retry(tenant("org:other"), &id(), rx).await.unwrap();
    assert_eq!(retried.state, "succeeded");
    assert_eq!(retried.queries[0], prior);
    assert_eq!(retried.queries[1].attempts[0], failed);
    assert_eq!(e.success_calls.load(Ordering::SeqCst), 1);
    assert_eq!(e.failed_calls.load(Ordering::SeqCst), 2);
    let after = match snapshots.read(&cursor).await {
        SnapshotLookup::Found(s) => ssx_domain::storage::snapshot::encode(&s).unwrap(),
        other => panic!("{other:?}"),
    };
    assert_eq!(before, after);
    search.shutdown().await;
}
#[tokio::test]
async fn dropped_caller_releases_owned_socket_capacity_and_lease_without_harming_other_flight() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let server = Server::start();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut deps = dependencies(vec![(
        "fake",
        Arc::new(SocketEngine(listener.local_addr().unwrap())),
    )]);
    deps.policy.policy.grants.insert("research".into());
    let service = SearchService::new(
        SearchConfig {
            max_callers: 2,
            max_flights: 2,
            ..Default::default()
        },
        deps,
    )
    .unwrap();
    let jobs = ResearchStore::new(Arc::new(server.store()), tenant("org:other"));
    jobs.create(&job("org:other")).await.unwrap();
    let snapshots = SnapshotService::new(
        Arc::new(ValkeySnapshots::new(Arc::new(server.store()))),
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        tenant("org:other"),
        SnapshotTtl::new(60).unwrap(),
    );
    let runner = ResearchRunner::new(
        jobs.clone(),
        service.clone(),
        snapshots,
        RunnerConfig {
            owner: "original".into(),
            lease_seconds: NonZeroU64::new(1).unwrap(),
            workers: NonZeroU64::new(1).unwrap(),
            poll_interval: Duration::from_millis(20),
            ceilings: ResearchCeilings {
                queries: 4,
                engines_per_query: 2,
                results: 20,
            },
        },
    )
    .unwrap();
    let (_stop, rx) = shutdown();
    let owned = tokio::spawn(async move { runner.run_once(rx).await });
    let (mut peer1, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let mut bytes = [0u8; 256];
    assert!(peer1.read(&mut bytes).await.unwrap() > 0);
    let svc = service.clone();
    let other = tokio::spawn(async move {
        svc.search(ScopeProposal::generic(SearchRequest {
            query: "unrelated".into(),
            engines: Some(vec!["fake".into()]),
            ..Default::default()
        }))
        .await
    });
    let (mut peer2, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
        .await
        .unwrap()
        .unwrap();
    assert!(peer2.read(&mut bytes).await.unwrap() > 0);
    owned.abort();
    assert!(owned.await.unwrap_err().is_cancelled());
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), peer1.read(&mut bytes))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(40), peer2.read(&mut bytes))
            .await
            .is_err()
    );
    assert!(!other.is_finished());
    // This service admits only two callers/flights. With the unrelated caller
    // still active, a third accepted socket proves the dropped slot was freed.
    let svc = service.clone();
    let next = tokio::spawn(async move {
        svc.search(ScopeProposal::generic(SearchRequest {
            query: "next".into(),
            engines: Some(vec!["fake".into()]),
            ..Default::default()
        }))
        .await
    });
    let (mut peer3, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
        .await
        .unwrap()
        .unwrap();
    assert!(peer3.read(&mut bytes).await.unwrap() > 0);
    peer3.write_all(b"x").await.unwrap();
    next.await.unwrap().unwrap();
    peer2.write_all(b"x").await.unwrap();
    other.await.unwrap().unwrap();
    tokio::time::sleep(Duration::from_millis(1250)).await;
    let replacement = jobs
        .claim(
            &id(),
            "replacement",
            NonZeroU64::new(2).unwrap(),
            &mut TestTokens,
            now(),
            || AttemptId::new("recovered-after-drop").unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replacement.job.budget_used["attempts"], 1);
    assert_eq!(replacement.job.queries[0].attempts[0].state, "interrupted");
    jobs.release(&id(), &replacement.token).await.unwrap();
    service.shutdown().await;
}
#[tokio::test]
async fn large_finite_persisted_deadlines_execute_without_duration_or_instant_panic() {
    for horizon in [1.0e300, 1.0e20, 1.0e12] {
        let server = Server::start();
        let e = engine(false);
        let (runner, jobs, service, _) = setup(&server, e.clone());
        let mut j = job("org:other");
        j.deadline = FiniteF64::new(horizon).unwrap();
        jobs.for_tenant(tenant("org:other"))
            .create(&j)
            .await
            .unwrap();
        let (_stop, rx) = shutdown();
        let j = runner.run_once(rx).await.unwrap().unwrap();
        assert_eq!(j.state, "succeeded");
        assert_eq!(j.deadline.get(), horizon);
        assert_eq!(e.calls.load(Ordering::SeqCst), 1);
        service.shutdown().await;
    }
}
#[tokio::test]
async fn runner_rejects_unrepresentable_lease_and_poll_configuration() {
    let server = Server::start();
    let (_, jobs, service, snapshots) = setup(&server, engine(false));
    let config = |ttl, poll| RunnerConfig {
        owner: "test".into(),
        lease_seconds: NonZeroU64::new(ttl).unwrap(),
        poll_interval: poll,
        workers: NonZeroU64::new(1).unwrap(),
        ceilings: ResearchCeilings {
            queries: 1,
            engines_per_query: 1,
            results: 1,
        },
    };
    assert!(
        ResearchRunner::new(
            jobs.clone(),
            service.clone(),
            snapshots.clone(),
            config(1, Duration::from_millis(1))
        )
        .is_ok()
    );
    assert!(matches!(
        ResearchRunner::new(
            jobs.clone(),
            service.clone(),
            snapshots.clone(),
            config(u64::MAX, Duration::from_millis(1))
        ),
        Err(RunnerError::Invalid(_))
    ));
    assert!(matches!(
        ResearchRunner::new(jobs, service.clone(), snapshots, config(1, Duration::MAX)),
        Err(RunnerError::Invalid(_))
    ));
    service.shutdown().await;
}
#[tokio::test]
async fn malformed_workflow_budget_never_defaults_into_more_spend() {
    for value in [
        ssx_json::json!(null),
        ssx_json::json!(-1),
        ssx_json::json!(1.5),
        ssx_json::json!("1"),
        ssx_json::json!({}),
    ] {
        let server = Server::start();
        let e = engine(false);
        let (runner, jobs, service, _) = setup_policy(
            server.store(),
            e.clone(),
            &["research", "security", "dependency_dossier"],
        );
        let mut j = job("org:other");
        j.workflow
            .insert("kind".into(), ssx_json::json!("dependency_dossier"));
        j.workflow.insert(
            "budget".into(),
            ssx_json::json!({"used_adapter_calls":value,"max_adapter_calls":10}),
        );
        jobs.for_tenant(tenant("org:other"))
            .create(&j)
            .await
            .unwrap();
        let (_stop, rx) = shutdown();
        let j = runner.run_once(rx).await.unwrap().unwrap();
        assert_eq!(j.state, "failed");
        assert_eq!(e.calls.load(Ordering::SeqCst), 0);
        assert_eq!(j.budget_used["attempts"], 0);
        service.shutdown().await;
    }
}
#[tokio::test]
async fn source_success_at_query_ceiling_distinguishes_dossier_stop_reason() {
    for is_dossier in [false, true] {
        let server = Server::start();
        let (runner, jobs, service, _) = setup_policy(
            server.store(),
            engine(false),
            &["research", "security", "dependency_dossier"],
        );
        let mut j = job("org:other");
        j.initialize_budget(ResearchCeilings {
            queries: 1,
            engines_per_query: 1,
            results: 10,
        })
        .unwrap();
        if is_dossier {
            j.workflow
                .insert("kind".into(), ssx_json::json!("dependency_dossier"));
            j.workflow.insert(
                "budget".into(),
                ssx_json::json!({"max_adapter_calls":1,"max_results":10}),
            );
        }
        jobs.for_tenant(tenant("org:other"))
            .create(&j)
            .await
            .unwrap();
        let (_stop, rx) = shutdown();
        let j = runner.run_once(rx).await.unwrap().unwrap();
        assert_eq!(j.state, "succeeded");
        assert_eq!(
            j.stop_reason.as_deref(),
            Some(if is_dossier {
                "plan_executed"
            } else {
                "query_budget_exhausted"
            })
        );
        service.shutdown().await;
    }
}
#[tokio::test]
async fn denied_workflow_audit_survives_restored_authority_retry_without_actual_call_charge() {
    let server = Server::start();
    let e = Arc::new(PartialEngine {
        success_calls: AtomicUsize::new(0),
        failed_calls: AtomicUsize::new(0),
    });
    let (initial, jobs, service, _) = setup_policy(
        server.store(),
        e,
        &["research", "security", "dependency_dossier"],
    );
    let scoped = jobs.for_tenant(tenant("org:other"));
    let mut j = job("org:other");
    j.queries.push(ResearchQuery {
        index: 1,
        query: "failure".into(),
        intent: "web".into(),
        engines: vec!["fake".into()],
        ..Default::default()
    });
    j.workflow
        .insert("kind".into(), ssx_json::json!("dependency_dossier"));
    j.workflow.insert(
        "budget".into(),
        ssx_json::json!({"max_adapter_calls":5,"max_results":10}),
    );
    scoped.create(&j).await.unwrap();
    let (_stop, rx) = shutdown();
    let before = initial.run_once(rx.clone()).await.unwrap().unwrap();
    assert_eq!(before.state, "partial");
    let successful = before.queries[0].clone();
    assert_eq!(
        before.workflow["budget"]["used_adapter_calls"],
        ssx_json::json!(2)
    );
    let blocked_engine = engine(false);
    let (denied, _, denied_service, _) = setup_policy(server.store(), blocked_engine.clone(), &[]);
    let audited = denied
        .retry(tenant("org:other"), &id(), rx.clone())
        .await
        .unwrap();
    assert_eq!(blocked_engine.calls.load(Ordering::SeqCst), 0);
    assert_eq!(audited.queries[0], successful);
    assert_eq!(audited.budget_used["attempts"], 3);
    assert_eq!(
        audited.workflow["budget"]["used_adapter_calls"],
        ssx_json::json!(2)
    );
    let denied_attempt = audited.queries[1].attempts[1].clone();
    assert_eq!(denied_attempt.state, "failed");
    assert!(denied_attempt.error.as_deref().unwrap().contains("grant"));
    assert!(
        denied_attempt
            .engine_coverage
            .iter()
            .all(|c| c.bucket == "not-selected")
    );
    let restored_engine = engine(false);
    let (restored, _, restored_service, _) = setup_policy(
        server.store(),
        restored_engine.clone(),
        &["research", "security", "dependency_dossier"],
    );
    let done = restored
        .retry(tenant("org:other"), &id(), rx)
        .await
        .unwrap();
    assert_eq!(done.state, "succeeded");
    assert_eq!(done.queries[0], successful);
    assert_eq!(done.queries[1].attempts[1], denied_attempt);
    assert_eq!(done.queries[1].attempts[0], before.queries[1].attempts[0]);
    assert_eq!(done.budget_used["attempts"], 4);
    assert_eq!(
        done.workflow["budget"]["used_adapter_calls"],
        ssx_json::json!(3)
    );
    assert_eq!(restored_engine.calls.load(Ordering::SeqCst), 1);
    service.shutdown().await;
    denied_service.shutdown().await;
    restored_service.shutdown().await;
}
#[tokio::test]
async fn exhausted_denial_reservation_never_fabricates_attempt_or_refunds_counts() {
    let server = Server::start();
    let e = engine(false);
    let (runner, jobs, service, _) = setup_policy(server.store(), e.clone(), &[]);
    let mut j = job("org:other");
    j.initialize_budget(ResearchCeilings {
        queries: 0,
        engines_per_query: 1,
        results: 10,
    })
    .unwrap();
    j.workflow
        .insert("kind".into(), ssx_json::json!("dependency_dossier"));
    let original = j.budget_used.clone();
    jobs.for_tenant(tenant("org:other"))
        .create(&j)
        .await
        .unwrap();
    let (_stop, rx) = shutdown();
    let j = runner.run_once(rx).await.unwrap().unwrap();
    assert_eq!(j.state, "failed");
    assert!(j.queries[0].attempts.is_empty());
    assert_eq!(j.budget_used, original);
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    assert_eq!(j.stop_reason.as_deref(), Some("attempt_budget_exhausted"));
    service.shutdown().await;
}
