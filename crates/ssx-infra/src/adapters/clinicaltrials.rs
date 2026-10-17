//! ClinicalTrials.gov and private mechanics for the A07 biomedical batch.
//! These helpers are batch owned, not a replacement shared transport or codec.
use super::source_json::{self, Map, Number, Text, Value};
use crate::http::{AdapterHttp, Cancellation, Options, Request};
use ssx_application::{
    AdmissionDecision, PortFuture, SearchAdmission, SearchContext, SearchEngine,
};
use ssx_config::ConfigScalar;
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) struct Failure {
    status: EngineStatus,
    message: Option<String>,
    _charge: Option<ssx_owned_budget::Charge>,
}
impl std::fmt::Debug for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Failure")
            .field("status", &self.status)
            .field("message", &self.message)
            .finish()
    }
}
impl Failure {
    pub(super) fn empty_timeout_diagnostic(mut self) -> Self {
        if self.status == EngineStatus::Timeout {
            self.message = Some(String::new());
        }
        self
    }
}
pub(super) type Result<T> = std::result::Result<T, Failure>;
/// Retain F06's raw resolved scalars: YAML and environment coercion happen upstream.
#[derive(Clone, Debug)]
pub struct BiomedicalConfig {
    pub base_url: String,
    pub timeout_ms: ConfigScalar,
    pub max_results: ConfigScalar,
    /// Direct adapter override; absent from Python EngineEntry and not invented as an env field.
    pub aggregate_timeout_ms: Option<ConfigScalar>,
}
impl BiomedicalConfig {
    pub fn defaults(base: &str) -> Self {
        Self {
            base_url: base.into(),
            timeout_ms: ConfigScalar::Integer(5000),
            max_results: ConfigScalar::Integer(10),
            aggregate_timeout_ms: None,
        }
    }
    pub fn from_resolved(config: &ssx_config::EngineConfig) -> Self {
        Self {
            base_url: config.base_url.clone(),
            timeout_ms: config.timeout_ms.clone(),
            max_results: config.max_results.clone(),
            aggregate_timeout_ms: None,
        }
    }
}
pub(super) fn error(message: impl Into<String>) -> Failure {
    Failure {
        status: EngineStatus::Error,
        message: Some(message.into()),
        _charge: None,
    }
}
pub(super) fn timeout() -> Failure {
    Failure {
        status: EngineStatus::Timeout,
        message: None,
        _charge: None,
    }
}
pub(super) fn elapsed(start: Instant) -> FiniteF64 {
    FiniteF64::new(start.elapsed().as_secs_f64() * 1000.).unwrap_or_default()
}
pub(super) fn outcome(result: Result<Vec<SearchResult>>, start: Instant) -> AdapterResponse {
    match result {
        Ok(results) => AdapterResponse {
            results,
            status: EngineStatus::Ok,
            latency_ms: elapsed(start),
            ..AdapterResponse::default()
        },
        Err(Failure {
            status,
            message,
            _charge,
        }) => {
            let response = AdapterResponse {
                status,
                error_message: message,
                latency_ms: elapsed(start),
                ..AdapterResponse::default()
            };
            drop(_charge);
            response
        }
    }
}
pub(super) async fn admit(admission: &Arc<dyn SearchAdmission>, name: &'static str) -> Result<()> {
    match admission.admit(name).await {
        AdmissionDecision::Allowed => Ok(()),
        AdmissionDecision::Denied => Err(Failure {
            status: EngineStatus::RateLimited,
            message: Some("rate limited".into()),
            _charge: None,
        }),
        AdmissionDecision::Unavailable => Err(error(format!("{name} rate policy unavailable"))),
    }
}
pub(super) async fn admit_with_context(
    admission: &Arc<dyn SearchAdmission>,
    name: &'static str,
    context: Option<Instant>,
) -> Result<()> {
    if let Some(deadline) = context {
        check(deadline)?;
        let admitted = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            admit(admission, name),
        )
        .await
        .map_err(|_| timeout())?;
        check(deadline)?;
        admitted
    } else {
        admit(admission, name).await
    }
}
pub(super) fn network_deadline(
    config: &BiomedicalConfig,
    pubmed: bool,
    context: Option<Instant>,
) -> Result<Instant> {
    network_deadline_using(config, pubmed, context, Instant::now)
}
fn network_deadline_using(
    config: &BiomedicalConfig,
    pubmed: bool,
    context: Option<Instant>,
    now: impl FnOnce() -> Instant,
) -> Result<Instant> {
    if !pubmed {
        let millis = numeric(&config.timeout_ms, false).map_err(|_| error("invalid timeout_ms"))?;
        if millis <= 0. {
            return Err(timeout());
        }
        if millis.is_nan() {
            return Err(error("invalid timeout_ms"));
        }
        let configured = Duration::try_from_secs_f64(millis / 1000.)
            .ok()
            .and_then(|duration| now().checked_add(duration));
        return match (configured, context) {
            (Some(configured), Some(limit)) => Ok(limit.min(configured)),
            (Some(configured), None) => Ok(configured),
            (None, Some(limit)) => Ok(limit),
            (None, None) => Err(error("timeout requires a finite lifecycle deadline")),
        };
    }
    let configured = deadline(config, true)?;
    Ok(context.map_or(configured, |limit| limit.min(configured)))
}
/// Only direct non-PubMed requests need the HTTP owner's configured timer.
/// Production deadline_now is Instant::now; caller context remains unchanged.
pub(super) fn network_deadline_for_http(
    http: &AdapterHttp,
    config: &BiomedicalConfig,
    context: Option<Instant>,
) -> Result<Instant> {
    #[cfg(test)]
    http.assert_direct_replay_context(context);
    network_deadline_using(config, false, context, || http.deadline_now())
}
pub(super) fn scalar_text(v: &ConfigScalar) -> String {
    match v {
        ConfigScalar::Boolean(v) => if *v { "true" } else { "false" }.into(),
        ConfigScalar::Integer(v) => v.to_string(),
        ConfigScalar::Float(v) => ssx_json::python_float(*v).unwrap_or_else(|_| v.to_string()),
        ConfigScalar::Text(v) => v.clone(),
    }
}
pub(super) fn numeric(v: &ConfigScalar, coerce: bool) -> Result<f64> {
    match v {
        ConfigScalar::Boolean(v) => Ok(if *v { 1. } else { 0. }),
        ConfigScalar::Integer(v) => Ok(*v as f64),
        ConfigScalar::Float(v) => Ok(*v),
        ConfigScalar::Text(v) if coerce => ssx_domain::storage::normalize_python_number_text(v)
            .map_err(|_| error("invalid numeric setting"))?
            .parse()
            .map_err(|_| error("invalid numeric setting")),
        _ => Err(error("setting does not support arithmetic")),
    }
}
pub(super) fn slice_end(config: &ConfigScalar, len: usize) -> Result<usize> {
    let n = match config {
        ConfigScalar::Boolean(v) => i64::from(*v),
        ConfigScalar::Integer(v) => *v,
        _ => return Err(error("slice indices must be integers")),
    };
    Ok(if n < 0 {
        len.saturating_sub(n.unsigned_abs().min(usize::MAX as u64) as usize)
    } else {
        len.min(n as usize)
    })
}
pub(super) fn int_value(v: &ConfigScalar) -> Result<i64> {
    match v {
        ConfigScalar::Boolean(v) => Ok(i64::from(*v)),
        ConfigScalar::Integer(v) => Ok(*v),
        ConfigScalar::Float(v)
            if v.is_finite() && *v >= i64::MIN as f64 && *v < i64::MAX as f64 =>
        {
            Ok(*v as i64)
        }
        ConfigScalar::Text(v) => ssx_domain::storage::normalize_python_number_text(v)
            .map_err(|_| error("invalid integer setting"))?
            .parse()
            .map_err(|_| error("invalid integer setting")),
        _ => Err(error("invalid integer setting")),
    }
}
pub(super) fn deadline(config: &BiomedicalConfig, pubmed: bool) -> Result<Instant> {
    let millis = numeric(&config.timeout_ms, pubmed).map_err(|_| {
        error(if pubmed {
            "timeout_ms must be a positive finite number"
        } else {
            "invalid timeout_ms"
        })
    })?;
    if !millis.is_finite()
        || millis < 0.
        || (pubmed && (millis == 0. || matches!(config.timeout_ms, ConfigScalar::Boolean(_))))
    {
        return Err(error(if pubmed {
            "timeout_ms must be a positive finite number"
        } else {
            "invalid timeout_ms"
        }));
    }
    let aggregate = if pubmed {
        if let Some(v) = &config.aggregate_timeout_ms {
            if matches!(v, ConfigScalar::Boolean(_)) {
                return Err(error(
                    "aggregate_timeout_ms must be a positive finite number",
                ));
            }
            let n = numeric(v, true)
                .map_err(|_| error("aggregate_timeout_ms must be a positive finite number"))?;
            if n > millis {
                return Err(error("aggregate_timeout_ms cannot exceed timeout_ms"));
            }
            if !n.is_finite() || n <= 0. {
                return Err(error(
                    "aggregate_timeout_ms must be a positive finite number",
                ));
            }
            n
        } else {
            millis * 0.9
        }
    } else {
        millis
    };
    if pubmed && (!aggregate.is_finite() || aggregate <= 0.) {
        return Err(error(
            "aggregate_timeout_ms must be a positive finite number",
        ));
    }
    let duration = Duration::try_from_secs_f64(aggregate / 1000.).map_err(|_| {
        error(if pubmed {
            "timeout_ms must be a positive finite number"
        } else {
            "invalid timeout_ms"
        })
    })?;
    Instant::now()
        .checked_add(duration)
        .ok_or_else(|| error("timeout overflow"))
}
pub(super) fn check(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err(timeout())
    } else {
        Ok(())
    }
}
pub(super) fn request(
    url: &str,
    params: &[(&str, &str)],
    ua: bool,
    accept: bool,
) -> Result<Request> {
    let mut url = url::Url::parse(url).map_err(|_| error("invalid biomedical base URL"))?;
    if !params.is_empty() {
        url.set_query(Some(&super::broad_query(params)));
    }
    let mut req =
        Request::get(url.as_str()).map_err(|_| error("invalid biomedical request URL"))?;
    if ua {
        req.headers.insert(
            "user-agent",
            "SlopSearX/0.1.0 (meta search engine; agent-native)"
                .parse()
                .unwrap(),
        );
    }
    req.headers.insert(
        "accept",
        if accept { "application/json" } else { "*/*" }
            .parse()
            .unwrap(),
    );
    Ok(req)
}
#[cfg(test)]
pub(super) async fn get(
    session: &mut crate::http::Session,
    req: Request,
    deadline: Instant,
    empty404: bool,
    details: bool,
    parse_deadline: Option<Instant>,
) -> Result<Option<Value>> {
    get_observed(
        session,
        req,
        deadline,
        empty404,
        details,
        parse_deadline,
        None,
    )
    .await
}
#[allow(clippy::too_many_arguments)] // Existing request policy plus a private observational seam.
pub(super) async fn get_observed(
    session: &mut crate::http::Session,
    req: Request,
    deadline: Instant,
    empty404: bool,
    details: bool,
    parse_deadline: Option<Instant>,
    observer: Observer<'_>,
) -> Result<Option<Value>> {
    check(deadline)?;
    let response = session.execute(req).await.map_err(|e| match e {
        crate::http::Error::Deadline => Failure {
            status: EngineStatus::Timeout,
            message: details.then(|| "biomedical provider request timed out".into()),
            _charge: None,
        },
        _ => error("biomedical provider request failed"),
    })?;
    if empty404 && response.status == 404 {
        return Ok(None);
    }
    if response.status == 429 {
        return Err(Failure {
            status: EngineStatus::RateLimited,
            message: details.then(|| "provider rate limited".into()),
            _charge: None,
        });
    }
    if !(200..300).contains(&response.status) {
        return Err(error(format!("provider HTTP {}", response.status)));
    }
    let value = parse_body_observed(&response.body, parse_deadline, observer).await?;
    Ok(Some(value))
}
/// Only PubMed supplies an aggregate parse budget. None retains the accepted
/// codec's byte/depth/text/integer limits and cooperative cancellation points.
#[cfg(test)]
pub(super) async fn parse_body(body: &[u8], deadline: Option<Instant>) -> Result<Value> {
    parse_body_observed(body, deadline, None).await
}
pub(super) async fn parse_body_observed(
    body: &[u8],
    deadline: Option<Instant>,
    observer: Observer<'_>,
) -> Result<Value> {
    observe(observer, Phase::Decode);
    let value = source_json::from_slice(body, deadline).await.map_err(|e| {
        if matches!(e, source_json::Error::Deadline) {
            timeout()
        } else {
            error(format!("invalid source JSON: {e:?}"))
        }
    })?;
    if let Some(deadline) = deadline {
        check(deadline)?;
    }
    Ok(value)
}
pub(super) async fn row_checkpoint(deadline: Option<Instant>) -> Result<()> {
    if let Some(deadline) = deadline {
        check(deadline)?;
    }
    tokio::task::yield_now().await;
    if let Some(deadline) = deadline {
        check(deadline)?;
    }
    Ok(())
}
pub(super) fn session(http: &AdapterHttp, deadline: Instant) -> Result<crate::http::Session> {
    http.session(
        Options::default(),
        tokio::time::Instant::from_std(deadline),
        Cancellation::default(),
    )
    .map_err(|_| error("biomedical HTTP session unavailable"))
}
pub(super) fn object(v: &Value) -> Result<&Map> {
    v.as_object().ok_or_else(|| error("expected source object"))
}
pub(super) fn array(v: &Value) -> Result<&[Value]> {
    v.as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| error("expected source array"))
}
/// Source slices result collections before iterating records. An empty sliced
/// string has no malformed rows; maps still reject slicing (including empty maps).
pub(super) fn result_rows<'a>(value: &'a Value, limit: &ConfigScalar) -> Result<&'a [Value]> {
    match value {
        Value::Array(rows) => Ok(&rows[..slice_end(limit, rows.len())?]),
        Value::String(text) => {
            if slice_end(limit, text.len())? == 0 {
                Ok(&[])
            } else {
                Err(error("source result element does not support get"))
            }
        }
        _ => Err(error("source result collection does not support slicing")),
    }
}
pub(super) fn field<'a>(v: &'a Value, key: &str, default: &'a Value) -> Result<&'a Value> {
    Ok(object(v)?.get(key).unwrap_or(default))
}
pub(super) fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(Number::Float(v)) => *v != 0.,
        Value::Number(n) => n.integer_text() != Some("0"),
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}
pub(super) fn empty() -> Value {
    Value::String(Text::from(""))
}
pub(super) fn map() -> Value {
    Value::Object(Map::new())
}
pub(super) fn list() -> Value {
    Value::Array(Vec::new())
}
pub struct ClinicalTrialsAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: BiomedicalConfig,
}
impl ClinicalTrialsAdapter {
    pub fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: BiomedicalConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn run(&self, q: &SearchRequest, context: Option<Instant>) -> AdapterResponse {
        let start = Instant::now();
        let budget = Budget::new(ROW_BYTES).expect("valid biomedical quota");
        finish_batch(self.run_with_budget(q, context, &budget).await, start).await
    }
    pub(super) async fn run_with_budget<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        budget: &'b Budget,
    ) -> Result<ProducedBatch<'b>> {
        self.run_observed(q, context, budget, None).await
    }
    pub(super) async fn run_observed<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        budget: &'b Budget,
        observer: Observer<'_>,
    ) -> Result<ProducedBatch<'b>> {
        admit_with_context(&self.admission, "clinicaltrials", context).await?;
        let d = network_deadline(&self.config, false, context)?;
        let max = scalar_text(&self.config.max_results);
        let req = request(
            &self.config.base_url,
            &[
                ("query.term", &q.query),
                ("pageSize", &max),
                ("format", "json"),
                ("sort", "@relevance"),
            ],
            false,
            false,
        )?;
        let mut session = session(&self.http, d)?;
        let data = get_observed(&mut session, req, d, false, false, context, observer)
            .await?
            .unwrap();
        let l = list();
        let rows = result_rows(field(&data, "studies", &l)?, &self.config.max_results)?;
        let mut batch =
            ProducedBatch::new(budget, rows.len(), context).map_err(projection_error)?;
        for (idx, study) in rows.iter().enumerate() {
            row_checkpoint(context).await?;
            let m = map();
            let e = empty();
            let null = &Value::Null;
            let protocol = field(study, "protocolSection", &m)?;
            let identification = field(protocol, "identificationModule", &m)?;
            let status_module = field(protocol, "statusModule", &m)?;
            let design = field(protocol, "designModule", &m)?;
            let condition_module = field(protocol, "conditionsModule", &m)?;
            let nct = owned_stringify(budget, field(identification, "nctId", &e)?, context).await?;
            let title = owned_join(
                budget,
                &[borrowed_text(field(identification, "briefTitle", &e)?)?],
                "",
                None,
                context,
            )
            .await?;
            let status = field(status_module, "overallStatus", &e)?;
            let phases = field(design, "phases", null)?;
            let phase = if !truthy(phases) {
                OwnedString::new(budget).map_err(owned_error)?
            } else {
                match phases {
                    Value::Array(values) => match values.first().filter(|v| truthy(v)) {
                        Some(v) => {
                            owned_join(budget, &[borrowed_text(v)?], "", None, context).await?
                        }
                        None => OwnedString::new(budget).map_err(owned_error)?,
                    },
                    Value::String(text) => owned_prefix(budget, text, 1, context).await?,
                    _ => return Err(error("phases does not support index zero")),
                }
            };
            let conditions = field(condition_module, "conditions", &l)?;
            let conditions = if !truthy(conditions) {
                OwnedString::new(budget).map_err(owned_error)?
            } else {
                match conditions {
                    Value::Array(values) => {
                        let mut parts =
                            OwnedVec::with_capacity(budget, values.len()).map_err(owned_error)?;
                        for (iteration, v) in values.iter().enumerate() {
                            if iteration >= 1024 {
                                observe(observer, Phase::Inner);
                            }
                            row_checkpoint(context).await?;
                            parts.try_push(borrowed_text(v)?).map_err(owned_error)?;
                        }
                        owned_join(budget, parts.as_slice(), "; ", None, context).await?
                    }
                    Value::Object(values) => {
                        let mut parts =
                            OwnedVec::with_capacity(budget, values.len()).map_err(owned_error)?;
                        for (iteration, (k, _)) in values.iter().enumerate() {
                            if iteration >= 1024 {
                                observe(observer, Phase::Inner);
                            }
                            row_checkpoint(context).await?;
                            parts.try_push(k.as_str().ok_or_else(||error("source text contains non-scalar Unicode; requires reviewed projection"))?).map_err(owned_error)?;
                        }
                        owned_join(budget, parts.as_slice(), "; ", None, context).await?
                    }
                    Value::String(text) => {
                        let mut out = OwnedString::new(budget).map_err(owned_error)?;
                        out.try_reserve_exact(text.len().checked_mul(6).ok_or_else(resource)?)
                            .map_err(owned_error)?;
                        for (i, p) in text.codepoints().iter().enumerate() {
                            if i % 1024 == 0 {
                                row_checkpoint(context).await?;
                            }
                            if i > 0 {
                                out.try_push_str("; ").map_err(owned_error)?;
                            }
                            out.try_push(char::from_u32(*p).ok_or_else(||error("source text contains non-scalar Unicode; requires reviewed projection"))?).map_err(owned_error)?;
                        }
                        out
                    }
                    _ => return Err(error("conditions is not iterable")),
                }
            };
            let mut parts = OwnedVec::with_capacity(budget, 3).map_err(owned_error)?;
            if truthy(status) {
                parts
                    .try_push(borrowed_text(status)?)
                    .map_err(owned_error)?;
            }
            if !phase.is_empty() {
                parts.try_push(phase.as_str()).map_err(owned_error)?;
            }
            if !conditions.is_empty() {
                parts.try_push(conditions.as_str()).map_err(owned_error)?;
            }
            let content = owned_join(budget, parts.as_slice(), " — ", Some(500), context).await?;
            let url = owned_join(
                budget,
                &["https://clinicaltrials.gov/study/", nct.as_str()],
                "",
                None,
                context,
            )
            .await?;
            let draft = Draft {
                url,
                title,
                content,
                published_date: None,
                position: idx as i64 + 1,
                score: 1.,
            };
            batch
                .push(draft.skeleton("clinicaltrials"), None)
                .await
                .map_err(projection_error)?;
        }
        Ok(batch)
    }
}
impl SearchEngine for ClinicalTrialsAdapter {
    fn search_with_context<'a>(
        &'a self,
        q: &'a SearchRequest,
        context: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move { self.run(q, Some(context.deadline)).await })
    }

    fn search<'a>(&'a self, q: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move { self.run(q, None).await })
    }
}

