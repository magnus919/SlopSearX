//! W03 actual native planning, fenced admission, transport and recovery on private Valkey.
use ssx_application::{
    NoopSink, PortFuture, SearchEngine, cache::*, policy::*, scope::*, snapshot::*,
};
use ssx_domain::{research::*, *};
use ssx_infra::research_planning::*;
use ssx_infra::{
    research_runner::*,
    search::*,
    valkey::{research::*, snapshot::*, *},
};
use ssx_json::{Value, json};
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
    count: AtomicUsize,
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
                results: (0..self.count.load(Ordering::SeqCst))
                    .map(|i| SearchResult {
                        url: format!("https://example.test/{}/{i}", request.query),
                        title: format!("fake {i}"),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }
        })
    }
}
fn engine(held: bool) -> Arc<FixtureEngine> {
    Arc::new(FixtureEngine {
        count: AtomicUsize::new(3),
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
fn adaptive(
    jobs: ResearchStore,
    search: SearchService,
    snapshots: SnapshotService,
) -> AdaptiveResearch {
    AdaptiveResearch::new(
        jobs,
        search,
        snapshots,
        RunnerConfig {
            owner: "planner".into(),
            lease_seconds: NonZeroU64::new(2).unwrap(),
            poll_interval: Duration::from_millis(20),
            workers: NonZeroU64::new(2).unwrap(),
            ceilings: ResearchCeilings {
                queries: 8,
                engines_per_query: 4,
                results: 100,
            },
        },
        500,
    )
    .unwrap()
}
fn start_args() -> Value {
    json!({"question":"Compare maintenance and alternatives","max_queries":4,"max_attempts":4,"max_engine_attempts":4,"max_results":20,"subquestions":[{"id":"a","question":"Maintained?"},{"id":"b","question":"Alternatives?"}],"initial_plan":[{"query":"maintenance","engines":["fake"],"subquestion_id":"a"}]})
}
async fn first(service: &AdaptiveResearch, args: Value) -> ResearchJob {
    let j = service.start(tenant("org:test"), &args).await.unwrap();
    let (_stop, rx) = shutdown();
    service
        .run(tenant("org:test"), &j.job_id, rx)
        .await
        .unwrap()
}
fn followup(j: &ResearchJob) -> Value {
    let a = &j.queries[0].attempts[0];
    json!({"job_id":j.job_id,"query":"new maintenance evidence","engines":["fake"],"parent_attempt_id":a.attempt_id,"evidence_result_ids":[a.admitted_result_ids[0]],"rationale":"Investigate the lead","subquestion_id":"a"})
}
fn counts(server: &Server) -> u64 {
    redis::cmd("DBSIZE")
        .query(&mut server.connection())
        .unwrap()
}
#[tokio::test]
async fn decomposition_preview_execution_parity_and_no_preview_writes() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let before = counts(&server);
    let v=p.preview_initial(&json!({"question":"Compare maintenance and alternatives","engines":["fake"],"max_queries":3,"max_attempts":3,"max_engine_attempts":3,"subquestions":[{"id":"a","question":"maintenance"},{"id":"b","question":"alternatives"}]}),false).await.unwrap();
    assert_eq!(v["dispatched"], json!(false));
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    assert_eq!(counts(&server), before);
    assert_eq!(
        v["execution"]["arguments"]["initial_plan"][0]["query"],
        v["execution"]["arguments"]["question"]
    );
    let j = first(&p, v["execution"]["arguments"].clone()).await;
    assert_eq!(e.calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        j.queries
            .iter()
            .map(|q| q.planning_method.as_deref())
            .collect::<Vec<_>>(),
        vec![
            Some("original"),
            Some("decomposition"),
            Some("decomposition")
        ]
    );
    assert!(j.subquestions.values().all(|s| s["state"] == "unresolved"));
    assert_eq!(
        ResearchJob::from_payload(j.to_payload().unwrap()).unwrap(),
        j
    );
    search.shutdown().await;
}
#[tokio::test]
async fn malformed_decomposition_matrix_is_atomic() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let before = counts(&server);
    let base =
        json!({"question":"q","subquestions":[{"id":"a","question":"x"}],"engines":["fake"]});
    for changes in [
        json!({"max_queries":true}),
        json!({"max_attempts":1}),
        json!({"max_engine_attempts":1}),
        json!({"subquestions":[]}),
        json!({"subquestions":[{"id":"a","question":"x"},{"id":"a","question":"x"}]}),
        json!({"subquestions":[{"id":"a","question":"x","extra":"no"}]}),
        json!({"subquestions":[{"id":"a","question":"x","query":true}]}),
        json!({"subquestions":[{"id":"a","question":"x","intent":"science"}]}),
        json!({"subquestions":[{"id":"a","question":"x","engines":["fake","hibp"]}]}),
    ] {
        let mut args = base.clone();
        for (k, v) in changes.as_object().unwrap() {
            args[k.as_str()] = v.clone();
        }
        assert!(p.preview_initial(&args, false).await.is_err(), "{args:?}");
        assert_eq!(counts(&server), before);
        assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    }
    search.shutdown().await;
}
#[test]
fn identifiers_exact_prefix_unicode_and_punctuation() {
    for id in [
        "CVE-2026-12345",
        "10.1000/example",
        "2303.07678v2",
        "@scope/package",
        "CVE-٢٠٢٦-١٢٣٤",
        "@ſcope/Kit",
    ] {
        validate_variant(&format!("Find {id}"), &format!("evidence {id}")).unwrap();
        assert!(validate_variant(&format!("Find {id}"), "missing").is_err());
    }
    assert!(validate_variant("CVE-2026-1234", "CVE-2026-12345").is_err());
    validate_variant("See DOI 10.1000/example.", "10.1000/example evidence").unwrap();
    assert!(validate_variant("2303.07678v2", "2303.07678v3").is_err());
    validate_variant("@ſcope/Kit", "@scope/kit").unwrap();
    assert_eq!(
        query_identity(" Straße\u{1c}A ", &["b".into(), "a".into(), "a".into()]),
        query_identity("STRASSE A", &["a".into(), "b".into()])
    );
    assert_ne!(
        query_identity("x", &["a".into()]),
        query_identity("x", &["b".into()])
    );
}
#[tokio::test]
async fn variant_identifiers_revalidated_after_edited_preview() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let before = counts(&server);
    for id in [
        "CVE-2026-12345",
        "10.1000/example",
        "2303.07678v2",
        "@scope/package",
    ] {
        let v=p.preview_initial(&json!({"question":format!("Find {id}"),"variants":[format!("evidence {id}")],"engines":["fake"],"max_queries":2}),true).await.unwrap();
        let mut args = v["execution"]["arguments"].clone();
        args["initial_plan"].as_array_mut().unwrap()[1]["query"] = json!("missing identifier");
        assert_eq!(
            p.start(tenant("org:test"), &args).await.unwrap_err().code,
            "invalid_input"
        );
        assert_eq!(counts(&server), before);
        assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    }
    search.shutdown().await;
}
#[tokio::test]
async fn duplicate_variants_and_edited_initial_plans_never_persist() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let before = counts(&server);
    assert_eq!(
        p.preview_initial(
            &json!({"question":"original","variants":[" ORIGINAL "],"engines":["fake"]}),
            true
        )
        .await
        .unwrap_err()
        .code,
        "duplicate_query"
    );
    for variants in [true, false] {
        let args = if variants {
            json!({"question":"original","variants":["variant"],"engines":["fake"]})
        } else {
            json!({"question":"original","subquestions":[{"id":"a","question":"facet"}],"engines":["fake"]})
        };
        let v = p.preview_initial(&args, variants).await.unwrap();
        let mut args = v["execution"]["arguments"].clone();
        args["initial_plan"].as_array_mut().unwrap()[1]["query"] = json!(" ORIGINAL ");
        assert_eq!(
            p.start(tenant("org:test"), &args).await.unwrap_err().code,
            "duplicate_query"
        );
    }
    assert_eq!(counts(&server), before);
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    search.shutdown().await;
}
#[tokio::test]
async fn evidence_preview_extend_replay_and_history_roundtrip() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let j = first(&p, start_args()).await;
    let before = j.to_payload().unwrap();
    let size = counts(&server);
    let v = p
        .preview_followup(tenant("org:test"), &followup(&j))
        .await
        .unwrap();
    assert_eq!(v["dispatched"], json!(false));
    assert_eq!(v["evidence"][0]["title"], json!("fake 0"));
    assert_eq!(counts(&server), size);
    assert_eq!(
        p.load(tenant("org:test"), &j.job_id)
            .await
            .unwrap()
            .to_payload()
            .unwrap(),
        before
    );
    assert_eq!(e.calls.load(Ordering::SeqCst), 1);
    let mut args = v["execution"]["arguments"].clone();
    args["continuation_key"] = json!("next");
    let (_stop, rx) = shutdown();
    let extended = p
        .extend(tenant("org:test"), &args, rx.clone())
        .await
        .unwrap();
    assert_eq!(extended.queries[0].attempts, j.queries[0].attempts);
    assert_eq!(extended.queries[1].evidence_engines, vec!["fake"]);
    assert_eq!(
        extended.queries[1].evidence_result_ids,
        j.queries[0].attempts[0].admitted_result_ids[..1]
    );
    assert_eq!(
        ResearchJob::from_payload(extended.to_payload().unwrap()).unwrap(),
        extended
    );
    let done = p
        .update(
            tenant("org:test"),
            &json!({"job_id":j.job_id,"subquestion_states":{},"complete":true}),
        )
        .await
        .unwrap();
    let replay = p
        .extend(tenant("org:test"), &args, rx.clone())
        .await
        .unwrap();
    assert_eq!(replay, done);
    assert_eq!(e.calls.load(Ordering::SeqCst), 2);
    args["evidence_result_ids"] = json!([j.queries[0].attempts[0].admitted_result_ids[1]]);
    assert_eq!(
        p.extend(tenant("org:test"), &args, rx)
            .await
            .unwrap_err()
            .code,
        "idempotency_conflict"
    );
    search.shutdown().await;
}
#[tokio::test]
async fn malformed_followups_reject_without_budget_or_dispatch() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let j = first(&p, start_args()).await;
    let before = j.to_payload().unwrap();
    for changes in [
        json!({"parent_attempt_id":"foreign"}),
        json!({"evidence_result_ids":["foreign:0"]}),
        json!({"evidence_result_ids":[]}),
        json!({"evidence_result_ids":[true]}),
        json!({"evidence_result_ids":["a:0","a:0"]}),
        json!({"evidence_result_ids":(0..11).map(|i|format!("a:{i}")).collect::<Vec<_>>()}),
        json!({"subquestion_id":"foreign"}),
        json!({"rationale":""}),
        json!({"query":" MAINTENANCE "}),
    ] {
        let mut args = followup(&j);
        for (k, v) in changes.as_object().unwrap() {
            args[k.as_str()] = v.clone();
        }
        assert!(
            p.preview_followup(tenant("org:test"), &args).await.is_err(),
            "{args:?}"
        );
        args["planning_method"] = json!("evidence_followup");
        let (_stop, rx) = shutdown();
        assert!(p.extend(tenant("org:test"), &args, rx).await.is_err());
        assert_eq!(
            p.load(tenant("org:test"), &j.job_id)
                .await
                .unwrap()
                .to_payload()
                .unwrap(),
            before
        );
        assert_eq!(e.calls.load(Ordering::SeqCst), 1);
    }
    search.shutdown().await;
}
#[tokio::test]
async fn cross_tenant_and_unadmitted_snapshot_records_never_authorize() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let mut args = start_args();
    args["max_results"] = json!(1);
    let j = first(&p, args).await;
    let mut args = followup(&j);
    assert_eq!(
        p.preview_followup(tenant("other"), &args)
            .await
            .unwrap_err()
            .code,
        "invalid_job_id"
    );
    args["evidence_result_ids"] = json!([format!("{}:1", j.queries[0].cursor.as_ref().unwrap())]);
    assert_eq!(
        p.preview_followup(tenant("org:test"), &args)
            .await
            .unwrap_err()
            .code,
        "invalid_result_id"
    );
    assert_eq!(e.calls.load(Ordering::SeqCst), 1);
    assert_eq!(j.queries[0].attempts[0].result_count, 3);
    assert_eq!(j.queries[0].attempts[0].admitted_result_ids.len(), 1);
    search.shutdown().await;
}
#[tokio::test]
async fn resolved_progress_and_completed_plans_revalidate_before_admission() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let j = first(&p, start_args()).await;
    let preview = p
        .preview_followup(tenant("org:test"), &followup(&j))
        .await
        .unwrap();
    let (_stop, rx) = shutdown();
    p.update(
        tenant("org:test"),
        &json!({"job_id":j.job_id,"subquestion_states":{"a":"resolved"}}),
    )
    .await
    .unwrap();
    assert_eq!(
        p.extend(tenant("org:test"), &preview["execution"]["arguments"], rx)
            .await
            .unwrap_err()
            .code,
        "invalid_job_state"
    );
    assert_eq!(e.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        p.load(tenant("org:test"), &j.job_id)
            .await
            .unwrap()
            .queries
            .len(),
        1
    );
    search.shutdown().await;
}
#[tokio::test]
async fn legacy_untagged_repeat_and_digest_remain_compatible() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let j = first(&p, start_args()).await;
    let (_stop, rx) = shutdown();
    let rejected=p.extend(tenant("org:test"),&json!({"job_id":j.job_id,"query":" MAINTENANCE ","engines":["fake"],"planning_method":"terminology_expansion"}),rx.clone()).await.unwrap_err();
    assert_eq!(rejected.code, "duplicate_query");
    let legacy=p.extend(tenant("org:test"),&json!({"job_id":j.job_id,"query":"maintenance","engines":["fake"],"continuation_key":"legacy"}),rx).await.unwrap();
    assert_eq!(legacy.queries.len(), 2);
    assert_eq!(e.calls.load(Ordering::SeqCst), 2);
    assert!(legacy.queries[1].planning_method.is_none());
    assert_eq!(
        legacy.queries[1].continuation_digest,
        Some(continuation_digest(&legacy.queries[1]))
    );
    search.shutdown().await;
}
#[tokio::test]
async fn denied_grants_and_evidence_intent_replay_reject_before_spend() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) =
        setup_policy(server.store(), e.clone(), &["research", "science"]);
    let p = adaptive(jobs.clone(), search.clone(), snapshots.clone());
    let mut args = start_args();
    args["initial_plan"].as_array_mut().unwrap()[0]["intent"] = json!("science");
    let j = first(&p, args).await;
    let v = p
        .preview_followup(tenant("org:test"), &followup(&j))
        .await
        .unwrap();
    let mut args = v["execution"]["arguments"].clone();
    args["continuation_key"] = json!("bound-parent");
    let (_stop, rx) = shutdown();
    let accepted = p
        .extend(tenant("org:test"), &args, rx.clone())
        .await
        .unwrap();
    let (_, _, denied_search, _) = setup_policy(server.store(), e.clone(), &["research"]);
    let denied = adaptive(jobs.clone(), denied_search.clone(), snapshots.clone());
    assert!(denied.extend(tenant("org:test"), &args, rx).await.is_err());
    assert_eq!(e.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        jobs.for_tenant(tenant("org:test"))
            .load(&JobId::new(&j.job_id).unwrap())
            .await,
        JobRead::Found(Box::new(accepted))
    );
    let (_, _, off_search, _) = setup_policy(server.store(), e.clone(), &[]);
    let off = adaptive(jobs, off_search.clone(), snapshots);
    for variants in [true, false] {
        assert!(off.preview_initial(&json!({"question":"q","variants":["v"],"subquestions":[{"id":"a","question":"x"}]}),variants).await.is_err());
    }
    assert!(
        off.preview_followup(tenant("org:test"), &followup(&j))
            .await
            .is_err()
    );
    search.shutdown().await;
    denied_search.shutdown().await;
    off_search.shutdown().await;
}
#[tokio::test]
async fn exact_query_and_result_cap_completion_survives_reload_and_deadline() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs.clone(), search.clone(), snapshots.clone());
    for results in [1, 20] {
        let mut args = start_args();
        args["max_queries"] = json!(1);
        args["subquestions"] = json!([{"id":"a","question":"Maintained?"}]);
        args["max_results"] = json!(results);
        let j = first(&p, args).await;
        let reason = if results == 1 {
            "result_budget_exhausted"
        } else {
            "query_budget_exhausted"
        };
        assert_eq!(j.stop_reason.as_deref(), Some(reason));
        assert!(!j.caller_completed);
        let key = ssx_infra::valkey::keys::research("job", &tenant("org:test"), &j.job_id).unwrap();
        let raw: String = redis::cmd("GET")
            .arg(&key)
            .query(&mut server.connection())
            .unwrap();
        let mut payload: Value = ssx_json::from_str(&raw).unwrap();
        payload["deadline"] = json!(1.0);
        redis::cmd("SET")
            .arg(key)
            .arg(ssx_json::to_string(&payload).unwrap())
            .query::<()>(&mut server.connection())
            .unwrap();
        let request = json!({"job_id":j.job_id,"subquestion_states":{"a":"resolved"},"complete":true,"rationale":"evidence is sufficient"});
        let complete = p.update(tenant("org:test"), &request).await.unwrap();
        assert!(complete.caller_completed);
        assert_eq!(complete.stop_reason.as_deref(), Some(reason));
        assert_eq!(complete.queries, j.queries);
        assert_eq!(complete.budget_used, j.budget_used);
        let reloaded = adaptive(
            ResearchStore::new(Arc::new(server.store()), tenant("poller")),
            search.clone(),
            snapshots.clone(),
        );
        assert_eq!(
            reloaded.update(tenant("org:test"), &request).await.unwrap(),
            complete
        );
        let mut conflict = request.clone();
        conflict["rationale"] = json!("different");
        assert_eq!(
            reloaded
                .update(tenant("org:test"), &conflict)
                .await
                .unwrap_err()
                .code,
            "idempotency_conflict"
        );
        let mut empty = request;
        empty["subquestion_states"] = json!({});
        assert_eq!(
            reloaded
                .update(tenant("org:test"), &empty)
                .await
                .unwrap_err()
                .code,
            "idempotency_conflict"
        );
    }
    assert_eq!(e.calls.load(Ordering::SeqCst), 2);
    search.shutdown().await;
}
#[tokio::test]
async fn concurrent_extension_serializes_last_slot_and_keeps_history() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs.clone(), search.clone(), snapshots.clone());
    let mut args = start_args();
    args["max_queries"] = json!(2);
    let j = first(&p, args).await;
    let held = engine(true);
    let (_, _, held_search, _) = setup(&server, held.clone());
    let second = adaptive(jobs, held_search.clone(), snapshots);
    let mut args = followup(&j);
    args["planning_method"] = json!("evidence_followup");
    let (_stop, rx) = shutdown();
    let task = {
        let p = second.clone();
        let args = args.clone();
        let rx = rx.clone();
        tokio::spawn(async move { p.extend(tenant("org:test"), &args, rx).await })
    };
    tokio::time::timeout(Duration::from_secs(5), held.entered.notified())
        .await
        .unwrap();
    let mut competing = args.clone();
    competing["query"] = json!("competing");
    assert_eq!(
        p.extend(tenant("org:test"), &competing, rx.clone())
            .await
            .unwrap_err()
            .code,
        "job_busy"
    );
    assert_eq!(
        p.update(
            tenant("org:test"),
            &json!({"job_id":j.job_id,"subquestion_states":{},"complete":true})
        )
        .await
        .unwrap_err()
        .code,
        "job_busy"
    );
    held.release.notify_waiters();
    let complete = task.await.unwrap().unwrap();
    assert_eq!(complete.queries.len(), 2);
    assert_eq!(complete.budget_used["attempts"], 2);
    assert_eq!(complete.queries[0].attempts, j.queries[0].attempts);
    assert_eq!(held.calls.load(Ordering::SeqCst), 1);
    assert_eq!(held.drops.load(Ordering::SeqCst), 1);
    search.shutdown().await;
    held_search.shutdown().await;
}
#[tokio::test]
async fn native_recovery_retains_uncertain_attempt_and_prior_admitted_evidence() {
    let server = Server::start();
    let e = engine(false);
    let (runner, jobs, search, _snapshots) = setup(&server, e.clone());
    let scoped = jobs.for_tenant(tenant("org:test"));
    let mut j = job("org:test");
    j.queries[0].state = "running".into();
    j.state = "running".into();
    j.queries[0].attempts = vec![ResearchQueryAttempt {
        attempt_id: "orphan".into(),
        state: "running".into(),
        ..Default::default()
    }];
    scoped.create(&j).await.unwrap();
    let (_stop, rx) = shutdown();
    let recovered = runner.run_once(rx).await.unwrap().unwrap();
    assert_eq!(recovered.queries[0].attempts[0].attempt_id, "orphan");
    assert_eq!(recovered.queries[0].attempts[0].state, "interrupted");
    assert_eq!(recovered.queries[0].attempts.len(), 2);
    assert_eq!(recovered.budget_used["attempts"], 2);
    assert_eq!(e.calls.load(Ordering::SeqCst), 1);
    assert_eq!(found(&scoped).await, recovered);
    search.shutdown().await;
}
fn source_setup(server: &Server, e: Arc<FixtureEngine>) -> (AdaptiveResearch, SearchService) {
    let store = Arc::new(server.store());
    let jobs = ResearchStore::new(store.clone(), tenant("poller"));
    let mut deps = dependencies(vec![("wikipedia", e)]);
    deps.policy.policy.grants.insert("research".into());
    let search = SearchService::new(SearchConfig::default(), deps).unwrap();
    let snapshots = SnapshotService::new(
        Arc::new(ValkeySnapshots::new(store)),
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        tenant("poller"),
        SnapshotTtl::new(60).unwrap(),
    );
    (
        AdaptiveResearch::new(
            jobs,
            search.clone(),
            snapshots,
            RunnerConfig {
                owner: "source-planner".into(),
                lease_seconds: NonZeroU64::new(2).unwrap(),
                poll_interval: Duration::from_millis(20),
                workers: NonZeroU64::new(2).unwrap(),
                ceilings: ResearchCeilings {
                    queries: 20,
                    engines_per_query: 10,
                    results: 500,
                },
            },
            500,
        )
        .unwrap(),
        search,
    )
}
#[tokio::test]
async fn complete_actual_source_previews_and_identifier_observations_match() {
    let source: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w03/source-observation-001.json"
    ))
    .unwrap();
    let server = Server::start();
    let e = engine(false);
    let (p, search) = source_setup(&server, e.clone());
    let before = counts(&server);
    for row in source["observations"].as_array().unwrap() {
        match row["kind"].as_str().unwrap() {
            "decomposition" | "variants" => {
                let output = p
                    .preview_initial(&row["arguments"], row["kind"] == json!("variants"))
                    .await
                    .unwrap_or_else(|e| e.value());
                assert_eq!(output, row["output"], "full output mismatch: {row:?}");
                assert_eq!(e.calls.load(Ordering::SeqCst), 0);
                assert_eq!(counts(&server), before);
            }
            "identifier" => {
                let actual = validate_variant(
                    row["question"].as_str().unwrap(),
                    row["variant"].as_str().unwrap(),
                )
                .err()
                .map(|e| e.message);
                assert_eq!(json!(actual), row["error"], "{row:?}");
            }
            "metadata" => {
                let ids = row["ids"].as_array().unwrap();
                if ids.iter().all(Value::is_string) {
                    let q = ResearchQuery {
                        planning_method: row["method"].as_str().map(str::to_owned),
                        evidence_result_ids: ids
                            .iter()
                            .map(|s| s.as_str().unwrap().into())
                            .collect(),
                        parent_attempt_id: row["parent"].as_str().map(str::to_owned),
                        rationale: row["rationale"].as_str().map(str::to_owned),
                        ..Default::default()
                    };
                    let actual = validate_metadata(&q).err().map(|e| e.message);
                    assert_eq!(json!(actual), row["error"], "{row:?}");
                } else {
                    let error=p.prepare_query(&json!({"query":"q","engines":["wikipedia"],"evidence_result_ids":row["ids"]}),1).await.unwrap_err();
                    assert_eq!(error.code, "invalid_input");
                }
            }
            kind => panic!("unhandled oracle case {kind}"),
        }
    }
    search.shutdown().await;
}
fn mutate_job(server: &Server, j: &ResearchJob, f: impl FnOnce(&mut Value)) {
    let key = ssx_infra::valkey::keys::research("job", &tenant(&j.tenant), &j.job_id).unwrap();
    let raw: String = redis::cmd("GET")
        .arg(&key)
        .query(&mut server.connection())
        .unwrap();
    let mut payload: Value = ssx_json::from_str(&raw).unwrap();
    f(&mut payload);
    redis::cmd("SET")
        .arg(key)
        .arg(ssx_json::to_string(&payload).unwrap())
        .query::<()>(&mut server.connection())
        .unwrap();
}
fn mutate_snapshot(server: &Server, j: &ResearchJob, f: impl FnOnce(&mut Value)) {
    let key = ssx_infra::valkey::keys::snapshot(
        &tenant(&j.tenant),
        j.queries[0].cursor.as_ref().unwrap(),
    );
    let raw: String = redis::cmd("GET")
        .arg(&key)
        .query(&mut server.connection())
        .unwrap();
    let mut payload: Value = ssx_json::from_str(&raw).unwrap();
    f(&mut payload);
    redis::cmd("SET")
        .arg(key)
        .arg(ssx_json::to_string(&payload).unwrap())
        .query::<()>(&mut server.connection())
        .unwrap();
}
#[tokio::test]
async fn stale_snapshot_and_actual_provenance_denied_without_evidence_disclosure_or_spend() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    for mode in [
        "expired",
        "foreign_source",
        "empty_sources",
        "cross_tenant",
        "missing",
    ] {
        let j = first(&p, start_args()).await;
        let v = p
            .preview_followup(tenant("org:test"), &followup(&j))
            .await
            .unwrap();
        let before = j.to_payload().unwrap();
        if mode == "missing" {
            let key = ssx_infra::valkey::keys::snapshot(
                &tenant(&j.tenant),
                j.queries[0].cursor.as_ref().unwrap(),
            );
            redis::cmd("DEL")
                .arg(key)
                .query::<u64>(&mut server.connection())
                .unwrap();
        } else {
            mutate_snapshot(&server, &j, |value| match mode {
                "expired" => {
                    value["created_at"] = json!(now().seconds() - 2.0);
                    value["expires_at"] = json!(now().seconds() - 1.0);
                }
                "foreign_source" => {
                    let r = &mut value["results"].as_array_mut().unwrap()[0];
                    r["engine"] = json!("hibp");
                    r["engines"] = json!(["hibp"]);
                }
                "empty_sources" => {
                    let r = &mut value["results"].as_array_mut().unwrap()[0];
                    r["engine"] = json!("");
                    r["engines"] = json!([]);
                }
                "cross_tenant" => value["tenant"] = json!("other"),
                _ => unreachable!(),
            });
        }
        let denied = p
            .preview_followup(tenant("org:test"), &followup(&j))
            .await
            .unwrap_err();
        assert!(!denied.value().as_object().unwrap().contains_key("evidence"));
        if mode == "expired" {
            assert_eq!(denied.code, "expired_handle");
        }
        let (_stop, rx) = shutdown();
        assert!(
            p.extend(tenant("org:test"), &v["execution"]["arguments"], rx)
                .await
                .is_err()
        );
        assert_eq!(
            p.load(tenant("org:test"), &j.job_id)
                .await
                .unwrap()
                .to_payload()
                .unwrap(),
            before
        );
    }
    assert_eq!(e.calls.load(Ordering::SeqCst), 5);
    search.shutdown().await;
}
#[tokio::test]
async fn followup_state_deadline_workflow_store_and_terminal_parent_matrix() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs.clone(), search.clone(), snapshots);
    for state in ["queued", "running", "cancelled", "expired"] {
        let j = first(&p, start_args()).await;
        mutate_job(&server, &j, |v| v["state"] = json!(state));
        assert_eq!(
            p.preview_followup(tenant("org:test"), &followup(&j))
                .await
                .unwrap_err()
                .code,
            "invalid_job_state"
        );
    }
    let j = first(&p, start_args()).await;
    mutate_job(&server, &j, |v| {
        v["deadline"] = json!(now().seconds() - 1.0)
    });
    assert_eq!(
        p.preview_followup(tenant("org:test"), &followup(&j))
            .await
            .unwrap_err()
            .code,
        "deadline_exceeded"
    );
    let j = first(&p, start_args()).await;
    mutate_job(&server, &j, |v| {
        v["workflow"] = json!({"kind":"dependency_dossier"})
    });
    assert!(
        p.preview_followup(tenant("org:test"), &followup(&j))
            .await
            .is_err()
    );
    for state in [
        "running",
        "pending",
        "invalid",
        "done",
        "failed",
        "cancelled",
        "expired",
        "interrupted",
    ] {
        let j = first(&p, start_args()).await;
        mutate_job(&server, &j, |v| {
            v["queries"].as_array_mut().unwrap()[0]["attempts"]
                .as_array_mut()
                .unwrap()[0]["state"] = json!(state)
        });
        let result = p.preview_followup(tenant("org:test"), &followup(&j)).await;
        if ["done", "failed", "cancelled", "expired", "interrupted"].contains(&state) {
            assert!(result.is_ok(), "{state}: {result:?}");
        } else {
            assert!(result.is_err(), "{state}");
        }
    }
    let j = first(&p, start_args()).await;
    jobs.for_tenant(tenant("org:test"))
        .load(&JobId::new(&j.job_id).unwrap())
        .await;
    let unavailable = Valkey::new(
        "unix:///private/tmp/w03-does-not-exist.sock",
        Duration::from_millis(50),
    )
    .unwrap();
    let dead = adaptive(
        ResearchStore::new(Arc::new(unavailable), tenant("poller")),
        search.clone(),
        p_snapshots(&server),
    );
    assert_eq!(
        dead.preview_followup(tenant("org:test"), &followup(&j))
            .await
            .unwrap_err()
            .code,
        "store_unavailable"
    );
    search.shutdown().await;
}
fn p_snapshots(server: &Server) -> SnapshotService {
    SnapshotService::new(
        Arc::new(ValkeySnapshots::new(Arc::new(server.store()))),
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        tenant("poller"),
        SnapshotTtl::new(60).unwrap(),
    )
}
struct HeldSnapshotRead {
    inner: ValkeySnapshots,
    entered: Notify,
    release: Notify,
}
impl SnapshotRepository for HeldSnapshotRead {
    fn create<'a>(
        &'a self,
        snapshot: &'a SearchSnapshot,
        ttl: NonZeroU64,
    ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
        self.inner.create(snapshot, ttl)
    }
    fn read<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a SnapshotId,
    ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
        Box::pin(async move {
            let result = self.inner.read(tenant, id).await;
            self.entered.notify_one();
            self.release.notified().await;
            result
        })
    }
}
#[tokio::test]
async fn progress_changes_during_snapshot_read_fail_at_authoritative_claim_before_budget() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs.clone(), search.clone(), snapshots);
    let j = first(&p, start_args()).await;
    let v = p
        .preview_followup(tenant("org:test"), &followup(&j))
        .await
        .unwrap();
    let held = Arc::new(HeldSnapshotRead {
        inner: ValkeySnapshots::new(Arc::new(server.store())),
        entered: Notify::new(),
        release: Notify::new(),
    });
    let snapshots = SnapshotService::new(
        held.clone(),
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        tenant("poller"),
        SnapshotTtl::new(60).unwrap(),
    );
    let delayed = adaptive(jobs, search.clone(), snapshots);
    let task = tokio::spawn({
        let args = v["execution"]["arguments"].clone();
        async move {
            let (_stop, rx) = shutdown();
            delayed.extend(tenant("org:test"), &args, rx).await
        }
    });
    tokio::time::timeout(Duration::from_secs(5), held.entered.notified())
        .await
        .unwrap();
    let resolved = p
        .update(
            tenant("org:test"),
            &json!({"job_id":j.job_id,"subquestion_states":{"a":"resolved"}}),
        )
        .await
        .unwrap();
    held.release.notify_one();
    assert_eq!(
        error_value(task.await.unwrap()),
        json!({"error":{"code":"invalid_job_state","message":"subquestion is already caller-resolved","field":"query"}})
    );
    let current = p.load(tenant("org:test"), &j.job_id).await.unwrap();
    assert_eq!(current.queries, j.queries);
    assert_eq!(current.budget_used, resolved.budget_used);
    assert_eq!(e.calls.load(Ordering::SeqCst), 1);
    search.shutdown().await;
}
#[tokio::test]
async fn nonbudget_late_completion_and_legacy_completion_replay_keep_metadata_contract() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let j = first(&p, start_args()).await;
    mutate_job(&server, &j, |v| {
        v["deadline"] = json!(now().seconds() - 1.0)
    });
    let denied = p
        .update(
            tenant("org:test"),
            &json!({"job_id":j.job_id,"subquestion_states":{"a":"resolved"},"complete":true}),
        )
        .await
        .unwrap_err();
    assert_eq!(denied.code, "deadline_exceeded");
    let current = p.load(tenant("org:test"), &j.job_id).await.unwrap();
    assert!(!current.caller_completed);
    assert_eq!(current.queries, j.queries);
    let mut args = start_args();
    args["max_results"] = json!(1);
    let j = first(&p, args).await;
    let request = json!({"job_id":j.job_id,"subquestion_states":{"a":"resolved"},"complete":true,"rationale":"enough"});
    p.update(tenant("org:test"), &request).await.unwrap();
    mutate_job(&server, &j, |v| {
        v.as_object_mut()
            .unwrap()
            .shift_remove("completion_subquestion_states");
    });
    let mut empty = request.clone();
    empty["subquestion_states"] = json!({});
    assert_eq!(
        p.update(tenant("org:test"), &empty).await.unwrap_err().code,
        "idempotency_conflict"
    );
    let replay = p.update(tenant("org:test"), &request).await.unwrap();
    assert!(replay.completion_subquestion_states.is_none());
    assert_eq!(
        replay.stop_reason.as_deref(),
        Some("result_budget_exhausted")
    );
    search.shutdown().await;
}
#[tokio::test]
async fn pure_planning_policy_never_calls_configured_optional_router() {
    let server = Server::start();
    let e = engine(false);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/route", listener.local_addr().unwrap());
    let mut deps = dependencies(vec![("fake", e.clone())]);
    deps.policy.policy.grants.insert("research".into());
    deps.jev = Some(Arc::new(ssx_infra::jev::JevRouter::with_endpoint(
        ssx_infra::jev::JevConfig::from_environment(&BTreeMap::from([(
            "TYPESAFE_API_KEY".into(),
            "fixture-only".into(),
        )]))
        .unwrap(),
        ssx_registry::Registry::builtins().unwrap(),
        endpoint,
    )));
    let search = SearchService::new(SearchConfig::default(), deps).unwrap();
    let p = adaptive(
        ResearchStore::new(Arc::new(server.store()), tenant("poller")),
        search.clone(),
        p_snapshots(&server),
    );
    let before = counts(&server);
    let v = p
        .preview_initial(&json!({"question":"original","variants":["variant"]}), true)
        .await
        .unwrap();
    assert_eq!(v["dispatched"], json!(false));
    assert_eq!(counts(&server), before);
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
    let q = p
        .prepare_query(&json!({"query":"original","engines":["fake"]}), 1)
        .await
        .unwrap();
    let a = search
        .prepare_research_plan(&ScopeProposal::from_research_query(&q))
        .unwrap();
    let b = search
        .prepare_scope(&ScopeProposal::from_research_query(&q))
        .await
        .unwrap();
    assert_eq!(a.preview(), b.preview());
    search.shutdown().await;
}
#[tokio::test]
async fn complete_default_template_records_order_bounds_and_warnings_match_actual_source() {
    let server = Server::start();
    let e = engine(false);
    let source: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w03/template-observation-001.json"
    ))
    .unwrap();
    for row in source["observations"].as_array().unwrap() {
        let mut deps = dependencies(vec![("fake", e.clone())]);
        deps.policy.policy.grants.insert("research".into());
        deps.policy.policy.sensitive_allowed = row["sensitive_allowed"].as_bool().unwrap();
        deps.policy.catalog = CatalogSnapshot::new(
            source["catalog"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| {
                    let mut cap = capability(c["name"].as_str().unwrap());
                    cap.categories = c["categories"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s.as_str().unwrap().into())
                        .collect();
                    cap
                })
                .collect(),
        )
        .unwrap();
        let search = SearchService::new(SearchConfig::default(), deps).unwrap();
        let p = adaptive(
            ResearchStore::new(Arc::new(server.store()), tenant("poller")),
            search.clone(),
            p_snapshots(&server),
        );
        let (q, w) = p
            .plan_defaults(
                "Question CVE-2026-12345",
                row["strategy"].as_str().unwrap(),
                row["max_queries"].as_u64().unwrap(),
                row["max_engines_per_query"].as_u64().unwrap(),
            )
            .unwrap();
        assert_eq!(ssx_json::to_value(q).unwrap(), row["queries"], "{row:?}");
        assert_eq!(json!(w), row["warnings"]);
        assert_eq!(e.calls.load(Ordering::SeqCst), 0);
        search.shutdown().await;
    }
}
// Captured naive timestamps belong to one source-host timezone. Run only the
// pure capture assertions in an owned subprocess so parallel tests never mutate
// process-global TZ, and keep normal cargo test independent of its host zone.
fn capture_timezone_child(test: &str) -> bool {
    const MARKER: &str = "SSX_W03_CAPTURE_TIMEZONE_CHILD";
    if std::env::var(MARKER).as_deref() == Ok(test) {
        assert_eq!(std::env::var("TZ").as_deref(), Ok("America/New_York"));
        return false;
    }
    // Production parsing still follows the actual caller process's local zone.
    let naive = chrono::NaiveDate::from_ymd_opt(2026, 10, 17)
        .unwrap()
        .and_hms_opt(14, 30, 12)
        .unwrap();
    let local = chrono::TimeZone::from_local_datetime(&chrono::Local, &naive)
        .single()
        .unwrap();
    assert_eq!(
        parse_iso_deadline("2026-10-17T14:30:12"),
        Some(local.timestamp() as f64)
    );
    struct Owned {
        child: Option<Child>,
        dir: PathBuf,
    }
    impl Drop for Owned {
        fn drop(&mut self) {
            if let Some(child) = &mut self.child {
                let _ = child.kill();
                let _ = child.wait();
            }
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
    let mut bytes = [0; 16];
    fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut bytes)
        .unwrap();
    let nonce: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let dir = std::env::temp_dir().join(format!("ssx-w03-timezone-{nonce}"));
    fs::create_dir(&dir).unwrap();
    let mut owner = Owned { child: None, dir };
    let output_path = owner.dir.join("child.log");
    let output = fs::File::create(&output_path).unwrap();
    owner.child = Some(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(MARKER, test)
            .env("TZ", "America/New_York")
            .stdin(Stdio::null())
            .stdout(output.try_clone().unwrap())
            .stderr(output)
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = owner.child.as_mut().unwrap().try_wait().unwrap() {
            owner.child = None; // try_wait collected the terminal child.
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "timezone fixture child exceeded deadline"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    let mut raw = String::new();
    fs::File::open(output_path)
        .unwrap()
        .take(65536)
        .read_to_string(&mut raw)
        .unwrap();
    assert!(
        status.success(),
        "timezone fixture {test} failed: {status}\n{raw}"
    );
    assert!(
        raw.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
        "timezone fixture did not execute exactly once: {raw}"
    );
    assert!(
        raw.contains(&format!("test {test} ... ok")),
        "timezone fixture name missing: {raw}"
    );
    true
}

#[test]
fn complete_deadline_iso_formats_clamps_and_errors_match_actual_source() {
    if capture_timezone_child("complete_deadline_iso_formats_clamps_and_errors_match_actual_source")
    {
        return;
    }
    let source: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w03/deadline-observation-001.json"
    ))
    .unwrap();
    for row in source["observations"].as_array().unwrap() {
        let actual = resolve_deadline_at(
            Some(&row["input"]),
            source["now"].as_f64().unwrap(),
            source["default_seconds"].as_u64().unwrap(),
        )
        .map(|v| json!(v))
        .unwrap_or_else(|e| e.value());
        assert_eq!(actual, row["output"], "{row:?}");
    }
}
#[tokio::test]
async fn default_science_template_retains_internal_grant_distinction_and_caller_cannot_flip_flag() {
    let server = Server::start();
    let e = engine(false);
    let mut deps = dependencies(vec![("wikipedia", e.clone()), ("arxiv", e.clone())]);
    deps.policy.policy.grants.insert("research".into());
    let mut caps = vec![capability("wikipedia"), capability("arxiv")];
    caps[0].categories = vec!["general".into(), "reference".into()];
    caps[1].categories = vec!["science".into()];
    deps.policy.catalog = CatalogSnapshot::new(caps).unwrap();
    let search = SearchService::new(SearchConfig::default(), deps).unwrap();
    let p = adaptive(
        ResearchStore::new(Arc::new(server.store()), tenant("poller")),
        search.clone(),
        p_snapshots(&server),
    );
    let j = first(&p, json!({"question":"default templates","max_queries":3})).await;
    assert_eq!(
        j.queries
            .iter()
            .map(|q| q.intent.as_str())
            .collect::<Vec<_>>(),
        vec!["web", "science", "reference"]
    );
    assert!(j.queries.iter().all(|q| !q.requires_intent_grant));
    assert_eq!(e.calls.load(Ordering::SeqCst), 3);
    assert_eq!(p.start(tenant("org:test"),&json!({"question":"explicit","initial_plan":[{"query":"science","intent":"science","engines":["arxiv"]}]})).await.unwrap_err().code,"tool_disabled");
    for flag in [true, false] {
        assert_eq!(p.start(tenant("org:test"),&json!({"question":"forged","initial_plan":[{"query":"q","engines":["wikipedia"],"requires_intent_grant":flag}]})).await.unwrap_err().code,"invalid_input");
    }
    assert_eq!(e.calls.load(Ordering::SeqCst), 3);
    let before = p.get_job(tenant("org:test"), &j.job_id).await.unwrap();
    let bytes = stored_values(&server);
    let science = &j.queries[1];
    assert!(!science.requires_intent_grant);
    let proposal = json!({"job_id":j.job_id,"query":"follow a science lead","engines":["wikipedia"],"parent_attempt_id":science.attempts[0].attempt_id,"evidence_result_ids":[science.attempts[0].admitted_result_ids[0]],"rationale":"Caller selected the internal science template"});
    // A real default parent retains false: the revoked/absent science grant is
    // not invented. Its genuine exhausted query budget remains the next error.
    assert_eq!(
        error_value(p.preview_followup(tenant("org:test"), &proposal).await),
        json!({"error":{"code":"job_budget_exceeded","message":"job has insufficient remaining budget"}})
    );
    assert_eq!(
        p.get_job(tenant("org:test"), &j.job_id).await.unwrap(),
        before
    );
    assert_eq!(stored_values(&server), bytes);
    assert_eq!(e.calls.load(Ordering::SeqCst), 3);
    search.shutdown().await;
}
#[tokio::test]
async fn invalid_initial_admission_matrix_is_atomic_and_snapshots_stay_complete() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let before = counts(&server);
    for changes in [
        json!({"max_queries":0}),
        json!({"max_attempts":-1}),
        json!({"max_attempts":true}),
        json!({"max_results":false}),
        json!({"max_engine_attempts":0}),
        json!({"initial_plan":[]}),
        json!({"initial_plan":[{"query":"x","engines":[]}]}),
        json!({"initial_plan":[{"query":"x","engines":["fake"],"subquestion_id":"missing"}]}),
        json!({"initial_plan":[{"query":"x","engines":["fake"],"parent_attempt_id":"elsewhere"}]}),
        json!({"subquestions":[{"id":"a","question":"x"},{"id":"a","question":"y"}]}),
    ] {
        let mut args = start_args();
        for (k, v) in changes.as_object().unwrap() {
            args[k.as_str()] = v.clone();
        }
        assert!(
            p.start(tenant("org:test"), &args).await.is_err(),
            "{args:?}"
        );
        assert_eq!(counts(&server), before);
        assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    }
    search.shutdown().await;
}
#[tokio::test]
async fn default_pending_queries_revalidate_research_sensitive_and_engine_authority_before_dispatch()
 {
    for mode in ["research", "sensitive", "inactive"] {
        let server = Server::start();
        let e = engine(false);
        let mut deps = dependencies(vec![("fake", e.clone())]);
        deps.policy.policy.grants.insert("research".into());
        let search = SearchService::new(SearchConfig::default(), deps).unwrap();
        let jobs = ResearchStore::new(Arc::new(server.store()), tenant("poller"));
        let snapshots = p_snapshots(&server);
        let p = adaptive(jobs.clone(), search.clone(), snapshots.clone());
        let j = p
            .start(
                tenant("org:test"),
                &json!({"question":"admitted default","max_queries":1}),
            )
            .await
            .unwrap();
        assert!(!j.queries[0].requires_intent_grant);
        let mut denied = dependencies(vec![("fake", e.clone())]);
        denied.policy.policy.grants.insert("research".into());
        match mode {
            "research" => {
                denied.policy.policy.grants.clear();
            }
            "sensitive" => {
                denied.policy.policy.sensitive_engines.insert("fake".into());
            }
            "inactive" => {
                let mut c = capability("fake");
                c.active = false;
                c.available = false;
                denied.policy.catalog = CatalogSnapshot::new(vec![c]).unwrap();
            }
            _ => unreachable!(),
        };
        let denied_search = SearchService::new(SearchConfig::default(), denied).unwrap();
        let current = adaptive(jobs.clone(), denied_search.clone(), snapshots);
        let (_stop, rx) = shutdown();
        let output = current
            .run(tenant("org:test"), &j.job_id, rx)
            .await
            .unwrap();
        assert_eq!(e.calls.load(Ordering::SeqCst), 0);
        assert_eq!(output.queries[0].attempts.len(), 1);
        assert_eq!(output.queries[0].attempts[0].state, "failed");
        assert_eq!(output.budget_used["attempts"], 1);
        assert_eq!(output.budget_used["results"], 0);
        search.shutdown().await;
        denied_search.shutdown().await;
    }
}
#[tokio::test]
async fn direct_extension_persisted_engine_cap_rejects_without_trimming_and_nullable_ids_remain_legacy()
 {
    let server = Server::start();
    let e = engine(false);
    let mut deps = dependencies(vec![("fake", e.clone()), ("other", e.clone())]);
    deps.policy.policy.grants.insert("research".into());
    let search = SearchService::new(SearchConfig::default(), deps).unwrap();
    let jobs = ResearchStore::new(Arc::new(server.store()), tenant("poller"));
    let snapshots = p_snapshots(&server);
    let p = AdaptiveResearch::new(
        jobs,
        search.clone(),
        snapshots,
        RunnerConfig {
            owner: "slice-before-dedup".into(),
            lease_seconds: NonZeroU64::new(2).unwrap(),
            poll_interval: Duration::from_millis(20),
            workers: NonZeroU64::new(2).unwrap(),
            ceilings: ResearchCeilings {
                queries: 8,
                engines_per_query: 2,
                results: 100,
            },
        },
        500,
    )
    .unwrap();
    let mut args = start_args();
    args["max_engines_per_query"] = json!(1);
    let j = first(&p, args).await;
    let before = j.to_payload().unwrap();
    let (_stop, rx) = shutdown();
    let denied = p
        .extend(
            tenant("org:test"),
            &json!({"job_id":j.job_id,"query":"next","engines":["fake","other"]}),
            rx.clone(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        denied.value(),
        json!({"error":{"code":"job_budget_exceeded","message":"follow-up exceeds per-query engine budget","field":"query"}})
    );
    assert_eq!(
        p.load(tenant("org:test"), &j.job_id)
            .await
            .unwrap()
            .to_payload()
            .unwrap(),
        before
    );
    let legacy=p.extend(tenant("org:test"),&json!({"job_id":j.job_id,"query":"next","engines":["fake","fake","other"],"evidence_result_ids":null}),rx).await.unwrap();
    assert_eq!(legacy.queries.len(), 2);
    assert_eq!(legacy.queries[1].engines, vec!["fake"]);
    assert!(legacy.queries[1].evidence_result_ids.is_empty());
    assert_eq!(e.calls.load(Ordering::SeqCst), 2);
    search.shutdown().await;
}
#[tokio::test]
async fn orphan_cancellation_expiry_and_caller_completion_preserve_attempt_history_and_charges() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    for action in ["cancel", "expire", "complete"] {
        let j = first(&p, start_args()).await;
        let prior = j.queries[0].attempts.clone();
        mutate_job(&server, &j, |v| {
            v["state"] = json!("running");
            let q = &mut v["queries"].as_array_mut().unwrap()[0];
            q["state"] = json!("running");
            q["attempts"].as_array_mut().unwrap().push(
                ssx_json::to_value(ResearchQueryAttempt {
                    attempt_id: "orphan".into(),
                    state: "running".into(),
                    ..Default::default()
                })
                .unwrap(),
            );
            v["budget_used"]["attempts"] = json!(2);
            v["budget_used"]["engine_attempts"] = json!(2);
            if action == "expire" {
                v["deadline"] = json!(now().seconds() - 1.0);
            }
        });
        let current = match action {
            "cancel" => p.cancel(tenant("org:test"), &j.job_id).await.unwrap(),
            "complete" => p
                .update(
                    tenant("org:test"),
                    &json!({"job_id":j.job_id,"subquestion_states":{},"complete":true}),
                )
                .await
                .unwrap(),
            _ => {
                let (_stop, rx) = shutdown();
                p.run(tenant("org:test"), &j.job_id, rx).await.unwrap()
            }
        };
        assert_eq!(
            &current.queries[0].attempts[..prior.len()],
            prior.as_slice()
        );
        let last = current.queries[0].attempts.last().unwrap();
        assert_eq!(last.state, "interrupted");
        assert!(last.finished_at.is_some());
        assert_eq!(current.queries[0].state, "cancelled", "{action}");
        assert_eq!(current.budget_used["attempts"], 2);
        assert_eq!(
            current.stop_reason.as_deref(),
            Some(match action {
                "cancel" => "cancelled",
                "expire" => "deadline_expired",
                _ => "caller_completed",
            })
        );
    }
    assert_eq!(e.calls.load(Ordering::SeqCst), 3);
    search.shutdown().await;
}
#[tokio::test]
async fn nonalphabetical_subquestion_order_survives_valkey_reload_and_caller_resolution() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let mut args = start_args();
    args["subquestions"] = json!([{"id":"z","question":"Last letter first"},{"id":"a","question":"First letter second"}]);
    args["initial_plan"].as_array_mut().unwrap()[0]["subquestion_id"] = json!("z");
    let j = first(&p, args).await;
    assert_eq!(
        job_summary(&j)["unresolved_subquestions"],
        json!(["z", "a"])
    );
    let reloaded = ResearchStore::new(Arc::new(server.store()), tenant("org:test"))
        .load(&JobId::new(&j.job_id).unwrap())
        .await;
    let JobRead::Found(reloaded) = reloaded else {
        panic!("reload failed")
    };
    assert_eq!(
        job_summary(&reloaded)["unresolved_subquestions"],
        json!(["z", "a"])
    );
    let resolved = p
        .update(
            tenant("org:test"),
            &json!({"job_id":j.job_id,"subquestion_states":{"z":"resolved"}}),
        )
        .await
        .unwrap();
    assert_eq!(
        job_summary(&resolved)["unresolved_subquestions"],
        json!(["a"])
    );
    assert_eq!(resolved.queries, j.queries);
    search.shutdown().await;
}
#[tokio::test]
async fn all_execution_budget_caps_stop_dispatch_without_implicit_caller_completion() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    for field in [
        "max_queries",
        "max_results",
        "max_attempts",
        "max_engine_attempts",
    ] {
        let mut args = start_args();
        args[field] = json!(1);
        if field == "max_queries" {
            args["subquestions"] = json!([{"id":"a","question":"Maintained?"}]);
        }
        let j = first(&p, args).await;
        let calls = e.calls.load(Ordering::SeqCst);
        let (_stop, rx) = shutdown();
        assert_eq!(
            p.extend(
                tenant("org:test"),
                &json!({"job_id":j.job_id,"query":"next","engines":["fake"]}),
                rx
            )
            .await
            .unwrap_err()
            .code,
            "job_budget_exceeded"
        );
        assert!(!j.caller_completed);
        assert_eq!(e.calls.load(Ordering::SeqCst), calls);
        let completed = p
            .update(
                tenant("org:test"),
                &json!({"job_id":j.job_id,"subquestion_states":{},"complete":true}),
            )
            .await
            .unwrap();
        assert!(completed.caller_completed);
        let expected = if j
            .stop_reason
            .as_deref()
            .is_some_and(|s| s.ends_with("budget_exhausted"))
        {
            j.stop_reason.clone()
        } else {
            Some("caller_completed".into())
        };
        assert_eq!(completed.stop_reason, expected);
        assert_eq!(completed.queries, j.queries);
        assert_eq!(completed.budget_used, j.budget_used);
    }
    search.shutdown().await;
}
#[tokio::test]
async fn legacy_nullable_planning_collections_and_digest_preserve_source_contract() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let j = first(&p, start_args()).await;
    for null in [false, true] {
        let mut payload = j.to_payload().unwrap();
        let q = &mut payload["queries"].as_array_mut().unwrap()[0];
        for field in [
            "planning_method",
            "evidence_result_ids",
            "evidence_engines",
            "evidence_intent",
        ] {
            if null && ["evidence_result_ids", "evidence_engines"].contains(&field) {
                q[field] = Value::Null;
            } else {
                q.as_object_mut().unwrap().shift_remove(field);
            }
        }
        let restored = ResearchJob::from_payload(payload).unwrap();
        assert!(restored.queries[0].planning_method.is_none());
        assert!(restored.queries[0].evidence_result_ids.is_empty());
        assert!(restored.queries[0].evidence_engines.is_empty());
        search
            .prepare_research_plan(&ScopeProposal::from_research_query(&restored.queries[0]))
            .unwrap();
    }
    search.shutdown().await;
}

