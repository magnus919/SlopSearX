//! F14 real execution composition. All sockets and Valkey state are private.
use ssx_application::{
    NoopSink, PortFuture, SearchEngine, cache::*, policy::*, scope::*, snapshot::*,
};
use ssx_domain::*;
use ssx_infra::{
    RecordingSink,
    http::*,
    search::*,
    suggest::SuggestionService,
    valkey::{Valkey, cache::ValkeyCache, snapshot::*},
};
use ssx_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::watch,
    task::JoinSet,
    time::Instant,
};

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

#[derive(Default)]
struct BoundaryCounts {
    quota: AtomicUsize,
    reads: AtomicUsize,
    writes: AtomicUsize,
}
impl ClientAdmission for BoundaryCounts {
    fn acquire<'a>(&'a self, _: &'a str) -> PortFuture<'a, bool> {
        Box::pin(async {
            self.quota.fetch_add(1, Ordering::SeqCst);
            true
        })
    }
}
impl CacheRepository for BoundaryCounts {
    fn read<'a>(
        &'a self,
        _: &'a str,
    ) -> PortFuture<'a, std::result::Result<Option<CacheEntry>, CacheFailure>> {
        Box::pin(async {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        })
    }
    fn write<'a>(
        &'a self,
        _: &'a str,
        _: &'a CacheEntry,
        _: std::num::NonZeroU64,
    ) -> PortFuture<'a, std::result::Result<(), CacheFailure>> {
        Box::pin(async {
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}
impl BoundaryCounts {
    fn assert_untouched(&self) {
        assert_eq!(self.quota.load(Ordering::SeqCst), 0);
        assert_eq!(self.reads.load(Ordering::SeqCst), 0);
        assert_eq!(self.writes.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn typed_unavailable_preserves_policy_precedence_without_side_effects() {
    for name in [None, Some("hibp")] {
        let engine = Arc::new(FixedEngine::new(name.unwrap_or("unused")));
        let mut deps = dependencies(
            name.map(|n| vec![(n, engine.clone() as Arc<dyn SearchEngine>)])
                .unwrap_or_default(),
        );
        let counts = Arc::new(BoundaryCounts::default());
        deps.client_admission = Some(counts.clone());
        deps.cache = Some(Arc::new(CanonicalCache::new(
            counts.clone(),
            CacheTtls::new(900, 60, 30).unwrap(),
        )));
        let service = SearchService::new(SearchConfig::default(), deps).unwrap();
        let mut p = proposal("availability");
        p.request.client_identifier = Some("blocked".into());
        for result in [
            service.search(p.clone()).await.map(|_| ()),
            service.prepare_scope(&p).await.map(|_| ()),
        ] {
            let Err(SearchError::Unavailable(unavailable)) = result else {
                panic!("expected typed unavailable");
            };
            assert_eq!(unavailable.reason, UnavailableReason::NoEligibleEngines);
            let corpus = fixture();
            let source = corpus["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| {
                    c["id"]
                        == if name.is_some() {
                            "availability-sensitive-only"
                        } else {
                            "availability-empty"
                        }
                })
                .unwrap();
            assert_eq!(
                ssx_json::to_value(&unavailable.preview.scope).unwrap(),
                source["output"]["scope"]
            );
        }
        p.request.engines = Some(vec!["hibp".into()]);
        assert!(matches!(
            service.search(p.clone()).await,
            Err(SearchError::Policy(_))
        ));
        assert!(matches!(
            service.prepare_scope(&p).await,
            Err(SearchError::Policy(_))
        ));
        counts.assert_untouched();
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        service.shutdown().await;
    }
}

#[tokio::test(start_paused = true)]
async fn optional_routing_timeout_falls_back_and_unavailable_implementation_is_typed() {
    let provider = Loopback::start(vec![json!({})], Duration::from_secs(1)).await;
    let config = ssx_infra::jev::JevConfig::from_environment(&BTreeMap::from([
        ("TYPESAFE_API_KEY".into(), "synthetic-key".into()),
        // Supplemental HTTP owner bound: this test advances only the shorter
        // service timer. Pausing Tokio does not stop the HTTP System clock.
        ("JEV_TIMEOUT_MS".into(), "60000".into()),
    ]))
    .unwrap();
    let engine = Arc::new(FixedEngine::new("brave"));
    let specialist = Arc::new(FixedEngine::new("pubmed"));
    let mut deps = dependencies(vec![("brave", engine.clone()), ("pubmed", specialist)]);
    deps.jev = Some(Arc::new(ssx_infra::jev::JevRouter::with_endpoint(
        config,
        ssx_registry::Registry::builtins().unwrap(),
        provider.endpoint.clone(),
    )));
    deps.policy.resolver.topics = Some(vec![]);
    let service = SearchService::new(
        SearchConfig {
            optional_port_timeout: Duration::from_millis(50),
            ..Default::default()
        },
        deps,
    )
    .unwrap();
    let request = proposal("routing timeout");
    let preparation = service.prepare_scope(&request);
    tokio::pin!(preparation);
    let setup = std::time::Instant::now();
    let frozen = Instant::now();
    // Keep a runnable yield branch while driving real loopback I/O. This
    // prevents automatic virtual-time advancement before the complete request
    // is recorded, instead of assuming 50 wall-clock milliseconds suffice.
    loop {
        tokio::select! {
            result = &mut preparation => panic!("routing completed before the request witness: {result:?}"),
            _ = tokio::task::yield_now() => {
                assert!(setup.elapsed() < Duration::from_secs(5), "loopback request setup exceeded its real bound");
                assert_eq!(Instant::now(), frozen);
                if provider.requests.lock().unwrap().len() == 1 {
                    break;
                }
            }
        }
    }
    tokio::time::advance(Duration::from_millis(50)).await;
    let prepared = preparation.await.unwrap();
    assert_eq!(Instant::now() - frozen, Duration::from_millis(50));
    assert!(prepared.preview().scope.jev_added_engines.is_empty());
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    service
        .search_prepared(prepared, SearchCancellation::default())
        .await
        .unwrap();
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    service.shutdown().await;
    provider.shutdown().await;
    let mut deps = dependencies(vec![("brave", engine.clone())]);
    let mut cap = capability("brave");
    cap.available = false;
    deps.policy.catalog = CatalogSnapshot::new(vec![cap]).unwrap();
    let counts = Arc::new(BoundaryCounts::default());
    deps.client_admission = Some(counts.clone());
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let mut p = proposal("unavailable");
    p.request.engines = Some(vec!["brave".into()]);
    assert!(matches!(
        service.search(p.clone()).await,
        Err(SearchError::Unavailable(SearchUnavailable {
            reason: UnavailableReason::NativeImplementationUnavailable,
            ..
        }))
    ));
    assert!(matches!(
        service.prepare_scope(&p).await,
        Err(SearchError::Unavailable(_))
    ));
    counts.assert_untouched();
    service.shutdown().await;
}

#[tokio::test]
async fn actual_amended_source_date_admission_and_proof() {
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f14/dates.json")).unwrap();
    for case in corpus["cases"].as_array().unwrap() {
        if case["kind"] == "cache" {
            continue;
        }
        let engine = Arc::new(FixedEngine::new("openalex"));
        let mut deps = dependencies(vec![("openalex", engine.clone())]);
        let mut cap = capability("openalex");
        cap.categories = vec!["science".into()];
        cap.enforced_filters = if case["layers"].is_null() {
            [
                ("date_from".into(), "upstream".into()),
                ("date_to".into(), "upstream".into()),
            ]
            .into()
        } else {
            ssx_json::from_value(case["layers"].clone()).unwrap()
        };
        deps.policy.catalog = CatalogSnapshot::new(vec![cap]).unwrap();
        let counts = Arc::new(BoundaryCounts::default());
        deps.client_admission = Some(counts.clone());
        deps.cache = Some(Arc::new(CanonicalCache::new(
            counts.clone(),
            CacheTtls::new(900, 60, 30).unwrap(),
        )));
        let service = SearchService::new(SearchConfig::default(), deps).unwrap();
        let params = &case["request"];
        let mut request = SearchRequest {
            engines: params
                .get("engines")
                .map(|v| ssx_json::from_value(v.clone()).unwrap()),
            date_from: params["date_from"].as_str().map(str::to_owned),
            date_to: params["date_to"].as_str().map(str::to_owned),
            time_range: params["time_range"].as_str().map(str::to_owned),
            media_type: params["media_type"].as_str().map(str::to_owned),
            ..Default::default()
        };
        request.query = "date".into();
        request.client_identifier = Some("client".into());
        let mut p = ScopeProposal::generic(request);
        p.surface = SearchSurface::Http;
        let result = service.search(p.clone()).await;
        if case["output"]["error"].is_string() {
            let Err(SearchError::Policy(error)) = result else {
                panic!("{} did not reject", case["id"]);
            };
            assert_eq!(
                error.field,
                case["output"]["field"].as_str().unwrap(),
                "{}",
                case["id"]
            );
            assert_eq!(
                error.message,
                case["output"]["message"].as_str().unwrap(),
                "{}",
                case["id"]
            );
            assert!(matches!(
                service.prepare_scope(&p).await,
                Err(SearchError::Policy(_))
            ));
            counts.assert_untouched();
            assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        } else {
            let response = result.unwrap();
            assert_eq!(
                response.date_filter.unwrap().to_value(),
                case["output"]["date_filter"],
                "{}",
                case["id"]
            );
            assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
        }
        service.shutdown().await;
    }
    let engine = Arc::new(FixedEngine::new("openalex"));
    let mut deps = dependencies(vec![("openalex", engine.clone())]);
    deps.policy.policy.grants.insert("science".into());
    let counts = Arc::new(BoundaryCounts::default());
    deps.client_admission = Some(counts.clone());
    deps.cache = Some(Arc::new(CanonicalCache::new(
        counts.clone(),
        CacheTtls::default(),
    )));
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    for surface in [
        SearchSurface::Http,
        SearchSurface::Generic,
        SearchSurface::Targeted,
        SearchSurface::Science,
        SearchSurface::Preview,
    ] {
        let mut p = proposal("date");
        p.surface = surface;
        p.request.engines = Some(vec!["openalex".into()]);
        p.request.date_from = Some("2025-01-01".into());
        p.request.time_range = Some(String::new());
        p.request.client_identifier = Some("client".into());
        for result in [
            service.search(p.clone()).await.map(|_| ()),
            service.prepare_scope(&p).await.map(|_| ()),
        ] {
            let Err(SearchError::Policy(error)) = result else {
                panic!("surface bypassed date constraint: {surface:?}");
            };
            assert_eq!(error.field, "time_range", "{surface:?}");
        }
    }
    counts.assert_untouched();
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    service.shutdown().await;
}

#[tokio::test]
async fn real_valkey_amended_date_proof_cache_admission() {
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(250)).unwrap());
    let engine = Arc::new(FixedEngine::new("openalex"));
    let mut deps = dependencies(vec![("openalex", engine.clone())]);
    let mut cap = capability("openalex");
    cap.enforced_filters = [
        ("date_from".into(), "upstream".into()),
        ("date_to".into(), "upstream".into()),
    ]
    .into();
    deps.policy.catalog = CatalogSnapshot::new(vec![cap]).unwrap();
    deps.cache = Some(Arc::new(CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    )));
    deps.owned_stores.push(store);
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let mut p = proposal("date");
    p.request.engines = Some(vec!["openalex".into()]);
    p.request.date_from = Some("2025-01-01".into());
    let initial = service.search(p.clone()).await.unwrap();
    let expected = initial.date_filter.as_ref().unwrap().to_value();
    let key = server.keys("search:*").await.pop().unwrap();
    let mut conn = redis::Client::open(server.url.as_str())
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    let original: String = redis::cmd("GET")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .unwrap();
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f14/dates.json")).unwrap();
    let mut calls = 1;
    for case in corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "cache")
    {
        let mutation = case["mutation"].as_str().unwrap();
        let mut payload: Value = ssx_json::from_str(&original).unwrap();
        match mutation {
            "unchanged" => {}
            "missing" => {
                payload.as_object_mut().unwrap().shift_remove("date_filter");
            }
            "requested" | "applied" => {
                payload["date_filter"][mutation]["date_from"] = json!("2024-01-01")
            }
            "enforced_by" => payload["date_filter"][mutation] = json!(["local:openalex"]),
            "engines" => payload["date_filter"][mutation] = json!(["brave"]),
            "scope" | "scope-self-consistent" => {
                payload["scope"]["selected_engines"] = json!(["brave"]);
                if mutation == "scope-self-consistent" {
                    payload["date_filter"]["engines"] = json!(["brave"]);
                }
            }
            _ => panic!("uncovered source mutation"),
        }
        let _: () = redis::cmd("SET")
            .arg(&key)
            .arg(ssx_json::to_string(&payload).unwrap())
            .query_async(&mut conn)
            .await
            .unwrap();
        let response = service.search(p.clone()).await.unwrap();
        let hit = mutation == "unchanged";
        assert_eq!(response.cached, hit, "{mutation}");
        if !hit {
            calls += 1;
        }
        assert_eq!(engine.calls.load(Ordering::SeqCst), calls, "{mutation}");
        assert_eq!(response.date_filter.unwrap().to_value(), expected);
        assert_eq!(response.scope.selected_engines, vec!["openalex"]);
        if mutation != "scope-self-consistent" {
            assert_eq!(case["cached"], json!(hit));
        } else {
            assert_eq!(case["cached"], json!(true)); /* explicit corrupt-source hardening */
        }
    }
    let mut legacy = SearchResponse::default();
    let old = storage::encode_response(&legacy).unwrap();
    assert!(old.get("date_filter").is_none());
    assert!(storage::decode_response(old).unwrap().date_filter.is_none());
    legacy.date_filter = initial.date_filter;
    let bytes = storage::encode_response(&legacy).unwrap();
    assert_eq!(
        storage::decode_response(bytes).unwrap().date_filter,
        legacy.date_filter
    );
    service.shutdown().await;
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
fn proposal(query: &str) -> ScopeProposal {
    ScopeProposal::generic(SearchRequest {
        query: query.into(),
        ..Default::default()
    })
}
struct FixedEngine {
    name: String,
    status: EngineStatus,
    calls: AtomicUsize,
    requests: Mutex<Vec<SearchRequest>>,
    delay: Duration,
    active: Arc<AtomicUsize>,
    count: usize,
    content: String,
    page_evidence: bool,
}
impl FixedEngine {
    fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            status: EngineStatus::Ok,
            calls: AtomicUsize::new(0),
            requests: Mutex::new(vec![]),
            delay: Duration::ZERO,
            active: Arc::new(AtomicUsize::new(0)),
            count: 3,
            content: "body".into(),
            page_evidence: false,
        }
    }
}
struct Active(Arc<AtomicUsize>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
struct StalledAdmission(Arc<AtomicUsize>);
impl ClientAdmission for StalledAdmission {
    fn acquire<'a>(&'a self, _: &'a str) -> PortFuture<'a, bool> {
        Box::pin(async {
            self.0.fetch_add(1, Ordering::SeqCst);
            let _active = Active(self.0.clone());
            std::future::pending().await
        })
    }
}
#[tokio::test]
async fn shutdown_cancels_and_joins_preflight_callers_before_returning() {
    let engine = Arc::new(FixedEngine::new("brave"));
    let active = Arc::new(AtomicUsize::new(0));
    let mut deps = dependencies(vec![("brave", engine.clone())]);
    deps.client_admission = Some(Arc::new(StalledAdmission(active.clone())));
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let mut p = proposal("preflight shutdown");
    p.request.client_identifier = Some("client".into());
    let copy = service.clone();
    let caller = tokio::spawn(async move { copy.search(p).await });
    until(|| active.load(Ordering::SeqCst) == 1).await;
    tokio::time::timeout(Duration::from_secs(1), service.shutdown())
        .await
        .unwrap();
    assert_eq!(active.load(Ordering::SeqCst), 0);
    assert_eq!(caller.await.unwrap(), Err(SearchError::Closed));
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    service.shutdown().await;
    let provider = Loopback::start(vec![json!({})], Duration::from_secs(10)).await;
    let config = ssx_infra::jev::JevConfig::from_environment(&BTreeMap::from([
        ("TYPESAFE_API_KEY".into(), "synthetic-key".into()),
        ("JEV_TIMEOUT_MS".into(), "5000".into()),
    ]))
    .unwrap();
    let specialist = Arc::new(FixedEngine::new("pubmed"));
    let mut deps = dependencies(vec![("brave", engine.clone()), ("pubmed", specialist)]);
    deps.policy.resolver.topics = Some(vec![]);
    deps.jev = Some(Arc::new(ssx_infra::jev::JevRouter::with_endpoint(
        config,
        ssx_registry::Registry::builtins().unwrap(),
        provider.endpoint.clone(),
    )));
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let copy = service.clone();
    let caller = tokio::spawn(async move {
        copy.prepare_scope(&proposal("routing shutdown"))
            .await
            .map(|_| ())
    });
    until(|| provider.requests.lock().unwrap().len() == 1).await;
    tokio::time::timeout(Duration::from_secs(1), async {
        tokio::join!(service.shutdown(), service.shutdown());
    })
    .await
    .unwrap();
    assert_eq!(caller.await.unwrap(), Err(SearchError::Closed));
    until(|| provider.disconnects.load(Ordering::SeqCst) == 1).await;
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    provider.shutdown().await;
}
impl SearchEngine for FixedEngine {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.requests.lock().unwrap().push(request.clone());
            self.active.fetch_add(1, Ordering::SeqCst);
            let _active = Active(self.active.clone());
            tokio::time::sleep(self.delay).await;
            AdapterResponse {
                results: if self.status == EngineStatus::Ok {
                    (0..self.count)
                        .map(|i| SearchResult {
                            url: format!("https://example.test/{}/{i}", self.name),
                            title: format!("{} {i}", self.name),
                            content: if self.page_evidence {
                                format!("page:{}", request.page)
                            } else {
                                self.content.clone()
                            },
                            engine: self.name.clone(),
                            ..Default::default()
                        })
                        .collect()
                } else {
                    vec![]
                },
                status: self.status.clone(),
                latency_ms: FiniteF64::new(5.0).unwrap(),
                error_message: (self.status != EngineStatus::Ok).then(|| "fixture failure".into()),
                answers: vec![json!({"answer":self.name})],
                corrections: vec![self.name.clone()],
                infoboxes: vec![json!({"name":self.name})],
                ..Default::default()
            }
        })
    }
}
fn fixture() -> Value {
    ssx_json::from_str(include_str!("../../xtask/fixtures/f14/source.json")).unwrap()
}
fn status(value: &str) -> EngineStatus {
    match value {
        "error" => EngineStatus::Error,
        "timeout" => EngineStatus::Timeout,
        "blocked" => EngineStatus::Blocked,
        "rate_limited" => EngineStatus::RateLimited,
        _ => EngineStatus::Ok,
    }
}
#[tokio::test]
async fn actual_source_service_replay() {
    for case in fixture()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "service")
    {
        let engines: Vec<_> = ["brave", "wikipedia"]
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let mut e = FixedEngine::new(name);
                e.status = status(case["statuses"][i].as_str().unwrap());
                Arc::new(e)
            })
            .collect();
        let service = SearchService::new(
            SearchConfig::default(),
            dependencies(vec![
                ("brave", engines[0].clone()),
                ("wikipedia", engines[1].clone()),
            ]),
        )
        .unwrap();
        let raw = &case["request"];
        let mut p = proposal(" café 🐙 ");
        if let Some(n) = raw.get("max_results") {
            p.request.max_results = n.as_u64();
        }
        if let Some(a) = raw.get("include") {
            p.request.include = a
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().to_owned())
                .collect();
        }
        if let Some(a) = raw.get("engines") {
            p.request.engines = Some(
                a.as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap().to_owned())
                    .collect(),
            );
        }
        if let Some(a) = raw.get("categories") {
            p.request.categories = Some(
                a.as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap().to_owned())
                    .collect(),
            );
        }
        if let Some(v) = raw.get("generate_suggestions") {
            p.request.generate_suggestions = v.as_bool().unwrap();
        }
        let mut response = service.search(p).await.unwrap();
        response.query_id = "<query-id>".into();
        response.response_time_ms = 0;
        let actual = storage::encode_response(&response).unwrap();
        assert_eq!(actual, case["output"], "{}", case["id"]);
        for (i, name) in ["brave", "wikipedia"].iter().enumerate() {
            assert_eq!(
                engines[i].calls.load(Ordering::SeqCst),
                case["calls"][*name].as_array().unwrap().len(),
                "{} {name}",
                case["id"]
            );
        }
        service.shutdown().await;
    }
}
#[test]
fn actual_source_suggestion_parser_and_key_replay() {
    for case in fixture()["cases"].as_array().unwrap() {
        match case["kind"].as_str().unwrap() {
            "suggest-key" => assert_eq!(
                ssx_infra::suggest::cache_key(case["query"].as_str().unwrap()),
                case["output"].as_str().unwrap()
            ),
            "suggest" => {
                let actual = ssx_infra::suggest::parse(&case["input"], case["provider"] == "brave");
                if case["input"]["results"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v.get("q").is_some_and(|q| !q.is_string())))
                {
                    assert!(actual.is_empty());
                    assert_ne!(json!(actual), case["output"]);
                } else {
                    assert_eq!(json!(actual), case["output"], "{}", case["id"]);
                }
            }
            _ => {}
        }
    }
}
async fn until(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !condition() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn singleflight_views_fresh_isolation_and_final_waiter_join() {
    let mut e = FixedEngine::new("brave");
    e.delay = Duration::from_millis(80);
    let engine = Arc::new(e);
    let service = SearchService::new(
        SearchConfig::default(),
        dependencies(vec![("brave", engine.clone())]),
    )
    .unwrap();
    let mut p = proposal("shared");
    p.request.max_results = Some(1);
    p.request.include.clear();
    let cancellation = SearchCancellation::default();
    let first_service = service.clone();
    let token = cancellation.clone();
    let first = tokio::spawn(async move { first_service.search_with_cancellation(p, token).await });
    until(|| engine.calls.load(Ordering::SeqCst) == 1).await;
    let second_service = service.clone();
    let second = tokio::spawn(async move { second_service.search(proposal("shared")).await });
    tokio::time::sleep(Duration::from_millis(10)).await;
    cancellation.cancel();
    assert_eq!(first.await.unwrap(), Err(SearchError::Cancelled));
    let response = second.await.unwrap().unwrap();
    assert_eq!(response.results.len(), 3);
    assert_eq!(response.engine_outcomes.len(), 1);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    let mut fresh = proposal("fresh");
    fresh.request.freshness = "prefer_fresh".into();
    let (a, b) = tokio::join!(service.search(fresh.clone()), service.search(fresh));
    assert_ne!(a.unwrap().query_id, b.unwrap().query_id);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 3);
    let mut isolated = proposal("workflow");
    isolated.request.execution_isolation_key = Some("one".into());
    let mut other = isolated.clone();
    other.request.execution_isolation_key = Some("two".into());
    let (a, b) = tokio::join!(service.search(isolated), service.search(other));
    assert!(a.is_ok() && b.is_ok());
    assert_eq!(engine.calls.load(Ordering::SeqCst), 5);
    let cancel = SearchCancellation::default();
    let s = service.clone();
    let token = cancel.clone();
    let last =
        tokio::spawn(async move { s.search_with_cancellation(proposal("cancel"), token).await });
    until(|| engine.active.load(Ordering::SeqCst) == 1).await;
    cancel.cancel();
    assert_eq!(last.await.unwrap(), Err(SearchError::Cancelled));
    assert_eq!(engine.active.load(Ordering::SeqCst), 0);
    let ((), ()) = tokio::join!(service.shutdown(), service.shutdown());
    service.shutdown().await;
    assert_eq!(
        service.search(proposal("closed")).await,
        Err(SearchError::Closed)
    );
}
#[tokio::test]
async fn simultaneous_views_ids_and_bounded_admission() {
    let mut e = FixedEngine::new("brave");
    e.delay = Duration::from_millis(40);
    let engine = Arc::new(e);
    let service = SearchService::new(
        SearchConfig {
            max_callers: 2,
            max_flights: 1,
            ..Default::default()
        },
        dependencies(vec![("brave", engine.clone())]),
    )
    .unwrap();
    let s = service.clone();
    let first = tokio::spawn(async move {
        let mut p = proposal("x");
        p.request.include.clear();
        p.request.max_results = Some(1);
        s.search(p).await
    });
    until(|| engine.active.load(Ordering::SeqCst) == 1).await;
    assert_eq!(
        service.search(proposal("different-flight")).await,
        Err(SearchError::Busy)
    );
    let second = service.search(proposal(" x ")).await.unwrap();
    let first = first.await.unwrap().unwrap();
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    assert_eq!(first.results.len(), 1);
    assert!(first.engine_outcomes.is_empty());
    assert_eq!(second.results.len(), 3);
    assert_ne!(first.query_id, second.query_id);
    assert_eq!(first.query, "x");
    assert_eq!(second.query, " x ");
    service.shutdown().await;
}

