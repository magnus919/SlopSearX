//! Bounded optional Score provider and host-owned result ordering.
use crate::http::{AdapterHttp, Bounds, Cancellation, Options, Request};
use crate::search::{RerankDecision, RerankStage, RerankState};
use reqwest::{
    Method,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue},
};
pub use rerank::{MAX_RESPONSE_BYTES, MODEL};
use sha2::{Digest, Sha256};
use ssx_application::{
    PortFuture,
    rerank::{self, Candidate, Decision, Provider},
};
use ssx_domain::{SearchRequest, SearchResult};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};
use tokio::{sync::Semaphore, time::Instant};
mod strict_json;

/// Deliberately neither Debug nor Serialize: credential bytes never enter reports.
pub struct JevConfig {
    key: String,
}
impl JevConfig {
    pub fn from_environment(env: &BTreeMap<String, String>) -> Option<Self> {
        let key = env
            .get("TYPESAFE_API_KEY")?
            .trim_matches(ssx_json::python_whitespace);
        (!key.is_empty()).then(|| Self { key: key.into() })
    }
}
/// Only aggregate usage is observable; no query, candidate, key or response text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: String,
    pub output_tokens: String,
    pub candidates: usize,
    pub latency: Duration,
}
pub trait UsageSink: Send + Sync {
    fn observed(&self, usage: Usage);
}
pub struct NoopUsage;
impl UsageSink for NoopUsage {
    fn observed(&self, _: Usage) {}
}
pub struct JevReranker {
    config: JevConfig,
    http: AdapterHttp,
    endpoint: String,
    semaphore: Semaphore,
    usage: Arc<dyn UsageSink>,
}
impl JevReranker {
    pub fn new(config: JevConfig, usage: Arc<dyn UsageSink>) -> Self {
        Self::with_transport(
            config,
            AdapterHttp::default(),
            "https://api.typesafe.ai/v1/systemone".into(),
            usage,
        )
    }
    /// Trusted native composition/test seam; endpoint is never a search input.
    pub fn with_transport(
        config: JevConfig,
        http: AdapterHttp,
        endpoint: String,
        usage: Arc<dyn UsageSink>,
    ) -> Self {
        Self {
            config,
            http,
            endpoint,
            semaphore: Semaphore::new(2),
            usage,
        }
    }
    pub async fn shutdown(&self) {
        self.semaphore.close();
        self.http.shutdown_and_wait().await;
    }
    pub fn request_body(query: &str, candidates: &[Candidate]) -> Option<Vec<u8>> {
        if !rerank::valid_input(query, candidates) {
            return None;
        }
        let quote = |s: &str| ssx_json::to_string(s).expect("valid Unicode string");
        let rows = candidates
            .iter()
            .map(|c| {
                format!(
                    "{{\"id\": {}, \"title\": {}, \"url\": {}, \"snippet\": {}}}",
                    quote(&c.id),
                    quote(&c.title),
                    quote(&c.url),
                    quote(&c.snippet)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let levels = format!(
            "[{}]",
            rerank::LEVELS
                .iter()
                .map(|s| quote(s))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let questions = candidates
            .iter()
            .map(|c| {
                format!(
                    "{}: {{\"type\": \"score\", \"instructions\": {}, \"criteria\": {levels}}}",
                    quote(&c.id),
                    quote(&rerank::INSTRUCTIONS.replace("{id}", &c.id))
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let raw = format!("{{\"model\": {}, \"state\": {{\"query\": {}, \"candidates\": [{rows}]}}, \"questions\": {{{questions}}}}}", quote(MODEL), quote(query)).into_bytes();
        (raw.len() <= rerank::MAX_REQUEST_BYTES).then_some(raw)
    }
    async fn parse(
        &self,
        bytes: &[u8],
        candidates: &[Candidate],
        started: Instant,
        deadline: Instant,
    ) -> Option<Decision> {
        let doc = strict_json::parse(bytes, deadline).await?;
        let root = doc.root();
        // Observe exact nonnegative Python integer usage even if answers are invalid.
        if let Some(usage) = root.get("usage")
            && let (Some(input), Some(output)) =
                (usage.get("input_tokens"), usage.get("output_tokens"))
            && let (Some(input), Some(output)) =
                (input.unsigned_integer(), output.unsigned_integer())
        {
            self.usage.observed(Usage {
                input_tokens: input.into(),
                output_tokens: output.into(),
                candidates: candidates.len(),
                latency: started.elapsed(),
            });
        }
        if !root.get("model")?.is_string(MODEL) {
            return None;
        }
        let answers = root.get("answers")?;
        if !answers.has_keys(
            &candidates
                .iter()
                .map(|c| c.id.clone())
                .collect::<BTreeSet<_>>(),
        ) {
            return None;
        }
        let mut scores = candidates
            .iter()
            .map(|c| {
                let answer = answers.get(&c.id)?;
                if !answer.get("type")?.is_string("score") {
                    return None;
                }
                Some((c.id.clone(), answer.get("score")?.score()?))
            })
            .collect::<Option<Vec<_>>>()?;
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).expect("finite scores"));
        Some(Decision {
            ordered_ids: scores.into_iter().map(|(id, _)| id).collect(),
        })
    }
    async fn execute(&self, query: &str, candidates: &[Candidate]) -> Option<Decision> {
        let body = Self::request_body(query, candidates)?;
        let started = Instant::now();
        let deadline = started + Duration::from_secs(1);
        tokio::time::timeout_at(deadline, async {
            let _permit = self.semaphore.acquire().await.ok()?;
            let mut headers = HeaderMap::new();
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", self.config.key)).ok()?,
            );
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let options = Options {
                trust_env: false,
                identity_announced_bytes: Some(MAX_RESPONSE_BYTES),
                follow_redirects: false,
                bounds: Bounds {
                    requests: 1,
                    redirects: 0,
                    request_bytes: rerank::MAX_REQUEST_BYTES,
                    // Encoded bytes have a separate finite native transport bound.
                    wire_bytes: 8 * 1024 * 1024,
                    decoded_bytes: MAX_RESPONSE_BYTES,
                },
                ..Default::default()
            };
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
            if crate::http::identity_announced_oversize(&response.headers, MAX_RESPONSE_BYTES) {
                return None;
            }
            let decision = self
                .parse(&response.body, candidates, started, deadline)
                .await?;
            (Instant::now() < deadline).then_some(decision)
        })
        .await
        .ok()
        .flatten()
    }
}
impl Provider for JevReranker {
    fn shutdown(&self) -> PortFuture<'_, ()> {
        Box::pin(JevReranker::shutdown(self))
    }
    fn rerank<'a>(
        &'a self,
        query: &'a str,
        candidates: &'a [Candidate],
    ) -> PortFuture<'a, Option<Decision>> {
        Box::pin(self.execute(query, candidates))
    }
    fn cache_identity(&self) -> String {
        let body = ssx_json::json!({"provider":"typesafe", "model":MODEL, "version":2,
            "response_limit_bytes":MAX_RESPONSE_BYTES, "response_parser":"strict_json_unique_keys_finite_numbers",
            "levels":rerank::LEVELS, "instructions":rerank::INSTRUCTIONS,
            "limits":[rerank::MAX_CANDIDATES,rerank::MAX_QUERY_BYTES,rerank::MAX_TITLE_BYTES,rerank::MAX_URL_BYTES,rerank::MAX_SNIPPET_BYTES],
            "ordering":"ordinal_score_descending_stable_ties"});
        let bytes = ssx_json::canonical_json(&body, ssx_json::JsonProfile::AsciiSpaced)
            .expect("fixed finite identity");
        let digest = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        format!("jev-score-v2:{}", &digest[..16])
    }
}
/// The only bridge from provider decisions to F14's trusted native result stage.
pub struct HostReranker {
    provider: Arc<dyn Provider>,
}
impl HostReranker {
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self { provider }
    }
    pub fn identity(&self) -> (String, String) {
        (
            self.provider.cache_identity(),
            rerank::HOST_POLICY_IDENTITY.into(),
        )
    }
}
impl RerankStage for HostReranker {
    fn identity(&self) -> Option<(String, String)> {
        Some(HostReranker::identity(self))
    }
    fn shutdown(&self) -> PortFuture<'_, ()> {
        self.provider.shutdown()
    }
    fn rerank<'a>(
        &'a self,
        request: &'a SearchRequest,
        results: &'a [SearchResult],
        deadline: Instant,
    ) -> PortFuture<'a, Result<RerankDecision, ()>> {
        Box::pin(async move {
            if request.query.len() > rerank::MAX_QUERY_BYTES {
                return Ok(RerankDecision {
                    results: results.to_vec(),
                    state: RerankState::Skipped,
                });
            }
            if Instant::now() >= deadline {
                return Ok(RerankDecision {
                    results: results.to_vec(),
                    state: RerankState::Fallback,
                });
            }
            let candidates = rerank::project(results);
            if Instant::now() >= deadline {
                return Ok(RerankDecision {
                    results: results.to_vec(),
                    state: RerankState::Fallback,
                });
            }
            let decision = tokio::time::timeout_at(
                deadline,
                self.provider.rerank(&request.query, &candidates),
            )
            .await
            .ok()
            .flatten();
            let ordered = decision.and_then(|d| rerank::reorder(results, &d));
            Ok(match ordered {
                Some(results) => RerankDecision {
                    results,
                    state: RerankState::Applied,
                },
                None => RerankDecision {
                    results: results.to_vec(),
                    state: RerankState::Fallback,
                },
            })
        })
    }
}

