#![allow(dead_code)] // Reused qualification helpers intentionally expose more than each target uses.
//! Qualification-only engines and wiring. Never selected by a public server.
#[path = "deadline_clock.rs"]
mod deadline_clock;
use ssx_application::{
    NoopSink, PortFuture, SearchEngine,
    cache::*,
    policy::{PolicyService, SearchPolicy},
    scope::*,
    snapshot::*,
};
use ssx_domain::*;
use ssx_infra::{
    search::*,
    valkey::{Valkey, cache::ValkeyCache, snapshot::*},
};
use ssx_mcp::search_surface::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
#[derive(Clone)]
pub struct FakeEngine {
    pub name: String,
    pub calls: Arc<AtomicUsize>,
    pub dispatch_receipt: Option<Arc<Valkey>>,
    pub count: usize,
    pub status: EngineStatus,
    pub delay: Duration,
    pub results: Option<Vec<SearchResult>>,
}
impl SearchEngine for FakeEngine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            let dispatch = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if let Some(store) = &self.dispatch_receipt {
                store
                    .put(
                        &format!("t06:qualification:dispatch:{}:{dispatch}", self.name),
                        b"1",
                        std::num::NonZeroU64::new(3600).unwrap(),
                    )
                    .await
                    .expect("private dispatch receipt write");
            }
            if !self.delay.is_zero() {
                tokio::time::sleep(self.delay).await;
            }
            if self.status != EngineStatus::Ok {
                return AdapterResponse {
                    status: self.status.clone(),
                    error_message: Some("simulated engine failure".into()),
                    latency_ms: FiniteF64::new(1.0).unwrap(),
                    ..Default::default()
                };
            }
            let results = self.results.clone().unwrap_or_else(|| {
                (0..self.count)
                    .map(|i| SearchResult {
                        url: format!("https://{}{i}.example", self.name),
                        title: format!("{} result {i}", self.name),
                        content: format!("Content for {} result {i}.", self.name),
                        engine: self.name.clone(),
                        engines: BTreeSet::from([self.name.clone()]),
                        score: FiniteF64::new((self.count - i) as f64).unwrap(),
                        position: (i + 1) as i64,
                        category: "general".into(),
                        tier: 1,
                        ..Default::default()
                    })
                    .collect()
            });
            AdapterResponse {
                results,
                status: EngineStatus::Ok,
                latency_ms: FiniteF64::new(2.0).unwrap(),
                ..Default::default()
            }
        })
    }
}
#[derive(Clone)]
pub struct FixedCatalog(pub CatalogSnapshot);
impl AdvisoryCatalog for FixedCatalog {
    fn snapshot(&self) -> CatalogSnapshot {
        self.0.clone()
    }
}
struct Diagnostics(RuntimeDiagnostics);
impl RuntimeDiagnosticsState for Diagnostics {
    fn snapshot(&self) -> RuntimeDiagnostics {
        self.0.clone()
    }
}
pub struct Fixture {
    pub surface: SearchSurface,
    pub service: SearchService,
    pub engines: BTreeMap<String, FakeEngine>,
    pub store: Arc<Valkey>,
    pub policy: PolicyService,
    pub registry: Arc<ssx_registry::Registry>,
}
impl Fixture {
    pub async fn new(url: &str, grants: bool) -> Self {
        Self::with_engines(
            url,
            grants,
            ["wikipedia", "brave", "duckduckgo"]
                .into_iter()
                .map(|name| FakeEngine {
                    name: name.into(),
                    calls: Default::default(),
                    dispatch_receipt: None,
                    count: 3,
                    status: EngineStatus::Ok,
                    delay: Duration::ZERO,
                    results: None,
                })
                .collect(),
        )
        .await
    }
    pub async fn with_engines(url: &str, grants: bool, engines: Vec<FakeEngine>) -> Self {
        Self::with_policy(
            url,
            SearchPolicy {
                grants: if grants {
                    ["jobs", "security", "science"]
                        .into_iter()
                        .map(str::to_owned)
                        .collect()
                } else {
                    BTreeSet::new()
                },
                ..Default::default()
            },
            engines,
            &[],
        )
        .await
    }
    pub async fn with_policy(
        url: &str,
        search_policy: SearchPolicy,
        engines: Vec<FakeEngine>,
        disabled: &[&str],
    ) -> Self {
        Self::with_options(
            url,
            search_policy,
            engines,
            disabled,
            SearchConfig::default(),
            None,
        )
        .await
    }
    pub async fn with_options(
        url: &str,
        search_policy: SearchPolicy,
        engines: Vec<FakeEngine>,
        disabled: &[&str],
        search_config: SearchConfig,
        jev: Option<Arc<ssx_infra::jev::JevRouter>>,
    ) -> Self {
        Self::with_options_and_clock(
            url,
            search_policy,
            engines,
            disabled,
            search_config,
            jev,
            None,
            false,
        )
        .await
    }
    /// Same fixed fake-engine work and declarations as new(); only execution time
    /// is manually scheduled. No public server/configuration imports this method.
    pub async fn frozen(url: &str, grants: bool) -> Self {
        let engines = ["wikipedia", "brave", "duckduckgo"]
            .into_iter()
            .map(|name| FakeEngine {
                name: name.into(),
                calls: Default::default(),
                dispatch_receipt: None,
                count: 3,
                status: EngineStatus::Ok,
                delay: Duration::ZERO,
                results: None,
            })
            .collect();
        Self::with_options_and_clock(
            url,
            SearchPolicy {
                grants: if grants {
                    ["jobs", "security", "science"]
                        .into_iter()
                        .map(str::to_owned)
                        .collect()
                } else {
                    BTreeSet::new()
                },
                ..Default::default()
            },
            engines,
            &[],
            SearchConfig::default(),
            None,
            Some(deadline_clock::ManualDeadlineClock::frozen()),
            true,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)] // Explicit qualification construction, not caller input.
    async fn with_options_and_clock(
        url: &str,
        search_policy: SearchPolicy,
        engines: Vec<FakeEngine>,
        disabled: &[&str],
        search_config: SearchConfig,
        jev: Option<Arc<ssx_infra::jev::JevRouter>>,
        deadline_clock: Option<Arc<dyn SearchDeadlineClock>>,
        reference_catalog: bool,
    ) -> Self {
        let store = Arc::new(Valkey::new(url, Duration::from_secs(2)).unwrap());
        store.ping().await.unwrap();
        let mut configuration = ssx_config::load_config_with(
            Some(std::path::Path::new("/no/t06-fixture-config")),
            &BTreeMap::new(),
        )
        .unwrap();
        configuration
            .engines
            .retain(|name, _| engines.iter().any(|e| &e.name == name));
        for name in disabled {
            configuration
                .engines
                .entry((*name).into())
                .or_default()
                .enabled = false;
        }
        let config = Arc::new(configuration);
        let engines: BTreeMap<_, _> = engines.into_iter().map(|e| (e.name.clone(), e)).collect();
        // Only the frozen Python comparison uses its original engine universe.
        // Native/runtime fixture construction continues to expose native additions.
        let raw = if reference_catalog {
            ssx_registry::Registry::migration_baseline().unwrap()
        } else {
            ssx_registry::Registry::builtins().unwrap()
        };
        let mut descriptors: Vec<_> = raw.descriptors().cloned().collect();
        // The source fixture injects EngineAdapter subclasses whose declarations are
        // intentionally the base defaults, while static class names/types remain.
        for d in &mut descriptors {
            if engines.contains_key(&d.name) {
                d.categories = vec!["general".into()];
                d.supported_filters = [
                    "language",
                    "time_range",
                    "safesearch",
                    "pagination",
                    "date_from",
                    "date_to",
                ]
                .into_iter()
                .map(|k| (k.into(), false))
                .collect();
                d.enforced_filters.clear();
                d.cost_class = None;
                d.failure_classes = [
                    "rate_limited",
                    "blocked",
                    "error",
                    "timeout",
                    "auth_required",
                    "unavailable",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect();
                d.supported_result_types = vec!["text".into()];
                d.supported_media_types.clear();
            }
        }
        let registry = Arc::new(ssx_registry::Registry::from_descriptors(descriptors).unwrap());
        let mut caps: Vec<_> = registry
            .descriptors()
            .map(|d| {
                let (enabled, auth, ready, categories) =
                    ssx_config::capability_configured(d, &config);
                EngineCapability {
                    name: d.name.clone(),
                    enabled,
                    active: engines.contains_key(&d.name),
                    available: engines.contains_key(&d.name),
                    categories,
                    media_types: d.supported_media_types.clone(),
                    auth_ready: auth != ssx_registry::AuthClass::Required || ready,
                    circuit_open: false,
                    cost_class: d.cost_class.clone().unwrap_or_default(),
                    health_status: "unknown".into(),
                    health_stale: false,
                    enforced_filters: d.enforced_filters.clone(),
                }
            })
            .collect();
        caps.sort_by_key(|c| match c.name.as_str() {
            "wikipedia" => 0,
            "brave" => 1,
            "duckduckgo" => 2,
            _ => 3,
        });
        let catalog = Arc::new(FixedCatalog(CatalogSnapshot::new(caps).unwrap()));
        let policy = PolicyService {
            policy: search_policy,
            catalog: catalog.0.clone(),
            resolver: ScopeResolver {
                tier1: engines.keys().cloned().collect(),
                ..Default::default()
            },
        };
        let mut owned_stores = vec![store.clone()];
        for engine in engines.values() {
            if let Some(receipt) = &engine.dispatch_receipt
                && !owned_stores.iter().any(|s| Arc::ptr_eq(s, receipt))
            {
                owned_stores.push(receipt.clone());
            }
        }
        let dependencies = SearchDependencies {
            policy: policy.clone(),
            engines: engines
                .iter()
                .map(|(n, e)| {
                    (
                        n.clone(),
                        EngineBinding {
                            engine: Arc::new(e.clone()),
                            generation: "t06-source-fixture".into(),
                            timeout: Duration::from_secs(3),
                            scrape: false,
                        },
                    )
                })
                .collect(),
            routing_identity: RoutingIdentity {
                catalog: None,
                sensitive: policy.policy.sensitive_engines.clone(),
                tier1: policy.resolver.tier1.clone(),
                budget: None,
                ranking_strategy: "presence".into(),
                work_group_version: "scholarly-work-v3".into(),
                rerank_identity: None,
                routing_extension: None,
            },
            cache: Some(Arc::new(CanonicalCache::new(
                Arc::new(ValkeyCache::new(store.clone())),
                CacheTtls::default(),
            ))),
            client_admission: None,
            suggestions: None,
            jev,
            routing_cache: None,
            metrics: Arc::new(NoopSink),
            supervisor: Arc::new(NoopSink),
            audit: Arc::new(NoopSink),
            cooldowns: None,
            owned_http: vec![],
            owned_stores,
            clock: Arc::new(SystemSearchClock),
            reranker: None,
        };
        let service = SearchService::new_with_read_catalog(
            search_config,
            dependencies,
            registry.clone(),
            Some(catalog.clone()),
        )
        .unwrap();
        let service = match deadline_clock {
            Some(clock) => service
                .with_deadline_clock(clock)
                .expect("unshared fixture search owner"),
            None => service,
        };
        let diagnostics = Arc::new(Diagnostics(RuntimeDiagnostics {
            version: "0.0.0".into(),
            valkey_connected: false,
            valkey_fail_closed: false,
            cache_connected: true,
            snapshots_available: true,
            job_store_available: true,
            durable_research: false,
            worker_id: "fixture-worker".into(),
            job_lease_ttl_seconds: 60,
            job_poll_interval_seconds: FiniteF64::new(1.0).unwrap(),
            job_max_concurrent_jobs: 1,
            job_max_queries: 20,
            job_max_engines_per_query: 10,
            job_max_results: 500,
            job_default_deadline_seconds: 600,
            snapshot_ttl_seconds: 3600,
            max_results: 50,
            router_enabled: false,
            workflow_availability: [
                "research",
                "dependency_dossier",
                "staged_search",
                "saved_search",
                "retrieval_receipt",
            ]
            .into_iter()
            .map(|s| (s.into(), true))
            .collect(),
        }));
        let discovery = Arc::new(
            RuntimeDiscovery::new(
                registry.clone(),
                config,
                service.clone(),
                catalog,
                policy.policy.clone(),
                diagnostics,
                300.0,
            )
            .unwrap(),
        );
        let snapshots = SnapshotService::new(
            Arc::new(ValkeySnapshots::new(store.clone())),
            Arc::new(SystemSnapshotClock),
            Arc::new(RandomSnapshotIds),
            TenantId::new("default").unwrap(),
            SnapshotTtl::new(3600).unwrap(),
        );
        let surface = SearchSurface::new(
            SearchState::new(service.clone(), snapshots, discovery, 50).unwrap(),
        );
        Self {
            surface,
            service,
            engines,
            store,
            policy,
            registry,
        }
    }
    pub fn calls(&self) -> usize {
        self.engines
            .values()
            .map(|e| e.calls.load(Ordering::SeqCst))
            .sum()
    }
}
