//! Shared mechanics for A03; response interpretation remains engine-local.
use crate::http::{AdapterHttp, Cancellation, Error as HttpError, Options, Request};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64};
use ssx_json::{Map, Value};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) type Parsed<T> = Result<T, ()>;
/// Resolved operator settings. Invalid values are refused before dispatch.
#[derive(Clone, Debug)]
pub struct CommunitySettings {
    pub base_url: String,
    pub timeout_ms: f64,
    pub max_results: usize,
    pub api_key: Option<ssx_domain::oauth::Secret>,
}
impl CommunitySettings {
    /// Resolve the numeric boundary without reading files or environment.
    /// Integral float lexemes are deliberately not integer configuration.
    pub fn resolve(
        base_url: String,
        timeout_ms: f64,
        max_results: &Value,
        api_key: Option<ssx_domain::oauth::Secret>,
    ) -> Result<Self, ssx_domain::DomainError> {
        let invalid = || ssx_domain::DomainError("invalid community adapter settings");
        let Value::Number(max_results) = max_results else {
            return Err(invalid());
        };
        if max_results.is_f64() {
            return Err(invalid());
        }
        let max_results = max_results
            .as_str()
            .parse::<usize>()
            .map_err(|_| invalid())?;
        let settings = Self {
            base_url,
            timeout_ms,
            max_results,
            api_key,
        };
        settings.duration().map_err(|_| invalid())?;
        Ok(settings)
    }

    pub(super) fn defaults(base: &str, timeout_ms: f64, max_results: usize) -> Self {
        Self {
            base_url: base.into(),
            timeout_ms,
            max_results,
            api_key: None,
        }
    }
    pub(super) fn duration(&self) -> Parsed<Duration> {
        if self.timeout_ms < 0.0 || !self.timeout_ms.is_finite() {
            return Err(());
        }
        let duration = Duration::try_from_secs_f64(self.timeout_ms / 1000.0).map_err(|_| ())?;
        tokio::time::Instant::now()
            .checked_add(duration)
            .ok_or(())?;
        Ok(duration)
    }
}
pub(super) fn elapsed(start: Instant) -> FiniteF64 {
    FiniteF64::new(start.elapsed().as_secs_f64() * 1000.0).unwrap_or_default()
}
pub(super) fn failed(
    status: EngineStatus,
    message: Option<&str>,
    start: Instant,
) -> AdapterResponse {
    AdapterResponse {
        status,
        error_message: message.map(str::to_owned),
        latency_ms: elapsed(start),
        ..AdapterResponse::default()
    }
}
pub(super) fn success(results: Vec<ssx_domain::SearchResult>, start: Instant) -> AdapterResponse {
    AdapterResponse {
        results,
        status: EngineStatus::Ok,
        latency_ms: elapsed(start),
        ..AdapterResponse::default()
    }
}
pub(super) async fn send(
    http: &Arc<AdapterHttp>,
    request: Request,
    timeout: Duration,
) -> Result<crate::http::Response, HttpError> {
    if timeout.is_zero() {
        return Err(HttpError::Deadline);
    }
    let deadline = tokio::time::Instant::now()
        .checked_add(timeout)
        .ok_or(HttpError::Deadline)?;
    let mut session = http.session(Options::default(), deadline, Cancellation::default())?;
    session.execute(request).await
}
pub(super) fn strip(value: &str) -> &str {
    value.trim_matches(|ch: char| ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch))
}
pub(super) fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) if v.is_f64() => v.as_f64().is_some_and(|v| v != 0.0),
        Value::Number(v) => v.as_str().bytes().any(|b| b.is_ascii_digit() && b != b'0'),
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}
pub(super) fn raw(value: Option<&Value>, default: &str) -> String {
    value
        .map(ssx_domain::storage::python_value_string)
        .unwrap_or_else(|| default.into())
}
pub(super) fn text(value: Option<&Value>, default: &str) -> Parsed<String> {
    match value {
        None => Ok(default.into()),
        Some(Value::String(v)) => Ok(v.clone()),
        _ => Err(()),
    }
}
pub(super) fn or_text(value: Option<&Value>) -> Parsed<String> {
    match value {
        None => Ok(String::new()),
        Some(v) if !truthy(v) => Ok(String::new()),
        Some(Value::String(v)) => Ok(v.clone()),
        _ => Err(()),
    }
}
pub(super) fn optional_text(value: Option<&Value>) -> Parsed<Option<String>> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(v)) => Ok(Some(v.clone())),
        _ => Err(()),
    }
}
pub(super) fn number(value: Option<&Value>, default: f64) -> Parsed<FiniteF64> {
    let n = match value {
        None => default,
        Some(Value::Bool(v)) => f64::from(u8::from(*v)),
        Some(Value::Number(v)) => v.as_f64().ok_or(())?,
        Some(Value::String(v)) => ssx_domain::storage::normalize_python_number_text(v)
            .map_err(|_| ())?
            .parse::<f64>()
            .map_err(|_| ())?,
        _ => return Err(()),
    };
    FiniteF64::new(n).map_err(|_| ())
}
pub(super) fn rows(value: &Value) -> Parsed<Vec<&Map<String, Value>>> {
    match value {
        Value::Array(rows) => rows.iter().map(|v| v.as_object().ok_or(())).collect(),
        Value::Object(v) if v.is_empty() => Ok(Vec::new()),
        Value::String(v) if v.is_empty() => Ok(Vec::new()),
        _ => Err(()),
    }
}
pub(super) fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}
pub(super) fn get_rows<'a>(data: &'a Value, field: &str) -> Parsed<Vec<&'a Map<String, Value>>> {
    let data = data.as_object().ok_or(())?;
    match data.get(field) {
        None => Ok(Vec::new()),
        Some(value) => rows(value),
    }
}
pub(super) fn payload(fields: &[(&str, Option<&Value>)]) -> Option<Value> {
    let mut data = Map::new();
    for (field, value) in fields {
        if let Some(value) = value.filter(|v| truthy(v)) {
            data.insert((*field).into(), value.clone());
        }
    }
    let names = data.keys().cloned().map(Value::String).collect();
    let mut provenance = Map::new();
    provenance.insert("engine".into(), Value::String("github".into()));
    provenance.insert("adapter_fields".into(), Value::Array(names));
    provenance.insert("normalized_fields".into(), Value::Array(Vec::new()));
    provenance.insert("inferred_fields".into(), Value::Array(Vec::new()));
    let mut payload = Map::new();
    payload.insert("domain".into(), Value::String("code".into()));
    payload.insert("type".into(), Value::String("repository".into()));
    payload.insert(
        "schema_version".into(),
        Value::Number(ssx_json::Number::new("1").ok()?),
    );
    payload.insert("data".into(), Value::Object(data));
    payload.insert("provenance".into(), Value::Object(provenance));
    Some(Value::Object(payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractional_timeout_and_zero_results_remain_representable() {
        let mut config = CommunitySettings::defaults("https://fixture.invalid", 1.5, 0);
        assert_eq!(config.duration().unwrap(), Duration::from_micros(1500));
        assert_eq!(config.max_results, 0);
        config.timeout_ms = 0.0;
        assert_eq!(config.duration().unwrap(), Duration::ZERO);
        for invalid in [-1.0, f64::NAN, f64::INFINITY, 1e300] {
            config.timeout_ms = invalid;
            assert!(config.duration().is_err());
        }
    }
}
