//! Native shared search execution and task ownership.
use ssx_application::{
    Audit, Metrics, PortFuture, SearchContext, SearchEngine, Supervisor, TaskEvent, TaskState,
    cache::{
        CacheAdmission, CacheEntry, CacheKeyInput, CanonicalCache, CanonicalResponse,
        RoutingIdentity, composed_key,
    },
    health::{HealthState, OutcomeOrigin},
    policy::{PolicyError, PolicyService, ScopePreview, ScopeProposal, ValidatedScope},
    ranking::{Feeds, rank, ranking_explanation},
    scholarly::group_publications,
};
use ssx_domain::{
    AdapterResponse, EngineOutcome, EngineStatus, ScopeDecision, SearchRequest, SearchResponse,
    WallClockTimestamp,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::{Notify, OwnedSemaphorePermit, Semaphore, watch},
    task::{JoinHandle, JoinSet},
    time::Instant,
};

/// Logical generation must change whenever an adapter is replaced or reconfigured.
/// It is supplied by the composition root, never a process address.
#[derive(Clone)]
pub struct EngineBinding {
    pub engine: Arc<dyn SearchEngine>,
    pub generation: String,
    pub timeout: Duration,
    pub scrape: bool,
}
pub trait ClientAdmission: Send + Sync {
    fn acquire<'a>(&'a self, client: &'a str) -> PortFuture<'a, bool>;
}
pub trait Suggestions: Send + Sync {
    fn fetch<'a>(&'a self, query: &'a str) -> PortFuture<'a, Vec<String>>;
    fn shutdown(&self) -> PortFuture<'_, ()>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RerankState {
    Skipped,
    Applied,
    Fallback,
}
pub struct RerankDecision {
    pub results: Vec<ssx_domain::SearchResult>,
    pub state: RerankState,
}
/// F15 implements this optional stage. Absence executes the complete baseline
/// ranking path; a classified fallback must never populate canonical cache.
pub trait RerankStage: Send + Sync {
    fn identity(&self) -> Option<(String, String)> {
        None
    }
    fn shutdown(&self) -> PortFuture<'_, ()> {
        Box::pin(async {})
    }
    fn rerank<'a>(
        &'a self,
        request: &'a SearchRequest,
        results: &'a [ssx_domain::SearchResult],
        deadline: Instant,
    ) -> PortFuture<'a, Result<RerankDecision, ()>>;
}
pub trait SearchClock: Send + Sync {
    /// One captured instant supplies both epoch and process-local calendar date.
    fn capture(&self) -> (WallClockTimestamp, String);
}
/// Monotonic search-execution clock, separate from calendar/epoch capture.
/// The default is System. Only trusted construction can supply an alternate;
/// public server startup has no operator/environment/request selection path.
pub trait SearchDeadlineClock: Send + Sync {
    fn now(&self) -> Instant;
    fn sleep_until(&self, deadline: Instant) -> PortFuture<'_, ()>;
}
#[derive(Clone, Default)]
enum DeadlineClock {
    #[default]
    System,
    Injected(Arc<dyn SearchDeadlineClock>),
}
impl DeadlineClock {
    fn now(&self) -> Instant {
        match self {
            Self::System => Instant::now(),
            Self::Injected(clock) => clock.now(),
        }
    }
    async fn sleep_until(&self, deadline: Instant) {
        match self {
            Self::System => tokio::time::sleep_until(deadline).await,
            Self::Injected(clock) => clock.sleep_until(deadline).await,
        }
    }
    async fn timeout_at<T>(
        &self,
        deadline: Instant,
        future: impl std::future::Future<Output = T>,
    ) -> Option<T> {
        match self {
            // Preserve Tokio's exact existing poll/deadline semantics by default.
            Self::System => tokio::time::timeout_at(deadline, future).await.ok(),
            Self::Injected(clock) => {
                tokio::select! { biased; _ = clock.sleep_until(deadline) => None, value = future => Some(value) }
            }
        }
    }
}
pub struct SystemSearchClock;
impl SearchClock for SystemSearchClock {
    fn capture(&self) -> (WallClockTimestamp, String) {
        Self::at(SystemTime::now())
    }
}
impl SystemSearchClock {
    pub fn at(now: SystemTime) -> (WallClockTimestamp, String) {
        let local: chrono::DateTime<chrono::Local> = now.into();
        let seconds = now
            .duration_since(UNIX_EPOCH)
            .map_or_else(|e| -e.duration().as_secs_f64(), |v| v.as_secs_f64());
        (
            WallClockTimestamp::new(seconds).expect("finite system clock"),
            local.format("%Y-%m-%d").to_string(),
        )
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum SearchError {
    Policy(PolicyError),
    ClientLimited,
    Busy,
    Closed,
    Cancelled,
    InvalidConfiguration,
    Unavailable(SearchUnavailable),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnavailableReason {
    NoEligibleEngines,
    NativeImplementationUnavailable,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SearchUnavailable {
    /// Policy-checked explanation only. This type grants no dispatch capability.
    pub preview: Box<ScopePreview>,
    pub reason: UnavailableReason,
}
#[derive(Clone)]
pub struct SearchConfig {
    pub max_callers: usize,
    pub max_flights: usize,
    pub max_parallel_engines: usize,
    pub max_selected_engines: usize,
    pub ranking_strategy: String,
    pub empty_scrape_diagnostics: bool,
    /// Backstop for injected optional ports; native implementations also carry
    /// their accepted per-operation bounds. This is separate from dispatch time.
    pub optional_port_timeout: Duration,
}
impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_callers: 256,
            max_flights: 64,
            max_parallel_engines: 10,
            max_selected_engines: 128,
            ranking_strategy: "presence".into(),
            empty_scrape_diagnostics: false,
            optional_port_timeout: Duration::from_secs(30),
        }
    }
}
pub struct SearchSettings {
    pub execution: SearchConfig,
    pub client_quota: ssx_domain::rate::QuotaConfig,
    pub client_fail_closed: bool,
    pub brave_category_routing: bool,
}
impl SearchSettings {
    /// Parse a captured environment, never ambient process state. Invalid source
    /// text falls back; native F10 rejects nonfinite/unsafe quota arithmetic.
    pub fn from_environment(
        env: &BTreeMap<String, String>,
        ranking_strategy: &str,
    ) -> Result<Self, SearchError> {
        let concurrency = env
            .get("MAX_CONCURRENT_ENGINES")
            .map_or_else(|| "10".into(), |s| s.clone());
        let normalized =
            ssx_domain::storage::normalize_python_number_text(&concurrency).unwrap_or_default();
        let digits = normalized.strip_prefix(['+', '-']).unwrap_or(&normalized);
        let max_parallel_engines =
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
                if normalized.starts_with('-') {
                    1
                } else {
                    normalized
                        .trim_start_matches('+')
                        .parse::<usize>()
                        .map_err(|_| SearchError::InvalidConfiguration)?
                        .max(1)
                }
            } else {
                10
            };
        let float = |name: &str, default: f64| {
            env.get(name)
                .and_then(|v| ssx_domain::storage::normalize_python_number_text(v).ok())
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(default)
        };
        let client_quota = ssx_domain::rate::QuotaConfig::new(
            float("PER_CLIENT_REQUESTS", 30.0),
            float("PER_CLIENT_WINDOW_SECONDS", 60.0),
        )
        .map_err(|_| SearchError::InvalidConfiguration)?;
        if max_parallel_engines > Semaphore::MAX_PERMITS {
            return Err(SearchError::InvalidConfiguration);
        }
        let raw_flag = |name: &str| {
            env.get(name)
                .map(|v| v.to_lowercase())
                .is_some_and(|v| matches!(v.as_str(), "true" | "1"))
        };
        Ok(Self {
            execution: SearchConfig {
                max_parallel_engines,
                ranking_strategy: ranking_strategy.into(),
                empty_scrape_diagnostics: raw_flag("FEATURE_EMPTY_SCRAPE_DIAGNOSTICS"),
                ..Default::default()
            },
            client_quota,
            client_fail_closed: env.get("FAIL_CLOSED").is_some_and(|v| {
                matches!(
                    v.trim_matches(ssx_json::python_whitespace)
                        .to_lowercase()
                        .as_str(),
                    "true" | "1" | "yes"
                )
            }),
            brave_category_routing: raw_flag("FEATURE_BRAVE_CATEGORY_ROUTING"),
        })
    }
}
pub struct SearchDependencies {
    pub policy: PolicyService,
    pub engines: BTreeMap<String, EngineBinding>,
    pub routing_identity: RoutingIdentity,
    pub cache: Option<Arc<CanonicalCache>>,
    pub client_admission: Option<Arc<dyn ClientAdmission>>,
    pub suggestions: Option<Arc<dyn Suggestions>>,
    pub jev: Option<Arc<crate::jev::JevRouter>>,
    pub routing_cache: Option<crate::valkey::Valkey>,
    pub metrics: Arc<dyn Metrics>,
    pub supervisor: Arc<dyn Supervisor>,
    pub audit: Arc<dyn Audit>,
    /// Cooldown-only lookup; adapter-owned F10 admission consumes engine quota.
    pub cooldowns: Option<Arc<dyn ssx_application::rate::RateStore>>,
    /// Explicitly owned transports/stores, closed only after all flights join.
    pub owned_http: Vec<Arc<crate::http::AdapterHttp>>,
    pub owned_stores: Vec<Arc<crate::valkey::Valkey>>,
    pub clock: Arc<dyn SearchClock>,
    pub reranker: Option<Arc<dyn RerankStage>>,
}
struct Flight {
    waiters: Mutex<usize>,
    cancel: watch::Sender<bool>,
    result: watch::Sender<Option<Result<CanonicalResponse, SearchError>>>,
    task: Mutex<Option<JoinHandle<()>>>,
    done: watch::Sender<bool>,
}
struct Completion(watch::Sender<bool>);
impl Drop for Completion {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}
#[derive(Clone)]
pub struct SearchCancellation(watch::Sender<bool>);
impl Default for SearchCancellation {
    fn default() -> Self {
        Self(watch::channel(false).0)
    }
}
impl SearchCancellation {
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
    /// Observe the caller's existing signal without creating an owner or task.
    pub async fn cancelled(&self) {
        cancelled(&mut self.0.subscribe()).await;
    }
}
struct Waiter(Arc<Flight>);
impl Drop for Waiter {
    fn drop(&mut self) {
        let mut waiters = self.0.waiters.lock().unwrap();
        *waiters -= 1;
        if *waiters == 0 {
            self.0.cancel.send_replace(true);
        }
    }
}
/// Trusted read-only runtime facts. Implementations must return an in-memory,
/// secret-free snapshot; this seam never authorizes or dispatches engines.
pub trait AdvisoryCatalog: Send + Sync {
    fn snapshot(&self) -> ssx_application::scope::CatalogSnapshot;
}
/// Trusted, bounded, in-memory current policy. Never caller input or I/O.
pub trait CurrentPolicy: Send + Sync {
    fn current(&self) -> PolicyService;
}
struct Inner {
    policy_source: Option<Arc<dyn CurrentPolicy>>,
    deadline_clock: DeadlineClock,
    advisory_catalog: Option<Arc<dyn AdvisoryCatalog>>,
    registry: Arc<ssx_registry::Registry>,
    config: SearchConfig,
    deps: SearchDependencies,
    callers: Arc<Semaphore>,
    callers_done: Arc<Notify>,
    stop: watch::Sender<bool>,
    engines: Arc<Semaphore>,
    flights: Mutex<HashMap<String, Arc<Flight>>>,
    health: Mutex<BTreeMap<String, HealthState>>,
    closed: AtomicBool,
    #[cfg(test)]
    collection_pause: Mutex<Option<Arc<CollectionPause>>>,
}
#[derive(Clone)]
pub struct SearchService(Arc<Inner>);
struct CallerGuard {
    permit: Option<OwnedSemaphorePermit>,
    done: Arc<Notify>,
}
impl Drop for CallerGuard {
    fn drop(&mut self) {
        drop(self.permit.take());
        self.done.notify_waiters();
    }
}
pub struct CapturedSearch {
    pub canonical: CanonicalResponse,
    pub response: SearchResponse,
    /// Optional persistence failure never discards the fresh complete evidence.
    pub snapshot: Result<ssx_domain::SnapshotId, ssx_application::snapshot::SnapshotFailure>,
}
/// Immutable read capture. Presentation bounds and admitted caller context remain
/// private; obtaining a capture executes the original single authority/quota path.
pub struct SearchReadCapture {
    canonical: CanonicalResponse,
    request: SearchRequest,
    scope: ScopeDecision,
    query_id: String,
    elapsed_ms: i64,
    cached: bool,
    service: std::sync::Weak<Inner>,
}
pub struct SearchReadView {
    pub response: SearchResponse,
    /// Read-only notes are deliberately outside canonical cache/snapshot evidence.
    pub advisories: Vec<ssx_json::Value>,
}
impl SearchReadCapture {
    fn new(
        service: &SearchService,
        canonical: CanonicalResponse,
        request: SearchRequest,
        mut scope: ScopeDecision,
        query_id: String,
        start: Instant,
        cached: bool,
    ) -> Self {
        if canonical.response().deadline_exceeded
            && !scope
                .warnings
                .iter()
                .any(|w| w.contains("Interactive deadline"))
        {
            scope.warnings.push(
                "Interactive deadline reached; captured results or suggestions may be incomplete."
                    .into(),
            );
        }
        Self {
            canonical,
            request,
            scope,
            query_id,
            elapsed_ms: i64::try_from(start.elapsed().as_millis()).unwrap_or(i64::MAX),
            cached,
            service: Arc::downgrade(&service.0),
        }
    }
    pub fn full_result_count(&self) -> usize {
        self.canonical.response().results.len()
    }
    fn personalize(&self, mut response: SearchResponse) -> SearchResponse {
        response.query = self.request.query.clone();
        response.query_id = self.query_id.clone();
        response.scope = self.scope.clone();
        response.response_time_ms = self.elapsed_ms;
        response
    }
    pub fn present(&self) -> SearchResponse {
        self.personalize(
            self.canonical
                .view(
                    self.request.include.clone(),
                    self.request.max_results.and_then(|v| i64::try_from(v).ok()),
                    self.cached,
                )
                .to_response(),
        )
    }
    fn full_personalized(&self) -> SearchResponse {
        self.personalize(
            self.canonical
                .view(
                    ["results", "suggestions", "engine_status", "diagnostics"]
                        .into_iter()
                        .map(str::to_owned)
                        .collect(),
                    None,
                    self.cached,
                )
                .to_response(),
        )
    }
    pub fn present_with_advisories(&self) -> Result<SearchReadView, SearchError> {
        let inner = self.service.upgrade().ok_or(SearchError::Closed)?;
        let service = SearchService(inner);
        let advisories =
            service.read_advisories(&self.request, &self.scope, self.full_result_count())?;
        Ok(SearchReadView {
            response: self.present(),
            advisories,
        })
    }
}
impl SearchService {
    /// Read-only public notes. This does not acquire scope, quota, cache or dispatch authority.
    /// HTTP uses the same writer for a policy-checked empty eligible scope.
    pub fn read_advisories(
        &self,
        request: &SearchRequest,
        scope: &ScopeDecision,
        full_result_count: usize,
    ) -> Result<Vec<ssx_json::Value>, SearchError> {
        let policy = self.current_policy(self.0.deps.clock.capture().0)?;
        let query = request.query.to_lowercase();
        let topic = policy.resolver.topics.as_ref().and_then(|topics| {
            topics
                .iter()
                .find(|t| !t.engines.is_empty() && t.keywords.iter().any(|k| query.contains(k)))
        });
        let automatic_names = topic.map_or_else(
            || policy.resolver.tier1.clone(),
            |t| t.engines.iter().cloned().collect(),
        );
        let catalog = self
            .0
            .advisory_catalog
            .as_ref()
            .map(|source| source.snapshot())
            .unwrap_or_else(|| policy.catalog.clone());
        let engines = catalog
            .engines()
            .iter()
            .filter_map(|e| {
                let descriptor = self.0.registry.descriptor(&e.name)?;
                Some(ssx_application::advisories::Engine {
                    name: e.name.clone(),
                    display_name: descriptor.display_name.clone(),
                    enabled: e.enabled,
                    credentials_required: descriptor.auth_class
                        == ssx_registry::AuthClass::Required,
                    credentials_configured: e.auth_ready,
                    sensitive: descriptor.sensitive,
                    categories: e.categories.clone(),
                    media_types: e.media_types.clone(),
                })
            })
            .collect();
        let hidden = policy
            .policy
            .sensitive_engines
            .union(&policy.resolver.sensitive)
            .cloned()
            .collect();
        let context = ssx_application::advisories::Context {
            engines,
            hidden,
            automatic_names,
            routing_configured: self.0.deps.jev.is_some(),
            reranking_configured: self.0.deps.reranker.is_some(),
        };
        Ok(ssx_application::advisories::search(
            request,
            scope,
            &context,
            full_result_count,
        ))
    }
}

