//! Actual OpenAlex HTTP/SourceJSON/owned sorting/projection on one caller root.
//! F08/G02 opaque transport/URL/public lifetime qualification remains separate.
use super::{
    arxiv_expat::{AtomPool, RawAdmissionError},
    openalex_projection,
    scholarly_config::Scalar,
    source_json,
    source_payload::{BudgetError, Outcome, ProducedBatch},
};
use crate::http::{AdapterHttp, Cancellation, Error as HttpError, Options, Request, Response};
use ssx_application::{
    AdmissionDecision, PortFuture, SearchAdmission, SearchContext, SearchEngine,
    filters::PublicationBounds,
};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest};
use ssx_owned_budget::{Budget, OwnedString};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
#[derive(Clone, Debug)]
pub struct OpenAlexConfig {
    pub base_url: Arc<str>,
    pub timeout_ms: Scalar,
    pub max_results: Scalar,
    pub transport: Options,
}
impl Default for OpenAlexConfig {
    fn default() -> Self {
        Self {
            base_url: Arc::from("https://api.openalex.org"),
            timeout_ms: Scalar::Integer(5000),
            max_results: Scalar::Integer(10),
            transport: Options::default(),
        }
    }
}
pub struct OpenAlexAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: OpenAlexConfig,
    lifecycle: AtomPool,
}
impl OpenAlexAdapter {
    pub fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: OpenAlexConfig,
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
        // Shared canonical validator runs before any admission/acquisition, matching
        // source ordering. Diagnostic text is static until copied by caller root.
        let bounds = match PublicationBounds::new_borrowed(
            request.date_from.as_deref(),
            request.date_to.as_deref(),
        ) {
            Ok(v) => v,
            Err(e) => {
                return preflight_failure(budget, context, e.message, start).await;
            }
        };
        if request.time_range.as_deref().is_some_and(|r| {
            !r.is_empty() && ssx_application::filters::time_range_days(r).is_none()
        }) {
            return preflight_failure(
                budget,
                context,
                "time_range must be day, week, month, or year",
                start,
            )
            .await;
        }
        let admitted = if let Some(c) = context {
            match self
                .http
                .admission_timeout(c.deadline, self.admission.admit("openalex"))
                .await
            {
                Ok(v) => v,
                Err(_) => return bare(EngineStatus::Timeout, None, start),
            }
        } else {
            self.admission.admit("openalex").await
        };
        match admitted {
            AdmissionDecision::Denied => return bare(EngineStatus::RateLimited, None, start),
            AdmissionDecision::Unavailable => {
                return bare(
                    EngineStatus::Error,
                    Some("OpenAlex admission unavailable"),
                    start,
                );
            }
            AdmissionDecision::Allowed => {}
        }
        if let Err(m) = self.config.timeout_ms.validate_timeout_operand(true) {
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
        let operation =
            match self
                .lifecycle
                .begin_raw_for_http(budget.clone(), deadline.into(), &self.http)
            {
                Ok(v) => v,
                Err(RawAdmissionError::Deadline) => {
                    return bare(EngineStatus::Timeout, None, start);
                }
                Err(RawAdmissionError::Shutdown) => {
                    return bare(
                        EngineStatus::Error,
                        Some("OpenAlex lifecycle shutdown"),
                        start,
                    );
                }
                Err(RawAdmissionError::Saturated) => {
                    return bare(
                        EngineStatus::Error,
                        Some("OpenAlex lifecycle saturated"),
                        start,
                    );
                }
            };
        let result = operation
            .run_until_cancelled(self.execute(
                request,
                bounds,
                budget,
                deadline,
                network_deadline,
                start,
            ))
            .await;
        match result {
            Ok(Ok(v)) => v,
            Err(RawAdmissionError::Deadline) | Ok(Err(Failure::Deadline)) => {
                bare(EngineStatus::Timeout, None, start)
            }
            Err(_) => bare(
                EngineStatus::Error,
                Some("OpenAlex lifecycle shutdown"),
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
            Ok(Err(Failure::RateLimited(m))) => {
                finish(
                    budget,
                    deadline,
                    EngineStatus::RateLimited,
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
        bounds: PublicationBounds,
        budget: &Budget,
        deadline: Instant,
        network_deadline: Instant,
        start: Instant,
    ) -> Result<AdapterResponse, Failure> {
        let url = outgoing_url_for_clock(
            &self.config.base_url,
            &request.query,
            &self.config.max_results,
            request.date_from.as_deref(),
            request.date_to.as_deref(),
            budget,
            deadline,
            &self.http.deadline_clock(),
        )
        .await?;
        let mut options = self.config.transport.clone();
        options.follow_redirects = false;
        let mut session = self
            .http
            .session(options, network_deadline.into(), Cancellation::default())
            .map_err(http_error)?;
        // No engine-owned headers in the source. HTTPX default headers belong to the
        // explicit F08 transport compatibility boundary, not an invented engine UA.
        let outgoing = Request::get(url.as_str()).map_err(http_error)?;
        let retained = charge_response(
            session.execute(outgoing).await.map_err(http_error)?,
            budget,
            deadline,
        )
        .await?;
        let response = &retained.response;
        if !(200..300).contains(&response.status) {
            let m = status_error(budget, response, deadline).await?;
            return Err(if response.status == 429 {
                Failure::RateLimited(m)
            } else {
                Failure::Source(m)
            });
        }
        let data =
            match source_json::from_slice_budgeted(&response.body, Some(deadline), budget).await {
                Ok(v) => v,
                Err(e) => {
                    let m = source_json::message_budgeted(
                        &e,
                        &response.body,
                        "OpenAlex",
                        budget,
                        Some(deadline),
                    )
                    .await
                    .map_err(|e| match e {
                        source_json::DiagnosticError::Deadline => Failure::Deadline,
                        source_json::DiagnosticError::Resource(_) => Failure::Resource,
                        source_json::DiagnosticError::Invariant => {
                            Failure::Static("OpenAlex native JSON diagnostic invariant")
                        }
                    })?;
                    return Err(Failure::Source(m));
                }
            };
        let batch = openalex_projection::parse(
            data.value(),
            &self.config.max_results,
            bounds,
            request.date_from.is_some() || request.date_to.is_some(),
            budget,
            deadline,
        )
        .await
        .map_err(|e| match e {
            openalex_projection::Error::Source(m) => Failure::Source(m),
            openalex_projection::Error::Output(m) => Failure::Static(m),
            openalex_projection::Error::Resource => Failure::Resource,
            openalex_projection::Error::Deadline => Failure::Deadline,
            openalex_projection::Error::Defect => {
                Failure::Static("OpenAlex native projection invariant")
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
// Preserve the original request inputs and explicit owner clock as separate dependencies.
#[allow(clippy::too_many_arguments)]
async fn outgoing_url_for_clock(
    base: &str,
    query: &str,
    limit: &Scalar,
    lower: Option<&str>,
    upper: Option<&str>,
    budget: &Budget,
    deadline: Instant,
    clock: &crate::http::DeadlineClock,
) -> Result<OwnedString, Failure> {
    async fn quote(
        out: &mut OwnedString,
        text: &str,
        deadline: Instant,
        clock: &crate::http::DeadlineClock,
    ) -> Result<(), Failure> {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        out.try_reserve_exact(text.len().checked_mul(3).ok_or(Failure::Resource)?)
            .map_err(|_| Failure::Resource)?;
        for (i, b) in text.bytes().enumerate() {
            if i % 1024 == 0 {
                tokio::task::yield_now().await;
                if clock.expired(deadline) {
                    return Err(Failure::Deadline);
                }
            }
            if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
                out.try_push(char::from(b)).map_err(|_| Failure::Resource)?;
            } else {
                for c in [b'%', HEX[(b >> 4) as usize], HEX[(b & 15) as usize]] {
                    out.try_push(char::from(c)).map_err(|_| Failure::Resource)?;
                }
            }
        }
        Ok(())
    }
    let mut out = OwnedString::new(budget).map_err(|_| Failure::Resource)?;
    append_for_clock(&mut out, base, deadline, clock).await?;
    append_for_clock(&mut out, "/works?search=", deadline, clock).await?;
    quote(&mut out, query, deadline, clock).await?;
    append_for_clock(&mut out, "&per_page=", deadline, clock).await?;
    let _numeric = budget.reserve(256).map_err(|_| Failure::Resource)?;
    append_for_clock(&mut out, &limit.python_text(), deadline, clock).await?;
    if lower.is_some() || upper.is_some() {
        append_for_clock(&mut out, "&filter=", deadline, clock).await?;
        if let Some(lower) = lower {
            append_for_clock(&mut out, "from_publication_date%3A", deadline, clock).await?;
            quote(&mut out, lower, deadline, clock).await?;
        }
        if let Some(upper) = upper {
            if lower.is_some() {
                append_for_clock(&mut out, "%2C", deadline, clock).await?;
            }
            append_for_clock(&mut out, "to_publication_date%3A", deadline, clock).await?;
            quote(&mut out, upper, deadline, clock).await?;
        }
    }
    Ok(out)
}

async fn preflight_failure(
    budget: &Budget,
    context: Option<SearchContext>,
    message: &str,
    start: Instant,
) -> AdapterResponse {
    let result = async {
        ProducedBatch::new(budget, 0, context.map(|c| c.deadline))?
            .into_adapter_response(Outcome {
                status: EngineStatus::Error,
                error_message: Some(message),
                latency_ms: start.elapsed().as_secs_f64() * 1000.,
                synthetic: false,
            })
            .await
    }
    .await;
    match result {
        Ok(v) => v,
        Err(BudgetError::Deadline) => bare(EngineStatus::Timeout, None, start),
        Err(_) => resource(start),
    }
}

fn budget_error(e: BudgetError) -> Failure {
    match e {
        BudgetError::Deadline => Failure::Deadline,
        _ => Failure::Resource,
    }
}
impl SearchEngine for OpenAlexAdapter {
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
            Some("OpenAlex requires a finite execution deadline"),
        ))
}
enum Failure {
    Source(OwnedString),
    RateLimited(OwnedString),
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
        HttpError::Closed | HttpError::Cancelled => "OpenAlex HTTP transport closed",
        HttpError::Network => "OpenAlex HTTP transport network failure",
        HttpError::InvalidRequest => "OpenAlex invalid HTTP request",
        HttpError::InvalidResponse => "OpenAlex invalid HTTP response",
        HttpError::UnsupportedEncoding => "OpenAlex unsupported HTTP content encoding",
        HttpError::RedirectLimit => "OpenAlex HTTP redirect limit exceeded",
        HttpError::RequestLimit => "OpenAlex HTTP request limit exceeded",
        HttpError::RequestBodyLimit => "OpenAlex HTTP request body limit exceeded",
        HttpError::WireBodyLimit => "OpenAlex HTTP wire body limit exceeded",
        HttpError::DecodedBodyLimit => "OpenAlex HTTP decoded body limit exceeded",
        HttpError::Deadline => "OpenAlex HTTP deadline exceeded",
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
async fn append_for_clock(
    out: &mut OwnedString,
    text: &str,
    deadline: Instant,
    clock: &crate::http::DeadlineClock,
) -> Result<(), Failure> {
    out.try_reserve_exact(text.len())
        .map_err(|_| Failure::Resource)?;
    for (i, c) in text.chars().enumerate() {
        if i % 1024 == 0 {
            tokio::task::yield_now().await;
            if clock.expired(deadline) {
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
        Some("OpenAlex native resource limit exceeded"),
        start,
    )
}
pub fn register(
    registry: &mut ssx_registry::Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: OpenAlexConfig,
    parser: AtomPool,
) -> Result<(), ssx_registry::RegistryError> {
    registry.register(
        "openalex",
        Arc::new(move || {
            Box::new(OpenAlexAdapter::new(
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
    async fn actual_runtime_root_refusal_charged_cpu_drop_and_real_deadline_need_no_helper() {
        for action in ["resource", "cancel", "deadline"] {
            let mut body = Vec::new();
            body.extend_from_slice(b"{\"results\":[{\"title\":\"Paper\",\"id\":\"https://openalex.org/W1\",\"abstract_inverted_index\":{\"");
            body.extend(std::iter::repeat_n(b'x', 512 * 1024));
            body.extend_from_slice(b"\":[0]}}]}");
            let body_capacity = body.capacity();
            let http = Arc::new(AdapterHttp::default());
            http.set_transport(Arc::new(Once {
                body: Mutex::new(Some(body)),
            }))
            .unwrap();
            let pool = AtomPool::new(
                PathBuf::from("/nonexistent-openalex-root-unit-helper"),
                NonZeroUsize::new(1).unwrap(),
            )
            .unwrap();
            let adapter = OpenAlexAdapter::new(
                http.clone(),
                Arc::new(super::super::AllowAllAdmission),
                OpenAlexConfig::default(),
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
                let result = future.await;
                assert_eq!(result.status, EngineStatus::Error);
                assert_eq!(
                    result.error_message.as_deref(),
                    Some("OpenAlex native resource limit exceeded")
                );
            } else {
                let mut context = Context::from_waker(Waker::noop());
                let mut observed = 0;
                for _ in 0..100000 {
                    assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
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
                    assert_eq!(future.await.status, EngineStatus::Timeout);
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
