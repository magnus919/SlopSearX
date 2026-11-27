use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Method, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
};
use serde_json::{Value, json};
use ssx_mcp::opaque::RawGateway;
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

const TARGET: &str = "/keep/%2e%2E/mcp?";
const LIMIT: Duration = Duration::from_secs(5);
type ObservedRequest = (Method, String, String, String);

struct Remote {
    requests: mpsc::UnboundedSender<ObservedRequest>,
    hold_discovery: bool,
}

async fn remote(State(state): State<Arc<Remote>>, request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    let session = parts
        .headers
        .get("mcp-session-id")
        .map(|header| header.to_str().unwrap())
        .unwrap_or_default()
        .to_owned();
    let target = parts.uri.path_and_query().unwrap().to_string();
    if parts.method == Method::DELETE {
        state
            .requests
            .send((parts.method, target, session, String::new()))
            .unwrap();
        return StatusCode::NO_CONTENT.into_response();
    }
    assert_eq!(parts.method, Method::POST);
    let body = axum::body::to_bytes(body, 16384).await.unwrap();
    let request: Value = serde_json::from_slice(&body).unwrap();
    let method = request["method"].as_str().unwrap();
    state
        .requests
        .send((parts.method, target, session, method.into()))
        .unwrap();
    match method {
        "initialize" => (
            [("mcp-session-id", "owned-session")],
            axum::Json(json!({"jsonrpc":"2.0","id":request["id"],"result":{
                "protocolVersion":"2025-11-25","capabilities":{"tools":{}},
                "serverInfo":{"name":"startup-fixture","version":"1"}
            }})),
        )
            .into_response(),
        "notifications/initialized" => StatusCode::ACCEPTED.into_response(),
        "tools/list" if state.hold_discovery => std::future::pending().await,
        "tools/list" => axum::Json(json!({
            "jsonrpc": "2.0",
            "id": request["id"],
            "result": {"tools": [{"name": "held", "inputSchema": {"type": "object"}}]}
        }))
        .into_response(),
        "tools/call" | "notifications/cancelled" => std::future::pending().await,
        _ => {
            let field = match method {
                "resources/list" => "resources",
                "prompts/list" => "prompts",
                "resources/templates/list" => "resourceTemplates",
                _ => panic!("unexpected remote operation {method}"),
            };
            axum::Json(json!({"jsonrpc":"2.0","id":request["id"],"result":{(field):[]}}))
                .into_response()
        }
    }
}

async fn start_remote(
    hold_discovery: bool,
) -> std::io::Result<(
    String,
    mpsc::UnboundedReceiver<ObservedRequest>,
    JoinHandle<std::io::Result<()>>,
)> {
    let (send, received) = mpsc::unbounded_channel();
    let app = Router::new()
        .fallback(any(remote))
        .with_state(Arc::new(Remote {
            requests: send,
            hold_discovery,
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}{TARGET}", listener.local_addr()?);
    let server = tokio::spawn(async move { axum::serve(listener, app).await });
    Ok((url, received, server))
}

#[tokio::test]
async fn cancelled_gateway_startup_retires_the_granted_session() -> ssx_mcp::Result<()> {
    let (url, mut received, server) = start_remote(true).await?;
    let token = CancellationToken::new();
    let startup = {
        let token = token.clone();
        tokio::spawn(async move {
            RawGateway::connect_cancellable(url, None, Duration::from_secs(15), &token).await
        })
    };
    for (method, session) in [
        ("initialize", ""),
        ("notifications/initialized", "owned-session"),
        ("tools/list", "owned-session"),
    ] {
        let observed = tokio::time::timeout(LIMIT, received.recv()).await?.unwrap();
        assert_eq!(
            observed,
            (Method::POST, TARGET.into(), session.into(), method.into())
        );
    }

    token.cancel();
    let outcome = tokio::time::timeout(LIMIT, startup).await??;
    assert_eq!(
        outcome.err().unwrap().to_string(),
        "remote MCP startup cancelled"
    );
    assert_retired(&mut received)?;
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    Ok(())
}

fn assert_retired(received: &mut mpsc::UnboundedReceiver<ObservedRequest>) -> ssx_mcp::Result<()> {
    assert_eq!(
        received.try_recv()?,
        (
            Method::DELETE,
            TARGET.into(),
            "owned-session".into(),
            String::new()
        )
    );
    assert!(matches!(
        received.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
    Ok(())
}

#[tokio::test]
async fn cancelled_gateway_stdio_retires_session_with_pending_rpc_and_open_input()
-> ssx_mcp::Result<()> {
    let (url, mut received, server) = start_remote(false).await?;
    let gateway = RawGateway::connect(url, None, Duration::from_secs(15)).await?;
    let discovery = std::iter::from_fn(|| received.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(discovery.len(), 6);

    let (input, mut caller) = tokio::io::duplex(4096);
    let (output, replies) = tokio::io::duplex(4096);
    let cancel = CancellationToken::new();
    let task = tokio::spawn(gateway.stdio_with_io(input, output, cancel.clone()));
    caller.write_all(concat!(
        r#"{"jsonrpc":"2.0","id":42,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"stdio-fixture","version":"1"}}}"#,
        "\n"
    ).as_bytes()).await?;
    let mut replies = BufReader::new(replies).lines();
    let reply = tokio::time::timeout(LIMIT, replies.next_line())
        .await??
        .unwrap();
    let reply: Value = serde_json::from_str(&reply)?;
    assert_eq!(reply["id"], 42);
    assert_eq!(reply["result"]["protocolVersion"], "2025-11-25");

    caller
        .write_all(
            concat!(
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                "\n",
                r#"{"jsonrpc":"2.0","id":43,"method":"tools/call","params":{"name":"held","arguments":{}}}"#,
                "\n"
            )
            .as_bytes(),
        )
        .await?;
    let call = tokio::time::timeout(LIMIT, received.recv()).await?.unwrap();
    assert_eq!(
        call,
        (
            Method::POST,
            TARGET.into(),
            "owned-session".into(),
            "tools/call".into()
        )
    );
    caller
        .write_all(
            concat!(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":43}}"#,
                "\n"
            )
            .as_bytes(),
        )
        .await?;
    let notification = tokio::time::timeout(LIMIT, received.recv()).await?.unwrap();
    assert_eq!(
        notification,
        (
            Method::POST,
            TARGET.into(),
            "owned-session".into(),
            "notifications/cancelled".into()
        )
    );

    cancel.cancel();
    // The remote holds both HTTP exchanges. The stdio owner must cancel them
    // before waiting on the SDK's two-second response drain or the HTTP deadline.
    tokio::time::timeout(Duration::from_millis(1500), task).await???;
    assert_retired(&mut received)?;
    drop(caller);
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    Ok(())
}
