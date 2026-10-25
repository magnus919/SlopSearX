//! Hermetic actual native executable fixtures, not a simulated forwarding seam.
use axum::{
    Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout},
    sync::Mutex,
};
use tokio_util::sync::CancellationToken;
#[derive(Default)]
struct Observed {
    calls: Vec<Value>,
    registration: Option<Value>,
    registrations: usize,
    challenge: Option<String>,
    token_forms: Vec<HashMap<String, String>>,
    deletes: usize,
    authorization_visits: usize,
    delete_authorizations: Vec<Option<String>>,
    cancel_authorizations: Vec<Option<String>>,
}
#[derive(Clone)]
struct App {
    base: String,
    state: Arc<Mutex<Observed>>,
    init_entered: Arc<tokio::sync::Notify>,
    init_release: Arc<tokio::sync::Notify>,
    rpc_entered: Arc<tokio::sync::Notify>,
    rpc_release: Arc<tokio::sync::Notify>,
    hold_initialize: bool,
    reject_refresh: bool,
    refresh_reject_status: StatusCode,
    registration_status: StatusCode,
    token_status: StatusCode,
    authorization: Arc<Mutex<String>>,
    delete_status: Arc<Mutex<StatusCode>>,
    token_expiry: Arc<Mutex<i64>>,
    challenge_rpc: Arc<Mutex<bool>>,
    cancel_status: Arc<Mutex<StatusCode>>,
}
async fn mcp(State(app): State<App>, headers: HeaderMap, body: axum::body::Bytes) -> Response {
    let required = app.authorization.lock().await.clone();
    if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some(required.as_str()) {
        return(StatusCode::UNAUTHORIZED,[("www-authenticate",format!("Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\", scope=\"read\"",app.base))]).into_response();
    }
    let value: Value = serde_json::from_slice(&body).unwrap();
    app.state.lock().await.calls.push(value.clone());
    if value["method"] == "notifications/cancelled" {
        app.state.lock().await.cancel_authorizations.push(
            headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned),
        );
        app.rpc_release.notify_one();
        let status = *app.cancel_status.lock().await;
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            let error = if status == StatusCode::FORBIDDEN {
                "insufficient_scope"
            } else {
                "invalid_token"
            };
            return (status, [("www-authenticate", format!("Bearer error=\"{error}\", scope=\"admin\", resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\"", app.base))]).into_response();
        }
        return status.into_response();
    }
    if value["method"] == "tools/call"
        && value["params"]["arguments"]["query"] == 99
        && *app.challenge_rpc.lock().await
    {
        app.rpc_entered.notify_one();
        return (StatusCode::UNAUTHORIZED, [("www-authenticate", format!("Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\", scope=\"read\"", app.base))]).into_response();
    }
    if value["method"] == "initialize" && app.hold_initialize {
        app.init_entered.notify_one();
        app.init_release.notified().await;
    }
    if value["method"] == "tools/call" && value["params"]["arguments"]["query"] == 99 {
        app.rpc_entered.notify_one();
        app.rpc_release.notified().await;
    }
    if value["method"] == "notifications/cancelled" {
        app.rpc_release.notify_one();
    }
    let result = match value["method"].as_str().unwrap() {
        "initialize" => {
            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"private-upstream-name","version":"9"}})
        }
        "notifications/initialized" | "notifications/cancelled" => {
            return StatusCode::ACCEPTED.into_response();
        }
        "tools/list" => {
            json!({"tools":[{"name":"echo","description":"Remote echo","inputSchema":{"type":"object","properties":{"query":{"type":"integer"},"optional":{"type":"string"},"items":{"type":"array","items":{"type":"string"}}},"required":["query"],"x-remote-future":{"opaque":true}},"_meta":{"private":"not-reexposed"}}]})
        }
        "tools/call" => {
            json!({"content":[{"type":"text","text":"display loses to structured"}],"structuredContent":value["params"]["arguments"]})
        }
        "resources/read" => {
            json!({"contents":[{"uri":"slopsearx://capabilities","text":"first","mimeType":"text/plain"},{"uri":"slopsearx://capabilities","blob":"AA=="},{"uri":"slopsearx://capabilities","text":"second"}]})
        }
        "prompts/get" => {
            json!({"messages":[{"role":"assistant","content":{"type":"text","text":"first"}},{"role":"user","content":{"type":"text","text":"second"}}]})
        }
        _ => panic!("unexpected upstream request {value}"),
    };
    (
        [("mcp-session-id", "private-loopback-session")],
        axum::Json(json!({"jsonrpc":"2.0","id":value["id"],"result":result})),
    )
        .into_response()
}
async fn delete(State(app): State<App>, headers: HeaderMap) -> Response {
    let mut observed = app.state.lock().await;
    observed.deletes += 1;
    observed.delete_authorizations.push(
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
    );
    drop(observed);
    let status = *app.delete_status.lock().await;
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        let error = if status == StatusCode::FORBIDDEN {
            "insufficient_scope"
        } else {
            "invalid_token"
        };
        (status, [("www-authenticate", format!("Bearer error=\"{error}\", scope=\"admin\", resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\"", app.base))]).into_response()
    } else {
        status.into_response()
    }
}
async fn redirect_rpc() -> Response {
    (StatusCode::TEMPORARY_REDIRECT, [("location", "/mcp")]).into_response()
}
async fn redirect_register() -> Response {
    (
        StatusCode::TEMPORARY_REDIRECT,
        [("location", "/register-real")],
    )
        .into_response()
}
async fn redirect_token() -> Response {
    (
        StatusCode::TEMPORARY_REDIRECT,
        [("location", "/token-real")],
    )
        .into_response()
}
async fn redirect_metadata() -> Response {
    (
        StatusCode::TEMPORARY_REDIRECT,
        [("location", "/metadata-real")],
    )
        .into_response()
}
async fn metadata(State(app): State<App>) -> axum::Json<Value> {
    axum::Json(
        json!({"resource":format!("{}/mcp",app.base),"authorization_servers":[format!("{}/oauth",app.base)],"scopes_supported":["read"]}),
    )
}
async fn oauth_metadata(State(app): State<App>) -> axum::Json<Value> {
    axum::Json(
        json!({"issuer":format!("{}/oauth",app.base),"authorization_endpoint":format!("{}/authorize",app.base),"token_endpoint":format!("{}/token",app.base),"response_types_supported":["code"],"grant_types_supported":["authorization_code","refresh_token"],"code_challenge_methods_supported":["S256"],"token_endpoint_auth_methods_supported":["none"],"scopes_supported":["read","offline_access"]}),
    )
}
async fn register(State(app): State<App>, axum::Json(value): axum::Json<Value>) -> Response {
    assert!(value.get("application_type").is_none());
    assert_eq!(value["token_endpoint_auth_method"], "none");
    let mut observed = app.state.lock().await;
    observed.registration = Some(value.clone());
    observed.registrations += 1;
    drop(observed);
    (app.registration_status,axum::Json(
        json!({"client_id":"loopback-client","token_endpoint_auth_method":"none","redirect_uris":value["redirect_uris"],"issuer":17}),
    )).into_response()
}
async fn authorize(
    State(app): State<App>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(query["prompt"], "consent");
    {
        let mut observed = app.state.lock().await;
        observed.challenge = Some(query["code_challenge"].clone());
        observed.authorization_visits += 1;
    }
    let mut url = reqwest::Url::parse(&query["redirect_uri"]).unwrap();
    url.query_pairs_mut()
        .append_pair("code", "private-code")
        .append_pair("state", &query["state"]);
    (StatusCode::FOUND, [("location", url.to_string())]).into_response()
}
async fn token(
    State(app): State<App>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> Response {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    assert!(!headers.contains_key("authorization"));
    assert!(!form.contains_key("client_secret"));
    assert_eq!(form["client_id"], "loopback-client");
    assert_eq!(form["resource"], format!("{}/mcp", app.base));
    if form["grant_type"] == "authorization_code" {
        let expected = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(Sha256::digest(form["code_verifier"].as_bytes()));
        assert_eq!(Some(expected), app.state.lock().await.challenge.clone());
    }
    let mut observed = app.state.lock().await;
    let first = observed.token_forms.is_empty();
    let refresh = form["grant_type"] == "refresh_token";
    if refresh {
        assert!(!form.contains_key("scope"));
    }
    observed.token_forms.push(form);
    drop(observed);
    if app.reject_refresh && refresh {
        return (
            app.refresh_reject_status,
            axum::Json(if app.refresh_reject_status==StatusCode::CREATED {json!({"access_token":"refresh201-rejected","token_type":"Bearer","expires_in":3600})}else{json!({"error":"invalid_grant"})}),
        )
            .into_response();
    }
    let expiry = if app.reject_refresh && first {
        -1
    } else {
        *app.token_expiry.lock().await
    };
    (app.token_status,axum::Json(
        json!({"access_token":"loopback-access","token_type":"Bearer","refresh_token":"loopback-refresh","expires_in":expiry,"scope":"read offline_access"}),
    )).into_response()
}
struct Fixture {
    app: App,
    stop: CancellationToken,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}
impl Fixture {
    async fn start(oauth: bool) -> Self {
        Self::start_held(oauth, false).await
    }
    async fn start_held(oauth: bool, hold_initialize: bool) -> Self {
        Self::start_mode(oauth, hold_initialize, false).await
    }
    async fn start_mode(oauth: bool, hold_initialize: bool, reject_refresh: bool) -> Self {
        Self::start_statuses(
            oauth,
            hold_initialize,
            reject_refresh,
            StatusCode::OK,
            StatusCode::OK,
        )
        .await
    }
    async fn start_statuses(
        oauth: bool,
        hold_initialize: bool,
        reject_refresh: bool,
        registration_status: StatusCode,
        token_status: StatusCode,
    ) -> Self {
        Self::start_status_profile(
            oauth,
            hold_initialize,
            reject_refresh,
            registration_status,
            token_status,
            StatusCode::BAD_REQUEST,
        )
        .await
    }
    async fn start_status_profile(
        oauth: bool,
        hold_initialize: bool,
        reject_refresh: bool,
        registration_status: StatusCode,
        token_status: StatusCode,
        refresh_reject_status: StatusCode,
    ) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let app = App {
            base: format!("http://{}", listener.local_addr().unwrap()),
            state: Default::default(),
            init_entered: Default::default(),
            init_release: Default::default(),
            rpc_entered: Default::default(),
            rpc_release: Default::default(),
            hold_initialize,
            reject_refresh,
            refresh_reject_status,
            registration_status,
            token_status,
            delete_status: Arc::new(Mutex::new(StatusCode::NO_CONTENT)),
            token_expiry: Arc::new(Mutex::new(3600)),
            challenge_rpc: Arc::new(Mutex::new(false)),
            cancel_status: Arc::new(Mutex::new(StatusCode::ACCEPTED)),
            authorization: Arc::new(Mutex::new(
                if oauth {
                    "Bearer loopback-access"
                } else {
                    "Bearer static-sentinel"
                }
                .into(),
            )),
        };
        let router = Router::new()
            .route("/mcp", post(mcp).delete(delete))
            .route("/mcp-redirect", axum::routing::any(redirect_rpc))
            .route("/.well-known/oauth-protected-resource/mcp", get(metadata))
            .route(
                "/.well-known/oauth-authorization-server/oauth",
                get(redirect_metadata),
            )
            .route("/metadata-real", get(oauth_metadata))
            .route("/register", post(redirect_register))
            .route("/register-real", post(register))
            .route("/authorize", get(authorize))
            .route("/token", post(redirect_token))
            .route("/token-real", post(token))
            .with_state(app.clone());
        let stop = CancellationToken::new();
        let stopped = stop.clone();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(stopped.cancelled_owned())
                .await
        });
        Self { app, stop, task }
    }
    async fn close(self) {
        self.stop.cancel();
        self.task.await.unwrap().unwrap();
    }
}
// No spawned/detached assertion task and no new dependency. A panic from any
// polled await/assert/unwrap is retained until the caller retires its real child,
// stderr reader, loopback fixture and owned temporary directory.
async fn catch_process_case<F: std::future::Future>(future: F) -> std::thread::Result<F::Output> {
    let mut future = Box::pin(future);
    std::future::poll_fn(move |context| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            std::future::Future::poll(future.as_mut(), context)
        })) {
            Ok(std::task::Poll::Pending) => std::task::Poll::Pending,
            Ok(std::task::Poll::Ready(value)) => std::task::Poll::Ready(Ok(value)),
            Err(panic) => std::task::Poll::Ready(Err(panic)),
        }
    })
    .await
}
fn finish_process_case(outcome: std::thread::Result<ssx_mcp::Result<()>>) -> ssx_mcp::Result<()> {
    match outcome {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
struct Process {
    child: Child,
    input: Option<ChildStdin>,
    output: Lines<BufReader<ChildStdout>>,
}
impl Process {
    fn spawn(fixture: &Fixture, extra: &[String]) -> Self {
        Self::spawn_url(&format!("{}/mcp", fixture.app.base), extra)
    }
    fn spawn_url(remote: &str, extra: &[String]) -> Self {
        Self::spawn_url_with_browser(remote, extra, None)
    }
    fn spawn_url_with_browser(
        remote: &str,
        extra: &[String],
        browser: Option<(&std::path::Path, &std::path::Path)>,
    ) -> Self {
        let installed = ssx_mcp::gateway_client::resolver::install::stage_cargo_gateway(
            std::path::Path::new(env!("CARGO_BIN_EXE_ssx-gateway")),
            std::path::Path::new(env!("CARGO_BIN_EXE_ssx-gateway-dns")),
            #[cfg(windows)]
            std::path::Path::new(env!("CARGO_BIN_EXE_ssx-gateway-input")),
        )
        .expect("preregistered native gateway package fixture");
        let mut command = tokio::process::Command::new(installed);
        command
            .env_clear()
            .env("no_proxy", "*")
            .arg("--remote")
            .arg(remote)
            .args(extra)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        if let Some((bin, record)) = browser {
            command
                .env("PATH", bin)
                .env("SLOPSEARX_TEST_BROWSER_RECORD", record);
        }
        let mut child = command.spawn().unwrap();
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().unwrap()).lines();
        Self {
            child,
            input,
            output,
        }
    }
    async fn rpc(&mut self, value: Value) -> Value {
        let input = self.input.as_mut().unwrap();
        input
            .write_all(format!("{value}\n").as_bytes())
            .await
            .unwrap();
        input.flush().await.unwrap();
        let line = tokio::time::timeout(Duration::from_secs(20), self.output.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        serde_json::from_str(&line).unwrap()
    }
    async fn initialize(&mut self) {
        let result=self.rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"independent-native-client","version":"1"}}})).await;
        assert_eq!(result["result"]["serverInfo"]["name"], "slopsearx-remote");
        assert!(
            result["result"]["instructions"]
                .as_str()
                .unwrap()
                .starts_with("SlopSearX remote gateway.")
        );
    }
    async fn close(mut self) {
        self.input.take();
        let status = tokio::time::timeout(Duration::from_secs(20), self.child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(status.success());
        let mut stderr = String::new();
        use tokio::io::AsyncReadExt;
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .await
            .unwrap();
        assert!(!stderr.contains("static-sentinel"));
        assert!(!stderr.contains("loopback-access"));
    }
}
#[tokio::test]
async fn actual_native_static_process_preserves_signature_schema_projection_error_channels_and_eof_cleanup()
 {
    let fixture = Fixture::start(false).await;
    let mut process = Process::spawn(&fixture, &["--token".into(), "static-sentinel".into()]);
    process.initialize().await;
    let listed = process
        .rpc(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
        .await;
    assert_eq!(
        listed["result"]["tools"][0]["inputSchema"]["x-remote-future"]["opaque"],
        true
    );
    assert_eq!(listed["result"]["tools"][0]["title"], "Echo");
    assert!(
        listed["result"]["tools"][0]["_meta"]
            .get("private")
            .is_none()
    );
    let result=process.rpc(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"echo","arguments":{"query":"7","items":[1,{"future":true}]}}})).await;
    assert_eq!(
        result["result"]["structuredContent"],
        json!({"query":7,"optional":null,"items":[1,{"future":true}]})
    );
    let before = fixture.app.state.lock().await.calls.len();
    let invalid=process.rpc(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"echo","arguments":{"query":"bad"}}})).await;
    assert_eq!(invalid["result"]["isError"], true);
    assert!(invalid.get("error").is_none());
    assert_eq!(fixture.app.state.lock().await.calls.len(), before);
    let before = fixture.app.state.lock().await.calls.len();
    let missing=process.rpc(json!({"jsonrpc":"2.0","id":40,"method":"prompts/get","params":{"name":"research_with_source_coverage","arguments":{}}})).await;
    assert_eq!(missing["error"]["code"], -32603);
    assert_eq!(fixture.app.state.lock().await.calls.len(), before);
    let resources = process
        .rpc(json!({"jsonrpc":"2.0","id":5,"method":"resources/list"}))
        .await;
    assert_eq!(
        resources["result"]["resources"].as_array().unwrap().len(),
        3
    );
    let unknown=process.rpc(json!({"jsonrpc":"2.0","id":6,"method":"resources/read","params":{"uri":"private://credential"}})).await;
    assert_eq!(unknown["error"]["code"], -32602);
    assert_eq!(unknown["error"]["data"]["uri"], "private://credential");
    let read=process.rpc(json!({"jsonrpc":"2.0","id":7,"method":"resources/read","params":{"uri":"slopsearx://capabilities"}})).await;
    assert_eq!(read["result"]["contents"][0]["text"], "first\nsecond");
    let prompt=process.rpc(json!({"jsonrpc":"2.0","id":8,"method":"prompts/get","params":{"name":"research_with_source_coverage","arguments":{"question":"test"}}})).await;
    assert_eq!(
        prompt["result"]["messages"][0]["content"]["text"],
        "first\nsecond"
    );
    assert_eq!(prompt["result"]["messages"][0]["role"], "user");
    process.close().await;
    assert!(fixture.app.state.lock().await.deletes > 0);
    fixture.close().await;
}
#[tokio::test]
async fn actual_native_oauth_process_challenge_registration_pkce_callback_secure_persistence_and_cleanup()
 {
    let fixture = Fixture::start(true).await;
    let callback = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = callback.local_addr().unwrap().port();
    drop(callback);
    let directory =
        std::env::temp_dir().join(format!("ssx-t07-oauth-{}-{port}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("tokens.json");
    let mut process = Process::spawn(
        &fixture,
        &[
            "--oauth".into(),
            "--oauth-no-browser".into(),
            "--oauth-callback-port".into(),
            port.to_string(),
            "--oauth-token-file".into(),
            path.to_string_lossy().into_owned(),
        ],
    );
    let stderr = process.child.stderr.take().unwrap();
    let mut lines = BufReader::new(stderr).lines();
    let authorization = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let line = lines.next_line().await.unwrap().unwrap();
            if line.trim_start().starts_with("http://") {
                break line.trim().to_owned();
            }
        }
    })
    .await
    .unwrap();
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let response = client.get(authorization).send().await.unwrap();
    assert!(response.status().is_success());
    process.initialize().await;
    let tokens: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(tokens["client_info"]["client_id"], "loopback-client");
    assert_eq!(
        tokens["client_info"]["issuer"],
        format!("{}/oauth", fixture.app.base)
    );
    assert_eq!(tokens["tokens"]["access_token"], "loopback-access");
    assert!(tokens.get("token_response").is_none());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // Retain stderr's actual reader owner through process exit, then prove no token diagnostics.
    process.input.take();
    let status = tokio::time::timeout(Duration::from_secs(20), process.child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success());
    let mut remainder = String::new();
    while let Some(line) = lines.next_line().await.unwrap() {
        remainder.push_str(&line);
    }
    assert!(!remainder.contains("loopback-access"));
    assert_eq!(fixture.app.state.lock().await.token_forms.len(), 1);
    assert!(fixture.app.state.lock().await.deletes > 0);
    fixture.close().await;
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn actual_native_startup_failure_redacts_configuration_and_never_publishes() {
    let fixture = Fixture::start(false).await;
    let mut process = Process::spawn(&fixture, &["--token".into(), "wrong-private-secret".into()]);
    process.input.take();
    let output = tokio::time::timeout(Duration::from_secs(20), process.child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(process.output.next_line().await.unwrap().is_none());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(!error.contains("wrong-private-secret"));
    assert!(!error.contains(&fixture.app.base));
    assert!(fixture.app.state.lock().await.calls.is_empty());
    fixture.close().await;
}
#[tokio::test]
async fn actual_native_two_instances_keep_remote_sessions_and_catalogs_independent() {
    let a = Fixture::start(false).await;
    let b = Fixture::start(false).await;
    let args = ["--token".into(), "static-sentinel".into()];
    let mut pa = Process::spawn(&a, &args);
    let mut pb = Process::spawn(&b, &args);
    pa.initialize().await;
    pb.initialize().await;
    pa.close().await;
    assert!(a.app.state.lock().await.deletes > 0);
    assert_eq!(b.app.state.lock().await.deletes, 0);
    let listed = pb
        .rpc(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
        .await;
    assert_eq!(listed["result"]["tools"][0]["name"], "echo");
    pb.close().await;
    a.close().await;
    b.close().await;
}
#[tokio::test]
async fn actual_native_stdio_eof_cancels_inflight_rpc_before_owned_shutdown() {
    let fixture = Fixture::start(false).await;
    let mut process = Process::spawn(&fixture, &["--token".into(), "static-sentinel".into()]);
    process.initialize().await;
    let value = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"echo","arguments":{"query":99}}});
    process
        .input
        .as_mut()
        .unwrap()
        .write_all(format!("{value}\n").as_bytes())
        .await
        .unwrap();
    process.input.as_mut().unwrap().flush().await.unwrap();
    tokio::time::timeout(Duration::from_secs(20), fixture.app.rpc_entered.notified())
        .await
        .unwrap();
    process.close().await;
    let state = fixture.app.state.lock().await;
    assert!(
        state
            .calls
            .iter()
            .any(|v| v["method"] == "notifications/cancelled")
    );
    assert!(state.deletes > 0);
    drop(state);
    fixture.close().await;
}
#[cfg(unix)]
#[tokio::test]
async fn actual_native_sigterm_during_initialization_joins_http_owner() {
    let fixture = Fixture::start_held(false, true).await;
    let mut process = Process::spawn(&fixture, &["--token".into(), "static-sentinel".into()]);
    tokio::time::timeout(Duration::from_secs(20), fixture.app.init_entered.notified())
        .await
        .unwrap();
    let status = tokio::process::Command::new("/bin/kill")
        .args(["-TERM", &process.child.id().unwrap().to_string()])
        .status()
        .await
        .unwrap();
    assert!(status.success());
    let status = tokio::time::timeout(Duration::from_secs(20), process.child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success());
    fixture.app.init_release.notify_one();
    fixture.close().await;
}

