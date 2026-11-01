#[path = "support/default_ca_environment.rs"]
mod default_ca_environment;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchContext};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{AllowAllAdmission, RegulatorySettings, edgar::EdgarConfig, register_regulatory},
    http::{AdapterHttp, Cancellation, Options, Request},
};
use ssx_registry::Registry;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
const BODY: &str = r#"{"hits":{"hits":[{"_source":{"cik":"8","adsh":"0000000008-24-000001","display_names":["Company"],"form_type":"10-K","filed_at":"2024-01-01"},"_score":2},{"_source":{"cik":"8","adsh":"0000000008-24-000002","display_names":["Company"],"form_type":"10-Q"},"_score":1},{"_source":{"cik":"8","display_names":["Company"]}},{"_source":{"cik":"8","form_type":"10-Q"}}]}}"#;
#[tokio::test]
async fn edgar_factory_actual_loopback_request_and_distinct_filing_outcome() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(String::new()));
    let wire = seen.clone();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut data = vec![0; 8192];
        let n = stream.read(&mut data).await.unwrap();
        *wire.lock().unwrap() = String::from_utf8(data[..n].to_vec()).unwrap();
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{BODY}",
                    BODY.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap()
    });
    let http = Arc::new(AdapterHttp::default());
    let mut registry = Registry::builtins().unwrap();
    register_regulatory(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        settings(EdgarConfig::defaults(&format!("http://{addr}/search"))),
    )
    .unwrap();
    let result = registry
        .instantiate("edgar")
        .unwrap()
        .search(&SearchRequest {
            query: "a & b".into(),
            page: ssx_domain::SearchPage::from_u64(8).expect("positive fixture page"),
            max_results: Some(1),
            ..Default::default()
        })
        .await;
    server.await.unwrap();
    assert_eq!(result.status, EngineStatus::Ok);
    assert_eq!(result.results.len(), 3);
    assert_ne!(result.results[0].url, result.results[1].url);
    assert!(result.results[2].url.contains("browse-edgar"));
    assert_eq!(
        result.results[0].published_date.as_deref(),
        Some("2024-01-01")
    );
    assert_eq!(result.results[2].position, 3);
    assert_eq!(
        result.error_message.as_deref(),
        Some(
            "1 SEC result(s) omitted because no validated SEC URL or filing identity was available"
        )
    );
    {
        let wire = seen.lock().unwrap();
        assert!(wire.starts_with("GET /search?q=a+%26+b&limit=10&offset=0 HTTP/1.1\r\n"));
        assert!(
            wire.to_lowercase()
                .contains("user-agent: slopsearx/0.1.0 (meta search engine; research)")
        );
        assert!(wire.contains("application/json, text/plain, */*"));
    }
    http.shutdown_and_wait().await;
}
struct Denied;
impl SearchAdmission for Denied {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async { AdmissionDecision::Denied })
    }
}
#[tokio::test]
async fn edgar_denied_and_expired_context_have_zero_loopback_dispatch() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http = Arc::new(AdapterHttp::default());
    let mut registry = Registry::builtins().unwrap();
    register_regulatory(
        &mut registry,
        http.clone(),
        Arc::new(Denied),
        settings(EdgarConfig::defaults(&format!(
            "http://{}/search",
            listener.local_addr().unwrap()
        ))),
    )
    .unwrap();
    let engine = registry.instantiate("edgar").unwrap();
    assert_eq!(
        engine.search(&SearchRequest::default()).await.status,
        EngineStatus::RateLimited
    );
    assert_eq!(
        engine
            .search_with_context(
                &SearchRequest::default(),
                SearchContext {
                    deadline: Instant::now() - Duration::from_secs(1)
                }
            )
            .await
            .status,
        EngineStatus::Timeout
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(30), listener.accept())
            .await
            .is_err()
    );
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn edgar_drop_closes_real_pending_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (accepted, ready) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = vec![0; 8192];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        accepted.send(()).unwrap();
        let mut b = [0; 1];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), socket.read(&mut b))
                .await
                .unwrap()
                .unwrap(),
            0
        )
    });
    let http = Arc::new(AdapterHttp::default());
    let mut registry = Registry::builtins().unwrap();
    register_regulatory(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        settings(EdgarConfig::defaults(&format!("http://{addr}/slow"))),
    )
    .unwrap();
    let engine = registry.instantiate("edgar").unwrap();
    let q = SearchRequest::default();
    let mut future = engine.search(&q);
    tokio::select! {_=ready=>{},_= &mut future=>panic!("request must remain pending")};
    drop(future);
    server.await.unwrap();
    http.shutdown_and_wait().await;
}

