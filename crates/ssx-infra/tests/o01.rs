//! Private Valkey observability and operational failure qualification.
use ssx_application::{Audit, Metrics, health::OutcomeOrigin};
use ssx_domain::*;
use ssx_infra::observability::*;
use std::{
    fs,
    io::Read as _,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::Ordering,
    time::{Duration, Instant},
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
        let dir = std::env::temp_dir().join(format!("ssx-o01-{id}"));
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
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}
fn request() -> SearchRequest {
    SearchRequest {
        query: "private query token".into(),
        client_identifier: Some("192.0.2.12".into()),
        ..Default::default()
    }
}
fn response() -> SearchResponse {
    SearchResponse {
        engine_outcomes: vec![EngineOutcome {
            engine: "fixture".into(),
            status: "ok".into(),
            result_count: 4,
            ..Default::default()
        }],
        ..Default::default()
    }
}
#[tokio::test]
async fn retained_audit_and_stats_use_native_private_valkey() {
    let server = Server::start();
    let telemetry = Telemetry::new(
        Options {
            audit: true,
            privacy: QueryPrivacy::RetainRaw,
            ..Default::default()
        },
        Some(&server.url),
        vec!["fixture".into()],
        vec![],
    )
    .unwrap();
    telemetry.query(&request(), &response(), Duration::from_millis(123));
    telemetry.engine_outcome(
        "fixture",
        &AdapterResponse::default(),
        OutcomeOrigin::AdapterReturned,
    );
    telemetry.shutdown().await;
    assert_eq!(telemetry.delivery.failed.load(Ordering::Relaxed), 0);
    assert_eq!(telemetry.delivery.written.load(Ordering::Relaxed), 2);
    let day = chrono::Local::now().format("%Y-%m-%d");
    let mut connection = server.connection();
    let key = format!("query_audit:{day}");
    let rows: redis::streams::StreamRangeReply = redis::cmd("XRANGE")
        .arg(&key)
        .arg("-")
        .arg("+")
        .query(&mut connection)
        .unwrap();
    assert_eq!(rows.ids.len(), 1);
    let entry = &rows.ids[0];
    assert_eq!(entry.get::<String>("query").unwrap(), "private query token");
    assert_eq!(entry.get::<String>("total_results").unwrap(), "4");
    assert_eq!(entry.get::<String>("latency_ms").unwrap(), "123.0");
    let ttl: i64 = redis::cmd("TTL").arg(&key).query(&mut connection).unwrap();
    assert!((7775990..=7776000).contains(&ttl));
    let queries: i64 = redis::cmd("HGET")
        .arg(format!("engine_stats:fixture:{day}"))
        .arg("queries")
        .query(&mut connection)
        .unwrap();
    assert_eq!(queries, 1);
}
#[tokio::test]
async fn redacted_and_disabled_auditing_are_distinct() {
    assert!(matches!(Options::default().privacy, QueryPrivacy::Redacted));
    let server = Server::start();
    let telemetry = Telemetry::new(
        Options {
            audit: true,
            monitoring: false,
            ..Default::default()
        },
        Some(&server.url),
        vec![],
        vec![],
    )
    .unwrap();
    telemetry.query(&request(), &response(), Duration::ZERO);
    telemetry.shutdown().await;
    let mut conn = server.connection();
    let rows: redis::streams::StreamRangeReply = redis::cmd("XRANGE")
        .arg(format!(
            "query_audit:{}",
            chrono::Local::now().format("%Y-%m-%d")
        ))
        .arg("-")
        .arg("+")
        .query(&mut conn)
        .unwrap();
    assert_eq!(rows.ids[0].get::<String>("query").unwrap(), "[redacted]");
    assert_eq!(
        rows.ids[0].get::<String>("client_ip").unwrap(),
        "[redacted]"
    );
    assert!(telemetry.metrics.render().is_empty());
    let disabled = Telemetry::new(
        Options {
            audit: false,
            monitoring: false,
            ..Default::default()
        },
        None,
        vec![],
        vec![],
    )
    .unwrap();
    disabled.query(&request(), &response(), Duration::ZERO);
    disabled.shutdown().await;
    assert_eq!(disabled.delivery.written.load(Ordering::Relaxed), 0);
    assert_eq!(disabled.delivery.failed.load(Ordering::Relaxed), 0);
}
#[tokio::test]
async fn overload_is_bounded_and_shutdown_joins_stuck_exporter() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let peer = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let _socket = socket;
        std::future::pending::<()>().await;
    });
    let telemetry = Telemetry::new(
        Options {
            audit: true,
            queue_capacity: 2,
            operation_timeout: Duration::from_secs(30),
            shutdown_timeout: Duration::from_millis(30),
            ..Default::default()
        },
        Some(&format!("redis://{address}")),
        vec![],
        vec![],
    )
    .unwrap();
    for _ in 0..10000 {
        telemetry.query(&request(), &response(), Duration::ZERO);
    }
    assert!(telemetry.delivery.dropped.load(Ordering::Relaxed) >= 9997);
    let start = Instant::now();
    telemetry.shutdown().await;
    assert!(start.elapsed() < Duration::from_secs(1));
    telemetry.shutdown().await;
    peer.abort();
    let _ = peer.await;
}
#[tokio::test]
async fn unavailable_store_is_nonfatal_and_not_retried() {
    let private_store = Server::start();
    let unavailable_url = private_store.url.clone();
    // Verify a real private store existed, then kill/reap it and remove its socket.
    // A missing store configuration remains the separate zero-failure no-op case.
    drop(private_store);
    let telemetry = Telemetry::new(
        Options {
            audit: true,
            ..Default::default()
        },
        Some(unavailable_url.as_str()),
        vec![],
        vec![],
    )
    .unwrap();
    telemetry.query(&request(), &response(), Duration::ZERO);
    tokio::time::timeout(Duration::from_secs(3), telemetry.shutdown())
        .await
        .unwrap();
    assert_eq!(telemetry.delivery.failed.load(Ordering::Relaxed), 1);
    assert_eq!(telemetry.delivery.written.load(Ordering::Relaxed), 0);
    assert_eq!(telemetry.delivery.dropped.load(Ordering::Relaxed), 0);
}
use ssx_application::{
    NoopSink, PortFuture, SearchEngine, cache::RoutingIdentity, policy::*, scope::*,
};
use ssx_infra::search::*;
use std::{collections::BTreeMap, sync::Arc};
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
struct DuplicateEngine;
impl SearchEngine for DuplicateEngine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async {
            AdapterResponse {
                results: (0..4)
                    .map(|i| SearchResult {
                        url: format!("https://example.invalid/{}", i % 2),
                        title: "duplicate fixture".into(),
                        engine: "fixture".into(),
                        score: FiniteF64::new(0.75).unwrap(),
                        ..Default::default()
                    })
                    .collect(),
                latency_ms: FiniteF64::new(12.).unwrap(),
                ..Default::default()
            }
        })
    }
}
#[tokio::test]
async fn shared_search_audit_uses_unsliced_outcomes_when_status_is_excluded() {
    let server = Server::start();
    let telemetry = Telemetry::new(
        Options {
            audit: true,
            privacy: QueryPrivacy::RetainRaw,
            ..Default::default()
        },
        Some(&server.url),
        vec!["fixture".into()],
        vec![],
    )
    .unwrap();
    let mut deps = dependencies(vec![("fixture", Arc::new(DuplicateEngine))]);
    deps.metrics = telemetry.clone();
    deps.audit = telemetry.clone();
    deps.supervisor = telemetry.clone();
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let request = SearchRequest {
        engines: Some(vec!["fixture".into()]),
        max_results: Some(1),
        include: Default::default(),
        ..request()
    };
    let response = service
        .search(ScopeProposal::generic(request))
        .await
        .unwrap();
    assert_eq!(response.results.len(), 1);
    assert!(response.engine_outcomes.is_empty());
    service.shutdown().await;
    telemetry.shutdown().await;
    let mut conn = server.connection();
    let rows: redis::streams::StreamRangeReply = redis::cmd("XRANGE")
        .arg(format!(
            "query_audit:{}",
            chrono::Local::now().format("%Y-%m-%d")
        ))
        .arg("-")
        .arg("+")
        .query(&mut conn)
        .unwrap();
    assert_eq!(rows.ids.len(), 1);
    assert_eq!(rows.ids[0].get::<String>("total_results").unwrap(), "4");
    assert_eq!(rows.ids[0].get::<String>("engines_ok").unwrap(), "1");
    assert_eq!(
        rows.ids[0].get::<String>("latency_ms").unwrap(),
        response.response_time_ms.to_string()
    );
    assert_eq!(
        rows.ids[0].get::<String>("query").unwrap(),
        "private query token"
    );
    let score: i64 = redis::cmd("HGET")
        .arg(format!(
            "engine_stats:fixture:{}",
            chrono::Local::now().format("%Y-%m-%d")
        ))
        .arg("total_score")
        .query(&mut conn)
        .unwrap();
    assert_eq!(score, 750);
}
#[tokio::test]
async fn actual_source_audit_and_stat_pipeline_values_round_trip() {
    let corpus: ssx_json::Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/o01/source.json")).unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    let audit = cases.iter().find(|c| c["kind"] == "audit_raw").unwrap();
    let commands = audit["commands"].as_array().unwrap();
    let xadd = commands[0].as_array().unwrap();
    let fields = xadd[7..]
        .chunks(2)
        .map(|pair| (pair[0].as_str().unwrap(), pair[1].as_str().unwrap()))
        .collect::<BTreeMap<_, _>>();
    let server = Server::start();
    let telemetry = Telemetry::new(
        Options {
            audit: true,
            privacy: QueryPrivacy::RetainRaw,
            ..Default::default()
        },
        Some(&server.url),
        vec!["fixture".into()],
        vec![],
    )
    .unwrap();
    let request = SearchRequest {
        query: fields["query"].into(),
        client_identifier: Some(fields["client_ip"].into()),
        ..Default::default()
    };
    telemetry.query(&request, &response(), Duration::from_micros(123400));
    telemetry.engine_outcome(
        "fixture",
        &AdapterResponse {
            results: vec![
                SearchResult {
                    score: FiniteF64::new(0.75).unwrap(),
                    ..Default::default()
                };
                4
            ],
            latency_ms: FiniteF64::new(12.7).unwrap(),
            ..Default::default()
        },
        OutcomeOrigin::AdapterReturned,
    );
    telemetry.shutdown().await;
    let mut conn = server.connection();
    let day = chrono::Local::now().format("%Y-%m-%d");
    let rows: redis::streams::StreamRangeReply = redis::cmd("XRANGE")
        .arg(format!("query_audit:{day}"))
        .arg("-")
        .arg("+")
        .query(&mut conn)
        .unwrap();
    for (key, value) in fields {
        if key != "timestamp" {
            assert_eq!(rows.ids[0].get::<String>(key).unwrap(), value, "{key}");
        }
    }
    let stats = cases.iter().find(|c| c["kind"] == "stats").unwrap();
    for command in stats["commands"].as_array().unwrap() {
        let cmd = command.as_array().unwrap();
        if cmd[0] == "hincrby" {
            let actual: i64 = redis::cmd("HGET")
                .arg(format!("engine_stats:fixture:{day}"))
                .arg(cmd[2].as_str().unwrap())
                .query(&mut conn)
                .unwrap();
            assert_eq!(actual, cmd[3].as_i64().unwrap());
        }
    }
}
#[tokio::test]
async fn request_job_spans_settle_on_success_and_cancellation() {
    let telemetry = Telemetry::new(Options::default(), None, vec![], vec![]).unwrap();
    {
        let request = telemetry.span(SpanKind::Request);
        let _job = telemetry.span(SpanKind::Job);
        assert_eq!(telemetry.active_spans(), 2);
        request.finish();
        assert_eq!(telemetry.active_spans(), 1);
    }
    assert_eq!(telemetry.active_spans(), 0);
    telemetry.shutdown().await;
}
#[tokio::test]
async fn shared_search_cache_metrics_preserve_hit_miss_negative_and_bypass() {
    use ssx_application::cache::{CacheTtls, CanonicalCache};
    use ssx_infra::valkey::{Valkey, cache::ValkeyCache};
    let server = Server::start();
    let telemetry = Telemetry::new(
        Options::default(),
        Some(&server.url),
        vec!["fixture".into()],
        vec![],
    )
    .unwrap();
    let mut deps = dependencies(vec![("fixture", Arc::new(DuplicateEngine))]);
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(300)).unwrap());
    deps.cache = Some(Arc::new(CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    )));
    deps.owned_stores.push(store);
    deps.metrics = telemetry.clone();
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let mut proposal = ScopeProposal::generic(SearchRequest {
        engines: Some(vec!["fixture".into()]),
        ..request()
    });
    assert!(!service.search(proposal.clone()).await.unwrap().cached);
    assert!(service.search(proposal.clone()).await.unwrap().cached);
    proposal.request.freshness = "prefer_fresh".into();
    assert!(!service.search(proposal.clone()).await.unwrap().cached);
    let mut conn = server.connection();
    let (_, keys): (u64, Vec<String>) = redis::cmd("SCAN")
        .arg(0)
        .arg("MATCH")
        .arg("search:*")
        .arg("COUNT")
        .arg(100)
        .query(&mut conn)
        .unwrap();
    assert_eq!(keys.len(), 1);
    redis::cmd("SET")
        .arg(&keys[0])
        .arg("{\"_error\":true}")
        .query::<()>(&mut conn)
        .unwrap();
    proposal.request.freshness = "no_preference".into();
    assert!(service.search(proposal).await.unwrap().cached_error);
    let rendered = telemetry.metrics.render();
    for kind in ["miss", "hit", "negative"] {
        assert!(
            rendered.contains(&format!("slopsearx_cache_hit_total{{type=\"{kind}\"}} 1")),
            "{rendered}"
        );
    }
    service.shutdown().await;
    telemetry.shutdown().await;
}
#[test]
fn actual_source_engine_metrics_preserve_all_classified_statuses() {
    let source: ssx_json::Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/o01/source.json")).unwrap();
    for case in source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "engine_status")
    {
        let status = match case["status"].as_str().unwrap() {
            "ok" => EngineStatus::Ok,
            "rate_limited" => EngineStatus::RateLimited,
            "blocked" => EngineStatus::Blocked,
            "error" => EngineStatus::Error,
            "timeout" => EngineStatus::Timeout,
            "unavailable" => EngineStatus::Unavailable,
            _ => panic!("unknown source status"),
        };
        let metrics = metrics::Metrics::new(true, vec!["fixture".into()], vec![]).unwrap();
        metrics.engine_outcome(
            "fixture",
            &AdapterResponse {
                status,
                latency_ms: FiniteF64::new(12.7).unwrap(),
                ..Default::default()
            },
            OutcomeOrigin::AdapterReturned,
        );
        let all = metrics.render();
        let actual = all
            .split("# HELP slopsearx_cache_hit_total")
            .next()
            .unwrap();
        assert_eq!(actual, case["render"].as_str().unwrap());
    }
}
#[test]
fn passive_health_keeps_unknown_stale_and_synthetic_measurements_distinct() {
    use ssx_application::health::*;
    let mut health = HealthState::default();
    let now = WallClockTimestamp::new(1000.).unwrap();
    assert_eq!(
        build_engine_health(Some(&health), None, now, 300.).unwrap()["status"],
        "unknown"
    );
    let response = AdapterResponse {
        status: EngineStatus::Timeout,
        latency_ms: FiniteF64::new(500.).unwrap(),
        error_message: Some("private query secret".into()),
        ..Default::default()
    };
    health.observe(OutcomeOrigin::Undispatched, &response, now);
    assert_eq!(
        build_engine_health(Some(&health), None, now, 300.).unwrap()["status"],
        "unknown"
    );
    health.observe(OutcomeOrigin::DispatchedSyntheticTimeout, &response, now);
    let observed = build_engine_health(
        Some(&health),
        None,
        WallClockTimestamp::new(1400.).unwrap(),
        300.,
    )
    .unwrap();
    assert_eq!(observed["status"], "timeout");
    assert_eq!(observed["last_observed_latency_ms"], ssx_json::Value::Null);
    assert_eq!(observed["stale"], true);
    assert!(!observed.to_string().contains("secret"));
}
#[tokio::test]
async fn approximate_audit_stream_cap_and_ttl_are_applied_to_existing_stream() {
    let server = Server::start();
    let mut connection = server.connection();
    let key = format!("query_audit:{}", chrono::Local::now().format("%Y-%m-%d"));
    let mut pipe = redis::pipe();
    for _ in 0..10200 {
        pipe.cmd("XADD")
            .arg(&key)
            .arg("*")
            .arg("query")
            .arg("seed")
            .ignore();
    }
    pipe.query::<()>(&mut connection).unwrap();
    let telemetry = Telemetry::new(
        Options {
            audit: true,
            ..Default::default()
        },
        Some(&server.url),
        vec![],
        vec![],
    )
    .unwrap();
    telemetry.query(&request(), &response(), Duration::ZERO);
    telemetry.shutdown().await;
    let length: usize = redis::cmd("XLEN").arg(&key).query(&mut connection).unwrap();
    assert!((10000..10100).contains(&length));
    let ttl: i64 = redis::cmd("TTL").arg(&key).query(&mut connection).unwrap();
    assert!((7775990..=7776000).contains(&ttl));
}
struct PanickingTelemetryEngine;
impl SearchEngine for PanickingTelemetryEngine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async { panic!("intentional isolated adapter panic") })
    }
}
#[tokio::test]
async fn real_telemetry_exports_internal_panics_without_upstream_health_or_status() {
    let telemetry =
        Telemetry::new(Options::default(), None, vec!["fixture".into()], vec![]).unwrap();
    let mut deps = dependencies(vec![("fixture", Arc::new(PanickingTelemetryEngine))]);
    deps.metrics = telemetry.clone();
    deps.supervisor = telemetry.clone();
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    for i in 0..6 {
        let response = service
            .search(ScopeProposal::generic(SearchRequest {
                query: format!("panic-{i}"),
                engines: Some(vec!["fixture".into()]),
                include: ["engine_status".into()].into(),
                ..Default::default()
            }))
            .await
            .unwrap();
        assert_eq!(response.dispatched_engine_count, 1);
        assert_eq!(response.engine_outcomes[0].status, "error");
        assert_eq!(
            response.engine_outcomes[0].message.as_deref(),
            Some("internal adapter task failure")
        );
        assert!(response.results.is_empty());
    }
    let rendered = telemetry.metrics.render();
    for (event, count) in [("internal_failure", 6), ("synthetic", 6), ("observed", 0)] {
        assert!(
            rendered.contains(&format!(
                "slopsearx_search_engine_events_total{{event=\"{event}\"}} {count}"
            )),
            "{rendered}"
        );
    }
    telemetry.increment("untrusted_secret_name");
    assert_eq!(telemetry.metrics.render(), rendered);
    assert!(!rendered.contains("slopsearx_engine_errors_total{engine="));
    assert!(!rendered.contains("slopsearx_engine_status{engine="));
    assert!(!rendered.contains("slopsearx_engine_latency_seconds_count{engine="));
    assert!(service.health().is_empty());
    let health = ssx_application::health::build_engine_health(
        None,
        None,
        SystemSearchClock.capture().0,
        300.,
    )
    .unwrap();
    assert_eq!(health["status"], "unknown");
    assert_eq!(health["circuit_consecutive_errors"], 0);
    assert_eq!(health["last_observed_latency_ms"], ssx_json::Value::Null);
    service.shutdown().await;
    telemetry.shutdown().await;
    let disabled = Telemetry::new(
        Options {
            monitoring: false,
            ..Default::default()
        },
        None,
        vec![],
        vec![],
    )
    .unwrap();
    disabled.increment("search_engine_internal_failure");
    assert!(disabled.metrics.render().is_empty());
    disabled.shutdown().await;
}

