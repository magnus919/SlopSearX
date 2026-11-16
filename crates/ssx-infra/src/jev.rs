//! Optional, bounded specialist advice. F09 remains the sole scope authority.
use crate::{
    http::{AdapterHttp, Bounds, Cancellation, Options, Request},
    valkey::{Read, Valkey},
};
use reqwest::{
    Method,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use sha2::{Digest, Sha256};
use ssx_application::{
    policy::{PolicyError, PolicyService, ScopePreview, ScopeProposal, ValidatedScope},
    scope::CatalogSnapshot,
};
use ssx_domain::FiniteF64;
use ssx_json::{Value, json};
use ssx_registry::{Registry, RoutingRole};
use std::{collections::BTreeMap, num::NonZeroU64, time::Duration};

pub const MODEL: &str = "jev-1.13.0";
const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
fn strip(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
}
fn hash(s: &[u8]) -> String {
    Sha256::digest(s)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
/// Secrets deliberately have no Debug/Serialize implementation.
pub struct JevConfig {
    key: String,
    threshold: f64,
    pub timeout: Duration,
    pub cache_ttl: Option<NonZeroU64>,
}
impl JevConfig {
    pub fn from_environment(env: &BTreeMap<String, String>) -> Option<Self> {
        let key = strip(env.get("TYPESAFE_API_KEY").map_or("", String::as_str));
        if key.is_empty() {
            return None;
        }
        let threshold = env
            .get("JEV_SPECIALIST_THRESHOLD")
            .map_or("0.65", String::as_str);
        let timeout = env.get("JEV_TIMEOUT_MS").map_or("1000", String::as_str);
        let (threshold, timeout) = match (number(threshold), integer(timeout)) {
            (Some(t), ParsedInt::Value(ms)) => (t, ms.max(50)),
            (Some(t), ParsedInt::NegativeOverflow) => (t, 50),
            (Some(_), ParsedInt::PositiveOverflow) => return None,
            _ => (0.65, 1000),
        };
        let threshold = if threshold.is_nan() || threshold <= 0.0 {
            0.0
        } else if threshold >= 1.0 {
            1.0
        } else {
            threshold
        };
        let ttl = match integer(
            env.get("JEV_ROUTING_CACHE_TTL_SECONDS")
                .map_or("3600", String::as_str),
        ) {
            ParsedInt::Value(n) => NonZeroU64::new(n.max(1) as u64),
            ParsedInt::NegativeOverflow => NonZeroU64::new(1),
            ParsedInt::Invalid | ParsedInt::PositiveOverflow => None,
        };
        Some(Self {
            key: key.into(),
            threshold,
            timeout: Duration::from_millis(timeout as u64),
            cache_ttl: ttl,
        })
    }
    pub fn threshold(&self) -> f64 {
        self.threshold
    }
    pub fn cache_identity(&self) -> String {
        let raw = ssx_json::canonical_json(
            &json!({"model":MODEL,"cards":"1","threshold":self.threshold}),
            ssx_json::JsonProfile::AsciiSpaced,
        )
        .expect("finite threshold");
        hash(&raw)[..16].into()
    }
    pub fn cache_key(&self, query: &str, candidates: &[String]) -> String {
        let mut names = candidates.to_vec();
        names.sort();
        // Python sorted-key, UTF-8, default-spaced JSON. Engine IDs are ASCII;
        // query must preserve non-ASCII and whitespace is stripped only here.
        let names = ssx_json::canonical_json(&json!(names), ssx_json::JsonProfile::AsciiSpaced)
            .expect("engine names");
        let raw = format!(
            "{{\"candidates\": {}, \"identity\": {}, \"query\": {}}}",
            String::from_utf8(names).unwrap(),
            ssx_json::to_string(&self.cache_identity()).unwrap(),
            ssx_json::to_string(strip(query)).unwrap()
        );
        format!("jev-routing:v1:{}", hash(raw.as_bytes()))
    }
}
// Python accepts underscores only between digits. Keep malformed values atomic.
fn numeric_text(s: &str) -> Option<String> {
    ssx_domain::storage::normalize_python_number_text(s).ok()
}
enum ParsedInt {
    Invalid,
    Value(i64),
    PositiveOverflow,
    NegativeOverflow,
}
fn integer(s: &str) -> ParsedInt {
    let Some(text) = numeric_text(s) else {
        return ParsedInt::Invalid;
    };
    let digits = text.strip_prefix(['+', '-']).unwrap_or(&text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return ParsedInt::Invalid;
    }
    match text.parse() {
        Ok(n) => ParsedInt::Value(n),
        Err(_) if text.starts_with('-') => ParsedInt::NegativeOverflow,
        Err(_) => ParsedInt::PositiveOverflow,
    }
}
fn number(s: &str) -> Option<f64> {
    numeric_text(s)?.parse().ok()
}
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub engines: Vec<String>,
    pub scores: BTreeMap<String, FiniteF64>,
    pub latency_ms: f64,
}
impl Decision {
    pub fn advice(&self) -> Vec<(String, FiniteF64)> {
        self.engines
            .iter()
            .map(|n| (n.clone(), self.scores[n].clone()))
            .collect()
    }
}
fn score(v: &Value, boolean: bool) -> Option<FiniteF64> {
    let n = match v {
        Value::Bool(b) if boolean => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        _ => v.as_f64()?,
    };
    if !(0.0..=1.0).contains(&n) {
        return None;
    }
    FiniteF64::new(n).ok()
}
fn selected(scores: &BTreeMap<String, FiniteF64>, threshold: f64) -> Vec<String> {
    let mut names: Vec<_> = scores
        .iter()
        .filter(|(_, v)| v.get() >= threshold)
        .map(|(n, _)| n.clone())
        .collect();
    names.sort_by(|a, b| {
        scores[b]
            .get()
            .partial_cmp(&scores[a].get())
            .expect("finite score")
            .then_with(|| a.cmp(b))
    });
    names
}
fn cached(v: &Value, names: &[String], threshold: f64) -> Option<Decision> {
    let scores: BTreeMap<_, _> = v
        .get("scores")?
        .as_object()?
        .iter()
        .map(|(n, v)| Some((n.clone(), score(v, false)?)))
        .collect::<Option<_>>()?;
    if scores.keys().ne(names.iter()) {
        return None;
    }
    let engines: Vec<String> = ssx_json::from_value(v.get("engines")?.clone()).ok()?;
    if engines != selected(&scores, threshold) {
        return None;
    }
    Some(Decision {
        engines,
        scores,
        latency_ms: 0.0,
    })
}
/// Own one router per runtime so the F08 client pool is reused. No retries.
pub struct JevRouter {
    config: JevConfig,
    http: std::sync::Arc<AdapterHttp>,
    registry: Registry,
    endpoint: String,
}
impl JevRouter {
    pub fn new(config: JevConfig, registry: Registry) -> Self {
        Self {
            config,
            http: std::sync::Arc::new(AdapterHttp::default()),
            registry,
            endpoint: ENDPOINT.into(),
        }
    }
    /// Initialize the ordinary F08 profile before publishing this production
    /// router. Startup work never sends provider requests or spends a query
    /// deadline. Ephemeral default environments remain explicitly cold.
    pub async fn new_ready(
        config: JevConfig,
        registry: Registry,
        deadline: tokio::time::Instant,
        cancel: Cancellation,
    ) -> crate::http::Result<(Self, crate::http::Preparation)> {
        let (http, preparation) = AdapterHttp::new_ready(deadline, cancel).await?;
        Ok((
            Self {
                config,
                http,
                registry,
                endpoint: ENDPOINT.into(),
            },
            preparation,
        ))
    }
    /// Explicit fixture endpoint; production composition uses `new` or `new_ready`.
    pub fn with_endpoint(config: JevConfig, registry: Registry, endpoint: String) -> Self {
        Self {
            config,
            http: std::sync::Arc::new(AdapterHttp::default()),
            registry,
            endpoint,
        }
    }
    pub fn config(&self) -> &JevConfig {
        &self.config
    }
    pub fn candidates(&self, service: &PolicyService, proposal: &ScopeProposal) -> Vec<String> {
        let mut names: Vec<_> = self
            .registry
            .descriptors()
            .filter(|d| {
                d.routing.role == RoutingRole::Specialist
                    && !d.sensitive
                    && !service.policy.sensitive_engines.contains(&d.name)
            })
            .filter(|d| {
                service
                    .resolver
                    .advice_eligible(&proposal.request, &service.catalog, &d.name)
            })
            .map(|d| d.name.clone())
            .collect();
        names.sort();
        names
    }
    pub fn request_body(
        &self,
        query: &str,
        names: &[String],
        catalog: &CatalogSnapshot,
    ) -> Option<Vec<u8>> {
        let mut questions = BTreeMap::new();
        for name in names {
            let d = self.registry.descriptor(name)?;
            let e = catalog.get(name)?;
            let text = format!(
                "Specialist engine id: {name}. Display name: {}. Purpose: {}. Use when: {}. Categories: {}. Would this named specialist provide distinctive, directly useful evidence beyond ordinary general web search for the user query? Answer no when general search is sufficient.",
                d.display_name,
                d.routing.purpose.as_deref()?,
                d.routing.use_when.as_deref()?,
                e.categories.join(", ")
            );
            questions.insert(name, json!({"type":"noul","instructions":text}));
        }
        // HTTPX compact encoding preserves insertion order, including question fields.
        let questions = questions
            .iter()
            .map(|(n, v)| {
                format!(
                    "{}:{{\"type\":\"noul\",\"instructions\":{}}}",
                    ssx_json::to_string(n).unwrap(),
                    ssx_json::to_string(&v["instructions"]).unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        Some(format!("{{\"model\":\"{MODEL}\",\"state\":{{\"user_query\":{}}},\"questions\":{{{questions}}}}}",ssx_json::to_string(query).ok()?).into_bytes())
    }
    pub async fn route(
        &self,
        proposal: &ScopeProposal,
        service: &PolicyService,
        cache: Option<&Valkey>,
    ) -> Option<Decision> {
        // Validate policy before any disclosure/provider I/O.
        service.preview(proposal).ok()?;
        if !ScopeResolverAdvice::allowed(&proposal.request) {
            return None;
        }
        let names = self.candidates(service, proposal);
        if names.is_empty() {
            return Some(Decision {
                engines: vec![],
                scores: BTreeMap::new(),
                latency_ms: 0.0,
            });
        }
        let key = self.config.cache_key(&proposal.request.query, &names);
        if let Some(store) = cache
            && let Read::Found(v) = store.get_object(&key).await
            && let Some(d) = cached(&v, &names, self.config.threshold)
        {
            return Some(d);
        }
        let body = self.request_body(&proposal.request.query, &names, &service.catalog)?;
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.config.key)).ok()?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let options = Options {
            bounds: Bounds {
                requests: 1,
                redirects: 0,
                wire_bytes: 1024 * 1024,
                decoded_bytes: 1024 * 1024,
                request_bytes: 1024 * 1024,
            },
            ..Default::default()
        };
        let started = tokio::time::Instant::now();
        let deadline = started.checked_add(self.config.timeout)?;
        let mut session = self
            .http
            .session(options, deadline, Cancellation::default())
            .ok()?;
        let response = session
            .execute(Request {
                method: Method::POST,
                url: self.endpoint.parse().ok()?,
                headers,
                body,
            })
            .await
            .ok()?;
        if !(200..300).contains(&response.status) {
            return None;
        }
        let v: Value = ssx_json::from_slice(&response.body).ok()?;
        if v["model"] != MODEL {
            return None;
        }
        let scores: BTreeMap<_, _> = names
            .iter()
            .map(|n| Some((n.clone(), score(&v["answers"][n.as_str()]["noul"], true)?)))
            .collect::<Option<_>>()?;
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        let engines = selected(&scores, self.config.threshold);
        let decision = Decision {
            engines,
            scores,
            latency_ms: started.elapsed().as_secs_f64() * 1000.0,
        };
        if let (Some(store), Some(ttl)) = (cache, self.config.cache_ttl) {
            let bytes = ssx_json::canonical_json(
                &json!({"engines":decision.engines,"scores":decision.scores}),
                ssx_json::JsonProfile::AsciiSpaced,
            )
            .ok()?;
            let _ = store.put(&key, &bytes, ttl).await;
        }
        Some(decision)
    }
    pub async fn prepare(
        &self,
        proposal: &ScopeProposal,
        service: &PolicyService,
        cache: Option<&Valkey>,
    ) -> Result<PreparedAdvice, PolicyError> {
        service.preview(proposal)?;
        let advice = self
            .route(proposal, service, cache)
            .await
            .map_or_else(Vec::new, |d| d.advice());
        let preview = service.preview_with_advice(proposal, &advice)?;
        Ok(PreparedAdvice {
            proposal: proposal.clone(),
            advice,
            preview,
        })
    }
    pub async fn shutdown(&self) {
        self.http.shutdown_and_wait().await;
    }
}
use ssx_application::scope::ScopeResolver as ScopeResolverAdvice;
/// A preview never becomes authority. Reuse the frozen advice when authorizing;
/// current policy/catalog are reapplied, with no second provider request.
pub struct PreparedAdvice {
    proposal: ScopeProposal,
    advice: Vec<(String, FiniteF64)>,
    pub preview: ScopePreview,
}
impl PreparedAdvice {
    pub fn authorize(self, service: &PolicyService) -> Result<ValidatedScope, PolicyError> {
        service.authorize_with_advice(self.proposal, self.advice)
    }
}

/// Shared keyless/enhanced entry point. None preserves deterministic F09 choices.
pub async fn prepare_optional(
    router: Option<&JevRouter>,
    proposal: &ScopeProposal,
    service: &PolicyService,
    cache: Option<&Valkey>,
) -> Result<PreparedAdvice, PolicyError> {
    match router {
        Some(router) => router.prepare(proposal, service, cache).await,
        None => Ok(PreparedAdvice {
            proposal: proposal.clone(),
            advice: Vec::new(),
            preview: service.preview(proposal)?,
        }),
    }
}

#[cfg(test)]
mod ready_constructor_tests {
    use super::*;
    #[tokio::test]
    async fn actual_production_ready_router_constructor_prepares_without_dispatch() {
        if crate::http::default_ca_test_environment(
            "jev::ready_constructor_tests::actual_production_ready_router_constructor_prepares_without_dispatch",
        ) {
            return;
        }
        let config = JevConfig::from_environment(&BTreeMap::from([(
            "TYPESAFE_API_KEY".into(),
            "synthetic".into(),
        )]))
        .unwrap();
        let (router, preparation) = JevRouter::new_ready(
            config,
            Registry::builtins().unwrap(),
            tokio::time::Instant::now() + Duration::from_secs(10),
            Cancellation::default(),
        )
        .await
        .unwrap();
        assert_eq!(preparation, crate::http::Preparation::Prepared);
        assert_eq!(router.endpoint, ENDPOINT);
        assert_eq!(router.config.timeout, Duration::from_millis(1000));
        // Constructor has no query, request, URL or provider exchange path.
        assert_eq!(router.http.retained_pools(), 1);
        router.shutdown().await;
        assert_eq!(router.http.retained_pools(), 0);
    }
}
