//! Oyez frontend search and explicit legacy REST route; source order is relevance.
use super::{clinicaltrials as c, source_json::Value};
use crate::http::AdapterHttp;
use ssx_application::{PortFuture, SearchAdmission, SearchContext, SearchEngine};
use ssx_domain::{AdapterResponse, SearchRequest};
use std::{sync::Arc, time::Instant};
pub const ENDPOINT: &str = "https://beta-search.oyez.org/elasticsearch_index_scotus_nodes/_search";
pub const FIELDS: [&str; 13] = [
    "field_docket_number^3",
    "field_additional_docket_numbers^3",
    "title^2",
    "field_court_term",
    "field_first_party",
    "field_second_party",
    "field_facts_of_the_case:value",
    "field_question:value",
    "field_conclusion:value",
    "field_biography:value",
    "field_citation:field_volume",
    "field_citation:field_page",
    "field_citation:field_year^0.2",
];
#[derive(Clone, Debug)]
pub struct OyezConfig {
    pub settings: c::BiomedicalConfig,
    pub search_url: Option<String>,
}
impl Default for OyezConfig {
    fn default() -> Self {
        Self {
            settings: c::BiomedicalConfig::defaults(""),
            search_url: None,
        }
    }
}
pub struct OyezAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: OyezConfig,
}
impl OyezAdapter {
    pub fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: OyezConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn run(&self, q: &SearchRequest, context: Option<Instant>) -> AdapterResponse {
        let start = Instant::now();
        let budget = c::Budget::new(c::ROW_BYTES).expect("valid A08 quota");
        c::finish_batch(self.execute(q, context, &budget).await, start).await
    }
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        budget: &'b c::Budget,
    ) -> c::Result<c::ProducedBatch<'b>> {
        c::admit_with_context(&self.admission, "oyez", context).await?;
        let deadline = c::network_deadline(&self.config.settings, false, context)?;
        let max = c::scalar_text(&self.config.settings.max_results);
        let search = self.config.search_url.as_deref().filter(|s| !s.is_empty());
        let mut req = if search.is_none()
            && !self
                .config
                .settings
                .base_url
                .trim_matches(ssx_json::python_whitespace)
                .is_empty()
        {
            c::request(
                &format!(
                    "{}/cases",
                    self.config.settings.base_url.trim_end_matches('/')
                ),
                &[
                    ("page", "1"),
                    ("per_page", &max),
                    ("order", "desc"),
                    ("sort", "decision_date"),
                ],
                false,
                false,
            )?
        } else {
            // HTTPX refuses nonfinite JSON numbers before transport dispatch.
            if let ssx_config::ConfigScalar::Float(v) = self.config.settings.max_results
                && !v.is_finite()
            {
                let value = if v.is_nan() {
                    "nan"
                } else if v.is_sign_positive() {
                    "inf"
                } else {
                    "-inf"
                };
                return Err(c::error(format!(
                    "Out of range float values are not JSON compliant: {value}"
                )));
            }
            let mut req = c::request(search.unwrap_or(ENDPOINT), &[], false, false)?;
            req.method = reqwest::Method::POST;
            let max = match &self.config.settings.max_results {
                ssx_config::ConfigScalar::Text(v) => serde_json::to_string(v).unwrap(),
                _ => max,
            };
            req.body=format!("{{\"size\":{max},\"query\":{{\"multi_match\":{{\"query\":{},\"type\":\"cross_fields\",\"fields\":{}}}}}}}",serde_json::to_string(&q.query).unwrap(),serde_json::to_string(&FIELDS).unwrap()).into_bytes();
            req.headers
                .insert("content-type", "application/json".parse().unwrap());
            req
        };
        req.headers.insert("accept", "*/*".parse().unwrap());
        let data = super::regulatory::fetch(&self.http, req, deadline, true, context).await?;
        let Some(data) = data else {
            return c::ProducedBatch::new(budget, 0, context).map_err(c::projection_error);
        };
        let mut records = c::OwnedVec::new(budget).map_err(c::owned_error)?;
        if let Some(hits) = data.get("hits").filter(|v| v.as_object().is_some()) {
            if let Some(hits) = hits.get("hits").and_then(Value::as_array) {
                for hit in hits {
                    c::row_checkpoint(context).await?;
                    if let Some(src) = hit.get("_source").filter(|v| v.as_object().is_some()) {
                        records
                            .try_push((src, hit.get("_score")))
                            .map_err(c::owned_error)?
                    }
                }
            }
        } else {
            let rows = if data.as_array().is_some() {
                Some(&data)
            } else {
                data.get("results").or_else(|| data.get("data"))
            };
            if let Some(rows) = rows.and_then(Value::as_array) {
                for row in rows {
                    c::row_checkpoint(context).await?;
                    if row.as_object().is_some() {
                        records.try_push((row, None)).map_err(c::owned_error)?
                    }
                }
            }
        }
        let mut batch =
            c::ProducedBatch::new(budget, records.len(), context).map_err(c::projection_error)?;
        for (case, source_score) in records.as_slice() {
            c::row_checkpoint(context).await?;
            let mut url = None;
            for field in ["search_api_url", "url", "href"] {
                let candidate = display(case, &[field])?;
                if let Some(candidate) = case_url(candidate)? {
                    url = Some(candidate);
                    break;
                }
            }
            let Some(url) = url else { continue };
            let name = display(case, &["name", "title"])?;
            let term = display(case, &["term", "field_court_term"])?;
            let date = display(case, &["decision_date"])?;
            let description = display(case, &["description", "field_case_description"])?;
            let content = if !description.is_empty() {
                c::owned_join(budget, &[description], "", None, context).await?
            } else if !term.is_empty() {
                c::owned_join(budget, &["Supreme Court term: ", term], "", None, context).await?
            } else {
                c::owned_join(budget, &[], "", None, context).await?
            };
            let content = if !date.is_empty() {
                c::owned_join(
                    budget,
                    &[
                        "Decided: ",
                        date,
                        if content.as_str().is_empty() {
                            ""
                        } else {
                            " | "
                        },
                        content.as_str(),
                    ],
                    "",
                    Some(500),
                    context,
                )
                .await?
            } else {
                c::owned_join(
                    budget,
                    &[if content.as_str().is_empty() {
                        "U.S. Supreme Court case"
                    } else {
                        content.as_str()
                    }],
                    "",
                    Some(500),
                    context,
                )
                .await?
            };
            let score = match source_score {
                None | Some(Value::Null) => 1.,
                Some(v) => super::regulatory::float(v).unwrap_or(1.),
            };
            batch
                .push(
                    c::Skeleton {
                        url: &url,
                        title: name,
                        content: content.as_str(),
                        engine: "oyez",
                        category: "general",
                        score,
                        position: batch.results().len() as i64 + 1,
                        tier: 1,
                        published_date: (!date.is_empty()).then_some(date),
                    },
                    None,
                )
                .await
                .map_err(c::projection_error)?;
            let limit = c::numeric(&self.config.settings.max_results, false)?;
            if batch.results().len() as f64 >= limit {
                break;
            }
        }
        Ok(batch)
    }
}
impl SearchEngine for OyezAdapter {
    fn search<'a>(&'a self, q: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, None))
    }
    fn search_with_context<'a>(
        &'a self,
        q: &'a SearchRequest,
        c: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, Some(c.deadline)))
    }
}
fn display<'a>(case: &'a Value, keys: &[&str]) -> c::Result<&'a str> {
    fn one(value: &Value) -> c::Result<Option<&str>> {
        let Some(text) = value.as_text() else {
            return Ok(None);
        };
        let text = text
            .as_str()
            .ok_or_else(|| c::error("Oyez text contains non-scalar Unicode"))?
            .trim_matches(ssx_json::python_whitespace);
        Ok((!text.is_empty()).then_some(text))
    }
    for key in keys {
        match case.get(key) {
            Some(Value::Array(values)) => {
                for value in values {
                    if let Some(text) = one(value)? {
                        return Ok(text);
                    }
                }
            }
            Some(value) => {
                if let Some(text) = one(value)? {
                    return Ok(text);
                }
            }
            None => {}
        }
    }
    Ok("")
}

