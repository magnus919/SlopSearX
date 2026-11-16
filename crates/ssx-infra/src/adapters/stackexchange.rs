use super::community::*;
use crate::http::{AdapterHttp, Request};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use ssx_json::Value;
use std::{sync::Arc, time::Instant};
#[derive(Clone, Debug)]
pub struct StackExchangeConfig(pub CommunitySettings);
impl Default for StackExchangeConfig {
    fn default() -> Self {
        Self(CommunitySettings::defaults(
            "https://api.stackexchange.com/2.3",
            5000.0,
            10,
        ))
    }
}
pub(super) struct StackExchangeAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: StackExchangeConfig,
}
impl StackExchangeAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: StackExchangeConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn search_inner(&self, request: &SearchRequest) -> AdapterResponse {
        let start = Instant::now();
        match self.admission.admit("stackexchange").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => return failed(EngineStatus::RateLimited, None, start),
            AdmissionDecision::Unavailable => {
                return failed(
                    EngineStatus::Error,
                    Some("Stack Exchange admission unavailable"),
                    start,
                );
            }
        }
        let cfg = &self.config.0;
        let Ok(timeout) = cfg.duration() else {
            return failed(
                EngineStatus::Error,
                Some("invalid Stack Exchange settings"),
                start,
            );
        };
        let site = request
            .categories
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find_map(|v| match v.as_str() {
                "stackexchange:serverfault" => Some("serverfault"),
                "stackexchange:code" => Some("stackoverflow"),
                _ => None,
            })
            .unwrap_or("stackoverflow");
        let url = format!(
            "{}/search?order=desc&sort=relevance&intitle={}&site={site}&pagesize={}",
            cfg.base_url,
            quote(&request.query),
            cfg.max_results
        );
        let outgoing = (|| {
            let mut outgoing = Request::get(&url).map_err(|_| ())?;
            if let Some(key) = &cfg.api_key {
                let key = strip(key.expose());
                if !key.is_empty() {
                    outgoing
                        .headers
                        .insert("x-api-key", key.parse().map_err(|_| ())?);
                }
            }
            Ok::<_, ()>(outgoing)
        })();
        let Ok(outgoing) = outgoing else {
            return failed(
                EngineStatus::Error,
                Some("invalid Stack Exchange request"),
                start,
            );
        };
        let response = match send(&self.http, outgoing, timeout).await {
            Ok(v) => v,
            Err(_) => {
                return failed(
                    EngineStatus::Error,
                    Some("Stack Exchange request failed"),
                    start,
                );
            }
        };
        if response.status == 400 {
            return failed(
                EngineStatus::RateLimited,
                Some("Stack Exchange rate limited (no API key)"),
                start,
            );
        }
        if !(200..300).contains(&response.status) {
            return failed(
                EngineStatus::Error,
                Some("Stack Exchange returned an unsuccessful status"),
                start,
            );
        }
        // The Python parser runs outside its transport catch. The native port
        // deliberately classifies these failures rather than unwinding search.
        match ssx_json::from_slice::<Value>(&response.body)
            .map_err(|_| ())
            .and_then(|v| parse(&v, cfg.max_results))
        {
            Ok(v) => success(v, start),
            Err(()) => failed(
                EngineStatus::Error,
                Some("invalid Stack Exchange response"),
                start,
            ),
        }
    }
}
impl SearchEngine for StackExchangeAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.search_inner(request))
    }
}
fn quote(input: &str) -> String {
    let mut out = String::new();
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || b"_.-/~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(out, "%{byte:02X}").expect("String write");
        }
    }
    out
}
fn parse(data: &Value, limit: usize) -> Parsed<Vec<SearchResult>> {
    let object = data.as_object().ok_or(())?;
    // Slice before inspecting rows: malformed rows beyond the requested slice
    // are never read by the source adapter (including max_results=0).
    let items = match object.get("items") {
        None => &[][..],
        Some(Value::Array(v)) => v.as_slice(),
        Some(Value::String(v)) if v.is_empty() || limit == 0 => &[][..],
        _ => return Err(()),
    };
    let mut results = Vec::new();
    for item in items.iter().take(limit) {
        let item = item.as_object().ok_or(())?;
        let published_date = match item.get("creation_date") {
            None | Some(Value::Null) => None,
            Some(Value::Bool(v)) => {
                Some(ssx_application::iso_timestamp(f64::from(u8::from(*v))).map_err(|_| ())?)
            }
            Some(Value::Number(v)) => {
                Some(ssx_application::iso_timestamp(v.as_f64().ok_or(())?).map_err(|_| ())?)
            }
            _ => return Err(()),
        };
        results.push(SearchResult {
            url: text(item.get("link"), "")?,
            title: text(item.get("title"), "")?,
            content: truncate(&text(item.get("body_markdown"), "")?, 500),
            engine: "stackexchange".into(),
            score: FiniteF64::new(number(item.get("score"), 0.0)?.get() / 100.0).map_err(|_| ())?,
            published_date,
            ..SearchResult::default()
        });
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_string_slice_precedes_row_interpretation_without_accepting_unsliceable_values() {
        let string: Value = ssx_json::from_str(r#"{"items":"nonempty"}"#).unwrap();
        assert!(parse(&string, 0).unwrap().is_empty());
        assert!(parse(&string, 1).is_err());
        for text in [
            r#"{"items":{}}"#,
            r#"{"items":1}"#,
            r#"{"items":null}"#,
            r#"{"items":true}"#,
        ] {
            assert!(parse(&ssx_json::from_str::<Value>(text).unwrap(), 0).is_err());
        }
    }
}
