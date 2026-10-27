//! Real private-loopback native A09 F06 factories and caller-owned lifecycle.
#[path = "support/default_ca_environment.rs"]
mod default_ca_environment;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchContext};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{AllowAllAdmission, CatalogMediaSettings, register_catalog_media},
    http::{AdapterHttp, Cancellation, Options, Request},
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
// Diagnostic harness bounds do not change any configured adapter/context timer.
async fn a09_stage<T>(
    label: &str,
    future: impl std::future::Future<Output = std::io::Result<T>>,
) -> T {
    tokio::time::timeout(Duration::from_secs(2), future)
        .await
        .unwrap_or_else(|_| panic!("{label}: fixture stage guard elapsed"))
        .unwrap_or_else(|error| panic!("{label}: fixture I/O failed: {error}"))
}
async fn a09_headers(socket: &mut tokio::net::TcpStream, label: &str) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut bytes = Vec::new();
        loop {
            let mut buffer = [0; 1024];
            let n = socket.read(&mut buffer).await.unwrap();
            assert!(n > 0, "{label}: EOF before complete HTTP headers");
            bytes.extend_from_slice(&buffer[..n]);
            assert!(bytes.len() <= 8192, "{label}: fixture header bound");
            if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                assert_eq!(bytes.len(), end + 4, "{label}: unexpected GET body bytes");
                return bytes;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{label}: complete HTTP header guard elapsed"))
}
struct A09OwnedServer<T>(tokio::task::JoinHandle<T>);
impl<T> Drop for A09OwnedServer<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}
fn a09_spawn<T: Send + 'static>(
    future: impl std::future::Future<Output = T> + Send + 'static,
) -> A09OwnedServer<T> {
    A09OwnedServer(tokio::spawn(future))
}
async fn a09_join<T>(task: A09OwnedServer<T>, label: &str) -> Result<T, String> {
    let mut owned = task;
    match tokio::time::timeout(Duration::from_secs(5), &mut owned.0).await {
        Ok(result) => result.map_err(|error| format!("{label}: server failed: {error}")),
        Err(_) => {
            owned.0.abort();
            let reap = match tokio::time::timeout(Duration::from_secs(1), &mut owned.0).await {
                Ok(Ok(_)) => "completed".to_owned(),
                Ok(Err(error)) => format!("{error}"),
                Err(_) => "abort reap guard elapsed".to_owned(),
            };
            Err(format!(
                "{label}: server join guard elapsed; abort/reap: {reap}"
            ))
        }
    }
}
async fn a09_client(
    future: impl std::future::Future<Output = ssx_domain::AdapterResponse>,
) -> Result<ssx_domain::AdapterResponse, tokio::time::error::Elapsed> {
    // Timeout owns/drops the pending response future before caller HTTP drain.
    tokio::time::timeout(Duration::from_secs(5), future).await
}
async fn a09_drain(http: &AdapterHttp) -> Result<(), tokio::time::error::Elapsed> {
    tokio::time::timeout(Duration::from_secs(5), http.shutdown_and_wait()).await
}
fn a09_response_diagnostic(
    response: &Result<ssx_domain::AdapterResponse, tokio::time::error::Elapsed>,
) -> String {
    match response {
        Ok(response) => format!(
            "status={:?} message={:?} results={}",
            response.status,
            response.error_message,
            response.results.len()
        ),
        Err(error) => format!("response harness guard: {error}"),
    }
}
// The control request warms only the same default-profile owner/origin. It is
// separate from the source scenario and does not widen its configured deadline.
async fn a09_warm(http: &AdapterHttp, listener: &Arc<TcpListener>, label: &str) {
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{addr}/a09-warm");
    assert_loopback(&url);
    let warm_listener = Arc::clone(listener);
    let server_label = format!("{label}: warmup");
    let server = a09_spawn(async move {
        let (mut socket, _) = a09_stage(&server_label, warm_listener.accept()).await;
        let bytes = a09_headers(&mut socket, &server_label).await;
        let wire = String::from_utf8(bytes).unwrap();
        assert!(
            wire.starts_with("GET /a09-warm HTTP/1.1\r\n"),
            "{server_label}: {wire}"
        );
        a09_stage(
            &server_label,
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
        )
        .await;
        wire
    });
    let warm = tokio::time::timeout(Duration::from_secs(5), async {
        let mut session = http
            .session(
                Options::default(),
                (Instant::now() + Duration::from_millis(5000)).into(),
                Cancellation::default(),
            )
            .map_err(|error| format!("warm session: {error:?}"))?;
        session
            .execute(Request::get(&url).map_err(|error| format!("warm request: {error:?}"))?)
            .await
            .map_err(|error| format!("warm exchange: {error:?}"))
    })
    .await;
    let server_result = a09_join(server, &format!("{label}: warmup")).await;
    let retained = http.retained_pools();
    let ok = matches!(&warm, Ok(Ok(response)) if response.status == 200 && response.body.is_empty())
        && matches!(&server_result, Ok(wire) if wire.starts_with("GET /a09-warm HTTP/1.1\r\n"))
        && retained == 1;
    if !ok {
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
        let drain = a09_drain(http).await;
        panic!(
            "{label}: warmup failed: response={warm:?}; server={server_result:?}; pools={retained}; override names={overrides:?}; drain={drain:?}"
        );
    }
    eprintln!(
        "a09 warmup control case={label} actual_control_requests=1 scenario_requests=0 retained_pools={retained}"
    );
}
#[tokio::test]
async fn five_real_timeout_and_expired_context_cleanup() {
    if default_ca_environment::default_ca_test_environment(
        "five_real_timeout_and_expired_context_cleanup",
    ) {
        return;
    }
    for name in NAMES {
        let listener = Arc::new(TcpListener::bind("127.0.0.1:0").await.unwrap());
        let addr = listener.local_addr().unwrap();
        let (r, h) = wired(
            &format!("http://{addr}/slow"),
            30,
            Arc::new(AllowAllAdmission),
        );
        a09_warm(&h, &listener, name).await;
        let server = a09_spawn(async move {
            let (mut socket, _) = a09_stage(&format!("{name}: accept"), listener.accept()).await;
            let data = a09_headers(&mut socket, &format!("{name}: headers")).await;
            assert!(!data.is_empty());
            let mut b = [0; 1];
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), socket.read(&mut b))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
        });
        let result = a09_client(r.instantiate(name).unwrap().search(&SearchRequest {
            query: "test".into(),
            ..Default::default()
        }))
        .await;
        let server_result = a09_join(server, name).await;
        let expired = a09_client(r.instantiate(name).unwrap().search_with_context(
            &SearchRequest::default(),
            SearchContext {
                deadline: Instant::now() - Duration::from_secs(1),
            },
        ))
        .await;
        let drain_result = a09_drain(&h).await;
        eprintln!(
            "a09 fixture diagnostic case={name} configured_ms=30 response={} server={server_result:?} drain={drain_result:?}",
            a09_response_diagnostic(&result)
        );
        let result = result.expect("bounded configured timer response");
        assert_eq!(
            result.status,
            if name == "internetarchive" {
                EngineStatus::Error
            } else {
                EngineStatus::Timeout
            }
        );
        server_result.unwrap();
        let result = expired.expect("bounded already-expired context response");
        assert_eq!(result.status, EngineStatus::Timeout);
        drain_result.expect("bounded fixture HTTP shutdown");
    }
}
#[tokio::test]
async fn archive_real_cdx_timer_one_fallback_and_aggregate_context() {
    if default_ca_environment::default_ca_test_environment(
        "archive_real_cdx_timer_one_fallback_and_aggregate_context",
    ) {
        return;
    }
    use ssx_application::SearchEngine;
    for remaining in [true, false] {
        let listener = Arc::new(TcpListener::bind("127.0.0.1:0").await.unwrap());
        let addr = listener.local_addr().unwrap();
        let h = Arc::new(AdapterHttp::default());
        a09_warm(&h, &listener, &format!("archive remaining={remaining}")).await;
        let server = a09_spawn(async move {
            let (mut first, _) = a09_stage(
                &format!("archive remaining={remaining}: CDX accept"),
                listener.accept(),
            )
            .await;
            let data = a09_headers(
                &mut first,
                &format!("archive remaining={remaining}: CDX headers"),
            )
            .await;
            assert!(String::from_utf8_lossy(&data).starts_with("GET /cdx?url=example.com&output=json&limit=10&filter=statuscode:200&collapse=timestamp:6 "));
            let mut b = [0; 1];
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), first.read(&mut b))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
            if remaining {
                let (mut next, _) = a09_stage("archive fallback accept", listener.accept()).await;
                let data = a09_headers(&mut next, "archive fallback headers").await;
                assert!(
                    String::from_utf8_lossy(&data).starts_with("GET /available?url=example.com ")
                );
                let body = r#"{"archived_snapshots":{"closest":{"available":true,"status":"200","timestamp":"20240102123456","url":"http://web.archive.org/web/20240102123456/https://example.com/"}}}"#;
                a09_stage("archive fallback response", next.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                ))
                .await;
            }
            assert!(
                tokio::time::timeout(Duration::from_millis(50), listener.accept())
                    .await
                    .is_err()
            );
        });
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
            a09_client(engine.search(&q)).await
        } else {
            a09_client(engine.search_with_context(
                &q,
                SearchContext {
                    deadline: Instant::now() + Duration::from_millis(30),
                },
            ))
            .await
        };
        let label = format!("archive remaining={remaining}");
        let server_result = a09_join(server, &label).await;
        let drain_result = a09_drain(&h).await;
        eprintln!(
            "a09 fixture diagnostic case={label} configured_ms=200 context_ms={} response={} server={server_result:?} drain={drain_result:?}",
            if remaining { "none" } else { "30" },
            a09_response_diagnostic(&response)
        );
        let response = response.expect("bounded CDX/fallback response");
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
        server_result.unwrap();
        drain_result.expect("bounded fixture HTTP shutdown");
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
    if default_ca_environment::default_ca_test_environment(
        "actual_source_archive_configured_timers_keep_error_and_context_timeout_distinct",
    ) {
        return;
    }
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
        let case_id = case["id"].as_str().unwrap().to_owned();
        if millis > 0 {
            a09_warm(&http, &listener, &case_id).await;
        }
        let server = if millis > 0 {
            let listener = Arc::clone(&listener);
            let server_case = case_id.clone();
            Some(a09_spawn(async move {
                let (mut socket, _) =
                    a09_stage(&format!("{server_case}: accept"), listener.accept()).await;
                let raw = a09_headers(&mut socket, &format!("{server_case}: headers")).await;
                let wire = String::from_utf8(raw).unwrap();
                assert!(wire.starts_with(
                    "GET /advancedsearch.php?q=books&output=json&rows=10 HTTP/1.1\r\n"
                ));
                assert!(!wire.to_lowercase().contains("authorization:"));
                if !stall {
                    let body = r#"{"response":{"docs":[{"identifier":"book","title":"Book","downloads":5000}]}}"#;
                    a09_stage(&format!("{server_case}: immediate response"), socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes())).await;
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
        let response = a09_client(registry.instantiate("internetarchive").unwrap().search(
            &SearchRequest {
                query: "books".into(),
                ..Default::default()
            },
        ))
        .await;
        let server_result = match server {
            Some(server) => Some(a09_join(server, &case_id).await),
            None => None,
        };
        // Already-expired caller context remains independent of configured Error.
        let expired = a09_client(
            registry
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
                ),
        )
        .await;
        let drain_result = a09_drain(&http).await;
        eprintln!(
            "a09 fixture diagnostic case={case_id} configured_ms={millis} stall={stall} response={} server={server_result:?} drain={drain_result:?}",
            a09_response_diagnostic(&response)
        );
        let response = response.expect("bounded frozen source timer response");
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
        if let Some(server) = server_result {
            let wire = server.unwrap();
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
        let expired = expired.expect("bounded already-expired context response");
        assert_eq!(expired.status, EngineStatus::Timeout);
        drain_result.expect("bounded fixture HTTP shutdown");
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
