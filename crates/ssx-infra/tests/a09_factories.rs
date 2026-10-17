//! Real private-loopback native A09 F06 factories and caller-owned lifecycle.
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchContext};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{AllowAllAdmission, CatalogMediaSettings, register_catalog_media},
    http::{AdapterHttp, Cancellation, Preparation},
};
use ssx_registry::Registry;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
#[path = "support/default_ca_environment.rs"]
mod default_ca_environment;

const NAMES: [&str; 5] = [
    "openlibrary",
    "musicbrainz",
    "tmdb",
    "nominatim",
    "internetarchive",
];
const BODIES: [&str; 5] = [
    r#"{"docs":[{"key":"/works/OL1W","title":"Book","cover_i":7}]}"#,
    r#"{"artists":[{"id":"a","name":"Artist"}]}"#,
    r#"{"results":[{"media_type":"movie","id":42,"title":"Movie","vote_average":7.5}]}"#,
    r#"[{"display_name":"Place","osm_type":"node","osm_id":2}]"#,
    r#"{"response":{"docs":[{"identifier":"book","title":"Archive"}]}}"#,
];
fn private_endpoint(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    parsed.scheme() == "http"
        && matches!(parsed.host(),Some(url::Host::Ipv4(ip)) if ip.is_loopback())
        && parsed.port().is_some_and(|p| p != 0)
        && parsed.username().is_empty()
        && parsed.password().is_none()
}
fn proxy_enabled(flags: &[(String, i32)]) -> bool {
    flags.iter().any(|(_, v)| *v != 0)
}
fn audit_system_proxy() {
    static SAFE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    SAFE.get_or_init(|| {
        #[cfg(target_os = "macos")]
        {
            let out = std::process::Command::new("/usr/sbin/scutil")
                .arg("--proxy")
                .output()
                .expect("system proxy audit must execute before any loopback call");
            assert!(out.status.success(), "system proxy audit failed closed");
            let raw = String::from_utf8(out.stdout).expect("system proxy audit encoding");
            let keys = [
                "HTTPEnable",
                "HTTPSEnable",
                "SOCKSEnable",
                "ProxyAutoConfigEnable",
                "ProxyAutoDiscoveryEnable",
            ];
            let flags = keys.map(|key| {
                let values: Vec<_> = raw
                    .lines()
                    .filter_map(|line| {
                        let (name, value) = line.trim().split_once(':')?;
                        (name.trim() == key).then(|| {
                            value
                                .trim()
                                .parse::<i32>()
                                .expect("proxy flag must be integer")
                        })
                    })
                    .collect();
                assert!(values.len() <= 1, "ambiguous system proxy flag");
                (key.to_owned(), values.first().copied().unwrap_or(0))
            });
            assert!(
                !proxy_enabled(&flags),
                "system proxy enabled: loopback execution denied"
            );
            println!(
                "A09_SYSTEM_PROXY_FLAGS {}",
                serde_json::to_string(&flags).unwrap()
            );
        }
    });
}
fn assert_loopback_environment(url: &str, env: &BTreeMap<String, String>) {
    assert!(
        private_endpoint(url),
        "non-private endpoint denied before registration"
    );
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        assert!(
            env.get(key).is_none_or(|v| v.is_empty()),
            "proxy environment denied before loopback dispatch: {key}"
        );
    }
    audit_system_proxy();
}
fn assert_loopback(url: &str) {
    assert_loopback_environment(url, &std::env::vars().collect());
}
#[test]
fn endpoint_guard_rejects_unresolved_public_fallbacks_and_proxy_flags() {
    assert!(private_endpoint("http://127.0.0.1:12345/path"));
    for u in [
        "https://openlibrary.org/search.json",
        "http://127.0.0.1/path",
        "http://example.com:12345/",
        "http://user:secret@127.0.0.1:12345/",
        ssx_infra::adapters::internetarchive::AVAILABILITY,
    ] {
        assert!(!private_endpoint(u));
    }
    let cfg = ssx_config::load_config_with(
        None,
        &BTreeMap::from([(
            "ENGINE_OPENLIBRARY_BASE_URL".into(),
            "http://127.0.0.1:12345/ignored-without-entry".into(),
        )]),
    )
    .unwrap();
    let resolved = CatalogMediaSettings::from_resolved(&cfg);
    assert_eq!(
        resolved.openlibrary.base_url,
        ssx_infra::adapters::openlibrary::ENDPOINT
    );
    assert!(!private_endpoint(&resolved.openlibrary.base_url));
    assert!(proxy_enabled(&[("HTTP_PROXY".into(), 1)]));
    for flag in [
        "HTTPEnable",
        "HTTPSEnable",
        "SOCKSEnable",
        "ProxyAutoConfigEnable",
        "ProxyAutoDiscoveryEnable",
    ] {
        assert!(proxy_enabled(&[(flag.into(), 1)]));
    }
    assert!(!proxy_enabled(&[("HTTPEnable".into(), 0)]));
}
#[test]
fn negative_guard_cases_stop_before_registration_and_dispatch() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let registration = AtomicUsize::new(0);
    let dispatch = AtomicUsize::new(0);
    let cfg = ssx_config::load_config_with(
        None,
        &BTreeMap::from([(
            "ENGINE_OPENLIBRARY_BASE_URL".into(),
            "http://127.0.0.1:12345/ignored".into(),
        )]),
    )
    .unwrap();
    let settings = CatalogMediaSettings::from_resolved(&cfg);
    let public = "https://openlibrary.org/search.json";
    for (base, availability, env) in [
        (public, None, BTreeMap::new()),
        (
            settings.openlibrary.base_url.as_str(),
            None,
            BTreeMap::new(),
        ),
        (
            "http://127.0.0.1:12345/base",
            Some(ssx_infra::adapters::internetarchive::AVAILABILITY),
            BTreeMap::new(),
        ),
        (
            "http://127.0.0.1:12345/base",
            None,
            BTreeMap::from([("HTTP_PROXY".into(), "http://proxy.invalid:3128".into())]),
        ),
    ] {
        let result = std::panic::catch_unwind(|| {
            assert_loopback_environment(base, &env);
            if let Some(url) = availability {
                assert_loopback_environment(url, &env)
            }
            registration.fetch_add(1, Ordering::SeqCst);
            dispatch.fetch_add(1, Ordering::SeqCst);
        });
        assert!(result.is_err());
        assert_eq!(registration.load(Ordering::SeqCst), 0);
        assert_eq!(dispatch.load(Ordering::SeqCst), 0);
    }
}
fn resolved(base: &str, timeout: i64) -> CatalogMediaSettings {
    let mut env = BTreeMap::new();
    for name in NAMES {
        env.insert(
            format!("ENGINE_{}_BASE_URL", name.to_ascii_uppercase()),
            base.into(),
        );
        env.insert(
            format!("ENGINE_{}_TIMEOUT_MS", name.to_ascii_uppercase()),
            timeout.to_string(),
        );
    }
    env.insert("ENGINE_TMDB_API_KEY".into(), "synthetic-tmdb-key".into());
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "a09-loopback-config-{}-{}.yaml",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let yaml = format!(
        "engines:\n{}",
        NAMES
            .iter()
            .map(|name| format!("  {name}: {{}}\n"))
            .collect::<String>()
    );
    std::fs::write(&path, yaml).unwrap();
    let cfg = ssx_config::load_config_with(Some(&path), &env).unwrap();
    std::fs::remove_file(path).unwrap();
    let settings = CatalogMediaSettings::from_resolved(&cfg);
    for resolved in [
        &settings.openlibrary.base_url,
        &settings.musicbrainz.base_url,
        &settings.tmdb.settings.base_url,
        &settings.nominatim.base_url,
        &settings.internetarchive.settings.base_url,
    ] {
        assert_eq!(
            resolved, base,
            "endpoint must resolve to exact private-loopback before any dispatch"
        );
        assert_loopback(resolved);
    }
    settings
}
fn wired(
    base: &str,
    timeout: i64,
    admission: Arc<dyn SearchAdmission>,
) -> (Registry, Arc<AdapterHttp>) {
    let mut r = Registry::builtins().unwrap();
    let h = Arc::new(AdapterHttp::default());
    register_catalog_media(&mut r, h.clone(), admission, resolved(base, timeout)).unwrap();
    (r, h)
}
#[tokio::test]
async fn five_f06_factories_execute_exact_real_loopback_requests_and_outcomes() {
    for (name, body) in NAMES.into_iter().zip(BODIES) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0; 8192];
            let n = socket.read(&mut bytes).await.unwrap();
            let request = String::from_utf8(bytes[..n].to_vec()).unwrap();
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            request
        });
        let (registry, http) = wired(
            &format!("http://{addr}/base"),
            5000,
            Arc::new(AllowAllAdmission),
        );
        let engine = registry.instantiate(name).unwrap();
        let response = engine
            .search(&SearchRequest {
                query: "a & b".into(),
                page: 8,
                max_results: Some(1),
                ..Default::default()
            })
            .await;
        assert_eq!(response.status, EngineStatus::Ok, "{name}");
        assert_eq!(response.results.len(), 1);
        assert!(response.results[0].media.is_none());
        assert!(response.results[0].img_src.is_none());
        let wire = server.await.unwrap();
        let path = match name {
            "openlibrary" => "/base?q=a+%26+b&limit=10",
            "musicbrainz" => "/base/artist/?query=a+%26+b&limit=10&fmt=json",
            "tmdb" => "/base?query=a+%26+b&page=1",
            "nominatim" => "/base?q=a+%26+b&format=json&limit=10&addressdetails=1",
            _ => "/base/advancedsearch.php?q=a%20%26%20b&output=json&rows=10",
        };
        assert!(
            wire.starts_with(&format!("GET {path} HTTP/1.1\r\n")),
            "{wire}"
        );
        assert_eq!(
            wire.to_lowercase()
                .contains("authorization: bearer synthetic-tmdb-key"),
            name == "tmdb"
        );
        assert!(!wire.contains("api_key="));
        if name == "openlibrary" {
            assert_eq!(
                response.results[0].thumbnail.as_deref(),
                Some("https://covers.openlibrary.org/b/id/7-M.jpg")
            );
        }
        drop(engine);
        http.shutdown_and_wait().await;
    }
}
struct Denied;
impl SearchAdmission for Denied {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async { AdmissionDecision::Denied })
    }
}
struct Pending;
impl SearchAdmission for Pending {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(std::future::pending())
    }
}
struct Unavailable;
impl SearchAdmission for Unavailable {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async { AdmissionDecision::Unavailable })
    }
}
#[tokio::test]
async fn five_admission_denial_unavailable_and_deadline_have_zero_http() {
    for name in NAMES {
        for (admission, status) in [
            (
                Arc::new(Denied) as Arc<dyn SearchAdmission>,
                EngineStatus::RateLimited,
            ),
            (Arc::new(Pending), EngineStatus::Timeout),
            (Arc::new(Unavailable), EngineStatus::Error),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let (r, h) = wired(
                &format!("http://{}/blocked", listener.local_addr().unwrap()),
                5000,
                admission,
            );
            let result = r
                .instantiate(name)
                .unwrap()
                .search_with_context(
                    &SearchRequest {
                        query: "test".into(),
                        ..Default::default()
                    },
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
            h.shutdown_and_wait().await;
        }
    }
}
#[tokio::test]
async fn five_future_drops_and_owner_shutdown_close_real_pending_sockets() {
    for name in NAMES {
        for shutdown in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let (tx, rx) = tokio::sync::oneshot::channel();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut data = [0; 8192];
                assert!(socket.read(&mut data).await.unwrap() > 0);
                tx.send(()).unwrap();
                let mut b = [0; 1];
                assert_eq!(
                    tokio::time::timeout(Duration::from_secs(2), socket.read(&mut b))
                        .await
                        .unwrap()
                        .unwrap(),
                    0
                );
            });
            let (r, h) = wired(
                &format!("http://{addr}/slow"),
                5000,
                Arc::new(AllowAllAdmission),
            );
            let engine = r.instantiate(name).unwrap();
            let q = SearchRequest {
                query: "test".into(),
                ..Default::default()
            };
            let mut future = engine.search(&q);
            tokio::select! {_=rx=>{},_= &mut future=>panic!("must remain pending")};
            if shutdown {
                h.shutdown();
                let result = future.await;
                assert_eq!(result.status, EngineStatus::Error);
            } else {
                drop(future)
            }
            server.await.unwrap();
            h.shutdown_and_wait().await;
        }
    }
}
#[tokio::test]
async fn five_real_timeout_and_expired_context_cleanup() {
    if default_ca_environment::default_ca_test_environment(
        "five_real_timeout_and_expired_context_cleanup",
    ) {
        return;
    }
    for name in NAMES {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (r, h) = wired(
            &format!("http://{addr}/slow"),
            30,
            Arc::new(AllowAllAdmission),
        );
        // Prepare the ordinary pool before the unchanged request deadline starts.
        assert_eq!(
            h.prepare_default(
                tokio::time::Instant::now() + Duration::from_secs(5),
                Cancellation::default(),
            )
            .await
            .unwrap(),
            Preparation::Prepared
        );
        let server = tokio::spawn(async move {
            let (mut socket, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
                .await
                .expect("prepared request must reach the loopback server")
                .unwrap();
            let mut data = [0; 8192];
            assert!(socket.read(&mut data).await.unwrap() > 0);
            let mut b = [0; 1];
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), socket.read(&mut b))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
        });
        let result = r
            .instantiate(name)
            .unwrap()
            .search(&SearchRequest {
                query: "test".into(),
                ..Default::default()
            })
            .await;
        assert_eq!(
            result.status,
            if name == "internetarchive" {
                EngineStatus::Error
            } else {
                EngineStatus::Timeout
            }
        );
        let server_result = server.await;
        let result = r
            .instantiate(name)
            .unwrap()
            .search_with_context(
                &SearchRequest::default(),
                SearchContext {
                    deadline: Instant::now() - Duration::from_secs(1),
                },
            )
            .await;
        assert_eq!(result.status, EngineStatus::Timeout);
        h.shutdown_and_wait().await;
        server_result.unwrap();
    }
}
#[tokio::test]
async fn archive_real_cdx_timer_one_fallback_and_aggregate_context() {
    use ssx_application::SearchEngine;
    for remaining in [true, false] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut first, _) = listener.accept().await.unwrap();
            let mut data = [0; 8192];
            let n = first.read(&mut data).await.unwrap();
            assert!(String::from_utf8_lossy(&data[..n]).starts_with("GET /cdx?url=example.com&output=json&limit=10&filter=statuscode:200&collapse=timestamp:6 "));
            let mut b = [0; 1];
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), first.read(&mut b))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
            if remaining {
                let (mut next, _) = listener.accept().await.unwrap();
                let n = next.read(&mut data).await.unwrap();
                assert!(
                    String::from_utf8_lossy(&data[..n])
                        .starts_with("GET /available?url=example.com ")
                );
                let body = r#"{"archived_snapshots":{"closest":{"available":true,"status":"200","timestamp":"20240102123456","url":"http://web.archive.org/web/20240102123456/https://example.com/"}}}"#;
                next.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            }
            assert!(
                tokio::time::timeout(Duration::from_millis(50), listener.accept())
                    .await
                    .is_err()
            );
        });
        let h = Arc::new(AdapterHttp::default());
        let mut c = ssx_infra::adapters::internetarchive::InternetArchiveConfig::default();
        c.settings.timeout_ms = ssx_config::ConfigScalar::Integer(200);
        c.cdx_url = Some(format!("http://{addr}/cdx"));
        c.availability_url = Some(format!("http://{addr}/available"));
        assert_loopback(c.cdx_url.as_deref().unwrap());
        assert_loopback(c.availability_url.as_deref().unwrap());
        let engine = ssx_infra::adapters::internetarchive::InternetArchiveAdapter::new(
            h.clone(),
            Arc::new(AllowAllAdmission),
            c,
        );
        let q = SearchRequest {
            query: "example.com".into(),
            ..Default::default()
        };
        let response = if remaining {
            engine.search(&q).await
        } else {
            engine
                .search_with_context(
                    &q,
                    SearchContext {
                        deadline: Instant::now() + Duration::from_millis(30),
                    },
                )
                .await
        };
        assert_eq!(
            response.status,
            if remaining {
                EngineStatus::Ok
            } else {
                EngineStatus::Timeout
            }
        );
        if remaining {
            assert_eq!(response.results.len(), 1);
            assert!(
                response
                    .error_message
                    .unwrap()
                    .contains("not a CDX history")
            );
        } else {
            assert!(response.results.is_empty());
        }
        server.await.unwrap();
        h.shutdown_and_wait().await;
    }
}
#[test]
fn resolved_yaml_env_secret_defaults_and_metadata() {
    let path = std::env::temp_dir().join(format!("a09-config-{}.yaml", std::process::id()));
    std::fs::write(&path,"engines:\n  tmdb:\n    api_key: yaml-secret\n    max_results: '2'\n  openlibrary: {}\n  internetarchive: {}\n").unwrap();
    let cfg = ssx_config::load_config_with(
        Some(&path),
        &BTreeMap::from([
            ("ENGINE_TMDB_API_KEY".into(), "env-secret".into()),
            ("ENGINE_OPENLIBRARY_MAX_RESULTS".into(), "3".into()),
        ]),
    )
    .unwrap();
    std::fs::remove_file(&path).unwrap();
    let settings = CatalogMediaSettings::from_resolved(&cfg);
    assert_eq!(settings.tmdb.api_key.expose(), Some("env-secret"));
    assert!(!format!("{settings:?}").contains("env-secret"));
    assert!(!format!("{settings:?}").contains("yaml-secret"));
    assert_eq!(
        settings.tmdb.settings.max_results,
        ssx_config::ConfigScalar::Text("2".into())
    );
    assert_eq!(
        settings.openlibrary.max_results,
        ssx_config::ConfigScalar::Integer(3)
    );
    assert!(!cfg.engines["internetarchive"].enabled);
    let mut registry = Registry::builtins().unwrap();
    register_catalog_media(
        &mut registry,
        Arc::new(AdapterHttp::default()),
        Arc::new(AllowAllAdmission),
        settings,
    )
    .unwrap();
    for name in NAMES {
        let d = registry.descriptor(name).unwrap();
        assert!(registry.is_available(name));
        assert!(d.enforced_filters.is_empty());
        assert!(d.supported_filters.values().all(|v| !*v));
        assert_eq!(d.routing.role, ssx_registry::RoutingRole::Specialist);
        assert!(d.routing.purpose.as_ref().is_some_and(|v| !v.is_empty()));
        if name == "internetarchive" {
            assert!(!d.enabled_by_default);
            assert_eq!(d.categories, vec!["reference", "web:archive", "historical"]);
        }
        if name == "tmdb" {
            assert_eq!(d.auth_class, ssx_registry::AuthClass::Required);
        }
    }
}