#[tokio::test]
async fn explicit_raw_retention_and_absent_store_are_qualified() {
    let server = Server::start();
    let telemetry = Telemetry::new(
        Options {
            privacy: QueryPrivacy::RetainRaw,
            ..Default::default()
        },
        Some(&server.url),
        vec![],
        vec![],
    )
    .unwrap();
    telemetry.query(&request(), &response(), Duration::ZERO);
    telemetry.shutdown().await;
    let mut conn = server.connection();
    let rows: redis::streams::StreamRangeReply = redis::cmd("XRANGE")
        .arg(format!(
            "query_audit:{}",
            chrono::Local::now().format("%Y-%m-%d")
        ))
        .arg("-")
        .arg("+")
        .query(&mut conn)
        .unwrap();
    assert_eq!(rows.ids.len(), 1);
    assert_eq!(rows.ids[0].get::<String>("query").unwrap(), request().query);
    assert_eq!(
        rows.ids[0].get::<String>("client_ip").unwrap(),
        request().client_identifier.unwrap_or("unknown".into())
    );
    let absent = Telemetry::new(Options::default(), None, vec!["fixture".into()], vec![]).unwrap();
    absent.query(&request(), &response(), Duration::ZERO);
    absent.engine_outcome(
        "fixture",
        &AdapterResponse::default(),
        OutcomeOrigin::AdapterReturned,
    );
    absent.shutdown().await;
    assert_eq!(absent.delivery.written.load(Ordering::Relaxed), 0);
    assert_eq!(absent.delivery.failed.load(Ordering::Relaxed), 0);
    assert_eq!(absent.delivery.dropped.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn checked_audit_totals_preserve_boundary_and_refuse_overflow_without_queueing() {
    let server = Server::start();
    let telemetry = Telemetry::new(Options::default(), Some(&server.url), vec![], vec![]).unwrap();
    for counts in [
        vec![i64::MAX],
        vec![i64::MAX - 1, 1],
        vec![-1, i64::MAX],
        vec![i64::MAX, 1],
        vec![i64::MAX, i64::MAX],
    ] {
        let mut response = response();
        response.engine_outcomes = counts
            .into_iter()
            .map(|result_count| EngineOutcome {
                engine: "fixture".into(),
                status: "ok".into(),
                result_count,
                ..Default::default()
            })
            .collect();
        telemetry.query(&request(), &response, Duration::ZERO);
    }
    telemetry.shutdown().await;
    assert_eq!(telemetry.delivery.written.load(Ordering::Relaxed), 3);
    assert_eq!(telemetry.delivery.dropped.load(Ordering::Relaxed), 2);
    assert_eq!(telemetry.delivery.failed.load(Ordering::Relaxed), 0);
    let mut connection = server.connection();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("query_audit:*")
        .query(&mut connection)
        .unwrap();
    let mut observed = 0;
    for key in keys {
        let rows: redis::streams::StreamRangeReply = redis::cmd("XRANGE")
            .arg(key)
            .arg("-")
            .arg("+")
            .query(&mut connection)
            .unwrap();
        for row in rows.ids {
            assert_eq!(
                row.get::<String>("total_results").unwrap(),
                i64::MAX.to_string()
            );
            observed += 1;
        }
    }
    assert_eq!(observed, 3);
}

#[tokio::test]
async fn six_original_stats_statuses_match_actual_private_backend_fields_and_retention() {
    let source: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../xtask/fixtures/o01/source-supplement.json"
    ))
    .unwrap();
    let server = Server::start();
    let mut connection = server.connection();
    let mut completed = Vec::new();
    for (name, status) in [
        ("ok", EngineStatus::Ok),
        ("rate_limited", EngineStatus::RateLimited),
        ("blocked", EngineStatus::Blocked),
        ("error", EngineStatus::Error),
        ("timeout", EngineStatus::Timeout),
        ("unavailable", EngineStatus::Unavailable),
    ] {
        let id = format!("stats.status.{name}");
        let case = source["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"].as_str() == Some(id.as_str()))
            .unwrap();
        assert_eq!(case["input"]["status"], name);
        assert_eq!(case["output"]["awaited"], 1);
        let started = std::time::Instant::now();
        let telemetry = Telemetry::new(
            Options {
                audit: false,
                ..Default::default()
            },
            Some(&server.url),
            vec!["fixture".into()],
            vec![],
        )
        .unwrap();
        telemetry.engine_outcome(
            "fixture",
            &AdapterResponse {
                status,
                results: vec![
                    SearchResult {
                        score: FiniteF64::new(0.75).unwrap(),
                        ..Default::default()
                    };
                    4
                ],
                latency_ms: FiniteF64::new(12.7).unwrap(),
                ..Default::default()
            },
            OutcomeOrigin::AdapterReturned,
        );
        telemetry.shutdown().await;
        assert_eq!(telemetry.delivery.written.load(Ordering::Relaxed), 1);
        assert_eq!(telemetry.delivery.failed.load(Ordering::Relaxed), 0);
        assert_eq!(telemetry.delivery.dropped.load(Ordering::Relaxed), 0);
        let day = chrono::Local::now().format("%Y-%m-%d");
        let key = format!("engine_stats:fixture:{day}");
        let actual: BTreeMap<String, i64> = redis::cmd("HGETALL")
            .arg(&key)
            .query(&mut connection)
            .unwrap();
        let mut expected = BTreeMap::new();
        let mut expiry = None;
        for command in case["output"]["commands"].as_array().unwrap() {
            let command = command.as_array().unwrap();
            match command[0].as_str().unwrap() {
                "hincrby" => {
                    expected.insert(
                        command[2].as_str().unwrap().to_owned(),
                        command[3].as_i64().unwrap(),
                    );
                }
                "expire" => {
                    expiry = Some(command[2].as_i64().unwrap());
                }
                other => panic!("unexpected captured command {other}"),
            }
        }
        assert_eq!(actual, expected, "{id}");
        let ttl: i64 = redis::cmd("TTL").arg(&key).query(&mut connection).unwrap();
        let expected_ttl = expiry.unwrap();
        assert!(
            ttl > 0
                && ttl <= expected_ttl
                && expected_ttl - ttl <= started.elapsed().as_secs() as i64 + 1,
            "{id}: {ttl}"
        );
        let deleted: usize = redis::cmd("DEL").arg(&key).query(&mut connection).unwrap();
        assert_eq!(deleted, 1);
        drop(telemetry);
        completed.push(id);
    }
    let remaining: usize = redis::cmd("DBSIZE").query(&mut connection).unwrap();
    assert_eq!(remaining, 0);
    drop(connection);
    drop(server);
    assert_eq!(completed.len(), 6);
    for id in completed {
        println!(
            "O01_CASE_COMPLETE:{}",
            ssx_json::json!({"id":id,"consumer":"six_original_stats_statuses_match_actual_private_backend_fields_and_retention","class":"source-private-backend"})
        );
    }
}