#[cfg(test)]
mod f15_provider_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Reply(AtomicUsize);
    impl crate::http::Transport for Reply {
        fn exchange(&self, request: Request) -> crate::http::ExchangeFuture<'_> {
            Box::pin(async move {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(crate::http::Response {status:200,url:request.url,headers:HeaderMap::new(),body:br#"{"model":"jev-1.13.0","answers":{"c0":{"type":"score","score":1},"c1":{"type":"score","score":9}}}"#.to_vec()})
            })
        }
    }
    struct NoUsage;
    impl UsageSink for NoUsage {
        fn observed(&self, _: Usage) {}
    }
    #[tokio::test]
    async fn native_queue_wait_expires_without_dispatch_and_releases_admission() {
        let reply = Arc::new(Reply(AtomicUsize::new(0)));
        let http = AdapterHttp::default();
        http.set_transport(reply.clone()).unwrap();
        let provider = JevReranker::with_transport(
            JevConfig {
                key: "synthetic".into(),
            },
            http,
            "http://127.0.0.1:1".into(),
            Arc::new(NoUsage),
        );
        let candidates = (0..2)
            .map(|i| Candidate {
                id: format!("c{i}"),
                title: "title".into(),
                url: "https://example.test".into(),
                snippet: "snippet".into(),
            })
            .collect::<Vec<_>>();
        let permits = provider.semaphore.acquire_many(2).await.unwrap();
        let started = Instant::now();
        assert!(provider.execute("queued", &candidates).await.is_none());
        assert!(started.elapsed() >= Duration::from_secs(1));
        assert_eq!(
            reply.0.load(Ordering::SeqCst),
            0,
            "a queue-expired request must never dispatch"
        );
        drop(permits);
        assert_eq!(
            provider
                .execute("next", &candidates)
                .await
                .unwrap()
                .ordered_ids,
            ["c1", "c0"]
        );
        assert_eq!(reply.0.load(Ordering::SeqCst), 1);
        provider.shutdown().await;
    }
}
