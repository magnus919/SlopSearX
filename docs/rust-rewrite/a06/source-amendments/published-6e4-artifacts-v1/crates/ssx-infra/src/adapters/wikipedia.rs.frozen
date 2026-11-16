use super::super::http::{AdapterHttp, Cancellation, Options, Request};
use super::source_json::{self, Map, ProjectionError, Text, Value};
use crate::http::Error as HttpError;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use url::Url;

const DEFAULT_URL: &str = "https://en.wikipedia.org/w/api.php";
const DEFAULT_UA: &str =
    "SlopSearX/0.1.0 (https://github.com/magnus919/SlopSearX; meta search engine; agent-native)";
#[derive(Clone, Debug)]
pub struct WikipediaConfig {
    pub base_url: String,
    pub user_agent: String,
    pub timeout_ms: u64,
    pub max_results: usize,
}
impl Default for WikipediaConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_URL.into(),
            user_agent: String::new(),
            timeout_ms: 3000,
            max_results: 3,
        }
    }
}
pub(super) struct WikipediaAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: WikipediaConfig,
}
impl WikipediaAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: WikipediaConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn run(&self, request: &SearchRequest) -> AdapterResponse {
        let started = Instant::now();
        match self.admission.admit("wikipedia").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => {
                return failed(
                    EngineStatus::RateLimited,
                    "Wikipedia request denied by rate policy",
                    started,
                );
            }
            AdmissionDecision::Unavailable => {
                return failed(
                    EngineStatus::Error,
                    "Wikipedia rate policy unavailable",
                    started,
                );
            }
        }
        let base = if self.config.base_url.is_empty() {
            DEFAULT_URL
        } else {
            &self.config.base_url
        };
        let ua = if self.config.user_agent.is_empty() {
            DEFAULT_UA
        } else {
            &self.config.user_agent
        };
        let mut session = match self.http.session(
            Options::default(),
            tokio::time::Instant::now() + Duration::from_millis(self.config.timeout_ms),
            Cancellation::default(),
        ) {
            Ok(s) => s,
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    "Wikipedia HTTP session unavailable",
                    started,
                );
            }
        };
        let first = match get(
            base,
            &[
                ("action", "opensearch"),
                ("search", &request.query),
                ("limit", &self.config.max_results.to_string()),
                ("format", "json"),
                ("origin", "*"),
            ],
            ua,
        ) {
            Ok(r) => r,
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    "invalid Wikipedia request URL",
                    started,
                );
            }
        };
        let stage_deadline = Instant::now() + Duration::from_millis(self.config.timeout_ms);
        session.set_deadline(tokio::time::Instant::from_std(stage_deadline));
        let response = match session.execute(first).await {
            Ok(r) => r,
            Err(HttpError::Deadline) => {
                return failed(
                    EngineStatus::Timeout,
                    &diagnostic("opensearch", None, "unavailable", "timeout"),
                    started,
                );
            }
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    &diagnostic("opensearch", None, "unavailable", "transport error"),
                    started,
                );
            }
        };
        if !(200..300).contains(&response.status) {
            return failed(
                http_status(response.status),
                &diagnostic(
                    "opensearch",
                    Some(&response),
                    "unparsed",
                    "provider HTTP error",
                ),
                started,
            );
        }
        let value: Value = match source_json::from_slice(&response.body, Some(stage_deadline)).await
        {
            Ok(v) => v,
            Err(source_json::Error::Deadline) => {
                return failed(
                    EngineStatus::Timeout,
                    &diagnostic("opensearch", None, "unavailable", "timeout"),
                    started,
                );
            }
            Err(
                error @ (source_json::Error::ByteLimit
                | source_json::Error::DepthLimit
                | source_json::Error::TextLimit),
            ) => {
                return failed(
                    EngineStatus::Error,
                    &error.message("Wikipedia", ""),
                    started,
                );
            }
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    &diagnostic(
                        "opensearch",
                        Some(&response),
                        "malformed_json",
                        "malformed JSON",
                    ),
                    started,
                );
            }
        };
        let Some(arr) = value.as_array() else {
            return failed(
                EngineStatus::Error,
                &diagnostic(
                    "opensearch",
                    Some(&response),
                    &shape_signature(&value),
                    "unexpected JSON shape",
                ),
                started,
            );
        };
        if arr.len() < 2 || arr[1].as_array().is_none() {
            return failed(
                EngineStatus::Error,
                &diagnostic(
                    "opensearch",
                    Some(&response),
                    &shape_signature(&value),
                    "unexpected JSON shape",
                ),
                started,
            );
        }
        let source_titles = arr[1]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_text)
            .take(self.config.max_results)
            .cloned()
            .collect::<Vec<_>>();
        let joined = Text::concat(
            &source_titles
                .iter()
                .enumerate()
                .flat_map(|(i, title)| {
                    if i == 0 {
                        vec![title.clone()]
                    } else {
                        vec![Text::from("|"), title.clone()]
                    }
                })
                .collect::<Vec<_>>(),
        );
        if let Some(error) = joined.utf8_error() {
            return failed(EngineStatus::Error, &error, started);
        }
        let titles = source_titles
            .iter()
            .map(|t| t.scalar().expect("UTF8 checked"))
            .collect::<Vec<_>>();
        if titles.is_empty() {
            return AdapterResponse {
                status: EngineStatus::Ok,
                latency_ms: elapsed(started),
                ..AdapterResponse::default()
            };
        }
        let correction = correction(&request.query, &titles);
        let title_join = titles.join("|");
        let rich = match get(
            base,
            &[
                ("action", "query"),
                ("titles", &title_join),
                ("prop", "extracts|pageimages|pageprops"),
                ("exintro", "1"),
                ("explaintext", "1"),
                ("pithumbsize", "300"),
                ("format", "json"),
                ("origin", "*"),
            ],
            ua,
        ) {
            Ok(r) => r,
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    "invalid Wikipedia request URL",
                    started,
                );
            }
        };
        let stage_deadline = Instant::now() + Duration::from_millis(self.config.timeout_ms);
        session.set_deadline(tokio::time::Instant::from_std(stage_deadline));
        let response = match session.execute(rich).await {
            Ok(r) => r,
            Err(HttpError::Deadline) => {
                return failed(
                    EngineStatus::Timeout,
                    &diagnostic("rich_query", None, "unavailable", "timeout"),
                    started,
                );
            }
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    &diagnostic("rich_query", None, "unavailable", "transport error"),
                    started,
                );
            }
        };
        if !(200..300).contains(&response.status) {
            return failed(
                http_status(response.status),
                &diagnostic(
                    "rich_query",
                    Some(&response),
                    "unparsed",
                    "provider HTTP error",
                ),
                started,
            );
        }
        let value: Value = match source_json::from_slice(&response.body, Some(stage_deadline)).await
        {
            Ok(v) => v,
            Err(source_json::Error::Deadline) => {
                return failed(
                    EngineStatus::Timeout,
                    &diagnostic("rich_query", None, "unavailable", "timeout"),
                    started,
                );
            }
            Err(
                error @ (source_json::Error::ByteLimit
                | source_json::Error::DepthLimit
                | source_json::Error::TextLimit),
            ) => {
                return failed(
                    EngineStatus::Error,
                    &error.message("Wikipedia", ""),
                    started,
                );
            }
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    &diagnostic(
                        "rich_query",
                        Some(&response),
                        "malformed_json",
                        "malformed JSON",
                    ),
                    started,
                );
            }
        };
        let Some(pages) = value
            .as_object()
            .and_then(|v| v.get("query"))
            .and_then(Value::as_object)
            .and_then(|v| v.get("pages"))
            .and_then(Value::as_object)
        else {
            return failed(
                EngineStatus::Error,
                &diagnostic(
                    "rich_query",
                    Some(&response),
                    &shape_signature(&value),
                    "unexpected JSON shape",
                ),
                started,
            );
        };
        let mut results = Vec::new();
        let mut refusal = None;
        let mut infoboxes = Vec::new();
        for (index, (page_id, page)) in pages.iter().enumerate() {
            if page_id.as_str() == Some("-1") {
                continue;
            }
            let Some(page) = page.as_object() else {
                return failed(
                    EngineStatus::Error,
                    &format!(
                        "'{}' object has no attribute 'get'",
                        super::broad_type(page)
                    ),
                    started,
                );
            };
            let empty = Value::String(Text::from(""));
            let extract = page.get("extract").unwrap_or(&empty);
            let Some(extract) = extract.as_text() else {
                return failed(
                    EngineStatus::Error,
                    &format!(
                        "expected string or bytes-like object, got '{}'",
                        super::broad_type(extract)
                    ),
                    started,
                );
            };
            let clean = extract.collapse_whitespace();
            // Python cleans extract before invoking title.replace.
            let title = page.get("title").unwrap_or(&empty);
            let Some(title) = title.as_text() else {
                return failed(
                    EngineStatus::Error,
                    &format!(
                        "'{}' object has no attribute 'replace'",
                        super::broad_type(title)
                    ),
                    started,
                );
            };
            let title = title.clone();
            let content = if clean.len() > 200 {
                Text::concat(&[clean.take(200), Text::from("…")])
            } else {
                clean
            };
            let page_url = Text::concat(&[
                Text::from("https://en.wikipedia.org/wiki/"),
                title.replace_ascii(32, 95),
            ]);
            let page_url = super::broad_scalar_output(&page_url, "Wikipedia", "url", &mut refusal);
            let title = super::broad_scalar_output(&title, "Wikipedia", "title", &mut refusal);
            let content =
                super::broad_scalar_output(&content, "Wikipedia", "content", &mut refusal);
            let raw_thumb = page
                .get("thumbnail")
                .and_then(Value::as_object)
                .and_then(|v| v.get("source"));
            super::broad_record_refusal(
                &mut refusal,
                super::broad_field_error(raw_thumb, true, "Wikipedia", "thumbnail"),
            );
            let thumb = page
                .get("thumbnail")
                .and_then(Value::as_object)
                .and_then(|v| v.get("source"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            results.push(SearchResult {
                url: page_url.clone(),
                title: title.clone(),
                content,
                engine: "wikipedia".into(),
                position: index as i64 + 1,
                thumbnail: thumb.clone(),
                ..SearchResult::default()
            });
            if let Some(short) = page
                .get("pageprops")
                .and_then(Value::as_object)
                .and_then(|v| v.get("wikibase-shortdesc"))
                .filter(|v| super::broad_truthy(Some(v)))
            {
                let mut info = Map::new();
                info.insert(
                    "id".into(),
                    Value::String(Text::from(format!("wiki:{}", title.replace(" ", "_")))),
                );
                info.insert("title".into(), Value::String(Text::from(title.clone())));
                info.insert("content".into(), short.clone());
                info.insert(
                    "img_src".into(),
                    Value::String(Text::from(thumb.unwrap_or_default())),
                );
                info.insert("url".into(), Value::String(Text::from(page_url.clone())));
                let mut source = Map::new();
                source.insert("title".into(), Value::String("Wikipedia".into()));
                source.insert("url".into(), Value::String(Text::from(page_url)));
                info.insert("urls".into(), Value::Array(vec![Value::Object(source)]));
                let info = Value::Object(info);
                super::broad_record_refusal(
                    &mut refusal,
                    source_json::opaque_error(&info, "Wikipedia", "infobox"),
                );
                infoboxes.push(info);
            }
        }
        if let Some(error) = refusal {
            return failed(EngineStatus::Error, &error, started);
        }
        let infoboxes = match infoboxes
            .iter()
            .map(Value::project)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(values) => values,
            Err(error) => {
                return failed(
                    EngineStatus::Error,
                    match error {
                        ProjectionError::Nonfinite => {
                            "Wikipedia infobox must contain only finite JSON numbers"
                        }
                        ProjectionError::Unicode => {
                            "Wikipedia infobox must contain only Unicode scalar strings"
                        }
                    },
                    started,
                );
            }
        };
        AdapterResponse {
            results,
            status: EngineStatus::Ok,
            latency_ms: elapsed(started),
            corrections: correction.into_iter().collect(),
            infoboxes,
            ..AdapterResponse::default()
        }
    }
}
impl SearchEngine for WikipediaAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move { self.run(request).await })
    }
}
fn get(base: &str, params: &[(&str, &str)], ua: &str) -> Result<Request, ()> {
    let mut url = Url::parse(base).map_err(|_| ())?;
    url.set_query(None);
    url.set_query(Some(&super::broad_query(params)));
    let mut r = Request::get(url.as_str()).map_err(|_| ())?;
    r.headers.insert("user-agent", ua.parse().map_err(|_| ())?);
    Ok(r)
}
fn correction(query: &str, titles: &[String]) -> Option<String> {
    let first = titles.first()?;
    let f = first.trim_matches(super::broad_whitespace).to_lowercase();
    let q = query.trim_matches(super::broad_whitespace).to_lowercase();
    if f != q && !f.contains(&q) && !q.contains(&f) {
        Some(first.clone())
    } else {
        None
    }
}
fn http_status(code: u16) -> EngineStatus {
    match code {
        429 => EngineStatus::RateLimited,
        403 => EngineStatus::Blocked,
        _ => EngineStatus::Error,
    }
}
fn diagnostic(
    stage: &str,
    response: Option<&super::super::http::Response>,
    shape: &str,
    detail: &str,
) -> String {
    let status = response
        .map(|r| r.status.to_string())
        .unwrap_or_else(|| "unknown".into());
    let mime_header = response
        .and_then(|r| r.headers.get("content-type"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    let mime = mime_header
        .split(';')
        .next()
        .unwrap_or("")
        .trim_matches(super::broad_whitespace)
        .to_ascii_lowercase();
    let valid = mime.len() <= 64
        && mime
            .split_once('/')
            .is_some_and(|(major, minor)| valid_mime_token(major) && valid_mime_token(minor));
    let mime = if mime.is_empty() {
        "unknown".into()
    } else if valid {
        mime
    } else {
        "invalid".into()
    };
    format!(
        "Wikipedia stage={stage} failed: status={status}; content_type={mime}; response_shape={shape}; {detail}"
    )
}
fn shape_signature(value: &Value) -> String {
    match value {
        Value::Array(a) => format!(
            "list(len={}{})",
            a.len().min(1000),
            if a.len() > 1000 { "+" } else { "" }
        ),
        Value::Object(o) => format!(
            "object(keys={}{})",
            o.len().min(1000),
            if o.len() > 1000 { "+" } else { "" }
        ),
        Value::Null => "null".into(),
        Value::Bool(_) => "bool".into(),
        Value::Number(n) => {
            if !n.is_f64() {
                "int".into()
            } else {
                "float".into()
            }
        }
        Value::String(_) => "str".into(),
    }
}
fn valid_mime_token(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"!#$&^_.+-".contains(&b))
}
fn failed(status: EngineStatus, msg: &str, started: Instant) -> AdapterResponse {
    AdapterResponse {
        status,
        error_message: Some(msg.into()),
        latency_ms: elapsed(started),
        ..AdapterResponse::default()
    }
}
fn elapsed(started: Instant) -> FiniteF64 {
    FiniteF64::new(started.elapsed().as_secs_f64() * 1000.0).unwrap_or_default()
}