impl SearchService {
    /// Install only before sharing, with the exact initial dependency policy.
    /// Dynamic consumers still use the existing shared policy methods.
    pub fn with_policy_source(
        mut self,
        source: Arc<dyn CurrentPolicy>,
    ) -> Result<Self, SearchError> {
        let inner = Arc::get_mut(&mut self.0).ok_or(SearchError::InvalidConfiguration)?;
        if format!("{:?}", source.current()) != format!("{:?}", inner.deps.policy) {
            return Err(SearchError::InvalidConfiguration);
        }
        inner.policy_source = Some(source);
        Ok(self)
    }
    /// Construction-only injection. Refuse replacement after the owner is shared.
    /// Normal constructors and production startup always retain the System clock.
    pub fn with_deadline_clock(
        mut self,
        clock: Arc<dyn SearchDeadlineClock>,
    ) -> Result<Self, SearchError> {
        Arc::get_mut(&mut self.0)
            .ok_or(SearchError::InvalidConfiguration)?
            .deadline_clock = DeadlineClock::Injected(clock);
        Ok(self)
    }

    pub fn new(config: SearchConfig, deps: SearchDependencies) -> Result<Self, SearchError> {
        let registry =
            ssx_registry::Registry::builtins().map_err(|_| SearchError::InvalidConfiguration)?;
        Self::new_with_registry(config, deps, Arc::new(registry))
    }
    /// Trusted composition only; this registry never comes from a search input.
    pub fn new_with_registry(
        config: SearchConfig,
        deps: SearchDependencies,
        registry: Arc<ssx_registry::Registry>,
    ) -> Result<Self, SearchError> {
        Self::new_with_read_catalog(config, deps, registry, None)
    }
    /// Constructor-only live advisory source. It cannot change policy authority.
    pub fn new_with_read_catalog(
        config: SearchConfig,
        mut deps: SearchDependencies,
        registry: Arc<ssx_registry::Registry>,
        advisory_catalog: Option<Arc<dyn AdvisoryCatalog>>,
    ) -> Result<Self, SearchError> {
        if let Some(identity) = deps.reranker.as_ref().and_then(|stage| stage.identity()) {
            deps.routing_identity.rerank_identity = Some(identity);
        }
        if config.max_callers == 0
            || config.max_flights == 0
            || config.max_parallel_engines == 0
            || config.max_selected_engines == 0
            || config.max_callers > Semaphore::MAX_PERMITS
            || config.max_parallel_engines > Semaphore::MAX_PERMITS
            || config.optional_port_timeout.is_zero()
            || Instant::now()
                .checked_add(config.optional_port_timeout)
                .is_none()
            || deps.engines.values().any(|e| {
                e.generation.is_empty()
                    || e.timeout.is_zero()
                    || Instant::now().checked_add(e.timeout).is_none()
            })
        {
            return Err(SearchError::InvalidConfiguration);
        }
        Ok(Self(Arc::new(Inner {
            policy_source: None,
            deadline_clock: DeadlineClock::System,
            advisory_catalog,
            registry,
            callers: Arc::new(Semaphore::new(config.max_callers)),
            callers_done: Arc::new(Notify::new()),
            stop: watch::channel(false).0,
            engines: Arc::new(Semaphore::new(config.max_parallel_engines)),
            config,
            deps,
            flights: Mutex::new(HashMap::new()),
            health: Mutex::new(BTreeMap::new()),
            closed: AtomicBool::new(false),
            #[cfg(test)]
            collection_pause: Mutex::new(None),
        })))
    }
    pub fn health(&self) -> BTreeMap<String, HealthState> {
        self.0.health.lock().unwrap().clone()
    }
    fn current_policy(&self, now: WallClockTimestamp) -> Result<PolicyService, SearchError> {
        let mut policy = self
            .0
            .policy_source
            .as_ref()
            .map_or_else(|| self.0.deps.policy.clone(), |source| source.current());
        let mut capabilities = policy.catalog.engines().to_vec();
        for capability in &mut capabilities {
            if let Some(health) = self.0.health.lock().unwrap().get(&capability.name) {
                capability.circuit_open |= health.circuit_open(now);
                if let Ok(value) =
                    ssx_application::health::build_engine_health(Some(health), None, now, 300.0)
                {
                    capability.health_status = value["status"].as_str().unwrap_or("unknown").into();
                    capability.health_stale = value["stale"].as_bool().unwrap_or(true);
                }
            }
        }
        policy.catalog = ssx_application::scope::CatalogSnapshot::new(capabilities)
            .map_err(|_| SearchError::InvalidConfiguration)?;
        Ok(policy)
    }
    /// Check workflow authority using the same current runtime policy as search.
    pub fn validate_research_workflow(
        &self,
        job: &ssx_domain::research::ResearchJob,
    ) -> Result<(), SearchError> {
        self.current_policy(self.0.deps.clock.capture().0)?
            .validate_research_workflow(job)
            .map_err(SearchError::Policy)
    }
    /// Source coverage order and credential provenance from the trusted catalog.
    pub fn research_coverage(
        &self,
        query: &ssx_domain::research::ResearchQuery,
        response: &SearchResponse,
    ) -> Result<Vec<ssx_domain::research::EngineCoverage>, SearchError> {
        let policy = self.current_policy(self.0.deps.clock.capture().0)?;
        let mut coverage = BTreeMap::new();
        for outcome in &response.engine_outcomes {
            let missing = self
                .0
                .registry
                .descriptor(&outcome.engine)
                .is_some_and(|descriptor| {
                    descriptor.auth_class == ssx_registry::AuthClass::Required
                })
                && policy
                    .catalog
                    .get(&outcome.engine)
                    .is_some_and(|capability| !capability.auth_ready);
            coverage.insert(
                outcome.engine.clone(),
                ssx_domain::research::classify_coverage(
                    outcome.engine.clone(),
                    true,
                    Some(&outcome.status),
                    outcome.result_count.max(0) as u64,
                    missing,
                ),
            );
        }
        for exclusion in &response.scope.excluded_engines {
            coverage.insert(
                exclusion.engine.clone(),
                ssx_domain::research::classify_coverage(
                    exclusion.engine.clone(),
                    false,
                    None,
                    0,
                    false,
                ),
            );
        }
        let mut ordered = Vec::new();
        for name in query.engines.iter().chain(
            response
                .scope
                .excluded_engines
                .iter()
                .map(|exclusion| &exclusion.engine),
        ) {
            if !ordered
                .iter()
                .any(|entry: &ssx_domain::research::EngineCoverage| &entry.engine == name)
            {
                ordered.push(coverage.remove(name).unwrap_or_else(|| {
                    ssx_domain::research::classify_coverage(name.clone(), false, None, 0, false)
                }));
            }
        }
        Ok(ordered)
    }
    /// Synchronous delayed-publication gate for an authoritative store callback.
    /// Returns no mutable policy and never dispatches optional providers.
    pub fn revalidate_research_scope(&self, authority: &ValidatedScope) -> Result<(), SearchError> {
        self.current_policy(self.0.deps.clock.capture().0)?
            .with_revalidated(authority, |_| ())
            .map_err(SearchError::Policy)
    }
    /// Resolve source default-strategy metadata through the current shared policy.
    /// Returned names are proposals, never authority; callers must authorize each
    /// nonempty query before persistence and execution.
    pub fn research_template_scope(
        &self,
        intent: &str,
    ) -> Result<(Vec<String>, Vec<String>), SearchError> {
        if self.0.closed.load(Ordering::Acquire) {
            return Err(SearchError::Closed);
        }
        self.current_policy(self.0.deps.clock.capture().0)?
            .research_template_scope(intent)
            .map_err(SearchError::Policy)
    }
    /// Pure caller-directed planning admission. Uses the same current policy
    /// authority, without optional routing providers, cache writes or dispatch.
    /// Research previews and execution admission must not execute Jev advice.
    pub fn prepare_research_plan(
        &self,
        proposal: &ScopeProposal,
    ) -> Result<ValidatedScope, SearchError> {
        if self.0.closed.load(Ordering::Acquire) {
            return Err(SearchError::Closed);
        }
        self.current_policy(self.0.deps.clock.capture().0)?
            .authorize(proposal.clone())
            .map_err(SearchError::Policy)
    }
    /// Reuse this opaque authority after a transport's enforcement preview; the
    /// dispatch path revalidates it but never repeats optional routing disclosure.
    pub async fn prepare_scope(
        &self,
        proposal: &ScopeProposal,
    ) -> Result<ValidatedScope, SearchError> {
        self.prepare_scope_with_cancellation(proposal, SearchCancellation::default())
            .await
    }
    pub async fn prepare_scope_with_cancellation(
        &self,
        proposal: &ScopeProposal,
        cancellation: SearchCancellation,
    ) -> Result<ValidatedScope, SearchError> {
        if self.0.closed.load(Ordering::Acquire) {
            return Err(SearchError::Closed);
        }
        let _caller = CallerGuard {
            permit: Some(
                self.0
                    .callers
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| SearchError::Busy)?,
            ),
            done: self.0.callers_done.clone(),
        };
        let policy = self.current_policy(self.0.deps.clock.capture().0)?;
        self.authorize_proposal(proposal, &policy, &mut cancellation.0.subscribe())
            .await
    }
    /// Read-only scope preview using the same live policy and optional routing
    /// boundary as dispatch. It never returns or constructs dispatch authority.
    pub async fn preview_scope(
        &self,
        proposal: &ScopeProposal,
    ) -> Result<ScopePreview, SearchError> {
        self.preview_scope_with_cancellation(proposal, SearchCancellation::default())
            .await
    }
    pub async fn preview_scope_with_cancellation(
        &self,
        proposal: &ScopeProposal,
        cancellation: SearchCancellation,
    ) -> Result<ScopePreview, SearchError> {
        if self.0.closed.load(Ordering::Acquire) {
            return Err(SearchError::Closed);
        }
        let _caller = CallerGuard {
            permit: Some(
                self.0
                    .callers
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| SearchError::Busy)?,
            ),
            done: self.0.callers_done.clone(),
        };
        let policy = self.current_policy(self.0.deps.clock.capture().0)?;
        let advice = crate::jev::prepare_optional(
            self.0.deps.jev.as_deref(),
            proposal,
            &policy,
            self.0.deps.routing_cache.as_ref(),
        );
        let mut stop = self.0.stop.subscribe();
        let mut cancel = cancellation.0.subscribe();
        tokio::select! {
            biased;
            _ = cancelled(&mut stop) => Err(SearchError::Closed),
            _ = cancelled(&mut cancel) => Err(SearchError::Cancelled),
            prepared = tokio::time::timeout(self.0.config.optional_port_timeout, advice) => match prepared {
                Ok(result) => result.map(|prepared| prepared.preview).map_err(SearchError::Policy),
                Err(_) => policy.preview(proposal).map_err(SearchError::Policy),
            }
        }
    }
    async fn authorize_proposal(
        &self,
        proposal: &ScopeProposal,
        policy: &PolicyService,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<ValidatedScope, SearchError> {
        let advice = crate::jev::prepare_optional(
            self.0.deps.jev.as_deref(),
            proposal,
            policy,
            self.0.deps.routing_cache.as_ref(),
        );
        let mut stop = self.0.stop.subscribe();
        tokio::select! {
            biased;
            _ = cancelled(&mut stop) => Err(SearchError::Closed),
            _ = cancelled(cancel) => Err(SearchError::Cancelled),
            prepared = tokio::time::timeout(self.0.config.optional_port_timeout, advice) => match prepared {
                Ok(result) => { let prepared = result.map_err(SearchError::Policy)?; classify_availability(policy, &prepared.preview)?; prepared.authorize(policy).map_err(SearchError::Policy) },
                Err(_) => { let preview = policy.preview(proposal).map_err(SearchError::Policy)?; classify_availability(policy, &preview)?; policy.authorize(proposal.clone()).map_err(SearchError::Policy) },
            }
        }
    }
    pub async fn search_prepared(
        &self,
        authority: ValidatedScope,
        cancellation: SearchCancellation,
    ) -> Result<SearchResponse, SearchError> {
        Ok(self
            .capture_inner(None, Some(authority), cancellation)
            .await?
            .present())
    }
    pub async fn search_snapshot(
        &self,
        proposal: ScopeProposal,
        snapshots: &ssx_application::snapshot::SnapshotService,
        parents: &[ssx_domain::ArtifactRef],
    ) -> Result<CapturedSearch, SearchError> {
        self.search_snapshot_with_cancellation(
            proposal,
            snapshots,
            parents,
            SearchCancellation::default(),
        )
        .await
    }
    /// Cancellation joins the owned search flight. Snapshot persistence may have
    /// committed before cancellation; unknown completion never proves rollback.
    pub async fn search_snapshot_with_cancellation(
        &self,
        proposal: ScopeProposal,
        snapshots: &ssx_application::snapshot::SnapshotService,
        parents: &[ssx_domain::ArtifactRef],
        cancellation: SearchCancellation,
    ) -> Result<CapturedSearch, SearchError> {
        let capture = self.capture_read(proposal, cancellation.clone()).await?;
        self.snapshot_read_capture(capture, snapshots, parents, cancellation)
            .await
    }
    /// Consume this service's admitted capture and persist its complete immutable
    /// evidence without resolving policy, charging quota or executing again.
    pub async fn snapshot_read_capture(
        &self,
        capture: SearchReadCapture,
        snapshots: &ssx_application::snapshot::SnapshotService,
        parents: &[ssx_domain::ArtifactRef],
        cancellation: SearchCancellation,
    ) -> Result<CapturedSearch, SearchError> {
        if !std::sync::Weak::ptr_eq(&capture.service, &Arc::downgrade(&self.0)) {
            return Err(SearchError::InvalidConfiguration);
        }
        let canonical = CanonicalResponse::new(capture.full_personalized());
        let snapshot = if let Ok(permit) = self.0.callers.clone().try_acquire_owned() {
            let _caller = CallerGuard {
                permit: Some(permit),
                done: self.0.callers_done.clone(),
            };
            let mut stop = self.0.stop.subscribe();
            let mut cancel = cancellation.0.subscribe();
            tokio::select! {
                biased;
                _ = cancelled(&mut cancel) => Err(ssx_application::snapshot::SnapshotFailure::CompletionUnknown),
                _ = cancelled(&mut stop) => Err(ssx_application::snapshot::SnapshotFailure::CompletionUnknown),
                result = tokio::time::timeout(self.0.config.optional_port_timeout, snapshots.create(&canonical, parents)) => result.unwrap_or(Err(ssx_application::snapshot::SnapshotFailure::CompletionUnknown)),
            }
        } else {
            Err(ssx_application::snapshot::SnapshotFailure::CompletionUnknown)
        };
        let response = capture.present();
        Ok(CapturedSearch {
            canonical,
            response,
            snapshot,
        })
    }
    pub async fn search(&self, proposal: ScopeProposal) -> Result<SearchResponse, SearchError> {
        self.search_with_cancellation(proposal, SearchCancellation::default())
            .await
    }
    /// Explicit caller cancellation joins work when this was the final waiter.
    /// Dropping a Rust future signals cancellation via RAII; shutdown also joins.
    pub async fn search_with_cancellation(
        &self,
        proposal: ScopeProposal,
        cancellation: SearchCancellation,
    ) -> Result<SearchResponse, SearchError> {
        Ok(self.capture_read(proposal, cancellation).await?.present())
    }
    pub async fn capture_read(
        &self,
        proposal: ScopeProposal,
        cancellation: SearchCancellation,
    ) -> Result<SearchReadCapture, SearchError> {
        self.capture_inner(Some(proposal), None, cancellation).await
    }
    pub async fn capture_prepared(
        &self,
        authority: ValidatedScope,
        cancellation: SearchCancellation,
    ) -> Result<SearchReadCapture, SearchError> {
        self.capture_inner(None, Some(authority), cancellation)
            .await
    }
    async fn capture_inner(
        &self,
        proposal: Option<ScopeProposal>,
        prepared: Option<ValidatedScope>,
        cancellation: SearchCancellation,
    ) -> Result<SearchReadCapture, SearchError> {
        let start = Instant::now();
        if *cancellation.0.borrow() {
            return Err(SearchError::Cancelled);
        }
        if self.0.closed.load(Ordering::Acquire) {
            return Err(SearchError::Closed);
        }
        // No task is created for excess callers; queued futures have a finite bound.
        let _caller = CallerGuard {
            permit: Some(
                self.0
                    .callers
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| SearchError::Busy)?,
            ),
            done: self.0.callers_done.clone(),
        };
        let (now, calendar_date) = self.0.deps.clock.capture();
        if ssx_application::filters::publication_day(&calendar_date).is_none() {
            return Err(SearchError::InvalidConfiguration);
        }
        let policy = self.current_policy(now)?;
        let mut preflight_cancel = cancellation.0.subscribe();
        let mut stop = self.0.stop.subscribe();
        let authority = match prepared {
            Some(authority) => authority,
            None => {
                self.authorize_proposal(
                    proposal.as_ref().ok_or(SearchError::InvalidConfiguration)?,
                    &policy,
                    &mut preflight_cancel,
                )
                .await?
            }
        };
        let preview = policy
            .revalidate_preview(&authority)
            .map_err(SearchError::Policy)?;
        classify_availability(&policy, &preview)?;
        let (mut request, scope) = policy
            .with_revalidated(&authority, |command| {
                (command.request().clone(), command.preview().scope.clone())
            })
            .map_err(SearchError::Policy)?;
        // The trusted calendar anchor is resolved once, independent of caller input.
        request.time_range_anchor = request
            .time_range
            .as_deref()
            .and_then(ssx_application::filters::time_range_days)
            .map(|_| calendar_date);
        if scope.selected_engines.len() > self.0.config.max_selected_engines
            || scope
                .selected_engines
                .iter()
                .any(|n| !self.0.deps.engines.contains_key(n))
        {
            return Err(SearchError::InvalidConfiguration);
        }
        if let (Some(admission), Some(client)) =
            (&self.0.deps.client_admission, &request.client_identifier)
            && !client.is_empty()
        {
            let allowed = tokio::select! { biased; _ = cancelled(&mut stop) => return Err(SearchError::Closed), _ = cancelled(&mut preflight_cancel) => return Err(SearchError::Cancelled), allowed = tokio::time::timeout(self.0.config.optional_port_timeout, admission.acquire(client)) => allowed.unwrap_or(false) };
            if !allowed {
                return Err(SearchError::ClientLimited);
            }
        }
        let mut random = [0; 6];
        aws_lc_rs::rand::fill(&mut random).map_err(|_| SearchError::InvalidConfiguration)?;
        let query_id: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let key = cache_key(&self.0, &request, &policy, &scope);
        let fresh = request.freshness == "prefer_fresh";
        let expected_date_proof = publication_date_proof(&request, &scope)?;
        let cache_categories = request_categories(&request).to_vec();
        if let Some(cache) = &self.0.deps.cache {
            let entry = tokio::select! { biased; _ = cancelled(&mut stop) => return Err(SearchError::Closed), _ = cancelled(&mut preflight_cancel) => return Err(SearchError::Cancelled), entry = tokio::time::timeout(self.0.config.optional_port_timeout, cache.read(&key, fresh)) => entry.ok().flatten() };
            let cache_observation = if fresh {
                None
            } else {
                Some(match &entry {
                    None => ssx_application::CacheObservation::Miss,
                    Some(CacheEntry::Negative { .. }) => {
                        ssx_application::CacheObservation::Negative
                    }
                    Some(CacheEntry::Canonical(_)) => ssx_application::CacheObservation::Hit,
                })
            };
            let entry = entry.filter(|entry| {
                expected_date_proof.is_none()
                    || match entry {
                        CacheEntry::Canonical(canonical) => {
                            canonical.response().date_filter == expected_date_proof
                                && canonical.response().scope.selected_engines
                                    == scope.selected_engines
                        }
                        CacheEntry::Negative { .. } => false,
                    }
            });
            if let Some(observation) = cache_observation
                && (observation != ssx_application::CacheObservation::Hit || entry.is_some())
            {
                self.0.deps.metrics.cache_lookup(observation);
            }
            if let Some(entry) = entry {
                let context = ssx_application::cache::CachedViewContext {
                    query: request.query.clone(),
                    query_id: ssx_domain::QueryId::new(query_id.clone())
                        .map_err(|_| SearchError::InvalidConfiguration)?,
                    ranking_explanation: ranking_explanation(&self.0.config.ranking_strategy)
                        .into(),
                    include: request.include.clone(),
                    max_results: request.max_results.and_then(|v| i64::try_from(v).ok()),
                };
                let canonical = entry.view(context).canonical().clone();
                return Ok(SearchReadCapture::new(
                    self, canonical, request, scope, query_id, start, true,
                ));
            }
        }
        let identities: Vec<_> = scope
            .selected_engines
            .iter()
            .map(|n| (n, &self.0.deps.engines[n].generation))
            .collect();
        let flight_key = ssx_json::to_string(&(
            key.as_str(),
            identities,
            &scope.jev_scores,
            &request.execution_isolation_key,
            fresh.then_some(&query_id),
        ))
        .expect("closed identity primitives");
        let flight = {
            let mut flights = self.0.flights.lock().unwrap();
            flights.retain(|_, f| {
                *f.waiters.lock().unwrap() != 0
                    || !f
                        .task
                        .lock()
                        .unwrap()
                        .as_ref()
                        .is_some_and(JoinHandle::is_finished)
            });
            if self.0.closed.load(Ordering::Acquire) {
                return Err(SearchError::Closed);
            }
            if let Some(flight) = flights.get(&flight_key) {
                let mut waiters = flight.waiters.lock().unwrap();
                if *flight.cancel.borrow() {
                    return Err(SearchError::Busy);
                }
                *waiters += 1;
                drop(waiters);
                flight.clone()
            } else {
                if flights.len() >= self.0.config.max_flights {
                    return Err(SearchError::Busy);
                }
                let flight = Arc::new(Flight {
                    waiters: Mutex::new(1),
                    cancel: watch::channel(false).0,
                    result: watch::channel(None).0,
                    task: Mutex::new(None),
                    done: watch::channel(false).0,
                });
                let owner = self.0.clone();
                let worker = flight.clone();
                let req = request.clone();
                let selected = scope.clone();
                let task = tokio::spawn(async move {
                    let _completion = Completion(worker.done.clone());
                    owner.deps.supervisor.observed(TaskEvent {
                        task: "search_flight",
                        state: TaskState::Started,
                    });
                    let result = execute(
                        &owner,
                        req,
                        selected,
                        owner.deadline_clock.now(),
                        worker.cancel.subscribe(),
                    )
                    .await;
                    if let Ok((canonical, admission)) = &result
                        && let Some(cache) = &owner.deps.cache
                    {
                        let write = cache.write(&key, canonical, &cache_categories, *admission);
                        // Optional persistence cannot outlive cancellation indefinitely.
                        let mut cancel = worker.cancel.subscribe();
                        tokio::select! { _ = tokio::time::timeout(owner.config.optional_port_timeout, write) => {}, _ = cancelled(&mut cancel) => {} }
                    }
                    owner.deps.supervisor.observed(TaskEvent {
                        task: "search_flight",
                        state: if matches!(result, Err(SearchError::Cancelled)) {
                            TaskState::Cancelled
                        } else if result.is_ok() {
                            TaskState::Completed
                        } else {
                            TaskState::Failed
                        },
                    });
                    worker
                        .result
                        .send_replace(Some(result.map(|(canonical, _)| canonical)));
                });
                *flight.task.lock().unwrap() = Some(task);
                flights.insert(flight_key, flight.clone());
                flight
            }
        };
        let waiter = Waiter(flight.clone());
        let mut rx = flight.result.subscribe();
        let mut done = flight.done.subscribe();
        let mut caller_cancel = cancellation.0.subscribe();
        let canonical = loop {
            if let Some(result) = rx.borrow_and_update().clone() {
                break result?;
            }
            tokio::select! {
                _ = cancelled(&mut caller_cancel) => {
                    drop(waiter);
                    if *flight.cancel.borrow() { cancelled(&mut done).await; }
                    return Err(SearchError::Cancelled);
                }
                _ = cancelled(&mut done) => { if rx.borrow().is_none() { return Err(SearchError::Cancelled); } }
                result = rx.changed() => { result.map_err(|_| SearchError::Cancelled)?; }
            }
        };
        let capture =
            SearchReadCapture::new(self, canonical, request, scope, query_id, start, false);
        self.0.deps.audit.query_completed(
            &capture.request,
            capture.canonical.response(),
            capture.elapsed_ms,
        );
        self.0.deps.audit.record("search_query");
        drop(waiter);
        Ok(capture)
    }
    /// Cancels all owned flights and joins their engine/suggestion descendants.
    /// Concurrent/repeated shutdown is safe; transport owners close after joins.
    pub async fn shutdown(&self) {
        self.0.closed.store(true, Ordering::Release);
        self.0.callers.close();
        self.0.stop.send_replace(true);
        let flights: Vec<_> = self.0.flights.lock().unwrap().values().cloned().collect();
        for flight in &flights {
            flight.cancel.send_replace(true);
        }
        for flight in flights {
            let task = flight.task.lock().unwrap().take();
            if let Some(task) = task {
                let _ = task.await;
            } else {
                cancelled(&mut flight.done.subscribe()).await;
            }
        }
        loop {
            let settled = self.0.callers_done.notified();
            if self.0.callers.available_permits() == self.0.config.max_callers {
                break;
            }
            settled.await;
        }
        if let Some(s) = &self.0.deps.suggestions {
            s.shutdown().await;
        }
        if let Some(j) = &self.0.deps.jev {
            j.shutdown().await;
        }
        if let Some(stage) = &self.0.deps.reranker {
            stage.shutdown().await;
        }
        for http in &self.0.deps.owned_http {
            http.shutdown_and_wait().await;
        }
        for store in &self.0.deps.owned_stores {
            store.close().await;
        }
    }
}
fn request_categories(request: &SearchRequest) -> &[String] {
    request.categories.as_deref().unwrap_or(&[])
}
fn classify_availability(
    policy: &PolicyService,
    preview: &ScopePreview,
) -> Result<(), SearchError> {
    let reason = if preview.scope.selected_engines.is_empty() {
        Some(UnavailableReason::NoEligibleEngines)
    } else if preview.scope.selected_engines.iter().any(|name| {
        policy
            .catalog
            .get(name)
            .is_some_and(|engine| !engine.available)
    }) {
        Some(UnavailableReason::NativeImplementationUnavailable)
    } else {
        None
    };
    match reason {
        Some(reason) => Err(SearchError::Unavailable(SearchUnavailable {
            preview: Box::new(preview.clone()),
            reason,
        })),
        None => Ok(()),
    }
}
fn cache_key(
    inner: &Inner,
    request: &SearchRequest,
    policy: &PolicyService,
    scope: &ScopeDecision,
) -> ssx_application::cache::SearchCacheKey {
    let window = request
        .time_range
        .as_deref()
        .and_then(ssx_application::filters::time_range_days)
        .and_then(|days| {
            request
                .time_range_anchor
                .as_deref()
                .and_then(ssx_application::filters::publication_day)
                .and_then(|day| ssx_application::iso_timestamp((day - days) as f64 * 86400.0).ok())
                .map(|v| {
                    (
                        v[..10].to_owned(),
                        request.time_range_anchor.clone().unwrap(),
                    )
                })
        });
    let mut routing = inner.deps.routing_identity.clone();
    let generations: Vec<_> = inner
        .deps
        .engines
        .iter()
        .map(|(name, engine)| (name, &engine.generation))
        .collect();
    let with_health = policy
        .resolver
        .budget
        .as_ref()
        .is_some_and(|b| b.max_engines > 0);
    let eligibility: Vec<_> = policy
        .catalog
        .engines()
        .iter()
        .map(|e| {
            (
                &e.name,
                e.active,
                e.enabled,
                e.auth_ready,
                e.circuit_open,
                &e.cost_class,
                with_health.then_some((&e.health_status, e.health_stale)),
            )
        })
        .collect();
    let jev = inner.deps.jev.as_ref().map(|j| j.config().cache_identity());
    routing.routing_extension = Some(
        ssx_json::to_string(&(
            &routing.routing_extension,
            generations,
            eligibility,
            &scope.selected_engines,
            &scope.jev_scores,
            jev,
            &request.page,
        ))
        .expect("logical identity"),
    );
    composed_key(
        &CacheKeyInput {
            query: request.query.clone(),
            subreddit: request.subreddit.clone(),
            language: request.language.clone(),
            safesearch: request.safesearch.into(),
            categories: request.categories.clone().unwrap_or_default(),
            engines: request.engines.clone().unwrap_or_default(),
            page: request.page.as_number().clone(),
            time_range: request.time_range.clone(),
            media_type: request.media_type.clone(),
            date_from: request.date_from.clone(),
            date_to: request.date_to.clone(),
            relative_window: window,
        },
        &routing,
        request.interactive_timeout_ms,
        request.generate_suggestions,
    )
}
async fn cancelled(rx: &mut watch::Receiver<bool>) {
    loop {
        if *rx.borrow_and_update() {
            return;
        }
        if rx.changed().await.is_err() {
            return;
        }
    }
}
fn synthetic(status: EngineStatus, message: &str) -> AdapterResponse {
    AdapterResponse {
        status,
        synthetic: true,
        error_message: Some(message.into()),
        ..Default::default()
    }
}
#[cfg(test)]
#[derive(Default)]
struct CollectionPause {
    entered: Notify,
    release: Notify,
    tasks: Mutex<Vec<tokio::task::AbortHandle>>,
    hold_completion: Mutex<Option<usize>>,
    completion_entered: Notify,
    completion_release: Notify,
    cancelled_joins: std::sync::atomic::AtomicUsize,
}
struct Finished(usize, AdapterResponse, bool);
impl Finished {
    fn undispatched(
        index: usize,
        response: AdapterResponse,
        service_deadline: Instant,
        observed: Instant,
    ) -> Self {
        Self(index, response, observed >= service_deadline)
    }

