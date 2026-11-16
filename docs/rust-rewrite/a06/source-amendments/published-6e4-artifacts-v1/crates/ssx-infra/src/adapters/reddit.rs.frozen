use super::super::http::{AdapterHttp, Cancellation, Options, Request};
use super::source_json::{self, Text, Value};
use crate::http::Error as HttpError;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use url::Url;

const DEFAULT_URL: &str = "https://www.reddit.com";
const USER_AGENT: &str = "SlopSearX/0.1.0 (meta search engine; agent-native; by /u/SlopSearX)";
#[derive(Clone, Debug)]
pub struct RedditConfig {
    pub base_url: String,
    pub access_token: String,
    pub api_key: String,
    pub timeout_ms: u64,
    pub max_results: usize,
}
impl Default for RedditConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_URL.into(),
            access_token: String::new(),
            api_key: String::new(),
            timeout_ms: 5000,
            max_results: 10,
        }
    }
}
pub(super) struct RedditAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: RedditConfig,
}
impl RedditAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: RedditConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn run(&self, request: &SearchRequest) -> AdapterResponse {
        let started = Instant::now();
        match self.admission.admit("reddit").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => {
                return failed(
                    EngineStatus::RateLimited,
                    "Reddit request denied by rate policy",
                    started,
                );
            }
            AdmissionDecision::Unavailable => {
                return failed(
                    EngineStatus::Error,
                    "Reddit rate policy unavailable",
                    started,
                );
            }
        }
        let base = if self.config.base_url.is_empty() {
            DEFAULT_URL
        } else {
            &self.config.base_url
        };
        let base = base.trim_end_matches('/');
        // The Python adapter selects by raw truthiness, then strips the selected
        // value. A whitespace access token therefore suppresses api_key fallback.
        let selected_token = if !self.config.access_token.is_empty() {
            &self.config.access_token
        } else {
            &self.config.api_key
        };
        let token = selected_token.trim_matches(super::broad_whitespace);
        let host = if !token.is_empty() && base == DEFAULT_URL {
            "https://oauth.reddit.com"
        } else {
            base
        };
        let subreddit = request
            .categories
            .as_ref()
            .is_some_and(|v| v.iter().any(|x| x == "reddit:subreddit"));
        let sub = request.subreddit.as_deref().unwrap_or("all");
        let path = if subreddit {
            format!(
                "/r/{sub}/search{}",
                if token.is_empty() { ".json" } else { "" }
            )
        } else {
            format!("/search{}", if token.is_empty() { ".json" } else { "" })
        };
        let mut url = match Url::parse(&format!("{host}{path}")) {
            Ok(v) => v,
            Err(_) => return failed(EngineStatus::Error, "invalid Reddit base URL", started),
        };
        url.set_query(Some(&super::broad_query(&[
            ("q", &request.query),
            ("limit", &self.config.max_results.to_string()),
            ("sort", "relevance"),
            ("t", "all"),
            ("raw_json", "1"),
        ])));
        let mut outbound = match Request::get(url.as_str()) {
            Ok(v) => v,
            Err(_) => return failed(EngineStatus::Error, "invalid Reddit request URL", started),
        };
        outbound
            .headers
            .insert("user-agent", USER_AGENT.parse().unwrap());
        outbound
            .headers
            .insert("accept", "application/json".parse().unwrap());
        if !token.is_empty() {
            let Ok(value) = format!("Bearer {token}").parse() else {
                return failed(
                    EngineStatus::Error,
                    "invalid Reddit access token configuration",
                    started,
                );
            };
            outbound.headers.insert("authorization", value);
        }
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
                    "Reddit HTTP session unavailable",
                    started,
                );
            }
        };
        let response = match session.execute(outbound).await {
            Ok(v) => v,
            Err(HttpError::Deadline) => {
                return failed(EngineStatus::Timeout, "", started);
            }
            Err(_) => return failed(EngineStatus::Error, "Reddit request failed", started),
        };
        if response.status == 429 {
            return failed(EngineStatus::RateLimited, "rate limited by Reddit", started);
        }
        if response.status == 403 {
            return failed(
                EngineStatus::Blocked,
                if token.is_empty() {
                    "Reddit blocked unauthenticated API access; set ENGINE_REDDIT_API_KEY to an approved OAuth access token for hosted/provider IPs"
                } else {
                    "Reddit blocked the authenticated request; verify OAuth approval, scope, and access policy"
                },
                started,
            );
        }
        if response.status == 401 {
            return failed(
                EngineStatus::Unavailable,
                "Reddit rejected the OAuth access token; configure a valid approved token",
                started,
            );
        }
        if !(200..300).contains(&response.status) {
            return failed(
                EngineStatus::Error,
                "Reddit returned an unsuccessful status",
                started,
            );
        }
        let value: Value = match source_json::from_slice(&response.body, Some(operation_deadline))
            .await
        {
            Ok(v) => v,
            Err(source_json::Error::Deadline) => return failed(EngineStatus::Timeout, "", started),
            Err(source_json::Error::IntegerDigits(_)) => {
                return failed(EngineStatus::Error, "Reddit returned invalid JSON", started);
            }
            Err(error) => {
                return failed(
                    EngineStatus::Error,
                    &error.message("Reddit", "Reddit returned invalid JSON"),
                    started,
                );
            }
        };
        if value
            .as_object()
            .and_then(|v| v.get("data"))
            .and_then(Value::as_object)
            .is_none()
        {
            return failed(
                EngineStatus::Error,
                "Reddit returned an invalid listing shape",
                started,
            );
        }
        let Some(children) = value
            .as_object()
            .and_then(|v| v.get("data"))
            .and_then(Value::as_object)
            .and_then(|v| v.get("children"))
            .and_then(Value::as_array)
        else {
            return failed(
                EngineStatus::Error,
                "Reddit returned an invalid listing children field",
                started,
            );
        };
        let mut results = Vec::new();
        let mut refusal = None;
        for child in children {
            let Some(child) = child.as_object() else {
                return failed(
                    EngineStatus::Error,
                    &format!("'{}' object has no attribute 'get'", py_type(child)),
                    started,
                );
            };
            let Some(data) = child.get("data").and_then(Value::as_object) else {
                continue;
            };
            if truthy(data.get("over_18")) {
                continue;
            }
            if !truthy(data.get("title")) {
                continue;
            }
            super::broad_record_refusal(
                &mut refusal,
                super::broad_field_error(data.get("title"), false, "Reddit", "title"),
            );
            let title = text(data.get("title"));
            let empty = Value::String(Text::from(""));
            let permalink = data.get("permalink").unwrap_or(&empty);
            let raw_url = data.get("url").unwrap_or(&empty);
            let fallback = if !truthy(Some(raw_url)) {
                true
            } else {
                let Some(url) = raw_url.as_text() else {
                    return failed(
                        EngineStatus::Error,
                        &format!(
                            "'{}' object has no attribute 'startswith'",
                            py_type(raw_url)
                        ),
                        started,
                    );
                };
                let Some(permalink) = permalink.as_text() else {
                    return failed(
                        EngineStatus::Error,
                        &format!(
                            "startswith first arg must be str or a tuple of str, not {}",
                            py_type(permalink)
                        ),
                        started,
                    );
                };
                url.starts_with(permalink) || !url.starts_with_ascii("http")
            };
            let url = if fallback {
                if truthy(Some(permalink)) {
                    Text::concat(&[
                        Text::from("https://www.reddit.com"),
                        permalink.python_string(),
                    ])
                } else {
                    Text::from("")
                }
            } else {
                raw_url.as_text().unwrap().clone()
            };
            let selftext = data.get("selftext").unwrap_or(&empty);
            let selftext = if truthy(Some(selftext)) {
                match selftext {
                    Value::String(s) => s.take(500),
                    Value::Object(_) => {
                        return failed(EngineStatus::Error, "slice(None, 500, None)", started);
                    }
                    Value::Array(_) => {
                        super::broad_record_refusal(
                            &mut refusal,
                            Some("Reddit selftext must be a string when nonempty".into()),
                        );
                        Text::from("")
                    }
                    other => {
                        return failed(
                            EngineStatus::Error,
                            &format!("'{}' object is not subscriptable", py_type(other)),
                            started,
                        );
                    }
                }
            } else {
                Text::from("")
            };
            let score_value = data.get("score").cloned().unwrap_or(Value::integer(0));
            let comments_value = data
                .get("num_comments")
                .cloned()
                .unwrap_or(Value::integer(0));
            let author_value = data
                .get("author")
                .cloned()
                .unwrap_or(Value::String(Text::from("[deleted]")));
            let content = if !selftext.is_empty() {
                selftext.take(500)
            } else {
                Text::concat(&[
                    Text::from("Score: "),
                    score_value.python_string(),
                    Text::from(" | Comments: "),
                    comments_value.python_string(),
                    Text::from(" | Author: "),
                    author_value.python_string(),
                ])
            };
            let subreddit = data
                .get("subreddit")
                .filter(|v| truthy(Some(v)))
                .map(Value::python_string)
                .unwrap_or_else(|| Text::from(""));
            let category = if subreddit.is_empty() {
                Text::from("social")
            } else {
                Text::concat(&[Text::from("social:"), subreddit])
            };
            let url = super::broad_scalar_output(&url, "Reddit", "url", &mut refusal);
            let content = super::broad_scalar_output(&content, "Reddit", "content", &mut refusal);
            let category =
                super::broad_scalar_output(&category, "Reddit", "category", &mut refusal);
            let date_value = data
                .get("created_utc")
                .cloned()
                .unwrap_or(Value::integer(0));
            let published = if truthy(Some(&date_value)) {
                let Some(epoch) = py_int(&date_value) else {
                    return failed(EngineStatus::Error, &date_error(&date_value), started);
                };
                let Some(date) = epoch_iso(epoch) else {
                    return failed(EngineStatus::Error, &date_error(&date_value), started);
                };
                Some(date)
            } else {
                None
            };
            let Some(score_number) = py_float(&score_value) else {
                return failed(EngineStatus::Error, &score_error(&score_value), started);
            };
            let score = match FiniteF64::new(score_number) {
                Ok(score) => score,
                Err(_) => {
                    super::broad_record_refusal(
                        &mut refusal,
                        Some("Reddit score must be finite".into()),
                    );
                    FiniteF64::default()
                }
            };
            let raw_thumb = data.get("thumbnail").filter(|v| {
                truthy(Some(v)) && !matches!(v.as_str(), Some("self" | "default" | "nsfw" | ""))
            });
            super::broad_record_refusal(
                &mut refusal,
                super::broad_field_error(raw_thumb, true, "Reddit", "thumbnail"),
            );
            let thumbnail = data
                .get("thumbnail")
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty() && !matches!(*v, "self" | "default" | "nsfw"))
                .map(str::to_owned);
            results.push(SearchResult {
                url,
                title,
                content,
                engine: "reddit".into(),
                score,
                position: results.len() as i64 + 1,
                category,
                published_date: published,
                thumbnail,
                ..SearchResult::default()
            });
        }
        if let Some(error) = refusal {
            return failed(EngineStatus::Error, &error, started);
        }
        AdapterResponse {
            results,
            status: EngineStatus::Ok,
            latency_ms: elapsed(started),
            ..AdapterResponse::default()
        }
    }
}
impl SearchEngine for RedditAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move { self.run(request).await })
    }
}
fn text(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or_default().to_owned()
}
fn truthy(value: Option<&Value>) -> bool {
    super::broad_truthy(value)
}
fn number_f64(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| {
        ssx_domain::storage::normalize_python_number_text(v.as_str()?)
            .ok()?
            .parse()
            .ok()
    })
}
fn py_float(v: &Value) -> Option<f64> {
    match v {
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Number(_) | Value::String(_) => number_f64(v),
        _ => None,
    }
}
fn py_int(v: &Value) -> Option<i64> {
    match v {
        Value::Bool(b) => Some(i64::from(*b)),
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_u64().and_then(|x| i64::try_from(x).ok()))
            .or_else(|| {
                n.is_f64().then(|| n.as_f64()).flatten().and_then(|x| {
                    (x.is_finite() && x >= i64::MIN as f64 && x < i64::MAX as f64)
                        .then(|| x.trunc() as i64)
                })
            }),
        Value::String(s) => ssx_domain::storage::normalize_python_number_text(s.as_str()?)
            .ok()?
            .parse()
            .ok(),
        _ => None,
    }
}
fn py_type(value: &Value) -> &'static str {
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
fn score_error(value: &Value) -> String {
    match value {
        Value::Number(n) if !n.is_f64() => "int too large to convert to float".into(),
        Value::String(s) => {
            format!("could not convert string to float: {}", s.repr())
        }
        _ => format!(
            "float() argument must be a string or a real number, not '{}'",
            py_type(value)
        ),
    }
}
fn date_error(value: &Value) -> String {
    if let Value::Number(source_json::Number::Float(f)) = value {
        if f.is_nan() {
            return "cannot convert float NaN to integer".into();
        }
        if f.is_infinite() {
            return "cannot convert float infinity to integer".into();
        }
    }
    if let Value::String(s) = value {
        let valid_integer = s
            .as_str()
            .and_then(|s| ssx_domain::storage::normalize_python_number_text(s).ok())
            .is_some_and(|s| {
                let digits = s.strip_prefix(['+', '-']).unwrap_or(&s);
                !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit())
            });
        if !valid_integer {
            return format!("invalid literal for int() with base 10: {}", s.repr());
        }
    } else if !matches!(value, Value::Number(_) | Value::Bool(_)) {
        return format!(
            "int() argument must be a string, a bytes-like object or a real number, not '{}'",
            py_type(value)
        );
    }
    let Some(epoch) = py_int(value) else {
        return "timestamp out of range for platform time_t".into();
    };
    if !(-67_768_040_609_740_800..=67_768_036_191_676_799).contains(&epoch) {
        "[Errno 84] Value too large to be stored in data type".into()
    } else {
        "year out of range".into()
    }
}
fn epoch_iso(secs: i64) -> Option<String> {
    // Exact macOS source-runtime gmtime bounds: tm_year is signed i32,
    // relative to 1900. CPython additionally rejects calendar years > i32::MAX.
    if !(-67_768_040_609_740_800..=67_767_976_233_532_799).contains(&secs) {
        return None;
    }
    let (year, month, day) = civil_date(secs.div_euclid(86400));
    let within = secs.rem_euclid(86400);
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        within / 3600,
        (within % 3600) / 60,
        within % 60
    ))
}
fn civil_date(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    (y, m, d)
}
fn failed(status: EngineStatus, msg: &str, started: Instant) -> AdapterResponse {
    AdapterResponse {
        status,
        error_message: (!msg.is_empty()).then(|| msg.into()),
        latency_ms: elapsed(started),
        ..AdapterResponse::default()
    }
}
fn elapsed(started: Instant) -> FiniteF64 {
    FiniteF64::new(started.elapsed().as_secs_f64() * 1000.0).unwrap_or_default()
}
