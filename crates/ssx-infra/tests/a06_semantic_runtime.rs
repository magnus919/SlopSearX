//! Actual factory/HTTP/SourceJSON/borrowed projection; no helper executable.
#[path = "support/a06_case.rs"]
mod a06_case;
use base64::Engine as _;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchContext};
use ssx_domain::{AdapterResponse, EngineStatus};
use ssx_infra::{
    adapters::{
        arxiv_expat::AtomPool,
        scholarly_config::Scalar,
        semantic_runtime::{self, SemanticConfig},
        semantic_scholar::{self, ParseError},
        source_json,
    },
    http::{AdapterHttp, Error, ExchangeFuture, Request, Response, Transport},
};
use ssx_json::{Value, json};
use std::{
    num::NonZeroUsize,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
fn normalized(response: &AdapterResponse) -> Value {
    assert!(response.results.iter().all(|r| r.media.is_none()));
    json!({"results":response.results.iter().map(|r|json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"engines":r.engines.iter().collect::<Vec<_>>(),"score":r.score,"position":r.position,"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"tier":r.tier,"media":null,"payload":r.payload,"work_group":r.work_group})).collect::<Vec<_>>(),"status":response.status,"error_message":response.error_message,"answers":response.answers,"corrections":response.corrections,"infoboxes":response.infoboxes,"synthetic":response.synthetic})
}

#[tokio::test]
async fn actual_factory_replays_all_28_reachable_max_results_configurations() {
    let corpus: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/source-config.json"
    ))
    .unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for row in corpus["records"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        if !id.starts_with("semanticscholar.config.max_results.")
            || row["native_reachability"]
                .as_str()
                .unwrap()
                .starts_with("direct-source-only")
        {
            continue;
        }
        assert!(ids.insert(id));
        let pool = AtomPool::new(
            PathBuf::from("/nonexistent-config-json-helper"),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let script = Arc::new(Script {
            row: row.clone(),
            next: Mutex::new(0),
            pool: pool.clone(),
        });
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(script.clone()).unwrap();
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        semantic_runtime::register(
            &mut registry,
            http.clone(),
            Arc::new(Admission::default()),
            SemanticConfig {
                max_results: scalar(&row["config"]["max_results"]).unwrap(),
                ..Default::default()
            },
            pool.clone(),
        )
        .unwrap();
        let request = ssx_domain::SearchRequest {
            query: row["query"].as_str().unwrap().into(),
            ..Default::default()
        };
        let response = registry
            .instantiate("semanticscholar")
            .unwrap()
            .search_with_context(
                &request,
                SearchContext {
                    deadline: Instant::now() + Duration::from_secs(30),
                },
            )
            .await;
        assert_eq!(normalized(&response), row["output"], "{id}");
        assert_eq!(
            *script.next.lock().unwrap(),
            row["requests"].as_array().unwrap().len(),
            "{id}"
        );
        assert_eq!(pool.available_operations(), 1);
        assert_eq!(pool.available_slots(), 1);
        pool.shutdown().await.unwrap();
        http.shutdown_and_wait().await;
        a06_case::completed(
            "ssx-infra/a06_semantic_runtime/actual_factory_replays_all_28_reachable_max_results_configurations",
            "docs/rust-rewrite/a06/source-config.json",
            id,
            "default",
            "exact_outcome_request",
        );
    }
    assert_eq!(ids.len(), 28);
    let ledger: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/timeout-config-classification.json"
    ))
    .unwrap();
    assert_eq!(
        ids,
        ledger["semantic_max_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect()
    );
}
#[derive(Default)]
struct Admission {
    cooldowns: Mutex<Vec<(String, f64)>>,
}
impl SearchAdmission for Admission {
    fn admit<'a>(&'a self, _: &'a str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async { AdmissionDecision::Allowed })
    }
    fn set_upstream_cooldown<'a>(&'a self, engine: &'a str, seconds: f64) -> PortFuture<'a, ()> {
        Box::pin(async move {
            self.cooldowns
                .lock()
                .unwrap()
                .push((engine.into(), seconds));
        })
    }
}
struct Script {
    row: Value,
    next: Mutex<usize>,
    pool: AtomPool,
}
impl Transport for Script {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            assert_eq!(
                self.pool.available_slots(),
                1,
                "JSON must not reserve a child slot"
            );
            assert_eq!(
                self.pool.available_operations(),
                0,
                "HTTP must retain lifecycle admission"
            );
            let mut next = self.next.lock().unwrap();
            let i = *next;
            *next += 1;
            let expected = &self.row["requests"][i];
            let reply = &self.row["responses"][i];
            assert_eq!(
                request.method.as_str(),
                expected["method"].as_str().unwrap()
            );
            assert_eq!(
                request.url.as_str(),
                expected["url"].as_str().unwrap(),
                "{}",
                self.row["id"]
            );
            for name in ["user-agent", "x-api-key"] {
                assert_eq!(
                    request.headers.get(name).map(|v| v.as_bytes()),
                    expected["headers"][name].as_str().map(str::as_bytes),
                    "{}",
                    self.row["id"]
                );
            }
            if let Some(e) = reply["exception"].as_str() {
                return Err(if e == "timeout" {
                    Error::Deadline
                } else {
                    Error::Network
                });
            }
            let mut headers = reqwest::header::HeaderMap::new();
            for (name, value) in reply["headers"].as_object().unwrap() {
                headers.append(
                    reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                    reqwest::header::HeaderValue::from_bytes(value.as_str().unwrap().as_bytes())
                        .unwrap(),
                );
            }
            Ok(Response {
                status: reply["status"].as_u64().unwrap() as u16,
                url: request.url,
                headers,
                body: base64::engine::general_purpose::STANDARD
                    .decode(reply["body_b64"].as_str().unwrap())
                    .unwrap(),
            })
        })
    }
}
fn scalar(v: &Value) -> Option<Scalar> {
    Some(match v {
        Value::Bool(v) => Scalar::Boolean(*v),
        Value::String(v) => Scalar::Text(Arc::from(v.as_str())),
        Value::Number(v) if !v.is_f64() => Scalar::Integer(v.as_str().parse().ok()?),
        Value::Number(v) => Scalar::Float(v.as_str().parse().ok()?),
        Value::Object(v) => Scalar::Float(match v.get("$source_nonfinite_float")?.as_str()? {
            "nan" => f64::NAN,
            "inf" => f64::INFINITY,
            "-inf" => f64::NEG_INFINITY,
            _ => return None,
        }),
        _ => return None,
    })
}
#[tokio::test]
async fn actual_registered_factory_preserves_complete_original_outcomes_requests_and_cooldowns_without_helper()
 {
    let pool = AtomPool::new(
        PathBuf::from("/nonexistent-a06-semantic-helper-must-never-be-started"),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let (mut exact, mut typed, mut transport) = (Vec::new(), Vec::new(), Vec::new());
    let mut unique = std::collections::BTreeSet::new();
    use sha2::Digest;
    let selection: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/semantic-runtime-selected-ids.json"
    ))
    .unwrap();
    for (source_index, raw) in [
        include_str!("../../../docs/rust-rewrite/a06/source-initial.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-regressions.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-shared.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-json-matrix.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-selected-values.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-json-diagnostic.json"),
    ]
    .into_iter()
    .enumerate()
    {
        let digest = sha2::Sha256::digest(raw.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(
            digest,
            selection["sources"][source_index]["sha256"]
                .as_str()
                .unwrap()
        );
        let corpus: Value = ssx_json::from_str(raw).unwrap();
        for row in corpus["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["engine"].as_str() == Some("semanticscholar"))
        {
            let id = row["id"].as_str().unwrap();
            assert!(unique.insert(id.to_owned()));
            let mut config = SemanticConfig {
                wall_clock: || 1700000000.,
                ..Default::default()
            };
            if let Some(base) = row["config"]["base_url"].as_str() {
                config.base_url = Arc::from(base);
            }
            if let Some(v) = scalar(&row["config"]["max_results"]) {
                config.max_results = v;
            }
            if let Some(key) = row["config"]["api_key"].as_str() {
                config.api_key = Some(Arc::from(key));
            }
            let admission = Arc::new(Admission::default());
            let script = Arc::new(Script {
                row: row.clone(),
                next: Mutex::new(0),
                pool: pool.clone(),
            });
            let http = Arc::new(AdapterHttp::default());
            http.set_transport(script.clone()).unwrap();
            let mut registry = ssx_registry::Registry::builtins().unwrap();
            semantic_runtime::register(
                &mut registry,
                http.clone(),
                admission.clone(),
                config,
                pool.clone(),
            )
            .unwrap();
            assert!(registry.is_available("semanticscholar"));
            let actual = registry
                .instantiate("semanticscholar")
                .unwrap()
                .search_with_context(
                    &ssx_domain::SearchRequest {
                        query: row["query"].as_str().unwrap().into(),
                        ..Default::default()
                    },
                    SearchContext {
                        deadline: Instant::now() + Duration::from_secs(30),
                    },
                )
                .await;
            assert_eq!(
                *script.next.lock().unwrap(),
                row["requests"].as_array().unwrap().len(),
                "{id}"
            );
            assert_eq!(pool.available_slots(), 1);
            assert_eq!(pool.available_operations(), 1);
            let reply = &row["responses"][0];
            let mut policy = None;
            if reply["status"].as_u64() == Some(200) {
                let body = base64::engine::general_purpose::STANDARD
                    .decode(reply["body_b64"].as_str().unwrap())
                    .unwrap();
                if let Ok(data) = source_json::from_slice(&body, None).await
                    && let Err(ParseError::Output(m)) =
                        semantic_scholar::parse(&data, Instant::now() + Duration::from_secs(30))
                            .await
                {
                    policy = Some(m);
                }
            }
            if let Some(message) = policy {
                assert_eq!(actual.status, EngineStatus::Error);
                assert_eq!(
                    actual.error_message.as_deref(),
                    Some(message.as_str()),
                    "{id}"
                );
                assert!(actual.results.is_empty());
                typed.push(id.to_owned());
            } else if id == "semanticscholar.connect" {
                assert_eq!(
                    actual.error_message.as_deref(),
                    Some("Semantic Scholar HTTP transport network failure")
                );
                transport.push(id.to_owned());
            } else {
                assert_eq!(normalized(&actual), row["output"], "{id}");
                exact.push(id.to_owned());
            }
            if reply["status"].as_u64() == Some(429) {
                let log = admission.cooldowns.lock().unwrap();
                assert_eq!(log.len(), 1);
                assert_eq!(log[0].0, "semanticscholar");
                let raw = reply["headers"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
                    .and_then(|(_, v)| v.as_str());
                let controls: Value = ssx_json::from_str(include_str!(
                    "../../../docs/rust-rewrite/a06/source-retry-after-bounded.json"
                ))
                .unwrap();
                let expected = controls["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["raw"].as_str() == raw)
                    .unwrap();
                assert_eq!(
                    format!("{:016x}", log[0].1.to_bits()),
                    expected["bits"].as_str().unwrap(),
                    "{id}"
                );
            } else {
                assert!(admission.cooldowns.lock().unwrap().is_empty());
            }
            http.shutdown_and_wait().await;
            a06_case::completed(
                "ssx-infra/a06_semantic_runtime/actual_registered_factory_preserves_complete_original_outcomes_requests_and_cooldowns_without_helper",
                selection["sources"][source_index]["path"].as_str().unwrap(),
                id,
                "default",
                if typed.iter().any(|v| v == id) {
                    "typed_refusal"
                } else if transport.iter().any(|v| v == id) {
                    "transport_difference"
                } else {
                    "exact_outcome_request"
                },
            );
        }
    }
    println!(
        "exact {} {:?}; typed {} {:?}; transport {} {:?}",
        exact.len(),
        exact,
        typed.len(),
        typed,
        transport.len(),
        transport
    );
    for (name, actual) in [
        ("exact_ids", &exact),
        ("typed_ids", &typed),
        ("transport_ids", &transport),
    ] {
        assert_eq!(
            actual.iter().map(String::as_str).collect::<Vec<_>>(),
            selection[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>()
        );
    }
    pool.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn actual_adapter_loopback_http_cancellation_shutdown_and_parent_deadline() {
    use tokio::{io::AsyncReadExt, net::TcpListener, sync::oneshot};
    for action in ["cancel", "shutdown", "deadline"] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (sent, seen) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut bytes = [0; 1024];
            loop {
                let n = socket.read(&mut bytes).await.unwrap();
                assert_ne!(n, 0);
                request.extend_from_slice(&bytes[..n]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
                assert!(request.len() < 8192);
            }
            sent.send(request).unwrap();
            assert_eq!(socket.read(&mut bytes).await.unwrap(), 0);
        });
        let parser = AtomPool::new(
            PathBuf::from("/nonexistent-a06-semantic-helper-must-never-be-started"),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let http = Arc::new(AdapterHttp::default());
        let config = SemanticConfig {
            base_url: Arc::from(format!("http://{address}/atom")),
            timeout_ms: Scalar::Float(f64::INFINITY),
            transport: ssx_infra::http::Options {
                trust_env: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        semantic_runtime::register(
            &mut registry,
            http.clone(),
            Arc::new(Admission::default()),
            config,
            parser.clone(),
        )
        .unwrap();
        let adapter = registry.instantiate("semanticscholar").unwrap();
        let deadline = Instant::now() + Duration::from_secs(3600);
        let job = tokio::spawn(async move {
            adapter
                .search_with_context(
                    &ssx_domain::SearchRequest::default(),
                    SearchContext { deadline },
                )
                .await
        });
        let request = tokio::time::timeout(Duration::from_secs(5), seen)
            .await
            .unwrap()
            .unwrap();
        let request = String::from_utf8(request).unwrap();
        assert!(request.starts_with("GET /atom?query=&limit=5&fields=title%2Curl%2Cabstract%2CcitationCount%2CpublicationDate%2CexternalIds%2Cauthors HTTP/1.1\r\n"));
        let lower = request.to_ascii_lowercase();
        assert!(
            lower.contains("user-agent: slopsearx/0.1.0 (meta search engine; agent-native)\r\n")
        );
        assert_eq!(parser.available_operations(), 0);
        assert_eq!(parser.available_slots(), 1);
        match action {
            "cancel" => {
                job.abort();
                assert!(job.await.unwrap_err().is_cancelled());
            }
            "shutdown" => {
                tokio::time::timeout(Duration::from_secs(5), parser.shutdown())
                    .await
                    .unwrap()
                    .unwrap();
                let response = job.await.unwrap();
                assert_eq!(response.status, ssx_domain::EngineStatus::Error);
                assert_eq!(
                    response.error_message.as_deref(),
                    Some("Semantic Scholar lifecycle shutdown")
                );
            }
            "deadline" => {
                tokio::time::pause();
                tokio::time::advance(Duration::from_secs(3601)).await;
                let response = job.await.unwrap();
                tokio::time::resume();
                assert_eq!(response.status, ssx_domain::EngineStatus::Timeout);
            }
            _ => unreachable!(),
        }
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(parser.available_operations(), 1);
        parser.shutdown().await.unwrap();
        http.shutdown_and_wait().await;
    }
}

struct StalledCooldown {
    seen: tokio::sync::Notify,
}
impl SearchAdmission for StalledCooldown {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async { AdmissionDecision::Allowed })
    }
    fn set_upstream_cooldown<'a>(
        &'a self,
        engine: &'static str,
        seconds: f64,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            assert_eq!(engine, "semanticscholar");
            assert_eq!(seconds, 30.);
            self.seen.notify_one();
            std::future::pending().await
        })
    }
}
#[tokio::test(flavor = "current_thread")]
async fn actual_cooldown_persistence_stall_remains_cancellable_and_shutdown_owned() {
    let corpus: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/source-initial.json"
    ))
    .unwrap();
    let row = corpus["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"].as_str() == Some("semanticscholar.http_429"))
        .unwrap();
    for action in ["cancel", "shutdown", "deadline"] {
        let pool = AtomPool::new(
            PathBuf::from("/nonexistent-semantic-stalled-cooldown-helper"),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(Arc::new(Script {
            row: row.clone(),
            next: Mutex::new(0),
            pool: pool.clone(),
        }))
        .unwrap();
        let admission = Arc::new(StalledCooldown {
            seen: tokio::sync::Notify::new(),
        });
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        semantic_runtime::register(
            &mut registry,
            http.clone(),
            admission.clone(),
            SemanticConfig {
                timeout_ms: Scalar::Float(f64::INFINITY),
                ..Default::default()
            },
            pool.clone(),
        )
        .unwrap();
        let adapter = registry.instantiate("semanticscholar").unwrap();
        let query = row["query"].as_str().unwrap().to_owned();
        let deadline = Instant::now() + Duration::from_secs(3600);
        let job = tokio::spawn(async move {
            adapter
                .search_with_context(
                    &ssx_domain::SearchRequest {
                        query,
                        ..Default::default()
                    },
                    SearchContext { deadline },
                )
                .await
        });
        tokio::time::timeout(Duration::from_secs(5), admission.seen.notified())
            .await
            .unwrap();
        assert_eq!(pool.available_operations(), 0);
        assert_eq!(pool.available_slots(), 1);
        match action {
            "cancel" => {
                job.abort();
                assert!(job.await.unwrap_err().is_cancelled());
            }
            "shutdown" => {
                tokio::time::timeout(Duration::from_secs(5), pool.shutdown())
                    .await
                    .unwrap()
                    .unwrap();
                let actual = job.await.unwrap();
                assert_eq!(actual.status, EngineStatus::Error);
                assert_eq!(
                    actual.error_message.as_deref(),
                    Some("Semantic Scholar lifecycle shutdown")
                );
            }
            "deadline" => {
                tokio::time::pause();
                tokio::time::advance(Duration::from_secs(3601)).await;
                let actual = job.await.unwrap();
                tokio::time::resume();
                assert_eq!(actual.status, EngineStatus::Timeout);
            }
            _ => unreachable!(),
        }
        assert_eq!(pool.available_operations(), 1);
        assert_eq!(pool.available_slots(), 1);
        pool.shutdown().await.unwrap();
        http.shutdown_and_wait().await;
    }
}
