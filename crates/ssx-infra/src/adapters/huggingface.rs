use super::community::*;
use crate::http::{AdapterHttp, Error as HttpError, Request};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, SearchRequest, SearchResult};
use ssx_json::Value;
use std::{sync::Arc, time::Instant};
use url::Url;
#[derive(Clone, Debug)]
pub struct HuggingFaceConfig(pub CommunitySettings);
impl Default for HuggingFaceConfig {
    fn default() -> Self {
        Self(CommunitySettings::defaults(
            "https://huggingface.co/api",
            5000.0,
            5,
        ))
    }
}
pub(super) struct HuggingFaceAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: HuggingFaceConfig,
}
impl HuggingFaceAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: HuggingFaceConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn search_inner(&self, request: &SearchRequest) -> AdapterResponse {
        let start = Instant::now();
        match self.admission.admit("huggingface").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => return failed(EngineStatus::RateLimited, None, start),
            AdmissionDecision::Unavailable => {
                return failed(
                    EngineStatus::Error,
                    Some("HuggingFace admission unavailable"),
                    start,
                );
            }
        }
        let cfg = &self.config.0;
        let Ok(timeout) = cfg.duration() else {
            return failed(
                EngineStatus::Error,
                Some("invalid HuggingFace settings"),
                start,
            );
        };
        let categories = request.categories.as_deref().unwrap_or_default();
        let mode = if categories.iter().any(|v| v == "huggingface:datasets") {
            "datasets"
        } else if categories.iter().any(|v| v == "huggingface:papers") {
            "papers"
        } else {
            "models"
        };
        let endpoint = format!("{}/{mode}", cfg.base_url);
        let outgoing = (|| {
            let mut url = Url::parse(&endpoint).map_err(|_| ())?;
            url.set_query(None);
            url.query_pairs_mut()
                .append_pair("search", &request.query)
                .append_pair("limit", &cfg.max_results.to_string());
            let mut outgoing = Request::get(url.as_str()).map_err(|_| ())?;
            outgoing.headers.insert(
                "user-agent",
                "SlopSearX/0.1.0 (meta search engine; agent-native)"
                    .parse()
                    .map_err(|_| ())?,
            );
            if let Some(key) = &cfg.api_key {
                outgoing.headers.insert(
                    "authorization",
                    format!("Bearer {}", key.expose()).parse().map_err(|_| ())?,
                );
            }
            Ok::<_, ()>(outgoing)
        })();
        let Ok(outgoing) = outgoing else {
            return failed(
                EngineStatus::Error,
                Some("invalid HuggingFace request"),
                start,
            );
        };
        let response = match send(&self.http, outgoing, timeout).await {
            Ok(v) => v,
            Err(HttpError::Deadline) => return failed(EngineStatus::Timeout, None, start),
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    Some("HuggingFace request failed"),
                    start,
                );
            }
        };
        if response.status == 429 {
            return failed(EngineStatus::RateLimited, None, start);
        }
        if !(200..300).contains(&response.status) {
            return failed(
                EngineStatus::Error,
                Some("HuggingFace returned an unsuccessful status"),
                start,
            );
        }
        match ssx_json::from_slice::<Value>(&response.body)
            .map_err(|_| ())
            .and_then(|v| parse(&v, &endpoint))
        {
            Ok(v) => success(v, start),
            Err(()) => failed(
                EngineStatus::Error,
                Some("invalid HuggingFace response"),
                start,
            ),
        }
    }
}
impl SearchEngine for HuggingFaceAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.search_inner(request))
    }
}
fn parse(data: &Value, endpoint: &str) -> Parsed<Vec<SearchResult>> {
    let mut results = Vec::new();
    for (idx, item) in rows(data)?.into_iter().enumerate() {
        let mut result = SearchResult {
            engine: "huggingface".into(),
            position: (idx + 1) as i64,
            ..SearchResult::default()
        };
        if endpoint.contains("/models") || endpoint.contains("/datasets") {
            let model = endpoint.contains("/models");
            let id = if model {
                item.get("modelId").or_else(|| item.get("id"))
            } else {
                item.get("id")
            };
            result.title = text(id, "")?;
            result.url = if id.is_some_and(truthy) {
                format!(
                    "https://huggingface.co/{}{}",
                    if model { "" } else { "datasets/" },
                    raw(id, "")
                )
            } else {
                String::new()
            };
            let mut content = Vec::new();
            let description = or_text(item.get("description"))?;
            if !description.is_empty() {
                content.push(truncate(&description, 200));
            }
            let mut tags = Vec::new();
            if model {
                let pipeline = or_text(item.get("pipeline_tag"))?;
                let library = or_text(item.get("library_name"))?;
                result.category = if pipeline.is_empty() {
                    "models".into()
                } else {
                    pipeline.clone()
                };
                if !pipeline.is_empty() {
                    tags.push(pipeline);
                }
                if !library.is_empty() {
                    tags.push(library);
                }
            } else {
                result.category = "datasets".into();
            }
            tags.push(format!("▼{}", raw(item.get("downloads"), "0")));
            tags.push(format!("♥{}", raw(item.get("likes"), "0")));
            content.push(tags.join(" | "));
            result.content = content.join(" — ");
            result.score = number(item.get("likes"), 0.0)?;
        } else {
            let id = raw(item.get("id"), "");
            let fallback = if item.get("id").is_some_and(truthy) {
                format!("https://huggingface.co/papers/{id}")
            } else {
                String::new()
            };
            result.url = text(item.get("url"), &fallback)?;
            result.title = match item.get("title") {
                Some(v) => text(Some(v), "")?,
                None => text(item.get("id"), "")?,
            };
            result.content = format!("▲ {}", raw(item.get("upvotes"), "0"));
            if let Some(authors) = item.get("authors").filter(|v| truthy(v)) {
                let author = authors
                    .as_array()
                    .ok_or(())?
                    .first()
                    .ok_or(())?
                    .as_object()
                    .ok_or(())?
                    .get("name");
                if author.is_some_and(truthy) {
                    result.content.push_str(&format!(" | {}", raw(author, "")));
                }
            }
            result.score = number(item.get("upvotes"), 0.0)?;
            result.published_date = optional_text(item.get("publishedAt").filter(|v| truthy(v)))?;
            result.category = "papers".into();
        }
        results.push(result);
    }
    Ok(results)
}
