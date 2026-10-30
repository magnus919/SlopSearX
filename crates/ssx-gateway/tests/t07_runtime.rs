//! Hermetic actual native executable fixtures, not a simulated forwarding seam.
#[path = "platform_support/production.rs"]
mod production;
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
    catalog_override: Option<String>,
    registration: Option<Value>,
    registrations: usize,
    challenge: Option<String>,
    token_forms: Vec<HashMap<String, String>>,
    token_redirect_visits: usize,
    deletes: usize,
    authorization_visits: usize,
    private_callback_visits: usize,
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
    private_callback_release: Arc<tokio::sync::Notify>,
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
            if let Some(tools) = app.state.lock().await.catalog_override.clone() {
                return (
                    [
                        ("mcp-session-id", "private-loopback-session"),
                        ("content-type", "application/json"),
                    ],
                    format!(
                        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{tools}}}",
                        value["id"]
                    ),
                )
                    .into_response();
            }
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
async fn redirect_token(State(app): State<App>) -> Response {
    app.state.lock().await.token_redirect_visits += 1;
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
    assert_eq!(value["application_type"], "native");
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
            private_callback_release: Default::default(),
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
            .route(
                "/private-callback-hold",
                get(|State(app): State<App>| async move {
                    app.state.lock().await.private_callback_visits += 1;
                    app.private_callback_release.notified().await;
                    StatusCode::OK
                }),
            )
            .route(
                "/private-callback-error",
                get(|State(app): State<App>| async move {
                    app.state.lock().await.private_callback_visits += 1;
                    StatusCode::INTERNAL_SERVER_ERROR
                }),
            )
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CallbackFailure {
    Timeout,
    Transport,
    Status,
    BodyBound,
}
impl CallbackFailure {
    fn stage(self) -> &'static str {
        match self {
            Self::Timeout => "callback_timeout",
            Self::Transport => "callback_transport",
            Self::Status => "callback_status",
            Self::BodyBound => "callback_body_bound",
        }
    }
}
async fn bounded_callback(
    client: &reqwest::Client,
    url: &str,
    budget: Duration,
) -> Result<(), CallbackFailure> {
    tokio::time::timeout(budget, async {
        let mut response = client
            .get(url)
            .send()
            .await
            .map_err(|_| CallbackFailure::Transport)?;
        if !response.status().is_success() {
            return Err(CallbackFailure::Status);
        }
        let mut bytes = 0usize;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| CallbackFailure::Transport)?
        {
            bytes = bytes.saturating_add(chunk.len());
            if bytes > 16384 {
                return Err(CallbackFailure::BodyBound);
            }
        }
        Ok(())
    })
    .await
    .map_err(|_| CallbackFailure::Timeout)?
}
struct CaseAux<'a> {
    agent: &'a mut Option<tokio::task::JoinHandle<Result<usize, RefreshReadError>>>,
    diagnostic: Arc<Mutex<Vec<u8>>>,
}
#[derive(Clone, Copy)]
struct AclPreflight {
    ordinary_admitted: bool,
    io_kind: Option<std::io::ErrorKind>,
    os_code: Option<i32>,
}
struct Observation {
    failure: Option<ProcessFailure>,
    retired: Option<std::process::ExitStatus>,
    stderr_retired: bool,
    truncated: bool,
    disclosed: bool,
    observer_state: Option<&'static str>,
    authorization_count: Option<usize>,
    acl: Option<AclPreflight>,
    counts: Option<(usize, usize, usize, usize, usize)>,
    retained: Vec<u8>,
}
impl Observation {
    // Invoked only after the enclosing owned scope has finished all cleanup.
    // These are finite typed fields plus the shared bounded whitelist buffer.
    fn emit(
        &self,
        sink: &mut impl std::io::Write,
        process: bool,
        fixture: bool,
        owned_path: bool,
    ) -> bool {
        let acl = self
            .acl
            .map(|acl| (acl.ordinary_admitted, acl.io_kind, acl.os_code));
        let line = format!(
            "T07_PRIVATE_CASE_RETIREMENT process={process} fixture={fixture} owned_path={owned_path} failure={:?} retired={:?} stderr_retired={} truncated={} disclosed={} observer_state={:?} authorization_count={:?} acl={acl:?} counts={:?} stderr={:?}\n",
            self.failure,
            self.retired,
            self.stderr_retired,
            self.truncated,
            self.disclosed,
            self.observer_state,
            self.authorization_count,
            self.counts,
            String::from_utf8_lossy(&self.retained)
        );
        sink.write_all(line.as_bytes()).is_ok() && sink.flush().is_ok()
    }
}
struct CaseReport {
    observation: Observation,
    primary: std::thread::Result<ssx_mcp::Result<()>>,
    process_retired: bool,
    fixture_retired: bool,
    owned_path_retired: bool,
    stage: Option<&'static str>,
}
impl CaseReport {
    fn resume(self) {
        let _ = self.observation.emit(
            &mut std::io::stderr().lock(),
            self.process_retired,
            self.fixture_retired,
            self.owned_path_retired,
        );
        let cleanup = self.process_retired && self.fixture_retired && self.owned_path_retired;
        match self.primary {
            Err(primary) => std::panic::resume_unwind(primary),
            Ok(Err(_)) => panic!(
                "private case failed after cleanup; finite stage={:?} cleanup={cleanup}",
                self.stage
            ),
            Ok(Ok(())) => assert!(cleanup, "private case cleanup failed"),
        }
    }
}
async fn finish_case(
    process: &mut Process,
    fixture: Fixture,
    directory: Option<std::path::PathBuf>,
    expected_success: bool,
    mut aux: Option<CaseAux<'_>>,
    mut primary: std::thread::Result<ssx_mcp::Result<()>>,
) -> CaseReport {
    let already_retired = process.retired_status.is_some() && process.stderr_retired;
    if !already_retired {
        if matches!(&primary, Ok(Ok(()))) && aux.as_ref().is_none_or(|aux| aux.agent.is_none()) {
            let closing = catch_process_case(process.finish_owned(expected_success)).await;
            if let Err(error) = closing {
                primary = Err(error);
            }
        } else {
            let failure = process
                .last_failure
                .unwrap_or_else(|| ProcessFailure::plain("case_body"));
            process.report_failure(failure, None).await;
            if matches!(&primary, Ok(Ok(()))) {
                primary = Ok(Err("private case left auxiliary owner active".into()));
            }
        }
    }
    if let Some(aux) = aux.as_mut() {
        if let Some(agent) = aux.agent.as_mut() {
            match tokio::time::timeout(Duration::from_secs(5), &mut *agent).await {
                Ok(_) => {
                    aux.agent.take();
                    process.stderr_retired = true;
                }
                Err(_) => {
                    agent.abort();
                    if let Some(agent) = aux.agent.take() {
                        let _ = agent.await;
                    }
                    process.stderr_retired = true;
                    if matches!(&primary, Ok(Ok(()))) {
                        primary = Ok(Err(
                            "private callback observer exceeded retirement bound".into()
                        ));
                    }
                }
            }
        }
        process
            .retained_stderr
            .clone_from(&*aux.diagnostic.lock().await);
    }
    let process_retired = process.retired_status.is_some() && process.stderr_retired;
    fixture.app.private_callback_release.notify_waiters();
    fixture.stop.cancel();
    let mut server = fixture.task;
    let fixture_retired = match tokio::time::timeout(Duration::from_secs(5), &mut server).await {
        Ok(Ok(Ok(()))) => true,
        Ok(_) => false,
        Err(_) => {
            server.abort();
            let _ = server.await;
            false
        }
    };
    let owned_path_retired = match directory {
        None => true,
        Some(path) if process_retired && fixture_retired => std::fs::remove_dir_all(path).is_ok(),
        Some(_) => false,
    };
    if !matches!(&primary, Ok(Ok(()))) && process.last_failure.is_none() {
        process.last_failure = Some(ProcessFailure::plain("case_body"));
    }
    let counts = tokio::time::timeout(Duration::from_secs(1), fixture.app.state.lock())
        .await
        .ok()
        .map(|state| {
            (
                state.registrations,
                state.authorization_visits,
                state.token_forms.len(),
                state.deletes,
                state.private_callback_visits,
            )
        });
    let observation = process.observation(counts);
    CaseReport {
        observation,
        primary,
        process_retired,
        fixture_retired,
        owned_path_retired,
        stage: process.last_failure.map(|failure| failure.stage),
    }
}
macro_rules! owned_case {
    ($process:ident, $fixture:ident, $directory:expr, $success:expr, $aux:expr, $body:block) => {{
        let primary = catch_process_case(async $body).await;
        finish_case(&mut $process, $fixture, $directory, $success, $aux, primary).await
    }};
}

