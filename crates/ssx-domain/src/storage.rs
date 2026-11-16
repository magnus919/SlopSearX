//! Explicit codecs for the canonical cache/snapshot records.
//!
//! These are durable backend records, not public wire DTOs. Their field names,
//! null behavior, engine ordering, and legacy reads follow
//! `slopsearx.service.search_result_to_dict` and
//! `search_response_to_payload`.
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use ssx_json::{self, Value};

use crate::{
    EngineExclusion, EngineOutcome, FiniteF64, MediaInfo, RoutingTradeoff, ScopeDecision,
    SearchResponse, SearchResult,
};

pub const PAYLOAD_MAX_PERSIST_BYTES: usize = 16_384;
pub const WORK_GROUP_MAX_PERSIST_BYTES: usize = 128_000;

/// Call-scoped storage bounds; callers may inject config without making this
/// pure codec read environment or process-global state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageLimits {
    pub payload_max_bytes: usize,
    pub work_group_max_bytes: usize,
}
impl Default for StorageLimits {
    fn default() -> Self {
        Self {
            payload_max_bytes: PAYLOAD_MAX_PERSIST_BYTES,
            work_group_max_bytes: WORK_GROUP_MAX_PERSIST_BYTES,
        }
    }
}

/// Source-compatible media constructor: validates the closed type vocabulary
/// and strips sensitive query keys from URLs before a record is persisted.
/// Legacy media reads deliberately bypass this constructor and preserve their
/// historical URL bytes.
pub fn build_media(
    media_type: &str,
    url: Option<&str>,
    thumbnail: Option<&str>,
    source: Option<&str>,
    width: Option<i64>,
    height: Option<i64>,
    duration: Option<FiniteF64>,
) -> Option<MediaInfo> {
    if !matches!(media_type, "image" | "video") {
        return None;
    }
    Some(MediaInfo {
        media_type: media_type.into(),
        url: url
            .filter(|value| !value.is_empty())
            .map(sanitize_media_url),
        thumbnail: thumbnail
            .filter(|value| !value.is_empty())
            .map(sanitize_media_url),
        source: source
            .filter(|value| !value.is_empty())
            .map(sanitize_media_url),
        width,
        height,
        duration,
    })
}

fn valid_urllib_netloc(netloc: &str) -> bool {
    media_url::valid_netloc(netloc)
}
fn python_string_repr(text: &str) -> String {
    let mut out = String::new();
    write_python_string_repr(text, &mut out).expect("String writer cannot fail");
    out
}
/// CPython 3.14 URL netloc diagnostic using the qualified Unicode16 delimiter
/// table and validator. Existing media validation remains unchanged and does not
/// allocate diagnostics. Callers must first apply urllib's URL parsing view.
pub fn python_url_netloc_error(netloc: &str) -> Option<String> {
    if valid_urllib_netloc(netloc) {
        return None;
    }
    let invalid_ipv6 = || Some("Invalid IPv6 URL".to_owned());
    if netloc.contains('[') != netloc.contains(']') {
        return invalid_ipv6();
    }
    if netloc.contains('[') {
        let host_port = netloc.rsplit_once('@').map_or(netloc, |(_, host)| host);
        let host = if let Some((before, bracketed)) = host_port.split_once('[') {
            if !before.is_empty() {
                return invalid_ipv6();
            }
            let Some((host, port)) = bracketed.split_once(']') else {
                return invalid_ipv6();
            };
            if !port.is_empty() && !port.starts_with(':') {
                return invalid_ipv6();
            }
            host
        } else {
            host_port
                .split_once(':')
                .map_or(host_port, |(host, _)| host)
        };
        if host.starts_with(['v', 'V']) {
            if !host[1..].split_once('.').is_some_and(|(version, address)| {
                !version.is_empty()
                    && version.bytes().all(|b| b.is_ascii_hexdigit())
                    && !address.is_empty()
            }) {
                return Some("IPvFuture address is invalid".into());
            }
        } else {
            if host.parse::<std::net::Ipv4Addr>().is_ok() {
                return Some("An IPv4 address cannot be in brackets".into());
            }
            let address = match host.split_once('%') {
                Some((address, scope)) if !scope.is_empty() && !scope.contains('%') => address,
                Some(_) => "",
                None => host,
            };
            if address.parse::<std::net::Ipv6Addr>().is_err() {
                return Some(format!(
                    "{} does not appear to be an IPv4 or IPv6 address",
                    python_string_repr(host)
                ));
            }
        }
    }
    // urllib._checknetloc concatenates the original netloc verbatim here;
    // its ipaddress diagnostic above instead uses Python repr escaping.
    Some(format!(
        "netloc '{netloc}' contains invalid characters under NFKC normalization"
    ))
}

fn sanitize_media_url(original: &str) -> String {
    media_url::sanitize_legacy(original)
}

/// Exact persisted result envelope. Optional result fields are explicit nulls;
/// optional fields inside media are omitted when absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredSearchResult {
    pub url: String,
    pub title: String,
    pub content: String,
    pub engine: String,
    pub engines: Vec<String>,
    pub score: FiniteF64,
    pub position: i64,
    pub category: String,
    pub published_date: Option<String>,
    pub thumbnail: Option<String>,
    pub img_src: Option<String>,
    pub media: Option<StoredMedia>,
    pub tier: i64,
    pub payload: Option<Value>,
    pub work_group: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredMedia {
    pub media_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<FiniteF64>,
}

impl From<&MediaInfo> for StoredMedia {
    fn from(media: &MediaInfo) -> Self {
        Self {
            media_type: media.media_type.clone(),
            url: media.url.clone().filter(|value| !value.is_empty()),
            thumbnail: media.thumbnail.clone().filter(|value| !value.is_empty()),
            source: media.source.clone().filter(|value| !value.is_empty()),
            width: media.width,
            height: media.height,
            duration: media.duration.clone(),
        }
    }
}

impl From<StoredMedia> for MediaInfo {
    fn from(media: StoredMedia) -> Self {
        Self {
            media_type: media.media_type,
            url: media.url,
            thumbnail: media.thumbnail,
            source: media.source,
            width: media.width,
            height: media.height,
            duration: media.duration.clone(),
        }
    }
}

impl StoredSearchResult {
    pub fn from_domain(result: &SearchResult) -> Result<Self, StorageError> {
        Self::from_domain_with_limits(result, StorageLimits::default())
    }