    #[cfg(test)]
    fn before_dispatch_deadline(index: usize, deadline: Instant) -> Option<Self> {
        Self::before_dispatch_deadline_at(index, deadline, Instant::now())
    }
    fn before_dispatch_deadline_at(
        index: usize,
        deadline: Instant,
        observed: Instant,
    ) -> Option<Self> {
        (observed >= deadline).then(|| {
            Self(
                index,
                synthetic(
                    EngineStatus::Unavailable,
                    "not started before the search deadline",
                ),
                true,
            )
        })
    }
    #[cfg(test)]
    fn dispatched(
        index: usize,
        response: AdapterResponse,
        execution_deadline: Instant,
        service_deadline: Instant,
        binding_timeout: Duration,
    ) -> Self {
        Self::dispatched_at(
            index,
            response,
            execution_deadline,
            service_deadline,
            binding_timeout,
            Instant::now(),
        )
    }
    fn dispatched_at(
        index: usize,
        mut response: AdapterResponse,
        execution_deadline: Instant,
        service_deadline: Instant,
        binding_timeout: Duration,
        completed_at: Instant,
    ) -> Self {
        // Completion time, not collection time, determines origin. Service wins ties.
        let service_expired = completed_at >= service_deadline;
        if !service_expired && completed_at >= execution_deadline {
            response = timeout_response(binding_timeout);
        }
        Self(index, response, service_expired)
    }

