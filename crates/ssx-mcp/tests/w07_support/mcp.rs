#![allow(dead_code)] // Shared only by the evidence qualification process and transport tests.
//! One qualification-only search/evidence owner, never selected by public startup.
#[path = "../t06_support/fixture.rs"]
mod engine_fixture;
use ssx_application::{
    NoopSink,
    policy::{PolicyService, SearchPolicy},
    saved::SavedLimits,
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
    snapshot::{SnapshotService, SnapshotTtl},
    staged,
};
use ssx_domain::{EngineStatus, TenantId};
use ssx_infra::{
    search::{
        EngineBinding, RoutingIdentity, SearchClock, SearchConfig, SearchDependencies,
        SearchService, SystemSearchClock,
    },
    valkey::{
        Valkey,
        snapshot::{RandomSnapshotIds, SystemSnapshotClock, ValkeySnapshots},
    },
};
use ssx_json::{Value, json};
use ssx_mcp::{
    evidence::{
        authority::{EvidenceAuthority, EvidenceAuthoritySnapshot, SearchPolicyAdapter},
        factory::EvidenceFactory,
    },
    search_surface::{DiscoveryState, SearchState},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, RwLock, atomic::AtomicUsize},
    time::Duration,
};
pub struct Current(pub RwLock<EvidenceAuthoritySnapshot>);
impl EvidenceAuthority for Current {
    fn snapshot(&self) -> EvidenceAuthoritySnapshot {
        self.0.read().unwrap().clone()
    }
}
struct Discovery;
impl DiscoveryState for Discovery {
    fn capabilities(&self) -> Vec<Value> {
        vec![]
    }
    fn diagnostics(&self) -> Value {
        json!({"valkey_connected":true})
    }
}
pub struct Fixture {
    pub factory: EvidenceFactory,
    pub service: SearchService,
    pub authority: Arc<Current>,
    pub dispatches: Arc<AtomicUsize>,
    pub backend: Arc<Valkey>,
}
impl Fixture {
    pub fn new(url: &str, source_url: &str) -> Self {
        let backend = Arc::new(Valkey::new(url, Duration::from_secs(2)).unwrap());
        let clock: Arc<dyn SearchClock> = Arc::new(SystemSearchClock);
        let mut search_policy = SearchPolicy::default();
        search_policy.grants.insert("retrieval_receipts".into());
        let policy = PolicyService {
            policy: search_policy,
            catalog: CatalogSnapshot::new(vec![EngineCapability {
                name: "wikipedia".into(),
                enabled: true,
                active: true,
                available: true,
                categories: vec!["general".into()],
                media_types: vec![],
                auth_ready: true,
                circuit_open: false,
                cost_class: "free".into(),
                health_status: "unknown".into(),
                health_stale: false,
                enforced_filters: BTreeMap::new(),
            }])
            .unwrap(),
            resolver: ScopeResolver {
                tier1: BTreeSet::from(["wikipedia".into()]),
                ..Default::default()
            },
        };
        let authority = Arc::new(Current(RwLock::new(EvidenceAuthoritySnapshot {
            search: policy.clone(),
            saved_limits: SavedLimits::default(),
            staged_limits: staged::Limits::default(),
            auth_configured: BTreeMap::from([("wikipedia".into(), false)]),
            valkey_connected: true,
        })));
        let dispatches = Arc::new(AtomicUsize::new(0));
        let engine = engine_fixture::FakeEngine {
            name: "wikipedia".into(),
            calls: dispatches.clone(),
            dispatch_receipt: None,
            count: 1,
            status: EngineStatus::Ok,
            delay: Duration::ZERO,
            results: Some(vec![ssx_domain::SearchResult {
                title: "captured source".into(),
                url: source_url.into(),
                engine: "wikipedia".into(),
                engines: BTreeSet::from(["wikipedia".into()]),
                ..Default::default()
            }]),
        };
        let service = SearchService::new(
            SearchConfig::default(),
            SearchDependencies {
                policy: policy.clone(),
                engines: BTreeMap::from([(
                    "wikipedia".into(),
                    EngineBinding {
                        engine: Arc::new(engine),
                        generation: "w07-private".into(),
                        timeout: Duration::from_secs(2),
                        scrape: false,
                    },
                )]),
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
                owned_stores: vec![backend.clone()],
                clock: clock.clone(),
                reranker: None,
            },
        )
        .unwrap()
        .with_policy_source(Arc::new(SearchPolicyAdapter {
            authority: authority.clone(),
        }))
        .unwrap();
        let snapshots = SnapshotService::new(
            Arc::new(ValkeySnapshots::new(backend.clone())),
            Arc::new(SystemSnapshotClock),
            Arc::new(RandomSnapshotIds),
            TenantId::new("default").unwrap(),
            SnapshotTtl::new(3600).unwrap(),
        );
        let state = SearchState::new(service.clone(), snapshots, Arc::new(Discovery), 50).unwrap();
        let factory =
            EvidenceFactory::new(state, backend.clone(), authority.clone(), clock).unwrap();
        Self {
            factory,
            service,
            authority,
            dispatches,
            backend,
        }
    }
}