    pub fn from_domain_with_limits(
        result: &SearchResult,
        limits: StorageLimits,
    ) -> Result<Self, StorageError> {
        Ok(Self {
            url: result.url.clone(),
            title: result.title.clone(),
            content: result.content.clone(),
            engine: result.engine.clone(),
            engines: result.engines.iter().cloned().collect(),
            score: result.score.clone(),
            position: result.position,
            category: result.category.clone(),
            published_date: result.published_date.clone(),
            thumbnail: result.thumbnail.clone(),
            img_src: result.img_src.clone(),
            media: result.media.as_ref().map(StoredMedia::from),
            tier: result.tier,
            payload: bounded_object(result.payload.as_ref(), limits.payload_max_bytes),
            work_group: bounded_object(result.work_group.as_ref(), limits.work_group_max_bytes),
        })
    }

    pub fn into_domain(self) -> SearchResult {
        SearchResult {
            url: self.url,
            title: self.title,
            content: self.content,
            engine: self.engine,
            engines: self.engines.into_iter().collect(),
            score: self.score,
            position: self.position,
            category: self.category,
            published_date: self.published_date,
            thumbnail: self.thumbnail,
            img_src: self.img_src,
            tier: self.tier,
            media: self.media.map(Into::into),
            payload: self.payload,
            work_group: self.work_group,
        }
    }
}

/// Legacy-aware result decoder. Python accepts missing/null fields, numeric
/// strings, and several historical `engines` representations.
pub fn decode_result(value: Value) -> Result<SearchResult, StorageError> {
    decode_result_ref(&value)
}
/// Same complete F05 result grammar/defaults/errors, borrowing the parsed row.
/// Owned payload/work_group projections remain owned; this avoids only the
/// redundant whole-row clone in a caller already owning the validated record.
pub fn decode_result_ref(value: &Value) -> Result<SearchResult, StorageError> {
    let object = value
        .as_object()
        .ok_or(StorageError::ExpectedObject("result"))?;
    let engines = legacy_engines(object.get("engines"));
    let score = parse_f64(object.get("score"), 0.0, "score")?;
    let position = parse_i64(object.get("position"), 0, "position")?;
    let tier = parse_i64(object.get("tier"), 1, "tier")?;
    let media = decode_media(object.get("media"));
    let payload = object.get("payload").filter(|v| v.is_object()).cloned();
    let work_group = object.get("work_group").filter(|v| v.is_object()).cloned();
    Ok(SearchResult {
        url: string_default(object.get("url"), ""),
        title: string_default(object.get("title"), ""),
        content: string_default(object.get("content"), ""),
        engine: string_default(object.get("engine"), ""),
        engines,
        score: FiniteF64::new(score).map_err(|_| StorageError::NonFinite("score"))?,
        position,
        category: nullable_string(object.get("category")).unwrap_or_else(|| "general".into()),
        published_date: nullable_string(object.get("published_date")),
        thumbnail: nullable_string(object.get("thumbnail")),
        img_src: nullable_string(object.get("img_src")),
        tier,
        media,
        payload,
        work_group,
    })
}

pub fn encode_result(result: &SearchResult) -> Result<Value, StorageError> {
    encode_result_with_limits(result, StorageLimits::default())
}

pub fn encode_result_with_limits(
    result: &SearchResult,
    limits: StorageLimits,
) -> Result<Value, StorageError> {
    ssx_json::to_value(StoredSearchResult::from_domain_with_limits(result, limits)?)
        .map_err(|_| StorageError::Serialization)
}

/// Complete, unsliced canonical response stored by the cache. `cached` is
/// always false in the written record, matching the Python writer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StoredSearchResponse {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub results: Vec<StoredSearchResult>,
    #[serde(default)]
    pub scope: StoredScope,
    #[serde(default)]
    pub engine_outcomes: Vec<StoredEngineOutcome>,
    #[serde(default)]
    pub suggestions: Vec<String>,
    #[serde(default)]
    pub answers: Vec<Value>,
    #[serde(default)]
    pub corrections: Vec<String>,
    #[serde(default)]
    pub infoboxes: Vec<Value>,
    #[serde(default)]
    pub query_id: String,
    #[serde(default)]
    pub cached: bool,
    #[serde(default)]
    pub response_time_ms: i64,
    #[serde(default)]
    pub partial: bool,
    #[serde(default)]
    pub deadline_exceeded: bool,
    #[serde(default)]
    pub all_unresponsive: bool,
    #[serde(default)]
    pub empty_engines: Vec<Vec<String>>,
    #[serde(default)]
    pub cached_error: bool,
    #[serde(default = "default_ranking_explanation")]
    pub ranking_explanation: String,
    #[serde(default)]
    pub dispatched_engine_count: i64,
    /// Optional additive extension: legacy unbounded records retain their bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_filter: Option<crate::PublicationDateProof>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StoredScope {
    #[serde(default)]
    pub selected_engines: Vec<String>,
    #[serde(default)]
    pub resolved_categories: Vec<String>,
    #[serde(default)]
    pub routing_rule: String,
    #[serde(default)]
    pub matched_topic: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub excluded_engines: Vec<EngineExclusion>,
    #[serde(default)]
    pub routing_fallback: bool,
    #[serde(default)]
    pub routing_budget_applied: bool,
    #[serde(default)]
    pub routing_tradeoffs: Vec<RoutingTradeoff>,
    #[serde(default)]
    pub jev_added_engines: Vec<String>,
    #[serde(default)]
    pub jev_scores: std::collections::BTreeMap<String, FiniteF64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StoredEngineOutcome {
    #[serde(default)]
    pub engine: String,
    #[serde(default = "default_error_status")]
    pub status: String,
    #[serde(default)]
    pub result_count: i64,
    #[serde(default)]
    pub latency_ms: Option<FiniteF64>,
    #[serde(default)]
    pub message: Option<String>,
}

impl StoredSearchResponse {
    pub fn from_domain(response: &SearchResponse) -> Result<Self, StorageError> {
        Self::from_domain_with_limits(response, StorageLimits::default())
    }

    pub fn from_domain_with_limits(
        response: &SearchResponse,
        limits: StorageLimits,
    ) -> Result<Self, StorageError> {
        Ok(Self {
            query: response.query.clone(),
            results: response
                .results
                .iter()
                .map(|result| StoredSearchResult::from_domain_with_limits(result, limits))
                .collect::<Result<_, _>>()?,
            scope: StoredScope::from(&response.scope),
            engine_outcomes: response
                .engine_outcomes
                .iter()
                .map(StoredEngineOutcome::from)
                .collect(),
            suggestions: response.suggestions.clone(),
            answers: response.answers.clone(),
            corrections: response.corrections.clone(),
            infoboxes: response.infoboxes.clone(),
            query_id: response.query_id.clone(),
            cached: false,
            response_time_ms: response.response_time_ms,
            partial: response.partial,
            deadline_exceeded: response.deadline_exceeded,
            all_unresponsive: response.all_unresponsive,
            empty_engines: response.empty_engines.clone(),
            cached_error: response.cached_error,
            ranking_explanation: response.ranking_explanation.clone(),
            dispatched_engine_count: response.dispatched_engine_count,
            date_filter: response.date_filter.clone(),
        })
    }

