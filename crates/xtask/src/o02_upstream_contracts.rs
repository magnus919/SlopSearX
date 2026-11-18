//! Bounded O02 upstream probes. Offline fixtures are the default; live I/O is explicit.
use crate::Result;
use reqwest::{
    Client, Method,
    header::{
        ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HeaderMap, HeaderValue,
        USER_AGENT,
    },
    redirect::Policy,
};
use serde_json::{Value, json};
use ssx_application::SearchEngine;
use ssx_domain::{AdapterResponse, EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{
        AllowAllAdmission, CratesConfig, NpmConfig, PilotConfig,
        arxiv_expat::AtomPool,
        openalex_runtime::{OpenAlexAdapter, OpenAlexConfig},
        register_pilots,
        scholarly_config::Scalar,
    },
    http::{
        AdapterHttp, Bounds, Error as HttpError, ExchangeFuture, Options,
        Request as AdapterRequest, Response as AdapterHttpResponse, Transport,
    },
};
use ssx_registry::Registry;
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    future::Future,
    io::Write,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use url::Url;

const MAX_BYTES: usize = 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(10);
const MAX_RESULTS: usize = 3;

#[derive(Clone, Copy)]
struct Probe {
    name: &'static str,
    query: &'static str,
    host: &'static str,
    path: &'static str,
    base_url: &'static str,
    params: &'static [(&'static str, &'static str)],
}

const NPM_PARAMS: &[(&str, &str)] = &[("text", "typescript"), ("size", "3")];
const OPENALEX_PARAMS: &[(&str, &str)] =
    &[("search", "Attention Is All You Need"), ("per_page", "3")];
const CRATES_PARAMS: &[(&str, &str)] = &[("q", "serde"), ("per_page", "3")];
const PROBES: &[Probe] = &[
    Probe {
        name: "npm",
        query: "typescript",
        host: "registry.npmjs.org",
        path: "/-/v1/search",
        base_url: "https://registry.npmjs.org/-/v1/search",
        params: NPM_PARAMS,
    },
    Probe {
        name: "openalex",
        query: "Attention Is All You Need",
        host: "api.openalex.org",
        path: "/works",
        base_url: "https://api.openalex.org",
        params: OPENALEX_PARAMS,
    },
    Probe {
        name: "crates",
        query: "serde",
        host: "crates.io",
        path: "/api/v1/crates",
        base_url: "https://crates.io/api/v1/crates",
        params: CRATES_PARAMS,
    },
];

type WireResult<T> = std::result::Result<T, WireError>;
type WireFuture<'a> = Pin<Box<dyn Future<Output = WireResult<WireResponse>> + Send + 'a>>;
type BodyFuture<'a> = Pin<Box<dyn Future<Output = WireResult<Option<Vec<u8>>>> + Send + 'a>>;

#[derive(Clone, Copy)]
enum WireError {
    Network,
    Deadline,
}

trait ResponseBody: Send {
    fn next_chunk(&mut self) -> BodyFuture<'_>;
}

struct WireResponse {
    status: u16,
    url: Url,
    headers: HeaderMap,
    body: Box<dyn ResponseBody>,
}

trait ProbeWire: Send + Sync {
    fn send<'a>(&'a self, request: AdapterRequest) -> WireFuture<'a>;
}

struct ReqwestWire {
    client: Client,
}

impl ReqwestWire {
    fn client() -> std::result::Result<Client, reqwest::Error> {
        Client::builder()
            .no_proxy()
            .retry(reqwest::retry::never())
            .redirect(Policy::none())
            .timeout(DEADLINE)
            .connect_timeout(DEADLINE)
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
    }
}

struct ReqwestBody(reqwest::Response);

impl ResponseBody for ReqwestBody {
    fn next_chunk(&mut self) -> BodyFuture<'_> {
        Box::pin(async move {
            self.0
                .chunk()
                .await
                .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
                .map_err(|error| {
                    if error.is_timeout() {
                        WireError::Deadline
                    } else {
                        WireError::Network
                    }
                })
        })
    }
}

impl ProbeWire for ReqwestWire {
    fn send<'a>(&'a self, request: AdapterRequest) -> WireFuture<'a> {
        Box::pin(async move {
            let mut headers = request.headers;
            headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
            let response = self
                .client
                .request(request.method, request.url)
                .headers(headers)
                .body(request.body)
                .send()
                .await
                .map_err(|error| {
                    if error.is_timeout() {
                        WireError::Deadline
                    } else {
                        WireError::Network
                    }
                })?;
            Ok(WireResponse {
                status: response.status().as_u16(),
                url: response.url().clone(),
                headers: response.headers().clone(),
                body: Box::new(ReqwestBody(response)),
            })
        })
    }
}

#[derive(Clone, Default)]
struct Observation {
    attempts: usize,
    network_requests: usize,
    http_status: Option<u16>,
    raw_body: Vec<u8>,
    failure: Option<&'static str>,
    unavailable: bool,
    deadline: bool,
}

struct BoundedTransport {
    probe: Probe,
    wire: Arc<dyn ProbeWire>,
    live: bool,
    observation: Mutex<Observation>,
}

impl BoundedTransport {
    fn new(probe: Probe, wire: Arc<dyn ProbeWire>, live: bool) -> Self {
        Self {
            probe,
            wire,
            live,
            observation: Mutex::new(Observation::default()),
        }
    }

    fn observation(&self) -> Observation {
        self.observation.lock().expect("probe observation").clone()
    }

    fn fail(&self, reason: &'static str) {
        self.observation.lock().expect("probe observation").failure = Some(reason);
    }

