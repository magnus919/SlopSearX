//! Real worker composition. No fixture runtime, synthetic registry or durable fallback.
use crate::{
    adapters,
    http::AdapterHttp,
    observability::{Options, Telemetry},
    research_runner::{ResearchRunner, RunnerConfig},
    search::{
        DistributedClientAdmission, EngineBinding, SearchDependencies, SearchService,
        SearchSettings, SystemSearchClock,
    },
    valkey::{
        Valkey,
        cache::ValkeyCache,
        rate::DistributedQuota,
        research::ResearchStore,
        snapshot::{RandomSnapshotIds, SystemSnapshotClock, ValkeySnapshots},
    },
};
use ssx_application::{
    AdmissionDecision, PortFuture, SearchAdmission,
    cache::{CacheTtls, CanonicalCache, RoutingEngineIdentity, RoutingIdentity},
    policy::{PolicyService, SearchPolicy},
    rate::LocalTokenBucket,
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
    snapshot::{SnapshotService, SnapshotTtl},
};
use ssx_config::Config;
use ssx_domain::{TenantId, rate::EngineKey, research::ResearchCeilings};
use ssx_registry::Registry;
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::watch;
mod factories;
mod settings;

/// Codes/field names only. Never include a URL, credential, query or arbitrary config value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub stage: &'static str,
    pub field: String,
}
impl Error {
    fn new(stage: &'static str, field: impl Into<String>) -> Self {
        Self {
            stage,
            field: field.into(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.stage, self.field)
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

/// Source default: 10 tokens/s, burst 30, three consecutive denials deactivate.
/// This is explicitly process-local admission, not durable state or a Valkey fallback.
struct LocalAdmission {
    started: Instant,
    state: Mutex<(LocalTokenBucket, BTreeMap<String, u8>)>,
}
impl LocalAdmission {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            state: Mutex::new((
                LocalTokenBucket::new(10.0, 30.0).expect("literal bucket bounds"),
                BTreeMap::new(),
            )),
        }
    }
}
impl SearchAdmission for LocalAdmission {
    fn admit<'a>(&'a self, engine: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move {
            let Ok(key) = EngineKey::new(engine) else {
                return AdmissionDecision::Unavailable;
            };
            let Ok(mut state) = self.state.lock() else {
                return AdmissionDecision::Unavailable;
            };
            if state.1.get(engine).copied().unwrap_or(0) >= 3 {
                return AdmissionDecision::Denied;
            }
            match state
                .0
                .acquire(&key, 1, self.started.elapsed().as_secs_f64())
            {
                Ok(true) => {
                    state.1.insert(engine.into(), 0);
                    AdmissionDecision::Allowed
                }
                Ok(false) => {
                    let n = state.1.entry(engine.into()).or_default();
                    *n = n.saturating_add(1);
                    AdmissionDecision::Denied
                }
                Err(_) => AdmissionDecision::Unavailable,
            }
        })
    }
}