#[tokio::test]
async fn lower_retry_attempt_cap_and_current_policy_matrix_preserve_original_attempts() {
    for mode in [
        "attempt_cap",
        "research",
        "sensitive",
        "science",
        "legacy_science",
    ] {
        let server = Server::start();
        let e = engine(false);
        e.count.store(0, Ordering::SeqCst);
        let (_, jobs, search, snapshots) =
            setup_policy(server.store(), e.clone(), &["research", "science"]);
        let p = adaptive(jobs.clone(), search.clone(), snapshots.clone());
        let mut args = start_args();
        if mode == "attempt_cap" {
            args["max_attempts"] = json!(1);
        }
        let j = first(&p, args).await;
        let original = j.queries[0].attempts.clone();
        assert_eq!(original.len(), 1);
        assert_eq!(j.queries[0].result_count, 0);
        assert!(j.queries[0].retryable());
        if ["science", "legacy_science"].contains(&mode) {
            mutate_job(&server, &j, |v| {
                let q = &mut v["queries"].as_array_mut().unwrap()[0];
                q["intent"] = json!("science");
                q["requires_intent_grant"] = json!(true);
                if mode == "legacy_science" {
                    q.as_object_mut()
                        .unwrap()
                        .shift_remove("requires_intent_grant");
                }
            });
        }
        let mut deps = dependencies(vec![("fake", e.clone())]);
        deps.policy.policy.grants.insert("research".into());
        if mode == "research" {
            deps.policy.policy.grants.clear();
        }
        if mode == "sensitive" {
            deps.policy.policy.sensitive_engines.insert("fake".into());
        }
        let current_search = SearchService::new(SearchConfig::default(), deps).unwrap();
        let current = adaptive(jobs.clone(), current_search.clone(), snapshots);
        let (_stop, rx) = shutdown();
        let output = current.retry(tenant("org:test"), &j.job_id, rx).await;
        if mode == "research" {
            assert_eq!(output.unwrap_err().code, "tool_disabled");
        } else {
            assert!(output.is_ok(), "{mode}: {output:?}");
        }
        let latest = current
            .get_job(tenant("org:test"), &j.job_id)
            .await
            .unwrap();
        assert_eq!(
            &latest.queries[0].attempts[..original.len()],
            original.as_slice()
        );
        assert_eq!(e.calls.load(Ordering::SeqCst), 1);
        if mode == "attempt_cap" {
            assert_eq!(
                latest.stop_reason.as_deref(),
                Some("attempt_budget_exhausted")
            );
            assert_eq!(latest.queries[0].attempts, original);
        } else if mode != "research" {
            let last = latest.queries[0].attempts.last().unwrap();
            assert_eq!(last.state, "failed");
            assert!(last.cursor.is_none());
            assert_eq!(latest.queries[0].attempts.len(), 2);
        } else {
            assert_eq!(latest.queries[0].attempts, original);
        }
        assert_eq!(
            ResearchJob::from_payload(latest.to_payload().unwrap()).unwrap(),
            latest
        );
        search.shutdown().await;
        current_search.shutdown().await;
    }
}

