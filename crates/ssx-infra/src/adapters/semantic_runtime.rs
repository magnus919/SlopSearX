//! Actual Semantic Scholar HTTP/SourceJSON/projection on one caller root.
//! AtomPool supplies lifecycle admission only: no executable/process reservation.
use super::{
    arxiv_expat::{AtomPool, RawAdmissionError},
    scholarly_config::Scalar,
    scholarly_frame::Header,
    semantic_projection, semantic_retry_after, source_json,
    source_payload::{BudgetError, Outcome, ProducedBatch},
};
use crate::http::{AdapterHttp, Cancellation, Error as HttpError, Options, Request, Response};
use ssx_application::{
    AdmissionDecision, PortFuture, SearchAdmission, SearchContext, SearchEngine,
};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest};
use ssx_owned_budget::{Budget, OwnedString};
use std::{
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[derive(Clone)]
pub struct SemanticConfig {
    pub base_url: Arc<str>,
    pub timeout_ms: Scalar,
    pub max_results: Scalar,
    pub api_key: Option<Arc<str>>,
    pub transport: Options,
    pub wall_clock: fn() -> f64,
}
fn wall_clock() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0., |v| v.as_secs_f64())
}
impl Default for SemanticConfig {
    fn default() -> Self {
        Self {
            base_url: Arc::from("https://api.semanticscholar.org/graph/v1/paper/search"),
            timeout_ms: Scalar::Integer(5000),
            max_results: Scalar::Integer(5),
            api_key: None,
            transport: Options::default(),
            wall_clock,
        }
    }
}
pub struct SemanticAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: SemanticConfig,
    lifecycle: AtomPool,
}
impl SemanticAdapter {
    pub fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: SemanticConfig,
        lifecycle: AtomPool,
    ) -> Self {
        Self {
            http,
            admission,
            config,
            lifecycle,
        }
    }
    async fn run(
        &self,
        request: &SearchRequest,
        context: Option<SearchContext>,
    ) -> AdapterResponse {
        let budget = match Budget::new(512 * 1024 * 1024) {
            Ok(v) => v,
            Err(_) => return resource(Instant::now()),
        };
        self.run_budgeted(request, context, &budget).await
    }
    async fn run_budgeted(
        &self,
        request: &SearchRequest,
        context: Option<SearchContext>,
        budget: &Budget,
    ) -> AdapterResponse {
        let start = Instant::now();
        let admitted = if let Some(c) = context {
            match self
                .http
                .admission_timeout(c.deadline, self.admission.admit("semanticscholar"))
                .await
            {
                Ok(v) => v,
                Err(_) => return bare(EngineStatus::Timeout, None, start),
            }
        } else {
            self.admission.admit("semanticscholar").await
        };
        match admitted {
            AdmissionDecision::Denied => return bare(EngineStatus::RateLimited, None, start),
            AdmissionDecision::Unavailable => {
                return bare(
                    EngineStatus::Error,
                    Some("Semantic Scholar admission unavailable"),
                    start,
                );
            }
            AdmissionDecision::Allowed => {}
        }
        if let Err(m) = self.config.timeout_ms.validate_timeout_operand(false) {
            return bare(EngineStatus::Error, Some(m), start);
        }
        let network_deadline =
            match deadline(&self.config.timeout_ms, context, self.http.deadline_now()) {
                Ok(v) => v,
                Err((s, m)) => return bare(s, m, start),
            };
        let deadline = context.map_or(network_deadline, |c| c.deadline);
        #[cfg(test)]
        self.http
            .record_scholarly_deadlines(network_deadline, deadline);
        let operation = match self
            .lifecycle
            .begin_raw_for_http(
                budget.clone(),
                deadline.into(),
                network_deadline.into(),
                &self.http,
            )
            .await
        {
            Ok(v) => v,
            Err(RawAdmissionError::Deadline) => {
                return bare(EngineStatus::Timeout, None, start);
            }
            Err(RawAdmissionError::Shutdown) => {
                return bare(
                    EngineStatus::Error,
                    Some("Semantic Scholar lifecycle shutdown"),
                    start,
                );
            }
            Err(RawAdmissionError::Saturated) => {
                return bare(
                    EngineStatus::Error,
                    Some("Semantic Scholar lifecycle saturated"),
                    start,
                );
            }
        };
        let result = operation
            .run_until_cancelled(self.execute(request, budget, deadline, network_deadline, start))
            .await;
        match result {
            Ok(Ok(v)) => v,
            Err(RawAdmissionError::Deadline) | Ok(Err(Failure::Deadline)) => {
                bare(EngineStatus::Timeout, None, start)
            }
            Err(_) => bare(
                EngineStatus::Error,
                Some("Semantic Scholar lifecycle shutdown"),
                start,
            ),
            Ok(Err(Failure::Resource)) => resource(start),
            Ok(Err(Failure::Transport(e))) => {
                bare(EngineStatus::Error, Some(transport_message(&e)), start)
            }
            Ok(Err(Failure::Source(m))) => {
                finish(
                    budget,
                    deadline,
                    EngineStatus::Error,
                    Some(m.as_str()),
                    start,
                )
                .await
            }
            Ok(Err(Failure::Static(m))) => {
                finish(budget, deadline, EngineStatus::Error, Some(m), start).await
            }
        }
    }
    async fn execute(
        &self,
        request: &SearchRequest,
        budget: &Budget,
        deadline: Instant,
        network_deadline: Instant,
        start: Instant,
    ) -> Result<AdapterResponse, Failure> {
        let params = parameters_for_clock(
            &request.query,
            &self.config.max_results,
            budget,
            deadline,
            &self.http.deadline_clock(),
        )
        .await?;
        let mut url = url::Url::parse(&self.config.base_url)
            .map_err(|_| Failure::Transport(HttpError::InvalidRequest))?;
        url.set_query(Some(params.as_str()));
        let mut options = self.config.transport.clone();
        options.follow_redirects = false;
        let mut session = self
            .http
            .session(options, network_deadline.into(), Cancellation::default())
            .map_err(http_error)?;
        let mut outgoing = Request::get(url.as_str()).map_err(http_error)?;
        outgoing.headers.insert(
            "user-agent",
            reqwest::header::HeaderValue::from_static(
                "SlopSearX/0.1.0 (meta search engine; agent-native)",
            ),
        );
        if let Some(key) = self.config.api_key.as_deref().filter(|s| !s.is_empty()) {
            outgoing.headers.insert(
                "x-api-key",
                reqwest::header::HeaderValue::from_str(key)
                    .map_err(|_| Failure::Transport(HttpError::InvalidRequest))?,
            );
        }
        let retained = charge_response(
            session.execute(outgoing).await.map_err(http_error)?,
            budget,
            deadline,
        )
        .await?;
        let response = &retained.response;
        if response.status == 429 {
            let pairs = raw_headers(response, budget, deadline).await?;
            let header = super::http_headers::get_cooperative(
                pairs.as_slice().iter().map(|h| (h.name, h.value)),
                b"retry-after",
                budget,
                deadline,
            )
            .await
            .map_err(|e| match e {
                super::http_headers::HeaderError::Output(
                    ssx_source_codecs::BoundedTextError::Deadline,
                ) => Failure::Deadline,
                _ => Failure::Resource,
            })?;
            let seconds = semantic_retry_after::retry_after(
                header.as_ref().map(|s| s.as_str()),
                (self.config.wall_clock)(),
                budget,
                deadline,
            )
            .await
            .map_err(|e| match e {
                super::scholarly_float::Error::Deadline => Failure::Deadline,
                _ => Failure::Resource,
            })?;
            self.admission
                .set_upstream_cooldown("semanticscholar", seconds)
                .await;
            let _small = budget.reserve(256).map_err(|_| Failure::Resource)?;
            let mut message = OwnedString::new(budget).map_err(|_| Failure::Resource)?;
            append(
                &mut message,
                &format!("upstream rate limited; cooldown {seconds:.0}s"),
                deadline,
            )
            .await?;
            return outcome(
                budget,
                deadline,
                EngineStatus::RateLimited,
                Some(message.as_str()),
                start,
            )
            .await;
        }
        if !(200..300).contains(&response.status) {
            return Err(Failure::Source(
                status_error(budget, response, deadline).await?,
            ));
        }
        let data =
            match source_json::from_slice_budgeted(&response.body, Some(deadline), budget).await {
                Ok(v) => v,
                Err(e) => {
                    let message = source_json::message_budgeted(
                        &e,
                        &response.body,
                        "Semantic Scholar",
                        budget,
                        Some(deadline),
                    )
                    .await
                    .map_err(|e| match e {
                        source_json::DiagnosticError::Deadline => Failure::Deadline,
                        source_json::DiagnosticError::Resource(_) => Failure::Resource,
                        source_json::DiagnosticError::Invariant => {
                            Failure::Static("Semantic Scholar native JSON diagnostic invariant")
                        }
                    })?;
                    return Err(Failure::Source(message));
                }
            };
        let batch = semantic_projection::parse(data.value(), budget, deadline)
            .await
            .map_err(|e| match e {
                semantic_projection::Error::Source(m) => Failure::Source(m),
                semantic_projection::Error::Output(m) => Failure::Static(m),
                semantic_projection::Error::Resource => Failure::Resource,
                semantic_projection::Error::Deadline => Failure::Deadline,
                semantic_projection::Error::Defect => {
                    Failure::Static("Semantic Scholar native projection invariant")
                }
            })?;
        batch
            .into_adapter_response(Outcome {
                status: EngineStatus::Ok,
                error_message: None,
                latency_ms: start.elapsed().as_secs_f64() * 1000.,
                synthetic: false,
            })
            .await
            .map_err(budget_error)
    }
}
fn budget_error(e: BudgetError) -> Failure {
    match e {
        BudgetError::Deadline => Failure::Deadline,
        _ => Failure::Resource,
    }
}
async fn outcome(
    budget: &Budget,
    deadline: Instant,
    status: EngineStatus,
    message: Option<&str>,
    start: Instant,
) -> Result<AdapterResponse, Failure> {
    ProducedBatch::new(budget, 0, Some(deadline))
        .map_err(budget_error)?
        .into_adapter_response(Outcome {
            status,
            error_message: message,
            latency_ms: start.elapsed().as_secs_f64() * 1000.,
            synthetic: false,
        })
        .await
        .map_err(budget_error)
}
async fn parameters_for_clock(
    query: &str,
    limit: &Scalar,
    budget: &Budget,
    deadline: Instant,
    clock: &crate::http::DeadlineClock,
) -> Result<OwnedString, Failure> {
    async fn encode(
        out: &mut OwnedString,
        raw: &str,
        deadline: Instant,
        clock: &crate::http::DeadlineClock,
    ) -> Result<(), Failure> {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        let cap = raw.len().checked_mul(3).ok_or(Failure::Resource)?;
        out.try_reserve_exact(cap).map_err(|_| Failure::Resource)?;
        for (i, b) in raw.bytes().enumerate() {
            if i % 1024 == 0 {
                tokio::task::yield_now().await;
                if clock.expired(deadline) {
                    return Err(Failure::Deadline);
                }
            }
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                out.try_push(char::from(b)).map_err(|_| Failure::Resource)?;
            } else if b == b' ' {
                out.try_push('+').map_err(|_| Failure::Resource)?;
            } else {
                for c in [b'%', HEX[(b >> 4) as usize], HEX[(b & 15) as usize]] {
                    out.try_push(char::from(c)).map_err(|_| Failure::Resource)?;
                }
            }
        }
        Ok(())
    }
    let mut out = OwnedString::new(budget).map_err(|_| Failure::Resource)?;
    out.try_reserve_exact(256).map_err(|_| Failure::Resource)?;
    out.try_push_str("query=").map_err(|_| Failure::Resource)?;
    encode(&mut out, query, deadline, clock).await?;
    out.try_push_str("&limit=").map_err(|_| Failure::Resource)?;
    let _numeric = budget.reserve(256).map_err(|_| Failure::Resource)?;
    let limit = limit.query_text();
    encode(&mut out, &limit, deadline, clock).await?;
    out.try_push_str(
        "&fields=title%2Curl%2Cabstract%2CcitationCount%2CpublicationDate%2CexternalIds%2Cauthors",
    )
    .map_err(|_| Failure::Resource)?;
    Ok(out)
}

