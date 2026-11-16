//! Final1xx is an injected response seam; real loopback qualification uses600.
use base64::Engine as _;
use ssx_application::SearchContext;
use ssx_domain::{AdapterResponse, EngineStatus};
use ssx_infra::{
    adapters::{
        AllowAllAdmission,
        arxiv_expat::AtomPool,
        arxiv_runtime::{self, ArxivConfig},
        semantic_runtime::{self, SemanticConfig},
    },
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
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
struct Script {
    row: Value,
    count: Mutex<usize>,
}
impl Transport for Script {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let mut n = self.count.lock().unwrap();
            assert_eq!(*n, 0);
            *n += 1;
            assert_eq!(
                request.method.as_str(),
                self.row["requests"][0]["method"].as_str().unwrap()
            );
            assert_eq!(
                request.url.as_str(),
                self.row["requests"][0]["url"].as_str().unwrap()
            );
            assert_eq!(
                request.headers["user-agent"].as_bytes(),
                self.row["requests"][0]["headers"]["user-agent"]
                    .as_str()
                    .unwrap()
                    .as_bytes()
            );
            let reply = &self.row["responses"][0];
            Ok(Response {
                status: reply["status"].as_u64().unwrap() as u16,
                url: request.url,
                headers: Default::default(),
                body: base64::engine::general_purpose::STANDARD
                    .decode(reply["body_b64"].as_str().unwrap())
                    .unwrap(),
            })
        })
    }
}
#[tokio::test]
async fn whole_original_status_boundaries_preserve_exact_classes_outcomes_and_requests() {
    let corpus: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/source-http-status-boundaries.json"
    ))
    .unwrap();
    let pool = AtomPool::new(
        PathBuf::from(env!("CARGO_BIN_EXE_ssx-atom-parser")),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for row in corpus["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["engine"].as_str() != Some("openalex"))
    {
        let id = row["id"].as_str().unwrap();
        assert!(ids.insert(id));
        let engine = row["engine"].as_str().unwrap();
        let script = Arc::new(Script {
            row: row.clone(),
            count: Mutex::new(0),
        });
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(script.clone()).unwrap();
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        if engine == "arxiv" {
            arxiv_runtime::register(
                &mut registry,
                http.clone(),
                Arc::new(AllowAllAdmission),
                ArxivConfig::default(),
                pool.clone(),
            )
            .unwrap();
        } else {
            semantic_runtime::register(
                &mut registry,
                http.clone(),
                Arc::new(AllowAllAdmission),
                SemanticConfig::default(),
                pool.clone(),
            )
            .unwrap();
        }
        let result = registry
            .instantiate(engine)
            .unwrap()
            .search_with_context(
                &ssx_domain::SearchRequest {
                    query: "test".into(),
                    ..Default::default()
                },
                SearchContext {
                    deadline: Instant::now() + Duration::from_secs(30),
                },
            )
            .await;
        assert_eq!(normalized(&result), row["output"], "{id}");
        assert_eq!(*script.count.lock().unwrap(), 1);
        assert_eq!(pool.available_operations(), 1);
        assert_eq!(pool.available_slots(), 1);
        http.shutdown_and_wait().await;
    }
    assert_eq!(ids.len(), 24);
    for engine in ["arxiv", "semanticscholar"] {
        for status in [100, 199, 200, 299, 300, 399, 400, 499, 500, 599, 600, 999] {
            assert!(ids.contains(format!("{engine}.http_status_boundary.{status}").as_str()));
        }
    }
    pool.shutdown().await.unwrap();
}
#[tokio::test]
async fn actual_loopback600_returns_source_class_and_restores_lifecycle_without_helper() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    for engine in ["arxiv", "semanticscholar"] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut raw = Vec::new();
            let mut bytes = [0; 1024];
            loop {
                let n = socket.read(&mut bytes).await.unwrap();
                assert_ne!(n, 0);
                raw.extend_from_slice(&bytes[..n]);
                if raw.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
                assert!(raw.len() < 8192);
            }
            socket
                .write_all(b"HTTP/1.1 600 \r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
        });
        let pool = AtomPool::new(
            PathBuf::from("/nonexistent-status600-helper"),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let http = Arc::new(AdapterHttp::default());
        let mut registry = ssx_registry::Registry::builtins().unwrap();
        let transport = ssx_infra::http::Options {
            trust_env: false,
            ..Default::default()
        };
        let base = Arc::from(format!("http://{addr}/status"));
        if engine == "arxiv" {
            arxiv_runtime::register(
                &mut registry,
                http.clone(),
                Arc::new(AllowAllAdmission),
                ArxivConfig {
                    base_url: base,
                    transport,
                    ..Default::default()
                },
                pool.clone(),
            )
            .unwrap();
        } else {
            semantic_runtime::register(
                &mut registry,
                http.clone(),
                Arc::new(AllowAllAdmission),
                SemanticConfig {
                    base_url: base,
                    transport,
                    ..Default::default()
                },
                pool.clone(),
            )
            .unwrap();
        }
        let result = registry
            .instantiate(engine)
            .unwrap()
            .search_with_context(
                &ssx_domain::SearchRequest {
                    query: "test".into(),
                    ..Default::default()
                },
                SearchContext {
                    deadline: Instant::now() + Duration::from_secs(30),
                },
            )
            .await;
        assert_eq!(result.status, EngineStatus::Error);
        assert!(result.results.is_empty());
        assert!(
            result
                .error_message
                .as_deref()
                .unwrap()
                .starts_with("Invalid status code '600 ' for url '")
        );
        assert_eq!(pool.available_operations(), 1);
        assert_eq!(pool.available_slots(), 1);
        server.await.unwrap();
        pool.shutdown().await.unwrap();
        http.shutdown_and_wait().await;
    }
}