    fn unavailable(&self) {
        let mut observed = self.observation.lock().expect("probe observation");
        if observed.failure.is_none() {
            observed.unavailable = true;
        }
    }

    fn deadline(&self) {
        let mut observed = self.observation.lock().expect("probe observation");
        if observed.failure.is_none() {
            observed.unavailable = true;
            observed.deadline = true;
        }
    }

    async fn exchange_inner(
        &self,
        request: AdapterRequest,
    ) -> std::result::Result<AdapterHttpResponse, HttpError> {
        {
            let mut observed = self.observation.lock().expect("probe observation");
            observed.attempts += 1;
            if observed.attempts > 1 {
                observed.failure = Some("request_limit");
                return Err(HttpError::RequestLimit);
            }
        }
        if let Err(reason) = validate_request(self.probe, &request) {
            self.fail(reason);
            return Err(HttpError::InvalidRequest);
        }
        if self.live {
            self.observation
                .lock()
                .expect("probe observation")
                .network_requests += 1;
        }
        let mut response = match self.wire.send(request.clone()).await {
            Ok(response) => response,
            Err(WireError::Network) => {
                self.unavailable();
                return Err(HttpError::Network);
            }
            Err(WireError::Deadline) => {
                self.deadline();
                return Err(HttpError::Deadline);
            }
        };
        {
            let mut observed = self.observation.lock().expect("probe observation");
            observed.http_status = Some(response.status);
        }
        if response.url != request.url {
            self.fail("unexpected_redirect");
            return Err(HttpError::InvalidResponse);
        }
        let declared = match validate_response_headers(&response.headers) {
            Ok(length) => length,
            Err(reason) => {
                self.fail(reason);
                return Err(if reason == "response_size_limit" {
                    HttpError::WireBodyLimit
                } else {
                    HttpError::InvalidResponse
                });
            }
        };
        let mut body = Vec::with_capacity(declared.unwrap_or(0).min(MAX_BYTES));
        loop {
            let chunk = match response.body.next_chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(WireError::Network) => {
                    self.unavailable();
                    return Err(HttpError::Network);
                }
                Err(WireError::Deadline) => {
                    self.deadline();
                    return Err(HttpError::Deadline);
                }
            };
            if chunk.len() > MAX_BYTES.saturating_sub(body.len()) {
                self.fail("response_size_limit");
                return Err(HttpError::WireBodyLimit);
            }
            body.extend_from_slice(&chunk);
        }
        if declared.is_some_and(|length| length != body.len()) {
            self.fail("content_length_mismatch");
            return Err(HttpError::InvalidResponse);
        }
        self.observation.lock().expect("probe observation").raw_body = body.clone();
        Ok(AdapterHttpResponse {
            status: response.status,
            url: response.url,
            headers: response.headers,
            body,
        })
    }
}

impl Transport for BoundedTransport {
    fn exchange(&self, request: AdapterRequest) -> ExchangeFuture<'_> {
        Box::pin(async move { self.exchange_inner(request).await })
    }
}

fn validate_request(
    probe: Probe,
    request: &AdapterRequest,
) -> std::result::Result<(), &'static str> {
    let url = &request.url;
    if request.method != Method::GET {
        return Err("request_outside_allowlist");
    }
    if url.scheme() != "https"
        || url.host_str() != Some(probe.host)
        || url.port_or_known_default() != Some(443)
        || url.path() != probe.path
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !request.body.is_empty()
    {
        return Err("request_outside_allowlist");
    }
    let mut actual: Vec<_> = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let mut expected: Vec<_> = probe
        .params
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    actual.sort();
    expected.sort();
    if actual != expected {
        return Err("request_outside_allowlist");
    }
    if request.headers.keys().any(|name| name != USER_AGENT)
        || request.headers.get_all(USER_AGENT).iter().count() > 1
    {
        return Err("request_header_outside_allowlist");
    }
    Ok(())
}

fn validate_response_headers(
    headers: &HeaderMap,
) -> std::result::Result<Option<usize>, &'static str> {
    let mut encodings = headers.get_all(CONTENT_ENCODING).iter();
    if let Some(encoding) = encodings.next()
        && (encodings.next().is_some()
            || !encoding
                .to_str()
                .is_ok_and(|value| value.trim().eq_ignore_ascii_case("identity")))
    {
        return Err("unexpected_content_encoding");
    }
    let mut lengths = headers.get_all(CONTENT_LENGTH).iter();
    let Some(length) = lengths.next() else {
        return Ok(None);
    };
    if lengths.next().is_some() {
        return Err("ambiguous_content_length");
    }
    let value = length
        .to_str()
        .map_err(|_| "invalid_content_length")?
        .trim_matches([' ', '\t']);
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("invalid_content_length");
    }
    let mut parsed = 0usize;
    for byte in value.bytes() {
        parsed = parsed
            .saturating_mul(10)
            .saturating_add(usize::from(byte - b'0'));
        if parsed > MAX_BYTES {
            return Err("response_size_limit");
        }
    }
    Ok(Some(parsed))
}

