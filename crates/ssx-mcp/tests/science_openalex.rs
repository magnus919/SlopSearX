//! Real registered OpenAlex through authenticated MCP, shared policy and Valkey.
//! No production startup, gateway, browser or live upstream is exercised here.
#[path = "../../ssx-infra/tests/support/owned_loopback_fixture.rs"]
mod owned_loopback_fixture;
#[path = "t06_support/server.rs"]
mod private_valkey;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use owned_loopback_fixture::OwnedLoopback;
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientConfig, ProtocolVersion},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use ssx_application::{
    AdmissionDecision, NoopSink, PortFuture, SearchAdmission, SearchEngine,
    cache::{
        CacheEntry, CacheFailure, CacheRepository, CacheTtls, CanonicalCache, RoutingIdentity,
    },
    policy::{PolicyService, SearchPolicy},
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
    snapshot::{SnapshotService, SnapshotTtl},
};
use ssx_domain::{PublicationDateProof, TenantId, storage};
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
    valkey::{
        Valkey,
        cache::ValkeyCache,
        snapshot::{RandomSnapshotIds, SystemSnapshotClock, ValkeySnapshots},
    },
};
use ssx_mcp::{
    harness::{Host, http_router_for},
    oauth::NetworkAuth,
    search_surface::{DiscoveryState, SearchState, SearchSurface},
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
use tokio_util::sync::CancellationToken;

const TOOL: &str = "slopsearx_search_science";
const TOKEN: &str = "science-date-fixture";
type Client = rmcp::service::RunningService<rmcp::RoleClient, ClientConfig>;

// Discovery is not a dispatch authority. These tests must use the actual policy
// catalog; they neither fabricate nor claim a passive-discovery response.
struct SearchOnly;
impl DiscoveryState for SearchOnly {
    fn capabilities(&self) -> Vec<ssx_json::Value> {
        panic!("science call must not query discovery")
    }
    fn diagnostics(&self) -> ssx_json::Value {
        panic!("science call must not query diagnostics")
    }
}
#[derive(Default)]
struct Counts {
    adapter: AtomicUsize,
    client: AtomicUsize,
    reads: AtomicUsize,
    writes: AtomicUsize,
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
    fn observations(&self) -> [usize; 4] {
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
    counts: Arc<Counts>,
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
struct Upstream {
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
    fn lines(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.lines().next().unwrap().to_owned())
            .collect()
    }
    async fn close(self) {
        if self.expected_requests.is_some() {
            self.owner.finish().await;
        } else {
            assert!(self.owner.stop_and_join().await.is_none());
        }
    }
}
#[derive(Clone, Copy)]
enum Mode {
    Audited,
    BroadAvailable,
    NoScienceGrant,
    Unaudited,
    Unavailable,
}
struct Fixture {
    service: SearchService,
    host: Host,
    upstream: Upstream,
    server: private_valkey::Server,
    store: Arc<Valkey>,
    http: Arc<AdapterHttp>,
    pool: AtomPool,
    counts: Arc<Counts>,
}
impl Fixture {
    async fn new(mode: Mode, status: u16, body: Vec<u8>, expected_requests: Option<usize>) -> Self {
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
        let snapshots = SnapshotService::new(
            Arc::new(ValkeySnapshots::new(store.clone())),
            Arc::new(SystemSnapshotClock),
            Arc::new(RandomSnapshotIds),
            TenantId::new("default").unwrap(),
            SnapshotTtl::new(3600).unwrap(),
        );
        let surface = SearchSurface::new(
            SearchState::new(service.clone(), snapshots, Arc::new(SearchOnly), 50).unwrap(),
        );
        let cancel = CancellationToken::new();
        let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new(TOKEN.into()).unwrap());
        let host = Host::start(
            http_router_for(surface, SearchSurface::catalogs(), cancel.clone())
                .map(|r| auth.mount(r)),
            cancel,
        )
        .await
        .unwrap();
        Self {
            service,
            host,
            upstream,
            server,
            store,
            http,
            pool,
            counts,
        }
    }
    async fn connect(&self) -> Client {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {TOKEN}").parse().unwrap(),
        );
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .unwrap();
        let transport = StreamableHttpClientTransport::with_client(
            http,
            StreamableHttpClientTransportConfig::with_uri(format!(
                "http://{}/mcp",
                self.host.address
            )),
        );
        ClientConfig::default()
            .with_protocol_version(ProtocolVersion::V_2025_11_25)
            .serve(transport)
            .await
            .unwrap()
    }
    async fn connection(&self) -> redis::aio::MultiplexedConnection {
        redis::Client::open(self.server.url())
            .unwrap()
            .get_multiplexed_async_connection()
            .await
            .unwrap()
    }
    async fn keys(&self, pattern: &str) -> Vec<String> {
        let mut conn = self.connection().await;
        let mut keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .unwrap();
        keys.sort();
        keys
    }
    async fn close(self) {
        self.host.shutdown().await.unwrap();
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
fn corpus() -> ssx_json::Value {
    ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/source-openalex-actual-dates.json"
    ))
    .unwrap()
}
fn equal_body() -> Vec<u8> {
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
fn args() -> Value {
    json!({"query":"date control / +","engines":["openalex"],"date_from":"2024-02-29","date_to":"2024-02-29"})
}
fn proof(from: Option<&str>, to: Option<&str>) -> Value {
    json!({"requested":{"date_from":from,"date_to":to,"time_range":null},"applied":{"date_from":from,"date_to":to},"status":"enforced","reason":"Publication dates enforced by the audited OpenAlex adapter.","engines":["openalex"],"enforced_by":["upstream:openalex"],"warnings":[]})
}
async fn call(client: &Client, args: Value) -> Value {
    let response = client
        .call_tool(
            CallToolRequestParams::new(TOOL.to_owned())
                .with_arguments(args.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    let value = serde_json::to_value(response).unwrap();
    assert_ne!(
        value["isError"],
        json!(true),
        "policy failures remain structured source envelopes"
    );
    let text: Value = serde_json::from_str(value["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(value["structuredContent"], text);
    text
}
fn assert_result(value: &Value, days: &[&str], expected: &Value, cached: bool) {
    assert!(value.get("error").is_none(), "{value}");
    assert_eq!(value["meta"]["cached"], cached);
    assert_eq!(value["meta"]["date_filter"], *expected);
    assert_eq!(value["scope"]["selected_engines"], json!(["openalex"]));
    assert_eq!(value["engine_outcomes"].as_array().unwrap().len(), 1);
    assert_eq!(value["engine_outcomes"][0]["engine"], "openalex");
    assert_eq!(value["engine_outcomes"][0]["status"], "ok");
    let mut actual: Vec<_> = value["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            assert_eq!(r["primary_engine"], "openalex");
            assert!(
                r["url"]
                    .as_str()
                    .unwrap()
                    .starts_with("https://openalex.org/")
            );
            r["published_at"].as_str().unwrap()
        })
        .collect();
    actual.sort_unstable();
    assert_eq!(actual, days);
    for field in ["date_from", "date_to"] {
        if !expected["requested"][field].is_null() {
            assert_eq!(
                value["enforcement"][field],
                json!({"requested":expected["requested"][field],"status":"enforced","reason":format!("{field} is enforced upstream by every selected adapter"),"enforced_by":["upstream:openalex"]})
            );
        }
    }
}

#[tokio::test]
async fn authenticated_science_openalex_dates_preserve_wire_proof_and_valkey_cache() {
    let fixture = Fixture::new(Mode::Audited, 200, equal_body(), Some(7)).await;
    let client = fixture.connect().await;
    let c = corpus();
    let mut equal_key = None;
    for (index, (suffix, from, to, days)) in [
        (
            "lower_leap",
            Some("2024-02-29"),
            None,
            vec!["2024-02-29", "2024-03-01"],
        ),
        (
            "upper_leap",
            None,
            Some("2024-02-29"),
            vec!["2024-02-28", "2024-02-29"],
        ),
        (
            "equal_leap",
            Some("2024-02-29"),
            Some("2024-02-29"),
            vec!["2024-02-29"],
        ),
        (
            "both_span",
            Some("2024-02-28"),
            Some("2024-03-01"),
            vec!["2024-02-28", "2024-02-29", "2024-03-01"],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let row = c["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"].as_str().unwrap().ends_with(suffix))
            .unwrap();
        assert_eq!(
            STANDARD
                .decode(row["responses"][0]["body_b64"].as_str().unwrap())
                .unwrap(),
            equal_body()
        );
        let mut request = args();
        request.as_object_mut().unwrap().remove("date_from");
        request.as_object_mut().unwrap().remove("date_to");
        if let Some(v) = from {
            request["date_from"] = json!(v);
        }
        if let Some(v) = to {
            request["date_to"] = json!(v);
        }
        let expected = proof(from, to);
        let before = fixture.keys("search:*").await;
        let response = call(&client, request.clone()).await;
        assert_result(&response, &days, &expected, false);
        let mut actual_rows: Vec<_> = response["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["url"].as_str().unwrap(),
                    r["published_at"].as_str().unwrap(),
                )
            })
            .collect();
        let mut source_rows: Vec<_> = row["output"]["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["url"].as_str().unwrap(),
                    r["published_date"].as_str().unwrap(),
                )
            })
            .collect();
        actual_rows.sort_unstable();
        source_rows.sort_unstable();
        assert_eq!(actual_rows, source_rows);
        let expected_wire = format!(
            "GET {} HTTP/1.1",
            row["requests"][0]["url"]
                .as_str()
                .unwrap()
                .strip_prefix("https://api.openalex.org")
                .unwrap()
        );
        assert_eq!(fixture.upstream.lines().len(), index + 1);
        assert_eq!(fixture.upstream.lines()[index], expected_wire);
        let after = fixture.keys("search:*").await;
        let added: Vec<_> = after
            .iter()
            .filter(|k| !before.contains(k))
            .cloned()
            .collect();
        assert_eq!(
            added.len(),
            1,
            "date bounds must separate canonical cache identity"
        );
        if suffix == "equal_leap" {
            equal_key = Some(added[0].clone());
        }
        assert_result(&call(&client, request).await, &days, &expected, true);
        assert_eq!(
            fixture.upstream.lines().len(),
            index + 1,
            "MCP cache hit dispatched OpenAlex"
        );
    }
    let mut broad = args();
    broad["date_to"] = json!("2024-03-01");
    broad["max_results"] = json!(1);
    let broad_proof = proof(Some("2024-02-29"), Some("2024-03-01"));
    let view = call(&client, broad.clone()).await;
    assert_eq!(view["results"].as_array().unwrap().len(), 1);
    assert_eq!(view["meta"]["total"], 2);
    assert_eq!(view["meta"]["date_filter"], broad_proof);
    assert_eq!(view["meta"]["cached"], false);
    let equal_wire = fixture.upstream.lines()[2].clone();
    assert_eq!(
        fixture.upstream.lines()[4],
        equal_wire.replace(
            "to_publication_date%3A2024-02-29",
            "to_publication_date%3A2024-03-01"
        )
    );
    assert_eq!(fixture.keys("search:*").await.len(), 5);
    broad.as_object_mut().unwrap().remove("max_results");
    assert_result(
        &call(&client, broad).await,
        &["2024-02-29", "2024-03-01"],
        &broad_proof,
        true,
    );
    assert_eq!(fixture.upstream.lines().len(), 5);

    let equal_key = equal_key.unwrap();
    let mut conn = fixture.connection().await;
    let original: String = redis::cmd("GET")
        .arg(&equal_key)
        .query_async(&mut conn)
        .await
        .unwrap();
    let original: ssx_json::Value = ssx_json::from_str(&original).unwrap();
    for (index, mutation) in ["missing", "mismatched"].into_iter().enumerate() {
        let mut corrupted = original.clone();
        if mutation == "missing" {
            corrupted
                .as_object_mut()
                .unwrap()
                .shift_remove("date_filter");
        } else {
            corrupted["date_filter"] = PublicationDateProof::openalex(
                Some("2024-02-28".into()),
                Some("2024-02-28".into()),
            )
            .unwrap()
            .to_value();
        }
        let _: () = redis::cmd("SET")
            .arg(&equal_key)
            .arg(ssx_json::to_string(&corrupted).unwrap())
            .query_async(&mut conn)
            .await
            .unwrap();
        assert_result(
            &call(&client, args()).await,
            &["2024-02-29"],
            &proof(Some("2024-02-29"), Some("2024-02-29")),
            false,
        );
        assert_eq!(
            fixture.upstream.lines().len(),
            index + 6,
            "corrupt proof did not reacquire actual OpenAlex"
        );
        assert_eq!(fixture.upstream.lines()[index + 5], equal_wire);
        let repaired: String = redis::cmd("GET")
            .arg(&equal_key)
            .query_async(&mut conn)
            .await
            .unwrap();
        let repaired = storage::decode_response(ssx_json::from_str(&repaired).unwrap()).unwrap();
        assert_eq!(
            repaired.date_filter.as_ref().unwrap().to_value(),
            ssx_json::Value::from(proof(Some("2024-02-29"), Some("2024-02-29")))
        );
        assert_eq!(repaired.scope.selected_engines, ["openalex"]);
        assert_eq!(repaired.results.len(), 1);
        assert_eq!(
            repaired.results[0].published_date.as_deref(),
            Some("2024-02-29")
        );
        assert_eq!(fixture.keys("search:*").await.len(), 5);
    }
    assert_eq!(fixture.counts.adapter.load(Ordering::SeqCst), 7);
    drop(conn);
    client.cancel().await.unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn science_date_denials_never_reach_real_adapter_or_shared_acquisition() {
    let mut failures = Vec::new();
    for (mode, label, changes, code) in [
        (Mode::NoScienceGrant, "grant", json!({}), "tool_disabled"),
        (Mode::Unaudited, "unaudited", json!({}), "invalid_input"),
        (Mode::Unavailable, "unavailable", json!({}), "invalid_scope"),
        (
            Mode::Audited,
            "invalid_leap",
            json!({"date_from":"1900-02-29"}),
            "invalid_input",
        ),
        (
            Mode::Audited,
            "empty",
            json!({"date_from":""}),
            "invalid_input",
        ),
        (Mode::Audited, "implicit", json!({}), "invalid_input"),
        (
            Mode::Audited,
            "reversed",
            json!({"date_from":"2024-03-01"}),
            "invalid_input",
        ),
        (
            Mode::Audited,
            "relative",
            json!({"time_range":"week"}),
            "unexpected_keyword_argument",
        ),
        (
            Mode::BroadAvailable,
            "mixed",
            json!({"engines":["openalex","brave"]}),
            "invalid_input",
        ),
        (
            Mode::BroadAvailable,
            "unsupported",
            json!({"engines":["brave"]}),
            "invalid_input",
        ),
        (
            Mode::Audited,
            "sensitive",
            json!({"engines":["openalex","hibp"]}),
            "tool_disabled",
        ),
        (
            Mode::Audited,
            "safe",
            json!({"safesearch":"strict"}),
            "unexpected_keyword_argument",
        ),
    ] {
        let fixture = Fixture::new(mode, 200, equal_body(), None).await;
        let url = format!("http://{}/mcp", fixture.host.address);
        let raw = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let mut case_errors = Vec::new();
        for token in [None, Some("wrong-token")] {
            let mut request = raw.post(&url).json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":TOOL,"arguments":args()}}));
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            let status = request.send().await.unwrap().status();
            if status != reqwest::StatusCode::UNAUTHORIZED {
                case_errors.push(format!(
                    "auth token {token:?}: expected401, actual {status}"
                ));
            }
        }
        let client = fixture.connect().await;
        let mut request = args();
        request
            .as_object_mut()
            .unwrap()
            .extend(changes.as_object().unwrap().clone());
        if label == "implicit" {
            request.as_object_mut().unwrap().remove("engines");
        }
        // FastMCP validates declared arguments before the tool body. Science
        // has no time_range or safesearch parameter: these two are tool errors,
        // while the ten policy/date denials are structured successful results.
        let response = client
            .call_tool(
                CallToolRequestParams::new(TOOL.to_owned())
                    .with_arguments(request.as_object().unwrap().clone()),
            )
            .await
            .unwrap_or_else(|error| {
                panic!("{label}: request={request}; MCP call failed: {error:?}")
            });
        let envelope = serde_json::to_value(response).unwrap();
        let text = envelope["content"][0]["text"].as_str();
        let mut check = |condition: bool, detail: &str| {
            if !condition {
                case_errors.push(detail.to_owned());
            }
        };
        if let Some((field, input)) = match label {
            "relative" => Some(("time_range", "week")),
            "safe" => Some(("safesearch", "strict")),
            _ => None,
        } {
            let expected = format!(
                "1 validation error for call[{TOOL}]\n{field}\n  Unexpected keyword argument [type={code}, input_value='{input}', input_type=str]\n    For further information visit https://errors.pydantic.dev/2.13/v/{code}"
            );
            check(
                envelope["isError"] == true,
                "schema refusal must set isError=true",
            );
            check(
                envelope.get("structuredContent").is_none(),
                "schema refusal must not have structuredContent",
            );
            check(
                envelope["content"] == json!([{"type":"text","text":expected}]),
                "schema refusal must contain exactly the original FastMCP validation text",
            );
        } else {
            check(
                envelope["isError"] == false,
                "policy denial must set isError=false",
            );
            let result = text.and_then(|text| serde_json::from_str::<Value>(text).ok());
            check(result.is_some(), "policy tool text must be JSON");
            if let Some(result) = result {
                check(
                    envelope.get("structuredContent") == Some(&result),
                    "policy structuredContent must equal tool text",
                );
                check(result["error"]["code"] == code, "policy error code differs");
                if label == "unavailable" {
                    // Existing shared MCP/F09 explicit inactive-engine ordering.
                    check(
                        result
                            == json!({"error":{
                                "code":"invalid_scope","field":"engines",
                                "message":"unknown or inactive engines: openalex (inactive)",
                                "valid_alternatives":[]
                            }}),
                        "inactive OpenAlex must preserve the exact shared policy envelope",
                    );
                }
                if matches!(label, "mixed" | "unsupported") {
                    check(
                        result
                            == json!({"error":{
                                "code":"invalid_input","field":"engines",
                                "message":"absolute dates require explicit engines=['openalex']"
                            }}),
                        "eligible broad-engine date scope must reach the explicit OpenAlex date gate",
                    );
                }
                let field = match label {
                    "grant" => None,
                    "invalid_leap" | "empty" | "reversed" => Some("date_from"),
                    _ => Some("engines"),
                };
                check(
                    result["error"].get("field").and_then(Value::as_str) == field,
                    "policy error field differs",
                );
            }
        }
        let observed = fixture.counts.observations();
        check(
            observed == [0; 4],
            &format!("adapter/client/cache reads/writes must remainzero: {observed:?}"),
        );
        check(
            fixture.upstream.lines().is_empty(),
            "denied call reached actual OpenAlex HTTP",
        );
        check(
            fixture.keys("*").await.is_empty(),
            "denied MCP call wrote shared state",
        );
        if !case_errors.is_empty() {
            failures.push(format!(
                "{label}: request={request}; envelope={envelope}; {}",
                case_errors.join("; ")
            ));
        }
        client.cancel().await.unwrap();
        fixture.close().await;
    }
    assert!(
        failures.is_empty(),
        "science denial contract mismatches:\n{}",
        failures.join("\n")
    );
}

#[tokio::test]
async fn science_real_openalex_empty_and_http_failure_remain_distinct() {
    for (status, expected) in [(200, "ok"), (500, "error")] {
        let fixture = Fixture::new(
            Mode::Audited,
            status,
            br#"{"results":[]}"#.to_vec(),
            Some(1),
        )
        .await;
        let client = fixture.connect().await;
        let result = call(&client, args()).await;
        assert_eq!(fixture.upstream.lines().len(), 1);
        assert_eq!(
            fixture.upstream.lines()[0],
            "GET /works?search=date%20control%20/%20%2B&per_page=10&filter=from_publication_date%3A2024-02-29%2Cto_publication_date%3A2024-02-29 HTTP/1.1"
        );
        if status == 200 {
            assert_result(
                &result,
                &[],
                &proof(Some("2024-02-29"), Some("2024-02-29")),
                false,
            );
        } else {
            assert_eq!(result["error"]["code"], "all_engines_failed");
            assert_eq!(
                result["error"]["engine_outcomes"].as_array().unwrap().len(),
                1
            );
            assert_eq!(result["error"]["engine_outcomes"][0]["engine"], "openalex");
            assert_eq!(result["error"]["engine_outcomes"][0]["status"], expected);
            assert!(
                result.get("results").is_none(),
                "upstream failure must not become empty success"
            );
        }
        client.cancel().await.unwrap();
        fixture.close().await;
    }
}
