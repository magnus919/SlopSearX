//! Full canonical evidence and immutable request-specific cache views.
use crate::PortFuture;
use sha2::{Digest, Sha256};
use ssx_domain::{
    EngineOutcome, SearchResponse, SearchResult,
    storage::{self},
};
use ssx_json::Value;
use std::{collections::BTreeSet, num::NonZeroU64, sync::Arc};

/// Shared canonical values expose no mutation, including nested JSON.
/// ```compile_fail
/// use ssx_application::cache::CanonicalResponse;
/// let canonical=CanonicalResponse::new(ssx_domain::SearchResponse::default());
/// canonical.response().answers.clear();
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalResponse(Arc<SearchResponse>);
impl CanonicalResponse {
    /// Typed fresh evidence has no optional persistence size/count limit.
    pub fn new(mut response: SearchResponse) -> Self {
        response.cached = false;
        Self(Arc::new(response))
    }
    pub fn response(&self) -> &SearchResponse {
        &self.0
    }
    pub fn shares_evidence(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn view(
        &self,
        include: BTreeSet<String>,
        max_results: Option<i64>,
        cached: bool,
    ) -> ResponseView {
        let end = max_results
            .filter(|n| *n > 0)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(self.0.results.len())
            .min(self.0.results.len());
        ResponseView {
            canonical: self.clone(),
            include,
            end,
            cached,
        }
    }
}
#[derive(Debug, Clone)]
pub struct ResponseView {
    canonical: CanonicalResponse,
    include: BTreeSet<String>,
    end: usize,
    cached: bool,
}
impl ResponseView {
    pub fn canonical(&self) -> &CanonicalResponse {
        &self.canonical
    }
    pub fn results(&self) -> &[SearchResult] {
        &self.canonical.0.results[..self.end]
    }
    pub fn engine_outcomes(&self) -> &[EngineOutcome] {
        if self.include.contains("engine_status") {
            &self.canonical.0.engine_outcomes
        } else {
            &[]
        }
    }
    pub fn suggestions(&self) -> &[String] {
        if self.include.contains("suggestions") {
            &self.canonical.0.suggestions
        } else {
            &[]
        }
    }
    pub fn empty_engines(&self) -> &[Vec<String>] {
        if self.include.contains("diagnostics") {
            &self.canonical.0.empty_engines
        } else {
            &[]
        }
    }
    pub fn cached(&self) -> bool {
        self.cached
    }
    /// An owned transport projection cannot mutate the shared canonical value.
    pub fn to_response(&self) -> SearchResponse {
        let source = self.canonical.response();
        SearchResponse {
            query: source.query.clone(),
            scope: source.scope.clone(),
            query_id: source.query_id.clone(),
            answers: source.answers.clone(),
            corrections: source.corrections.clone(),
            infoboxes: source.infoboxes.clone(),
            ranking_explanation: source.ranking_explanation.clone(),
            response_time_ms: source.response_time_ms,
            partial: source.partial,
            deadline_exceeded: source.deadline_exceeded,
            all_unresponsive: source.all_unresponsive,
            cached_error: source.cached_error,
            dispatched_engine_count: source.dispatched_engine_count,
            date_filter: source.date_filter.clone(),
            results: self.results().to_vec(),
            engine_outcomes: self.engine_outcomes().to_vec(),
            suggestions: self.suggestions().to_vec(),
            empty_engines: self.empty_engines().to_vec(),
            cached: self.cached,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheFailure {
    Unavailable,
    Closed,
    Malformed,
    CompletionUnknown,
    InvalidInput,
}
#[derive(Debug, Clone, PartialEq)]
pub enum CacheEntry {
    Canonical(CanonicalResponse),
    Negative {
        timestamp: Option<ssx_domain::WallClockTimestamp>,
    },
}
/// Cache errors remain observable at the port even when optional reads degrade.
pub trait CacheRepository: Send + Sync {
    fn read<'a>(&'a self, key: &'a str)
    -> PortFuture<'a, Result<Option<CacheEntry>, CacheFailure>>;
    fn write<'a>(
        &'a self,
        key: &'a str,
        value: &'a CacheEntry,
        ttl: NonZeroU64,
    ) -> PortFuture<'a, Result<(), CacheFailure>>;
}
/// Current-request inputs used only to project a negative cached outcome.
#[derive(Debug, Clone)]
pub struct CachedViewContext {
    pub query: String,
    pub query_id: ssx_domain::QueryId,
    pub ranking_explanation: String,
    pub include: BTreeSet<String>,
    pub max_results: Option<i64>,
}
impl CacheEntry {
    pub fn view(&self, context: CachedViewContext) -> ResponseView {
        let canonical = match self {
            Self::Canonical(response) => response.clone(),
            Self::Negative { .. } => CanonicalResponse::new(SearchResponse {
                query: context.query,
                query_id: context.query_id.as_str().into(),
                ranking_explanation: context.ranking_explanation,
                cached_error: true,
                ..Default::default()
            }),
        };
        canonical.view(context.include, context.max_results, true)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheAdmission {
    Complete,
    Partial,
    AllUnresponsive,
    DeadlineTruncated,
    RerankFallback,
    Rejected,
}
impl CacheAdmission {
    pub fn admits(self) -> bool {
        matches!(self, Self::Complete | Self::Partial)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct CacheTtls {
    general: NonZeroU64,
    partial: NonZeroU64,
    negative: NonZeroU64,
}
impl CacheTtls {
    pub fn new(general: u64, partial: u64, negative: u64) -> Result<Self, CacheFailure> {
        if partial > 300 {
            return Err(CacheFailure::InvalidInput);
        }
        Ok(Self {
            general: NonZeroU64::new(general).ok_or(CacheFailure::InvalidInput)?,
            partial: NonZeroU64::new(partial).ok_or(CacheFailure::InvalidInput)?,
            negative: NonZeroU64::new(negative).ok_or(CacheFailure::InvalidInput)?,
        })
    }
    /// Pure boundary parsing reuses accepted Python integer text normalization.
    pub fn from_text(general: &str, partial: &str, negative: &str) -> Result<Self, CacheFailure> {
        let parse = |text: &str| {
            storage::normalize_python_number_text(text)
                .map_err(|_| CacheFailure::InvalidInput)?
                .parse::<u64>()
                .map_err(|_| CacheFailure::InvalidInput)
        };
        Self::new(parse(general)?, parse(partial)?, parse(negative)?)
    }
    pub fn for_response(self, categories: &[String], partial: bool) -> NonZeroU64 {
        let mut ttl = self.general.get();
        if categories.iter().any(|c| c.to_lowercase().contains("news")) {
            ttl = ttl.min(300);
        }
        if partial {
            ttl = ttl.min(self.partial.get());
        }
        NonZeroU64::new(ttl).expect("positive configured TTL")
    }
}
impl Default for CacheTtls {
    fn default() -> Self {
        Self::new(3600, 30, 60).expect("valid defaults")
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheWriteOutcome {
    Stored,
    Skipped,
    NotStored(CacheFailure),
}
pub struct CanonicalCache {
    store: Arc<dyn CacheRepository>,
    ttls: CacheTtls,
}
impl CanonicalCache {
    pub fn new(store: Arc<dyn CacheRepository>, ttls: CacheTtls) -> Self {
        Self { store, ttls }
    }
    pub async fn read(&self, key: &SearchCacheKey, prefer_fresh: bool) -> Option<CacheEntry> {
        if prefer_fresh {
            return None;
        }
        self.store.read(key.as_str()).await.ok().flatten()
    }
    pub async fn write(
        &self,
        key: &SearchCacheKey,
        response: &CanonicalResponse,
        categories: &[String],
        admission: CacheAdmission,
    ) -> CacheWriteOutcome {
        if !admission.admits() {
            return CacheWriteOutcome::Skipped;
        }
        match self
            .store
            .write(
                key.as_str(),
                &CacheEntry::Canonical(response.clone()),
                self.ttls
                    .for_response(categories, admission == CacheAdmission::Partial),
            )
            .await
        {
            Ok(()) => CacheWriteOutcome::Stored,
            Err(error) => CacheWriteOutcome::NotStored(error),
        }
    }
    pub async fn write_negative(&self, key: &SearchCacheKey, now: f64) -> Result<(), CacheFailure> {
        if !now.is_finite() {
            return Err(CacheFailure::InvalidInput);
        }
        self.store
            .write(
                key.as_str(),
                &CacheEntry::Negative {
                    timestamp: Some(
                        ssx_domain::WallClockTimestamp::new(now)
                            .map_err(|_| CacheFailure::InvalidInput)?,
                    ),
                },
                self.ttls.negative,
            )
            .await
    }
}

/// Search identities cannot be substituted with query-only answer keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCacheKey(String);
impl SearchCacheKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Display for SearchCacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Exact source base-key inputs. Date bounds are already resolved once by caller.
#[derive(Debug, Clone)]
pub struct CacheKeyInput {
    pub query: String,
    /// A04 provider scope: omission preserves historical keys; empty is explicit.
    pub subreddit: Option<String>,
    pub language: String,
    pub safesearch: i64,
    pub categories: Vec<String>,
    pub engines: Vec<String>,
    pub page: ssx_json::Number,
    pub time_range: Option<String>,
    pub media_type: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub relative_window: Option<(String, String)>,
}
impl Default for CacheKeyInput {
    fn default() -> Self {
        Self {
            query: String::new(),
            subreddit: None,
            language: "en".into(),
            safesearch: 0,
            categories: vec![],
            engines: vec![],
            page: ssx_json::Number::new("1").expect("integer literal"),
            time_range: None,
            media_type: None,
            date_from: None,
            date_to: None,
            relative_window: None,
        }
    }
}
pub fn source_base_key(input: &CacheKeyInput) -> SearchCacheKey {
    let mut categories = input.categories.clone();
    categories.sort();
    let mut engines = input.engines.clone();
    engines.sort();
    let window = if input.time_range.as_ref().is_some_and(|v| !v.is_empty()) {
        Some(
            input
                .relative_window
                .as_ref()
                .map_or_else(Vec::new, |(a, b)| vec![a.as_str(), b.as_str()]),
        )
    } else {
        None
    };
    // Identity serialization is pure, not the bounded durable Value conversion.
    // This closed tuple contains only strings, integer primitives and arrays.
    let bytes = ssx_json::to_vec(&(
        input.query.trim_matches(ssx_json::python_whitespace),
        &input.language,
        input.safesearch,
        categories,
        engines,
        &input.page,
        &input.time_range,
        &input.media_type,
        &input.date_from,
        &input.date_to,
        window,
    ))
    .expect("closed identity primitives serialize");
    let legacy = SearchCacheKey(format!("search:v2:{}", digest(&bytes)));
    match &input.subreddit {
        None => legacy,
        Some(subreddit) => {
            // A distinct framing preserves every historical omitted-field key,
            // while empty and arbitrary string scopes cannot alias that namespace.
            let scoped = ssx_json::to_vec(&(legacy.as_str(), subreddit))
                .expect("closed subreddit identity primitives serialize");
            SearchCacheKey(format!("search:v2:subreddit:{}", digest(&scoped)))
        }
    }
}
pub fn answer_key(query: &str) -> String {
    format!(
        "answer:v2:{}",
        digest(query.trim_matches(ssx_json::python_whitespace).as_bytes())
    )
}
pub(crate) fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A separately typed query-only answer cache, preserving the legacy broad key.
pub trait AnswerRepository: Send + Sync {
    fn read_answer<'a>(
        &'a self,
        key: &'a str,
    ) -> PortFuture<'a, Result<Option<Value>, CacheFailure>>;
    fn write_answer<'a>(
        &'a self,
        key: &'a str,
        value: &'a Value,
        ttl: NonZeroU64,
    ) -> PortFuture<'a, Result<(), CacheFailure>>;
}
pub struct AnswerCache {
    store: Arc<dyn AnswerRepository>,
    ttl: NonZeroU64,
}
impl AnswerCache {
    pub fn new(store: Arc<dyn AnswerRepository>, ttl: NonZeroU64) -> Self {
        Self { store, ttl }
    }
    pub async fn get(&self, query: &str) -> Option<Value> {
        self.store
            .read_answer(&answer_key(query))
            .await
            .ok()
            .flatten()
    }
    pub async fn set(&self, query: &str, value: &Value) -> Result<(), CacheFailure> {
        if !value.is_object() {
            return Err(CacheFailure::Malformed);
        }
        self.store
            .write_answer(&answer_key(query), value, self.ttl)
            .await
    }
}

/// One pre-dispatch snapshot of every source routing contributor. Opaque routing
/// extensions are supplied by their accepted owners, never inferred from a view.
#[derive(Debug, Clone)]
pub struct RoutingIdentity {
    pub catalog: Option<Vec<RoutingEngineIdentity>>,
    pub sensitive: BTreeSet<String>,
    pub tier1: BTreeSet<String>,
    pub budget: Option<RoutingBudgetIdentity>,
    pub ranking_strategy: String,
    pub work_group_version: String,
    pub rerank_identity: Option<(String, String)>,
    pub routing_extension: Option<String>,
}
#[derive(Debug, Clone)]
pub struct RoutingEngineIdentity {
    pub name: String,
    pub active: bool,
    pub enabled: bool,
    pub auth_class: String,
    pub auth_configured: bool,
    pub cost_class: String,
    pub circuit_open: bool,
    pub observed_status: String,
    pub observed_stale: bool,
}
#[derive(Debug, Clone)]
pub struct RoutingBudgetIdentity {
    pub max_engines: usize,
    pub max_cost_class: String,
    pub coverage_target: usize,
}
impl RoutingIdentity {
    /// Native fingerprint v1 uses framed JSON, preserving source contributors
    /// without importing Python repr/delimiter ambiguity into a recomputable key.
    pub fn fingerprint(&self) -> String {
        let health = self.budget.as_ref().is_some_and(|b| b.max_engines > 0);
        let engines = self.catalog.as_ref().map(|engines| {
            let mut engines = engines
                .iter()
                .filter(|e| e.active && e.enabled)
                .collect::<Vec<_>>();
            engines.sort_by(|a, b| a.name.cmp(&b.name));
            engines
                .into_iter()
                .map(|e| {
                    (
                        e.name.as_str(),
                        e.auth_class.as_str(),
                        e.auth_configured,
                        e.cost_class.as_str(),
                        e.circuit_open,
                        health.then_some((e.observed_status.as_str(), e.observed_stale)),
                    )
                })
                .collect::<Vec<_>>()
        });
        let budget = self
            .budget
            .as_ref()
            .map(|b| (b.max_engines, b.max_cost_class.as_str(), b.coverage_target));
        let bytes = ssx_json::to_vec(&(
            engines,
            &self.sensitive,
            &self.tier1,
            budget,
            &self.ranking_strategy,
            &self.work_group_version,
            &self.rerank_identity,
            &self.routing_extension,
        ))
        .expect("closed routing primitives serialize");
        digest(&bytes)
    }
}

pub fn composed_key(
    input: &CacheKeyInput,
    routing: &RoutingIdentity,
    interactive_timeout_ms: Option<u64>,
    generate_suggestions: bool,
) -> SearchCacheKey {
    let base = source_base_key(input);
    let bytes = ssx_json::to_vec(&(
        base.as_str(),
        routing.fingerprint(),
        interactive_timeout_ms,
        generate_suggestions,
    ))
    .expect("closed composed primitives serialize");
    SearchCacheKey(format!("search:native:v1:{}", digest(&bytes)))
}