#[tokio::test]
async fn unavailable_authoritative_store_refuses_start_without_dispatch_or_ephemeral_success() {
    let mut server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    server.child.kill().unwrap();
    server.child.wait().unwrap();
    assert_eq!(
        p.start(tenant("org:test"), &start_args())
            .await
            .unwrap_err()
            .code,
        "store_unavailable"
    );
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    search.shutdown().await;
}

#[tokio::test]
async fn deadline_cancellation_and_completion_during_evidence_read_never_append_or_spend() {
    for mode in ["deadline", "cancel", "complete"] {
        let server = Server::start();
        let e = engine(false);
        let (_, jobs, search, snapshots) = setup(&server, e.clone());
        let p = adaptive(jobs.clone(), search.clone(), snapshots);
        let j = first(&p, start_args()).await;
        let preview = p
            .preview_followup(tenant("org:test"), &followup(&j))
            .await
            .unwrap();
        let held = Arc::new(HeldSnapshotRead {
            inner: ValkeySnapshots::new(Arc::new(server.store())),
            entered: Notify::new(),
            release: Notify::new(),
        });
        let delayed = adaptive(
            jobs.clone(),
            search.clone(),
            SnapshotService::new(
                held.clone(),
                Arc::new(SystemSnapshotClock),
                Arc::new(RandomSnapshotIds),
                tenant("poller"),
                SnapshotTtl::new(60).unwrap(),
            ),
        );
        let args = preview["execution"]["arguments"].clone();
        let task = tokio::spawn(async move {
            let (_stop, rx) = shutdown();
            delayed.extend(tenant("org:test"), &args, rx).await
        });
        tokio::time::timeout(Duration::from_secs(5), held.entered.notified())
            .await
            .unwrap();
        match mode {
            "deadline" => mutate_job(&server, &j, |v| {
                v["deadline"] = json!(now().seconds() - 1.0)
            }),
            "cancel" => {
                // Finished jobs ignore cancellation. Model a newly observed
                // orphan running transition, then use the actual cancel API.
                mutate_job(&server, &j, |v| v["state"] = json!("running"));
                p.cancel(tenant("org:test"), &j.job_id).await.unwrap();
            }
            _ => {
                p.update(
                    tenant("org:test"),
                    &json!({"job_id":j.job_id,"subquestion_states":{},"complete":true}),
                )
                .await
                .unwrap();
            }
        }
        held.release.notify_one();
        let outcome = task.await.unwrap();
        if mode == "complete" {
            assert_eq!(
                error_value(outcome),
                json!({"error":{"code":"invalid_job_state","message":"caller already completed this job","field":"query"}})
            );
        } else {
            assert_eq!(
                outcome.unwrap().state,
                if mode == "deadline" {
                    "expired"
                } else {
                    "cancelled"
                }
            );
        }
        let current = p.get_job(tenant("org:test"), &j.job_id).await.unwrap();
        assert_eq!(current.queries, j.queries);
        assert_eq!(current.budget_used, j.budget_used);
        assert_eq!(e.calls.load(Ordering::SeqCst), 1);
        search.shutdown().await;
    }
}