struct Loopback {
    endpoint: String,
    requests: Arc<Mutex<Vec<String>>>,
    disconnects: Arc<AtomicUsize>,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}
impl Loopback {
    async fn start(bodies: Vec<Value>, delay: Duration) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/search", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(vec![]));
        let calls = requests.clone();
        let disconnects = Arc::new(AtomicUsize::new(0));
        let closed = disconnects.clone();
        let (stop, mut rx) = watch::channel(false);
        let bodies = Arc::new(bodies);
        let task = tokio::spawn(async move {
            let mut children = JoinSet::new();
            loop {
                tokio::select! { _ = rx.changed() => break, accepted = listener.accept() => {
                    let (mut socket, _) = accepted.unwrap(); let calls = calls.clone(); let bodies = bodies.clone(); let closed = closed.clone();
                    children.spawn(async move { let mut bytes = vec![]; let mut buf = [0; 4096]; loop { let Ok(n) = socket.read(&mut buf).await else { return; }; if n == 0 { return; } bytes.extend_from_slice(&buf[..n]); if bytes.windows(4).any(|b| b == b"\r\n\r\n") { break; } assert!(bytes.len() < 128_000); }
                        let header_end = bytes.windows(4).position(|b| b == b"\r\n\r\n").unwrap() + 4;
                        let length: usize = String::from_utf8_lossy(&bytes[..header_end]).lines().find_map(|line| line.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap())).unwrap_or(0);
                        while bytes.len() < header_end + length { let n = socket.read(&mut buf).await.unwrap(); if n == 0 { return; } bytes.extend_from_slice(&buf[..n]); assert!(bytes.len() < 128_000); }
                        let index = { let mut calls = calls.lock().unwrap(); let index = calls.len(); calls.push(String::from_utf8(bytes).unwrap()); index };
                        let mut probe = [0; 1]; tokio::select! { _ = tokio::time::sleep(delay) => {}, result = socket.read(&mut probe) => { if matches!(result, Ok(0) | Err(_)) { closed.fetch_add(1, Ordering::SeqCst); } return; } }
                        let body = ssx_json::to_vec(&bodies[index.min(bodies.len() - 1)]).unwrap(); let head = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()); let _ = socket.write_all(head.as_bytes()).await; let _ = socket.write_all(&body).await;
                    });
                }, _ = children.join_next(), if !children.is_empty() => {} }
            }
            children.shutdown().await;
        });
        Self {
            endpoint,
            requests,
            disconnects,
            stop,
            task,
        }
    }
    async fn shutdown(self) {
        self.stop.send_replace(true);
        self.task.await.unwrap();
    }
}
struct MultiRequestEngine {
    http: Arc<AdapterHttp>,
    endpoint: String,
    rounds: usize,
    active: Arc<AtomicUsize>,
    requests: Mutex<Vec<SearchRequest>>,
}
impl SearchEngine for MultiRequestEngine {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            self.active.fetch_add(1, Ordering::SeqCst);
            let _active = Active(self.active.clone());
            self.requests.lock().unwrap().push(request.clone());
            let mut session = self
                .http
                .session(
                    Options {
                        trust_env: false,
                        ..Default::default()
                    },
                    Instant::now() + Duration::from_secs(1),
                    Cancellation::default(),
                )
                .unwrap();
            for _ in 0..self.rounds {
                session.set_deadline(Instant::now() + Duration::from_secs(1));
                if session
                    .execute(Request::get(&self.endpoint).unwrap())
                    .await
                    .is_err()
                {
                    return AdapterResponse {
                        status: EngineStatus::Error,
                        ..Default::default()
                    };
                }
            }
            AdapterResponse {
                results: vec![SearchResult {
                    url: "https://example.test/multi".into(),
                    title: "multi".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }
        })
    }
}
#[tokio::test]
async fn actual_loopback_multirequest_deadline_queue_and_shutdown() {
    let server = Loopback::start(vec![json!({"ok":true})], Duration::from_millis(100)).await;
    let http = Arc::new(AdapterHttp::default());
    let engine = Arc::new(MultiRequestEngine {
        http: http.clone(),
        endpoint: server.endpoint.clone(),
        rounds: 4,
        active: Arc::new(AtomicUsize::new(0)),
        requests: Mutex::new(vec![]),
    });
    // Initialize the real socket/TLS client before measuring the multi-fetch budget.
    let mut warm = http
        .session(
            Options {
                trust_env: false,
                ..Default::default()
            },
            Instant::now() + Duration::from_secs(2),
            Cancellation::default(),
        )
        .unwrap();
    warm.execute(Request::get(&server.endpoint).unwrap())
        .await
        .unwrap();
    drop(warm);
    server.requests.lock().unwrap().clear();
    let sink = Arc::new(RecordingSink::default());
    let mut deps = dependencies(vec![
        ("brave", engine.clone()),
        ("wikipedia", engine.clone()),
    ]);
    deps.owned_http.push(http.clone());
    deps.metrics = sink.clone();
    deps.supervisor = sink.clone();
    let service = SearchService::new(
        SearchConfig {
            max_parallel_engines: 1,
            ..Default::default()
        },
        deps,
    )
    .unwrap();
    let mut p = proposal("deadline");
    p.request.interactive_timeout_ms = Some(180);
    let response = service.search(p).await.unwrap();
    assert!(response.deadline_exceeded && response.all_unresponsive);
    assert_eq!(response.dispatched_engine_count, 1);
    assert_eq!(
        response
            .engine_outcomes
            .iter()
            .map(|o| o.status.as_str())
            .collect::<Vec<_>>(),
        ["unavailable", "unavailable"]
    );
    assert_eq!(engine.active.load(Ordering::SeqCst), 0);
    assert_eq!(server.requests.lock().unwrap().len(), 2);
    for state in service.health().values() {
        let health = ssx_application::health::build_engine_health(
            Some(state),
            None,
            WallClockTimestamp::new(1_000_000_000.0).unwrap(),
            300.0,
        )
        .unwrap();
        assert_eq!(health["status"], "unknown");
    }
    assert!(!sink.0.lock().unwrap().contains(&"search_engine_observed"));
    service.shutdown().await;
    service.shutdown().await;
    assert!(matches!(
        http.session(
            Options::default(),
            Instant::now() + Duration::from_secs(1),
            Cancellation::default()
        ),
        Err(Error::Closed)
    ));
    server.shutdown().await;
}
#[tokio::test]
async fn actual_registered_npm_adapter_executes_through_service() {
    let server = Loopback::start(vec![json!({"objects":[{"package":{"name":"fixture","version":"1","description":"package"},"score":{"final":0.5}}]})], Duration::ZERO).await;
    let http = Arc::new(AdapterHttp::default());
    let mut registry = ssx_registry::Registry::builtins().unwrap();
    ssx_infra::adapters::register_pilots(
        &mut registry,
        http.clone(),
        Arc::new(ssx_infra::adapters::AllowAllAdmission),
        ssx_infra::adapters::PilotConfig {
            npm: ssx_infra::adapters::NpmConfig {
                base_url: server.endpoint.clone(),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    let mut deps = dependencies(vec![(
        "npm",
        Arc::from(registry.instantiate("npm").unwrap()),
    )]);
    deps.owned_http.push(http);
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let response = service.search(proposal("fixture")).await.unwrap();
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].engine, "npm");
    assert_eq!(response.results[0].tier, 2);
    assert_eq!(server.requests.lock().unwrap().len(), 1);
    assert!(server.requests.lock().unwrap()[0].contains("text=fixture"));
    service.shutdown().await;
    server.shutdown().await;
}
struct PrivateValkey {
    child: std::process::Child,
    dir: std::path::PathBuf,
    url: String,
}
impl PrivateValkey {
    async fn start() -> Self {
        let mut random = [0; 8];
        aws_lc_rs::rand::fill(&mut random).unwrap();
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("ssx-f14-{suffix}"));
        std::fs::create_dir(&dir).unwrap();
        let child = std::process::Command::new(
            std::env::var("SLOPSEARX_TEST_VALKEY_SERVER")
                .unwrap_or_else(|_| "valkey-server".into()),
        )
        .args([
            "--port",
            "0",
            "--save",
            "",
            "--appendonly",
            "no",
            "--unixsocketperm",
            "700",
            "--dir",
        ])
        .arg(&dir)
        .arg("--unixsocket")
        .arg(dir.join("s.sock"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
        let server = Self {
            child,
            url: format!("unix://{}", dir.join("s.sock").display()),
            dir,
        };
        let store = Valkey::new(&server.url, Duration::from_millis(100)).unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while store.ping().await.is_err() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        store.close().await;
        server
    }
    async fn keys(&self, prefix: &str) -> Vec<String> {
        let mut c = redis::Client::open(self.url.as_str())
            .unwrap()
            .get_multiplexed_async_connection()
            .await
            .unwrap();
        redis::cmd("KEYS")
            .arg(prefix)
            .query_async(&mut c)
            .await
            .unwrap()
    }
    async fn ttl(&self, key: &str) -> i64 {
        let mut c = redis::Client::open(self.url.as_str())
            .unwrap()
            .get_multiplexed_async_connection()
            .await
            .unwrap();
        redis::cmd("TTL")
            .arg(key)
            .query_async(&mut c)
            .await
            .unwrap()
    }
}
impl Drop for PrivateValkey {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
#[tokio::test]
async fn real_valkey_cache_views_partial_deadline_and_snapshot_composition() {
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(250)).unwrap());
    let cache = Arc::new(CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    ));
    let engine = Arc::new(FixedEngine::new("brave"));
    let mut deps = dependencies(vec![("brave", engine.clone())]);
    deps.cache = Some(cache);
    deps.owned_stores.push(store.clone());
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let snapshots = SnapshotService::new(
        Arc::new(ValkeySnapshots::new(store.clone())),
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        TenantId::new("f14-private").unwrap(),
        SnapshotTtl::new(60).unwrap(),
    );
    let mut p = proposal("cache-full");
    p.request.max_results = Some(1);
    p.request.include.clear();
    let captured = service.search_snapshot(p, &snapshots, &[]).await.unwrap();
    assert_eq!(captured.response.results.len(), 1);
    assert_eq!(captured.canonical.response().results.len(), 3);
    let id = captured.snapshot.unwrap();
    let SnapshotLookup::Found(snapshot) = snapshots.read(&id).await else {
        panic!("snapshot not stored");
    };
    assert_eq!(snapshot.total(), 3);
    let next = service.search(proposal("cache-full")).await.unwrap();
    assert!(next.cached);
    assert_eq!(next.results.len(), 3);
    assert_eq!(next.engine_outcomes.len(), 1);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    assert_ne!(next.query_id, captured.response.query_id);
    let keys = server.keys("search:native:v1:*").await;
    assert_eq!(keys.len(), 1);
    assert!((3590..=3600).contains(&server.ttl(&keys[0]).await));
    service.shutdown().await;
    assert!(store.ping().await.is_err());
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(250)).unwrap());
    let mut failed = FixedEngine::new("wikipedia");
    failed.status = EngineStatus::Error;
    let failed = Arc::new(failed);
    let mut deps = dependencies(vec![("brave", engine.clone()), ("wikipedia", failed)]);
    deps.cache = Some(Arc::new(CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    )));
    let partial = SearchService::new(SearchConfig::default(), deps).unwrap();
    let result = partial.search(proposal("partial-cache")).await.unwrap();
    assert!(result.partial && !result.all_unresponsive);
    let keys = server.keys("search:native:v1:*").await;
    assert_eq!(keys.len(), 2);
    let mut partial_ttl = false;
    for key in keys {
        partial_ttl |= (20..=30).contains(&server.ttl(&key).await);
    }
    assert!(partial_ttl);
    partial.shutdown().await;
    let mut slow = FixedEngine::new("brave");
    slow.delay = Duration::from_secs(1);
    let mut deps = dependencies(vec![("brave", Arc::new(slow))]);
    deps.cache = Some(Arc::new(CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    )));
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let mut p = proposal("truncated");
    p.request.interactive_timeout_ms = Some(10);
    assert!(service.search(p).await.unwrap().deadline_exceeded);
    assert_eq!(server.keys("search:native:v1:*").await.len(), 2);
    service.shutdown().await;
    store.close().await;
}
#[tokio::test]
async fn real_valkey_optional_failure_and_oversized_evidence_stay_complete() {
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(100)).unwrap());
    let mut large = FixedEngine::new("brave");
    large.count = 80;
    large.content = "x".repeat(60_000);
    let engine = Arc::new(large);
    let mut deps = dependencies(vec![("brave", engine.clone())]);
    deps.cache = Some(Arc::new(CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    )));
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let snapshots = SnapshotService::new(
        Arc::new(ValkeySnapshots::new(store.clone())),
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        TenantId::new("oversized").unwrap(),
        SnapshotTtl::new(60).unwrap(),
    );
    let mut p = proposal("oversized");
    p.request.max_results = Some(1);
    let captured = service.search_snapshot(p, &snapshots, &[]).await.unwrap();
    assert_eq!(captured.response.results.len(), 1);
    assert_eq!(captured.canonical.response().results.len(), 80);
    assert!(captured.snapshot.is_err());
    assert!(server.keys("search:native:v1:*").await.is_empty());
    store.close().await;
    let result = service.search(proposal("store-closed")).await.unwrap();
    assert_eq!(result.results.len(), 80);
    assert!(!result.cached);
    service.shutdown().await;
}
#[tokio::test]
async fn real_valkey_client_authority_precedes_cache_and_denied_dispatch() {
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(250)).unwrap());
    let quota = Arc::new(ssx_infra::valkey::rate::DistributedQuota::new(
        store.clone(),
        ssx_domain::rate::QuotaConfig::new(1.0, 86_400.0).unwrap(),
    ));
    let engine = Arc::new(FixedEngine::new("brave"));
    let sensitive = Arc::new(FixedEngine::new("hibp"));
    let mut deps = dependencies(vec![("brave", engine.clone()), ("hibp", sensitive.clone())]);
    deps.client_admission = Some(Arc::new(DistributedClientAdmission {
        quota,
        fail_closed: true,
    }));
    deps.cache = Some(Arc::new(CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    )));
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let mut p = proposal("quota");
    p.request.client_identifier = Some("private-client".into());
    p.request.engines = Some(vec!["brave".into()]);
    assert!(!service.search(p.clone()).await.unwrap().cached);
    assert_eq!(service.search(p).await, Err(SearchError::ClientLimited));
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    let mut denied = proposal("quota");
    denied.request.engines = Some(vec!["brave".into(), "hibp".into()]);
    assert!(matches!(
        service.search(denied).await,
        Err(SearchError::Policy(_))
    ));
    let mut strict = proposal("quota");
    strict.request.safesearch = 2;
    assert!(matches!(
        service.search(strict).await,
        Err(SearchError::Policy(_))
    ));
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    assert_eq!(sensitive.calls.load(Ordering::SeqCst), 0);
    let keys = server.keys("ratelimit:client:*").await;
    assert_eq!(keys.len(), 1);
    assert!(!keys[0].contains("private-client"));
    service.shutdown().await;
    store.close().await;
}
#[tokio::test]
async fn native_suggestions_loopback_fallback_cache_malformed_and_cancellation() {
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(250)).unwrap());
    let brave = Loopback::start(
        vec![json!({"results":[{"q":"valid"},{"q":null}]})],
        Duration::ZERO,
    )
    .await;
    let ddg = Loopback::start(
        vec![json!(["query", ["fallback", "second"]])],
        Duration::ZERO,
    )
    .await;
    let suggestions = Arc::new(SuggestionService::with_endpoints(
        "synthetic-key".into(),
        Some(store.clone()),
        brave.endpoint.clone(),
        ddg.endpoint.clone(),
    ));
    assert_eq!(suggestions.fetch("Query").await, ["fallback", "second"]);
    assert_eq!(suggestions.fetch(" query ").await, ["fallback", "second"]);
    assert_eq!(brave.requests.lock().unwrap().len(), 1);
    assert_eq!(ddg.requests.lock().unwrap().len(), 1);
    assert!(
        brave.requests.lock().unwrap()[0]
            .to_lowercase()
            .contains("x-subscription-token: synthetic-key")
    );
    let key = ssx_infra::suggest::cache_key("query");
    assert!((1790..=1800).contains(&server.ttl(&key).await));
    suggestions.shutdown().await;
    brave.shutdown().await;
    ddg.shutdown().await;
    let slow = Loopback::start(vec![json!(["query", ["late"]])], Duration::from_secs(1)).await;
    let engine = Arc::new(FixedEngine::new("brave"));
    let mut deps = dependencies(vec![("brave", engine.clone())]);
    deps.suggestions = Some(Arc::new(SuggestionService::with_endpoints(
        String::new(),
        None,
        "unused".into(),
        slow.endpoint.clone(),
    )));
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let mut p = proposal("only-source-suggestions");
    p.request.interactive_timeout_ms = Some(100);
    let response = service.search(p).await.unwrap();
    assert!(response.deadline_exceeded && response.partial);
    assert_eq!(response.results.len(), 3);
    assert!(response.suggestions.is_empty());
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    service.shutdown().await;
    slow.shutdown().await;
    store.close().await;
}

