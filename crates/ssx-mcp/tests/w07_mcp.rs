//! Authenticated native transport against the same private Valkey/search owner.
//! Execution belongs to the exclusive root qualification controller.
#[path = "t06_support/fixture.rs"]
mod engine_fixture;
#[path = "t06_support/server.rs"]
mod private_valkey;
use sha2::{Digest, Sha256};
use ssx_application::{
    NoopSink,
    policy::{PolicyService, SearchPolicy},
    saved::SavedLimits,
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
    snapshot::{SnapshotService, SnapshotTtl},
    staged,
};
use ssx_domain::{EngineStatus, TenantId};
use ssx_infra::{
    search::{
        EngineBinding, RoutingIdentity, SearchCancellation, SearchClock, SearchConfig,
        SearchDependencies, SearchService, SystemSearchClock,
    },
    valkey::{
        Valkey,
        snapshot::{RandomSnapshotIds, SystemSnapshotClock, ValkeySnapshots},
    },
};
use ssx_json::{Value, json};
use ssx_mcp::{
    Result,
    evidence::{
        authority::{EvidenceAuthority, EvidenceAuthoritySnapshot, SearchPolicyAdapter},
        factory::EvidenceFactory,
        tools::TOOL_NAMES,
    },
    harness::Host,
    oauth::NetworkAuth,
    search_surface::{DiscoveryState, SearchState},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

struct Current(RwLock<EvidenceAuthoritySnapshot>);
impl EvidenceAuthority for Current {
    fn snapshot(&self) -> EvidenceAuthoritySnapshot {
        self.0.read().unwrap().clone()
    }
}
struct Discovery;
impl DiscoveryState for Discovery {
    fn capabilities(&self) -> Vec<Value> {
        vec![]
    }
    fn diagnostics(&self) -> Value {
        json!({"valkey_connected":true})
    }
}
fn server() -> private_valkey::Server {
    let path =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER").expect("explicit qualified private Valkey");
    let expected =
        std::env::var("SLOPSEARX_TEST_VALKEY_SERVER_SHA256").expect("pinned private Valkey digest");
    let metadata = std::fs::symlink_metadata(&path).unwrap();
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    let actual: String = Sha256::digest(std::fs::read(path).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(actual, expected);
    assert!(
        !cfg!(target_os = "macos")
            || std::env::temp_dir().starts_with("/Volumes/tank01/magnus/rust-builds/slopsearx")
    );
    private_valkey::Server::start()
}
struct Fixture {
    factory: EvidenceFactory,
    service: SearchService,
    authority: Arc<Current>,
    dispatches: Arc<AtomicUsize>,
}
impl Fixture {
    fn new(url: &str, source_url: &str) -> Self {
        let backend = Arc::new(Valkey::new(url, Duration::from_secs(2)).unwrap());
        let clock: Arc<dyn SearchClock> = Arc::new(SystemSearchClock);
        let mut search_policy = SearchPolicy::default();
        search_policy.grants.insert("retrieval_receipts".into());
        let policy = PolicyService {
            policy: search_policy,
            catalog: CatalogSnapshot::new(vec![EngineCapability {
                name: "wikipedia".into(),
                enabled: true,
                active: true,
                available: true,
                categories: vec!["general".into()],
                media_types: vec![],
                auth_ready: true,
                circuit_open: false,
                cost_class: "free".into(),
                health_status: "unknown".into(),
                health_stale: false,
                enforced_filters: BTreeMap::new(),
            }])
            .unwrap(),
            resolver: ScopeResolver {
                tier1: BTreeSet::from(["wikipedia".into()]),
                ..Default::default()
            },
        };
        let authority = Arc::new(Current(RwLock::new(EvidenceAuthoritySnapshot {
            search: policy.clone(),
            saved_limits: SavedLimits::default(),
            staged_limits: staged::Limits::default(),
            auth_configured: BTreeMap::from([("wikipedia".into(), false)]),
            valkey_connected: true,
        })));
        let dispatches = Arc::new(AtomicUsize::new(0));
        let engine = engine_fixture::FakeEngine {
            name: "wikipedia".into(),
            calls: dispatches.clone(),
            dispatch_receipt: None,
            count: 1,
            status: EngineStatus::Ok,
            delay: Duration::ZERO,
            results: Some(vec![ssx_domain::SearchResult {
                title: "captured source".into(),
                url: source_url.into(),
                engine: "wikipedia".into(),
                engines: BTreeSet::from(["wikipedia".into()]),
                ..Default::default()
            }]),
        };
        let service = SearchService::new(
            SearchConfig::default(),
            SearchDependencies {
                policy: policy.clone(),
                engines: BTreeMap::from([(
                    "wikipedia".into(),
                    EngineBinding {
                        engine: Arc::new(engine),
                        generation: "w07-private".into(),
                        timeout: Duration::from_secs(2),
                        scrape: false,
                    },
                )]),
                routing_identity: RoutingIdentity {
                    catalog: None,
                    sensitive: policy.policy.sensitive_engines.clone(),
                    tier1: policy.resolver.tier1.clone(),
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
                owned_stores: vec![backend.clone()],
                clock: clock.clone(),
                reranker: None,
            },
        )
        .unwrap()
        .with_policy_source(Arc::new(SearchPolicyAdapter {
            authority: authority.clone(),
        }))
        .unwrap();
        let snapshots = SnapshotService::new(
            Arc::new(ValkeySnapshots::new(backend.clone())),
            Arc::new(SystemSnapshotClock),
            Arc::new(RandomSnapshotIds),
            TenantId::new("default").unwrap(),
            SnapshotTtl::new(3600).unwrap(),
        );
        let state = SearchState::new(service.clone(), snapshots, Arc::new(Discovery), 50).unwrap();
        let factory = EvidenceFactory::new(state, backend, authority.clone(), clock).unwrap();
        Self {
            factory,
            service,
            authority,
            dispatches,
        }
    }
}
async fn rpc(client: &reqwest::Client, url: &str, method: &str, params: Value) -> Result<Value> {
    let response = client
        .post(url)
        .header("content-type", "application/json")
        .header("accept", "application/json,text/event-stream")
        .header("mcp-protocol-version", "2025-11-25")
        .body(ssx_json::to_string(
            &json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}),
        )?)
        .send()
        .await?;
    assert_eq!(response.status(), 200);
    let raw = response.text().await?;
    let envelope: Value = ssx_json::from_str(&raw)?;
    assert!(envelope.get("error").is_none(), "{envelope}");
    Ok(envelope["result"].clone())
}
async fn call(client: &reqwest::Client, url: &str, name: &str, args: Value) -> Result<Value> {
    let result = rpc(
        client,
        url,
        "tools/call",
        json!({"name":name,"arguments":args}),
    )
    .await?;
    assert_eq!(result["isError"], false, "{result}");
    Ok(ssx_json::from_str(
        result["content"][0]["text"]
            .as_str()
            .ok_or("native text missing")?,
    )?)
}
#[tokio::test]
async fn authenticated_http_four_tools_replay_lineage_tenant_grants_and_no_source_fetch()
-> Result<()> {
    let server = server();
    let sentinel = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}/must-remain-inert", sentinel.local_addr()?);
    let source_fetches = Arc::new(AtomicUsize::new(0));
    let seen = source_fetches.clone();
    let sentinel_task = tokio::spawn(async move {
        while let Ok((mut stream, _)) = sentinel.accept().await {
            seen.fetch_add(1, Ordering::SeqCst);
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await;
        }
    });
    let fixture = Fixture::new(server.url(), &url);
    let stop = CancellationToken::new();
    let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("w07-token".into())?);
    let host = Host::start(fixture.factory.http(auth, stop.clone()), stop).await?;
    let endpoint = format!("http://{}/mcp", host.address);
    let client = reqwest::Client::builder()
        .no_proxy()
        .default_headers(reqwest::header::HeaderMap::from_iter([(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_static("Bearer w07-token"),
        )]))
        .build()?;
    let tools = rpc(&client, &endpoint, "tools/list", json!({})).await?;
    let actual: Vec<_> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(&actual[actual.len() - 4..], &TOOL_NAMES);
    let search = call(
        &client,
        &endpoint,
        "slopsearx_search",
        json!({"query":"retained source","include":["results"]}),
    )
    .await?;
    let id = search["results"][0]["result_id"]
        .as_str()
        .ok_or("result handle")?;
    let observation = json!({"result_id":id,"retriever":"transport-reader","idempotency_key":"once","status":"succeeded","captured_at":"2026-09-11T04:00:00Z","final_url":url,"capture_ref":"capture://inert"});
    let first = call(&client, &endpoint, TOOL_NAMES[0], observation.clone()).await?;
    assert_eq!(first["state"], "created");
    assert_eq!(first["receipt"]["observations_verified"], false);
    assert_eq!(
        call(&client, &endpoint, TOOL_NAMES[0], observation).await?["state"],
        "replayed"
    );
    let read = call(&client, &endpoint, TOOL_NAMES[1], json!({"result_id":id})).await?;
    assert_eq!(read["total"], 1);
    assert_eq!(read["source_snapshot_status"], "live");
    let manifest = call(
        &client,
        &endpoint,
        TOOL_NAMES[2],
        json!({"result_ids":[id]}),
    )
    .await?;
    assert_eq!(
        manifest["items"][0]["receipts"][0]["receipt_id"],
        first["receipt"]["receipt_id"]
    );
    assert_eq!(manifest["observations_verified"], false);
    let root = first["receipt"]["artifact"].clone();
    let graph = call(
        &client,
        &endpoint,
        TOOL_NAMES[3],
        json!({"artifact":root,"max_depth":1}),
    )
    .await?;
    assert_eq!(graph["contract"], "slopsearx.artifact_lineage");
    assert_eq!(graph["nodes"][0]["status"], "live");
    let selected = call(
        &client,
        &endpoint,
        TOOL_NAMES[2],
        json!({"artifacts":[first["receipt"]["artifact"]]}),
    )
    .await?;
    assert_eq!(selected["items"][0]["result_id"], id);
    assert_eq!(
        fixture
            .factory
            .surface()
            .execute_lossless(
                TOOL_NAMES[1],
                json!({"result_id":id}).as_object().unwrap().clone(),
                TenantId::new("foreign")?,
                SearchCancellation::default()
            )
            .await?["error"]["code"],
        "invalid_result_id"
    );
    fixture
        .authority
        .0
        .write()
        .unwrap()
        .search
        .policy
        .grants
        .remove("retrieval_receipts");
    assert_eq!(
        call(&client, &endpoint, TOOL_NAMES[1], json!({"result_id":id})).await?["error"]["code"],
        "tool_disabled"
    );
    let denied = call(
        &client,
        &endpoint,
        TOOL_NAMES[3],
        json!({"artifact":first["receipt"]["artifact"]}),
    )
    .await?;
    assert_eq!(denied["nodes"][0]["status"], "policy_denied");
    assert_eq!(fixture.dispatches.load(Ordering::SeqCst), 1);
    assert_eq!(source_fetches.load(Ordering::SeqCst), 0);
    host.shutdown().await?;
    fixture.service.shutdown().await;
    sentinel_task.abort();
    let _ = sentinel_task.await;
    Ok(())
}