    fn collect(self, responses: &mut [Option<AdapterResponse>], truncated: &mut bool) {
        let Self(index, response, service_expired) = self;
        if service_expired {
            *truncated = true;
        } else {
            responses[index] = Some(response);
        }
    }
}

fn collect_engine_join(
    result: Result<(tokio::task::Id, Finished), tokio::task::JoinError>,
    owner_aborted: bool,
    task_ids: &HashMap<tokio::task::Id, usize>,
    responses: &mut [Option<AdapterResponse>],
    truncated: &mut bool,
    internal_failures: &mut std::collections::BTreeSet<usize>,
) {
    match result {
        Ok((_, finished)) => finished.collect(responses, truncated),
        Err(error) if owner_aborted && error.is_cancelled() => {}
        Err(error) => {
            if let Some(index) = task_ids.get(&error.id()) {
                internal_failures.insert(*index);
                responses[*index] = Some(synthetic(
                    EngineStatus::Error,
                    "internal adapter task failure",
                ));
            }
        }
    }
}

fn resolve_outcome(
    response: Option<AdapterResponse>,
    started: bool,
    interactive: bool,
    budget: Duration,
    internal_failure: bool,
) -> (AdapterResponse, Option<OutcomeOrigin>) {
    let result = response.unwrap_or_else(|| {
        if started && !interactive {
            timeout_response(budget)
        } else {
            let mut response = synthetic(
                EngineStatus::Unavailable,
                if interactive {
                    "interactive deadline reached"
                } else {
                    "not started before the search deadline"
                },
            );
            if started {
                response.latency_ms =
                    ssx_domain::FiniteF64::new(budget.as_secs_f64() * 1000.0).unwrap();
            }
            response
        }
    });
    let origin = if internal_failure {
        None
    } else {
        Some(if !started {
            OutcomeOrigin::Undispatched
        } else if result.synthetic && interactive && result.status == EngineStatus::Unavailable {
            OutcomeOrigin::CallerCancelled
        } else if result.synthetic && result.status == EngineStatus::Timeout {
            OutcomeOrigin::DispatchedSyntheticTimeout
        } else {
            OutcomeOrigin::AdapterReturned
        })
    };
    (result, origin)
}

