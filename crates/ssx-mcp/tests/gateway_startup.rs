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
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct Remote {
    requests: mpsc::UnboundedSender<(Method, String, String, String)>,
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
                "protocolVersion":"2025-11-25","capabilities":{},
                "serverInfo":{"name":"startup-fixture","version":"1"}
            }})),
        )
            .into_response(),
        "notifications/initialized" => StatusCode::ACCEPTED.into_response(),
        "tools/list" => std::future::pending().await,
        _ => panic!("unexpected remote operation {method}"),
    }
}

#[tokio::test]
async fn cancelled_gateway_startup_retires_the_granted_session() -> ssx_mcp::Result<()> {
    const TARGET: &str = "/keep/%2e%2E/mcp?";
    let (send, mut received) = mpsc::unbounded_channel();
    let app = Router::new()
        .fallback(any(remote))
        .with_state(Arc::new(Remote { requests: send }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}{TARGET}", listener.local_addr()?);
    let server = tokio::spawn(async move { axum::serve(listener, app).await });
    let token = CancellationToken::new();
    let startup = {
        let token = token.clone();
        tokio::spawn(async move {
            RawGateway::connect_cancellable(url, None, Duration::from_secs(15), &token).await
        })
    };
    let limit = Duration::from_secs(5);
    for (method, session) in [
        ("initialize", ""),
        ("notifications/initialized", "owned-session"),
        ("tools/list", "owned-session"),
    ] {
        let observed = tokio::time::timeout(limit, received.recv()).await?.unwrap();
        assert_eq!(
            observed,
            (Method::POST, TARGET.into(), session.into(), method.into())
        );
    }

    token.cancel();
    let outcome = tokio::time::timeout(limit, startup).await??;
    assert_eq!(
        outcome.err().unwrap().to_string(),
        "remote MCP startup cancelled"
    );
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
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    Ok(())
}
