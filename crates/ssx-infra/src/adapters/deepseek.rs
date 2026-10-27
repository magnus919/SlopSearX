//! Opt-in standalone web retrieval; no model, Fetch or hidden query operators.
use crate::{
    http::{AdapterHttp, Bounds, Cancellation, Options, Request},
    search::EngineBinding,
};
use reqwest::{
    Method,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue},
};
use ssx_application::{
    AdmissionDecision, PortFuture, SearchAdmission, SearchContext, SearchEngine,
};
use ssx_config::EngineConfig;
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use ssx_registry::{EngineFactory, Registry, RegistryError};
use std::{
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const ATTEMPTS: usize = 3;

/// Disabled configurations are ignored before credential/numeric validation.
/// Returns the binding used by the common search service; the caller owns HTTP.
pub fn register_deepseek(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: &EngineConfig,
    generation: &str,
) -> Result<Option<EngineBinding>, RegistryError> {
    if !config.enabled {
        return Ok(None);
    }
    let invalid = |message: &str| RegistryError::InvalidCatalog(format!("DeepSeek: {message}"));
    if generation.trim().is_empty() {
        return Err(invalid(
            "set a nonempty deployment generation; change it on configuration/key rotation",
        ));
    }
    let key = config
        .api_key
        .expose()
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| invalid("set ENGINE_DEEPSEEK_API_KEY when explicitly enabled"))?;
    if key.trim() != key || !key.is_ascii() || key.chars().any(char::is_control) {
        return Err(invalid("invalid ENGINE_DEEPSEEK_API_KEY"));
    }
    let mut authorization = HeaderValue::from_str(&format!("Bearer {key}"))
        .map_err(|_| invalid("invalid ENGINE_DEEPSEEK_API_KEY"))?;
    authorization.set_sensitive(true);
    let count = config
        .max_results
        .effective_i64()
        .map_err(|_| invalid("MAX_RESULTS must be 1..50"))?;
    if !(1..=50).contains(&count) {
        return Err(invalid("MAX_RESULTS must be 1..50"));
    }
    let timeout = config
        .timeout_ms
        .effective_i64()
        .map_err(|_| invalid("TIMEOUT_MS must be positive"))?;
    if !(1..=300_000).contains(&timeout) {
        return Err(invalid("TIMEOUT_MS must be 1..300000"));
    }
    let url = url::Url::parse(&config.base_url).map_err(|_| invalid("invalid search base URL"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid(
            "search base URL must be HTTP(S) without credentials, query or fragment",
        ));
    }
    let timeout = Duration::from_millis(timeout as u64);
    let adapter = Arc::new(DeepSeek {
        http,
        admission,
        authorization,
        url: url.to_string(),
        count,
        timeout,
    });
    let factory_adapter = adapter.clone();
    registry.register(
        "deepseek",
        Arc::new(move || Box::new(Shared(factory_adapter.clone())) as Box<dyn SearchEngine>)
            as EngineFactory,
    )?;
    Ok(Some(EngineBinding {
        engine: adapter,
        generation: generation.into(),
        timeout,
        scrape: false,
    }))
}
struct Shared(Arc<DeepSeek>);
impl SearchEngine for Shared {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        self.0.search(request)
    }
    fn search_with_context<'a>(
        &'a self,
        request: &'a SearchRequest,
        context: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        self.0.search_with_context(request, context)
    }
}
// No Debug implementation: credentials are never diagnostic data.
struct DeepSeek {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    authorization: HeaderValue,
    url: String,
    count: i64,
    timeout: Duration,
}
impl SearchEngine for DeepSeek {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        self.search_with_context(
            request,
            SearchContext {
                deadline: Instant::now() + self.timeout,
            },
        )
    }
    fn search_with_context<'a>(
        &'a self,
        request: &'a SearchRequest,
        context: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            let started = Instant::now();
            let deadline = context.deadline.min(started + self.timeout);
            match tokio::time::timeout_at(deadline.into(), self.run(request, deadline, started))
                .await
            {
                Ok(response) if Instant::now() < deadline => response,
                // JSON normalization is bounded synchronous work; timeout futures
                // cannot preempt a poll. Recheck before publishing its outcome.
                _ => failed(EngineStatus::Timeout, "DeepSeek deadline exceeded", started),
            }
        })
    }
}
impl DeepSeek {
    async fn run(
        &self,
        request: &SearchRequest,
        deadline: Instant,
        started: Instant,
    ) -> AdapterResponse {
        if request.page != 1 {
            return failed(
                EngineStatus::Error,
                "DeepSeek pagination unsupported; only page 1 is available",
                started,
            );
        }
        if request.query.trim().is_empty() || request.query.chars().count() > 128 {
            return failed(
                EngineStatus::Error,
                "DeepSeek query must contain 1..128 Unicode characters and not be blank",
                started,
            );
        }
        let body = serde_json::to_vec(
            &serde_json::json!({"query":request.query,"count":self.count,"need_content":false}),
        )
        .expect("bounded request JSON");
        let options = Options {
            bounds: Bounds {
                requests: ATTEMPTS,
                decoded_bytes: 2 * 1024 * 1024,
                wire_bytes: 2 * 1024 * 1024,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut session = match self
            .http
            .session(options, deadline.into(), Cancellation::default())
        {
            Ok(session) => session,
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    "DeepSeek HTTP session unavailable",
                    started,
                );
            }
        };
        for attempt in 0..ATTEMPTS {
            match self.admission.admit("deepseek").await {
                AdmissionDecision::Allowed => {}
                AdmissionDecision::Denied => {
                    return failed(
                        EngineStatus::RateLimited,
                        "DeepSeek admission denied",
                        started,
                    );
                }
                AdmissionDecision::Unavailable => {
                    return failed(
                        EngineStatus::Error,
                        "DeepSeek admission unavailable",
                        started,
                    );
                }
            }
            if Instant::now() >= deadline {
                return failed(EngineStatus::Timeout, "DeepSeek deadline exceeded", started);
            }
            let mut outbound = Request::get(&self.url).expect("validated URL");
            outbound.method = Method::POST;
            outbound
                .headers
                .insert(AUTHORIZATION, self.authorization.clone());
            outbound
                .headers
                .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            outbound.body = body.clone();
            let response = match session.execute(outbound).await {
                Ok(response) => response,
                Err(crate::http::Error::Deadline) => {
                    return failed(EngineStatus::Timeout, "DeepSeek deadline exceeded", started);
                }
                Err(_) => return failed(EngineStatus::Error, "DeepSeek request failed", started),
            };
            let retry = response.status == 408
                || response.status == 429
                || (500..600).contains(&response.status);
            let retry_after = response
                .headers
                .get("retry-after")
                .and_then(|v| v.to_str().ok());
            let delay = if retry_after.is_some() {
                let epoch = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64();
                Duration::from_secs_f64(ssx_application::retry_after::retry_after(
                    retry_after,
                    epoch,
                ))
            } else {
                Duration::from_millis(100 << attempt)
            };
            if response.status == 429 {
                self.admission
                    .set_upstream_cooldown("deepseek", delay.as_secs_f64())
                    .await;
            }
            // The shared cooldown codec caps at 300 seconds. Never use a clipped
            // value to retry earlier than the provider asked: return the outcome
            // instead and let the caller decide when to issue a new search.
            let clipped_retry_after = retry_after.is_some() && delay >= Duration::from_secs(300);
            if retry && attempt + 1 < ATTEMPTS && !clipped_retry_after {
                tokio::time::sleep(delay).await;
                continue;
            }
            match response.status {
                200 => return normalize(&response.body, started),
                401 => {
                    return failed(
                        EngineStatus::Blocked,
                        "DeepSeek authentication failed; check ENGINE_DEEPSEEK_API_KEY",
                        started,
                    );
                }
                402 => {
                    return failed(
                        EngineStatus::Blocked,
                        "DeepSeek balance insufficient; top up the provider account",
                        started,
                    );
                }
                429 => return failed(EngineStatus::RateLimited, "DeepSeek rate limited", started),
                _ => {
                    return failed(
                        EngineStatus::Error,
                        "DeepSeek returned an unsuccessful status",
                        started,
                    );
                }
            }
        }
        unreachable!("bounded attempt loop returns")
    }
}
fn normalize(body: &[u8], started: Instant) -> AdapterResponse {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return failed(
            EngineStatus::Error,
            "DeepSeek returned malformed JSON",
            started,
        );
    };
    let Some(items) = value.get("results").and_then(serde_json::Value::as_array) else {
        return failed(
            EngineStatus::Error,
            "DeepSeek response requires a results array",
            started,
        );
    };
    let results = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let raw_url = item.get("url")?.as_str()?;
            let parts = ssx_application::parsed_url(raw_url)?;
            let url = url::Url::parse(raw_url).ok()?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || parts[1].is_empty()
            {
                return None;
            }
            let title = item.get("title")?.as_str()?.to_owned();
            let content = item.get("snippet")?.as_str()?.to_owned();
            let published_date = item
                .get("date")
                .and_then(serde_json::Value::as_str)
                .filter(|raw| chrono::DateTime::parse_from_rfc3339(raw).is_ok())
                .map(str::to_owned);
            Some(SearchResult {
                url: raw_url.into(),
                title,
                content,
                engine: "deepseek".into(),
                engines: ["deepseek".into()].into(),
                position: index as i64 + 1,
                published_date,
                category: "general".into(),
                tier: 2,
                ..Default::default()
            })
        })
        .collect();
    AdapterResponse {
        results,
        latency_ms: FiniteF64::new(started.elapsed().as_secs_f64() * 1000.).unwrap(),
        ..Default::default()
    }
}
fn failed(status: EngineStatus, message: &str, started: Instant) -> AdapterResponse {
    AdapterResponse {
        status,
        error_message: Some(message.into()),
        latency_ms: FiniteF64::new(started.elapsed().as_secs_f64() * 1000.).unwrap(),
        ..Default::default()
    }
}
