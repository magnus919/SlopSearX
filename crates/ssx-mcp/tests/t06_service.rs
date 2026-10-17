//! Actual shared service seams: preview authority, ownership, capture identity.
#[path = "t06_support/fixture.rs"]
mod fixture;
#[path = "t06_support/server.rs"]
mod private_valkey;
use ssx_application::{
    policy::{ScopeProposal, SearchPolicy, SearchSurface},
    snapshot::SnapshotLookup,
};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    jev::{JevConfig, JevRouter},
    search::{SearchCancellation, SearchConfig, SearchError},
};
use ssx_mcp::Result;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::io::AsyncReadExt;
fn engines() -> Vec<fixture::FakeEngine> {
    ["wikipedia", "pubmed"]
        .into_iter()
        .map(|name| fixture::FakeEngine {
            name: name.into(),
            calls: Default::default(),
            dispatch_receipt: None,
            count: 3,
            status: EngineStatus::Ok,
            delay: Duration::ZERO,
            results: None,
        })
        .collect()
}
fn preview() -> ScopeProposal {
    let mut p = ScopeProposal::generic(SearchRequest {
        query: "public unrelated query".into(),
        ..Default::default()
    });
    p.surface = SearchSurface::Preview;
    p
}
struct Provider {
    endpoint: String,
    started: Arc<tokio::sync::Semaphore>,
    closed: Arc<AtomicUsize>,
    cancel: tokio_util::sync::CancellationToken,
    task: tokio::task::JoinHandle<()>,
}
impl Provider {
    async fn new() -> Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("{e:?}"))?;
        let endpoint = format!("http://{}/v1/systemone", listener.local_addr()?);
        let started = Arc::new(tokio::sync::Semaphore::new(0));
        let closed = Arc::new(AtomicUsize::new(0));
        let cancel = tokio_util::sync::CancellationToken::new();
        let (s, c, ct) = (started.clone(), closed.clone(), cancel.clone());
        let task = tokio::spawn(async move {
            loop {
                let mut socket = tokio::select! {_=ct.cancelled()=>break,socket=listener.accept()=>socket.unwrap().0};
                let mut bytes = Vec::new();
                let mut buf = [0; 4096];
                loop {
                    let n = socket.read(&mut buf).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buf[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]);
                        let length = header
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .map(str::to_owned)
                            })
                            .and_then(|n| n.parse::<usize>().ok())
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                s.add_permits(1);
                tokio::select! {_=ct.cancelled()=>{},n=socket.read(&mut buf)=>{assert_eq!(n.unwrap(),0,"client connection did not close");c.fetch_add(1,Ordering::SeqCst);}}
            }
        });
        Ok(Self {
            endpoint,
            started,
            closed,
            cancel,
            task,
        })
    }
    fn router(&self) -> Arc<JevRouter> {
        Arc::new(JevRouter::with_endpoint(
            JevConfig::from_environment(&BTreeMap::from([
                ("TYPESAFE_API_KEY".into(), "fixture-only".into()),
                ("JEV_TIMEOUT_MS".into(), "5000".into()),
            ]))
            .unwrap(),
            ssx_registry::Registry::builtins().unwrap(),
            self.endpoint.clone(),
        ))
    }
    async fn observed(&self) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(3), self.started.acquire())
            .await
            .map_err(|e| format!("{e:?}"))??
            .forget();
        Ok(())
    }
    async fn close(self) -> Result<()> {
        self.cancel.cancel();
        tokio::time::timeout(Duration::from_secs(2), self.task)
            .await
            .map_err(|e| format!("{e:?}"))??;
        Ok(())
    }
}
#[tokio::test]
async fn preview_rejects_sensitive_and_disabled_before_optional_provider_and_never_dispatches()
-> Result<()> {
    let provider = Provider::new().await.map_err(|e| format!("{e:?}"))?;
    let server = private_valkey::Server::start();
    let fixture = fixture::Fixture::with_options(
        server.url(),
        SearchPolicy::default(),
        engines(),
        &["openalex"],
        SearchConfig::default(),
        Some(provider.router()),
    )
    .await;
    for engine in ["hibp", "openalex"] {
        let mut p = preview();
        p.request.engines = Some(vec!["wikipedia".into(), engine.into()]);
        let result = fixture.service.preview_scope(&p).await;
        assert!(matches!(result, Err(SearchError::Policy(_))));
    }
    assert_eq!(provider.started.available_permits(), 0);
    assert_eq!(fixture.calls(), 0);
    let mut explicit = preview();
    explicit.request.engines = Some(vec!["wikipedia".into()]);
    let value = fixture
        .service
        .preview_scope(&explicit)
        .await
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(value.scope.selected_engines, ["wikipedia"]);
    assert_eq!(fixture.calls(), 0);
    assert!(
        fixture.service.prepare_scope(&explicit).await.is_err(),
        "preview created dispatch authority"
    );
    fixture.service.shutdown().await;
    assert!(matches!(
        fixture.service.preview_scope(&explicit).await,
        Err(SearchError::Closed)
    ));
    provider.close().await.map_err(|e| format!("{e:?}"))?;
    Ok(())
}
#[tokio::test]
async fn preview_timeout_drop_busy_and_shutdown_drain_optional_http_ownership() -> Result<()> {
    for mode in ["timeout", "drop", "shutdown"] {
        let provider = Provider::new().await.map_err(|e| format!("{e:?}"))?;
        let server = private_valkey::Server::start();
        let fixture = fixture::Fixture::with_options(
            server.url(),
            SearchPolicy::default(),
            engines(),
            &[],
            SearchConfig {
                max_callers: 1,
                optional_port_timeout: if mode == "timeout" {
                    Duration::from_millis(150)
                } else {
                    Duration::from_secs(5)
                },
                ..Default::default()
            },
            Some(provider.router()),
        )
        .await;
        let service = fixture.service.clone();
        let task = tokio::spawn(async move { service.preview_scope(&preview()).await });
        provider.observed().await.map_err(|e| format!("{e:?}"))?;
        assert!(matches!(
            fixture.service.preview_scope(&preview()).await,
            Err(SearchError::Busy)
        ));
        match mode {
            "timeout" => {
                let result = tokio::time::timeout(Duration::from_secs(2), task)
                    .await
                    .map_err(|e| format!("{e:?}"))??;
                assert!(result.is_ok());
            }
            "drop" => {
                task.abort();
                assert!(task.await.unwrap_err().is_cancelled());
                let mut p = preview();
                p.request.engines = Some(vec!["wikipedia".into()]);
                assert!(fixture.service.preview_scope(&p).await.is_ok());
            }
            _ => {
                fixture.service.shutdown().await;
                assert!(matches!(
                    task.await.map_err(|e| format!("{e:?}"))?,
                    Err(SearchError::Closed)
                ));
            }
        }
        fixture.service.shutdown().await;
        let until = tokio::time::Instant::now() + Duration::from_secs(2);
        while provider.closed.load(Ordering::SeqCst) == 0 {
            assert!(
                tokio::time::Instant::now() < until,
                "optional HTTP ownership did not close"
            );
            tokio::task::yield_now().await;
        }
        assert_eq!(fixture.calls(), 0);
        provider.close().await.map_err(|e| format!("{e:?}"))?;
    }
    Ok(())
}
#[tokio::test]
async fn captured_snapshot_is_service_bound_full_include_independent_and_not_reexecuted()
-> Result<()> {
    let server = private_valkey::Server::start();
    let first = fixture::Fixture::new(server.url(), false).await;
    let second = fixture::Fixture::new(server.url(), false).await;
    let proposal = ScopeProposal::generic(SearchRequest {
        query: "captured identity".into(),
        engines: Some(vec!["wikipedia".into()]),
        include: Default::default(),
        max_results: Some(1),
        ..Default::default()
    });
    let capture = first
        .service
        .capture_read(proposal.clone(), SearchCancellation::default())
        .await
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(capture.full_result_count(), 3);
    assert_eq!(capture.present().results.len(), 1);
    let mut connection = server.connection();
    let before: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut connection)?;
    assert!(matches!(
        second
            .service
            .snapshot_read_capture(
                capture,
                &first.surface.state().snapshots,
                &[],
                SearchCancellation::default()
            )
            .await,
        Err(SearchError::InvalidConfiguration)
    ));
    let after: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut connection)?;
    assert_eq!(after.len(), before.len());
    let capture = first
        .service
        .capture_read(proposal.clone(), SearchCancellation::default())
        .await
        .map_err(|e| format!("{e:?}"))?;
    let count = first.calls();
    let saved = first
        .service
        .snapshot_read_capture(
            capture,
            &first.surface.state().snapshots,
            &[],
            SearchCancellation::default(),
        )
        .await
        .map_err(|e| format!("{e:?}"))?;
    let id = saved.snapshot.unwrap();
    assert_eq!(first.calls(), count);
    assert_eq!(saved.response.results.len(), 1);
    let SnapshotLookup::Found(snapshot) = first.surface.state().snapshots.read(&id).await else {
        panic!("saved capture missing")
    };
    assert_eq!(snapshot.results().len(), 3);
    let capture = first
        .service
        .capture_read(proposal, SearchCancellation::default())
        .await
        .map_err(|e| format!("{e:?}"))?;
    let cancel = SearchCancellation::default();
    cancel.cancel();
    let saved = first
        .service
        .snapshot_read_capture(capture, &first.surface.state().snapshots, &[], cancel)
        .await
        .map_err(|e| format!("{e:?}"))?;
    assert!(saved.snapshot.is_err());
    first.service.shutdown().await;
    second.service.shutdown().await;
    Ok(())
}
#[tokio::test]
async fn actual_stdio_rpc_cancellation_drains_prepare_and_preview_before_long_optional_timeout()
-> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    for name in ["slopsearx_search", "slopsearx_explain_search_scope"] {
        let provider = Provider::new().await?;
        let server = private_valkey::Server::start();
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_search-qualification"))
            .args(["stdio", server.url(), &provider.endpoint])
            .env_clear()
            .env("PATH", "")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let mut input = child.stdin.take().unwrap();
        let mut output = tokio::io::BufReader::new(child.stdout.take().unwrap());
        let init = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"cancel","version":"1"}}}"#;
        input.write_all(format!("{init}\n").as_bytes()).await?;
        input.flush().await?;
        let mut response = String::new();
        tokio::time::timeout(Duration::from_secs(5), output.read_line(&mut response)).await??;
        for cycle in 0..32 {
            let request = format!(
                r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"{name}","arguments":{{"query":"public unrelated query {cycle}"}}}}}}"#
            );
            input.write_all(format!("{request}\n").as_bytes()).await?;
            input.flush().await?;
            provider.observed().await?;
            input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/cancelled\",\"params\":{\"requestId\":1}}\n").await?;
            input.flush().await?;
            // rmcp intentionally suppresses cancelled replies. Observe actual owned
            // provider drain and then prove the same external id can be reused.
            let until = tokio::time::Instant::now() + Duration::from_secs(1);
            while provider.closed.load(Ordering::SeqCst) <= cycle {
                assert!(
                    tokio::time::Instant::now() < until,
                    "cancel retained optional request"
                );
                tokio::task::yield_now().await;
            }
            // A completed status call is a same-connection barrier, before EOF.
            let barrier = format!(
                r#"{{"jsonrpc":"2.0","id":{},"method":"tools/call","params":{{"name":"slopsearx_get_service_status","arguments":{{}}}}}}"#,
                100 + cycle
            );
            input.write_all(format!("{barrier}\n").as_bytes()).await?;
            input.flush().await?;
            response.clear();
            tokio::time::timeout(Duration::from_secs(1), output.read_line(&mut response)).await??;
            let value: serde_json::Value = serde_json::from_str(&response)?;
            assert_eq!(value["id"], 100 + cycle);
        }
        assert_eq!(
            redis::cmd("KEYS")
                .arg("t06:qualification:dispatch:*")
                .query::<Vec<String>>(&mut server.connection())?
                .len(),
            0,
            "stdio cancellation dispatched engine"
        );
        let status = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"slopsearx_get_service_status","arguments":{}}}"#;
        input.write_all(format!("{status}\n").as_bytes()).await?;
        input.flush().await?;
        response.clear();
        tokio::time::timeout(Duration::from_secs(1), output.read_line(&mut response)).await??;
        let result: serde_json::Value = serde_json::from_str(&response)?;
        assert_eq!(
            result["result"]["structuredContent"]["engine_health"]["ok"],
            0
        );
        let reuse = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"slopsearx_search_targeted","arguments":{"query":"reuse","engines":["wikipedia"]}}}"#;
        input.write_all(format!("{reuse}\n").as_bytes()).await?;
        input.flush().await?;
        response.clear();
        tokio::time::timeout(Duration::from_secs(1), output.read_line(&mut response)).await??;
        let result: serde_json::Value = serde_json::from_str(&response)?;
        assert_eq!(result["result"]["structuredContent"]["meta"]["total"], 3);
        assert_eq!(
            redis::cmd("KEYS")
                .arg("t06:qualification:dispatch:*")
                .query::<Vec<String>>(&mut server.connection())?
                .len(),
            1,
            "stdio targeted reuse did not execute exactly once"
        );
        input.shutdown().await?;
        drop(input);
        assert!(
            tokio::time::timeout(Duration::from_secs(5), child.wait())
                .await??
                .success()
        );
        provider.close().await?;
    }
    Ok(())
}
#[tokio::test]
async fn pre_cancelled_prepare_and_preview_never_contact_provider_or_dispatch() -> Result<()> {
    let provider = Provider::new().await?;
    let server = private_valkey::Server::start();
    let fixture = fixture::Fixture::with_options(
        server.url(),
        SearchPolicy::default(),
        engines(),
        &[],
        SearchConfig::default(),
        Some(provider.router()),
    )
    .await;
    let cancel = SearchCancellation::default();
    cancel.cancel();
    assert!(matches!(
        fixture
            .service
            .preview_scope_with_cancellation(&preview(), cancel.clone())
            .await,
        Err(SearchError::Cancelled)
    ));
    let mut p = preview();
    p.surface = SearchSurface::Generic;
    assert!(matches!(
        fixture
            .service
            .prepare_scope_with_cancellation(&p, cancel)
            .await,
        Err(SearchError::Cancelled)
    ));
    assert_eq!(provider.started.available_permits(), 0);
    assert_eq!(fixture.calls(), 0);
    fixture.service.shutdown().await;
    provider.close().await?;
    Ok(())
}