fn make_adapter(
    probe: Probe,
    http: Arc<AdapterHttp>,
    root: &Path,
) -> Result<(Box<dyn SearchEngine>, Option<AtomPool>)> {
    let admission = Arc::new(AllowAllAdmission);
    if probe.name == "npm" || probe.name == "crates" {
        let mut registry = Registry::builtins().map_err(|error| error.to_string())?;
        register_pilots(
            &mut registry,
            http,
            admission,
            PilotConfig {
                npm: NpmConfig {
                    base_url: probe.base_url.into(),
                    timeout_ms: DEADLINE.as_millis() as u64,
                    max_results: MAX_RESULTS,
                },
                crates: CratesConfig {
                    base_url: probe.base_url.into(),
                    timeout_ms: DEADLINE.as_millis() as u64,
                    max_results: MAX_RESULTS,
                },
            },
        )
        .map_err(|error| error.to_string())?;
        return registry
            .instantiate(probe.name)
            .map(|engine| (engine, None))
            .ok_or_else(|| "native probe adapter is unavailable".into());
    }
    if probe.name == "openalex" {
        let pool = AtomPool::new(
            root.join("unused-openalex-parser-helper"),
            NonZeroUsize::new(1).expect("one is nonzero"),
        )
        .map_err(|_| "OpenAlex lifecycle initialization failed".to_owned())?;
        let config = OpenAlexConfig {
            base_url: Arc::from(probe.base_url),
            timeout_ms: Scalar::Integer(DEADLINE.as_millis() as i64),
            max_results: Scalar::Integer(MAX_RESULTS as i64),
            transport: Options {
                trust_env: false,
                follow_redirects: false,
                bounds: Bounds {
                    wire_bytes: MAX_BYTES,
                    decoded_bytes: MAX_BYTES,
                    requests: 1,
                    ..Bounds::default()
                },
                identity_announced_bytes: Some(MAX_BYTES),
                ..Options::default()
            },
        };
        return Ok((
            Box::new(OpenAlexAdapter::new(http, admission, config, pool.clone())),
            Some(pool),
        ));
    }
    Err("unknown upstream probe".into())
}

#[derive(Clone)]
struct ProbeOutput {
    row: Value,
    raw_body: Vec<u8>,
}

async fn run_probe(probe: Probe, wire: Arc<dyn ProbeWire>, live: bool, root: &Path) -> ProbeOutput {
    run_probe_with_deadline(probe, wire, live, root, DEADLINE).await
}

async fn run_probe_with_deadline(
    probe: Probe,
    wire: Arc<dyn ProbeWire>,
    live: bool,
    root: &Path,
    probe_deadline: Duration,
) -> ProbeOutput {
    let bounded = Arc::new(BoundedTransport::new(probe, wire, live));
    let http = Arc::new(AdapterHttp::default());
    if http.set_transport(bounded.clone()).is_err() {
        return failed_output(probe, "adapter_setup_failure");
    }
    let (engine, lifecycle) = match make_adapter(probe, http, root) {
        Ok(value) => value,
        Err(_) => return failed_output(probe, "adapter_setup_failure"),
    };
    let started = Instant::now();
    let mut request = SearchRequest {
        query: probe.query.to_owned(),
        max_results: Some(MAX_RESULTS as u64),
        ..SearchRequest::default()
    };
    request.generate_suggestions = false;
    let deadline = tokio::time::Instant::now() + probe_deadline;
    let searched = tokio::time::timeout_at(deadline, engine.search(&request)).await;
    let search_timed_out = searched.is_err();
    drop(engine);
    let cleanup_failed = if let Some(pool) = lifecycle {
        tokio::time::timeout_at(deadline, pool.shutdown())
            .await
            .is_err()
    } else {
        false
    };
    if cleanup_failed && searched.is_ok() {
        bounded.fail("adapter_cleanup_failure");
    }
    let observed = bounded.observation();
    let response = searched.ok();
    let status = response.as_ref().map(|value| status_name(&value.status));
    let (outcome, reason) = classify(response.as_ref(), &observed, search_timed_out);
    let latency = response
        .as_ref()
        .map(|value| value.latency_ms.get())
        .unwrap_or_else(|| started.elapsed().as_secs_f64() * 1000.0);
    let row = json!({
        "engine": probe.name,
        "query": probe.query,
        "adapter_status": status,
        "result_count": response.as_ref().map_or(0, |value| value.results.len()),
        "latency_ms": round_two(latency),
        "outcome": outcome,
        "reason": reason,
        "http_status": observed.http_status,
        "requests": observed.attempts,
        "network_requests": observed.network_requests
    });
    ProbeOutput {
        row,
        raw_body: observed.raw_body,
    }
}

fn failed_output(probe: Probe, reason: &'static str) -> ProbeOutput {
    ProbeOutput {
        row: json!({
            "engine": probe.name,
            "query": probe.query,
            "adapter_status": "error",
            "result_count": 0,
            "latency_ms": 0.0,
            "outcome": "contract_failure",
            "reason": reason,
            "http_status": null,
            "requests": 0,
            "network_requests": 0
        }),
        raw_body: Vec::new(),
    }
}

fn classify(
    response: Option<&AdapterResponse>,
    observed: &Observation,
    search_timed_out: bool,
) -> (&'static str, &'static str) {
    if search_timed_out
        || observed.deadline
        || response.is_some_and(|value| value.status == EngineStatus::Timeout)
    {
        return ("upstream_unavailable", "probe_deadline");
    }
    if let Some(reason) = observed.failure {
        return ("contract_failure", reason);
    }
    if observed.unavailable {
        return ("upstream_unavailable", "network_or_http_failure");
    }
    let Some(http_status) = observed.http_status else {
        return ("contract_failure", "adapter_raised");
    };
    if !(200..300).contains(&http_status) {
        return ("upstream_unavailable", "network_or_http_failure");
    }
    let Some(response) = response else {
        return ("contract_failure", "adapter_raised");
    };
    if response.status != EngineStatus::Ok {
        return ("contract_failure", "successful_http_not_parsed");
    }
    if response.results.is_empty() {
        return ("contract_failure", "empty_known_query");
    }
    if response
        .results
        .iter()
        .any(|result| result.title.trim().is_empty() || !usable_url(&result.url))
    {
        return ("contract_failure", "invalid_title_or_url");
    }
    let latency = response.latency_ms.get();
    if !latency.is_finite() || latency <= 0.0 {
        return ("contract_failure", "invalid_latency");
    }
    if observed.attempts != 1 {
        return ("contract_failure", "request_count_mismatch");
    }
    ("passed", "nonempty_usable_results")
}

