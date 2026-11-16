//! Passive runtime discovery uses real service observations and trusted typed ports.
#[path = "t06_support/fixture.rs"]
mod fixture;
#[path = "t06_support/server.rs"]
mod private_valkey;
use ssx_application::{policy::SearchPolicy, scope::CatalogSnapshot};
use ssx_domain::{EngineStatus, FiniteF64, TenantId};
use ssx_infra::search::{AdvisoryCatalog, SearchCancellation};
use ssx_json::{Value, json};
use ssx_mcp::{
    Result,
    search_surface::{
        DiscoveryState, RuntimeDiagnostics, RuntimeDiagnosticsState, RuntimeDiscovery,
    },
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
struct Catalog(Mutex<CatalogSnapshot>);
impl AdvisoryCatalog for Catalog {
    fn snapshot(&self) -> CatalogSnapshot {
        self.0.lock().unwrap().clone()
    }
}
struct Diagnostics(Mutex<RuntimeDiagnostics>);
impl RuntimeDiagnosticsState for Diagnostics {
    fn snapshot(&self) -> RuntimeDiagnostics {
        self.0.lock().unwrap().clone()
    }
}
fn diagnostics() -> RuntimeDiagnostics {
    RuntimeDiagnostics {
        version: "fixture".into(),
        valkey_connected: false,
        valkey_fail_closed: true,
        cache_connected: false,
        snapshots_available: false,
        job_store_available: false,
        durable_research: false,
        worker_id: "worker-fixture".into(),
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
fn capability(discovery: &RuntimeDiscovery, name: &str) -> Value {
    discovery
        .capabilities()
        .into_iter()
        .find(|c| c["name"].as_str() == Some(name))
        .unwrap()
}
#[tokio::test]
async fn live_passive_health_categories_finite_diagnostics_and_secret_redaction() -> Result<()> {
    let server = private_valkey::Server::start();
    let fixture = fixture::Fixture::with_engines(
        server.url(),
        false,
        vec![fixture::FakeEngine {
            name: "wikipedia".into(),
            calls: Default::default(),
            dispatch_receipt: None,
            count: 0,
            status: EngineStatus::Error,
            delay: Duration::ZERO,
            results: None,
        }],
    )
    .await;
    let mut config = ssx_config::load_config_with(
        Some(std::path::Path::new("/no/t06-discovery-config")),
        &BTreeMap::new(),
    )?;
    for d in fixture.registry.descriptors() {
        config.engines.entry(d.name.clone()).or_default().enabled = d.name == "wikipedia";
    }
    let wiki = config.engines.get_mut("wikipedia").unwrap();
    wiki.api_key = ssx_config::Secret::new(Some("T06_NEVER_DISCLOSE_KEY".into()));
    wiki.api_secret = ssx_config::Secret::new(Some("T06_NEVER_DISCLOSE_SECRET".into()));
    wiki.base_url = "https://T06_NEVER_DISCLOSE_HOST.invalid".into();
    wiki.extra_env_settings
        .insert("PRIVATE".into(), "T06_NEVER_DISCLOSE_EXTENSION".into());
    config.engines.get_mut("pubmed").unwrap().categories = Some(vec!["configured:category".into()]);
    let catalog = Arc::new(Catalog(Mutex::new(fixture.policy.catalog.clone())));
    let observed = Arc::new(Diagnostics(Mutex::new(diagnostics())));
    let discovery = RuntimeDiscovery::new(
        fixture.registry.clone(),
        Arc::new(config),
        fixture.service.clone(),
        catalog.clone(),
        SearchPolicy::default(),
        observed.clone(),
        300.0,
    )
    .unwrap();
    let before = capability(&discovery, "wikipedia");
    assert_eq!(before["last_known_status"], "unknown");
    assert_eq!(before["auth"]["class"], "optional");
    assert_eq!(before["auth"]["configured"], true);
    let state = discovery.diagnostics();
    assert_eq!(
        state["valkey"]["connected"], false,
        "configured private store must not imply observed connection"
    );
    assert_eq!(state["research_execution"]["mode"], "degraded");
    assert_eq!(state["grants"]["targeted_sensitive_allowed"], false);
    assert_eq!(fixture.calls(), 0, "discovery actively probed an engine");
    let mut caps = catalog.snapshot().engines().to_vec();
    caps.retain(|c| c.name != "pubmed");
    caps.iter_mut()
        .find(|c| c.name == "wikipedia")
        .unwrap()
        .categories = vec!["live:category".into()];
    *catalog.0.lock().unwrap() = CatalogSnapshot::new(caps).unwrap();
    assert_eq!(
        capability(&discovery, "wikipedia")["categories"],
        json!(["live:category"])
    );
    assert_eq!(
        capability(&discovery, "pubmed")["categories"],
        json!(["configured:category"])
    );
    assert_eq!(capability(&discovery, "pubmed")["enabled"], false);
    assert_eq!(fixture.calls(), 0);
    for i in 0..5 {
        fixture
            .surface
            .execute_lossless(
                "slopsearx_search_targeted",
                json!({"query":format!("passive health observation {i}"),"engines":["wikipedia"]})
                    .as_object()
                    .unwrap()
                    .clone(),
                TenantId::new("fixture")?,
                SearchCancellation::default(),
            )
            .await?;
    }
    assert_eq!(fixture.calls(), 5);
    let health = capability(&discovery, "wikipedia");
    assert_eq!(health["last_known_status"], "error");
    assert_eq!(health["circuit_open"], true);
    assert_eq!(health["circuit_consecutive_errors"], 5);
    assert!(health["last_known_status_at"].as_str().is_some());
    fixture
        .surface
        .execute_lossless(
            "slopsearx_search_targeted",
            json!({"query":"circuit must fence","engines":["wikipedia"]})
                .as_object()
                .unwrap()
                .clone(),
            TenantId::new("fixture")?,
            SearchCancellation::default(),
        )
        .await?;
    assert_eq!(
        fixture.calls(),
        5,
        "current policy did not fence observed circuit"
    );
    {
        let mut d = observed.0.lock().unwrap();
        d.valkey_connected = true;
        d.cache_connected = true;
        d.snapshots_available = true;
        d.job_store_available = true;
        d.durable_research = true;
        d.workflow_availability.insert("research".into(), true);
    }
    let state = discovery.diagnostics();
    assert_eq!(state["research_execution"]["mode"], "durable_leased");
    assert_eq!(state["degradation"]["operational"], true);
    assert_eq!(state["workflow_health"]["research"]["available"], true);
    assert_eq!(state["engine_health"]["error"], 1);
    let wire = format!(
        "{}{}",
        ssx_json::to_string(&discovery.capabilities())?,
        state
    );
    assert!(!wire.contains("T06_NEVER_DISCLOSE"));
    assert_eq!(fixture.calls(), 5);
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(FiniteF64::new(value).is_err());
    }
    fixture.service.shutdown().await;
    Ok(())
}
