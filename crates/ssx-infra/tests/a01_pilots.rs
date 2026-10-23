#[path = "support/owned_loopback_fixture.rs"]
mod owned_loopback_fixture;
use owned_loopback_fixture::OwnedLoopback;
use ssx_application::{
    PortFuture,
    rate::{RateLimiter, RateStore},
};
use ssx_domain::{
    EngineStatus, SearchRequest, ValkeyQuotaTime,
    rate::{EngineKey, QuotaReceipt, RateFailure, SharedCooldown},
};
use ssx_infra::adapters::{
    AllowAllAdmission, CratesConfig, F10Admission, NpmConfig, PilotConfig, f10_monotonic_seconds,
    register_pilots,
};
use ssx_registry::Registry;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const NPM_FIXTURE: &str = include_str!("../../../tests/fixtures/upstream_contracts/npm.json");
const CRATES_FIXTURE: &str = include_str!("../../../tests/fixtures/upstream_contracts/crates.json");
const A01_CASES: &str = include_str!("../../../docs/rust-rewrite/a01/cases.json");

#[tokio::test]
async fn registry_factories_execute_search_via_f08_loopback_transport() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let npm = fixture_payload(NPM_FIXTURE);
    let crates = fixture_payload(CRATES_FIXTURE);
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0; 8192];
            let count = stream.read(&mut bytes).await.unwrap();
            let head = String::from_utf8_lossy(&bytes[..count]);
            let line = head.lines().next().unwrap_or_default();
            captured.lock().unwrap().push(head.to_string());
            let body = if line.starts_with("GET /npm?") {
                npm.as_bytes()
            } else {
                crates.as_bytes()
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.write_all(body).await.unwrap();
            stream.shutdown().await.unwrap();
        }
    });

    let mut registry = Registry::builtins().unwrap();
    register_pilots(
        &mut registry,
        Arc::new(ssx_infra::http::AdapterHttp::default()),
        Arc::new(AllowAllAdmission),
        PilotConfig {
            npm: NpmConfig {
                base_url: format!("http://{address}/npm?keep=yes"),
                timeout_ms: 2_000,
                max_results: 2,
            },
            crates: CratesConfig {
                base_url: format!("http://{address}/crates?keep=yes"),
                timeout_ms: 2_000,
                max_results: 2,
            },
        },
    )
    .unwrap();

    let npm_engine = registry.instantiate("npm").unwrap();
    let npm_response = npm_engine
        .search(&SearchRequest {
            query: "typescript & types".into(),
            page: 7,
            categories: Some(vec!["packages".into()]),
            media_type: Some("image".into()),
            language: "fr".into(),
            safesearch: 2,
            time_range: Some("day".into()),
            max_results: Some(1),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(npm_response.status, EngineStatus::Ok);
    assert_eq!(
        npm_response.results.len(),
        2,
        "configured max_results controls request and local slice"
    );
    assert_eq!(npm_response.results[0].title, "typescript@7.0.2");
    assert_eq!(npm_response.results[0].engine, "npm");
    assert!(npm_response.results[0].media.is_none());
    let npm_payload = npm_response.results[0].payload.as_ref().unwrap();
    assert_eq!(npm_payload["data"]["name"], "typescript");
    assert_eq!(npm_payload["data"]["version"], "7.0.2");
    assert_eq!(
        npm_payload["data"]["summary"],
        "TypeScript is a language for application scale JavaScript development"
    );
    assert!(
        npm_payload["data"]
            .as_object()
            .unwrap()
            .get("repository_url")
            .is_none(),
        "source payload drops absent fields"
    );
    assert_eq!(npm_payload["provenance"]["adapter_fields"][0], "name");
    assert_eq!(npm_payload["provenance"]["adapter_fields"][2], "summary");

    let crates_engine = registry.instantiate("crates").unwrap();
    let crates_response = crates_engine
        .search(&SearchRequest {
            query: "serde & rust".into(),
            page: 9,
            categories: Some(vec!["packages".into()]),
            media_type: Some("video".into()),
            language: "de".into(),
            safesearch: 1,
            time_range: Some("month".into()),
            max_results: Some(1),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(crates_response.status, EngineStatus::Ok);
    assert_eq!(crates_response.results.len(), 2);
    assert_eq!(crates_response.results[0].title, "serde v1.0.229");
    assert_eq!(crates_response.results[0].score.get(), 294_274_544.0);
    assert_eq!(
        crates_response.results[0].content,
        "A generic serialization/deserialization framework — Total downloads: 1,360,042,789"
    );
    assert!(
        crates_response.results[0].payload.is_none(),
        "crates source has no package payload"
    );
    assert!(crates_response.results[0].media.is_none());
    server.await.unwrap();

    let lines = requests.lock().unwrap().clone();
    let queries = lines
        .iter()
        .map(|line| {
            let target = line
                .split_whitespace()
                .nth(1)
                .expect("captured request target");
            url::Url::parse(&format!("http://fixture.invalid{target}"))
                .unwrap()
                .query_pairs()
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(
        queries.contains(&vec![
            ("text".into(), "typescript & types".into()),
            ("size".into(), "2".into())
        ]),
        "{queries:?}"
    );
    assert!(
        queries.contains(&vec![
            ("q".into(), "serde & rust".into()),
            ("per_page".into(), "2".into())
        ]),
        "{queries:?}"
    );
    assert!(
        queries
            .iter()
            .flatten()
            .all(|(key, _)| key != "page" && key != "keep"),
        "legacy page params are ignored and HTTPX replaces the configured query: {queries:?}"
    );
    assert!(
        requests
            .lock()
            .unwrap()
            .iter()
            .all(|line| !line.contains("Authorization:")),
        "these public APIs use no auth credential"
    );
}

#[tokio::test]
async fn missing_result_collections_are_empty_but_null_and_wrong_outer_types_fail() {
    let cases = [
        ("npm", b"{}".as_slice(), EngineStatus::Ok, 0),
        ("crates", b"{}".as_slice(), EngineStatus::Ok, 0),
        ("npm", br#"{"objects":[]}"#.as_slice(), EngineStatus::Ok, 0),
        (
            "crates",
            br#"{"crates":[]}"#.as_slice(),
            EngineStatus::Ok,
            0,
        ),
        (
            "npm",
            b"{\"objects\":null}".as_slice(),
            EngineStatus::Error,
            0,
        ),
        (
            "crates",
            b"{\"crates\":null}".as_slice(),
            EngineStatus::Error,
            0,
        ),
        ("npm", b"[]".as_slice(), EngineStatus::Error, 0),
        ("crates", b"null".as_slice(), EngineStatus::Error, 0),
    ];
    for (engine, body, expected_status, count) in cases {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let body = body.to_vec();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            let _ = stream.read(&mut request).await.unwrap();
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
        });
        let mut registry = Registry::builtins().unwrap();
        let mut config = PilotConfig::default();
        config.npm.base_url = format!("http://{address}/npm");
        config.crates.base_url = format!("http://{address}/crates");
        register_pilots(
            &mut registry,
            Arc::new(ssx_infra::http::AdapterHttp::default()),
            Arc::new(AllowAllAdmission),
            config,
        )
        .unwrap();
        let adapter = registry.instantiate(engine).unwrap();
        let result = adapter.search(&SearchRequest::default()).await;
        assert_eq!(result.status, expected_status, "{engine}");
        assert_eq!(result.results.len(), count, "{engine}");
        server.await.unwrap();
    }
}

#[tokio::test]
async fn both_pilots_classify_http_malformed_and_timeout_cases() {
    let cases: [(&str, u16, &[u8], EngineStatus); 10] = [
        ("npm", 429, b"{}", EngineStatus::RateLimited),
        ("crates", 429, b"{}", EngineStatus::RateLimited),
        ("npm", 503, b"{}", EngineStatus::Error),
        ("crates", 503, b"{}", EngineStatus::Error),
        ("npm", 404, b"{}", EngineStatus::Error),
        ("crates", 404, b"{}", EngineStatus::Error),
        ("npm", 200, b"not-json", EngineStatus::Error),
        ("crates", 200, b"not-json", EngineStatus::Error),
        (
            "npm",
            200,
            br#"{"objects":[{"package":null}]}"#,
            EngineStatus::Error,
        ),
        ("crates", 200, br#"{"crates":[null]}"#, EngineStatus::Error),
    ];
    for (engine, status, body, expected) in cases {
        let response = exercise_http_case(engine, status, body, 2_000, false).await;
        assert_eq!(response.status, expected, "{engine}, HTTP {status}");
        assert!(response.results.is_empty());
    }
    for engine in ["npm", "crates"] {
        // Real 20ms timeout classification does not promise dispatch. Zero
        // budget deterministically covers a fixture still waiting in accept.
        for timeout_ms in [20, 0] {
            let response = exercise_http_case(engine, 200, b"{}", timeout_ms, true).await;
            assert_eq!(response.status, EngineStatus::Timeout, "{engine}");
            assert!(response.results.is_empty());
        }
    }
}

#[tokio::test]
async fn both_pilots_reject_invalid_base_url_configuration() {
    for engine in ["npm", "crates"] {
        let mut config = PilotConfig::default();
        config.npm.base_url = "not a URL".into();
        config.crates.base_url = "not a URL".into();
        let mut registry = Registry::builtins().unwrap();
        register_pilots(
            &mut registry,
            Arc::new(ssx_infra::http::AdapterHttp::default()),
            Arc::new(AllowAllAdmission),
            config,
        )
        .unwrap();
        let response = registry
            .instantiate(engine)
            .unwrap()
            .search(&SearchRequest::default())
            .await;
        assert_eq!(response.status, EngineStatus::Error, "{engine}");
        assert!(response.results.is_empty());
    }
}

#[tokio::test]
async fn npm_non_mapping_package_links_are_ignored() {
    let response = exercise_http_case(
        "npm",
        200,
        br#"{"objects":[{"package":{"name":"serde","links":["https://example.test"]}}]}"#,
        2_000,
        false,
    )
    .await;
    assert_eq!(response.status, EngineStatus::Ok);
    assert!(
        response.results[0].payload.as_ref().unwrap()["data"]
            .get("repository_url")
            .is_none()
    );
}

#[tokio::test]
async fn crates_download_rendering_matches_offline_python_observations() {
    let fixture: ssx_json::Value = ssx_json::from_str(A01_CASES).unwrap();
    for observation in fixture["source_observations"].as_array().unwrap() {
        let case = observation["case"].as_str().unwrap();
        if !case.starts_with("crates_") {
            continue;
        }
        let body = if let Some(raw) = observation["input"]["raw_body"].as_str() {
            raw.as_bytes().to_vec()
        } else {
            let source_row = observation["input"]["crates"][0]
                .as_object()
                .or_else(|| observation["input"].as_object());
            let mut row = ssx_json::Value::Object(ssx_json::Map::new());
            row["name"] = ssx_json::Value::String("probe".into());
            row["max_version"] = ssx_json::Value::String("1".into());
            if let Some(source_row) = source_row {
                for field in [
                    "name",
                    "max_version",
                    "downloads",
                    "recent_downloads",
                    "description",
                ] {
                    if let Some(value) = source_row.get(field) {
                        row[field] = value.clone();
                    }
                }
            }
            let mut response = ssx_json::Map::new();
            response.insert("crates".into(), ssx_json::Value::Array(vec![row]));
            ssx_json::to_vec(&ssx_json::Value::Object(response)).unwrap()
        };
        let response = exercise_http_case("crates", 200, &body, 2_000, false).await;
        let expected_status = match observation["output"]["status"].as_str().unwrap() {
            "ok" => EngineStatus::Ok,
            "error" => EngineStatus::Error,
            other => panic!("unsupported source status {other}"),
        };
        assert_eq!(response.status, expected_status, "{case}");
        if expected_status == EngineStatus::Ok {
            assert_eq!(
                response.results[0].content,
                observation["output"]["content"].as_str().unwrap(),
                "{case}"
            );
            assert_eq!(
                response.results[0].score.get(),
                observation["output"]["score"].as_f64().unwrap(),
                "{case}"
            );
        } else {
            assert!(response.results.is_empty(), "{case}");
        }
    }
}

#[tokio::test]
async fn npm_nested_python_container_stringification_matches_captured_python_observation() {
    let fixture: ssx_json::Value = ssx_json::from_str(A01_CASES).unwrap();
    let observation = fixture["source_observations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["case"].as_str() == Some("npm_nested_python_container_stringification"))
        .unwrap();
    let mut object = ssx_json::Map::new();
    object.insert("package".into(), observation["input"]["package"].clone());
    let mut response = ssx_json::Map::new();
    response.insert(
        "objects".into(),
        ssx_json::Value::Array(vec![ssx_json::Value::Object(object)]),
    );
    let body = ssx_json::to_vec(&ssx_json::Value::Object(response)).unwrap();
    let response = exercise_http_case("npm", 200, &body, 2_000, false).await;
    assert_eq!(response.status, EngineStatus::Ok);
    assert_eq!(
        response.results[0].title,
        observation["output"]["title"].as_str().unwrap()
    );
    assert_eq!(
        response.results[0].content,
        observation["output"]["content"].as_str().unwrap()
    );
}

#[tokio::test]
async fn npm_truthiness_matches_captured_python_observations() {
    let fixture: serde_json::Value = serde_json::from_str(A01_CASES).unwrap();
    for observation in fixture["source_observations"].as_array().unwrap() {
        let case = observation["case"].as_str().unwrap();
        if !(case.starts_with("npm_publisher_") || case.starts_with("npm_empty_")) {
            continue;
        }
        let input = &observation["input"];
        let mut package = serde_json::Map::new();
        for field in ["description", "publisher"] {
            if let Some(value) = input.get(field) {
                package.insert(field.into(), value.clone());
            }
        }
        let mut object = serde_json::json!({"package":package});
        if let Some(value) = input.get("score") {
            object["score"] = value.clone();
        }
        let body = serde_json::to_vec(&serde_json::json!({"objects":[object]})).unwrap();
        let response = exercise_http_case("npm", 200, &body, 2_000, false).await;
        assert_eq!(response.status, EngineStatus::Ok, "{case}");
        if let Some(content) = observation["output"]["content"].as_str() {
            assert_eq!(response.results[0].content, content, "{case}");
        }
        if let Some(score) = observation["output"]["score"].as_f64() {
            assert_eq!(response.results[0].score.get(), score, "{case}");
        }
    }
}

#[tokio::test]
async fn npm_popularity_strings_match_captured_python_float_observations() {
    let fixture: serde_json::Value = serde_json::from_str(A01_CASES).unwrap();
    for observation in fixture["source_observations"].as_array().unwrap() {
        let case = observation["case"].as_str().unwrap();
        if !case.starts_with("npm_popularity_") {
            continue;
        }
        let body = serde_json::to_vec(&serde_json::json!({
            "objects":[{"package":{},"score":observation["input"]["score"]}]
        }))
        .unwrap();
        let response = exercise_http_case("npm", 200, &body, 2_000, false).await;
        match observation["output"]["status"].as_str().unwrap() {
            "ok" => {
                assert_eq!(response.status, EngineStatus::Ok, "{case}");
                assert_eq!(
                    response.results[0].score.get(),
                    observation["output"]["score"].as_f64().unwrap(),
                    "{case}"
                );
            }
            "error" => {
                assert_eq!(response.status, EngineStatus::Error, "{case}");
                assert!(response.results.is_empty(), "{case}");
            }
            other => panic!("unsupported source status {other}"),
        }
    }
}

#[tokio::test]
async fn npm_source_edge_cases_match_captured_python_parser_observations() {
    let cases: &[(&[u8], &str, f64, bool)] = &[
        (
            br#"{"objects":[{"package":{"name":null,"version":null}}]}"#,
            "None@None",
            0.0,
            true,
        ),
        (
            br#"{"objects":[{"package":{},"score":{"detail":{"popularity":"0.25"}}}]}"#,
            "@",
            250.0,
            true,
        ),
        (
            br#"{"objects":[{"package":{},"score":{"detail":{"popularity":true}}}]}"#,
            "@",
            1000.0,
            true,
        ),
        (br#"{"objects":[{"package":{}}]}"#, "@", 0.0, true),
        (
            br#"{"objects":[{"package":{},"score":null}]}"#,
            "",
            0.0,
            false,
        ),
    ];
    for (body, title, score, accepted) in cases {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let body = body.to_vec();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            let _ = stream.read(&mut request).await.unwrap();
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
        });
        let mut config = PilotConfig::default();
        config.npm.base_url = format!("http://{address}/npm");
        let mut registry = Registry::builtins().unwrap();
        register_pilots(
            &mut registry,
            Arc::new(ssx_infra::http::AdapterHttp::default()),
            Arc::new(AllowAllAdmission),
            config,
        )
        .unwrap();
        let result = registry
            .instantiate("npm")
            .unwrap()
            .search(&SearchRequest::default())
            .await;
        assert_eq!(result.status == EngineStatus::Ok, *accepted);
        if *accepted {
            assert_eq!(result.results[0].title, *title);
            assert_eq!(result.results[0].score.get(), *score);
        }
        server.await.unwrap();
    }
}

#[tokio::test]
async fn f10_rate_denial_from_registry_adapter_dispatches_no_provider_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut config = PilotConfig::default();
    config.npm.base_url = format!("http://{address}/npm");
    let limiter = Arc::new(tokio::sync::Mutex::new(RateLimiter::new(
        Arc::new(DenyStore),
        true,
    )));
    let admission = Arc::new(F10Admission::new(limiter));
    let mut registry = Registry::builtins().unwrap();
    register_pilots(
        &mut registry,
        Arc::new(ssx_infra::http::AdapterHttp::default()),
        admission,
        config,
    )
    .unwrap();
    let adapter = registry.instantiate("npm").unwrap();
    let response = adapter.search(&SearchRequest::default()).await;
    assert_eq!(response.status, EngineStatus::RateLimited);
    assert!(response.results.is_empty());
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), listener.accept())
            .await
            .is_err(),
        "denied adapter must not dispatch HTTP"
    );
}

#[tokio::test]
async fn f10_store_failure_preserves_fail_open_and_fail_closed_classification() {
    use ssx_application::{AdmissionDecision, SearchAdmission};
    let fail_open = Arc::new(tokio::sync::Mutex::new(RateLimiter::new(
        Arc::new(UnavailableStore),
        false,
    )));
    let fail_closed = Arc::new(tokio::sync::Mutex::new(RateLimiter::new(
        Arc::new(UnavailableStore),
        true,
    )));
    assert_eq!(
        F10Admission::new(fail_open).admit("npm").await,
        AdmissionDecision::Allowed
    );
    assert_eq!(
        F10Admission::new(fail_closed).admit("npm").await,
        AdmissionDecision::Unavailable
    );
}

#[tokio::test]
async fn f10_admission_handles_share_the_limiter_clock_and_cooldown_state() {
    use ssx_application::{AdmissionDecision, SearchAdmission};
    let cooldown_reads = Arc::new(AtomicUsize::new(0));
    let limiter = Arc::new(tokio::sync::Mutex::new(RateLimiter::new(
        Arc::new(ActiveCooldownStore {
            reads: cooldown_reads.clone(),
        }),
        true,
    )));

    let first_handle = F10Admission::new(limiter.clone());
    assert_eq!(first_handle.admit("npm").await, AdmissionDecision::Denied);
    drop(first_handle);

    let second_handle = F10Admission::new(limiter);
    assert_eq!(second_handle.admit("npm").await, AdmissionDecision::Denied);
    assert_eq!(cooldown_reads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn composition_cooldown_uses_the_public_shared_f10_clock_and_blocks_dispatch() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let limiter = Arc::new(tokio::sync::Mutex::new(RateLimiter::new(
        Arc::new(UnavailableStore),
        true,
    )));
    let key = EngineKey::new("npm").unwrap();
    let (seconds, published) = limiter
        .lock()
        .await
        .set_upstream_cooldown(&key, 30.0, f10_monotonic_seconds())
        .await
        .unwrap();
    assert_eq!(seconds, 30.0);
    assert!(published.is_ok());

    let mut config = PilotConfig::default();
    config.npm.base_url = format!("http://{address}/npm");
    let mut registry = Registry::builtins().unwrap();
    register_pilots(
        &mut registry,
        Arc::new(ssx_infra::http::AdapterHttp::default()),
        Arc::new(F10Admission::new(limiter)),
        config,
    )
    .unwrap();
    let response = registry
        .instantiate("npm")
        .unwrap()
        .search(&SearchRequest::default())
        .await;
    assert_eq!(response.status, EngineStatus::RateLimited);
    assert!(response.results.is_empty());
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), listener.accept())
            .await
            .is_err(),
        "composition-set cooldown must prevent upstream dispatch"
    );
}