// The private refresh observer bounds both line assembly and retained bytes.
// It drains overflow to newline/EOF so its writer can still retire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefreshReadError {
    Read,
    LineBound,
    RetentionBound,
    Encoding,
    Credential,
    CallbackStatus,
    CallbackRequest,
    CallbackTimeout,
    CallbackBodyBound,
    Join,
    Retirement,
}
fn refresh_agent_fields(result: &Result<usize, RefreshReadError>) -> (&'static str, Option<usize>) {
    match result {
        Ok(count) => ("completed", Some(*count)),
        Err(error) => (
            match error {
                RefreshReadError::Read => "read",
                RefreshReadError::LineBound => "line_bound",
                RefreshReadError::RetentionBound => "retention_bound",
                RefreshReadError::Encoding => "encoding",
                RefreshReadError::Credential => "credential",
                RefreshReadError::CallbackStatus => "callback_status",
                RefreshReadError::CallbackRequest => "callback_request",
                RefreshReadError::CallbackTimeout => "callback_timeout",
                RefreshReadError::CallbackBodyBound => "callback_body_bound",
                RefreshReadError::Join => "join",
                RefreshReadError::Retirement => "retirement",
            },
            None,
        ),
    }
}
async fn read_refresh_stderr(
    mut input: impl tokio::io::AsyncRead + Unpin,
    client: reqwest::Client,
    diagnostic: Arc<Mutex<Vec<u8>>>,
) -> Result<usize, RefreshReadError> {
    use tokio::io::AsyncReadExt;
    let mut chunk = [0u8; 1024];
    let mut line = Vec::new();
    let mut overflow = false;
    let mut reader_error = None;
    let mut authorizations = 0;
    loop {
        let count = input
            .read(&mut chunk)
            .await
            .map_err(|_| RefreshReadError::Read)?;
        if count == 0 {
            if line.is_empty() && !overflow {
                break;
            }
            // Classify the final unterminated bounded line once.
            chunk[0] = b'\n';
        }
        for byte in &chunk[..count.max(1)] {
            if *byte != b'\n' {
                if line.len() < 16384 {
                    line.push(*byte);
                } else {
                    overflow = true;
                    reader_error.get_or_insert(RefreshReadError::LineBound);
                }
                continue;
            }
            if !overflow {
                match std::str::from_utf8(&line) {
                    Err(_) => {
                        reader_error.get_or_insert(RefreshReadError::Encoding);
                    }
                    Ok(text) => {
                        if [
                            "loopback-access",
                            "loopback-refresh",
                            "refresh201-rejected",
                            "private-code",
                            "static-sentinel",
                        ]
                        .iter()
                        .any(|token| text.contains(token))
                        {
                            reader_error.get_or_insert(RefreshReadError::Credential);
                        } else if text.trim_start().starts_with("http://") {
                            // Complete bounded URLs still drive both private
                            // callbacks; their queries never enter diagnostics.
                            match bounded_callback(&client, text.trim(), Duration::from_secs(20))
                                .await
                            {
                                Ok(()) => {
                                    authorizations += 1;
                                }
                                Err(failure) => {
                                    reader_error.get_or_insert(match failure {
                                        CallbackFailure::Status => RefreshReadError::CallbackStatus,
                                        CallbackFailure::Timeout => {
                                            RefreshReadError::CallbackTimeout
                                        }
                                        CallbackFailure::BodyBound => {
                                            RefreshReadError::CallbackBodyBound
                                        }
                                        CallbackFailure::Transport => {
                                            RefreshReadError::CallbackRequest
                                        }
                                    });
                                }
                            }
                        } else {
                            let safe = retained_stderr_line(text);
                            let mut retained = diagnostic.lock().await;
                            let room = 16384usize.saturating_sub(retained.len());
                            let take = safe.len().min(room);
                            retained.extend_from_slice(&safe[..take]);
                            let delimiter_omitted = count != 0 && retained.len() == 16384;
                            if count != 0 && !delimiter_omitted {
                                retained.push(b'\n');
                            }
                            if take != safe.len() || delimiter_omitted {
                                reader_error.get_or_insert(RefreshReadError::RetentionBound);
                            }
                        }
                    }
                }
            }
            // Overflow never grows the assembly or stops pipe drainage.
            line.clear();
            overflow = false;
        }
        if count == 0 {
            break;
        }
    }
    if let Some(error) = reader_error {
        return Err(error);
    }
    Ok(authorizations)
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

// Only fixed classifications enter failure receipts. Authorization URLs remain
// private transport inputs and never enter retained diagnostics.
fn classification_line(text: &str) -> bool {
    let fields: Vec<_> = text.split(' ').collect();
    if fields.len() != 7 || fields[0] != "gateway-classification-v1" {
        return false;
    }
    let stages = [
        "NetworkStartup",
        "SourcePoolTimedOut",
        "SourceDnsTcpTimedOut",
        "SourceProxyTlsTimedOut",
        "SourceOriginTlsTimedOut",
        "SourceProxyRefused",
        "OAuthStartup",
        "OAuthCallbackPortFailed",
        "OAuthCallbackBindFailed",
        "OAuthCallbackBindDeadline",
        "OAuthCallbackAddressFailed",
        "OAuthAuthorizationUrlFailed",
        "OAuthRegistrationFailed",
        "OAuthRegistrationModelMissing",
        "OAuthRegistrationDecodeFailed",
        "OAuthRegistrationShapeFailed",
        "OAuthRegistrationClientFailed",
        "OAuthRegistrationEncodeFailed",
        "OAuthClientPersistenceFailed",
        "OAuthCallbackComplete",
        "OAuthCallbackStateMissing",
        "OAuthExchangeFailed",
        "OAuthExchangeComplete",
        "AuthAuthorizationRequired",
        "AuthAuthorizationFailed",
        "AuthTokenExchangeFailed",
        "AuthTokenRefreshFailed",
        "AuthTokenRefreshRejected",
        "AuthCredentialStore",
        "AuthHttp",
        "AuthOAuth",
        "AuthMetadata",
        "AuthPkceUnsupported",
        "AuthUrl",
        "AuthNoAuthorizationSupport",
        "AuthInternal",
        "AuthInvalidTokenType",
        "AuthTokenExpired",
        "AuthInvalidScope",
        "AuthRegistrationFailed",
        "AuthInsufficientScope",
        "AuthIssuerMismatch",
        "AuthIssuerMissing",
        "AuthClientCredentials",
        "AuthOther",
        "OAuthSourceClientSynced",
        "OAuthSourceClientSyncFailed",
        "OAuthCodeSend",
        "OAuthCodeSendFailed",
        "OAuthCodeStatus",
        "OAuthCodeBodyComplete",
        "OAuthCodeBodyFailed",
        "OAuthCodeBodyLimit",
        "AuthSourceOperation",
        "RemoteConstruction",
        "RemoteInitialization",
        "RemoteHttpResponse",
        "RemoteDiscovery",
        "Projection",
        "Stdio",
        "RemoteCleanup",
        "CallbackCleanup",
        "BrowserCleanup",
        "NetworkCleanup",
        "FileCleanup",
        "TokenRead",
        "TokenWrite",
    ];
    let kinds = [
        "none",
        "not_found",
        "permission_denied",
        "connection_refused",
        "connection_reset",
        "connection_aborted",
        "not_connected",
        "addr_in_use",
        "addr_not_available",
        "broken_pipe",
        "already_exists",
        "would_block",
        "invalid_input",
        "invalid_data",
        "timed_out",
        "write_zero",
        "interrupted",
        "unexpected_eof",
        "unsupported",
        "out_of_memory",
        "other",
    ];
    let optional_number = |field: &str, prefix: &str, status: bool| {
        let Some(value) = field.strip_prefix(prefix) else {
            return false;
        };
        value == "None"
            || value
                .strip_prefix("Some(")
                .and_then(|value| value.strip_suffix(')'))
                .and_then(|value| value.parse::<i32>().ok())
                .is_some_and(|value| !status || (100..=599).contains(&value))
    };
    fields[1]
        .strip_prefix("stage=")
        .is_some_and(|v| stages.contains(&v))
        && fields[2]
            .strip_prefix("io_kind=")
            .is_some_and(|v| kinds.contains(&v))
        && optional_number(fields[3], "os_code=", false)
        && fields[4]
            .strip_prefix("http_flags=")
            .and_then(|v| v.parse::<u16>().ok())
            .is_some_and(|v| v <= 255)
        && optional_number(fields[5], "http_status=", true)
        && matches!(fields[6], "chain_truncated=true" | "chain_truncated=false")
}
fn retained_stderr_line(text: &str) -> &[u8] {
    if text.is_empty()
        || classification_line(text)
        || text == "gateway connection or cleanup failed"
    {
        text.as_bytes()
    } else {
        b"[private stderr line omitted]"
    }
}
struct CausalStderr<R> {
    reader: BufReader<R>,
    retained: Vec<u8>,
    truncated: bool,
    disclosed: bool,
    authorization_seen: usize,
    authorization_prompt_seen: bool,
}
impl<R: tokio::io::AsyncRead + Unpin> CausalStderr<R> {
    fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            retained: Vec::new(),
            truncated: false,
            disclosed: false,
            authorization_seen: 0,
            authorization_prompt_seen: false,
        }
    }
    async fn drain(&mut self) -> std::io::Result<()> {
        while self.next_line().await?.is_some() {}
        Ok(())
    }
    async fn next_line(&mut self) -> std::io::Result<Option<String>> {
        use tokio::io::AsyncReadExt;
        let mut line = Vec::new();
        let mut overflow = false;
        loop {
            let mut byte = [0];
            let count = self.reader.read(&mut byte).await?;
            if count == 0 && line.is_empty() && !overflow {
                return Ok(None);
            }
            if count == 0 || byte[0] == b'\n' {
                break;
            }
            if line.len() < 16384 {
                line.push(byte[0]);
            } else {
                overflow = true;
            }
        }
        if overflow {
            self.truncated = true;
            return Ok(Some("[private stderr line exceeded cap]".into()));
        }
        let text = String::from_utf8(line).map_err(|_| std::io::ErrorKind::InvalidData)?;
        self.disclosed |= [
            "loopback-access",
            "loopback-refresh",
            "refresh201-rejected",
            "private-code",
            "static-sentinel",
        ]
        .iter()
        .any(|secret| text.contains(secret));
        self.authorization_prompt_seen |=
            text.trim_start() == "Authorize the SlopSearX gateway by visiting:";
        if text.trim_start().starts_with("http://") || text.trim_start().starts_with("https://") {
            self.authorization_seen = self.authorization_seen.saturating_add(1);
        }
        if !text.trim_start().starts_with("http://") && !text.trim_start().starts_with("https://") {
            let safe = retained_stderr_line(&text);
            let room = 16384usize.saturating_sub(self.retained.len());
            if safe.len().saturating_add(1) <= room {
                self.retained.extend_from_slice(safe);
                self.retained.push(b'\n');
            } else {
                self.truncated = true;
            }
        }
        Ok(Some(text))
    }
}
fn admit_oauth_directory(
    directory: &std::path::Path,
    _capture: &mut Option<AclPreflight>,
) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        // This is the exact production admission reader, not an ACL approximation.
        let ordinary = ssx_mcp::gateway_client::resolver::install::check_output_root(directory);
        if let Err(error) = &ordinary {
            *_capture = Some(AclPreflight {
                ordinary_admitted: false,
                io_kind: Some(error.kind()),
                os_code: error.raw_os_error(),
            });
            // Only the empty directory created immediately above is replaced.
            std::fs::remove_dir(directory)?;
            ssx_mcp::gateway_client::resolver::install::prepare_output_root(directory)?;
        } else {
            *_capture = Some(AclPreflight {
                ordinary_admitted: true,
                io_kind: None,
                os_code: None,
            });
        }
        ssx_mcp::gateway_client::resolver::install::check_output_root(directory)?;
    }
    let _ = directory;
    Ok(())
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum SetupFault {
    CallbackBind,
    DirectoryAfterCreate,
    SpawnMissing,
}
struct SetupOwner {
    fixture: Option<Fixture>,
    directory: Option<std::path::PathBuf>,
    callback: Option<tokio::net::TcpListener>,
    callback_port: Option<u16>,
    process: Option<Process>,
    transient_child: Option<Child>,
    acl: Option<AclPreflight>,
    failure: Option<ProcessFailure>,
}
impl SetupOwner {
    fn new() -> Self {
        Self {
            fixture: None,
            directory: None,
            callback: None,
            callback_port: None,
            process: None,
            transient_child: None,
            acl: None,
            failure: None,
        }
    }
    fn io_failure(&mut self, stage: &'static str, error: &std::io::Error) -> ssx_mcp::Error {
        self.failure = Some(ProcessFailure::io(stage, error));
        "private setup failed".into()
    }
    async fn retire(mut self, primary: std::thread::Result<ssx_mcp::Result<()>>) -> SetupReport {
        self.callback.take();
        let fixture_acquired = self.fixture.is_some();
        let directory_created = self.directory.is_some();
        let process_created = self.process.is_some() || self.transient_child.is_some();
        let mut process_retired = true;
        let mut observation = Observation {
            failure: self.failure,
            retired: None,
            stderr_retired: true,
            truncated: false,
            disclosed: false,
            observer_state: None,
            authorization_count: None,
            acl: self.acl,
            counts: None,
            retained: Vec::new(),
        };
        if let Some(mut process) = self.process.take() {
            process
                .report_failure(
                    self.failure
                        .unwrap_or_else(|| ProcessFailure::plain("setup_body")),
                    None,
                )
                .await;
            process_retired = process.retired_status.is_some() && process.stderr_retired;
            observation = process.observation(None);
        }
        if let Some(mut child) = self.transient_child.take() {
            let _ = child.start_kill();
            let waited = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
            process_retired &= matches!(waited, Ok(Ok(_)));
        }
        let fixture_retired = match self.fixture.take() {
            None => true,
            Some(fixture) => {
                fixture.app.private_callback_release.notify_waiters();
                fixture.stop.cancel();
                let mut task = fixture.task;
                match tokio::time::timeout(Duration::from_secs(5), &mut task).await {
                    Ok(Ok(Ok(()))) => true,
                    Ok(_) => false,
                    Err(_) => {
                        task.abort();
                        let _ = task.await;
                        false
                    }
                }
            }
        };
        let owned_path_retired = match self.directory {
            None => true,
            Some(path) if process_retired && fixture_retired => {
                match std::fs::symlink_metadata(&path) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
                    Ok(_) => {
                        std::fs::remove_dir_all(&path).is_ok()
                            && matches!(std::fs::symlink_metadata(&path), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
                    }
                    Err(_) => false,
                }
            }
            Some(_) => false,
        };
        SetupReport {
            primary,
            observation,
            process_retired,
            fixture_retired,
            owned_path_retired,
            fixture_acquired,
            directory_created,
            process_created,
            callback_port: self.callback_port,
        }
    }
}
struct SetupReport {
    primary: std::thread::Result<ssx_mcp::Result<()>>,
    observation: Observation,
    process_retired: bool,
    fixture_retired: bool,
    owned_path_retired: bool,
    fixture_acquired: bool,
    directory_created: bool,
    process_created: bool,
    callback_port: Option<u16>,
}
impl SetupReport {
    fn resume(self) -> ! {
        let _ = self.observation.emit(
            &mut std::io::stderr().lock(),
            self.process_retired,
            self.fixture_retired,
            self.owned_path_retired,
        );
        match self.primary {
            Err(primary) => std::panic::resume_unwind(primary),
            Ok(_) => panic!("private setup failed after owned retirement"),
        }
    }
}
// Fixture::start_status_profile spawns its sole task only after all fallible
// listener/router setup; returning Self then storing Some contains no fallible
// step. Before a successful spawn there is no background task to retire.
async fn prepare_oauth_case(
    label: &'static str,
    reject_refresh: bool,
    registration: StatusCode,
    token: StatusCode,
    refresh: StatusCode,
    fault: Option<SetupFault>,
) -> Result<(Process, Fixture, std::path::PathBuf, std::path::PathBuf), Box<SetupReport>> {
    let mut owner = SetupOwner::new();
    let primary = catch_process_case(async {
        owner.fixture = Some(
            Fixture::start_status_profile(
                true,
                false,
                reject_refresh,
                registration,
                token,
                refresh,
            )
            .await,
        );
        owner.callback = Some(
            tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(|error| owner.io_failure("setup_callback_bind", &error))?,
        );
        let address = owner
            .callback
            .as_ref()
            .unwrap()
            .local_addr()
            .map_err(|error| owner.io_failure("setup_callback_address", &error))?;
        owner.callback_port = Some(address.port());
        if fault == Some(SetupFault::CallbackBind) {
            match tokio::net::TcpListener::bind(address).await {
                Err(error) => return Err(owner.io_failure("setup_callback_bind", &error)),
                Ok(extra) => {
                    drop(extra);
                    return Err("private bind fault unexpectedly succeeded".into());
                }
            }
        }
        owner.callback.take();
        let directory = std::env::temp_dir().join(format!(
            "ssx-t07-{label}-{}-{}",
            std::process::id(),
            address.port()
        ));
        std::fs::create_dir(&directory)
            .map_err(|error| owner.io_failure("setup_directory_create", &error))?;
        // Record ownership immediately after successful creation, before ACL work.
        owner.directory = Some(directory.clone());
        if fault == Some(SetupFault::DirectoryAfterCreate) {
            return match std::fs::File::open(directory.join("__missing_admission_control")) {
                Err(error) => Err(owner.io_failure("setup_directory_after_create", &error)),
                Ok(_) => Err("private directory fault unexpectedly succeeded".into()),
            };
        }
        admit_oauth_directory(&directory, &mut owner.acl)
            .map_err(|error| owner.io_failure("setup_directory_admission", &error))?;
        if fault == Some(SetupFault::SpawnMissing) {
            return match tokio::process::Command::new(directory.join("__missing_process_control"))
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .spawn()
            {
                Err(error) => Err(owner.io_failure("setup_spawn", &error)),
                Ok(child) => {
                    owner.transient_child = Some(child);
                    Err("private spawn fault unexpectedly succeeded".into())
                }
            };
        }
        let path = directory.join("tokens.json");
        owner.process = Some(Process::spawn(
            owner.fixture.as_ref().unwrap(),
            &[
                "--oauth".into(),
                "--oauth-no-browser".into(),
                "--oauth-callback-port".into(),
                address.port().to_string(),
                "--oauth-token-file".into(),
                path.to_string_lossy().into_owned(),
            ],
        ));
        owner.process.as_mut().unwrap().acl_preflight = owner.acl;
        Ok::<(), ssx_mcp::Error>(())
    })
    .await;
    if matches!(&primary, Ok(Ok(())))
        && owner.fixture.is_some()
        && owner.process.is_some()
        && owner.directory.is_some()
    {
        // Synchronous checked handoff: no await, I/O, assertion or fallible step.
        let process = owner.process.take().unwrap();
        let fixture = owner.fixture.take().unwrap();
        let directory = owner.directory.take().unwrap();
        let path = directory.join("tokens.json");
        return Ok((process, fixture, directory, path));
    }
    Err(Box::new(owner.retire(primary).await))
}
async fn prepare_basic_case() -> Result<(Process, Fixture), Box<SetupReport>> {
    let mut owner = SetupOwner::new();
    let primary = catch_process_case(async {
        owner.fixture = Some(Fixture::start(false).await);
        let fixture = owner.fixture.as_ref().unwrap();
        *fixture.app.authorization.lock().await = "Basic dXNlcjpwYXNz".into();
        let remote = fixture.app.base.replacen("http://", "http://user:pass@", 1)
            + "/mcp-redirect#private-fragment";
        owner.process = Some(Process::spawn_url(
            &remote,
            &["--token".into(), "static-sentinel".into()],
        ));
        Ok::<(), ssx_mcp::Error>(())
    })
    .await;
    if matches!(&primary, Ok(Ok(()))) && owner.fixture.is_some() && owner.process.is_some() {
        let process = owner.process.take().unwrap();
        let fixture = owner.fixture.take().unwrap();
        return Ok((process, fixture));
    }
    Err(Box::new(owner.retire(primary).await))
}

