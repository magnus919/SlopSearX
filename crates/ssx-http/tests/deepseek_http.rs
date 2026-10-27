//! Offline native-provider acceptance: layered config, registry, real adapters,
//! shared canonical cache, and a loopback native HTTP/HTML surface.
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{Request, State},
    http::StatusCode,
    response::IntoResponse,
};
use ssx_application::{
    NoopSink, PortFuture,
    cache::{
        CacheEntry, CacheFailure, CacheRepository, CacheTtls, CanonicalCache, RoutingIdentity,
    },
    policy::{PolicyService, ScopeProposal, SearchPolicy},
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
};
use ssx_domain::SearchRequest;
use ssx_http::{HttpState, discovery, html::PublicHtml};
use ssx_infra::{
    adapters::{AllowAllAdmission, BroadApiConfig, register_broad_api, register_deepseek},
    http::AdapterHttp,
    observability::{Options, Telemetry},
    search::{EngineBinding, SearchConfig, SearchDependencies, SearchService, SystemSearchClock},
};
use ssx_json::{Value, json};
use ssx_registry::Registry;
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

#[derive(Default)]
struct MemoryCache(Mutex<BTreeMap<String, CacheEntry>>);
impl CacheRepository for MemoryCache {
    fn read<'a>(
        &'a self,
        key: &'a str,
    ) -> PortFuture<'a, Result<Option<CacheEntry>, CacheFailure>> {
        Box::pin(async move { Ok(self.0.lock().unwrap().get(key).cloned()) })
    }
    fn write<'a>(
        &'a self,
        key: &'a str,
        value: &'a CacheEntry,
        _: NonZeroU64,
    ) -> PortFuture<'a, Result<(), CacheFailure>> {
        Box::pin(async move {
            self.0.lock().unwrap().insert(key.into(), value.clone());
            Ok(())
        })
    }
}
#[derive(Clone)]
struct Exchange {
    method: String,
    path: String,
    body: serde_json::Value,
}
type Exchanges = Arc<Mutex<Vec<Exchange>>>;
async fn provider(State(seen): State<Exchanges>, request: Request) -> impl IntoResponse {
    let method = request.method().to_string();
    let path = request.uri().path().to_owned();
    if path == "/v1/tools/search" {
        assert_eq!(
            request.headers()["authorization"],
            "Bearer offline-deepseek-key"
        );
        assert_eq!(request.headers()["content-type"], "application/json");
    }
    let raw = to_bytes(request.into_body(), 16_384).await.unwrap();
    let body = if raw.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&raw).unwrap()
    };
    seen.lock().unwrap().push(Exchange {
        method,
        path: path.clone(),
        body,
    });
    let response = if path == "/v1/tools/search" {
        serde_json::json!({"results":[
            {"title":"First <result>","url":"https://deepseek.example/first","snippet":"Display <snippet>","summary":"Do not display model summary","date":"2026-10-26T00:00:00Z"},
            {"title":"Second result","url":"https://deepseek.example/second","snippet":"Second snippet","date":null},
            {"title":"Third result","url":"https://deepseek.example/third","snippet":"Third snippet","date":"bad-date"}
        ]})
    } else if path == "/res/v1/web/search" {
        serde_json::json!({"web":{"results":[{"title":"Brave peer","url":"https://brave.example/peer","description":"Peer remains available"}]}})
    } else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({}))).into_response();
    };
    Json(response).into_response()
}
struct Fixture {
    url: String,
    service: SearchService,
    seen: Exchanges,
    cache: Arc<MemoryCache>,
    stop_provider: Option<oneshot::Sender<()>>,
    stop_surface: Option<oneshot::Sender<()>>,
    provider: JoinHandle<()>,
    surface: JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.provider.abort();
        self.surface.abort();
    }
}
impl Fixture {
    async fn new(with_brave: bool) -> Self {
        let seen: Exchanges = Arc::default();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream = format!("http://{}", listener.local_addr().unwrap());
        let (stop_provider, rx) = oneshot::channel();
        let provider_app = Router::new().fallback(provider).with_state(seen.clone());
        let provider = tokio::spawn(async move {
            axum::serve(listener, provider_app)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await
                .unwrap();
        });
        let config = ssx_config::load_config_with(
            Some(std::path::Path::new("/dev/null")),
            &BTreeMap::from([
                ("ENGINE_DEEPSEEK_ENABLED".into(), "true".into()),
                (
                    "ENGINE_DEEPSEEK_API_KEY".into(),
                    "offline-deepseek-key".into(),
                ),
                (
                    "ENGINE_DEEPSEEK_BASE_URL".into(),
                    format!("{upstream}/v1/tools/search"),
                ),
                ("ENGINE_DEEPSEEK_MAX_RESULTS".into(), "10".into()),
            ]),
        )
        .unwrap();
        let http = Arc::new(AdapterHttp::default());
        let mut registry = Registry::builtins().unwrap();
        let mut engines = BTreeMap::from([(
            "deepseek".to_owned(),
            register_deepseek(
                &mut registry,
                http.clone(),
                Arc::new(AllowAllAdmission),
                &config.engines["deepseek"],
            )
            .unwrap()
            .unwrap(),
        )]);
        assert!(registry.instantiate("deepseek").is_some());
        if with_brave {
            let mut broad = BroadApiConfig::default();
            broad.brave.base_url = format!("{upstream}/res/v1/web/search");
            broad.brave.api_key = "offline-brave-key".into();
            register_broad_api(
                &mut registry,
                http.clone(),
                Arc::new(AllowAllAdmission),
                broad,
            )
            .unwrap();
            engines.insert(
                "brave".into(),
                EngineBinding {
                    engine: Arc::from(registry.instantiate("brave").unwrap()),
                    generation: "offline-brave-v1".into(),
                    timeout: Duration::from_secs(3),
                    scrape: false,
                },
            );
        }
        let policy = SearchPolicy::default();
        let catalog = CatalogSnapshot::new(
            engines
                .keys()
                .map(|name| {
                    let descriptor = registry.descriptor(name).unwrap();
                    EngineCapability {
                        name: name.clone(),
                        enabled: true,
                        active: true,
                        available: true,
                        categories: descriptor.categories.clone(),
                        media_types: vec![],
                        auth_ready: true,
                        circuit_open: false,
                        cost_class: descriptor.cost_class.clone().unwrap_or_default(),
                        health_status: "unknown".into(),
                        health_stale: true,
                        enforced_filters: descriptor.enforced_filters.clone(),
                    }
                })
                .collect(),
        )
        .unwrap();
        let cache = Arc::new(MemoryCache::default());
        let service = SearchService::new_with_registry(
            SearchConfig::default(),
            SearchDependencies {
                policy: PolicyService {
                    policy: policy.clone(),
                    catalog,
                    resolver: ScopeResolver {
                        catalog_routing: false,
                        ..Default::default()
                    },
                },
                engines,
                routing_identity: RoutingIdentity {
                    catalog: None,
                    sensitive: policy.sensitive_engines.clone(),
                    tier1: ScopeResolver::default().tier1,
                    budget: None,
                    ranking_strategy: "presence".into(),
                    work_group_version: "scholarly-work-v3".into(),
                    rerank_identity: None,
                    routing_extension: None,
                },
                cache: Some(Arc::new(CanonicalCache::new(
                    cache.clone(),
                    CacheTtls::new(900, 60, 30).unwrap(),
                ))),
                client_admission: None,
                suggestions: None,
                jev: None,
                routing_cache: None,
                metrics: Arc::new(NoopSink),
                supervisor: Arc::new(NoopSink),
                audit: Arc::new(NoopSink),
                cooldowns: None,
                owned_http: vec![http],
                owned_stores: vec![],
                clock: Arc::new(SystemSearchClock),
                reranker: None,
            },
            Arc::new(registry.clone()),
        )
        .unwrap();
        let names = if with_brave {
            vec!["deepseek".into(), "brave".into()]
        } else {
            vec!["deepseek".into()]
        };
        let discovery = discovery::build(&registry, &config, &names, &policy, "offline").unwrap();
        let telemetry = Telemetry::new(
            Options {
                audit: false,
                ..Default::default()
            },
            None,
            names,
            vec!["general".into()],
        )
        .unwrap();
        let state = HttpState {
            search: service.clone(),
            policy: Arc::new(policy),
            telemetry,
            formats: ["json".into(), "html".into()].into_iter().collect(),
            discovery: discovery.config,
            health_contexts: discovery.health_contexts,
            valkey_connected: Arc::new(|| false),
            valkey_fail_closed: true,
            valkey_quota_configured: false,
            html: Arc::new(PublicHtml::default()),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (stop_surface, rx) = oneshot::channel();
        let surface = tokio::spawn(async move {
            ssx_http::native::serve_with_shutdown(listener, state, async {
                let _ = rx.await;
            })
            .await
            .unwrap();
        });
        Self {
            url,
            service,
            seen,
            cache,
            stop_provider: Some(stop_provider),
            stop_surface: Some(stop_surface),
            provider,
            surface,
        }
    }
    async fn get(&self, path: &str) -> (StatusCode, String) {
        let response = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
            .get(format!("{}{path}", self.url))
            .send()
            .await
            .unwrap();
        (response.status(), response.text().await.unwrap())
    }
    fn calls(&self, path: &str) -> usize {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.path == path)
            .count()
    }
    async fn finish(mut self) {
        self.stop_surface.take().unwrap().send(()).unwrap();
        (&mut self.surface).await.unwrap();
        self.stop_provider.take().unwrap().send(()).unwrap();
        (&mut self.provider).await.unwrap();
    }
}
fn request(query: &str, max: u64) -> ScopeProposal {
    ScopeProposal::generic(SearchRequest {
        query: query.into(),
        engines: Some(vec!["deepseek".into()]),
        max_results: Some(max),
        ..Default::default()
    })
}
#[tokio::test]
async fn canonical_capture_survives_both_view_orders_and_reaches_native_json_and_html() {
    let fixture = Fixture::new(false).await;
    for (query, limits) in [("small-first", [1, 3]), ("large-first", [3, 1])] {
        for (index, limit) in limits.into_iter().enumerate() {
            let response = fixture.service.search(request(query, limit)).await.unwrap();
            assert_eq!(response.cached, index != 0);
            assert_eq!(response.results.len(), limit as usize);
            assert!(
                response
                    .results
                    .iter()
                    .all(|r| r.engine == "deepseek" && r.tier == 2)
            );
        }
        assert!(fixture.cache.0.lock().unwrap().values().any(|entry| matches!(entry,
            CacheEntry::Canonical(capture) if capture.response().query == query && capture.response().results.len() == 3)));
        let (status, body) = fixture
            .get(&format!("/search?q={query}&engines=deepseek&format=json"))
            .await;
        assert_eq!(status, StatusCode::OK);
        let body: Value = ssx_json::from_str(&body).unwrap();
        assert_eq!(body["results"].as_array().unwrap().len(), 3);
        assert_eq!(body["meta"]["cached"], json!(true));
    }
    assert_eq!(fixture.calls("/v1/tools/search"), 2);
    for exchange in fixture.seen.lock().unwrap().iter() {
        assert_eq!(exchange.method, "POST");
        assert_eq!(exchange.body["count"], 10);
        assert_eq!(exchange.body["need_content"], false);
        assert_eq!(exchange.body.as_object().unwrap().len(), 3);
    }
    let (status, html) = fixture
        .get("/search?q=small-first&engines=deepseek&format=html")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("First &lt;result&gt;"));
    assert!(html.contains("Display &lt;snippet&gt;"));
    assert!(html.contains("deepseek"));
    assert!(!html.contains("Do not display model summary"));
    assert!(!html.contains("offline-deepseek-key"));
    let (status, discovery) = fixture.get("/config").await;
    assert_eq!(status, StatusCode::OK);
    let discovery: Value = ssx_json::from_str(&discovery).unwrap();
    let engine = discovery["engines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == json!("deepseek"))
        .unwrap();
    assert_eq!(engine["display_name"], json!("DeepSeek Search"));
    assert_eq!(engine["supported_filters"]["pagination"], json!(false));
    assert_eq!(engine["supported_filters"]["safesearch"], json!(false));
    assert!(!discovery.to_string().contains("offline-deepseek-key"));
    assert_eq!(fixture.calls("/v1/tools/search"), 2);
    fixture.finish().await;
}
#[tokio::test]
async fn strict_filter_and_later_pages_never_repeat_paid_search_and_brave_continues() {
    let fixture = Fixture::new(true).await;
    let (status, body) = fixture
        .get("/search?q=restricted&engines=deepseek&format=json&safesearch=2")
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let rejection: Value = ssx_json::from_str(&body).unwrap();
    assert_eq!(rejection["error"], json!("invalid_filter"));
    assert_eq!(rejection["field"], json!("safesearch"));
    assert_eq!(fixture.calls("/v1/tools/search"), 0);
    let (status, body) = fixture
        .get("/search?q=page-two&engines=deepseek,brave&format=json&pageno=2")
        .await;
    assert_eq!(status, StatusCode::OK);
    let body: Value = ssx_json::from_str(&body).unwrap();
    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["engine"], json!("brave"));
    // HTTP preserves its existing response shape; the shared policy preview
    // owns the explicit enforcement report consumed by search-capable surfaces.
    let mut later_page = request("page-two", 10);
    later_page.request.page = 2;
    later_page.request.engines = Some(vec!["deepseek".into(), "brave".into()]);
    let authority = fixture.service.prepare_scope(&later_page).await.unwrap();
    assert_eq!(
        authority.preview().enforcement["pagination"]
            .status
            .as_str(),
        "unsupported"
    );
    assert_eq!(
        body["meta"]["engine_status"]["deepseek"]["status"],
        json!("error")
    );
    assert_eq!(fixture.calls("/v1/tools/search"), 0);
    assert_eq!(fixture.calls("/res/v1/web/search"), 1);
    fixture.finish().await;
}
#[tokio::test]
async fn default_and_key_only_configuration_cannot_activate_deepseek() {
    for env in [
        BTreeMap::new(),
        BTreeMap::from([(
            "ENGINE_DEEPSEEK_API_KEY".into(),
            "offline-deepseek-key".into(),
        )]),
    ] {
        let config =
            ssx_config::load_config_with(Some(std::path::Path::new("/dev/null")), &env).unwrap();
        assert_eq!(config.default_engines, ["brave", "wikipedia"]);
        assert!(!config.engines["deepseek"].enabled);
        let mut registry = Registry::builtins().unwrap();
        let http = Arc::new(AdapterHttp::default());
        assert!(
            register_deepseek(
                &mut registry,
                http.clone(),
                Arc::new(AllowAllAdmission),
                &config.engines["deepseek"]
            )
            .unwrap()
            .is_none()
        );
        assert!(!registry.is_available("deepseek"));
        http.shutdown_and_wait().await;
    }
}