#[test]
fn local_calendar_subprocess_probe() {
    let Ok(epoch) = std::env::var("SSX_F14_CALENDAR_EPOCH") else {
        return;
    };
    let expected = std::env::var("SSX_F14_CALENDAR_EXPECTED").unwrap();
    let (wall, date) =
        SystemSearchClock::at(std::time::UNIX_EPOCH + Duration::from_secs(epoch.parse().unwrap()));
    assert_eq!(date, expected);
    assert_eq!(wall.seconds(), epoch.parse::<f64>().unwrap());
}
#[test]
fn actual_source_local_midnight_and_dst_calendar() {
    let current = std::env::current_exe().unwrap();
    let mut count = 0;
    for case in fixture()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "calendar")
    {
        let output = std::process::Command::new(&current)
            .args(["--exact", "local_calendar_subprocess_probe", "--nocapture"])
            .env("TZ", case["zone"].as_str().unwrap())
            .env("SSX_F14_CALENDAR_EPOCH", case["epoch"].to_string())
            .env("SSX_F14_CALENDAR_EXPECTED", case["date"].as_str().unwrap())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            case["id"],
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed; 0 ignored;"));
        count += 1;
    }
    assert_eq!(count, 8);
}
#[tokio::test]
async fn actual_jev_prepared_scope_reuses_advice_and_source_promotion() {
    let case = fixture()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["kind"] == "jev-service")
        .unwrap()
        .clone();
    let provider = Loopback::start(
        vec![json!({"model":ssx_infra::jev::MODEL,"answers":{"pubmed":{"noul":0.9}}})],
        Duration::ZERO,
    )
    .await;
    let config = ssx_infra::jev::JevConfig::from_environment(&BTreeMap::from([(
        "TYPESAFE_API_KEY".into(),
        "synthetic-key".into(),
    )]))
    .unwrap();
    let router = Arc::new(ssx_infra::jev::JevRouter::with_endpoint(
        config,
        ssx_registry::Registry::builtins().unwrap(),
        provider.endpoint.clone(),
    ));
    let brave = Arc::new(FixedEngine::new("brave"));
    let pubmed = Arc::new(FixedEngine::new("pubmed"));
    let mut deps = dependencies(vec![("brave", brave.clone()), ("pubmed", pubmed.clone())]);
    deps.jev = Some(router);
    deps.policy.resolver.topics = Some(vec![]);
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let authority = service.prepare_scope(&proposal(" café 🐙 ")).await.unwrap();
    assert_eq!(authority.preview().scope.jev_added_engines, ["pubmed"]);
    let mut response = service
        .search_prepared(authority, SearchCancellation::default())
        .await
        .unwrap();
    response.query_id = "<query-id>".into();
    response.response_time_ms = 0;
    assert_eq!(storage::encode_response(&response).unwrap(), case["output"]);
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    assert_eq!(brave.calls.load(Ordering::SeqCst), 1);
    assert_eq!(pubmed.calls.load(Ordering::SeqCst), 1);
    service.shutdown().await;
    provider.shutdown().await;
}
#[tokio::test]
async fn actual_configured_slowest_timeout_survives_ten_second_floor() {
    let provider = Loopback::start(
        vec![json!({"objects":[{"package":{"name":"slow","version":"1"}}]})],
        Duration::from_millis(10_100),
    )
    .await;
    let http = Arc::new(AdapterHttp::default());
    let mut registry = ssx_registry::Registry::builtins().unwrap();
    ssx_infra::adapters::register_pilots(
        &mut registry,
        http.clone(),
        Arc::new(ssx_infra::adapters::AllowAllAdmission),
        ssx_infra::adapters::PilotConfig {
            npm: ssx_infra::adapters::NpmConfig {
                base_url: provider.endpoint.clone(),
                timeout_ms: 11_000,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    let mut deps = dependencies(vec![(
        "npm",
        Arc::from(registry.instantiate("npm").unwrap()),
    )]);
    deps.engines.get_mut("npm").unwrap().timeout = Duration::from_secs(11);
    deps.owned_http.push(http);
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let response = service
        .search(proposal("slow-native-adapter"))
        .await
        .unwrap();
    assert!(!response.all_unresponsive && !response.deadline_exceeded);
    assert_eq!(response.results.len(), 1);
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    service.shutdown().await;
    provider.shutdown().await;
}
#[tokio::test]
async fn real_cooldown_and_observed_circuit_skip_dispatch() {
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(250)).unwrap());
    let quota = Arc::new(ssx_infra::valkey::rate::DistributedQuota::new(
        store.clone(),
        ssx_domain::rate::QuotaConfig::new(10.0, 60.0).unwrap(),
    ));
    let key = ssx_domain::rate::EngineKey::new("brave").unwrap();
    quota.publish_cooldown(&key, 60.0).await.unwrap();
    let engine = Arc::new(FixedEngine::new("brave"));
    let mut deps = dependencies(vec![("brave", engine.clone())]);
    deps.cooldowns = Some(quota);
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let response = service.search(proposal("cooldown")).await.unwrap();
    assert_eq!(response.engine_outcomes[0].status, "rate_limited");
    assert_eq!(response.dispatched_engine_count, 0);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    service.shutdown().await;
    let mut error = FixedEngine::new("brave");
    error.status = EngineStatus::Error;
    let engine = Arc::new(error);
    let sink = Arc::new(RecordingSink::default());
    let mut deps = dependencies(vec![("brave", engine.clone())]);
    deps.metrics = sink.clone();
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    for n in 0..5 {
        assert!(
            service
                .search(proposal(&format!("failure-{n}")))
                .await
                .unwrap()
                .all_unresponsive
        );
    }
    let response = service.search(proposal("open-circuit")).await.unwrap();
    assert_eq!(response.dispatched_engine_count, 0);
    assert_eq!(
        response.engine_outcomes[0].message.as_deref(),
        Some("circuit open")
    );
    assert_eq!(engine.calls.load(Ordering::SeqCst), 5);
    assert_eq!(service.health()["brave"].consecutive_errors, 5);
    assert_eq!(
        sink.0
            .lock()
            .unwrap()
            .iter()
            .filter(|n| **n == "search_engine_observed")
            .count(),
        5
    );
    service.shutdown().await;
    store.close().await;
}
struct Stage {
    state: RerankState,
    calls: AtomicUsize,
}
impl RerankStage for Stage {
    fn rerank<'a>(
        &'a self,
        _: &'a SearchRequest,
        results: &'a [SearchResult],
        _: Instant,
    ) -> PortFuture<'a, std::result::Result<RerankDecision, ()>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let mut results = results.to_vec();
            if self.state == RerankState::Applied {
                results.reverse();
            }
            Ok(RerankDecision {
                results,
                state: self.state,
            })
        })
    }
}
#[tokio::test]
async fn downstream_rerank_stage_cache_admission_and_sensitive_guard() {
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(250)).unwrap());
    let engine = Arc::new(FixedEngine::new("brave"));
    for state in [RerankState::Fallback, RerankState::Applied] {
        let stage = Arc::new(Stage {
            state,
            calls: AtomicUsize::new(0),
        });
        let mut deps = dependencies(vec![("brave", engine.clone())]);
        deps.cache = Some(Arc::new(CanonicalCache::new(
            Arc::new(ValkeyCache::new(store.clone())),
            CacheTtls::default(),
        )));
        deps.reranker = Some(stage.clone());
        let service = SearchService::new(SearchConfig::default(), deps).unwrap();
        let query = format!("stage-{state:?}");
        let response = service.search(proposal(&query)).await.unwrap();
        let again = service.search(proposal(&query)).await.unwrap();
        assert_eq!(again.cached, state == RerankState::Applied);
        assert_eq!(
            stage.calls.load(Ordering::SeqCst),
            if state == RerankState::Applied { 1 } else { 2 }
        );
        if state == RerankState::Applied {
            assert_eq!(response.results[0].title, "brave 2");
            assert_eq!(response.results[0].position, 1);
            assert_eq!(response.ranking_explanation, "semantic_shortlist_rerank");
        }
        service.shutdown().await;
    }
    let stage = Arc::new(Stage {
        state: RerankState::Applied,
        calls: AtomicUsize::new(0),
    });
    let engine = Arc::new(FixedEngine::new("hibp"));
    let mut deps = dependencies(vec![("hibp", engine)]);
    deps.policy.policy.sensitive_allowed = true;
    deps.reranker = Some(stage.clone());
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let mut p = proposal("sensitive");
    p.request.engines = Some(vec!["hibp".into()]);
    p.surface = SearchSurface::Targeted;
    let response = service.search(p).await.unwrap();
    assert_eq!(response.results.len(), 3);
    assert_eq!(stage.calls.load(Ordering::SeqCst), 0);
    service.shutdown().await;
    store.close().await;
}
#[tokio::test]
async fn dedicated_client_quota_namespace_exact_keys_and_failures() {
    use ssx_infra::valkey::rate::{ClientQuotaKey, DistributedQuota};
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(100)).unwrap());
    let config = ssx_domain::rate::QuotaConfig::new(2.0, 86_400.0).unwrap();
    let quota = Arc::new(DistributedQuota::new(store.clone(), config));
    assert!(matches!(
        ClientQuotaKey::new(""),
        Err(ssx_domain::rate::RateFailure::InvalidInput)
    ));
    for client in ["2001:db8::1", "tenant:alice/a?b=1", " "] {
        let key = ClientQuotaKey::new(client).unwrap();
        let same = ClientQuotaKey::new(client).unwrap();
        assert_eq!(key.digest(), same.digest());
        let provider =
            ssx_domain::rate::EngineKey::new(&format!("client-{}", key.digest())).unwrap();
        let first = quota.acquire_client(&key, 1).await.unwrap();
        let second = quota.acquire_client(&same, 1).await.unwrap();
        let denied = quota.acquire_client(&key, 1).await.unwrap();
        let engine = quota.acquire(&provider, 1).await.unwrap();
        assert_eq!(
            (first.count, second.count, denied.count, engine.count),
            (1, 2, 3, 1)
        );
        assert!(first.allowed && second.allowed && !denied.allowed && engine.allowed);
        assert_eq!(
            first.bucket_key,
            format!(
                "ratelimit:client:{}:{}",
                key.digest(),
                first.server_time.milliseconds() / config.window_ms()
            )
        );
        assert_eq!(
            engine.bucket_key,
            format!(
                "ratelimit:{}:{}",
                provider.as_str(),
                engine.server_time.milliseconds() / config.window_ms()
            )
        );
        assert_ne!(first.bucket_key, engine.bucket_key);
        assert!(!first.bucket_key.contains(client));
        assert!((172_790..=172_800).contains(&server.ttl(&first.bucket_key).await));
    }
    assert_eq!(server.keys("ratelimit:client:*").await.len(), 3);
    let open = DistributedClientAdmission {
        quota: quota.clone(),
        fail_closed: false,
    };
    let closed = DistributedClientAdmission {
        quota,
        fail_closed: true,
    };
    store.close().await;
    // Terminal close can never become fail-open admission.
    assert!(!open.acquire("closed").await && !closed.acquire("closed").await);
    let unavailable =
        Arc::new(Valkey::new("redis://127.0.0.1:1/0", Duration::from_millis(50)).unwrap());
    let quota = Arc::new(DistributedQuota::new(unavailable.clone(), config));
    let open = DistributedClientAdmission {
        quota: quota.clone(),
        fail_closed: false,
    };
    let closed = DistributedClientAdmission {
        quota,
        fail_closed: true,
    };
    assert!(open.acquire("unavailable").await);
    assert!(!closed.acquire("unavailable").await);
    unavailable.close().await;
}
#[test]
fn actual_source_environment_settings_and_explicit_finite_boundaries() {
    let mut count = 0;
    for case in fixture()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "settings")
    {
        let env = case["input"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
            .collect();
        let settings = SearchSettings::from_environment(&env, "presence");
        let output = &case["output"];
        let native_invalid = output["rate"].as_f64().is_none_or(|r| r < 0.0)
            || output["window"].as_f64().is_none_or(|w| w < 0.001)
            || output["max_parallel_engines"]
                .as_str()
                .unwrap()
                .parse::<usize>()
                .is_err();
        if native_invalid {
            assert!(
                matches!(settings, Err(SearchError::InvalidConfiguration)),
                "{}",
                case["id"]
            );
        } else {
            let settings = settings.unwrap();
            assert_eq!(
                settings.execution.max_parallel_engines.to_string(),
                output["max_parallel_engines"].as_str().unwrap()
            );
            assert_eq!(
                settings.client_quota.rate(),
                output["rate"].as_f64().unwrap()
            );
            assert_eq!(
                settings.client_quota.window_ms() as f64 / 1000.0,
                output["window"].as_f64().unwrap()
            );
            assert_eq!(
                settings.client_fail_closed,
                output["fail_closed"].as_bool().unwrap()
            );
            assert_eq!(
                settings.execution.empty_scrape_diagnostics,
                output["diagnostics"].as_bool().unwrap()
            );
        }
        count += 1;
    }
    assert_eq!(count, 14);
}
struct MutableCalendar(Mutex<(WallClockTimestamp, String)>);
impl SearchClock for MutableCalendar {
    fn capture(&self) -> (WallClockTimestamp, String) {
        self.0.lock().unwrap().clone()
    }
}
struct DatedEngine(Arc<FixedEngine>, Vec<String>);
impl SearchEngine for DatedEngine {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            let mut response = self.0.search(request).await;
            for (row, date) in response.results.iter_mut().zip(&self.1) {
                row.published_date = Some(date.clone());
            }
            response
        })
    }
}
#[tokio::test]
async fn local_calendar_is_frozen_for_filter_dispatch_and_cache_identity() {
    let calendars: Vec<_> = fixture()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "calendar")
        .take(2)
        .cloned()
        .collect();
    let old = calendars[0]["date"].as_str().unwrap();
    let new = calendars[1]["date"].as_str().unwrap();
    let clock = Arc::new(MutableCalendar(Mutex::new((
        WallClockTimestamp::new(calendars[0]["epoch"].as_f64().unwrap()).unwrap(),
        old.into(),
    ))));
    let mut engine = FixedEngine::new("openalex");
    engine.delay = Duration::from_millis(30);
    let engine = Arc::new(engine);
    let dates = vec![
        old.to_owned(),
        calendars[0]["window"][0].as_str().unwrap().to_owned(),
        "1999-01-01".into(),
    ];
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(250)).unwrap());
    let mut deps = dependencies(vec![(
        "openalex",
        Arc::new(DatedEngine(engine.clone(), dates)),
    )]);
    deps.clock = clock.clone();
    deps.policy.catalog = CatalogSnapshot::new(vec![EngineCapability {
        enforced_filters: BTreeMap::from([("time_range".into(), "local".into())]),
        ..capability("openalex")
    }])
    .unwrap();
    deps.cache = Some(Arc::new(CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    )));
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let mut p = proposal("calendar-frozen");
    p.request.time_range = Some("day".into());
    p.request.time_range_anchor = Some("2099-01-01".into());
    let second = p.clone();
    let s = service.clone();
    let first = tokio::spawn(async move { s.search(p).await });
    until(|| engine.calls.load(Ordering::SeqCst) == 1).await;
    *clock.0.lock().unwrap() = (
        WallClockTimestamp::new(calendars[1]["epoch"].as_f64().unwrap()).unwrap(),
        new.into(),
    );
    let first = first.await.unwrap().unwrap();
    assert_eq!(first.results.len(), 2);
    assert_eq!(
        engine.requests.lock().unwrap()[0]
            .time_range_anchor
            .as_deref(),
        Some(old)
    );
    let second = service.search(second).await.unwrap();
    assert_eq!(second.results.len(), 1);
    assert!(!second.cached);
    assert_eq!(
        engine.requests.lock().unwrap()[1]
            .time_range_anchor
            .as_deref(),
        Some(new)
    );
    assert_eq!(server.keys("search:native:v1:*").await.len(), 2);
    service.shutdown().await;
    store.close().await;
}
#[tokio::test]
async fn native_suggestion_primary_and_failed_fallback_cache_empty() {
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(100)).unwrap());
    let primary = Loopback::start(
        vec![json!({"results":[{"q":"first"},{"q":"second"}]})],
        Duration::ZERO,
    )
    .await;
    let unused = Loopback::start(vec![json!({})], Duration::ZERO).await;
    let suggestions = SuggestionService::with_endpoints(
        "synthetic".into(),
        Some(store.clone()),
        primary.endpoint.clone(),
        unused.endpoint.clone(),
    );
    assert_eq!(
        suggestions.fetch("original query").await,
        ["first", "second"]
    );
    assert!(unused.requests.lock().unwrap().is_empty());
    let request = primary.requests.lock().unwrap()[0].clone();
    assert!(request.starts_with("GET /search?q=original+query&count=5 "));
    suggestions.shutdown().await;
    primary.shutdown().await;
    unused.shutdown().await;
    let malformed = Loopback::start(
        vec![json!({"results":[{"q":"discarded sibling"},{"q":false}]})],
        Duration::ZERO,
    )
    .await;
    let failed = Loopback::start(vec![json!({})], Duration::ZERO).await;
    let suggestions = SuggestionService::with_endpoints(
        "synthetic".into(),
        Some(store.clone()),
        malformed.endpoint.clone(),
        failed.endpoint.clone(),
    );
    assert!(suggestions.fetch("empty-fallback").await.is_empty());
    assert!(suggestions.fetch(" EMPTY-FALLBACK ").await.is_empty());
    assert_eq!(malformed.requests.lock().unwrap().len(), 1);
    assert_eq!(failed.requests.lock().unwrap().len(), 1);
    assert!(
        !failed.requests.lock().unwrap()[0]
            .to_lowercase()
            .contains("subscription")
    );
    store.close().await;
    assert!(suggestions.fetch("cache-outage").await.is_empty());
    assert_eq!(malformed.requests.lock().unwrap().len(), 2);
    assert_eq!(failed.requests.lock().unwrap().len(), 2);
    suggestions.shutdown().await;
    malformed.shutdown().await;
    failed.shutdown().await;
}
#[tokio::test]
async fn actual_last_waiter_and_shutdown_close_owned_loopback_sockets() {
    let provider = Loopback::start(vec![json!(["query", ["late"]])], Duration::from_secs(1)).await;
    let engine = Arc::new(FixedEngine::new("brave"));
    let mut deps = dependencies(vec![("brave", engine.clone())]);
    deps.suggestions = Some(Arc::new(SuggestionService::with_endpoints(
        String::new(),
        None,
        "unused".into(),
        provider.endpoint.clone(),
    )));
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let cancel = SearchCancellation::default();
    let token = cancel.clone();
    let s = service.clone();
    let caller = tokio::spawn(async move {
        s.search_with_cancellation(proposal("original query"), token)
            .await
    });
    until(|| provider.requests.lock().unwrap().len() == 1).await;
    cancel.cancel();
    assert_eq!(caller.await.unwrap(), Err(SearchError::Cancelled));
    until(|| provider.disconnects.load(Ordering::SeqCst) == 1).await;
    assert_eq!(engine.requests.lock().unwrap()[0].query, "original query");
    service.shutdown().await;
    let http = Arc::new(AdapterHttp::default());
    let engine = Arc::new(MultiRequestEngine {
        http: http.clone(),
        endpoint: provider.endpoint.clone(),
        rounds: 4,
        active: Arc::new(AtomicUsize::new(0)),
        requests: Mutex::new(vec![]),
    });
    let mut deps = dependencies(vec![("brave", engine.clone())]);
    deps.owned_http.push(http.clone());
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let s = service.clone();
    let caller = tokio::spawn(async move { s.search(proposal("shutdown")).await });
    until(|| provider.requests.lock().unwrap().len() == 2).await;
    tokio::join!(service.shutdown(), service.shutdown());
    assert_eq!(caller.await.unwrap(), Err(SearchError::Cancelled));
    assert_eq!(engine.active.load(Ordering::SeqCst), 0);
    until(|| provider.disconnects.load(Ordering::SeqCst) == 2).await;
    assert_eq!(http.retained_pools(), 0);
    service.shutdown().await;
    provider.shutdown().await;
}
#[tokio::test]
async fn coalesced_deadline_warns_each_waiter_once() {
    let mut engine = FixedEngine::new("brave");
    engine.delay = Duration::from_millis(100);
    let engine = Arc::new(engine);
    let service = SearchService::new(
        SearchConfig::default(),
        dependencies(vec![("brave", engine.clone())]),
    )
    .unwrap();
    let mut p = proposal("warnings");
    p.request.interactive_timeout_ms = Some(20);
    let (first, second) = tokio::join!(service.search(p.clone()), service.search(p));
    let mut first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    assert!(first.deadline_exceeded && second.deadline_exceeded);
    assert_ne!(first.query_id, second.query_id);
    assert_eq!(
        first
            .scope
            .warnings
            .iter()
            .filter(|w| w.contains("Interactive deadline"))
            .count(),
        1
    );
    assert_eq!(
        second
            .scope
            .warnings
            .iter()
            .filter(|w| w.contains("Interactive deadline"))
            .count(),
        1
    );
    first.scope.warnings.clear();
    assert!(!second.scope.warnings.is_empty());
    service.shutdown().await;
}
#[tokio::test]
async fn logical_generation_and_scope_order_bound_cache_identity() {
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(100)).unwrap());
    let cache = Arc::new(CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    ));
    let brave = Arc::new(FixedEngine::new("brave"));
    let wiki = Arc::new(FixedEngine::new("wikipedia"));
    let make = |generation: &str| {
        let mut deps = dependencies(vec![("brave", brave.clone()), ("wikipedia", wiki.clone())]);
        deps.cache = Some(cache.clone());
        deps.engines.get_mut("brave").unwrap().generation = generation.into();
        SearchService::new(SearchConfig::default(), deps).unwrap()
    };
    let mut p = proposal("identity");
    p.request.engines = Some(vec!["brave".into(), "wikipedia".into()]);
    let first = make("one");
    assert!(!first.search(p.clone()).await.unwrap().cached);
    first.shutdown().await;
    let same = make("one");
    assert!(same.search(p.clone()).await.unwrap().cached);
    assert_eq!(brave.calls.load(Ordering::SeqCst), 1);
    same.shutdown().await;
    let replaced = make("two");
    assert!(!replaced.search(p.clone()).await.unwrap().cached);
    assert_eq!(brave.calls.load(Ordering::SeqCst), 2);
    p.request.engines = Some(vec!["wikipedia".into(), "brave".into()]);
    let reversed = replaced.search(p.clone()).await.unwrap();
    assert!(!reversed.cached);
    assert_eq!(reversed.results[0].engine, "wikipedia");
    assert_eq!(brave.calls.load(Ordering::SeqCst), 3);
    assert_eq!(server.keys("search:native:v1:*").await.len(), 3);
    p.surface = SearchSurface::Http;
    p.request.page = ssx_domain::SearchPage::new("9223372036854775807").unwrap();
    assert!(!replaced.search(p.clone()).await.unwrap().cached);
    p.request.page = p.request.page.next_page().unwrap();
    assert!(!replaced.search(p.clone()).await.unwrap().cached);
    assert!(replaced.search(p).await.unwrap().cached);
    assert_eq!(brave.calls.load(Ordering::SeqCst), 5);
    replaced.shutdown().await;
    store.close().await;
}

