use super::super::http::{AdapterHttp, Cancellation, Options, Request};
use crate::http::Error as HttpError;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use url::Url;

const DEFAULT_URL: &str = "https://crates.io/api/v1/crates";
const USER_AGENT: &str = "SlopSearX/0.1.0 (meta search engine; agent-native)";

#[derive(Clone, Debug)]
pub struct CratesConfig {
    pub base_url: String,
    pub timeout_ms: u64,
    pub max_results: usize,
}
impl Default for CratesConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_URL.into(),
            timeout_ms: 5_000,
            max_results: 10,
        }
    }
}

pub(super) struct CratesAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: CratesConfig,
}
impl CratesAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: CratesConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
}
impl SearchEngine for CratesAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move { self.search_inner(request).await })
    }
}
impl CratesAdapter {
    async fn search_inner(&self, request: &SearchRequest) -> AdapterResponse {
        let started = Instant::now();
        match self.admission.admit("crates").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => {
                return failed(
                    EngineStatus::RateLimited,
                    "crates.io request denied by rate policy",
                    started,
                );
            }
            AdmissionDecision::Unavailable => {
                return failed(
                    EngineStatus::Error,
                    "crates.io rate policy unavailable",
                    started,
                );
            }
        }
        let mut url = match Url::parse(if self.config.base_url.is_empty() {
            DEFAULT_URL
        } else {
            &self.config.base_url
        }) {
            Ok(url) => url,
            Err(_) => return failed(EngineStatus::Error, "invalid crates.io base URL", started),
        };
        // The source adapter appends parameters and does not consume SearchRequest.page.
        url.set_query(None);
        url.query_pairs_mut()
            .append_pair("q", &request.query)
            .append_pair("per_page", &self.config.max_results.to_string());
        let mut outbound = match Request::get(url.as_str()) {
            Ok(request) => request,
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    "invalid crates.io request URL",
                    started,
                );
            }
        };
        outbound
            .headers
            .insert("user-agent", USER_AGENT.parse().expect("static header"));
        let timeout = Duration::from_millis(self.config.timeout_ms);
        let mut session = match self.http.session(
            Options::default(),
            tokio::time::Instant::now() + timeout,
            Cancellation::default(),
        ) {
            Ok(session) => session,
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    "crates.io HTTP session unavailable",
                    started,
                );
            }
        };
        let response = match session.execute(outbound).await {
            Ok(response) => response,
            Err(HttpError::Deadline) => {
                return failed(
                    EngineStatus::Timeout,
                    "crates.io request timed out",
                    started,
                );
            }
            Err(_) => return failed(EngineStatus::Error, "crates.io request failed", started),
        };
        if response.status == 429 {
            return failed(EngineStatus::RateLimited, "crates.io rate limited", started);
        }
        if !(200..300).contains(&response.status) {
            return failed(
                EngineStatus::Error,
                "crates.io returned an unsuccessful status",
                started,
            );
        }
        let data: ssx_json::Value = match ssx_json::from_slice(&response.body) {
            Ok(value) => value,
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    "crates.io returned invalid JSON",
                    started,
                );
            }
        };
        let Some(data) = data.as_object() else {
            return failed(
                EngineStatus::Error,
                "crates.io response must be an object",
                started,
            );
        };
        let crates = match data.get("crates") {
            None => Vec::new(),
            Some(value) => match value.as_array() {
                Some(rows) => rows
                    .iter()
                    .take(self.config.max_results)
                    .collect::<Vec<_>>(),
                None => {
                    return failed(
                        EngineStatus::Error,
                        "crates.io crates must be an array",
                        started,
                    );
                }
            },
        };
        let mut results = Vec::with_capacity(crates.len());
        for (idx, value) in crates.into_iter().enumerate() {
            let Some(crate_row) = value.as_object() else {
                return failed(
                    EngineStatus::Error,
                    "crates.io result must be an object",
                    started,
                );
            };
            let name = python_string(crate_row.get("name"));
            let version = python_string(crate_row.get("max_version"));
            let description = match source_text(crate_row.get("description")) {
                Ok(v) => v,
                Err(()) => {
                    return failed(
                        EngineStatus::Error,
                        "crates.io description must be text",
                        started,
                    );
                }
            };
            let downloads = crate_row.get("downloads");
            let recent = crate_row.get("recent_downloads");
            let content = if downloads.is_some_and(truthy) {
                let formatted = match python_grouped(downloads.expect("truthy value")) {
                    Ok(value) => value,
                    Err(()) => {
                        return failed(
                            EngineStatus::Error,
                            "crates.io download count cannot be formatted",
                            started,
                        );
                    }
                };
                format!("{description} — Total downloads: {formatted}")
            } else {
                description.clone()
            };
            let score = match if recent.is_some_and(truthy) {
                recent
            } else {
                downloads
            } {
                Some(value) => match python_float(value) {
                    Ok(value) => value,
                    Err(()) => {
                        return failed(
                            EngineStatus::Error,
                            "crates.io download count is invalid",
                            started,
                        );
                    }
                },
                // Python's dict.get default is integer zero.
                None => 0.0,
            };
            results.push(SearchResult {
                url: format!("https://crates.io/crates/{name}"),
                title: format!("{name} v{version}"),
                content: content.chars().take(500).collect(),
                engine: "crates".into(),
                position: (idx + 1) as i64,
                score: FiniteF64::new(score).unwrap_or_default(),
                ..SearchResult::default()
            });
        }
        AdapterResponse {
            results,
            status: EngineStatus::Ok,
            latency_ms: elapsed(started),
            ..AdapterResponse::default()
        }
    }
}

