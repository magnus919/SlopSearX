use super::crates_io::{python_string, source_text, truthy};
use crate::http::{AdapterHttp, Cancellation, Error as HttpError, Options, Request, Session};
use scraper::{Html, Selector};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use ssx_json::{Map, Value};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct PyPIConfig {
    pub base_url: String,
    pub timeout_ms: u64,
    pub max_results: usize,
}
impl Default for PyPIConfig {
    fn default() -> Self {
        Self {
            base_url: "https://pypi.org".into(),
            timeout_ms: 5000,
            max_results: 10,
        }
    }
}
pub(super) struct PyPIAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: PyPIConfig,
}
impl PyPIAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: PyPIConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn search_inner(&self, request: &SearchRequest) -> AdapterResponse {
        let started = Instant::now();
        match self.admission.admit("pypi").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => {
                return failed(
                    EngineStatus::RateLimited,
                    "request denied by rate policy",
                    started,
                );
            }
            AdmissionDecision::Unavailable => {
                return failed(EngineStatus::Error, "rate policy unavailable", started);
            }
        }
        let result = self.fetch(&request.query).await;
        match result {
            Ok(results) => AdapterResponse {
                results,
                status: EngineStatus::Ok,
                latency_ms: elapsed(started),
                ..AdapterResponse::default()
            },
            Err((status, message)) => failed(status, message, started),
        }
    }
    fn session(&self, follow_redirects: bool) -> Result<Session, (EngineStatus, &'static str)> {
        self.http
            .session(
                Options {
                    follow_redirects,
                    ..Options::default()
                },
                tokio::time::Instant::now() + Duration::from_millis(self.config.timeout_ms),
                Cancellation::default(),
            )
            .map_err(|_| error("HTTP session unavailable"))
    }
    async fn fetch(&self, query: &str) -> Result<Vec<SearchResult>, (EngineStatus, &'static str)> {
        let base = &self.config.base_url;
        let request = outbound(&format!("{base}/pypi/{query}/json"))?;
        let mut direct = self.session(true)?;
        let response = direct.execute(request).await.map_err(|e| {
            if matches!(e, HttpError::Deadline) {
                (EngineStatus::Timeout, "request timed out")
            } else {
                error("request failed")
            }
        })?;
        if response.status == 200 {
            let data: Value =
                ssx_json::from_slice(&response.body).map_err(|_| error("invalid JSON"))?;
            let info = info(&data)?;
            let name = info
                .get("name")
                .cloned()
                .unwrap_or(Value::String(query.into()));
            return Ok(vec![package(&info, &name, 1, true)?]);
        }
        // The Python fallback creates a distinct session with redirects disabled.
        let mut fallback = self.session(false)?;
        let response = fallback
            .execute(outbound(&format!("{base}/simple/"))?)
            .await
            .map_err(|_| error("simple index request failed"))?;
        if !(200..300).contains(&response.status) {
            return Err(error("simple index unsuccessful status"));
        }
        let decoded = decode_source_html(&response.body);
        if !source_html_has_document(&decoded) {
            return Err(error("empty HTML document"));
        }
        let matched = {
            let html = Html::parse_document(&decoded);
            let anchors = Selector::parse("a").expect("static selector");
            let clean = |s: &str| s.to_lowercase().replace(['-', '_'], "");
            let query_clean = clean(query);
            let mut matched = Vec::new();
            for anchor in html.select(&anchors) {
                let href = anchor.value().attr("href").unwrap_or_default();
                let name = if href.is_empty() {
                    ""
                } else {
                    href.trim_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap_or_default()
                };
                if clean(name).contains(&query_clean) {
                    matched.push(name.to_owned());
                }
                if matched.len() >= self.config.max_results {
                    break;
                }
            }
            matched
        };
        let mut results = Vec::new();
        for name in matched {
            // Every failed detail lookup is skipped, without replacing the candidate.
            let result = async {
                fallback.set_deadline(
                    tokio::time::Instant::now() + Duration::from_millis(self.config.timeout_ms),
                );
                let response = fallback
                    .execute(outbound(&format!("{base}/pypi/{name}/json"))?)
                    .await
                    .map_err(|_| error("detail request failed"))?;
                if response.status != 200 {
                    return Err(error("detail unsuccessful status"));
                }
                let data: Value = ssx_json::from_slice(&response.body)
                    .map_err(|_| error("invalid detail JSON"))?;
                package(
                    &info(&data)?,
                    &Value::String(name),
                    results.len() + 1,
                    false,
                )
            }
            .await;
            if let Ok(result) = result {
                results.push(result);
            }
        }
        Ok(results)
    }
}
impl SearchEngine for PyPIAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move { self.search_inner(request).await })
    }
}
fn outbound(url: &str) -> Result<Request, (EngineStatus, &'static str)> {
    let mut request = Request::get(url).map_err(|_| error("invalid request URL"))?;
    request.headers.insert(
        "user-agent",
        "SlopSearX/0.1.0 (meta search engine; agent-native)"
            .parse()
            .expect("static header"),
    );
    Ok(request)
}
fn info(data: &Value) -> Result<Map<String, Value>, (EngineStatus, &'static str)> {
    let object = data.as_object().ok_or(error("root must be object"))?;
    match object.get("info") {
        None => Ok(Map::new()),
        Some(value) => value
            .as_object()
            .cloned()
            .ok_or(error("info must be object")),
    }
}
fn package(
    info: &Map<String, Value>,
    name: &Value,
    position: usize,
    exact: bool,
) -> Result<SearchResult, (EngineStatus, &'static str)> {
    let display = python_string(Some(name));
    let version = python_string(info.get("version"));
    let summary = source_text(info.get("summary")).map_err(|_| error("summary must be text"))?;
    let content: String = summary.chars().take(500).collect();
    let content = if exact && content.is_empty() {
        format!("Python package: {display}")
    } else {
        content
    };
    Ok(SearchResult {
        url: format!("https://pypi.org/project/{display}/"),
        title: format!("{display} {version}"),
        content,
        engine: "pypi".into(),
        position: position as i64,
        score: FiniteF64::new(1.0).expect("finite"),
        payload: Some(package_payload(info, name)),
        ..SearchResult::default()
    })
}
fn package_payload(info: &Map<String, Value>, name: &Value) -> Value {
    let mut data = Map::new();
    if let Some(value) = info
        .get("name")
        .filter(|v| truthy(v))
        .or_else(|| truthy(name).then_some(name))
    {
        data.insert("name".into(), value.clone());
    }
    for (target, source) in [
        ("version", "version"),
        ("summary", "summary"),
        ("license", "license"),
        ("homepage", "home_page"),
    ] {
        if let Some(value) = info.get(source).filter(|v| truthy(v)) {
            data.insert(target.into(), value.clone());
        }
    }
    if let Some(urls) = info.get("project_urls").and_then(Value::as_object) {
        for (label, value) in urls {
            if value.is_string()
                && ["source", "repository", "code"]
                    .iter()
                    .any(|token| ssx_json::casefold(label).contains(token))
            {
                data.insert("repository_url".into(), value.clone());
                break;
            }
        }
    }
    // Same source build_payload contract as the accepted npm local envelope:
    // retain only non-null data, insertion-order provenance and no invented fields.
    let fields = data.keys().cloned().map(Value::String).collect::<Vec<_>>();
    ssx_json::json!({"domain":"packages","type":"package","schema_version":1,"data":data,"provenance":{"engine":"pypi","adapter_fields":fields,"normalized_fields":[],"inferred_fields":[]}})
}
fn error(message: &'static str) -> (EngineStatus, &'static str) {
    (EngineStatus::Error, message)
}
fn elapsed(start: Instant) -> FiniteF64 {
    FiniteF64::new(start.elapsed().as_secs_f64() * 1000.0).unwrap_or_default()
}
fn failed(status: EngineStatus, message: &str, start: Instant) -> AdapterResponse {
    AdapterResponse {
        status,
        error_message: Some(message.into()),
        latency_ms: elapsed(start),
        ..AdapterResponse::default()
    }
}

