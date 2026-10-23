//! Real A07 adapters under the accepted F14 caller-owned context.
#[path = "support/default_ca_environment.rs"]
mod default_ca_environment;
use ssx_application::{
    AdmissionDecision, NoopSink, PortFuture, SearchAdmission, SearchContext, SearchEngine,
};
use ssx_config::ConfigScalar;
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{
        AllowAllAdmission,
        clinicaltrials::{BiomedicalConfig, ClinicalTrialsAdapter},
        europepmc::EuropePMCAdapter,
        pubchem::PubChemAdapter,
        pubmed::PubMedAdapter,
        uniprot::UniProtAdapter,
    },
    http::{AdapterHttp, Cancellation, ExchangeFuture, Preparation, Request, Response, Transport},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
const PM_SEARCH: &str = include_str!("../../../docs/rust-rewrite/a07/fixtures/pubmed-search.json");
const PM_SUMMARY: &str =
    include_str!("../../../docs/rust-rewrite/a07/fixtures/pubmed-summary.json");
const PC_SEARCH: &str = include_str!("../../../docs/rust-rewrite/a07/fixtures/pubchem-search.json");
const PC_DETAIL: &str = include_str!("../../../docs/rust-rewrite/a07/fixtures/pubchem-detail.json");
fn adapter(
    name: &str,
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: BiomedicalConfig,
) -> Arc<dyn SearchEngine> {
    match name {
        "clinicaltrials" => Arc::new(ClinicalTrialsAdapter::new(http, admission, config)),
        "europepmc" => Arc::new(EuropePMCAdapter::new(http, admission, config)),
        "pubchem" => Arc::new(PubChemAdapter::new(http, admission, config)),
        "pubmed" => Arc::new(PubMedAdapter::new(http, admission, config)),
        "uniprot" => Arc::new(UniProtAdapter::new(http, admission, config)),
        _ => panic!("unknown engine"),
    }
}
fn request() -> SearchRequest {
    SearchRequest {
        query: "context".into(),
        interactive_timeout_ms: Some(5000),
        ..Default::default()
    }
}
// Production startup is separate from these query-context assertions. This
// constructor builds the ordinary default-profile client without an HTTP GET;
// its existing five-second idle expiry and environment/proxy policy apply.
async fn ready_http() -> Arc<AdapterHttp> {
    let startup = Instant::now();
    let (http, preparation) = AdapterHttp::new_ready(
        tokio::time::Instant::now() + Duration::from_secs(5),
        Cancellation::default(),
    )
    .await
    .unwrap();
    assert_eq!(preparation, Preparation::Prepared);
    eprintln!(
        "A07_CONTEXT_STARTUP preparation={preparation:?} elapsed_ns={}",
        startup.elapsed().as_nanos()
    );
    http
}
struct Replay {
    seen: Mutex<Vec<Request>>,
    replies: Mutex<VecDeque<(u16, Vec<u8>)>>,
}
impl Transport for Replay {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let url = request.url.clone();
            self.seen.lock().unwrap().push(request);
            let (status, body) = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(ssx_infra::http::Error::RequestLimit)?;
            Ok(Response {
                url,
                status,
                body,
                headers: Default::default(),
            })
        })
    }
}
async fn server(
    bodies: Vec<(String, Duration)>,
) -> (String, Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let requests = seen.clone();
    let task = tokio::spawn(async move {
        for (body, delay) in bodies {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buf = [0; 4096];
                let n = stream.read(&mut buf).await.unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&buf[..n]);
                if bytes.windows(4).any(|b| b == b"\r\n\r\n") {
                    break;
                }
            }
            requests
                .lock()
                .unwrap()
                .push(String::from_utf8(bytes).unwrap());
            tokio::time::sleep(delay).await;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes()).await;
            let _ = stream.write_all(body.as_bytes()).await;
        }
    });
    (format!("http://{addr}"), seen, task)
}
#[tokio::test]
async fn expired_context_dispatches_no_request_for_any_biomedical_engine() {
    for name in [
        "clinicaltrials",
        "europepmc",
        "pubchem",
        "pubmed",
        "uniprot",
    ] {
        let replay = Arc::new(Replay {
            seen: Mutex::new(vec![]),
            replies: Mutex::new(VecDeque::new()),
        });
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(replay.clone()).unwrap();
        let engine = adapter(
            name,
            http.clone(),
            Arc::new(AllowAllAdmission),
            BiomedicalConfig::defaults("http://127.0.0.1:1"),
        );
        let out = engine
            .search_with_context(
                &request(),
                SearchContext {
                    deadline: Instant::now().checked_sub(Duration::from_secs(1)).unwrap(),
                },
            )
            .await;
        assert_eq!(out.status, EngineStatus::Timeout, "{name}");
        assert!(out.results.is_empty());
        assert!(replay.seen.lock().unwrap().is_empty());
        http.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn ample_context_preserves_all_legacy_result_and_response_fields() {
    let source: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a07/source_complete.json"
    ))
    .unwrap();
    for (id, name) in [
        ("ct-success", "clinicaltrials"),
        ("epmc-ppr", "europepmc"),
        ("pc-success", "pubchem"),
        ("pm-success", "pubmed"),
        ("up-success", "uniprot"),
    ] {
        let row = source["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        let mut outputs = Vec::new();
        for contextual in [false, true] {
            let replay = Arc::new(Replay {
                seen: Mutex::new(vec![]),
                replies: Mutex::new(
                    row["responses"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|r| {
                            (
                                r["status"].as_u64().unwrap() as u16,
                                ssx_json::to_vec(&r["body"]).unwrap(),
                            )
                        })
                        .collect(),
                ),
            });
            let http = Arc::new(AdapterHttp::default());
            http.set_transport(replay.clone()).unwrap();
            let engine = adapter(
                name,
                http.clone(),
                Arc::new(AllowAllAdmission),
                BiomedicalConfig::defaults("https://context.invalid"),
            );
            let req = SearchRequest {
                query: row["query"].as_str().unwrap().into(),
                ..Default::default()
            };
            let output = if contextual {
                engine
                    .search_with_context(
                        &req,
                        SearchContext {
                            deadline: Instant::now() + Duration::from_secs(5),
                        },
                    )
                    .await
            } else {
                engine.search(&req).await
            };
            assert_eq!(output.status, EngineStatus::Ok);
            assert_eq!(
                replay.seen.lock().unwrap().len(),
                row["requests"].as_array().unwrap().len()
            );
            outputs.push(output);
            http.shutdown_and_wait().await;
        }
        let a = &outputs[0];
        let b = &outputs[1];
        assert_eq!(a.results, b.results, "{id}");
        assert_eq!(a.status, b.status);
        assert_eq!(a.error_message, b.error_message);
        assert_eq!(a.answers, b.answers);
        assert_eq!(a.corrections, b.corrections);
        assert_eq!(a.infoboxes, b.infoboxes);
        assert_eq!(a.synthetic, b.synthetic);
    }
}
#[tokio::test]
async fn pubchem_second_request_renewal_and_pubmed_pipeline_cannot_extend_context() {
    if default_ca_environment::default_ca_test_environment(
        "pubchem_second_request_renewal_and_pubmed_pipeline_cannot_extend_context",
    ) {
        return;
    }
    for (name, first, second) in [
        ("pubchem", PC_SEARCH, PC_DETAIL),
        ("pubmed", PM_SEARCH, PM_SUMMARY),
    ] {
        let http = ready_http().await;
        let start = Instant::now();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let requests = seen.clone();
        let (entered_tx, mut entered_rx) = tokio::sync::mpsc::channel(2);
        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
        let mut task = tokio::spawn(async move {
            let mut release = Some(release_rx);
            for (stage, body) in [first, second].into_iter().enumerate() {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut buf = [0; 4096];
                    let n = stream.read(&mut buf).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buf[..n]);
                    if bytes.windows(4).any(|b| b == b"\r\n\r\n") {
                        break;
                    }
                }
                requests
                    .lock()
                    .unwrap()
                    .push(String::from_utf8(bytes).unwrap());
                entered_tx.send(stage).await.unwrap();
                if stage == 0 {
                    // Consume the original budget so renewing it at stage two is observable.
                    tokio::time::sleep_until(tokio::time::Instant::from_std(
                        start + Duration::from_millis(1500),
                    ))
                    .await;
                }
                if stage == 1 {
                    // Keep the real second response unavailable until the caller finishes.
                    let _ = release.take().unwrap().await;
                }
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(body.as_bytes()).await;
            }
        });
        let mut c = BiomedicalConfig::defaults(&base);
        c.timeout_ms = ConfigScalar::Integer(15_000);
        let engine = adapter(name, http.clone(), Arc::new(AllowAllAdmission), c);
        let context = SearchContext {
            deadline: start + Duration::from_secs(3),
        };
        let mut client =
            tokio::spawn(async move { engine.search_with_context(&request(), context).await });
        let stages = tokio::time::timeout_at(
            tokio::time::Instant::from_std(start + Duration::from_millis(2500)),
            async { (entered_rx.recv().await, entered_rx.recv().await) },
        )
        .await;
        // Never renew the original context after observing either stage.
        let outcome = tokio::time::timeout_at(
            tokio::time::Instant::from_std(start + Duration::from_secs(6)),
            &mut client,
        )
        .await;
        let elapsed = start.elapsed();
        if outcome.is_err() {
            client.abort();
            let _ = client.await;
        }
        let _ = release_tx.send(());
        let server_join = tokio::time::timeout(Duration::from_secs(1), &mut task).await;
        if server_join.is_err() {
            task.abort();
            let _ = task.await;
        }
        http.shutdown_and_wait().await;
        assert_eq!(
            stages.unwrap(),
            (Some(0), Some(1)),
            "{name}: both stages must enter"
        );
        let out = outcome.unwrap().unwrap();
        assert_eq!(out.status, EngineStatus::Timeout, "{name}");
        assert!(out.results.is_empty());
        assert!(
            elapsed >= Duration::from_millis(2900),
            "{name}: premature timeout"
        );
        assert!(
            elapsed < Duration::from_secs(4),
            "{name}: original deadline was extended"
        );
        assert_eq!(seen.lock().unwrap().len(), 2);
        assert!(server_join.unwrap().is_ok());
    }
}
#[tokio::test]
async fn queue_time_consumes_absolute_context_instead_of_original_request_timeout() {
    if default_ca_environment::default_ca_test_environment(
        "queue_time_consumes_absolute_context_instead_of_original_request_timeout",
    ) {
        return;
    }
    let (base, seen, task) = server(vec![(
        r#"{"studies":[]}"#.into(),
        Duration::from_millis(100),
    )])
    .await;
    let http = ready_http().await;
    assert!(
        seen.lock().unwrap().is_empty(),
        "startup sends no provider request"
    );
    let engine = adapter(
        "clinicaltrials",
        http.clone(),
        Arc::new(AllowAllAdmission),
        BiomedicalConfig::defaults(&base),
    );
    let start = Instant::now();
    let context = SearchContext {
        deadline: start + Duration::from_millis(70),
    };
    tokio::time::sleep(Duration::from_millis(35)).await;
    let out = engine.search_with_context(&request(), context).await;
    assert_eq!(out.status, EngineStatus::Timeout);
    assert!(start.elapsed() < Duration::from_millis(180));
    assert_eq!(seen.lock().unwrap().len(), 1);
    task.abort();
    let _ = task.await;
    http.shutdown_and_wait().await;
}
struct SlowAdmission;
impl SearchAdmission for SlowAdmission {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            AdmissionDecision::Allowed
        })
    }
}
#[tokio::test]
async fn context_bounds_admission_wait_before_any_provider_request() {
    let replay = Arc::new(Replay {
        seen: Mutex::new(vec![]),
        replies: Mutex::new(VecDeque::new()),
    });
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(replay.clone()).unwrap();
    let engine = adapter(
        "pubmed",
        http.clone(),
        Arc::new(SlowAdmission),
        BiomedicalConfig::defaults("https://context.invalid"),
    );
    let out = tokio::time::timeout(
        Duration::from_millis(200),
        engine.search_with_context(
            &request(),
            SearchContext {
                deadline: Instant::now() + Duration::from_millis(20),
            },
        ),
    )
    .await
    .unwrap();
    assert_eq!(out.status, EngineStatus::Timeout);
    assert!(replay.seen.lock().unwrap().is_empty());
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn real_f14_service_context_reaches_pubchem_second_request_and_keeps_synthetic_origin() {
    use ssx_application::{cache::RoutingIdentity, policy::*, scope::*};
    use ssx_infra::search::*;
    use std::collections::BTreeMap;
    let (base, seen, task) = server(vec![
        (PC_SEARCH.into(), Duration::from_millis(15)),
        (PC_DETAIL.into(), Duration::from_millis(120)),
    ])
    .await;
    let http = Arc::new(AdapterHttp::default());
    let engine = adapter(
        "pubchem",
        http.clone(),
        Arc::new(AllowAllAdmission),
        BiomedicalConfig::defaults(&base),
    );
    let capability = EngineCapability {
        name: "pubchem".into(),
        enabled: true,
        active: true,
        available: true,
        categories: vec!["science".into()],
        media_types: vec![],
        auth_ready: true,
        circuit_open: false,
        cost_class: "free".into(),
        health_status: "unknown".into(),
        health_stale: true,
        enforced_filters: BTreeMap::new(),
    };
    let deps = SearchDependencies {
        policy: PolicyService {
            policy: SearchPolicy::default(),
            catalog: CatalogSnapshot::new(vec![capability]).unwrap(),
            resolver: ScopeResolver {
                catalog_routing: false,
                ..Default::default()
            },
        },
        engines: BTreeMap::from([(
            "pubchem".into(),
            EngineBinding {
                engine,
                generation: "a07-context-fixture".into(),
                timeout: Duration::from_millis(500),
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
        owned_http: vec![http],
        owned_stores: vec![],
        clock: Arc::new(SystemSearchClock),
        reranker: None,
    };
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let out = service
        .search(ScopeProposal::generic(SearchRequest {
            query: "aspirin".into(),
            engines: Some(vec!["pubchem".into()]),
            interactive_timeout_ms: Some(80),
            generate_suggestions: false,
            ..Default::default()
        }))
        .await
        .unwrap();
    assert!(out.deadline_exceeded);
    assert_eq!(out.engine_outcomes[0].status, "unavailable");
    assert!(out.results.is_empty());
    assert_eq!(seen.lock().unwrap().len(), 2);
    service.shutdown().await;
    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn exceptional_raw_timers_use_finite_context_and_direct_refusal_is_explicit() {
    for name in ["clinicaltrials", "europepmc", "pubchem", "uniprot"] {
        for value in [-1., f64::NEG_INFINITY, f64::INFINITY, 1e300] {
            for contextual in [false, true] {
                let replay = Arc::new(Replay {
                    seen: Mutex::new(vec![]),
                    replies: Mutex::new(VecDeque::new()),
                });
                let http = Arc::new(AdapterHttp::default());
                http.set_transport(replay.clone()).unwrap();
                let mut cfg = BiomedicalConfig::defaults("https://example.invalid");
                cfg.timeout_ms = ConfigScalar::Float(value);
                let engine = adapter(name, http.clone(), Arc::new(AllowAllAdmission), cfg);
                if value > 0. && contextual {
                    // Real HTTPX loopback source controls establish success with these values.
                    // Native loopback verifies the finite caller context replaces an unrepresentable timer.
                    http.shutdown_and_wait().await;
                    let body = match name {
                        "clinicaltrials" => r#"{"studies":[]}"#,
                        "europepmc" => r#"{"resultList":{"result":[]}}"#,
                        "pubchem" => r#"{"IdentifierList":{"CID":[]}}"#,
                        _ => r#"{"results":[]}"#,
                    };
                    let (url, seen, task) =
                        server(vec![(body.into(), Duration::from_millis(25))]).await;
                    let http = Arc::new(AdapterHttp::default());
                    let mut cfg = BiomedicalConfig::defaults(&url);
                    cfg.timeout_ms = ConfigScalar::Float(value);
                    let out = adapter(name, http.clone(), Arc::new(AllowAllAdmission), cfg)
                        .search_with_context(
                            &request(),
                            SearchContext {
                                deadline: Instant::now() + Duration::from_secs(2),
                            },
                        )
                        .await;
                    assert_eq!(out.status, EngineStatus::Ok, "{name}/{value}");
                    task.await.unwrap();
                    assert_eq!(seen.lock().unwrap().len(), 1);
                    http.shutdown_and_wait().await;
                    continue;
                }
                let out = if contextual {
                    engine
                        .search_with_context(
                            &request(),
                            SearchContext {
                                deadline: Instant::now() + Duration::from_secs(2),
                            },
                        )
                        .await
                } else {
                    engine.search(&request()).await
                };
                assert!(replay.seen.lock().unwrap().is_empty());
                if value <= 0. {
                    assert_eq!(out.status, EngineStatus::Timeout);
                    assert_eq!(
                        out.error_message.as_deref(),
                        if name == "europepmc" { Some("") } else { None }
                    );
                } else {
                    assert_eq!(out.status, EngineStatus::Error);
                    assert_eq!(
                        out.error_message.as_deref(),
                        Some("timeout requires a finite lifecycle deadline")
                    );
                }
                http.shutdown_and_wait().await;
            }
        }
    }
}
