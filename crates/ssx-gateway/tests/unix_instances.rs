#![cfg(unix)]

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
};
use serde_json::{Value, json};
use std::{io, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    net::TcpListener,
    process::{Child, ChildStdin, Command},
    task::JoinHandle,
    time::timeout,
};

const LIMIT: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct Fixture {
    identity: &'static str,
    token: &'static str,
    session: &'static str,
    tool: &'static str,
    answer: &'static str,
}

async fn remote(State(fixture): State<Fixture>, request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    if parts.method == Method::DELETE {
        assert_eq!(
            parts.headers["authorization"],
            format!("Bearer {}", fixture.token)
        );
        assert_eq!(parts.headers["mcp-session-id"], fixture.session);
        return StatusCode::NO_CONTENT.into_response();
    }
    assert_eq!(parts.method, Method::POST);
    assert_eq!(
        parts.headers["authorization"],
        format!("Bearer {}", fixture.token)
    );

    let body = to_bytes(body, 16_384).await.unwrap();
    let request: Value = serde_json::from_slice(&body).unwrap();
    let id = &request["id"];
    let method = request["method"].as_str().unwrap();
    if method == "initialize" {
        assert!(parts.headers.get("mcp-session-id").is_none());
        return (
            [("mcp-session-id", fixture.session)],
            Json(json!({
                "jsonrpc":"2.0", "id":id, "result":{
                    "protocolVersion":"2025-11-25",
                    "capabilities":{"tools":{}},
                    "serverInfo":{"name":fixture.identity,"version":"1"}
                }
            })),
        )
            .into_response();
    }

    assert_eq!(parts.headers["mcp-session-id"], fixture.session);
    match method {
        "notifications/initialized" => StatusCode::ACCEPTED.into_response(),
        "tools/list" => Json(json!({
            "jsonrpc":"2.0", "id":id, "result":{
                "tools":[{"name":fixture.tool,"inputSchema":{"type":"object"}}]
            }
        }))
        .into_response(),
        "tools/call" => {
            assert_eq!(request["params"]["name"], fixture.tool);
            Json(json!({
                "jsonrpc":"2.0", "id":id, "result":{
                    "content":[{"type":"text","text":fixture.answer}],
                    "isError":false
                }
            }))
            .into_response()
        }
        _ => panic!("unexpected MCP method {method}"),
    }
}

async fn serve(fixture: Fixture) -> io::Result<(String, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().fallback(any(remote)).with_state(fixture),
        )
        .await
        .unwrap();
    });
    Ok((format!("http://{address}/mcp"), task))
}

struct Gateway {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Lines<BufReader<tokio::process::ChildStdout>>,
}

fn spawn(url: &str, token: &str) -> io::Result<Gateway> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ssx-gateway"))
        .env("MCP_REMOTE_URL", url)
        .env("MCP_REMOTE_TOKEN", token)
        .env_remove("MCP_REMOTE_OAUTH")
        .env_remove("MCP_REMOTE_CA_BUNDLE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    Ok(Gateway {
        child,
        stdin: Some(stdin),
        stdout,
    })
}

async fn exchange(gateway: &mut Gateway, request: Value) -> io::Result<Value> {
    let input = gateway.stdin.as_mut().unwrap();
    input.write_all(request.to_string().as_bytes()).await?;
    input.write_all(b"\n").await?;
    let line = timeout(LIMIT, gateway.stdout.next_line())
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "gateway response timed out"))??
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "gateway exited"))?;
    serde_json::from_str(&line).map_err(io::Error::other)
}

async fn notify(gateway: &mut Gateway, method: &str) -> io::Result<()> {
    gateway
        .stdin
        .as_mut()
        .unwrap()
        .write_all(
            json!({"jsonrpc":"2.0","method":method})
                .to_string()
                .as_bytes(),
        )
        .await?;
    gateway.stdin.as_mut().unwrap().write_all(b"\n").await
}

async fn finish(mut gateway: Gateway) -> io::Result<()> {
    drop(gateway.stdin.take());
    let status = timeout(LIMIT, gateway.child.wait())
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "gateway did not stop"))??;
    assert!(status.success(), "gateway exited with {status}");
    Ok(())
}

#[tokio::test]
async fn installed_gateway_instances_keep_sessions_tools_and_credentials_isolated()
-> Result<(), Box<dyn std::error::Error>> {
    let left_fixture = Fixture {
        identity: "left-instance",
        token: "left-secret",
        session: "left-session",
        tool: "left-only",
        answer: "left-response",
    };
    let right_fixture = Fixture {
        identity: "right-instance",
        token: "right-secret",
        session: "right-session",
        tool: "right-only",
        answer: "right-response",
    };
    let (left_url, left_server) = serve(left_fixture.clone()).await?;
    let (right_url, right_server) = serve(right_fixture.clone()).await?;
    let mut left = spawn(&left_url, left_fixture.token)?;
    let mut right = spawn(&right_url, right_fixture.token)?;

    let initialize = |name| {
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-11-25","capabilities":{},
            "clientInfo":{"name":name,"version":"1"}
        }})
    };
    let (left_init, right_init) = tokio::join!(
        exchange(&mut left, initialize("left-client")),
        exchange(&mut right, initialize("right-client"))
    );
    let left_init = left_init?;
    let right_init = right_init?;
    assert_eq!(
        left_init["result"]["serverInfo"]["name"],
        left_fixture.identity
    );
    assert_eq!(
        right_init["result"]["serverInfo"]["name"],
        right_fixture.identity
    );

    let (left_notify, right_notify) = tokio::join!(
        notify(&mut left, "notifications/initialized"),
        notify(&mut right, "notifications/initialized")
    );
    left_notify?;
    right_notify?;

    let (left_tools, right_tools) = tokio::join!(
        exchange(
            &mut left,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})
        ),
        exchange(
            &mut right,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})
        )
    );
    let left_tools = left_tools?;
    let right_tools = right_tools?;
    assert_eq!(left_tools["result"]["tools"][0]["name"], left_fixture.tool);
    assert_eq!(
        right_tools["result"]["tools"][0]["name"],
        right_fixture.tool
    );

    let call = |tool| {
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
            "name":tool,"arguments":{}
        }})
    };
    let (left_call, right_call) = tokio::join!(
        exchange(&mut left, call(left_fixture.tool)),
        exchange(&mut right, call(right_fixture.tool))
    );
    let left_call = left_call?;
    let right_call = right_call?;
    assert_eq!(
        left_call["result"]["content"][0]["text"],
        left_fixture.answer
    );
    assert_eq!(
        right_call["result"]["content"][0]["text"],
        right_fixture.answer
    );

    let (left_exit, right_exit) = tokio::join!(finish(left), finish(right));
    left_exit?;
    right_exit?;
    left_server.abort();
    right_server.abort();
    Ok(())
}
