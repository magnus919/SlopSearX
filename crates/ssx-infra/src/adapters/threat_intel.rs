//! A11-owned factory and request/projection glue over accepted native seams.
use super::{clinicaltrials as c, source_json::Value, source_payload};
use crate::http::{AdapterHttp, Request};
pub(super) use c::{Budget, OwnedString, OwnedVec};
use ssx_application::SearchAdmission;
use ssx_config::ConfigScalar;
use ssx_domain::{AdapterResponse, EngineStatus};
use ssx_registry::{EngineFactory, Registry, RegistryError};
use std::{sync::Arc, time::Instant};
#[derive(Clone)]
pub struct ThreatIntelConfig {
    pub base_url: String,
    pub api_key: ssx_config::Secret,
    pub timeout_ms: ConfigScalar,
    pub max_results: ConfigScalar,
}
impl std::fmt::Debug for ThreatIntelConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreatIntelConfig")
            .field("base_url_configured", &!self.base_url.is_empty())
            .field("api_key", &"[redacted]")
            .field("timeout_ms", &self.timeout_ms)
            .field("max_results", &self.max_results)
            .finish()
    }
}
impl ThreatIntelConfig {
    pub fn defaults(base: &str) -> Self {
        Self {
            base_url: base.into(),
            api_key: ssx_config::Secret::new(None),
            timeout_ms: ConfigScalar::Integer(10000),
            max_results: ConfigScalar::Integer(10),
        }
    }
    pub fn from_resolved(config: &ssx_config::EngineConfig, base: &str) -> Self {
        Self {
            base_url: if config.base_url.chars().all(ssx_json::python_whitespace) {
                base.into()
            } else {
                config.base_url.clone()
            },
            api_key: config.api_key.clone(),
            timeout_ms: config.timeout_ms.clone(),
            max_results: config.max_results.clone(),
        }
    }
    pub(super) fn key(&self) -> &str {
        self.api_key.expose().unwrap_or("")
    }
    pub(super) fn network(&self, context: Option<Instant>) -> Result<Instant> {
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
}
#[derive(Clone, Debug)]
pub struct ThreatIntelSettings {
    pub abuseipdb: ThreatIntelConfig,
    pub greynoise: ThreatIntelConfig,
    pub otx: ThreatIntelConfig,
    pub urlhaus: ThreatIntelConfig,
}
impl ThreatIntelSettings {
    pub fn from_resolved(config: &ssx_config::Config) -> Self {
        fn resolve(config: &ssx_config::Config, name: &str, base: &str) -> ThreatIntelConfig {
            config.engines.get(name).map_or_else(
                || ThreatIntelConfig::defaults(base),
                |c| ThreatIntelConfig::from_resolved(c, base),
            )
        }
        Self {
            abuseipdb: resolve(config, "abuseipdb", super::abuseipdb::ENDPOINT),
            greynoise: resolve(config, "greynoise", super::greynoise::ENDPOINT),
            otx: resolve(config, "otx", super::otx::ENDPOINT),
            urlhaus: resolve(config, "urlhaus", super::urlhaus::ENDPOINT),
        }
    }
}
pub fn register_threat_intelligence(
    registry: &mut Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: ThreatIntelSettings,
) -> std::result::Result<(), RegistryError> {
    macro_rules! attach {
        ($module:ident,$adapter:ident) => {{
            let h = http.clone();
            let a = admission.clone();
            let c = config.$module;
            registry.register(
                stringify!($module),
                Arc::new(move || {
                    Box::new(super::$module::$adapter::new(
                        h.clone(),
                        a.clone(),
                        c.clone(),
                    )) as Box<dyn ssx_registry::Engine>
                }) as EngineFactory,
            )?;
        }};
    }
    attach!(abuseipdb, AbuseIPDBAdapter);
    attach!(greynoise, GreyNoiseAdapter);
    attach!(otx, OTXAdapter);
    attach!(urlhaus, URLhausAdapter);
    Ok(())
}
#[derive(Debug)]
pub(super) struct Failure {
    pub status: EngineStatus,
    pub message: Option<String>,
}
pub(super) type Result<T> = std::result::Result<T, Failure>;
impl From<c::Failure> for Failure {
    fn from(e: c::Failure) -> Self {
        let r = c::outcome(Err(e), Instant::now());
        Self {
            status: r.status,
            message: r.error_message,
        }
    }
}
pub(super) fn error(message: impl Into<String>) -> Failure {
    Failure {
        status: EngineStatus::Error,
        message: Some(message.into()),
    }
}
pub(super) fn classified(status: EngineStatus, message: Option<&str>) -> Failure {
    Failure {
        status,
        message: message.map(str::to_owned),
    }
}
pub(super) fn resource(_: ssx_owned_budget::Error) -> Failure {
    error("intelligence projection resource limit exceeded")
}
// Fixed production quota; test-only task-local tracing never changes shared observers.
pub(super) fn new_budget() -> Budget {
    #[cfg(test)]
    let limit = TRACE.try_with(|trace| trace.limit).unwrap_or(c::ROW_BYTES);
    #[cfg(not(test))]
    let limit = c::ROW_BYTES;
    let budget = Budget::new(limit).expect("valid A11 quota");
    #[cfg(test)]
    let _ = TRACE.try_with(|trace| *trace.budget.lock().unwrap() = Some(budget.clone()));
    budget
}
#[cfg(test)]
tokio::task_local! { static TRACE: Arc<Trace>; }
#[cfg(test)]
struct Trace {
    limit: usize,
    budget: std::sync::Mutex<Option<Budget>>,
    query_steps: std::sync::atomic::AtomicUsize,
    projection: std::sync::atomic::AtomicBool,
}
#[cfg(test)]
pub(super) fn query_step() {
    let _ = TRACE.try_with(|trace| {
        trace
            .query_steps
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    });
}
#[cfg(test)]
pub(super) fn projection_entered() {
    let _ = TRACE.try_with(|trace| {
        trace
            .projection
            .store(true, std::sync::atomic::Ordering::SeqCst)
    });
}
pub(super) async fn checkpoint(deadline: Option<Instant>) -> Result<()> {
    Ok(c::row_checkpoint(deadline).await?)
}
pub(super) async fn admit(
    a: &Arc<dyn SearchAdmission>,
    name: &'static str,
    deadline: Option<Instant>,
) -> Result<()> {
    Ok(c::admit_with_context(a, name, deadline).await?)
}
pub(super) async fn finish(
    result: Result<c::ProducedBatch<'_>>,
    start: Instant,
) -> AdapterResponse {
    let result = match result {
        Ok(batch) => batch
            .into_adapter_response(source_payload::Outcome {
                status: EngineStatus::Ok,
                error_message: None,
                latency_ms: start.elapsed().as_secs_f64() * 1000.,
                synthetic: false,
            })
            .await
            .map_err(|e| Failure::from(c::projection_error(e))),
        Err(e) => Err(e),
    };
    result.unwrap_or_else(|e| AdapterResponse {
        status: e.status,
        error_message: e.message,
        latency_ms: c::elapsed(start),
        ..Default::default()
    })
}
pub(super) async fn fetch(
    http: &AdapterHttp,
    req: Request,
    deadline: Instant,
    context: Option<Instant>,
    engine: &str,
    urlhaus: bool,
) -> Result<Option<Value>> {
    let mut session = c::session(http, deadline)?;
    c::check(deadline)?;
    let response = session.execute(req).await.map_err(|e| {
        if matches!(e, crate::http::Error::Deadline) {
            classified(EngineStatus::Timeout, None)
        } else {
            error(format!("{engine} request failed"))
        }
    })?;
    let status = response.status;
    if status == 429 {
        return Err(classified(EngineStatus::RateLimited, None));
    }
    if status == 403 {
        return Err(classified(
            EngineStatus::Blocked,
            if urlhaus {
                Some("URLhaus request blocked")
            } else {
                None
            },
        ));
    }
    if urlhaus && status == 401 {
        return Err(classified(
            EngineStatus::Blocked,
            Some("URLhaus authentication required"),
        ));
    }
    if engine == "GreyNoise" && status == 404 {
        return Ok(None);
    }
    if urlhaus && (400..500).contains(&status) {
        return Err(error(format!("URLhaus rejected request (HTTP {status})")));
    }
    if !(200..300).contains(&status) {
        return Err(error(format!("{engine} returned HTTP {status}")));
    }
    Ok(Some(
        c::parse_body_observed(&response.body, context, None).await?,
    ))
}
pub(super) fn get(url: &str, params: &[(&str, &str)], accept_json: bool) -> Result<Request> {
    Ok(c::request(url, params, true, accept_json)?)
}
pub(super) fn credential(req: &mut Request, name: &'static str, value: &str) -> Result<()> {
    if !value.is_ascii() {
        return Err(error("invalid intelligence credential configuration"));
    }
    let value = value
        .parse()
        .map_err(|_| error("invalid intelligence credential configuration"))?;
    req.headers.insert(name, value);
    Ok(())
}
pub(super) async fn owned(b: &Budget, s: &str, d: Option<Instant>) -> Result<OwnedString> {
    Ok(c::owned_join(b, &[s], "", None, d).await?)
}
pub(super) async fn join(
    b: &Budget,
    p: &[&str],
    sep: &str,
    d: Option<Instant>,
) -> Result<OwnedString> {
    Ok(c::owned_join(b, p, sep, None, d).await?)
}
pub(super) async fn stringify(b: &Budget, v: &Value, d: Option<Instant>) -> Result<OwnedString> {
    Ok(c::owned_stringify(b, v, d).await?)
}
pub(super) fn text(v: &Value) -> Result<&str> {
    Ok(c::borrowed_text(v)?)
}
pub(super) fn field<'a>(v: &'a Value, key: &str, default: &'a Value) -> Result<&'a Value> {
    Ok(c::field(v, key, default)?)
}
pub(super) fn truthy(v: &Value) -> bool {
    c::truthy(v)
}
pub(super) fn empty() -> Value {
    c::empty()
}
pub(super) async fn prefix(
    b: &Budget,
    v: &Value,
    n: usize,
    d: Option<Instant>,
) -> Result<OwnedString> {
    let t = v
        .as_text()
        .ok_or_else(|| error("source value does not support string slicing"))?;
    Ok(c::owned_prefix(b, t, n, d).await?)
}
// Source f-string formatting after a finite prefix slice, without cloning nested values.
pub(super) async fn slice_format(
    b: &Budget,
    v: &Value,
    n: usize,
    d: Option<Instant>,
) -> Result<OwnedString> {
    match v {
        Value::String(_) => prefix(b, v, n, d).await,
        Value::Array(values) => {
            let mut parts = OwnedVec::with_capacity(b, values.len().min(n)).map_err(resource)?;
            for v in values.iter().take(n) {
                checkpoint(d).await?;
                let value =
                    super::source_repr::repr_budgeted(v, b, d)
                        .await
                        .map_err(|e| match e {
                            super::source_repr::Error::Deadline => {
                                classified(EngineStatus::Timeout, None)
                            }
                            _ => error(
                                "source sliced value cannot be formatted within projection bounds",
                            ),
                        })?;
                parts.try_push(value).map_err(resource)?
            }
            let mut refs = OwnedVec::with_capacity(b, parts.len()).map_err(resource)?;
            for part in parts.as_slice() {
                refs.try_push(part.as_str()).map_err(resource)?
            }
            let inner = join(b, refs.as_slice(), ", ", d).await?;
            join(b, &["[", inner.as_str(), "]"], "", d).await
        }
        _ => Err(error("source value does not support slicing")),
    }
}
pub(super) async fn tags(b: &Budget, v: &Value, d: Option<Instant>) -> Result<OwnedString> {
    let mut parts = OwnedVec::new(b).map_err(resource)?;
    match v {
        Value::Array(rows) => {
            for item in rows.iter().take(5) {
                checkpoint(d).await?;
                parts
                    .try_push(owned(b, text(item)?, d).await?)
                    .map_err(resource)?
            }
        }
        Value::String(t) => {
            for point in t.codepoints().iter().take(5) {
                checkpoint(d).await?;
                let ch = char::from_u32(*point)
                    .ok_or_else(|| error("source tag contains non-scalar Unicode"))?;
                let mut s = OwnedString::new(b).map_err(resource)?;
                s.try_push(ch).map_err(resource)?;
                parts.try_push(s).map_err(resource)?
            }
        }
        _ => return Err(error("source tags do not support slicing")),
    }
    let mut refs = OwnedVec::with_capacity(b, parts.len()).map_err(resource)?;
    for s in parts.as_slice() {
        refs.try_push(s.as_str()).map_err(resource)?
    }
    join(b, refs.as_slice(), ", ", d).await
}
pub(super) struct Fields<'a> {
    pub url: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub score: f64,
    pub position: usize,
    pub date: Option<&'a str>,
}
pub(super) async fn push(
    batch: &mut c::ProducedBatch<'_>,
    engine: &str,
    fields: Fields<'_>,
) -> Result<()> {
    batch
        .push(
            c::Skeleton {
                url: fields.url,
                title: fields.title,
                content: fields.content,
                engine,
                category: "general",
                score: fields.score,
                position: fields.position as i64,
                tier: 1,
                published_date: fields.date,
            },
            None,
        )
        .await
        .map_err(|e| Failure::from(c::projection_error(e)))
}
// Borrow query spans with cooperative work; never allocate a stripped copy.
pub(super) async fn strip(text: &str, d: Option<Instant>) -> Result<&str> {
    let mut start = text.len();
    for (n, (at, ch)) in text.char_indices().enumerate() {
        if n % 1024 == 0 {
            checkpoint(d).await?;
        }
        if !ssx_json::python_whitespace(ch) {
            start = at;
            break;
        }
    }
    if start == text.len() {
        return Ok(&text[text.len()..]);
    }
    let mut end = text.len();
    for (n, (at, ch)) in text[start..].char_indices().rev().enumerate() {
        if n % 1024 == 0 {
            checkpoint(d).await?;
        }
        if !ssx_json::python_whitespace(ch) {
            end = start + at + ch.len_utf8();
            break;
        }
    }
    Ok(&text[start..end])
}
pub(super) async fn first_word(text: &str, d: Option<Instant>) -> Result<&str> {
    for (n, (at, ch)) in text.char_indices().enumerate() {
        if n % 1024 == 0 {
            checkpoint(d).await?;
        }
        if ssx_json::python_whitespace(ch) {
            return Ok(&text[..at]);
        }
    }
    Ok(text)
}