fn rerank_scope_and_results_public(
    names: &[String],
    results: &[ssx_domain::SearchResult],
    sensitive: &std::collections::BTreeSet<String>,
) -> bool {
    !names.iter().any(|n| sensitive.contains(n))
        && !results.iter().any(|r| {
            sensitive.contains(&r.engine) || r.engines.iter().any(|n| sensitive.contains(n))
        })
}

async fn execute(
    inner: &Arc<Inner>,
    mut request: SearchRequest,
    scope: ScopeDecision,
    start: Instant,
    mut cancel: watch::Receiver<bool>,
) -> Result<(CanonicalResponse, CacheAdmission), SearchError> {
    let date_filter = publication_date_proof(&request, &scope)?;
    let mut names = Vec::new();
    for name in &scope.selected_engines {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    let timeouts: Vec<_> = names
        .iter()
        .map(|n| inner.deps.engines[n].timeout)
        .collect();
    let budget = Duration::from_secs(10)
        .max(timeouts.iter().copied().max().unwrap_or_default())
        .max(
            timeouts
                .iter()
                .copied()
                .fold(Duration::ZERO, Duration::saturating_add)
                .min(Duration::from_secs(30)),
        );
    let interactive = request.interactive_timeout_ms.is_some();
    let budget = request
        .interactive_timeout_ms
        .map_or(budget, |ms| budget.min(Duration::from_millis(ms)));
    let deadline = start
        .checked_add(budget)
        .ok_or(SearchError::InvalidConfiguration)?;
    // Media intent determines adapter endpoint even with an explicit text category.
    if let Some(media) = request.media_type.as_deref() {
        request.categories = Some(vec![
            if media == "image" { "images" } else { "videos" }.into(),
        ]);
    } else if request.categories.as_ref().is_none_or(Vec::is_empty) {
        request.categories = Some(vec!["general".into()]);
    }
    let request = Arc::new(request);
    let started = Arc::new(Mutex::new(vec![false; names.len()]));
    let mut tasks = JoinSet::new();
    let mut task_ids = HashMap::new();
    #[cfg(test)]
    let collection_pause = inner.collection_pause.lock().unwrap().clone();
    for (index, name) in names.iter().enumerate() {
        let binding = inner.deps.engines[name].clone();
        let semaphore = inner.engines.clone();
        let request = request.clone();
        let started = started.clone();
        let owner = inner.clone();
        let name = name.clone();
        #[cfg(test)]
        let task_pause = collection_pause.clone();
        let task = tasks.spawn(async move {
            if let Some(expired) =
                Finished::before_dispatch_deadline_at(index, deadline, owner.deadline_clock.now())
            {
                return expired;
            }
            if let Some(cooldowns) = &owner.deps.cooldowns
                && let Ok(key) = ssx_domain::rate::EngineKey::new(&name)
                && let Ok(ssx_domain::rate::SharedCooldown::Active { seconds }) =
                    cooldowns.cooldown(&key).await
                && seconds > 0
            {
                return Finished::undispatched(
                    index,
                    synthetic(
                        EngineStatus::RateLimited,
                        &format!("upstream cooldown active; retry in {seconds}s"),
                    ),
                    deadline,
                    owner.deadline_clock.now(),
                );
            }
            let Ok(_permit) = semaphore.acquire_owned().await else {
                return Finished::undispatched(
                    index,
                    synthetic(EngineStatus::Unavailable, "service closed"),
                    deadline,
                    owner.deadline_clock.now(),
                );
            };
            if let Some(expired) =
                Finished::before_dispatch_deadline_at(index, deadline, owner.deadline_clock.now())
            {
                return expired;
            }
            if owner
                .health
                .lock()
                .unwrap()
                .get(&name)
                .is_some_and(|h| !h.circuit_allowed(owner.deps.clock.capture().0))
            {
                return Finished::undispatched(
                    index,
                    synthetic(EngineStatus::Error, "circuit open"),
                    deadline,
                    owner.deadline_clock.now(),
                );
            }
            started.lock().unwrap()[index] = true;
            // Overflow adds no tighter binding limit to the checked service bound.
            let engine_deadline = owner.deadline_clock.now().checked_add(binding.timeout);
            let execution_deadline = engine_deadline.map_or(deadline, |d| d.min(deadline));
            let response = owner
                .deadline_clock
                .timeout_at(
                    execution_deadline,
                    binding.engine.search_with_context(
                        &request,
                        SearchContext {
                            deadline: execution_deadline.into_std(),
                        },
                    ),
                )
                .await
                .unwrap_or_else(|| timeout_response(binding.timeout));
            // Test scheduling seam: retain an actually owned task until the real
            // owner aborts it; do not alter deadline selection or classification.
            #[cfg(test)]
            if let Some(pause) = &task_pause {
                let hold = *pause.hold_completion.lock().unwrap() == Some(index);
                if hold {
                    pause.completion_entered.notify_one();
                    pause.completion_release.notified().await;
                }
            }
            Finished::dispatched_at(
                index,
                response,
                execution_deadline,
                deadline,
                binding.timeout,
                owner.deadline_clock.now(),
            )
        });
        task_ids.insert(task.id(), index);
        #[cfg(test)]
        if let Some(pause) = &collection_pause {
            pause.tasks.lock().unwrap().push(task);
        }
    }
    let mut suggestion_tasks = JoinSet::new();
    if request.generate_suggestions
        && inner.deadline_clock.now() < deadline
        && let Some(suggestions) = inner.deps.suggestions.clone()
    {
        let query = request.query.clone();
        let timeout = inner.config.optional_port_timeout;
        suggestion_tasks.spawn(async move {
            tokio::time::timeout(timeout, suggestions.fetch(&query))
                .await
                .unwrap_or_default()
        });
    }
    #[cfg(test)]
    if let Some(pause) = &collection_pause {
        pause.entered.notify_one();
        pause.release.notified().await;
    }
    let mut responses: Vec<Option<AdapterResponse>> = vec![None; names.len()];
    // Unexpected JoinError is a service defect; owner-requested cancellation is not.
    let mut internal_failures = std::collections::BTreeSet::new();
    let mut truncated = false;
    let mut was_cancelled = false;
    while !tasks.is_empty() {
        tokio::select! {
            biased;
            _ = cancelled(&mut cancel) => { was_cancelled = true; break; }
            _ = inner.deadline_clock.sleep_until(deadline) => { break; }
            result = tasks.join_next_with_id() => {
                if let Some(result) = result {
                    collect_engine_join(result, false, &task_ids, &mut responses, &mut truncated, &mut internal_failures);
                }
            }
        }
    }
    // Abort does not discard already completed outputs. Join every owned task,
    // retaining its completion-time decision and waiting for cancellation cleanup.
    tasks.abort_all();
    while let Some(result) = tasks.join_next_with_id().await {
        #[cfg(test)]
        if result
            .as_ref()
            .is_err_and(tokio::task::JoinError::is_cancelled)
            && let Some(pause) = &collection_pause
        {
            pause.cancelled_joins.fetch_add(1, Ordering::SeqCst);
        }
        collect_engine_join(
            result,
            true,
            &task_ids,
            &mut responses,
            &mut truncated,
            &mut internal_failures,
        );
    }
    truncated |= responses.iter().any(Option::is_none);
    if was_cancelled {
        suggestion_tasks.shutdown().await;
        return Err(SearchError::Cancelled);
    }
    let now = inner.deps.clock.capture().0;
    let started = started.lock().unwrap().clone();
    let mut feeds = Feeds::new();
    let mut response = SearchResponse {
        query: request.query.clone(),
        date_filter,
        scope: scope.clone(),
        deadline_exceeded: interactive && truncated,
        dispatched_engine_count: started.iter().filter(|v| **v).count() as i64,
        ranking_explanation: ranking_explanation(&inner.config.ranking_strategy).into(),
        ..Default::default()
    };
    let cooldown = |i: usize| {
        responses[i]
            .as_ref()
            .is_some_and(|r| r.synthetic && r.status == EngineStatus::RateLimited && !started[i])
    };
    let circuit = |i: usize| {
        responses[i].as_ref().is_some_and(|r| {
            r.synthetic && r.error_message.as_deref() == Some("circuit open") && !started[i]
        })
    };
    let mut outcome_order: Vec<_> = (0..names.len())
        .filter(|i| !cooldown(*i) && !circuit(*i))
        .collect();
    outcome_order.extend((0..names.len()).filter(|i| cooldown(*i)));
    outcome_order.extend((0..names.len()).filter(|i| circuit(*i)));
    for i in outcome_order {
        let name = &names[i];
        let (mut result, origin) = resolve_outcome(
            responses[i].take(),
            started[i],
            interactive,
            budget,
            internal_failures.contains(&i),
        );
        match origin {
            None => {
                inner
                    .deps
                    .metrics
                    .increment("search_engine_internal_failure");
                inner.deps.metrics.increment("search_engine_synthetic");
            }
            Some(origin) => {
                inner
                    .health
                    .lock()
                    .unwrap()
                    .entry(name.clone())
                    .or_default()
                    .observe(origin, &result, now);
                inner
                    .deps
                    .metrics
                    .increment(if matches!(origin, OutcomeOrigin::AdapterReturned) {
                        "search_engine_observed"
                    } else {
                        "search_engine_synthetic"
                    });
            }
        }
        for row in &mut result.results {
            row.tier = if inner.deps.policy.resolver.tier1.contains(name) {
                1
            } else {
                2
            };
        }
        if let Some(range) = &request.time_range
            && inner.deps.policy.catalog.get(name).is_some_and(|e| {
                e.enforced_filters
                    .get("time_range")
                    .is_some_and(|v| v == "local")
            })
            && let Some(day) = request
                .time_range_anchor
                .as_deref()
                .and_then(ssx_application::filters::publication_day)
        {
            result.results =
                ssx_application::filters::filter_results_by_time_range(&result.results, range, day);
        }
        if inner.config.empty_scrape_diagnostics
            && inner.deps.engines[name].scrape
            && result.status == EngineStatus::Ok
            && result.results.is_empty()
        {
            response.empty_engines.push(vec![
                name.clone(),
                "successful scrape returned no results".into(),
            ]);
        }
        if let Some(origin) = origin {
            inner.deps.metrics.engine_outcome(name, &result, origin);
        }
        response.engine_outcomes.push(EngineOutcome {
            engine: name.clone(),
            status: status(&result.status).into(),
            result_count: result.results.len() as i64,
            latency_ms: Some(
                ssx_domain::FiniteF64::new(if (result.latency_ms.get() * 10.0).is_finite() {
                    (result.latency_ms.get() * 10.0).round_ties_even() / 10.0
                } else {
                    result.latency_ms.get()
                })
                .unwrap(),
            ),
            message: result.error_message,
        });
        response.answers.extend(result.answers);
        response.corrections.extend(result.corrections);
        response.infoboxes.extend(result.infoboxes);
        feeds.insert(name.clone(), result.results);
    }
    response.all_unresponsive = response.engine_outcomes.iter().all(|r| r.status != "ok");
    response.partial = !response.all_unresponsive
        && (response.deadline_exceeded
            || response.engine_outcomes.iter().any(|r| r.status != "ok"));
    response.results = rank(
        &group_publications(&feeds, &request.query),
        &inner.config.ranking_strategy,
        &BTreeMap::new(),
    );
    for row in &mut response.results {
        if let Some(representative) = row
            .work_group
            .as_ref()
            .and_then(|g| g.get("representative_engine"))
            .and_then(ssx_json::Value::as_str)
        {
            row.engine = representative.into();
        }
    }
    let mut rerank_state = RerankState::Skipped;
    let sensitive = &inner.deps.policy.policy.sensitive_engines;
    if let Some(reranker) = &inner.deps.reranker {
        if response.results.len() > 1
            && rerank_scope_and_results_public(&names, &response.results, sensitive)
        {
            let rerank_deadline = if interactive {
                deadline.min(inner.deadline_clock.now() + Duration::from_secs(1))
            } else {
                inner.deadline_clock.now() + Duration::from_secs(1)
            };
            if rerank_deadline > inner.deadline_clock.now() {
                let mut rerank_tasks = JoinSet::new();
                let stage = reranker.clone();
                let stage_request = request.clone();
                let stage_results = response.results.clone();
                rerank_tasks.spawn(async move {
                    stage
                        .rerank(&stage_request, &stage_results, rerank_deadline)
                        .await
                });
                let decision = tokio::select! { _ = cancelled(&mut cancel) => { rerank_tasks.shutdown().await; suggestion_tasks.shutdown().await; return Err(SearchError::Cancelled); }, decision = rerank_tasks.join_next() => decision, _ = inner.deadline_clock.sleep_until(rerank_deadline) => None };
                rerank_tasks.shutdown().await;
                match decision {
                    Some(Ok(Ok(decision))) => {
                        rerank_state = decision.state;
                        response.results = decision.results;
                    }
                    _ => rerank_state = RerankState::Fallback,
                }
            }
        }
    } else if !scope.jev_added_engines.is_empty() {
        response.results = promote_specialists(response.results, &feeds, &scope);
    }
    if rerank_state == RerankState::Applied {
        response.ranking_explanation = "semantic_shortlist_rerank".into();
        for (i, row) in response.results.iter_mut().enumerate() {
            row.position = i as i64 + 1;
        }
    }
    if !suggestion_tasks.is_empty() {
        tokio::select! {
            _ = cancelled(&mut cancel) => { suggestion_tasks.shutdown().await; return Err(SearchError::Cancelled); }
            result = suggestion_tasks.join_next() => { if let Some(Ok(suggestions)) = result { response.suggestions = suggestions; } }
            _ = inner.deadline_clock.sleep_until(deadline), if interactive => { response.deadline_exceeded = true; }
        }
    }
    suggestion_tasks.shutdown().await;
    response.partial = !response.all_unresponsive
        && (response.deadline_exceeded
            || response.engine_outcomes.iter().any(|r| r.status != "ok"));
    let admission = if response.deadline_exceeded {
        CacheAdmission::DeadlineTruncated
    } else if rerank_state == RerankState::Fallback {
        CacheAdmission::RerankFallback
    } else if response.all_unresponsive {
        CacheAdmission::AllUnresponsive
    } else if response.partial {
        CacheAdmission::Partial
    } else {
        CacheAdmission::Complete
    };
    Ok((CanonicalResponse::new(response), admission))
}
fn status(status: &EngineStatus) -> &'static str {
    match status {
        EngineStatus::Ok => "ok",
        EngineStatus::RateLimited => "rate_limited",
        EngineStatus::Blocked => "blocked",
        EngineStatus::Error => "error",
        EngineStatus::Timeout => "timeout",
        EngineStatus::Unavailable => "unavailable",
    }
}
fn publication_date_proof(
    request: &SearchRequest,
    scope: &ScopeDecision,
) -> Result<Option<ssx_domain::PublicationDateProof>, SearchError> {
    if request.date_from.is_none() && request.date_to.is_none() {
        return Ok(None);
    }
    if scope.selected_engines != ["openalex"] {
        return Err(SearchError::InvalidConfiguration);
    }
    ssx_domain::PublicationDateProof::openalex(request.date_from.clone(), request.date_to.clone())
        .map(Some)
        .map_err(|_| SearchError::InvalidConfiguration)
}
fn timeout_response(duration: Duration) -> AdapterResponse {
    let mut seconds = duration.as_secs_f64().to_string();
    if !seconds.contains('.') {
        seconds.push_str(".0");
    }
    AdapterResponse {
        status: EngineStatus::Timeout,
        synthetic: true,
        error_message: Some(format!("timed out after {seconds}s")),
        latency_ms: ssx_domain::FiniteF64::new(duration.as_secs_f64() * 1000.0)
            .expect("finite timeout"),
        ..Default::default()
    }
}
fn promote_specialists(
    ranked: Vec<ssx_domain::SearchResult>,
    feeds: &Feeds,
    scope: &ScopeDecision,
) -> Vec<ssx_domain::SearchResult> {
    let mut ordered = scope.jev_added_engines.clone();
    ordered.sort_by(|a, b| {
        scope
            .jev_scores
            .get(b)
            .map_or(0.0, |v| v.get())
            .total_cmp(&scope.jev_scores.get(a).map_or(0.0, |v| v.get()))
            .then_with(|| a.cmp(b))
    });
    let mut seen = std::collections::BTreeSet::new();
    let mut promoted = Vec::new();
    for name in ordered {
        if let Some(rows) = feeds.get(&name) {
            for row in rows {
                let marker = if row.url.is_empty() {
                    format!("{name}:{}", row.title)
                } else {
                    row.url.clone()
                };
                if seen.insert(marker) {
                    promoted.push(row.clone());
                    break;
                }
            }
        }
    }
    for row in ranked {
        let marker = if row.url.is_empty() {
            format!("merged:{}", row.title)
        } else {
            row.url.clone()
        };
        if seen.insert(marker) {
            promoted.push(row);
        }
    }
    promoted
}