impl SearchEngine for SemanticAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(request, None))
    }
    fn search_with_context<'a>(
        &'a self,
        request: &'a SearchRequest,
        context: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(request, Some(context)))
    }
}
fn deadline(
    timeout: &Scalar,
    context: Option<SearchContext>,
    now: Instant,
) -> Result<Instant, (EngineStatus, Option<&'static str>)> {
    let millis = match timeout {
        Scalar::Boolean(v) => f64::from(u8::from(*v)),
        Scalar::Integer(v) => *v as f64,
        Scalar::Float(v) => *v,
        Scalar::Text(_) => unreachable!("validated division operand"),
    };
    if millis <= 0.0 {
        return Err((EngineStatus::Timeout, None));
    }
    // An unrepresentable/nonfinite source timeout cannot extend the caller's
    // finite lifecycle limit. Without context the legacy entry fails explicitly.
    let configured = Duration::try_from_secs_f64(millis / 1000.0)
        .ok()
        .and_then(|d| now.checked_add(d));
    configured
        .into_iter()
        .chain(context.map(|c| c.deadline))
        .min()
        .ok_or((
            EngineStatus::Error,
            Some("Semantic Scholar requires a finite execution deadline"),
        ))
}
enum Failure {
    Source(OwnedString),
    Static(&'static str),
    Transport(HttpError),
    Deadline,
    Resource,
}
fn http_error(error: HttpError) -> Failure {
    match error {
        HttpError::Deadline => Failure::Deadline,
        other => Failure::Transport(other),
    }
}
async fn raw_headers<'a>(
    response: &'a Response,
    budget: &Budget,
    deadline: Instant,
) -> Result<ssx_owned_budget::OwnedVec<Header<'a>>, Failure> {
    let mut headers = ssx_owned_budget::OwnedVec::with_capacity(budget, response.headers.len())
        .map_err(|_| Failure::Resource)?;
    for (i, (name, value)) in response.headers.iter().enumerate() {
        if i.is_multiple_of(64) {
            tokio::task::yield_now().await;
            if Instant::now() >= deadline {
                return Err(Failure::Deadline);
            }
        }
        headers
            .try_push(Header {
                name: name.as_str().as_bytes(),
                value: value.as_bytes(),
            })
            .map_err(|_| Failure::Resource)?;
    }
    Ok(headers)
}
struct ChargedResponse {
    response: Response,
    _body: ssx_owned_budget::Charge,
    _visible_headers: ssx_owned_budget::Charge,
}
async fn charge_response(
    response: Response,
    budget: &Budget,
    deadline: Instant,
) -> Result<ChargedResponse, Failure> {
    // Requested body capacity is visible only after transport handoff. Earlier
    // allocation peaks, pooled storage and opaque HeaderValue backing remain G02.
    let body = budget
        .reserve(response.body.capacity())
        .map_err(|_| Failure::Resource)?;
    let mut visible = 0usize;
    for (i, (name, value)) in response.headers.iter().enumerate() {
        if i % 64 == 0 {
            tokio::task::yield_now().await;
            if Instant::now() >= deadline {
                return Err(Failure::Deadline);
            }
        }
        visible = visible
            .checked_add(name.as_str().len())
            .and_then(|n| n.checked_add(value.as_bytes().len()))
            .ok_or(Failure::Resource)?;
    }
    let headers = budget.reserve(visible).map_err(|_| Failure::Resource)?;
    Ok(ChargedResponse {
        response,
        _body: body,
        _visible_headers: headers,
    })
}
fn transport_message(error: &HttpError) -> &'static str {
    match error {
        HttpError::Closed | HttpError::Cancelled => "Semantic Scholar HTTP transport closed",
        HttpError::Network => "Semantic Scholar HTTP transport network failure",
        HttpError::InvalidRequest => "Semantic Scholar invalid HTTP request",
        HttpError::InvalidResponse => "Semantic Scholar invalid HTTP response",
        HttpError::UnsupportedEncoding => "Semantic Scholar unsupported HTTP content encoding",
        HttpError::RedirectLimit => "Semantic Scholar HTTP redirect limit exceeded",
        HttpError::RequestLimit => "Semantic Scholar HTTP request limit exceeded",
        HttpError::RequestBodyLimit => "Semantic Scholar HTTP request body limit exceeded",
        HttpError::WireBodyLimit => "Semantic Scholar HTTP wire body limit exceeded",
        HttpError::DecodedBodyLimit => "Semantic Scholar HTTP decoded body limit exceeded",
        HttpError::Deadline => "Semantic Scholar HTTP deadline exceeded",
    }
}
async fn append(out: &mut OwnedString, text: &str, deadline: Instant) -> Result<(), Failure> {
    out.try_reserve_exact(text.len())
        .map_err(|_| Failure::Resource)?;
    for (i, c) in text.chars().enumerate() {
        if i % 1024 == 0 {
            tokio::task::yield_now().await;
            if Instant::now() >= deadline {
                return Err(Failure::Deadline);
            }
        }
        out.try_push(c).map_err(|_| Failure::Resource)?;
    }
    Ok(())
}
async fn status_error(
    budget: &Budget,
    response: &Response,
    deadline: Instant,
) -> Result<OwnedString, Failure> {
    let mut out = OwnedString::new(budget).map_err(|_| Failure::Resource)?;
    let _small = budget.reserve(256).map_err(|_| Failure::Resource)?;
    let status = reqwest::StatusCode::from_u16(response.status)
        .map_err(|_| Failure::Transport(HttpError::InvalidResponse))?;
    let class = match response.status {
        100..=199 => "Informational response",
        300..=399 => "Redirect response",
        400..=499 => "Client error",
        500..=599 => "Server error",
        _ => "Invalid status code",
    };
    append(
        &mut out,
        &format!(
            "{class} '{} {}' for url '",
            response.status,
            status.canonical_reason().unwrap_or("")
        ),
        deadline,
    )
    .await?;
    append(&mut out, response.url.as_str(), deadline).await?;
    append(
        &mut out,
        "'\nFor more information check: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/",
        deadline,
    )
    .await?;
    append(&mut out, &response.status.to_string(), deadline).await?;
    Ok(out)
}
async fn finish(
    budget: &Budget,
    deadline: Instant,
    status: EngineStatus,
    message: Option<&str>,
    start: Instant,
) -> AdapterResponse {
    let result = async {
        ProducedBatch::new(budget, 0, Some(deadline))?
            .into_adapter_response(Outcome {
                status,
                error_message: message,
                latency_ms: start.elapsed().as_secs_f64() * 1000.0,
                synthetic: false,
            })
            .await
    }
    .await;
    match result {
        Ok(value) => value,
        Err(BudgetError::Deadline) => bare(EngineStatus::Timeout, None, start),
        Err(_) => resource(start),
    }
}
fn elapsed(start: Instant) -> FiniteF64 {
    FiniteF64::new(start.elapsed().as_secs_f64() * 1000.0).unwrap_or_default()
}
fn bare(status: EngineStatus, message: Option<&str>, start: Instant) -> AdapterResponse {
    AdapterResponse {
        status,
        error_message: message.map(str::to_owned),
        latency_ms: elapsed(start),
        ..Default::default()
    }
}
fn resource(start: Instant) -> AdapterResponse {
    bare(
        EngineStatus::Error,
        Some("Semantic Scholar native resource limit exceeded"),
        start,
    )
}
pub fn register(
    registry: &mut ssx_registry::Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: SemanticConfig,
    parser: AtomPool,
) -> Result<(), ssx_registry::RegistryError> {
    registry.register(
        "semanticscholar",
        Arc::new(move || {
            Box::new(SemanticAdapter::new(
                http.clone(),
                admission.clone(),
                config.clone(),
                parser.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as ssx_registry::EngineFactory,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{ExchangeFuture, Transport};
    use std::{
        future::Future,
        num::NonZeroUsize,
        path::PathBuf,
        sync::Mutex,
        task::{Context, Poll, Waker},
    };
    struct Once {
        body: Mutex<Option<Vec<u8>>>,
    }
    impl Transport for Once {
        fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
            Box::pin(async move {
                Ok(Response {
                    status: 200,
                    url: request.url,
                    headers: Default::default(),
                    body: self.body.lock().unwrap().take().unwrap(),
                })
            })
        }
    }
    #[tokio::test]
    async fn actual_json_runtime_root_refusal_cancel_and_real_deadline_need_no_helper() {
        for action in ["resource", "cancel", "deadline"] {
            let mut body = Vec::new();
            body.extend_from_slice(b"{\"data\":[{\"title\":\"");
            body.extend(std::iter::repeat_n(b'x', 512 * 1024));
            body.extend_from_slice(b"\",\"url\":\"https://example.org/\",\"citationCount\":0}]}");
            let body_capacity = body.capacity();
            let http = Arc::new(AdapterHttp::default());
            http.set_transport(Arc::new(Once {
                body: Mutex::new(Some(body)),
            }))
            .unwrap();
            let pool = AtomPool::new(
                PathBuf::from("/nonexistent-semantic-runtime-unit-helper"),
                NonZeroUsize::new(1).unwrap(),
            )
            .unwrap();
            let adapter = SemanticAdapter::new(
                http.clone(),
                Arc::new(super::super::AllowAllAdmission),
                SemanticConfig::default(),
                pool.clone(),
            );
            let budget = Budget::new(if action == "resource" {
                1024
            } else {
                64 * 1024 * 1024
            })
            .unwrap();
            let request = SearchRequest {
                query: "test".into(),
                ..Default::default()
            };
            let deadline =
                Instant::now() + Duration::from_secs(if action == "deadline" { 2 } else { 30 });
            let mut future =
                Box::pin(adapter.run_budgeted(&request, Some(SearchContext { deadline }), &budget));
            if action == "resource" {
                let response = future.await;
                assert_eq!(response.status, EngineStatus::Error);
                assert_eq!(
                    response.error_message.as_deref(),
                    Some("Semantic Scholar native resource limit exceeded")
                );
            } else {
                let mut cx = Context::from_waker(Waker::noop());
                let mut observed = 0;
                for _ in 0..100000 {
                    assert!(matches!(future.as_mut().poll(&mut cx), Poll::Pending));
                    if budget.snapshot().unwrap().used > body_capacity + 64 * 1024 {
                        observed += 1;
                        assert_eq!(pool.available_slots(), 1);
                        assert_eq!(pool.available_operations(), 0);
                        if observed == 16 {
                            break;
                        }
                    }
                }
                assert_eq!(observed, 16);
                if action == "deadline" {
                    tokio::time::sleep_until(deadline.into()).await;
                    let response = future.await;
                    assert_eq!(response.status, EngineStatus::Timeout);
                } else {
                    drop(future);
                }
            }
            assert_eq!(budget.snapshot().unwrap().used, 0);
            assert_eq!(pool.available_slots(), 1);
            assert_eq!(pool.available_operations(), 1);
            pool.shutdown().await.unwrap();
            http.shutdown_and_wait().await;
        }
    }
}