#[tokio::test]
async fn actual_native_oauth_non200_refresh_falls_back_to_authorization_without_second_registration()
 {
    for refresh_status in [StatusCode::BAD_REQUEST, StatusCode::CREATED] {
        let fixture = Fixture::start_status_profile(
            true,
            false,
            true,
            StatusCode::OK,
            StatusCode::OK,
            refresh_status,
        )
        .await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let directory =
            std::env::temp_dir().join(format!("ssx-t07-refresh-{}-{port}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("tokens.json");
        let mut process = Process::spawn(
            &fixture,
            &[
                "--oauth".into(),
                "--oauth-no-browser".into(),
                "--oauth-callback-port".into(),
                port.to_string(),
                "--oauth-token-file".into(),
                path.to_string_lossy().into_owned(),
            ],
        );
        let stderr = process.child.stderr.take().unwrap();
        let agent = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            let mut authorizations = 0;
            while let Some(line) = lines.next_line().await.unwrap() {
                assert!(!line.contains("loopback-access"));
                assert!(!line.contains("refresh201-rejected"));
                if line.trim_start().starts_with("http://") {
                    client
                        .get(line.trim())
                        .send()
                        .await
                        .unwrap()
                        .error_for_status()
                        .unwrap();
                    authorizations += 1;
                }
            }
            authorizations
        });
        process.initialize().await;
        process.input.take();
        assert!(
            tokio::time::timeout(Duration::from_secs(20), process.child.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
        assert_eq!(agent.await.unwrap(), 2);
        let observed = fixture.app.state.lock().await;
        assert_eq!(observed.token_forms.len(), 3);
        assert_eq!(observed.registrations, 1);
        assert_eq!(observed.token_forms[0]["grant_type"], "authorization_code");
        assert_eq!(observed.token_forms[1]["grant_type"], "refresh_token");
        assert_eq!(observed.token_forms[2]["grant_type"], "authorization_code");
        drop(observed);
        let tokens: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(tokens["tokens"]["expires_in"], 3600);
        assert_eq!(tokens["tokens"]["access_token"], "loopback-access");
        fixture.close().await;
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[tokio::test]
async fn actual_native_static_userinfo_basic_precedes_bearer_and_fragment_never_reaches_http() {
    let fixture = Fixture::start(false).await;
    *fixture.app.authorization.lock().await = "Basic dXNlcjpwYXNz".into();
    let remote = fixture.app.base.replacen("http://", "http://user:pass@", 1)
        + "/mcp-redirect#private-fragment";
    let mut process = Process::spawn_url(&remote, &["--token".into(), "static-sentinel".into()]);
    process.initialize().await;
    process.close().await;
    assert!(fixture.app.state.lock().await.deletes > 0);
    fixture.close().await;
}

#[tokio::test]
async fn actual_native_oauth_status_matrix_preserves_registration_code_200_201_and_rejects_202_without_token_persistence()
 {
    for (registration_status, token_status, accepted) in [
        (StatusCode::ACCEPTED, StatusCode::OK, false),
        (StatusCode::CREATED, StatusCode::OK, true),
        (StatusCode::OK, StatusCode::CREATED, true),
        (StatusCode::OK, StatusCode::ACCEPTED, false),
    ] {
        let fixture =
            Fixture::start_statuses(true, false, false, registration_status, token_status).await;
        let callback = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = callback.local_addr().unwrap().port();
        drop(callback);
        let directory =
            std::env::temp_dir().join(format!("ssx-t07-status-{}-{port}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("tokens.json");
        let mut process = Process::spawn(
            &fixture,
            &[
                "--oauth".into(),
                "--oauth-no-browser".into(),
                "--oauth-callback-port".into(),
                port.to_string(),
                "--oauth-token-file".into(),
                path.to_string_lossy().into_owned(),
            ],
        );
        let stderr = process.child.stderr.take().unwrap();
        let mut lines = BufReader::new(stderr).lines();
        let mut diagnostic = String::new();
        if registration_status != StatusCode::ACCEPTED {
            let authorization = tokio::time::timeout(Duration::from_secs(20), async {
                loop {
                    let line = lines
                        .next_line()
                        .await
                        .unwrap()
                        .expect("authorization URL before exit");
                    if line.trim_start().starts_with("http://") {
                        break line.trim().to_owned();
                    }
                    diagnostic.push_str(&line)
                }
            })
            .await
            .unwrap();
            let response = reqwest::Client::builder()
                .no_proxy()
                .build()
                .unwrap()
                .get(authorization)
                .send()
                .await
                .unwrap();
            assert!(response.status().is_success());
        }
        if accepted {
            process.initialize().await;
        }
        process.input.take();
        let status = tokio::time::timeout(Duration::from_secs(20), process.child.wait())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status.success(), accepted);
        while let Some(line) = lines.next_line().await.unwrap() {
            diagnostic.push_str(&line);
        }
        assert!(!diagnostic.contains("loopback-access"));
        assert!(!diagnostic.contains("loopback-refresh"));
        if accepted {
            let stored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!(stored["tokens"]["access_token"], "loopback-access");
        } else {
            assert!(process.output.next_line().await.unwrap().is_none());
            if path.exists() {
                let raw = std::fs::read(&path).unwrap();
                let stored: Value = serde_json::from_slice(&raw).unwrap();
                assert!(stored.get("tokens").is_none_or(Value::is_null));
                assert!(!String::from_utf8(raw).unwrap().contains("loopback-access"));
            }
            assert!(fixture.app.state.lock().await.calls.is_empty());
        }
        if registration_status != StatusCode::ACCEPTED {
            assert_eq!(fixture.app.state.lock().await.token_forms.len(), 1);
        }
        if registration_status == StatusCode::ACCEPTED {
            assert!(!diagnostic.contains("Authorize the SlopSearX gateway"));
            assert!(!path.exists());
            assert!(fixture.app.state.lock().await.token_forms.is_empty());
        }
        fixture.close().await;
        std::fs::remove_dir_all(directory).unwrap();
    }
}

// Current-main integration: these are the accepted native SearchSurface and
// disposable Valkey fixtures, never a broad synthetic echo catalog.
#[path = "../../ssx-mcp/tests/t06_support/fixture.rs"]
mod current_t06_fixture;
#[path = "../../ssx-mcp/tests/t06_support/server.rs"]
mod current_t06_valkey;
#[tokio::test]
async fn actual_native_current_t06_eleven_tool_catalog_and_discovery_match_direct_http()
-> ssx_mcp::Result<()> {
    use ssx_mcp::{
        harness::{Host, http_router_for},
        oauth::NetworkAuth,
        search_surface::SearchSurface,
    };
    let valkey = current_t06_valkey::Server::start();
    let fixture = current_t06_fixture::Fixture::frozen(valkey.url(), false).await;
    let stop = CancellationToken::new();
    let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("static-sentinel".into())?);
    let host = Host::start(
        http_router_for(
            fixture.surface.clone(),
            SearchSurface::catalogs(),
            stop.clone(),
        )
        .map(|r| auth.mount(r)),
        stop,
    )
    .await?;
    let url = format!("http://{}/mcp", host.address);
    let client = reqwest::Client::builder().no_proxy().build()?;
    let mut gateway = Process::spawn_url(&url, &["--token".into(), "static-sentinel".into()]);
    let result = async {
        async fn direct(client: &reqwest::Client, url: &str, request: &Value) -> ssx_mcp::Result<Value> {
            let response = client.post(url).bearer_auth("static-sentinel")
                .header("accept", "application/json,text/event-stream")
                .header("content-type", "application/json")
                .header("mcp-protocol-version", "2025-11-25")
                .body(request.to_string()).send().await?;
            assert_eq!(response.status(), StatusCode::OK);
            Ok(serde_json::from_str(&response.text().await?)?)
        }
        gateway.initialize().await;
        let catalog_request = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}});
        let direct_catalog = direct(&client, &url, &catalog_request).await?;
        let gateway_catalog = gateway.rpc(catalog_request).await;
        let expected = ["slopsearx_search", "slopsearx_search_targeted", "slopsearx_search_jobs", "slopsearx_search_security", "slopsearx_search_science", "slopsearx_list_capabilities", "slopsearx_explain_search_scope", "slopsearx_get_service_status", "slopsearx_read_results", "slopsearx_read_result", "slopsearx_read_entities"];
        let direct_tools = direct_catalog["result"]["tools"].as_array().unwrap();
        let gateway_tools = gateway_catalog["result"]["tools"].as_array().unwrap();
        assert_eq!(direct_tools.len(), 11);
        assert_eq!(gateway_tools.len(), 11);
        for name in expected {
            let original = direct_tools.iter().find(|t| t["name"] == name).unwrap();
            let proxied = gateway_tools.iter().find(|t| t["name"] == name).unwrap();
            assert_eq!(proxied["inputSchema"], original["inputSchema"], "full {name} schema");
        }
        for (id, name, arguments) in [(3, "slopsearx_list_capabilities", json!({})), (4, "slopsearx_get_service_status", json!({}))] {
            let request = json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}});
            let call_started = chrono::Utc::now();
            let mut original = direct(&client, &url, &request).await?;
            let mut proxied = gateway.rpc(request).await;
            let call_finished = chrono::Utc::now();
            // Service health uses the live observation clock even in this
            // deadline-frozen fixture. Validate each timestamp independently,
            // then compare every other declared status field exactly.
            if name == "slopsearx_get_service_status" {
                for response in [&mut original, &mut proxied] {
                    let status = response["result"]["structuredContent"].as_object_mut().expect("native status object");
                    let freshness = status.get("freshness").and_then(Value::as_str).expect("native status freshness string");
                    let observed = chrono::DateTime::parse_from_rfc3339(freshness).expect("native status RFC3339 freshness");
                    assert!(observed.timestamp_micros() >= call_started.timestamp_micros()
                        && observed.timestamp_micros() <= call_finished.timestamp_micros(),
                        "native status freshness outside observed call bounds: {freshness}");
                    status.remove("freshness");
                }
            }
            assert!(original["result"]["structuredContent"].is_object(), "direct native {name} object required");
            assert!(proxied["result"]["structuredContent"].is_object(), "gateway native {name} object required");
            assert_eq!(proxied["result"]["structuredContent"], original["result"]["structuredContent"], "native {name} result");
            assert_ne!(proxied["result"]["isError"], true);
        }
        let rejected = gateway.rpc(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"slopsearx_search","arguments":{}}})).await;
        assert_eq!(rejected["result"]["isError"], true);
        assert_eq!(fixture.calls(), 0, "passive discovery or rejected call dispatched an engine");
        Ok::<(), ssx_mcp::Error>(())
    }.await;
    gateway.close().await;
    host.shutdown().await?;
    fixture.service.shutdown().await;
    result
}

