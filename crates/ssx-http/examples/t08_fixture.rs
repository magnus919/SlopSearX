//! T08 qualification assembly of production HTTP/service/npm/cache components.
//! This is not production startup. Only the upstream response server is synthetic.
use ssx_application::{
    NoopSink, PortFuture, SearchEngine,
    cache::{CacheTtls, CanonicalCache, RoutingIdentity},
    policy::*,
    scope::*,
};
use ssx_domain::*;
use ssx_http::{
    HttpState,
    discovery::{ActiveEngine, build_active},
    html::PublicHtml,
};
use ssx_infra::adapters::{AllowAllAdmission, NpmConfig, PilotConfig, register_pilots};
use ssx_infra::http::AdapterHttp;
use ssx_infra::{
    observability::{Options, Telemetry},
    search::*,
    valkey::{Valkey, cache::ValkeyCache},
};
use ssx_json::json;
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Write,
    sync::{Arc, Mutex},
    time::Duration,
};
struct RecordedAdapter {
    inner: Box<dyn SearchEngine>,
    calls: Mutex<std::fs::File>,
}
impl SearchEngine for RecordedAdapter {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            let response = self.inner.search(request).await;
            writeln!(self.calls.lock().unwrap(), "{}", json!({"query":request.query,"page":request.page,"status":response.status,"error_message":response.error_message})).unwrap();
            response
        })
    }
}
fn capability(name: &str, active: bool, auth: bool) -> EngineCapability {
    EngineCapability {
        name: name.into(),
        enabled: true,
        active,
        available: active,
        categories: vec!["packages".into()],
        media_types: vec![],
        auth_ready: auth,
        circuit_open: false,
        cost_class: "free".into(),
        health_status: "unknown".into(),
        health_stale: true,
        enforced_filters: BTreeMap::new(),
    }
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let calls_path = args.next().ok_or("call log required")?;
    let upstream = args.next().ok_or("owned loopback upstream URL required")?;
    let store_url = args.next().ok_or("owned private Valkey URL required")?;
    let config_path = args.next().ok_or("owned absent config path required")?;
    let url = url::Url::parse(&upstream)?;
    if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") {
        return Err("qualification upstream must be IPv4 loopback HTTP".into());
    }
    if !store_url.starts_with("unix://") {
        return Err("qualification requires owned private Valkey Unix socket".into());
    }
    let store = Arc::new(
        Valkey::new(&store_url, Duration::from_secs(2))
            .map_err(|e| format!("private cache: {e:?}"))?,
    );
    let calls = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(calls_path)?;
    let mut registry = ssx_registry::Registry::builtins()?;
    let http = Arc::new(AdapterHttp::default());
    register_pilots(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        PilotConfig {
            npm: NpmConfig {
                base_url: upstream,
                timeout_ms: 2000,
                max_results: 5,
            },
            ..Default::default()
        },
    )?;
    let engine = RecordedAdapter {
        inner: registry.instantiate("npm").ok_or("npm factory absent")?,
        calls: Mutex::new(calls),
    };
    let categories = vec!["packages".to_owned()];
    let telemetry = Telemetry::new(
        Options {
            audit: false,
            ..Default::default()
        },
        None,
        vec!["npm".into()],
        categories.clone(),
    )?;
    let policy = SearchPolicy::default();
    let policy_arc = Arc::new(policy.clone());
    let service = SearchService::new(
        SearchConfig::default(),
        SearchDependencies {
            policy: PolicyService {
                policy,
                catalog: CatalogSnapshot::new(vec![capability("npm", true, true)])?,
                resolver: ScopeResolver {
                    catalog_routing: false,
                    ..Default::default()
                },
            },
            engines: BTreeMap::from([(
                "npm".into(),
                EngineBinding {
                    engine: Arc::new(engine),
                    generation: "t08-production-npm-v1".into(),
                    timeout: Duration::from_secs(3),
                    scrape: false,
                },
            )]),
            routing_identity: RoutingIdentity {
                catalog: None,
                sensitive: policy_arc.sensitive_engines.clone(),
                tier1: ScopeResolver::default().tier1,
                budget: None,
                ranking_strategy: "presence".into(),
                work_group_version: "scholarly-work-v3".into(),
                rerank_identity: None,
                routing_extension: None,
            },
            cache: Some(Arc::new(CanonicalCache::new(
                Arc::new(ValkeyCache::new(store.clone())),
                CacheTtls::new(900, 60, 30).unwrap(),
            ))),
            client_admission: None,
            suggestions: None,
            jev: None,
            routing_cache: None,
            metrics: telemetry.clone(),
            supervisor: telemetry.clone(),
            audit: Arc::new(NoopSink),
            cooldowns: None,
            owned_http: vec![http],
            owned_stores: vec![store],
            clock: Arc::new(SystemSearchClock),
            reranker: None,
        },
    )
    .map_err(|e| format!("service: {e:?}"))?;
    let config =
        ssx_config::load_config_with(Some(std::path::Path::new(&config_path)), &BTreeMap::new())?;
    let discovery = build_active(
        Some(&registry),
        &config,
        &[ActiveEngine {
            name: "npm".into(),
            display_name: "npm".into(),
            engine_type: "api".into(),
            categories,
        }],
        &policy_arc,
        "0.0.0",
    );
    let state = HttpState {
        search: service,
        policy: policy_arc,
        telemetry,
        formats: ["html", "json", "csv", "rss"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        discovery: discovery.config,
        health_contexts: discovery.health_contexts,
        valkey_connected: Arc::new(|| true),
        valkey_fail_closed: true,
        valkey_quota_configured: false,
        html: Arc::new(PublicHtml::from_options(
            ssx_http::html::PortalOptions::from_environment(&BTreeMap::new()),
        )),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    println!(
        "{}",
        json!({"address": listener.local_addr()?.to_string(), "mode": "t08-production-components", "adapter": "npm"})
    );
    ssx_http::native_serve(listener, state).await?;
    Ok(())
}