struct PanickingEngine;
#[tokio::test]
async fn actual_source_suggestion_wire_encoding_primary_and_fallback() {
    let mut count = 0;
    for case in fixture()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "suggest-wire")
    {
        let primary = case["provider"] == "brave";
        let brave = Loopback::start(
            vec![if primary {
                json!({"results":[{"q":"x"}]})
            } else {
                json!({"results":[]})
            }],
            Duration::ZERO,
        )
        .await;
        let ddg = Loopback::start(vec![json!(["query", ["x"]])], Duration::ZERO).await;
        let suggestions = SuggestionService::with_endpoints(
            "synthetic-key".into(),
            None,
            brave.endpoint.clone(),
            ddg.endpoint.clone(),
        );
        let result = suggestions.fetch(case["query"].as_str().unwrap()).await;
        assert_eq!(json!(result), case["output"]);
        let mut requests = brave.requests.lock().unwrap().clone();
        requests.extend(ddg.requests.lock().unwrap().clone());
        let queries: Vec<_> = requests
            .iter()
            .map(|request| {
                request
                    .lines()
                    .next()
                    .unwrap()
                    .split_once('?')
                    .unwrap()
                    .1
                    .split(' ')
                    .next()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(json!(queries), case["wire_queries"], "{}", case["id"]);
        assert_eq!(requests.len(), if primary { 1 } else { 2 });
        suggestions.shutdown().await;
        brave.shutdown().await;
        ddg.shutdown().await;
        count += 1;
    }
    assert_eq!(count, 8);
}
impl SearchEngine for PanickingEngine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async { panic!("private local adapter panic") })
    }
}
#[tokio::test]
async fn internal_task_panic_stays_synthetic_without_upstream_health() {
    // This deliberately violates the adapter contract to exercise JoinError.
    let sink = Arc::new(RecordingSink::default());
    let mut deps = dependencies(vec![("brave", Arc::new(PanickingEngine))]);
    deps.metrics = sink.clone();
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    for i in 0..6 {
        let response = service
            .search(proposal(&format!("panic-{i}")))
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
    assert!(service.health().is_empty());
    let health = ssx_application::health::build_engine_health(
        None,
        None,
        SystemSearchClock.capture().0,
        300.0,
    )
    .unwrap();
    assert_eq!(health["status"], json!("unknown"));
    assert_eq!(health["circuit_consecutive_errors"], json!(0));
    assert_eq!(health["last_observed_latency_ms"], Value::Null);
    let events = sink.0.lock().unwrap().clone();
    assert!(!events.contains(&"search_engine_observed"));
    assert_eq!(
        events
            .iter()
            .filter(|e| **e == "search_engine_internal_failure")
            .count(),
        6
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| **e == "search_engine_synthetic")
            .count(),
        6
    );
    service.shutdown().await;
}
struct PauseCooldown {
    pause: std::sync::atomic::AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
impl ssx_application::rate::RateStore for PauseCooldown {
    fn acquire<'a>(
        &'a self,
        _: &'a ssx_domain::rate::EngineKey,
        _: u64,
    ) -> PortFuture<
        'a,
        std::result::Result<ssx_domain::rate::QuotaReceipt, ssx_domain::rate::RateFailure>,
    > {
        Box::pin(async { Err(ssx_domain::rate::RateFailure::Unavailable) })
    }
    fn cooldown<'a>(
        &'a self,
        _: &'a ssx_domain::rate::EngineKey,
    ) -> PortFuture<
        'a,
        std::result::Result<ssx_domain::rate::SharedCooldown, ssx_domain::rate::RateFailure>,
    > {
        Box::pin(async {
            if self.pause.swap(false, Ordering::SeqCst) {
                self.entered.notify_one();
                self.release.notified().await;
            }
            Ok(ssx_domain::rate::SharedCooldown::Missing)
        })
    }
    fn publish_cooldown<'a>(
        &'a self,
        _: &'a ssx_domain::rate::EngineKey,
        _: f64,
    ) -> PortFuture<'a, std::result::Result<u64, ssx_domain::rate::RateFailure>> {
        Box::pin(async { Ok(0) })
    }
    fn warmup(&self) -> PortFuture<'_, std::result::Result<(), ssx_domain::rate::RateFailure>> {
        Box::pin(async { Ok(()) })
    }
    fn shutdown(&self) -> PortFuture<'_, ()> {
        Box::pin(async {})
    }
}
#[tokio::test]
async fn live_circuit_rechecked_after_awaited_cooldown_before_dispatch() {
    let mut engine = FixedEngine::new("brave");
    engine.status = EngineStatus::Error;
    let engine = Arc::new(engine);
    let pause = Arc::new(PauseCooldown {
        pause: std::sync::atomic::AtomicBool::new(false),
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let mut deps = dependencies(vec![("brave", engine.clone())]);
    deps.cooldowns = Some(pause.clone());
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    for i in 0..4 {
        service.search(proposal(&format!("warm{i}"))).await.unwrap();
    }
    pause.pause.store(true, Ordering::SeqCst);
    let s = service.clone();
    let pending = tokio::spawn(async move { s.search(proposal("stalled before semaphore")).await });
    pause.entered.notified().await;
    service
        .search(proposal("fifth failure opens circuit"))
        .await
        .unwrap();
    assert_eq!(engine.calls.load(Ordering::SeqCst), 5);
    assert!(service.health()["brave"].circuit_open(SystemSearchClock.capture().0));
    pause.release.notify_one();
    let r = pending.await.unwrap().unwrap();
    assert_eq!(r.dispatched_engine_count, 0);
    assert_eq!(
        r.engine_outcomes[0].message.as_deref(),
        Some("circuit open")
    );
    assert_eq!(service.health()["brave"].consecutive_errors, 5);
    service.shutdown().await;
    assert_eq!(
        engine.calls.load(Ordering::SeqCst),
        5,
        "engine dispatched after the circuit became open while pre-dispatch I/O awaited"
    );
}

struct ContextProbe {
    seen: Mutex<Vec<(Instant, Instant)>>,
    early: bool,
}
impl SearchEngine for ContextProbe {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        panic!("context override must be used")
    }
    fn search_with_context<'a>(
        &'a self,
        _: &'a SearchRequest,
        context: ssx_application::SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            let deadline = Instant::from_std(context.deadline);
            self.seen.lock().unwrap().push((Instant::now(), deadline));
            if !self.early {
                tokio::time::sleep_until(deadline).await;
            }
            AdapterResponse {
                status: EngineStatus::Timeout,
                latency_ms: FiniteF64::new(7.0).unwrap(),
                ..Default::default()
            }
        })
    }
}