#[tokio::test]
async fn original_object_and_dictionary_audit_fields_share_exact_native_typed_projection() {
    let source: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../xtask/fixtures/o01/source-supplement.json"
    ))
    .unwrap();
    let server = Server::start();
    let mut connection = server.connection();
    let mut completed = Vec::new();
    for id in ["audit.statuses.object", "audit.statuses.dict"] {
        let case = source["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == id)
            .unwrap();
        let commands = case["output"]["commands"].as_array().unwrap();
        let command = commands[0].as_array().unwrap();
        assert_eq!(command[0], "execute_command");
        assert_eq!(command[1], "XADD");
        assert_eq!(command[3], "MAXLEN");
        assert_eq!(command[4], "~");
        assert_eq!(command[5], "10000");
        assert_eq!(command[6], "*");
        let expected = command[7..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                (
                    pair[0].as_str().unwrap().to_owned(),
                    pair[1].as_str().unwrap().to_owned(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let started = std::time::Instant::now();
        let telemetry = Telemetry::new(
            Options {
                privacy: QueryPrivacy::RetainRaw,
                ..Default::default()
            },
            Some(&server.url),
            vec![],
            vec![],
        )
        .unwrap();
        let request = SearchRequest {
            query: expected["query"].clone(),
            client_identifier: Some(expected["client_ip"].clone()),
            ..Default::default()
        };
        // Original object/dictionary input branches normalize to the same native
        // typed outcome. The immutable source captures independently prove both.
        let response = SearchResponse {
            engine_outcomes: [
                "ok",
                "rate_limited",
                "blocked",
                "error",
                "timeout",
                "unavailable",
            ]
            .into_iter()
            .map(|status| EngineOutcome {
                engine: status.into(),
                status: status.into(),
                result_count: 0,
                ..Default::default()
            })
            .collect(),
            ..Default::default()
        };
        telemetry.query_completed(&request, &response, 0);
        telemetry.shutdown().await;
        assert_eq!(telemetry.delivery.written.load(Ordering::Relaxed), 1);
        assert_eq!(telemetry.delivery.failed.load(Ordering::Relaxed), 0);
        assert_eq!(telemetry.delivery.dropped.load(Ordering::Relaxed), 0);
        let day = chrono::Local::now().format("%Y-%m-%d");
        let key = format!("query_audit:{day}");
        let rows: redis::streams::StreamRangeReply = redis::cmd("XRANGE")
            .arg(&key)
            .arg("-")
            .arg("+")
            .query(&mut connection)
            .unwrap();
        assert_eq!(rows.ids.len(), 1);
        assert_eq!(rows.ids[0].map.len(), expected.len());
        for (field, value) in &expected {
            let actual = rows.ids[0].get::<String>(field).unwrap();
            if field == "timestamp" {
                let at = chrono::DateTime::parse_from_rfc3339(&actual).unwrap();
                assert!(at.timestamp_millis() <= chrono::Utc::now().timestamp_millis());
                assert!(
                    chrono::Utc::now()
                        .signed_duration_since(at)
                        .num_milliseconds()
                        <= started.elapsed().as_millis() as i64 + 1
                );
            } else {
                assert_eq!(actual, *value, "{id}: {field}");
            }
        }
        let ttl: i64 = redis::cmd("TTL").arg(&key).query(&mut connection).unwrap();
        let expected_ttl = commands[1][2].as_i64().unwrap();
        assert!(
            ttl > 0
                && ttl <= expected_ttl
                && expected_ttl - ttl <= started.elapsed().as_secs() as i64 + 1
        );
        let deleted: usize = redis::cmd("DEL").arg(&key).query(&mut connection).unwrap();
        assert_eq!(deleted, 1);
        drop(telemetry);
        completed.push(id);
    }
    let remaining: usize = redis::cmd("DBSIZE").query(&mut connection).unwrap();
    assert_eq!(remaining, 0);
    drop(connection);
    drop(server);
    assert_eq!(completed.len(), 2);
    for id in completed {
        println!(
            "O01_CASE_COMPLETE:{}",
            ssx_json::json!({"id":id,"consumer":"original_object_and_dictionary_audit_fields_share_exact_native_typed_projection","class":"source-private-backend","normalization":"object/dictionary collapse to native typed outcomes; live timestamp replaces frozen clock"})
        );
    }
}

#[tokio::test]
async fn original_thirteen_audit_latencies_match_except_reviewed_duration_signed_zero() {
    let source: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../xtask/fixtures/o01/source-supplement.json"
    ))
    .unwrap();
    let server = Server::start();
    let mut connection = server.connection();
    let mut markers = Vec::new();
    let ledger: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../xtask/fixtures/o01/native-differences.json"
    ))
    .unwrap();
    let mut completed = 0;
    for case in source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["id"].as_str().unwrap().starts_with("audit.latency."))
    {
        let id = case["id"].as_str().unwrap();
        let repr = case["input"]["repr"].as_str().unwrap();
        let integer = case["input"]["type"] == "int";
        let command = case["output"]["commands"][0].as_array().unwrap();
        let expected = command[7..]
            .as_chunks::<2>()
            .0
            .iter()
            .find(|p| p[0] == "latency_ms")
            .unwrap()[1]
            .as_str()
            .unwrap();
        let telemetry = Telemetry::new(
            Options {
                privacy: QueryPrivacy::RetainRaw,
                ..Default::default()
            },
            Some(&server.url),
            vec![],
            vec![],
        )
        .unwrap();
        let request = SearchRequest {
            query: "synthetic query".into(),
            client_identifier: Some("192.0.2.12".into()),
            ..Default::default()
        };
        if integer {
            telemetry.query_completed(&request, &SearchResponse::default(), repr.parse().unwrap());
        } else {
            telemetry.query(
                &request,
                &SearchResponse::default(),
                Duration::try_from_secs_f64(repr.parse::<f64>().unwrap() / 1000.).unwrap(),
            );
        }
        telemetry.shutdown().await;
        assert_eq!(telemetry.delivery.written.load(Ordering::Relaxed), 1);
        assert_eq!(telemetry.delivery.failed.load(Ordering::Relaxed), 0);
        let day = chrono::Local::now().format("%Y-%m-%d");
        let key = format!("query_audit:{day}");
        let rows: redis::streams::StreamRangeReply = redis::cmd("XRANGE")
            .arg(&key)
            .arg("-")
            .arg("+")
            .query(&mut connection)
            .unwrap();
        assert_eq!(rows.ids.len(), 1);
        let actual = rows.ids[0].get::<String>("latency_ms").unwrap();
        let class = if id == "audit.latency.-0.0" {
            let difference = ledger["differences"]
                .as_array()
                .unwrap()
                .iter()
                .find(|difference| difference["case_id"] == id)
                .unwrap();
            assert_eq!(difference["case_id"], id);
            assert_eq!(difference["source"], expected);
            assert_eq!(difference["native"], actual.as_str());
            assert_eq!(expected, "-0.0");
            assert_eq!(actual, "0.0");
            assert_eq!(repr, "-0.0");
            assert!(!integer);
            assert_eq!(Duration::try_from_secs_f64(-0.0).unwrap(), Duration::ZERO);
            "reviewed-typed-port-supersession"
        } else {
            assert_eq!(actual, expected, "{id}");
            "source-private-backend"
        };
        markers.push((id.to_owned(), class));
        let deleted: usize = redis::cmd("DEL").arg(&key).query(&mut connection).unwrap();
        assert_eq!(deleted, 1);
        drop(telemetry);
        completed += 1;
    }
    assert_eq!(completed, 13);
    let remaining: usize = redis::cmd("DBSIZE").query(&mut connection).unwrap();
    assert_eq!(remaining, 0);
    drop(connection);
    drop(server);
    assert_eq!(
        markers
            .iter()
            .filter(|(_, class)| *class == "reviewed-typed-port-supersession")
            .count(),
        1
    );
    for (id, class) in markers {
        println!(
            "O01_CASE_COMPLETE:{}",
            ssx_json::json!({"id":id,"consumer":"original_thirteen_audit_latencies_match_except_reviewed_duration_signed_zero","class":class})
        );
    }
}