async fn authorization_url(
    process: &mut Process,
    lines: &mut CausalStderr<tokio::process::ChildStderr>,
) -> String {
    let deadline = tokio::time::Instant::now() + process.rpc_budget;
    loop {
        let failure = match tokio::time::timeout_at(deadline, lines.next_line()).await {
            Ok(Ok(Some(line))) if line.trim_start().starts_with("http://") => {
                return line.trim().to_owned();
            }
            Ok(Ok(Some(_))) => continue,
            Ok(Ok(None)) => ProcessFailure::plain("authorization_eof"),
            Ok(Err(error)) => ProcessFailure::io("authorization_read", &error),
            Err(_) => ProcessFailure::plain("authorization_timeout"),
        };
        process.report_failure(failure, Some(lines)).await;
        panic!("private authorization failed after owned retirement");
    }
}
#[tokio::test]
async fn private_causal_stderr_accepts_only_finite_fields_and_drains_secret_overflow() {
    use tokio::io::AsyncWriteExt;
    let good = "gateway-classification-v1 stage=TokenWrite io_kind=permission_denied os_code=Some(5) http_flags=0 http_status=None chain_truncated=false";
    assert!(classification_line(good));
    for stage in [
        "OAuthCallbackPortFailed",
        "OAuthCallbackBindFailed",
        "OAuthCallbackBindDeadline",
        "OAuthCallbackAddressFailed",
        "OAuthAuthorizationUrlFailed",
        "OAuthRegistrationFailed",
        "OAuthRegistrationModelMissing",
        "OAuthRegistrationDecodeFailed",
        "OAuthRegistrationShapeFailed",
        "OAuthRegistrationClientFailed",
        "OAuthRegistrationEncodeFailed",
        "OAuthClientPersistenceFailed",
        "OAuthCallbackComplete",
        "OAuthCallbackStateMissing",
        "OAuthExchangeFailed",
        "OAuthExchangeComplete",
        "AuthAuthorizationRequired",
        "AuthAuthorizationFailed",
        "AuthTokenExchangeFailed",
        "AuthTokenRefreshFailed",
        "AuthTokenRefreshRejected",
        "AuthCredentialStore",
        "AuthHttp",
        "AuthOAuth",
        "AuthMetadata",
        "AuthPkceUnsupported",
        "AuthUrl",
        "AuthNoAuthorizationSupport",
        "AuthInternal",
        "AuthInvalidTokenType",
        "AuthTokenExpired",
        "AuthInvalidScope",
        "AuthRegistrationFailed",
        "AuthInsufficientScope",
        "AuthIssuerMismatch",
        "AuthIssuerMissing",
        "AuthClientCredentials",
        "AuthOther",
        "OAuthSourceClientSynced",
        "OAuthSourceClientSyncFailed",
        "OAuthCodeSend",
        "OAuthCodeSendFailed",
        "OAuthCodeStatus",
        "OAuthCodeBodyComplete",
        "OAuthCodeBodyFailed",
        "OAuthCodeBodyLimit",
        "AuthSourceOperation",
    ] {
        assert!(classification_line(&good.replace("TokenWrite", stage)));
    }
    for bad in [
        good.replace(
            "TokenWrite",
            "https://user:secret@invalid/?code=private-code",
        ),
        good.replace("permission_denied", "private-secret"),
        good.replace("Some(5)", "Some(private-secret)"),
        format!("{good} private-secret"),
    ] {
        assert!(!classification_line(&bad));
    }
    let (mut writer, reader) = tokio::io::duplex(1024);
    let expected = good.to_owned();
    let task = tokio::spawn(async move {
        writer.write_all(&vec![b'x'; 65536]).await.unwrap();
        writer
            .write_all(b"\nhttps://user:private-secret@invalid/?code=private-code\n")
            .await
            .unwrap();
        writer.write_all(b"gateway-classification-v1 stage=Stdio io_kind=private-secret os_code=None http_flags=0 http_status=None chain_truncated=false\n").await.unwrap();
        writer.write_all(expected.as_bytes()).await.unwrap();
        writer.write_all(b"\n").await.unwrap();
        writer.shutdown().await.unwrap();
    });
    let mut lines = CausalStderr::new(reader);
    tokio::time::timeout(Duration::from_secs(5), async {
        while lines.next_line().await.unwrap().is_some() {}
        task.await.unwrap();
    })
    .await
    .unwrap();
    assert!(lines.truncated && lines.disclosed);
    assert_eq!(
        lines.retained,
        format!("[private stderr line omitted]\n{good}\n").as_bytes()
    );
    assert!(!String::from_utf8_lossy(&lines.retained).contains("private-secret"));
    assert!(lines.retained.len() <= 16384);
    let (mut writer, reader) = tokio::io::duplex(1024);
    let expected = good.to_owned();
    let writer = tokio::spawn(async move {
        writer
            .write_all(
                b"unlisted_credential=private-unlisted token_path=/Users/private/credential.json\n",
            )
            .await
            .unwrap();
        writer.write_all(expected.as_bytes()).await.unwrap();
        writer.write_all(b"\n").await.unwrap();
        writer.shutdown().await.unwrap();
    });
    let captured = Arc::new(Mutex::new(Vec::new()));
    let actual = tokio::time::timeout(
        Duration::from_secs(5),
        read_refresh_stderr(reader, reqwest::Client::new(), captured.clone()),
    )
    .await
    .unwrap();
    writer.await.unwrap();
    assert_eq!(actual, Ok(0));
    assert_eq!(
        captured.lock().await.as_slice(),
        format!("[private stderr line omitted]\n{good}\n").as_bytes()
    );
    struct SecretReadError;
    impl std::fmt::Debug for SecretReadError {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("IO Debug must never run: private-unlisted /Users/private/credential.json");
        }
    }
    impl std::fmt::Display for SecretReadError {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("IO Display must never run: private-unlisted /Users/private/credential.json");
        }
    }
    impl std::error::Error for SecretReadError {}
    struct FailingRead;
    impl tokio::io::AsyncRead for FailingRead {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Err(std::io::Error::other(SecretReadError)))
        }
    }
    let failed = read_refresh_stderr(
        FailingRead,
        reqwest::Client::new(),
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    assert_eq!(failed, Err(RefreshReadError::Read));
    let (state, count) = refresh_agent_fields(&failed);
    assert_eq!(
        format!("agent_state={state} authorization_count={count:?}"),
        "agent_state=read authorization_count=None"
    );
    for (fault, stage, directory_created, kind) in [
        (
            SetupFault::CallbackBind,
            "setup_callback_bind",
            false,
            std::io::ErrorKind::AddrInUse,
        ),
        (
            SetupFault::DirectoryAfterCreate,
            "setup_directory_after_create",
            true,
            std::io::ErrorKind::NotFound,
        ),
        (
            SetupFault::SpawnMissing,
            "setup_spawn",
            true,
            std::io::ErrorKind::NotFound,
        ),
    ] {
        let result = prepare_oauth_case(
            "setup-control",
            false,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::BAD_REQUEST,
            Some(fault),
        )
        .await;
        let report = match result {
            Err(report) => report,
            Ok((mut process, fixture, directory, _)) => {
                let receipt = finish_case(
                    &mut process,
                    fixture,
                    Some(directory),
                    false,
                    None,
                    Ok(Err("setup control unexpectedly succeeded".into())),
                )
                .await;
                receipt.resume();
                panic!("setup control did not reject");
            }
        };
        assert!(report.fixture_acquired && !report.process_created);
        assert_eq!(report.directory_created, directory_created);
        assert!(report.process_retired && report.fixture_retired && report.owned_path_retired);
        assert!(matches!(&report.primary, Ok(Err(_))));
        let failure = report.observation.failure.unwrap();
        assert_eq!(failure.stage, stage);
        assert_eq!(failure.io_kind, Some(kind));
        let port = report.callback_port.unwrap();
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        drop(listener); // Exact reserved callback socket is physically released.
        let propagated =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> () { report.resume() }));
        assert!(propagated.is_err());
    }
}