fn unquote(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn case_url(raw: &str) -> c::Result<Option<String>> {
    let view = super::regulatory::url_view(raw);
    let raw = view.as_str();
    let parsed = || -> Option<String> {
        let path = if let Some((scheme, rest)) = raw.split_once("://") {
            if !scheme.eq_ignore_ascii_case("https") {
                return None;
            }
            let (authority, path) = rest.split_once('/')?;
            let host = authority.rsplit('@').next()?.split(':').next()?;
            if !["api.oyez.org", "www.oyez.org"]
                .iter()
                .any(|h| host.eq_ignore_ascii_case(h))
            {
                return None;
            }
            path
        } else {
            if !raw.starts_with('/') || (raw.starts_with("//") && !raw.starts_with("///")) {
                return None;
            }
            raw
        };
        let path = path.split(['?', '#']).next()?;
        let path = super::regulatory::path_without_params(path);
        let parts: Vec<_> = path
            .split('/')
            .filter(|p| !p.is_empty())
            .map(unquote)
            .collect();
        if parts.len() != 3 || parts[0] != "cases" || parts[1..].iter().any(|p| p.contains('/')) {
            return None;
        }
        Some(format!(
            "https://www.oyez.org/cases/{}/{}",
            parts[1], parts[2]
        ))
    };
    let remainder = raw
        .split_once(':')
        .filter(|(scheme, _)| {
            scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
        })
        .map_or(raw, |(_, rest)| rest);
    if let Some(rest) = remainder.strip_prefix("//") {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        if let Some(message) = ssx_domain::storage::python_url_netloc_error(authority) {
            return Err(c::error(message));
        }
    }
    Ok(parsed())
}