#[tokio::test]
async fn dropping_registry_search_cancels_the_outstanding_f08_socket() {
    for engine in ["npm", "crates"] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (seen_tx, seen_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            let read = stream.read(&mut request).await.unwrap();
            seen_tx.send(()).unwrap();
            let mut tail = [0; 1];
            let closed = stream.read(&mut tail).await.unwrap() == 0;
            (read > 0, closed)
        });
        let mut config = PilotConfig::default();
        config.npm.base_url = format!("http://{address}/npm");
        config.crates.base_url = format!("http://{address}/crates");
        config.npm.timeout_ms = 10_000;
        config.crates.timeout_ms = 10_000;
        let mut registry = Registry::builtins().unwrap();
        register_pilots(
            &mut registry,
            Arc::new(ssx_infra::http::AdapterHttp::default()),
            Arc::new(AllowAllAdmission),
            config,
        )
        .unwrap();
        let adapter = registry.instantiate(engine).unwrap();
        let request = tokio::spawn(async move { adapter.search(&SearchRequest::default()).await });
        seen_rx.await.unwrap();
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        let (received, closed) = tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
        assert!(received);
        assert!(
            closed,
            "{engine}: dropping search must close its F08 socket"
        );
    }
}

fn fixture_payload(fixture: &str) -> String {
    let fixture: serde_json::Value = serde_json::from_str(fixture).unwrap();
    serde_json::to_string(&fixture["payload"]).unwrap()
}