fn status_name(status: &EngineStatus) -> &'static str {
    match status {
        EngineStatus::Ok => "ok",
        EngineStatus::RateLimited => "rate_limited",
        EngineStatus::Blocked => "blocked",
        EngineStatus::Error => "error",
        EngineStatus::Timeout => "timeout",
        EngineStatus::Unavailable => "unavailable",
    }
}

fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn usable_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && !matches!(
            url.path().trim_end_matches('/'),
            "" | "/package" | "/crates"
        )
        && !url.path().contains("https://")
        && !url.path().contains("http://")
}

fn sanitized_payload(probe: Probe, body: &[u8]) -> std::result::Result<Value, &'static str> {
    let data: Value = serde_json::from_slice(body).map_err(|_| "capture_schema_mismatch")?;
    let (array_name, fields) = match probe.name {
        "npm" => ("objects", &["name", "version", "description"][..]),
        "openalex" => (
            "results",
            &["id", "doi", "title", "publication_date", "relevance_score"][..],
        ),
        "crates" => (
            "crates",
            &[
                "name",
                "max_version",
                "description",
                "downloads",
                "recent_downloads",
            ][..],
        ),
        _ => return Err("capture_schema_mismatch"),
    };
    let rows = data
        .get(array_name)
        .and_then(Value::as_array)
        .ok_or("capture_schema_mismatch")?;
    let mut output = Vec::with_capacity(rows.len().min(MAX_RESULTS));
    for row in rows.iter().take(MAX_RESULTS) {
        let object = if probe.name == "npm" {
            row.get("package")
                .and_then(Value::as_object)
                .ok_or("capture_schema_mismatch")?
        } else {
            row.as_object().ok_or("capture_schema_mismatch")?
        };
        let mut filtered = serde_json::Map::new();
        for field in fields {
            filtered.insert(
                (*field).to_owned(),
                object.get(*field).cloned().unwrap_or(Value::Null),
            );
        }
        if probe.name == "npm" {
            output.push(json!({"package": filtered}));
        } else {
            output.push(Value::Object(filtered));
        }
    }
    Ok(match probe.name {
        "npm" => json!({"objects": output}),
        "openalex" => json!({"results": output}),
        "crates" => json!({"crates": output}),
        _ => return Err("capture_schema_mismatch"),
    })
}

fn fixture_record(probe: Probe) -> Result<(&'static str, Value)> {
    let (path, source) = match probe.name {
        "npm" => (
            "tests/fixtures/upstream_contracts/npm.json",
            include_str!("../../../tests/fixtures/upstream_contracts/npm.json"),
        ),
        "openalex" => (
            "tests/fixtures/upstream_contracts/openalex.json",
            include_str!("../../../tests/fixtures/upstream_contracts/openalex.json"),
        ),
        "crates" => (
            "tests/fixtures/upstream_contracts/crates.json",
            include_str!("../../../tests/fixtures/upstream_contracts/crates.json"),
        ),
        _ => return Err("unknown upstream probe".into()),
    };
    let fixture: Value = serde_json::from_str(source).map_err(|_| "fixture JSON invalid")?;
    if fixture["query"].as_str() != Some(probe.query) {
        return Err("fixture query differs from the fixed probe".into());
    }
    Ok((path, fixture))
}

fn fixture_payload(probe: Probe) -> Result<Vec<u8>> {
    let (_, fixture) = fixture_record(probe)?;
    serde_json::to_vec(&fixture["payload"])
        .map_err(|_| "fixture payload serialization failed".into())
}

struct FixtureWire {
    body: Vec<u8>,
}

impl ProbeWire for FixtureWire {
    fn send<'a>(&'a self, request: AdapterRequest) -> WireFuture<'a> {
        let body = self.body.clone();
        Box::pin(async move {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            headers.insert(
                CONTENT_LENGTH,
                HeaderValue::from_str(&body.len().to_string()).expect("decimal content length"),
            );
            Ok(WireResponse {
                status: 200,
                url: request.url,
                headers,
                body: Box::new(Chunks::new(vec![body])),
            })
        })
    }
}

struct Chunks {
    chunks: VecDeque<Vec<u8>>,
}

impl Chunks {
    fn new(chunks: Vec<Vec<u8>>) -> Self {
        Self {
            chunks: chunks.into(),
        }
    }
}

impl ResponseBody for Chunks {
    fn next_chunk(&mut self) -> BodyFuture<'_> {
        Box::pin(async move { Ok(self.chunks.pop_front()) })
    }
}

struct CliOptions {
    output: PathBuf,
    capture_dir: Option<PathBuf>,
    live: bool,
}

fn parse_cli(args: &[String]) -> Result<CliOptions> {
    let mut output = None;
    let mut capture_dir = None;
    let mut live = false;
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "--live" if !live => {
                live = true;
                index += 1;
            }
            "--output" | "--capture-dir" => {
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with("--"))
                    .ok_or("usage: cargo xtask upstream-contracts --output PATH [--capture-dir PATH] [--live]")?;
                let target = if argument == "--output" {
                    &mut output
                } else {
                    &mut capture_dir
                };
                if target.replace(PathBuf::from(value)).is_some() {
                    return Err("duplicate upstream-contracts path option".into());
                }
                index += 2;
            }
            _ => return Err(
                "usage: cargo xtask upstream-contracts --output PATH [--capture-dir PATH] [--live]"
                    .into(),
            ),
        }
    }
    Ok(CliOptions {
        output: output.ok_or(
            "usage: cargo xtask upstream-contracts --output PATH [--capture-dir PATH] [--live]",
        )?,
        capture_dir,
        live,
    })
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("output filename missing"))?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let mut created_temporary = false;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        created_temporary = true;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        if let Ok(directory) = fs::File::open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if result.is_err() && created_temporary {
        let _ = fs::remove_file(&temporary);
    }
    result
}

