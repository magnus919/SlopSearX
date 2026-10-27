use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::adapters::{
    AllowAllAdmission, DockerHubConfig, PackageConfig, PyPIConfig, RubyGemsConfig,
    register_packages,
};
use ssx_infra::http::AdapterHttp;
use ssx_registry::Registry;
use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
const META: &str = include_str!("../../../docs/rust-rewrite/a02/corpus-meta.json");
const REVIEW: &str = include_str!("../../../docs/rust-rewrite/a02/corpus-review.json");
const EXTRA: &str = include_str!("../../../docs/rust-rewrite/a02/corpus-extra.json");
const CORPUS: &str = include_str!("../../../docs/rust-rewrite/a02/corpus.json");

#[tokio::test]
async fn source_bound_package_corpus_replays_through_registry_and_real_http() {
    let corpus: ssx_json::Value = ssx_json::from_str(CORPUS).unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 122);
    let extra: ssx_json::Value = ssx_json::from_str(EXTRA).unwrap();
    let extra = extra["cases"].as_array().unwrap();
    assert_eq!(extra.len(), 13);
    let review: ssx_json::Value = ssx_json::from_str(REVIEW).unwrap();
    let review = review["cases"].as_array().unwrap();
    assert_eq!(review.len(), 20);
    let meta: ssx_json::Value = ssx_json::from_str(META).unwrap();
    let meta = meta["cases"].as_array().unwrap();
    assert_eq!(meta.len(), 3);
    let mut ids = std::collections::BTreeSet::new();
    for case in cases
        .iter()
        .chain(extra.iter())
        .chain(review.iter())
        .chain(meta.iter())
    {
        assert!(ids.insert(case["id"].as_str().unwrap()));
        replay(case).await;
    }
    assert_eq!(ids.len(), 158);
}
async fn replay(case: &ssx_json::Value) {
    let id = case["id"].as_str().unwrap();
    let engine = case["engine"].as_str().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("http://{address}");
    let base = format!(
        "{origin}{}",
        case["base_suffix"].as_str().unwrap_or_default()
    );
    let timeout_ms = if case["exchanges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["timeout"].as_bool() == Some(true))
    {
        100
    } else {
        2000
    };
    let captured = Arc::new(Mutex::new(Vec::new()));
    let capture = captured.clone();
    let exchanges = case["exchanges"].as_array().unwrap().clone();
    let server = tokio::spawn(async move {
        for exchange in exchanges {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut data = Vec::new();
            loop {
                let mut buf = [0; 4096];
                let count = stream.read(&mut buf).await.unwrap();
                if count == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..count]);
                if data.windows(4).any(|v| v == b"\r\n\r\n") {
                    break;
                }
            }
            capture
                .lock()
                .unwrap()
                .push(String::from_utf8(data).unwrap());
            if exchange["timeout"].as_bool() == Some(true) {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                continue;
            }
            let status = exchange["status"].as_u64().unwrap();
            let body = if let Some(hex) = exchange["body_hex"].as_str() {
                (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                    .collect::<Vec<_>>()
            } else {
                exchange["body"].as_str().unwrap().as_bytes().to_vec()
            };
            let response_headers = exchange["headers"]
                .as_object()
                .map(|headers| {
                    headers
                        .iter()
                        .map(|(name, value)| format!("{name}: {}\r\n", value.as_str().unwrap()))
                        .collect::<String>()
                })
                .unwrap_or_default();
            let head = format!(
                "HTTP/1.1 {status} Response\r\n{response_headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes()).await;
            let _ = stream.write_all(&body).await;
        }
    });
    let max = case["max_results"].as_u64().unwrap() as usize;
    let mut registry = Registry::builtins().unwrap();
    let http = Arc::new(AdapterHttp::default());
    register_packages(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        PackageConfig {
            pypi: PyPIConfig {
                base_url: base.clone(),
                max_results: max,
                timeout_ms,
            },
            rubygems: RubyGemsConfig {
                base_url: base.clone(),
                max_results: max,
                timeout_ms,
            },
            dockerhub: DockerHubConfig {
                base_url: base.clone(),
                max_results: max,
                timeout_ms,
            },
        },
    )
    .unwrap();
    let response = registry
        .instantiate(engine)
        .unwrap()
        .search(&SearchRequest {
            query: case["query"].as_str().unwrap().into(),
            page: 7,
            language: "fr".into(),
            safesearch: 2,
            time_range: Some("day".into()),
            max_results: Some(1),
            ..SearchRequest::default()
        })
        .await;
    let expected = &case["expected"];
    assert_eq!(
        ssx_json::to_value(response.status)
            .unwrap()
            .as_str()
            .unwrap(),
        expected["status"].as_str().unwrap(),
        "{id}: {}",
        response.error_message.as_deref().unwrap_or_default()
    );
    assert_eq!(
        response.results.len(),
        expected["results"].as_array().unwrap().len(),
        "{id}"
    );
    for (actual, expected) in response
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        let actual = ssx_json::json!({"url":actual.url,"title":actual.title,"content":actual.content,"engine":actual.engine,"position":actual.position,"score":actual.score.get(),"payload":actual.payload});
        assert_eq!(actual, *expected, "{id}");
    }
    let requests = captured.lock().unwrap().clone();
    assert_eq!(
        requests.len(),
        case["requests"].as_array().unwrap().len(),
        "{id}: request count"
    );
    for (raw, expected) in requests.iter().zip(case["requests"].as_array().unwrap()) {
        let expected_url = expected["url"]
            .as_str()
            .unwrap()
            .replace("http://127.0.0.1:12345", &origin);
        let url = url::Url::parse(&expected_url).unwrap();
        let path = format!(
            "{}{}",
            url.path(),
            url.query().map(|v| format!("?{v}")).unwrap_or_default()
        );
        assert_eq!(
            raw.lines().next().unwrap(),
            format!("GET {path} HTTP/1.1"),
            "{id}: outbound URL"
        );
        let lower = raw.to_ascii_lowercase();
        let actual_cookie = raw.lines().find_map(|line| {
            line.split_once(':')
                .filter(|(name, _)| name.eq_ignore_ascii_case("cookie"))
                .map(|(_, value)| value.trim())
        });
        assert_eq!(
            actual_cookie,
            expected["headers"]["cookie"].as_str(),
            "{id}: cookie session continuity"
        );

        assert!(!lower.contains("authorization:"), "{id}");
        if engine != "rubygems" {
            assert!(
                lower.contains("user-agent: slopsearx/0.1.0 (meta search engine; agent-native)"),
                "{id}"
            );
        }
        if engine == "dockerhub" {
            assert!(lower.contains("accept: application/json"), "{id}");
        }
    }
    server.abort();
    http.shutdown();
}
struct Denied(AdmissionDecision);
impl SearchAdmission for Denied {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move { self.0 })
    }
}
#[tokio::test]
async fn package_admission_denials_and_unavailability_dispatch_zero_requests() {
    for (decision, status) in [
        (AdmissionDecision::Denied, EngineStatus::RateLimited),
        (AdmissionDecision::Unavailable, EngineStatus::Error),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let mut registry = Registry::builtins().unwrap();
        let http = Arc::new(AdapterHttp::default());
        register_packages(
            &mut registry,
            http.clone(),
            Arc::new(Denied(decision)),
            PackageConfig {
                pypi: PyPIConfig {
                    base_url: base.clone(),
                    ..PyPIConfig::default()
                },
                rubygems: RubyGemsConfig {
                    base_url: base.clone(),
                    ..RubyGemsConfig::default()
                },
                dockerhub: DockerHubConfig {
                    base_url: base,
                    ..DockerHubConfig::default()
                },
            },
        )
        .unwrap();
        for engine in ["pypi", "rubygems", "dockerhub"] {
            let response = registry
                .instantiate(engine)
                .unwrap()
                .search(&SearchRequest {
                    query: "python".into(),
                    ..SearchRequest::default()
                })
                .await;
            assert_eq!(response.status, status);
            assert!(response.results.is_empty());
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), listener.accept())
                .await
                .is_err()
        );
        http.shutdown();
    }
}

