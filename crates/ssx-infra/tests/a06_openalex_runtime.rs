//! Actual registered OpenAlex runtime: no helper process, explicit header gap.
#[path = "support/a06_case.rs"]
mod a06_case;
use base64::Engine as _;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchContext};
use ssx_domain::{AdapterResponse, EngineStatus};
use ssx_infra::{
    adapters::{
        arxiv_expat::AtomPool,
        openalex_runtime::{self, OpenAlexConfig},
        scholarly_config::Scalar,
    },
    http::{AdapterHttp, Error, ExchangeFuture, Request, Response, Transport},
};
use ssx_json::{Value, json};
use std::{
    num::NonZeroUsize,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
fn normalized(response: &AdapterResponse) -> Value {
    assert!(response.results.iter().all(|r| r.media.is_none()));
    json!({"results":response.results.iter().map(|r|json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"engines":r.engines.iter().collect::<Vec<_>>(),"score":r.score,"position":r.position,"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"tier":r.tier,"media":null,"payload":r.payload,"work_group":r.work_group})).collect::<Vec<_>>(),"status":response.status,"error_message":response.error_message,"answers":response.answers,"corrections":response.corrections,"infoboxes":response.infoboxes,"synthetic":response.synthetic})
}
#[derive(Default)]
struct Admission {
    calls: AtomicUsize,
}
impl SearchAdmission for Admission {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            AdmissionDecision::Allowed
        })
    }
}
struct Script {
    row: Value,
    count: AtomicUsize,
    pool: AtomPool,
}
impl Transport for Script {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            assert_eq!(self.pool.available_slots(), 1);
            assert_eq!(self.pool.available_operations(), 0);
            let n = self.count.fetch_add(1, Ordering::SeqCst);
            assert_eq!(n, 0);
            let expected = &self.row["requests"][0];
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
            // The source has no engine-owned headers. Source HTTPX default UA differs
            // from reviewed native transport; assert both identities, not header parity.
            assert_eq!(
                expected["headers"]["user-agent"].as_str(),
                Some("python-httpx/0.28.1")
            );
            assert_eq!(
                request.headers["user-agent"].as_bytes(),
                ssx_infra::http::DEFAULT_USER_AGENT.as_bytes()
            );
            assert_ne!(
                request.headers["user-agent"].as_bytes(),
                expected["headers"]["user-agent"]
                    .as_str()
                    .unwrap()
                    .as_bytes()
            );
            let reply = &self.row["responses"][0];
            if let Some(e) = reply["exception"].as_str() {
                return Err(if e == "timeout" {
                    Error::Deadline
                } else {
                    Error::Network
                });
            }
            let mut headers = reqwest::header::HeaderMap::new();
            for (k, v) in reply["headers"].as_object().unwrap() {
                headers.append(
                    reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                    reqwest::header::HeaderValue::from_bytes(v.as_str().unwrap().as_bytes())
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
async fn actual_factory_preserves_original_outcomes_urls_dates_and_reachable_max_configs() {
    let pool = AtomPool::new(
        PathBuf::from("/nonexistent-openalex-json-helper"),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let (mut exact, mut typed, mut transport) = (Vec::new(), Vec::new(), Vec::new());
    let mut unique = std::collections::BTreeSet::new();
    use sha2::Digest;
    let selection: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/openalex-runtime-selected-ids.json"
    ))
    .unwrap();
    for (source_index, raw) in [
        include_str!("../../../docs/rust-rewrite/a06/source-initial.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-regressions.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-shared.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-json-matrix.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-openalex-order.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-http-status-boundaries.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-config.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-openalex-actual-dates.json"),
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
            .filter(|r| r["engine"].as_str() == Some("openalex"))
        {
            let id = row["id"].as_str().unwrap();
            if id.starts_with("openalex.config.")
                && (!id.contains(".max_results.")
                    || row["native_reachability"]
                        .as_str()
                        .unwrap()
                        .starts_with("direct-source-only"))
            {
                continue;
            }
            assert!(unique.insert(id.to_owned()));
            let mut config = OpenAlexConfig::default();
            if let Some(base) = row["config"]["base_url"].as_str() {
                config.base_url = Arc::from(base);
            }
            if let Some(limit) = scalar(&row["config"]["max_results"]) {
                config.max_results = limit;
            }
            let admission = Arc::new(Admission::default());
            let script = Arc::new(Script {
                row: row.clone(),
                count: AtomicUsize::new(0),
                pool: pool.clone(),
            });
            let http = Arc::new(AdapterHttp::default());
            http.set_transport(script.clone()).unwrap();
            let mut registry = ssx_registry::Registry::builtins().unwrap();
            openalex_runtime::register(
                &mut registry,
                http.clone(),
                admission.clone(),
                config,
                pool.clone(),
            )
            .unwrap();
            let request = ssx_domain::SearchRequest {
                query: row["query"].as_str().unwrap().into(),
                date_from: row["params"]["date_from"].as_str().map(str::to_owned),
                date_to: row["params"]["date_to"].as_str().map(str::to_owned),
                time_range: row["params"]["time_range"].as_str().map(str::to_owned),
                ..Default::default()
            };
            let actual = registry
                .instantiate("openalex")
                .unwrap()
                .search_with_context(
                    &request,
                    SearchContext {
                        deadline: Instant::now() + Duration::from_secs(30),
                    },
                )
                .await;
            assert_eq!(
                script.count.load(Ordering::SeqCst),
                row["requests"].as_array().unwrap().len(),
                "{id}"
            );
            assert_eq!(
                admission.calls.load(Ordering::SeqCst),
                usize::from(!row["requests"].as_array().unwrap().is_empty()),
                "preflight before admission {id}"
            );
            assert_eq!(pool.available_operations(), 1);
            assert_eq!(pool.available_slots(), 1);
            if actual.error_message.as_deref().is_some_and(|m| {
                m.starts_with("OpenAlex result") || m.starts_with("OpenAlex payload")
            }) {
                assert_eq!(
                    row["output"]["status"].as_str(),
                    Some("ok"),
                    "source error precedence {id}"
                );
                assert_eq!(actual.status, EngineStatus::Error);
                assert!(actual.results.is_empty());
                typed.push((id.to_owned(), actual.error_message.unwrap()));
            } else if id == "openalex.connect" {
                assert_eq!(
                    actual.error_message.as_deref(),
                    Some("OpenAlex HTTP transport network failure")
                );
                transport.push(id.to_owned());
            } else if id == "openalex.timeout" {
                assert_eq!(actual.status, EngineStatus::Timeout);
                assert!(actual.error_message.is_none());
                transport.push(id.to_owned());
            } else {
                assert_eq!(normalized(&actual), row["output"], "{id}");
                exact.push(id.to_owned());
            }
            http.shutdown_and_wait().await;
            a06_case::completed(
                "ssx-infra/a06_openalex_runtime/actual_factory_preserves_original_outcomes_urls_dates_and_reachable_max_configs",
                selection["sources"][source_index]["path"].as_str().unwrap(),
                id,
                "default",
                if typed.iter().any(|(v, _)| v == id) {
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
    for (name, actual) in [("exact_ids", &exact), ("transport_ids", &transport)] {
        assert_eq!(
            actual.iter().map(String::as_str).collect::<Vec<_>>(),
            selection[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r.as_str().unwrap())
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(
        typed
            .iter()
            .map(|(id, m)| (id.as_str(), m.as_str()))
            .collect::<Vec<_>>(),
        selection["typed_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (r[0].as_str().unwrap(), r[1].as_str().unwrap()))
            .collect::<Vec<_>>()
    );
    pool.shutdown().await.unwrap();
}

#[tokio::test]
async fn real_loopback_date_requests_filter_only_proven_publication_dates_and_slice_first() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let corpus: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/source-openalex-actual-dates.json"
    ))
    .unwrap();
    for suffix in [
        "lower_leap",
        "upper_leap",
        "equal_leap",
        "slice_before_filter",
        "guard_before_malformed",
    ] {
        let row = corpus["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"].as_str().unwrap().ends_with(suffix))
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let body = base64::engine::general_purpose::STANDARD
            .decode(row["responses"][0]["body_b64"].as_str().unwrap())
            .unwrap();
        let source_url = row["requests"][0]["url"].as_str().unwrap();
        let expected_path = source_url
            .strip_prefix("https://api.openalex.org")
            .unwrap()
            .to_owned();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 1024];
            let mut request = Vec::new();
            loop {
                let n = socket.read(&mut bytes).await.unwrap();
                assert_ne!(n, 0);
                request.extend_from_slice(&bytes[..n]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
                assert!(request.len() < 8192);
            }
            let request = String::from_utf8(request).unwrap();
            assert!(request.starts_with(&format!("GET {expected_path} HTTP/1.1\r\n")));
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(headers.as_bytes()).await.unwrap();
            socket.write_all(&body).await.unwrap();
        });
        let pool = AtomPool::new(
            PathBuf::from("/nonexistent-openalex-date-loopback-helper"),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let http = Arc::new(AdapterHttp::default());
        let admission = Arc::new(Admission::default());
        let config = OpenAlexConfig {
            base_url: Arc::from(format!("http://{address}")),
            max_results: scalar(&row["config"]["max_results"]).unwrap_or(Scalar::Integer(10)),
            transport: ssx_infra::http::Options {
                trust_env: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        openalex_runtime::register(
            &mut registry,
            http.clone(),
            admission.clone(),
            config,
            pool.clone(),
        )
        .unwrap();
        let actual = registry
            .instantiate("openalex")
            .unwrap()
            .search_with_context(
                &ssx_domain::SearchRequest {
                    query: row["query"].as_str().unwrap().into(),
                    date_from: row["params"]["date_from"].as_str().map(str::to_owned),
                    date_to: row["params"]["date_to"].as_str().map(str::to_owned),
                    ..Default::default()
                },
                SearchContext {
                    deadline: Instant::now() + Duration::from_secs(30),
                },
            )
            .await;
        assert_eq!(normalized(&actual), row["output"], "{suffix}");
        assert_eq!(admission.calls.load(Ordering::SeqCst), 1);
        assert_eq!(pool.available_slots(), 1);
        assert_eq!(pool.available_operations(), 1);
        server.await.unwrap();
        pool.shutdown().await.unwrap();
        http.shutdown_and_wait().await;
    }
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
        let config = OpenAlexConfig {
            base_url: Arc::from(format!("http://{address}")),
            timeout_ms: Scalar::Float(f64::INFINITY),
            transport: ssx_infra::http::Options {
                trust_env: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        openalex_runtime::register(
            &mut registry,
            http.clone(),
            Arc::new(Admission::default()),
            config,
            parser.clone(),
        )
        .unwrap();
        let adapter = registry.instantiate("openalex").unwrap();
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
        assert!(request.starts_with("GET /works?search=&per_page=10 HTTP/1.1\r\n"));
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
                    Some("OpenAlex lifecycle shutdown")
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
