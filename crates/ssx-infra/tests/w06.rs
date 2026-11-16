//! Actual owned staged acquisition: private Valkey and recorded loopback HTTP only.
use ssx_application::{
    NoopSink, PortFuture, SearchEngine, cache::RoutingIdentity, policy::*, scope::*, snapshot::*,
    staged::*,
};
use ssx_domain::*;
use ssx_infra::{
    search::*,
    staged_runner::{NoEvents, StagedRunner, StaticStagedAuthority},
    valkey::{snapshot::*, staged::*, *},
};
use ssx_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::Read as _,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::watch,
};
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
        let dir = std::env::temp_dir().join(format!("ssx-w06-{id}"));
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

struct LoopbackEngine {
    url: String,
    requests: Arc<Mutex<Vec<SearchRequest>>>,
}
impl SearchEngine for LoopbackEngine {
    fn search<'a>(&'a self, r: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            self.requests.lock().unwrap().push(r.clone());
            let body = reqwest::Client::builder()
                .no_proxy()
                .build()
                .unwrap()
                .get({
                    let mut url = url::Url::parse(&self.url).unwrap();
                    url.query_pairs_mut().append_pair("q", &r.query);
                    url
                })
                .send()
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap();
            let wire: Value = ssx_json::from_slice(&body).unwrap();
            let rows: Vec<Value> = if wire.is_array() {
                wire.as_array().unwrap().clone()
            } else {
                wire["results"].as_array().unwrap().clone()
            };
            let status = match wire["status"].as_str() {
                Some("error") => EngineStatus::Error,
                Some("timeout") => EngineStatus::Timeout,
                Some("blocked") => EngineStatus::Blocked,
                _ => EngineStatus::Ok,
            };
            let results = rows
                .into_iter()
                .map(|v| storage::decode_result(v).unwrap())
                .collect();
            AdapterResponse {
                results,
                status,
                ..Default::default()
            }
        })
    }
}
struct Exchange {
    url: String,
    received: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
    release: Arc<tokio::sync::Notify>,
    closed: Arc<std::sync::atomic::AtomicUsize>,
}
impl Drop for Exchange {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Exchange {
    async fn shutdown(&mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }
}
async fn exchange(body: &str) -> Exchange {
    exchange_gated(body, false).await
}
async fn exchange_gated(body: &str, gated: bool) -> Exchange {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/fixture", listener.local_addr().unwrap());
    let received = Arc::new(Mutex::new(vec![]));
    let seen = received.clone();
    let body = body.to_string();
    let release = Arc::new(tokio::sync::Notify::new());
    let gate = release.clone();
    let closed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let closed_streams = closed.clone();
    let task = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = vec![];
            loop {
                let mut b = [0; 4096];
                let n = stream.read(&mut b).await.unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&b[..n]);
                if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            seen.lock().unwrap().push(String::from_utf8(bytes).unwrap());
            if gated {
                let mut one = [0; 1];
                tokio::select! {_=gate.notified()=>(),r=stream.read(&mut one)=>{if r.unwrap_or(0)==0{closed_streams.fetch_add(1,std::sync::atomic::Ordering::SeqCst);continue;}}}
            }
            let wire = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(wire.as_bytes()).await.unwrap();
        }
    });
    Exchange {
        url,
        received,
        task,
        release,
        closed,
    }
}
fn input() -> Input {
    Input {
        query: "canonical search".into(),
        objectives: json!({"deadline_ms":30000,"max_engine_calls":4}),
        initial_scope: json!({"engines":["alpha"]}),
        fallback_scope: Some(json!({"engines":["beta"]})),
        allow_scope_expansion: true,
        filters: None,
    }
}
fn limits() -> Limits {
    Limits {
        granted: true,
        ..Default::default()
    }
}
fn snapshot_service(store: Arc<Valkey>, tenant: TenantId) -> SnapshotService {
    SnapshotService::new(
        Arc::new(ValkeySnapshots::new(store)),
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        tenant,
        SnapshotTtl::new(3600).unwrap(),
    )
}
#[tokio::test]
async fn nonempty_initial_real_acquisition_snapshot_fence_replay() {
    let server = Server::start();
    let backend = Arc::new(server.store());
    let tenant = TenantId::new("org:team").unwrap();
    let id = StagedSearchId::new("native-vertical").unwrap();
    let result = SearchResult {
        title: "Full canonical result".into(),
        url: "https://example.test/evidence".into(),
        engine: "alpha".into(),
        engines: ["alpha".into()].into_iter().collect(),
        content: "retained body".into(),
        payload: Some(json!({"marker":"unsliced"})),
        ..Default::default()
    };
    let results: Vec<_> = (0..3)
        .map(|i| SearchResult {
            title: format!("{} {i}", result.title),
            url: format!("{}-{i}", result.url),
            payload: Some(json!({"marker":"unsliced","row":i})),
            ..result.clone()
        })
        .collect();
    let encoded: Vec<_> = results
        .iter()
        .map(|r| storage::encode_result(r).unwrap())
        .collect();
    let mut alpha = exchange(&ssx_json::to_string(&encoded).unwrap()).await;
    let mut beta = exchange("[]").await;
    let requests = Arc::new(Mutex::new(vec![]));
    let deps = dependencies(vec![
        (
            "alpha",
            Arc::new(LoopbackEngine {
                url: alpha.url.clone(),
                requests: requests.clone(),
            }),
        ),
        (
            "beta",
            Arc::new(LoopbackEngine {
                url: beta.url.clone(),
                requests: requests.clone(),
            }),
        ),
    ]);
    let policy = deps.policy.clone();
    let search = SearchService::new(SearchConfig::default(), deps).unwrap();
    let snapshots = snapshot_service(backend.clone(), tenant.clone());
    let plan = Plan::build(&policy, &limits(), &input()).unwrap();
    let now = SystemSnapshotClock.now();
    let record = plan.record(&tenant, &id, now.seconds()).unwrap();
    let store = StagedStore::new(&backend);
    let tokens = plan.scopes.iter().collect::<Vec<_>>();
    assert_eq!(
        store
            .admit(&policy, &record, "once", &tokens, now)
            .await
            .unwrap()
            .status,
        scripts::Staged::Created
    );
    let runner = StagedRunner::new(
        backend.clone(),
        search.clone(),
        snapshots.clone(),
        Arc::new(StaticStagedAuthority {
            policy: policy.clone(),
            limits: limits(),
        }),
        Arc::new(SystemSnapshotClock),
        Arc::new(NoEvents),
    );
    let (_stop, rx) = watch::channel(false);
    let completed = runner
        .run_one(tenant.clone(), id.clone(), rx.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.state(), "completed");
    assert_eq!(completed.value()["stop_reason"], json!("initial_nonempty"));
    assert_eq!(completed.value()["stages"][1]["state"], json!("skipped"));
    assert_eq!(
        completed.value()["budget"],
        json!({"unit":"adapter_search_invocations","limit":4,"reserved":1,"observed":1})
    );
    let a = &completed.value()["stages"][0]["attempts"][0];
    assert_eq!(a["engine_outcomes"][0]["engine"], json!("alpha"));
    assert_eq!(a["engine_outcomes"][0]["status"], json!("ok"));
    assert_eq!(a["result_count"], json!(3));
    assert_eq!(
        a["attempt_id"],
        completed.value()["selected_result_attempt_id"]
    );
    let sid = SnapshotId::new(a["cursor"].as_str().unwrap()).unwrap();
    let SnapshotLookup::Found(snapshot) = snapshots.read(&sid).await else {
        panic!("snapshot missing")
    };
    assert_eq!(snapshot.total(), 3);
    for (actual, expected) in snapshot.results().iter().zip(&results) {
        assert_eq!(actual.title, expected.title);
        assert_eq!(actual.url, expected.url);
        assert_eq!(actual.content, expected.content);
        assert_eq!(actual.payload, expected.payload);
        assert_eq!(actual.engines, expected.engines);
    }
    let canonical_before = snapshot
        .results()
        .iter()
        .map(|r| storage::encode_result(r).unwrap())
        .collect::<Vec<_>>();
    let service = ssx_infra::staged_service::Service {
        store: backend.clone(),
        policy: policy.clone(),
        limits: limits(),
        authority: Arc::new(StaticStagedAuthority {
            policy: policy.clone(),
            limits: limits(),
        }),
        clock: Arc::new(SystemSnapshotClock),
        snapshots: snapshots.clone(),
        sources: None,
        events: Arc::new(NoEvents),
    };
    let view = ssx_infra::staged_service::View::parse(
        Some(&json!(["results", "payload"])),
        Some(&json!(1)),
        20,
    )
    .unwrap();
    let rendered = service
        .render(&completed, &view, |r, _, payload| {
            assert!(payload);
            storage::encode_result(r).unwrap()
        })
        .await
        .unwrap();
    assert_eq!(rendered["results"], json!([canonical_before[0].clone()]));
    assert_eq!(rendered["meta"]["total"], json!(3));
    assert_eq!(rendered["meta"]["returned"], json!(1));
    assert_eq!(rendered["meta"]["has_more"], json!(true));
    assert_eq!(service.get(&tenant, &id).await.unwrap(), completed);
    let SnapshotLookup::Found(retained) = snapshots.read(&sid).await else {
        panic!("retained snapshot missing")
    };
    assert_eq!(
        retained
            .results()
            .iter()
            .map(|r| storage::encode_result(r).unwrap())
            .collect::<Vec<_>>(),
        canonical_before
    );
    assert_eq!(retained.total(), 3);
    assert!(matches!(
        snapshots
            .for_tenant(TenantId::new("other").unwrap())
            .read(&sid)
            .await,
        SnapshotLookup::Missing
    ));
    assert!(runner.run_one(tenant, id, rx).await.unwrap().is_none());
    let RecordRead::Found(replayed) = store
        .find_by_idempotency(&record.tenant(), "once", SystemSnapshotClock.now())
        .await
        .unwrap()
    else {
        panic!("replay missing")
    };
    assert_eq!(completed, replayed);
    assert_eq!(alpha.received.lock().unwrap().len(), 1);
    assert!(beta.received.lock().unwrap().is_empty());
    assert!(
        alpha.received.lock().unwrap()[0].starts_with("GET /fixture?q=canonical+search HTTP/1.1")
    );
    let r = requests.lock().unwrap()[0].clone();
    assert_eq!(r.engines, Some(vec!["alpha".into()]));
    assert!(!r.generate_suggestions);
    assert!(r.interactive_timeout_ms.unwrap() <= 30000);
    assert_eq!(
        r.execution_isolation_key,
        Some(format!(
            "native-vertical:{}",
            a["attempt_id"].as_str().unwrap()
        ))
    );
    search.shutdown().await;
    alpha.shutdown().await;
    beta.shutdown().await;
    backend.close().await;
    case_complete(
        "native.nonempty_initial_real_acquisition_snapshot_fence_replay",
        "nonempty_initial_real_acquisition_snapshot_fence_replay",
    );
}