async fn exercise_http_case(
    engine: &str,
    status: u16,
    body: &[u8],
    timeout_ms: u64,
    stall: bool,
) -> ssx_domain::AdapterResponse {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let body = body.to_vec();
    let accepted = Arc::new(AtomicUsize::new(0));
    let observed = accepted.clone();
    let server = OwnedLoopback::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        observed.fetch_add(1, Ordering::SeqCst);
        let mut request = Vec::new();
        let mut chunk = [0; 2048];
        loop {
            let count = stream.read(&mut chunk).await.unwrap();
            if count == 0 {
                assert!(
                    stall,
                    "non-timeout fixture saw EOF before complete request headers"
                );
                return;
            }
            request.extend_from_slice(&chunk[..count]);
            assert!(
                request.len() <= 16 * 1024,
                "loopback request headers exceeded fixture bound"
            );
            if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                break;
            }
        }
        if stall {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            return;
        }
        let reason = if status == 200 { "OK" } else { "Unavailable" };
        let header = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes()).await.unwrap();
        stream.write_all(&body).await.unwrap();
    });
    let mut config = PilotConfig::default();
    config.npm.base_url = format!("http://{address}/npm");
    config.npm.timeout_ms = timeout_ms;
    config.crates.base_url = format!("http://{address}/crates");
    config.crates.timeout_ms = timeout_ms;
    let mut registry = Registry::builtins().unwrap();
    register_pilots(
        &mut registry,
        Arc::new(ssx_infra::http::AdapterHttp::default()),
        Arc::new(AllowAllAdmission),
        config,
    )
    .unwrap();
    let response = registry
        .instantiate(engine)
        .unwrap()
        .search(&SearchRequest::default())
        .await;
    if stall {
        server.stop_and_join().await;
    } else {
        server.finish().await;
    }
    assert!(accepted.load(Ordering::SeqCst) <= 1);
    if stall && timeout_ms == 0 {
        assert_eq!(accepted.load(Ordering::SeqCst), 0);
    }
    response
}

