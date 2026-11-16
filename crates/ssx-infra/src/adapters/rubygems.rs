use super::crates_io::{python_float, python_grouped, python_string, source_text, truthy};
use crate::http::{AdapterHttp, Cancellation, Error as HttpError, Options, Request};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use ssx_json::Value;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use url::Url;

#[derive(Clone, Debug)]
pub struct RubyGemsConfig {
    pub base_url: String,
    pub timeout_ms: u64,
    pub max_results: usize,
}
impl Default for RubyGemsConfig {
    fn default() -> Self {
        Self {
            base_url: "https://rubygems.org/api/v1/search.json".into(),
            timeout_ms: 5000,
            max_results: 10,
        }
    }
}
pub(super) struct RubyGemsAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: RubyGemsConfig,
}
impl RubyGemsAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: RubyGemsConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn search_inner(&self, request: &SearchRequest) -> AdapterResponse {
        let started = Instant::now();
        match self.admission.admit("rubygems").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => {
                return failed(
                    EngineStatus::RateLimited,
                    "request denied by rate policy",
                    started,
                );
            }
            AdmissionDecision::Unavailable => {
                return failed(EngineStatus::Error, "rate policy unavailable", started);
            }
        }
        let outcome = self.fetch(request).await;
        match outcome {
            Ok(results) => AdapterResponse {
                results,
                status: EngineStatus::Ok,
                latency_ms: elapsed(started),
                ..AdapterResponse::default()
            },
            Err((status, message)) => failed(status, message, started),
        }
    }
    async fn fetch(
        &self,
        request: &SearchRequest,
    ) -> Result<Vec<SearchResult>, (EngineStatus, &'static str)> {
        let base = &self.config.base_url;
        let mut url = Url::parse(base).map_err(|_| error("invalid RubyGems URL"))?;
        url.set_query(None);
        url.query_pairs_mut().append_pair("query", &request.query);
        let outbound = Request::get(url.as_str()).map_err(|_| error("invalid request URL"))?;
        let mut session = self
            .http
            .session(
                Options::default(),
                tokio::time::Instant::now() + Duration::from_millis(self.config.timeout_ms),
                Cancellation::default(),
            )
            .map_err(|_| error("HTTP session unavailable"))?;
        let response = session.execute(outbound).await.map_err(|e| {
            if matches!(e, HttpError::Deadline) {
                (EngineStatus::Timeout, "request timed out")
            } else {
                error("request failed")
            }
        })?;
        if response.status == 429 {
            return Err((EngineStatus::RateLimited, "rate limited"));
        }
        if !(200..300).contains(&response.status) {
            return Err(error("unsuccessful status"));
        }
        let data: Value =
            ssx_json::from_slice(&response.body).map_err(|_| error("invalid JSON"))?;
        let empty = Vec::new();
        let rows = if let Some(rows) = data.as_array() {
            rows
        } else {
            let obj = data
                .as_object()
                .ok_or(error("root must be array or object"))?;
            match obj.get("results") {
                None => &empty,
                Some(v) => v.as_array().ok_or(error("results must be array"))?,
            }
        };
        let mut results = Vec::new();
        for (idx, value) in rows.iter().take(self.config.max_results).enumerate() {
            let row = value.as_object().ok_or(error("result must be object"))?;
            let name = python_string(row.get("name"));
            let version = python_string(row.get("version"));
            let info = row
                .get("info")
                .filter(|v| truthy(v))
                .or_else(|| row.get("description").filter(|v| truthy(v)));
            let authors = row.get("authors").filter(|v| truthy(v));
            // An f-string converts arbitrary info when authors is truthy; without it,
            // Python string concatenation/slicing still requires text.
            let mut content = if let Some(authors) = authors {
                let author = python_string(Some(authors));
                if let Some(info) = info {
                    format!("{} — by {author}", python_string(Some(info)))
                } else {
                    format!("by {author}")
                }
            } else {
                source_text(info).map_err(|_| error("description must be text"))?
            };
            let downloads = row.get("downloads").filter(|v| truthy(v));
            let score = if let Some(downloads) = downloads {
                content.push_str(&format!(
                    " | Downloads: {}",
                    python_grouped(downloads).map_err(|_| error("invalid download formatting"))?
                ));
                python_float(downloads).map_err(|_| error("invalid downloads"))? / 1000.0
            } else {
                0.0
            };
            results.push(SearchResult {
                url: format!("https://rubygems.org/gems/{name}"),
                title: format!("{name} {version}"),
                content: content.chars().take(500).collect(),
                engine: "rubygems".into(),
                position: (idx + 1) as i64,
                score: FiniteF64::new(score).map_err(|_| error("nonfinite score"))?,
                ..SearchResult::default()
            });
        }
        Ok(results)
    }
}
impl SearchEngine for RubyGemsAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move { self.search_inner(request).await })
    }
}
fn error(message: &'static str) -> (EngineStatus, &'static str) {
    (EngineStatus::Error, message)
}
fn elapsed(start: Instant) -> FiniteF64 {
    FiniteF64::new(start.elapsed().as_secs_f64() * 1000.0).unwrap_or_default()
}
fn failed(status: EngineStatus, message: &str, start: Instant) -> AdapterResponse {
    AdapterResponse {
        status,
        error_message: Some(message.into()),
        latency_ms: elapsed(start),
        ..AdapterResponse::default()
    }
}