#[test]
fn naive_deadline_gap_and_fold_match_actual_pinned_stdlib_primitives() {
    if capture_timezone_child("naive_deadline_gap_and_fold_match_actual_pinned_stdlib_primitives") {
        return;
    }
    let source: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w03/deadline-local-stdlib-observation-001.json"
    ))
    .unwrap();
    assert_eq!(source["timezone"], json!("America/New_York"));
    assert_eq!(source["backend_modules"], json!([]));
    assert_eq!(source["forbidden_effect_attempts"], json!([]));
    for row in source["observations"].as_array().unwrap() {
        let expected = row["timestamp"].as_f64().unwrap();
        assert_eq!(
            parse_iso_deadline(row["input"].as_str().unwrap()),
            Some(expected),
            "{row:?}"
        );
        let now = 1792238400.0;
        assert_eq!(
            resolve_deadline_at(Some(&row["input"]), now, 600).unwrap(),
            expected.clamp(now + 60.0, now + 86400.0)
        );
    }
}

// Full bodies below are source-derived expectations from the pinned branches,
// not additional original-runtime observations. The service/Valkey execution is real.
fn error_value<T>(result: std::result::Result<T, PlanningError>) -> Value {
    match result {
        Err(error) => error.value(),
        Ok(_) => panic!("expected business refusal"),
    }
}
#[tokio::test]
async fn source_business_input_envelopes_and_precedence_are_exact_and_atomic() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let before = counts(&server);
    for (args, expected) in [
        (
            json!({"question":"  "}),
            json!({"error":{"code":"invalid_input","message":"question is required","field":"question"}}),
        ),
        (
            json!({"question":"x".repeat(501)}),
            json!({"error":{"code":"invalid_input","message":"question exceeds the maximum length of 500 characters","field":"question"}}),
        ),
        (
            json!({"question":"q","strategy":"invented"}),
            json!({"error":{"code":"invalid_input","message":"unknown strategy 'invented'","field":"strategy","valid_alternatives":["triangulate","broad","fresh","counterevidence"]}}),
        ),
        (
            json!({"question":" ","strategy":"invented"}),
            json!({"error":{"code":"invalid_input","message":"question is required","field":"question"}}),
        ),
    ] {
        assert_eq!(
            error_value(p.start(tenant("org:test"), &args).await),
            expected,
            "{args:?}"
        );
    }
    for (args, expected, variants) in [
        (
            json!({"question":" ","subquestions":[]}),
            "subquestions must leave a query slot for the original",
            false,
        ),
        (
            json!({"question":" ","variants":[]}),
            "variants must leave a query slot for the original",
            true,
        ),
        (
            json!({"question":" ","subquestions":[{"id":"a"}]}),
            "subquestion ids must be nonempty and unique",
            false,
        ),
        (
            json!({"question":" ","subquestions":[{"id":"a","question":"x"}]}),
            "question must be a nonempty bounded string",
            false,
        ),
    ] {
        assert_eq!(
            error_value(p.preview_initial(&args, variants).await),
            json!({"error":{"code":"invalid_input","message":expected}})
        );
    }
    for (entry, expected) in [
        (
            json!({"query":"q","intent":"unknown"}),
            json!({"error":{"code":"invalid_input","message":"unknown research intent"}}),
        ),
        (
            json!({"query":"q","intent":"images"}),
            json!({"error":{"code":"invalid_intent","message":"research subqueries do not support media searches"}}),
        ),
        (
            json!({"query":"q","intent":"science","engines":["fake"]}),
            json!({"error":{"code":"tool_disabled","message":"intent science requires the science grant"}}),
        ),
        (
            json!({"query":"q","engines":[]}),
            json!({"error":{"code":"invalid_input","message":"engines must be a nonempty list of names"}}),
        ),
        (
            json!({"query":"q","engines":["missing"]}),
            json!({"error":{"code":"invalid_scope","message":"unknown or inactive engines: missing (unknown)"}}),
        ),
        (
            json!({"query":"q","engines":["fake"],"evidence_result_ids":null}),
            json!({"error":{"code":"invalid_input","message":"evidence_result_ids must be a list of at most 10 result IDs"}}),
        ),
        (
            json!({"query":"q","engines":["fake"],"evidence_result_ids":[7]}),
            json!({"error":{"code":"invalid_input","message":"evidence_result_ids must contain nonempty bounded strings"}}),
        ),
    ] {
        assert_eq!(
            error_value(p.prepare_query(&entry, 4).await),
            expected,
            "{entry:?}"
        );
    }
    for (changes, expected) in [
        (
            json!({"max_attempts":1,"initial_plan":[{"query":"one","engines":["fake"]},{"query":"two","engines":["fake"]}]}),
            json!({"error":{"code":"job_budget_exceeded","message":"initial plan exceeds execution budget"}}),
        ),
        (
            json!({"max_attempts":1,"initial_plan":[{"query":"one","engines":["fake"]},{"query":"","engines":["fake"]}]}),
            json!({"error":{"code":"invalid_input","message":"query must be a nonempty bounded string"}}),
        ),
    ] {
        let mut args = start_args();
        for (k, v) in changes.as_object().unwrap() {
            args[k.as_str()] = v.clone();
        }
        assert_eq!(
            error_value(p.start(tenant("org:test"), &args).await),
            expected
        );
    }
    assert_eq!(counts(&server), before);
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    search.shutdown().await;
}
#[tokio::test]
async fn source_missing_job_and_method_grant_error_envelopes_preserve_fields() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs.clone(), search.clone(), snapshots.clone());
    let expected =
        json!({"error":{"code":"invalid_job_id","message":"unknown job id","field":"job_id"}});
    let (_stop, rx) = shutdown();
    assert_eq!(
        error_value(p.get_job(tenant("org:test"), "missing").await),
        expected
    );
    assert_eq!(
        error_value(p.cancel(tenant("org:test"), "missing").await),
        expected
    );
    assert_eq!(
        error_value(
            p.retry_public(tenant("org:test"), "missing", rx.clone())
                .await
        ),
        expected
    );
    assert_eq!(
        error_value(
            p.update(
                tenant("org:test"),
                &json!({"job_id":"missing","subquestion_states":{}})
            )
            .await
        ),
        expected
    );
    assert_eq!(
        error_value(
            p.extend(
                tenant("org:test"),
                &json!({"job_id":"missing","query":"next"}),
                rx.clone()
            )
            .await
        ),
        expected
    );
    assert_eq!(
        error_value(
            p.preview_followup(
                tenant("org:test"),
                &json!({"job_id":"missing","query":"next"})
            )
            .await
        ),
        json!({"error":{"code":"invalid_job_id","message":"unknown job id"}})
    );
    let (_, _, denied_search, _) = setup_policy(server.store(), e.clone(), &[]);
    let denied = adaptive(jobs, denied_search.clone(), snapshots);
    let before = counts(&server);
    for (operation, actual) in [
        (
            "slopsearx_start_research",
            error_value(
                denied
                    .start(tenant("org:test"), &json!({"question":"q"}))
                    .await,
            ),
        ),
        (
            "slopsearx_retry_research",
            error_value(
                denied
                    .retry_public(tenant("org:test"), "missing", rx.clone())
                    .await,
            ),
        ),
        (
            "slopsearx_extend_research",
            error_value(
                denied
                    .extend(
                        tenant("org:test"),
                        &json!({"job_id":"missing","query":"next"}),
                        rx,
                    )
                    .await,
            ),
        ),
    ] {
        assert_eq!(
            actual,
            json!({"error":{"code":"tool_disabled","message":format!("{operation} requires the research grant (MCP_GRANT_RESEARCH=1)")}})
        );
    }
    assert_eq!(
        error_value(
            denied
                .update(
                    tenant("org:test"),
                    &json!({"job_id":"missing","subquestion_states":{}})
                )
                .await
        ),
        json!({"error":{"code":"tool_disabled","message":"research grant is required"}})
    );
    for variants in [false, true] {
        assert_eq!(
            error_value(denied.preview_initial(&json!({}), variants).await),
            json!({"error":{"code":"tool_disabled","message":"query planning requires MCP_GRANT_RESEARCH=1"}})
        );
    }
    assert_eq!(
        error_value(
            denied
                .preview_followup(tenant("org:test"), &json!({"job_id":"missing"}))
                .await
        ),
        json!({"error":{"code":"tool_disabled","message":"query planning requires MCP_GRANT_RESEARCH=1"}})
    );
    assert_eq!(counts(&server), before);
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    search.shutdown().await;
    denied_search.shutdown().await;
}
#[tokio::test]
async fn source_direct_extension_errors_keep_policy_details_and_terminal_context() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let job = first(&p, start_args()).await;
    let before = job.to_payload().unwrap();
    let calls = e.calls.load(Ordering::SeqCst);
    let (_stop, rx) = shutdown();
    for (extra, expected) in [
        (
            json!({"query":" "}),
            json!({"error":{"code":"invalid_input","message":"query is required","field":"query"}}),
        ),
        (
            json!({"query":"x".repeat(501)}),
            json!({"error":{"code":"invalid_input","message":"query exceeds the maximum length of 500 characters","field":"query","max_length":500}}),
        ),
        (
            json!({"intent":"unknown"}),
            json!({"error":{"code":"invalid_input","message":"unknown intent 'unknown'","field":"intent","valid_alternatives":["code","finance","geography","historical","images","jobs","legal","media","medical","news","packages","reference","science","security","social","videos","web"]}}),
        ),
        (
            json!({"intent":"science"}),
            json!({"error":{"code":"tool_disabled","message":"intent 'science' requires the science grant (MCP_GRANT_SCIENCE=1)","field":"intent","grant":"MCP_GRANT_SCIENCE"}}),
        ),
        (
            json!({"intent":"images"}),
            json!({"error":{"code":"invalid_intent","message":"intent 'images' selects a media type; research subqueries do not support media searches","field":"intent"}}),
        ),
        (
            json!({"engines":[]}),
            json!({"error":{"code":"invalid_input","message":"engines must be a nonempty list when supplied","field":"engines"}}),
        ),
        (
            json!({"engines":["missing"]}),
            json!({"error":{"code":"invalid_scope","message":"unknown or inactive engines: missing (unknown)","field":"engines","valid_alternatives":["fake"]}}),
        ),
    ] {
        let mut args = json!({"job_id":job.job_id,"query":"next","engines":["fake"]});
        for (k, v) in extra.as_object().unwrap() {
            args[k.as_str()] = v.clone();
        }
        assert_eq!(
            error_value(p.extend(tenant("org:test"), &args, rx.clone()).await),
            expected,
            "{args:?}"
        );
        assert_eq!(
            p.get_job(tenant("org:test"), &job.job_id)
                .await
                .unwrap()
                .to_payload()
                .unwrap(),
            before
        );
    }
    for (extra, message) in [
        (
            json!({"subquestion_id":"missing"}),
            "unknown subquestion_id",
        ),
        (
            json!({"parent_attempt_id":"missing"}),
            "parent_attempt_id must identify a terminal attempt in this job",
        ),
        (
            json!({"query":"maintenance","planning_method":"original"}),
            "query already searched or planned in this scope",
        ),
    ] {
        let mut args = json!({"job_id":job.job_id,"query":"next","engines":["fake"]});
        for (k, v) in extra.as_object().unwrap() {
            args[k.as_str()] = v.clone();
        }
        let code = if message.starts_with("query already") {
            "duplicate_query"
        } else {
            "invalid_input"
        };
        assert_eq!(
            error_value(p.extend(tenant("org:test"), &args, rx.clone()).await),
            json!({"error":{"code":code,"message":message,"field":"query"}})
        );
    }
    let invalid_order = json!({"job_id":job.job_id,"query":"next","engines":["fake"],"continuation_key":" ","planning_method":"invalid","evidence_result_ids":null});
    assert_eq!(
        error_value(
            p.extend(tenant("org:test"), &invalid_order, rx.clone())
                .await
        ),
        json!({"error":{"code":"invalid_input","message":"continuation_key must be a nonempty string of at most 128 characters"}})
    );
    assert_eq!(e.calls.load(Ordering::SeqCst), calls);
    mutate_job(&server, &job, |v| {
        v["state"] = json!("cancelled");
    });
    assert_eq!(
        error_value(
            p.extend(
                tenant("org:test"),
                &json!({"job_id":job.job_id,"query":"next","engines":["fake"]}),
                rx.clone()
            )
            .await
        ),
        json!({"error":{"code":"invalid_job_state","message":"job is in terminal state 'cancelled'; cannot extend","field":"query","job_id":job.job_id,"state":"cancelled"}})
    );
    mutate_job(&server, &job, |v| {
        v["deadline"] = json!(now().seconds() - 1.0);
    });
    assert_eq!(
        error_value(
            p.extend(
                tenant("org:test"),
                &json!({"job_id":job.job_id,"query":"next","engines":["fake"]}),
                rx
            )
            .await
        ),
        json!({"error":{"code":"deadline_exceeded","message":"job deadline has passed; cannot extend","field":"query"}})
    );
    search.shutdown().await;
}
#[tokio::test]
async fn unrelated_retained_intent_does_not_replace_original_parent_authority_or_flags() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) =
        setup_policy(server.store(), e.clone(), &["research", "science"]);
    let p = adaptive(jobs.clone(), search.clone(), snapshots.clone());
    let job=first(&p,json!({"question":"two independent source scopes","max_queries":4,"max_attempts":4,"max_engine_attempts":4,"initial_plan":[{"query":"web parent","engines":["fake"]},{"query":"science parent","intent":"science","engines":["fake"]}]})).await;
    let (_, _, denied_search, _) = setup_policy(server.store(), e.clone(), &["research"]);
    let current = adaptive(jobs, denied_search.clone(), snapshots);
    let before = job.to_payload().unwrap();
    let calls = e.calls.load(Ordering::SeqCst);
    let arguments = |index: usize| json!({"job_id":job.job_id,"query":"a new evidence question","engines":["fake"],"parent_attempt_id":job.queries[index].attempts[0].attempt_id,"evidence_result_ids":[job.queries[index].attempts[0].admitted_result_ids[0]],"rationale":"Caller selected this original parent"});
    let web = current
        .preview_followup(tenant("org:test"), &arguments(0))
        .await
        .unwrap();
    assert_eq!(web["dispatched"], json!(false));
    assert_eq!(
        error_value(
            current
                .preview_followup(tenant("org:test"), &arguments(1))
                .await
        ),
        json!({"error":{"code":"policy_rejected","message":"science requires the science grant"}})
    );
    let retained = p.get_job(tenant("org:test"), &job.job_id).await.unwrap();
    assert_eq!(retained.to_payload().unwrap(), before);
    assert!(retained.queries.iter().all(|q| q.requires_intent_grant));
    assert_eq!(e.calls.load(Ordering::SeqCst), calls);
    search.shutdown().await;
    denied_search.shutdown().await;
}
#[tokio::test]
async fn retained_dossier_budget_errors_and_tenant_bound_grantless_cancel_match_source() {
    let server = Server::start();
    let e = engine(false);
    e.count.store(0, Ordering::SeqCst);
    let (_, jobs, search, snapshots) = setup_policy(
        server.store(),
        e.clone(),
        &["research", "dependency_dossier", "security"],
    );
    let p = adaptive(jobs.clone(), search.clone(), snapshots.clone());
    let job = first(&p, start_args()).await;
    let calls = e.calls.load(Ordering::SeqCst);
    let (_stop, rx) = shutdown();
    for (budget, message) in [
        (
            json!({"used_adapter_calls":1,"max_adapter_calls":1,"captured_results":0,"max_results":0}),
            "dependency dossier adapter call budget is exhausted",
        ),
        (
            json!({"used_adapter_calls":0,"captured_results":0,"max_results":0}),
            "dependency dossier result budget is exhausted",
        ),
        (
            json!({"used_adapter_calls":" 1 ","max_adapter_calls":1,"captured_results":0,"max_results":10}),
            "dependency dossier adapter call budget is exhausted",
        ),
        (
            json!({"used_adapter_calls":"+١_٠","max_adapter_calls":10,"captured_results":0,"max_results":10}),
            "dependency dossier adapter call budget is exhausted",
        ),
        (
            json!({"used_adapter_calls":"9".repeat(160),"max_adapter_calls":"8".repeat(160),"captured_results":0,"max_results":10}),
            "dependency dossier adapter call budget is exhausted",
        ),
        (
            json!({"used_adapter_calls":1.9,"max_adapter_calls":1,"captured_results":0,"max_results":10}),
            "dependency dossier adapter call budget is exhausted",
        ),
    ] {
        mutate_job(&server, &job, |v| {
            v["workflow"] = json!({"kind":"dependency_dossier","budget":budget})
        });
        let before = p.get_job(tenant("org:test"), &job.job_id).await.unwrap();
        assert_eq!(
            error_value(
                p.retry_public(tenant("org:test"), &job.job_id, rx.clone())
                    .await
            ),
            json!({"error":{"code":"budget_exceeded","message":message}})
        );
        assert_eq!(
            p.get_job(tenant("org:test"), &job.job_id).await.unwrap(),
            before
        );
    }
    for malformed in [
        json!({"used_adapter_calls":null}),
        json!([1]),
        json!({"used_adapter_calls":"not-an-integer"}),
    ] {
        mutate_job(&server, &job, |v| {
            v["workflow"] = json!({"kind":"dependency_dossier","budget":malformed})
        });
        let before = p.get_job(tenant("org:test"), &job.job_id).await.unwrap();
        assert_eq!(
            error_value(
                p.retry_public(tenant("org:test"), &job.job_id, rx.clone())
                    .await
            ),
            json!({"error":{"code":"invalid_input","message":"invalid dependency dossier budget"}})
        );
        assert_eq!(
            p.get_job(tenant("org:test"), &job.job_id).await.unwrap(),
            before
        );
    }
    mutate_job(&server, &job, |v| {
        v["state"] = json!("queued");
        v["queries"].as_array_mut().unwrap()[0]["state"] = json!("pending");
    });
    let (_, _, denied_search, _) = setup_policy(server.store(), e.clone(), &[]);
    let denied = adaptive(jobs.clone(), denied_search.clone(), snapshots);
    assert_eq!(
        error_value(denied.get_job(tenant("org:test"), &job.job_id).await),
        json!({"error":{"code":"tool_disabled","message":"dependency dossier requires MCP_GRANT_DEPENDENCY_DOSSIER","grant":"MCP_GRANT_DEPENDENCY_DOSSIER"}})
    );
    assert_eq!(
        error_value(denied.cancel(tenant("other"), &job.job_id).await),
        json!({"error":{"code":"invalid_job_id","message":"unknown job id","field":"job_id"}})
    );
    let cancelled = denied
        .cancel(tenant("org:test"), &job.job_id)
        .await
        .unwrap();
    assert_eq!(cancelled.state, "cancelled");
    assert_eq!(cancelled.queries[0].attempts, job.queries[0].attempts);
    assert_eq!(e.calls.load(Ordering::SeqCst), calls);
    search.shutdown().await;
    denied_search.shutdown().await;
}
#[tokio::test]
async fn exact_snapshot_error_bodies_and_preexecution_budget_refusals_preserve_history() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let (_stop, rx) = shutdown();
    for mode in [
        "missing",
        "index",
        "expired",
        "empty_sources",
        "foreign_source",
        "malformed",
        "negative",
    ] {
        let job = first(&p, start_args()).await;
        let mut args = followup(&job);
        let mut expires = 0.0;
        match mode {
            "missing" => {
                let key = ssx_infra::valkey::keys::snapshot(
                    &tenant(&job.tenant),
                    job.queries[0].cursor.as_ref().unwrap(),
                );
                redis::cmd("DEL")
                    .arg(key)
                    .query::<u64>(&mut server.connection())
                    .unwrap();
            }
            "expired" => {
                expires = now().seconds() - 1.0;
                mutate_snapshot(&server, &job, |v| {
                    v["created_at"] = json!(expires - 1.0);
                    v["expires_at"] = json!(expires);
                });
            }
            "index" | "malformed" | "negative" => {
                let id = match mode {
                    "index" => format!("{}:99", job.queries[0].cursor.as_ref().unwrap()),
                    "negative" => format!("{}:-1", job.queries[0].cursor.as_ref().unwrap()),
                    _ => "not-a-result-id".into(),
                };
                mutate_job(&server, &job, |v| {
                    v["queries"].as_array_mut().unwrap()[0]["attempts"]
                        .as_array_mut()
                        .unwrap()[0]["admitted_result_ids"] = json!([id])
                });
                args["evidence_result_ids"] = json!([id]);
            }
            "empty_sources" | "foreign_source" => mutate_snapshot(&server, &job, |v| {
                v["results"].as_array_mut().unwrap()[0]["engine"] =
                    json!(if mode == "empty_sources" {
                        ""
                    } else {
                        "foreign"
                    });
                v["results"].as_array_mut().unwrap()[0]["engines"] = json!([]);
            }),
            _ => unreachable!(),
        }
        let expected = match mode {
            "missing" => {
                json!({"error":{"code":"invalid_cursor","message":"unknown or expired snapshot","field":"result_id"}})
            }
            "index" | "negative" => {
                json!({"error":{"code":"invalid_result_id","message":"result index out of range","field":"result_id"}})
            }
            "malformed" => {
                json!({"error":{"code":"invalid_result_id","message":"result_id must be a server-issued snapshot result ID","field":"result_id"}})
            }
            "expired" => {
                json!({"error":{"code":"expired_handle","message":"snapshot has expired","field":"result_id","handle":args["evidence_result_ids"][0],"expires_at":iso_timestamp(expires)}})
            }
            "empty_sources" => {
                json!({"error":{"code":"invalid_scope","message":"unknown or inactive engines:  (unknown)","field":"engines","valid_alternatives":["fake"]}})
            }
            _ => {
                json!({"error":{"code":"invalid_scope","message":"unknown or inactive engines: foreign (unknown)","field":"engines","valid_alternatives":["fake"]}})
            }
        };
        let before = p.get_job(tenant("org:test"), &job.job_id).await.unwrap();
        let calls = e.calls.load(Ordering::SeqCst);
        assert_eq!(
            error_value(p.preview_followup(tenant("org:test"), &args).await),
            expected,
            "{mode}"
        );
        assert_eq!(
            error_value(p.extend(tenant("org:test"), &args, rx.clone()).await),
            expected,
            "{mode}"
        );
        assert_eq!(
            p.get_job(tenant("org:test"), &job.job_id).await.unwrap(),
            before
        );
        assert_eq!(e.calls.load(Ordering::SeqCst), calls);
    }
    for key in ["queries", "attempts", "engine_attempts", "results"] {
        let job = first(&p, start_args()).await;
        mutate_job(&server, &job, |v| {
            let used = if key == "queries" {
                v["queries"].as_array().unwrap().len() as u64
            } else {
                v["budget_used"][key].as_u64().unwrap()
            };
            v["budget_limits"][key] = json!(used);
        });
        let before = p.get_job(tenant("org:test"), &job.job_id).await.unwrap();
        let calls = e.calls.load(Ordering::SeqCst);
        assert_eq!(
            error_value(
                p.extend(
                    tenant("org:test"),
                    &json!({"job_id":job.job_id,"query":"another","engines":["fake"]}),
                    rx.clone()
                )
                .await
            ),
            json!({"error":{"code":"job_budget_exceeded","message":format!("job {} budget is exhausted",if key=="queries"{"query"}else{key}),"field":"query"}}),
            "{key}"
        );
        assert_eq!(
            p.get_job(tenant("org:test"), &job.job_id).await.unwrap(),
            before
        );
        assert_eq!(e.calls.load(Ordering::SeqCst), calls);
    }
    search.shutdown().await;
}
#[tokio::test]
async fn unavailable_store_errors_keep_each_method_source_envelope() {
    let mut server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    server.child.kill().unwrap();
    server.child.wait().unwrap();
    let (_stop, rx) = shutdown();
    let public = json!({"error":{"code":"store_unavailable","message":"job store is unavailable; research jobs are not persisted","field":"job_id"}});
    assert_eq!(
        error_value(p.get_job(tenant("org:test"), "job").await),
        public
    );
    assert_eq!(
        error_value(p.cancel(tenant("org:test"), "job").await),
        public
    );
    assert_eq!(
        error_value(p.retry_public(tenant("org:test"), "job", rx.clone()).await),
        public
    );
    assert_eq!(
        error_value(
            p.extend(tenant("org:test"), &json!({"job_id":"job","query":"q"}), rx)
                .await
        ),
        public
    );
    assert_eq!(
        error_value(
            p.preview_followup(tenant("org:test"), &json!({"job_id":"job"}))
                .await
        ),
        json!({"error":{"code":"store_unavailable","message":"research job store is unavailable"}})
    );
    assert_eq!(
        error_value(
            p.update(
                tenant("org:test"),
                &json!({"job_id":"job","subquestion_states":{}})
            )
            .await
        ),
        json!({"error":{"code":"store_unavailable","message":"job store is unavailable"}})
    );
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    search.shutdown().await;
}
#[tokio::test]
async fn sensitive_scope_error_details_and_known_foreign_evidence_follow_source_order() {
    let server = Server::start();
    let e = engine(false);
    let mut deps = dependencies(vec![("fake", e.clone()), ("other", e.clone())]);
    deps.policy.policy.grants.insert("research".into());
    let search = SearchService::new(SearchConfig::default(), deps).unwrap();
    let jobs = ResearchStore::new(Arc::new(server.store()), tenant("poller"));
    let snapshots = p_snapshots(&server);
    let p = adaptive(jobs.clone(), search.clone(), snapshots.clone());
    let job = first(&p, start_args()).await;
    let before = job.to_payload().unwrap();
    let calls = e.calls.load(Ordering::SeqCst);
    let mut denied_deps = dependencies(vec![("fake", e.clone()), ("other", e.clone())]);
    denied_deps.policy.policy.grants.insert("research".into());
    denied_deps
        .policy
        .policy
        .sensitive_engines
        .insert("fake".into());
    let denied_search = SearchService::new(SearchConfig::default(), denied_deps).unwrap();
    let denied = adaptive(jobs, denied_search.clone(), snapshots);
    let (_stop, rx) = shutdown();
    let full = json!({"error":{"code":"tool_disabled","message":"sensitive engines are unreachable without the sensitive-engine grant (MCP_TARGETED_SENSITIVE_ALLOWED=1): fake","field":"engines","engines":["fake"],"grant":"MCP_TARGETED_SENSITIVE_ALLOWED"}});
    assert_eq!(
        error_value(
            denied
                .extend(
                    tenant("org:test"),
                    &json!({"job_id":job.job_id,"query":"next","engines":["fake"]}),
                    rx
                )
                .await
        ),
        full
    );
    assert_eq!(
        error_value(
            denied
                .preview_followup(tenant("org:test"), &followup(&job))
                .await
        ),
        full
    );
    assert_eq!(
        error_value(
            denied
                .prepare_query(&json!({"query":"next","engines":["fake"]}), 4)
                .await
        ),
        json!({"error":{"code":"tool_disabled","message":"sensitive engines are unreachable without the sensitive-engine grant (MCP_TARGETED_SENSITIVE_ALLOWED=1): fake"}})
    );
    mutate_snapshot(&server, &job, |v| {
        let r = &mut v["results"].as_array_mut().unwrap()[0];
        r["engine"] = json!("other");
        r["engines"] = json!(["other"]);
    });
    assert_eq!(
        error_value(
            p.preview_followup(tenant("org:test"), &followup(&job))
                .await
        ),
        json!({"error":{"code":"invalid_result_id","message":"captured evidence does not match its parent source scope"}})
    );
    assert_eq!(
        p.get_job(tenant("org:test"), &job.job_id)
            .await
            .unwrap()
            .to_payload()
            .unwrap(),
        before
    );
    assert_eq!(e.calls.load(Ordering::SeqCst), calls);
    search.shutdown().await;
    denied_search.shutdown().await;
}
#[tokio::test]
async fn progress_completion_error_bodies_retain_source_precedence_and_busy_state() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs.clone(), search.clone(), snapshots);
    let job = first(&p, start_args()).await;
    let calls = e.calls.load(Ordering::SeqCst);
    for (args, message) in [
        (
            json!({"job_id":job.job_id,"subquestion_states":{"a":"invented"}}),
            "subquestion states must be resolved or unresolved",
        ),
        (
            json!({"job_id":job.job_id,"subquestion_states":{},"complete":null}),
            "complete must be a boolean",
        ),
        (
            json!({"job_id":job.job_id,"subquestion_states":{"missing":"resolved"}}),
            "unknown subquestion id",
        ),
    ] {
        assert_eq!(
            error_value(p.update(tenant("org:test"), &args).await),
            json!({"error":{"code":"invalid_input","message":message}})
        );
    }
    let request = json!({"job_id":job.job_id,"subquestion_states":{},"complete":true,"rationale":"caller stop"});
    let done = p.update(tenant("org:test"), &request).await.unwrap();
    let changed = json!({"job_id":job.job_id,"subquestion_states":{},"complete":true,"rationale":"different stop"});
    assert_eq!(
        error_value(p.update(tenant("org:test"), &changed).await),
        json!({"error":{"code":"idempotency_conflict","message":"completion request conflicts with the persisted completion"}})
    );
    assert_eq!(error_value(p.update(tenant("org:test"),&json!({"job_id":job.job_id,"subquestion_states":{"missing":"resolved"},"complete":false})).await),json!({"error":{"code":"invalid_job_state","message":"job is already terminal"}}));
    assert_eq!(error_value(p.update(tenant("org:test"),&json!({"job_id":job.job_id,"subquestion_states":{"missing":"resolved"},"complete":true})).await),json!({"error":{"code":"invalid_input","message":"unknown subquestion id"}}));
    assert_eq!(
        p.get_job(tenant("org:test"), &job.job_id).await.unwrap(),
        done
    );
    struct HeldTokens;
    impl TokenSource<JobId> for HeldTokens {
        fn next_token(&mut self) -> LeaseToken<JobId> {
            LeaseToken::new("held-error-body-token").unwrap()
        }
    }
    let live = first(&p, start_args()).await;
    let claim = jobs
        .for_tenant(tenant("org:test"))
        .claim_prepared(
            &JobId::new(&live.job_id).unwrap(),
            "held",
            NonZeroU64::new(2).unwrap(),
            &mut HeldTokens,
            now(),
            || AttemptId::new("held-error-body-attempt").unwrap(),
            false,
            |_| Ok(()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        error_value(
            p.update(
                tenant("org:test"),
                &json!({"job_id":live.job_id,"subquestion_states":{"a":"resolved"}})
            )
            .await
        ),
        json!({"error":{"code":"job_busy","message":"job is owned by a live worker; retry the progress update later"}})
    );
    jobs.for_tenant(tenant("org:test"))
        .release(&JobId::new(&live.job_id).unwrap(), &claim.token)
        .await
        .unwrap();
    assert_eq!(e.calls.load(Ordering::SeqCst), calls + 1);
    search.shutdown().await;
}