fn timeout_output(response: &ssx_domain::AdapterResponse) -> serde_json::Value {
    serde_json::json!({"status":&response.status,"error_message":response.error_message,"results":response.results.iter().map(|r|{assert!(r.media.is_none());serde_json::json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"position":r.position,"score":r.score.get(),"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"media":serde_json::Value::Null,"payload":r.payload.as_ref().map(|p|serde_json::from_str::<serde_json::Value>(&p.to_string()).unwrap())})}).collect::<Vec<_>>()})
}
#[tokio::test]
async fn actual_source_archive_configured_timers_keep_error_and_context_timeout_distinct() {
    let source: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/rust-rewrite/a09/timeouts.json")).unwrap();
    let differences: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a09/timeout-differences.json"
    ))
    .unwrap();
    for case in source["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["timeout_ms"].is_i64())
    {
        let millis = case["timeout_ms"].as_i64().unwrap();
        let stall = case["stall"].as_bool().unwrap();
        let listener = Arc::new(TcpListener::bind("127.0.0.1:0").await.unwrap());
        let addr = listener.local_addr().unwrap();
        let (registry, http) = wired(
            &format!("http://{addr}"),
            millis,
            Arc::new(AllowAllAdmission),
        );
        let server = if millis > 0 {
            let listener = Arc::clone(&listener);
            Some(tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut raw = [0; 8192];
                let n = socket.read(&mut raw).await.unwrap();
                let wire = String::from_utf8(raw[..n].to_vec()).unwrap();
                assert!(wire.starts_with(
                    "GET /advancedsearch.php?q=books&output=json&rows=10 HTTP/1.1\r\n"
                ));
                assert!(!wire.to_lowercase().contains("authorization:"));
                if !stall {
                    let body = r#"{"response":{"docs":[{"identifier":"book","title":"Book","downloads":5000}]}}"#;
                    socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
                }
                let mut b = [0; 1];
                assert_eq!(
                    tokio::time::timeout(Duration::from_secs(2), socket.read(&mut b))
                        .await
                        .unwrap()
                        .unwrap(),
                    0
                );
                wire
            }))
        } else {
            None
        };
        let response = registry
            .instantiate("internetarchive")
            .unwrap()
            .search(&SearchRequest {
                query: "books".into(),
                ..Default::default()
            })
            .await;
        let expected = if let Some(d) = differences["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["id"] == case["id"])
        {
            assert_eq!(d["source_output"], case["output"]);
            &d["native_output"]
        } else {
            &case["output"]
        };
        assert_eq!(timeout_output(&response), *expected, "{}", case["id"]);
        if let Some(server) = server {
            let wire = server.await.unwrap();
            assert_eq!(case["requests"].as_array().unwrap().len(), 1);
            assert!(
                case["requests"][0]
                    .as_str()
                    .unwrap()
                    .starts_with(wire.lines().next().unwrap())
            );
        } else {
            assert!(case["requests"].as_array().unwrap().is_empty());
        }
        // A caller's already-expired execution context takes precedence over configured Error.
        let expired = registry
            .instantiate("internetarchive")
            .unwrap()
            .search_with_context(
                &SearchRequest {
                    query: "books".into(),
                    ..Default::default()
                },
                SearchContext {
                    deadline: Instant::now() - Duration::from_secs(1),
                },
            )
            .await;
        assert_eq!(expired.status, EngineStatus::Timeout);
        http.shutdown_and_wait().await;
        assert!(
            tokio::time::timeout(Duration::from_millis(5), listener.accept())
                .await
                .is_err(),
            "no request for nonpositive timeout and no extra request after positive timeout"
        );
        println!("\nA09_TIMER_CASE {}", case["id"]);
    }
}
#[test]
fn actual_f06_nonfinite_timeout_rejects_before_instantiation_supersedes_raw_adapter_acceptance() {
    let source: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/rust-rewrite/a09/timeouts.json")).unwrap();
    for case in source["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["timeout_ms"].is_object())
    {
        let raw = match case["timeout_ms"]["$nonfinite"].as_str().unwrap() {
            "inf" => ".inf",
            "-inf" => "-.inf",
            "nan" => ".nan",
            _ => panic!("unreviewed scalar"),
        };
        let path = std::env::temp_dir().join(format!(
            "a09-nonfinite-timeout-{}-{}.yaml",
            std::process::id(),
            case["id"].as_str().unwrap()
        ));
        std::fs::write(
            &path,
            format!("engines:\n  internetarchive:\n    timeout_ms: {raw}\n"),
        )
        .unwrap();
        let config = ssx_config::load_config_with(Some(&path), &BTreeMap::new());
        std::fs::remove_file(path).unwrap();
        assert!(
            config.is_err(),
            "F06 typed JSON profile must deny before factory registration/instantiation"
        );
        println!("\nA09_TIMER_CASE {}", case["id"]);
    }
}