fn failed(status: EngineStatus, message: &str, started: Instant) -> AdapterResponse {
    AdapterResponse {
        status,
        error_message: Some(message.into()),
        latency_ms: elapsed(started),
        ..AdapterResponse::default()
    }
}
fn elapsed(start: Instant) -> FiniteF64 {
    FiniteF64::new(start.elapsed().as_secs_f64() * 1000.0).unwrap_or_default()
}
pub(super) fn python_string(value: Option<&ssx_json::Value>) -> String {
    value
        .map(ssx_domain::storage::python_value_string)
        .unwrap_or_default()
}
pub(super) fn source_text(value: Option<&ssx_json::Value>) -> Result<String, ()> {
    match value {
        None => Ok(String::new()),
        Some(value) if !truthy(value) => Ok(String::new()),
        Some(ssx_json::Value::String(s)) => Ok(s.clone()),
        _ => Err(()),
    }
}
pub(super) fn python_float(value: &ssx_json::Value) -> Result<f64, ()> {
    let value = match value {
        ssx_json::Value::Bool(value) => f64::from(u8::from(*value)),
        ssx_json::Value::Number(value) => value.as_f64().ok_or(())?,
        ssx_json::Value::String(value) => parse_python_float(value).ok_or(())?,
        _ => return Err(()),
    };
    value.is_finite().then_some(value).ok_or(())
}
fn parse_python_float(input: &str) -> Option<f64> {
    let normalized = ssx_domain::storage::normalize_python_number_text(input).ok()?;
    let value = normalized.parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}
pub(super) fn truthy(value: &ssx_json::Value) -> bool {
    match value {
        ssx_json::Value::Null => false,
        ssx_json::Value::Bool(value) => *value,
        ssx_json::Value::Number(value) if value.is_f64() => {
            value.as_f64().is_some_and(|number| number != 0.0)
        }
        ssx_json::Value::Number(value) => value.as_str() != "0",
        ssx_json::Value::String(value) => !value.is_empty(),
        ssx_json::Value::Array(value) => !value.is_empty(),
        ssx_json::Value::Object(value) => !value.is_empty(),
    }
}
pub(super) fn python_grouped(value: &ssx_json::Value) -> Result<String, ()> {
    match value {
        ssx_json::Value::Bool(value) => Ok(if *value { "1" } else { "0" }.into()),
        ssx_json::Value::Number(number) if !number.is_f64() => Ok(grouped_decimal(number.as_str())),
        ssx_json::Value::Number(number) => {
            let numeric = number
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or(())?;
            let python = ssx_json::python_float(numeric).map_err(|_| ())?;
            // Match Python's `format(float_value, ',')`: Python retains repr's
            // scientific notation at abs(x) >= 1e16 or 0 < abs(x) < 1e-4.
            if numeric.abs() >= 1e16 || (numeric != 0.0 && numeric.abs() < 1e-4) {
                return Ok(python);
            }
            let mut decimal = python;
            if !decimal.contains('.') {
                decimal.push_str(".0");
            }
            Ok(grouped_decimal(&decimal))
        }
        _ => Err(()),
    }
}

fn grouped_decimal(value: &str) -> String {
    let (sign, unsigned) = value
        .strip_prefix('-')
        .map_or(("", value), |value| ("-", value));
    let (digits, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let first_group = digits.len() % 3;
    let mut out = String::with_capacity(value.len() + digits.len() / 3);
    out.push_str(sign);
    out.push_str(&digits[..first_group]);
    for (index, chunk) in digits.as_bytes()[first_group..].chunks(3).enumerate() {
        if first_group > 0 || index > 0 {
            out.push(',');
        }
        out.push_str(std::str::from_utf8(chunk).expect("digits are ASCII"));
    }
    if !fraction.is_empty() {
        out.push('.');
        out.push_str(fraction);
    }
    out
}