#[cfg(unix)]
async fn causal_process(script: &'static str) -> Process {
    use tokio::io::AsyncReadExt;
    let mut child = tokio::process::Command::new("/bin/sh")
        .args(["-c", script])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut ready = [0; 6];
    let readiness = tokio::time::timeout(
        Duration::from_secs(5),
        child.stderr.as_mut().unwrap().read_exact(&mut ready),
    )
    .await;
    if !matches!(readiness, Ok(Ok(_))) || ready != *b"ready\n" {
        let _ = child.start_kill();
        let retired = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        assert!(matches!(retired, Ok(Ok(_))), "fake child retirement failed");
        panic!("fake child did not enter exact readiness boundary");
    }
    let input = child.stdin.take();
    let output = BufReader::new(child.stdout.take().unwrap()).lines();
    let observed_stderr = Some(CausalStderr::new(child.stderr.take().unwrap()));
    Process {
        child,
        input,
        output,
        observed_stderr,
        rpc_budget: Duration::from_millis(50),
        last_failure: None,
        retired_status: None,
        stderr_retired: false,
        acl_preflight: None,
        observer_state: None,
        authorization_count: None,
        retained_stderr: Vec::new(),
        stderr_truncated: false,
        stderr_disclosed: false,
        authorization_seen: 0,
        authorization_prompt_seen: false,
    }
}
#[cfg(unix)]
#[tokio::test]
async fn private_process_io_authorization_and_close_failures_retire_actual_child_and_bounded_stderr()
 {
    for (script, stage) in [
        (
            "exec 0<&-; printf 'ready\\n' >&2; exec /bin/sleep 60",
            "write",
        ),
        (
            "exec 1>&-; printf 'ready\\n' >&2; exec /bin/sleep 60",
            "early_eof",
        ),
        ("printf 'ready\\n' >&2; exec /bin/sleep 60", "read_timeout"),
    ] {
        let mut process = causal_process(script).await;
        let result = catch_process_case(process.rpc(json!({"probe":"private-unlisted"}))).await;
        assert!(result.is_err());
        assert_eq!(process.last_failure.unwrap().stage, stage);
        assert!(process.retired_status.is_some() && process.stderr_retired);
        assert!(matches!(process.child.try_wait(), Ok(Some(_))));
    }
    let mut process = causal_process("printf 'ready\\n' >&2; exec /bin/sleep 60").await;
    let mut lines = process.observed_stderr.take().unwrap();
    let result = catch_process_case(authorization_url(&mut process, &mut lines)).await;
    assert!(result.is_err());
    assert_eq!(process.last_failure.unwrap().stage, "authorization_timeout");
    assert!(process.retired_status.is_some() && process.stderr_retired);
    let mut process = causal_process(r#"printf 'ready\n' >&2; i=0; while [ "$i" -lt 2000 ]; do printf 'unlisted_credential=private-unlisted token_path=/Users/private/credential.json\n' >&2; i=$((i+1)); done"#).await;
    process.rpc_budget = Duration::from_secs(5);
    let result = catch_process_case(process.close()).await;
    assert!(result.is_err());
    assert_eq!(process.last_failure.unwrap().stage, "stderr_policy");
    assert!(
        process
            .retired_status
            .is_some_and(|status| status.success())
            && process.stderr_retired
    );
    assert!(process.stderr_truncated && process.retained_stderr.len() <= 16384);
    let retained = String::from_utf8_lossy(&process.retained_stderr);
    assert!(!retained.contains("private-unlisted") && !retained.contains("/Users/private"));
    // This ordinary Process-only scope has actually joined its child/reader;
    // the receipt sink is exercised only after those retirement assertions.
    struct SinkError;
    impl std::fmt::Debug for SinkError {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("receipt Error Debug forbidden: sink-secret /private/token-path");
        }
    }
    impl std::fmt::Display for SinkError {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("receipt Error Display forbidden: sink-secret /private/token-path");
        }
    }
    impl std::error::Error for SinkError {}
    struct FailingSink;
    impl std::io::Write for FailingSink {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other(SinkError))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other(SinkError))
        }
    }
    let observation = process.observation(None);
    let emitted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        observation.emit(&mut FailingSink, true, true, true)
    }));
    assert!(matches!(emitted, Ok(false)));
    for (endpoint, expected) in [
        ("private-callback-error", "callback_status"),
        ("private-callback-hold", "callback_timeout"),
    ] {
        use std::os::unix::fs::DirBuilderExt;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let fixture = Fixture::start(false).await;
        let app = fixture.app.clone();
        let url = format!("{}/{endpoint}", fixture.app.base);
        let root =
            std::path::PathBuf::from(std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT").unwrap());
        let directory = root.join(format!("causal-{endpoint}-{}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let mut process = causal_process("printf 'ready\\n' >&2; exec /bin/sleep 60").await;
        let report = owned_case!(process, fixture, Some(directory.clone()), false, None, {
            process
                .callback(&client, &url, Duration::from_millis(500))
                .await?;
            Ok::<(), ssx_mcp::Error>(())
        });
        assert!(matches!(report.primary, Ok(Err(_))));
        assert_eq!(report.stage, Some(expected));
        assert!(report.process_retired && report.fixture_retired && report.owned_path_retired);
        assert_eq!(app.state.lock().await.private_callback_visits, 1);
        assert!(!directory.exists());
        let mut retained_receipt = Vec::new();
        assert!(report.observation.emit(
            &mut retained_receipt,
            report.process_retired,
            report.fixture_retired,
            report.owned_path_retired
        ));
        assert!(retained_receipt.len() < 32768);
        assert!(!String::from_utf8_lossy(&retained_receipt).contains("/Users/private"));
    }
    let fixture = Fixture::start(false).await;
    let mut process = causal_process("printf 'ready\\n' >&2; exec /bin/sleep 60").await;
    let report = owned_case!(process, fixture, None, false, None, {
        assert!(std::hint::black_box(false), "owned-primary-probe");
        Ok::<(), ssx_mcp::Error>(())
    });
    assert!(report.process_retired && report.fixture_retired && report.owned_path_retired);
    assert!(report.primary.is_err());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProcessFailure {
    stage: &'static str,
    io_kind: Option<std::io::ErrorKind>,
    os_code: Option<i32>,
}
impl ProcessFailure {
    fn plain(stage: &'static str) -> Self {
        Self {
            stage,
            io_kind: None,
            os_code: None,
        }
    }
    fn io(stage: &'static str, error: &std::io::Error) -> Self {
        Self {
            stage,
            io_kind: Some(error.kind()),
            os_code: error.raw_os_error(),
        }
    }
}
struct Process {
    child: Child,
    rpc_budget: Duration,
    last_failure: Option<ProcessFailure>,
    retired_status: Option<std::process::ExitStatus>,
    stderr_retired: bool,
    acl_preflight: Option<AclPreflight>,
    observer_state: Option<&'static str>,
    authorization_count: Option<usize>,
    retained_stderr: Vec<u8>,
    stderr_truncated: bool,
    stderr_disclosed: bool,
    authorization_seen: usize,
    authorization_prompt_seen: bool,
    observed_stderr: Option<CausalStderr<tokio::process::ChildStderr>>,
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
        let images = production::optional();
        let installed = ssx_mcp::gateway_client::resolver::install::stage_cargo_gateway(
            &images.gateway,
            &images.dns,
            #[cfg(windows)]
            &images.input,
        )
        .expect("preregistered native gateway package fixture");
        let mut command = tokio::process::Command::new(installed);
        command
            .env_clear()
            .env("no_proxy", "*")
            .env("MCP_REMOTE_DIAGNOSTICS", "classification-v1")
            .arg("--remote")
            .arg(remote)
            .args(extra)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        // Windows executable verification and process creation require the OS
        // directory even in this otherwise empty fixture environment.
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
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
            observed_stderr: None,
            rpc_budget: Duration::from_secs(20),
            last_failure: None,
            retired_status: None,
            stderr_retired: false,
            acl_preflight: None,
            observer_state: None,
            authorization_count: None,
            retained_stderr: Vec::new(),
            stderr_truncated: false,
            stderr_disclosed: false,
            authorization_seen: 0,
            authorization_prompt_seen: false,
            input,
            output,
        }
    }
    async fn callback(
        &mut self,
        client: &reqwest::Client,
        url: &str,
        budget: Duration,
    ) -> Result<(), &'static str> {
        bounded_callback(client, url, budget)
            .await
            .map_err(|failure| {
                self.last_failure = Some(ProcessFailure::plain(failure.stage()));
                "private callback failed"
            })
    }
    async fn rpc(&mut self, value: Value) -> Value {
        let result: Result<Value, ProcessFailure> = async {
            let input = self
                .input
                .as_mut()
                .ok_or_else(|| ProcessFailure::plain("input_missing"))?;
            tokio::time::timeout(
                self.rpc_budget,
                input.write_all(format!("{value}\n").as_bytes()),
            )
            .await
            .map_err(|_| ProcessFailure::plain("write_timeout"))?
            .map_err(|error| ProcessFailure::io("write", &error))?;
            tokio::time::timeout(self.rpc_budget, input.flush())
                .await
                .map_err(|_| ProcessFailure::plain("flush_timeout"))?
                .map_err(|error| ProcessFailure::io("flush", &error))?;
            let line = tokio::time::timeout(self.rpc_budget, self.output.next_line())
                .await
                .map_err(|_| ProcessFailure::plain("read_timeout"))?
                .map_err(|error| ProcessFailure::io("read", &error))?
                .ok_or_else(|| ProcessFailure::plain("early_eof"))?;
            serde_json::from_str(&line).map_err(|_| ProcessFailure::plain("reply_json"))
        }
        .await;
        match result {
            Ok(value) => value,
            Err(failure) => {
                self.report_failure(failure, None).await;
                panic!("private gateway RPC failed after owned retirement");
            }
        }
    }
    fn observation(&self, counts: Option<(usize, usize, usize, usize, usize)>) -> Observation {
        Observation {
            failure: self.last_failure,
            retired: self.retired_status,
            stderr_retired: self.stderr_retired,
            truncated: self.stderr_truncated,
            disclosed: self.stderr_disclosed,
            observer_state: self.observer_state,
            authorization_count: self.authorization_count,
            acl: self.acl_preflight,
            counts,
            retained: self.retained_stderr.clone(),
        }
    }
    fn capture_stderr(&mut self, observed: &CausalStderr<tokio::process::ChildStderr>) {
        self.retained_stderr.clone_from(&observed.retained);
        self.stderr_truncated = observed.truncated;
        self.stderr_disclosed = observed.disclosed;
        self.authorization_seen = observed.authorization_seen;
        self.authorization_prompt_seen = observed.authorization_prompt_seen;
    }
    async fn finish_owned(&mut self, expected_success: bool) {
        self.input.take();
        let Some(mut observed) = self
            .observed_stderr
            .take()
            .or_else(|| self.child.stderr.take().map(CausalStderr::new))
        else {
            self.report_failure(ProcessFailure::plain("stderr_owner_missing"), None)
                .await;
            panic!("private stderr owner absent after owned retirement");
        };
        let result = tokio::time::timeout(self.rpc_budget, async {
            tokio::join!(self.child.wait(), observed.drain())
        })
        .await;
        let failure = match result {
            Ok((Ok(status), Ok(()))) => {
                self.retired_status = Some(status);
                self.stderr_retired = true;
                self.capture_stderr(&observed);
                if status.success() != expected_success {
                    ProcessFailure::plain("exit_status")
                } else if observed.truncated || observed.disclosed {
                    ProcessFailure::plain("stderr_policy")
                } else {
                    return;
                }
            }
            Ok((Err(error), _)) => ProcessFailure::io("wait", &error),
            Ok((_, Err(error))) => ProcessFailure::io("stderr_read", &error),
            Err(_) => ProcessFailure::plain("close_timeout"),
        };
        self.report_failure(failure, Some(&mut observed)).await;
        panic!("private close failed after owned retirement");
    }
    async fn report_failure(
        &mut self,
        failure: ProcessFailure,
        external: Option<&mut CausalStderr<tokio::process::ChildStderr>>,
    ) {
        self.last_failure = Some(failure);
        self.input.take();
        let initial = self.child.try_wait().ok().flatten();
        if initial.is_none() {
            let _ = self.child.start_kill();
        }
        self.retired_status = tokio::time::timeout(Duration::from_secs(5), self.child.wait())
            .await
            .ok()
            .and_then(Result::ok);
        let mut owned = self
            .observed_stderr
            .take()
            .or_else(|| self.child.stderr.take().map(CausalStderr::new));
        let observed = match external {
            Some(observed) => Some(observed),
            None => owned.as_mut(),
        };
        if let Some(observed) = observed {
            self.stderr_retired = matches!(
                tokio::time::timeout(Duration::from_secs(5), async {
                    while observed.next_line().await?.is_some() {}
                    Ok::<_, std::io::Error>(())
                })
                .await,
                Ok(Ok(()))
            );
            self.capture_stderr(observed);
        }
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
    async fn close(&mut self) {
        self.finish_owned(true).await;
    }
}

#[tokio::test]
async fn private_refresh_stderr_overflow_drains_newline_free_pipe_and_retires_writer() {
    for newline in [false, true] {
        let (mut writer, reader) = tokio::io::duplex(1024);
        let writing = tokio::spawn(async move {
            for _ in 0..20 {
                writer.write_all(&[b'x'; 1024]).await.unwrap();
            }
            if newline {
                writer.write_all(b"\nafter-overflow\n").await.unwrap();
            }
            writer.shutdown().await.unwrap();
        });
        let diagnostic = Arc::new(Mutex::new(Vec::new()));
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let reading = tokio::spawn(read_refresh_stderr(reader, client, diagnostic.clone()));
        let error = tokio::time::timeout(Duration::from_secs(5), reading)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert_eq!(error, RefreshReadError::LineBound);
        tokio::time::timeout(Duration::from_secs(5), writing)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            diagnostic.lock().await.as_slice(),
            if newline {
                b"[private stderr line omitted]\n".as_slice()
            } else {
                b""
            }
        );
    }
}