/// Native server-authoritative per-client fixed-window admission. A digest keeps
/// raw client identities out of a dedicated, provider-disjoint F10 namespace.
pub struct DistributedClientAdmission {
    pub quota: Arc<crate::valkey::rate::DistributedQuota>,
    pub fail_closed: bool,
}
impl ClientAdmission for DistributedClientAdmission {
    fn acquire<'a>(&'a self, client: &'a str) -> PortFuture<'a, bool> {
        Box::pin(async move {
            let Ok(key) = crate::valkey::rate::ClientQuotaKey::new(client) else {
                return false;
            };
            match self.quota.acquire_client(&key, 1).await {
                Ok(receipt) => receipt.allowed,
                Err(
                    ssx_domain::rate::RateFailure::Closed
                    | ssx_domain::rate::RateFailure::InvalidInput,
                ) => false,
                Err(_) => !self.fail_closed,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssx_application::{
        NoopSink,
        policy::SearchPolicy,
        scope::{CatalogSnapshot, ScopeResolver},
    };
    use std::sync::atomic::AtomicUsize;
    struct NeverPolled(Arc<AtomicUsize>);
    impl SearchEngine for NeverPolled {
        fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { AdapterResponse::default() })
        }
    }
    #[tokio::test(start_paused = true)]
    async fn cooperative_completion_collection_preserves_real_health_origin() {
        // Each case invokes the production completion, collection, fallback and
        // origin functions directly, bypassing the outer select intentionally.
        for (service_ms, binding_ms, completed_ms, interactive, expected_origin) in [
            (100, 200, 100, true, OutcomeOrigin::CallerCancelled),
            (100, 200, 101, true, OutcomeOrigin::CallerCancelled),
            (100, 100, 100, true, OutcomeOrigin::CallerCancelled),
            (100, 100, 101, true, OutcomeOrigin::CallerCancelled),
            (
                100,
                100,
                100,
                false,
                OutcomeOrigin::DispatchedSyntheticTimeout,
            ),
            (
                200,
                100,
                100,
                true,
                OutcomeOrigin::DispatchedSyntheticTimeout,
            ),
            (
                200,
                100,
                101,
                true,
                OutcomeOrigin::DispatchedSyntheticTimeout,
            ),
            (200, 100, 200, true, OutcomeOrigin::CallerCancelled),
            (200, 100, 50, true, OutcomeOrigin::AdapterReturned),
        ] {
            let start = Instant::now();
            let service_deadline = start + Duration::from_millis(service_ms);
            let binding_deadline = start + Duration::from_millis(binding_ms);
            tokio::time::advance(Duration::from_millis(completed_ms)).await;
            let finished = Finished::dispatched(
                0,
                AdapterResponse {
                    status: EngineStatus::Timeout,
                    latency_ms: ssx_domain::FiniteF64::new(7.0).unwrap(),
                    ..Default::default()
                },
                binding_deadline.min(service_deadline),
                service_deadline,
                Duration::from_millis(binding_ms),
            );
            // Delayed collection must not reclassify a genuine early response.
            tokio::time::advance(Duration::from_secs(1)).await;
            let mut responses = vec![None];
            let mut truncated = false;
            finished.collect(&mut responses, &mut truncated);
            let service_expired = completed_ms >= service_ms;
            assert_eq!(truncated, service_expired);
            assert_eq!(responses[0].is_none(), service_expired);
            let (response, origin) = resolve_outcome(
                responses[0].take(),
                true,
                interactive,
                Duration::from_millis(service_ms),
                false,
            );
            assert_eq!(origin, Some(expected_origin));
            let mut health = HealthState::default();
            let observed_at = SystemSearchClock.capture().0;
            health.observe(origin.unwrap(), &response, observed_at);
            let value = ssx_application::health::build_engine_health(
                Some(&health),
                None,
                observed_at,
                300.0,
            )
            .unwrap();
            let caller = expected_origin == OutcomeOrigin::CallerCancelled;
            assert_eq!(
                value["status"],
                ssx_json::json!(if caller { "unknown" } else { "timeout" })
            );
            assert_eq!(
                value["circuit_consecutive_errors"],
                ssx_json::json!(u64::from(!caller))
            );
            assert_eq!(
                value["last_observed_latency_ms"],
                if expected_origin == OutcomeOrigin::AdapterReturned {
                    ssx_json::json!(7.0)
                } else {
                    ssx_json::Value::Null
                }
            );
            if caller {
                assert_eq!(value["status_at"], ssx_json::Value::Null);
            } else {
                let recorded =
                    chrono::DateTime::parse_from_rfc3339(value["status_at"].as_str().unwrap())
                        .unwrap();
                assert!(
                    (recorded.timestamp_micros() as f64 / 1_000_000.0 - observed_at.seconds())
                        .abs()
                        <= 0.000_001
                );
            }
        }
    }

    #[test]
    fn current_policy_source_is_construction_only_and_preserves_default_generation() {
        struct Current(std::sync::RwLock<PolicyService>);
        impl CurrentPolicy for Current {
            fn current(&self) -> PolicyService {
                self.0.read().unwrap().clone()
            }
        }
        let policy = PolicyService {
            policy: SearchPolicy::default(),
            catalog: CatalogSnapshot::new(vec![]).unwrap(),
            resolver: ScopeResolver::default(),
        };
        let current = Arc::new(Current(std::sync::RwLock::new(policy.clone())));
        let service = SearchService::new(
            SearchConfig::default(),
            SearchDependencies {
                policy: policy.clone(),
                engines: BTreeMap::new(),
                routing_identity: RoutingIdentity {
                    catalog: None,
                    sensitive: Default::default(),
                    tier1: Default::default(),
                    budget: None,
                    ranking_strategy: "presence".into(),
                    work_group_version: "scholarly-work-v3".into(),
                    rerank_identity: None,
                    routing_extension: None,
                },
                cache: None,
                client_admission: None,
                suggestions: None,
                jev: None,
                routing_cache: None,
                metrics: Arc::new(NoopSink),
                supervisor: Arc::new(NoopSink),
                audit: Arc::new(NoopSink),
                cooldowns: None,
                owned_http: vec![],
                owned_stores: vec![],
                clock: Arc::new(SystemSearchClock),
                reranker: None,
            },
        )
        .unwrap();
        assert_eq!(
            format!(
                "{:?}",
                service
                    .current_policy(WallClockTimestamp::new(1.0).unwrap())
                    .unwrap()
            ),
            format!("{policy:?}")
        );
        let service = service.with_policy_source(current.clone()).unwrap();
        current.0.write().unwrap().policy.sensitive_allowed = true;
        assert!(
            service
                .current_policy(WallClockTimestamp::new(1.0).unwrap())
                .unwrap()
                .policy
                .sensitive_allowed
        );
        assert!(!service.0.deps.policy.policy.sensitive_allowed);
        let clone = service.clone();
        assert!(matches!(
            service.with_policy_source(current),
            Err(SearchError::InvalidConfiguration)
        ));
        drop(clone);
    }

    #[tokio::test]
    async fn zero_remaining_budget_never_polls_engine() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = SearchService::new(
            SearchConfig::default(),
            SearchDependencies {
                policy: PolicyService {
                    policy: SearchPolicy::default(),
                    catalog: CatalogSnapshot::new(vec![]).unwrap(),
                    resolver: ScopeResolver::default(),
                },
                engines: BTreeMap::from([(
                    "brave".into(),
                    EngineBinding {
                        engine: Arc::new(NeverPolled(calls.clone())),
                        generation: "zero-budget".into(),
                        timeout: Duration::from_secs(3),
                        scrape: false,
                    },
                )]),
                routing_identity: RoutingIdentity {
                    catalog: None,
                    sensitive: Default::default(),
                    tier1: Default::default(),
                    budget: None,
                    ranking_strategy: "presence".into(),
                    work_group_version: "scholarly-work-v3".into(),
                    rerank_identity: None,
                    routing_extension: None,
                },
                cache: None,
                client_admission: None,
                suggestions: None,
                jev: None,
                routing_cache: None,
                metrics: Arc::new(NoopSink),
                supervisor: Arc::new(NoopSink),
                audit: Arc::new(NoopSink),
                cooldowns: None,
                owned_http: vec![],
                owned_stores: vec![],
                clock: Arc::new(SystemSearchClock),
                reranker: None,
            },
        )
        .unwrap();
        let (_sender, receiver) = watch::channel(false);
        // This is an internal elapsed-execution-budget seam, not a transport input.
        let (response, admission) = execute(
            &service.0,
            SearchRequest {
                query: "zero".into(),
                interactive_timeout_ms: Some(1),
                ..Default::default()
            },
            ScopeDecision {
                selected_engines: vec!["brave".into()],
                ..Default::default()
            },
            Instant::now() - Duration::from_secs(1),
            receiver,
        )
        .await
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(response.response().dispatched_engine_count, 0);
        assert!(response.response().deadline_exceeded);
        assert_eq!(admission, CacheAdmission::DeadlineTruncated);
        service.shutdown().await;
    }
}

#[cfg(test)]
mod f15_sensitive_tests {
    use super::*;
    #[test]
    fn sensitive_post_rank_engine_and_provenance_are_excluded() {
        let sensitive =
            std::collections::BTreeSet::from(["hibp".to_owned(), "dehashed".to_owned()]);
        let selected = vec!["brave".to_owned()];
        let public = ssx_domain::SearchResult {
            engine: "brave".into(),
            ..Default::default()
        };
        assert!(rerank_scope_and_results_public(
            &selected,
            std::slice::from_ref(&public),
            &sensitive
        ));
        assert!(!rerank_scope_and_results_public(
            &["hibp".into()],
            std::slice::from_ref(&public),
            &sensitive
        ));
        let mut row = public.clone();
        row.engine = "hibp".into();
        assert!(!rerank_scope_and_results_public(
            &selected,
            &[row],
            &sensitive
        ));
        let mut row = public;
        row.engines.insert("dehashed".into());
        assert!(!rerank_scope_and_results_public(
            &selected,
            &[row],
            &sensitive
        ));
    }
}

#[cfg(test)]
mod deadline1010_tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn expired_undispatched_service_deadline_keeps_interactive_origin() {
        let start = Instant::now();
        let budget = Duration::from_millis(1);
        let deadline = start + budget;
        assert!(Finished::before_dispatch_deadline(0, deadline).is_none());
        tokio::time::advance(budget).await;
        let observed = Instant::now();
        assert_eq!(observed, deadline);
        let expired = Finished::before_dispatch_deadline(0, deadline)
            .expect("actual original deadline reached");
        let mut responses = vec![None];
        let mut truncated = false;
        expired.collect(&mut responses, &mut truncated);
        assert!(
            truncated,
            "expired undispatched work must carry service-deadline origin independently of timer collection"
        );
        assert!(responses[0].is_none());
        let (response, origin) = resolve_outcome(responses[0].take(), false, true, budget, false);
        assert!(response.synthetic);
        assert_eq!(response.status, EngineStatus::Unavailable);
        assert_eq!(
            response.error_message.as_deref(),
            Some("interactive deadline reached")
        );
        assert_eq!(response.latency_ms.get(), 0.0);
        assert_eq!(origin, Some(OutcomeOrigin::Undispatched));
        assert!(response.results.is_empty());
        println!("T06_1010_COMPONENT_COMPLETE expired-undispatched-deadline-origin");
    }
}

#[cfg(test)]
#[path = "search_deadline_drain_tests.rs"]
mod deadline_drain_tests;
