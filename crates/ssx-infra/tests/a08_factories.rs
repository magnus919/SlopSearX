#[path = "support/default_ca_environment.rs"]
mod default_ca_environment;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchContext};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{AllowAllAdmission, RegulatorySettings, edgar::EdgarConfig, register_regulatory},
    http::AdapterHttp,
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
            page: 8,
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
async fn wired(
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
    let startup = Instant::now();
    let (http, preparation) = AdapterHttp::new_ready(
        tokio::time::Instant::now() + Duration::from_secs(10),
        ssx_infra::http::Cancellation::default(),
    )
    .await
    .unwrap();
    eprintln!(
        "A08_STARTUP_DIAGNOSTIC engine={name} elapsed={:?} preparation={preparation:?}",
        startup.elapsed()
    );
    assert_eq!(preparation, ssx_infra::http::Preparation::Prepared);
    let mut registry = Registry::builtins().unwrap();
    register_regulatory(&mut registry, http.clone(), admission, config).unwrap();
    assert!(registry.is_available(name));
    (registry, http)
}
#[tokio::test]
async fn all_four_future_drops_close_real_pending_sockets() {
    if default_ca_environment::default_ca_test_environment(
        "all_four_future_drops_close_real_pending_sockets",
    ) {
        return;
    }
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
        )
        .await;
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
            )
            .await;
            let engine = registry.instantiate(name).unwrap();
            if timeout > 0 {
                let started = Instant::now();
                let server = tokio::spawn(async move {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let accepted = started.elapsed();
                    let mut data = vec![0; 8192];
                    let n = stream.read(&mut data).await.unwrap();
                    assert!(
                        n > 0,
                        "{name} timeout={timeout} accepted={accepted:?} read_elapsed={:?} bytes={n}",
                        started.elapsed()
                    );
                    let mut b = [0; 1];
                    assert_eq!(
                        tokio::time::timeout(Duration::from_secs(2), stream.read(&mut b))
                            .await
                            .unwrap()
                            .unwrap(),
                        0
                    )
                });
                let request = SearchRequest {
                    query: "test".into(),
                    ..Default::default()
                };
                let mut operation = engine.search(&request);
                let mut polls = 0usize;
                let result = std::future::poll_fn(|cx| {
                    let entered = Instant::now();
                    let state = std::future::Future::poll(operation.as_mut(), cx);
                    polls += 1;
                    eprintln!("A08_POLL_DIAGNOSTIC engine={name} timeout={timeout} poll={polls} entered={:?} duration={:?} ready={}", entered.duration_since(started), entered.elapsed(), state.is_ready());
                    state
                })
                .await;
                eprintln!(
                    "A08_TIMER_DIAGNOSTIC engine={name} timeout={timeout} elapsed={:?} status={:?} message={:?}",
                    started.elapsed(),
                    result.status,
                    result.error_message
                );
                assert_eq!(result.status, EngineStatus::Timeout, "{name}");
                assert!(result.results.is_empty());
                assert_eq!(row["requests"].as_array().unwrap().len(), 1);
                assert_eq!(row["socket_eof"], serde_json::json!([true]));
                server.await.unwrap();
            } else {
                let result = engine
                    .search(&SearchRequest {
                        query: "test".into(),
                        ..Default::default()
                    })
                    .await;
                assert_eq!(result.status, EngineStatus::Timeout);
                assert!(
                    tokio::time::timeout(Duration::from_millis(10), listener.accept())
                        .await
                        .is_err()
                );
                assert!(row["requests"].as_array().unwrap().is_empty());
            }
            http.shutdown_and_wait().await;
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
    if default_ca_environment::default_ca_test_environment(
        "all_four_denied_unavailable_and_pending_admission_stop_before_http",
    ) {
        return;
    }
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
            )
            .await;
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