#[tokio::test]
async fn cancelling_package_search_drops_work_and_prevents_followup_dispatch() {
    for engine in ["pypi", "rubygems", "dockerhub"] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let mut registry = Registry::builtins().unwrap();
        let http = Arc::new(AdapterHttp::default());
        register_packages(
            &mut registry,
            http.clone(),
            Arc::new(AllowAllAdmission),
            PackageConfig {
                pypi: PyPIConfig {
                    base_url: base.clone(),
                    ..PyPIConfig::default()
                },
                rubygems: RubyGemsConfig {
                    base_url: base.clone(),
                    ..RubyGemsConfig::default()
                },
                dockerhub: DockerHubConfig {
                    base_url: base,
                    ..DockerHubConfig::default()
                },
            },
        )
        .unwrap();
        let adapter = registry.instantiate(engine).unwrap();
        let task = tokio::spawn(async move {
            adapter
                .search(&SearchRequest {
                    query: "python".into(),
                    ..SearchRequest::default()
                })
                .await
        });
        let (mut stream, _) =
            tokio::time::timeout(std::time::Duration::from_secs(2), listener.accept())
                .await
                .unwrap()
                .unwrap();
        let mut bytes = [0; 8192];
        assert!(stream.read(&mut bytes).await.unwrap() > 0);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        // A non-200 response would trigger PyPI's second request if cancellation did
        // not drop the search future before continuing the fallback state machine.
        let _ = stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await;
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(30), listener.accept())
                .await
                .is_err()
        );
        http.shutdown();
    }
}