// libxml2's HTML byte parser defaults to ISO-8859-1, rather than the
// Windows-1252 default used by browser sniffers. Preserve raw C1 controls.
fn decode_source_html(bytes: &[u8]) -> String {
    if let Some((encoding, bom)) = encoding_rs::Encoding::for_bom(bytes) {
        return encoding
            .decode_without_bom_handling(&bytes[bom..])
            .0
            .into_owned();
    }
    let latin1: String = bytes.iter().map(|byte| char::from(*byte)).collect();
    let parsed = Html::parse_document(&latin1);
    let meta = Selector::parse("meta").expect("static selector");
    for node in parsed.select(&meta) {
        let label = if let Some(label) = node.value().attr("charset") {
            Some(label.to_owned())
        } else if node
            .value()
            .attr("http-equiv")
            .is_some_and(|value| value.eq_ignore_ascii_case("content-type"))
        {
            node.value().attr("content").and_then(|content| {
                content.split(';').find_map(|part| {
                    let (name, value) = part.split_once('=')?;
                    name.trim()
                        .eq_ignore_ascii_case("charset")
                        .then(|| value.trim().trim_matches(['\'', '"']).to_owned())
                })
            })
        } else {
            None
        };
        let Some(label) = label else {
            continue;
        };
        let normalized = label.trim().to_ascii_lowercase().replace(['-', '_'], "");
        if matches!(
            normalized.as_str(),
            "iso88591" | "latin1" | "l1" | "ibm819" | "cp819" | "csisolatin1"
        ) {
            return latin1;
        }
        // Unknown declarations do not hide a later recognized source charset.
        if let Some(encoding) = encoding_rs::Encoding::for_label(label.trim().as_bytes()) {
            return encoding.decode_without_bom_handling(bytes).0.into_owned();
        }
    }
    latin1
}

// lxml.html.fromstring rejects a document containing only whitespace,
// comments, declarations or a doctype; scraper inserts an empty html tree.
// This finite preflight preserves that boundary before actual anchor parsing.
fn source_html_has_document(mut text: &str) -> bool {
    loop {
        text = text.trim_start_matches([' ', '\t', '\r', '\n', '\u{000c}']);
        let end = if text.starts_with("<!--") {
            text.find("-->").map(|end| end + 3)
        } else if text.starts_with("<?") {
            text.find("?>").map(|end| end + 2)
        } else if text
            .get(..9)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<!doctype"))
        {
            text.find('>').map(|end| end + 1)
        } else {
            return !text.is_empty();
        };
        let Some(end) = end else {
            return false;
        };
        text = &text[end..];
    }
}
