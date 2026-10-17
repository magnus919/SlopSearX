//! Native HTTP adapter over the accepted shared search capture/read service.
use crate::formats;
use axum::{
    Router,
    body::to_bytes,
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{MethodFilter, get, on},
};
use ssx_application::{
    health::{HealthContext, build_engine_health},
    policy::{ScopeProposal, SearchSurface},
};
use ssx_domain::{SearchRequest, SearchResponse, WallClockTimestamp};
use ssx_infra::{
    observability::Telemetry,
    search::{SearchError, SearchService},
};
use ssx_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

/// T03 supplies its complete portal writer through this shared request boundary.
/// A renderer receives only public response data; it cannot dispatch engines.
pub trait HtmlRenderer: Send + Sync {
    fn landing(&self) -> String;
    fn error(&self, error: &str, message: &str, field: Option<&str>) -> String;
    fn search(&self, response: &SearchResponse, meta: &Value, request: &SearchRequest) -> String;
}
#[derive(Clone)]
pub struct HttpState {
    pub search: SearchService,
    pub policy: Arc<ssx_application::policy::SearchPolicy>,
    pub telemetry: Arc<Telemetry>,
    pub formats: BTreeSet<String>,
    /// Startup-frozen, redacted discovery. Never pass raw config/environment.
    pub discovery: Value,
    pub health_contexts: BTreeMap<String, HealthContext>,
    /// A local view of the owned quota connection; this never probes providers.
    pub valkey_connected: Arc<dyn Fn() -> bool + Send + Sync>,
    pub valkey_fail_closed: bool,
    pub valkey_quota_configured: bool,
    pub html: Arc<dyn HtmlRenderer>,
}
pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/", on(MethodFilter::GET.or(MethodFilter::POST), search))
        .route(
            "/search",
            on(MethodFilter::GET.or(MethodFilter::POST), search),
        )
        .route(
            "/healthz",
            get(|| async {
                (
                    StatusCode::OK,
                    [("content-type", "text/plain; charset=utf-8")],
                    "OK",
                )
            }),
        )
        .route("/openapi.json", get(openapi))
        .route("/health", get(health))
        .route("/config", get(config))
        .route("/metrics", get(metrics))
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                [("content-type", "application/json")],
                "{\"detail\":\"Not Found\"}",
            )
        })
        .layer(middleware::from_fn(request_id))
        .with_state(state)
}
pub async fn serve(
    listener: tokio::net::TcpListener,
    state: HttpState,
) -> Result<(), std::io::Error> {
    serve_with_shutdown(listener, state, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
}
/// The composition root owns its shutdown trigger. Draining is always shared.
pub async fn serve_with_shutdown<F>(
    listener: tokio::net::TcpListener,
    state: HttpState,
    shutdown: F,
) -> Result<(), std::io::Error>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let result = axum::serve(
        listener,
        router(state.clone()).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await;
    state.search.shutdown().await;
    state.telemetry.shutdown().await;
    result
}
/// Static typed request schema is independent of ambient configuration and credentials.
#[derive(Clone, Copy)]
enum ParameterType {
    Text,
    Integer,
    Date,
}
fn search_parameters() -> Vec<Value> {
    use ParameterType::*;
    [
        ("q", Text),
        ("format", Text),
        ("categories", Text),
        ("engines", Text),
        ("language", Text),
        ("pageno", Integer),
        ("safesearch", Integer),
        ("time_range", Text),
        ("interactive_timeout_ms", Integer),
        ("date_from", Date),
        ("date_to", Date),
    ]
    .into_iter()
    .map(|(name, kind)| {
        let schema = match kind {
            Text => json!({"type":"string"}),
            Integer => json!({"type":"integer"}),
            Date => json!({"type":"string","format":"date"}),
        };
        json!({"name":name,"in":"query","required":false,"schema":schema})
    })
    .collect()
}
async fn openapi() -> Response {
    let parameters = search_parameters();
    let responses = json!({"200":{"description":"Formatted search results"},"400":{"description":"Invalid search parameter"},"403":{"description":"Format or source unavailable"},"429":{"description":"Client rate limited"},"503":{"description":"No responsive engines"}});
    let operation = json!({"parameters":parameters,"responses":responses});
    let mut post = operation.clone();
    post["requestBody"] = json!({"required":false,"content":{"application/x-www-form-urlencoded":{"schema":{"type":"object","properties":search_parameters().into_iter().map(|p|(p["name"].as_str().unwrap().to_owned(),p["schema"].clone())).collect::<ssx_json::Map<String,Value>>()}}}});
    reply(200,"json",json!({"openapi":"3.1.0","info":{"title":"SlopSearX native HTTP API","version":env!("CARGO_PKG_VERSION")},"paths":{"/search":{"get":operation,"post":post},"/":{"get":operation,"post":post},"/healthz":{"get":{"responses":{"200":{"description":"Local readiness"}}}},"/health":{"get":{"responses":{"200":{"description":"Passive engine observations"}}}},"/config":{"get":{"responses":{"200":{"description":"Redacted runtime discovery"}}}},"/metrics":{"get":{"responses":{"200":{"description":"OpenMetrics telemetry"}}}}}}).to_string())
}
/// Request-scoped transport ID. Consumers may read it without parsing or trusting its contents.
#[derive(Clone, Debug)]
pub struct RequestId(pub HeaderValue);
async fn request_id(mut request: Request, next: Next) -> Response {
    let id=request.headers().get("x-request-id").filter(|v|!v.as_bytes().is_empty()).cloned().unwrap_or_else(||{
        let mut bytes=[0u8;16];aws_lc_rs::rand::fill(&mut bytes).expect("operating system randomness unavailable");bytes[6]=(bytes[6]&15)|64;bytes[8]=(bytes[8]&63)|128;
        HeaderValue::from_str(&format!("{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",bytes[0],bytes[1],bytes[2],bytes[3],bytes[4],bytes[5],bytes[6],bytes[7],bytes[8],bytes[9],bytes[10],bytes[11],bytes[12],bytes[13],bytes[14],bytes[15])).expect("ASCII UUID")
    });
    request.extensions_mut().insert(RequestId(id.clone()));
    let mut response = next.run(request).await;
    response.headers_mut().insert("x-request-id", id);
    response
}
fn reply(status: u16, format: &str, body: String) -> Response {
    let media = match format {
        "html" => "text/html; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        "rss" => "application/rss+xml; charset=utf-8",
        "yaml" => "text/vnd.yaml+markdown; charset=utf-8",
        _ => "application/json",
    };
    let mut response = (
        StatusCode::from_u16(status).expect("fixed HTTP status"),
        [("content-type", media)],
        body,
    )
        .into_response();
    if format == "html" {
        for (k, v) in [
            (
                "content-security-policy",
                "default-src 'self'; base-uri 'self'; connect-src 'none'; form-action 'self'; frame-ancestors 'none'; object-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: http: https:",
            ),
            ("referrer-policy", "no-referrer"),
            ("x-content-type-options", "nosniff"),
            (
                "permissions-policy",
                "camera=(), microphone=(), geolocation=()",
            ),
        ] {
            response
                .headers_mut()
                .insert(k, HeaderValue::from_static(v));
        }
    }
    response
}
fn error(
    state: &HttpState,
    format: &str,
    status: u16,
    code: &str,
    message: &str,
    field: Option<&str>,
    extra: Option<Value>,
) -> Response {
    let mut payload = json!({"error":code,"message":message});
    if let Some(field) = field {
        payload["field"] = json!(field);
    }
    if let Some(Value::Object(extra)) = extra {
        for (k, v) in extra {
            payload[k.as_str()] = v;
        }
    }
    let body = match format {
        "html" => state.html.error(code, message, field),
        "csv" => format!("{}{code},{message}\n", formats::csv(&[])),
        "rss" => formats::rss(&[], message),
        "yaml" => payload
            .as_object()
            .expect("error mapping")
            .iter()
            .map(|(k, v)| format!("{k}: {}\n", formats::python_repr(v)))
            .collect(),
        _ => payload.to_string(),
    };
    reply(status, format, body)
}
fn accept_format(accept: &str) -> &'static str {
    let mut best: Option<(f64, usize, &str)> = None;
    for (index, item) in accept.split(',').enumerate() {
        let mut parts = item
            .split(';')
            .map(|s| s.trim_matches(ssx_json::python_whitespace).to_lowercase());
        let media = parts.next().unwrap_or_default();
        let mut quality = 1.;
        for part in parts {
            if let Some(q) = part.strip_prefix("q=") {
                quality = python_float(q).unwrap_or(0.);
            }
        }
        if quality <= 0. {
            continue;
        }
        let format = match media.as_str() {
            "text/html" | "application/xhtml+xml" => "html",
            "application/json" | "application/*+json" => "json",
            "text/csv" => "csv",
            "application/rss+xml" | "application/atom+xml" | "application/xml" | "text/xml" => {
                "rss"
            }
            "text/vnd.yaml+markdown" | "application/yaml" | "text/yaml" => "yaml",
            _ => continue,
        };
        if best.is_none_or(|(q, i, _)| quality > q || (quality == q && index < i)) {
            best = Some((quality, index, format));
        }
    }
    best.map_or("html", |(_, _, f)| f)
}
fn select_format(
    state: &HttpState,
    requested: Option<&str>,
    accept: &str,
) -> Result<String, Box<Response>> {
    let format = requested
        .map(|s| s.trim_matches(ssx_json::python_whitespace).to_lowercase())
        .unwrap_or_else(|| accept_format(accept).into());
    if !matches!(format.as_str(), "html" | "json" | "csv" | "rss" | "yaml") {
        return Err(Box::new(error(
            state,
            "json",
            400,
            "unsupported_format",
            &format!("Unsupported response format: {}", requested.unwrap_or("")),
            Some("format"),
            None,
        )));
    }
    if format != "yaml" && !state.formats.contains(&format) {
        return Err(Box::new(error(
            state,
            &format,
            403,
            "format_disabled",
            &format!("Response format '{format}' is disabled."),
            Some("format"),
            None,
        )));
    }
    Ok(format)
}
fn numeric_text(raw: &str) -> Option<String> {
    let mut text = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii() {
            text.push(ch);
        } else if let Some(d) = ssx_json::decimal_value(ch) {
            text.push(char::from(b'0' + d));
        } else if ssx_json::python_whitespace(ch) {
            text.push(' ');
        } else {
            return None;
        }
    }
    let text = text.trim_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let bytes = text.as_bytes();
    for (index, b) in bytes.iter().enumerate() {
        if *b == b'_'
            && (index == 0
                || index + 1 == bytes.len()
                || !bytes[index - 1].is_ascii_digit()
                || !bytes[index + 1].is_ascii_digit())
        {
            return None;
        }
    }
    Some(text.replace('_', ""))
}
fn python_float(raw: &str) -> Option<f64> {
    let text = numeric_text(raw)?;
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(&text);
    match unsigned.to_ascii_lowercase().as_str() {
        "inf" | "infinity" => Some(if text.starts_with('-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }),
        "nan" => Some(f64::NAN),
        _ => text.parse().ok(),
    }
}
/// Classify Python's integer grammar before bounded domain conversion. Values
/// outside i128 retain their sign/range classification; no accepted value is
/// truncated or saturated into a provider request.
fn integer(raw: &str) -> Option<i128> {
    let text = numeric_text(raw)?;
    let (negative, raw) = if let Some(s) = text.strip_prefix('-') {
        (true, s)
    } else {
        (false, text.strip_prefix('+').unwrap_or(&text))
    };
    if raw.is_empty() || raw.len() > 4300 || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let result = raw.bytes().fold(0i128, |n, b| {
        n.saturating_mul(10).saturating_add(i128::from(b - b'0'))
    });
    Some(if negative { -result } else { result })
}
fn param<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .rev()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}
fn names(raw: &str) -> Option<Vec<String>> {
    let names = raw
        .split(',')
        .map(|s| s.trim_matches(ssx_json::python_whitespace))
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    (!names.is_empty()).then_some(names)
}
struct SearchCompletion {
    telemetry: Arc<Telemetry>,
    start: Instant,
    outcome: Option<&'static str>,
}
impl Drop for SearchCompletion {
    fn drop(&mut self) {
        if let Some(outcome) = self.outcome {
            let _ = self.telemetry.metrics.update(
                "slopsearx_http_search_completed_total",
                &[("outcome", outcome)],
                1.,
            );
            let _ = self.telemetry.metrics.update(
                "slopsearx_http_search_duration_seconds",
                &[("outcome", outcome)],
                self.start.elapsed().as_secs_f64(),
            );
        }
    }
}
async fn search(State(state): State<HttpState>, request: Request) -> Response {
    // Drop records failure during cancellation or unwind without swallowing it.
    let mut completion = SearchCompletion {
        telemetry: state.telemetry.clone(),
        start: Instant::now(),
        outcome: Some("failure"),
    };
    let root = request.uri().path() == "/";
    let is_get = request.method() == axum::http::Method::GET;
    let query_q = url::form_urlencoded::parse(request.uri().query().unwrap_or("").as_bytes())
        .filter(|(k, _)| k == "q")
        .last()
        .map(|(_, v)| v.to_string())
        .unwrap_or_default();
    let response = search_impl(&state, request).await;
    let landing = root
        && is_get
        && query_q.trim_matches(ssx_json::python_whitespace).is_empty()
        && response.status() == StatusCode::OK;
    let status = response.status().as_u16();
    completion.outcome = if landing {
        None
    } else {
        Some(if (200..400).contains(&status) {
            "success"
        } else if (400..500).contains(&status) && status != 429 {
            "rejected"
        } else {
            "failure"
        })
    };
    response
}
/// ASGI/Starlette exposes HTTP header octets through ISO-8859-1, including obs-text.
fn latin1_header(request: &Request, name: &str) -> String {
    request
        .headers()
        .get(name)
        .map(|value| value.as_bytes().iter().copied().map(char::from).collect())
        .unwrap_or_default()
}
async fn search_impl(state: &HttpState, request: Request) -> Response {
    let root = request.uri().path() == "/";
    let is_get = request.method() == axum::http::Method::GET;
    let accept = latin1_header(&request, "accept");
    let form = request.method() == axum::http::Method::POST
        && latin1_header(&request, "content-type")
            .split(';')
            .next()
            .unwrap_or("")
            .trim_matches(ssx_json::python_whitespace)
            .eq_ignore_ascii_case("application/x-www-form-urlencoded");
    let peer = request
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|p| p.0.ip().to_string());
    let mut pairs = url::form_urlencoded::parse(request.uri().query().unwrap_or("").as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect::<Vec<_>>();
    if form {
        let body = match to_bytes(request.into_body(), usize::MAX).await {
            Ok(body) => body,
            Err(_) => {
                return error(
                    state,
                    "json",
                    400,
                    "invalid_request",
                    "Invalid search request.",
                    None,
                    None,
                );
            }
        };
        let decoded = String::from_utf8_lossy(&body);
        pairs.extend(
            url::form_urlencoded::parse(decoded.as_bytes())
                .map(|(k, v)| (k.into_owned(), v.into_owned())),
        );
    }
    let early_error = |field: Option<&str>, message: &str| match select_format(
        state,
        param(&pairs, "format"),
        &accept,
    ) {
        Ok(format) => error(state, &format, 400, "invalid_filter", message, field, None),
        Err(response) => *response,
    };
    let mut seen = BTreeSet::new();
    for (key, _) in &pairs {
        if !seen.insert(key) {
            continue;
        }
        let compact = key.to_lowercase().replace(['_', '-'], "");
        if (compact.starts_with("date")
            || matches!(
                compact.as_str(),
                "fromdate" | "todate" | "startdate" | "enddate" | "timerange"
            ))
            && !matches!(key.as_str(), "date_from" | "date_to" | "time_range")
        {
            return early_error(Some(key), "Unknown or repeated date parameter.");
        }
    }
    for key in ["date_from", "date_to"] {
        if pairs.iter().filter(|(k, _)| k == key).count() > 1 {
            return early_error(Some(key), "Unknown or repeated date parameter.");
        }
    }
    let page = integer(param(&pairs, "pageno").unwrap_or("1"));
    let safe = integer(param(&pairs, "safesearch").unwrap_or("0"));
    let timeout = param(&pairs, "interactive_timeout_ms")
        .filter(|s| !s.is_empty())
        .map(integer);
    let (Some(page), Some(safe)) = (page, safe) else {
        return early_error(None, "Invalid search parameter.");
    };
    if matches!(timeout, Some(None)) {
        return early_error(None, "Invalid search parameter.");
    }
    let timeout = timeout.flatten();
    for (field, bad) in [
        ("pageno", page < 1),
        ("safesearch", !(0..=2).contains(&safe)),
        (
            "interactive_timeout_ms",
            timeout.is_some_and(|ms| !(1..=30000).contains(&ms)),
        ),
    ] {
        if bad {
            return early_error(Some(field), &format!("Invalid value for '{field}'."));
        }
    }
    let query = param(&pairs, "q").unwrap_or("");
    if root && is_get && query.trim_matches(ssx_json::python_whitespace).is_empty() {
        match select_format(state, param(&pairs, "format"), &accept) {
            Err(response) => return *response,
            Ok(format) if format == "html" => return reply(200, "html", state.html.landing()),
            _ => {}
        }
    }
    let metric_format = param(&pairs, "format")
        .map(|s| s.trim_matches(ssx_json::python_whitespace).to_lowercase())
        .unwrap_or_else(|| accept_format(&accept).into());
    let metric_format = if matches!(
        metric_format.as_str(),
        "html" | "json" | "csv" | "rss" | "yaml"
    ) {
        metric_format.as_str()
    } else {
        "other"
    };
    let _ = state
        .telemetry
        .metrics
        .update("slopsearx_server_requests_total", &[], 1.);
    let _ = state.telemetry.metrics.update(
        "slopsearx_server_requests_by_format_total",
        &[("format", metric_format)],
        1.,
    );
    let categories = param(&pairs, "categories").unwrap_or("");
    for category in categories
        .split(',')
        .map(|s| s.trim_matches(ssx_json::python_whitespace))
        .filter(|s| !s.is_empty())
        .map(|category| {
            let known = state.discovery["categories"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(category)));
            if known { category } else { "other" }
        })
        .collect::<BTreeSet<_>>()
    {
        let _ = state.telemetry.metrics.update(
            "slopsearx_server_requests_by_category_total",
            &[("category", category)],
            1.,
        );
    }
    let format = match select_format(state, param(&pairs, "format"), &accept) {
        Ok(format) => format,
        Err(response) => return *response,
    };
    let fail = |field: Option<&str>, message: &str| {
        error(state, &format, 400, "invalid_filter", message, field, None)
    };
    if (param(&pairs, "date_from").is_some() || param(&pairs, "date_to").is_some())
        && matches!(format.as_str(), "csv" | "rss")
    {
        return fail(
            Some("format"),
            "Bounded publication-date proof requires json, yaml, or html format.",
        );
    }
    let restricted = names(param(&pairs, "engines").unwrap_or(""))
        .unwrap_or_default()
        .into_iter()
        .filter(|n| state.policy.sensitive_engines.contains(n) && !state.policy.sensitive_allowed)
        .collect::<BTreeSet<_>>();
    if !restricted.is_empty() {
        return error(
            state,
            &format,
            403,
            "engine_restricted",
            "Explicit selection of one or more sensitive sources requires operator authorization.",
            Some("engines"),
            Some(json!({"engines":restricted})),
        );
    }
    let Ok(page) = u64::try_from(page) else {
        return fail(Some("pageno"), "Invalid value for 'pageno'.");
    };
    let request = SearchRequest {
        query: query.into(),
        page,
        safesearch: safe as u8,
        interactive_timeout_ms: timeout.map(|ms| ms as u64),
        categories: names(categories),
        engines: names(param(&pairs, "engines").unwrap_or("")),
        language: param(&pairs, "language").unwrap_or("en").into(),
        time_range: param(&pairs, "time_range")
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        date_from: param(&pairs, "date_from").map(str::to_owned),
        date_to: param(&pairs, "date_to").map(str::to_owned),
        client_identifier: peer,
        ..Default::default()
    };
    let portal_request = request.clone();
    let mut proposal = ScopeProposal::generic(request);
    proposal.surface = SearchSurface::Http;
    let view = match state
        .search
        .capture_read(proposal, Default::default())
        .await
        .and_then(|capture| capture.present_with_advisories())
    {
        Ok(view) => view,
        Err(SearchError::Policy(policy)) => {
            if policy.code == "engine_restricted" || policy.code == "sensitive_engine_denied" {
                return error(
                    state,
                    &format,
                    403,
                    "engine_restricted",
                    "Explicit selection of one or more sensitive sources requires operator authorization.",
                    Some("engines"),
                    Some(*policy.details),
                );
            }
            return if policy.field == "query" {
                error(
                    state,
                    &format,
                    400,
                    "query_required",
                    "The 'q' parameter is required.",
                    None,
                    None,
                )
            } else {
                fail(Some(&policy.field), "Invalid search parameter.")
            };
        }
        Err(SearchError::ClientLimited) => {
            return error(
                state,
                &format,
                429,
                "rate_limited",
                "Too many requests. Please slow down.",
                None,
                None,
            );
        }
        Err(SearchError::Unavailable(unavailable)) => {
            let response = SearchResponse {
                query: query.into(),
                all_unresponsive: true,
                ..Default::default()
            };
            let advisories = state
                .search
                .read_advisories(&portal_request, &unavailable.preview.scope, 0)
                .unwrap_or_default();
            let mut random = [0; 4];
            aws_lc_rs::rand::fill(&mut random).expect("operating system randomness unavailable");
            let query_id = format!(
                "ssx-{:02x}{:02x}{:02x}{:02x}",
                random[0], random[1], random[2], random[3]
            );
            let meta = json!({"response_time_ms":0,"cached":false,"query_id":query_id,"engine_status":{},"advisories":advisories});
            return render(state, &format, &response, meta, 503, true, &portal_request);
        }
        Err(_) => {
            return error(
                state,
                &format,
                503,
                "service_unavailable",
                "Temporarily unavailable",
                None,
                None,
            );
        }
    };
    let response = &view.response;
    let meta = formats::meta(response, &view.advisories);
    if response.cached_error {
        return error(
            state,
            &format,
            503,
            "service_unavailable",
            "Temporarily unavailable (cached error)",
            None,
            Some(
                json!({"meta":{"cached":true,"query_id":format!("ssx-{}",response.query_id),"advisories":view.advisories}}),
            ),
        );
    }
    render(
        state,
        &format,
        response,
        meta,
        if response.all_unresponsive { 503 } else { 200 },
        false,
        &portal_request,
    )
}
fn render(
    state: &HttpState,
    format: &str,
    response: &SearchResponse,
    meta: Value,
    status: u16,
    no_engines: bool,
    request: &SearchRequest,
) -> Response {
    let body = match format {
        "html" => state.html.search(response, &meta, request),
        "csv" => formats::csv(&response.results),
        "rss" => formats::rss(&response.results, &response.query),
        "yaml" => formats::yaml_document(response, &meta),
        _ => {
            let mut body = formats::json_document(response, meta);
            if no_engines {
                body["unresponsive_engines"] = json!([["all", "no engines available"]]);
            }
            body.to_string()
        }
    };
    reply(status, format, body)
}
async fn metrics(State(state): State<HttpState>) -> Response {
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        state.telemetry.metrics.render(),
    )
        .into_response()
}
async fn config(State(state): State<HttpState>) -> Response {
    reply(200, "json", state.discovery.to_string())
}
async fn health(State(state): State<HttpState>) -> Response {
    let connected = (state.valkey_connected)();
    let overall = if state.valkey_quota_configured && !connected && state.valkey_fail_closed {
        "degraded"
    } else {
        "ok"
    };
    let now = WallClockTimestamp::new(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64(),
    )
    .expect("system clock");
    let health = state.search.health();
    let mut engines = json!({});
    for (name, context) in &state.health_contexts {
        let record = build_engine_health(health.get(name), Some(context), now, 300.)
            .unwrap_or_else(|_| json!({"status":"unknown"}));
        engines[name.as_str()] = record;
    }
    reply(
        200,
        "json",
        json!({"status":overall,"version":"0.1.0","valkey_connected":connected,"engines":engines})
            .to_string(),
    )
}