    pub fn into_domain(self) -> SearchResponse {
        SearchResponse {
            query: self.query,
            results: self
                .results
                .into_iter()
                .map(StoredSearchResult::into_domain)
                .collect(),
            scope: self.scope.into(),
            engine_outcomes: self.engine_outcomes.into_iter().map(Into::into).collect(),
            suggestions: self.suggestions,
            answers: self.answers,
            corrections: self.corrections,
            infoboxes: self.infoboxes,
            query_id: self.query_id,
            cached: self.cached,
            response_time_ms: self.response_time_ms,
            partial: self.partial,
            deadline_exceeded: self.deadline_exceeded,
            all_unresponsive: self.all_unresponsive,
            empty_engines: self.empty_engines,
            cached_error: self.cached_error,
            ranking_explanation: self.ranking_explanation,
            dispatched_engine_count: self.dispatched_engine_count,
            date_filter: self.date_filter,
        }
    }
}

impl From<&ScopeDecision> for StoredScope {
    fn from(scope: &ScopeDecision) -> Self {
        Self {
            selected_engines: scope.selected_engines.clone(),
            resolved_categories: scope.resolved_categories.clone(),
            routing_rule: scope.routing_rule.clone(),
            matched_topic: scope.matched_topic.clone(),
            warnings: scope.warnings.clone(),
            excluded_engines: scope.excluded_engines.clone(),
            routing_fallback: scope.routing_fallback,
            routing_budget_applied: scope.routing_budget_applied,
            routing_tradeoffs: scope.routing_tradeoffs.clone(),
            jev_added_engines: scope.jev_added_engines.clone(),
            jev_scores: scope.jev_scores.clone(),
        }
    }
}
impl From<StoredScope> for ScopeDecision {
    fn from(scope: StoredScope) -> Self {
        Self {
            selected_engines: scope.selected_engines,
            resolved_categories: scope.resolved_categories,
            routing_rule: scope.routing_rule,
            matched_topic: scope.matched_topic,
            warnings: scope.warnings,
            excluded_engines: scope.excluded_engines,
            routing_fallback: scope.routing_fallback,
            routing_budget_applied: scope.routing_budget_applied,
            routing_tradeoffs: scope.routing_tradeoffs,
            jev_added_engines: scope.jev_added_engines,
            jev_scores: scope.jev_scores,
        }
    }
}
impl From<&EngineOutcome> for StoredEngineOutcome {
    fn from(value: &EngineOutcome) -> Self {
        Self {
            engine: value.engine.clone(),
            status: value.status.clone(),
            result_count: value.result_count,
            latency_ms: value.latency_ms.clone(),
            message: value.message.clone(),
        }
    }
}
impl From<StoredEngineOutcome> for EngineOutcome {
    fn from(value: StoredEngineOutcome) -> Self {
        Self {
            engine: value.engine,
            status: value.status,
            result_count: value.result_count,
            latency_ms: value.latency_ms.clone(),
            message: value.message,
        }
    }
}

pub fn encode_response(response: &SearchResponse) -> Result<Value, StorageError> {
    encode_response_with_limits(response, StorageLimits::default())
}

pub fn encode_response_with_limits(
    response: &SearchResponse,
    limits: StorageLimits,
) -> Result<Value, StorageError> {
    ssx_json::to_value(StoredSearchResponse::from_domain_with_limits(
        response, limits,
    )?)
    .map_err(|_| StorageError::Serialization)
}

/// Decode a complete or legacy partial response. Python defaults missing
/// fields and accepts numeric strings for response timing and result values.
pub fn decode_response(value: Value) -> Result<SearchResponse, StorageError> {
    decode_response_with_limits(value, StorageLimits::default())
}

