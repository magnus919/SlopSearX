//! A12-owned mechanics; provider interpretation stays in the five named modules.
use super::{
    source_json::{self, Map, Number, Value},
    source_payload, source_repr,
};
use crate::http::{AdapterHttp, Cancellation, Options, Request};
use ssx_application::{AdmissionDecision, SearchAdmission};
use ssx_config::{ConfigScalar, Secret};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64};
use ssx_owned_budget::{Budget, OwnedString, OwnedVec};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
pub(super) type Result<T> = std::result::Result<T, Failure>;
#[derive(Debug)]
pub(super) struct Failure {
    pub status: EngineStatus,
    pub message: Option<String>,
}
pub(super) fn error(s: impl Into<String>) -> Failure {
    Failure {
        status: EngineStatus::Error,
        message: Some(s.into()),
    }
}
pub(super) fn timeout() -> Failure {
    Failure {
        status: EngineStatus::Timeout,
        message: None,
    }
}
pub(super) fn resource(_: ssx_owned_budget::Error) -> Failure {
    error("infrastructure result exceeds operation budget")
}
pub(super) fn projection(e: source_payload::BudgetError) -> Failure {
    match e {
        source_payload::BudgetError::Deadline => timeout(),
        _ => error("infrastructure result cannot be projected within native bounds"),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Censys,
    Shodan,
    VirusTotal,
    VulnCheck,
    CrtSh,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Censys => "censys",
            Self::Shodan => "shodan",
            Self::VirusTotal => "virustotal",
            Self::VulnCheck => "vulncheck",
            Self::CrtSh => "crtsh",
        }
    }
    pub fn display(self) -> &'static str {
        match self {
            Self::Censys => "Censys",
            Self::Shodan => "Shodan",
            Self::VirusTotal => "VirusTotal",
            Self::VulnCheck => "VulnCheck",
            Self::CrtSh => "CRT.sh",
        }
    }
    pub fn base(self) -> &'static str {
        match self {
            Self::Censys => "https://search.censys.io/api/v2",
            Self::Shodan => "https://api.shodan.io",
            Self::VirusTotal => "https://www.virustotal.com/api/v3",
            Self::VulnCheck => "https://api.vulncheck.com/v3",
            Self::CrtSh => "https://crt.sh",
        }
    }
}
#[derive(Clone)]
pub struct SecurityConfig {
    pub base_url: String,
    pub api_key: Secret,
    pub api_secret: Secret,
    pub timeout_ms: ConfigScalar,
    pub max_results: ConfigScalar,
}
impl std::fmt::Debug for SecurityConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecurityConfig")
            .field("base_url_configured", &!self.base_url.is_empty())
            .field("api_key", &"[redacted]")
            .field("api_secret", &"[redacted]")
            .field("timeout_ms", &self.timeout_ms)
            .field("max_results", &self.max_results)
            .finish()
    }
}
impl SecurityConfig {
    pub fn defaults(kind: Kind) -> Self {
        Self {
            base_url: kind.base().into(),
            api_key: Secret::new(None),
            api_secret: Secret::new(None),
            timeout_ms: ConfigScalar::Integer(10000),
            max_results: ConfigScalar::Integer(10),
        }
    }
    /// Mirrors service._normalize_engine_configs; direct construction preserves an explicit empty URL.
    pub fn from_resolved(c: &ssx_config::EngineConfig, kind: Kind) -> Self {
        Self {
            base_url: if c.base_url.chars().all(ssx_json::python_whitespace) {
                kind.base().into()
            } else {
                c.base_url.clone()
            },
            api_key: c.api_key.clone(),
            api_secret: c.api_secret.clone(),
            timeout_ms: c.timeout_ms.clone(),
            max_results: c.max_results.clone(),
        }
    }
    fn deadline(&self, context: Option<Instant>) -> Result<Instant> {
        let n = match &self.timeout_ms {
            ConfigScalar::Integer(n) => *n as f64,
            ConfigScalar::Boolean(v) => u8::from(*v) as f64,
            ConfigScalar::Float(n) => *n,
            ConfigScalar::Text(_) => {
                return Err(error(
                    "unsupported operand type(s) for /: 'str' and 'float'",
                ));
            }
        };
        if n.is_nan() {
            return Err(error("invalid timeout_ms"));
        }
        if n <= 0. {
            return Err(timeout());
        }
        let configured = Duration::try_from_secs_f64(n / 1000.)
            .ok()
            .and_then(|d| Instant::now().checked_add(d));
        match (configured, context) {
            (Some(d), Some(c)) => Ok(d.min(c)),
            (Some(d), None) => Ok(d),
            (None, Some(d)) => Ok(d),
            (None, None) => Err(error("timeout requires a finite lifecycle deadline")),
        }
    }
}
pub(super) struct Runtime {
    pub http: Arc<AdapterHttp>,
    pub admission: Arc<dyn SearchAdmission>,
    pub config: SecurityConfig,
    pub kind: Kind,
}
impl Runtime {
    pub(super) async fn run(
        &self,
        q: &ssx_domain::SearchRequest,
        context: Option<Instant>,
    ) -> AdapterResponse {
        let started = Instant::now();
        let budget = Budget::new(32 * 1024 * 1024).expect("fixed operation budget");
        let result = self.execute(q, context, &budget).await;
        let result = match result {
            Ok(batch) => batch
                .into_adapter_response(source_payload::Outcome {
                    status: EngineStatus::Ok,
                    error_message: None,
                    latency_ms: started.elapsed().as_secs_f64() * 1000.,
                    synthetic: false,
                })
                .await
                .map_err(projection),
            Err(e) => Err(e),
        };
        result.unwrap_or_else(|e| AdapterResponse {
            status: e.status,
            error_message: e.message,
            latency_ms: FiniteF64::new(started.elapsed().as_secs_f64() * 1000.).unwrap_or_default(),
            ..Default::default()
        })
    }
    async fn execute<'a>(
        &self,
        q: &ssx_domain::SearchRequest,
        context: Option<Instant>,
        budget: &'a Budget,
    ) -> Result<source_payload::ProducedBatch<'a>> {
        checkpoint(context).await?;
        let admitted = match context {
            Some(d) => tokio::time::timeout_at(d.into(), self.admission.admit(self.kind.name()))
                .await
                .map_err(|_| timeout())?,
            None => self.admission.admit(self.kind.name()).await,
        };
        checkpoint(context).await?;
        match admitted {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => {
                return Err(Failure {
                    status: EngineStatus::RateLimited,
                    message: Some("rate limited".into()),
                });
            }
            AdmissionDecision::Unavailable => {
                return Err(error(format!(
                    "{} rate policy unavailable",
                    self.kind.display()
                )));
            }
        }
        let raw_key = self.config.api_key.expose().unwrap_or("");
        let key = if self.kind == Kind::Shodan {
            raw_key.trim_matches(ssx_json::python_whitespace)
        } else {
            raw_key
        };
        if self.kind != Kind::CrtSh
            && (key.is_empty()
                || (self.kind == Kind::Censys
                    && self.config.api_secret.expose().unwrap_or("").is_empty()))
        {
            return Err(error(match self.kind {
                Kind::Censys => {
                    "Censys API credentials not configured (set ENGINE_CENSYS_API_KEY and ENGINE_CENSYS_API_SECRET)"
                }
                Kind::Shodan => "Shodan API key not configured (set ENGINE_SHODAN_API_KEY)",
                Kind::VirusTotal => {
                    "VirusTotal API key not configured (set ENGINE_VIRUSTOTAL_API_KEY)"
                }
                Kind::VulnCheck => {
                    "VulnCheck API key not configured (set ENGINE_VULNCHECK_API_KEY)"
                }
                Kind::CrtSh => unreachable!(),
            }));
        }
        let cve = if self.kind == Kind::VulnCheck {
            super::vulncheck::cve(&q.query, context, budget).await?
        } else {
            None
        };
        if self.kind == Kind::VulnCheck && cve.is_none() {
            return source_payload::ProducedBatch::new(budget, 0, context).map_err(projection);
        }
        let deadline = self.config.deadline(context)?;
        checkpoint(Some(deadline)).await?;
        let (suffix, params, accept) = match self.kind {
            Kind::Censys => (
                "/hosts/search",
                vec![
                    ("q", q.query.clone()),
                    ("per_page", scalar(&self.config.max_results)),
                ],
                true,
            ),
            Kind::Shodan => (
                "/shodan/host/search",
                vec![
                    ("query", q.query.clone()),
                    ("limit", scalar(&self.config.max_results)),
                ],
                false,
            ),
            Kind::VirusTotal => (
                "/search",
                vec![
                    ("query", q.query.clone()),
                    ("limit", scalar(&self.config.max_results)),
                ],
                true,
            ),
            Kind::VulnCheck => ("", Vec::new(), true),
            Kind::CrtSh => ("/?output=json", vec![("q", q.query.clone())], false),
        };
        let endpoint = if let Some(cve) = &cve {
            format!("{}/community/cve/{}", self.config.base_url, cve.as_str())
        } else {
            format!("{}{suffix}", self.config.base_url)
        };
        let mut request =
            Request::get(&endpoint).map_err(|_| error("invalid infrastructure base URL"))?;
        // HTTPX params replaces an existing query rather than merging output=json.
        if !params.is_empty() {
            request.url.set_query(None);
            request
                .url
                .query_pairs_mut()
                .extend_pairs(params.iter().map(|(k, v)| (*k, v.as_str())));
        }
        if accept {
            request.headers.insert(
                "accept",
                "application/json".parse().expect("literal header"),
            );
        }
        match self.kind {
            Kind::Censys => {
                use base64::Engine as _;
                let credentials =
                    format!("{key}:{}", self.config.api_secret.expose().unwrap_or(""));
                let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
                credential(&mut request, "authorization", &format!("Basic {encoded}"))?;
            }
            Kind::Shodan | Kind::VulnCheck => {
                credential(&mut request, "authorization", &format!("Bearer {key}"))?
            }
            Kind::VirusTotal => credential(&mut request, "x-apikey", key)?,
            Kind::CrtSh => {}
        }
        let mut session = self
            .http
            .session(Options::default(), deadline.into(), Cancellation::default())
            .map_err(|_| error("infrastructure HTTP session unavailable"))?;
        let response = session.execute(request).await.map_err(|e| {
            if e == crate::http::Error::Deadline {
                timeout()
            } else {
                error(format!("{} request failed", self.kind.display()))
            }
        })?;
        checkpoint(Some(deadline)).await?;
        if response.status == 429 {
            return Err(Failure {
                status: EngineStatus::RateLimited,
                message: None,
            });
        }
        if response.status == 403 {
            return Err(Failure {
                status: EngineStatus::Blocked,
                message: None,
            });
        }
        if !(200..300).contains(&response.status) {
            return Err(error(format!(
                "{} returned HTTP {}",
                self.kind.display(),
                response.status
            )));
        }
        let value = if self.kind == Kind::CrtSh {
            super::crtsh::decode(&response, deadline, budget).await?
        } else {
            source_json::from_slice(&response.body, Some(deadline))
                .await
                .map_err(|e| {
                    if e == source_json::Error::Deadline {
                        timeout()
                    } else {
                        error(e.message(self.kind.display(), "provider returned malformed JSON"))
                    }
                })?
        };
        let mut batch = source_payload::ProducedBatch::new(
            budget,
            row_capacity(&value, self.kind, &self.config.max_results)?,
            Some(deadline),
        )
        .map_err(projection)?;
        match self.kind {
            Kind::Censys => super::censys::parse(&value, &mut batch, budget, deadline).await?,
            Kind::Shodan => {
                super::shodan::parse(
                    &value,
                    &self.config.max_results,
                    &mut batch,
                    budget,
                    deadline,
                )
                .await?
            }
            Kind::VirusTotal => {
                super::virustotal::parse(&value, &mut batch, budget, deadline).await?
            }
            Kind::VulnCheck => {
                super::vulncheck::parse(
                    &value,
                    cve.as_ref().expect("validated CVE").as_str(),
                    &mut batch,
                    budget,
                    deadline,
                )
                .await?
            }
            Kind::CrtSh => {
                super::crtsh::parse(
                    &value,
                    &self.config.max_results,
                    &mut batch,
                    budget,
                    deadline,
                )
                .await?
            }
        }
        checkpoint(Some(deadline)).await?;
        Ok(batch)
    }
}
fn row_capacity(v: &Value, kind: Kind, limit: &ConfigScalar) -> Result<usize> {
    let list = Value::Array(Vec::new());
    let map = Value::Object(Default::default());
    match kind {
        Kind::CrtSh => Ok(slice(v, limit)?.len()),
        Kind::Shodan => Ok(slice(field(v, "matches", &list)?, limit)?.len()),
        Kind::VirusTotal => Ok(iterable(field(v, "data", &list)?)?.len()),
        Kind::Censys => {
            let result = field(v, "result", &map)?;
            if result.as_object().is_some() {
                Ok(iterable(field(result, "hits", &list)?)?.len())
            } else {
                Ok(0)
            }
        }
        Kind::VulnCheck => Ok(usize::from(truth(field(v, "data", &map)?))),
    }
}
fn credential(r: &mut Request, name: &'static str, v: &str) -> Result<()> {
    if !v.is_ascii() {
        return Err(error("invalid infrastructure credential configuration"));
    }
    r.headers.insert(
        name,
        v.parse()
            .map_err(|_| error("invalid infrastructure credential configuration"))?,
    );
    Ok(())
}
fn scalar(v: &ConfigScalar) -> String {
    match v {
        ConfigScalar::Boolean(v) => v.to_string(),
        ConfigScalar::Integer(v) => v.to_string(),
        ConfigScalar::Float(v) => ssx_json::python_float(*v).unwrap_or_else(|_| v.to_string()),
        ConfigScalar::Text(v) => v.clone(),
    }
}
pub(super) async fn checkpoint(d: Option<Instant>) -> Result<()> {
    if d.is_some_and(|d| Instant::now() >= d) {
        return Err(timeout());
    }
    tokio::task::yield_now().await;
    if d.is_some_and(|d| Instant::now() >= d) {
        return Err(timeout());
    }
    Ok(())
}
pub(super) fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n @ Number::Integer(_)) => n
            .integer_text()
            .is_some_and(|s| s.bytes().any(|b| b.is_ascii_digit() && b != b'0')),
        Value::Number(Number::Float(f)) => *f != 0.,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}
