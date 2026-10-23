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

const DEFAULT_URL: &str = "https://api.search.brave.com/res/v1/web/search";
const USER_AGENT: &str =
    "SlopSearX/0.1.0 (https://github.com/magnus919/SlopSearX; meta search engine; agent-native)";

#[derive(Clone, Debug)]
pub struct BraveConfig {
    pub base_url: String,
    pub api_key: String,
    pub timeout_ms: u64,
    pub max_results: usize,
    pub category_routing: bool,
}
impl Default for BraveConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_URL.into(),
            api_key: String::new(),
            timeout_ms: 5_000,
            max_results: 20,
            category_routing: true,
        }
    }
}

pub(super) struct BraveAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: BraveConfig,
}
impl BraveAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: BraveConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn run(&self, request: &SearchRequest) -> AdapterResponse {
        let started = Instant::now();
        match self.admission.admit("brave").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => {
                return failed(
                    EngineStatus::RateLimited,
                    "Brave request denied by rate policy",
                    started,
                );
            }
            AdmissionDecision::Unavailable => {
                return failed(
                    EngineStatus::Error,
                    "Brave rate policy unavailable",
                    started,
                );
            }
        }
        if self.config.api_key.is_empty() {
            return failed(EngineStatus::Error, "Brave API key not configured", started);
        }
        let mut url = match Url::parse(if self.config.base_url.is_empty() {
            DEFAULT_URL
        } else {
            &self.config.base_url
        }) {
            Ok(value) => value,
            Err(_) => return failed(EngineStatus::Error, "invalid Brave base URL", started),
        };
        let endpoint = if self.config.category_routing {
            endpoint(request.categories.as_deref().unwrap_or(&[]))
        } else {
            "web"
        };
        let path = url.path().to_owned();
        if path.contains("/web/") {
            url.set_path(&path.replace("/web/", &format!("/{endpoint}/")));
        } else {
            url.set_path(&format!("/res/v1/{endpoint}/search"));
        }
        url.set_query(None);
        url.set_query(Some(&super::broad_query(&[
            ("q", &request.query),
            ("count", &self.config.max_results.to_string()),
            ("safesearch", "off"),
        ])));
        let mut outbound = match Request::get(url.as_str()) {
            Ok(value) => value,
            Err(_) => return failed(EngineStatus::Error, "invalid Brave request URL", started),
        };
        outbound
            .headers
            .insert("accept", "application/json".parse().unwrap());
        outbound
            .headers
            .insert("accept-encoding", "gzip".parse().unwrap());
        outbound
            .headers
            .insert("user-agent", USER_AGENT.parse().unwrap());
        let Ok(subscription_token) = self.config.api_key.parse() else {
            return failed(
                EngineStatus::Error,
                "invalid Brave API key configuration",
                started,
            );
        };
        outbound
            .headers
            .insert("x-subscription-token", subscription_token);
        let operation_deadline = Instant::now() + Duration::from_millis(self.config.timeout_ms);
        let mut session = match self.http.session(
            Options::default(),
            tokio::time::Instant::from_std(operation_deadline),
            Cancellation::default(),
        ) {
            Ok(s) => s,
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    "Brave HTTP session unavailable",
                    started,
                );
            }
        };
        let response = match session.execute(outbound).await {
            Ok(r) => r,
            Err(HttpError::Deadline) => {
                return failed(EngineStatus::Timeout, "", started);
            }
            Err(_) => return failed(EngineStatus::Error, "Brave request failed", started),
        };
        if response.status == 429 {
            return failed(EngineStatus::RateLimited, "", started);
        }
        if response.status == 403 {
            return failed(EngineStatus::Blocked, "", started);
        }
        if !(200..300).contains(&response.status) {
            return failed(
                EngineStatus::Error,
                "Brave returned an unsuccessful status",
                started,
            );
        }
        let value: Value = match source_json::from_slice(&response.body, Some(operation_deadline))
            .await
        {
            Ok(v) => v,
            Err(source_json::Error::Deadline) => return failed(EngineStatus::Timeout, "", started),
            Err(error) => {
                return failed(
                    EngineStatus::Error,
                    &error.message("Brave", "Brave returned invalid JSON"),
                    started,
                );
            }
        };
        let Some(data) = value.as_object() else {
            return failed(
                EngineStatus::Error,
                &format!("'{}' object has no attribute 'get'", python_type(&value)),
                started,
            );
        };
        let raw = match endpoint {
            "web" => data
                .get("web")
                .and_then(Value::as_object)
                .and_then(|v| v.get("results")),
            _ => data.get("results"),
        };
        // Python's `for` accepts empty strings and mappings. Nonempty strings
        // and mappings reach item parsing and fail at `.get`; null and scalar
        // values fail at iteration itself.
        if raw.is_some_and(|value| !value.is_array() && !empty_python_iterable(value)) {
            return failed(
                EngineStatus::Error,
                &python_iteration_error(raw.unwrap()),
                started,
            );
        }
        let mut results = Vec::new();
        let mut refusal = None;
        if let Some(items) = raw.and_then(Value::as_array) {
            for item in items {
                let Some(item) = item.as_object() else {
                    return failed(
                        EngineStatus::Error,
                        &format!("'{}' object has no attribute 'get'", python_type(item)),
                        started,
                    );
                };
                let source_url = if endpoint == "images" {
                    item.get("page_url").or_else(|| item.get("url"))
                } else {
                    item.get("url")
                };
                for (field, value) in [
                    ("url", source_url),
                    ("title", item.get("title")),
                    ("content", item.get("description")),
                ] {
                    // The existing image-only null URL/title exception is retained.
                    let exception = endpoint == "images"
                        && field != "content"
                        && value.is_some_and(Value::is_null);
                    if !exception {
                        super::broad_record_refusal(
                            &mut refusal,
                            super::broad_field_error(value, false, "Brave", field),
                        );
                    }
                }
                let raw_thumb = item
                    .get("thumbnail")
                    .and_then(Value::as_object)
                    .and_then(|v| v.get("src"));
                super::broad_record_refusal(
                    &mut refusal,
                    super::broad_field_error(raw_thumb, true, "Brave", "thumbnail"),
                );
                let title = text(item.get("title"));
                let content = text(item.get("description"));
                let thumb = item
                    .get("thumbnail")
                    .and_then(Value::as_object)
                    .and_then(|v| v.get("src"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let mut result = SearchResult {
                    engine: "brave".into(),
                    title,
                    content,
                    position: results.len() as i64 + 1,
                    thumbnail: thumb.clone(),
                    ..SearchResult::default()
                };
                if endpoint == "images" {
                    let image = item.get("image").and_then(Value::as_object);
                    result.url = text(item.get("page_url").or_else(|| item.get("url")));
                    result.img_src = thumb.clone();
                    let raw_full = image.and_then(|v| v.get("src"));
                    let selected = if super::broad_truthy(raw_full) {
                        raw_full
                    } else {
                        raw_thumb
                    };
                    if super::broad_truthy(raw_full) || super::broad_truthy(raw_thumb) {
                        let selected_url = selected
                            .and_then(Value::as_text)
                            .and_then(source_json::scalar_media_url);
                        let selected_error = match selected {
                            Some(Value::String(_)) if selected_url.is_some() => None,
                            _ => super::broad_field_error(selected, true, "Brave", "media.url"),
                        };
                        super::broad_record_refusal(&mut refusal, selected_error);
                        let width = match python_int(image.and_then(|v| v.get("width")), "width") {
                            Ok(v) => v,
                            Err(e) => {
                                super::broad_record_refusal(&mut refusal, Some(e));
                                None
                            }
                        };
                        let height = match python_int(image.and_then(|v| v.get("height")), "height")
                        {
                            Ok(v) => v,
                            Err(e) => {
                                super::broad_record_refusal(&mut refusal, Some(e));
                                None
                            }
                        };
                        result.media = ssx_domain::storage::build_media(
                            "image",
                            None,
                            raw_thumb.and_then(Value::as_str).filter(|v| !v.is_empty()),
                            Some(&result.url),
                            width,
                            height,
                            None,
                        );
                        if let Some(media) = result.media.as_mut() {
                            media.url = selected_url;
                        }
                    }
                } else {
                    result.url = text(item.get("url"));
                    if endpoint == "videos" && super::broad_truthy(raw_thumb) {
                        let duration = match python_number(item.get("duration")) {
                            Ok(v) => v,
                            Err(e) => {
                                super::broad_record_refusal(&mut refusal, Some(e.into()));
                                None
                            }
                        };
                        result.media = ssx_domain::storage::build_media(
                            "video",
                            None,
                            raw_thumb.and_then(Value::as_str),
                            Some(&result.url),
                            None,
                            None,
                            duration,
                        );
                    }
                }
                results.push(result);
            }
        }
        let mut answers = Vec::new();
        if let Some(rows) = data
            .get("infobox")
            .and_then(Value::as_object)
            .and_then(|v| v.get("results"))
            .and_then(Value::as_array)
        {
            for item in rows {
                if let Some(obj) = item.as_object()
                    && let Err(e) = push_answer(
                        &mut answers,
                        obj.get("url"),
                        obj.get("title"),
                        obj.get("description"),
                        &mut refusal,
                    )
                {
                    return failed(EngineStatus::Error, &e, started);
                }
            }
        }
        let mixed_main = data
            .get("mixed")
            .and_then(Value::as_object)
            .and_then(|mixed| mixed.get("main"));
        if mixed_main.is_some_and(|main| {
            !matches!(main, Value::Array(_) | Value::Object(_) | Value::String(_))
        }) {
            return failed(
                EngineStatus::Error,
                &python_iteration_error(mixed_main.unwrap()),
                started,
            );
        }
        if let Some(rows) = data
            .get("mixed")
            .and_then(Value::as_object)
            .and_then(|v| v.get("main"))
            .and_then(Value::as_array)
        {
            for row in rows {
                if let Some(obj) = row.as_object()
                    && obj.get("type").and_then(Value::as_str) == Some("answer")
                    && let Some(ans) = obj.get("data").and_then(Value::as_object)
                    && let Err(e) = push_answer(
                        &mut answers,
                        ans.get("url"),
                        ans.get("title"),
                        ans.get("description"),
                        &mut refusal,
                    )
                {
                    return failed(EngineStatus::Error, &e, started);
                }
            }
        }
        if let Some(error) = refusal {
            return failed(EngineStatus::Error, &error, started);
        }
        let answers = match answers
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
                            "Brave answer must contain only finite JSON numbers"
                        }
                        ProjectionError::Unicode => {
                            "Brave answer must contain only Unicode scalar strings"
                        }
                    },
                    started,
                );
            }
        };
        AdapterResponse {
            results,
            answers,
            status: EngineStatus::Ok,
            latency_ms: elapsed(started),
            ..AdapterResponse::default()
        }
    }
}
impl SearchEngine for BraveAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move { self.run(request).await })
    }
}
fn endpoint(categories: &[String]) -> &'static str {
    for c in categories {
        match c.as_str() {
            "images" => return "images",
            "news" => return "news",
            "videos" | "video" => return "videos",
            _ => (),
        }
    }
    "web"
}
fn text(value: Option<&Value>) -> String {
    value.and_then(Value::as_str).unwrap_or_default().to_owned()
}
fn python_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) => {
            if n.is_f64() {
                "float"
            } else {
                "int"
            }
        }
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}
fn python_iteration_error(value: &Value) -> String {
    if matches!(value, Value::String(_) | Value::Object(_)) {
        "'str' object has no attribute 'get'".into()
    } else {
        format!("'{}' object is not iterable", python_type(value))
    }
}
fn empty_python_iterable(value: &Value) -> bool {
    match value {
        Value::String(value) => value.is_empty(),
        Value::Object(value) => value.is_empty(),
        _ => false,
    }
}
fn python_int(value: Option<&Value>, field: &str) -> Result<Option<i64>, String> {
    match value {
        Some(Value::Bool(v)) => Ok(Some(i64::from(*v))),
        Some(Value::Number(n)) if !n.is_f64() => n
            .as_i64()
            .map(Some)
            .ok_or_else(|| format!("Brave image {field} must fit a signed 64-bit integer")),
        _ => Ok(None),
    }
}
fn python_number(value: Option<&Value>) -> Result<Option<FiniteF64>, &'static str> {
    const ERROR: &str = "Brave video duration must be a finite non-boolean number";
    match value {
        Some(Value::Bool(_)) => Err(ERROR),
        Some(Value::Number(n)) => {
            let canonical = if n.is_f64() {
                let text = ssx_json::python_float(n.as_f64().ok_or(ERROR)?).map_err(|_| ERROR)?;
                ssx_json::Number::new(&text).map_err(|_| ERROR)?
            } else {
                n.project().map_err(|_| ERROR)?
            };
            FiniteF64::from_number(canonical)
                .map(Some)
                .map_err(|_| ERROR)
        }
        _ => Ok(None),
    }
}
fn push_answer(
    out: &mut Vec<Value>,
    url: Option<&Value>,
    title: Option<&Value>,
    description: Option<&Value>,
    refusal: &mut Option<String>,
) -> Result<(), String> {
    if super::broad_truthy(description) {
        let content = match description.unwrap() {
            Value::String(s) => Value::String(s.take(500)),
            Value::Array(a) => Value::Array(a.iter().take(500).cloned().collect()),
            Value::Object(_) => return Err("slice(None, 500, None)".into()),
            other => {
                return Err(format!(
                    "'{}' object is not subscriptable",
                    python_type(other)
                ));
            }
        };
        let mut m = Map::new();
        m.insert(
            "url".into(),
            url.cloned().unwrap_or(Value::String(Text::from(""))),
        );
        m.insert(
            "title".into(),
            title.cloned().unwrap_or(Value::String(Text::from(""))),
        );
        m.insert("content".into(), content);
        let answer = Value::Object(m);
        super::broad_record_refusal(
            refusal,
            source_json::opaque_error(&answer, "Brave", "answer"),
        );
        out.push(answer);
    }
    Ok(())
}
fn failed(status: EngineStatus, message: &str, started: Instant) -> AdapterResponse {
    AdapterResponse {
        status,
        error_message: (!message.is_empty()).then(|| message.into()),
        latency_ms: elapsed(started),
        ..AdapterResponse::default()
    }
}
fn elapsed(started: Instant) -> FiniteF64 {
    FiniteF64::new(started.elapsed().as_secs_f64() * 1000.0).unwrap_or_default()
}