/// Eight actual fresh-process cases: denied DELETE401/403 on EOF/SIGTERM,
/// each with a valid and an expired bearer; cleanup may neither refresh nor
/// reopen interactive authorization. The private browser executable is a spy.
#[cfg(unix)]
#[tokio::test]
async fn actual_native_oauth_denied_delete_on_eof_and_sigterm_never_reauthorizes_or_refreshes_and_retires_owners()
-> ssx_mcp::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::AsyncReadExt;
    for status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
        for signal in [false, true] {
            for expire_before_stop in [false, true] {
                let fixture = Fixture::start(true).await;
                *fixture.app.delete_status.lock().await = status;
                *fixture.app.token_expiry.lock().await = if expire_before_stop { 5 } else { 3600 };
                let reserved = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
                let port = reserved.local_addr()?.port();
                drop(reserved);
                let directory = std::env::temp_dir()
                    .join(format!("ssx-t07-stop-{}-{port}", std::process::id()));
                std::fs::create_dir(&directory)?;
                let bin = directory.join("bin");
                std::fs::create_dir(&bin)?;
                let record = directory.join("browser-record");
                let token_file = directory.join("tokens.json");
                let script = b"#!/bin/sh\nprintf '%s\\n' \"$$\" \"$1\" >> \"$SLOPSEARX_TEST_BROWSER_RECORD\"\n";
                #[cfg(target_os = "macos")]
                let browser = bin.join("open");
                #[cfg(not(target_os = "macos"))]
                let browser = bin.join("xdg-open");
                std::fs::write(&browser, script)?;
                std::fs::set_permissions(&browser, std::fs::Permissions::from_mode(0o700))?;
                let mut process = Process::spawn_url_with_browser(
                    &format!("{}/mcp", fixture.app.base),
                    &[
                        "--oauth".into(),
                        "--oauth-callback-port".into(),
                        port.to_string(),
                        "--oauth-token-file".into(),
                        token_file.to_string_lossy().into_owned(),
                    ],
                    Some((&bin, &record)),
                );
                let stderr = process.child.stderr.take().unwrap();
                let urls: Arc<Mutex<Vec<String>>> = Default::default();
                let observed_urls = urls.clone();
                let mut stderr_task = tokio::spawn(async move {
                    let mut stderr = BufReader::new(stderr).lines();
                    let mut text = String::new();
                    while let Some(line) = stderr.next_line().await? {
                        if text.len() + line.len() > 65536 {
                            return Err(std::io::Error::other(
                                "private gateway stderr cap exceeded",
                            ));
                        }
                        if line.trim_start().starts_with("http://") {
                            observed_urls.lock().await.push(line.trim().to_owned());
                        }
                        text.push_str(&line);
                        text.push('\n');
                    }
                    Ok::<_, std::io::Error>(text)
                });
                let stage = std::cell::Cell::new("initial authorization URL");
                let outcome = catch_process_case(async {
                let client = reqwest::Client::builder().no_proxy().build()?;
                let authorization = tokio::time::timeout(Duration::from_secs(10), async {
                    loop {
                        if let Some(url) = urls.lock().await.first().cloned() { break url; }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }).await?;
                stage.set("initial authorization callback visit");
                assert!(client.get(&authorization).send().await?.status().is_success());
                stage.set("stdio initialization");
                process.initialize().await;
                stage.set("initial browser record");
                tokio::time::timeout(Duration::from_secs(3), async {
                    loop {
                        if std::fs::read_to_string(&record).is_ok_and(|v| v.lines().count() == 2) { break; }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }).await?;
                let before_file = std::fs::read(&token_file)?;
                {
                    let observed = fixture.app.state.lock().await;
                    assert_eq!(observed.registrations, 1);
                    assert_eq!(observed.authorization_visits, 1);
                    assert_eq!(observed.token_forms.len(), 1, "initial token must not have refreshed before the stop control");
                }
                assert_eq!(urls.lock().await.as_slice(), std::slice::from_ref(&authorization));
                // Source expiry is wall-clock based. Wait past the five-second
                // issued expiry with no intervening MCP request.
                if expire_before_stop {
                    tokio::time::sleep(Duration::from_secs(6)).await;
                }
                let stopped_at = tokio::time::Instant::now();
                if signal {
                    let killed = tokio::process::Command::new("/bin/kill").args(["-TERM", &process.child.id().ok_or("gateway PID unavailable")?.to_string()]).status().await?;
                    assert!(killed.success());
                } else {
                    process.input.take();
                }
                stage.set("owned exit after stop");
                let exit = tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        if urls.lock().await.len() != 1 {
                            return Err("cleanup opened a second authorization URL");
                        }
                        let observed = fixture.app.state.lock().await;
                        if observed.registrations != 1 || observed.authorization_visits != 1 || observed.token_forms.len() != 1 {
                            return Err("cleanup performed OAuth registration, authorization or refresh");
                        }
                        drop(observed);
                        if let Ok(value) = std::fs::read_to_string(&record)
                            && value.lines().count() != 2 {
                            return Err("cleanup launched a second browser");
                        }
                        match process.child.try_wait() {
                            Ok(Some(exit)) => break Ok(exit),
                            Ok(None) => tokio::time::sleep(Duration::from_millis(10)).await,
                            Err(_) => return Err("gateway wait failed"),
                        }
                    }
                }).await?.map_err(std::io::Error::other)?;
                assert!(stopped_at.elapsed() < Duration::from_secs(5));
                assert!(!exit.success(), "denied remote cleanup must remain an explicit failure");
                assert_eq!(std::fs::read(&token_file)?, before_file, "cleanup mutated stored credentials");
                assert_eq!(urls.lock().await.len(), 1);
                let observed = fixture.app.state.lock().await;
                assert_eq!(observed.deletes, 1);
                assert_eq!(observed.delete_authorizations, [Some("Bearer loopback-access".into())]);
                assert_eq!(observed.registrations, 1);
                assert_eq!(observed.authorization_visits, 1);
                assert_eq!(observed.token_forms.len(), 1);
                drop(observed);
                let lines = std::fs::read_to_string(&record)?;
                assert_eq!(lines.lines().count(), 2, "exactly one browser process");
                let pid = lines.lines().next().ok_or("browser PID missing")?;
                let alive = tokio::process::Command::new("/bin/kill").args(["-0", pid]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().await?;
                assert!(!alive.success(), "browser process not retired/reaped");
                // Actual callback listener is owned until shutdown; rebinding
                // after the joined child proves it was released on both paths.
                let rebound = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
                drop(rebound);
                Ok::<(), ssx_mcp::Error>(())
            }).await;
                // Even a failed oracle retains the actual process and stderr owner
                // until killed/waited, so a negative control cannot leak a process.
                if process.child.try_wait()?.is_none() {
                    process.child.start_kill()?;
                }
                process.child.wait().await?;
                process.input.take();
                let stderr_result: ssx_mcp::Result<String> =
                    match tokio::time::timeout(Duration::from_secs(5), &mut stderr_task).await {
                        Ok(joined) => joined
                            .map_err(|e| -> ssx_mcp::Error { e.into() })
                            .and_then(|value| value.map_err(|e| -> ssx_mcp::Error { e.into() })),
                        Err(_) => {
                            stderr_task.abort();
                            let _ = stderr_task.await;
                            Err("private stderr reader did not retire".into())
                        }
                    };
                let mut stdout = String::new();
                let mut output = process.output.into_inner();
                let stdout_result = tokio::time::timeout(
                    Duration::from_secs(5),
                    output.read_to_string(&mut stdout),
                )
                .await;
                let browser_record = std::fs::read_to_string(&record).ok();
                let observed = fixture.app.state.lock().await;
                let fixture_counts = (
                    observed.registrations,
                    observed.authorization_visits,
                    observed.token_forms.len(),
                    observed.deletes,
                    observed.cancel_authorizations.len(),
                );
                drop(observed);
                fixture.close().await;
                std::fs::remove_dir_all(&directory)?;
                if !matches!(&outcome, Ok(Ok(()))) {
                    eprintln!(
                        "T07_PRIVATE_CASE_FAILURE status={status:?} signal={signal} expired={expire_before_stop} stage={} fixture_counts={fixture_counts:?} stderr={stderr_result:?} browser_record={browser_record:?} stdout={stdout:?}",
                        stage.get()
                    );
                }
                finish_process_case(outcome)?;
                let stderr = stderr_result?;
                stdout_result??;
                assert!(stdout.is_empty(), "gateway emitted data after stop");
                assert!(stderr.contains("gateway connection or cleanup failed"));
                assert!(
                    !stderr.contains("loopback-access") && !stderr.contains("loopback-refresh")
                );
            }
        }
    }
    Ok(())
}