pub(super) fn typ(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(Number::Integer(_)) => "int",
        Value::Number(_) => "float",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}
pub(super) fn object(v: &Value) -> Result<&Map> {
    v.as_object()
        .ok_or_else(|| error(format!("'{}' object has no attribute 'get'", typ(v))))
}
pub(super) fn field<'a>(v: &'a Value, k: &str, default: &'a Value) -> Result<&'a Value> {
    Ok(object(v)?.get(k).unwrap_or(default))
}
pub(super) fn iterable(v: &Value) -> Result<&[Value]> {
    match v {
        Value::Array(a) => Ok(a.as_slice()),
        Value::Object(m) if m.is_empty() => Ok(&[]),
        Value::String(s) if s.is_empty() => Ok(&[]),
        Value::Object(_) | Value::String(_) => Err(error("'str' object has no attribute 'get'")),
        _ => Err(error(format!("'{}' object is not iterable", typ(v)))),
    }
}
pub(super) fn slice<'a>(v: &'a Value, limit: &ConfigScalar) -> Result<&'a [Value]> {
    let n = match limit {
        ConfigScalar::Boolean(b) => i64::from(*b),
        ConfigScalar::Integer(n) => *n,
        _ => {
            return Err(error(
                "slice indices must be integers or None or have an __index__ method",
            ));
        }
    };
    match v {
        Value::Array(a) => {
            let end = if n < 0 {
                a.len().saturating_sub(n.unsigned_abs() as usize)
            } else {
                a.len().min(usize::try_from(n).unwrap_or(usize::MAX))
            };
            Ok(&a[..end])
        }
        Value::String(s) if s.is_empty() => Ok(&[]),
        Value::String(s) if n == 0 || (n < 0 && n.unsigned_abs() as usize >= s.len()) => Ok(&[]),
        Value::String(_) => Err(error("'str' object has no attribute 'get'")),
        Value::Object(_) => Err(error("unhashable type: 'slice'")),
        _ => Err(error(format!("'{}' object is not subscriptable", typ(v)))),
    }
}
pub(super) fn text(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(|| {
        error(format!(
            "source {} cannot project to a text result field",
            typ(v)
        ))
    })
}
pub(super) async fn owned(b: &Budget, s: &str, d: Instant) -> Result<OwnedString> {
    let mut out = OwnedString::new(b).map_err(resource)?;
    out.try_reserve_exact(s.len()).map_err(resource)?;
    for (i, c) in s.chars().enumerate() {
        if i % 1024 == 0 {
            checkpoint(Some(d)).await?;
        }
        out.try_push(c).map_err(resource)?;
    }
    Ok(out)
}
pub(super) async fn stringify(b: &Budget, v: &Value, d: Instant) -> Result<OwnedString> {
    if let Some(s) = v.as_str() {
        owned(b, s, d).await
    } else {
        source_repr::repr_budgeted(v, b, Some(d))
            .await
            .map_err(|e| match e {
                source_repr::Error::Deadline => timeout(),
                _ => error("source representation exceeds native projection bounds"),
            })
    }
}
pub(super) async fn join(b: &Budget, parts: &[&str], sep: &str, d: Instant) -> Result<OwnedString> {
    let mut out = OwnedString::new(b).map_err(resource)?;
    for (i, p) in parts.iter().enumerate() {
        checkpoint(Some(d)).await?;
        for s in [if i == 0 { "" } else { sep }, *p] {
            let value = owned(b, s, d).await?;
            out.try_push_str(value.as_str()).map_err(resource)?;
        }
    }
    Ok(out)
}
fn source_points(v: &Value) -> Result<&[u32]> {
    v.as_text().map(|text| text.codepoints()).ok_or_else(|| {
        error(format!(
            "source {} cannot project to a text result field",
            typ(v)
        ))
    })
}
pub(super) async fn append_points(out: &mut OwnedString, points: &[u32], d: Instant) -> Result<()> {
    let scalar = |point| {
        char::from_u32(point)
            .ok_or_else(|| error("source str cannot project to a text result field"))
    };
    let mut bytes = 0usize;
    for (i, point) in points.iter().enumerate() {
        if i % 1024 == 0 {
            checkpoint(Some(d)).await?;
        }
        bytes = bytes
            .checked_add(scalar(*point)?.len_utf8())
            .ok_or_else(|| resource(ssx_owned_budget::Error::Overflow))?;
    }
    out.try_reserve_exact(bytes).map_err(resource)?;
    for (i, point) in points.iter().enumerate() {
        if i % 1024 == 0 {
            checkpoint(Some(d)).await?;
        }
        out.try_push(scalar(*point)?).map_err(resource)?;
    }
    Ok(())
}
pub(super) async fn first_line_points(v: &Value, d: Instant) -> Result<&[u32]> {
    let points = source_points(v)?;
    for (i, point) in points.iter().enumerate() {
        if i % 1024 == 0 {
            checkpoint(Some(d)).await?;
        }
        if *point == u32::from('\n') {
            return Ok(&points[..i]);
        }
    }
    Ok(points)
}
pub(super) async fn prefix(b: &Budget, v: &Value, n: usize, d: Instant) -> Result<OwnedString> {
    let points = source_points(v)?;
    let mut out = OwnedString::new(b).map_err(resource)?;
    append_points(&mut out, &points[..points.len().min(n)], d).await?;
    Ok(out)
}
pub(super) struct Row<'a> {
    pub url: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub score: f64,
    pub position: usize,
    pub date: Option<&'a str>,
}
pub(super) async fn push(
    batch: &mut source_payload::ProducedBatch<'_>,
    kind: Kind,
    row: Row<'_>,
) -> Result<()> {
    batch
        .push(
            source_payload::Skeleton {
                url: row.url,
                title: row.title,
                content: row.content,
                engine: kind.name(),
                category: "general",
                score: row.score,
                position: row.position as i64 + 1,
                tier: 1,
                published_date: row.date,
            },
            None,
        )
        .await
        .map_err(projection)
}
pub(super) async fn parts(b: &Budget, n: usize) -> Result<OwnedVec<OwnedString>> {
    OwnedVec::with_capacity(b, n).map_err(resource)
}
pub(super) async fn content(
    b: &Budget,
    p: &OwnedVec<OwnedString>,
    fallback: &str,
    d: Instant,
) -> Result<OwnedString> {
    if p.is_empty() {
        owned(b, fallback, d).await
    } else {
        let refs = p
            .as_slice()
            .iter()
            .map(OwnedString::as_str)
            .collect::<Vec<_>>();
        join(b, &refs, " | ", d).await
    }
}
macro_rules! adapter {
    ($name:ident,$kind:expr) => {
        pub struct $name(pub(super) super::infrastructure_security::Runtime);
        impl $name {
            pub fn new(
                http: std::sync::Arc<crate::http::AdapterHttp>,
                admission: std::sync::Arc<dyn ssx_application::SearchAdmission>,
                config: super::infrastructure_security::SecurityConfig,
            ) -> Self {
                Self(super::infrastructure_security::Runtime {
                    http,
                    admission,
                    config,
                    kind: $kind,
                })
            }
        }
        impl ssx_application::SearchEngine for $name {
            fn search<'a>(
                &'a self,
                q: &'a ssx_domain::SearchRequest,
            ) -> ssx_application::PortFuture<'a, ssx_domain::AdapterResponse> {
                Box::pin(self.0.run(q, None))
            }
            fn search_with_context<'a>(
                &'a self,
                q: &'a ssx_domain::SearchRequest,
                c: ssx_application::SearchContext,
            ) -> ssx_application::PortFuture<'a, ssx_domain::AdapterResponse> {
                Box::pin(self.0.run(q, Some(c.deadline)))
            }
        }
    };
}
pub(super) use adapter;
pub(super) async fn add(p: &mut OwnedVec<OwnedString>, v: OwnedString) -> Result<()> {
    p.try_push(v).map_err(resource)
}
pub(super) async fn slice_repr(b: &Budget, v: &Value, n: usize, d: Instant) -> Result<OwnedString> {
    match v {
        Value::String(_) => prefix(b, v, n, d).await,
        Value::Array(a) => {
            let mut out = owned(b, "[", d).await?;
            for (i, v) in a.iter().take(n).enumerate() {
                checkpoint(Some(d)).await?;
                if i > 0 {
                    out.try_push_str(", ").map_err(resource)?;
                }
                let part = source_repr::repr_budgeted(v, b, Some(d))
                    .await
                    .map_err(|e| {
                        if e == source_repr::Error::Deadline {
                            timeout()
                        } else {
                            error("source slice exceeds representation bounds")
                        }
                    })?;
                out.try_push_str(part.as_str()).map_err(resource)?;
            }
            out.try_push(']').map_err(resource)?;
            Ok(out)
        }
        Value::Object(_) => Err(error("unhashable type: 'slice'")),
        _ => Err(error(format!("'{}' object is not subscriptable", typ(v)))),
    }
}
pub(super) async fn joined_values(
    b: &Budget,
    v: &Value,
    limit: usize,
    d: Instant,
) -> Result<OwnedString> {
    let mut out = OwnedString::new(b).map_err(resource)?;
    match v {
        Value::Array(a) => {
            for (i, v) in a.iter().take(limit).enumerate() {
                let text = v.as_str().ok_or_else(|| {
                    error(format!(
                        "sequence item {i}: expected str instance, {} found",
                        typ(v)
                    ))
                })?;
                if i > 0 {
                    out.try_push_str(", ").map_err(resource)?;
                }
                let text = owned(b, text, d).await?;
                out.try_push_str(text.as_str()).map_err(resource)?;
            }
        }
        Value::String(s) => {
            for (i, c) in text(v)?.chars().take(limit).enumerate() {
                checkpoint(Some(d)).await?;
                if i > 0 {
                    out.try_push_str(", ").map_err(resource)?;
                }
                out.try_push(c).map_err(resource)?;
            }
            let _ = s;
        }
        Value::Object(m) => {
            for (i, (k, _)) in m.iter().take(limit).enumerate() {
                checkpoint(Some(d)).await?;
                if i > 0 {
                    out.try_push_str(", ").map_err(resource)?;
                }
                out.try_push_str(
                    k.as_str()
                        .ok_or_else(|| error("non-scalar source key cannot project"))?,
                )
                .map_err(resource)?;
            }
        }
        _ => return Err(error(format!("'{}' object is not iterable", typ(v)))),
    }
    Ok(out)
}
pub(super) fn float(v: &Value) -> Result<f64> {
    match v {
        Value::Bool(b) => Ok(u8::from(*b) as f64),
        Value::Number(Number::Float(f)) => Ok(*f),
        Value::Number(n) => n
            .integer_text()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|n| n.is_finite())
            .ok_or_else(|| error("int too large to convert to float")),
        Value::String(t) => {
            let s = t
                .as_str()
                .ok_or_else(|| error("non-scalar float string"))?
                .trim_matches(ssx_json::python_whitespace);
            let mut out = String::new();
            let chars = s.chars().collect::<Vec<_>>();
            for (i, c) in chars.iter().enumerate() {
                if *c == '_' {
                    if i == 0
                        || i + 1 == chars.len()
                        || ssx_json::decimal_value(chars[i - 1]).is_none()
                        || ssx_json::decimal_value(chars[i + 1]).is_none()
                    {
                        return Err(error("could not convert string to float"));
                    }
                } else if let Some(n) = ssx_json::decimal_value(*c) {
                    out.push(char::from(b'0' + n))
                } else {
                    out.push(*c)
                }
            }
            if out.is_empty() {
                return Err(error("could not convert string to float"));
            }
            out.parse()
                .map_err(|_| error("could not convert string to float"))
        }
        _ => Err(error(format!(
            "float() argument must be a string or a real number, not '{}'",
            typ(v)
        ))),
    }
}
#[derive(Clone, Debug)]
pub struct SecuritySettings {
    pub censys: SecurityConfig,
    pub shodan: SecurityConfig,
    pub virustotal: SecurityConfig,
    pub vulncheck: SecurityConfig,
    pub crtsh: SecurityConfig,
}
impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            censys: SecurityConfig::defaults(Kind::Censys),
            shodan: SecurityConfig::defaults(Kind::Shodan),
            virustotal: SecurityConfig::defaults(Kind::VirusTotal),
            vulncheck: SecurityConfig::defaults(Kind::VulnCheck),
            crtsh: SecurityConfig::defaults(Kind::CrtSh),
        }
    }
}
impl SecuritySettings {
    pub fn from_resolved(c: &ssx_config::Config) -> Self {
        let resolve = |kind: Kind| {
            c.engines.get(kind.name()).map_or_else(
                || SecurityConfig::defaults(kind),
                |v| SecurityConfig::from_resolved(v, kind),
            )
        };
        Self {
            censys: resolve(Kind::Censys),
            shodan: resolve(Kind::Shodan),
            virustotal: resolve(Kind::VirusTotal),
            vulncheck: resolve(Kind::VulnCheck),
            crtsh: resolve(Kind::CrtSh),
        }
    }
}
/// Engine-local factory pack; the composition root retains HTTP and admission ownership.
pub fn register_infrastructure_security(
    registry: &mut ssx_registry::Registry,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    settings: SecuritySettings,
) -> std::result::Result<(), ssx_registry::RegistryError> {
    macro_rules! attach {
        ($field:ident,$adapter:ident) => {
            let h = http.clone();
            let a = admission.clone();
            let c = settings.$field;
            registry.register(
                stringify!($field),
                Arc::new(move || {
                    Box::new(super::$field::$adapter::new(
                        h.clone(),
                        a.clone(),
                        c.clone(),
                    )) as Box<dyn ssx_registry::Engine>
                }) as ssx_registry::EngineFactory,
            )?;
        };
    }
    attach!(censys, CensysAdapter);
    attach!(shodan, ShodanAdapter);
    attach!(virustotal, VirusTotalAdapter);
    attach!(vulncheck, VulnCheckAdapter);
    attach!(crtsh, CrtShAdapter);
    Ok(())
}