struct DenyStore;
impl RateStore for DenyStore {
    fn acquire<'a>(
        &'a self,
        engine: &'a EngineKey,
        _: u64,
    ) -> PortFuture<'a, Result<QuotaReceipt, RateFailure>> {
        Box::pin(async move {
            Ok(QuotaReceipt {
                allowed: false,
                count: 1,
                bucket_key: engine.as_str().into(),
                server_time: ValkeyQuotaTime::from_milliseconds(1),
                remaining_ttl_ms: 1,
            })
        })
    }
    fn cooldown<'a>(
        &'a self,
        _: &'a EngineKey,
    ) -> PortFuture<'a, Result<SharedCooldown, RateFailure>> {
        Box::pin(async { Ok(SharedCooldown::Missing) })
    }
    fn publish_cooldown<'a>(
        &'a self,
        _: &'a EngineKey,
        _: f64,
    ) -> PortFuture<'a, Result<u64, RateFailure>> {
        Box::pin(async { Ok(1) })
    }
    fn warmup(&self) -> PortFuture<'_, Result<(), RateFailure>> {
        Box::pin(async { Ok(()) })
    }
    fn shutdown(&self) -> PortFuture<'_, ()> {
        Box::pin(async {})
    }
}

struct UnavailableStore;
impl RateStore for UnavailableStore {
    fn acquire<'a>(
        &'a self,
        _: &'a EngineKey,
        _: u64,
    ) -> PortFuture<'a, Result<QuotaReceipt, RateFailure>> {
        Box::pin(async { Err(RateFailure::Unavailable) })
    }
    fn cooldown<'a>(
        &'a self,
        _: &'a EngineKey,
    ) -> PortFuture<'a, Result<SharedCooldown, RateFailure>> {
        Box::pin(async { Ok(SharedCooldown::Missing) })
    }
    fn publish_cooldown<'a>(
        &'a self,
        _: &'a EngineKey,
        _: f64,
    ) -> PortFuture<'a, Result<u64, RateFailure>> {
        Box::pin(async { Ok(1) })
    }
    fn warmup(&self) -> PortFuture<'_, Result<(), RateFailure>> {
        Box::pin(async { Ok(()) })
    }
    fn shutdown(&self) -> PortFuture<'_, ()> {
        Box::pin(async {})
    }
}

struct ActiveCooldownStore {
    reads: Arc<AtomicUsize>,
}
impl RateStore for ActiveCooldownStore {
    fn acquire<'a>(
        &'a self,
        _: &'a EngineKey,
        _: u64,
    ) -> PortFuture<'a, Result<QuotaReceipt, RateFailure>> {
        Box::pin(async { panic!("active cooldown must stop before quota acquisition") })
    }
    fn cooldown<'a>(
        &'a self,
        _: &'a EngineKey,
    ) -> PortFuture<'a, Result<SharedCooldown, RateFailure>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(SharedCooldown::Active { seconds: 30 }) })
    }
    fn publish_cooldown<'a>(
        &'a self,
        _: &'a EngineKey,
        _: f64,
    ) -> PortFuture<'a, Result<u64, RateFailure>> {
        Box::pin(async { Ok(1) })
    }
    fn warmup(&self) -> PortFuture<'_, Result<(), RateFailure>> {
        Box::pin(async { Ok(()) })
    }
    fn shutdown(&self) -> PortFuture<'_, ()> {
        Box::pin(async {})
    }
}
