//! Current native MCP discovery is independent of frozen Python comparison fixtures.
//! This composition has no engines, HTTP transport, or external shared store.
use ssx_application::{
    NoopSink, PortFuture,
    cache::RoutingIdentity,
    policy::{PolicyService, SearchPolicy},
    scope::{CatalogSnapshot, ScopeResolver},
    snapshot::{SnapshotFailure, SnapshotRepository, SnapshotService, SnapshotTtl, SnapshotWrite},
};
use ssx_domain::{FiniteF64, SearchSnapshot, SnapshotId, TenantId};
use ssx_infra::{
    search::{
        AdvisoryCatalog, SearchCancellation, SearchConfig, SearchDependencies, SearchService,
        SystemSearchClock,
    },
    valkey::snapshot::{RandomSnapshotIds, SystemSnapshotClock},
};
use ssx_json::{Value, json};
use ssx_mcp::search_surface::{
    RuntimeDiagnostics, RuntimeDiagnosticsState, RuntimeDiscovery, SearchState, SearchSurface,
};
use ssx_registry::Registry;
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
struct CurrentCatalog(CatalogSnapshot);
impl AdvisoryCatalog for CurrentCatalog {
    fn snapshot(&self) -> CatalogSnapshot {
        self.0.clone()
    }
}
struct Diagnostics;
impl RuntimeDiagnosticsState for Diagnostics {
    fn snapshot(&self) -> RuntimeDiagnostics {
        RuntimeDiagnostics {
            version: "native-discovery-test".into(),
            valkey_connected: false,
            valkey_fail_closed: true,
            cache_connected: false,
            snapshots_available: true,
            job_store_available: false,
            durable_research: false,
            worker_id: "".into(),
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
            workflow_availability: BTreeMap::new(),
        }
    }
}
#[derive(Default)]
struct MemorySnapshots {
    rows: Mutex<BTreeMap<(String, String), SearchSnapshot>>,
    calls: AtomicUsize,
}
impl SnapshotRepository for MemorySnapshots {
    fn create<'a>(
        &'a self,
        snapshot: &'a SearchSnapshot,
        _: NonZeroU64,
    ) -> PortFuture<'a, Result<SnapshotWrite, SnapshotFailure>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let mut rows = self.rows.lock().unwrap();
            let key = (
                snapshot.tenant().as_str().into(),
                snapshot.id().as_str().into(),
            );
            if rows.contains_key(&key) {
                return Ok(SnapshotWrite::Exists);
            }
            rows.insert(key, snapshot.clone());
            Ok(SnapshotWrite::Stored)
        })
    }
    fn read<'a>(
        &'a self,
        tenant: &'a TenantId,
        id: &'a SnapshotId,
    ) -> PortFuture<'a, Result<Option<SearchSnapshot>, SnapshotFailure>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .rows
                .lock()
                .unwrap()
                .get(&(tenant.as_str().into(), id.as_str().into()))
                .cloned())
        })
    }
}
#[tokio::test]
async fn native_mcp_capability_tool_keeps_deepseek_visible_disabled_and_truthful() {
    let registry = Arc::new(Registry::builtins().unwrap());
    let config = Arc::new(
        ssx_config::load_config_with(Some(std::path::Path::new("/dev/null")), &BTreeMap::new())
            .unwrap(),
    );
    assert_eq!(config.default_engines, ["brave", "wikipedia"]);
    assert!(!config.engines["deepseek"].enabled);
    let policy = SearchPolicy::default();
    let catalog = CatalogSnapshot::new(vec![]).unwrap();
    let service = SearchService::new_with_registry(
        SearchConfig::default(),
        SearchDependencies {
            policy: PolicyService {
                policy: policy.clone(),
                catalog: catalog.clone(),
                resolver: ScopeResolver::default(),
            },
            engines: BTreeMap::new(),
            routing_identity: RoutingIdentity {
                catalog: None,
                sensitive: policy.sensitive_engines.clone(),
                tier1: ScopeResolver::default().tier1,
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
        registry.clone(),
    )
    .unwrap();
    let discovery = Arc::new(
        RuntimeDiscovery::new(
            registry.clone(),
            config,
            service.clone(),
            Arc::new(CurrentCatalog(catalog)),
            policy,
            Arc::new(Diagnostics),
            300.0,
        )
        .unwrap(),
    );
    let snapshots = Arc::new(MemorySnapshots::default());
    let surface = SearchSurface::new(
        SearchState::new(
            service.clone(),
            SnapshotService::new(
                snapshots.clone(),
                Arc::new(SystemSnapshotClock),
                Arc::new(RandomSnapshotIds),
                TenantId::new("discovery").unwrap(),
                SnapshotTtl::new(3600).unwrap(),
            ),
            discovery,
            50,
        )
        .unwrap(),
    );
    let enabled = surface
        .execute(
            "slopsearx_list_capabilities",
            serde_json::Map::new(),
            TenantId::new("discovery").unwrap(),
            SearchCancellation::default(),
        )
        .await
        .unwrap();
    assert!(
        enabled["engines"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["name"] != json!("deepseek"))
    );
    let all = surface
        .execute(
            "slopsearx_list_capabilities",
            serde_json::Map::from_iter([(
                "include_disabled".into(),
                serde_json::Value::Bool(true),
            )]),
            TenantId::new("discovery").unwrap(),
            SearchCancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(all["count"], json!(registry.len()));
    let deepseek: &Value = all["engines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == json!("deepseek"))
        .unwrap();
    assert_eq!(deepseek["display_name"], json!("DeepSeek Search"));
    assert_eq!(deepseek["categories"], json!(["general"]));
    assert_eq!(deepseek["enabled"], json!(false));
    assert_eq!(
        deepseek["auth"],
        json!({"class":"required","configured":false})
    );
    assert_eq!(deepseek["cost_class"], json!("paid"));
    for filter in [
        "language",
        "time_range",
        "safesearch",
        "pagination",
        "date_from",
        "date_to",
    ] {
        assert_eq!(deepseek["supported_filters"][filter], json!(false));
        assert_eq!(deepseek["enforced_filters"][filter], Value::Null);
    }
    assert_eq!(snapshots.calls.load(Ordering::SeqCst), 0);
    assert!(service.health().is_empty());
    service.shutdown().await;
}