#[tokio::test]
async fn private_refresh_stderr_exact_cap_accounts_for_every_delimiter() {
    for extra_delimiter in [false, true] {
        let (mut writer, reader) = tokio::io::duplex(1024);
        let writing = tokio::spawn(async move {
            let finite = "gateway-classification-v1 stage=Stdio io_kind=none os_code=None http_flags=0 http_status=None chain_truncated=false\n";
            assert_eq!(finite.len(), 116);
            writer
                .write_all(finite.repeat(141).as_bytes())
                .await
                .unwrap();
            writer.write_all(&[b'\n'; 28]).await.unwrap();
            if extra_delimiter {
                writer.write_all(b"\n").await.unwrap();
            }
            writer.shutdown().await.unwrap();
        });
        let diagnostic = Arc::new(Mutex::new(Vec::new()));
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            read_refresh_stderr(reader, client, diagnostic.clone()),
        )
        .await
        .unwrap();
        if extra_delimiter {
            assert_eq!(result.unwrap_err(), RefreshReadError::RetentionBound);
        } else {
            assert_eq!(result.unwrap(), 0);
        }
        tokio::time::timeout(Duration::from_secs(5), writing)
            .await
            .unwrap()
            .unwrap();
        let retained = diagnostic.lock().await;
        assert_eq!(retained.len(), 16384);
        assert_eq!(retained.last(), Some(&b'\n'));
        assert!(retained[16356..].iter().all(|byte| *byte == b'\n'));
        assert!(retained.starts_with(b"gateway-classification-v1 stage=Stdio"));
    }
}