async fn event(stream: &mut reqwest::Response, pending: &mut Vec<u8>) -> Result<(String, String)> {
    loop {
        if let Some(i) = pending.windows(2).position(|v| v == b"\n\n") {
            let frame = String::from_utf8(pending.drain(..i + 2).collect())?;
            let kind = frame
                .lines()
                .find_map(|s| s.strip_prefix("event: "))
                .unwrap_or("message")
                .to_owned();
            let data = frame
                .lines()
                .filter_map(|s| s.strip_prefix("data: "))
                .collect::<Vec<_>>()
                .join("\n");
            if !data.is_empty() {
                return Ok((kind, data));
            }
        }
        pending.extend_from_slice(
            &tokio::time::timeout(Duration::from_secs(1), stream.chunk())
                .await??
                .ok_or("legacy stream EOF")?,
        );
        assert!(pending.len() < 4 * 1024 * 1024);
    }
}
#[tokio::test]
async fn actual_legacy_rpc_cancellation_releases_owned_work_and_message_capacity_before_disconnect()
-> Result<()> {
    use ssx_mcp::{
        harness::Host, legacy::Legacy, oauth::NetworkAuth,
        search_surface::SearchSurface as NativeSurface,
    };
    use tokio_util::sync::CancellationToken;
    for name in ["slopsearx_search", "slopsearx_explain_search_scope"] {
        let provider = Provider::new().await?;
        let server = private_valkey::Server::start();
        let fixture = fixture::Fixture::with_options(
            server.url(),
            SearchPolicy::default(),
            engines(),
            &[],
            SearchConfig {
                max_callers: 1,
                optional_port_timeout: Duration::from_secs(10),
                ..Default::default()
            },
            Some(provider.router()),
        )
        .await;
        let surface = fixture.surface.clone();
        let cancel = CancellationToken::new();
        let legacy = Legacy::with_factory(
            Arc::new(move |wire, owner| surface.clone().bind_connection(wire, owner)),
            NativeSurface::catalogs(),
            "/messages/",
            Duration::from_secs(60),
            cancel.clone(),
        )?;
        let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("cancel-token".into())?);
        let host = Host::start(auth.mount(legacy.clone().router("/sse")?), cancel).await?;
        let base = format!("http://{}", host.address);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            "Bearer cancel-token".parse()?,
        );
        let client = reqwest::Client::builder()
            .no_proxy()
            .default_headers(headers)
            .build()?;
        let mut stream = client.get(format!("{base}/sse")).send().await?;
        let mut pending = Vec::new();
        let (_, path) = event(&mut stream, &mut pending).await?;
        let url = format!("{base}{path}");
        let post = |v: ssx_json::Value| {
            client
                .post(&url)
                .header("content-type", "application/json")
                .body(v.to_string())
                .send()
        };
        assert_eq!(post(ssx_json::json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"legacy-cancel","version":"1"}}})).await?.status(),202);
        event(&mut stream, &mut pending).await?;
        for cycle in 0..32 {
            assert_eq!(post(ssx_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":{"query":format!("unrelated public query {cycle}")}}})).await?.status(),202);
            provider.observed().await?;
            assert_eq!(post(ssx_json::json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}})).await?.status(),202);
            let until = tokio::time::Instant::now() + Duration::from_secs(1);
            while provider.closed.load(Ordering::SeqCst) <= cycle
                || legacy.available_message_permits() != 32
            {
                assert!(
                    tokio::time::Instant::now() < until,
                    "legacy cancellation retained request or pending permit"
                );
                tokio::task::yield_now().await;
            }
            assert_eq!(fixture.calls(), 0);
            assert_eq!(post(ssx_json::json!({"jsonrpc":"2.0","id":100+cycle,"method":"tools/call","params":{"name":"slopsearx_get_service_status","arguments":{}}})).await?.status(),202);
            let (_, line) = event(&mut stream, &mut pending).await?;
            let reply: ssx_json::Value = ssx_json::from_str(&line)?;
            assert_eq!(reply["id"].as_u64(), Some((100 + cycle) as u64));
        }
        assert_eq!(post(ssx_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"slopsearx_search_targeted","arguments":{"query":"reuse","engines":["wikipedia"]}}})).await?.status(),202);
        let (_, line) = event(&mut stream, &mut pending).await?;
        let reply: ssx_json::Value = ssx_json::from_str(&line)?;
        assert_eq!(
            reply["result"]["structuredContent"]["meta"]["total"].as_u64(),
            Some(3)
        );
        assert_eq!(fixture.calls(), 1);
        drop(stream);
        host.shutdown().await?;
        assert_eq!(legacy.available_message_permits(), 32);
        assert_eq!(legacy.available_permits(), 128);
        fixture.service.shutdown().await;
        provider.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn actual_stateless_http_disconnect_drains_inflight_optional_request() -> Result<()> {
    use ssx_mcp::{
        harness::{Host, http_router_for},
        search_surface::SearchSurface as NativeSurface,
    };
    use tokio::io::AsyncWriteExt;
    for name in ["slopsearx_search", "slopsearx_explain_search_scope"] {
        let provider = Provider::new().await?;
        let server = private_valkey::Server::start();
        let fixture = fixture::Fixture::with_options(
            server.url(),
            SearchPolicy::default(),
            engines(),
            &[],
            SearchConfig {
                max_callers: 1,
                optional_port_timeout: Duration::from_secs(10),
                ..Default::default()
            },
            Some(provider.router()),
        )
        .await;
        let cancel = tokio_util::sync::CancellationToken::new();
        let host = Host::start(
            http_router_for(
                fixture.surface.clone(),
                NativeSurface::catalogs(),
                cancel.clone(),
            ),
            cancel,
        )
        .await?;
        let body=ssx_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":{"query":"unrelated public query"}}}).to_string();
        let mut socket = tokio::net::TcpStream::connect(host.address).await?;
        let request = format!(
            "POST /mcp HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nAccept: application/json,text/event-stream\r\nMCP-Protocol-Version: 2025-11-25\r\nContent-Length: {}\r\n\r\n{body}",
            host.address,
            body.len()
        );
        socket.write_all(request.as_bytes()).await?;
        socket.flush().await?;
        provider.observed().await?;
        drop(socket);
        let check = tokio::time::timeout(Duration::from_secs(1), async {
            while provider.closed.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await;
        assert_eq!(fixture.calls(), 0);
        fixture.service.shutdown().await;
        host.shutdown().await?;
        provider.close().await?;
        check
            .map_err(|_| format!("stateless HTTP disconnect retained optional request: {name}"))?;
    }
    Ok(())
}

#[tokio::test]
async fn actual_stdio_eof_during_optional_prepare_and_preview_drains_owned_process() -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    for name in ["slopsearx_search", "slopsearx_explain_search_scope"] {
        let provider = Provider::new().await?;
        let server = private_valkey::Server::start();
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_search-qualification"))
            .args(["stdio", server.url(), &provider.endpoint])
            .env_clear()
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let mut input = child.stdin.take().unwrap();
        let mut output = tokio::io::BufReader::new(child.stdout.take().unwrap());
        let init = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"owned-eof","version":"1"}}}"#;
        input.write_all(format!("{init}\n").as_bytes()).await?;
        input.flush().await?;
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(1), output.read_line(&mut line)).await??;
        let request = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":{"query":"unrelated public query"}}});
        input.write_all(format!("{request}\n").as_bytes()).await?;
        input.flush().await?;
        provider.observed().await?;
        input.shutdown().await?;
        drop(input);
        assert!(
            tokio::time::timeout(Duration::from_secs(5), child.wait())
                .await??
                .success(),
            "EOF drain process failed"
        );
        // Process exit closes descriptors, but the independently scheduled
        // provider task must still observe peer EOF before its exact count is ready.
        tokio::time::timeout(Duration::from_secs(1), async {
            while provider.closed.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .map_err(|_| "EOF retained owned provider connection")?;
        assert_eq!(
            provider.closed.load(Ordering::SeqCst),
            1,
            "EOF retained owned provider connection"
        );
        assert_eq!(
            redis::cmd("KEYS")
                .arg("t06:qualification:dispatch:*")
                .query::<Vec<String>>(&mut server.connection())?
                .len(),
            0,
            "EOF dispatched engine"
        );
        provider.close().await?;
    }
    Ok(())
}

