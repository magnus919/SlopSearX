//! Actual browser transport. Workflow rendering/execution is injected by W10.
use axum::{
    Extension, Router,
    body::to_bytes,
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use ssx_application::browser_identity::{BrowserIdentity, BrowserMutation, BrowserWorkflow};
use ssx_domain::browser_identity::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use url::Url;
const SESSION: &str = "__Host-slopsearx_session";
const TRANSACTION: &str = "__Secure-slopsearx_login";
#[derive(Clone)]
pub struct BrowserRuntime {
    identity: Arc<dyn BrowserIdentity>,
    workflows: Arc<dyn BrowserWorkflow>,
    origin: String,
    host: String,
    actions: BTreeSet<String>,
    mutations: BTreeSet<String>,
}
// Per-request state is never stored in the shared router configuration.
#[derive(Clone)]
struct BrowserRequest {
    runtime: BrowserRuntime,
    request_id: String,
}
impl std::ops::Deref for BrowserRequest {
    type Target = BrowserRuntime;
    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}
fn request_id(headers: &HeaderMap) -> Result<String, ()> {
    if let Some(value) = headers.get("x-request-id").and_then(|v| v.to_str().ok())
        && !value.is_empty()
    {
        return Ok(value.to_owned());
    }
    let mut bytes = [0_u8; 16];
    rustls::crypto::aws_lc_rs::default_provider()
        .secure_random
        .fill(&mut bytes)
        .map_err(|_| ())?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}
impl BrowserRuntime {
    pub fn new(
        identity: Arc<dyn BrowserIdentity>,
        workflows: Arc<dyn BrowserWorkflow>,
        origin: &str,
        actions: BTreeSet<String>,
        mutations: BTreeSet<String>,
        trust_proxy: bool,
    ) -> Result<Self, BrowserError> {
        let u = Url::parse(origin).map_err(|_| BrowserError::InvalidInput)?;
        if trust_proxy
            || u.scheme() != "https"
            || u.host_str().is_none()
            || !u.username().is_empty()
            || u.password().is_some()
            || u.path() != "/"
            || u.query().is_some()
            || u.fragment().is_some()
            || actions
                .iter()
                .chain(&mutations)
                .any(|a| !BROWSER_ACTIONS.contains(&a.as_str()))
        {
            return Err(BrowserError::InvalidInput);
        }
        Ok(Self {
            identity,
            workflows,
            origin: u.origin().ascii_serialization(),
            host: u[url::Position::BeforeHost..url::Position::AfterPort].into(),
            actions,
            mutations,
        })
    }
}
/// Authentication routes exist only behind this listener's established direct TLS.
type Handshake = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Option<(
                    tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
                    SocketAddr,
                )>,
            > + Send,
    >,
>;
pub struct BrowserTlsListener {
    listener: tokio::net::TcpListener,
    acceptor: tokio_rustls::TlsAcceptor,
    handshakes: Vec<Handshake>,
}
impl BrowserTlsListener {
    pub fn new(listener: tokio::net::TcpListener, config: Arc<rustls::ServerConfig>) -> Self {
        Self {
            listener,
            acceptor: config.into(),
            handshakes: Vec::new(),
        }
    }
}
impl axum::serve::Listener for BrowserTlsListener {
    type Io = tokio_rustls::server::TlsStream<tokio::net::TcpStream>;
    type Addr = SocketAddr;
    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        // Pending handshakes are owned futures, not detached tasks. There are at
        // most 16; dropping this listener synchronously closes every pending peer.
        loop {
            let pending = &mut self.handshakes;
            tokio::select! {
                finished=std::future::poll_fn(|cx| {
                    for i in 0..pending.len() {
                        if let std::task::Poll::Ready(result)=pending[i].as_mut().poll(cx) {
                            drop(pending.swap_remove(i));
                            return std::task::Poll::Ready(result)
                        }
                    }
                    std::task::Poll::Pending
                }), if !pending.is_empty()=> {if let Some(connection)=finished{return connection}},
                accepted=self.listener.accept()=> {
                    match accepted {
                        Ok((stream,addr)) if pending.len()<16=> {
                            let acceptor=self.acceptor.clone();
                            pending.push(Box::pin(async move{
                                tokio::time::timeout(Duration::from_secs(5),acceptor.accept(stream)).await.ok()?.ok().map(|tls|(tls,addr))
                            }));
                        },
                        Ok((_stream,_addr))=>(), // bounded overload refusal
                        Err(_)=>tokio::time::sleep(Duration::from_millis(50)).await,
                    }
                }
            }
        }
    }
    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.listener.local_addr()
    }
}
#[derive(Clone, Copy)]
pub struct BrowserTlsPeer(SocketAddr);
impl axum::extract::connect_info::Connected<axum::serve::IncomingStream<'_, BrowserTlsListener>>
    for BrowserTlsPeer
{
    fn connect_info(stream: axum::serve::IncomingStream<'_, BrowserTlsListener>) -> Self {
        Self(*stream.remote_addr())
    }
}
pub fn browser_router(runtime: BrowserRuntime) -> Router {
    Router::new()
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
        .route("/auth/tenant", get(tenant_choice).post(tenant_switch))
        .route("/auth/logout", post(logout))
        .route("/workflows", get(workflows))
        .route("/workflows/{kind}/{id}", get(detail))
        .route("/workflows/{kind}/{id}/{action}", post(mutation))
        .route_layer(middleware::from_fn_with_state(runtime.clone(), boundary))
        .with_state(runtime)
}
async fn boundary(State(rt): State<BrowserRuntime>, request: Request, next: Next) -> Response {
    let id = match request_id(request.headers()) {
        Ok(id) => id,
        Err(()) => return problem(503, "Request identity unavailable"),
    };
    let request_rt = BrowserRequest {
        runtime: rt,
        request_id: id.clone(),
    };
    let mut response = boundary_request(request_rt, request, next).await;
    // The ID came from an accepted header or our ASCII UUID formatter.
    if let Ok(value) = HeaderValue::from_str(&id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}
async fn boundary_request(rt: BrowserRequest, mut request: Request, next: Next) -> Response {
    if request
        .extensions()
        .get::<ConnectInfo<BrowserTlsPeer>>()
        .is_none()
    {
        return problem(400, "Direct HTTPS required");
    }
    let hosts: Vec<_> = request.headers().get_all("host").iter().collect();
    if hosts.len() != 1 || hosts[0].to_str().ok() != Some(rt.host.as_str()) {
        return problem(400, "Invalid request host");
    }
    request.extensions_mut().insert(rt);
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("same-origin"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert("content-security-policy",HeaderValue::from_static("default-src 'self'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'; object-src 'none'"));
    response
}
fn problem(status: u16, title: &str) -> Response {
    let mut r = (
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        axum::Json(serde_json::json!({"type":"about:blank","title":title,"status":status})),
    )
        .into_response();
    r.headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    if status == 401 {
        r.headers_mut().insert(
            "www-authenticate",
            HeaderValue::from_static("Session realm=\"SlopSearX workflows\""),
        );
    }
    r
}
fn redirect(location: &str) -> Response {
    let mut r = StatusCode::SEE_OTHER.into_response();
    match HeaderValue::from_str(location) {
        Ok(v) => {
            r.headers_mut().insert("location", v);
        }
        Err(_) => return problem(500, "Request unavailable"),
    };
    r.headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    r
}
fn cookie(r: &mut Response, name: &str, value: &str, path: &str, age: Option<u64>) {
    let age = age.map_or(String::new(), |n| format!("; Max-Age={n}"));
    if let Ok(v) = HeaderValue::from_str(&format!(
        "{name}={value}; Path={path}; Secure; HttpOnly; SameSite=Lax{age}"
    )) {
        r.headers_mut().append("set-cookie", v);
    }
}
fn cookies(headers: &HeaderMap) -> Result<BTreeMap<String, String>, BrowserError> {
    let mut out = BTreeMap::new();
    let mut total = 0;
    for raw in headers.get_all("cookie") {
        let raw = raw.to_str().map_err(|_| BrowserError::Denied)?;
        total += raw.len();
        if total > 8192 {
            return Err(BrowserError::Denied);
        }
        for field in raw.split(';') {
            if let Some((k, v)) = field.trim().split_once('=')
                && out.insert(k.into(), v.into()).is_some()
            {
                return Err(BrowserError::Denied);
            }
        }
    }
    Ok(out)
}
fn secret_cookie(headers: &HeaderMap, name: &str) -> Result<BrowserSecret, BrowserError> {
    let values = cookies(headers)?;
    BrowserSecret::new(values.get(name).cloned().ok_or(BrowserError::Missing)?)
}
fn machine(headers: &HeaderMap) -> bool {
    let a = headers
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    a.contains("application/json") || (!a.contains("text/html") && !a.is_empty() && a != "*/*")
}
fn same_origin(headers: &HeaderMap, origin: &str) -> bool {
    let origins: Vec<_> = headers.get_all("origin").iter().collect();
    if !origins.is_empty() {
        return origins.len() == 1
            && origins[0]
                .to_str()
                .is_ok_and(|v| v.trim_end_matches('/') == origin);
    }
    let refs: Vec<_> = headers.get_all("referer").iter().collect();
    refs.len() == 1
        && refs[0]
            .to_str()
            .is_ok_and(|v| v == origin || v.starts_with(&format!("{origin}/")))
}
async fn form(request: Request) -> Result<(HeaderMap, BTreeMap<String, String>), BrowserError> {
    let headers = request.headers().clone();
    if headers.get_all("content-type").iter().count() != 1
        || headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .map(str::trim)
            != Some("application/x-www-form-urlencoded")
    {
        return Err(BrowserError::Denied);
    }
    let bytes = to_bytes(request.into_body(), 16384)
        .await
        .map_err(|_| BrowserError::Denied)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| BrowserError::Denied)?;
    let mut out = BTreeMap::new();
    for (k, v) in url::form_urlencoded::parse(text.as_bytes()) {
        if out.len() >= 12 || out.insert(k.into(), v.into()).is_some() {
            return Err(BrowserError::Denied);
        }
    }
    Ok((headers, out))
}
fn network(request: &Request) -> String {
    request
        .extensions()
        .get::<ConnectInfo<BrowserTlsPeer>>()
        .map_or_else(|| "unavailable".into(), |p| p.0.0.ip().to_string())
}
async fn auth(
    rt: &BrowserRequest,
    headers: HeaderMap,
    target: String,
) -> Result<(BrowserSecret, BrowserSession), Box<Response>> {
    let result: Result<(), BrowserError> = match secret_cookie(&headers, SESSION) {
        Ok(h) => match rt.identity.resolve(&h, &rt.actions).await {
            Ok(s) => return Ok((h, s)),
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    };
    let reason = if matches!(
        result,
        Err(BrowserError::Unavailable | BrowserError::Closed | BrowserError::CompletionUnknown)
    ) {
        BrowserAuditReason::StoreUnavailable
    } else {
        BrowserAuditReason::AuthenticationRequired
    };
    let _ = rt
        .identity
        .audit(
            &rt.request_id,
            BrowserAuditAction::Login,
            BrowserAuditOutcome::Denied,
            reason,
            None,
            None,
        )
        .await;
    if machine(&headers) {
        Err(Box::new(problem(401, "Authentication required")))
    } else {
        let target = browser_return_to(Some(&target));
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("return_to", &target)
            .finish();
        Err(Box::new(redirect(&format!("/auth/login?{query}"))))
    }
}
fn context(session: &BrowserSession, headers: &HeaderMap) -> Result<BrowserContext, Box<Response>> {
    session.context.clone().ok_or_else(|| {
        if machine(headers) {
            Box::new(problem(409, "Workspace selection required"))
        } else {
            Box::new(redirect("/auth/tenant"))
        }
    })
}
async fn login(Extension(rt): Extension<BrowserRequest>, request: Request) -> Response {
    let network = network(&request);
    if rt.identity.rate(BrowserRate::Login, &[&network]).await != Ok(true) {
        return problem(429, "Sign-in temporarily unavailable");
    }
    let pairs: Vec<_> =
        url::form_urlencoded::parse(request.uri().query().unwrap_or("").as_bytes()).collect();
    let target = pairs
        .iter()
        .find(|(k, _)| k == "return_to")
        .map(|(_, v)| v.as_ref());
    match rt.identity.begin(&browser_return_to(target)).await {
        Ok(start) => {
            let mut r = redirect(&start.location);
            cookie(
                &mut r,
                TRANSACTION,
                start.transaction.expose(),
                "/auth/callback",
                Some(600),
            );
            r
        }
        Err(_) => problem(503, "Sign-in temporarily unavailable"),
    }
}
async fn callback(Extension(rt): Extension<BrowserRequest>, request: Request) -> Response {
    let transaction = match secret_cookie(request.headers(), TRANSACTION) {
        Ok(v) => v,
        Err(_) => return login_failure(),
    };
    let network = network(&request);
    if rt
        .identity
        .rate(BrowserRate::Callback, &[transaction.expose(), &network])
        .await
        != Ok(true)
    {
        return problem(429, "Sign-in temporarily unavailable");
    }
    let old = secret_cookie(request.headers(), SESSION).ok();
    match rt
        .identity
        .complete(
            request.uri().query().unwrap_or(""),
            &transaction,
            old.as_ref(),
        )
        .await
    {
        Ok(s) => {
            let mut r = redirect(if s.session.active_tenant.is_some() {
                &s.return_to
            } else {
                "/auth/tenant"
            });
            cookie(&mut r, SESSION, s.handle.expose(), "/", None);
            cookie(&mut r, TRANSACTION, "", "/auth/callback", Some(0));
            if rt
                .identity
                .audit(
                    &rt.request_id,
                    BrowserAuditAction::Login,
                    BrowserAuditOutcome::Succeeded,
                    BrowserAuditReason::Ok,
                    s.session.context.as_ref(),
                    None,
                )
                .await
                .is_err()
            {
                let _ = rt.identity.revoke(&s.handle).await;
                return problem(503, "Audit temporarily unavailable");
            }
            r
        }
        Err(_) => {
            let _ = rt
                .identity
                .audit(
                    &rt.request_id,
                    BrowserAuditAction::Login,
                    BrowserAuditOutcome::Denied,
                    BrowserAuditReason::CallbackRejected,
                    None,
                    None,
                )
                .await;
            login_failure()
        }
    }
}
fn login_failure() -> Response {
    let mut response=axum::response::Html(r#"<!doctype html><h1>Sign-in failed</h1><p>We could not complete sign-in.</p><p><a href="/auth/login">Try again</a> · <a href="/">Return to search</a></p>"#).into_response();
    *response.status_mut() = StatusCode::UNAUTHORIZED;
    response
}
fn escape(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
async fn tenant_choice(Extension(rt): Extension<BrowserRequest>, request: Request) -> Response {
    let (_, s) = match auth(&rt, request.headers().clone(), request.uri().to_string()).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match rt.identity.tenants(&s).await {
        Ok(tenants) => {
            let mut html = format!(
                "<h1>Choose workspace</h1><form method=\"post\" action=\"/auth/tenant\"><input type=\"hidden\" name=\"csrf\" value=\"{}\">",
                s.tenant_csrf.expose()
            );
            for t in tenants {
                html += &format!(
                    "<button name=\"tenant\" value=\"{}\">{}</button>",
                    escape(&t.tenant_id),
                    escape(&t.display_name)
                )
            }
            html += "</form>";
            axum::response::Html(html).into_response()
        }
        Err(_) => problem(401, "Authentication required"),
    }
}
fn csrf_valid(
    rt: &BrowserRequest,
    expected: &BrowserSecret,
    headers: &HeaderMap,
    fields: &BTreeMap<String, String>,
) -> bool {
    let values: Vec<_> = headers.get_all("x-csrf-token").iter().collect();
    if values.len() > 1 {
        return false;
    }
    if values.first().is_some_and(|v| v.to_str().is_err()) {
        return false;
    }
    let from_header = values.first().and_then(|v| v.to_str().ok());
    let from_form = fields.get("csrf").map(String::as_str);
    if from_header.is_none() && from_form.is_none() {
        return false;
    }
    from_header.is_none_or(|s| rt.identity.csrf(expected, s))
        && from_form.is_none_or(|s| rt.identity.csrf(expected, s))
}
async fn denied(
    rt: &BrowserRequest,
    action: BrowserAuditAction,
    reason: BrowserAuditReason,
    context: Option<&BrowserContext>,
    object: Option<(&str, &str)>,
) {
    let _ = rt
        .identity
        .audit(
            &rt.request_id,
            action,
            BrowserAuditOutcome::Denied,
            reason,
            context,
            object,
        )
        .await;
}
async fn tenant_switch(Extension(rt): Extension<BrowserRequest>, request: Request) -> Response {
    let (handle, s) = match auth(&rt, request.headers().clone(), request.uri().to_string()).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let (headers, f) = match form(request).await {
        Ok(v) => v,
        Err(_) => {
            denied(
                &rt,
                BrowserAuditAction::TenantSwitch,
                BrowserAuditReason::CsrfDenied,
                s.context.as_ref(),
                None,
            )
            .await;
            return problem(403, "Request denied");
        }
    };
    if !same_origin(&headers, &rt.origin) || !csrf_valid(&rt, &s.tenant_csrf, &headers, &f) {
        denied(
            &rt,
            BrowserAuditAction::TenantSwitch,
            BrowserAuditReason::CsrfDenied,
            s.context.as_ref(),
            None,
        )
        .await;
        return problem(403, "Request denied");
    }
    if rt
        .identity
        .rate(
            BrowserRate::TenantSwitch,
            &[&s.principal_id, s.active_tenant.as_deref().unwrap_or("")],
        )
        .await
        != Ok(true)
    {
        return problem(429, "Action temporarily unavailable");
    }
    match rt
        .identity
        .rotate(&handle, &s, f.get("tenant").map_or("", String::as_str))
        .await
    {
        Ok(new) => {
            let mut r = redirect("/workflows");
            cookie(&mut r, SESSION, new.handle.expose(), "/", None);
            if rt
                .identity
                .audit(
                    &rt.request_id,
                    BrowserAuditAction::TenantSwitch,
                    BrowserAuditOutcome::Succeeded,
                    BrowserAuditReason::Ok,
                    new.session.context.as_ref(),
                    None,
                )
                .await
                .is_err()
            {
                let _ = rt.identity.revoke(&new.handle).await;
                return problem(503, "Audit temporarily unavailable");
            }
            r
        }
        Err(_) => {
            denied(
                &rt,
                BrowserAuditAction::TenantSwitch,
                BrowserAuditReason::AuthorizationDenied,
                s.context.as_ref(),
                None,
            )
            .await;
            problem(403, "Request denied")
        }
    }
}
async fn logout(Extension(rt): Extension<BrowserRequest>, request: Request) -> Response {
    let (handle, s) = match auth(&rt, request.headers().clone(), request.uri().to_string()).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let c = match context(&s, request.headers()) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    let (headers, f) = match form(request).await {
        Ok(v) => v,
        Err(_) => {
            denied(
                &rt,
                BrowserAuditAction::Logout,
                BrowserAuditReason::CsrfDenied,
                Some(&c),
                None,
            )
            .await;
            return problem(403, "Request denied");
        }
    };
    if !same_origin(&headers, &rt.origin) || !csrf_valid(&rt, &c.csrf, &headers, &f) {
        denied(
            &rt,
            BrowserAuditAction::Logout,
            BrowserAuditReason::CsrfDenied,
            Some(&c),
            None,
        )
        .await;
        return problem(403, "Request denied");
    }
    if rt
        .identity
        .rate(BrowserRate::Logout, &[&c.principal_id, &c.tenant_id])
        .await
        != Ok(true)
    {
        return problem(429, "Action temporarily unavailable");
    }
    if rt.identity.revoke(&handle).await.is_err() {
        return problem(503, "Action temporarily unavailable");
    }
    if rt
        .identity
        .audit(
            &rt.request_id,
            BrowserAuditAction::Logout,
            BrowserAuditOutcome::Succeeded,
            BrowserAuditReason::Ok,
            Some(&c),
            None,
        )
        .await
        .is_err()
    {
        return problem(503, "Audit temporarily unavailable");
    }
    let mut r = redirect("/");
    cookie(&mut r, SESSION, "", "/", Some(0));
    r.headers_mut()
        .insert("clear-site-data", HeaderValue::from_static("\"cache\""));
    r
}
async fn workflows(Extension(rt): Extension<BrowserRequest>, request: Request) -> Response {
    read(rt, request, None).await
}
async fn detail(
    Extension(rt): Extension<BrowserRequest>,
    axum::extract::Path((kind, id)): axum::extract::Path<(String, String)>,
    request: Request,
) -> Response {
    read(rt, request, Some((kind, id))).await
}
fn unavailable() -> Response {
    let mut r=axum::response::Html("<!doctype html><h1>Not found or unavailable</h1><p>The workflow is unavailable. It may be unknown, expired, revoked, or outside your current access.</p>").into_response();
    *r.status_mut() = StatusCode::NOT_FOUND;
    r
}
async fn read(rt: BrowserRequest, request: Request, object: Option<(String, String)>) -> Response {
    let (_, s) = match auth(&rt, request.headers().clone(), request.uri().to_string()).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let c = match context(&s, request.headers()) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    if rt
        .identity
        .rate(BrowserRate::Read, &[&c.principal_id, &c.tenant_id])
        .await
        != Ok(true)
    {
        return problem(429, "Workflow access temporarily unavailable");
    }
    if !c.grants.contains("workflow.read") {
        denied(
            &rt,
            BrowserAuditAction::Read,
            BrowserAuditReason::AuthorizationDenied,
            Some(&c),
            None,
        )
        .await;
        return problem(403, "Access unavailable");
    }
    let result = rt
        .workflows
        .read(
            &c,
            object.as_ref().map(|v| v.0.as_str()),
            object.as_ref().map(|v| v.1.as_str()),
        )
        .await;
    let (outcome, reason) = if result.is_ok() {
        (BrowserAuditOutcome::Allowed, BrowserAuditReason::Ok)
    } else {
        (
            BrowserAuditOutcome::Denied,
            BrowserAuditReason::ObjectUnavailable,
        )
    };
    if rt
        .identity
        .audit(
            &rt.request_id,
            BrowserAuditAction::Read,
            outcome,
            reason,
            Some(&c),
            object.as_ref().map(|v| (v.0.as_str(), v.1.as_str())),
        )
        .await
        .is_err()
    {
        return problem(503, "Audit temporarily unavailable");
    }
    match result{Ok(body)=>axum::response::Html(format!("<h1>Workflows</h1><pre>{}</pre><form method=\"post\" action=\"/auth/logout\"><input type=\"hidden\" name=\"csrf\" value=\"{}\"><button>Sign out</button></form>",escape(&body),c.csrf.expose())).into_response(),Err(_)=>unavailable()}
}
async fn mutation(
    Extension(rt): Extension<BrowserRequest>,
    axum::extract::Path((kind, id, action)): axum::extract::Path<(String, String, String)>,
    request: Request,
) -> Response {
    let (_, s) = match auth(&rt, request.headers().clone(), request.uri().to_string()).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let c = match context(&s, request.headers()) {
        Ok(c) => c,
        Err(r) => return *r,
    };
    let (grant, audit) = match (kind.as_str(), action.as_str()) {
        ("research", "cancel") => (
            "workflow.research.cancel",
            BrowserAuditAction::ResearchCancel,
        ),
        ("research", "retry") => ("workflow.research.retry", BrowserAuditAction::ResearchRetry),
        ("saved_search", "pause" | "resume") => {
            ("workflow.saved.control", BrowserAuditAction::SavedControl)
        }
        ("saved_search", "ack") => ("workflow.saved.ack", BrowserAuditAction::SavedAck),
        ("staged_search", "compose") => ("workflow.compose", BrowserAuditAction::Compose),
        _ => return problem(403, "Action unavailable"),
    };
    if !c.grants.contains(grant) || !rt.mutations.contains(grant) {
        denied(
            &rt,
            audit,
            BrowserAuditReason::AuthorizationDenied,
            Some(&c),
            Some((&kind, &id)),
        )
        .await;
        return problem(403, "Action unavailable");
    }
    if rt
        .identity
        .rate(
            BrowserRate::Mutation,
            &[&c.principal_id, &c.tenant_id, grant],
        )
        .await
        != Ok(true)
    {
        return problem(429, "Action temporarily unavailable");
    }
    let (headers, f) = match form(request).await {
        Ok(v) => v,
        Err(_) => {
            denied(
                &rt,
                audit,
                BrowserAuditReason::CsrfDenied,
                Some(&c),
                Some((&kind, &id)),
            )
            .await;
            return problem(403, "Request denied");
        }
    };
    if !same_origin(&headers, &rt.origin) || !csrf_valid(&rt, &c.csrf, &headers, &f) {
        let _ = rt
            .identity
            .audit(
                &rt.request_id,
                audit,
                BrowserAuditOutcome::Denied,
                BrowserAuditReason::CsrfDenied,
                Some(&c),
                Some((&kind, &id)),
            )
            .await;
        return problem(403, "Request denied");
    }
    let revision = match f
        .get("expected_revision")
        .and_then(|v| v.parse::<u64>().ok())
    {
        Some(v) => v,
        None => return problem(403, "Request denied"),
    };
    let idem = f.get("idempotency_key").map_or("", String::as_str);
    if !(12..=128).contains(&idem.chars().count()) {
        return problem(400, "Invalid idempotency key");
    }
    // Source semantic identity deliberately excludes CSRF/idem presentation.
    let cursor = f.get("cursor").map_or("", String::as_str);
    let question = f.get("question").map_or("", String::as_str);
    let strategy = f.get("strategy").map_or("triangulate", String::as_str);
    // The source digest uses NUL separators. Reject decoded separators before
    // claiming admission so distinct semantic tuples cannot share an identity.
    if [
        kind.as_str(),
        id.as_str(),
        action.as_str(),
        cursor,
        question,
        strategy,
    ]
    .iter()
    .any(|value| value.contains('\0'))
    {
        return problem(403, "Request denied");
    }
    // HTTP-only CSRF/cookies and unrelated form fields never enter workflow commands.
    let fields = vec![
        ("cursor".into(), cursor.into()),
        ("question".into(), question.into()),
        ("strategy".into(), strategy.into()),
    ];
    let digest = semantic_digest((&kind, &id, &action, revision, cursor, question, strategy));
    let claim = match rt.identity.claim_mutation(&c, grant, idem, &digest).await {
        Ok(BrowserAdmission::Replay) => return redirect("/workflows"),
        Ok(BrowserAdmission::Claimed(v)) => v,
        Err(_) => {
            denied(
                &rt,
                audit,
                BrowserAuditReason::Duplicate,
                Some(&c),
                Some((&kind, &id)),
            )
            .await;
            return problem(409, "Action already submitted or changed");
        }
    };
    match rt
        .workflows
        .mutate(
            &c,
            BrowserMutation {
                kind: &kind,
                id: &id,
                action: &action,
                revision,
                idempotency: idem,
                fields: &fields,
            },
        )
        .await
    {
        Ok(()) => {
            if rt
                .identity
                .finish_mutation(&c, grant, idem, &digest, &claim)
                .await
                .is_err()
            {
                return problem(409, "Action already submitted or changed");
            }
            if rt
                .identity
                .audit(
                    &rt.request_id,
                    audit,
                    BrowserAuditOutcome::Succeeded,
                    BrowserAuditReason::Ok,
                    Some(&c),
                    Some((&kind, &id)),
                )
                .await
                .is_err()
            {
                return problem(503, "Audit temporarily unavailable");
            }
            redirect("/workflows")
        }
        Err(BrowserError::Conflict) => {
            denied(
                &rt,
                audit,
                BrowserAuditReason::RevisionConflict,
                Some(&c),
                Some((&kind, &id)),
            )
            .await;
            problem(409, "Action already submitted or changed")
        }
        Err(_) => {
            denied(
                &rt,
                audit,
                BrowserAuditReason::ObjectUnavailable,
                Some(&c),
                Some((&kind, &id)),
            )
            .await;
            unavailable()
        }
    }
}

fn semantic_digest(parts: (&str, &str, &str, u64, &str, &str, &str)) -> String {
    use sha2::Digest as _;
    let (kind, id, action, revision, cursor, question, strategy) = parts;
    sha2::Sha256::digest(
        format!("{kind}\0{id}\0{action}\0{revision}\0{cursor}\0{question}\0{strategy}").as_bytes(),
    )
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect()
}
#[cfg(test)]
mod source_replay {
    use super::*;
    #[test]
    fn semantic_digest_matches_actual_pinned_source_expression() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../xtask/fixtures/t09/source.json")).unwrap();
        let cases = v["mutation_digests"].as_array().unwrap();
        assert_eq!(cases.len(), 5);
        for c in cases {
            let i = &c["input"];
            assert_eq!(
                semantic_digest((
                    i["kind"].as_str().unwrap(),
                    i["object_id"].as_str().unwrap(),
                    i["action"].as_str().unwrap(),
                    i["revision"].as_u64().unwrap(),
                    i["cursor"].as_str().unwrap(),
                    i["question"].as_str().unwrap(),
                    i["strategy"].as_str().unwrap()
                )),
                c["source"].as_str().unwrap(),
                "{}",
                c["id"]
            );
        }
    }
}
