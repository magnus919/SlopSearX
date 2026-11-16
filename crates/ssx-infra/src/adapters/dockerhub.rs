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
pub struct DockerHubConfig {
    pub base_url: String,
    pub timeout_ms: u64,
    pub max_results: usize,
}
impl Default for DockerHubConfig {
    fn default() -> Self {
        Self {
            base_url: "".into(),
            timeout_ms: 5000,
            max_results: 10,
        }
    }
}
pub(super) struct DockerHubAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: DockerHubConfig,
}
impl DockerHubAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: DockerHubConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn search_inner(&self, request: &SearchRequest) -> AdapterResponse {
        let started = Instant::now();
        match self.admission.admit("dockerhub").await {
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
        let (namespace, repository) = repository_scope(&request.query);
        let base = if self.config.base_url.is_empty() {
            default_url(&namespace)
        } else {
            self.config.base_url.clone()
        };
        let mut url = Url::parse(&base).map_err(|_| error("invalid Docker Hub URL"))?;
        url.set_query(None);
        url.query_pairs_mut()
            .append_pair("name", &repository)
            .append_pair(
                "page_size",
                &candidate_size(self.config.max_results).to_string(),
            );
        let mut outbound = Request::get(url.as_str()).map_err(|_| error("invalid request URL"))?;
        outbound.headers.insert(
            "user-agent",
            "SlopSearX/0.1.0 (meta search engine; agent-native)"
                .parse()
                .expect("static header"),
        );
        outbound
            .headers
            .insert("accept", "application/json".parse().expect("static header"));
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
        let obj = data.as_object().ok_or(error("root must be object"))?;
        let empty = Vec::new();
        let rows = match obj.get("results") {
            None => &empty,
            Some(Value::Object(v)) if v.is_empty() => &empty,
            Some(Value::String(v)) if v.is_empty() => &empty,
            Some(v) => v.as_array().ok_or(error("results must be array"))?,
        };
        let mut ordered = Vec::new();
        for (idx, row) in rows.iter().enumerate() {
            let row = row.as_object().ok_or(error("result must be object"))?;
            let name = python_string(row.get("name"));
            let ns = row
                .get("namespace")
                .map(|v| python_string(Some(v)))
                .unwrap_or_else(|| namespace.clone());
            ordered.push((
                ssx_json::casefold(name.trim_matches(ssx_json::python_whitespace)) != repository,
                ssx_json::casefold(ns.trim_matches(ssx_json::python_whitespace)) != namespace,
                idx,
                row,
            ));
        }
        ordered.sort_by_key(|(name, ns, idx, _)| (*name, *ns, *idx));
        let zero = ssx_json::json!(0);
        let mut results = Vec::new();
        for (idx, (_, _, _, row)) in ordered
            .into_iter()
            .take(self.config.max_results)
            .enumerate()
        {
            let name = python_string(row.get("name"));
            let ns = row
                .get("namespace")
                .map(|v| python_string(Some(v)))
                .unwrap_or_else(|| namespace.clone());
            let ns = ns.trim_matches(ssx_json::python_whitespace);
            let ns = if ns.is_empty() { &namespace } else { ns };
            let mut content = source_text(row.get("description"))
                .map_err(|_| error("description must be text"))?;
            let pulls = row.get("pull_count").unwrap_or(&zero);
            let stars = row.get("star_count").unwrap_or(&zero);
            if truthy(pulls) {
                content.push_str(&format!(
                    " — Pulls: {}",
                    python_grouped(pulls).map_err(|_| error("invalid pull formatting"))?
                ));
            }
            let score = if truthy(stars) {
                python_float(stars)
            } else {
                python_float(pulls).map(|v| v / 100000.0)
            }
            .map_err(|_| error("invalid score"))?;
            let (url, title) = if ssx_json::casefold(ns) == "library" {
                (format!("https://hub.docker.com/_/{name}"), name)
            } else {
                (
                    format!("https://hub.docker.com/r/{ns}/{name}"),
                    format!("{ns}/{name}"),
                )
            };
            results.push(SearchResult {
                url,
                title,
                content: content.chars().take(500).collect(),
                engine: "dockerhub".into(),
                position: (idx + 1) as i64,
                score: FiniteF64::new(score).map_err(|_| error("nonfinite score"))?,
                ..SearchResult::default()
            });
        }
        Ok(results)
    }
}
impl SearchEngine for DockerHubAdapter {
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
fn candidate_size(max: usize) -> usize {
    max.saturating_add(max.saturating_mul(2).min(40)).min(100)
}
fn repository_scope(query: &str) -> (String, String) {
    let value = query.trim_matches(ssx_json::python_whitespace);
    let normalized = ssx_json::casefold(value);
    let mut parts = normalized
        .split('/')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();
    if parts
        .first()
        .is_some_and(|v| ["docker.io", "index.docker.io", "registry-1.docker.io"].contains(v))
    {
        parts.remove(0);
    }
    let (ns, repo) = match parts.as_slice() {
        [repo] => ("library", *repo),
        [ns, repo] => (*ns, *repo),
        _ => return ("library".into(), value.into()),
    };
    (
        ns.into(),
        repo.split('@')
            .next()
            .unwrap_or_default()
            .split(':')
            .next()
            .unwrap_or_default()
            .into(),
    )
}

fn default_url(namespace: &str) -> String {
    let encoded = namespace
        .as_bytes()
        .iter()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(byte) {
                (*byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect::<String>();
    format!("https://hub.docker.com/v2/namespaces/{encoded}/repositories")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_route_scope_and_percent_encoding_match_source() {
        for (query, namespace, name) in [
            ("python", "library", "python"),
            ("docker.io/acme/python:3.13", "acme", "python"),
            (
                "registry-1.docker.io/acme/python@sha256:abc",
                "acme",
                "python",
            ),
            ("quay.io/a/b/c", "library", "quay.io/a/b/c"),
            ("STRASSE/straße", "strasse", "strasse"),
            ("a:b/python", "a:b", "python"),
        ] {
            assert_eq!(repository_scope(query), (namespace.into(), name.into()));
        }
        assert_eq!(
            default_url("a:b @&"),
            "https://hub.docker.com/v2/namespaces/a%3Ab%20%40%26/repositories"
        );
        assert_eq!(
            default_url("münchen"),
            "https://hub.docker.com/v2/namespaces/m%C3%BCnchen/repositories"
        );
        assert_eq!(candidate_size(2), 6);
        assert_eq!(candidate_size(80), 100);
    }
}
