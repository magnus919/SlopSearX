//! Actual scholarly factories: timed admission and shared private-store cooldown.
use base64::Engine as _;
use ssx_application::cache::RoutingIdentity;
use ssx_application::{
    AdmissionDecision, NoopSink, PortFuture, SearchAdmission, SearchContext, SearchEngine,
    policy::*,
    rate::{RateLimiter, RateStore},
    scope::*,
};
use ssx_domain::{
    EngineStatus, SearchRequest,
    rate::{EngineKey, QuotaConfig},
};
use ssx_infra::{
    adapters::{
        F10Admission,
        arxiv_expat::AtomPool,
        arxiv_runtime::{self, ArxivConfig},
        openalex_runtime::{self, OpenAlexConfig},
        semantic_runtime::{self, SemanticConfig},
    },
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
    search::*,
    valkey::{Valkey, rate::DistributedQuota},
};
use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
struct SlowDecision(AdmissionDecision);
impl SearchAdmission for SlowDecision {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            self.0
        })
    }
}
struct Replies(AtomicUsize);
impl Transport for Replies {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let n = self.0.fetch_add(1, Ordering::SeqCst);
            let mut headers = reqwest::header::HeaderMap::new();
            if n == 2 {
                headers.insert(
                    "retry-after",
                    reqwest::header::HeaderValue::from_static("1"),
                );
            }
            Ok(Response {
                status: if n < 2 {
                    500
                } else if n == 2 {
                    429
                } else {
                    200
                },
                url: request.url,
                headers,
                body: b"{\"data\":[]}".to_vec(),
            })
        })
    }
}
#[tokio::test]
async fn all_actual_factories_time_denied_and_unavailable_admission_without_http_or_helper() {
    for name in ["arxiv", "semanticscholar", "openalex"] {
        for decision in [AdmissionDecision::Denied, AdmissionDecision::Unavailable] {
            let pool = AtomPool::new(
                PathBuf::from("/nonexistent-admission-parser"),
                NonZeroUsize::new(1).unwrap(),
            )
            .unwrap();
            let http = Arc::new(AdapterHttp::default());
            let transport = Arc::new(Replies(AtomicUsize::new(0)));
            http.set_transport(transport.clone()).unwrap();
            let admission = Arc::new(SlowDecision(decision));
            let mut registry = ssx_registry::Registry::builtins().unwrap();
            match name {
                "arxiv" => arxiv_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    ArxivConfig::default(),
                    pool.clone(),
                )
                .unwrap(),
                "openalex" => openalex_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    OpenAlexConfig::default(),
                    pool.clone(),
                )
                .unwrap(),
                _ => semantic_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    SemanticConfig::default(),
                    pool.clone(),
                )
                .unwrap(),
            }
            assert!(registry.is_available(name));
            assert!(
                registry
                    .descriptor(name)
                    .unwrap()
                    .categories
                    .iter()
                    .any(|v| v == "science")
            );
            if name == "openalex" {
                let categories = &registry.descriptor(name).unwrap().categories;
                assert!(categories.iter().any(|v| v == "reference"));
                assert!(!categories.iter().any(|v| v == "general"));
            }
            if name == "arxiv" {
                assert_eq!(
                    ArxivConfig::default().base_url.as_ref(),
                    "https://export.arxiv.org/api/query"
                );
            }
            let result = registry
                .instantiate(name)
                .unwrap()
                .search_with_context(
                    &SearchRequest {
                        query: "admission".into(),
                        ..Default::default()
                    },
                    SearchContext {
                        deadline: Instant::now() + Duration::from_secs(5),
                    },
                )
                .await;
            assert_eq!(
                result.status,
                if decision == AdmissionDecision::Denied {
                    EngineStatus::RateLimited
                } else {
                    EngineStatus::Error
                }
            );
            assert!(result.latency_ms.get() >= 5.0, "{name}");
            assert!(!result.synthetic);
            assert!(result.results.is_empty());
            assert_eq!(transport.0.load(Ordering::SeqCst), 0);
            assert_eq!(pool.available_operations(), 1);
            assert_eq!(pool.available_slots(), 1);
            pool.shutdown().await.unwrap();
            http.shutdown_and_wait().await;
        }
    }
}
struct DelayedReply(ssx_json::Value);
impl Transport for DelayedReply {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            let reply = &self.0["responses"][0];
            if reply["exception"] == "timeout" {
                return Err(ssx_infra::http::Error::Deadline);
            }
            Ok(Response {
                status: reply["status"].as_u64().unwrap() as u16,
                url: request.url,
                headers: reqwest::header::HeaderMap::new(),
                body: base64::engine::general_purpose::STANDARD
                    .decode(reply["body_b64"].as_str().unwrap())
                    .unwrap(),
            })
        })
    }
}
#[tokio::test]
async fn actual_factories_measure_all_six_source_outcome_classes_without_synthetic_latency() {
    let source: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/source-initial.json"
    ))
    .unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for row in source["records"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        if ![
            "success",
            "empty",
            "http_429",
            "http_500",
            "malformed",
            "timeout",
        ]
        .iter()
        .any(|suffix| id.ends_with(&format!(".{suffix}")))
        {
            continue;
        }
        assert!(ids.insert(id));
        let name = row["engine"].as_str().unwrap();
        let pool = AtomPool::new(
            PathBuf::from(env!("CARGO_BIN_EXE_ssx-atom-parser")),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(Arc::new(DelayedReply(row.clone())))
            .unwrap();
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        let admission = Arc::new(ssx_infra::adapters::AllowAllAdmission);
        match name {
            "arxiv" => arxiv_runtime::register(
                &mut registry,
                http.clone(),
                admission,
                ArxivConfig::default(),
                pool.clone(),
            )
            .unwrap(),
            "openalex" => openalex_runtime::register(
                &mut registry,
                http.clone(),
                admission,
                OpenAlexConfig::default(),
                pool.clone(),
            )
            .unwrap(),
            _ => semantic_runtime::register(
                &mut registry,
                http.clone(),
                admission,
                SemanticConfig::default(),
                pool.clone(),
            )
            .unwrap(),
        }
        let out = registry
            .instantiate(name)
            .unwrap()
            .search_with_context(
                &SearchRequest {
                    query: row["query"].as_str().unwrap().into(),
                    ..Default::default()
                },
                SearchContext {
                    deadline: Instant::now() + Duration::from_secs(5),
                },
            )
            .await;
        assert_eq!(
            ssx_json::to_value(&out.status).unwrap(),
            row["output"]["status"],
            "{id}"
        );
        assert!(out.latency_ms.get() >= 5., "{id}");
        assert!(!out.synthetic);
        assert!(out.observed_latency_ms().is_some());
        assert_eq!(pool.available_operations(), 1);
        assert_eq!(pool.available_slots(), 1);
        pool.shutdown().await.unwrap();
        http.shutdown_and_wait().await;
    }
    assert_eq!(ids.len(), 18);
}
// Same private Unix-socket launch policy as accepted F10 tests; no environment URL.
struct Store {
    child: Child,
    dir: PathBuf,
    authority: Arc<Valkey>,
    url: String,
}
impl Store {
    fn start() -> Self {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ssx-a06-{}-{id}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let url = format!("unix://{}", dir.join("s.sock").display());
        let child = Command::new(
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
        .arg(&dir)
        .arg("--unixsocket")
        .arg(dir.join("s.sock"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
        let mut store = Self {
            child,
            dir,
            authority: Arc::new(Valkey::new(&url, Duration::from_millis(300)).unwrap()),
            url,
        };
        let until = Instant::now() + Duration::from_secs(4);
        while redis::Client::open(store.url.as_str())
            .unwrap()
            .get_connection()
            .is_err()
        {
            assert!(store.child.try_wait().unwrap().is_none());
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(10));
        }
        store
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
fn quota_snapshot(store: &Store) -> Vec<(String, String)> {
    let mut connection = redis::Client::open(store.url.as_str())
        .unwrap()
        .get_connection()
        .unwrap();
    let mut keys: Vec<String> = redis::cmd("KEYS")
        .arg("ratelimit:semanticscholar:*")
        .query(&mut connection)
        .unwrap();
    keys.sort();
    keys.into_iter()
        .map(|key| {
            let value: String = redis::cmd("GET").arg(&key).query(&mut connection).unwrap();
            (key, value)
        })
        .collect()
}
fn dependencies(
    engine: Arc<dyn SearchEngine>,
    cooldowns: Arc<dyn RateStore>,
) -> SearchDependencies {
    let capability = EngineCapability {
        name: "semanticscholar".into(),
        enabled: true,
        active: true,
        available: true,
        categories: vec!["science".into()],
        media_types: vec![],
        auth_ready: true,
        circuit_open: false,
        cost_class: "free".into(),
        health_status: "unknown".into(),
        health_stale: true,
        enforced_filters: BTreeMap::new(),
    };
    SearchDependencies {
        policy: PolicyService {
            policy: SearchPolicy::default(),
            catalog: CatalogSnapshot::new(vec![capability]).unwrap(),
            resolver: ScopeResolver {
                catalog_routing: false,
                ..Default::default()
            },
        },
        engines: BTreeMap::from([(
            "semanticscholar".into(),
            EngineBinding {
                engine,
                generation: "actual-a06".into(),
                timeout: Duration::from_secs(5),
                scrape: false,
            },
        )]),
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
        cooldowns: Some(cooldowns),
        owned_http: vec![],
        owned_stores: vec![],
        clock: Arc::new(SystemSearchClock),
        reranker: None,
    }
}
#[tokio::test]
async fn actual_semantic_factory_shared_f10_and_f14_cooldown_preserve_health_and_recover() {
    let store = Store::start();
    let quota = Arc::new(DistributedQuota::new(
        store.authority.clone(),
        QuotaConfig::new(100., 60.).unwrap(),
    ));
    let local = Arc::new(tokio::sync::Mutex::new(RateLimiter::new(
        quota.clone(),
        true,
    )));
    let remote = Arc::new(tokio::sync::Mutex::new(RateLimiter::new(
        quota.clone(),
        true,
    )));
    let admission = Arc::new(F10Admission::new(local.clone()));
    let remote_admission = F10Admission::new(remote.clone());
    let pool = AtomPool::new(
        PathBuf::from("/nonexistent-shared-semantic-helper"),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let http = Arc::new(AdapterHttp::default());
    let transport = Arc::new(Replies(AtomicUsize::new(0)));
    http.set_transport(transport.clone()).unwrap();
    let mut registry = ssx_registry::Registry::builtins().unwrap();
    semantic_runtime::register(
        &mut registry,
        http.clone(),
        admission,
        SemanticConfig::default(),
        pool.clone(),
    )
    .unwrap();
    let engine: Arc<dyn SearchEngine> = Arc::from(registry.instantiate("semanticscholar").unwrap());
    let service =
        SearchService::new(SearchConfig::default(), dependencies(engine, quota.clone())).unwrap();
    let proposal = || {
        ScopeProposal::generic(SearchRequest {
            query: "actual shared cooldown".into(),
            engines: Some(vec!["semanticscholar".into()]),
            freshness: "prefer_fresh".into(),
            ..Default::default()
        })
    };
    for _ in 0..2 {
        assert_eq!(
            service.search(proposal()).await.unwrap().engine_outcomes[0].status,
            "error"
        );
    }
    assert_eq!(service.health()["semanticscholar"].consecutive_errors, 2);
    let first = service.search(proposal()).await.unwrap();
    assert_eq!(first.engine_outcomes[0].status, "rate_limited");
    assert_eq!(transport.0.load(Ordering::SeqCst), 3);
    let health = service.health();
    assert_eq!(health["semanticscholar"].consecutive_errors, 2);
    let observed = ssx_application::health::build_engine_health(
        Some(&health["semanticscholar"]),
        None,
        ssx_domain::WallClockTimestamp::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
        )
        .unwrap(),
        300.,
    )
    .unwrap();
    assert_eq!(observed["status"], "rate_limited");
    let before = quota_snapshot(&store);
    assert!(!before.is_empty());
    assert_eq!(
        before
            .iter()
            .map(|(_, v)| v.parse::<u64>().unwrap())
            .sum::<u64>(),
        3
    );
    let key = EngineKey::new("semanticscholar").unwrap();
    assert_eq!(
        remote_admission.admit("semanticscholar").await,
        AdmissionDecision::Denied
    );
    assert_eq!(
        remote.lock().await.state(&key).unwrap().consecutive_denials,
        0
    );
    let second = service.search(proposal()).await.unwrap();
    assert_eq!(second.engine_outcomes[0].status, "rate_limited");
    assert!(
        second.engine_outcomes[0]
            .message
            .as_deref()
            .unwrap()
            .contains("cooldown")
    );
    assert_eq!(transport.0.load(Ordering::SeqCst), 3);
    assert_eq!(service.health()["semanticscholar"].consecutive_errors, 2);
    assert_eq!(
        quota_snapshot(&store),
        before,
        "remote and service cooldown skips must not acquire quota"
    );
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let recovered = service.search(proposal()).await.unwrap();
    assert_eq!(recovered.engine_outcomes[0].status, "ok");
    assert_eq!(transport.0.load(Ordering::SeqCst), 4);
    assert_eq!(service.health()["semanticscholar"].consecutive_errors, 0);
    assert_eq!(pool.available_operations(), 1);
    assert_eq!(pool.available_slots(), 1);
    service.shutdown().await;
    pool.shutdown().await.unwrap();
    http.shutdown_and_wait().await;
    store.authority.close().await;
}
