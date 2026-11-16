use super::super::http::{AdapterHttp, Cancellation, Options, Request};
use crate::http::Error as HttpError;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use url::Url;

const DEFAULT_URL: &str = "https://registry.npmjs.org/-/v1/search";
const USER_AGENT: &str = "SlopSearX/0.1.0 (meta search engine; agent-native)";

#[derive(Clone, Debug)]
pub struct NpmConfig {
    pub base_url: String,
    pub timeout_ms: u64,
    pub max_results: usize,
}
impl Default for NpmConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_URL.into(),
            timeout_ms: 5_000,
            max_results: 10,
        }
    }
}

pub(super) struct NpmAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: NpmConfig,
}
impl NpmAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: NpmConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
}
impl SearchEngine for NpmAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move { self.search_inner(request).await })
    }
}
impl NpmAdapter {
    async fn search_inner(&self, request: &SearchRequest) -> AdapterResponse {
        let started = Instant::now();
        match self.admission.admit("npm").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => {
                return failed(
                    EngineStatus::RateLimited,
                    "npm request denied by rate policy",
                    started,
                );
            }
            AdmissionDecision::Unavailable => {
                return failed(EngineStatus::Error, "npm rate policy unavailable", started);
            }
        }
        let mut url = match Url::parse(if self.config.base_url.is_empty() {
            DEFAULT_URL
        } else {
            &self.config.base_url
        }) {
            Ok(url) => url,
            Err(_) => return failed(EngineStatus::Error, "invalid npm base URL", started),
        };
        // HTTPX params replace the configured query rather than merge with it.
        url.set_query(None);
        url.query_pairs_mut()
            .append_pair("text", &request.query)
            .append_pair("size", &self.config.max_results.to_string());
        let mut outbound = match Request::get(url.as_str()) {
            Ok(request) => request,
            Err(_) => return failed(EngineStatus::Error, "invalid npm request URL", started),
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
            Err(_) => return failed(EngineStatus::Error, "npm HTTP session unavailable", started),
        };
        let response = match session.execute(outbound).await {
            Ok(response) => response,
            Err(HttpError::Deadline) => {
                return failed(EngineStatus::Timeout, "npm request timed out", started);
            }
            Err(_) => return failed(EngineStatus::Error, "npm request failed", started),
        };
        if response.status == 429 {
            return failed(EngineStatus::RateLimited, "npm rate limited", started);
        }
        if !(200..300).contains(&response.status) {
            return failed(
                EngineStatus::Error,
                "npm returned an unsuccessful status",
                started,
            );
        }
        let data: ssx_json::Value = match ssx_json::from_slice(&response.body) {
            Ok(value) => value,
            Err(_) => return failed(EngineStatus::Error, "npm returned invalid JSON", started),
        };
        let Some(data) = data.as_object() else {
            return failed(
                EngineStatus::Error,
                "npm response must be an object",
                started,
            );
        };
        let objects = match data.get("objects") {
            None => Vec::new(),
            Some(value) => match value.as_array() {
                Some(rows) => rows
                    .iter()
                    .take(self.config.max_results)
                    .collect::<Vec<_>>(),
                None => {
                    return failed(EngineStatus::Error, "npm objects must be an array", started);
                }
            },
        };
        let mut results = Vec::with_capacity(objects.len());
        for (idx, object) in objects.into_iter().enumerate() {
            let Some(object) = object.as_object() else {
                return failed(EngineStatus::Error, "npm result must be an object", started);
            };
            let package = match object.get("package") {
                None => ssx_json::Map::new(),
                Some(value) => match value.as_object() {
                    Some(package) => package.clone(),
                    None => {
                        return failed(
                            EngineStatus::Error,
                            "npm package must be an object",
                            started,
                        );
                    }
                },
            };
            let name = python_string(package.get("name"));
            let version = python_string(package.get("version"));
            let description = match source_text(package.get("description")) {
                Ok(v) => v,
                Err(()) => {
                    return failed(EngineStatus::Error, "npm description is not text", started);
                }
            };
            let publisher = match package.get("publisher") {
                None => Ok(String::new()),
                Some(value) => value.as_object().ok_or(()).map(|publisher| {
                    publisher
                        .get("username")
                        .filter(|username| truthy(username))
                        .map(|username| python_string(Some(username)))
                        .unwrap_or_default()
                }),
            };
            let publisher = match publisher {
                Ok(v) => v,
                Err(()) => {
                    return failed(
                        EngineStatus::Error,
                        "npm publisher must be an object",
                        started,
                    );
                }
            };
            let repository = package
                .get("links")
                .and_then(ssx_json::Value::as_object)
                .and_then(|links| links.get("repository"));
            let score =
                match popularity(object).and_then(|n| FiniteF64::new(n * 1000.0).map_err(|_| ())) {
                    Ok(v) => v,
                    Err(()) => {
                        return failed(
                            EngineStatus::Error,
                            "npm popularity score is invalid",
                            started,
                        );
                    }
                };
            let content = if publisher.is_empty() {
                description.clone()
            } else {
                format!("{description} — Published by {publisher}")
            };
            let payload = npm_payload(&package, &description, repository);
            results.push(SearchResult {
                url: format!("https://www.npmjs.com/package/{name}"),
                title: format!("{name}@{version}"),
                content: content.chars().take(500).collect(),
                engine: "npm".into(),
                position: (idx + 1) as i64,
                score,
                payload,
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
fn source_text(value: Option<&ssx_json::Value>) -> Result<String, ()> {
    match value {
        None => Ok(String::new()),
        Some(value) if !truthy(value) => Ok(String::new()),
        Some(ssx_json::Value::String(s)) => Ok(s.clone()),
        _ => Err(()),
    }
}
fn python_string(value: Option<&ssx_json::Value>) -> String {
    value
        .map(ssx_domain::storage::python_value_string)
        .unwrap_or_default()
}
fn popularity(object: &ssx_json::Map<String, ssx_json::Value>) -> Result<f64, ()> {
    let score = match object.get("score") {
        None => return Ok(0.0),
        Some(v) => v.as_object().ok_or(())?,
    };
    let detail = match score.get("detail") {
        None => return Ok(0.0),
        Some(v) => v.as_object().ok_or(())?,
    };
    let Some(value) = detail.get("popularity") else {
        return Ok(0.0);
    };
    if !truthy(value) {
        return Ok(0.0);
    }
    let number = match value {
        ssx_json::Value::Bool(v) => f64::from(u8::from(*v)),
        ssx_json::Value::Number(v) => v.as_f64().ok_or(())?,
        ssx_json::Value::String(v) => parse_python_float(v).ok_or(())?,
        _ => return Err(()),
    };
    number.is_finite().then_some(number).ok_or(())
}
fn parse_python_float(input: &str) -> Option<f64> {
    let normalized = ssx_domain::storage::normalize_python_number_text(input).ok()?;
    let value = normalized.parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}
fn npm_payload(
    package: &ssx_json::Map<String, ssx_json::Value>,
    description: &str,
    repository: Option<&ssx_json::Value>,
) -> Option<ssx_json::Value> {
    use ssx_json::{Map, Value};
    let mut data = Map::new();
    for field in ["name", "version"] {
        if let Some(value) = package.get(field).filter(|value| truthy(value)) {
            data.insert(field.into(), value.clone());
        }
    }
    if !description.is_empty() {
        data.insert("summary".into(), Value::String(description.into()));
    }
    if let Some(repository) = repository.filter(|value| truthy(value)) {
        data.insert("repository_url".into(), repository.clone());
    }
    let fields = data.keys().cloned().map(Value::String).collect();
    let mut provenance = Map::new();
    provenance.insert("engine".into(), Value::String("npm".into()));
    provenance.insert("adapter_fields".into(), Value::Array(fields));
    provenance.insert("normalized_fields".into(), Value::Array(Vec::new()));
    provenance.insert("inferred_fields".into(), Value::Array(Vec::new()));
    let mut envelope = Map::new();
    envelope.insert("domain".into(), Value::String("packages".into()));
    envelope.insert("type".into(), Value::String("package".into()));
    envelope.insert(
        "schema_version".into(),
        Value::Number(ssx_json::Number::new("1").ok()?),
    );
    envelope.insert("data".into(), Value::Object(data));
    envelope.insert("provenance".into(), Value::Object(provenance));
    Some(Value::Object(envelope))
}
fn truthy(value: &ssx_json::Value) -> bool {
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
