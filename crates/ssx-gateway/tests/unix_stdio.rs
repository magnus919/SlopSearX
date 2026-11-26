#![cfg(unix)]

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
};
use nix::{
    errno::Errno,
    fcntl::{FcntlArg, OFlag, fcntl},
    sys::signal::{Signal, kill},
    unistd::{Pid, pipe, write},
};
use serde_json::{Value, json};
use std::{fs::File, io, os::fd::OwnedFd, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::mpsc,
    time::timeout,
};

const LIMIT: Duration = Duration::from_secs(8);
const TARGET: &str = "/keep/%2e%2E/mcp?";
const SESSION: &str = "gateway-fixture-session";
const TOKEN: &str = "gateway-fixture-token";
const INIT: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":42,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"stdio-fixture\",\"version\":\"1\"}}}\n";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExitMode {
    Signal,
    SignalWithFullOutput,
    StartupEof,
}

#[derive(Clone)]
struct Fixture {
    observed: mpsc::UnboundedSender<String>,
    mode: ExitMode,
}

async fn remote(
    State(Fixture { observed, mode }): State<Fixture>,
    request: Request<Body>,
) -> Response {
    let (parts, body) = request.into_parts();
    assert_eq!(parts.uri.path_and_query().unwrap().as_str(), TARGET);
    assert_eq!(parts.headers["authorization"], format!("Bearer {TOKEN}"));
    let session = parts.headers.get("mcp-session-id");
    if parts.method == Method::DELETE {
        assert_eq!(session.unwrap(), SESSION);
        observed.send("DELETE".into()).unwrap();
        return StatusCode::NO_CONTENT.into_response();
    }
    assert_eq!(parts.method, Method::POST);
    let body = to_bytes(body, 16384).await.unwrap();
    let message: Value = serde_json::from_slice(&body).unwrap();
    let id = &message["id"];
    let method = message["method"].as_str().unwrap();
    if method == "initialize" {
        assert!(session.is_none());
        return (
            [("mcp-session-id", SESSION)],
            Json(json!({"jsonrpc":"2.0","id":id,"result":{
                "protocolVersion":"2025-11-25","capabilities":{"tools":{}},
                "serverInfo":{"name":"pipe-fixture","version":"1"}
            }})),
        )
            .into_response();
    }
    assert_eq!(session.unwrap(), SESSION);
    let result = match method {
        "notifications/initialized" => return StatusCode::ACCEPTED.into_response(),
        "tools/list" if mode == ExitMode::StartupEof => {
            observed.send("tools/list".into()).unwrap();
            return std::future::pending().await;
        }
        "tools/list" => json!({"tools":[{"name":"held","inputSchema":{"type":"object"}}]}),
        "resources/list" => json!({"resources":[]}),
        "resources/templates/list" => json!({"resourceTemplates":[]}),
        "prompts/list" => json!({"prompts":[]}),
        "tools/call" => {
            observed.send("tools/call".into()).unwrap();
            return std::future::pending().await;
        }
        _ => panic!("unexpected remote operation {method}"),
    };
    Json(json!({"jsonrpc":"2.0","id":id,"result":result})).into_response()
}

fn fill_output(pipe: &OwnedFd) -> io::Result<()> {
    // Fill every available byte, including the last partial PIPE_BUF.
    let mut buffer = &[42; 4096][..];
    loop {
        match write(pipe, buffer) {
            Ok(count) if count > 0 => {}
            Err(Errno::EAGAIN) if buffer.len() > 1 => buffer = &buffer[..1],
            Err(Errno::EAGAIN) => return Ok(()),
            Err(err) => return Err(err.into()),
            _ => return Err(io::ErrorKind::WriteZero.into()),
        }
    }
}