#[cfg(test)]
mod deadline_tests {
    use super::*;
    #[tokio::test]
    async fn only_pubmed_aggregate_budget_applies_to_source_decode() {
        let expired = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
        let bytes = br#"{"studies":[]}"#;
        let failure = parse_body(bytes, Some(expired)).await.unwrap_err();
        assert_eq!(failure.status, EngineStatus::Timeout);
        assert!(failure.message.is_none());
        assert!(parse_body(bytes, None).await.is_ok());
        // A missing aggregate parse budget does not remove source codec bounds.
        let oversized = vec![b' '; source_json::MAX_BYTES + 1];
        assert_eq!(
            parse_body(&oversized, None).await.unwrap_err().status,
            EngineStatus::Error
        );
        // Per-request F08 network admission still rejects an expired budget.
        let http = AdapterHttp::default();
        let mut s = session(&http, expired).unwrap();
        let req = request("http://127.0.0.1:1", &[], false, false).unwrap();
        assert_eq!(
            get(&mut s, req, expired, false, false, None)
                .await
                .unwrap_err()
                .status,
            EngineStatus::Timeout
        );
        http.shutdown_and_wait().await;
    }
}

// Candidate operation accounting covers row scratch, payloads and result copies.
// F08/input/SourceJSON storage and returned-response retention remain separate.
pub(super) use super::source_payload::{ProducedBatch, Skeleton};
pub(super) use ssx_owned_budget::{Budget, OwnedString, OwnedVec};
pub(super) const ROW_BYTES: usize = 32 * 1024 * 1024;
pub(super) fn resource() -> Failure {
    error("biomedical normalization resource limit exceeded")
}
pub(super) fn owned_error(_: ssx_owned_budget::Error) -> Failure {
    resource()
}
pub(super) fn projection_error(e: super::source_payload::BudgetError) -> Failure {
    match e {
        super::source_payload::BudgetError::Deadline => timeout(),
        super::source_payload::BudgetError::Resource => resource(),
        _ => error("source payload requires reviewed JSON projection"),
    }
}
pub(super) async fn finish_batch(
    result: Result<ProducedBatch<'_>>,
    start: Instant,
) -> AdapterResponse {
    let result = match result {
        Ok(batch) => batch
            .into_adapter_response(super::source_payload::Outcome {
                status: EngineStatus::Ok,
                error_message: None,
                latency_ms: start.elapsed().as_secs_f64() * 1000.,
                synthetic: false,
            })
            .await
            .map_err(projection_error),
        Err(failure) => Err(failure),
    };
    match result {
        Ok(response) => response,
        Err(failure) => outcome(Err(failure), start),
    }
}
pub(super) fn borrowed_text(value: &Value) -> Result<&str> {
    value
        .as_text()
        .ok_or_else(|| error("expected source string"))?
        .as_str()
        .ok_or_else(|| {
            error("source text contains non-scalar Unicode; requires reviewed projection")
        })
}
pub(super) async fn owned_join(
    budget: &Budget,
    parts: &[&str],
    separator: &str,
    cap: Option<usize>,
    deadline: Option<Instant>,
) -> Result<OwnedString> {
    let mut bytes = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if i % 1024 == 0 {
            row_checkpoint(deadline).await?;
        }
        bytes = bytes.checked_add(part.len()).ok_or_else(resource)?;
    }
    bytes = bytes
        .checked_add(
            separator
                .len()
                .checked_mul(parts.len().saturating_sub(1))
                .ok_or_else(resource)?,
        )
        .ok_or_else(resource)?;
    let mut out = OwnedString::new(budget).map_err(owned_error)?;
    out.try_reserve_exact(cap.map_or(bytes, |n| bytes.min(n.saturating_mul(4))))
        .map_err(owned_error)?;
    let mut emitted = 0usize;
    for (i, part) in parts.iter().enumerate() {
        for text in [if i == 0 { "" } else { separator }, *part] {
            for c in text.chars() {
                if cap.is_some_and(|bound| emitted >= bound) {
                    return Ok(out);
                }
                if emitted.is_multiple_of(1024) {
                    row_checkpoint(deadline).await?;
                }
                out.try_push(c).map_err(owned_error)?;
                emitted += 1;
            }
        }
    }
    row_checkpoint(deadline).await?;
    Ok(out)
}
pub(super) async fn owned_prefix(
    budget: &Budget,
    text: &Text,
    bound: usize,
    deadline: Option<Instant>,
) -> Result<OwnedString> {
    let mut out = OwnedString::new(budget).map_err(owned_error)?;
    out.try_reserve_exact(text.len().min(bound).checked_mul(4).ok_or_else(resource)?)
        .map_err(owned_error)?;
    for (i, point) in text.codepoints().iter().take(bound).enumerate() {
        if i % 1024 == 0 {
            row_checkpoint(deadline).await?;
        }
        out.try_push(char::from_u32(*point).ok_or_else(|| {
            error("source text contains non-scalar Unicode; requires reviewed projection")
        })?)
        .map_err(owned_error)?;
    }
    Ok(out)
}
pub(super) async fn owned_quote(
    budget: &Budget,
    value: &str,
    slash: bool,
    deadline: Option<Instant>,
) -> Result<OwnedString> {
    let mut out = OwnedString::new(budget).map_err(owned_error)?;
    out.try_reserve_exact(value.len().checked_mul(3).ok_or_else(resource)?)
        .map_err(owned_error)?;
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for (i, b) in value.bytes().enumerate() {
        if i % 1024 == 0 {
            row_checkpoint(deadline).await?;
        }
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) || (slash && b == b'/') {
            out.try_push(char::from(b)).map_err(owned_error)?;
        } else {
            let bytes = [b'%', HEX[(b >> 4) as usize], HEX[(b & 15) as usize]];
            out.try_push_str(std::str::from_utf8(&bytes).expect("ASCII hex"))
                .map_err(owned_error)?;
        }
    }
    Ok(out)
}
pub(super) async fn owned_stringify(
    budget: &Budget,
    value: &Value,
    deadline: Option<Instant>,
) -> Result<OwnedString> {
    if let Value::String(text) = value {
        let scalar = text
            .as_str()
            .ok_or_else(|| error("source stringification contains non-scalar Unicode"))?;
        return owned_join(budget, &[scalar], "", None, deadline).await;
    }
    super::source_repr::repr_budgeted(value, budget, deadline)
        .await
        .map_err(|error| match error {
            super::source_repr::Error::Deadline => timeout(),
            super::source_repr::Error::Resource => resource(),
            super::source_repr::Error::Unicode => {
                self::error("source stringification contains non-scalar Unicode")
            }
        })
}
pub(super) struct Draft {
    pub url: OwnedString,
    pub title: OwnedString,
    pub content: OwnedString,
    pub published_date: Option<OwnedString>,
    pub position: i64,
    pub score: f64,
}
impl Draft {
    pub fn skeleton<'a>(&'a self, engine: &'a str) -> Skeleton<'a> {
        Skeleton {
            url: self.url.as_str(),
            title: self.title.as_str(),
            content: self.content.as_str(),
            engine,
            category: "general",
            score: self.score,
            position: self.position,
            tier: 1,
            published_date: self.published_date.as_ref().map(OwnedString::as_str),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Decode,
    Payload,
    Inner,
    CidParts,
    CidPartsComplete,
    MedDigits,
    MedDigitsComplete,
    RankPartition,
    RankPartitionComplete,
}
pub(super) type Observer<'a> = Option<&'a (dyn Fn(Phase) + Send + Sync)>;
pub(super) fn observe(observer: Observer<'_>, phase: Phase) {
    if let Some(callback) = observer {
        callback(phase);
    }
}
#[cfg(test)]
#[path = "../../tests/support/a07_resources.rs"]
mod resource_tests;

// Source-derived diagnostics retain their allocation owner to the same named
// public response boundary; only fixed-size control metadata uses the fallback.
pub(super) async fn id_error(
    budget: &Budget,
    prefix: &str,
    id: &str,
    suffix: &str,
    deadline: Option<Instant>,
) -> Result<Failure> {
    let owned = owned_join(budget, &[prefix, id, suffix], "", None, deadline).await?;
    let charge = budget.reserve(owned.len()).map_err(owned_error)?;
    let mut message = String::new();
    message
        .try_reserve_exact(owned.len())
        .map_err(|_| resource())?;
    if message.capacity() > charge.bytes() {
        drop(message);
        return Err(resource());
    }
    let mut remaining = owned.as_str();
    while !remaining.is_empty() {
        row_checkpoint(deadline).await?;
        let end = remaining
            .char_indices()
            .nth(1024)
            .map_or(remaining.len(), |(i, _)| i);
        message.push_str(&remaining[..end]);
        remaining = &remaining[end..];
    }
    Ok(Failure {
        status: EngineStatus::Error,
        message: Some(message),
        _charge: Some(charge),
    })
}

/// Fixed small numeric/date formatting into an already charged destination.
/// The sink refuses unexpected width before any additional allocation.
pub(super) fn fixed_format(
    budget: &Budget,
    capacity: usize,
    args: std::fmt::Arguments<'_>,
) -> Result<OwnedString> {
    struct Sink<'a> {
        out: &'a mut OwnedString,
        limit: usize,
    }
    impl std::fmt::Write for Sink<'_> {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            if self
                .out
                .len()
                .checked_add(text.len())
                .is_none_or(|n| n > self.limit)
            {
                return Err(std::fmt::Error);
            }
            self.out.try_push_str(text).map_err(|_| std::fmt::Error)
        }
    }
    let mut out = OwnedString::new(budget).map_err(owned_error)?;
    out.try_reserve_exact(capacity).map_err(owned_error)?;
    std::fmt::write(
        &mut Sink {
            out: &mut out,
            limit: capacity,
        },
        args,
    )
    .map_err(|_| resource())?;
    Ok(out)
}