#[derive(Default)]
struct EventLog(Mutex<Vec<ssx_infra::staged_runner::Event>>);
impl ssx_infra::staged_runner::Events for EventLog {
    fn acknowledged(&self, event: ssx_infra::staged_runner::Event) {
        self.0.lock().unwrap().push(event);
    }
}
struct Authority {
    policy: PolicyService,
    limits: Mutex<Limits>,
}
impl ssx_infra::staged_runner::StagedAuthority for Authority {
    fn validate(
        &self,
        r: &ssx_domain::staged::Record,
    ) -> Result<(), ssx_application::staged::Error> {
        revalidate(&self.policy, &self.limits.lock().unwrap(), r)
    }
}
struct FixedTime(Mutex<f64>);
impl SnapshotClock for FixedTime {
    fn now(&self) -> WallClockTimestamp {
        WallClockTimestamp::new(*self.0.lock().unwrap()).unwrap()
    }
}
struct Harness {
    clock: Arc<FixedTime>,
    server: Server,
    backend: Arc<Valkey>,
    tenant: TenantId,
    alpha: Exchange,
    beta: Exchange,
    requests: Arc<Mutex<Vec<SearchRequest>>>,
    search: SearchService,
    runner: StagedRunner,
    service: ssx_infra::staged_service::Service,
    events: Arc<EventLog>,
    authority: Arc<Authority>,
}
impl Harness {
    async fn new(alpha_body: &str, beta_body: &str, cached: bool) -> Self {
        Self::new_gated(alpha_body, beta_body, cached, false).await
    }
    async fn new_gated(alpha_body: &str, beta_body: &str, cached: bool, gated: bool) -> Self {
        let server = Server::start();
        let backend = Arc::new(server.store());
        let tenant = TenantId::new("org:team").unwrap();
        let alpha = exchange_gated(alpha_body, gated).await;
        let beta = exchange(beta_body).await;
        let requests = Arc::new(Mutex::new(vec![]));
        let mut deps = dependencies(vec![
            (
                "alpha",
                Arc::new(LoopbackEngine {
                    url: alpha.url.clone(),
                    requests: requests.clone(),
                }),
            ),
            (
                "beta",
                Arc::new(LoopbackEngine {
                    url: beta.url.clone(),
                    requests: requests.clone(),
                }),
            ),
        ]);
        if cached {
            deps.cache = Some(Arc::new(ssx_application::cache::CanonicalCache::new(
                Arc::new(cache::ValkeyCache::new(backend.clone())),
                ssx_application::cache::CacheTtls::default(),
            )));
        }
        let clock = Arc::new(FixedTime(Mutex::new(
            SystemSnapshotClock.now().seconds().floor(),
        )));
        let policy = deps.policy.clone();
        let search = SearchService::new(SearchConfig::default(), deps).unwrap();
        let snapshots = snapshot_service(backend.clone(), tenant.clone());
        let events = Arc::new(EventLog::default());
        let authority = Arc::new(Authority {
            policy: policy.clone(),
            limits: Mutex::new(limits()),
        });
        let runner = StagedRunner::new(
            backend.clone(),
            search.clone(),
            snapshots.clone(),
            authority.clone(),
            clock.clone(),
            events.clone(),
        );
        let service = ssx_infra::staged_service::Service {
            store: backend.clone(),
            policy,
            limits: limits(),
            authority: authority.clone(),
            clock: clock.clone(),
            snapshots,
            sources: None,
            events: events.clone(),
        };
        Self {
            clock,
            server,
            backend,
            tenant,
            alpha,
            beta,
            requests,
            search,
            runner,
            service,
            events,
            authority,
        }
    }
    fn submission(&self, key: &str) -> ssx_infra::staged_service::Submission {
        let i = input();
        ssx_infra::staged_service::Submission {
            query: Some(i.query),
            objectives: i.objectives,
            initial_scope: i.initial_scope,
            fallback_scope: i.fallback_scope,
            allow_scope_expansion: i.allow_scope_expansion,
            filters: i.filters,
            idempotency_key: key.into(),
            source: None,
        }
    }
    async fn admit(&self, key: &str) -> ssx_domain::staged::Record {
        self.service
            .submit(&self.tenant, &self.submission(key))
            .await
            .unwrap()
    }
    async fn run_one(&self, id: StagedSearchId) -> Option<ssx_domain::staged::Record> {
        let (_stop, rx) = watch::channel(false);
        self.runner
            .run_one(self.tenant.clone(), id, rx)
            .await
            .unwrap()
    }
    async fn close(mut self) {
        self.search.shutdown().await;
        self.alpha.shutdown().await;
        self.beta.shutdown().await;
    }
}
#[tokio::test]
async fn clean_empty_runs_disjoint_fallback_with_owned_histories() {
    let h = Harness::new("[]", "[]", false).await;
    let admitted = h.admit("fallback").await;
    let first = h.run_one(admitted.id()).await.unwrap();
    assert_eq!(first.state(), "queued");
    assert_eq!(first.value()["next_stage"], json!(1));
    let original = ssx_json::to_string(first.value()).unwrap();
    let normalized: String = redis::cmd("EVAL")
        .arg("return cjson.encode(cjson.decode(ARGV[1]))")
        .arg(0)
        .arg(original)
        .query(&mut h.server.connection())
        .unwrap();
    let immutable = ssx_domain::staged::Record::decode(normalized.as_bytes())
        .unwrap()
        .value()["stages"][0]
        .clone();
    let done = h.run_one(admitted.id()).await.unwrap();
    assert_eq!(done.state(), "completed");
    assert_eq!(done.value()["stop_reason"], json!("clean_empty"));
    assert_eq!(done.value()["stages"][0], immutable);
    assert_eq!(done.value()["budget"]["reserved"], json!(2));
    assert_eq!(done.value()["budget"]["observed"], json!(2));
    assert_eq!(h.alpha.received.lock().unwrap().len(), 1);
    assert_eq!(h.beta.received.lock().unwrap().len(), 1);
    let view = ssx_infra::staged_service::View::parse(None, None, 5).unwrap();
    let rendered = h
        .service
        .render(&done, &view, |r, _, _| storage::encode_result(r).unwrap())
        .await
        .unwrap();
    assert_eq!(rendered["meta"]["result_status"], json!("available"));
    assert_eq!(rendered["meta"]["total"], json!(0));
    assert_eq!(rendered["budget"]["remaining"], json!(2));
    assert_eq!(
        h.events
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.to.as_str())
            .collect::<Vec<_>>(),
        vec!["queued", "running", "queued", "running", "completed"]
    );
    h.close().await;
    case_complete(
        "native.clean_empty_runs_disjoint_fallback_with_owned_histories",
        "clean_empty_runs_disjoint_fallback_with_owned_histories",
    );
}
#[tokio::test]
async fn failed_empty_initial_never_authorizes_fallback() {
    for status in ["error", "timeout", "blocked"] {
        let h = Harness::new(
            &format!("{{\"results\":[],\"status\":\"{status}\"}}"),
            "[]",
            false,
        )
        .await;
        let r = h.admit(status).await;
        let failed = h.run_one(r.id()).await.unwrap();
        assert_eq!(failed.state(), "failed");
        assert_eq!(
            failed.value()["stages"][0]["attempts"][0]["engine_outcomes"][0]["status"],
            json!(status)
        );
        assert_eq!(failed.value()["stop_reason"], json!("execution_failed"));
        assert_eq!(
            failed.value()["objectives"]["unmet"],
            json!([{"objective":"fallback","reason":"initial_stage_failed"}])
        );
        assert!(h.beta.received.lock().unwrap().is_empty());
        h.close().await;
        case_complete(
            &format!("native.failed_empty.{status}"),
            "failed_empty_initial_never_authorizes_fallback",
        );
    }
    // One real shared-search stage contains a successful empty engine and an
    // errored engine. Fallback remains a separate, undispatched third engine.
    let h = Harness::new("[]", "{\"results\":[],\"status\":\"error\"}", false).await;
    let mut fallback = exchange("[]").await;
    let mut deps = dependencies(vec![
        (
            "alpha",
            Arc::new(LoopbackEngine {
                url: h.alpha.url.clone(),
                requests: h.requests.clone(),
            }),
        ),
        (
            "beta",
            Arc::new(LoopbackEngine {
                url: h.beta.url.clone(),
                requests: h.requests.clone(),
            }),
        ),
        (
            "gamma",
            Arc::new(LoopbackEngine {
                url: fallback.url.clone(),
                requests: h.requests.clone(),
            }),
        ),
    ]);
    deps.cache = Some(Arc::new(ssx_application::cache::CanonicalCache::new(
        Arc::new(cache::ValkeyCache::new(h.backend.clone())),
        ssx_application::cache::CacheTtls::default(),
    )));
    let policy = deps.policy.clone();
    let search = SearchService::new(SearchConfig::default(), deps).unwrap();
    let authority = Arc::new(StaticStagedAuthority {
        policy: policy.clone(),
        limits: limits(),
    });
    let mut service = h.service.clone();
    service.policy = policy;
    service.authority = authority.clone();
    let mut submission = h.submission("mixed-partial");
    submission.initial_scope = json!({"engines":["alpha","beta"]});
    submission.fallback_scope = Some(json!({"engines":["gamma"]}));
    let admitted = service.submit(&h.tenant, &submission).await.unwrap();
    let runner = StagedRunner::new(
        h.backend.clone(),
        search.clone(),
        service.snapshots.clone(),
        authority,
        h.clock.clone(),
        h.events.clone(),
    );
    let (_stop, rx) = watch::channel(false);
    let failed = runner
        .run_one(h.tenant.clone(), admitted.id(), rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.state(), "failed");
    assert_eq!(failed.value()["stop_reason"], json!("execution_failed"));
    assert_eq!(
        failed.value()["objectives"]["unmet"],
        json!([{"objective":"fallback","reason":"initial_stage_failed"}])
    );
    assert_eq!(
        failed.value()["budget"],
        json!({"unit":"adapter_search_invocations","limit":4,"reserved":2,"observed":2})
    );
    assert!(
        failed.value()["stages"][1]["attempts"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let attempt = &failed.value()["stages"][0]["attempts"][0];
    let outcomes = attempt["engine_outcomes"].as_array().unwrap();
    assert_eq!(outcomes.len(), 2);
    assert_eq!(
        outcomes
            .iter()
            .map(|o| (o["engine"].as_str().unwrap(), o["status"].as_str().unwrap()))
            .collect::<BTreeMap<_, _>>(),
        BTreeMap::from([("alpha", "ok"), ("beta", "error")])
    );
    assert_eq!(attempt["result_count"], json!(0));
    assert_eq!(attempt["observed_engine_calls"], json!(2));
    assert_eq!(attempt["error"], Value::Null);
    let sid = SnapshotId::new(attempt["cursor"].as_str().unwrap()).unwrap();
    let SnapshotLookup::Found(snapshot) = service.snapshots.read(&sid).await else {
        panic!("mixed partial snapshot missing")
    };
    assert!(snapshot.results().is_empty());
    assert_eq!(snapshot.scope().selected_engines, vec!["alpha", "beta"]);
    let cache_keys: Vec<String> = redis::cmd("KEYS")
        .arg("search:*")
        .query(&mut h.server.connection())
        .unwrap();
    assert_eq!(cache_keys.len(), 1);
    let canonical: String = redis::cmd("GET")
        .arg(&cache_keys[0])
        .query(&mut h.server.connection())
        .unwrap();
    let response = storage::decode_response(ssx_json::from_str(&canonical).unwrap()).unwrap();
    assert!(response.partial);
    assert!(!response.cached);
    assert!(!response.all_unresponsive);
    assert!(!response.deadline_exceeded);
    assert_eq!(response.dispatched_engine_count, 2);
    assert_eq!(json!(response.engine_outcomes), attempt["engine_outcomes"]);
    assert!(response.results.is_empty());
    assert_eq!(h.alpha.received.lock().unwrap().len(), 1);
    assert_eq!(h.beta.received.lock().unwrap().len(), 1);
    assert!(fallback.received.lock().unwrap().is_empty());
    assert_eq!(h.requests.lock().unwrap().len(), 2);
    assert!(
        h.requests
            .lock()
            .unwrap()
            .iter()
            .all(|r| r.engines == Some(vec!["alpha".into(), "beta".into()]))
    );
    assert!(
        runner
            .run_one(h.tenant.clone(), admitted.id(), _stop.subscribe())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        service.get(&h.tenant, &admitted.id()).await.unwrap(),
        failed
    );
    search.shutdown().await;
    fallback.shutdown().await;
    h.close().await;
    case_complete(
        "native.failed_empty.mixed_partial",
        "failed_empty_initial_never_authorizes_fallback",
    );
}
#[tokio::test]
async fn cached_empty_never_authorizes_fallback_observed_zero() {
    let h = Harness::new("[]", "[]", true).await;
    let prefill = h
        .search
        .search(ScopeProposal::generic(SearchRequest {
            query: input().query,
            interactive_timeout_ms: Some(30000),
            engines: Some(vec!["alpha".into()]),
            generate_suggestions: false,
            ..Default::default()
        }))
        .await
        .unwrap();
    assert!(!prefill.cached);
    let r = h.admit("cached-empty").await;
    let failed = h.run_one(r.id()).await.unwrap();
    assert_eq!(failed.state(), "failed");
    assert_eq!(failed.value()["budget"]["reserved"], json!(1));
    assert_eq!(failed.value()["budget"]["observed"], json!(0));
    assert_eq!(
        failed.value()["stages"][0]["attempts"][0]["cached"],
        json!(true)
    );
    assert_eq!(
        failed.value()["objectives"]["unmet"],
        json!([{"objective":"fallback","reason":"initial_stage_not_clean"}])
    );
    assert_eq!(h.alpha.received.lock().unwrap().len(), 1);
    assert!(h.beta.received.lock().unwrap().is_empty());
    h.close().await;
    case_complete(
        "native.cached_empty_never_authorizes_fallback_observed_zero",
        "cached_empty_never_authorizes_fallback_observed_zero",
    );
}
#[tokio::test]
async fn preview_strict_integer_scope_filters_and_replay_identity() {
    let h = Harness::new("[]", "[]", false).await;
    let good = input();
    let preview = h.service.preview(&good).unwrap();
    assert_eq!(preview["dispatch"], json!(false));
    assert_eq!(preview, h.service.preview(&good).unwrap());
    assert!(h.alpha.received.lock().unwrap().is_empty());
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("mcp:staged:*")
        .query(&mut h.server.connection())
        .unwrap();
    assert!(keys.is_empty());
    for value in [json!(true), json!(0), json!(30001), json!(1.0), json!(null)] {
        let mut i = good.clone();
        i.objectives["deadline_ms"] = value;
        assert_eq!(
            h.service.preview(&i).unwrap_err().field,
            "objectives.deadline_ms"
        );
    }
    for value in [json!(true), json!(0), json!(65), json!(1.0)] {
        let mut i = good.clone();
        i.objectives["max_engine_calls"] = value;
        assert_eq!(
            h.service.preview(&i).unwrap_err().field,
            "objectives.max_engine_calls"
        );
    }
    let language_evidence: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w06/evidence/language-input-source-outcomes.json"
    ))
    .unwrap();
    for row in language_evidence["cases"].as_array().unwrap() {
        let mut i = good.clone();
        i.filters = Some(row["input"]["filters"].clone());
        assert_eq!(
            h.service.preview(&i).unwrap_err().value(),
            json!({"error":{"code":"invalid_input","field":"filters.language","message":"language must be a string"}})
        );
        assert!(!row["plan"].is_null());
    }
    let mut overlap = good.clone();
    overlap.fallback_scope = Some(overlap.initial_scope.clone());
    overlap.filters = Some(json!({"safesearch":"strict"}));
    assert_eq!(
        h.service.preview(&overlap).unwrap_err().code,
        "objective_conflict"
    );
    let mut strict = good.clone();
    strict.filters = overlap.filters;
    assert_eq!(
        h.service.preview(&strict).unwrap_err().code,
        "policy_rejected"
    );
    for query in ["", "  ", "\u{1c}"] {
        let mut i = good.clone();
        i.query = query.into();
        i.objectives = json!({});
        assert_eq!(h.service.preview(&i).unwrap_err().field, "query");
    }
    let mut long = good.clone();
    long.query = "é".repeat(501);
    assert_eq!(h.service.preview(&long).unwrap_err().field, "query");
    let admitted = h.admit("same").await;
    let replay = h.admit("same").await;
    assert_eq!(admitted, replay);
    let mut changed = h.submission("same");
    changed.objectives["deadline_ms"] = json!(29000);
    assert_eq!(
        h.service
            .submit(&h.tenant, &changed)
            .await
            .unwrap_err()
            .code,
        "idempotency_conflict"
    );
    let mut broadened = h.service.clone();
    broadened.limits.deadline_ms = 30001;
    assert_eq!(
        admitted,
        broadened
            .submit(&h.tenant, &h.submission("same"))
            .await
            .unwrap()
    );
    h.authority.limits.lock().unwrap().engine_calls = 1;
    assert_eq!(
        h.service
            .get(&h.tenant, &admitted.id())
            .await
            .unwrap_err()
            .code,
        "policy_rejected"
    );
    assert_eq!(
        h.service
            .submit(&h.tenant, &h.submission("same"))
            .await
            .unwrap_err()
            .code,
        "policy_rejected"
    );
    assert!(h.alpha.received.lock().unwrap().is_empty());
    h.close().await;
    for row in language_evidence["cases"].as_array().unwrap() {
        case_complete(
            &format!("hardening.{}", row["id"].as_str().unwrap()),
            "preview_strict_integer_scope_filters_and_replay_identity",
        );
    }
    case_complete(
        "native.preview_strict_integer_scope_filters_and_replay_identity",
        "preview_strict_integer_scope_filters_and_replay_identity",
    );
}

