//! W02: actual native execution against private disposable Valkey, no providers.
use crate::{
    research_runner::*,
    search::*,
    valkey::{research::*, snapshot::*, *},
};
use ssx_application::{
    NoopSink, PortFuture, SearchEngine, cache::*, policy::*, scope::*, snapshot::*,
};
use ssx_domain::*;
use std::sync::atomic::AtomicBool;
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
use tokio::sync::{Notify, Semaphore, watch};
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
        let base = std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let dir = base.join(format!("ssx-w02-{id}"));
        use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
        assert!(dir.join("s.sock").as_os_str().as_bytes().len() < 104);
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
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

pub(crate) struct Probe {
    pub(crate) monitor_joined: AtomicBool,
    pub(crate) renewed: AtomicUsize,
    pub(crate) validated_owned: Semaphore,
    pub(crate) read_cleared: Semaphore,
    pub(crate) clear_saw_joined: AtomicBool,
    pub(crate) clear_committed: Semaphore,
    pub(crate) release_clear: Semaphore,
    pub(crate) pause_before_clear: bool,
    pub(crate) before_clear: Semaphore,
    pub(crate) release_before_clear: Semaphore,
}
impl Probe {
    fn new(pause_before_clear: bool) -> Arc<Self> {
        Arc::new(Self {
            monitor_joined: AtomicBool::new(false),
            renewed: AtomicUsize::new(0),
            validated_owned: Semaphore::new(0),
            read_cleared: Semaphore::new(0),
            clear_saw_joined: AtomicBool::new(false),
            clear_committed: Semaphore::new(0),
            release_clear: Semaphore::new(0),
            pause_before_clear,
            before_clear: Semaphore::new(0),
            release_before_clear: Semaphore::new(0),
        })
    }
}
async fn entered(gate: &Semaphore) {
    tokio::time::timeout(Duration::from_secs(3), gate.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
}
fn completed_job() -> ResearchJob {
    let mut job = job("org:other");
    job.caller_completed = true;
    job.state = "succeeded".into();
    job.queries[0].state = "done".into();
    job
}
async fn checked_completion(terminal: bool) {
    let server = Server::start();
    let engine = engine(false);
    let (mut runner, jobs, service, _) = setup(&server, engine.clone());
    let probe = Probe::new(false);
    runner.jobs = runner.jobs.clone().with_completion_probe(probe.clone());
    let scoped = runner.jobs.for_tenant(tenant("org:other"));
    let before = if terminal {
        completed_job()
    } else {
        job("org:other")
    };
    scoped.create(&before).await.unwrap();
    let (_stop, rx) = shutdown();
    let mut expected_terminal_queries = None;
    let task = if terminal {
        let claim = scoped
            .claim_prepared(
                &id(),
                "fixture-terminal",
                NonZeroU64::new(1).unwrap(),
                &mut super::RandomTokens,
                now(),
                super::attempt,
                false,
                |_| Ok(()),
            )
            .await
            .unwrap()
            .unwrap();
        expected_terminal_queries = Some(claim.job.queries.clone());
        tokio::spawn(async move { runner.execute_claim(claim, rx).await })
    } else {
        tokio::spawn(async move { runner.run_once(rx).await.map(|job| job.unwrap()) })
    };
    entered(&probe.clear_committed).await;
    let joined_at_clear = probe.clear_saw_joined.load(Ordering::SeqCst);
    // On the unchanged old ordering, this is an actual monitor read of our
    // committed clear while derived-index refresh is still held. No fake error.
    if !joined_at_clear {
        entered(&probe.read_cleared).await;
    }
    probe.release_clear.add_permits(1);
    let result = tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    let retained = found(&jobs.for_tenant(tenant("org:other"))).await;
    service.shutdown().await;
    println!(
        "W02_COMPLETION_RETIREMENT terminal={terminal} joined_at_clear={joined_at_clear} actual_result={result:?} renewed={} retained_state={} calls={}",
        probe.renewed.load(Ordering::SeqCst),
        retained.state,
        engine.calls.load(Ordering::SeqCst)
    );
    assert!(
        joined_at_clear,
        "actual authority monitor was not joined before committed ownership clear; result={result:?}"
    );
    let result = result.unwrap();
    assert!(result.lease_token.is_none() && result.owner_id.is_none());
    assert_eq!(result, retained);
    assert!(
        probe.renewed.load(Ordering::SeqCst) > 0,
        "real monitor did not participate"
    );
    assert_eq!(probe.read_cleared.available_permits(), 0);
    assert_eq!(engine.calls.load(Ordering::SeqCst), usize::from(!terminal));
    if terminal {
        assert_eq!(retained.queries, expected_terminal_queries.unwrap());
    }
}
#[tokio::test]
async fn initial_completion_joins_actual_monitor_before_own_clear() {
    checked_completion(false).await;
}
#[tokio::test]
async fn terminal_early_return_joins_actual_monitor_before_own_clear() {
    checked_completion(true).await;
}
#[tokio::test]
async fn reclaimer_after_monitor_join_before_clear_preserves_replacement_lease_and_evidence() {
    let server = Server::start();
    let engine = engine(false);
    let (mut runner, _, service, _) = setup(&server, engine.clone());
    let probe = Probe::new(true);
    runner.jobs = runner.jobs.clone().with_completion_probe(probe.clone());
    let scoped = runner.jobs.for_tenant(tenant("org:other"));
    scoped.create(&completed_job()).await.unwrap();
    let claim = scoped
        .claim_prepared(
            &id(),
            "old-owner",
            NonZeroU64::new(1).unwrap(),
            &mut super::RandomTokens,
            now(),
            super::attempt,
            false,
            |_| Ok(()),
        )
        .await
        .unwrap()
        .unwrap();
    let (_stop, rx) = shutdown();
    let task = tokio::spawn(async move { runner.execute_claim(claim, rx).await });
    entered(&probe.before_clear).await;
    assert!(probe.monitor_joined.load(Ordering::SeqCst));
    let lease_key = keys::research("lease", &tenant("org:other"), id().as_str()).unwrap();
    redis::cmd("DEL")
        .arg(&lease_key)
        .query::<u64>(&mut server.connection())
        .unwrap();
    let replacement = scoped
        .claim_prepared(
            &id(),
            "replacement",
            NonZeroU64::new(5).unwrap(),
            &mut super::RandomTokens,
            now(),
            super::attempt,
            true,
            |_| Ok(()),
        )
        .await
        .unwrap()
        .unwrap();
    let authoritative = found(&scoped).await;
    probe.release_before_clear.add_permits(1);
    let result = tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    let retained = found(&scoped).await;
    let lease: String = redis::cmd("GET")
        .arg(&lease_key)
        .query(&mut server.connection())
        .unwrap();
    service.shutdown().await;
    assert!(matches!(
        result,
        Err(RunnerError::Store(Error::OwnershipLost))
    ));
    assert_eq!(retained, authoritative);
    assert_eq!(lease, replacement.token.as_str());
    assert_eq!(probe.clear_committed.available_permits(), 0);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    println!(
        "W02_RECLAIMER_RETIREMENT monitor_joined=true stale_clear_rejected=true replacement_record_and_lease_preserved=true calls=0"
    );
}