async fn run_all(options: CliOptions, root: &Path) -> Result<i32> {
    let client = if options.live {
        Some(
            ReqwestWire::client()
                .map_err(|_| "bounded upstream HTTP client initialization failed")?,
        )
    } else {
        None
    };
    let mut rows = Vec::with_capacity(PROBES.len());
    for probe in PROBES {
        let wire: Arc<dyn ProbeWire> = if let Some(client) = &client {
            Arc::new(ReqwestWire {
                client: client.clone(),
            })
        } else {
            Arc::new(FixtureWire {
                body: fixture_payload(*probe)?,
            })
        };
        let mut output = run_probe(*probe, wire, options.live, root).await;
        if output.row["outcome"] == "passed"
            && let Some(directory) = &options.capture_dir
        {
            match sanitized_payload(*probe, &output.raw_body) {
                Ok(payload) => {
                    let capture = if options.live {
                        json!({
                            "engine": probe.name,
                            "query": probe.query,
                            "mode": "live",
                            "captured_at": timestamp(),
                            "source": format!("https://{}{}", probe.host, probe.path),
                            "sanitization": "allowlisted public parser fields; top three; no headers or personal author fields",
                            "payload": payload
                        })
                    } else {
                        let (fixture_path, fixture) = fixture_record(*probe)?;
                        let fixture_captured_at = fixture["captured_at"]
                            .as_str()
                            .ok_or("fixture capture time missing")?;
                        json!({
                            "engine": probe.name,
                            "query": probe.query,
                            "mode": "offline_fixture_replay",
                            "generated_at": timestamp(),
                            "source": format!("https://{}{}", probe.host, probe.path),
                            "fixture": {
                                "path": fixture_path,
                                "captured_at": fixture_captured_at
                            },
                            "sanitization": "allowlisted public parser fields; top three; no headers or personal author fields",
                            "payload": payload
                        })
                    };
                    let path = directory.join(format!("{}.json", probe.name));
                    let bytes = serde_json::to_vec_pretty(&capture)
                        .map_err(|_| "capture serialization failed")?;
                    let mut bytes = bytes;
                    bytes.push(b'\n');
                    if write_atomic(&path, &bytes).is_err() {
                        output.row["outcome"] = json!("contract_failure");
                        output.row["reason"] = json!("capture_write_failure");
                    }
                }
                Err(reason) => {
                    output.row["outcome"] = json!("contract_failure");
                    output.row["reason"] = json!(reason);
                }
            }
        }
        rows.push(output.row);
    }
    let report = json!({
        "captured_at": timestamp(),
        "mode": if options.live { "live" } else { "offline_fixture" },
        "max_requests": PROBES.len(),
        "per_probe_deadline_seconds": DEADLINE.as_secs(),
        "max_response_bytes": MAX_BYTES,
        "relevance_evaluated": false,
        "probes": rows
    });
    let mut bytes =
        serde_json::to_vec_pretty(&report).map_err(|_| "report serialization failed")?;
    bytes.push(b'\n');
    write_atomic(&options.output, &bytes).map_err(|_| "upstream report write failed")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|_| "report rendering failed")?
    );
    Ok(exit_code(
        report["probes"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
    ))
}

fn exit_code(rows: &[Value]) -> i32 {
    if rows.iter().any(|row| row["outcome"] == "contract_failure") {
        1
    } else if rows
        .iter()
        .any(|row| row["outcome"] == "upstream_unavailable")
    {
        2
    } else {
        0
    }
}

fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    timestamp_from_duration(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default(),
    )
}

fn timestamp_from_duration(value: Duration) -> String {
    let seconds = value.as_secs();
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX);
    let day_seconds = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = day_seconds / 3_600;
    let minute = day_seconds % 3_600 / 60;
    let second = day_seconds % 60;
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{:06}+00:00",
        value.subsec_micros()
    )
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = (if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    }) / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    (year + if month <= 2 { 1 } else { 0 }, month, day)
}