pub fn decode_response_with_limits(
    value: Value,
    _limits: StorageLimits,
) -> Result<SearchResponse, StorageError> {
    let object = value
        .as_object()
        .ok_or(StorageError::ExpectedObject("response"))?;
    let scope = object.get("scope").and_then(Value::as_object);
    let results = array_field(object.get("results"))
        .iter()
        .cloned()
        .map(decode_result)
        .collect::<Result<Vec<_>, _>>()?;
    let engine_outcomes = array_field(object.get("engine_outcomes"))
        .iter()
        .map(decode_engine_outcome)
        .collect::<Result<Vec<_>, _>>()?;
    let scope = decode_scope(scope)?;
    Ok(SearchResponse {
        query: source_string(object.get("query"), ""),
        results,
        scope,
        engine_outcomes,
        suggestions: string_array(object.get("suggestions")),
        answers: value_array(object.get("answers")),
        corrections: string_array(object.get("corrections")),
        infoboxes: value_array(object.get("infoboxes")),
        query_id: source_string(object.get("query_id"), ""),
        cached: bool_field(object.get("cached"), false),
        response_time_ms: parse_i64(object.get("response_time_ms"), 0, "response_time_ms")?,
        partial: bool_field(object.get("partial"), false),
        deadline_exceeded: bool_field(object.get("deadline_exceeded"), false),
        all_unresponsive: bool_field(object.get("all_unresponsive"), false),
        empty_engines: array_field(object.get("empty_engines"))
            .iter()
            .map(|entry| string_values(entry.as_array().map(Vec::as_slice).unwrap_or_default()))
            .collect(),
        cached_error: bool_field(object.get("cached_error"), false),
        ranking_explanation: source_string(
            object.get("ranking_explanation"),
            "tier_then_cross_engine_presence",
        ),
        dispatched_engine_count: parse_i64(
            object.get("dispatched_engine_count"),
            0,
            "dispatched_engine_count",
        )?,
        date_filter: object
            .get("date_filter")
            .filter(|v| !v.is_null())
            .map(|v| {
                crate::PublicationDateProof::from_value(v.clone())
                    .map_err(|_| StorageError::InvalidResponse)
            })
            .transpose()?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    ExpectedObject(&'static str),
    InvalidNumber(&'static str),
    NonFinite(&'static str),
    InvalidResponse,
    Serialization,
}
impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for StorageError {}

fn bounded_object(value: Option<&Value>, limit: usize) -> Option<Value> {
    let value = value.filter(|value| value.is_object())?;
    let bytes = ssx_json::canonical_json(value, ssx_json::JsonProfile::Utf8Compact).ok()?;
    (bytes.len() <= limit).then(|| value.clone())
}
fn decode_scope(
    scope: Option<&ssx_json::Map<String, Value>>,
) -> Result<ScopeDecision, StorageError> {
    let Some(scope) = scope else {
        return Ok(ScopeDecision::default());
    };
    let exclusions = array_field(scope.get("excluded_engines"))
        .iter()
        .map(|item| {
            let item = item.as_object();
            EngineExclusion {
                engine: source_string(item.and_then(|v| v.get("engine")), ""),
                reason: source_string(item.and_then(|v| v.get("reason")), ""),
                stage: source_string(item.and_then(|v| v.get("stage")), "unknown"),
            }
        })
        .collect();
    let tradeoffs = array_field(scope.get("routing_tradeoffs"))
        .iter()
        .map(|item| {
            let item = item.as_object();
            RoutingTradeoff {
                kind: source_string(item.and_then(|v| v.get("kind")), ""),
                detail: source_string(item.and_then(|v| v.get("detail")), ""),
            }
        })
        .collect();
    let mut jev_scores = std::collections::BTreeMap::new();
    if let Some(scores) = scope.get("jev_scores").and_then(Value::as_object) {
        for (name, score) in scores {
            let number = parse_value_f64(score)?;
            let score =
                FiniteF64::new(number).map_err(|_| StorageError::NonFinite("jev_scores"))?;
            jev_scores.insert(name.clone(), score);
        }
    }
    Ok(ScopeDecision {
        selected_engines: string_array(scope.get("selected_engines")),
        resolved_categories: string_array(scope.get("resolved_categories")),
        routing_rule: truthy_string(scope.get("routing_rule")),
        matched_topic: nullable_string(scope.get("matched_topic")),
        warnings: string_array(scope.get("warnings")),
        excluded_engines: exclusions,
        routing_fallback: bool_field(scope.get("routing_fallback"), false),
        routing_budget_applied: bool_field(scope.get("routing_budget_applied"), false),
        routing_tradeoffs: tradeoffs,
        jev_added_engines: string_array(scope.get("jev_added_engines")),
        jev_scores,
    })
}
fn decode_engine_outcome(value: &Value) -> Result<EngineOutcome, StorageError> {
    let object = value
        .as_object()
        .ok_or(StorageError::ExpectedObject("engine outcome"))?;
    let latency_ms = match object.get("latency_ms").filter(|value| !value.is_null()) {
        Some(value) => {
            let number = parse_value_f64(value)?;
            Some(FiniteF64::new(number).map_err(|_| StorageError::NonFinite("latency_ms"))?)
        }
        None => None,
    };
    Ok(EngineOutcome {
        engine: source_string(object.get("engine"), ""),
        status: source_string(object.get("status"), "error"),
        result_count: parse_i64(object.get("result_count"), 0, "result_count")?,
        latency_ms,
        message: object
            .get("message")
            .filter(|value| !value.is_null())
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}
fn array_field(value: Option<&Value>) -> &[Value] {
    value
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}
fn value_array(value: Option<&Value>) -> Vec<Value> {
    array_field(value).to_vec()
}
fn string_array(value: Option<&Value>) -> Vec<String> {
    string_values(array_field(value))
}
fn string_values(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .map(|value| source_string(Some(value), ""))
        .collect()
}
fn source_string(value: Option<&Value>, default: &str) -> String {
    value
        .map(value_to_string)
        .unwrap_or_else(|| default.to_owned())
}
fn truthy_string(value: Option<&Value>) -> String {
    match value {
        Some(value) if python_truthy(value) => value_to_string(value),
        _ => String::new(),
    }
}
fn python_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => {
            if value.is_f64() {
                value.as_f64().is_some_and(|number| number != 0.0)
            } else {
                value.as_str() != "0"
            }
        }
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}
fn decode_media(value: Option<&Value>) -> Option<MediaInfo> {
    let object = value?.as_object()?;
    let kind = object.get("media_type")?.as_str()?;
    if kind != "image" && kind != "video" {
        return None;
    }
    Some(MediaInfo {
        media_type: kind.into(),
        // The Python legacy reader deliberately does not sanitize here.
        url: nullable_string(object.get("url")),
        thumbnail: nullable_string(object.get("thumbnail")),
        source: nullable_string(object.get("source")),
        width: parse_optional_i64(object.get("width")),
        height: parse_optional_i64(object.get("height")),
        duration: object
            .get("duration")
            .and_then(|v| parse_value_f64(v).ok())
            .and_then(|n| FiniteF64::new(n).ok()),
    })
}
fn legacy_engines(value: Option<&Value>) -> BTreeSet<String> {
    match value {
        Some(Value::String(value)) => {
            let text = value.trim();
            if text.starts_with('{') && text.ends_with('}') {
                text[1..text.len() - 1]
                    .split(',')
                    .filter_map(|part| {
                        let item = part.trim();
                        (!item.is_empty()).then(|| item.trim_matches(['\'', '"']).to_owned())
                    })
                    .collect()
            } else {
                text.split_whitespace().map(str::to_owned).collect()
            }
        }
        Some(Value::Array(items)) => items.iter().map(value_to_string).collect(),
        _ => BTreeSet::new(),
    }
}
fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => python_repr(value),
    }
}

/// Format a lossless JSON value using the Python `str(value)` behavior used by
/// legacy adapter payloads, including Python container and scalar spellings.
pub fn python_value_string(value: &Value) -> String {
    value_to_string(value)
}

fn python_repr(value: &Value) -> String {
    let mut out = String::new();
    write_python_repr(value, &mut out).expect("String writer cannot fail");
    out
}

/// Write the legacy Python `str` representation without allocating intermediate
/// child strings. The caller owns sink capacity, accounting and execution policy.
pub fn write_python_value(value: &Value, out: &mut impl std::fmt::Write) -> std::fmt::Result {
    match value {
        Value::String(text) => out.write_str(text),
        _ => write_python_repr(value, out),
    }
}

fn write_python_repr(value: &Value, out: &mut impl std::fmt::Write) -> std::fmt::Result {
    match value {
        Value::Null => out.write_str("None"),
        Value::Bool(value) => out.write_str(if *value { "True" } else { "False" }),
        Value::Number(number) => {
            if number.is_f64() {
                let text = ssx_json::python_float(number.as_f64().expect("finite JSON invariant"))
                    .expect("finite JSON invariant");
                out.write_str(&text)
            } else {
                out.write_str(number.as_str())
            }
        }
        Value::String(text) => write_python_string_repr(text, out),
        Value::Array(items) => {
            out.write_char('[')?;
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    out.write_str(", ")?;
                }
                write_python_repr(item, out)?;
            }
            out.write_char(']')
        }
        Value::Object(items) => {
            out.write_char('{')?;
            for (index, (key, value)) in items.iter().enumerate() {
                if index != 0 {
                    out.write_str(", ")?;
                }
                write_python_string_repr(key, out)?;
                out.write_str(": ")?;
                write_python_repr(value, out)?;
            }
            out.write_char('}')
        }
    }
}

/// Checked count using exactly the same traversal as the writer. This is not a
/// deadline-aware traversal; supervised callers retain process cancellation.
pub fn python_value_string_len(value: &Value) -> Option<usize> {
    struct Count(usize);
    impl std::fmt::Write for Count {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            self.0 = self.0.checked_add(text.len()).ok_or(std::fmt::Error)?;
            Ok(())
        }
    }
    let mut count = Count(0);
    write_python_value(value, &mut count).ok()?;
    Some(count.0)
}
#[derive(Deserialize)]
struct PythonTextTables {
    decimal_zeroes: Vec<u32>,
    nonprintable_ranges: Vec<(u32, u32)>,
}
fn python_text_tables() -> &'static PythonTextTables {
    static TABLE: std::sync::OnceLock<PythonTextTables> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../contracts/unicode/python-text-16.0.0.json"
        ))
        .expect("reviewed Python Unicode16 text tables")
    })
}
/// One accepted Unicode16 Python repr character, held entirely on the stack.
/// Cold immutable Unicode-table initialization remains shared-process overhead.
#[derive(Clone, Copy)]
pub enum PythonReprQuote {
    Single,
    Double,
}
pub struct PythonReprCharacter {
    bytes: [u8; 10],
    length: usize,
}
impl PythonReprCharacter {
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.length]).expect("repr character is UTF8/ASCII")
    }
}
/// Escape one scalar under the caller's chosen single/double quote. This is the
/// same scalar rule used by the legacy Python formatter; no new Unicode table.
pub fn python_repr_character(ch: char, quote: PythonReprQuote) -> PythonReprCharacter {
    let quote = match quote {
        PythonReprQuote::Single => '\'',
        PythonReprQuote::Double => '"',
    };
    let mut bytes = [0; 10];
    let length;
    match ch {
        '\\' => {
            bytes[..2].copy_from_slice(b"\\\\");
            length = 2;
        }
        '\n' => {
            bytes[..2].copy_from_slice(b"\\n");
            length = 2;
        }
        '\r' => {
            bytes[..2].copy_from_slice(b"\\r");
            length = 2;
        }
        '\t' => {
            bytes[..2].copy_from_slice(b"\\t");
            length = 2;
        }
        ch if ch == quote => {
            bytes[0] = b'\\';
            bytes[1] = ch as u8;
            length = 2;
        }
        ch => {
            let code = ch as u32;
            let nonprintable = python_text_tables()
                .nonprintable_ranges
                .binary_search_by(|&(start, end)| {
                    if code < start {
                        std::cmp::Ordering::Greater
                    } else if code > end {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
                .is_ok();
            if nonprintable {
                let (width, prefix) = if code <= 255 {
                    (2, b'x')
                } else if code <= 65535 {
                    (4, b'u')
                } else {
                    (8, b'U')
                };
                bytes[0] = b'\\';
                bytes[1] = prefix;
                const HEX: &[u8; 16] = b"0123456789abcdef";
                for i in 0..width {
                    bytes[2 + i] = HEX[((code >> ((width - i - 1) * 4)) & 15) as usize];
                }
                length = width + 2;
            } else {
                let mut encoded = [0; 4];
                let text = ch.encode_utf8(&mut encoded);
                bytes[..text.len()].copy_from_slice(text.as_bytes());
                length = text.len();
            }
        }
    }
    PythonReprCharacter { bytes, length }
}
fn write_python_string_repr(text: &str, out: &mut impl std::fmt::Write) -> std::fmt::Result {
    let quote = if text.contains('\'') && !text.contains('"') {
        PythonReprQuote::Double
    } else {
        PythonReprQuote::Single
    };
    let delimiter = match quote {
        PythonReprQuote::Single => '\'',
        PythonReprQuote::Double => '"',
    };
    out.write_char(delimiter)?;
    for ch in text.chars() {
        out.write_str(python_repr_character(ch, quote).as_str())?;
    }
    out.write_char(delimiter)
}
/// Normalize Python numeric whitespace, Unicode decimal digits and valid digit
/// separators. Callers still validate numeric grammar, range and finite bounds.
pub fn normalize_python_number_text(text: &str) -> Result<String, StorageError> {
    let trimmed = text.trim_matches(|ch: char| {
        if ch.is_ascii() {
            ch.is_ascii_whitespace() || ch == '\u{b}'
        } else {
            ch.is_whitespace()
        }
    });
    let zeroes = &python_text_tables().decimal_zeroes;
    let mut ascii = String::new();
    for ch in trimmed.chars() {
        let code = ch as u32;
        let index = zeroes.partition_point(|zero| *zero <= code);
        if index > 0 && code - zeroes[index - 1] < 10 {
            ascii.push(char::from(b'0' + (code - zeroes[index - 1]) as u8));
        } else {
            ascii.push(ch);
        }
    }
    let bytes = ascii.as_bytes();
    for (index, ch) in bytes.iter().enumerate() {
        if *ch == b'_'
            && (index == 0
                || index + 1 == bytes.len()
                || !bytes[index - 1].is_ascii_digit()
                || !bytes[index + 1].is_ascii_digit())
        {
            return Err(StorageError::InvalidNumber("number"));
        }
    }
    Ok(ascii.replace('_', ""))
}
fn nullable_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}
fn string_default(value: Option<&Value>, default: &str) -> String {
    value.map(value_to_string).unwrap_or_else(|| default.into())
}
fn parse_value_f64(value: &Value) -> Result<f64, StorageError> {
    match value {
        Value::Number(number) => number.as_f64().ok_or(StorageError::InvalidNumber("number")),
        Value::Bool(value) => Ok(if *value { 1.0 } else { 0.0 }),
        Value::String(text) => normalize_python_number_text(text)?
            .parse()
            .map_err(|_| StorageError::InvalidNumber("number")),
        _ => Err(StorageError::InvalidNumber("number")),
    }
}
fn parse_f64(
    value: Option<&Value>,
    default: f64,
    field: &'static str,
) -> Result<f64, StorageError> {
    value
        .filter(|v| !v.is_null())
        .map(parse_value_f64)
        .unwrap_or(Ok(default))
        .map_err(|_| StorageError::InvalidNumber(field))
}
fn parse_i64(
    value: Option<&Value>,
    default: i64,
    field: &'static str,
) -> Result<i64, StorageError> {
    let Some(value) = value.filter(|v| !v.is_null()) else {
        return Ok(default);
    };
    match value {
        Value::Bool(value) => Ok(i64::from(*value)),
        Value::Number(number) => {
            if !number.is_f64() {
                return number.as_i64().ok_or(StorageError::InvalidNumber(field));
            }
            let number = number
                .as_f64()
                .ok_or(StorageError::InvalidNumber(field))?
                .trunc();
            if !(-9223372036854775808.0..9223372036854775808.0).contains(&number) {
                return Err(StorageError::InvalidNumber(field));
            }
            Ok(number as i64)
        }
        Value::String(text) => normalize_python_number_text(text)?
            .parse()
            .map_err(|_| StorageError::InvalidNumber(field)),
        _ => Err(StorageError::InvalidNumber(field)),
    }
}
fn parse_optional_i64(value: Option<&Value>) -> Option<i64> {
    value
        .filter(|v| !v.is_null())
        .and_then(|v| parse_i64(Some(v), 0, "media dimension").ok())
}
fn bool_field(value: Option<&Value>, default: bool) -> bool {
    value.map(python_truthy).unwrap_or(default)
}
fn default_error_status() -> String {
    "error".into()
}
fn default_ranking_explanation() -> String {
    "tier_then_cross_engine_presence".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_sorted_engines_explicit_nulls_and_private_work_group() {
        let result = SearchResult {
            engines: ["zeta".into(), "alpha".into()].into(),
            work_group: Some(ssx_json::json!({"private":true})),
            ..Default::default()
        };
        let value = encode_result(&result).unwrap();
        assert_eq!(value["engines"], ssx_json::json!(["alpha", "zeta"]));
        assert!(value["published_date"].is_null());
        assert!(value["work_group"]["private"].as_bool().unwrap());
    }

    #[test]
    fn reads_legacy_engine_forms_and_numeric_strings() {
        for (engines, expected) in [
            (
                ssx_json::json!("{'arxiv', 'github'}"),
                vec!["arxiv", "github"],
            ),
            (ssx_json::json!("arxiv github"), vec!["arxiv", "github"]),
            (
                ssx_json::json!(["github", "arxiv"]),
                vec!["arxiv", "github"],
            ),
        ] {
            let value =
                ssx_json::json!({"engines":engines,"score":"1.25","position":"-2","tier":"7"});
            let result = decode_result(value).unwrap();
            assert_eq!(result.engines.into_iter().collect::<Vec<_>>(), expected);
            assert_eq!(result.score.get(), 1.25);
            assert_eq!(result.position, -2);
            assert_eq!(result.tier, 7);
        }
    }

    #[test]
    fn media_reader_preserves_legacy_urls_while_writer_omits_absent_values() {
        let value = ssx_json::json!({"media":{"media_type":"image","url":"https://x.test/a?token=secret","width":0,"height":null}});
        let result = decode_result(value).unwrap();
        let media = result.media.unwrap();
        assert_eq!(media.url.as_deref(), Some("https://x.test/a?token=secret"));
        assert_eq!(media.width, Some(0));
        let out = ssx_json::to_value(StoredMedia::from(&media)).unwrap();
        assert_eq!(out["url"], "https://x.test/a?token=secret");
        assert_eq!(out["width"], 0);
        assert!(out.get("height").is_none());
    }

    #[test]
    fn payload_bounds_omit_oversized_values_and_nonfinite_scores_fail() {
        let result = SearchResult {
            payload: Some(ssx_json::json!({"data":"x".repeat(PAYLOAD_MAX_PERSIST_BYTES)})),
            ..Default::default()
        };
        assert!(encode_result(&result).unwrap()["payload"].is_null());
        assert!(matches!(
            decode_result(ssx_json::json!({"score":"1e999"})),
            Err(StorageError::NonFinite("score"))
        ));
    }

    #[test]
    fn configured_persistence_limits_apply_without_process_global_state() {
        let result = SearchResult {
            payload: Some(ssx_json::json!({"x":1})),
            ..Default::default()
        };
        let zero = StorageLimits {
            payload_max_bytes: 0,
            ..StorageLimits::default()
        };
        assert!(encode_result_with_limits(&result, zero).unwrap()["payload"].is_null());
        let exact = StorageLimits {
            payload_max_bytes: 7,
            ..StorageLimits::default()
        };
        assert_eq!(
            encode_result_with_limits(&result, exact).unwrap()["payload"],
            ssx_json::json!({"x":1})
        );
        let response = SearchResponse {
            results: vec![result],
            ..Default::default()
        };
        assert!(
            encode_response_with_limits(&response, zero).unwrap()["results"][0]["payload"]
                .is_null()
        );
    }

    #[test]
    fn lossless_payload_with_reserved_tag_and_large_integer_survives_storage_builder() {
        let payload = ssx_json::from_str::<Value>(
            r#"{"$serde_json::private::Number":"123","nested":{"n":18446744073709551617}}"#,
        )
        .unwrap();
        let result = SearchResult {
            payload: Some(payload),
            ..Default::default()
        };
        let encoded = encode_result(&result).unwrap();
        assert_eq!(encoded["payload"]["$serde_json::private::Number"], "123");
        assert_eq!(
            encoded["payload"]["nested"]["n"].to_string(),
            "18446744073709551617"
        );
        let decoded = decode_result(encoded).unwrap();
        assert_eq!(
            decoded.payload.unwrap()["nested"]["n"].to_string(),
            "18446744073709551617"
        );
    }

    #[test]
    fn media_constructor_sanitizes_sensitive_query_parameters() {
        let media = build_media(
            "image",
            Some("https://img.example.test/a?token=secret&size=small"),
            Some("https://img.example.test/t?api_key=secret"),
            None,
            Some(0),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            media.url.as_deref(),
            Some("https://img.example.test/a?size=small")
        );
        assert_eq!(
            media.thumbnail.as_deref(),
            Some("https://img.example.test/t")
        );
        assert_eq!(media.width, Some(0));
        assert!(build_media("audio", None, None, None, None, None, None).is_none());
        assert_eq!(
            sanitize_media_url(
                "https://x.test/a?a=hello%20world&tag=x&tag=y&empty=&token=s&%61pi_key=s#frag"
            ),
            "https://x.test/a?a=hello+world&tag=x&tag=y&empty=#frag"
        );
        assert_eq!(
            sanitize_media_url("https://x.test/a?space=hello%20world&star=*&tilde=~"),
            "https://x.test/a?space=hello+world&star=%2A&tilde=~"
        );
        assert_eq!(
            sanitize_media_url("https://x.test/a#frag?token=secret"),
            "https://x.test/a#frag?token=secret"
        );
        assert_eq!(
            sanitize_media_url("http://[bad?token=secret"),
            "http://[bad?token=secret"
        );
    }

    #[test]
    fn complete_response_write_forces_cached_false() {
        let response = SearchResponse {
            cached: true,
            ..Default::default()
        };
        let encoded = encode_response(&response).unwrap();
        assert_eq!(encoded["cached"], false);
        assert!(!decode_response(encoded).unwrap().cached);
    }

    #[test]
    fn legacy_response_reader_coerces_python_cache_shapes() {
        let value = ssx_json::json!({
            "query": 7,
            "results": null,
            "scope": {
                "routing_rule": 0,
                "routing_fallback": "false",
                "routing_budget_applied": [],
                "jev_scores": {"rank": "1.25"}
            },
            "engine_outcomes": [{
                "engine": 3,
                "status": null,
                "result_count": "2",
                "latency_ms": "3.5",
                "message": "legacy"
            }],
            "response_time_ms": "12",
            "cached": "false",
            "partial": 0,
            "dispatched_engine_count": "4"
        });
        let response = decode_response(value).unwrap();
        assert_eq!(response.query, "7");
        assert!(response.results.is_empty());
        assert!(response.scope.routing_rule.is_empty());
        assert!(response.scope.routing_fallback);
        assert!(!response.scope.routing_budget_applied);
        assert_eq!(response.scope.jev_scores["rank"].get(), 1.25);
        assert_eq!(response.engine_outcomes[0].engine, "3");
        assert_eq!(response.engine_outcomes[0].status, "None");
        assert_eq!(response.engine_outcomes[0].result_count, 2);
        assert_eq!(
            response.engine_outcomes[0]
                .latency_ms
                .as_ref()
                .unwrap()
                .get(),
            3.5
        );
        assert_eq!(
            response.engine_outcomes[0].message.as_deref(),
            Some("legacy")
        );
        assert_eq!(response.response_time_ms, 12);
        assert!(response.cached);
        assert!(!response.partial);
        assert_eq!(response.dispatched_engine_count, 4);
    }
}

#[cfg(test)]
mod reviewed_legacy_tests {
    use super::*;
    #[test]
    fn python_numeric_legacy_inputs_and_invalid_forms() {
        let result =
            decode_result(ssx_json::json!({"score":true,"position":2.75,"tier":" 2 "})).unwrap();
        assert_eq!(result.position, 2);
        assert_eq!(result.tier, 2);
        assert_eq!(result.score.get(), 1.0);
        let result =
            decode_result(ssx_json::json!({"score":" ١_٢.٥ ","position":" ٢ ","tier":false}))
                .unwrap();
        assert_eq!(result.position, 2);
        assert_eq!(result.tier, 0);
        assert_eq!(result.score.get(), 12.5);
        for value in ["2.75", "1__2", "_2", "2_", "9223372036854775808"] {
            assert!(
                parse_i64(Some(&Value::String(value.into())), 0, "test").is_err(),
                "{value}"
            );
        }
        for value in ["nan", "inf", "1e999", "1__2"] {
            assert!(
                decode_result(ssx_json::json!({"score":value})).is_err(),
                "{value}"
            );
        }
        assert!(parse_i64(Some(&ssx_json::json!(9223372036854775808.0)), 0, "test").is_err());
    }
    #[test]
    fn python_str_uses_repr_for_containers_and_retains_object_order() {
        let raw: Value = ssx_json::from_str(
            r#"{"engine":{"z":true,"a":null},"engines":[true,null,[false,"a'b","a\"b","\u200b"]]}"#,
        )
        .unwrap();
        let result = decode_result(raw).unwrap();
        assert_eq!(result.engine, "{'z': True, 'a': None}");
        assert!(result.engines.contains("True"));
        assert!(result.engines.contains("None"));
        assert!(
            result
                .engines
                .contains("[False, \"a'b\", 'a\"b', '\\u200b']")
        );
    }
    #[test]
    fn urllib_preprocesses_control_characters_before_secret_matching() {
        for control in ['\n', '\t', '\r'] {
            let value = format!("https://x.test/a?to{control}ken=secret&size=small");
            assert_eq!(sanitize_media_url(&value), "https://x.test/a?size=small");
        }
        assert_eq!(
            sanitize_media_url("\0 \tHTTPS://x.test/a?token=secret&size=small#"),
            "https://x.test/a?size=small"
        );
        assert_eq!(
            sanitize_media_url("https://x.test/a?\n#fragment"),
            "https://x.test/a?\n#fragment"
        );
    }
}

#[cfg(test)]
mod urllib_contract_tests {
    use super::sanitize_media_url;
    #[test]
    fn additive_netloc_diagnostics_preserve_boolean_and_source_precedence() {
        for (netloc, error) in [
            ("www.oyez.org:port", None),
            ("www.oyez.org:[x]", Some("Invalid IPv6 URL")),
            ("www.oyez.org:]", Some("Invalid IPv6 URL")),
            ("[127.0.0.1]", Some("An IPv4 address cannot be in brackets")),
            ("[vX.address]", Some("IPvFuture address is invalid")),
            ("[v1.address]", None),
            ("[V1.address]", None),
            ("[::1]", None),
            ("[::1]suffix", Some("Invalid IPv6 URL")),
            ("[fe80::1%zone]", None),
            (
                "[fe80::1%]",
                Some("'fe80::1%' does not appear to be an IPv4 or IPv6 address"),
            ),
            (
                "[has'quote]",
                Some("\"has'quote\" does not appear to be an IPv4 or IPv6 address"),
            ),
        ] {
            let actual = super::python_url_netloc_error(netloc);
            assert_eq!(actual.as_deref(), error, "{netloc}");
            assert_eq!(super::valid_urllib_netloc(netloc), actual.is_none());
        }
        let table: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/unicode/python-text-16.0.0.json"
        ))
        .unwrap();
        for code in table["nfkc_url_delimiters"].as_array().unwrap() {
            let netloc = format!(
                "www.oyez.org:{}",
                char::from_u32(code.as_u64().unwrap() as u32).unwrap()
            );
            assert_eq!(
                super::python_url_netloc_error(&netloc),
                Some(format!(
                    "netloc '{netloc}' contains invalid characters under NFKC normalization"
                ))
            );
            assert!(!super::valid_urllib_netloc(&netloc));
        }
    }
    #[test]
    fn scheme_recognition_and_reconstruction_match_urllib() {
        for (input, expected) in [
            ("A/B:C?token=secret&size=small", "A/B:C?size=small"),
            ("http:/foo?token=s&x=1", "http:///foo?x=1"),
            ("http:foo?token=s&x=1", "http:foo?x=1"),
            ("custom://?token=s&x=1", "custom:?x=1"),
            ("http:////x/a;?token=s&x=1", "http:////x/a?x=1"),
        ] {
            assert_eq!(sanitize_media_url(input), expected);
        }
    }
    #[test]
    fn invalid_brackets_and_nfkc_hosts_keep_original_source_fallback() {
        for input in [
            "https://[bad]/a?token=s&x=1",
            "//[bad]/a?token=s&x=1",
            "https://example.com]/a?token=s&x=1",
            "https://[::1]x/a?token=s&x=1",
            "https://x℀y/a?token=s&x=1",
            "https://[127.0.0.1]/a?token=s&x=1",
        ] {
            assert_eq!(sanitize_media_url(input), input);
        }
        assert_eq!(
            sanitize_media_url("https://[vF.example]/a?token=s&x=1"),
            "https://[vF.example]/a?x=1"
        );
        assert_eq!(
            sanitize_media_url("https://[fe80::1%scope]/a?token=s&x=1"),
            "https://[fe80::1%scope]/a?x=1"
        );
    }
}