macro_rules! adapter {
    ($adapter:ident) => {
        pub struct $adapter {
            http: std::sync::Arc<crate::http::AdapterHttp>,
            admission: std::sync::Arc<dyn ssx_application::SearchAdmission>,
            config: super::threat_intel::ThreatIntelConfig,
        }
        impl $adapter {
            pub fn new(
                http: std::sync::Arc<crate::http::AdapterHttp>,
                admission: std::sync::Arc<dyn ssx_application::SearchAdmission>,
                config: super::threat_intel::ThreatIntelConfig,
            ) -> Self {
                Self {
                    http,
                    admission,
                    config,
                }
            }
            async fn run(
                &self,
                q: &ssx_domain::SearchRequest,
                context: Option<std::time::Instant>,
            ) -> ssx_domain::AdapterResponse {
                let start = std::time::Instant::now();
                let b = super::threat_intel::new_budget();
                super::threat_intel::finish(self.execute(q, context, &b).await, start).await
            }
        }
        impl ssx_application::SearchEngine for $adapter {
            fn search<'a>(
                &'a self,
                q: &'a ssx_domain::SearchRequest,
            ) -> ssx_application::PortFuture<'a, ssx_domain::AdapterResponse> {
                Box::pin(self.run(q, None))
            }
            fn search_with_context<'a>(
                &'a self,
                q: &'a ssx_domain::SearchRequest,
                c: ssx_application::SearchContext,
            ) -> ssx_application::PortFuture<'a, ssx_domain::AdapterResponse> {
                Box::pin(self.run(q, Some(c.deadline)))
            }
        }
    };
}
pub(super) use adapter;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{ExchangeFuture, Response, Transport};
    use ssx_application::SearchContext;
    use ssx_domain::SearchRequest;
    use std::{
        future::Future,
        sync::{
            Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        task::Poll,
        time::Duration,
    };
    struct Fixture {
        body: Vec<u8>,
        calls: AtomicUsize,
    }
    impl Transport for Fixture {
        fn exchange(&self, r: Request) -> ExchangeFuture<'_> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                Ok(Response {
                    status: 200,
                    url: r.url,
                    headers: Default::default(),
                    body: self.body.clone(),
                })
            })
        }
    }
    fn trace(limit: usize) -> Arc<Trace> {
        Arc::new(Trace {
            limit,
            budget: Mutex::new(None),
            query_steps: AtomicUsize::new(0),
            projection: AtomicBool::new(false),
        })
    }
    fn engine(
        name: &str,
        body: Vec<u8>,
    ) -> (
        Box<dyn ssx_registry::Engine>,
        Arc<AdapterHttp>,
        Arc<Fixture>,
    ) {
        let mut r = Registry::builtins().unwrap();
        let h = Arc::new(AdapterHttp::default());
        let f = Arc::new(Fixture {
            body,
            calls: AtomicUsize::new(0),
        });
        h.set_transport(f.clone()).unwrap();
        let mut config = ThreatIntelConfig::defaults("http://127.0.0.1:1/injected-only");
        config.api_key = ssx_config::Secret::new(Some("fixture-only-key".into()));
        let settings = ThreatIntelSettings {
            abuseipdb: config.clone(),
            greynoise: config.clone(),
            otx: config.clone(),
            urlhaus: config,
        };
        register_threat_intelligence(
            &mut r,
            h.clone(),
            Arc::new(super::super::AllowAllAdmission),
            settings,
        )
        .unwrap();
        (r.instantiate(name).unwrap(), h, f)
    }
    fn body(name: &str, size: usize) -> Vec<u8> {
        let long = "x".repeat(size);
        serde_json::to_vec(&match name{"abuseipdb"=>serde_json::json!({"data":{"isp":long}}),"greynoise"=>serde_json::json!({"ip":"8.8.8.8","name":long}),"otx"=>serde_json::json!({"asn":long}),_=>serde_json::json!({"query_status":"ok","urls":[{"url":"https://evil.example/a","threat":long}]})}).unwrap()
    }
    async fn budget_zero(t: &Trace) {
        let b = t
            .budget
            .lock()
            .unwrap()
            .clone()
            .expect("actual adapter budget observed");
        assert_eq!(b.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn actual_four_factory_query_scans_cancel_expire_without_dispatch_and_release() {
        for name in ["abuseipdb", "greynoise", "otx", "urlhaus"] {
            for expire in [false, true] {
                let (e, h, f) = engine(name, b"{}".to_vec());
                let q = SearchRequest {
                    query: "x".repeat(500000),
                    ..Default::default()
                };
                let t = trace(c::ROW_BYTES);
                let deadline = Instant::now() + Duration::from_secs(1);
                let mut future = Box::pin(TRACE.scope(
                    t.clone(),
                    e.search_with_context(&q, SearchContext { deadline }),
                ));
                tokio::time::timeout(
                    Duration::from_secs(2),
                    std::future::poll_fn(|cx| match future.as_mut().poll(cx) {
                        Poll::Ready(_) => {
                            panic!("must observe actual query scan before completion")
                        }
                        Poll::Pending => {
                            if t.query_steps.load(Ordering::SeqCst) >= 2 {
                                Poll::Ready(())
                            } else {
                                Poll::Pending
                            }
                        }
                    }),
                )
                .await
                .unwrap();
                assert_eq!(f.calls.load(Ordering::SeqCst), 0);
                assert!(!t.projection.load(Ordering::SeqCst));
                if expire {
                    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
                    let response = future.await;
                    assert_eq!(response.status, EngineStatus::Timeout);
                    assert!(response.results.is_empty());
                } else {
                    drop(future);
                }
                drop(e);
                h.shutdown_and_wait().await;
                budget_zero(&t).await;
                assert_eq!(f.calls.load(Ordering::SeqCst), 0);
                println!("A11_OWNED_COMPLETE entered-query:{name}:{expire}");
            }
        }
    }
    #[tokio::test]
    async fn actual_four_post_source_parse_projection_cancels_expires_and_releases() {
        for name in ["abuseipdb", "greynoise", "otx", "urlhaus"] {
            for expire in [false, true] {
                let (e, h, f) = engine(name, body(name, 1024 * 1024));
                let q = SearchRequest {
                    query: "8.8.8.8".into(),
                    ..Default::default()
                };
                let t = trace(c::ROW_BYTES);
                let deadline = Instant::now() + Duration::from_secs(2);
                let mut future = Box::pin(TRACE.scope(
                    t.clone(),
                    e.search_with_context(&q, SearchContext { deadline }),
                ));
                tokio::time::timeout(
                    Duration::from_secs(3),
                    std::future::poll_fn(|cx| match future.as_mut().poll(cx) {
                        Poll::Ready(_) => panic!(
                            "must observe actual charged projection before completion: {name}"
                        ),
                        Poll::Pending => {
                            let used = t
                                .budget
                                .lock()
                                .unwrap()
                                .as_ref()
                                .map_or(0, |b| b.snapshot().unwrap().used);
                            if t.projection.load(Ordering::SeqCst) && used > 32768 {
                                Poll::Ready(())
                            } else {
                                Poll::Pending
                            }
                        }
                    }),
                )
                .await
                .unwrap();
                assert_eq!(f.calls.load(Ordering::SeqCst), 1);
                if expire {
                    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
                    let response = future.await;
                    assert_eq!(response.status, EngineStatus::Timeout);
                    assert!(response.results.is_empty());
                } else {
                    drop(future);
                }
                drop(e);
                h.shutdown_and_wait().await;
                budget_zero(&t).await;
                println!("A11_OWNED_COMPLETE entered-projection:{name}:{expire}");
            }
        }
    }
    #[tokio::test]
    async fn actual_four_projection_refusals_are_classified_after_parse_and_return_all_charges() {
        for name in ["abuseipdb", "greynoise", "otx", "urlhaus"] {
            let (e, h, f) = engine(name, body(name, 4096));
            let q = SearchRequest {
                query: "8.8.8.8".into(),
                ..Default::default()
            };
            let t = trace(1024);
            let response = TRACE.scope(t.clone(), e.search(&q)).await;
            drop(e);
            h.shutdown_and_wait().await;
            assert_eq!(f.calls.load(Ordering::SeqCst), 1);
            assert!(t.projection.load(Ordering::SeqCst));
            assert_eq!(response.status, EngineStatus::Error);
            assert!(response.results.is_empty());
            assert!(
                !response
                    .error_message
                    .as_deref()
                    .unwrap_or("")
                    .contains("fixture-only-key")
            );
            budget_zero(&t).await;
            println!("A11_OWNED_COMPLETE projection-refusal:{name}");
        }
    }
}