#[tokio::test]
async fn private_refresh_stderr_consumes_both_urls_and_redacts_credential_queries() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let visits = Arc::new(AtomicUsize::new(0));
    let counted = visits.clone();
    let app = axum::Router::new().route(
        "/authorize",
        axum::routing::get(move || {
            let visits = counted.clone();
            async move {
                visits.fetch_add(1, Ordering::SeqCst);
                "private callback complete"
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let stop = CancellationToken::new();
    let stopped = stop.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(stopped.cancelled_owned())
            .await
            .unwrap();
    });
    let url = format!("http://{address}/authorize?state=private-query-sentinel");
    let (mut writer, reader) = tokio::io::duplex(1024);
    let writing = tokio::spawn(async move {
        writer
            .write_all(format!("safe diagnostic\n  {url}\nembedded {url}\n  {url}\n").as_bytes())
            .await
            .unwrap();
        writer.shutdown().await.unwrap();
    });
    let diagnostic = Arc::new(Mutex::new(Vec::new()));
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        read_refresh_stderr(reader, client, diagnostic.clone()),
    )
    .await;
    let writer_result = tokio::time::timeout(Duration::from_secs(5), writing).await;
    stop.cancel();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.unwrap().unwrap(), 2);
    writer_result.unwrap().unwrap();
    assert_eq!(visits.load(Ordering::SeqCst), 2);
    assert_eq!(
        diagnostic.lock().await.as_slice(),
        b"[private stderr line omitted]\n[private stderr line omitted]\n"
    );
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
    let (mut process, fixture, directory, path) = prepare_oauth_case(
        "oauth",
        false,
        StatusCode::OK,
        StatusCode::OK,
        StatusCode::BAD_REQUEST,
        None,
    )
    .await
    .unwrap_or_else(|report| report.resume());
    owned_case!(process, fixture, Some(directory), true, None, {
        let stderr = process.child.stderr.take().unwrap();
        let mut lines = CausalStderr::new(stderr);
        let authorization = authorization_url(&mut process, &mut lines).await;
        process.observed_stderr = Some(lines);
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .map_err(|_| "private callback client failed")?;
        process
            .callback(&client, &authorization, Duration::from_secs(20))
            .await?;
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
        // The actual stderr reader drains concurrently with the original child wait.
        process.finish_owned(true).await;
        assert_eq!(fixture.app.state.lock().await.token_forms.len(), 1);
        assert!(fixture.app.state.lock().await.deletes > 0);
        Ok::<(), ssx_mcp::Error>(())
    })
    .resume();
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
    assert!(matches!(process.output.next_line().await, Ok(None)));
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
-> ssx_mcp::Result<()> {
    for refresh_status in [StatusCode::BAD_REQUEST, StatusCode::CREATED] {
        let (mut process, fixture, directory, path) = prepare_oauth_case(
            "refresh",
            true,
            StatusCode::OK,
            StatusCode::OK,
            refresh_status,
            None,
        )
        .await
        .unwrap_or_else(|report| report.resume());
        let diagnostic = Arc::new(Mutex::new(Vec::new()));
        let mut agent = None;
        owned_case!(
            process,
            fixture,
            Some(directory),
            true,
            Some(CaseAux {
                agent: &mut agent,
                diagnostic: diagnostic.clone()
            }),
            {
                let client = reqwest::Client::builder()
                    .no_proxy()
                    .build()
                    .map_err(|_| "private callback client failed")?;
                let stderr = process.child.stderr.take().unwrap();
                let captured = diagnostic.clone();
                agent = Some(tokio::spawn(read_refresh_stderr(stderr, client, captured)));
                let outcome = catch_process_case(async {
                    process.initialize().await;
                    process.input.take();
                    let status =
                        tokio::time::timeout(Duration::from_secs(20), process.child.wait())
                            .await??;
                    assert!(status.success());
                    let observed = fixture.app.state.lock().await;
                    assert_eq!(observed.token_forms.len(), 3);
                    assert_eq!(observed.registrations, 1);
                    assert_eq!(observed.token_forms[0]["grant_type"], "authorization_code");
                    assert_eq!(observed.token_forms[1]["grant_type"], "refresh_token");
                    assert_eq!(observed.token_forms[2]["grant_type"], "authorization_code");
                    drop(observed);
                    let tokens: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
                    assert_eq!(tokens["tokens"]["expires_in"], 3600);
                    assert_eq!(tokens["tokens"]["access_token"], "loopback-access");
                    Ok::<(), ssx_mcp::Error>(())
                })
                .await;
                // Retain the real child and reader even if the oracle panics on EOF.
                // This keeps status/fixture diagnostics and retires every owned task.
                let initial_exit = process.child.try_wait()?;
                if initial_exit.is_none() {
                    process.child.start_kill()?;
                }
                let retired_exit =
                    tokio::time::timeout(Duration::from_secs(5), process.child.wait()).await??;
                process.input.take();
                let agent_result: Result<usize, RefreshReadError> =
                    match tokio::time::timeout(Duration::from_secs(5), agent.as_mut().unwrap())
                        .await
                    {
                        Ok(joined) => {
                            agent.take();
                            joined
                                .map_err(|_| RefreshReadError::Join)
                                .and_then(|value| value)
                        }
                        Err(_) => {
                            if let Some(owned) = agent.take() {
                                owned.abort();
                                let _ = owned.await;
                            }
                            Err(RefreshReadError::Retirement)
                        }
                    };
                process.retired_status = Some(retired_exit);
                process.stderr_retired = true;
                process
                    .retained_stderr
                    .clone_from(&*diagnostic.lock().await);
                let (agent_state, authorization_count) = refresh_agent_fields(&agent_result);
                process.observer_state = Some(agent_state);
                process.authorization_count = authorization_count;
                finish_process_case(outcome)?;
                assert_eq!(
                    agent_result.map_err(|_| "private authorization observer failed")?,
                    2
                );
                Ok::<(), ssx_mcp::Error>(())
            }
        )
        .resume();
    }
    Ok(())
}

