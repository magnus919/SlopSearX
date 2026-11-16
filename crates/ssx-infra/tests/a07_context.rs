//! Real A07 adapters under the accepted F14 caller-owned context.
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
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
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
    for (name, first, second) in [
        ("pubchem", PC_SEARCH, PC_DETAIL),
        ("pubmed", PM_SEARCH, PM_SUMMARY),
    ] {
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
        let http = Arc::new(AdapterHttp::default());
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
    use ssx_application::{cache::RoutingIdentity, health::OutcomeOrigin, policy::*, scope::*};
    use ssx_infra::search::*;
    use std::{collections::BTreeMap, future::Future, task::Poll};

    // Only the trusted service owner is driven. The real AdapterHttp and its
    // absolute context/parser/network clocks remain System. A future anchor
    // keeps that context live while bounded real loopback stages run; this is
    // an 80ms logical service deadline control, not an 80ms wall-time claim.
    struct OwnerClock(tokio::sync::watch::Sender<tokio::time::Instant>);
    impl OwnerClock {
        fn advance_to(&self, now: tokio::time::Instant) {
            assert!(now >= *self.0.borrow());
            let _ = self.0.send_replace(now);
        }
    }
    impl SearchDeadlineClock for OwnerClock {
        fn now(&self) -> tokio::time::Instant {
            *self.0.borrow()
        }
        fn sleep_until(&self, deadline: tokio::time::Instant) -> PortFuture<'_, ()> {
            Box::pin(async move {
                let mut now = self.0.subscribe();
                loop {
                    if *now.borrow_and_update() >= deadline {
                        return;
                    }
                    now.changed().await.expect("fixture owner retained");
                }
            })
        }
    }
    struct ContextForwarder {
        engine: Arc<dyn SearchEngine>,
        deadlines: Arc<Mutex<Vec<Instant>>>,
        legacy: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl SearchEngine for ContextForwarder {
        fn search<'a>(
            &'a self,
            request: &'a SearchRequest,
        ) -> PortFuture<'a, ssx_domain::AdapterResponse> {
            self.legacy
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.engine.search(request)
        }
        fn search_with_context<'a>(
            &'a self,
            request: &'a SearchRequest,
            context: SearchContext,
        ) -> PortFuture<'a, ssx_domain::AdapterResponse> {
            self.deadlines.lock().unwrap().push(context.deadline);
            self.engine.search_with_context(request, context)
        }
    }
    #[derive(Default)]
    struct Observations(Mutex<Vec<(String, EngineStatus, bool, OutcomeOrigin)>>);
    impl ssx_application::Metrics for Observations {
        fn increment(&self, _: &'static str) {}
        fn engine_outcome(
            &self,
            name: &str,
            response: &ssx_domain::AdapterResponse,
            origin: OutcomeOrigin,
        ) {
            self.0.lock().unwrap().push((
                name.into(),
                response.status.clone(),
                response.synthetic,
                origin,
            ));
        }
    }
    async fn request_headers(stream: &mut tokio::net::TcpStream) -> Result<String, String> {
        let mut bytes = Vec::new();
        loop {
            let mut buf = [0; 1024];
            let n = stream.read(&mut buf).await.map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("EOF before complete request headers".into());
            }
            bytes.extend_from_slice(&buf[..n]);
            if bytes.len() > 8192 {
                return Err("fixture request headers exceed bound".into());
            }
            if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                if bytes.len() != end + 4 {
                    return Err("unexpected GET request body".into());
                }
                let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
                let mut length_seen = false;
                for line in text.lines().skip(1) {
                    if let Some((name, value)) = line.split_once(':') {
                        if name.eq_ignore_ascii_case("transfer-encoding") {
                            return Err("unexpected GET transfer encoding".into());
                        }
                        if name.eq_ignore_ascii_case("content-length") {
                            if length_seen || value.trim() != "0" {
                                return Err("unexpected GET content length".into());
                            }
                            length_seen = true;
                        }
                    }
                }
                return Ok(text);
            }
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let requests = seen.clone();
    let (entered_tx, mut entered_rx) = tokio::sync::oneshot::channel();
    let server = async move {
        for stage in 0..2 {
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
                .await
                .map_err(|_| "accept guard elapsed".to_owned())?
                .map_err(|e| e.to_string())?;
            let request =
                tokio::time::timeout(Duration::from_secs(2), request_headers(&mut stream))
                    .await
                    .map_err(|_| "headers guard elapsed".to_owned())??;
            let path = if stage == 0 {
                "/compound/name/aspirin/cids/JSON"
            } else {
                "/compound/cid/2244,962/JSON"
            };
            if !request.starts_with(&format!("GET {path} HTTP/1.1\r\n")) {
                return Err(format!("unexpected PubChem stage {stage} request"));
            }
            requests.lock().unwrap().push(request);
            if stage == 0 {
                // Actual first response is released immediately. The second
                // response remains unavailable until the service owns expiry.
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    PC_SEARCH.len()
                );
                tokio::time::timeout(Duration::from_secs(2), async {
                    stream.write_all(header.as_bytes()).await?;
                    stream.write_all(PC_SEARCH.as_bytes()).await
                })
                .await
                .map_err(|_| "first response guard elapsed".to_owned())?
                .map_err(|e| e.to_string())?;
            } else {
                entered_tx
                    .send(())
                    .map_err(|_| "second-stage observer dropped".to_owned())?;
                let mut byte = [0; 1];
                let n = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut byte))
                    .await
                    .map_err(|_| "second socket EOF guard elapsed".to_owned())?
                    .map_err(|e| e.to_string())?;
                if n != 0 {
                    return Err("unexpected bytes instead of second socket EOF".into());
                }
                return Ok(());
            }
        }
        Err("second stage was not observed".into())
    };
    let http = Arc::new(AdapterHttp::default());
    let deadlines = Arc::new(Mutex::new(Vec::new()));
    let legacy = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let engine: Arc<dyn SearchEngine> = Arc::new(ContextForwarder {
        engine: adapter(
            "pubchem",
            http.clone(),
            Arc::new(AllowAllAdmission),
            BiomedicalConfig::defaults(&base),
        ),
        deadlines: deadlines.clone(),
        legacy: legacy.clone(),
    });
    let observations = Arc::new(Observations::default());
    let anchor = tokio::time::Instant::now() + Duration::from_secs(60);
    let owner = Arc::new(OwnerClock(tokio::sync::watch::channel(anchor).0));
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
        metrics: observations.clone(),
        supervisor: Arc::new(NoopSink),
        audit: Arc::new(NoopSink),
        cooldowns: None,
        owned_http: vec![http.clone()],
        owned_stores: vec![],
        clock: Arc::new(SystemSearchClock),
        reranker: None,
    };
    let service = SearchService::new(SearchConfig::default(), deps)
        .unwrap()
        .with_deadline_clock(owner.clone())
        .unwrap();
    let proposal = ScopeProposal::generic(SearchRequest {
        query: "aspirin".into(),
        engines: Some(vec!["pubchem".into()]),
        interactive_timeout_ms: Some(80),
        generate_suggestions: false,
        ..Default::default()
    });
    let mut search = Box::pin(service.search(proposal));
    let client = async {
        tokio::select! {
            biased;
            _early = search.as_mut() => { return Err("service completed before held real second request".to_owned()); }
            entered = &mut entered_rx => entered.map_err(|_| "second-stage barrier dropped".to_owned())?,
        }
        if *deadlines.lock().unwrap() != vec![(anchor + Duration::from_millis(80)).into_std()] {
            return Err("F14_CONTEXT_FORWARD: exact 80ms owner deadline was not forwarded".into());
        }
        let pending =
            std::future::poll_fn(|cx| Poll::Ready(search.as_mut().poll(cx).is_pending())).await;
        if !pending {
            return Err("F14_CONTEXT_HELD: actual second response must still be pending before owner advance".into());
        }
        owner.advance_to(anchor + Duration::from_millis(79));
        let pending =
            std::future::poll_fn(|cx| Poll::Ready(search.as_mut().poll(cx).is_pending())).await;
        if !pending {
            return Err("F14_CONTEXT_PREEXPIRY: held second response completed before80ms".into());
        }
        owner.advance_to(anchor + Duration::from_millis(80));
        search
            .as_mut()
            .await
            .map_err(|e| format!("service search: {e:?}"))
    };
    let joined = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(client, server)
    })
    .await;
    // The bounded join owns both local futures. Cancellation drops their
    // listeners/sockets, then the real service/HTTP owners are drained.
    drop(search);
    let cleanup = tokio::time::timeout(Duration::from_secs(5), async {
        service.shutdown().await;
        http.shutdown_and_wait().await;
    })
    .await;
    cleanup.expect("F14 context fixture cleanup guard");
    let (out, server_result) = joined.expect("F14 context fixture deadlock guard");
    server_result.unwrap();
    let out = out.unwrap();
    assert!(out.deadline_exceeded);
    assert_eq!(out.engine_outcomes[0].status, "unavailable");
    assert!(out.results.is_empty());
    assert_eq!(out.dispatched_engine_count, 1);
    assert_eq!(
        out.engine_outcomes[0].message.as_deref(),
        Some("interactive deadline reached")
    );
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert_eq!(legacy.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(
        *observations.0.lock().unwrap(),
        vec![(
            "pubchem".into(),
            EngineStatus::Unavailable,
            true,
            OutcomeOrigin::CallerCancelled
        )]
    );
    println!(
        "F14_CONTEXT_CAUSAL two_headers=true logical_deadline_ms=80 binding_ms=500 real_http_clock=System origin=CallerCancelled synthetic=true"
    );
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