/// Execute the real MCP handler and its production opaque output projection,
/// using an in-memory SDK connection solely to obtain a real server peer.
async fn warning_tool_call(
    surface: &ssx_mcp::search_surface::SearchSurface,
    peer: rmcp::Peer<rmcp::RoleServer>,
    name: &str,
    arguments: serde_json::Value,
) -> Result<ssx_json::Value> {
    use rmcp::ServerHandler;
    let request = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":arguments}});
    let mut overlay = ssx_mcp::opaque::Overlay::default();
    overlay.request(
        &ssx_mcp::search_surface::SearchSurface::catalogs(),
        &request.to_string(),
    )?;
    let response = surface
        .call_tool(
            rmcp::model::CallToolRequestParams::new(name.to_owned())
                .with_arguments(arguments.as_object().unwrap().clone()),
            rmcp::service::RequestContext::new(rmcp::model::RequestId::Number(1), peer),
        )
        .await?;
    let rmcp::model::CallToolResponse::Complete(response) = response else {
        panic!("unexpected incomplete tool response")
    };
    let response = serde_json::json!({"jsonrpc":"2.0","id":1,"result":response});
    let full: ssx_json::Value = ssx_json::from_str(&overlay.response(&response.to_string())?)?;
    let result = &full["result"];
    let text: ssx_json::Value = ssx_json::from_str(result["content"][0]["text"].as_str().unwrap())?;
    assert_eq!(
        text, result["structuredContent"],
        "MCP text and structured envelopes differ"
    );
    Ok(text)
}

