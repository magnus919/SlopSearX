//! IntelX source adapter: one start, one unchanged2s delay, one result fetch.
use super::{
    clinicaltrials as c,
    source_json::{Text, Value},
    threat_intel as t,
};
use ssx_application::{PortFuture, SearchAdmission, SearchContext, SearchEngine};
use ssx_config::{ConfigScalar, Secret};
use ssx_domain::{AdapterResponse, EngineStatus, SearchRequest};
use ssx_registry::{EngineFactory, Registry, RegistryError};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
pub const ENDPOINT: &str = "https://2.intelx.io";
#[derive(Clone)]
pub struct IntelXConfig {
    pub base_url: String,
    pub api_key: Secret,
    pub timeout_ms: ConfigScalar,
    pub max_results: ConfigScalar,
}
impl std::fmt::Debug for IntelXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntelXConfig")
            .field("base_url_configured", &!self.base_url.is_empty())
            .field("api_key", &"[redacted]")
            .field("timeout_ms", &self.timeout_ms)
            .field("max_results", &self.max_results)
            .finish()
    }
}
impl IntelXConfig {
    pub fn defaults() -> Self {
        Self {
            base_url: ENDPOINT.into(),
            api_key: Secret::new(None),
            timeout_ms: ConfigScalar::Integer(15000),
            max_results: ConfigScalar::Integer(10),
        }
    }
    pub fn from_resolved(config: &ssx_config::EngineConfig) -> Self {
        Self {
            base_url: if config.base_url.chars().all(ssx_json::python_whitespace) {
                ENDPOINT.into()
            } else {
                config.base_url.clone()
            },
            api_key: config.api_key.clone(),
            timeout_ms: config.timeout_ms.clone(),
            max_results: config.max_results.clone(),
        }
    }
    fn network(&self, context: Option<Instant>) -> t::Result<Instant> {
        c::network_deadline(
            &c::BiomedicalConfig {
                base_url: String::new(),
                timeout_ms: self.timeout_ms.clone(),
                max_results: ConfigScalar::Integer(0),
                aggregate_timeout_ms: None,
            },
            false,
            context,
        )
        .map_err(Into::into)
    }
    fn key(&self) -> &str {
        self.api_key.expose().unwrap_or("")
    }
}
pub struct IntelXAdapter {
    http: Arc<crate::http::AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: IntelXConfig,
}
pub fn register_intelx(
    registry: &mut Registry,
    http: Arc<crate::http::AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: IntelXConfig,
) -> Result<(), RegistryError> {
    registry.register(
        "intelx",
        Arc::new(move || {
            Box::new(IntelXAdapter::new(
                http.clone(),
                admission.clone(),
                config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )
}
impl IntelXAdapter {
    pub fn new(
        http: Arc<crate::http::AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: IntelXConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn run(&self, q: &SearchRequest, context: Option<Instant>) -> AdapterResponse {
        let budget = t::Budget::new(c::ROW_BYTES).expect("existing positive native quota");
        let early = async {
            t::admit(&self.admission, "intelx", context).await?;
            if self.config.key().is_empty() {
                Err(t::error(
                    "IntelX API key not configured (set ENGINE_INTELX_API_KEY)",
                ))
            } else {
                Ok(())
            }
        }
        .await;
        if let Err(e) = early {
            return AdapterResponse {
                status: e.status,
                error_message: e.message,
                ..Default::default()
            };
        }
        let start = Instant::now();
        t::finish(self.execute(q, &budget, context).await, start).await
    }
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        budget: &'b t::Budget,
        context: Option<Instant>,
    ) -> t::Result<c::ProducedBatch<'b>> {
        let first_deadline = self.config.network(context)?;
        let mut session = c::session(&self.http, first_deadline)?;
        let key = self.config.key();
        if !key.is_ascii() {
            return Err(t::error("invalid IntelX credential configuration"));
        }
        let _header_charge = budget.reserve(key.len()).map_err(t::resource)?;
        let mut header = reqwest::header::HeaderValue::from_str(key)
            .map_err(|_| t::error("invalid IntelX credential configuration"))?;
        header.set_sensitive(true);
        session.headers.insert("x-key", header);
        session
            .headers
            .insert("accept", "application/json".parse().expect("fixed header"));
        session.headers.insert(
            "user-agent",
            "SlopSearX/0.1.0".parse().expect("fixed header"),
        );
        let search_url = t::join(
            budget,
            &[&self.config.base_url, "/intelligent/search"],
            "",
            Some(first_deadline),
        )
        .await?;
        let start_data = {
            let mut request = crate::http::Request::get(search_url.as_str())
                .map_err(|_| t::error("invalid IntelX start URL"))?;
            request.method = reqwest::Method::POST;
            request.headers.insert(
                "content-type",
                "application/json".parse().expect("fixed header"),
            );
            let body = start_body(
                &q.query,
                &self.config.max_results,
                budget,
                Some(first_deadline),
            )
            .await?;
            let _request_charge = budget.reserve(body.len()).map_err(t::resource)?;
            request
                .body
                .try_reserve_exact(body.len())
                .map_err(|_| t::error("IntelX request allocation refused"))?;
            request.body.extend_from_slice(body.as_str().as_bytes());
            let response = exchange(&mut session, request).await?;
            classify(response.status, true)?;
            c::parse_body_observed(&response.body, Some(first_deadline), None).await?
        };
        let empty = t::empty();
        let id = field(&start_data, "id", &empty)?;
        if !t::truthy(id) {
            return c::ProducedBatch::new(budget, 0, context)
                .map_err(c::projection_error)
                .map_err(Into::into);
        }
        t::checkpoint(context).await?;
        // Source waits once. No task, polling, retry or configured-budget renewal
        // occurs here. HTTP shutdown will reject GET before dispatch; a caller
        // may cancel this owned sleep immediately by dropping its search future.
        source_delay(context).await?;
        t::checkpoint(context).await?;
        // Fresh per-operation source timeout, capped by unchanged caller limit,
        // while retaining session cookies, request count and cancellation owner.
        let second_deadline = self.config.network(context)?;
        session.set_deadline(tokio::time::Instant::from_std(second_deadline));
        let mut values = t::OwnedVec::new(budget).map_err(t::resource)?;
        match id {
            Value::Array(values_in) => {
                for value in values_in {
                    values
                        .try_push(parameter(value, budget, Some(second_deadline)).await?)
                        .map_err(t::resource)?;
                }
            }
            _ => values
                .try_push(parameter(id, budget, Some(second_deadline)).await?)
                .map_err(t::resource)?,
        }
        let limit = scalar(&self.config.max_results, budget, Some(second_deadline)).await?;
        let mut params =
            t::OwnedVec::with_capacity(budget, values.len() + 1).map_err(t::resource)?;
        for value in values.as_slice() {
            params
                .try_push(("id", value.as_str()))
                .map_err(t::resource)?;
        }
        params
            .try_push(("limit", limit.as_str()))
            .map_err(t::resource)?;
        let result_url = t::join(
            budget,
            &[&self.config.base_url, "/intelligent/result"],
            "",
            Some(second_deadline),
        )
        .await?;
        let request = c::request(result_url.as_str(), params.as_slice(), false, true)
            .map_err(|_| t::error("invalid IntelX result URL"))?;
        let response = exchange(&mut session, request).await?;
        classify(response.status, false)?;
        let body = c::parse_body_observed(&response.body, Some(second_deadline), None).await?;
        project(
            &body,
            &self.config.max_results,
            budget,
            Some(second_deadline),
        )
        .await
    }
}
impl SearchEngine for IntelXAdapter {
    fn search<'a>(&'a self, q: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, None))
    }
    fn search_with_context<'a>(
        &'a self,
        q: &'a SearchRequest,
        context: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, Some(context.deadline)))
    }
}
#[cfg(test)]
tokio::task_local! { static DELAY_PENDING: Arc<tokio::sync::Notify>; }
async fn source_delay(context: Option<Instant>) -> t::Result<()> {
    use std::future::Future;
    let delay = tokio::time::sleep(Duration::from_secs(2));
    tokio::pin!(delay);
    let observed = std::future::poll_fn(|cx| {
        let state = delay.as_mut().poll(cx);
        #[cfg(test)]
        if state.is_pending() {
            let _ = DELAY_PENDING.try_with(|signal| signal.notify_one());
        }
        state
    });
    tokio::pin!(observed);
    if let Some(deadline) = context {
        tokio::select! {biased;_ =tokio::time::sleep_until(tokio::time::Instant::from_std(deadline))=>return Err(t::classified(EngineStatus::Timeout,None)),_ =&mut observed=>{}}
    } else {
        observed.await;
    }
    Ok(())
}
async fn exchange(
    session: &mut crate::http::Session,
    request: crate::http::Request,
) -> t::Result<crate::http::Response> {
    session.execute(request).await.map_err(|e| {
        if e == crate::http::Error::Deadline {
            t::classified(EngineStatus::Timeout, None)
        } else {
            t::error("IntelX request failed")
        }
    })
}
fn classify(status: u16, start: bool) -> t::Result<()> {
    match status {
        429 if start => Err(t::classified(EngineStatus::RateLimited, None)),
        403 if start => Err(t::classified(EngineStatus::Blocked, None)),
        s if !(200..300).contains(&s) => Err(t::error(format!("IntelX returned HTTP {s}"))),
        _ => Ok(()),
    }
}
fn field<'a>(v: &'a Value, key: &str, default: &'a Value) -> t::Result<&'a Value> {
    v.as_object()
        .map(|m| m.get(key).unwrap_or(default))
        .ok_or_else(|| t::error("IntelX source value does not support field access"))
}
async fn quoted(out: &mut t::OwnedString, value: &str, deadline: Option<Instant>) -> t::Result<()> {
    out.try_push('"').map_err(t::resource)?;
    for (i, ch) in value.chars().enumerate() {
        if i % 1024 == 0 {
            t::checkpoint(deadline).await?;
        }
        // Reuse the existing UTF8 scalar JSON encoder with fixed-size output;
        // no uncharged query-sized temporary or new JSON grammar is introduced.
        let mut bytes = [0u8; 8];
        let mut target = &mut bytes[..];
        serde_json::to_writer(&mut target, &ch)
            .map_err(|_| t::error("IntelX JSON scalar encoding failed"))?;
        let n = 8 - target.len();
        out.try_push_str(std::str::from_utf8(&bytes[1..n - 1]).expect("JSON scalar UTF8"))
            .map_err(t::resource)?;
    }
    out.try_push('"').map_err(t::resource)
}
async fn scalar(
    value: &ConfigScalar,
    budget: &t::Budget,
    deadline: Option<Instant>,
) -> t::Result<t::OwnedString> {
    t::owned(budget, &c::scalar_text(value), deadline).await
}
async fn start_body(
    query: &str,
    max: &ConfigScalar,
    budget: &t::Budget,
    deadline: Option<Instant>,
) -> t::Result<t::OwnedString> {
    let mut out = t::owned(budget, "{\"term\":", deadline).await?;
    quoted(&mut out, query, deadline).await?;
    out.try_push_str(",\"maxresults\":").map_err(t::resource)?;
    match max {
        ConfigScalar::Text(v) => quoted(&mut out, v, deadline).await?,
        ConfigScalar::Float(v) if !v.is_finite() => {
            return Err(t::error("IntelX request JSON requires finite numbers"));
        }
        _ => out
            .try_push_str(scalar(max, budget, deadline).await?.as_str())
            .map_err(t::resource)?,
    }
    out.try_push_str(",\"target\":0,\"sort\":2}")
        .map_err(t::resource)?;
    Ok(out)
}
async fn parameter(
    v: &Value,
    budget: &t::Budget,
    deadline: Option<Instant>,
) -> t::Result<t::OwnedString> {
    match v {
        Value::Null => t::owned(budget, "", deadline).await,
        Value::Bool(v) => t::owned(budget, if *v { "true" } else { "false" }, deadline).await,
        _ => t::stringify(budget, v, deadline).await,
    }
}
enum Prefix<'a> {
    Text(&'a Text, usize),
    Array(&'a [Value]),
}
impl Prefix<'_> {
    fn truthy(&self) -> bool {
        match self {
            Self::Text(v, n) => !v.is_empty() && *n > 0,
            Self::Array(v) => !v.is_empty(),
        }
    }
    fn is_text(&self) -> bool {
        matches!(self, Self::Text(..))
    }
}
fn prefix(v: &Value, n: usize) -> t::Result<Prefix<'_>> {
    match v {
        Value::String(v) => Ok(Prefix::Text(v, n)),
        Value::Array(v) => Ok(Prefix::Array(&v[..v.len().min(n)])),
        _ => Err(t::error("IntelX source value does not support slicing")),
    }
}
fn source_len(v: &Value) -> t::Result<usize> {
    match v {
        Value::String(v) => Ok(v.len()),
        Value::Array(v) => Ok(v.len()),
        Value::Object(v) => Ok(v.len()),
        _ => Err(t::error("IntelX source value has no length")),
    }
}
fn unwrap_record(v: &Value) -> Option<&Value> {
    let v = v.as_object().and_then(|m| m.get("record")).unwrap_or(v);
    v.as_object().map(|_| v)
}
fn type_label(v: &Value) -> t::Result<Option<&'static str>> {
    let numeric = match v {
        Value::Array(_) | Value::Object(_) => {
            return Err(t::error("IntelX source label key is unhashable"));
        }
        Value::Bool(v) => Some(if *v { 1. } else { 0. }),
        Value::Number(v) => v.as_f64(),
        _ => None,
    };
    Ok(match numeric {
        Some(1.) => Some("Pastebin"),
        Some(2.) => Some("Darknet"),
        Some(3.) => Some("Document"),
        Some(4.) => Some("Source Code"),
        Some(5.) => Some("Forum"),
        _ => None,
    })
}
async fn project<'b>(
    body: &Value,
    max: &ConfigScalar,
    budget: &'b t::Budget,
    deadline: Option<Instant>,
) -> t::Result<c::ProducedBatch<'b>> {
    let empty_array = Value::Array(Vec::new());
    let raw = if body.as_object().is_some() {
        field(body, "records", &empty_array)?
    } else {
        body
    };
    let rows = raw.as_array().map_or(&[][..], Vec::as_slice);
    let rows = &rows[..c::slice_end(max, rows.len())?];
    let mut count = 0usize;
    for (i, row) in rows.iter().enumerate() {
        if i % 1024 == 0 {
            t::checkpoint(deadline).await?;
        }
        if unwrap_record(row).is_some() {
            count += 1;
        }
    }
    let mut batch = c::ProducedBatch::new(budget, count, deadline).map_err(c::projection_error)?;
    let empty = t::empty();
    let zero = Value::integer(0);
    for (i, raw) in rows.iter().enumerate() {
        t::checkpoint(deadline).await?;
        let Some(row) = unwrap_record(raw) else {
            continue;
        };
        let id = field(row, "systemid", &empty)?;
        let name = field(row, "name", &empty)?;
        let kind = field(row, "type", &zero)?;
        let label = type_label(kind)?;
        let date = field(row, "date", &empty)?;
        let preview = prefix(field(row, "preview", &empty)?, 300)?;
        let bucket = field(row, "bucket", &empty)?;
        let title_prefix = if !t::truthy(name) && t::truthy(id) {
            Some(prefix(id, 32)?)
        } else {
            None
        };
        // Source chooses title before joining content, and handles date after
        // that join. Typed scalar validation is deferred until those operations.
        if preview.truthy() && !preview.is_text() {
            return Err(t::error("IntelX content join requires source strings"));
        }
        if t::truthy(bucket) && !matches!(bucket, Value::String(_)) {
            return Err(t::error("IntelX content join requires source strings"));
        }
        let published = if t::truthy(date) && source_len(date)? >= 10 {
            Some(prefix(date, 10)?)
        } else {
            None
        };
        let source = if let Some(label) = label {
            t::owned(budget, label, deadline).await?
        } else {
            let text = t::stringify(budget, kind, deadline).await?;
            t::join(budget, &["Type ", text.as_str()], "", deadline).await?
        };
        let mut parts = t::OwnedVec::with_capacity(budget, 3).map_err(t::resource)?;
        parts.try_push(source).map_err(t::resource)?;
        if let Prefix::Text(text, n) = preview {
            let selected = c::owned_prefix(budget, text, n, deadline).await?;
            if !selected.is_empty() {
                parts.try_push(selected).map_err(t::resource)?;
            }
        }
        if t::truthy(bucket) {
            parts
                .try_push(t::owned(budget, t::text(bucket)?, deadline).await?)
                .map_err(t::resource)?;
        }
        let mut refs = t::OwnedVec::with_capacity(budget, parts.len()).map_err(t::resource)?;
        for p in parts.as_slice() {
            refs.try_push(p.as_str()).map_err(t::resource)?;
        }
        let content = t::join(budget, refs.as_slice(), " | ", deadline).await?;
        let title = if t::truthy(name) {
            t::owned(budget, t::text(name)?, deadline).await?
        } else if let Some(prefix) = title_prefix {
            match prefix {
                Prefix::Text(text, n) => c::owned_prefix(budget, text, n, deadline).await?,
                Prefix::Array(_) => return Err(t::error("expected source string")),
            }
        } else {
            let position = t::stringify(budget, &Value::integer((i + 1) as i64), deadline).await?;
            t::join(budget, &["IntelX result ", position.as_str()], "", deadline).await?
        };
        let url = if t::truthy(id) {
            let text = t::stringify(budget, id, deadline).await?;
            t::join(
                budget,
                &["https://intelx.io/?did=", text.as_str()],
                "",
                deadline,
            )
            .await?
        } else {
            t::owned(budget, "", deadline).await?
        };
        let date = match published {
            Some(Prefix::Text(text, n)) => Some(c::owned_prefix(budget, text, n, deadline).await?),
            Some(Prefix::Array(_)) => return Err(t::error("expected source string")),
            None => None,
        };
        t::push(
            &mut batch,
            "intelx",
            t::Fields {
                url: url.as_str(),
                title: title.as_str(),
                content: content.as_str(),
                score: 0.,
                position: i + 1,
                date: date.as_ref().map(|v| v.as_str()),
            },
        )
        .await?;
    }
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::super::source_payload::{BATCH_PHASE, BatchPhase};
    use super::*;
    #[tokio::test]
    async fn projection_drop_releases_backing_after_an_actual_stored_row() {
        let body=super::super::source_json::from_slice(br#"{"records":[{"systemid":"one","name":"first"},{"systemid":"two","name":"second"}]}"#,None).await.unwrap();
        let budget = t::Budget::new(1024 * 1024).unwrap();
        let entered = Arc::new(tokio::sync::Notify::new());
        let signal = entered.clone();
        let observer: Arc<dyn Fn(BatchPhase) -> bool + Send + Sync> = Arc::new(move |phase| {
            if matches!(phase, BatchPhase::Stored(1)) {
                signal.notify_one();
                true
            } else {
                false
            }
        });
        BATCH_PHASE.scope(observer,async{let max=ConfigScalar::Integer(10);let mut call=Box::pin(project(&body,&max,&budget,None));tokio::time::timeout(Duration::from_secs(1),async{tokio::select!{_ =entered.notified()=>{},_ =&mut call=>panic!("must suspend after actual first stored row")}}).await.unwrap();assert!(budget.snapshot().unwrap().used>0);drop(call);assert_eq!(budget.snapshot().unwrap().used,0);}).await;
        println!("A13_REMAINING_UNIT_COMPLETE intelx-projection-stored-drop");
    }
    #[tokio::test]
    async fn late_failure_and_tiny_quota_leave_no_retained_partial_rows() {
        for(id,raw,limit)in[("late-failure",br#"{"records":[{"systemid":"one","name":"first"},{"systemid":"two","name":"second","preview":false}]}"#.as_slice(),1024*1024),("atomic-quota",br#"{"records":[{"systemid":"one","name":"first"},{"systemid":"two","name":"second"}]}"#.as_slice(),64)]{let body=super::super::source_json::from_slice(raw,None).await.unwrap();let budget=t::Budget::new(limit).unwrap();let Err(error)=project(&body,&ConfigScalar::Integer(10),&budget,None).await else{panic!("late or capacity failure must refuse partial batch")};assert_eq!(error.status,EngineStatus::Error);assert_eq!(budget.snapshot().unwrap().used,0);println!("A13_REMAINING_UNIT_COMPLETE intelx-{id}");}
    }
}

#[cfg(test)]
mod delay_tests {
    use super::*;
    use ssx_application::{AdmissionDecision, SearchAdmission};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Allowed;
    impl SearchAdmission for Allowed {
        fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
            Box::pin(async { AdmissionDecision::Allowed })
        }
    }
    struct StartOnly(AtomicUsize);
    impl crate::http::Transport for StartOnly {
        fn exchange(&self, request: crate::http::Request) -> crate::http::ExchangeFuture<'_> {
            Box::pin(async move {
                assert_eq!(
                    self.0.fetch_add(1, Ordering::SeqCst),
                    0,
                    "cancelled actual delay must never dispatch GET"
                );
                assert_eq!(request.method, reqwest::Method::POST);
                Ok(crate::http::Response {
                    status: 200,
                    body: br#"{"id":"synthetic"}"#.to_vec(),
                    headers: Default::default(),
                    url: request.url,
                })
            })
        }
    }
    #[tokio::test]
    async fn actual_source_sleep_pending_cancellation_never_dispatches_get() {
        let http = Arc::new(crate::http::AdapterHttp::default());
        let transport = Arc::new(StartOnly(AtomicUsize::new(0)));
        http.set_transport(transport.clone()).unwrap();
        let mut config = IntelXConfig::defaults();
        config.base_url = "http://127.0.0.1:1/fixture".into();
        config.api_key = Secret::new(Some("synthetic-key".into()));
        let adapter = IntelXAdapter::new(http.clone(), Arc::new(Allowed), config);
        let q = SearchRequest {
            query: "synthetic".into(),
            ..Default::default()
        };
        let signal = Arc::new(tokio::sync::Notify::new());
        DELAY_PENDING.scope(signal.clone(),async{let mut call=Box::pin(adapter.search(&q));tokio::time::timeout(Duration::from_secs(1),async{tokio::select!{_ =signal.notified()=>{},_ =&mut call=>panic!("actual source Sleep must poll Pending")}}).await.unwrap();assert_eq!(transport.0.load(Ordering::SeqCst),1);drop(call);}).await;
        tokio::time::sleep(Duration::from_millis(2100)).await;
        assert_eq!(transport.0.load(Ordering::SeqCst), 1);
        http.shutdown_and_wait().await;
        println!("A13_REMAINING_UNIT_COMPLETE intelx-actual-delay-pending-cancel");
    }
}