#[cfg(test)]
mod source_grammar_tests {
    use super::*;
    #[tokio::test]
    async fn completed_search_failure_survives_unwind_and_cancellation() {
        let telemetry = Telemetry::new(
            ssx_infra::observability::Options {
                audit: false,
                ..Default::default()
            },
            None,
            vec![],
            vec![],
        )
        .unwrap();
        let cloned = telemetry.clone();
        let panic = tokio::spawn(async move {
            let _completion = SearchCompletion {
                telemetry: cloned,
                start: Instant::now(),
                outcome: Some("failure"),
            };
            panic!("bounded injected handler failure");
        });
        assert!(panic.await.unwrap_err().is_panic());
        let cloned = telemetry.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _completion = SearchCompletion {
                telemetry: cloned,
                start: Instant::now(),
                outcome: Some("failure"),
            };
            let _ = tx.send(());
            std::future::pending::<()>().await;
        });
        rx.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let output = telemetry.metrics.render();
        assert!(
            output
                .lines()
                .any(|l| l == "slopsearx_http_search_completed_total{outcome=\"failure\"} 2")
        );
        assert!(output.lines().any(|l|l=="slopsearx_http_search_duration_seconds_count{outcome=\"failure\"} 2"));
        telemetry.shutdown().await;
    }
    #[tokio::test]
    async fn middleware_exposes_request_id_and_preserves_or_generates_uuid_v4() {
        use tower::ServiceExt;
        let app = Router::new()
            .route(
                "/",
                get(|request: Request| async move {
                    request
                        .extensions()
                        .get::<RequestId>()
                        .unwrap()
                        .0
                        .to_str()
                        .unwrap()
                        .to_owned()
                }),
            )
            .layer(middleware::from_fn(request_id));
        for supplied in [None, Some("source-fixed-id")] {
            let mut builder = axum::http::Request::builder().uri("/");
            if let Some(id) = supplied {
                builder = builder.header("x-request-id", id);
            }
            let response = app
                .clone()
                .oneshot(builder.body(axum::body::Body::empty()).unwrap())
                .await
                .unwrap();
            let id = response.headers()["x-request-id"]
                .to_str()
                .unwrap()
                .to_owned();
            let body = axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            assert_eq!(body.as_ref(), id.as_bytes());
            if let Some(supplied) = supplied {
                assert_eq!(id, supplied);
            } else {
                assert_eq!(id.len(), 36);
                for position in [8, 13, 18, 23] {
                    assert_eq!(id.as_bytes()[position], b'-');
                }
                assert_eq!(id.as_bytes()[14], b'4');
                assert!(matches!(id.as_bytes()[19], b'8' | b'9' | b'a' | b'b'));
            }
        }
    }
    #[test]
    fn pure_original_accept_helper_controls_are_not_unicode_header_claims() {
        let source: Value =
            ssx_json::from_str(include_str!("../tests/accept_source_controls.json")).unwrap();
        let cases = source["records"].as_array().unwrap();
        assert_eq!(cases.len(), 16);
        for case in cases {
            assert_eq!(
                accept_format(case["input"].as_str().unwrap()),
                case["output"].as_str().unwrap(),
                "{}",
                case["id"]
            );
            println!(
                "T02_BINDING {}",
                json!({"kind":"accept","id":case["id"],"input":case["input"],"output":case["output"]})
            );
        }
    }
}