/// Stable cache/flight identity, independent of process incarnation and lease ownership.
/// Secret bytes enter only the digest. They are never formatted, serialized or logged.
fn engine_generation(
    descriptor: &ssx_registry::EngineDescriptor,
    entry: Option<&ssx_config::EngineConfig>,
    settings: &SearchSettings,
) -> Result<String> {
    use sha2::{Digest, Sha256};
    fn framed(hash: &mut Sha256, value: &[u8]) {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value);
    }
    fn scalar(hash: &mut Sha256, value: &ssx_config::ConfigScalar) {
        use ssx_config::ConfigScalar;
        match value {
            ConfigScalar::Boolean(v) => {
                framed(hash, b"bool");
                framed(hash, &[u8::from(*v)]);
            }
            ConfigScalar::Integer(v) => {
                framed(hash, b"int");
                framed(hash, &v.to_be_bytes());
            }
            ConfigScalar::Float(v) => {
                framed(hash, b"float");
                framed(hash, &v.to_bits().to_be_bytes());
            }
            ConfigScalar::Text(v) => {
                framed(hash, b"text");
                framed(hash, v.as_bytes());
            }
        }
    }
    fn optional(hash: &mut Sha256, value: Option<&str>) {
        match value {
            Some(v) => {
                framed(hash, b"some");
                framed(hash, v.as_bytes());
            }
            None => framed(hash, b"none"),
        }
    }
    let mut hash = Sha256::new();
    framed(&mut hash, b"ssx-engine-generation:composition-v2:4bc0e618");
    framed(
        &mut hash,
        &serde_json::to_vec(descriptor)
            .map_err(|_| Error::new("registry", "descriptor_identity"))?,
    );
    framed(&mut hash, &[u8::from(settings.brave_category_routing)]);
    match entry {
        None => framed(&mut hash, b"adapter-defaults"),
        Some(e) => {
            framed(&mut hash, b"resolved-entry");
            framed(&mut hash, &[u8::from(e.enabled)]);
            framed(&mut hash, e.base_url.as_bytes());
            framed(&mut hash, e.engine_type.as_bytes());
            scalar(&mut hash, &e.timeout_ms);
            scalar(&mut hash, &e.max_results);
            scalar(&mut hash, &e.weight);
            match &e.rate_limit {
                Some(v) => {
                    framed(&mut hash, b"rate");
                    scalar(&mut hash, v);
                }
                None => framed(&mut hash, b"no-rate"),
            }
            optional(&mut hash, e.api_key.expose());
            optional(&mut hash, e.api_secret.expose());
            optional(&mut hash, e.user_agent.as_deref());
            optional(&mut hash, e.proxy_pool.as_deref());
            optional(&mut hash, e.scrape_proxy_url.as_deref());
            match &e.categories {
                Some(v) => {
                    framed(&mut hash, b"categories");
                    framed(&mut hash, &(v.len() as u64).to_be_bytes());
                    for s in v {
                        framed(&mut hash, s.as_bytes());
                    }
                }
                None => framed(&mut hash, b"default-categories"),
            }
            framed(&mut hash, b"extra-settings");
            framed(
                &mut hash,
                &(e.extra_env_settings.len() as u64).to_be_bytes(),
            );
            for (key, value) in &e.extra_env_settings {
                framed(&mut hash, key.as_bytes());
                framed(&mut hash, value.as_bytes());
            }
        }
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Resolved once from a single captured environment and a single file read.
/// No Debug/Serialize: this object owns credentials.
pub struct Resolved {
    config: Config,
    policy: settings::WorkerPolicy,
    settings: SearchSettings,
    observability: settings::ObservabilitySettings,
    registry: Arc<Registry>,
    catalog: CatalogSnapshot,
    http: Arc<AdapterHttp>,
    valkey_url: String,
    ttls: CacheTtls,
    timeouts: BTreeMap<String, Duration>,
    generations: BTreeMap<String, String>,
}
impl Resolved {
    pub fn catalog(&self) -> &CatalogSnapshot {
        &self.catalog
    }
    pub fn resolve(path: Option<&Path>, env: &BTreeMap<String, String>) -> Result<Self> {
        let entries: Vec<_> = env
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        Self::resolve_entries(path, &entries)
    }
    /// Production entry: retain OS iteration order for F06 case aliases. Exact-key
    /// consumers still use Python-like last value lookup after that projection.
    pub fn resolve_entries(path: Option<&Path>, entries: &[(String, String)]) -> Result<Self> {
        let env: &BTreeMap<String, String> = &entries.iter().cloned().collect();
        let (config, section, observability_section) =
            ssx_config::load_bundle_entries(path, entries)
                .map_err(|_| Error::new("config", "file_or_environment"))?;
        let policy = settings::WorkerPolicy::resolve(&section, env)?;
        let observability = settings::ObservabilitySettings::resolve(&observability_section, env)?;
        if !policy.search.grants.contains("research") {
            return Err(Error::new("policy", "research_grant"));
        }
        let valkey_url = env
            .get("VALKEY_URL")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| Error::new("config", "VALKEY_URL"))?
            .clone();
        redis::Client::open(valkey_url.as_str()).map_err(|_| Error::new("config", "VALKEY_URL"))?;
        let settings = SearchSettings::from_environment(env, &config.ranking.strategy)
            .map_err(|_| Error::new("config", "search_settings"))?;
        // Reject unsupported composition modes before resource acquisition or claims.
        if env.get("SENTRY_DSN").is_some_and(|v| !v.is_empty()) {
            return Err(Error::new("unsupported", "SENTRY_DSN"));
        }
        if config.enable_suggestions {
            return Err(Error::new("unsupported", "enable_suggestions"));
        }
        if env
            .get("TYPESAFE_API_KEY")
            .is_some_and(|v| !v.trim_matches(ssx_json::python_whitespace).is_empty())
        {
            return Err(Error::new("unsupported", "Jev_rerank_composition"));
        }
        if config.routing.topics.is_some()
            || config.routing.budget.is_some()
            || config.routing.fallback.is_some()
        {
            return Err(Error::new("unsupported", "custom_routing"));
        }
        let mut registry = Registry::builtins().map_err(|_| Error::new("registry", "catalog"))?;
        for name in config.engines.keys() {
            if registry.descriptor(name).is_none() {
                return Err(Error::new("registry", "unknown_configured_engine"));
            }
        }
        let http = Arc::new(AdapterHttp::default());
        factories::register(
            &mut registry,
            http.clone(),
            Arc::new(LocalAdmission::new()),
            &config,
            &settings,
        )?;
        let mut capabilities = Vec::new();
        let mut timeouts = BTreeMap::new();
        let mut generations = BTreeMap::new();
        for descriptor in registry.descriptors() {
            let (enabled, _auth, key, categories) =
                ssx_config::capability_configured(descriptor, &config);
            let available = registry.is_available(&descriptor.name);
            if enabled && !available {
                return Err(Error::new("unavailable_adapter", descriptor.name.clone()));
            }
            let required = policy.required.contains(&descriptor.name);
            let auth_ready = !required || key;
            if enabled {
                generations.insert(
                    descriptor.name.clone(),
                    engine_generation(descriptor, config.engines.get(&descriptor.name), &settings)?,
                );
                if let Some(e) = config.engines.get(&descriptor.name) {
                    if e.proxy_pool.is_some() || e.scrape_proxy_url.is_some() {
                        return Err(Error::new("unsupported", "engine_proxy"));
                    }
                    let ms = e
                        .timeout_ms
                        .effective_f64()
                        .map_err(|_| Error::new("config", "engine_timeout"))?;
                    let timeout = Duration::try_from_secs_f64(ms / 1000.0)
                        .map_err(|_| Error::new("config", "engine_timeout"))?;
                    if timeout.is_zero() || Instant::now().checked_add(timeout).is_none() {
                        return Err(Error::new("config", "engine_timeout"));
                    }
                    timeouts.insert(descriptor.name.clone(), timeout);
                } else {
                    timeouts.insert(
                        descriptor.name.clone(),
                        Duration::from_millis(
                            u64::try_from(descriptor.defaults.timeout_ms)
                                .map_err(|_| Error::new("config", "engine_timeout"))?,
                        ),
                    );
                }
            }
            capabilities.push(EngineCapability {
                name: descriptor.name.clone(),
                enabled,
                active: enabled && available,
                available,
                categories,
                media_types: descriptor.supported_media_types.clone(),
                auth_ready,
                circuit_open: false,
                cost_class: descriptor
                    .cost_class
                    .clone()
                    .unwrap_or_else(|| "free".into()),
                health_status: "unknown".into(),
                health_stale: true,
                enforced_filters: descriptor.enforced_filters.clone(),
            });
        }
        let catalog = CatalogSnapshot::new(capabilities)
            .map_err(|_| Error::new("registry", "capability_catalog"))?;
        if !catalog.engines().iter().any(|e| e.active && e.auth_ready) {
            return Err(Error::new("unavailable_adapter", "no_active_engines"));
        }
        let ttls = CacheTtls::from_text(
            env.get("SEARCH_CACHE_TTL_SECONDS")
                .map_or("3600", String::as_str),
            env.get("SEARCH_CACHE_PARTIAL_TTL_SECONDS")
                .map_or("30", String::as_str),
            env.get("SEARCH_CACHE_NEGATIVE_TTL_SECONDS")
                .map_or("60", String::as_str),
        )
        .map_err(|_| Error::new("config", "cache_ttl"))?;
        Ok(Self {
            config,
            policy,
            settings,
            observability,
            registry: Arc::new(registry),
            catalog,
            http,
            valkey_url,
            ttls,
            timeouts,
            generations,
        })
    }
    /// Once resources exist, every fallible constructor is inside the cleanup boundary.
    pub async fn open(self) -> Result<Runtime> {
        let (_keep_open, stop) = watch::channel(false);
        self.open_stoppable(stop).await
    }
    /// The caller keeps polling this owner through stop: no outer select drops a
    /// partially-owned constructor. Only the borrowed ping is cancellation-selected.
    pub async fn open_stoppable(self, mut stop: watch::Receiver<bool>) -> Result<Runtime> {
        if stop_requested(&stop) {
            return Err(Error::new("stopped", "startup"));
        }
        let store = Arc::new(
            Valkey::new(&self.valkey_url, Duration::from_secs(2))
                .map_err(|_| Error::new("store", "VALKEY_URL"))?,
        );
        let opened = self.open_with_store(store.clone(), &mut stop).await;
        if opened.is_err() {
            self.http.shutdown_and_wait().await;
            store.close().await;
        }
        opened
    }
    async fn open_with_store(
        &self,
        store: Arc<Valkey>,
        stop: &mut watch::Receiver<bool>,
    ) -> Result<Runtime> {
        tokio::select! {biased; _=wait_stop(stop)=>return Err(Error::new("stopped","startup")),result=store.ping()=>result.map_err(|_|Error::new("store","ping"))?}
        if stop_requested(stop) {
            return Err(Error::new("stopped", "startup"));
        }
        let mut bytes = [0; 16];
        aws_lc_rs::rand::fill(&mut bytes).map_err(|_| Error::new("runtime", "entropy"))?;
        let owner: String = bytes.iter().map(|v| format!("{v:02x}")).collect();
        let mut bindings = BTreeMap::new();
        for capability in self.catalog.engines().iter().filter(|e| e.active) {
            let engine = self
                .registry
                .instantiate(&capability.name)
                .ok_or_else(|| Error::new("registry", "factory_missing"))?;
            bindings.insert(
                capability.name.clone(),
                EngineBinding {
                    engine: Arc::from(engine),
                    generation: self
                        .generations
                        .get(&capability.name)
                        .cloned()
                        .ok_or_else(|| Error::new("config", "engine_generation"))?,
                    timeout: *self
                        .timeouts
                        .get(&capability.name)
                        .ok_or_else(|| Error::new("config", "engine_timeout"))?,
                    scrape: false,
                },
            );
        }
        let resolver = ScopeResolver {
            topics: if self.config.routing.enabled {
                Some(ssx_application::policy::default_topics())
            } else {
                None
            },
            sensitive: self.policy.search.sensitive_engines.clone(),
            ..ScopeResolver::default()
        };
        let routing_identity = RoutingIdentity {
            catalog: Some(
                self.catalog
                    .engines()
                    .iter()
                    .map(|e| RoutingEngineIdentity {
                        name: e.name.clone(),
                        active: e.active,
                        enabled: e.enabled,
                        auth_class: (if self.policy.required.contains(&e.name) {
                            "required"
                        } else if self
                            .config
                            .engines
                            .get(&e.name)
                            .is_some_and(|v| v.api_key.is_configured())
                        {
                            "optional"
                        } else if self.config.engines.contains_key(&e.name) {
                            "none"
                        } else {
                            "unknown"
                        })
                        .into(),
                        auth_configured: self
                            .config
                            .engines
                            .get(&e.name)
                            .is_some_and(|v| v.api_key.is_configured()),
                        cost_class: e.cost_class.clone(),
                        circuit_open: e.circuit_open,
                        observed_status: e.health_status.clone(),
                        observed_stale: e.health_stale,
                    })
                    .collect(),
            ),
            sensitive: self.policy.search.sensitive_engines.clone(),
            tier1: resolver.tier1.clone(),
            budget: None,
            ranking_strategy: self.config.ranking.strategy.clone(),
            work_group_version: "scholarly-work-v3".into(),
            rerank_identity: None,
            routing_extension: None,
        };
        let quota = Arc::new(DistributedQuota::new(
            store.clone(),
            self.settings.client_quota,
        ));
        let tenant =
            TenantId::new("worker-bootstrap").map_err(|_| Error::new("runtime", "tenant"))?;
        let snapshot_ttl = SnapshotTtl::new(self.policy.snapshot_ttl)
            .map_err(|_| Error::new("config", "snapshot_ttl"))?;
        let telemetry = Telemetry::new(
            self.observability.options(),
            Some(&self.valkey_url),
            bindings.keys().cloned().collect(),
            self.catalog
                .engines()
                .iter()
                .flat_map(|e| e.categories.clone())
                .collect(),
        )
        .map_err(|_| Error::new("runtime", "telemetry"))?;
        let deps = SearchDependencies {
            policy: PolicyService {
                policy: self.policy.search.clone(),
                catalog: self.catalog.clone(),
                resolver,
            },
            engines: bindings,
            routing_identity,
            cache: Some(Arc::new(CanonicalCache::new(
                Arc::new(ValkeyCache::new(store.clone())),
                self.ttls,
            ))),
            client_admission: Some(Arc::new(DistributedClientAdmission {
                quota,
                fail_closed: self.settings.client_fail_closed,
            })),
            suggestions: None,
            jev: None,
            routing_cache: None,
            metrics: telemetry.clone(),
            supervisor: telemetry.clone(),
            audit: telemetry.clone(),
            cooldowns: None,
            owned_http: vec![self.http.clone()],
            owned_stores: vec![],
            clock: Arc::new(SystemSearchClock),
            reranker: None,
        };
        let search = match SearchService::new_with_registry(
            self.settings.execution.clone(),
            deps,
            self.registry.clone(),
        ) {
            Ok(s) => s,
            Err(_) => {
                telemetry.shutdown().await;
                return Err(Error::new("runtime", "search"));
            }
        };
        let snapshots = SnapshotService::new(
            Arc::new(ValkeySnapshots::new(store.clone())),
            Arc::new(SystemSnapshotClock),
            Arc::new(RandomSnapshotIds),
            tenant.clone(),
            snapshot_ttl,
        );
        let jobs = ResearchStore::new(store.clone(), tenant);
        let runner = match ResearchRunner::new(
            jobs,
            search.clone(),
            snapshots,
            RunnerConfig {
                owner,
                lease_seconds: self.policy.lease,
                poll_interval: self.policy.poll,
                workers: self.policy.workers,
                ceilings: self.policy.ceilings,
            },
        ) {
            Ok(r) => r,
            Err(_) => {
                search.shutdown().await;
                telemetry.shutdown().await;
                return Err(Error::new("runtime", "research_runner"));
            }
        };
        let runtime = Runtime {
            search,
            runner,
            store,
            telemetry,
            workers: self.policy.workers.get(),
            poll: self.policy.poll,
        };
        if stop_requested(stop) {
            runtime.shutdown().await;
            return Err(Error::new("stopped", "startup"));
        }
        Ok(runtime)
    }
}
fn stop_requested(stop: &watch::Receiver<bool>) -> bool {
    *stop.borrow() || stop.has_changed().is_err()
}
async fn wait_stop(stop: &mut watch::Receiver<bool>) {
    while !stop_requested(stop) {
        if stop.changed().await.is_err() {
            break;
        }
    }
}
/// Caller must keep polling the owned run future through stop and join. Arbitrary
/// future-drop is not a joined-shutdown API. The executable follows this contract.
pub struct Runtime {
    search: SearchService,
    runner: ResearchRunner,
    store: Arc<Valkey>,
    telemetry: Arc<Telemetry>,
    workers: u64,
    poll: Duration,
}
impl Runtime {
    pub fn search(&self) -> &SearchService {
        &self.search
    }
    pub async fn run_research(
        self,
        mut stop: watch::Receiver<bool>,
        started: tokio::sync::oneshot::Sender<()>,
    ) -> Result<()> {
        let (cancel, receiver) = watch::channel(stop_requested(&stop));
        let (entered_tx, mut entered_rx) = tokio::sync::mpsc::channel(self.workers as usize);
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..self.workers {
            let runner = self.runner.clone();
            let mut stopped = receiver.clone();
            let poll = self.poll;
            let entered = entered_tx.clone();
            tasks.spawn(async move {
                if *stopped.borrow() {
                    return;
                }
                if entered.send(()).await.is_err() {
                    return;
                }
                drop(entered);
                loop {
                    if *stopped.borrow() {
                        break;
                    }
                    // The accepted runner owns claim/renewal/reservation/publication.
                    // Transient store/record errors leave durable authority recoverable.
                    if runner.run_once(stopped.clone()).await.is_err() && !*stopped.borrow() {
                        eprintln!("worker_iteration_unavailable role=research");
                    }
                    if *stopped.borrow() {
                        break;
                    }
                    tokio::select! {biased; _=stopped.changed()=>{}, _=tokio::time::sleep(poll)=>{}}
                }
            });
        }
        drop(entered_tx);
        let mut stopping = stop_requested(&stop);
        let mut failed = false;
        let mut entered = 0;
        // The acknowledgement comes from each actual task's first poll, not from
        // spawning a future or a synthetic readiness counter. Claims may follow
        // task entry; the handshake promises installed/entered workers, not IO success.
        while entered < self.workers && !stopping {
            tokio::select! {biased;
                _=wait_stop(&mut stop)=>{stopping=true;cancel.send_replace(true);},
                joined=tasks.join_next()=>{failed=joined.is_some();stopping=true;cancel.send_replace(true);},
                event=entered_rx.recv()=>{if event.is_some(){entered+=1;}else{failed=true;stopping=true;cancel.send_replace(true);}}
            }
        }
        if !stopping && !stop_requested(&stop) {
            eprintln!("worker_tasks_entered role=research");
            if started.send(()).is_err() {
                stopping = true;
                cancel.send_replace(true);
            }
        } else {
            stopping = true;
            cancel.send_replace(true);
        }

        while !tasks.is_empty() {
            tokio::select! {
                biased;
                _=stop.changed(), if !stopping=>{stopping=true;cancel.send_replace(true);},
                joined=tasks.join_next()=>{if joined.is_some_and(|v|v.is_err()) {failed=true;stopping=true;cancel.send_replace(true);}}
            }
        }
        self.search.shutdown().await;
        self.telemetry.shutdown().await;
        self.store.close().await;
        if failed {
            Err(Error::new("runtime", "worker_task"))
        } else {
            Ok(())
        }
    }
    pub async fn shutdown(self) {
        self.search.shutdown().await;
        self.telemetry.shutdown().await;
        self.store.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn local_admission_deactivates_after_three_denials() {
        let admission = LocalAdmission {
            started: Instant::now(),
            state: Mutex::new((LocalTokenBucket::new(0.0, 0.0).unwrap(), BTreeMap::new())),
        };
        for _ in 0..4 {
            assert_eq!(admission.admit("npm").await, AdmissionDecision::Denied);
        }
        assert_eq!(admission.state.lock().unwrap().1.get("npm"), Some(&3));
        assert_eq!(
            LocalAdmission::new().admit("crates").await,
            AdmissionDecision::Allowed
        );
    }
    #[test]
    fn engine_identity_is_stable_and_tracks_settings_and_credentials() {
        let registry = Registry::builtins().unwrap();
        let descriptor = registry.descriptor("npm").unwrap();
        let settings = SearchSettings::from_environment(&BTreeMap::new(), "presence").unwrap();
        let mut entry = ssx_config::EngineConfig::default();
        let first = engine_generation(descriptor, Some(&entry), &settings).unwrap();
        assert_eq!(
            first,
            engine_generation(descriptor, Some(&entry), &settings).unwrap()
        );
        entry.base_url = "http://127.0.0.1:9999/search".into();
        assert_ne!(
            first,
            engine_generation(descriptor, Some(&entry), &settings).unwrap()
        );
        let before = engine_generation(descriptor, Some(&entry), &settings).unwrap();
        entry.api_key = ssx_config::Secret::new(Some("fixture-secret-never-serialized".into()));
        let after = engine_generation(descriptor, Some(&entry), &settings).unwrap();
        assert_ne!(before, after);
        assert_eq!(after.len(), 64);
        assert!(!after.contains("fixture-secret"));
    }
}
