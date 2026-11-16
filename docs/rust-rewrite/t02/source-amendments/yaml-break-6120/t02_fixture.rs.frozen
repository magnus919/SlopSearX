//! Offline actual-process HTTP qualification. The single counted engine is synthetic;
//! no provider transport exists in this composition. This is not production wiring.
use ssx_application::{
    NoopSink, PortFuture, SearchEngine,
    cache::{
        CacheEntry, CacheFailure, CacheRepository, CacheTtls, CanonicalCache, RoutingIdentity,
    },
    policy::*,
    scope::*,
};
use ssx_domain::*;
use ssx_http::{
    HttpState,
    discovery::{ActiveEngine, build_active},
    html::PublicHtml,
};
use ssx_infra::{
    observability::{Options, Telemetry},
    search::*,
    valkey::{Valkey, cache::ValkeyCache},
};
use ssx_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Write,
    sync::{Arc, Mutex},
    time::Duration,
};
struct SeededValkeyCache {
    inner: ValkeyCache,
    seed_negative: bool,
    seeded: std::sync::atomic::AtomicBool,
}
impl CacheRepository for SeededValkeyCache {
    fn read<'a>(
        &'a self,
        key: &'a str,
    ) -> PortFuture<'a, Result<Option<CacheEntry>, CacheFailure>> {
        Box::pin(async move {
            if self.seed_negative && !self.seeded.swap(true, std::sync::atomic::Ordering::AcqRel) {
                self.inner
                    .write(
                        key,
                        &CacheEntry::Negative {
                            timestamp: Some(SystemSearchClock.capture().0),
                        },
                        std::num::NonZeroU64::new(30).unwrap(),
                    )
                    .await?;
            }
            self.inner.read(key).await
        })
    }
    fn write<'a>(
        &'a self,
        key: &'a str,
        value: &'a CacheEntry,
        ttl: std::num::NonZeroU64,
    ) -> PortFuture<'a, Result<(), CacheFailure>> {
        self.inner.write(key, value, ttl)
    }
}
struct Denied;
impl ClientAdmission for Denied {
    fn acquire<'a>(&'a self, _client: &'a str) -> PortFuture<'a, bool> {
        Box::pin(async { false })
    }
}
struct Contract {
    name: String,
    count: usize,
    controls: Value,
    calls: Mutex<std::fs::File>,
}
impl SearchEngine for Contract {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            writeln!(self.calls.lock().unwrap(),"{}",json!({"query":request.query,"language":request.language,"safesearch":request.safesearch,"pageno":request.page,"time_range":request.time_range,"date_from":request.date_from,"date_to":request.date_to,"categories":request.categories})).unwrap();
            let latency_ms = FiniteF64::new(1.).unwrap();
            if request.query == "panic-secret" {
                panic!("failed https://api.example.test/?api_key=secret-key-12345");
            }
            if request.query == "deadline" {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            if request.query == "empty" {
                return AdapterResponse {
                    latency_ms,
                    ..Default::default()
                };
            }
            if request.query == "error" || request.query == "timeout" {
                return AdapterResponse {
                    latency_ms,
                    status: if request.query == "timeout" {
                        EngineStatus::Timeout
                    } else {
                        EngineStatus::Error
                    },
                    error_message: Some("simulated error".into()),
                    ..Default::default()
                };
            }
            let mut result = SearchResult {
                url: "https://example.test/path;param?q=one#section".into(),
                title: format!("Contract result for {}", request.query),
                content: "Deterministic contract result".into(),
                engine: self.name.clone(),
                ..Default::default()
            };
            if let Some(id) = request.query.strip_prefix("formatter:") {
                let control = self.controls["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["id"].as_str() == Some(id))
                    .unwrap();
                if id.starts_with("epoch:") {
                    result.published_date = control["input"].as_str().map(str::to_owned);
                } else {
                    result.url = control["input"].as_str().unwrap_or("").into();
                }
            }
            if request.query == "surrogate" {
                result.payload = Some(
                    json!({"domain":"security","type":"vulnerability","data":{"note":"\u{fffd}"}}),
                );
            }
            let results = (0..self.count)
                .map(|index| {
                    let mut result = result.clone();
                    if index > 0 {
                        result.url = format!("https://example.test/result/{index}");
                    }
                    result
                })
                .collect();
            AdapterResponse {
                results,
                latency_ms,
                ..Default::default()
            }
        })
    }
}
fn capability(name: &str, active: bool, auth: bool) -> EngineCapability {
    EngineCapability {
        name: name.into(),
        enabled: true,
        active,
        available: active,
        categories: vec!["general".into(), "science".into()],
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
    let theme = args.next().unwrap_or_else(|| "dark".into());
    let shutdown = args.next();
    let scenario = args.next().unwrap_or_else(|| "contract".into());
    let store = args
        .next()
        .map(|url| Valkey::new(&url, Duration::from_secs(2)).map(Arc::new))
        .transpose()
        .map_err(|e| format!("private cache: {e:?}"))?;
    if matches!(scenario.as_str(), "cache" | "negative") && store.is_none() {
        return Err("cache scenario requires an explicitly owned private Valkey URL".into());
    }
    let calls = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(calls_path)?;
    let controls: ssx_json::Value =
        ssx_json::from_str(include_str!("../tests/formatter_http_controls.json"))?;
    let engine_name = if scenario == "date-proof" {
        "openalex"
    } else {
        "contract"
    };
    let categories: Vec<String> = if matches!(scenario.as_str(), "mock3" | "grants") {
        vec!["general", "news", "tech", "science"]
    } else {
        vec!["general", "science"]
    }
    .into_iter()
    .map(str::to_owned)
    .collect();
    let telemetry = Telemetry::new(
        Options {
            audit: false,
            ..Default::default()
        },
        None,
        if scenario == "none" {
            vec![]
        } else {
            vec![engine_name.into()]
        },
        categories.clone(),
    )?;
    let mut policy = SearchPolicy::default();
    if matches!(scenario.as_str(), "sensitive" | "restricted") {
        policy.sensitive_engines = ["contract".to_owned()].into_iter().collect();
        policy.sensitive_allowed = scenario == "sensitive";
    }
    if scenario == "grants" {
        policy.grants = ["jobs", "security", "science", "research", "events"]
            .into_iter()
            .map(str::to_owned)
            .collect();
    }
    let policy_arc = Arc::new(policy.clone());
    let mut contract = capability(engine_name, true, true);
    if scenario == "date-proof" {
        contract.enforced_filters = [
            ("date_from".into(), "upstream".into()),
            ("date_to".into(), "upstream".into()),
        ]
        .into_iter()
        .collect();
    }
    contract.categories = categories.clone();
    let catalog = CatalogSnapshot::new(if scenario == "none" {
        vec![capability("brave", false, false)]
    } else {
        vec![contract, capability("brave", false, false)]
    })?;
    let service = SearchService::new(
        SearchConfig {
            empty_scrape_diagnostics: scenario == "scrape-enabled",
            ..Default::default()
        },
        SearchDependencies {
            policy: PolicyService {
                policy,
                catalog,
                resolver: ScopeResolver {
                    catalog_routing: false,
                    ..Default::default()
                },
            },
            engines: if scenario == "none" {
                BTreeMap::new()
            } else {
                BTreeMap::from([(
                    engine_name.into(),
                    EngineBinding {
                        engine: Arc::new(Contract {
                            name: engine_name.into(),
                            count: if matches!(scenario.as_str(), "mock3" | "sensitive" | "grants")
                            {
                                3
                            } else {
                                1
                            },
                            controls,
                            calls: Mutex::new(calls),
                        }),
                        generation: "t02-source-contract-v1".into(),
                        timeout: Duration::from_secs(3),
                        scrape: scenario.starts_with("scrape-"),
                    },
                )])
            },
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
            cache: store.as_ref().map(|store| {
                Arc::new(CanonicalCache::new(
                    Arc::new(SeededValkeyCache {
                        inner: ValkeyCache::new(store.clone()),
                        seed_negative: scenario == "negative",
                        seeded: std::sync::atomic::AtomicBool::new(false),
                    }),
                    CacheTtls::new(900, 60, 30).unwrap(),
                ))
            }),
            client_admission: if scenario == "denied" {
                Some(Arc::new(Denied))
            } else {
                None
            },
            suggestions: None,
            jev: None,
            routing_cache: None,
            metrics: telemetry.clone(),
            supervisor: telemetry.clone(),
            audit: Arc::new(NoopSink),
            cooldowns: None,
            owned_http: vec![],
            owned_stores: store.iter().cloned().collect(),
            clock: Arc::new(SystemSearchClock),
            reranker: None,
        },
    )
    .map_err(|e| format!("service: {e:?}"))?;
    let registry = ssx_registry::Registry::builtins()?;
    let config = ssx_config::load_config_with(
        Some(std::path::Path::new(
            "/private/tmp/t02-absent-reference-config.yaml",
        )),
        &BTreeMap::new(),
    )?;
    let discovery = build_active(
        Some(&registry),
        &config,
        &if scenario == "none" {
            vec![]
        } else {
            vec![ActiveEngine {
                name: engine_name.into(),
                display_name: "Contract Engine".into(),
                engine_type: "api".into(),
                categories: categories.clone(),
            }]
        },
        &policy_arc,
        "0.0.0",
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    println!(
        "{}",
        json!({"address":listener.local_addr()?.to_string(),"mode":"offline-t02-fixture"})
    );
    let state = HttpState {
        search: service,
        policy: policy_arc,
        telemetry,
        formats: if scenario == "formats-json" {
            vec!["json"]
        } else {
            vec!["html", "json", "csv", "rss"]
        }
        .into_iter()
        .map(str::to_owned)
        .collect(),
        discovery: discovery.config,
        health_contexts: discovery.health_contexts,
        valkey_connected: Arc::new(|| false),
        valkey_fail_closed: true,
        valkey_quota_configured: false,
        html: Arc::new(PublicHtml::from_options(
            ssx_http::html::PortalOptions::from_environment(&BTreeMap::from([(
                "SLOPSEARX_PORTAL_DEFAULT_THEME".into(),
                theme,
            )])),
        )),
    };
    if shutdown.as_deref() == Some("stdin") {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let input = std::thread::spawn(move || {
            let mut line = String::new();
            if std::io::stdin().read_line(&mut line).is_ok() && line == "STOP\n" {
                let _ = tx.send(());
            }
        });
        ssx_http::native::serve_with_shutdown(listener, state, async {
            let _ = rx.await;
        })
        .await?;
        input.join().map_err(|_| "stdin shutdown thread failed")?;
    } else {
        ssx_http::native_serve(listener, state).await?;
    }
    Ok(())
}