#[tokio::test]
async fn actual_native_static_userinfo_basic_precedes_bearer_and_fragment_never_reaches_http() {
    let (mut process, fixture) = prepare_basic_case()
        .await
        .unwrap_or_else(|report| report.resume());
    owned_case!(process, fixture, None, true, None, {
        process.initialize().await;
        process.close().await;
        assert!(fixture.app.state.lock().await.deletes > 0);
        Ok::<(), ssx_mcp::Error>(())
    })
    .resume();
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
        let (mut process, fixture, directory, path) = prepare_oauth_case(
            "status",
            false,
            registration_status,
            token_status,
            StatusCode::BAD_REQUEST,
            None,
        )
        .await
        .unwrap_or_else(|report| report.resume());
        owned_case!(process, fixture, Some(directory), accepted, None, {
            let stderr = process.child.stderr.take().unwrap();
            process.observed_stderr = Some(CausalStderr::new(stderr));
            if registration_status != StatusCode::ACCEPTED {
                let mut lines = process.observed_stderr.take().unwrap();
                let authorization = authorization_url(&mut process, &mut lines).await;
                process.observed_stderr = Some(lines);
                let client = reqwest::Client::builder()
                    .no_proxy()
                    .build()
                    .map_err(|_| "private callback client failed")?;
                process
                    .callback(&client, &authorization, Duration::from_secs(20))
                    .await?;
            }
            if accepted {
                process.initialize().await;
            }
            process.finish_owned(accepted).await;
            if accepted {
                let stored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                assert_eq!(stored["tokens"]["access_token"], "loopback-access");
            } else {
                assert!(matches!(process.output.next_line().await, Ok(None)));
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
                assert_eq!(process.authorization_seen, 0);
                assert!(!process.authorization_prompt_seen);
                assert!(!path.exists());
                assert!(fixture.app.state.lock().await.token_forms.is_empty());
            }
            Ok::<(), ssx_mcp::Error>(())
        })
        .resume();
    }
}

// Current-main integration: these are the accepted native SearchSurface and
// disposable Valkey fixtures, never a broad synthetic echo catalog.
#[cfg(feature = "server-test-fixtures")]
#[path = "../../ssx-mcp/tests/t06_support/fixture.rs"]
mod current_t06_fixture;
#[cfg(feature = "server-test-fixtures")]
#[path = "../../ssx-mcp/tests/t06_support/server.rs"]
mod current_t06_valkey;
#[cfg(feature = "server-test-fixtures")]
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
        // The same authenticated native surface must produce identical resource
        // bodies through direct HTTP and the actual installed stdio gateway.
        // These passive resources contain no observation-time freshness field.
        for (id, uri) in [(20, "slopsearx://capabilities"), (21, "slopsearx://routing-profiles"), (22, "slopsearx://capabilities/wikipedia")] {
            let request = json!({"jsonrpc":"2.0","id":id,"method":"resources/read","params":{"uri":uri}});
            let original = direct(&client, &url, &request).await?;
            let proxied = gateway.rpc(request).await;
            assert!(original.get("error").is_none(), "direct resource failed: {uri}");
            assert_eq!(original["result"]["contents"].as_array().unwrap().len(), 1);
            assert!(!original["result"]["contents"][0]["text"].as_str().unwrap().is_empty());
            assert_eq!(proxied, original, "complete direct/gateway resource envelope: {uri}");
        }
        let prompt_request = json!({"jsonrpc":"2.0","id":30,"method":"prompts/get","params":{"name":"research_with_source_coverage","arguments":{"question":"paired-native-caller-data"}}});
        let original_prompt = direct(&client, &url, &prompt_request).await?;
        let proxied_prompt = gateway.rpc(prompt_request).await;
        assert!(original_prompt.get("error").is_none());
        assert_eq!(original_prompt["result"]["description"], "Search broadly, inspect capability coverage, return diverse evidence.");
        let messages = original_prompt["result"]["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"]["type"], "text");
        assert!(messages[0]["content"]["text"].as_str().unwrap().contains("paired-native-caller-data"));
        // Python gateway._extract_prompt_text returns text to the local prompt
        // wrapper. The source contract deliberately omits remote description;
        // compare the complete resulting envelope, without invoking production
        // projection code to manufacture this test's expected value.
        assert_eq!(proxied_prompt, json!({"jsonrpc":"2.0","id":30,"result":{"messages":messages}}));
        let before_error = fixture.calls();
        let error_request = json!({"jsonrpc":"2.0","id":40,"method":"tools/call","params":{"name":"slopsearx_search","arguments":{"query":""}}});
        let original_error = direct(&client, &url, &error_request).await?;
        let proxied_error = gateway.rpc(error_request).await;
        // Empty query is a real predispatch domain error, carried in the normal
        // structured result on both paths. Preserve its complete payload and
        // distinguish it from both protocol errors and framework isError=true.
        for response in [&original_error, &proxied_error] {
            assert!(response.get("error").is_none());
            assert_eq!(response["result"]["isError"], false);
            assert_eq!(response["result"]["structuredContent"]["error"]["code"], "invalid_input");
            assert_eq!(response["result"]["structuredContent"]["error"]["field"], "query");
        }
        assert_eq!(proxied_error["result"]["structuredContent"], original_error["result"]["structuredContent"], "complete direct/gateway domain-error payload");
        assert_eq!(fixture.calls(), before_error, "paired domain denial dispatched an engine");
        let rejected_request = json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"slopsearx_search","arguments":{}}});
        let original_rejected = direct(&client, &url, &rejected_request).await?;
        let rejected = gateway.rpc(rejected_request).await;
        for response in [&original_rejected, &rejected] {
            assert_eq!(response["result"]["isError"], true, "framework rejection became a normal domain result");
            assert!(response.get("error").is_none(), "framework rejection became a JSON-RPC error");
            assert!(response["result"].get("structuredContent").is_none(), "framework rejection acquired a structured payload");
        }
        // Exact independently frozen source signatures: the T06 server's
        // parameter is positional while the gateway proxy's is keyword-only.
        // Their documented diagnostic-text difference grants no permission to
        // normalize other fields, error classes, channels or arbitrary text.
        assert_eq!(original_rejected, json!({"jsonrpc":"2.0","id":5,"result":{"content":[{"type":"text","text":"1 validation error for call[slopsearx_search]\nquery\n  Missing required argument [type=missing_argument, input_value={}, input_type=dict]\n    For further information visit https://errors.pydantic.dev/2.13/v/missing_argument"}],"isError":true}}));
        assert_eq!(rejected, json!({"jsonrpc":"2.0","id":5,"result":{"content":[{"type":"text","text":"1 validation error for call[slopsearx_search]\nquery\n  Missing required keyword only argument [type=missing_keyword_only_argument, input_value={}, input_type=dict]\n    For further information visit https://errors.pydantic.dev/2.13/v/missing_keyword_only_argument"}],"isError":true}}));
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
                assert_eq!(fixture.app.state.lock().await.token_redirect_visits, 1);
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
            let process_failure = process.last_failure;
            let mut stdout = String::new();
            let mut output = process.output.into_inner();
            let stdout_result =
                tokio::time::timeout(Duration::from_secs(5), output.read_to_string(&mut stdout))
                    .await;
            let browser_record = std::fs::read_to_string(&record).ok();
            let observed = fixture.app.state.lock().await;
            let token_redirect_visits = observed.token_redirect_visits;
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
                    "T07_PRIVATE_CASE_FAILURE cancel_status={cancel_status:?} stop_mode={stop_mode} stage={} process_failure={process_failure:?} token_redirect_visits={token_redirect_visits} fixture_counts={fixture_counts:?} stderr={stderr_result:?} browser_record={browser_record:?} stdout={stdout:?}",
                    stage.get()
                );
            }
            finish_process_case(outcome)?;
            let stderr = stderr_result?;
            stdout_result??;
            // After actual child/readers/fixture/path retirement, prove that the
            // successful first OAuth flow exercised the finite production wiring.
            for stage in [
                "OAuthCallbackComplete",
                "OAuthSourceClientSynced",
                "OAuthCodeSend",
                "OAuthCodeStatus",
                "OAuthCodeBodyComplete",
                "OAuthExchangeComplete",
            ] {
                let marker = format!("gateway-classification-v1 stage={stage} ");
                assert!(
                    stderr.lines().any(|line| line.starts_with(&marker)),
                    "private held OAuth omitted finite stage {stage}"
                );
            }
            assert!(!stderr.contains("loopback-access") && !stderr.contains("loopback-refresh"));
            assert!(!stdout.contains("loopback-access") && !stdout.contains("loopback-refresh"));
            for line in stdout.lines() {
                assert_eq!(serde_json::from_str::<Value>(line)?["jsonrpc"], "2.0");
            }
        }
    }
    Ok(())
}