async fn exercise(mode: ExitMode) -> Result<(), Box<dyn std::error::Error>> {
    let (observed, mut requests) = mpsc::unbounded_channel();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}{TARGET}", listener.local_addr()?);
    let app = Router::new()
        .fallback(any(remote))
        .with_state(Fixture { observed, mode });
    let server = tokio::spawn(async move { axum::serve(listener, app).await });

    let mut output_pipe = None;
    let stdout = if mode == ExitMode::SignalWithFullOutput {
        let (read_end, write_end) = pipe()?;
        let original = OFlag::from_bits_retain(fcntl(&write_end, FcntlArg::F_GETFL)?);
        fcntl(&write_end, FcntlArg::F_SETFL(original | OFlag::O_NONBLOCK))?;
        let inherited = Stdio::from(write_end.try_clone()?);
        output_pipe = Some((File::from(read_end), write_end));
        inherited
    } else {
        Stdio::piped()
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_ssx-gateway"))
        .env("MCP_REMOTE_URL", url)
        .env("MCP_REMOTE_TOKEN", TOKEN)
        .stdin(Stdio::piped())
        .stdout(stdout)
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = child.stdin.take();
    let mut replies = child
        .stdout
        .take()
        .map(|stdout| BufReader::new(stdout).lines());
    if mode == ExitMode::StartupEof {
        assert_eq!(
            timeout(LIMIT, requests.recv()).await?.as_deref(),
            Some("tools/list")
        );
        drop(stdin.take());
    } else {
        let input = stdin.as_mut().unwrap();
        input.write_all(INIT).await?;
        let reply = if let Some((read_end, write_end)) = output_pipe.take() {
            let reader = tokio::task::spawn_blocking(move || {
                use std::io::BufRead;
                let mut reader = io::BufReader::new(read_end);
                let mut reply = String::new();
                reader.read_line(&mut reply)?;
                Ok::<_, io::Error>((reader.into_inner(), reply))
            });
            let (read_end, reply) = timeout(LIMIT, reader).await???;
            // Wait for initialization before applying backpressure so the
            // tool call always reaches an initialized local MCP server.
            fill_output(&write_end)?;
            output_pipe = Some((read_end, write_end));
            input
                .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":44,\"method\":\"ping\"}\n")
                .await?;
            reply
        } else {
            timeout(LIMIT, replies.as_mut().unwrap().next_line())
                .await??
                .unwrap()
        };
        let reply: Value = serde_json::from_str(&reply)?;
        assert_eq!(reply["id"], 42);
        assert_eq!(reply["result"]["protocolVersion"], "2025-11-25");
        input
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":43,\"method\":\"tools/call\",\"params\":{\"name\":\"held\",\"arguments\":{}}}\n")
            .await?;
        assert_eq!(
            timeout(LIMIT, requests.recv()).await?.as_deref(),
            Some("tools/call")
        );

        let pid = child
            .id()
            .ok_or_else(|| io::Error::other("gateway exited prematurely"))?;
        kill(Pid::from_raw(i32::try_from(pid)?), Signal::SIGTERM)?;
    }
    assert_eq!(
        timeout(LIMIT, requests.recv()).await?.as_deref(),
        Some("DELETE")
    );
    assert!(timeout(LIMIT, child.wait()).await??.success());
    assert!(matches!(
        requests.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
    drop(stdin);
    drop(replies);
    drop(output_pipe);
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    Ok(())
}

#[tokio::test]
async fn installed_gateway_joins_with_stdin_and_remote_call_open()
-> Result<(), Box<dyn std::error::Error>> {
    exercise(ExitMode::Signal).await
}

#[tokio::test]
async fn installed_gateway_joins_with_stdout_pipe_full() -> Result<(), Box<dyn std::error::Error>> {
    exercise(ExitMode::SignalWithFullOutput).await
}

#[tokio::test]
async fn installed_gateway_retires_session_when_stdin_closes_during_discovery()
-> Result<(), Box<dyn std::error::Error>> {
    exercise(ExitMode::StartupEof).await
}