#[tokio::test(start_paused = true)]
async fn context_deadlines_preserve_service_binding_and_adapter_health_origins() {
    // Service earlier, binding earlier, exact tie, and genuine early adapter Timeout.
    for (service_ms, binding_ms, early, expected_status, observed) in [
        (100, 200, false, EngineStatus::Unavailable, false),
        (200, 100, false, EngineStatus::Timeout, false),
        (100, 100, false, EngineStatus::Unavailable, false),
        (100, 200, true, EngineStatus::Timeout, true),
    ] {
        let probe = Arc::new(ContextProbe {
            seen: Mutex::new(vec![]),
            early,
        });
        let mut deps = dependencies(vec![("brave", probe.clone())]);
        deps.engines.get_mut("brave").unwrap().timeout = Duration::from_millis(binding_ms);
        let sink = Arc::new(RecordingSink::default());
        deps.metrics = sink.clone();
        let service = SearchService::new(SearchConfig::default(), deps).unwrap();
        let start = Instant::now();
        let mut p = proposal("context origin");
        p.request.interactive_timeout_ms = Some(service_ms);
        let response = service.search(p).await.unwrap();
        let seen = probe.seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 1);
        assert_eq!(
            seen[0].1,
            start + Duration::from_millis(service_ms.min(binding_ms))
        );
        assert_eq!(
            json!(response.engine_outcomes[0].status),
            json!(expected_status)
        );
        assert_eq!(
            response.deadline_exceeded,
            !early && service_ms <= binding_ms
        );
        assert_eq!(
            sink.0.lock().unwrap().contains(&"search_engine_observed"),
            observed
        );
        let health = ssx_application::health::build_engine_health(
            service.health().get("brave"),
            None,
            SystemSearchClock.capture().0,
            300.0,
        )
        .unwrap();
        assert_eq!(
            health["last_observed_latency_ms"],
            if observed { json!(7.0) } else { Value::Null }
        );
        assert_eq!(
            health["circuit_consecutive_errors"],
            json!(u64::from(expected_status == EngineStatus::Timeout))
        );
        service.shutdown().await;
    }
}

