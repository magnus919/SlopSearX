//! Native Brave-primary/DuckDuckGo-fallback suggestions with optional Valkey cache.
use crate::{
    http::{AdapterHttp, Cancellation, Options, Request},
    search::Suggestions,
    valkey::{Read, Valkey},
};
use sha2::{Digest, Sha256};
use ssx_application::PortFuture;
use std::{num::NonZeroU64, sync::Arc, time::Duration};
use tokio::time::Instant;

pub struct SuggestionService {
    http: Arc<AdapterHttp>,
    cache: Option<Arc<Valkey>>,
    brave_key: String,
    brave_endpoint: String,
    ddg_endpoint: String,
    trust_env: bool,
}
impl SuggestionService {
    pub fn new(brave_key: String, cache: Option<Arc<Valkey>>) -> Self {
        let mut service = Self::with_endpoints(
            brave_key,
            cache,
            "https://api.search.brave.com/res/v1/web/suggest".into(),
            "https://ac.duckduckgo.com/ac/".into(),
        );
        service.trust_env = true;
        service
    }
    /// Explicit controlled endpoints; ordinary composition uses `new`.
    pub fn with_endpoints(
        brave_key: String,
        cache: Option<Arc<Valkey>>,
        brave_endpoint: String,
        ddg_endpoint: String,
    ) -> Self {
        Self {
            http: Arc::new(AdapterHttp::default()),
            cache,
            brave_key,
            brave_endpoint,
            ddg_endpoint,
            trust_env: false,
        }
    }
    async fn get(&self, query: &str, brave: bool) -> Vec<String> {
        let Ok(mut request) = Request::get(if brave {
            &self.brave_endpoint
        } else {
            &self.ddg_endpoint
        }) else {
            return vec![];
        };
        // HTTPX uses Python quote_plus: '*' is escaped, '~' stays literal.
        // url::query_pairs_mut uses a different form-encoding safe set.
        let encoded = format!(
            "q={}&{}",
            quote_plus(query),
            if brave { "count=5" } else { "type=list" }
        );
        request.url.set_query(Some(&encoded));
        if brave {
            let Ok(token) = self.brave_key.parse() else {
                return vec![];
            };
            request.headers.insert("X-Subscription-Token", token);
            request
                .headers
                .insert("Accept", "application/json".parse().unwrap());
            request
                .headers
                .insert("Accept-Encoding", "gzip".parse().unwrap());
        }
        let options = Options {
            trust_env: self.trust_env,
            ..Default::default()
        };
        let Ok(mut session) = self.http.session(
            options,
            Instant::now() + Duration::from_secs(3),
            Cancellation::default(),
        ) else {
            return vec![];
        };
        let Ok(response) = session.execute(request).await else {
            return vec![];
        };
        if response.status != 200 {
            return vec![];
        }
        let Ok(value) = ssx_json::from_slice::<ssx_json::Value>(&response.body) else {
            return vec![];
        };
        parse(&value, brave)
    }
}
fn quote_plus(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            byte => {
                use std::fmt::Write;
                write!(&mut out, "%{byte:02X}").expect("string write");
            }
        }
    }
    out
}
pub fn cache_key(query: &str) -> String {
    let normalized = query.to_lowercase();
    let digest = Sha256::digest(
        normalized
            .trim_matches(ssx_json::python_whitespace)
            .as_bytes(),
    );
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("suggest:{hex}")
}
fn text(value: &ssx_json::Value) -> String {
    ssx_domain::storage::python_value_string(value)
}
pub fn parse(value: &ssx_json::Value, brave: bool) -> Vec<String> {
    let values = if brave {
        value.get("results").unwrap_or(value).as_array()
    } else {
        value
            .as_array()
            .and_then(|a| a.get(1))
            .and_then(ssx_json::Value::as_array)
    };
    values.map_or_else(Vec::new, |values| {
        if brave
            && values
                .iter()
                .any(|v| v.get("q").is_some_and(|q| !q.is_string()))
        {
            return vec![];
        }
        values
            .iter()
            .filter(|v| {
                brave
                    || match v {
                        ssx_json::Value::Null => false,
                        ssx_json::Value::Bool(b) => *b,
                        ssx_json::Value::String(s) => !s.is_empty(),
                        ssx_json::Value::Array(a) => !a.is_empty(),
                        ssx_json::Value::Object(o) => !o.is_empty(),
                        ssx_json::Value::Number(n) => n.as_f64() != Some(0.0),
                    }
            })
            .map(|v| text(if brave { v.get("q").unwrap_or(v) } else { v }))
            .collect()
    })
}
impl Suggestions for SuggestionService {
    fn fetch<'a>(&'a self, query: &'a str) -> PortFuture<'a, Vec<String>> {
        Box::pin(async move {
            if query.trim_matches(ssx_json::python_whitespace).is_empty() {
                return vec![];
            }
            let key = cache_key(query);
            if let Some(cache) = &self.cache
                && let Read::Found(value) = cache.get_object(&key).await
                && let Some(values) = value.get("suggestions").and_then(ssx_json::Value::as_array)
                && values.iter().all(ssx_json::Value::is_string)
            {
                return values
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect();
            }
            let mut values = if self.brave_key.is_empty() {
                vec![]
            } else {
                self.get(query, true).await
            };
            if values.is_empty() {
                values = self.get(query, false).await;
            }
            if let Some(cache) = &self.cache
                && let Ok(bytes) = ssx_json::to_vec(&ssx_json::json!({"suggestions": values}))
            {
                let _ = cache
                    .put(&key, &bytes, NonZeroU64::new(1800).unwrap())
                    .await;
            }
            values
        })
    }
    fn shutdown(&self) -> PortFuture<'_, ()> {
        Box::pin(self.http.shutdown_and_wait())
    }
}
