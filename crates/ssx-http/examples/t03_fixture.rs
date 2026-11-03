//! Offline T03 portal composition. Engines, suggestions and negative-cache
//! answers are explicit synthetic inputs; no provider transport exists here.
use ssx_application::{
    NoopSink, PortFuture, SearchEngine,
    cache::{
        CacheEntry, CacheFailure, CacheRepository, CacheTtls, CanonicalCache, RoutingIdentity,
    },
    policy::{PolicyService, SearchPolicy},
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use ssx_http::{HttpState, html::PublicHtml};
use ssx_infra::{
    observability::{Options, Telemetry},
    search::{
        ClientAdmission, EngineBinding, SearchConfig, SearchDependencies, SearchService,
        Suggestions, SystemSearchClock,
    },
};
use ssx_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::OpenOptions,
    io::Write,
    sync::{Arc, Mutex},
    time::Duration,
};

struct FixtureEngine {
    name: String,
    calls: Arc<Mutex<std::fs::File>>,
}
impl SearchEngine for FixtureEngine {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            writeln!(self.calls.lock().unwrap(),"{}",json!({"engine":self.name,"query":request.query,"page":request.page,"date_from":request.date_from,"date_to":request.date_to,"time_range":request.time_range})).unwrap();
            let mut response = AdapterResponse {
                latency_ms: FiniteF64::new(1.0).unwrap(),
                ..Default::default()
            };
            if request.query == "all-failed" || (request.query == "partial" && self.name == "beta")
            {
                response.status = EngineStatus::Error;
                response.error_message = Some("synthetic source failure".into());
                return response;
            }
            if request.query == "empty" {
                return response;
            }
            let mut result = SearchResult {
                url: "https://example.org/article?source=fixture".into(),
                title: "A useful deterministic result".into(),
                content: "A source-backed snippet with <strong>plain highlights</strong>.".into(),
                engine: self.name.clone(),
                category: if self.name == "openalex" {
                    "science"
                } else {
                    "general"
                }
                .into(),
                published_date: Some("2024-02-29T12:00:00Z".into()),
                ..Default::default()
            };
            match request.query.as_str() {
                "malicious" => {
                    result.url = "javascript:alert('unsafe')".into();
                    result.title = "<img src=x onerror=alert(1)> & source".into();
                    result.content="<STRONG>safe match</strong> <svg onload=alert(1)> <em onclick=alert(1)>unsafe</em>".into();
                    result.payload = Some(
                        json!({"schema_version":1,"domain":"<script>domain</script>","type":"<img src=x>","data":{},"provenance":{"engine":self.name,"adapter_fields":[],"inferred_fields":[]}}),
                    );
                }
                "long" => {
                    result.title = "Long title ".repeat(100);
                    result.content = "Unbroken".repeat(200);
                }
                "missing" => {
                    result.published_date = None;
                    result.content.clear();
                }
                "groups" => {
                    result.url = format!("https://example.org/{}/vulnerability", self.name);
                    result.payload = Some(
                        json!({"schema_version":1,"domain":"security","type":"vulnerability","data":{"cve_id":"CVE-2026-12345","summary":format!("{} <conflict>",self.name)},"provenance":{"engine":self.name,"adapter_fields":["cve_id","summary"],"inferred_fields":[]}}),
                    );
                }
                _ => {}
            }
            if self.name == "openalex" && request.query != "malicious" && request.query != "groups"
            {
                result.url = "https://doi.org/10.1234/fixture".into();
                result.payload = Some(
                    json!({"schema_version":1,"domain":"science","type":"publication","data":{"doi":"10.1234/fixture","title":"Fixture publication"},"provenance":{"engine":"openalex","adapter_fields":["doi","title"],"inferred_fields":[]}}),
                );
            }
            response.results.push(result);
            response
        })
    }
}
struct FixtureSuggestions;
impl Suggestions for FixtureSuggestions {
    fn fetch<'a>(&'a self, _query: &'a str) -> PortFuture<'a, Vec<String>> {
        Box::pin(async { vec!["broader query".into(), "<script>suggestion</script>".into()] })
    }
    fn shutdown(&self) -> PortFuture<'_, ()> {
        Box::pin(async {})
    }
}
struct Denied;
impl ClientAdmission for Denied {
    fn acquire<'a>(&'a self, _client: &'a str) -> PortFuture<'a, bool> {
        Box::pin(async { false })
    }
}
struct NegativeCache;
impl CacheRepository for NegativeCache {
    fn read<'a>(
        &'a self,
        _key: &'a str,
    ) -> PortFuture<'a, Result<Option<CacheEntry>, CacheFailure>> {
        Box::pin(async { Ok(Some(CacheEntry::Negative { timestamp: None })) })
    }
    fn write<'a>(
        &'a self,
        _key: &'a str,
        _value: &'a CacheEntry,
        _ttl: std::num::NonZeroU64,
    ) -> PortFuture<'a, Result<(), CacheFailure>> {
        Box::pin(async { Ok(()) })
    }
}
fn capability(name: &str) -> EngineCapability {
    EngineCapability {
        name: name.into(),
        enabled: true,
        active: true,
        available: true,
        categories: vec![
            if name == "openalex" {
                "science"
            } else {
                "general"
            }
            .into(),
        ],
        media_types: vec![],
        auth_ready: true,
        circuit_open: false,
        cost_class: "free".into(),
        health_status: "unknown".into(),
        health_stale: true,
        enforced_filters: if name == "openalex" {
            BTreeMap::from([
                ("date_from".into(), "upstream".into()),
                ("date_to".into(), "upstream".into()),
                ("time_range".into(), "local".into()),
            ])
        } else {
            BTreeMap::new()
        },
    }
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let calls_path = args.next().ok_or("owned call log path required")?;
    let theme = args.next().unwrap_or_else(|| "dark".into());
    let scenario = args.next().unwrap_or_else(|| "contract".into());
    if !["contract", "cached-error", "rate-limited", "html-disabled"].contains(&scenario.as_str()) {
        return Err("unknown fixture scenario".into());
    }
    let calls = Arc::new(Mutex::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(calls_path)?,
    ));
    let names = ["alpha", "beta", "openalex", "hibp"];
    let policy = SearchPolicy::default();
    let policy_arc = Arc::new(policy.clone());
    let telemetry = Telemetry::new(
        Options {
            audit: false,
            ..Default::default()
        },
        None,
        names.iter().map(|s| (*s).into()).collect(),
        vec!["general".into(), "science".into()],
    )?;
    let service = SearchService::new(
        SearchConfig::default(),
        SearchDependencies {
            policy: PolicyService {
                policy,
                catalog: CatalogSnapshot::new(names.iter().map(|s| capability(s)).collect())?,
                resolver: ScopeResolver {
                    catalog_routing: false,
                    ..Default::default()
                },
            },
            engines: names
                .iter()
                .map(|name| {
                    (
                        (*name).into(),
                        EngineBinding {
                            engine: Arc::new(FixtureEngine {
                                name: (*name).into(),
                                calls: calls.clone(),
                            }),
                            generation: "t03-synthetic-v1".into(),
                            timeout: Duration::from_secs(2),
                            scrape: false,
                        },
                    )
                })
                .collect(),
            routing_identity: RoutingIdentity {
                catalog: None,
                sensitive: policy_arc.sensitive_engines.clone(),
                tier1: BTreeSet::new(),
                budget: None,
                ranking_strategy: "presence".into(),
                work_group_version: "scholarly-work-v3".into(),
                rerank_identity: None,
                routing_extension: None,
            },
            cache: (scenario == "cached-error").then(|| {
                Arc::new(CanonicalCache::new(
                    Arc::new(NegativeCache),
                    CacheTtls::new(900, 60, 30).unwrap(),
                ))
            }),
            client_admission: (scenario == "rate-limited")
                .then(|| Arc::new(Denied) as Arc<dyn ClientAdmission>),
            suggestions: Some(Arc::new(FixtureSuggestions)),
            jev: None,
            routing_cache: None,
            metrics: telemetry.clone(),
            supervisor: telemetry.clone(),
            audit: Arc::new(NoopSink),
            cooldowns: None,
            owned_http: vec![],
            owned_stores: vec![],
            clock: Arc::new(SystemSearchClock),
            reranker: None,
        },
    )
    .map_err(|e| format!("fixture service: {e:?}"))?;
    let discovery = json!({"categories":["general","science","sensitive-only"],"category_engines":{"general":["alpha","beta"],"science":["openalex"],"sensitive-only":["hibp"]},"engines":names.iter().map(|name|{let c=capability(name);json!({"name":name,"display_name":if *name=="beta"{"Beta <source>"}else{name},"enabled":true,"sensitive":*name=="hibp","categories":if *name=="hibp"{vec!["sensitive-only".to_string()]}else{c.categories},"enforced_filters":c.enforced_filters})}).collect::<Vec<Value>>()});
    let state = HttpState {
        search: service,
        policy: policy_arc,
        telemetry,
        formats: if scenario == "html-disabled" {
            ["json", "yaml"].into_iter().map(str::to_owned).collect()
        } else {
            ["html", "json", "yaml", "csv", "rss"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        },
        discovery,
        health_contexts: BTreeMap::new(),
        valkey_connected: Arc::new(|| false),
        valkey_fail_closed: true,
        valkey_quota_configured: false,
        html: Arc::new(PublicHtml::default()),
    }
    .with_public_portal(&theme);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    println!(
        "{}",
        json!({"address":listener.local_addr()?.to_string(),"mode":"offline-t03-fixture","renderer":"PublicPortal","scenario":scenario})
    );
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
    input.join().map_err(|_| "fixture stdin thread failed")?;
    Ok(())
}