#[tokio::test]
async fn runtime_scope_warning_survives_native_mcp_and_snapshot_without_cache_admission()
-> Result<()> {
    use rmcp::ServiceExt;
    use ssx_json::json;
    let server = private_valkey::Server::start();
    let mut fake = engines();
    fake[0].count = 1;
    fake[1].count = 1;
    fake[1].delay = Duration::from_secs(60);
    let fixture = fixture::Fixture::with_engines(server.url(), true, fake).await;
    let (server_io, client_io) = tokio::io::duplex(65536);
    let surface = fixture.surface.clone();
    let serving = tokio::spawn(async move { surface.serve(server_io).await });
    let client = rmcp::model::ClientConfig::default()
        .serve(client_io)
        .await?;
    let service = serving.await??;
    let peer = service.peer().clone();
    let surface = fixture.surface.clone();
    // prefer_fresh returns before cache IO; deadline-truncated writes are skipped.
    // Startup, snapshot persistence/reads and connection drain use running time.
    tokio::time::pause();
    let call =
        tokio::spawn(async move {
            warning_tool_call(&surface, peer, "slopsearx_search", serde_json::json!({
            "query":"runtime warning regression", "engines":["wikipedia","pubmed"],
            "freshness":"prefer_fresh", "interactive_timeout_ms":100, "safesearch":"moderate",
        })).await
        });
    // Auto-advance occurs only when all runnable tasks are idle. The immediate
    // engine and collector therefore finish before this 1ms sleep completes;
    // the second engine remains pending beyond the 100ms interactive deadline.
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(fixture.calls(), 2);
    tokio::time::advance(Duration::from_millis(99)).await;
    tokio::time::resume();
    let response = call.await??;
    assert_eq!(response["meta"]["deadline_exceeded"], true);
    assert_eq!(response["meta"]["partial"], true);
    assert_eq!(response["meta"]["cached"], false);
    assert_eq!(response["results"].as_array().unwrap().len(), 1);
    assert_eq!(response["engine_outcomes"][0]["status"], "ok");
    assert_eq!(response["engine_outcomes"][1]["status"], "unavailable");
    assert_eq!(
        response["engine_outcomes"][1]["message"],
        "interactive deadline reached"
    );
    let warning =
        "Interactive deadline reached; captured results or suggestions may be incomplete.";
    assert_eq!(
        response["warnings"],
        json!([
            "language 'en' is not enforced by selected adapters",
            "moderate safesearch is requested but may not be enforced by every adapter",
            warning
        ])
    );
    let cursor =
        ssx_domain::SnapshotId::new(response["meta"]["cursor"].as_str().unwrap().to_owned())
            .unwrap();
    let SnapshotLookup::Found(snapshot) = fixture.surface.state().snapshots.read(&cursor).await
    else {
        panic!("snapshot missing")
    };
    assert_eq!(snapshot.scope().warnings, [warning]);
    assert_eq!(snapshot.results().len(), 1);
    let page = warning_tool_call(
        &fixture.surface,
        service.peer().clone(),
        "slopsearx_read_results",
        serde_json::json!({"cursor":cursor.as_str()}),
    )
    .await?;
    assert_eq!(page["results"].as_array().unwrap().len(), 1);
    assert!(
        page.get("warnings").is_none(),
        "result-page contract adds no warnings field"
    );
    assert_eq!(fixture.calls(), 2);
    let cached_keys: Vec<String> = redis::cmd("KEYS")
        .arg("search:*")
        .query(&mut server.connection())?;
    assert!(
        cached_keys.is_empty(),
        "deadline-truncated result entered cache"
    );
    // A deadline-truncated response was not admitted. A separate complete
    // query proves the ordinary cache path still returns identical warnings.
    let complete_args = serde_json::json!({"query":"admitted warning cache control", "engines":["wikipedia"], "safesearch":"moderate"});
    let first = warning_tool_call(
        &fixture.surface,
        service.peer().clone(),
        "slopsearx_search_targeted",
        complete_args.clone(),
    )
    .await?;
    let after_first = fixture.calls();
    let cached = warning_tool_call(
        &fixture.surface,
        service.peer().clone(),
        "slopsearx_search_targeted",
        complete_args,
    )
    .await?;
    assert_eq!(first["meta"]["cached"], false);
    assert_eq!(cached["meta"]["cached"], true);
    assert_eq!(cached["warnings"], first["warnings"]);
    assert_eq!(fixture.calls(), after_first);
    client.cancel().await?;
    service.cancel().await?;
    Ok(())
}
