//! Authenticated native transport against the same private Valkey/search owner.
//! Execution belongs to the exclusive root qualification controller.
#[path = "w07_support/mcp.rs"]
mod fixture;
#[path = "t06_support/server.rs"]
mod private_valkey;
use fixture::Fixture;
use sha2::{Digest, Sha256};
use ssx_domain::TenantId;
use ssx_infra::search::{SearchCancellation, SystemSearchClock};
use ssx_json::{Value, json};
use ssx_mcp::{Result, evidence::tools::TOOL_NAMES, harness::Host, oauth::NetworkAuth};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;
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

struct StdioProcess {
    child: tokio::process::Child,
    input: Option<tokio::process::ChildStdin>,
    output: tokio::io::BufReader<tokio::process::ChildStdout>,
    errors: tokio::io::BufReader<tokio::process::ChildStderr>,
    id: u64,
}
impl StdioProcess {
    fn start(url: &str, source: &str) -> Result<Self> {
        use std::process::Stdio;
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_evidence-qualification"))
            .args(["stdio", url, source])
            .env_clear()
            .env("PATH", "")
            .env("TMPDIR", std::env::temp_dir())
            .env("TZ", "UTC")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        Ok(Self {
            input: Some(child.stdin.take().ok_or("child stdin")?),
            output: tokio::io::BufReader::new(child.stdout.take().ok_or("child stdout")?),
            errors: tokio::io::BufReader::new(child.stderr.take().ok_or("child stderr")?),
            child,
            id: 0,
        })
    }
    async fn send(&mut self, request: Value) -> Result<()> {
        let input = self.input.as_mut().ok_or("stdio already closed")?;
        input
            .write_all(ssx_json::to_string(&request)?.as_bytes())
            .await?;
        input.write_all(b"\n").await?;
        input.flush().await?;
        Ok(())
    }
    async fn rpc(&mut self, method: &str, params: Value) -> Result<Value> {
        self.id += 1;
        let id = self.id;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        loop {
            let mut line = String::new();
            let read =
                tokio::time::timeout(Duration::from_secs(5), self.output.read_line(&mut line))
                    .await??;
            need(
                read > 0 && line.len() <= 4 * 1024 * 1024,
                "bounded child response missing",
            )?;
            let response: Value = ssx_json::from_str(&line)?;
            if response.get("id").is_none() {
                continue;
            }
            need(response["id"] == id, "stdio response ID mismatch")?;
            need(
                response.get("error").is_none(),
                "unexpected stdio protocol error",
            )?;
            return Ok(response["result"].clone());
        }
    }
    async fn call(&mut self, name: &str, args: Value) -> Result<Value> {
        let result = self
            .rpc("tools/call", json!({"name":name,"arguments":args}))
            .await?;
        need(result["isError"] == false, "unexpected tool type error")?;
        Ok(ssx_json::from_str(
            result["content"][0]["text"]
                .as_str()
                .ok_or("native tool text")?,
        )?)
    }
    async fn pending_read(&mut self, id: u64) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let mut line = String::new();
                let read = self.errors.read_line(&mut line).await?;
                need(
                    read > 0 && line.len() < 65536,
                    "pending-read acknowledgement missing",
                )?;
                let value: Value = ssx_json::from_str(&line)?;
                if value["w07_read_pending"] == true && value["id"] == id {
                    return Ok::<_, Box<dyn std::error::Error + Send + Sync>>(());
                }
            }
        })
        .await?
    }
    async fn reap(&mut self) -> Result<()> {
        self.input.take();
        if self.child.try_wait()?.is_none() {
            self.child.start_kill()?;
            tokio::time::timeout(Duration::from_secs(3), self.child.wait()).await??;
        }
        Ok(())
    }
}
fn need(condition: bool, message: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
async fn stdio_journey(
    process: &mut StdioProcess,
    control: &mut redis::Connection,
    source: &str,
) -> Result<()> {
    use tokio::io::AsyncReadExt;
    process.rpc("initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"w07-native","version":"1"}})).await?;
    process
        .send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await?;
    let catalog = process.rpc("tools/list", json!({})).await?;
    let names: Vec<_> = catalog["tools"]
        .as_array()
        .ok_or("tool catalog")?
        .iter()
        .map(|v| v["name"].as_str().unwrap_or_default())
        .collect();
    need(
        names.ends_with(&TOOL_NAMES),
        "stdio evidence catalog missing or reordered",
    )?;
    let search = process
        .call(
            "slopsearx_search",
            json!({"query":"stdio evidence","include":["results"]}),
        )
        .await?;
    let id = search["results"][0]["result_id"]
        .as_str()
        .ok_or("stdio result handle")?;
    let observation = json!({"result_id":id,"retriever":"stdio-reader","idempotency_key":"stdio-once","status":"succeeded","captured_at":"2026-09-11T04:00:00Z","final_url":source,"capture_ref":"capture://inert"});
    let first = process.call(TOOL_NAMES[0], observation.clone()).await?;
    need(
        first["state"] == "created"
            && first["receipt"]["attribution"]["authenticated_tenant"] == "default",
        "trusted stdio tenant not retained",
    )?;
    need(
        first["receipt"]["observations_verified"] == false,
        "stdio receipt incorrectly verified",
    )?;
    need(
        process.call(TOOL_NAMES[0], observation.clone()).await?["state"] == "replayed",
        "stdio replay failed",
    )?;
    let mut forged = observation;
    forged["tenant"] = json!("foreign");
    let rejected = process
        .rpc(
            "tools/call",
            json!({"name":TOOL_NAMES[0],"arguments":forged}),
        )
        .await?;
    need(
        rejected["isError"] == true,
        "caller tenant argument escaped function boundary",
    )?;
    let read = process.call(TOOL_NAMES[1], json!({"result_id":id})).await?;
    need(
        read["total"] == 1 && read["source_snapshot_status"] == "live",
        "stdio retained read failed",
    )?;
    let manifest = process
        .call(TOOL_NAMES[2], json!({"result_ids":[id]}))
        .await?;
    need(
        manifest["items"][0]["receipts"][0]["receipt_id"] == first["receipt"]["receipt_id"]
            && manifest["observations_verified"] == false,
        "stdio manifest lost attributed receipt",
    )?;
    let graph = process
        .call(
            TOOL_NAMES[3],
            json!({"artifact":first["receipt"]["artifact"],"max_depth":1}),
        )
        .await?;
    need(
        graph["contract"] == "slopsearx.artifact_lineage" && graph["nodes"][0]["status"] == "live",
        "stdio lineage failed",
    )?;
    // The only controlled server is this test's disposable Unix-socket Valkey.
    // Pause exceeds the complete ACK/EOF guard, so automatic expiry cannot
    // masquerade as an explicitly released read during the success assertion.
    redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(30000)
        .arg("ALL")
        .query::<()>(control)?;
    process.id += 1;
    let pending_id = process.id;
    let read_started = std::time::Instant::now();
    process.send(json!({"jsonrpc":"2.0","id":pending_id,"method":"tools/call","params":{"name":TOOL_NAMES[1],"arguments":{"result_id":id}}})).await?;
    process.pending_read(pending_id).await?;
    process.input.take(); // Actual stdin EOF reaches the existing owned transport.
    // The shared fixture's store budget is two seconds. Refuse an inconclusive
    // delayed ACK/EOF rather than accepting a read that already timed out.
    need(
        read_started.elapsed() < Duration::from_secs(1),
        "EOF did not precede the held-read store deadline",
    )?;
    let exit = tokio::time::timeout(Duration::from_secs(8), process.child.wait()).await??;
    need(
        exit.success(),
        "evidence child did not exit cleanly before Valkey release",
    )?;
    let mut stderr = String::new();
    (&mut process.errors)
        .take(65537)
        .read_to_string(&mut stderr)
        .await?;
    need(stderr.len() <= 65536, "bounded child stderr exceeded")?;
    let rows = stderr
        .lines()
        .map(ssx_json::from_str::<Value>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    need(
        rows.iter().any(|v| {
            v["w07_read_drained"] == true && v["id"] == pending_id && v["context_cancelled"] == true
        }),
        "EOF did not cancel and drain the pending evidence handler",
    )?;
    need(
        rows.iter().any(|v| {
            v["w07_owner_shutdown"] == true && v["backend_closed"] == true && v["dispatches"] == 1
        }),
        "overall owner did not close the existing backend",
    )?;
    // No CLIENT UNPAUSE has been sent: ACK, handler drain, owner close and actual
    // child exit all precede releasing the held private server.
    Ok(())
}
#[tokio::test]
async fn actual_owned_stdio_four_tools_tenant_inert_urls_and_pending_receipt_eof_drain()
-> Result<()> {
    let server = server();
    let sentinel = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let source = format!("http://{}/must-remain-inert", sentinel.local_addr()?);
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
    let mut control = server.connection();
    if let Err(error) = control.set_read_timeout(Some(Duration::from_secs(2))) {
        sentinel_task.abort();
        let _ = sentinel_task.await;
        return Err(error.into());
    }
    let mut process = match StdioProcess::start(server.url(), &source) {
        Ok(process) => process,
        Err(error) => {
            sentinel_task.abort();
            let _ = sentinel_task.await;
            return Err(error);
        }
    };
    let result = stdio_journey(&mut process, &mut control, &source).await;
    // Retirement runs for success and every Result error/timeout. This command
    // targets only the owned fixture connection; no ambient application URL.
    let resumed = redis::cmd("CLIENT")
        .arg("UNPAUSE")
        .query::<()>(&mut control);
    let reaped = process.reap().await;
    sentinel_task.abort();
    let _ = sentinel_task.await;
    result?;
    resumed?;
    reaped?;
    need(
        source_fetches.load(Ordering::SeqCst) == 0,
        "evidence fixture fetched an inert URL",
    )?;
    Ok(())
}