#[tokio::test(start_paused = true)]
async fn context_default_dispatches_legacy_once_and_queue_consumes_service_bound() {
    let mut legacy = FixedEngine::new("brave");
    legacy.delay = Duration::from_millis(40);
    let legacy = Arc::new(legacy);
    let probe = Arc::new(ContextProbe {
        seen: Mutex::new(vec![]),
        early: true,
    });
    let deps = dependencies(vec![
        ("brave", legacy.clone()),
        ("wikipedia", probe.clone()),
    ]);
    let service = SearchService::new(
        SearchConfig {
            max_parallel_engines: 1,
            ..Default::default()
        },
        deps,
    )
    .unwrap();
    let start = Instant::now();
    let mut p = proposal("queue context");
    p.request.interactive_timeout_ms = Some(100);
    service.search(p).await.unwrap();
    assert_eq!(legacy.calls.load(Ordering::SeqCst), 1);
    let seen = probe.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].1, start + Duration::from_millis(100));
    assert_eq!(seen[0].1 - seen[0].0, Duration::from_millis(60));
    service.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn context_full_service_deadline_retains_synthetic_timeout_health() {
    let probe = Arc::new(ContextProbe {
        seen: Mutex::new(vec![]),
        early: false,
    });
    let mut deps = dependencies(vec![("brave", probe.clone())]);
    deps.engines.get_mut("brave").unwrap().timeout = Duration::from_secs(10);
    let sink = Arc::new(RecordingSink::default());
    deps.metrics = sink.clone();
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let start = Instant::now();
    let response = service.search(proposal("full context")).await.unwrap();
    assert_eq!(
        probe.seen.lock().unwrap()[0].1,
        start + Duration::from_secs(10)
    );
    assert_eq!(response.engine_outcomes[0].status, "timeout");
    assert!(!response.deadline_exceeded);
    assert!(!sink.0.lock().unwrap().contains(&"search_engine_observed"));
    let health = ssx_application::health::build_engine_health(
        service.health().get("brave"),
        None,
        SystemSearchClock.capture().0,
        300.0,
    )
    .unwrap();
    assert_eq!(health["status"], json!("timeout"));
    assert_eq!(health["last_observed_latency_ms"], Value::Null);
    assert_eq!(health["circuit_consecutive_errors"], json!(1));
    service.shutdown().await;
}