// Expected envelopes are literal bounded original-gateway observations, including
// the three combined/quoted-name controls. See RUNTIME_SOURCE_REVIEW.md.
#[tokio::test]
async fn actual_native_catalog_validation_diagnostics_are_source_exact_and_never_dispatch() {
    let fixture = Fixture::start(false).await;
    // Keep the original signature's property order. serde_json::Value's map
    // ordering is irrelevant for envelopes but cannot construct this raw schema.
    fixture.app.state.lock().await.catalog_override = Some(r#"{"tools":[{"name":"slopsearx_search","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"intent":{"type":"string"},"language":{"type":"string"}},"required":["query"]}}]}"#.into());
    let mut process = Process::spawn(&fixture, &["--token".into(), "static-sentinel".into()]);
    let outcome = catch_process_case(async {
        let initialized = process.rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"source-catalog-control","version":"1"}}})).await;
        assert_eq!(initialized["result"]["capabilities"]["tools"]["listChanged"], true);
        assert_eq!(initialized["result"]["capabilities"]["resources"], json!({"subscribe":false,"listChanged":false}));
        assert_eq!(initialized["result"]["capabilities"]["prompts"]["listChanged"], false);
        assert_eq!(initialized["result"]["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
        let prompts = process.rpc(json!({"jsonrpc":"2.0","id":2,"method":"prompts/list"})).await;
        let expected_prompts: Value = serde_json::from_str(r####"[{"_meta":{"fastmcp":{"tags":[]}},"arguments":[{"description":"Provide a value matching the following JSON schema: {\"type\":\"string\"}. Encode non-string values as JSON.","name":"question","required":true}],"name":"research_with_source_coverage"},{"_meta":{"fastmcp":{"tags":[]}},"arguments":[{"description":"Provide a value matching the following JSON schema: {\"type\":\"string\"}. Encode non-string values as JSON.","name":"target","required":true}],"name":"investigate_vulnerability"},{"_meta":{"fastmcp":{"tags":[]}},"arguments":[{"description":"Provide a value matching the following JSON schema: {\"type\":\"string\"}. Encode non-string values as JSON.","name":"company","required":true}],"name":"find_company_jobs"},{"_meta":{"fastmcp":{"tags":[]}},"arguments":[{"description":"Provide a value matching the following JSON schema: {\"type\":\"string\"}. Encode non-string values as JSON.","name":"name","required":true}],"name":"compare_package_or_project"},{"_meta":{"fastmcp":{"tags":[]}},"arguments":[{"description":"Provide a value matching the following JSON schema: {\"type\":\"string\"}. Encode non-string values as JSON.","name":"question","required":true}],"name":"plan_research_with_evidence"}]"####).unwrap();
        assert_eq!(prompts["result"]["prompts"], expected_prompts);
        let cases: Vec<Value> = serde_json::from_str(r####"[{"request":{"method":"tools/call","params":{"name":"slopsearx_search","arguments":{}}},"result":{"content":[{"text":"1 validation error for call[slopsearx_search]\nquery\n  Missing required keyword only argument [type=missing_keyword_only_argument, input_value={}, input_type=dict]\n    For further information visit https://errors.pydantic.dev/2.13/v/missing_keyword_only_argument","type":"text"}],"isError":true}},{"request":{"method":"tools/call","params":{"name":"slopsearx_search","arguments":{"query":null}}},"result":{"content":[{"text":"1 validation error for call[slopsearx_search]\nquery\n  Input should be a valid string [type=string_type, input_value=None, input_type=NoneType]\n    For further information visit https://errors.pydantic.dev/2.13/v/string_type","type":"text"}],"isError":true}},{"request":{"method":"tools/call","params":{"name":"slopsearx_search","arguments":{"query":"test","unexpected_catalog_field":true}}},"result":{"content":[{"text":"1 validation error for call[slopsearx_search]\nunexpected_catalog_field\n  Unexpected keyword argument [type=unexpected_keyword_argument, input_value=True, input_type=bool]\n    For further information visit https://errors.pydantic.dev/2.13/v/unexpected_keyword_argument","type":"text"}],"isError":true}},{"request":{"method":"tools/call","params":{"name":"catalog_absent","arguments":{}}},"result":{"content":[{"text":"Unknown tool: 'catalog_absent'","type":"text"}],"isError":true}},{"request":{"method":"tools/call","params":{"name":"slopsearx_search","arguments":{"query":null,"intent":null,"language":5,"unexpected_catalog_field":true}}},"result":{"content":[{"text":"4 validation errors for call[slopsearx_search]\nquery\n  Input should be a valid string [type=string_type, input_value=None, input_type=NoneType]\n    For further information visit https://errors.pydantic.dev/2.13/v/string_type\nintent\n  Input should be a valid string [type=string_type, input_value=None, input_type=NoneType]\n    For further information visit https://errors.pydantic.dev/2.13/v/string_type\nlanguage\n  Input should be a valid string [type=string_type, input_value=5, input_type=int]\n    For further information visit https://errors.pydantic.dev/2.13/v/string_type\nunexpected_catalog_field\n  Unexpected keyword argument [type=unexpected_keyword_argument, input_value=True, input_type=bool]\n    For further information visit https://errors.pydantic.dev/2.13/v/unexpected_keyword_argument","type":"text"}],"isError":true}},{"request":{"method":"tools/call","params":{"name":"catalog_'\"\\\n_absent","arguments":{}}},"result":{"content":[{"text":"Unknown tool: 'catalog_\\'\"\\\\\\n_absent'","type":"text"}],"isError":true}},{"request":{"method":"tools/call","params":{"name":"slopsearx_search","arguments":{"unexpected_catalog_field":true}}},"result":{"content":[{"text":"2 validation errors for call[slopsearx_search]\nquery\n  Missing required keyword only argument [type=missing_keyword_only_argument, input_value={'unexpected_catalog_field': True}, input_type=dict]\n    For further information visit https://errors.pydantic.dev/2.13/v/missing_keyword_only_argument\nunexpected_catalog_field\n  Unexpected keyword argument [type=unexpected_keyword_argument, input_value=True, input_type=bool]\n    For further information visit https://errors.pydantic.dev/2.13/v/unexpected_keyword_argument","type":"text"}],"isError":true}}]"####).unwrap();
        let before = fixture.app.state.lock().await.calls.iter().filter(|row| row["method"] == "tools/call").count();
        for (index, case) in cases.into_iter().enumerate() {
            let mut request = case["request"].clone();
            request["jsonrpc"] = json!("2.0"); request["id"] = json!(index + 10);
            let result = process.rpc(request).await;
            assert_eq!(result, json!({"jsonrpc":"2.0","id":index+10,"result":case["result"]}), "source validation case {index}");
            assert_eq!(fixture.app.state.lock().await.calls.iter().filter(|row| row["method"] == "tools/call").count(), before, "validation must not dispatch case {index}");
        }
        let accepted = process.rpc(json!({"jsonrpc":"2.0","id":99,"method":"tools/call","params":{"name":"slopsearx_search","arguments":{"query":"ok"}}})).await;
        assert_eq!(accepted["result"]["structuredContent"], json!({"query":"ok","intent":null,"language":null}));
        assert_eq!(fixture.app.state.lock().await.calls.iter().filter(|row| row["method"] == "tools/call").count(), before+1);
        Ok(())
    }).await;
    process.close().await;
    fixture.close().await;
    finish_process_case(outcome).unwrap();
}
