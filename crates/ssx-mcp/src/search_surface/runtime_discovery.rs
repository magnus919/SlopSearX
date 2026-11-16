//! Redacted live discovery. Construction takes the same effective registry,
//! catalog and service used for dispatch; provider health is never probed here.
use super::DiscoveryState;
use ssx_application::{
    health::{HealthContext, build_engine_health},
    policy::SearchPolicy,
};
use ssx_domain::WallClockTimestamp;
use ssx_infra::search::{AdvisoryCatalog, SearchService};
use ssx_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

/// Trusted composition-root observations of shared authorities and worker
/// configuration. A configured store is not evidence that it is connected.
#[derive(Clone)]
pub struct RuntimeDiagnostics {
    pub version: String,
    pub valkey_connected: bool,
    pub valkey_fail_closed: bool,
    pub cache_connected: bool,
    pub snapshots_available: bool,
    pub job_store_available: bool,
    pub durable_research: bool,
    pub worker_id: String,
    pub job_lease_ttl_seconds: u64,
    pub job_poll_interval_seconds: ssx_domain::FiniteF64,
    pub job_max_concurrent_jobs: u64,
    pub job_max_queries: u64,
    pub job_max_engines_per_query: u64,
    pub job_max_results: u64,
    pub job_default_deadline_seconds: u64,
    pub snapshot_ttl_seconds: u64,
    pub max_results: u64,
    pub router_enabled: bool,
    pub workflow_availability: BTreeMap<String, bool>,
}
pub trait RuntimeDiagnosticsState: Send + Sync {
    fn snapshot(&self) -> RuntimeDiagnostics;
}
#[derive(Clone)]
pub struct RuntimeDiscovery {
    registry: Arc<ssx_registry::Registry>,
    config: Arc<ssx_config::Config>,
    service: SearchService,
    catalog: Arc<dyn AdvisoryCatalog>,
    policy: SearchPolicy,
    diagnostics: Arc<dyn RuntimeDiagnosticsState>,
    stale_after: f64,
}
impl RuntimeDiscovery {
    pub fn new(
        registry: Arc<ssx_registry::Registry>,
        config: Arc<ssx_config::Config>,
        service: SearchService,
        catalog: Arc<dyn AdvisoryCatalog>,
        policy: SearchPolicy,
        diagnostics: Arc<dyn RuntimeDiagnosticsState>,
        stale_after: f64,
    ) -> Result<Self, &'static str> {
        if !stale_after.is_finite() || stale_after <= 0.0 {
            return Err("invalid observed health freshness");
        }
        if catalog
            .snapshot()
            .engines()
            .iter()
            .any(|c| registry.descriptor(&c.name).is_none())
        {
            return Err("runtime catalog has no matching registry descriptor");
        }
        Ok(Self {
            registry,
            config,
            service,
            catalog,
            policy,
            diagnostics,
            stale_after,
        })
    }
    fn now() -> WallClockTimestamp {
        WallClockTimestamp::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        )
        .expect("finite system time")
    }
    fn observations(&self, now: WallClockTimestamp) -> BTreeMap<String, Value> {
        let health = self.service.health();
        self.registry
            .descriptors()
            .map(|d| {
                let (enabled, auth, configured, _) =
                    ssx_config::capability_configured(d, &self.config);
                let context = HealthContext {
                    configured: enabled,
                    auth_class: Some(auth_name(auth).into()),
                    auth_configured: Some(configured),
                };
                let view =
                    build_engine_health(health.get(&d.name), Some(&context), now, self.stale_after)
                        .expect("checked health freshness");
                (d.name.clone(), view)
            })
            .collect()
    }
}
fn auth_name(auth: ssx_registry::AuthClass) -> &'static str {
    match auth {
        ssx_registry::AuthClass::None => "none",
        ssx_registry::AuthClass::Optional => "optional",
        ssx_registry::AuthClass::Required => "required",
        ssx_registry::AuthClass::Unknown => "unknown",
    }
}
impl DiscoveryState for RuntimeDiscovery {
    fn capabilities(&self) -> Vec<Value> {
        let now = Self::now();
        let observed = self.observations(now);
        let live = self.catalog.snapshot();
        self.registry.descriptors().map(|d|{
            let (enabled,auth,configured,categories)=ssx_config::capability_configured(d,&self.config);
            let categories=live.get(&d.name).map(|c|c.categories.clone()).unwrap_or(categories);
            let subcategories:Vec<_>=categories.iter().filter(|s|s.contains(':')).cloned().collect();
            let mut hints=vec!["explicit"];if categories.iter().any(|s|s=="general"){hints.push("unscoped");}
            let h=&observed[&d.name];
            let supported:ssx_json::Map<String,Value>=["language","time_range","safesearch","pagination","date_from","date_to"].into_iter().map(|k|(k.into(),json!(d.supported_filters.get(k).copied().unwrap_or(false)))).collect();
            let enforced:ssx_json::Map<String,Value>=["language","time_range","safesearch","pagination","date_from","date_to"].into_iter().map(|k|(k.into(),json!(d.enforced_filters.get(k)))).collect();
            json!({"name":d.name,"display_name":d.display_name,"type":d.engine_type,"categories":categories,"subcategories":subcategories,"enabled":enabled,"sensitive":d.sensitive||self.policy.sensitive_engines.contains(&d.name),"supported_filters":supported,"enforced_filters":enforced,"supported_result_types":d.supported_result_types,"supported_media_types":d.supported_media_types,"failure_classes":d.failure_classes,"cost_class":d.cost_class,"last_known_status":h["status"],"last_known_status_at":h["status_at"],"last_known_status_stale":h["stale"],"circuit_open":h["circuit_open"],"circuit_consecutive_errors":h["circuit_consecutive_errors"],"scope_hints":hints,"caveats":d.caveats,"auth":{"class":auth_name(auth),"configured":configured}})
        }).collect()
    }
    fn diagnostics(&self) -> Value {
        let now = Self::now();
        let status = self.diagnostics.snapshot();
        let caps = self.capabilities();
        let observed = self.observations(now);
        let enabled: Vec<_> = caps
            .iter()
            .filter(|c| c["enabled"].as_bool() == Some(true))
            .collect();
        let mut health = json!({"ok":0,"rate_limited":0,"blocked":0,"error":0,"timeout":0,"unavailable":0,"unknown":0,"note":"/health does not actively probe external APIs; use search outcomes for passive engine health"});
        let mut engines = json!({});
        for c in &enabled {
            let name = c["name"].as_str().expect("name");
            let key = c["last_known_status"]
                .as_str()
                .filter(|s| health.get(s).is_some())
                .unwrap_or("unknown");
            health[key] = json!(health[key].as_u64().unwrap_or(0) + 1);
            engines[name] = observed[name].clone();
        }
        let mut causes = Vec::new();
        for (ready, label) in [
            (status.valkey_connected, "Valkey unavailable"),
            (status.cache_connected, "cache unavailable"),
            (status.snapshots_available, "snapshot store unavailable"),
            (status.job_store_available, "research job store unavailable"),
        ] {
            if !ready {
                causes.push(label);
            }
        }
        let specialist: BTreeMap<_, _> = [
            "jobs",
            "security",
            "science",
            "research",
            "saved_searches",
            "saved_search_events",
            "retrieval_receipts",
            "staged_search",
            "dependency_dossier",
        ]
        .into_iter()
        .map(|g| (g, self.policy.grants.contains(g)))
        .collect();
        let mut workflows = json!({});
        for kind in [
            "research",
            "dependency_dossier",
            "staged_search",
            "saved_search",
            "retrieval_receipt",
        ] {
            let available = status
                .workflow_availability
                .get(kind)
                .copied()
                .unwrap_or(false);
            workflows[kind] = json!({"available":available,"status":if available{"available"}else{"unavailable"}});
        }
        json!({"version":status.version,"contract_version":"1.0","valkey":{"connected":status.valkey_connected,"fail_closed":status.valkey_fail_closed},"cache_connected":status.cache_connected,"snapshots_available":status.snapshots_available,"job_store_available":status.job_store_available,"active_engines":enabled.len(),"router_enabled":status.router_enabled,"engine_health":health,"engines":engines,"grants":{"enabled":self.policy.grants,"specialist":specialist,"targeted_sensitive_allowed":self.policy.sensitive_allowed},"research_execution":{"mode":if status.durable_research{"durable_leased"}else{"degraded"},"worker_id":status.worker_id,"lease_ttl_seconds":status.job_lease_ttl_seconds,"poll_interval_seconds":status.job_poll_interval_seconds,"max_concurrent_jobs":status.job_max_concurrent_jobs,"note":if status.durable_research{"research jobs are claimed under an exclusive Valkey lease and reclaimed by another replica on lease expiry"}else{"Valkey unavailable — research jobs are not executed (no shared job store)"}},"workflow_health":workflows,"policy_bounds":{"max_query_length":self.policy.max_query_length,"max_results":status.max_results,"snapshot_ttl_seconds":status.snapshot_ttl_seconds,"job_max_queries":status.job_max_queries,"job_max_engines_per_query":status.job_max_engines_per_query,"job_max_results":status.job_max_results,"job_default_deadline_seconds":status.job_default_deadline_seconds,"job_lease_ttl_seconds":status.job_lease_ttl_seconds,"job_poll_interval_seconds":status.job_poll_interval_seconds,"job_max_concurrent_jobs":status.job_max_concurrent_jobs},"degradation":{"operational":causes.is_empty(),"summary":if causes.is_empty(){"fully operational"}else{"degraded"},"causes":causes},"freshness":ssx_application::iso_timestamp(now.seconds()).expect("timestamp")})
    }
}
