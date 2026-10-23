//! Incremental seam qualification only: test transport is not the protected
//! production TLS/proxy/decoder profile. No original-source completion claims.
use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, Method, StatusCode},
    response::Response,
    routing::any,
};
use ssx_mcp::{
    Result,
    opaque::{
        GatewayChunkFuture, GatewayHttp, GatewayHttpRequest, GatewayHttpResponse,
        GatewayOpenFuture, RawGatewayStart,
    },
};
use std::{
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{Notify, OwnedSemaphorePermit, Semaphore},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
#[derive(Clone)]
struct FixtureState {
    pending: bool,
    fail_catalog: bool,
    delete_unsupported_once: bool,
    deleted: Arc<Mutex<Vec<String>>>,
    body_drops: Arc<AtomicUsize>,
    changed: Arc<Notify>,
}
struct BodyDrop(FixtureState);
impl Drop for BodyDrop {
    fn drop(&mut self) {
        self.0.body_drops.fetch_add(1, Ordering::SeqCst);
        self.0.changed.notify_waiters();
    }
}
async fn handle(
    State(state): State<FixtureState>,
    method: Method,
    headers: HeaderMap,
    body: String,
) -> Response {
    if method == Method::DELETE {
        let mut deleted = state.deleted.lock().unwrap();
        deleted.push(
            headers
                .get("mcp-session-id")
                .unwrap()
                .to_str()
                .unwrap()
                .into(),
        );
        let status = if state.delete_unsupported_once && deleted.len() == 1 {
            StatusCode::METHOD_NOT_ALLOWED
        } else {
            StatusCode::NO_CONTENT
        };
        return Response::builder()
            .status(status)
            .body(Body::empty())
            .unwrap();
    }
    let request: serde_json::Value = serde_json::from_str(&body).unwrap();
    match request["method"].as_str().unwrap() {
        "initialize" => {
            let first = if state.pending {
                Bytes::from_static(b": headers-ready\n\n")
            } else {
                Bytes::from_static(b"data: {\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"serverInfo\":{\"name\":\"synthetic\",\"version\":\"1\"}}}\n\n")
            };
            let stream = futures_util::stream::unfold(
                (Some(first), BodyDrop(state.clone())),
                |(mut first, guard)| async move {
                    let chunk = if let Some(first) = first.take() {
                        first
                    } else {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        Bytes::from_static(b": still-open\n\n")
                    };
                    Some((Ok::<_, Infallible>(chunk), (first, guard)))
                },
            );
            Response::builder()
                .header("content-type", "text/event-stream")
                .header("mcp-session-id", "synthetic-session")
                .body(Body::from_stream(stream))
                .unwrap()
        }
        "notifications/initialized" => Response::builder().status(204).body(Body::empty()).unwrap(),
        "tools/list" if state.fail_catalog => Response::builder()
            .status(503)
            .body(Body::from("synthetic catalog failure"))
            .unwrap(),
        name => {
            let result = if name == "tools/list" {
                serde_json::json!({"tools":[]})
            } else {
                serde_json::json!({})
            };
            Response::builder()
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"jsonrpc":"2.0","id":request["id"],"result":result})
                        .to_string(),
                ))
                .unwrap()
        }
    }
}
struct Fixture {
    url: String,
    state: FixtureState,
    stop: CancellationToken,
    task: Option<JoinHandle<std::io::Result<()>>>,
}
impl Fixture {
    async fn new(pending: bool, fail_catalog: bool, delete_unsupported_once: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let state = FixtureState {
            pending,
            fail_catalog,
            delete_unsupported_once,
            deleted: Default::default(),
            body_drops: Default::default(),
            changed: Default::default(),
        };
        let stop = CancellationToken::new();
        let stopped = stop.clone();
        let app = Router::new()
            .route("/mcp", any(handle))
            .with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(stopped.cancelled_owned())
                .await
        });
        Self {
            url: format!("http://{address}/mcp"),
            state,
            stop,
            task: Some(task),
        }
    }
    async fn shutdown(&mut self) {
        self.stop.cancel();
        if let Some(task) = self.task.as_mut() {
            tokio::time::timeout(Duration::from_secs(2), task)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        }
        self.task.take();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.cancel();
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
struct TestHttp {
    client: reqwest::Client,
    slots: Arc<Semaphore>,
    headers: AtomicUsize,
    changed: Notify,
}
impl TestHttp {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            slots: Arc::new(Semaphore::new(1)),
            headers: AtomicUsize::new(0),
            changed: Notify::new(),
        })
    }
}
struct LeasedResponse {
    response: reqwest::Response,
    _permit: OwnedSemaphorePermit,
}
impl GatewayHttpResponse for LeasedResponse {
    fn status(&self) -> StatusCode {
        self.response.status()
    }
    fn headers(&self) -> &HeaderMap {
        self.response.headers()
    }
    fn content_length(&self) -> Option<u64> {
        self.response.content_length()
    }
    fn chunk(&mut self) -> GatewayChunkFuture<'_> {
        Box::pin(async move { Ok(self.response.chunk().await?) })
    }
}
impl GatewayHttp for TestHttp {
    fn open(&self, request: GatewayHttpRequest) -> GatewayOpenFuture<'_> {
        Box::pin(async move {
            let permit = self.slots.clone().acquire_owned().await?;
            let mut builder = self.client.request(request.method, request.url);
            for (name, value) in request.headers {
                builder = builder.header(name, value);
            }
            if let Some(body) = request.body {
                builder = builder.body(body);
            }
            let response = builder.send().await?;
            self.headers.fetch_add(1, Ordering::SeqCst);
            self.changed.notify_waiters();
            Ok(Box::new(LeasedResponse {
                response,
                _permit: permit,
            }) as Box<dyn GatewayHttpResponse>)
        })
    }
}
async fn wait_body_drop(state: &FixtureState) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let changed = state.changed.notified();
            if state.body_drops.load(Ordering::SeqCst) > 0 {
                return;
            }
            changed.await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn correlated_sse_completes_before_eof_and_releases_actual_body_lease() -> Result<()> {
    let mut fixture = Fixture::new(false, false, true).await;
    let http = TestHttp::new();
    let mut start = RawGatewayStart::new(
        fixture.url.clone(),
        Some("synthetic-bearer".into()),
        http.clone(),
    )?;
    assert!(start.ready().is_err());
    tokio::time::timeout(Duration::from_secs(2), start.initialize(None)).await??;
    assert!(start.ready().is_err());
    assert_eq!(http.slots.available_permits(), 1);
    wait_body_drop(&fixture.state).await;
    start.discover_tools(Duration::from_secs(2)).await?;
    let gateway = start.ready()?;
    assert!(gateway.catalogs.contains_key("tools/list"));
    assert_eq!(gateway.catalogs.len(), 1);
    let reply = gateway
        .rpc(r#"{"jsonrpc":"2.0","id":99,"method":"ping"}"#)
        .await?
        .unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&reply)?["id"], 99);
    start.close().await?;
    assert!(start.ready().is_err());
    start.close().await?;
    start.close().await?;
    assert_eq!(
        &*fixture.state.deleted.lock().unwrap(),
        &["synthetic-session", "synthetic-session"]
    );
    assert_eq!(http.slots.available_permits(), 1);
    fixture.shutdown().await;
    Ok(())
}
#[tokio::test]
async fn dropped_unbounded_initialize_retains_session_for_delete_and_disables_readiness()
-> Result<()> {
    let mut fixture = Fixture::new(true, false, false).await;
    let http = TestHttp::new();
    let mut start = RawGatewayStart::new(fixture.url.clone(), None, http.clone())?;
    let mut operation = Box::pin(start.initialize(None));
    tokio::time::timeout(Duration::from_secs(2),async{tokio::select!{biased;result=&mut operation=>panic!("pending SSE initialize returned {result:?}"),_=async{loop{let changed=http.changed.notified();if http.headers.load(Ordering::SeqCst)>0{return;}changed.await;}}=>{}}}).await.unwrap();
    assert_eq!(http.slots.available_permits(), 0);
    drop(operation);
    assert_eq!(http.slots.available_permits(), 1);
    assert!(start.ready().is_err());
    wait_body_drop(&fixture.state).await;
    start.close().await?;
    assert_eq!(
        &*fixture.state.deleted.lock().unwrap(),
        &["synthetic-session"]
    );
    fixture.shutdown().await;
    Ok(())
}
#[tokio::test]
async fn failed_discovery_keeps_cleanup_owner_and_zero_budget_dispatches_nothing() -> Result<()> {
    let mut fixture = Fixture::new(false, true, false).await;
    let http = TestHttp::new();
    let mut zero = RawGatewayStart::new(fixture.url.clone(), None, http.clone())?;
    assert!(zero.initialize(Some(Duration::ZERO)).await.is_err());
    assert_eq!(http.headers.load(Ordering::SeqCst), 0);
    assert!(zero.ready().is_err());
    zero.close().await?;
    let mut start = RawGatewayStart::new(fixture.url.clone(), None, http.clone())?;
    start.initialize(None).await?;
    assert!(start.discover_tools(Duration::from_secs(2)).await.is_err());
    assert!(start.ready().is_err());
    start.close().await?;
    assert_eq!(
        &*fixture.state.deleted.lock().unwrap(),
        &["synthetic-session"]
    );
    assert_eq!(http.slots.available_permits(), 1);
    wait_body_drop(&fixture.state).await;
    fixture.shutdown().await;
    Ok(())
}