fn settings(edgar: EdgarConfig) -> RegulatorySettings {
    RegulatorySettings {
        edgar,
        fred: Default::default(),
        openfda: EdgarConfig::defaults(ssx_infra::adapters::openfda::ENDPOINT),
        oyez: Default::default(),
    }
}

const NAMES: [&str; 4] = ["edgar", "fred", "openfda", "oyez"];
fn wired(
    name: &str,
    base: &str,
    timeout: i64,
    admission: Arc<dyn SearchAdmission>,
) -> (Registry, Arc<AdapterHttp>) {
    let mut config = settings(EdgarConfig::defaults(base));
    config.fred.settings.base_url = base.into();
    config.fred.api_key = ssx_config::Secret::new(Some("synthetic-fred-key".into()));
    config.openfda.base_url = base.into();
    config.oyez.search_url = Some(base.into());
    config.edgar.timeout_ms = ssx_config::ConfigScalar::Integer(timeout);
    config.fred.settings.timeout_ms = config.edgar.timeout_ms.clone();
    config.openfda.timeout_ms = config.edgar.timeout_ms.clone();
    config.oyez.settings.timeout_ms = config.edgar.timeout_ms.clone();
    let http = Arc::new(AdapterHttp::default());
    let mut registry = Registry::builtins().unwrap();
    register_regulatory(&mut registry, http.clone(), admission, config).unwrap();
    assert!(registry.is_available(name));
    (registry, http)
}
#[tokio::test]
async fn all_four_future_drops_close_real_pending_sockets() {
    for name in NAMES {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut data = vec![0; 8192];
            let count = stream.read(&mut data).await.unwrap();
            assert!(count > 0);
            if name != "edgar" {
                assert!(
                    String::from_utf8_lossy(&data[..count])
                        .to_ascii_lowercase()
                        .contains(&format!(
                            "user-agent: {}",
                            ssx_infra::http::DEFAULT_USER_AGENT.to_ascii_lowercase()
                        ))
                );
            }
            tx.send(()).unwrap();
            let mut b = [0; 1];
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), stream.read(&mut b))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            )
        });
        let (registry, http) = wired(
            name,
            &format!("http://{addr}/slow"),
            5000,
            Arc::new(AllowAllAdmission),
        );
        let engine = registry.instantiate(name).unwrap();
        let q = SearchRequest {
            query: "test".into(),
            ..Default::default()
        };
        let mut pending = engine.search(&q);
        tokio::select! {_=rx=>{},_= &mut pending=>panic!("{name} must remain pending")};
        drop(pending);
        server.await.unwrap();
        http.shutdown_and_wait().await;
    }
}
const LOOPBACK_GUARD: Duration = Duration::from_secs(5);
#[derive(Debug)]
struct LoopbackWire {
    request: String,
    body: Vec<u8>,
    socket_eof: Option<bool>,
}
async fn request_message(stream: &mut tokio::net::TcpStream) -> Result<(String, Vec<u8>), String> {
    let mut data = vec![0; 8192];
    let mut used = 0;
    let header_end = loop {
        if let Some(end) = data[..used].windows(4).position(|part| part == b"\r\n\r\n") {
            break end + 4;
        }
        if used == data.len() {
            return Err("bounded loopback request headers exceeded8192 bytes".into());
        }
        let count = stream
            .read(&mut data[used..])
            .await
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("loopback closed before complete request headers".into());
        }
        used += count;
    };
    let headers = String::from_utf8(data[..header_end].to_vec()).map_err(|e| e.to_string())?;
    let mut length = None;
    for line in headers
        .split("\r\n")
        .skip(1)
        .filter(|line| !line.is_empty())
    {
        let (name, value) = line
            .split_once(':')
            .ok_or("malformed loopback request header")?;
        if name.eq_ignore_ascii_case("transfer-encoding") || name.eq_ignore_ascii_case("trailer") {
            return Err("unsupported loopback request framing".into());
        }
        if name.eq_ignore_ascii_case("content-length") {
            let value = value.trim();
            if length.is_some()
                || value.is_empty()
                || !value.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err("invalid or duplicate loopback Content-Length".into());
            }
            let size = value
                .parse::<usize>()
                .map_err(|_| "loopback Content-Length overflow")?;
            if size > 4096 {
                return Err("loopback request body exceeded4096 bytes".into());
            }
            length = Some(size);
        }
    }
    if headers.starts_with("POST ") && length.is_none() {
        return Err("POST requires exact loopback Content-Length".into());
    }
    let length = length.unwrap_or(0);
    if used - header_end > length {
        return Err("buffered bytes exceed loopback Content-Length".into());
    }
    let mut body = data[header_end..used].to_vec();
    let buffered = body.len();
    body.resize(length, 0);
    stream
        .read_exact(&mut body[buffered..])
        .await
        .map_err(|e| e.to_string())?;
    Ok((headers, body))
}