#[tokio::test]
async fn lossless_high_pages_keep_distinct_valkey_views_and_canonical_snapshots() {
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(250)).unwrap());
    let mut engine = FixedEngine::new("brave");
    engine.page_evidence = true;
    let engine = Arc::new(engine);
    let mut deps = dependencies(vec![("brave", engine.clone())]);
    deps.cache = Some(Arc::new(CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    )));
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let snapshots = SnapshotService::new(
        Arc::new(ValkeySnapshots::new(store.clone())),
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        TenantId::new("f14-lossless-page").unwrap(),
        SnapshotTtl::new(60).unwrap(),
    );
    let pages = [
        "1",
        "9223372036854775807",
        "18446744073709551615",
        "170141183460469231731687303715884105727",
        "170141183460469231731687303715884105728",
    ];
    for raw in pages {
        let mut p = proposal("page identity");
        p.surface = SearchSurface::Http;
        p.request.page = SearchPage::new(raw).unwrap();
        p.request.max_results = Some(1);
        p.request.include = ["results".into()].into_iter().collect();
        let captured = service
            .search_snapshot(p.clone(), &snapshots, &[])
            .await
            .unwrap();
        assert!(!captured.response.cached);
        assert_eq!(captured.response.results.len(), 1);
        assert_eq!(captured.response.results[0].content, format!("page:{raw}"));
        assert_eq!(captured.canonical.response().results.len(), 3);
        let SnapshotLookup::Found(snapshot) = snapshots.read(&captured.snapshot.unwrap()).await
        else {
            panic!("high-page snapshot missing");
        };
        assert_eq!(snapshot.total(), 3);
        assert!(
            snapshot
                .results()
                .iter()
                .all(|r| r.content == format!("page:{raw}"))
        );
        let hit = service.search(p).await.unwrap();
        assert!(hit.cached);
        assert_eq!(hit.results.len(), 1);
        assert_eq!(hit.results[0].content, format!("page:{raw}"));
    }
    assert_eq!(engine.calls.load(Ordering::SeqCst), pages.len());
    assert_eq!(
        engine
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.page.as_str())
            .collect::<Vec<_>>(),
        pages
    );
    assert_eq!(server.keys("search:native:v1:*").await.len(), pages.len());
    service.shutdown().await;
    store.close().await;
}