/// The second authorization URL proves the actual original request has reached
/// callback wait while retaining OAuth.manager. Its cancellation notification
/// must use the independent held bearer, even if that POST is itself denied.
#[cfg(unix)]
#[tokio::test]
async fn actual_native_held_oauth_request_cancel_eof_sigterm_do_not_reenter_auth_or_deadlock_and_retire_owners()
-> ssx_mcp::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::AsyncReadExt;
    for cancel_status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
        // 0 = explicit RPC cancellation then EOF, 1 = EOF, 2 = SIGTERM.
        for stop_mode in 0..3 {
            let fixture = Fixture::start(true).await;
            *fixture.app.challenge_rpc.lock().await = true;
            *fixture.app.cancel_status.lock().await = cancel_status;
            let reserved = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let port = reserved.local_addr()?.port();
            drop(reserved);
            let directory =
                std::env::temp_dir().join(format!("ssx-t07-held-{}-{port}", std::process::id()));
            std::fs::create_dir(&directory)?;
            let bin = directory.join("bin");
            std::fs::create_dir(&bin)?;
            let record = directory.join("browser-record");
            let token_file = directory.join("tokens.json");
            #[cfg(target_os = "macos")]
            let browser = bin.join("open");
            #[cfg(not(target_os = "macos"))]
            let browser = bin.join("xdg-open");
            std::fs::write(
                &browser,
                b"#!/bin/sh\nprintf '%s\\n' \"$$\" \"$1\" >> \"$SLOPSEARX_TEST_BROWSER_RECORD\"\n",
            )?;
            std::fs::set_permissions(&browser, std::fs::Permissions::from_mode(0o700))?;
            let mut process = Process::spawn_url_with_browser(
                &format!("{}/mcp", fixture.app.base),
                &[
                    "--oauth".into(),
                    "--oauth-callback-port".into(),
                    port.to_string(),
                    "--oauth-token-file".into(),
                    token_file.to_string_lossy().into_owned(),
                ],
                Some((&bin, &record)),
            );
            let stderr = process.child.stderr.take().unwrap();
            let urls: Arc<Mutex<Vec<String>>> = Default::default();
            let observed_urls = urls.clone();
            let mut stderr_task = tokio::spawn(async move {
                let mut stderr = BufReader::new(stderr).lines();
                let mut text = String::new();
                while let Some(line) = stderr.next_line().await? {
                    if text.len() + line.len() > 65536 {
                        return Err(std::io::Error::other("private gateway stderr cap exceeded"));
                    }
                    if line.trim_start().starts_with("http://") {
                        observed_urls.lock().await.push(line.trim().to_owned());
                    }
                    text.push_str(&line);
                    text.push('\n');
                }
                Ok::<_, std::io::Error>(text)
            });
            let stage = std::cell::Cell::new("initial authorization URL");
            let outcome = catch_process_case(async {
                let client = reqwest::Client::builder().no_proxy().build()?;
                let first = tokio::time::timeout(Duration::from_secs(10), async {
                    loop {
                        if let Some(url) = urls.lock().await.first().cloned() { break url; }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }).await?;
                stage.set("initial authorization callback visit");
                assert!(client.get(first).send().await?.status().is_success());
                stage.set("stdio initialization");
                process.initialize().await;
                let before_file = std::fs::read(&token_file)?;
                let call = json!({"jsonrpc":"2.0","id":99,"method":"tools/call","params":{"name":"echo","arguments":{"query":99}}});
                stage.set("first browser helper exit before held challenge");
                tokio::time::timeout(Duration::from_secs(10), async {
                    // BrowserOwner intentionally avoids spawning while its last
                    // helper is still running. Observe actual helper exit before
                    // provoking the second challenge; the next open reaps it.
                    loop {
                        if let Ok(recorded) = std::fs::read_to_string(&record)
                            && let Some(pid) = recorded.lines().next() {
                            let state = tokio::process::Command::new("/bin/ps")
                                .args(["-p", pid, "-o", "stat="]).output().await?;
                            if state.status.code() == Some(1)
                                || (state.status.success() && String::from_utf8_lossy(&state.stdout).trim_start().starts_with('Z')) {
                                break;
                            }
                            assert!(state.status.success(), "private browser process-state observation failed");
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    process.input.as_mut().unwrap().write_all(format!("{call}\n").as_bytes()).await?;
                    process.input.as_mut().unwrap().flush().await?;
                    stage.set("held second authorization URL and browser record");
                    loop {
                        if urls.lock().await.len() == 2 && std::fs::read_to_string(&record).is_ok_and(|v| v.lines().count() == 4) { break; }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    Ok::<(), ssx_mcp::Error>(())
                }).await??;
                // Deliberately do not visit the second URL. The original future
                // is now suspended holding manager, not a synthetic pending task.
                assert_eq!(urls.lock().await.len(), 2);
                let stopped_at = tokio::time::Instant::now();
                if stop_mode == 0 {
                    let cancel = json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":99}});
                    process.input.as_mut().unwrap().write_all(format!("{cancel}\n").as_bytes()).await?;
                    process.input.as_mut().unwrap().flush().await?;
                    stage.set("remote cancellation notification");
                    tokio::time::timeout(Duration::from_secs(3), async {
                        loop {
                            if !fixture.app.state.lock().await.cancel_authorizations.is_empty() { break; }
                            if urls.lock().await.len() != 2 { return Err("RPC cancellation started a third authorization"); }
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                        Ok::<(), &'static str>(())
                    }).await?.map_err(std::io::Error::other)?;
                    // Before EOF/global auth cancellation, prove the original
                    // suspended authorize future has actually dropped its manager
                    // lock: a fresh normal RPC must acquire it and round-trip.
                    let probe = json!({"jsonrpc":"2.0","id":100,"method":"tools/call","params":{"name":"echo","arguments":{"query":7}}});
                    process.input.as_mut().unwrap().write_all(format!("{probe}\n").as_bytes()).await?;
                    process.input.as_mut().unwrap().flush().await?;
                    stage.set("normal RPC after cancellation");
                    let reply = tokio::time::timeout(Duration::from_secs(3), async {
                        loop {
                            let line = process.output.next_line().await?.ok_or_else(|| std::io::Error::other("stdio closed before post-cancel probe"))?;
                            let response: Value = serde_json::from_str(&line).map_err(std::io::Error::other)?;
                            if response["id"] == 100 { break Ok::<Value, std::io::Error>(response); }
                            assert_eq!(response["id"], 99, "unexpected response before post-cancel probe");
                        }
                    }).await??;
                    assert_eq!(reply["result"]["structuredContent"]["query"], 7);
                    assert_eq!(urls.lock().await.len(), 2);
                    assert!(tokio::net::TcpListener::bind(("127.0.0.1", port)).await.is_err(), "pending callback owner was lost before shutdown");
                    process.input.take();
                } else if stop_mode == 1 {
                    process.input.take();
                } else {
                    assert!(tokio::process::Command::new("/bin/kill").args(["-TERM", &process.child.id().ok_or("gateway PID unavailable")?.to_string()]).status().await?.success());
                }
                stage.set("owned exit after stop");
                let exit = tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        if urls.lock().await.len() != 2 { return Err("control or cleanup opened a third authorization URL"); }
                        let observed = fixture.app.state.lock().await;
                        if observed.registrations != 1 || observed.authorization_visits != 1 || observed.token_forms.len() != 1 { return Err("control or cleanup entered OAuth state/refresh/authorization"); }
                        drop(observed);
                        if let Ok(value) = std::fs::read_to_string(&record) && value.lines().count() != 4 { return Err("control or cleanup launched another browser"); }
                        match process.child.try_wait() {
                            Ok(Some(exit)) => break Ok(exit),
                            Ok(None) => tokio::time::sleep(Duration::from_millis(10)).await,
                            Err(_) => return Err("gateway wait failed"),
                        }
                    }
                }).await?.map_err(std::io::Error::other)?;
                assert!(stopped_at.elapsed() < Duration::from_secs(8));
                assert!(exit.success(), "denied cancel notification is best effort; successful session DELETE still closes cleanly");
                assert_eq!(std::fs::read(&token_file)?, before_file);
                assert_eq!(urls.lock().await.len(), 2);
                let observed = fixture.app.state.lock().await;
                assert_eq!(observed.cancel_authorizations, vec![Some("Bearer loopback-access".into())]);
                assert!(observed.calls.iter().any(|value| value["method"] == "notifications/cancelled" && value["params"]["requestId"] == 99));
                assert_eq!(observed.deletes, 1);
                assert_eq!(observed.registrations, 1);
                assert_eq!(observed.authorization_visits, 1);
                assert_eq!(observed.token_forms.len(), 1);
                drop(observed);
                let lines = std::fs::read_to_string(&record)?;
                assert_eq!(lines.lines().count(), 4);
                for pid in lines.lines().step_by(2) {
                    let alive = tokio::process::Command::new("/bin/kill").args(["-0", pid]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().await?;
                    assert!(!alive.success(), "browser owner survived gateway exit");
                }
                let rebound = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
                drop(rebound);
                Ok::<(), ssx_mcp::Error>(())
            }).await;
            if process.child.try_wait()?.is_none() {
                process.child.start_kill()?;
            }
            process.child.wait().await?;
            process.input.take();
            let stderr_result: ssx_mcp::Result<String> =
                match tokio::time::timeout(Duration::from_secs(5), &mut stderr_task).await {
                    Ok(joined) => joined
                        .map_err(|e| -> ssx_mcp::Error { e.into() })
                        .and_then(|value| value.map_err(|e| -> ssx_mcp::Error { e.into() })),
                    Err(_) => {
                        stderr_task.abort();
                        let _ = stderr_task.await;
                        Err("private stderr reader did not retire".into())
                    }
                };
            let mut stdout = String::new();
            let mut output = process.output.into_inner();
            let stdout_result =
                tokio::time::timeout(Duration::from_secs(5), output.read_to_string(&mut stdout))
                    .await;
            let browser_record = std::fs::read_to_string(&record).ok();
            let observed = fixture.app.state.lock().await;
            let fixture_counts = (
                observed.registrations,
                observed.authorization_visits,
                observed.token_forms.len(),
                observed.deletes,
                observed.cancel_authorizations.len(),
            );
            drop(observed);
            fixture.close().await;
            std::fs::remove_dir_all(&directory)?;
            if !matches!(&outcome, Ok(Ok(()))) {
                eprintln!(
                    "T07_PRIVATE_CASE_FAILURE cancel_status={cancel_status:?} stop_mode={stop_mode} stage={} fixture_counts={fixture_counts:?} stderr={stderr_result:?} browser_record={browser_record:?} stdout={stdout:?}",
                    stage.get()
                );
            }
            finish_process_case(outcome)?;
            let stderr = stderr_result?;
            stdout_result??;
            assert!(!stderr.contains("loopback-access") && !stderr.contains("loopback-refresh"));
            assert!(!stdout.contains("loopback-access") && !stdout.contains("loopback-refresh"));
            for line in stdout.lines() {
                assert_eq!(serde_json::from_str::<Value>(line)?["jsonrpc"], "2.0");
            }
        }
    }
    Ok(())
}
