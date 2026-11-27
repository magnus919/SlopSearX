#![cfg(unix)]

use axum::{
    Form, Json, Router,
    body::{Body, to_bytes},
    extract::{Query, State},
    http::{HeaderMap, Method, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env, fs, io,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    process::{Child, Command},
    sync::Barrier,
    time::timeout,
};

const LIMIT: Duration = Duration::from_secs(15);
const SESSION: &str = "oauth-fixture-session";
const INIT: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":42,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"oauth-fixture\",\"version\":\"1\"}}}\n";

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> io::Result<Self> {
        let base = env::var_os("SLOPSEARX_TEST_TEMP_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(env::temp_dir);
        let instant = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = base.join(format!(
            "ssx-gateway-oauth-{}-{instant}",
            std::process::id()
        ));
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
        Ok(Self(directory))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    origin: String,
    stale_calls: Barrier,
    exchanges: AtomicUsize,
    refreshes: AtomicUsize,
    initializations_with_renewed_bearer: AtomicUsize,
    deletes: AtomicUsize,
}

async fn remote(State(fixture): State<Arc<Fixture>>, request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    let bearer = parts
        .headers
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    let challenge = || {
        (StatusCode::UNAUTHORIZED, [("www-authenticate", format!(
            "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\", scope=\"search\"",
            fixture.origin
        ))]).into_response()
    };
    if !matches!(bearer, Some("Bearer initial" | "Bearer renewed")) {
        return challenge();
    }
    if parts.method == Method::DELETE {
        assert_eq!(parts.headers["mcp-session-id"], SESSION);
        assert_eq!(bearer, Some("Bearer renewed"));
        fixture.deletes.fetch_add(1, Ordering::SeqCst);
        return StatusCode::NO_CONTENT.into_response();
    }
    let bytes = to_bytes(body, 16384).await.unwrap();
    let request: Value = serde_json::from_slice(&bytes).unwrap();
    let id = &request["id"];
    let method = request["method"].as_str().unwrap();
    if method == "initialize" {
        if bearer == Some("Bearer renewed") {
            fixture
                .initializations_with_renewed_bearer
                .fetch_add(1, Ordering::SeqCst);
        }
        return (
            [("mcp-session-id", SESSION)],
            Json(json!({"jsonrpc":"2.0","id":id,"result":{
                "protocolVersion":"2025-11-25","capabilities":{"tools":{}},
                "serverInfo":{"name":"oauth-fixture","version":"1"}
            }})),
        )
            .into_response();
    }
    assert_eq!(parts.headers["mcp-session-id"], SESSION);
    let result = match method {
        "notifications/initialized" => return StatusCode::ACCEPTED.into_response(),
        "tools/list" => json!({"tools":[{"name":"probe","inputSchema":{"type":"object"}}]}),
        "resources/list" => json!({"resources":[]}),
        "resources/templates/list" => json!({"resourceTemplates":[]}),
        "prompts/list" => json!({"prompts":[]}),
        "tools/call" if bearer == Some("Bearer initial") => {
            fixture.stale_calls.wait().await;
            return challenge();
        }
        "tools/call" => json!({"content":[{"type":"text","text":"renewed"}]}),
        _ => panic!("unexpected MCP method {method}"),
    };
    Json(json!({"jsonrpc":"2.0","id":id,"result":result})).into_response()
}

fn child(url: &str, file: &Path) -> io::Result<Child> {
    Command::new(env!("CARGO_BIN_EXE_ssx-gateway"))
        .env("MCP_REMOTE_URL", url)
        .env_remove("MCP_REMOTE_TOKEN")
        .env("MCP_REMOTE_OAUTH", " YES ")
        .env("MCP_REMOTE_OAUTH_NO_BROWSER", "1")
        .env("MCP_REMOTE_OAUTH_CALLBACK_PORT", "0")
        .env("MCP_REMOTE_TOKEN_FILE", file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
}

async fn browser(authorization: &str, origin: &str) -> Result<(), Box<dyn std::error::Error>> {
    let uri: Uri = authorization.parse()?;
    let Query(params) = Query::<BTreeMap<String, String>>::try_from_uri(&uri)?;
    assert_eq!(params["code_challenge_method"], "S256");
    let redirect: Uri = params["redirect_uri"].parse()?;
    let host = redirect.authority().unwrap().as_str();
    let mut socket = TcpStream::connect(host).await?;
    let origin = origin.replace(':', "%3A").replace('/', "%2F");
    socket.write_all(format!(
        "GET /callback?state={}&code=fixture-code&iss={origin} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n",
        params["state"]
    ).as_bytes()).await?;
    let mut response = String::new();
    socket.read_to_string(&mut response).await?;
    assert!(response.starts_with("HTTP/1.1 200"));
    Ok(())
}

#[tokio::test]
async fn installed_oauth_gateway_signs_in_coalesces_refresh_and_reuses_saved_credentials()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let file = scratch.0.join("tokens.json");
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let origin = format!("http://{}", listener.local_addr()?);
    let remote_url = format!("{origin}/mcp");
    let fixture = Arc::new(Fixture {
        origin: origin.clone(),
        stale_calls: Barrier::new(2),
        exchanges: AtomicUsize::new(0),
        refreshes: AtomicUsize::new(0),
        initializations_with_renewed_bearer: AtomicUsize::new(0),
        deletes: AtomicUsize::new(0),
    });
    let router = Router::new()
        .route("/mcp", any(remote))
        .route("/.well-known/oauth-protected-resource/mcp", get(|State(state): State<Arc<Fixture>>| async move {
            Json(json!({"resource":format!("{}/mcp",state.origin),"authorization_servers":[state.origin],"scopes_supported":["search"]}))
        }))
        .route("/.well-known/oauth-authorization-server", get(|headers: HeaderMap| async move {
            let origin = format!("http://{}", headers["host"].to_str().unwrap());
            Json(json!({"issuer":origin,"authorization_endpoint":format!("{origin}/authorize"),
                "token_endpoint":format!("{origin}/token"),"registration_endpoint":format!("{origin}/register"),
                "code_challenge_methods_supported":["S256"],"authorization_response_iss_parameter_supported":true}))
        }))
        .route("/register", post(|Json(body): Json<Value>| async move {
            Json(json!({"client_id":"fixture-client","redirect_uris":body["redirect_uris"]}))
        }))
        .route("/token", post(|State(state): State<Arc<Fixture>>, Form(body): Form<BTreeMap<String,String>>| async move {
            assert_eq!(body["client_id"], "fixture-client");
            match body["grant_type"].as_str() {
                "authorization_code" => {
                    assert_eq!(body["code"], "fixture-code");
                    assert!(!body["code_verifier"].is_empty());
                    state.exchanges.fetch_add(1, Ordering::SeqCst);
                    Json(json!({"access_token":"initial","token_type":"Bearer","refresh_token":"saved","scope":"search"}))
                }
                "refresh_token" => {
                    assert_eq!(body["refresh_token"], "saved");
                    state.refreshes.fetch_add(1, Ordering::SeqCst);
                    Json(json!({"access_token":"renewed","token_type":"Bearer","scope":"search"}))
                }
                _ => panic!("unknown grant type"),
            }
        }))
        .with_state(fixture.clone());
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let result = timeout(LIMIT, async {
        let mut gateway = child(&remote_url, &file)?;
        let mut stdin = gateway.stdin.take().unwrap();
        let mut stdout = BufReader::new(gateway.stdout.take().unwrap()).lines();
        let mut stderr = BufReader::new(gateway.stderr.take().unwrap()).lines();
        stdin.write_all(INIT).await?;
        let prompt = stderr.next_line().await?.ok_or("gateway did not print the authorization prompt")?;
        assert!(prompt.contains("Authorize the SlopSearX gateway"));
        let url = stderr.next_line().await?.ok_or("gateway did not print the authorization URL")?;
        browser(url.trim(), &origin).await?;
        let initialized: Value = serde_json::from_str(&stdout.next_line().await?.ok_or("no initialize reply")?)?;
        assert_eq!(initialized["id"], 42);
        assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
        stdin.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n").await?;
        for id in [43, 44] {
            let call = json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"probe","arguments":{}}});
            stdin.write_all(format!("{call}\n").as_bytes()).await?;
        }
        let mut seen = Vec::new();
        for _ in 0..2 {
            let reply: Value = serde_json::from_str(&stdout.next_line().await?.ok_or("no tool reply")?)?;
            assert_eq!(reply["result"]["content"][0]["text"], "renewed");
            seen.push(reply["id"].as_u64().unwrap());
        }
        seen.sort();
        assert_eq!(seen, [43, 44]);
        drop(stdin);
        assert!(gateway.wait().await?.success());

        let stored: Value = serde_json::from_slice(&fs::read(&file)?)?;
        assert_eq!(stored["client_info"]["issuer"], origin);
        assert_eq!(stored["tokens"]["access_token"], "renewed");
        assert_eq!(stored["tokens"]["refresh_token"], "saved");
        assert_eq!(fs::metadata(&file)?.permissions().mode() & 0o777, 0o600);

        let mut restarted = child(&remote_url, &file)?;
        let mut stdin = restarted.stdin.take().unwrap();
        let mut stdout = BufReader::new(restarted.stdout.take().unwrap()).lines();
        stdin.write_all(INIT).await?;
        let initialized: Value = serde_json::from_str(&stdout.next_line().await?.ok_or("no restart reply")?)?;
        assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
        drop(stdin);
        assert!(restarted.wait().await?.success());
        assert_eq!(fixture.exchanges.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.refreshes.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.initializations_with_renewed_bearer.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.deletes.load(Ordering::SeqCst), 2);
        Ok::<_, Box<dyn std::error::Error>>(())
    }).await;
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    result?
}