struct SuspendedRead {
    entered: tokio::sync::Notify,
    cancelled: tokio::sync::Notify,
    resume: tokio::sync::Notify,
    reads: AtomicUsize,
    drained: std::sync::atomic::AtomicBool,
}
impl ssx_application::receipts::ReceiptRepository for SuspendedRead {
    fn replay<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a str,
        _: &'a str,
        _: &'a str,
        _: ssx_domain::WallClockTimestamp,
        _: &'a ssx_application::receipts::Cancellation,
    ) -> ssx_application::PortFuture<
        'a,
        std::result::Result<ssx_application::receipts::Replay, ssx_application::receipts::Failure>,
    > {
        unreachable!("read fixture must not replay or write")
    }
    fn submit<'a>(
        &'a self,
        _: ssx_application::receipts::Insert<'a>,
    ) -> ssx_application::PortFuture<
        'a,
        std::result::Result<
            ssx_application::receipts::Submission,
            ssx_application::receipts::Failure,
        >,
    > {
        unreachable!("read fixture must not replay or write")
    }
    fn read<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a str,
        _: ssx_domain::WallClockTimestamp,
        _: usize,
        cancel: &'a ssx_application::receipts::Cancellation,
    ) -> ssx_application::PortFuture<
        'a,
        std::result::Result<
            ssx_application::receipts::Retained,
            ssx_application::receipts::Failure,
        >,
    > {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.entered.notify_one();
            while !cancel.is_cancelled() {
                tokio::task::yield_now().await;
            }
            self.cancelled.notify_one();
            self.resume.notified().await;
            self.drained.store(true, Ordering::SeqCst);
            Err(ssx_application::receipts::Failure::Cancelled)
        })
    }
}
struct UnusedSource;
impl ssx_application::receipts::service::SourceReader for UnusedSource {
    fn discovery<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a str,
        _: &'a ssx_application::receipts::Cancellation,
    ) -> ssx_application::PortFuture<
        'a,
        std::result::Result<
            ssx_application::receipts::service::Source,
            ssx_application::receipts::Failure,
        >,
    > {
        unreachable!("cancelled repository must drain before source lookup")
    }
}
impl ssx_application::receipts::service::ManifestSources for UnusedSource {
    fn resolve<'a>(
        &'a self,
        _: &'a TenantId,
        _: &'a ssx_domain::ArtifactRef,
        _: &'a ssx_application::receipts::Cancellation,
    ) -> ssx_application::PortFuture<
        'a,
        std::result::Result<ssx_application::receipts::service::ResolvedManifestSource, Value>,
    > {
        unreachable!("read fixture must not resolve manifests")
    }
}
struct Granted;
impl ssx_application::receipts::service::Access for Granted {
    fn enabled(&self) -> bool {
        true
    }
    fn connected(&self) -> bool {
        true
    }
}
#[tokio::test]
async fn handler_cancellation_signals_and_drains_existing_receipt_operation_before_completion() {
    let server = server();
    let fixture = Fixture::new(server.url(), "https://inert.invalid");
    let repo = Arc::new(SuspendedRead {
        entered: Default::default(),
        cancelled: Default::default(),
        resume: Default::default(),
        reads: AtomicUsize::new(0),
        drained: Default::default(),
    });
    let lineage = fixture
        .factory
        .surface()
        .evidence()
        .unwrap()
        .lineage
        .clone();
    let evidence = Arc::new(ssx_mcp::evidence::EvidenceRuntime {
        service: ssx_application::receipts::service::Service::new(
            repo.clone(),
            Arc::new(UnusedSource),
            Arc::new(Granted),
            Arc::new(ssx_mcp::evidence::EvidenceClock {
                clock: Arc::new(SystemSearchClock),
            }),
            lineage.clone(),
            Arc::new(UnusedSource),
        ),
        lineage,
    });
    let surface =
        ssx_mcp::search_surface::SearchSurface::new(fixture.factory.surface().state().clone())
            .with_evidence(evidence)
            .unwrap();
    let cancel = SearchCancellation::default();
    let operation = surface.execute_lossless(
        TOOL_NAMES[1],
        json!({"result_id":"snap:0"}).as_object().unwrap().clone(),
        TenantId::new("a").unwrap(),
        cancel.clone(),
    );
    tokio::pin!(operation);
    tokio::select! { _ = repo.entered.notified() => {}, result = &mut operation => panic!("completed before repository read: {result:?}") }
    cancel.cancel();
    tokio::select! { _ = repo.cancelled.notified() => {}, result = &mut operation => panic!("completed before cancellation reached repository: {result:?}") }
    assert!(!repo.drained.load(Ordering::SeqCst));
    tokio::select! { biased; result = &mut operation => panic!("completed before read drained: {result:?}"), _ = tokio::task::yield_now() => {} }
    repo.resume.notify_one();
    assert!(operation.await.is_err());
    assert!(repo.drained.load(Ordering::SeqCst));
    let cancelled = SearchCancellation::default();
    cancelled.cancel();
    assert!(
        surface
            .execute_lossless(
                TOOL_NAMES[1],
                json!({"result_id":"snap:0"}).as_object().unwrap().clone(),
                TenantId::new("a").unwrap(),
                cancelled
            )
            .await
            .is_err()
    );
    assert_eq!(repo.reads.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.dispatches.load(Ordering::SeqCst), 0);
    fixture.service.shutdown().await;
}