#[tokio::test]
async fn start_idempotency_keys_preserve_source_opaque_empty_whitespace_and_long_values() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let mut ids = std::collections::BTreeSet::new();
    for key in ["", " ", "key", " key ", &"long".repeat(100)] {
        let mut args = start_args();
        args["idempotency_key"] = json!(key);
        let first = p.start(tenant("org:test"), &args).await.unwrap();
        assert_eq!(first.idempotency_key.as_deref(), Some(key));
        assert!(ids.insert(first.job_id.clone()));
        args["max_queries"] = json!(0);
        let replay = p.start(tenant("org:test"), &args).await;
        if key.is_empty() {
            assert_eq!(
                error_value(replay),
                json!({"error":{"code":"invalid_input","message":"max_queries must be a positive integer"}})
            );
        } else {
            assert_eq!(replay.unwrap(), first);
        }
    }
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    search.shutdown().await;
}

fn stored_values(server: &Server) -> BTreeMap<String, Vec<u8>> {
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("*")
        .query(&mut server.connection())
        .unwrap();
    keys.into_iter()
        .map(|key| {
            let value = redis::cmd("DUMP")
                .arg(&key)
                .query(&mut server.connection())
                .unwrap();
            (key, value)
        })
        .collect()
}
async fn science_parent(
    server: &Server,
    e: Arc<FixtureEngine>,
) -> (
    AdaptiveResearch,
    ResearchStore,
    SearchService,
    SnapshotService,
    ResearchJob,
) {
    let (_, jobs, search, snapshots) = setup_policy(server.store(), e, &["research", "science"]);
    let planning = adaptive(jobs.clone(), search.clone(), snapshots.clone());
    let mut args = start_args();
    args["question"] = json!("CVE-2026-12345 evidence");
    args["initial_plan"].as_array_mut().unwrap()[0]["intent"] = json!("science");
    let job = first(&planning, args).await;
    (planning, jobs, search, snapshots, job)
}
#[tokio::test]
async fn bare_parent_without_evidence_does_not_inherit_revoked_parent_intent_authority() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots, job) = science_parent(&server, e.clone()).await;
    let (_, _, current_search, _) = setup_policy(server.store(), e.clone(), &["research"]);
    let current = adaptive(jobs, current_search.clone(), snapshots);
    let (_stop, rx) = shutdown();
    let args = json!({"job_id":job.job_id,"query":"bare web continuation","engines":["fake"],"parent_attempt_id":job.queries[0].attempts[0].attempt_id,"continuation_key":"bare-parent"});
    let accepted = current.extend(tenant("org:test"), &args, rx).await.unwrap();
    assert_eq!(accepted.queries.len(), 2);
    assert_eq!(accepted.queries[0], job.queries[0]);
    let added = &accepted.queries[1];
    assert_eq!(
        added.parent_attempt_id,
        args["parent_attempt_id"].as_str().map(str::to_owned)
    );
    assert!(added.evidence_result_ids.is_empty());
    assert!(added.evidence_engines.is_empty());
    assert!(added.evidence_intent.is_none());
    assert_eq!(
        accepted.budget_used["attempts"],
        job.budget_used["attempts"] + 1
    );
    assert_eq!(
        accepted.budget_used["engine_attempts"],
        job.budget_used["engine_attempts"] + 1
    );
    assert_eq!(e.calls.load(Ordering::SeqCst), 2);
    search.shutdown().await;
    current_search.shutdown().await;
}
#[tokio::test]
async fn bare_parent_replay_after_revocation_completion_and_deadline_preserves_all_store_bytes() {
    let server = Server::start();
    let e = engine(false);
    let (planning, jobs, search, snapshots, job) = science_parent(&server, e.clone()).await;
    let (_stop, rx) = shutdown();
    let args = json!({"job_id":job.job_id,"query":"bare web replay","engines":["fake"],"parent_attempt_id":job.queries[0].attempts[0].attempt_id,"continuation_key":"bare-replay"});
    let accepted = planning
        .extend(tenant("org:test"), &args, rx.clone())
        .await
        .unwrap();
    let (_, _, current_search, _) = setup_policy(server.store(), e.clone(), &["research"]);
    let current = adaptive(jobs.clone(), current_search.clone(), snapshots.clone());
    current.update(tenant("org:test"),&json!({"job_id":job.job_id,"subquestion_states":{},"complete":true,"rationale":"caller closed work"})).await.unwrap();
    mutate_job(&server, &accepted, |v| {
        v["deadline"] = json!(now().seconds() - 1.0)
    });
    let before = current
        .get_job(tenant("org:test"), &job.job_id)
        .await
        .unwrap();
    let bytes = stored_values(&server);
    let calls = e.calls.load(Ordering::SeqCst);
    assert_eq!(
        current
            .extend(tenant("org:test"), &args, rx.clone())
            .await
            .unwrap(),
        before
    );
    assert_eq!(stored_values(&server), bytes);
    assert_eq!(e.calls.load(Ordering::SeqCst), calls);
    let mut science = args.clone();
    science["intent"] = json!("science");
    assert_eq!(
        error_value(
            current
                .extend(tenant("org:test"), &science, rx.clone())
                .await
        ),
        json!({"error":{"code":"tool_disabled","message":"intent 'science' requires the science grant (MCP_GRANT_SCIENCE=1)","field":"intent","grant":"MCP_GRANT_SCIENCE"}})
    );
    assert_eq!(
        error_value(current.extend(tenant("other"), &args, rx.clone()).await),
        json!({"error":{"code":"invalid_job_id","message":"unknown job id","field":"job_id"}})
    );
    let (_, _, revoked_search, _) = setup_policy(server.store(), e.clone(), &[]);
    let revoked = adaptive(jobs, revoked_search.clone(), snapshots);
    assert_eq!(
        error_value(revoked.extend(tenant("org:test"), &args, rx).await),
        json!({"error":{"code":"tool_disabled","message":"slopsearx_extend_research requires the research grant (MCP_GRANT_RESEARCH=1)"}})
    );
    assert_eq!(stored_values(&server), bytes);
    assert_eq!(e.calls.load(Ordering::SeqCst), calls);
    search.shutdown().await;
    current_search.shutdown().await;
    revoked_search.shutdown().await;
}
#[tokio::test]
async fn evidence_parent_policy_precedes_resolved_duplicate_variant_and_preview_budget_errors() {
    for mode in [
        "resolved",
        "duplicate",
        "variant",
        "preview_budget",
        "bad_admitted_id",
    ] {
        let server = Server::start();
        let e = engine(false);
        let (planning, jobs, search, snapshots, job) = science_parent(&server, e.clone()).await;
        if mode == "resolved" {
            planning
                .update(
                    tenant("org:test"),
                    &json!({"job_id":job.job_id,"subquestion_states":{"a":"resolved"}}),
                )
                .await
                .unwrap();
        }
        if mode == "preview_budget" {
            mutate_job(&server, &job, |v| v["budget_limits"]["queries"] = json!(1));
        }
        let (_, _, current_search, _) = setup_policy(server.store(), e.clone(), &["research"]);
        let current = adaptive(jobs, current_search.clone(), snapshots);
        let mut args = followup(&job);
        if mode == "duplicate" {
            args["query"] = json!(job.queries[0].query);
        }
        if mode == "variant" {
            args["query"] = json!("dropped the exact identifier");
            args["planning_method"] = json!("terminology_expansion");
        }
        if mode == "bad_admitted_id" {
            args["evidence_result_ids"] = json!(["forged:0"]);
        }
        let expected = if mode == "bad_admitted_id" {
            json!({"error":{"code":"invalid_result_id","message":"evidence must be admitted results of the named parent"}})
        } else {
            json!({"error":{"code":"policy_rejected","message":"science requires the science grant"}})
        };
        let before = current
            .get_job(tenant("org:test"), &job.job_id)
            .await
            .unwrap();
        let bytes = stored_values(&server);
        let calls = e.calls.load(Ordering::SeqCst);
        let (_stop, rx) = shutdown();
        if mode != "variant" {
            assert_eq!(
                error_value(current.preview_followup(tenant("org:test"), &args).await),
                expected,
                "preview {mode}"
            );
        }
        let mut direct_expected = expected.clone();
        direct_expected["error"]["field"] = json!("query");
        assert_eq!(
            error_value(current.extend(tenant("org:test"), &args, rx).await),
            direct_expected,
            "extend {mode}"
        );
        assert_eq!(
            current
                .get_job(tenant("org:test"), &job.job_id)
                .await
                .unwrap(),
            before
        );
        assert_eq!(stored_values(&server), bytes);
        assert_eq!(e.calls.load(Ordering::SeqCst), calls);
        search.shutdown().await;
        current_search.shutdown().await;
    }
}
#[tokio::test]
async fn start_and_preview_preserve_distinct_source_limit_validation_order() {
    let server = Server::start();
    let e = engine(false);
    let (_, jobs, search, snapshots) = setup(&server, e.clone());
    let p = adaptive(jobs, search.clone(), snapshots);
    let bytes = stored_values(&server);
    let mut args = json!({"question":"q","engines":["fake"],"subquestions":[{"id":"a","question":"facet"}],"variants":["variant"],"max_attempts":0,"max_engines_per_query":0});
    assert_eq!(
        error_value(p.start(tenant("org:test"), &args).await),
        json!({"error":{"code":"invalid_input","message":"max_attempts must be a positive integer"}})
    );
    for variants in [false, true] {
        assert_eq!(
            error_value(p.preview_initial(&args, variants).await),
            json!({"error":{"code":"invalid_input","message":"max_engines_per_query must be a positive integer"}})
        );
    }
    args["max_queries"] = json!(0);
    assert_eq!(
        error_value(p.start(tenant("org:test"), &args).await),
        json!({"error":{"code":"invalid_input","message":"max_queries must be a positive integer"}})
    );
    for variants in [false, true] {
        assert_eq!(
            error_value(p.preview_initial(&args, variants).await),
            json!({"error":{"code":"invalid_input","message":"max_queries must be a positive integer"}})
        );
    }
    assert_eq!(stored_values(&server), bytes);
    assert_eq!(e.calls.load(Ordering::SeqCst), 0);
    search.shutdown().await;
}

