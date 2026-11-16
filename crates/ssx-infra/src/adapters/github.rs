use super::community::*;
use crate::http::{AdapterHttp, Error as HttpError, Request};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, SearchRequest, SearchResult};
use ssx_json::Value;
use std::{sync::Arc, time::Instant};
use url::Url;

#[derive(Clone, Debug)]
pub struct GitHubConfig(pub CommunitySettings);
impl Default for GitHubConfig {
    fn default() -> Self {
        Self(CommunitySettings::defaults(
            "https://api.github.com",
            5000.0,
            5,
        ))
    }
}
pub(super) struct GitHubAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: GitHubConfig,
}
impl GitHubAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: GitHubConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn search_inner(&self, request: &SearchRequest) -> AdapterResponse {
        let start = Instant::now();
        match self.admission.admit("github").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => return failed(EngineStatus::RateLimited, None, start),
            AdmissionDecision::Unavailable => {
                return failed(
                    EngineStatus::Error,
                    Some("GitHub admission unavailable"),
                    start,
                );
            }
        }
        let cfg = &self.config.0;
        let Ok(timeout) = cfg.duration() else {
            return failed(EngineStatus::Error, Some("invalid GitHub settings"), start);
        };
        let categories = request.categories.as_deref().unwrap_or_default();
        let mode = if categories.iter().any(|v| v == "github:code") {
            "code"
        } else if categories
            .iter()
            .any(|v| v == "github:issues" || v == "github:prs")
        {
            "issues"
        } else {
            "repositories"
        };
        let endpoint = format!("{}/search/{mode}", cfg.base_url);
        let token = cfg
            .api_key
            .as_ref()
            .map(|v| strip(v.expose()))
            .unwrap_or_default();
        if endpoint.contains("/search/code") && token.is_empty() {
            return failed(
                EngineStatus::Unavailable,
                Some("GitHub token required for code search (set ENGINE_GITHUB_API_KEY)"),
                start,
            );
        }
        let result = async {
            let mut url = Url::parse(&endpoint).map_err(|_| ())?;
            url.set_query(None);
            url.query_pairs_mut()
                .append_pair("q", &request.query)
                .append_pair("per_page", &cfg.max_results.to_string())
                .append_pair("page", "1");
            let mut outgoing = Request::get(url.as_str()).map_err(|_| ())?;
            outgoing
                .headers
                .insert("user-agent", "SlopSearX/0.1.0".parse().map_err(|_| ())?);
            outgoing.headers.insert(
                "accept",
                "application/vnd.github.v3+json".parse().map_err(|_| ())?,
            );
            if !token.is_empty() {
                outgoing.headers.insert(
                    "authorization",
                    format!("Bearer {token}").parse().map_err(|_| ())?,
                );
            }
            Ok::<_, ()>(outgoing)
        }
        .await;
        let Ok(outgoing) = result else {
            return failed(EngineStatus::Error, Some("invalid GitHub request"), start);
        };
        let response = match send(&self.http, outgoing, timeout).await {
            Ok(v) => v,
            Err(HttpError::Deadline) => return failed(EngineStatus::Timeout, None, start),
            Err(_) => return failed(EngineStatus::Error, Some("GitHub request failed"), start),
        };
        match response.status {
            403 if String::from_utf8_lossy(&response.body)
                .to_lowercase()
                .contains("rate limit") =>
            {
                return failed(EngineStatus::RateLimited, None, start);
            }
            403 => return failed(EngineStatus::Blocked, None, start),
            401 => {
                return failed(
                    EngineStatus::Unavailable,
                    Some("GitHub authentication required for this request"),
                    start,
                );
            }
            422 => {
                return failed(
                    EngineStatus::Error,
                    Some("GitHub rejected the search query (validation failed)"),
                    start,
                );
            }
            200..300 => {}
            _ => {
                return failed(
                    EngineStatus::Error,
                    Some("GitHub returned an unsuccessful status"),
                    start,
                );
            }
        }
        match ssx_json::from_slice::<Value>(&response.body)
            .map_err(|_| ())
            .and_then(|v| parse(&v, &endpoint))
        {
            Ok(v) => success(v, start),
            Err(()) => failed(EngineStatus::Error, Some("invalid GitHub response"), start),
        }
    }
}
impl SearchEngine for GitHubAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.search_inner(request))
    }
}
fn parse(data: &Value, endpoint: &str) -> Parsed<Vec<SearchResult>> {
    let mut results = Vec::new();
    for (idx, item) in get_rows(data, "items")?.into_iter().enumerate() {
        let mut result = SearchResult {
            engine: "github".into(),
            position: (idx + 1) as i64,
            ..SearchResult::default()
        };
        if endpoint.contains("/search/code") {
            let empty = ssx_json::Map::new();
            let repo = match item.get("repository").filter(|v| truthy(v)) {
                Some(v) => v.as_object().ok_or(())?,
                None => &empty,
            };
            let repo_name = raw(repo.get("full_name"), "");
            let repo_truthy = repo.get("full_name").is_some_and(truthy);
            let path = raw(item.get("path"), "");
            result.url = if repo_truthy {
                format!("https://github.com/{repo_name}/blob/main/{path}")
            } else {
                text(item.get("html_url"), "")?
            };
            result.title = if repo_truthy {
                format!("{repo_name}: {path}")
            } else {
                text(item.get("path"), "")?
            };
            let content = match item.get("text_matches").filter(|v| truthy(v)) {
                Some(v) => or_text(
                    v.as_array()
                        .ok_or(())?
                        .first()
                        .ok_or(())?
                        .as_object()
                        .ok_or(())?
                        .get("fragment"),
                )?,
                None => String::new(),
            };
            result.content = if content.is_empty() {
                format!("Code file in {repo_name}")
            } else {
                truncate(&content, 300)
            };
            result.category = "code".into();
            result.payload = payload(&[
                ("repository", repo.get("full_name")),
                ("path", item.get("path")),
            ]);
        } else if endpoint.contains("/search/issues") {
            result.url = text(item.get("html_url"), "")?;
            result.title = text(item.get("title"), "")?;
            let state = raw(item.get("state"), "");
            let labels = match item.get("labels").filter(|v| truthy(v)) {
                None => String::new(),
                // Python iterates strings and mapping keys; none of those
                // entries is a dict, so its label filter discards all of them.
                Some(Value::String(_) | Value::Object(_)) => String::new(),
                Some(v) => v
                    .as_array()
                    .ok_or(())?
                    .iter()
                    .filter_map(Value::as_object)
                    .map(|v| text(Some(v.get("name").ok_or(())?), ""))
                    .collect::<Parsed<Vec<_>>>()?
                    .join(", "),
            };
            let body = or_text(item.get("body"))?;
            let clean = truncate(strip(&body), 300);
            let content = if labels.is_empty() {
                format!("[{state}] {clean}")
            } else {
                format!("[{state}] [{labels}] {clean}")
            };
            result.content = strip(&content).into();
            result.category = "issues".into();
            result.published_date = match item.get("created_at") {
                None => Some(String::new()),
                v => optional_text(v)?,
            };
        } else {
            result.url = text(item.get("html_url"), "")?;
            result.title = text(item.get("full_name").or_else(|| item.get("name")), "")?;
            let description = item
                .get("description")
                .filter(|v| truthy(v))
                .map(|v| raw(Some(v), ""))
                .unwrap_or_default();
            let language = or_text(item.get("language"))?;
            let topics = match item.get("topics").filter(|v| truthy(v)) {
                None => String::new(),
                Some(Value::String(v)) => v
                    .chars()
                    .map(|ch| ch.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                Some(Value::Object(v)) => v.keys().cloned().collect::<Vec<_>>().join(", "),
                Some(v) => v
                    .as_array()
                    .ok_or(())?
                    .iter()
                    .map(|v| text(Some(v), ""))
                    .collect::<Parsed<Vec<_>>>()?
                    .join(", "),
            };
            let mut details = vec![format!("★ {}", raw(item.get("stargazers_count"), "0"))];
            if !language.is_empty() {
                details.push(language);
            }
            if !topics.is_empty() {
                details.push(topics);
            }
            result.content = if description.is_empty() {
                details.join(" — ")
            } else {
                format!("{description} — {}", details.join(" | "))
            };
            result.score = number(item.get("stargazers_count"), 0.0)?;
            result.category = "repositories".into();
            result.payload = payload(&[("repository", item.get("full_name"))]);
        }
        results.push(result);
    }
    Ok(results)
}