// Both futures are owned here: a guard failure drops the socket/futures rather
// than detaching a server task. The completion signal only releases accept;
// once headers arrive, the timed path must still observe the actual socket EOF.
async fn bounded_loopback<T>(
    listener: &TcpListener,
    client: impl std::future::Future<Output = Result<T, String>>,
    respond: bool,
) -> Result<(T, Option<LoopbackWire>), String> {
    let (done, done_rx) = tokio::sync::oneshot::channel::<()>();
    let server = async move {
        let accepted = tokio::select! {
            biased;
            stream = tokio::time::timeout(LOOPBACK_GUARD, listener.accept()) => {
                Some(stream.map_err(|_| "loopback accept guard".to_owned())?.map_err(|e| e.to_string())?)
            }
            _ = done_rx => None,
        };
        let Some((mut stream, _)) = accepted else {
            return Ok::<_, String>(None);
        };
        let (request, body) = tokio::time::timeout(LOOPBACK_GUARD, request_message(&mut stream))
            .await
            .map_err(|_| "loopback request headers/body guard".to_owned())??;
        let socket_eof = if respond {
            tokio::time::timeout(
                LOOPBACK_GUARD,
                stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                ),
            )
            .await
            .map_err(|_| "loopback response guard".to_owned())?
            .map_err(|e| e.to_string())?;
            None
        } else {
            let mut byte = [0; 1];
            let count = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut byte))
                .await
                .map_err(|_| "loopback EOF guard".to_owned())?
                .map_err(|e| e.to_string())?;
            if count != 0 {
                return Err("timed socket produced further bytes instead of EOF".into());
            }
            Some(true)
        };
        Ok(Some(LoopbackWire {
            request,
            body,
            socket_eof,
        }))
    };
    let drive = async move {
        let result = tokio::time::timeout(LOOPBACK_GUARD, client)
            .await
            .map_err(|_| "loopback client guard".to_owned())
            .and_then(|result| result);
        let _ = done.send(());
        result
    };
    tokio::time::timeout(Duration::from_secs(15), async {
        let (wire, result) = tokio::join!(server, drive);
        Ok((result?, wire?))
    })
    .await
    .map_err(|_| "loopback join guard".to_owned())?
}
#[tokio::test]
async fn all_four_real_network_timers_match_actual_source_loopback() {
    if default_ca_environment::default_ca_test_environment(
        "all_four_real_network_timers_match_actual_source_loopback",
    ) {
        return;
    }
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a08/timeout-loopback.json"
    ))
    .unwrap();
    for name in NAMES {
        // Deterministic pre-dispatch regression: a listener with no connection
        // must finish with the engine, not await accept after its Timeout.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (registry, http) = wired(
            name,
            &format!("http://{}/expired", listener.local_addr().unwrap()),
            50,
            Arc::new(AllowAllAdmission),
        );
        let engine = registry.instantiate(name).unwrap();
        let query = SearchRequest {
            query: "test".into(),
            ..Default::default()
        };
        let expired = bounded_loopback(
            &listener,
            async {
                Ok(engine
                    .search_with_context(
                        &query,
                        SearchContext {
                            deadline: Instant::now() - Duration::from_secs(1),
                        },
                    )
                    .await)
            },
            false,
        )
        .await;
        let retained = http.retained_pools();
        http.shutdown_and_wait().await;
        let (result, wire) = expired.expect("expired-context fixture must finish without dispatch");
        assert_eq!(result.status, EngineStatus::Timeout, "{name}");
        assert!(result.results.is_empty());
        assert!(wire.is_none(), "{name} expired context dispatched HTTP");
        assert_eq!(retained, 0, "{name} expired context created a pool");

        for timeout in [0, -1, 50] {
            let row = source["records"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["engine"] == name && r["timeout_ms"] == timeout)
                .unwrap();
            assert_eq!(row["status"], "timeout");
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let (registry, http) = wired(
                name,
                &format!("http://{addr}/slow"),
                timeout,
                Arc::new(AllowAllAdmission),
            );
            let engine = registry.instantiate(name).unwrap();
            if timeout > 0 {
                // Separate session/request under5000ms, same default profile,
                // owner and origin. The timed adapter still receives50ms.
                let warm = bounded_loopback(
                    &listener,
                    async {
                        let mut session = http
                            .session(
                                Options::default(),
                                (Instant::now() + Duration::from_millis(5000)).into(),
                                Cancellation::default(),
                            )
                            .map_err(|e| format!("warm session: {e:?}"))?;
                        session
                            .execute(
                                Request::get(&format!("http://{addr}/warm"))
                                    .map_err(|e| format!("warm request: {e:?}"))?,
                            )
                            .await
                            .map_err(|e| format!("warm exchange: {e:?}"))
                    },
                    true,
                )
                .await;
                let reusable = http.retained_pools();
                let warm_ok = matches!(&warm, Ok((response, Some(wire)))
                    if response.status ==200 && response.body.is_empty()
                    && wire.request.starts_with("GET /warm HTTP/1.1\r\n") && wire.body.is_empty());
                if !warm_ok || reusable != 1 {
                    let overrides: Vec<_> = [
                        "HTTP_PROXY",
                        "HTTPS_PROXY",
                        "ALL_PROXY",
                        "http_proxy",
                        "https_proxy",
                        "all_proxy",
                        "SSL_CERT_FILE",
                        "SSL_CERT_DIR",
                    ]
                    .into_iter()
                    .filter(|key| std::env::var_os(key).is_some())
                    .collect();
                    http.shutdown_and_wait().await;
                    panic!(
                        "{name} reusable warmup failed: {warm:?}; pools={reusable}; override names={overrides:?}"
                    );
                }
            }
            let query = SearchRequest {
                query: "test".into(),
                ..Default::default()
            };
            let timed =
                bounded_loopback(&listener, async { Ok(engine.search(&query).await) }, false).await;
            http.shutdown_and_wait().await;
            let (result, wire) = timed.unwrap_or_else(|error| panic!("{name}/{timeout}: {error}"));
            assert_eq!(result.status, EngineStatus::Timeout, "{name}/{timeout}");
            assert!(result.results.is_empty());
            let actual: Vec<_> = wire.into_iter().collect();
            assert_eq!(
                actual.len(),
                usize::from(timeout > 0),
                "{name}/{timeout} actual dispatch count"
            );
            assert_eq!(row["requests"].as_array().unwrap().len(), actual.len());
            let eof: Vec<_> = actual
                .iter()
                .map(|wire| {
                    let method = if name == "oyez" { "POST" } else { "GET" };
                    assert!(wire.request.starts_with(&format!("{method} /slow")), "{name} timed method/path");
                    if name == "oyez" {
                        assert_eq!(wire.request.lines().next(), Some("POST /slow HTTP/1.1"));
                        assert!(wire.request.lines().any(|line| line.eq_ignore_ascii_case("content-type: application/json")));
                        let body: serde_json::Value = serde_json::from_slice(&wire.body).expect("complete Oyez JSON request body");
                        assert_eq!(body, serde_json::json!({"size":10,"query":{"multi_match":{
                            "query":"test","type":"cross_fields","fields":[
                                "field_docket_number^3", "field_additional_docket_numbers^3", "title^2",
                                "field_court_term", "field_first_party", "field_second_party",
                                "field_facts_of_the_case:value", "field_question:value", "field_conclusion:value",
                                "field_biography:value", "field_citation:field_volume", "field_citation:field_page",
                                "field_citation:field_year^0.2"
                            ]}}}));
                    } else {
                        assert!(wire.body.is_empty(), "{name} GET body must be empty");
                    }
                    wire.socket_eof
                        .expect("timed request must have actual EOF proof")
                })
                .collect();
            if timeout > 0 {
                assert_eq!(row["socket_eof"], serde_json::json!(eof));
            }
            assert!(
                tokio::time::timeout(Duration::from_millis(10), listener.accept())
                    .await
                    .is_err(),
                "{name}/{timeout} extra timed connection"
            );
        }
    }
}
struct Unavailable;
impl SearchAdmission for Unavailable {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async { AdmissionDecision::Unavailable })
    }
}
struct Pending;
impl SearchAdmission for Pending {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(std::future::pending())
    }
}
#[tokio::test]
async fn all_four_denied_unavailable_and_pending_admission_stop_before_http() {
    for name in NAMES {
        for (admission, status) in [
            (
                Arc::new(Denied) as Arc<dyn SearchAdmission>,
                EngineStatus::RateLimited,
            ),
            (Arc::new(Unavailable), EngineStatus::Error),
            (Arc::new(Pending), EngineStatus::Timeout),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let (registry, http) = wired(
                name,
                &format!("http://{}/blocked", listener.local_addr().unwrap()),
                5000,
                admission,
            );
            let engine = registry.instantiate(name).unwrap();
            let q = SearchRequest {
                query: "test".into(),
                ..Default::default()
            };
            let result = engine
                .search_with_context(
                    &q,
                    SearchContext {
                        deadline: Instant::now() + Duration::from_millis(20),
                    },
                )
                .await;
            assert_eq!(result.status, status, "{name}");
            assert!(result.results.is_empty());
            assert!(
                tokio::time::timeout(Duration::from_millis(5), listener.accept())
                    .await
                    .is_err()
            );
            http.shutdown_and_wait().await;
        }
    }
}
#[test]
fn actual_resolved_config_snapshots_and_audited_metadata() {
    let path = std::env::temp_dir().join(format!("a08-config-{}.yaml", std::process::id()));
    std::fs::write(&path,"engines:\n  edgar:\n    base_url: '  '\n    max_results: '2'\n  openfda: {}\n  fred:\n    api_key: file-key\n  oyez:\n    base_url: ''\n").unwrap();
    let env = std::collections::BTreeMap::from([
        ("ENGINE_FRED_API_KEY".into(), "synthetic-fred-key".into()),
        ("ENGINE_OPENFDA_MAX_RESULTS".into(), "3".into()),
        (
            "ENGINE_OYEZ_SEARCH_URL".into(),
            "https://ignored.test".into(),
        ),
    ]);
    let config = ssx_config::load_config_with(Some(&path), &env).unwrap();
    std::fs::remove_file(&path).unwrap();
    let resolved = RegulatorySettings::from_resolved(&config);
    assert_eq!(
        resolved.edgar.base_url,
        ssx_infra::adapters::edgar::ENDPOINT
    );
    assert_eq!(
        resolved.edgar.max_results,
        ssx_config::ConfigScalar::Text("2".into())
    );
    assert_eq!(
        resolved.openfda.max_results,
        ssx_config::ConfigScalar::Integer(3)
    );
    assert_eq!(resolved.fred.api_key.expose(), Some("synthetic-fred-key"));
    assert!(!format!("{resolved:?}").contains("synthetic-fred-key"));
    assert!(resolved.oyez.search_url.is_none());
    assert_eq!(resolved.oyez.settings.base_url, "");
    let mut registry = Registry::builtins().unwrap();
    register_regulatory(
        &mut registry,
        Arc::new(AdapterHttp::default()),
        Arc::new(AllowAllAdmission),
        resolved,
    )
    .unwrap();
    for name in NAMES {
        let d = registry.descriptor(name).unwrap();
        assert!(registry.is_available(name));
        assert_eq!(d.routing.role, ssx_registry::RoutingRole::Specialist);
        assert!(d.routing.purpose.as_deref().is_some_and(|s| !s.is_empty()));
        assert!(d.enforced_filters.is_empty());
    }
}
#[test]
fn oyez_yaml_direct_search_override_and_nontext_secrets_remain_rejected_by_f06() {
    for yaml in [
        "engines:\n  fred:\n    api_key: true\n",
        "engines:\n  oyez:\n    search_url: https://local.test/search\n",
        "engines:\n  fred:\n    api_key: 42\n",
        "engines:\n  oyez:\n    max_results: .nan\n",
        "engines:\n  oyez:\n    max_results: .inf\n",
        "engines:\n  oyez:\n    max_results: -.inf\n",
    ] {
        let path = std::env::temp_dir().join(format!("a08-reject-{}.yaml", std::process::id()));
        std::fs::write(&path, yaml).unwrap();
        assert!(ssx_config::load_config_with(Some(&path), &Default::default()).is_err());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn actual_registered_metadata_drives_specialist_scope_selection() {
    use ssx_application::scope::{CatalogSnapshot, EngineCapability, ScopeResolver};
    let config = ssx_config::load_config_with(None, &Default::default()).unwrap();
    let settings = RegulatorySettings::from_resolved(&config);
    let mut registry = Registry::builtins().unwrap();
    register_regulatory(
        &mut registry,
        Arc::new(AdapterHttp::default()),
        Arc::new(AllowAllAdmission),
        settings,
    )
    .unwrap();
    let mut caps = vec![];
    for name in NAMES {
        let d = registry.descriptor(name).unwrap();
        assert!(d.enabled_by_default);
        assert_eq!(
            d.auth_class,
            if name == "fred" {
                ssx_registry::AuthClass::Required
            } else {
                ssx_registry::AuthClass::None
            }
        );
        assert_eq!(
            d.cost_class.as_deref(),
            Some(if name == "fred" { "freemium" } else { "free" })
        );
        assert!(d.supported_filters.values().all(|v| !*v));
        assert_eq!(d.supported_result_types, vec!["text"]);
        assert!(d.supported_media_types.is_empty());
        assert!(d.routing.use_when.as_deref().is_some_and(|s| !s.is_empty()));
        caps.push(EngineCapability {
            name: name.into(),
            enabled: true,
            active: true,
            available: registry.is_available(name),
            categories: d.categories.clone(),
            media_types: vec![],
            auth_ready: true,
            circuit_open: false,
            cost_class: d.cost_class.clone().unwrap(),
            health_status: "healthy".into(),
            health_stale: false,
            enforced_filters: d.enforced_filters.clone(),
        });
    }
    let catalog = CatalogSnapshot::new(caps).unwrap();
    for (category, expected) in [
        ("finance", vec!["edgar", "fred"]),
        ("legal", vec!["oyez"]),
        ("medical", vec!["openfda"]),
    ] {
        let selected = ScopeResolver::default().resolve(
            &SearchRequest {
                query: "test".into(),
                categories: Some(vec![category.into()]),
                ..Default::default()
            },
            &catalog,
        );
        assert_eq!(
            selected
                .selected_engines
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>(),
            expected.into_iter().map(str::to_owned).collect()
        );
    }
}