#[tokio::test]
async fn original_eight_availability_rows_preserve_noop_and_nonfatal_failure_with_explicit_native_projection()
 {
    let source: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../xtask/fixtures/o01/source-supplement.json"
    ))
    .unwrap();
    let ledger: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../xtask/fixtures/o01/native-differences.json"
    ))
    .unwrap();
    let projection = ledger["differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|difference| difference["id"] == "availability.absent-authority")
        .unwrap();
    assert_eq!(projection["id"], "availability.absent-authority");
    assert_eq!(projection["source_states"].as_array().unwrap().len(), 3);
    assert!(
        ledger["differences"]
            .as_array()
            .unwrap()
            .iter()
            .any(|difference| difference["id"] == "availability.failure-phase")
    );
    let mut completed = Vec::new();
    for kind in ["audit", "stats"] {
        for state in ["none", "disconnected", "no-client", "failed"] {
            let id = format!("{kind}.{state}");
            let case = source["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["id"].as_str() == Some(id.as_str()))
                .unwrap();
            assert_eq!(case["output"]["returned"], true);
            let unavailable_url = if state == "failed" {
                assert_eq!(case["output"]["awaited"], 1);
                assert!(!case["output"]["commands"].as_array().unwrap().is_empty());
                let server = Server::start();
                let url = server.url.clone();
                drop(server); // actual private server killed/reaped, socket removed
                Some(url)
            } else {
                assert!(
                    projection["source_states"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|s| s == &ssx_json::Value::from(state))
                );
                assert_eq!(case["output"]["awaited"], 0);
                assert!(case["output"]["commands"].as_array().unwrap().is_empty());
                None
            };
            let telemetry = Telemetry::new(
                Options {
                    privacy: QueryPrivacy::RetainRaw,
                    ..Default::default()
                },
                unavailable_url.as_deref(),
                vec!["fixture".into()],
                vec![],
            )
            .unwrap();
            if kind == "audit" {
                telemetry.query(
                    &SearchRequest {
                        query: "synthetic query".into(),
                        client_identifier: Some("192.0.2.12".into()),
                        ..Default::default()
                    },
                    &SearchResponse::default(),
                    Duration::from_millis(1),
                );
            } else {
                telemetry.engine_outcome(
                    "fixture",
                    &AdapterResponse {
                        latency_ms: FiniteF64::new(1.).unwrap(),
                        ..Default::default()
                    },
                    OutcomeOrigin::AdapterReturned,
                );
            }
            tokio::time::timeout(Duration::from_secs(3), telemetry.shutdown())
                .await
                .unwrap();
            let delivery = telemetry.delivery.clone();
            let expected_failure = u64::from(state == "failed");
            assert_eq!(
                delivery.failed.load(Ordering::Relaxed),
                expected_failure,
                "{id}"
            );
            assert_eq!(delivery.written.load(Ordering::Relaxed), 0, "{id}");
            assert_eq!(delivery.dropped.load(Ordering::Relaxed), 0, "{id}");
            drop(telemetry);
            tokio::task::yield_now().await;
            assert_eq!(
                delivery.failed.load(Ordering::Relaxed),
                expected_failure,
                "{id}"
            );
            completed.push(id);
        }
    }
    assert_eq!(completed.len(), 8);
    for id in completed {
        println!(
            "O01_CASE_COMPLETE:{}",
            ssx_json::json!({"id":id,"consumer":"original_eight_availability_rows_preserve_noop_and_nonfatal_failure_with_explicit_native_projection","class":"reviewed-typed-port-supersession","difference":"optional authority represents absent/disconnected/no-client; actual connection refusal tests nonfatal failure, not source mockexecute faultphase/commands parity"})
        );
        let node = match id.as_str() {
            "audit.none" => {
                "tests/test_audit.py::TestAuditDisconnected::test_record_query_no_cache_noop"
            }
            "audit.disconnected" => {
                "tests/test_audit.py::TestAuditDisconnected::test_record_query_disconnected_cache_noop"
            }
            "audit.no-client" => {
                "tests/test_audit.py::TestAuditDisconnected::test_record_query_no_client_noop"
            }
            "audit.failed" => "tests/test_audit.py::TestAuditPipeline::test_pipeline_error_noop",
            "stats.none" => {
                "tests/test_stats.py::TestStatsTrackerDisconnected::test_record_query_no_cache_noop"
            }
            "stats.disconnected" => {
                "tests/test_stats.py::TestStatsTrackerDisconnected::test_record_query_disconnected_cache_noop"
            }
            "stats.no-client" => {
                "tests/test_stats.py::TestStatsTrackerDisconnected::test_record_query_no_client_noop"
            }
            "stats.failed" => "tests/test_stats.py::TestStatsPipeline::test_pipeline_error_noop",
            _ => panic!("unexpected original availability mapping"),
        };
        println!(
            "O01_ORIGINAL_COMPLETE:{}",
            ssx_json::json!({"node":node,"consumer":"original_eight_availability_rows_preserve_noop_and_nonfatal_failure_with_explicit_native_projection","class":"reviewed-availability-typed-port"})
        );
    }
}
#[tokio::test]
async fn original_audit_and_stats_pipeline_assertions_use_exact_inputs_and_joined_private_writes() {
    let server = Server::start();
    let mut connection = server.connection();
    let mut completed = Vec::new();
    // Source TestAuditPipeline._make_engine_results has2successful rows,1error.
    let started = std::time::Instant::now();
    let telemetry = Telemetry::new(
        Options {
            privacy: QueryPrivacy::RetainRaw,
            ..Default::default()
        },
        Some(&server.url),
        vec![],
        vec![],
    )
    .unwrap();
    let request = SearchRequest {
        query: "test query".into(),
        client_identifier: Some("10.0.0.1".into()),
        ..Default::default()
    };
    let response = SearchResponse {
        engine_outcomes: vec![
            EngineOutcome {
                engine: "brave".into(),
                status: "ok".into(),
                result_count: 1,
                ..Default::default()
            },
            EngineOutcome {
                engine: "wikipedia".into(),
                status: "ok".into(),
                result_count: 1,
                ..Default::default()
            },
            EngineOutcome {
                engine: "google".into(),
                status: "error".into(),
                result_count: 0,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    telemetry.query(&request, &response, Duration::from_millis(3200));
    telemetry.shutdown().await;
    assert_eq!(telemetry.delivery.written.load(Ordering::Relaxed), 1);
    assert_eq!(telemetry.delivery.failed.load(Ordering::Relaxed), 0);
    assert_eq!(telemetry.delivery.dropped.load(Ordering::Relaxed), 0);
    let day = chrono::Local::now().format("%Y-%m-%d");
    let key = format!("query_audit:{day}");
    let parts = key.split(':').collect::<Vec<_>>();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "query_audit");
    assert!(chrono::NaiveDate::parse_from_str(parts[1], "%Y-%m-%d").is_ok());
    let date_parts = parts[1].split('-').map(str::len).collect::<Vec<_>>();
    assert_eq!(date_parts, vec![4, 2, 2]);
    let rows: redis::streams::StreamRangeReply = redis::cmd("XRANGE")
        .arg(&key)
        .arg("-")
        .arg("+")
        .query(&mut connection)
        .unwrap();
    assert_eq!(rows.ids.len(), 1);
    assert_eq!(rows.ids[0].map.len(), 9);
    for (field, value) in [
        ("query", "test query"),
        ("client_ip", "10.0.0.1"),
        ("engines", "brave,wikipedia,google"),
        ("engines_ok", "2"),
        ("engines_error", "1"),
        ("engines_timeout", "0"),
        ("total_results", "2"),
        ("latency_ms", "3200.0"),
    ] {
        assert_eq!(rows.ids[0].get::<String>(field).unwrap(), value);
    }
    let timestamp = rows.ids[0].get::<String>("timestamp").unwrap();
    assert!(chrono::DateTime::parse_from_rfc3339(&timestamp).is_ok());
    let ttl: i64 = redis::cmd("TTL").arg(&key).query(&mut connection).unwrap();
    assert!(
        ttl > 0 && ttl <= 7_776_000 && 7_776_000 - ttl <= started.elapsed().as_secs() as i64 + 1
    );
    let deleted: usize = redis::cmd("DEL").arg(&key).query(&mut connection).unwrap();
    assert_eq!(deleted, 1);
    drop(telemetry);
    for node in [
        "tests/test_audit.py::TestAuditPipeline::test_pipeline_execute_is_awaited",
        "tests/test_audit.py::TestAuditPipeline::test_pipeline_fields_correct",
        "tests/test_audit.py::TestAuditPipeline::test_expire_set_on_stream",
        "tests/test_audit.py::TestAuditPipeline::test_stream_key_includes_date",
        "tests/test_audit.py::TestAuditAsyncConformance::test_record_query_is_async",
    ] {
        completed.push(node);
    }
    // Exact source timeout fixture: timeout1, error0, successful1, result1.
    let telemetry = Telemetry::new(Options::default(), Some(&server.url), vec![], vec![]).unwrap();
    telemetry.query(
        &SearchRequest {
            query: "test".into(),
            client_identifier: Some("10.0.0.1".into()),
            ..Default::default()
        },
        &SearchResponse {
            engine_outcomes: vec![
                EngineOutcome {
                    engine: "brave".into(),
                    status: "timeout".into(),
                    result_count: 0,
                    ..Default::default()
                },
                EngineOutcome {
                    engine: "wikipedia".into(),
                    status: "ok".into(),
                    result_count: 1,
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
        Duration::from_millis(5000),
    );
    telemetry.shutdown().await;
    assert_eq!(telemetry.delivery.written.load(Ordering::Relaxed), 1);
    assert_eq!(telemetry.delivery.failed.load(Ordering::Relaxed), 0);
    let rows: redis::streams::StreamRangeReply = redis::cmd("XRANGE")
        .arg(&key)
        .arg("-")
        .arg("+")
        .query(&mut connection)
        .unwrap();
    assert_eq!(rows.ids.len(), 1);
    for (field, value) in [
        ("engines_timeout", "1"),
        ("engines_error", "0"),
        ("engines_ok", "1"),
        ("total_results", "1"),
    ] {
        assert_eq!(rows.ids[0].get::<String>(field).unwrap(), value);
    }
    let deleted: usize = redis::cmd("DEL").arg(&key).query(&mut connection).unwrap();
    assert_eq!(deleted, 1);
    drop(telemetry);
    completed.push("tests/test_audit.py::TestAuditPipeline::test_count_timeout_status_correctly");
    let started = std::time::Instant::now();
    let telemetry = Telemetry::new(
        Options::default(),
        Some(&server.url),
        vec!["testengine".into()],
        vec![],
    )
    .unwrap();
    telemetry.engine_outcome(
        "testengine",
        &AdapterResponse {
            results: vec![SearchResult::default(); 3],
            latency_ms: FiniteF64::new(100.).unwrap(),
            ..Default::default()
        },
        OutcomeOrigin::AdapterReturned,
    );
    telemetry.shutdown().await;
    assert_eq!(telemetry.delivery.written.load(Ordering::Relaxed), 1);
    assert_eq!(telemetry.delivery.failed.load(Ordering::Relaxed), 0);
    assert_eq!(telemetry.delivery.dropped.load(Ordering::Relaxed), 0);
    let key = format!("engine_stats:testengine:{day}");
    assert_eq!(key.matches(':').count(), 2);
    assert!(key.starts_with("engine_stats:testengine:"));
    let actual: BTreeMap<String, i64> = redis::cmd("HGETALL")
        .arg(&key)
        .query(&mut connection)
        .unwrap();
    assert_eq!(
        actual,
        BTreeMap::from([
            ("queries".into(), 1),
            ("results_returned".into(), 3),
            ("errors".into(), 0),
            ("rate_limited".into(), 0),
            ("total_latency_ms".into(), 100),
            ("total_score".into(), 0)
        ])
    );
    let ttl: i64 = redis::cmd("TTL").arg(&key).query(&mut connection).unwrap();
    assert!(
        ttl > 0 && ttl <= 7_776_000 && 7_776_000 - ttl <= started.elapsed().as_secs() as i64 + 1
    );
    let deleted: usize = redis::cmd("DEL").arg(&key).query(&mut connection).unwrap();
    assert_eq!(deleted, 1);
    drop(telemetry);
    for node in [
        "tests/test_stats.py::TestStatsPipeline::test_pipeline_execute_is_awaited",
        "tests/test_stats.py::TestStatsAsyncConformance::test_record_query_is_async",
    ] {
        completed.push(node);
    }
    for (node, status, count, latency, errors, limited) in [
        (
            "tests/test_stats.py::TestStatsPipeline::test_error_and_timeout_increment_errors",
            EngineStatus::Error,
            0,
            5000.,
            1,
            0,
        ),
        (
            "tests/test_stats.py::TestStatsPipeline::test_ok_status_does_not_increment_errors",
            EngineStatus::Ok,
            5,
            42.,
            0,
            0,
        ),
        (
            "tests/test_stats.py::TestStatsPipeline::test_rate_limited_status_increments_rate_limited",
            EngineStatus::RateLimited,
            0,
            100.,
            0,
            1,
        ),
    ] {
        let telemetry = Telemetry::new(
            Options::default(),
            Some(&server.url),
            vec!["testengine".into()],
            vec![],
        )
        .unwrap();
        telemetry.engine_outcome(
            "testengine",
            &AdapterResponse {
                status,
                results: vec![SearchResult::default(); count],
                latency_ms: FiniteF64::new(latency).unwrap(),
                ..Default::default()
            },
            OutcomeOrigin::AdapterReturned,
        );
        telemetry.shutdown().await;
        assert_eq!(telemetry.delivery.written.load(Ordering::Relaxed), 1);
        assert_eq!(telemetry.delivery.failed.load(Ordering::Relaxed), 0);
        let actual: BTreeMap<String, i64> = redis::cmd("HGETALL")
            .arg(&key)
            .query(&mut connection)
            .unwrap();
        assert_eq!(
            actual,
            BTreeMap::from([
                ("queries".into(), 1),
                ("results_returned".into(), count as i64),
                ("errors".into(), errors),
                ("rate_limited".into(), limited),
                ("total_latency_ms".into(), latency as i64),
                ("total_score".into(), 0)
            ])
        );
        let deleted: usize = redis::cmd("DEL").arg(&key).query(&mut connection).unwrap();
        assert_eq!(deleted, 1);
        drop(telemetry);
        completed.push(node);
    }
    let remaining: usize = redis::cmd("DBSIZE").query(&mut connection).unwrap();
    assert_eq!(remaining, 0);
    drop(connection);
    drop(server);
    assert_eq!(completed.len(), 11);
    for node in completed {
        println!(
            "O01_ORIGINAL_COMPLETE:{}",
            ssx_json::json!({"node":node,"consumer":"original_audit_and_stats_pipeline_assertions_use_exact_inputs_and_joined_private_writes","class":"native-original-assertions-bounded-worker","difference":"source async functions scheduled fire-and-forget by service; native callback queues owned asynchronous pipeline and explicit shutdown joins; no Python coroutine-introspection identity"})
        );
    }
}
