//! Shared actual OpenAlex/service/private Valkey fixture for native HTTP and MCP.
#[path = "owned_loopback_fixture.rs"]
pub(crate) mod owned_loopback_fixture;
#[path = "../../../ssx-mcp/tests/t06_support/server.rs"]
mod private_valkey;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use owned_loopback_fixture::OwnedLoopback;
use serde_json::{Value, json};
use ssx_application::{
    AdmissionDecision, NoopSink, PortFuture, SearchAdmission, SearchEngine,
    cache::{
        CacheEntry, CacheFailure, CacheRepository, CacheTtls, CanonicalCache, RoutingIdentity,
    },
    policy::{PolicyService, SearchPolicy},
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
};
use ssx_infra::{
    adapters::{
        BraveConfig, BroadApiConfig,
        arxiv_expat::AtomPool,
        openalex_runtime::{self, OpenAlexConfig},
        register_broad_api,
    },
    http::{AdapterHttp, Options},
    search::{
        ClientAdmission, EngineBinding, SearchConfig, SearchDependencies, SearchService,
        SystemSearchClock,
    },
    valkey::{Valkey, cache::ValkeyCache},
};
use std::{
    collections::BTreeMap,
    num::{NonZeroU64, NonZeroUsize},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[derive(Default)]
pub(crate) struct Counts {
    pub(crate) adapter: AtomicUsize,
    pub(crate) client: AtomicUsize,
    pub(crate) reads: AtomicUsize,
    pub(crate) writes: AtomicUsize,
}
impl SearchAdmission for Counts {
    fn admit<'a>(&'a self, name: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move {
            self.adapter.fetch_add(1, Ordering::SeqCst);
            match name {
                "openalex" => AdmissionDecision::Allowed,
                // A policy regression is counted, then denied before Brave can
                // create an HTTP session or observe proxy/environment settings.
                "brave" => AdmissionDecision::Denied,
                _ => panic!("unexpected science fixture adapter: {name}"),
            }
        })
    }
}
impl ClientAdmission for Counts {
    fn acquire<'a>(&'a self, _: &'a str) -> PortFuture<'a, bool> {
        Box::pin(async move {
            self.client.fetch_add(1, Ordering::SeqCst);
            true
        })
    }
}
impl Counts {
    pub(crate) fn observations(&self) -> [usize; 4] {
        [
            self.adapter.load(Ordering::SeqCst),
            self.client.load(Ordering::SeqCst),
            self.reads.load(Ordering::SeqCst),
            self.writes.load(Ordering::SeqCst),
        ]
    }
}
struct ObservedCache {
    inner: ValkeyCache,
    pub(crate) counts: Arc<Counts>,
}
impl CacheRepository for ObservedCache {
    fn read<'a>(
        &'a self,
        key: &'a str,
    ) -> PortFuture<'a, Result<Option<CacheEntry>, CacheFailure>> {
        Box::pin(async move {
            self.counts.reads.fetch_add(1, Ordering::SeqCst);
            self.inner.read(key).await
        })
    }
    fn write<'a>(
        &'a self,
        key: &'a str,
        value: &'a CacheEntry,
        ttl: NonZeroU64,
    ) -> PortFuture<'a, Result<(), CacheFailure>> {
        Box::pin(async move {
            self.counts.writes.fetch_add(1, Ordering::SeqCst);
            self.inner.write(key, value, ttl).await
        })
    }
}
pub(crate) struct Upstream {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
    owner: OwnedLoopback<()>,
    expected_requests: Option<usize>,
}
impl Upstream {
    async fn start(status: u16, body: Vec<u8>, expected_requests: Option<usize>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let observed = requests.clone();
        let owner = OwnedLoopback::spawn(async move {
            let mut replies = 0;
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut chunk = [0; 4096];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = socket.read(&mut chunk).await.unwrap();
                    assert_ne!(n, 0);
                    request.extend_from_slice(&chunk[..n]);
                    assert!(request.len() <= 8192, "bounded upstream request");
                }
                observed
                    .lock()
                    .unwrap()
                    .push(String::from_utf8(request).unwrap());
                let head = format!(
                    "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(head.as_bytes()).await.unwrap();
                socket.write_all(&body).await.unwrap();
                socket.shutdown().await.unwrap();
                replies += 1;
                if Some(replies) == expected_requests {
                    return;
                }
            }
        });
        Self {
            base,
            requests,
            owner,
            expected_requests,
        }
    }
    pub(crate) fn lines(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.lines().next().unwrap().to_owned())
            .collect()
    }
    pub(crate) async fn close(self) {
        if self.expected_requests.is_some() {
            self.owner.finish().await;
        } else {
            assert!(self.owner.stop_and_join().await.is_none());
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum Mode {
    Audited,
    BroadAvailable,
    #[allow(dead_code)] // Only the MCP transport has a specialist grant.
    NoScienceGrant,
    Unaudited,
    Unavailable,
}
pub(crate) struct FixtureCore {
    #[allow(dead_code)] // The HTTP surface consumes policy; MCP consumes the same service gate.
    pub(crate) policy: Arc<SearchPolicy>,
    pub(crate) service: SearchService,
    pub(crate) upstream: Upstream,
    server: private_valkey::Server,
    pub(crate) store: Arc<Valkey>,
    http: Arc<AdapterHttp>,
    pool: AtomPool,
    pub(crate) counts: Arc<Counts>,
}
impl FixtureCore {
    pub(crate) async fn new(
        mode: Mode,
        status: u16,
        body: Vec<u8>,
        expected_requests: Option<usize>,
    ) -> Self {
        let upstream = Upstream::start(status, body, expected_requests).await;
        let server = private_valkey::Server::start();
        let store = Arc::new(server.store());
        assert!(store.ping().await.is_ok());
        let counts = Arc::new(Counts::default());
        let http = Arc::new(AdapterHttp::default());
        let pool = AtomPool::new(
            PathBuf::from("/nonexistent-science-openalex-helper"),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        openalex_runtime::register(
            &mut registry,
            http.clone(),
            counts.clone(),
            OpenAlexConfig {
                base_url: Arc::from(upstream.base.as_str()),
                transport: Options {
                    trust_env: false,
                    ..Default::default()
                },
                ..Default::default()
            },
            pool.clone(),
        )
        .unwrap();
        let engine: Arc<dyn SearchEngine> = Arc::from(registry.instantiate("openalex").unwrap());
        let mut engines = BTreeMap::from([(
            "openalex".into(),
            EngineBinding {
                engine,
                generation: "science-real-openalex-v1".into(),
                timeout: Duration::from_secs(3),
                scrape: false,
            },
        )]);
        if matches!(mode, Mode::BroadAvailable) {
            register_broad_api(
                &mut registry,
                http.clone(),
                counts.clone(),
                BroadApiConfig {
                    brave: BraveConfig {
                        base_url: upstream.base.clone(),
                        api_key: "synthetic-science-brave-key".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(registry.is_available("brave"));
            let engine: Arc<dyn SearchEngine> = Arc::from(registry.instantiate("brave").unwrap());
            engines.insert(
                "brave".into(),
                EngineBinding {
                    engine,
                    generation: "science-real-brave-denied-v1".into(),
                    timeout: Duration::from_secs(3),
                    scrape: false,
                },
            );
        }
        let mut capabilities = Vec::new();
        for name in ["openalex", "brave", "hibp"] {
            let d = registry.descriptor(name).unwrap();
            let available = registry.is_available(name);
            let mut cap = EngineCapability {
                name: name.into(),
                enabled: true,
                active: available,
                available,
                categories: d.categories.clone(),
                media_types: d.supported_media_types.clone(),
                auth_ready: true,
                circuit_open: false,
                cost_class: "free".into(),
                health_status: "unknown".into(),
                health_stale: true,
                enforced_filters: d.enforced_filters.clone(),
            };
            // These negative composition modes change only the trusted test
            // catalog. The positive uses the actual registered descriptor.
            if name == "openalex" {
                if matches!(mode, Mode::Unaudited) {
                    cap.enforced_filters.remove("date_to");
                }
                if matches!(mode, Mode::Unavailable) {
                    cap.active = false;
                    cap.available = false;
                }
            }
            capabilities.push(cap);
        }
        let policy = SearchPolicy {
            grants: if matches!(mode, Mode::NoScienceGrant) {
                Default::default()
            } else {
                ["science".into()].into_iter().collect()
            },
            ..Default::default()
        };
        let service = SearchService::new_with_registry(
            SearchConfig::default(),
            SearchDependencies {
                policy: PolicyService {
                    policy: policy.clone(),
                    catalog: CatalogSnapshot::new(capabilities).unwrap(),
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
                    Arc::new(ObservedCache {
                        inner: ValkeyCache::new(store.clone()),
                        counts: counts.clone(),
                    }),
                    CacheTtls::default(),
                ))),
                client_admission: Some(counts.clone()),
                suggestions: None,
                jev: None,
                routing_cache: None,
                metrics: Arc::new(NoopSink),
                supervisor: Arc::new(NoopSink),
                audit: Arc::new(NoopSink),
                cooldowns: None,
                owned_http: vec![http.clone()],
                owned_stores: vec![store.clone()],
                clock: Arc::new(SystemSearchClock),
                reranker: None,
            },
            Arc::new(registry),
        )
        .unwrap();
        Self {
            policy: Arc::new(policy),
            service,
            upstream,
            server,
            store,
            http,
            pool,
            counts,
        }
    }
    pub(crate) async fn connection(&self) -> redis::aio::MultiplexedConnection {
        redis::Client::open(self.server.url())
            .unwrap()
            .get_multiplexed_async_connection()
            .await
            .unwrap()
    }
    pub(crate) async fn keys(&self, pattern: &str) -> Vec<String> {
        let mut conn = self.connection().await;
        let mut keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .unwrap();
        keys.sort();
        keys
    }
    pub(crate) async fn close(self) {
        self.service.shutdown().await;
        assert!(
            self.store.ping().await.is_err(),
            "service must close owned Valkey"
        );
        assert_eq!(self.pool.available_slots(), 1);
        assert_eq!(self.pool.available_operations(), 1);
        self.pool.shutdown().await.unwrap();
        self.http.shutdown_and_wait().await;
        self.upstream.close().await;
        drop(self.server);
    }
}
pub(crate) fn corpus() -> ssx_json::Value {
    ssx_json::from_str(include_str!(
        "../../../../docs/rust-rewrite/a06/source-openalex-actual-dates.json"
    ))
    .unwrap()
}
pub(crate) fn equal_body() -> Vec<u8> {
    let c = corpus();
    let row = c["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "openalex.actual_dates.equal_leap")
        .unwrap();
    STANDARD
        .decode(row["responses"][0]["body_b64"].as_str().unwrap())
        .unwrap()
}
pub(crate) fn proof(from: Option<&str>, to: Option<&str>) -> Value {
    json!({"requested":{"date_from":from,"date_to":to,"time_range":null},"applied":{"date_from":from,"date_to":to},"status":"enforced","reason":"Publication dates enforced by the audited OpenAlex adapter.","engines":["openalex"],"enforced_by":["upstream:openalex"],"warnings":[]})
}