async fn legacy_event(
    stream: &mut reqwest::Response,
    pending: &mut Vec<u8>,
) -> Result<(String, String)> {
    loop {
        if let Some(end) = pending.windows(2).position(|bytes| bytes == b"\n\n") {
            let event = pending.drain(..end + 2).collect::<Vec<_>>();
            let event = String::from_utf8(event)?;
            let kind = event
                .lines()
                .find_map(|line| line.strip_prefix("event: "))
                .unwrap_or("message")
                .to_owned();
            let data = event
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .collect::<Vec<_>>()
                .join("\n");
            if !data.is_empty() {
                return Ok((kind, data));
            }
        }
        let chunk = tokio::time::timeout(Duration::from_secs(10), stream.chunk())
            .await??
            .ok_or("legacy stream EOF")?;
        pending.extend_from_slice(&chunk);
        assert!(pending.len() < 4 * 1024 * 1024);
    }
}
async fn legacy_rpc(
    client: &reqwest::Client,
    url: &str,
    stream: &mut reqwest::Response,
    pending: &mut Vec<u8>,
    id: u64,
    method: &str,
    params: Value,
) -> Result<Value> {
    let response = client
        .post(url)
        .header("content-type", "application/json")
        .body(ssx_json::to_string(
            &json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
        )?)
        .send()
        .await?;
    assert_eq!(response.status(), 202);
    let (_, raw) = legacy_event(stream, pending).await?;
    let response: Value = ssx_json::from_str(&raw)?;
    assert_eq!(response["id"], id);
    assert!(response.get("error").is_none(), "{response}");
    Ok(response["result"].clone())
}
#[tokio::test]
async fn authenticated_legacy_factory_advertises_and_dispatches_four_evidence_tools() -> Result<()>
{
    let server = server();
    let fixture = Fixture::new(server.url(), "https://inert.invalid");
    let stop = CancellationToken::new();
    let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("legacy-w07".into())?);
    let router = fixture.factory.legacy(
        auth,
        "/sse",
        "/messages/",
        Duration::from_secs(60),
        stop.clone(),
    )?;
    let host = Host::start(router, stop).await?;
    let base = format!("http://{}", host.address);
    let unauthenticated = reqwest::Client::builder().no_proxy().build()?;
    assert_eq!(
        unauthenticated
            .get(format!("{base}/sse"))
            .send()
            .await?
            .status(),
        401
    );
    let client = reqwest::Client::builder()
        .no_proxy()
        .default_headers(reqwest::header::HeaderMap::from_iter([(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_static("Bearer legacy-w07"),
        )]))
        .build()?;
    let mut stream = client
        .get(format!("{base}/sse"))
        .header("accept", "text/event-stream")
        .send()
        .await?;
    assert_eq!(stream.status(), 200);
    let mut pending = vec![];
    let (kind, endpoint) = legacy_event(&mut stream, &mut pending).await?;
    assert_eq!(kind, "endpoint");
    let messages = format!("{base}{endpoint}");
    legacy_rpc(&client,&messages,&mut stream,&mut pending,1,"initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"w07-native","version":"1"}})).await?;
    assert_eq!(
        client
            .post(&messages)
            .header("content-type", "application/json")
            .body(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .send()
            .await?
            .status(),
        202
    );
    let catalog = legacy_rpc(
        &client,
        &messages,
        &mut stream,
        &mut pending,
        2,
        "tools/list",
        json!({}),
    )
    .await?;
    let names: Vec<_> = catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(&names[names.len() - 4..], &TOOL_NAMES);
    let root = ssx_domain::ArtifactRef::new(ssx_domain::ArtifactKind::Snapshot, "missing")?;
    let cases = [
        (
            TOOL_NAMES[0],
            json!({"result_id":"missing:0","retriever":"reader","idempotency_key":"once","status":"failed","failure_code":"not_found"}),
        ),
        (TOOL_NAMES[1], json!({"result_id":"missing:0"})),
        (TOOL_NAMES[2], json!({"result_ids":["missing:0"]})),
        (TOOL_NAMES[3], json!({"artifact":root.to_value()})),
    ];
    for (i, (name, args)) in cases.into_iter().enumerate() {
        let result = legacy_rpc(
            &client,
            &messages,
            &mut stream,
            &mut pending,
            i as u64 + 3,
            "tools/call",
            json!({"name":name,"arguments":args}),
        )
        .await?;
        assert_eq!(result["isError"], false, "{result}");
        let value: Value =
            ssx_json::from_str(result["content"][0]["text"].as_str().ok_or("tool text")?)?;
        match i {
            0 | 1 => assert_eq!(value["error"]["code"], "invalid_result_id"),
            2 => assert_eq!(value["contract"], "slopsearx.research_manifest"),
            _ => assert_eq!(value["nodes"][0]["status"], "missing"),
        }
    }
    assert_eq!(fixture.dispatches.load(Ordering::SeqCst), 0);
    drop(stream);
    host.shutdown().await?;
    fixture.service.shutdown().await;
    Ok(())
}