pub fn cli(args: &[String]) -> Result<i32> {
    let options = parse_cli(args)?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|_| "workspace root unavailable")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "upstream probe runtime initialization failed")?;
    runtime.block_on(run_all(options, &root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::BTreeMap,
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct TestWire {
        status: u16,
        url_override: Option<Url>,
        headers: HeaderMap,
        chunks: Vec<Vec<u8>>,
        send_delay: Duration,
        body_delay: Duration,
        wire_error: Option<WireError>,
        request_count: Arc<AtomicUsize>,
        read_count: Arc<AtomicUsize>,
    }

    impl TestWire {
        fn new(
            status: u16,
            headers: HeaderMap,
            chunks: Vec<Vec<u8>>,
        ) -> (Self, Arc<AtomicUsize>, Arc<AtomicUsize>) {
            let request_count = Arc::new(AtomicUsize::new(0));
            let read_count = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    status,
                    url_override: None,
                    headers,
                    chunks,
                    send_delay: Duration::ZERO,
                    body_delay: Duration::ZERO,
                    wire_error: None,
                    request_count: request_count.clone(),
                    read_count: read_count.clone(),
                },
                request_count,
                read_count,
            )
        }
    }

    struct CountedChunks {
        chunks: VecDeque<Vec<u8>>,
        reads: Arc<AtomicUsize>,
        delay: Duration,
    }

    impl ResponseBody for CountedChunks {
        fn next_chunk(&mut self) -> BodyFuture<'_> {
            Box::pin(async move {
                self.reads.fetch_add(1, Ordering::SeqCst);
                if !self.delay.is_zero() {
                    tokio::time::sleep(self.delay).await;
                }
                Ok(self.chunks.pop_front())
            })
        }
    }

    impl ProbeWire for TestWire {
        fn send<'a>(&'a self, request: AdapterRequest) -> WireFuture<'a> {
            self.request_count.fetch_add(1, Ordering::SeqCst);
            let url = self.url_override.clone().unwrap_or(request.url);
            let body = CountedChunks {
                chunks: self.chunks.clone().into(),
                reads: self.read_count.clone(),
                delay: self.body_delay,
            };
            let status = self.status;
            let headers = self.headers.clone();
            let send_delay = self.send_delay;
            let wire_error = self.wire_error;
            Box::pin(async move {
                if !send_delay.is_zero() {
                    tokio::time::sleep(send_delay).await;
                }
                if let Some(error) = wire_error {
                    return Err(error);
                }
                Ok(WireResponse {
                    status,
                    url,
                    headers,
                    body: Box::new(body),
                })
            })
        }
    }

    fn request_for(probe: Probe) -> AdapterRequest {
        let mut url = if probe.path == "/works" {
            Url::parse(probe.base_url).unwrap().join("works").unwrap()
        } else {
            Url::parse(probe.base_url).unwrap()
        };
        url.query_pairs_mut()
            .extend_pairs(probe.params.iter().copied());
        let mut request = AdapterRequest::get(url.as_str()).unwrap();
        request.headers.insert(
            USER_AGENT,
            HeaderValue::from_static("SlopSearX/0.1.0 (meta search engine; agent-native)"),
        );
        request
    }

    fn content_length(length: usize) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&length.to_string()).unwrap(),
        );
        headers
    }

    #[tokio::test]
    async fn captured_fixtures_run_through_the_native_adapters_offline() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        for probe in PROBES {
            let body = fixture_payload(*probe).unwrap();
            let wire: Arc<dyn ProbeWire> = Arc::new(FixtureWire { body: body.clone() });
            let output = run_probe(*probe, wire, false, &root).await;
            assert_eq!(output.row["outcome"], "passed", "{}", probe.name);
            assert_eq!(output.row["result_count"], 3, "{}", probe.name);
            assert_eq!(output.row["requests"], 1, "{}", probe.name);
            assert_eq!(output.row["network_requests"], 0, "{}", probe.name);
            assert_eq!(
                sanitized_payload(*probe, &output.raw_body).unwrap(),
                serde_json::from_slice::<Value>(&body).unwrap()
            );
        }
    }

    fn test_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    #[tokio::test]
    async fn delayed_send_and_body_are_reported_as_probe_deadline() {
        let probe = PROBES[0];
        let deadline = Duration::from_millis(5);
        let mut send = TestWire::new(200, content_length(0), Vec::new()).0;
        send.send_delay = Duration::from_millis(30);
        let output =
            run_probe_with_deadline(probe, Arc::new(send), true, &test_root(), deadline).await;
        assert_eq!(output.row["outcome"], "upstream_unavailable");
        assert_eq!(output.row["reason"], "probe_deadline");

        let body = fixture_payload(probe).unwrap();
        let mut delayed_body = TestWire::new(200, content_length(body.len()), vec![body]).0;
        delayed_body.body_delay = Duration::from_millis(30);
        let output =
            run_probe_with_deadline(probe, Arc::new(delayed_body), true, &test_root(), deadline)
                .await;
        assert_eq!(output.row["outcome"], "upstream_unavailable");
        assert_eq!(output.row["reason"], "probe_deadline");
    }

    #[tokio::test]
    async fn upstream_http_and_network_failures_are_unavailable_and_redacted() {
        let probe = PROBES[0];
        let secret = b"provider diagnostic must not be reported";
        for status in [403, 429, 503] {
            let wire = TestWire::new(status, content_length(secret.len()), vec![secret.to_vec()]).0;
            let output =
                run_probe_with_deadline(probe, Arc::new(wire), true, &test_root(), DEADLINE).await;
            assert_eq!(output.row["outcome"], "upstream_unavailable");
            assert_eq!(output.row["reason"], "network_or_http_failure");
            assert_eq!(output.row["http_status"], status);
            assert!(!output.row.to_string().contains("provider diagnostic"));
        }

        let mut wire = TestWire::new(200, HeaderMap::new(), Vec::new()).0;
        wire.wire_error = Some(WireError::Network);
        let output =
            run_probe_with_deadline(probe, Arc::new(wire), true, &test_root(), DEADLINE).await;
        assert_eq!(output.row["outcome"], "upstream_unavailable");
        assert_eq!(output.row["reason"], "network_or_http_failure");
        assert!(!output.row.to_string().contains("provider diagnostic"));
    }

    #[tokio::test]
    async fn malformed_or_empty_successful_responses_fail_the_contract() {
        let probe = PROBES[0];
        for (body, reason) in [
            (b"not JSON".to_vec(), "successful_http_not_parsed"),
            (br#"{"objects":[]}"#.to_vec(), "empty_known_query"),
        ] {
            let wire = TestWire::new(200, content_length(body.len()), vec![body]).0;
            let output =
                run_probe_with_deadline(probe, Arc::new(wire), false, &test_root(), DEADLINE).await;
            assert_eq!(output.row["outcome"], "contract_failure");
            assert_eq!(output.row["reason"], reason);
        }
    }

    fn test_scratch(label: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "slopsearx-upstream-contracts-{}-{label}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn write_atomic_preserves_a_preexisting_temporary_file() {
        let scratch = test_scratch("preexisting-temp");
        let output = scratch.join("report.json");
        let temporary = scratch.join(format!(".report.json.{}.tmp", std::process::id()));
        fs::write(&temporary, b"belongs to another write").unwrap();

        assert!(write_atomic(&output, b"replacement").is_err());
        assert_eq!(fs::read(&temporary).unwrap(), b"belongs to another write");
        assert!(!output.exists());
        fs::remove_dir_all(scratch).unwrap();
    }

    #[tokio::test]
    async fn offline_capture_is_marked_as_replay_and_retains_fixture_provenance() {
        let scratch = test_scratch("offline-capture-provenance");
        let output = scratch.join("report.json");
        let capture_dir = scratch.join("captures");
        let code = run_all(
            CliOptions {
                output: output.clone(),
                capture_dir: Some(capture_dir.clone()),
                live: false,
            },
            &test_root(),
        )
        .await
        .unwrap();
        assert_eq!(code, 0);

        let report: Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(report["mode"], "offline_fixture");
        assert_eq!(report["probes"][1]["network_requests"], 0);
        let capture: Value =
            serde_json::from_slice(&fs::read(capture_dir.join("openalex.json")).unwrap()).unwrap();
        assert_eq!(capture["mode"], "offline_fixture_replay");
        assert!(capture.get("captured_at").is_none());
        assert!(capture["generated_at"].as_str().is_some());
        assert_eq!(capture["source"], "https://api.openalex.org/works");
        assert_eq!(
            capture["fixture"]["path"],
            "tests/fixtures/upstream_contracts/openalex.json"
        );
        assert_eq!(
            capture["fixture"]["captured_at"],
            "2026-09-07T01:51:36.864826+00:00"
        );
        fs::remove_dir_all(scratch).unwrap();
    }

    #[tokio::test]
    async fn capture_write_failure_is_reported_and_contract_failure_wins_exit_precedence() {
        let scratch = test_scratch("capture-write");
        let capture_file = scratch.join("capture-is-a-file");
        fs::write(&capture_file, b"occupied").unwrap();
        let output = scratch.join("report.json");
        let code = run_all(
            CliOptions {
                output: output.clone(),
                capture_dir: Some(capture_file),
                live: false,
            },
            &test_root(),
        )
        .await
        .unwrap();
        assert_eq!(code, 1);
        let report: Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(report["probes"][0]["outcome"], "contract_failure");
        assert_eq!(report["probes"][0]["reason"], "capture_write_failure");
        assert_eq!(exit_code(&[json!({"outcome": "upstream_unavailable"})]), 2);
        assert_eq!(exit_code(&[json!({"outcome": "passed"})]), 0);
        assert_eq!(
            exit_code(&[
                json!({"outcome": "upstream_unavailable"}),
                json!({"outcome": "contract_failure"})
            ]),
            1
        );
        fs::remove_dir_all(scratch).unwrap();
    }

    #[tokio::test]
    async fn report_write_failure_is_sanitized_and_leaves_no_partial_file() {
        let scratch = test_scratch("report-write");
        let output_directory = scratch.join("report-is-a-directory");
        fs::create_dir(&output_directory).unwrap();
        let error = run_all(
            CliOptions {
                output: output_directory.clone(),
                capture_dir: None,
                live: false,
            },
            &test_root(),
        )
        .await
        .unwrap_err();
        assert_eq!(error, "upstream report write failed");
        assert_eq!(fs::read_dir(&output_directory).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&scratch).unwrap().count(), 1);
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn fixed_request_policy_rejects_hosts_paths_queries_and_secrets() {
        let probe = PROBES[0];
        let good = request_for(probe);
        assert!(validate_request(probe, &good).is_ok());

        let mut cases: Vec<(&str, AdapterRequest)> = Vec::new();
        let mut value = good.clone();
        value.method = Method::POST;
        cases.push(("method", value));
        let mut value = good.clone();
        value.url.set_scheme("http").unwrap();
        cases.push(("scheme", value));
        let mut value = good.clone();
        value.url.set_host(Some("evil.example")).unwrap();
        cases.push(("host", value));
        let mut value = good.clone();
        value.url.set_port(Some(8443)).unwrap();
        cases.push(("port", value));
        let mut value = good.clone();
        value.url.set_username("user").unwrap();
        cases.push(("url username", value));
        let mut value = good.clone();
        value.url.set_password(Some("secret")).unwrap();
        cases.push(("url password", value));
        let mut value = good.clone();
        value.url.set_path("/other");
        cases.push(("path", value));
        let mut value = good.clone();
        value.url.set_fragment(Some("fragment"));
        cases.push(("fragment", value));
        let mut value = good.clone();
        value.url.query_pairs_mut().append_pair("extra", "value");
        cases.push(("query", value));
        let mut value = good.clone();
        value.body.push(b'x');
        cases.push(("body", value));
        let mut value = good.clone();
        value.headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer credential"),
        );
        cases.push(("authorization", value));
        let mut value = good;
        value.headers.insert(
            reqwest::header::COOKIE,
            HeaderValue::from_static("session=private"),
        );
        cases.push(("cookie", value));

        for (label, request) in cases {
            assert!(
                validate_request(probe, &request).is_err(),
                "{label} unexpectedly passed"
            );
        }
    }

    #[tokio::test]
    async fn denied_request_never_reaches_the_wire() {
        let probe = PROBES[0];
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
        let (wire, requests, _) = TestWire::new(200, headers, Vec::new());
        let bounded = BoundedTransport::new(probe, Arc::new(wire), true);
        let mut request = request_for(probe);
        request.url.set_host(Some("evil.example")).unwrap();
        assert!(bounded.exchange(request).await.is_err());
        assert_eq!(requests.load(Ordering::SeqCst), 0);
        assert_eq!(
            bounded.observation().failure,
            Some("request_outside_allowlist")
        );
    }

    #[tokio::test]
    async fn response_policy_rejects_compression_and_oversized_lengths_before_reading() {
        let probe = PROBES[0];
        let mut headers = content_length(MAX_BYTES + 1);
        let (wire, _, reads) = TestWire::new(200, headers.clone(), vec![vec![1]]);
        let bounded = BoundedTransport::new(probe, Arc::new(wire), true);
        assert!(bounded.exchange(request_for(probe)).await.is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert_eq!(bounded.observation().failure, Some("response_size_limit"));

        headers = content_length(1);
        headers.insert(CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        let (wire, _, reads) = TestWire::new(200, headers, vec![vec![1]]);
        let bounded = BoundedTransport::new(probe, Arc::new(wire), true);
        assert!(bounded.exchange(request_for(probe)).await.is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert_eq!(
            bounded.observation().failure,
            Some("unexpected_content_encoding")
        );
    }

    #[tokio::test]
    async fn unknown_length_stream_is_capped_and_exact_limit_is_accepted() {
        let probe = PROBES[0];
        let (wire, _, reads) = TestWire::new(
            200,
            HeaderMap::new(),
            vec![vec![b'x'; MAX_BYTES], vec![b'y']],
        );
        let bounded = BoundedTransport::new(probe, Arc::new(wire), true);
        assert!(bounded.exchange(request_for(probe)).await.is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert_eq!(bounded.observation().failure, Some("response_size_limit"));
        assert!(bounded.observation().raw_body.is_empty());

        let (wire, _, _) =
            TestWire::new(200, content_length(MAX_BYTES), vec![vec![b'x'; MAX_BYTES]]);
        let bounded = BoundedTransport::new(probe, Arc::new(wire), true);
        let response = bounded.exchange(request_for(probe)).await.unwrap();
        assert_eq!(response.body.len(), MAX_BYTES);
        assert_eq!(bounded.observation().failure, None);
    }

    #[tokio::test]
    async fn response_url_change_and_duplicate_lengths_fail_closed() {
        let probe = PROBES[0];
        let (mut wire, _, _) = TestWire::new(200, content_length(0), Vec::new());
        let mut changed = Url::parse("https://evil.example/").unwrap();
        changed.set_path(probe.path);
        wire.url_override = Some(changed);
        let bounded = BoundedTransport::new(probe, Arc::new(wire), true);
        assert!(bounded.exchange(request_for(probe)).await.is_err());
        assert_eq!(bounded.observation().failure, Some("unexpected_redirect"));

        let mut headers = content_length(0);
        headers.append(CONTENT_LENGTH, HeaderValue::from_static("0"));
        let (wire, _, reads) = TestWire::new(200, headers, Vec::new());
        let bounded = BoundedTransport::new(probe, Arc::new(wire), true);
        assert!(bounded.exchange(request_for(probe)).await.is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert_eq!(
            bounded.observation().failure,
            Some("ambiguous_content_length")
        );

        let mut headers = content_length(0);
        headers.append(CONTENT_ENCODING, HeaderValue::from_static("identity"));
        headers.append(CONTENT_ENCODING, HeaderValue::from_static("identity"));
        let (wire, _, reads) = TestWire::new(200, headers, Vec::new());
        let bounded = BoundedTransport::new(probe, Arc::new(wire), true);
        assert!(bounded.exchange(request_for(probe)).await.is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert_eq!(
            bounded.observation().failure,
            Some("unexpected_content_encoding")
        );
    }

    #[test]
    fn fixture_capture_projection_is_allowlisted() {
        let probe = PROBES[1];
        let body = fixture_payload(probe).unwrap();
        let projected = sanitized_payload(probe, &body).unwrap();
        let serialized = serde_json::to_string(&projected).unwrap();
        assert!(!serialized.contains("authors"));
        assert!(!serialized.contains("abstract"));
        assert!(projected["results"].as_array().unwrap().len() <= MAX_RESULTS);
    }

    #[test]
    fn cli_requires_output_and_rejects_duplicate_live_flags() {
        assert!(parse_cli(&[]).is_err());
        assert!(
            parse_cli(&[
                "--output".into(),
                "report.json".into(),
                "--live".into(),
                "--live".into()
            ])
            .is_err()
        );
        let options = parse_cli(&["--output".into(), "report.json".into()]).unwrap();
        assert!(!options.live);
    }

    #[test]
    fn status_and_url_classification_use_static_results_only() {
        assert_eq!(status_name(&EngineStatus::Ok), "ok");
        assert!(usable_url("https://registry.npmjs.org/typescript"));
        assert!(!usable_url("javascript:alert(1)"));
        assert!(!usable_url("https://example.com/package"));
    }

    #[test]
    fn report_timestamp_matches_the_source_iso8601_shape() {
        assert_eq!(
            timestamp_from_duration(Duration::from_millis(0)),
            "1970-01-01T00:00:00.000000+00:00"
        );
        assert_eq!(
            timestamp_from_duration(Duration::from_secs(951_782_400)),
            "2000-02-29T00:00:00.000000+00:00"
        );
    }

    #[test]
    fn fixed_parameter_tables_are_unique() {
        for probe in PROBES {
            let mut seen = BTreeMap::new();
            for (key, value) in probe.params {
                assert!(seen.insert(*key, *value).is_none());
            }
        }
    }
}