/// F13 durable snapshot compatibility codec.
pub mod snapshot;

#[path = "media_url.rs"]
mod media_url;
pub use media_url::{BudgetMediaInfo, MediaBudgetError, build_media_bounded};

#[cfg(test)]
mod a06_repr_scalar_compatibility {
    use super::*;
    #[test]
    fn additive_scalar_primitive_matches_existing_writer_for_every_unicode_scalar() {
        // A double quote in the input forces the existing writer's single-quote
        // branch, including when the inspected scalar itself is an apostrophe.
        for point in 0..=0x10ffff {
            let Some(ch) = char::from_u32(point) else {
                continue;
            };
            let input = format!("\"{ch}");
            let mut actual = String::new();
            write_python_string_repr(&input, &mut actual).unwrap();
            let expected = format!(
                "'\"{}'",
                python_repr_character(ch, PythonReprQuote::Single).as_str()
            );
            assert_eq!(actual, expected, "scalar U+{point:04X}");
        }
        assert_eq!(
            python_repr_character('"', PythonReprQuote::Double).as_str(),
            "\\\""
        );
        assert_eq!(
            python_repr_character('\'', PythonReprQuote::Double).as_str(),
            "'"
        );
    }
}
#[cfg(test)]
mod borrowed_result_tests {
    use super::*;
    #[test]
    fn borrowed_result_decoder_preserves_full_payloads_and_avoids_the_extra_row_array_clone() {
        fn arrays(value: &Value) -> usize {
            match value {
                Value::Array(values) => {
                    values.capacity() * std::mem::size_of::<Value>()
                        + values.iter().map(arrays).sum::<usize>()
                }
                Value::Object(values) => values.values().map(arrays).sum(),
                _ => 0,
            }
        }
        for field in ["payload", "work_group"] {
            let count = 100_000;
            let text = format!(
                "{{\"{field}\":{{\"rows\":[{}]}}}}",
                std::iter::repeat_n("{}", count)
                    .collect::<Vec<_>>()
                    .join(",")
            );
            assert!(text.len() < ssx_json::MAX_JSON_BYTES);
            let row: Value = ssx_json::from_str(&text).unwrap();
            let before = ssx_json::to_vec(&row).unwrap();
            let extra_row = row.clone();
            let extra_array_backing = arrays(&extra_row);
            assert_eq!(extra_array_backing, count * std::mem::size_of::<Value>());
            let owned = decode_result(extra_row).unwrap();
            let borrowed = decode_result_ref(&row).unwrap();
            assert_eq!(owned, borrowed);
            assert_eq!(encode_result(&owned), encode_result(&borrowed));
            assert_eq!(ssx_json::to_vec(&row).unwrap(), before);
            let retained = if field == "payload" {
                borrowed.payload.as_ref().unwrap()
            } else {
                borrowed.work_group.as_ref().unwrap()
            };
            assert_eq!(retained["rows"].as_array().unwrap().len(), count);
            println!(
                "W07_BORROWED_RESULT field={field} rows={count} input_bytes={} avoided_extra_row_array_capacity_bytes={extra_array_backing} owned_payload_array_capacity_bytes={} hash_map_string_decoder_allocator_rss_excluded=true",
                text.len(),
                arrays(retained)
            );
        }
        for (case, raw) in [
            ("empty", r#"{}"#),
            (
                "coercion_opaque",
                r#"{"engines":"{'z', 'a'}","score":" ١_٢.٥ ","position":false,"tier":" 2 ","payload":{"n":18446744073709551617,"x":1.000e0}}"#,
            ),
            ("invalid_score", r#"{"score":"invalid"}"#),
            ("invalid_tier", r#"{"tier":[]} "#),
            ("null", "null"),
            ("array", "[]"),
        ] {
            let row: Value = ssx_json::from_str(raw).unwrap();
            assert_eq!(decode_result(row.clone()), decode_result_ref(&row), "{raw}");
            println!("W07_BORROWED_RESULT_CASE case={case} full_owned_borrowed_equal=true");
        }
    }
}

#[cfg(test)]
#[path = "storage/snapshot_projection_tests.rs"]
mod snapshot_projection_tests;

// Static subset of accepted contracts/unicode/python-text-16.0.0.json.
// Source SHA256 9f6d9cb6e81031954d91cc03084237592e9d7bb2224f22cab5a5fba8cebefabd
const PYTHON_DECIMAL_ZEROES: &[u32] = &[
    0x30, 0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66,
    0xde6, 0xe50, 0xed0, 0xf20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90,
    0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0, 0xa9f0, 0xaa50, 0xabf0, 0xff10,
    0x104a0, 0x10d30, 0x10d40, 0x11066, 0x110f0, 0x11136, 0x111d0, 0x112f0, 0x11450, 0x114d0,
    0x11650, 0x116c0, 0x116d0, 0x116da, 0x11730, 0x118e0, 0x11950, 0x11bf0, 0x11c50, 0x11d50,
    0x11da0, 0x11f50, 0x16130, 0x16a60, 0x16ac0, 0x16b50, 0x16d70, 0x1ccf0, 0x1d7ce, 0x1d7d8,
    0x1d7e2, 0x1d7ec, 0x1d7f6, 0x1e140, 0x1e2f0, 0x1e4f0, 0x1e5f1, 0x1e950, 0x1fbf0,
];
/// Unicode16 decimal digit value without lazy JSON initialization or allocation.
pub fn python_decimal_digit(ch: char) -> Option<u8> {
    let code = ch as u32;
    let index = PYTHON_DECIMAL_ZEROES.partition_point(|zero| *zero <= code);
    if index > 0 {
        let digit = code - PYTHON_DECIMAL_ZEROES[index - 1];
        if digit < 10 {
            return Some(digit as u8);
        }
    }
    None
}
#[cfg(test)]
mod a06_decimal_subset_tests {
    use super::*;
    #[test]
    fn static_decimal_membership_equals_accepted_unicode16_source() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/unicode/python-text-16.0.0.json"
        ))
        .unwrap();
        let expected: Vec<u32> = source["decimal_zeroes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u32)
            .collect();
        assert_eq!(PYTHON_DECIMAL_ZEROES, expected);
        for point in 0..=0x10ffff {
            let Some(ch) = char::from_u32(point) else {
                continue;
            };
            let expected = expected.iter().find_map(|zero| {
                point
                    .checked_sub(*zero)
                    .filter(|v| *v < 10)
                    .map(|v| v as u8)
            });
            assert_eq!(python_decimal_digit(ch), expected, "scalar U+{point:04X}");
        }
    }
}
