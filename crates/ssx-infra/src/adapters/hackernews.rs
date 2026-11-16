use super::community::*;
use crate::http::{AdapterHttp, Error as HttpError, Request};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, SearchRequest, SearchResult};
use ssx_json::Value;
use std::{sync::Arc, time::Instant};
use url::Url;
#[derive(Clone, Debug)]
pub struct HackerNewsConfig(pub CommunitySettings);
impl Default for HackerNewsConfig {
    fn default() -> Self {
        Self(CommunitySettings::defaults(
            "https://hn.algolia.com/api/v1/search",
            3000.0,
            5,
        ))
    }
}
pub(super) struct HackerNewsAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: HackerNewsConfig,
}
impl HackerNewsAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: HackerNewsConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn search_inner(&self, request: &SearchRequest) -> AdapterResponse {
        let start = Instant::now();
        match self.admission.admit("hackernews").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => return failed(EngineStatus::RateLimited, None, start),
            AdmissionDecision::Unavailable => {
                return failed(
                    EngineStatus::Error,
                    Some("Hacker News admission unavailable"),
                    start,
                );
            }
        }
        let cfg = &self.config.0;
        let Ok(timeout) = cfg.duration() else {
            return failed(
                EngineStatus::Error,
                Some("invalid Hacker News settings"),
                start,
            );
        };
        let outgoing = (|| {
            let mut url = Url::parse(&cfg.base_url).map_err(|_| ())?;
            url.set_query(None);
            url.query_pairs_mut()
                .append_pair("query", &request.query)
                .append_pair("hitsPerPage", &cfg.max_results.to_string())
                .append_pair("tags", "story");
            let mut outgoing = Request::get(url.as_str()).map_err(|_| ())?;
            outgoing.headers.insert(
                "user-agent",
                "SlopSearX/0.1.0 (meta search engine; agent-native)"
                    .parse()
                    .map_err(|_| ())?,
            );
            Ok::<_, ()>(outgoing)
        })();
        let Ok(outgoing) = outgoing else {
            return failed(
                EngineStatus::Error,
                Some("invalid Hacker News request"),
                start,
            );
        };
        let response = match send(&self.http, outgoing, timeout).await {
            Ok(v) => v,
            Err(HttpError::Deadline) => return failed(EngineStatus::Timeout, None, start),
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    Some("Hacker News request failed"),
                    start,
                );
            }
        };
        if !(200..300).contains(&response.status) {
            return failed(
                EngineStatus::Error,
                Some("Hacker News returned an unsuccessful status"),
                start,
            );
        }
        match ssx_json::from_slice::<Value>(&response.body)
            .map_err(|_| ())
            .and_then(|v| parse(&v))
        {
            Ok(v) => success(v, start),
            Err(()) => failed(
                EngineStatus::Error,
                Some("invalid Hacker News response"),
                start,
            ),
        }
    }
}
impl SearchEngine for HackerNewsAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.search_inner(request))
    }
}
fn parse(data: &Value) -> Parsed<Vec<SearchResult>> {
    let mut results = Vec::new();
    for (idx, item) in get_rows(data, "hits")?.into_iter().enumerate() {
        let url = match item.get("url").filter(|v| truthy(v)) {
            Some(v) => text(Some(v), "")?,
            None => format!(
                "https://news.ycombinator.com/item?id={}",
                raw(item.get("objectID"), "")
            ),
        };
        let mut content = format!(
            "▲ {}  by {}",
            raw(item.get("points"), "0"),
            raw(item.get("author"), "")
        );
        if let Some(comments) = item.get("num_comments").filter(|v| truthy(v)) {
            content.push_str(&format!("  |  {} comments", raw(Some(comments), "0")));
        }
        results.push(SearchResult {
            url,
            title: text(item.get("title"), "")?,
            content,
            engine: "hackernews".into(),
            position: (idx + 1) as i64,
            score: number(item.get("points"), 0.0)?,
            published_date: match item.get("created_at") {
                None => Some(String::new()),
                v => optional_text(v)?,
            },
            category: "discussion".into(),
            ..SearchResult::default()
        });
    }
    Ok(results)
}