#[test]
fn package_factories_complete_catalog_without_changing_metadata() {
    // This port's parity target is the frozen migration catalog, not native additions.
    let mut registry = Registry::migration_baseline().unwrap();
    let http = Arc::new(AdapterHttp::default());
    register_packages(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        PackageConfig::default(),
    )
    .unwrap();
    ssx_infra::adapters::register_pilots(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        ssx_infra::adapters::PilotConfig::default(),
    )
    .unwrap();
    assert_eq!(registry.len(), 51);
    assert_eq!(
        registry
            .descriptors()
            .filter(|d| registry.is_available(&d.name))
            .count(),
        5
    );
    for engine in ["pypi", "rubygems", "dockerhub"] {
        assert!(registry.instantiate(engine).is_some());
        assert!(registry.is_available(engine));
        assert!(
            registry
                .descriptor(engine)
                .unwrap()
                .categories
                .iter()
                .any(|c| c == "packages")
        );
    }
    assert_eq!(PyPIConfig::default().max_results, 10);
    assert_eq!(RubyGemsConfig::default().timeout_ms, 5000);
    assert!(DockerHubConfig::default().base_url.is_empty());
    http.shutdown();
}

#[tokio::test]
async fn explicit_empty_pypi_and_rubygems_urls_fail_before_transport_dispatch() {
    let observations: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a02/empty-base-source.json"
    ))
    .unwrap();
    let mut registry = Registry::builtins().unwrap();
    let http = Arc::new(AdapterHttp::default());
    // Closing the caller pool ensures an erroneous fallback to a public default
    // cannot initiate network traffic; diagnostics prove URL validation wins.
    http.shutdown();
    register_packages(
        &mut registry,
        http,
        Arc::new(AllowAllAdmission),
        PackageConfig {
            pypi: PyPIConfig {
                base_url: String::new(),
                ..PyPIConfig::default()
            },
            rubygems: RubyGemsConfig {
                base_url: String::new(),
                ..RubyGemsConfig::default()
            },
            ..PackageConfig::default()
        },
    )
    .unwrap();
    for (engine, message) in [
        ("pypi", "invalid request URL"),
        ("rubygems", "invalid RubyGems URL"),
    ] {
        assert_eq!(observations[engine]["status"], "error");
        let result = registry
            .instantiate(engine)
            .unwrap()
            .search(&SearchRequest {
                query: "python".into(),
                ..SearchRequest::default()
            })
            .await;
        assert_eq!(result.status, EngineStatus::Error);
        assert!(result.results.is_empty());
        assert_eq!(result.error_message.as_deref(), Some(message));
    }
    assert_eq!(PyPIConfig::default().base_url, "https://pypi.org");
    assert_eq!(
        RubyGemsConfig::default().base_url,
        "https://rubygems.org/api/v1/search.json"
    );
}

#[tokio::test]
async fn resetting_operation_deadline_preserves_request_and_cancellation_guards() {
    use ssx_infra::http::{Bounds, Cancellation, Error, Options, Request};
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0; 4096];
        assert!(stream.read(&mut buf).await.unwrap() > 0);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        listener
    });
    let http = AdapterHttp::default();
    let mut session = http
        .session(
            Options {
                bounds: Bounds {
                    requests: 1,
                    ..Bounds::default()
                },
                ..Options::default()
            },
            tokio::time::Instant::now() + std::time::Duration::from_secs(2),
            Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        session
            .execute(Request::get(&base).unwrap())
            .await
            .unwrap()
            .status,
        200
    );
    session.set_deadline(tokio::time::Instant::now() + std::time::Duration::from_secs(2));
    assert!(matches!(
        session.execute(Request::get(&base).unwrap()).await,
        Err(Error::RequestLimit)
    ));
    let cancel = Cancellation::default();
    let mut cancelled = http
        .session(
            Options::default(),
            tokio::time::Instant::now(),
            cancel.clone(),
        )
        .unwrap();
    cancel.cancel();
    cancelled.set_deadline(tokio::time::Instant::now() + std::time::Duration::from_secs(2));
    assert!(matches!(
        cancelled.execute(Request::get(&base).unwrap()).await,
        Err(Error::Cancelled)
    ));
    let mut stopped = http
        .session(
            Options::default(),
            tokio::time::Instant::now(),
            Cancellation::default(),
        )
        .unwrap();
    http.shutdown();
    stopped.set_deadline(tokio::time::Instant::now() + std::time::Duration::from_secs(2));
    assert!(matches!(
        stopped.execute(Request::get(&base).unwrap()).await,
        Err(Error::Closed)
    ));
    let listener = server.await.unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), listener.accept())
            .await
            .is_err()
    );
}