#[tokio::test]
async fn legacy_retry_public_preserves_unknown_attempt_and_known_artifact() {
    for failed in [false, true] {
        let server = Server::start();
        let e = engine(false);
        e.count.store(0, Ordering::SeqCst);
        let (_, jobs, search, snapshots) = setup(&server, e.clone());
        let p = adaptive(jobs, search.clone(), snapshots);
        let initial = first(&p, start_args()).await;
        assert_eq!(initial.queries[0].state, "done");
        assert_eq!(initial.queries[0].result_count, 0);
        assert_eq!(initial.queries[0].attempts.len(), 1);
        mutate_job(&server, &initial, |v| {
            v["queries"].as_array_mut().unwrap()[0]["attempts"] = json!([]);
            if failed {
                v["state"] = json!("partial");
                v["queries"].as_array_mut().unwrap()[0]["state"] = json!("failed");
                v["queries"].as_array_mut().unwrap()[0]["error"] =
                    json!("retained legacy adapter failure");
                v["queries"].as_array_mut().unwrap()[0]["engine_coverage"] =
                    json!([classify_coverage(
                        "fake".into(),
                        true,
                        Some("error"),
                        0,
                        false
                    )]);
            }
        });
        let legacy = p
            .get_job(tenant("org:test"), &initial.job_id)
            .await
            .unwrap();
        let prior = legacy.queries[0].clone();
        assert!(prior.attempts.is_empty());
        assert_eq!(prior.state, if failed { "failed" } else { "done" });
        assert_eq!(
            prior.error.as_deref(),
            failed.then_some("retained legacy adapter failure")
        );
        assert_eq!(
            prior.engine_coverage[0].bucket,
            if failed { "failed" } else { "empty" }
        );
        let calls = e.calls.load(Ordering::SeqCst);
        e.count.store(3, Ordering::SeqCst);
        let (_stop, rx) = shutdown();
        let public = p
            .retry_public(tenant("org:test"), &initial.job_id, rx)
            .await
            .unwrap();
        let persisted = p
            .get_job(tenant("org:test"), &initial.job_id)
            .await
            .unwrap();
        let attempts = &persisted.queries[0].attempts;
        assert_eq!(attempts.len(), 2);
        let unknown = &attempts[0];
        assert!(unknown.attempt_id.is_empty());
        assert_eq!(unknown.attempted_at.get(), 0.0);
        assert!(unknown.finished_at.is_none());
        assert_eq!(unknown.state, prior.state);
        assert_eq!(unknown.cursor, prior.cursor);
        assert_eq!(unknown.query_id, prior.query_id);
        assert_eq!(unknown.result_count, prior.result_count);
        assert_eq!(unknown.error, prior.error);
        assert_eq!(unknown.engine_coverage, prior.engine_coverage);
        assert_eq!(unknown.enforcement, prior.enforcement);
        assert!(unknown.admitted_result_ids.is_empty());
        assert!(unknown.new_lead_ids.is_empty());
        assert!(!attempts[1].attempt_id.is_empty());
        assert_eq!(attempts[1].state, "done");
        assert_eq!(attempts[1].result_count, 3);
        assert_eq!(e.calls.load(Ordering::SeqCst), calls + 1);
        assert_eq!(persisted.budget_used["attempts"], 2);
        assert_eq!(persisted.budget_used["engine_attempts"], 2);
        assert_eq!(public["queries"][0]["attempts"][0]["artifact"], Value::Null);
        let serialized = ssx_json::to_value(unknown).unwrap();
        for (key, expected) in serialized.as_object().unwrap() {
            assert_eq!(
                &public["queries"][0]["attempts"][0][key.as_str()],
                expected,
                "legacy field {key}"
            );
        }
        assert_eq!(public["queries"][0]["attempts"][0]["snapshot_artifact"], prior.cursor.as_ref().map_or(Value::Null, |id| json!({"contract":"slopsearx.artifact_ref","version":1,"kind":"snapshot","id":id})));
        let known_id = ssx_domain::canonical::composite_artifact_id(&[
            &persisted.job_id,
            &attempts[1].attempt_id,
        ])
        .unwrap();
        assert_eq!(
            public["queries"][0]["attempts"][1]["artifact"],
            json!({"contract":"slopsearx.artifact_ref","version":1,"kind":"research_attempt","id":known_id})
        );
        assert_eq!(
            job_summary(&persisted)["queries"][0]["attempts"],
            public["queries"][0]["attempts"]
        );
        search.shutdown().await;
    }
}