async fn entered(h: &Harness) {
    let until = Instant::now() + Duration::from_secs(3);
    while h.alpha.received.lock().unwrap().is_empty() {
        assert!(Instant::now() < until, "HTTP did not arrive");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
fn lease_key(tenant: &TenantId, id: &StagedSearchId) -> String {
    keys::staged("lease", tenant, Some(id.as_str())).unwrap()
}
fn record_key(tenant: &TenantId, id: &StagedSearchId) -> String {
    keys::staged("record", tenant, Some(id.as_str())).unwrap()
}
#[tokio::test]
async fn acknowledged_deadline_budget_claims_and_recovery_are_not_inferred() {
    let h = Harness::new("{\"results\":[],\"status\":\"error\"}", "[]", false).await;
    let deadline = h.admit("deadline-ack").await;
    *h.clock.0.lock().unwrap() += 31.;
    assert!(h.run_one(deadline.id()).await.is_none());
    assert!(h.run_one(deadline.id()).await.is_none());
    let observed = h
        .events
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.from == Some("queued") && e.to == "failed" && e.record.id() == deadline.id())
        .map(|e| e.record.clone())
        .collect::<Vec<_>>();
    assert_eq!(observed.len(), 1);
    assert_eq!(
        observed[0].value()["stop_reason"],
        json!("deadline_expired")
    );
    assert!(
        observed[0].value()["stages"][0]["attempts"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(h.alpha.received.lock().unwrap().is_empty());
    let budget = h.admit("budget-ack").await;
    let failed = h.run_one(budget.id()).await.unwrap();
    let mut v = failed.value().clone();
    let a = v["stages"][0]["attempts"][0].clone();
    let mut attempts = vec![];
    for i in 0..4 {
        let mut a = a.clone();
        a["attempt_id"] = json!(format!("spent-{i}"));
        attempts.push(a);
    }
    v["stages"].as_array_mut().unwrap()[0]["attempts"] = json!(attempts);
    v["selected_result_attempt_id"] = json!("spent-3");
    v["budget"]["reserved"] = json!(4);
    v["budget"]["observed"] = json!(4);
    v["state"] = json!("queued");
    let queued = ssx_domain::staged::Record::from_value(v).unwrap();
    h.backend
        .put(
            &record_key(&h.tenant, &budget.id()),
            &queued.encode().unwrap(),
            std::num::NonZeroU64::new(3600).unwrap(),
        )
        .await
        .unwrap();
    assert!(h.run_one(budget.id()).await.is_none());
    assert!(h.run_one(budget.id()).await.is_none());
    let events = h.events.0.lock().unwrap().clone();
    let claims = events
        .iter()
        .filter(|e| e.from == Some("queued") && e.to == "failed" && e.record.id() == budget.id())
        .collect::<Vec<_>>();
    assert_eq!(claims.len(), 1);
    assert_eq!(
        claims[0].record.value()["stop_reason"],
        json!("budget_exhausted")
    );
    assert_eq!(claims[0].record.value()["budget"]["reserved"], json!(4));
    drop(events);
    h.close().await;
    case_complete(
        "native.acknowledged_deadline_budget_claims_and_recovery_are_not_inferred",
        "acknowledged_deadline_budget_claims_and_recovery_are_not_inferred",
    );
}
#[tokio::test]
async fn duplicate_owners_and_index_loss_conserve_unknown_retry_budgets() {
    let h = Harness::new_gated("[]", "[]", false, true).await;
    let original = h.admit("unknown-retry").await;
    let runner = h
        .runner
        .clone()
        .with_renewal_interval(Duration::from_millis(20))
        .unwrap();
    let tenant = h.tenant.clone();
    let id = original.id();
    let (stop, rx) = watch::channel(false);
    let owned = tokio::spawn(async move { runner.run_one(tenant, id, rx).await });
    entered(&h).await;
    assert!(h.run_one(original.id()).await.is_none());
    assert_eq!(h.requests.lock().unwrap().len(), 1);
    let mut conn = h.server.connection();
    let _: i64 = redis::cmd("DEL")
        .arg(lease_key(&h.tenant, &original.id()))
        .arg(keys::STAGED_RUNNING)
        .arg(keys::STAGED_READY)
        .query(&mut conn)
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(2), owned)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        result,
        Err(ssx_infra::staged_runner::Error::LeaseLost)
    ));
    drop(stop);
    let mut acknowledgments = vec![];
    let store = StagedStore::new(&h.backend);
    let mut cursor = 0;
    loop {
        cursor = store
            .reconcile_indexes_acknowledged(cursor, h.clock.now(), |r| acknowledgments.push(r))
            .await
            .unwrap();
        if cursor == 0 {
            break;
        }
    }
    assert_eq!(acknowledgments.len(), 1);
    let interrupted = acknowledgments.pop().unwrap();
    assert_eq!(interrupted.state(), "interrupted");
    assert_eq!(interrupted.value()["budget"]["observed"], Value::Null);
    assert_eq!(interrupted.value()["budget"]["reserved"], json!(1));
    assert_eq!(
        interrupted.value()["selected_result_attempt_id"],
        Value::Null
    );
    assert_eq!(
        store
            .recover_orphans_acknowledged(h.clock.now(), |_| panic!(
                "duplicate recovery acknowledgment"
            ))
            .await
            .unwrap(),
        0
    );
    let queued = h
        .service
        .retry(&h.tenant, &original.id(), "manual-retry")
        .await
        .unwrap();
    let replay = h
        .service
        .retry(&h.tenant, &original.id(), "manual-retry")
        .await
        .unwrap();
    assert_eq!(queued, replay);
    h.alpha.release.notify_one(); // drain the cancelled first socket before the retry
    let runner = h.runner.clone();
    let tenant = h.tenant.clone();
    let id = original.id();
    let (_stop, rx) = watch::channel(false);
    let task = tokio::spawn(async move { runner.run_one(tenant, id, rx).await });
    let until = Instant::now() + Duration::from_secs(3);
    while h.requests.lock().unwrap().len() < 2 {
        assert!(Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    h.alpha.release.notify_one();
    let first = task.await.unwrap().unwrap().unwrap();
    assert_eq!(first.state(), "queued");
    assert_eq!(first.value()["budget"]["observed"], Value::Null);
    assert_eq!(first.value()["budget"]["reserved"], json!(2));
    assert_eq!(
        first.value()["stages"][0]["attempts"][0]["attempt_id"],
        interrupted.value()["stages"][0]["attempts"][0]["attempt_id"]
    );
    assert_eq!(
        first.value()["stages"][0]["attempts"][0]["state"],
        json!("interrupted")
    );
    let done = h.run_one(original.id()).await.unwrap();
    assert_eq!(done.state(), "completed");
    assert_eq!(done.value()["budget"]["observed"], Value::Null);
    assert_eq!(done.value()["budget"]["reserved"], json!(3));
    assert_eq!(h.alpha.received.lock().unwrap().len(), 2);
    assert_eq!(h.beta.received.lock().unwrap().len(), 1);
    h.close().await;
    case_complete(
        "native.duplicate_owners_and_index_loss_conserve_unknown_retry_budgets",
        "duplicate_owners_and_index_loss_conserve_unknown_retry_budgets",
    );
}
#[tokio::test]
async fn owned_shutdown_interrupts_blocked_renewal_and_preserves_shared_flights() {
    let h = Harness::new_gated("[]", "[]", false, true).await;
    let r = h.admit("blocked-renew").await;
    let runner = h
        .runner
        .clone()
        .with_renewal_interval(Duration::from_millis(10))
        .unwrap();
    let tenant = h.tenant.clone();
    let id = r.id();
    let (stop, rx) = watch::channel(false);
    let task = tokio::spawn(async move { runner.run_one(tenant, id, rx).await });
    entered(&h).await;
    let _: String = redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(500)
        .arg("WRITE")
        .query(&mut h.server.connection())
        .unwrap();
    tokio::time::sleep(Duration::from_millis(25)).await;
    let start = Instant::now();
    stop.send_replace(true);
    assert!(matches!(
        tokio::time::timeout(Duration::from_millis(250), task)
            .await
            .unwrap()
            .unwrap(),
        Err(ssx_infra::staged_runner::Error::Shutdown)
    ));
    assert!(start.elapsed() < Duration::from_millis(250));
    assert_eq!(h.events.0.lock().unwrap().len(), 2);
    assert_eq!(h.alpha.received.lock().unwrap().len(), 1);
    assert!(h.beta.received.lock().unwrap().is_empty());
    // An unrelated beta flight still acquires through the shared service.
    let response = h
        .search
        .search(ScopeProposal::generic(SearchRequest {
            query: "unrelated".into(),
            engines: Some(vec!["beta".into()]),
            ..Default::default()
        }))
        .await
        .unwrap();
    assert_eq!(response.engine_outcomes[0].status, "ok");
    assert_eq!(h.beta.received.lock().unwrap().len(), 1);
    h.close().await;
    case_complete(
        "native.owned_shutdown_interrupts_blocked_renewal_and_preserves_shared_flights",
        "owned_shutdown_interrupts_blocked_renewal_and_preserves_shared_flights",
    );
}
#[tokio::test]
async fn policy_revocation_after_acquisition_retains_no_reference() {
    let h = Harness::new_gated("[]", "[]", false, true).await;
    let r = h.admit("revoke-after-dispatch").await;
    let runner = h.runner.clone();
    let tenant = h.tenant.clone();
    let id = r.id();
    let (_stop, rx) = watch::channel(false);
    let task = tokio::spawn(async move { runner.run_one(tenant, id, rx).await });
    entered(&h).await;
    h.authority.limits.lock().unwrap().granted = false;
    h.alpha.release.notify_one();
    assert!(matches!(
        task.await.unwrap(),
        Err(ssx_infra::staged_runner::Error::Policy(_))
    ));
    let RecordRead::Found(record) = StagedStore::new(&h.backend)
        .read(&h.tenant, &r.id(), h.clock.now())
        .await
        .unwrap()
    else {
        panic!("missing revoked record")
    };
    assert_eq!(record.state(), "running");
    assert_eq!(record.value()["selected_result_attempt_id"], Value::Null);
    assert_eq!(record.value()["budget"]["reserved"], json!(1));
    assert_eq!(h.events.0.lock().unwrap().len(), 2);
    assert!(h.beta.received.lock().unwrap().is_empty());
    assert_eq!(
        h.service.get(&h.tenant, &r.id()).await.unwrap_err().code,
        "tool_disabled"
    );
    h.close().await;
    case_complete(
        "native.policy_revocation_after_acquisition_retains_no_reference",
        "policy_revocation_after_acquisition_retains_no_reference",
    );
}
#[tokio::test]
async fn saved_source_membership_lineage_replay_and_error_precedence() {
    use ssx_application::saved::{SavedLimits, SavedPolicy, prepare_create};
    use ssx_infra::{
        saved_runner::{SavedRunner, StaticSavedAuthority},
        staged_service::{SavedReportSources, Sources},
    };
    let h = Harness::new("[]", "[]", false).await;
    let saved = ssx_infra::valkey::saved::SavedStore::new(h.backend.clone(), h.tenant.clone());
    let limits = SavedLimits::default();
    let auth = BTreeMap::from([("alpha".into(), false), ("beta".into(), false)]);
    let policy = SavedPolicy {
        search: &h.service.policy,
        saved_granted: true,
        auth_configured: &auth,
        limits: &limits,
    };
    let saved_id = SavedSearchId::new("source-definition").unwrap();
    let prepared=prepare_create(&policy,h.tenant.as_str(),saved_id.as_str(),&json!({"query":"historical observation query","engines":["alpha"],"interval_seconds":60,"start_immediately":true,"expires_in_seconds":600}),saved.now().await.unwrap()).unwrap();
    saved.create(&prepared).await.unwrap();
    let runner = SavedRunner::new(
        saved.clone(),
        h.search.clone(),
        Arc::new(StaticSavedAuthority {
            search: h.service.policy.clone(),
            saved_granted: true,
            auth_configured: auth.clone(),
            limits: limits.clone(),
        }),
        Duration::from_secs(2),
    )
    .unwrap();
    let (_stop, rx) = watch::channel(false);
    let report = runner.run_one(saved_id.clone(), rx).await.unwrap().unwrap();
    let run_id = report["run_id"].as_str().unwrap();
    saved
        .update(
            &saved_id,
            1,
            &json!({"query":"edited current definition query"}),
            &policy,
        )
        .await
        .unwrap();
    let resolver = SavedReportSources {
        store: saved,
        search: h.service.policy.clone(),
        granted: true,
        auth_configured: auth.clone(),
        limits: limits.clone(),
    };
    let artifact = ArtifactRef::new(
        ArtifactKind::SavedReport,
        canonical::composite_artifact_id(&[saved_id.as_str(), run_id]).unwrap(),
    )
    .unwrap()
    .to_value();
    let resolved = resolver.resolve(&h.tenant, &artifact).await.unwrap();
    assert_eq!(resolved["query"], json!("historical observation query"));
    assert_eq!(
        resolved["details"],
        json!({"search_id":saved_id.as_str(),"run_id":run_id,"report_status":report["status"],"revision":report["revision"]})
    );
    let mut service = h.service.clone();
    service.sources = Some(Arc::new(resolver));
    let mut submission = h.submission("composed");
    submission.query = None;
    submission.source = Some(artifact.clone());
    let composed = service.submit(&h.tenant, &submission).await.unwrap();
    assert_eq!(
        composed.value()["plan"]["query"],
        json!("historical observation query")
    );
    assert_eq!(composed.value()["composition"]["source"], resolved);
    assert_eq!(composed.value()["lineage"][0]["to"], artifact);
    assert_eq!(
        service.submit(&h.tenant, &submission).await.unwrap(),
        composed
    );
    assert_eq!(h.alpha.received.lock().unwrap().len(), 1);
    assert_eq!(
        service
            .submit(&TenantId::new("other").unwrap(), &submission)
            .await
            .unwrap_err()
            .code,
        "source_not_found"
    );
    let key = keys::saved(
        "report",
        &h.tenant,
        Some(&format!("{}:{run_id}", saved_id.as_str())),
    )
    .unwrap();
    let mut changed = report.clone();
    changed["status"] = json!("incomparable");
    changed["incomparable_reasons"] = json!([
        "dispatch_failed",
        "observation_truncated",
        "report_size_exceeded"
    ]);
    changed["observation"]["window"] = Value::Null;
    h.backend
        .put(
            &key,
            &ssx_json::to_vec(&changed).unwrap(),
            std::num::NonZeroU64::new(600).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .submit(&h.tenant, &submission)
            .await
            .unwrap_err()
            .value(),
        json!({"error":{"code":"source_truncated","message":"source saved-search report is truncated","reasons":["dispatch_failed","observation_truncated","report_size_exceeded"]}})
    );
    changed["incomparable_reasons"] = json!(["dispatch_failed"]);
    h.backend
        .put(
            &key,
            &ssx_json::to_vec(&changed).unwrap(),
            std::num::NonZeroU64::new(600).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .submit(&h.tenant, &submission)
            .await
            .unwrap_err()
            .value(),
        json!({"error":{"code":"source_incomparable","message":"source saved-search report is incomparable","reasons":["dispatch_failed"]}})
    );
    changed["incomparable_reasons"] = json!([]);
    changed["status"] = json!("baseline");
    h.backend
        .put(
            &key,
            &ssx_json::to_vec(&changed).unwrap(),
            std::num::NonZeroU64::new(600).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .submit(&h.tenant, &submission)
            .await
            .unwrap_err()
            .value(),
        json!({"error":{"code":"source_incomparable","message":"source saved-search report lacks a complete observation window"}})
    );
    h.backend
        .put(
            &key,
            &ssx_json::to_vec(&report).unwrap(),
            std::num::NonZeroU64::new(600).unwrap(),
        )
        .await
        .unwrap();
    let mut conn = h.server.connection();
    let _: i64 = redis::cmd("ZADD")
        .arg(keys::saved("reports", &h.tenant, Some(saved_id.as_str())).unwrap())
        .arg(9999999999999u64)
        .arg("newer-report")
        .query(&mut conn)
        .unwrap();
    let bounded = SavedReportSources {
        store: ssx_infra::valkey::saved::SavedStore::new(h.backend.clone(), h.tenant.clone()),
        search: h.service.policy.clone(),
        granted: true,
        auth_configured: auth.clone(),
        limits: SavedLimits {
            reports: 1,
            ..limits.clone()
        },
    };
    assert_eq!(
        bounded
            .resolve(&h.tenant, &artifact)
            .await
            .unwrap_err()
            .code,
        "source_not_found"
    );
    let denied = SavedReportSources {
        granted: false,
        ..bounded
    };
    assert_eq!(
        denied
            .resolve(&h.tenant, &artifact)
            .await
            .unwrap_err()
            .value(),
        json!({"error":{"code":"source_policy_denied","message":"saved-search access is disabled by current policy"}})
    );
    assert_eq!(h.alpha.received.lock().unwrap().len(), 1);
    assert!(h.beta.received.lock().unwrap().is_empty());
    h.close().await;
    case_complete(
        "native.saved_source_membership_lineage_replay_and_error_precedence",
        "saved_source_membership_lineage_replay_and_error_precedence",
    );
}
#[test]
fn native_staged_owner_process_probe() {
    let Ok(url) = std::env::var("SSX_W06_CHILD_VALKEY") else {
        return;
    };
    let alpha = std::env::var("SSX_W06_CHILD_ALPHA").unwrap();
    let beta = std::env::var("SSX_W06_CHILD_BETA").unwrap();
    let id = StagedSearchId::new(std::env::var("SSX_W06_CHILD_ID").unwrap()).unwrap();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let backend = Arc::new(Valkey::new(&url, Duration::from_millis(300)).unwrap());
        let tenant = TenantId::new("org:team").unwrap();
        let requests = Arc::new(Mutex::new(vec![]));
        let deps = dependencies(vec![
            (
                "alpha",
                Arc::new(LoopbackEngine {
                    url: alpha,
                    requests: requests.clone(),
                }),
            ),
            (
                "beta",
                Arc::new(LoopbackEngine {
                    url: beta,
                    requests,
                }),
            ),
        ]);
        let policy = deps.policy.clone();
        let search = SearchService::new(SearchConfig::default(), deps).unwrap();
        let runner = StagedRunner::new(
            backend.clone(),
            search,
            snapshot_service(backend, tenant.clone()),
            Arc::new(StaticStagedAuthority {
                policy,
                limits: limits(),
            }),
            Arc::new(SystemSnapshotClock),
            Arc::new(NoEvents),
        );
        let (_stop, rx) = watch::channel(false);
        runner.run_one(tenant, id, rx).await.unwrap();
    });
}
#[tokio::test]
async fn killed_native_owner_restart_preserves_identity_and_unknown_spend() {
    let h = Harness::new_gated("[]", "[]", false, true).await;
    let admitted = h.admit("process-loss").await;
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "native_staged_owner_process_probe",
            "--nocapture",
        ])
        .env("SSX_W06_CHILD_VALKEY", &h.server.url)
        .env("SSX_W06_CHILD_ALPHA", &h.alpha.url)
        .env("SSX_W06_CHILD_BETA", &h.beta.url)
        .env("SSX_W06_CHILD_ID", admitted.id().as_str())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    entered(&h).await;
    let store = StagedStore::new(&h.backend);
    let RecordRead::Found(before) = store
        .read(&h.tenant, &admitted.id(), h.clock.now())
        .await
        .unwrap()
    else {
        panic!("missing claimed native record")
    };
    assert_eq!(before.state(), "running");
    child.kill().unwrap();
    child.wait().unwrap();
    // Accelerate only the private dead owner's lease; the production lease remains60s.
    let mut conn = h.server.connection();
    let _: i64 = redis::cmd("PEXPIRE")
        .arg(lease_key(&h.tenant, &admitted.id()))
        .arg(1)
        .query(&mut conn)
        .unwrap();
    let _: i64 = redis::cmd("DEL")
        .arg(keys::STAGED_RUNNING)
        .arg(keys::STAGED_READY)
        .query(&mut conn)
        .unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    let mut acknowledgments = vec![];
    let mut cursor = 0;
    loop {
        cursor = store
            .reconcile_indexes_acknowledged(cursor, h.clock.now(), |r| acknowledgments.push(r))
            .await
            .unwrap();
        if cursor == 0 {
            break;
        }
    }
    assert_eq!(acknowledgments.len(), 1);
    let recovered = acknowledgments.pop().unwrap();
    assert_eq!(recovered.id(), admitted.id());
    assert_eq!(
        recovered.value()["stages"][0]["attempts"][0]["attempt_id"],
        before.value()["stages"][0]["attempts"][0]["attempt_id"]
    );
    assert_eq!(recovered.value()["budget"]["observed"], Value::Null);
    assert_eq!(recovered.value()["budget"]["reserved"], json!(1));
    h.service
        .retry(&h.tenant, &admitted.id(), "after-restart")
        .await
        .unwrap();
    h.alpha.release.notify_one();
    let runner = h.runner.clone();
    let tenant = h.tenant.clone();
    let id = admitted.id();
    let (_stop, rx) = watch::channel(false);
    let task = tokio::spawn(async move { runner.run_one(tenant, id, rx).await });
    let until = Instant::now() + Duration::from_secs(3);
    while h.alpha.received.lock().unwrap().len() < 2 {
        assert!(Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    h.alpha.release.notify_one();
    let retried = task.await.unwrap().unwrap().unwrap();
    assert_eq!(retried.value()["budget"]["reserved"], json!(2));
    assert_eq!(retried.value()["budget"]["observed"], Value::Null);
    let done = h.run_one(admitted.id()).await.unwrap();
    assert_eq!(done.state(), "completed");
    assert_eq!(done.value()["budget"]["reserved"], json!(3));
    assert_eq!(
        done.value()["stages"][0]["attempts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(h.alpha.received.lock().unwrap().len(), 2);
    h.close().await;
    case_complete(
        "native.killed_native_owner_restart_preserves_identity_and_unknown_spend",
        "killed_native_owner_restart_preserves_identity_and_unknown_spend",
    );
}
fn source_corpus() -> Value {
    ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w06/source.json")).unwrap()
}
fn case_complete(case: &str, test: &str) {
    println!("W06_CASE_COMPLETE {}", json!({"case_id":case,"test":test}));
}
fn source_policy() -> PolicyService {
    let engines = vec![capability("wikipedia"), capability("brave")];
    PolicyService {
        policy: SearchPolicy::default(),
        catalog: CatalogSnapshot::new(engines).unwrap(),
        resolver: ScopeResolver {
            catalog_routing: false,
            ..Default::default()
        },
    }
}
struct CapturedIds(Mutex<std::collections::VecDeque<String>>);
impl SnapshotIds for CapturedIds {
    fn next(&self) -> Result<SnapshotId, SnapshotFailure> {
        self.0
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(SnapshotFailure::Collision)
            .and_then(|id| SnapshotId::new(id).map_err(|_| SnapshotFailure::InvalidInput))
    }
}
fn shared_card(r: &SearchResult, id: &ResultId, _payload: bool) -> Value {
    let retrieval = ssx_application::retrieval::classify_retrieval_url(&r.url);
    ssx_json::to_value(ssx_mcp::projection::result_card(
        r,
        Some(id),
        ssx_wire::McpRetrievalDto {
            contract: "slopsearx.retrieval_handoff".into(),
            version: 1,
            eligible: retrieval.status == "ok",
            url_status: retrieval.status.into(),
            url_reason: retrieval.reason,
            scheme: retrieval.scheme,
        },
    ))
    .unwrap()
}
#[tokio::test]
async fn source_rendered_responses_use_full_native_snapshots_and_shared_mcp_projection() {
    let c = source_corpus();
    for row in c["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["id"].as_str().unwrap().starts_with("runner."))
    {
        let server = Server::start();
        let backend = Arc::new(server.store());
        let clock = Arc::new(FixedTime(Mutex::new(
            row["completion_time"].as_f64().unwrap(),
        )));
        let record = ssx_domain::staged::Record::from_value(row["record"].clone()).unwrap();
        let tenant = record.tenant();
        let ids = row["snapshot_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        let snapshots = SnapshotService::new(
            Arc::new(ValkeySnapshots::new(backend.clone())),
            clock.clone(),
            Arc::new(CapturedIds(Mutex::new(ids))),
            tenant,
            SnapshotTtl::new(3600).unwrap(),
        );
        for (index, response) in row["responses"].as_array().unwrap().iter().enumerate() {
            let response = storage::decode_response(response.clone()).unwrap();
            let parents = row["snapshot_create_inputs"][index]["kwargs"]["derived_from"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| ArtifactRef::from_value(v).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                snapshots
                    .create(
                        &ssx_application::cache::CanonicalResponse::new(response),
                        &parents
                    )
                    .await
                    .unwrap()
                    .as_str(),
                row["snapshot_ids"][index].as_str().unwrap()
            );
        }
        let policy = source_policy();
        let service = ssx_infra::staged_service::Service {
            store: backend.clone(),
            policy: policy.clone(),
            limits: limits(),
            authority: Arc::new(StaticStagedAuthority {
                policy,
                limits: limits(),
            }),
            clock,
            snapshots,
            sources: None,
            events: Arc::new(NoEvents),
        };
        let view = ssx_infra::staged_service::View::parse(
            Some(&row["render_input"]["include"]),
            Some(&row["render_input"]["max_results"]),
            20,
        )
        .unwrap();
        let rendered = service.render(&record, &view, shared_card).await.unwrap();
        assert_eq!(rendered, row["rendered"], "{}", row["id"].as_str().unwrap());
        backend.close().await;
        drop(server);
        case_complete(
            &format!("render.{}", row["id"].as_str().unwrap()),
            "source_rendered_responses_use_full_native_snapshots_and_shared_mcp_projection",
        );
    }
}
#[tokio::test]
async fn source_saved_report_resolution_preserves_full_original_error_envelopes() {
    use ssx_application::saved::{SavedLimits, SavedPolicy, prepare_create};
    use ssx_infra::staged_service::{SavedReportSources, Sources};
    let c = source_corpus();
    for row in c["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["id"].as_str().unwrap().starts_with("source."))
    {
        let server = Server::start();
        let backend = Arc::new(server.store());
        let tenant = TenantId::new("default").unwrap();
        let store = ssx_infra::valkey::saved::SavedStore::new(backend.clone(), tenant.clone());
        let mut policy = source_policy();
        let inputs = &row["inputs"];
        let auth = BTreeMap::from([("wikipedia".into(), false), ("brave".into(), false)]);
        let sl = SavedLimits {
            reports: inputs["saved_max_reports"].as_u64().unwrap(),
            ..Default::default()
        };
        let p = SavedPolicy {
            search: &policy,
            saved_granted: true,
            auth_configured: &auth,
            limits: &sl,
        };
        let now = store.now().await.unwrap();
        let prepared=prepare_create(&p,tenant.as_str(),"saved",&json!({"query":"current definition query","engines":inputs["definition"]["engines"],"interval_seconds":60,"expires_in_seconds":600}),now).unwrap();
        store.create(&prepared).await.unwrap();
        for (index, report) in inputs["reports"].as_array().unwrap().iter().enumerate() {
            let mut report = report.clone();
            report["search_id"] = json!("saved");
            report["expires_at"] = json!(now + 600.);
            let run = report["run_id"].as_str().unwrap();
            backend
                .put(
                    &keys::saved("report", &tenant, Some(&format!("saved:{run}"))).unwrap(),
                    &ssx_json::to_vec(&report).unwrap(),
                    std::num::NonZeroU64::new(900).unwrap(),
                )
                .await
                .unwrap();
            let _: i64 = redis::cmd("ZADD")
                .arg(keys::saved("reports", &tenant, Some("saved")).unwrap())
                .arg(now + index as f64)
                .arg(run)
                .query(&mut server.connection())
                .unwrap();
        }
        policy.policy.sensitive_engines = inputs["sensitive_engines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().into())
            .collect();
        if !inputs["store_available"].as_bool().unwrap() {
            backend.close().await;
        }
        let sources = SavedReportSources {
            store,
            search: policy,
            granted: inputs["saved_granted"].as_bool().unwrap(),
            auth_configured: auth,
            limits: sl,
        };
        let actual = match sources.resolve(&tenant, &row["artifact"]).await {
            Ok(v) => v,
            Err(e) => e.value(),
        };
        assert_eq!(actual, row["output"], "{}", row["id"].as_str().unwrap());
        backend.close().await;
        drop(server);
        case_complete(
            row["id"].as_str().unwrap(),
            "source_saved_report_resolution_preserves_full_original_error_envelopes",
        );
    }
}
struct RejectedSource {
    error: Value,
    calls: std::sync::atomic::AtomicUsize,
}
impl ssx_infra::staged_service::Sources for RejectedSource {
    fn resolve<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a Value,
    ) -> PortFuture<'a, Result<Value, ssx_application::staged::Error>> {
        Box::pin(async move {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut error = ssx_application::staged::Error::new(
                "source_unavailable",
                "",
                self.error["error"]["message"].as_str().unwrap(),
            );
            error.details = Box::new(json!({}));
            Err(error)
        })
    }
}
#[tokio::test]
async fn source_replay_checks_requested_identity_and_resolves_sources_before_lookup() {
    let c = source_corpus();
    for row in c["cases"].as_array().unwrap().iter().filter(|r| {
        r["id"].as_str().unwrap().starts_with("replay.") && r.get("submission").is_some()
    }) {
        let server = Server::start();
        let backend = Arc::new(server.store());
        let record = ssx_domain::staged::Record::from_value(row["record"].clone()).unwrap();
        let tenant = record.tenant();
        let clock = Arc::new(FixedTime(Mutex::new(1000.)));
        let current = Limits {
            granted: true,
            deadline_ms: row["limits"]["deadline_ms"].as_u64().unwrap_or(30000),
            engine_calls: row["limits"]["max_engine_calls"].as_u64().unwrap_or(64),
        };
        let policy = source_policy();
        let resolver = Arc::new(RejectedSource {
            error: row["source_resolution_error"].clone(),
            calls: std::sync::atomic::AtomicUsize::new(0),
        });
        backend
            .put(
                &record_key(&tenant, &record.id()),
                &record.encode().unwrap(),
                std::num::NonZeroU64::new(3600).unwrap(),
            )
            .await
            .unwrap();
        let rk = record_key(&tenant, &record.id());
        backend
            .put(
                &keys::staged("idem", &tenant, Some("replay")).unwrap(),
                &ssx_json::to_vec(&json!({"record_key":rk})).unwrap(),
                std::num::NonZeroU64::new(3600).unwrap(),
            )
            .await
            .unwrap();
        let service = ssx_infra::staged_service::Service {
            store: backend.clone(),
            policy: policy.clone(),
            limits: current.clone(),
            authority: Arc::new(StaticStagedAuthority {
                policy,
                limits: current,
            }),
            clock,
            snapshots: snapshot_service(backend.clone(), tenant.clone()),
            sources: Some(resolver.clone()),
            events: Arc::new(NoEvents),
        };
        let s = &row["submission"];
        let submission = ssx_infra::staged_service::Submission {
            query: s["query"].as_str().map(str::to_owned),
            objectives: s["objectives"].clone(),
            initial_scope: s["initial_scope"].clone(),
            fallback_scope: (!s["fallback_scope"].is_null()).then(|| s["fallback_scope"].clone()),
            allow_scope_expansion: s["allow_scope_expansion"].as_bool().unwrap(),
            filters: (!s["filters"].is_null()).then(|| s["filters"].clone()),
            idempotency_key: s["idempotency_key"].as_str().unwrap().into(),
            source: (!s["source"].is_null()).then(|| s["source"].clone()),
        };
        let error = service.submit(&tenant, &submission).await.unwrap_err();
        assert_eq!(
            error.value(),
            row["output"],
            "{}",
            row["id"].as_str().unwrap()
        );
        assert_eq!(
            resolver.calls.load(std::sync::atomic::Ordering::SeqCst),
            usize::from(row["id"] == json!("replay.source_resolution_before_lookup"))
        );
        backend.close().await;
        drop(server);
        case_complete(
            row["id"].as_str().unwrap(),
            "source_replay_checks_requested_identity_and_resolves_sources_before_lookup",
        );
    }
}
/// A real RESP relay records the complete EVAL before exposing the phase barrier.
/// It never fabricates store acknowledgments; held replies come from private Valkey.
struct RespProxy {
    url: String,
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    observed: Arc<Mutex<Vec<Vec<u8>>>>,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for RespProxy {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}
fn resp_len(bytes: &[u8]) -> Option<usize> {
    let line = bytes.windows(2).position(|b| b == b"\r\n")? + 2;
    let number = || {
        std::str::from_utf8(&bytes[1..line - 2])
            .ok()?
            .parse::<i64>()
            .ok()
    };
    match *bytes.first()? {
        b'$' | b'!' | b'=' => {
            let n = number()?;
            if n < 0 {
                return Some(line);
            }
            let n = line + usize::try_from(n).ok()? + 2;
            (bytes.len() >= n).then_some(n)
        }
        b'*' | b'%' | b'~' | b'>' | b'|' => {
            let n = number()?;
            if n < 0 {
                return Some(line);
            }
            let n = n as usize
                * if matches!(bytes[0], b'%' | b'|') {
                    2
                } else {
                    1
                };
            let mut end = line;
            for _ in 0..n {
                end += resp_len(bytes.get(end..)?)?;
            }
            Some(end)
        }
        _ => Some(line),
    }
}
async fn resp_read<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
) -> std::io::Result<Option<Vec<u8>>> {
    loop {
        if let Some(len) = resp_len(buffer) {
            return Ok(Some(buffer.drain(..len).collect()));
        }
        let mut chunk = [0; 4096];
        let n = reader.read(&mut chunk).await?;
        if n == 0 {
            return Ok(None);
        }
        buffer.extend_from_slice(&chunk[..n]);
    }
}
impl RespProxy {
    async fn start(server: &Server, needle: &str, before_forward: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("redis://{}", listener.local_addr().unwrap());
        let socket = server.dir.join("s.sock");
        let needle = needle.as_bytes().to_vec();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let observed = Arc::new(Mutex::new(vec![]));
        let e = entered.clone();
        let r = release.clone();
        let seen = observed.clone();
        let (stop, mut stopped) = watch::channel(false);
        let task = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            loop {
                tokio::select! {biased;_=stopped.changed()=>break,accepted=listener.accept()=>{let(mut client,_)=accepted.unwrap();let socket=socket.clone();let needle=needle.clone();let entered=e.clone();let release=r.clone();let seen=seen.clone();clients.spawn(async move{let Ok(mut upstream)=tokio::net::UnixStream::connect(socket).await else{return};let mut incoming=vec![];let mut replies=vec![];while let Ok(Some(frame))=resp_read(&mut client,&mut incoming).await{let held=frame.windows(needle.len()).any(|v|v==needle);seen.lock().unwrap().push(frame.clone());if held&&before_forward{entered.notify_one();release.notified().await;}
                if upstream.write_all(&frame).await.is_err(){break}let Ok(Some(reply))=resp_read(&mut upstream,&mut replies).await else{break};if held&&!before_forward{entered.notify_one();release.notified().await;}
                if client.write_all(&reply).await.is_err(){break}}});},Some(_)=clients.join_next(),if !clients.is_empty()=>()}
            }
            clients.shutdown().await;
        });
        Self {
            url,
            entered,
            release,
            observed,
            stop,
            task,
        }
    }
    async fn shutdown(&mut self) {
        self.stop.send_replace(true);
        let _ = (&mut self.task).await;
    }
    async fn reached(&self) {
        tokio::time::timeout(Duration::from_secs(3), self.entered.notified())
            .await
            .unwrap();
    }
}
struct BlockedSnapshots {
    inner: ValkeySnapshots,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    persist_first: bool,
}
impl SnapshotRepository for BlockedSnapshots {
    fn create<'a>(
        &'a self,
        s: &'a SearchSnapshot,
        ttl: std::num::NonZeroU64,
    ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
        Box::pin(async move {
            if self.persist_first {
                self.inner.create(s, ttl).await?;
            }
            self.entered.notify_one();
            self.release.notified().await;
            if self.persist_first {
                Ok(SnapshotWrite::Stored)
            } else {
                self.inner.create(s, ttl).await
            }
        })
    }
    fn read<'a>(
        &'a self,
        t: &'a TenantId,
        id: &'a SnapshotId,
    ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
        self.inner.read(t, id)
    }
}
#[tokio::test]
async fn renewal_reply_barrier_shutdown_is_joined_and_keeps_foreign_owner() {
    let h = Harness::new_gated("[]", "[]", false, true).await;
    let record = h.admit("renew-request-barrier").await;
    let mut proxy = RespProxy::start(
        &h.server,
        "redis.call('ZADD', KEYS[2], ARGV[3], ARGV[4])",
        false,
    )
    .await;
    let proxied = Arc::new(Valkey::new(&proxy.url, Duration::from_secs(5)).unwrap());
    let mut runner = h
        .runner
        .clone()
        .with_renewal_interval(Duration::from_millis(10))
        .unwrap();
    runner.store = proxied.clone();
    let tenant = h.tenant.clone();
    let id = record.id();
    let (stop, rx) = watch::channel(false);
    let task = tokio::spawn(async move { runner.run_one(tenant, id, rx).await });
    // First renewal is immediate before dispatch: release it, then observe HTTP.
    proxy.reached().await;
    proxy.release.notify_one();
    entered(&h).await;
    proxy.reached().await;
    assert!(
        proxy
            .observed
            .lock()
            .unwrap()
            .iter()
            .filter(|r| String::from_utf8_lossy(r).contains("redis.call('EXPIRE'"))
            .count()
            >= 2
    );
    let _: () = redis::cmd("SET")
        .arg(lease_key(&h.tenant, &record.id()))
        .arg("independent-reclaimer")
        .arg("EX")
        .arg(60)
        .query(&mut h.server.connection())
        .unwrap();
    stop.send_replace(true);
    let result = tokio::time::timeout(Duration::from_millis(250), task)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        result,
        Err(ssx_infra::staged_runner::Error::Shutdown)
    ));
    let owner: String = redis::cmd("GET")
        .arg(lease_key(&h.tenant, &record.id()))
        .query(&mut h.server.connection())
        .unwrap();
    assert_eq!(owner, "independent-reclaimer");
    assert_eq!(h.events.0.lock().unwrap().len(), 2);
    assert_eq!(h.alpha.received.lock().unwrap().len(), 1);
    proxied.close().await;
    proxy.shutdown().await;
    h.close().await;
    case_complete(
        "native.renewal_reply_barrier_shutdown_is_joined_and_keeps_foreign_owner",
        "renewal_reply_barrier_shutdown_is_joined_and_keeps_foreign_owner",
    );
}
#[tokio::test]
async fn snapshot_reply_lease_loss_or_caller_drop_never_publishes_stale_reference() {
    for persist_first in [false, true] {
        let h = Harness::new("[]", "[]", false).await;
        let record = h
            .admit(if persist_first {
                "snapshot-orphan"
            } else {
                "snapshot-blocked"
            })
            .await;
        let blocked = Arc::new(BlockedSnapshots {
            inner: ValkeySnapshots::new(h.backend.clone()),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            persist_first,
        });
        let snapshots = SnapshotService::new(
            blocked.clone(),
            h.clock.clone(),
            Arc::new(RandomSnapshotIds),
            h.tenant.clone(),
            SnapshotTtl::new(3600).unwrap(),
        );
        let mut runner = h
            .runner
            .clone()
            .with_renewal_interval(Duration::from_millis(10))
            .unwrap();
        runner.snapshots = snapshots;
        let tenant = h.tenant.clone();
        let id = record.id();
        let (_stop, rx) = watch::channel(false);
        let task = tokio::spawn(async move { runner.run_one(tenant, id, rx).await });
        tokio::time::timeout(Duration::from_secs(3), blocked.entered.notified())
            .await
            .unwrap();
        let _: () = redis::cmd("SET")
            .arg(lease_key(&h.tenant, &record.id()))
            .arg("new-valid-owner")
            .arg("EX")
            .arg(60)
            .query(&mut h.server.connection())
            .unwrap();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(2), task)
                .await
                .unwrap()
                .unwrap(),
            Err(ssx_infra::staged_runner::Error::LeaseLost)
        ));
        let RecordRead::Found(current) = StagedStore::new(&h.backend)
            .read(&h.tenant, &record.id(), h.clock.now())
            .await
            .unwrap()
        else {
            panic!("record missing")
        };
        assert_eq!(current.state(), "running");
        assert_eq!(current.value()["selected_result_attempt_id"], Value::Null);
        let snapshots: Vec<String> = redis::cmd("KEYS")
            .arg("mcp:snapshot:*")
            .query(&mut h.server.connection())
            .unwrap();
        assert_eq!(snapshots.len(), usize::from(persist_first));
        assert_eq!(h.events.0.lock().unwrap().len(), 2);
        h.close().await;
        case_complete(
            if persist_first {
                "native.snapshot_orphan_no_reference"
            } else {
                "native.snapshot_blocked_no_reference"
            },
            "snapshot_reply_lease_loss_or_caller_drop_never_publishes_stale_reference",
        );
    }
}
#[tokio::test]
async fn acknowledged_save_and_unknown_save_are_distinct_commit_races() {
    for acknowledge in [false, true] {
        let h = Harness::new("[]", "[]", false).await;
        let record = h
            .admit(if acknowledge {
                "save-ack"
            } else {
                "save-unknown"
            })
            .await;
        let mut proxy = RespProxy::start(&h.server, "if ARGV[4] == 'queued'", false).await;
        let proxied = Arc::new(Valkey::new(&proxy.url, Duration::from_secs(5)).unwrap());
        let mut runner = h.runner.clone();
        runner.store = proxied.clone();
        let tenant = h.tenant.clone();
        let id = record.id();
        let (stop, rx) = watch::channel(false);
        let task = tokio::spawn(async move { runner.run_one(tenant, id, rx).await });
        proxy.reached().await;
        // The actual Lua has run before its reply is withheld. Policy/stop changes
        // cannot retract this already-sent write; definite ack and unknown stay distinct.
        if acknowledge {
            proxy.release.notify_one();
            assert_eq!(task.await.unwrap().unwrap().unwrap().state(), "queued");
            assert_eq!(h.events.0.lock().unwrap().len(), 3);
        } else {
            stop.send_replace(true);
            assert!(matches!(
                tokio::time::timeout(Duration::from_millis(250), task)
                    .await
                    .unwrap()
                    .unwrap(),
                Err(ssx_infra::staged_runner::Error::Shutdown)
            ));
            assert_eq!(h.events.0.lock().unwrap().len(), 2);
        }
        let RecordRead::Found(current) = StagedStore::new(&h.backend)
            .read(&h.tenant, &record.id(), h.clock.now())
            .await
            .unwrap()
        else {
            panic!("record missing")
        };
        assert_eq!(current.state(), "queued");
        assert!(current.value()["selected_result_attempt_id"].is_string());
        assert_eq!(current.value()["budget"]["reserved"], json!(1));
        proxied.close().await;
        proxy.shutdown().await;
        h.close().await;
        case_complete(
            if acknowledge {
                "native.save_acknowledged"
            } else {
                "native.save_unknown_reply"
            },
            "acknowledged_save_and_unknown_save_are_distinct_commit_races",
        );
    }
}
#[tokio::test]
async fn dropped_caller_cancels_and_joins_owned_http_without_closing_shared_search() {
    let h = Harness::new_gated("[]", "[]", false, true).await;
    let record = h.admit("caller-drop").await;
    let runner = h.runner.clone();
    let tenant = h.tenant.clone();
    let id = record.id();
    let (_stop, rx) = watch::channel(false);
    let outer = tokio::spawn(async move { runner.run_one(tenant, id, rx).await });
    entered(&h).await;
    outer.abort();
    assert!(outer.await.unwrap_err().is_cancelled());
    let until = Instant::now() + Duration::from_secs(2);
    while h.alpha.closed.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        assert!(
            Instant::now() < until,
            "owned HTTP future did not close after caller drop"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let response = h
        .search
        .search(ScopeProposal::generic(SearchRequest {
            query: "independent".into(),
            engines: Some(vec!["beta".into()]),
            ..Default::default()
        }))
        .await
        .unwrap();
    assert_eq!(response.engine_outcomes[0].status, "ok");
    assert_eq!(h.events.0.lock().unwrap().len(), 2);
    let RecordRead::Found(record) = StagedStore::new(&h.backend)
        .read(&h.tenant, &record.id(), h.clock.now())
        .await
        .unwrap()
    else {
        panic!("record missing")
    };
    assert_eq!(record.state(), "running");
    assert_eq!(record.value()["selected_result_attempt_id"], Value::Null);
    h.close().await;
    case_complete(
        "native.dropped_caller_owned_http_joined",
        "dropped_caller_cancels_and_joins_owned_http_without_closing_shared_search",
    );
}
#[tokio::test]
async fn background_discovery_rebuilds_ready_index_and_joins_shutdown() {
    let h = Harness::new("[]", "[]", false).await;
    let admitted = h.admit("background").await;
    let _: i64 = redis::cmd("DEL")
        .arg(keys::STAGED_READY)
        .query(&mut h.server.connection())
        .unwrap();
    let runner = h.runner.clone();
    let (stop, rx) = watch::channel(false);
    let task = tokio::spawn(async move { runner.run(Duration::from_millis(5), rx).await });
    let until = Instant::now() + Duration::from_secs(3);
    let completed = loop {
        let record = h.service.get(&h.tenant, &admitted.id()).await.unwrap();
        if record.state() == "completed" {
            break record;
        }
        assert!(Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    stop.send_replace(true);
    tokio::time::timeout(Duration::from_millis(250), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(completed.value()["budget"]["reserved"], json!(2));
    assert_eq!(completed.value()["budget"]["observed"], json!(2));
    assert_eq!(h.alpha.received.lock().unwrap().len(), 1);
    assert_eq!(h.beta.received.lock().unwrap().len(), 1);
    assert_eq!(
        h.events
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.to.as_str())
            .collect::<Vec<_>>(),
        vec!["queued", "running", "queued", "running", "completed"]
    );
    h.close().await;
    case_complete(
        "native.background_discovery_index_loss",
        "background_discovery_rebuilds_ready_index_and_joins_shutdown",
    );
}
#[tokio::test]
async fn deadline_after_acknowledged_renewal_is_rechecked_without_dispatch() {
    let h = Harness::new("[]", "[]", false).await;
    let record = h.admit("renew-deadline").await;
    let mut proxy = RespProxy::start(
        &h.server,
        "redis.call('ZADD', KEYS[2], ARGV[3], ARGV[4])",
        false,
    )
    .await;
    let proxied = Arc::new(Valkey::new(&proxy.url, Duration::from_secs(5)).unwrap());
    let mut runner = h.runner.clone();
    runner.store = proxied.clone();
    let tenant = h.tenant.clone();
    let id = record.id();
    let (_stop, rx) = watch::channel(false);
    let task = tokio::spawn(async move { runner.run_one(tenant, id, rx).await });
    proxy.reached().await;
    *h.clock.0.lock().unwrap() += 31.;
    proxy.release.notify_one();
    let failed = task.await.unwrap().unwrap().unwrap();
    assert_eq!(failed.state(), "failed");
    assert_eq!(failed.value()["stop_reason"], json!("deadline_expired"));
    assert_eq!(failed.value()["budget"]["reserved"], json!(1));
    assert_eq!(failed.value()["budget"]["observed"], json!(0));
    assert_eq!(
        failed.value()["objectives"]["unmet"],
        json!([{"objective":"deadline_ms","reason":"deadline_expired"}])
    );
    assert_eq!(
        failed.value()["stages"][0]["attempts"][0]["error"],
        json!({"code":"deadline_exceeded","message":"deadline expired before dispatch"})
    );
    let mut expected = h.events.0.lock().unwrap()[1].record.value().clone();
    expected["state"] = json!("failed");
    expected["stop_reason"] = json!("deadline_expired");
    expected["objectives"]["unmet"] =
        json!([{"objective":"deadline_ms","reason":"deadline_expired"}]);
    expected["stages"].as_array_mut().unwrap()[0]["state"] = json!("failed");
    let attempt = &mut expected["stages"].as_array_mut().unwrap()[0]["attempts"]
        .as_array_mut()
        .unwrap()[0];
    attempt["state"] = json!("failed");
    attempt["finished_at"] = json!(h.clock.now().seconds());
    attempt["observed_engine_calls"] = json!(0);
    attempt["deadline_exceeded"] = json!(true);
    attempt["error"] =
        json!({"code":"deadline_exceeded","message":"deadline expired before dispatch"});
    assert_eq!(failed.value(), &expected);
    assert!(h.alpha.received.lock().unwrap().is_empty());
    proxied.close().await;
    proxy.shutdown().await;
    h.close().await;
    case_complete(
        "native.deadline_after_renewal_no_dispatch",
        "deadline_after_acknowledged_renewal_is_rechecked_without_dispatch",
    );
}
#[tokio::test]
async fn initial_policy_rejection_closes_owned_claim_without_acquisition() {
    let h = Harness::new("[]", "[]", false).await;
    let r = h.admit("prepolicy").await;
    h.authority.limits.lock().unwrap().granted = false;
    let failed = h.run_one(r.id()).await.unwrap();
    assert_eq!(failed.state(), "failed");
    assert_eq!(
        failed.value()["error"],
        json!({"code":"tool_disabled","message":"staged search is disabled","grant":"MCP_GRANT_STAGED_SEARCH"})
    );
    assert_eq!(
        failed.value()["stages"][0]["attempts"][0]["error"]["code"],
        json!("policy_rejected")
    );
    assert_eq!(failed.value()["budget"]["observed"], json!(0));
    assert_eq!(h.events.0.lock().unwrap().len(), 3);
    assert!(h.alpha.received.lock().unwrap().is_empty());
    h.close().await;
    case_complete(
        "native.initial_policy_rejection_cleanup",
        "initial_policy_rejection_closes_owned_claim_without_acquisition",
    );
}
#[tokio::test]
async fn recovery_acknowledgments_are_exact_single_owner_and_unknown_reply_is_not_an_event() {
    for unknown in [false, true] {
        let h = Harness::new("[]", "[]", false).await;
        let r = h
            .admit(if unknown {
                "recovery-lost-ack"
            } else {
                "recovery-race"
            })
            .await;
        let store = StagedStore::new(&h.backend);
        let owned = store
            .claim(
                &h.tenant,
                &r.id(),
                LeaseToken::new("dead-owner").unwrap(),
                "acknowledged-attempt",
                h.clock.now(),
            )
            .await
            .unwrap()
            .unwrap();
        let mut conn = h.server.connection();
        let _: i64 = redis::cmd("DEL")
            .arg(lease_key(&h.tenant, &r.id()))
            .query(&mut conn)
            .unwrap();
        let rk = record_key(&h.tenant, &r.id());
        let _: i64 = redis::cmd("ZADD")
            .arg(keys::STAGED_RUNNING)
            .arg(h.clock.now().seconds())
            .arg(&rk)
            .query(&mut conn)
            .unwrap();
        if unknown {
            let mut proxy =
                RespProxy::start(&h.server, "current.stop_reason='interrupted'", false).await;
            let proxied = Arc::new(Valkey::new(&proxy.url, Duration::from_millis(200)).unwrap());
            let backend = proxied.clone();
            let now = h.clock.now();
            let receipts = Arc::new(Mutex::new(vec![]));
            let seen = receipts.clone();
            let task = tokio::spawn(async move {
                StagedStore::new(&backend)
                    .recover_orphans_acknowledged(now, |r| seen.lock().unwrap().push(r))
                    .await
            });
            proxy.reached().await;
            assert_eq!(task.await.unwrap().unwrap_err(), Failure::CompletionUnknown);
            assert!(receipts.lock().unwrap().is_empty());
            let RecordRead::Found(current) =
                store.read(&h.tenant, &r.id(), h.clock.now()).await.unwrap()
            else {
                panic!("record missing")
            };
            assert_eq!(current.state(), "interrupted");
            assert_eq!(
                current.value()["stages"][0]["attempts"][0]["attempt_id"],
                json!("acknowledged-attempt")
            );
            assert_eq!(current.value()["budget"]["observed"], Value::Null);
            assert_eq!(
                store
                    .recover_orphans_acknowledged(h.clock.now(), |_| panic!(
                        "unknown ack inferred by replay"
                    ))
                    .await
                    .unwrap(),
                0
            );
            proxied.close().await;
            proxy.shutdown().await;
        } else {
            let receipts = Mutex::new(vec![]);
            let first = store
                .recover_orphans_acknowledged(h.clock.now(), |r| receipts.lock().unwrap().push(r));
            let other = h.server.store();
            let second = StagedStore::new(&other);
            let second = second
                .recover_orphans_acknowledged(h.clock.now(), |r| receipts.lock().unwrap().push(r));
            let (a, b) = tokio::join!(first, second);
            assert_eq!(a.unwrap() + b.unwrap(), 1);
            let receipts = receipts.into_inner().unwrap();
            assert_eq!(receipts.len(), 1);
            let raw: Vec<u8> = redis::cmd("GET").arg(&rk).query(&mut conn).unwrap();
            let actual = ssx_domain::staged::Record::decode(&raw).unwrap();
            assert_eq!(receipts[0], actual);
            assert_eq!(actual.value()["budget"]["reserved"], json!(1));
            assert_eq!(actual.value()["budget"]["observed"], Value::Null);
            assert_eq!(
                actual.value()["stages"][0]["attempts"][0]["state"],
                json!("interrupted")
            );
            let obsolete = complete(
                owned.record(),
                None,
                None,
                h.clock.now().seconds(),
                Some(("execution_failed", "stale")),
            )
            .unwrap();
            assert!(
                !store
                    .save_owned(&owned, &obsolete, h.clock.now())
                    .await
                    .unwrap()
            );
            other.close().await;
        }
        h.close().await;
        case_complete(
            if unknown {
                "native.recovery_unknown_reply"
            } else {
                "native.recovery_ack_exact_once"
            },
            "recovery_acknowledgments_are_exact_single_owner_and_unknown_reply_is_not_an_event",
        );
    }
}

#[tokio::test]
async fn retained_reads_and_replay_preserve_evidence_without_authorizing_dispatch() {
    let h = Harness::new("[]", "[]", false).await;
    let admitted = h.admit("retained-authority").await;
    for phase in ["inactive", "auth_missing", "circuit_open", "unavailable"] {
        let mut current = h.service.clone();
        let mut engines = current.policy.catalog.engines().to_vec();
        let alpha = engines.iter_mut().find(|e| e.name == "alpha").unwrap();
        match phase {
            "inactive" => alpha.active = false,
            "auth_missing" => alpha.auth_ready = false,
            "circuit_open" => alpha.circuit_open = true,
            "unavailable" => {
                alpha.available = false;
                alpha.health_status = "unavailable".into();
            }
            _ => unreachable!(),
        }
        current.policy.catalog = CatalogSnapshot::new(engines).unwrap();
        current.authority = Arc::new(StaticStagedAuthority {
            policy: current.policy.clone(),
            limits: limits(),
        });
        let events_before = h.events.0.lock().unwrap().len();
        assert_eq!(
            current.get(&h.tenant, &admitted.id()).await.unwrap(),
            admitted,
            "{phase}"
        );
        assert_eq!(
            current
                .submit(&h.tenant, &h.submission("retained-authority"))
                .await
                .unwrap(),
            admitted,
            "{phase}"
        );
        assert_eq!(
            h.events.0.lock().unwrap().len(),
            events_before,
            "retained access emitted event {phase}"
        );
        // Retained authorization supplies no acquisition token. Fresh admission
        // independently follows the existing explicit-scope policy: inactive or
        // missing implementations fail; credentials/circuit state remains the
        // acquisition scheduler's responsibility.
        let new = current
            .submit(&h.tenant, &h.submission(&format!("new-{phase}")))
            .await;
        let plan = Plan::build(&current.policy, &limits(), &input());
        let admission_allowed = matches!(phase, "auth_missing" | "circuit_open");
        assert_eq!(new.is_ok(), admission_allowed, "{phase}");
        assert_eq!(plan.is_ok(), admission_allowed, "{phase}");
        if let Ok(new) = new {
            assert_eq!(new.state(), "queued");
            assert_eq!(new.value()["budget"]["reserved"], json!(0));
            assert!(
                new.value()["stages"][0]["attempts"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
    }
    for target in ["alpha", "beta"] {
        let mut denied = h.service.clone();
        denied.policy.policy.sensitive_engines.insert(target.into());
        denied.authority = Arc::new(StaticStagedAuthority {
            policy: denied.policy.clone(),
            limits: limits(),
        });
        let expected = json!({"error":{"code":"policy_rejected","field":"engines","message":format!("sensitive engines are unreachable without the sensitive-engine grant (MCP_TARGETED_SENSITIVE_ALLOWED=1): {target}"),"engines":[target],"grant":"MCP_TARGETED_SENSITIVE_ALLOWED"}});
        assert_eq!(
            denied
                .get(&h.tenant, &admitted.id())
                .await
                .unwrap_err()
                .value(),
            expected
        );
        assert_eq!(
            denied
                .submit(&h.tenant, &h.submission("retained-authority"))
                .await
                .unwrap_err()
                .value(),
            expected
        );
    }
    let RecordRead::Found(after) = StagedStore::new(&h.backend)
        .read(&h.tenant, &admitted.id(), h.clock.now())
        .await
        .unwrap()
    else {
        panic!("retained record missing")
    };
    assert_eq!(after, admitted);
    assert!(h.requests.lock().unwrap().is_empty());
    assert!(h.alpha.received.lock().unwrap().is_empty());
    assert!(h.beta.received.lock().unwrap().is_empty());
    {
        let events = h.events.0.lock().unwrap();
        assert_eq!(events.len(), 3);
        assert!(events.iter().all(|e| e.from.is_none()
            && e.to == "queued"
            && e.record.value()["budget"]["reserved"] == json!(0)));
    }
    h.close().await;
    case_complete(
        "native.retained_reads_and_replay_preserve_evidence_without_authorizing_dispatch",
        "retained_reads_and_replay_preserve_evidence_without_authorizing_dispatch",
    );
}

#[tokio::test]
async fn source_initial_renewal_loss_preserves_full_claim_without_dispatch() {
    let c: Value =
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/w06/source.json")).unwrap();
    let source = c["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"].as_str() == Some("runner.renewal_lost"))
        .unwrap();
    let h = Harness::new("[]", "[]", false).await;
    let mut submission = h.submission("initial-renewal-loss");
    submission.objectives["deadline_ms"] =
        source["record"]["objectives"]["requested"]["deadline_ms"].clone();
    submission.objectives["max_engine_calls"] =
        source["record"]["objectives"]["requested"]["max_engine_calls"].clone();
    let record = h.service.submit(&h.tenant, &submission).await.unwrap();
    let mut proxy = RespProxy::start(
        &h.server,
        "redis.call('ZADD', KEYS[2], ARGV[3], ARGV[4])",
        true,
    )
    .await;
    let proxied = Arc::new(Valkey::new(&proxy.url, Duration::from_secs(5)).unwrap());
    let mut runner = h.runner.clone();
    runner.store = proxied.clone();
    let tenant = h.tenant.clone();
    let id = record.id();
    let (_stop, rx) = watch::channel(false);
    let task = tokio::spawn(async move { runner.run_one(tenant, id, rx).await });
    proxy.reached().await;
    let store = StagedStore::new(&h.backend);
    let RecordRead::Found(claimed) = store
        .read(&h.tenant, &record.id(), h.clock.now())
        .await
        .unwrap()
    else {
        panic!("claim missing")
    };
    let _: () = redis::cmd("SET")
        .arg(lease_key(&h.tenant, &record.id()))
        .arg("foreign-owner")
        .arg("PX")
        .arg(30000)
        .query(&mut h.server.connection())
        .unwrap();
    proxy.release.notify_one();
    assert!(matches!(
        task.await.unwrap(),
        Err(ssx_infra::staged_runner::Error::LeaseLost)
    ));
    let RecordRead::Found(after) = store
        .read(&h.tenant, &record.id(), h.clock.now())
        .await
        .unwrap()
    else {
        panic!("claim missing")
    };
    // Source leaves the complete acknowledged claim untouched when the first
    // renewal is false. Native preserves that full-record relation, including
    // unknown observation, deadline, owner, attempts and conservative reservation.
    assert_eq!(source["claimed"][0], source["record"]);
    assert_eq!(after, claimed);
    for key in [
        "state",
        "stop_reason",
        "budget",
        "selected_result_attempt_id",
        "next_stage",
        "retry_keys",
    ] {
        assert_eq!(after.value()[key], source["record"][key], "{key}");
    }
    assert_eq!(
        json!(h.requests.lock().unwrap().len()),
        json!(source["requests"].as_array().unwrap().len())
    );
    assert!(h.alpha.received.lock().unwrap().is_empty());
    assert!(h.beta.received.lock().unwrap().is_empty());
    assert_eq!(h.events.0.lock().unwrap().len(), 2);
    let lease: String = redis::cmd("GET")
        .arg(lease_key(&h.tenant, &record.id()))
        .query(&mut h.server.connection())
        .unwrap();
    assert_eq!(lease, "foreign-owner");
    proxied.close().await;
    proxy.shutdown().await;
    h.close().await;
    case_complete(
        "runner.renewal_lost",
        "source_initial_renewal_loss_preserves_full_claim_without_dispatch",
    );
}

#[tokio::test]
async fn claim_events_require_exact_acknowledgment_and_never_infer_lost_reply() {
    for acknowledge in [false, true] {
        let h = Harness::new_gated("[]", "[]", false, true).await;
        let admitted = h.admit("claim-event").await;
        let mut proxy = RespProxy::start(
            &h.server,
            "current.state='running'; current.owner_token=ARGV[2]",
            false,
        )
        .await;
        let proxied = Arc::new(Valkey::new(&proxy.url, Duration::from_secs(5)).unwrap());
        let mut runner = h.runner.clone();
        runner.store = proxied.clone();
        let tenant = h.tenant.clone();
        let id = admitted.id();
        let (stop, rx) = watch::channel(false);
        let task = tokio::spawn(async move { runner.run_one(tenant, id, rx).await });
        proxy.reached().await;
        let RecordRead::Found(committed) = StagedStore::new(&h.backend)
            .read(&h.tenant, &admitted.id(), h.clock.now())
            .await
            .unwrap()
        else {
            panic!("claim missing")
        };
        assert_eq!(committed.state(), "running");
        assert_eq!(committed.value()["budget"]["reserved"], json!(1));
        assert_eq!(h.events.0.lock().unwrap().len(), 1);
        if acknowledge {
            proxy.release.notify_one();
            entered(&h).await;
            let events = h.events.0.lock().unwrap().clone();
            assert_eq!(events.len(), 2);
            assert_eq!(events[1].from, Some("queued"));
            assert_eq!(events[1].to, "running");
            assert_eq!(events[1].record, committed);
            assert_eq!(
                events[1].record.value()["stages"][0]["attempts"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
        }
        stop.send_replace(true);
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(2), task)
                .await
                .unwrap()
                .unwrap(),
            Err(ssx_infra::staged_runner::Error::Shutdown)
        ));
        assert_eq!(
            h.events.0.lock().unwrap().len(),
            if acknowledge { 2 } else { 1 }
        );
        let RecordRead::Found(after) = StagedStore::new(&h.backend)
            .read(&h.tenant, &admitted.id(), h.clock.now())
            .await
            .unwrap()
        else {
            panic!("claim missing")
        };
        assert_eq!(after, committed);
        if !acknowledge {
            assert!(h.alpha.received.lock().unwrap().is_empty());
        }
        proxied.close().await;
        proxy.shutdown().await;
        h.close().await;
    }
    case_complete(
        "native.claim_acknowledged_vs_unknown",
        "claim_events_require_exact_acknowledgment_and_never_infer_lost_reply",
    );
}

#[tokio::test]
async fn source_saved_report_authority_uses_current_retained_policy_not_creation_fingerprint() {
    use ssx_application::saved::{SavedLimits, SavedPolicy, prepare_create};
    use ssx_infra::staged_service::{SavedReportSources, Sources};
    let c: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w06/evidence/saved-source-authority-outcomes.json"
    ))
    .unwrap();
    for row in c["cases"].as_array().unwrap() {
        let server = Server::start();
        let backend = Arc::new(server.store());
        let tenant = TenantId::new("default").unwrap();
        let store = ssx_infra::valkey::saved::SavedStore::new(backend.clone(), tenant.clone());
        let mut policy = source_policy();
        let initial_auth = BTreeMap::from([("wikipedia".into(), false), ("brave".into(), false)]);
        let sl = SavedLimits::default();
        let initial_policy = SavedPolicy {
            search: &policy,
            saved_granted: true,
            auth_configured: &initial_auth,
            limits: &sl,
        };
        let now = store.now().await.unwrap();
        let prepared = prepare_create(&initial_policy, tenant.as_str(), "saved", &json!({"query":row["definition"]["query"], "engines":row["definition"]["engines"], "interval_seconds":60,"expires_in_seconds":600}), now).unwrap();
        store.create(&prepared).await.unwrap();
        let definition_key = keys::saved("definition", &tenant, Some("saved")).unwrap();
        let Read::Found(definition_before) = backend.get(&definition_key).await else {
            panic!("definition missing")
        };
        let mut report = row["report"].clone();
        report["search_id"] = json!("saved");
        report["expires_at"] = json!(now + 600.);
        let report_key = keys::saved("report", &tenant, Some("saved:run")).unwrap();
        let report_bytes = ssx_json::to_vec(&report).unwrap();
        backend
            .put(
                &report_key,
                &report_bytes,
                std::num::NonZeroU64::new(900).unwrap(),
            )
            .await
            .unwrap();
        let _: i64 = redis::cmd("ZADD")
            .arg(keys::saved("reports", &tenant, Some("saved")).unwrap())
            .arg(now)
            .arg("run")
            .query(&mut server.connection())
            .unwrap();
        let mutations = &row["mutations"];
        let mut engines = policy.catalog.engines().to_vec();
        engines.retain(|e| {
            !mutations["catalog_remove"]
                .as_array()
                .is_some_and(|names| names.contains(&json!(e.name)))
        });
        for engine in &mut engines {
            if mutations["active_remove"]
                .as_array()
                .is_some_and(|names| names.contains(&json!(engine.name)))
            {
                engine.active = false;
            }
            let changes = &mutations["catalog_changes"][engine.name.as_str()];
            if let Some(v) = changes["enabled"].as_bool() {
                engine.enabled = v;
            }
            if let Some(v) = changes["auth_configured"].as_bool() {
                engine.auth_ready = v;
            }
            if let Some(v) = changes["circuit_open"].as_bool() {
                engine.circuit_open = v;
            }
            if let Some(v) = changes["last_known_status"].as_str() {
                engine.health_status = v.into();
                engine.available = v != "unavailable";
            }
            if let Some(v) = changes["categories"].as_array() {
                engine.categories = v.iter().map(|s| s.as_str().unwrap().to_string()).collect();
            }
        }
        policy.catalog = CatalogSnapshot::new(engines).unwrap();
        if let Some(v) = mutations["sensitive_engines"].as_array() {
            policy.policy.sensitive_engines =
                v.iter().map(|s| s.as_str().unwrap().to_string()).collect();
        }
        policy.policy.sensitive_allowed = mutations["targeted_sensitive_allowed"]
            .as_bool()
            .unwrap_or(false);
        let mut current_auth = initial_auth;
        for (engine, changes) in mutations["catalog_changes"]
            .as_object()
            .into_iter()
            .flatten()
        {
            if let Some(v) = changes["auth_configured"].as_bool() {
                current_auth.insert(engine.clone(), v);
            }
        }
        let sources = SavedReportSources {
            store,
            search: policy,
            granted: mutations["saved_granted"].as_bool().unwrap_or(true),
            auth_configured: current_auth,
            limits: sl,
        };
        let actual = match sources.resolve(&tenant, &row["artifact"]).await {
            Ok(v) => v,
            Err(e) => e.value(),
        };
        assert_eq!(actual, row["output"], "{}", row["id"].as_str().unwrap());
        assert!(
            matches!(backend.get(&definition_key).await, Read::Found(raw) if raw == definition_before)
        );
        assert!(matches!(backend.get(&report_key).await, Read::Found(raw) if raw == report_bytes));
        let staged_keys: Vec<String> = redis::cmd("KEYS")
            .arg("mcp:staged:*")
            .query(&mut server.connection())
            .unwrap();
        assert!(staged_keys.is_empty());
        backend.close().await;
        drop(server);
        case_complete(
            row["id"].as_str().unwrap(),
            "source_saved_report_authority_uses_current_retained_policy_not_creation_fingerprint",
        );
    }
}

struct RevokeBeforePublication {
    policy: PolicyService,
    calls: std::sync::atomic::AtomicUsize,
    limits: Mutex<Limits>,
}
impl ssx_infra::staged_runner::StagedAuthority for RevokeBeforePublication {
    fn validate(
        &self,
        record: &ssx_domain::staged::Record,
    ) -> Result<(), ssx_application::staged::Error> {
        let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let mut limits = self.limits.lock().unwrap();
        let result = revalidate(&self.policy, &limits, record);
        // Initial, postrenewal, postacquisition are permitted. The actual policy
        // changes immediately after postacquisition validation; presave must
        // independently observe it before sending any SAVE.
        if call == 3 {
            limits.granted = false;
        }
        result
    }
}
#[tokio::test]
async fn publication_rechecks_policy_after_acquisition_before_sending_save() {
    let h = Harness::new("[]", "[]", false).await;
    let admitted = h.admit("presave-policy").await;
    let authority = Arc::new(RevokeBeforePublication {
        policy: h.service.policy.clone(),
        calls: std::sync::atomic::AtomicUsize::new(0),
        limits: Mutex::new(limits()),
    });
    let mut proxy = RespProxy::start(&h.server, "if ARGV[4] == 'queued'", false).await;
    let proxied = Arc::new(Valkey::new(&proxy.url, Duration::from_secs(5)).unwrap());
    let mut runner = h.runner.clone();
    runner.authority = authority.clone();
    runner.store = proxied.clone();
    let (_stop, rx) = watch::channel(false);
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        runner.run_one(h.tenant.clone(), admitted.id(), rx),
    )
    .await
    .unwrap();
    let Err(ssx_infra::staged_runner::Error::Policy(error)) = result else {
        panic!("presave revocation ignored")
    };
    assert_eq!(
        error.value(),
        json!({"error":{"code":"tool_disabled","message":"staged search is disabled","grant":"MCP_GRANT_STAGED_SEARCH"}})
    );
    assert_eq!(authority.calls.load(std::sync::atomic::Ordering::SeqCst), 4);
    assert!(
        !proxy
            .observed
            .lock()
            .unwrap()
            .iter()
            .any(|frame| String::from_utf8_lossy(frame).contains("if ARGV[4] == 'queued'"))
    );
    let RecordRead::Found(retained) = StagedStore::new(&h.backend)
        .read(&h.tenant, &admitted.id(), h.clock.now())
        .await
        .unwrap()
    else {
        panic!("claim missing")
    };
    assert_eq!(retained, h.events.0.lock().unwrap()[1].record);
    assert_eq!(retained.value()["budget"]["reserved"], json!(1));
    assert_eq!(retained.value()["selected_result_attempt_id"], Value::Null);
    let snapshots: Vec<String> = redis::cmd("KEYS")
        .arg("mcp:snapshot:*")
        .query(&mut h.server.connection())
        .unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(h.alpha.received.lock().unwrap().len(), 1);
    assert!(h.beta.received.lock().unwrap().is_empty());
    assert_eq!(h.events.0.lock().unwrap().len(), 2);
    proxied.close().await;
    proxy.shutdown().await;
    h.close().await;
    case_complete(
        "native.presave_policy_recheck",
        "publication_rechecks_policy_after_acquisition_before_sending_save",
    );
}
#[tokio::test]
async fn typed_shared_search_failure_uses_reviewed_public_diagnostic_and_unknown_budget() {
    let h = Harness::new("[]", "[]", false).await;
    let admitted = h.admit("closed-acquisition").await;
    h.search.shutdown().await;
    let failed = h.run_one(admitted.id()).await.unwrap();
    let mut expected = h.events.0.lock().unwrap()[1].record.value().clone();
    expected["state"] = json!("failed");
    expected["stop_reason"] = json!("execution_failed");
    expected["budget"]["observed"] = Value::Null;
    expected["objectives"]["unmet"] =
        json!([{"objective":"fallback","reason":"initial_stage_failed"}]);
    expected["stages"].as_array_mut().unwrap()[0]["state"] = json!("failed");
    let attempt = &mut expected["stages"].as_array_mut().unwrap()[0]["attempts"]
        .as_array_mut()
        .unwrap()[0];
    attempt["state"] = json!("failed");
    attempt["finished_at"] = json!(h.clock.now().seconds());
    attempt["error"] =
        json!({"code":"execution_failed","message":"shared search acquisition failed"});
    assert_eq!(failed.value(), &expected);
    let RecordRead::Found(stored) = StagedStore::new(&h.backend)
        .read(&h.tenant, &admitted.id(), h.clock.now())
        .await
        .unwrap()
    else {
        panic!("completion missing")
    };
    assert_eq!(stored, failed);
    let events = h.events.0.lock().unwrap().clone();
    assert_eq!(events.len(), 3);
    assert_eq!(events[2].from, Some("running"));
    assert_eq!(events[2].to, "failed");
    assert_eq!(events[2].record, failed);
    assert!(h.requests.lock().unwrap().is_empty());
    assert!(h.alpha.received.lock().unwrap().is_empty());
    assert!(h.beta.received.lock().unwrap().is_empty());
    let snapshots: Vec<String> = redis::cmd("KEYS")
        .arg("mcp:snapshot:*")
        .query(&mut h.server.connection())
        .unwrap();
    assert!(snapshots.is_empty());
    h.close().await;
    case_complete(
        "native.typed_acquisition_failure_diagnostic",
        "typed_shared_search_failure_uses_reviewed_public_diagnostic_and_unknown_budget",
    );
}
fn review_corpus() -> Value {
    ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w06/evidence/review-public-source-outcomes.json"
    ))
    .unwrap()
}
fn review_submission(input: &Value) -> ssx_infra::staged_service::Submission {
    ssx_infra::staged_service::Submission {
        query: input["query"].as_str().map(str::to_owned),
        objectives: input["objectives"].clone(),
        initial_scope: input["initial_scope"].clone(),
        fallback_scope: (!input["fallback_scope"].is_null())
            .then(|| input["fallback_scope"].clone()),
        allow_scope_expansion: input["allow_scope_expansion"].as_bool().unwrap_or(false),
        filters: (!input["filters"].is_null()).then(|| input["filters"].clone()),
        idempotency_key: input["idempotency_key"].as_str().unwrap().into(),
        source: (!input["source"].is_null()).then(|| input["source"].clone()),
    }
}
fn review_view(
    input: &Value,
) -> Result<ssx_infra::staged_service::View, ssx_application::staged::Error> {
    ssx_infra::staged_service::View::parse(
        (!input["include"].is_null()).then_some(&input["include"]),
        (!input["max_results"].is_null()).then_some(&input["max_results"]),
        input["policy_max_results"].as_u64().unwrap_or(50),
    )
}
#[test]
fn source_review_invalid_and_default_views_preserve_complete_envelopes() {
    let c = review_corpus();
    for row in c["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"].as_str() == Some("view"))
    {
        let actual = match review_view(&row["input"]) {
            Ok(view) => json!([
                view.include
                    .into_iter()
                    .collect::<std::collections::BTreeSet<_>>(),
                view.limit
            ]),
            Err(error) => error.value(),
        };
        assert_eq!(actual, row["output"], "{}", row["id"].as_str().unwrap());
        case_complete(
            row["id"].as_str().unwrap(),
            "source_review_invalid_and_default_views_preserve_complete_envelopes",
        );
    }
}
struct ReviewService {
    server: Server,
    admin: Arc<Valkey>,
    backend: Arc<Valkey>,
    proxy: RespProxy,
    service: ssx_infra::staged_service::Service,
    events: Arc<EventLog>,
    tenant: TenantId,
}
impl ReviewService {
    async fn new(setup: &Value) -> Self {
        use ssx_application::saved::{SavedLimits, SavedPolicy, prepare_create};
        let server = Server::start();
        let admin = Arc::new(server.store());
        let tenant = TenantId::new("default").unwrap();
        let policy = source_policy();
        let auth = BTreeMap::from([("wikipedia".into(), false), ("brave".into(), false)]);
        let sl = SavedLimits::default();
        let saved = ssx_infra::valkey::saved::SavedStore::new(admin.clone(), tenant.clone());
        let now = saved.now().await.unwrap();
        let p = SavedPolicy {
            search: &policy,
            saved_granted: true,
            auth_configured: &auth,
            limits: &sl,
        };
        let prepared = prepare_create(&p,tenant.as_str(),"saved",&json!({"query":"current definition query","engines":["wikipedia"],"interval_seconds":60,"expires_in_seconds":600}),now).unwrap();
        saved.create(&prepared).await.unwrap();
        for run in ["run-a", "run-b"] {
            let report = json!({"search_id":"saved","run_id":run,"expires_at":now+600.,"status":"changed","revision":3,"incomparable_reasons":[],"observation":{"window":{"query":"historical public query","engines":["wikipedia"]}}});
            admin
                .put(
                    &keys::saved("report", &tenant, Some(&format!("saved:{run}"))).unwrap(),
                    &ssx_json::to_vec(&report).unwrap(),
                    std::num::NonZeroU64::new(900).unwrap(),
                )
                .await
                .unwrap();
            let _: i64 = redis::cmd("ZADD")
                .arg(keys::saved("reports", &tenant, Some("saved")).unwrap())
                .arg(now)
                .arg(run)
                .query(&mut server.connection())
                .unwrap();
        }
        if !setup["existing_record"].is_null() {
            let record =
                ssx_domain::staged::Record::from_value(setup["existing_record"].clone()).unwrap();
            let rk = record_key(&tenant, &record.id());
            admin
                .put(
                    &rk,
                    &record.encode().unwrap(),
                    std::num::NonZeroU64::new(900).unwrap(),
                )
                .await
                .unwrap();
            admin
                .put(
                    &keys::staged("idem", &tenant, Some("public-key")).unwrap(),
                    &ssx_json::to_vec(&json!({"record_key":rk})).unwrap(),
                    std::num::NonZeroU64::new(900).unwrap(),
                )
                .await
                .unwrap();
        }
        let proxy = RespProxy::start(&server, "w06-never-hold", false).await;
        let backend = Arc::new(Valkey::new(&proxy.url, Duration::from_secs(3)).unwrap());
        let events = Arc::new(EventLog::default());
        let clock = Arc::new(FixedTime(Mutex::new(
            setup["now"].as_f64().unwrap_or(1000.),
        )));
        let service = ssx_infra::staged_service::Service {
            store: backend.clone(),
            policy: policy.clone(),
            limits: limits(),
            authority: Arc::new(StaticStagedAuthority {
                policy: policy.clone(),
                limits: limits(),
            }),
            clock,
            snapshots: snapshot_service(backend.clone(), tenant.clone()),
            sources: Some(Arc::new(ssx_infra::staged_service::SavedReportSources {
                store: ssx_infra::valkey::saved::SavedStore::new(backend.clone(), tenant.clone()),
                search: policy,
                granted: true,
                auth_configured: auth,
                limits: sl,
            })),
            events: events.clone(),
        };
        if setup["store_mode"].as_str() == Some("unavailable") {
            backend.close().await;
        }
        Self {
            server,
            admin,
            backend,
            proxy,
            service,
            events,
            tenant,
        }
    }
    async fn close(mut self) {
        self.backend.close().await;
        self.proxy.shutdown().await;
        self.admin.close().await;
        drop(self.server);
    }
}
fn native_admission_presentation(source: &Value) -> Value {
    // F09's separately accepted additive pagination enforcement is included in
    // new native records. Retained original records keep their original shape.
    let mut expected = source.clone();
    for stage in expected["stages"].as_array_mut().unwrap() {
        stage["enforcement"]["pagination"] = json!({"requested":1,"status":"unsupported","reason":"no selected adapter enforces the pagination filter","enforced_by":[]});
    }
    expected
}
fn bind_fresh_operation(actual: &mut Value, expected: &Value) {
    // Opaque fresh identities differ across native OS randomness and the
    // deterministic Python fixture. Bind that one identity in the full view.
    // No query/plan/time/budget/scope/result/error field is normalized.
    if actual["operation_id"] != expected["operation_id"] {
        assert!(StagedSearchId::new(actual["operation_id"].as_str().unwrap()).is_ok());
        assert_eq!(actual["artifact"]["id"], actual["operation_id"]);
        actual["operation_id"] = expected["operation_id"].clone();
        actual["artifact"]["id"] = expected["artifact"]["id"].clone();
    }
}
#[tokio::test]
async fn source_review_public_preconditions_replay_sources_and_store_errors_are_exact() {
    let c = review_corpus();
    for row in c["cases"].as_array().unwrap().iter().filter(|r| {
        matches!(
            r["kind"].as_str(),
            Some("submit" | "get" | "retry" | "replay" | "source")
        )
    }) {
        let h = ReviewService::new(&row["setup"]).await;
        let input = &row["input"];
        let before_keys: Vec<String> = redis::cmd("KEYS")
            .arg("mcp:staged:v1:record:*")
            .query(&mut h.server.connection())
            .unwrap();
        let result = match row["kind"].as_str().unwrap() {
            "submit" | "replay" => {
                // Public view validation is an explicit transport-neutral seam;
                // these source cases carry otherwise valid preconditions.
                if input.get("include").is_some() {
                    match review_view(input) {
                        Err(e) => Err(e),
                        Ok(_) => h.service.submit(&h.tenant, &review_submission(input)).await,
                    }
                } else {
                    h.service.submit(&h.tenant, &review_submission(input)).await
                }
            }
            "get" => {
                h.service
                    .get(
                        &h.tenant,
                        &StagedSearchId::new(input["operation_id"].as_str().unwrap()).unwrap(),
                    )
                    .await
            }
            "retry" => {
                h.service
                    .retry(
                        &h.tenant,
                        &StagedSearchId::new(input["operation_id"].as_str().unwrap()).unwrap(),
                        input["retry_key"].as_str().unwrap(),
                    )
                    .await
            }
            "source" => {
                let result = h
                    .service
                    .sources
                    .as_ref()
                    .unwrap()
                    .resolve(&h.tenant, &input["source"])
                    .await;
                let actual = match result {
                    Ok(value) => value,
                    Err(error) => error.value(),
                };
                assert_eq!(actual, row["output"], "{}", row["id"].as_str().unwrap());
                assert!(h.events.0.lock().unwrap().is_empty());
                let after_keys: Vec<String> = redis::cmd("KEYS")
                    .arg("mcp:staged:v1:record:*")
                    .query(&mut h.server.connection())
                    .unwrap();
                assert_eq!(after_keys, before_keys);
                assert!(
                    !h.proxy
                        .observed
                        .lock()
                        .unwrap()
                        .iter()
                        .any(|f| f.windows(8).any(|w| w == b"$4\r\nEVAL"))
                );
                h.close().await;
                case_complete(
                    row["id"].as_str().unwrap(),
                    "source_review_public_preconditions_replay_sources_and_store_errors_are_exact",
                );
                continue;
            }
            _ => unreachable!(),
        };
        let mut actual = match result {
            Err(error) => error.value(),
            Ok(record) => {
                let rendered = h
                    .service
                    .render(&record, &review_view(input).unwrap(), shared_card)
                    .await
                    .unwrap();
                assert_eq!(
                    StagedStore::new(&h.admin)
                        .read(&h.tenant, &record.id(), h.service.clock.now())
                        .await
                        .unwrap(),
                    RecordRead::Found(record.clone())
                );
                if !row["after_record"].is_null() {
                    let mut captured = record.value().clone();
                    let expected = if row["setup"]["existing_record"].is_null()
                        || row["id"].as_str() == Some("review.submit.store.missing")
                    {
                        captured["operation_id"] = row["after_record"]["operation_id"].clone();
                        native_admission_presentation(&row["after_record"])
                    } else {
                        row["after_record"].clone()
                    };
                    assert_eq!(
                        captured,
                        expected,
                        "full record {}",
                        row["id"].as_str().unwrap()
                    );
                }
                rendered
            }
        };
        let fresh = (row["output"]["error"].is_null() && row["setup"]["existing_record"].is_null())
            || row["id"].as_str() == Some("review.submit.store.missing");
        let expected = if fresh {
            bind_fresh_operation(&mut actual, &row["output"]);
            native_admission_presentation(&row["output"])
        } else {
            row["output"].clone()
        };
        assert_eq!(actual, expected, "{}", row["id"].as_str().unwrap());
        let source_lookup = row["trace"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["method"].as_str() == Some("find_by_idempotency"));
        let frames = h.proxy.observed.lock().unwrap().clone();
        let native_lookup = frames
            .iter()
            .any(|f| String::from_utf8_lossy(f).contains("mcp:staged:v1:idem:"));
        if row["setup"]["store_mode"].as_str() != Some("unavailable") {
            assert_eq!(
                native_lookup,
                source_lookup,
                "lookup order {}",
                row["id"].as_str().unwrap()
            );
        }
        let admission = row["trace"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["method"].as_str() == Some("admit"));
        assert_eq!(
            frames
                .iter()
                .any(|f| String::from_utf8_lossy(f).contains("current.plan_digest == ARGV[1]")),
            admission,
            "admission {}",
            row["id"].as_str().unwrap()
        );
        let expected_events = usize::from(admission && row["output"]["error"].is_null());
        assert_eq!(
            h.events.0.lock().unwrap().len(),
            expected_events,
            "events {}",
            row["id"].as_str().unwrap()
        );
        if !admission {
            let after_keys: Vec<String> = redis::cmd("KEYS")
                .arg("mcp:staged:v1:record:*")
                .query(&mut h.server.connection())
                .unwrap();
            assert_eq!(after_keys, before_keys);
            if !row["setup"]["existing_record"].is_null() {
                let record =
                    ssx_domain::staged::Record::from_value(row["setup"]["existing_record"].clone())
                        .unwrap();
                assert!(
                    matches!(h.admin.get(&record_key(&h.tenant,&record.id())).await,Read::Found(raw) if raw==record.encode().unwrap())
                );
            }
        }
        assert!(
            !frames
                .iter()
                .any(|f| String::from_utf8_lossy(f).contains("mcp:snapshot:"))
        );
        h.close().await;
        case_complete(
            row["id"].as_str().unwrap(),
            "source_review_public_preconditions_replay_sources_and_store_errors_are_exact",
        );
    }
}
#[tokio::test]
async fn source_review_admission_collision_preserves_winner_and_exact_conflict() {
    let c = review_corpus();
    let row = c["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"].as_str() == Some("admission_race"))
        .unwrap();
    let h = ReviewService::new(&Value::Null).await;
    let key = keys::staged("idem", &h.tenant, Some("race-key")).unwrap();
    let needle = format!("$3\r\nGET\r\n${}\r\n{}\r\n", key.len(), key);
    let mut proxy = RespProxy::start(&h.server, &needle, false).await;
    let backend = Arc::new(Valkey::new(&proxy.url, Duration::from_secs(3)).unwrap());
    let mut loser = h.service.clone();
    loser.store = backend.clone();
    let tenant = h.tenant.clone();
    let second = review_submission(&row["input"][1]);
    let task = tokio::spawn(async move { loser.submit(&tenant, &second).await });
    proxy.reached().await;
    let winner = h
        .service
        .submit(&h.tenant, &review_submission(&row["input"][0]))
        .await
        .unwrap();
    proxy.release.notify_one();
    let conflict = task.await.unwrap().unwrap_err();
    assert_eq!(conflict.value(), row["output"][1]);
    let mut rendered = h
        .service
        .render(
            &winner,
            &review_view(&row["input"][0]).unwrap(),
            shared_card,
        )
        .await
        .unwrap();
    bind_fresh_operation(&mut rendered, &row["output"][0]);
    assert_eq!(rendered, native_admission_presentation(&row["output"][0]));
    assert_eq!(h.events.0.lock().unwrap().len(), 1);
    assert_eq!(h.events.0.lock().unwrap()[0].record, winner);
    let RecordRead::Found(after) = StagedStore::new(&h.admin)
        .read(&h.tenant, &winner.id(), h.service.clock.now())
        .await
        .unwrap()
    else {
        panic!("winner missing")
    };
    assert_eq!(after, winner);
    let mut winner_bound = winner.value().clone();
    winner_bound["operation_id"] = row["after_record"]["operation_id"].clone();
    assert_eq!(
        winner_bound,
        native_admission_presentation(&row["after_record"])
    );
    let record_keys: Vec<String> = redis::cmd("KEYS")
        .arg("mcp:staged:v1:record:*")
        .query(&mut h.server.connection())
        .unwrap();
    assert_eq!(record_keys.len(), 1);
    assert_eq!(
        proxy
            .observed
            .lock()
            .unwrap()
            .iter()
            .filter(|f| String::from_utf8_lossy(f).contains("current.plan_digest == ARGV[1]"))
            .count(),
        1
    );
    backend.close().await;
    proxy.shutdown().await;
    h.close().await;
    case_complete(
        row["id"].as_str().unwrap(),
        "source_review_admission_collision_preserves_winner_and_exact_conflict",
    );
}
