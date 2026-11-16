//! Actual registered arXiv adapter + F08 injection + supervised native helper.
#[path = "support/a06_case.rs"]
mod a06_case;
use base64::Engine as _;
use ssx_application::SearchContext;
use ssx_domain::AdapterResponse;
use ssx_infra::{
    adapters::{
        AllowAllAdmission, arxiv_expat::AtomPool, arxiv_runtime::ArxivConfig,
        scholarly_config::Scalar,
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
fn normalize(response: &AdapterResponse) -> Value {
    assert!(response.results.iter().all(|r| r.media.is_none()));
    json!({"results":response.results.iter().map(|r|json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"engines":r.engines.iter().collect::<Vec<_>>(),"score":r.score,"position":r.position,"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"tier":r.tier,"media":null,"payload":r.payload,"work_group":r.work_group})).collect::<Vec<_>>(),"status":response.status,"error_message":response.error_message,"answers":response.answers,"corrections":response.corrections,"infoboxes":response.infoboxes,"synthetic":response.synthetic})
}
struct Script {
    row: Value,
    next: Mutex<usize>,
}
impl Transport for Script {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let mut next = self.next.lock().unwrap();
            let index = *next;
            *next += 1;
            let expected = &self.row["requests"][index];
            let reply = &self.row["responses"][index];
            assert_eq!(
                request.method.as_str(),
                expected["method"].as_str().unwrap(),
                "{}",
                self.row["id"]
            );
            if self.row["id"].as_str() == Some("arxiv.redirect_boundary.ipv6_spelling") {
                // Explicit F08 wire mismatch: preserve both exact identities.
                // Source policy must still reject raw IPv6 spelling aliases.
                assert_eq!(
                    expected["url"].as_str().unwrap(),
                    "https://[0:0:0:0:0:0:0:1]/api/query?search_query=all%3Atest&start=0&max_results=5&sortBy=relevance&sortOrder=descending"
                );
                assert_eq!(
                    request.url.as_str(),
                    "https://[::1]/api/query?search_query=all%3Atest&start=0&max_results=5&sortBy=relevance&sortOrder=descending"
                );
                assert_ne!(request.url.as_str(), expected["url"].as_str().unwrap());
            } else {
                assert_eq!(
                    request.url.as_str(),
                    expected["url"].as_str().unwrap(),
                    "{}",
                    self.row["id"]
                );
            }
            for name in ["user-agent", "accept"] {
                assert_eq!(
                    request.headers[name].as_bytes(),
                    expected["headers"][name].as_str().unwrap().as_bytes(),
                    "{}",
                    self.row["id"]
                );
            }
            if let Some(kind) = reply["exception"].as_str() {
                return Err(if kind == "timeout" {
                    Error::Deadline
                } else {
                    Error::Network
                });
            }
            let mut headers = reqwest::header::HeaderMap::new();
            if let Some(pairs) = reply["headers_b64"].as_array() {
                for pair in pairs {
                    let name = base64::engine::general_purpose::STANDARD
                        .decode(pair[0].as_str().unwrap())
                        .unwrap();
                    let value = base64::engine::general_purpose::STANDARD
                        .decode(pair[1].as_str().unwrap())
                        .unwrap();
                    headers.append(
                        reqwest::header::HeaderName::from_bytes(&name).unwrap(),
                        reqwest::header::HeaderValue::from_bytes(&value).unwrap(),
                    );
                }
            } else if let Some(values) = reply["headers"].as_object() {
                for (name, value) in values {
                    headers.append(
                        reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                        reqwest::header::HeaderValue::from_bytes(
                            value.as_str().unwrap().as_bytes(),
                        )
                        .unwrap(),
                    );
                }
            }
            Ok(Response {
                status: reply["status"].as_u64().unwrap() as u16,
                url: request.url,
                headers,
                body: base64::engine::general_purpose::STANDARD
                    .decode(reply["body_b64"].as_str().unwrap_or(""))
                    .unwrap(),
            })
        })
    }
}
fn scalar(value: &Value) -> Option<Scalar> {
    Some(match value {
        Value::Bool(v) => Scalar::Boolean(*v),
        Value::String(v) => Scalar::Text(Arc::from(v.as_str())),
        Value::Number(v) if !v.is_f64() => Scalar::Integer(v.as_str().parse().ok()?),
        Value::Number(v) => Scalar::Float(v.as_str().parse().ok()?),
        Value::Object(value) => {
            Scalar::Float(match value.get("$source_nonfinite_float")?.as_str()? {
                "nan" => f64::NAN,
                "inf" => f64::INFINITY,
                "-inf" => f64::NEG_INFINITY,
                _ => return None,
            })
        }
        _ => return None,
    })
}
fn wire_reachable(row: &Value) -> bool {
    row["responses"]
        .as_array()
        .unwrap()
        .iter()
        .take(row["requests"].as_array().unwrap().len())
        .all(|reply| {
            reply["headers_b64"].as_array().is_none_or(|pairs| {
                pairs.iter().all(|pair| {
                    let name = base64::engine::general_purpose::STANDARD
                        .decode(pair[0].as_str().unwrap())
                        .unwrap();
                    let value = base64::engine::general_purpose::STANDARD
                        .decode(pair[1].as_str().unwrap())
                        .unwrap();
                    reqwest::header::HeaderName::from_bytes(&name).is_ok()
                        && reqwest::header::HeaderValue::from_bytes(&value).is_ok()
                })
            })
        })
}
#[tokio::test]
async fn actual_http_helper_path_preserves_source_outcomes_and_requests() {
    use sha2::Digest;
    let selection: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/arxiv-runtime-selected-ids.json"
    ))
    .unwrap();
    let mut eligible = Vec::new();
    let mut unique = std::collections::BTreeSet::new();
    let parser = AtomPool::new(
        PathBuf::from(env!("CARGO_BIN_EXE_ssx-atom-parser")),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let mut selected = Vec::new();
    let mut transport_boundary = Vec::new();
    let mut deltas = Vec::new();
    for (source_index, raw) in [
        include_str!("../../../docs/rust-rewrite/a06/source-charset.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-atom-decode.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-xml.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-initial.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-regressions.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-shared.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-xml-grammar.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-config.json"),
    ]
    .into_iter()
    .enumerate()
    {
        let digest = sha2::Sha256::digest(raw.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
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
            .filter(|row| row["engine"].as_str() == Some("arxiv"))
        {
            let id = row["id"].as_str().unwrap();
            if id.starts_with("arxiv.config.")
                && (row["config"]["max_results"].is_null()
                    || row["native_reachability"]
                        .as_str()
                        .unwrap()
                        .starts_with("direct-source-only"))
            {
                continue;
            }
            assert!(
                unique.insert(id.to_owned()),
                "duplicate selected source ID {id}"
            );
            eligible.push(id.to_owned());
            if !wire_reachable(row) {
                transport_boundary.push(id.to_owned());
                continue;
            }
            let mut config = ArxivConfig::default();
            if let Some(base) = row["config"]["base_url"].as_str() {
                config.base_url = Arc::from(base)
            }
            if let Some(limit) = scalar(&row["config"]["max_results"]) {
                config.max_results = limit;
            }
            let script = Arc::new(Script {
                row: row.clone(),
                next: Mutex::new(0),
            });
            let http = Arc::new(AdapterHttp::default());
            http.set_transport(script.clone()).unwrap();
            let mut registry = ssx_registry::Registry::builtins().unwrap();
            ssx_infra::adapters::arxiv_runtime::register(
                &mut registry,
                http.clone(),
                Arc::new(AllowAllAdmission),
                config,
                parser.clone(),
            )
            .unwrap();
            assert!(registry.is_available("arxiv"));
            let adapter = registry.instantiate("arxiv").unwrap();
            let actual = adapter
                .search_with_context(
                    &ssx_domain::SearchRequest {
                        query: row["query"].as_str().unwrap().to_owned(),
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
            if id == "arxiv.connect" {
                assert_eq!(
                    actual.error_message.as_deref(),
                    Some("arXiv HTTP transport network failure")
                );
                transport_boundary.push(id.to_owned());
            } else if matches!(
                id,
                "arxiv.grammar.declaration_2" | "arxiv.grammar.declaration_3"
            ) {
                assert!(actual.results.is_empty());
                assert_eq!(actual.status, ssx_domain::EngineStatus::Ok);
                assert!(actual.error_message.is_none());
                deltas.push(id.to_owned());
            } else {
                assert_eq!(normalize(&actual), row["output"], "{id}");
                selected.push(id.to_owned());
            }
            http.shutdown_and_wait().await;
            assert_eq!(parser.available_operations(), 1);
            assert_eq!(parser.available_slots(), 1);
            a06_case::completed(
                "ssx-infra/a06_arxiv_runtime/actual_http_helper_path_preserves_source_outcomes_and_requests",
                selection["sources"][source_index]["path"].as_str().unwrap(),
                id,
                "default",
                if id == "arxiv.connect" {
                    "transport_difference"
                } else if deltas.iter().any(|v| v == id) {
                    "approved_expat_difference"
                } else {
                    "exact_outcome_request"
                },
            );
        }
    }
    assert_eq!(
        eligible,
        selection["eligible_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        selected,
        selection["source_exact_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        transport_boundary,
        selection["transport_boundary_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        deltas,
        selection["approved_expat_difference_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>()
    );
    eprintln!(
        "selected={} IDs={selected:?}; transport boundary={transport_boundary:?}; approved Expat deltas={deltas:?}",
        selected.len()
    );
    parser.shutdown().await.unwrap();
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
            PathBuf::from(env!("CARGO_BIN_EXE_ssx-atom-parser")),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let http = Arc::new(AdapterHttp::default());
        let config = ArxivConfig {
            base_url: Arc::from(format!("http://{address}/atom")),
            timeout_ms: Scalar::Float(f64::INFINITY),
            transport: ssx_infra::http::Options {
                trust_env: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        ssx_infra::adapters::arxiv_runtime::register(
            &mut registry,
            http.clone(),
            Arc::new(AllowAllAdmission),
            config,
            parser.clone(),
        )
        .unwrap();
        let adapter = registry.instantiate("arxiv").unwrap();
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
        assert!(request.starts_with("GET /atom?search_query=all%3A&start=0&max_results=5&sortBy=relevance&sortOrder=descending HTTP/1.1\r\n"));
        let lower = request.to_ascii_lowercase();
        assert!(
            lower.contains("user-agent: slopsearx/0.1.0 (meta search engine; agent-native)\r\n")
        );
        assert!(lower.contains("accept: application/atom+xml\r\n"));
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
                    Some("arXiv native XML parser shutdown")
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
#[tokio::test]
async fn actual_loopback_duplicate_headers_preserve_source_join_fallback() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
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
        let body=b"<feed xmlns='http://www.w3.org/2005/Atom'><entry><title>caf\xe9</title></entry></feed>";
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=latin1\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket.write_all(header.as_bytes()).await.unwrap();
        socket.write_all(body).await.unwrap();
    });
    let parser = AtomPool::new(
        PathBuf::from(env!("CARGO_BIN_EXE_ssx-atom-parser")),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let http = Arc::new(AdapterHttp::default());
    let config = ArxivConfig {
        base_url: Arc::from(format!("http://{address}/atom")),
        transport: ssx_infra::http::Options {
            trust_env: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut registry = ssx_registry::Registry::builtins().unwrap();
    ssx_infra::adapters::arxiv_runtime::register(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        config,
        parser.clone(),
    )
    .unwrap();
    let response = registry
        .instantiate("arxiv")
        .unwrap()
        .search_with_context(
            &ssx_domain::SearchRequest::default(),
            SearchContext {
                deadline: Instant::now() + Duration::from_secs(30),
            },
        )
        .await;
    assert_eq!(response.status, ssx_domain::EngineStatus::Ok);
    assert_eq!(response.results.len(), 1);
    // HTTPX joins duplicate fields before email parameter parsing: the first
    // candidate is "latin1, text/plain", not latin1, so lookup falls back UTF8.
    // The same precedence is frozen in source-charset headers_0 selector data.
    assert_eq!(response.results[0].title, "caf�");
    server.await.unwrap();
    parser.shutdown().await.unwrap();
    http.shutdown_and_wait().await;
}

#[tokio::test]
async fn actual_redirect_raw_identity_boundary_source_controls() {
    use sha2::Digest;
    let raw = include_str!("../../../docs/rust-rewrite/a06/source-arxiv-redirect-boundaries.json");
    let corpus: Value = ssx_json::from_str(raw).unwrap();
    let parser = AtomPool::new(
        PathBuf::from(env!("CARGO_BIN_EXE_ssx-atom-parser")),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let mut selected = std::collections::BTreeSet::new();
    for row in corpus["records"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        assert!(selected.insert(id));
        let config = ArxivConfig {
            base_url: Arc::from(row["config"]["base_url"].as_str().unwrap()),
            ..Default::default()
        };
        let script = Arc::new(Script {
            row: row.clone(),
            next: Mutex::new(0),
        });
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(script.clone()).unwrap();
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        ssx_infra::adapters::arxiv_runtime::register(
            &mut registry,
            http,
            Arc::new(AllowAllAdmission),
            config,
            parser.clone(),
        )
        .unwrap();
        let adapter = registry.instantiate("arxiv").unwrap();
        let actual = adapter
            .search_with_context(
                &ssx_domain::SearchRequest {
                    query: row["query"].as_str().unwrap().to_owned(),
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
        assert_eq!(normalize(&actual), row["output"], "{id}");
    }
    assert_eq!(selected.len(), 15);
    let digest = sha2::Sha256::digest(raw.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        digest,
        "94a401b48e02c7d2d70e4861f0ee3529f31b3bec9670350f6a74ce5b9a5c21ee"
    );
    parser.shutdown().await.unwrap();
}
