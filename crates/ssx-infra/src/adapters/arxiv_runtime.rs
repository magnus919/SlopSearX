//! Actual arXiv HTTP path through the admitted, supervised raw Atom helper.
//! F08/G02 still owns the open transport backing/allocation qualification.
#[path = "arxiv_redirect.rs"]
mod redirect;
use super::{
    arxiv_expat::{AtomPool, RawAdmissionError},
    scholarly_config::Scalar,
    scholarly_frame::Header,
    scholarly_wire,
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
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct ArxivConfig {
    pub base_url: Arc<str>,
    pub timeout_ms: Scalar,
    pub max_results: Scalar,
    pub transport: Options,
}
impl Default for ArxivConfig {
    fn default() -> Self {
        Self {
            base_url: Arc::from("https://export.arxiv.org/api/query"),
            timeout_ms: Scalar::Integer(10_000),
            max_results: Scalar::Integer(5),
            transport: Options::default(),
        }
    }
}
pub struct ArxivAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: ArxivConfig,
    parser: AtomPool,
}
impl ArxivAdapter {
    pub fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: ArxivConfig,
        parser: AtomPool,
    ) -> Self {
        Self {
            http,
            admission,
            config,
            parser,
        }
    }
    async fn run(
        &self,
        request: &SearchRequest,
        context: Option<SearchContext>,
    ) -> AdapterResponse {
        let budget = match Budget::new(512 * 1024 * 1024) {
            Ok(value) => value,
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
        let admission = if let Some(context) = context {
            match self
                .http
                .admission_timeout(context.deadline, self.admission.admit("arxiv"))
                .await
            {
                Ok(value) => value,
                Err(_) => return bare(EngineStatus::Timeout, None, start),
            }
        } else {
            self.admission.admit("arxiv").await
        };
        match admission {
            AdmissionDecision::Denied => return bare(EngineStatus::RateLimited, None, start),
            AdmissionDecision::Unavailable => {
                return bare(
                    EngineStatus::Error,
                    Some("arXiv admission unavailable"),
                    start,
                );
            }
            AdmissionDecision::Allowed => {}
        }
        if let Err(message) = self.config.timeout_ms.validate_timeout_operand(false) {
            return bare(EngineStatus::Error, Some(message), start);
        }
        let network_deadline =
            match deadline(&self.config.timeout_ms, context, self.http.deadline_now()) {
                Ok(value) => value,
                Err((status, message)) => return bare(status, message, start),
            };
        let deadline = context.map_or(network_deadline, |c| c.deadline);
        #[cfg(test)]
        self.http
            .record_scholarly_deadlines(network_deadline, deadline);
        let operation =
            match self
                .parser
                .begin_raw_for_http(budget.clone(), deadline.into(), &self.http)
            {
                Ok(value) => value,
                Err(RawAdmissionError::Deadline) => {
                    return bare(EngineStatus::Timeout, None, start);
                }
                Err(RawAdmissionError::Shutdown) => {
                    return bare(
                        EngineStatus::Error,
                        Some("arXiv native XML parser shutdown"),
                        start,
                    );
                }
                Err(RawAdmissionError::Saturated) => {
                    return bare(
                        EngineStatus::Error,
                        Some("arXiv native XML parser saturated"),
                        start,
                    );
                }
            };
        let fetched = operation
            .run_until_cancelled(self.fetch(request, budget, deadline, network_deadline))
            .await;
        let response = match fetched {
            Err(RawAdmissionError::Deadline) => return bare(EngineStatus::Timeout, None, start),
            Err(_) => {
                return bare(
                    EngineStatus::Error,
                    Some("arXiv native XML parser shutdown"),
                    start,
                );
            }
            Ok(Err(Failure::Deadline)) => return bare(EngineStatus::Timeout, None, start),
            Ok(Err(Failure::Resource)) => return resource(start),
            Ok(Err(Failure::Transport(error))) => {
                return bare(EngineStatus::Error, Some(transport_message(&error)), start);
            }
            Ok(Err(Failure::Source(message))) => {
                return finish(
                    budget,
                    deadline,
                    EngineStatus::Error,
                    Some(message.as_str()),
                    start,
                )
                .await;
            }
            Ok(Ok(response)) => response,
        };
        let response = &response.response;
        if response.status == 429 {
            return finish(budget, deadline, EngineStatus::RateLimited, None, start).await;
        }
        if !(200..300).contains(&response.status) {
            let message = match status_error(budget, response, deadline).await {
                Ok(value) => value,
                Err(Failure::Deadline) => return bare(EngineStatus::Timeout, None, start),
                Err(Failure::Resource) => return resource(start),
                Err(Failure::Transport(error)) => {
                    return bare(EngineStatus::Error, Some(transport_message(&error)), start);
                }
                Err(Failure::Source(message)) => {
                    return finish(
                        budget,
                        deadline,
                        EngineStatus::Error,
                        Some(message.as_str()),
                        start,
                    )
                    .await;
                }
            };
            return finish(
                budget,
                deadline,
                EngineStatus::Error,
                Some(message.as_str()),
                start,
            )
            .await;
        }
        // HeaderMap::Iter is not Clone. The explicit borrowed descriptors are
        // charged before allocation; value/name bytes remain borrowed, with no
        // String escape or uncharged pair collection.
        let headers = operation
            .run_until_cancelled(raw_headers(response, budget, deadline))
            .await;
        let headers = match headers {
            Ok(Ok(headers)) => headers,
            Ok(Err(Failure::Deadline)) | Err(RawAdmissionError::Deadline) => {
                return bare(EngineStatus::Timeout, None, start);
            }
            Err(_) => {
                return bare(
                    EngineStatus::Error,
                    Some("arXiv native XML parser shutdown"),
                    start,
                );
            }
            Ok(Err(_)) => return resource(start),
        };
        let mut outcome = operation
            .parse(headers.as_slice().iter().copied(), &response.body)
            .await;
        outcome.latency_ms = elapsed(start);
        outcome
    }
    async fn fetch(
        &self,
        request: &SearchRequest,
        budget: &Budget,
        deadline: Instant,
        network_deadline: Instant,
    ) -> Result<ChargedResponse, Failure> {
        let parameters = scholarly_wire::arxiv_parameters_for_clock(
            budget,
            &request.query,
            &self.config.max_results,
            deadline,
            &self.http.deadline_clock(),
        )
        .await
        .map_err(|e| match e {
            BudgetError::Deadline => Failure::Deadline,
            _ => Failure::Resource,
        })?;
        // Url, HeaderMap and socket ownership are the shared F08/G02 boundary.
        // The source-owned encoded parameter buffer remains charged throughout.
        let mut url = url::Url::parse(&self.config.base_url)
            .map_err(|_| Failure::Transport(HttpError::InvalidRequest))?;
        url.set_query(Some(parameters.as_str()));
        let mut options = self.config.transport.clone();
        options.follow_redirects = false;
        let mut session = self
            .http
            .session(options, network_deadline.into(), Cancellation::default())
            .map_err(http_error)?;
        let outgoing = |url: &str| -> Result<Request, Failure> {
            let mut out = Request::get(url).map_err(http_error)?;
            out.headers.insert(
                "user-agent",
                reqwest::header::HeaderValue::from_static(
                    "SlopSearX/0.1.0 (meta search engine; agent-native)",
                ),
            );
            out.headers.insert(
                "accept",
                reqwest::header::HeaderValue::from_static("application/atom+xml"),
            );
            Ok(out)
        };
        let mut retained = charge_response(
            session
                .execute(outgoing(url.as_str())?)
                .await
                .map_err(http_error)?,
            budget,
            deadline,
        )
        .await?;
        let response = &retained.response;
        if response.status != 429 && (300..400).contains(&response.status) {
            let pairs = raw_headers(response, budget, deadline).await?;
            let location = super::http_headers::get_cooperative(
                pairs
                    .as_slice()
                    .iter()
                    .map(|header| (header.name, header.value)),
                b"location",
                budget,
                deadline,
            )
            .await
            .map_err(|error| match error {
                super::http_headers::HeaderError::Output(
                    ssx_source_codecs::BoundedTextError::Deadline,
                ) => Failure::Deadline,
                _ => Failure::Resource,
            })?;
            let Some(location) = location.filter(|value| !value.as_str().is_empty()) else {
                let mut message = OwnedString::new(budget).map_err(|_| Failure::Resource)?;
                let _small = budget.reserve(128).map_err(|_| Failure::Resource)?;
                append(
                    &mut message,
                    &format!(
                        "redirect response {} missing Location header",
                        response.status
                    ),
                    deadline,
                )
                .await?;
                return Err(Failure::Source(message));
            };
            let target =
                redirect::target(&self.config.base_url, location.as_str(), budget, deadline)
                    .await?;
            retained = charge_response(
                session
                    .execute(outgoing(target.as_str())?)
                    .await
                    .map_err(http_error)?,
                budget,
                deadline,
            )
            .await?;
        }
        Ok(retained)
    }
}
impl SearchEngine for ArxivAdapter {
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
            Some("arXiv requires a finite execution deadline"),
        ))
}
enum Failure {
    Source(OwnedString),
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
        HttpError::Closed | HttpError::Cancelled => "arXiv HTTP transport closed",
        HttpError::Network => "arXiv HTTP transport network failure",
        HttpError::InvalidRequest => "arXiv invalid HTTP request",
        HttpError::InvalidResponse => "arXiv invalid HTTP response",
        HttpError::UnsupportedEncoding => "arXiv unsupported HTTP content encoding",
        HttpError::RedirectLimit => "arXiv HTTP redirect limit exceeded",
        HttpError::RequestLimit => "arXiv HTTP request limit exceeded",
        HttpError::RequestBodyLimit => "arXiv HTTP request body limit exceeded",
        HttpError::WireBodyLimit => "arXiv HTTP wire body limit exceeded",
        HttpError::DecodedBodyLimit => "arXiv HTTP decoded body limit exceeded",
        HttpError::Deadline => "arXiv HTTP deadline exceeded",
    }
}
async fn append(out: &mut OwnedString, text: &str, deadline: Instant) -> Result<(), Failure> {
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
        Some("arXiv native resource limit exceeded"),
        start,
    )
}
/// Register the actual arXiv factory with caller-owned transport/admission/helper
/// lifetimes. Other scholarly factories are added only after their own runtime
/// qualification; registering this adapter does not activate the legacy parser.
pub fn register(
    registry: &mut ssx_registry::Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: ArxivConfig,
    parser: AtomPool,
) -> Result<(), ssx_registry::RegistryError> {
    registry.register(
        "arxiv",
        Arc::new(move || {
            Box::new(ArxivAdapter::new(
                http.clone(),
                admission.clone(),
                config.clone(),
                parser.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as ssx_registry::EngineFactory,
    )
}

#[cfg(test)]
#[path = "arxiv_runtime_tests.rs"]
mod tests;
