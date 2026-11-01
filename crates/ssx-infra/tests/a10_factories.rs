//! Actual A10 factories, private native HTTP wire, consumer and caller lifecycle.
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchContext};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{AllowAllAdmission, VulnerabilitySettings, register_vulnerability_with_parser},
    html::ParserPool,
    http::AdapterHttp,
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
const NAMES: [&str; 5] = ["cve", "nvd", "epss", "exploitdb", "mitreattack"];
const QUERY: &str = "T1059 CVE-2024-12345";
const BODIES: [&str; 5] = [
    r#"{"containers":{"cna":{"descriptions":[{"lang":"en","value":"CVE fixture"}],"datePublic":"2025-01-01"}}}"#,
    r#"{"vulnerabilities":[{"cve":{"id":"CVE-2024-12345","descriptions":[{"lang":"en","value":"NVD fixture"}]}}]}"#,
    r#"{"data":[{"epss":"0.5","percentile":"0.9","date":"2025-01-01"}]}"#,
    r#"{"data":[{"id":"7","description":["7","Exploit fixture"]}],"recordsTotal":1,"recordsFiltered":1}"#,
    "<html><body><h1>MITRE fixture</h1><div class=description><p>native parsed</p></div></body></html>",
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
#[cfg(target_os = "macos")]
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
                "A10_SYSTEM_PROXY_FLAGS {}",
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
    for key in [
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "REQUESTS_CA_BUNDLE",
        "CURL_CA_BUNDLE",
        "SSLKEYLOGFILE",
    ] {
        assert!(
            env.get(key).is_none_or(|v| v.is_empty()),
            "nondefault certificate environment denied: {key}"
        );
    }
    audit_system_proxy();
}
fn assert_loopback(url: &str) {
    assert_loopback_environment(url, &std::env::vars().collect());
}

fn helper() -> Arc<ParserPool> {
    let exe = std::env::current_exe().unwrap();
    Arc::new(
        ParserPool::new(
            exe.parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("ssx-html-parser"),
            std::num::NonZeroUsize::new(2).unwrap(),
        )
        .unwrap(),
    )
}
fn resolved(base: &str, timeout: i64) -> VulnerabilitySettings {
    assert_loopback(base);
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
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "a10-factory-{}-{}.yaml",
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
    let settings = VulnerabilitySettings::from_resolved(&cfg);
    for url in [
        &settings.cve.base_url,
        &settings.nvd.settings.base_url,
        &settings.epss.base_url,
        &settings.exploitdb.settings.base_url,
        &settings.mitreattack.settings.base_url,
    ] {
        assert_eq!(url, base);
        assert_loopback(url);
    }
    assert!(settings.nvd.raw_max_results.is_none());
    assert!(settings.exploitdb.raw_max_results.is_none());
    settings
}
fn wired(
    base: &str,
    timeout: i64,
    admission: Arc<dyn SearchAdmission>,
) -> (Registry, Arc<AdapterHttp>, Arc<ParserPool>) {
    let mut registry = Registry::builtins().unwrap();
    let http = Arc::new(AdapterHttp::default());
    let parser = helper();
    register_vulnerability_with_parser(
        &mut registry,
        http.clone(),
        admission,
        resolved(base, timeout),
        parser.clone(),
    )
    .unwrap();
    (registry, http, parser)
}
async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut bytes = vec![];
    let mut part = [0; 4096];
    loop {
        let n = tokio::time::timeout(Duration::from_secs(2), socket.read(&mut part))
            .await
            .unwrap()
            .unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&part[..n]);
        if bytes.windows(4).any(|v| v == b"\r\n\r\n") {
            break;
        }
        assert!(bytes.len() < 16384);
    }
    String::from_utf8(bytes).unwrap()
}
#[test]
fn private_endpoint_proxy_and_certificate_guards_precede_dispatch() {
    let mut registrations = 0;
    for (base, env) in [
        ("https://public.invalid/", BTreeMap::new()),
        ("http://localhost:1234/", BTreeMap::new()),
        ("http://user:secret@127.0.0.1:1234/", BTreeMap::new()),
        (
            "http://127.0.0.1:1234/",
            BTreeMap::from([("HTTPS_PROXY".into(), "http://public.invalid/".into())]),
        ),
        (
            "http://127.0.0.1:1234/",
            BTreeMap::from([("SSL_CERT_FILE".into(), "/private/synthetic.pem".into())]),
        ),
    ] {
        let result = std::panic::catch_unwind(|| assert_loopback_environment(base, &env));
        if result.is_ok() {
            registrations += 1;
        }
        assert!(result.is_err());
    }
    assert_eq!(registrations, 0);
    println!("A10_GUARD_COMPLETE endpoint-proxy-cert-negative-controls");
}
#[tokio::test]
async fn five_resolved_factories_real_wire_headers_full_fields_and_consumer() {
    if isolated_child("five_resolved_factories_real_wire_headers_full_fields_and_consumer") {
        return;
    }

    assert_loopback("http://127.0.0.1:1/");
    let mut observations = vec![];
    for (name, body) in NAMES.into_iter().zip(BODIES) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let wire = read_request(&mut socket).await;
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",if name=="mitreattack"{"text/html"}else{"application/json"},body.len()).as_bytes()).await.unwrap();
            wire
        });
        let (registry, http, parser) = wired(
            &format!("http://{addr}/base"),
            5000,
            Arc::new(AllowAllAdmission),
        );
        for engine in NAMES {
            assert!(registry.is_available(engine));
            assert!(
                !registry
                    .descriptor(engine)
                    .unwrap()
                    .categories
                    .contains(&"general".into())
            );
        }
        let descriptor = registry.descriptor(name).unwrap();
        assert!(descriptor.categories.contains(&"security".into()));
        if name == "cve" {
            assert!(descriptor.categories.contains(&"it".into()));
        }
        if name == "mitreattack" {
            assert!(descriptor.categories.contains(&"reference".into()));
        }
        let response = registry
            .instantiate(name)
            .unwrap()
            .search(&SearchRequest {
                query: QUERY.into(),
                page: ssx_domain::SearchPage::from_u64(9).expect("positive fixture page"),
                max_results: Some(1),
                language: "fr".into(),
                ..Default::default()
            })
            .await;
        let wire = server.await.unwrap();
        assert_eq!(
            response.status,
            EngineStatus::Ok,
            "{name}: {:?}",
            response.error_message
        );
        assert_eq!(response.results.len(), 1, "{name}");
        let result = &response.results[0];
        assert_eq!(result.engine, name);
        assert_eq!(result.position, 1);
        assert!(result.media.is_none());
        assert!(result.thumbnail.is_none());
        assert!(result.img_src.is_none());
        let stored = ssx_domain::storage::encode_result(result).unwrap();
        assert_eq!(
            ssx_domain::storage::decode_result(stored.clone()).unwrap(),
            *result,
            "actual canonical consumer"
        );
        let path = match name {
            "cve" => "/base/cve/CVE-2024-12345",
            "nvd" => "/base?resultsPerPage=10&cveIds=CVE-2024-12345",
            "epss" => "/base?cve=CVE-2024-12345",
            "exploitdb" => {
                "/base/search?draw=1&start=0&length=10&order%5B0%5D%5Bcolumn%5D=0&order%5B0%5D%5Bdir%5D=desc&columns%5B0%5D%5Bdata%5D=date_published&columns%5B0%5D%5Bname%5D=date_published&q=T1059+CVE-2024-12345"
            }
            _ => "/base/techniques/T1059/",
        };
        assert!(
            wire.starts_with(&format!("GET {path} HTTP/1.1\r\n")),
            "{name}: {wire}"
        );
        let headers: BTreeMap<_, _> = wire
            .lines()
            .skip(1)
            .filter_map(|s| s.split_once(':'))
            .map(|(n, v)| (n.to_ascii_lowercase(), v.trim().to_owned()))
            .collect();
        let ua = if matches!(name, "exploitdb" | "mitreattack") {
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
        } else {
            concat!(
                "SlopSearX/",
                env!("CARGO_PKG_VERSION"),
                " (meta search engine; agent-native)"
            )
        };
        assert_eq!(headers.get("user-agent").map(String::as_str), Some(ua));
        assert_eq!(headers.get("host").unwrap(), &addr.to_string());
        assert_eq!(
            headers.get("accept-encoding").map(String::as_str),
            Some("gzip, deflate")
        );
        assert!(
            !headers.contains_key("connection"),
            "native HTTP/1.1 keep-alive uses default persistence"
        );
        assert!(!headers.contains_key("authorization"));
        assert!(!headers.contains_key("apikey"));
        let expected_names: Vec<_> = if name == "exploitdb" {
            vec![
                "accept",
                "accept-encoding",
                "accept-language",
                "host",
                "user-agent",
                "x-requested-with",
            ]
        } else {
            vec!["accept", "accept-encoding", "host", "user-agent"]
        };
        assert_eq!(
            headers.keys().map(String::as_str).collect::<Vec<_>>(),
            expected_names,
            "complete real wire header set"
        );
        observations.push(ssx_json::json!({"engine":name,"wire":wire,"stored_result":stored}));
        http.shutdown_and_wait().await;
        parser.shutdown().await.unwrap();
        println!("A10_FACTORY_COMPLETE {name}");
    }
    if let Ok(path) = std::env::var("SSX_A10_FACTORY_WIRE") {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        f.write_all(ssx_json::to_string(&observations).unwrap().as_bytes())
            .unwrap();
    }
    println!(
        "A10_ISOLATED_CHILD_COMPLETE five_resolved_factories_real_wire_headers_full_fields_and_consumer"
    );
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
async fn all_five_closed_admission_and_context_have_zero_http() {
    if isolated_child("all_five_closed_admission_and_context_have_zero_http") {
        return;
    }

    assert_loopback("http://127.0.0.1:1/");
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
            let (registry, http, parser) = wired(
                &format!("http://{}/denied", listener.local_addr().unwrap()),
                5000,
                admission,
            );
            let response = registry
                .instantiate(name)
                .unwrap()
                .search_with_context(
                    &SearchRequest {
                        query: QUERY.into(),
                        ..Default::default()
                    },
                    SearchContext {
                        deadline: Instant::now() + Duration::from_millis(20),
                    },
                )
                .await;
            assert_eq!(response.status, status, "{name}");
            assert!(response.results.is_empty());
            assert!(
                tokio::time::timeout(Duration::from_millis(5), listener.accept())
                    .await
                    .is_err()
            );
            http.shutdown_and_wait().await;
            parser.shutdown().await.unwrap();
        }
        println!("A10_ADMISSION_COMPLETE {name}");
    }
    println!("A10_ISOLATED_CHILD_COMPLETE all_five_closed_admission_and_context_have_zero_http");
}
#[tokio::test]
async fn all_five_real_pending_requests_cancel_timeout_and_join() {
    if isolated_child("all_five_real_pending_requests_cancel_timeout_and_join") {
        return;
    }

    assert_loopback("http://127.0.0.1:1/");
    for name in NAMES {
        for mode in ["drop", "shutdown", "timeout"] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let (tx, rx) = tokio::sync::oneshot::channel();
            let server = tokio::spawn(async move {
                let (mut socket, _) =
                    tokio::time::timeout(Duration::from_secs(2), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let wire = read_request(&mut socket).await;
                assert!(!wire.is_empty());
                tx.send(()).unwrap();
                let mut byte = [0; 1];
                assert_eq!(
                    tokio::time::timeout(Duration::from_secs(2), socket.read(&mut byte))
                        .await
                        .unwrap()
                        .unwrap(),
                    0,
                    "pending native socket must close"
                );
            });
            let (registry, http, parser) = wired(
                &format!("http://{addr}/pending"),
                if mode == "timeout" { 100 } else { 5000 },
                Arc::new(AllowAllAdmission),
            );
            let engine = registry.instantiate(name).unwrap();
            let request = SearchRequest {
                query: QUERY.into(),
                ..Default::default()
            };
            let mut future = engine.search(&request);
            tokio::select! {_=rx=>{},_=&mut future=>panic!("request completed before real socket observation")};
            if mode == "drop" {
                drop(future);
            } else {
                if mode == "shutdown" {
                    http.shutdown();
                }
                let response = future.await;
                assert_eq!(
                    response.status,
                    if mode == "timeout" {
                        EngineStatus::Timeout
                    } else {
                        EngineStatus::Error
                    }
                );
                assert!(response.results.is_empty());
            }
            server.await.unwrap();
            let response = engine
                .search_with_context(
                    &request,
                    SearchContext {
                        deadline: Instant::now() - Duration::from_secs(1),
                    },
                )
                .await;
            assert_eq!(response.status, EngineStatus::Timeout);
            http.shutdown_and_wait().await;
            parser.shutdown().await.unwrap();
            println!("A10_REQUEST_LIFECYCLE_COMPLETE {name}:{mode}");
        }
    }
    println!("A10_ISOLATED_CHILD_COMPLETE all_five_real_pending_requests_cancel_timeout_and_join");
}

/// A test child clears only the intentional private fixture's proxy/CA settings.
/// Production CA/proxy behavior and the parent process environment stay untouched.
fn isolated_child(name: &str) -> bool {
    const FLAG: &str = "SSX_A10_ISOLATED_FACTORY_TEST";
    if let Ok(value) = std::env::var(FLAG) {
        assert_eq!(value, name, "unexpected isolated child identity");
        return false;
    }
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", name, "--nocapture"])
        .env(FLAG, name);
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "REQUESTS_CA_BUNDLE",
        "CURL_CA_BUNDLE",
        "SSLKEYLOGFILE",
    ] {
        command.env_remove(key);
    }
    let output = command
        .output()
        .expect("isolated native factory child must execute");
    let stdout = String::from_utf8(output.stdout).unwrap();
    print!("{stdout}");
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.status.success(), "isolated child failed: {name}");
    let marker = format!("A10_ISOLATED_CHILD_COMPLETE {name}");
    assert_eq!(
        stdout.lines().filter(|s| *s == marker).count(),
        1,
        "missing/duplicate actual child completion"
    );
    true
}

#[tokio::test]
async fn f06_yaml_and_environment_text_max_is_classified_before_http() {
    const NAME: &str = "f06_yaml_and_environment_text_max_is_classified_before_http";
    if isolated_child(NAME) {
        return;
    }
    assert_loopback("http://127.0.0.1:1/");
    for from_env in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/text-max", listener.local_addr().unwrap());
        assert_loopback(&base);
        let path = std::env::temp_dir().join(format!(
            "a10-text-max-{}-{from_env}.yaml",
            std::process::id()
        ));
        let yaml = format!(
            "engines:\n  nvd:\n    base_url: {base}\n    max_results: {}\n",
            "file-text"
        );
        std::fs::write(&path, yaml).unwrap();
        let env = if from_env {
            BTreeMap::from([("ENGINE_NVD_MAX_RESULTS".into(), "next-text".into())])
        } else {
            BTreeMap::new()
        };
        let config = ssx_config::load_config_with(Some(&path), &env).unwrap();
        std::fs::remove_file(path).unwrap();
        let expected = if from_env { "next-text" } else { "file-text" };
        assert_eq!(
            config.engines["nvd"].max_results,
            ssx_config::ConfigScalar::Text(expected.into())
        );
        let settings = VulnerabilitySettings::from_resolved(&config);
        assert_eq!(
            settings.nvd.settings.max_results,
            ssx_config::ConfigScalar::Text(expected.into())
        );
        assert!(settings.nvd.raw_max_results.is_none());
        let mut registry = Registry::builtins().unwrap();
        let http = Arc::new(AdapterHttp::default());
        let parser = helper();
        register_vulnerability_with_parser(
            &mut registry,
            http.clone(),
            Arc::new(AllowAllAdmission),
            settings,
            parser.clone(),
        )
        .unwrap();
        let response = registry
            .instantiate("nvd")
            .unwrap()
            .search(&SearchRequest {
                query: QUERY.into(),
                ..Default::default()
            })
            .await;
        assert_eq!(response.status, EngineStatus::Error);
        assert!(response.results.is_empty());
        assert_eq!(
            response.error_message.as_deref(),
            Some("'<' not supported between instances of 'int' and 'str'")
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(10), listener.accept())
                .await
                .is_err()
        );
        http.shutdown_and_wait().await;
        parser.shutdown().await.unwrap();
        println!(
            "A10_TYPED_TEXT_MAX_COMPLETE {}",
            if from_env { "env" } else { "yaml" }
        );
    }
    let path = std::env::temp_dir().join(format!("a10-integer-max-{}.yaml", std::process::id()));
    std::fs::write(&path, "engines:\n  nvd:\n    max_results: 10\n").unwrap();
    let rejected = ssx_config::load_config_with(
        Some(&path),
        &BTreeMap::from([("ENGINE_NVD_MAX_RESULTS".into(), "next-text".into())]),
    );
    std::fs::remove_file(path).unwrap();
    assert!(
        rejected.is_err_and(|error| error.to_string().contains("invalid integer engine setting"))
    );
    println!("A10_TYPED_TEXT_MAX_COMPLETE integer-seeded-env-rejected");
    println!("A10_ISOLATED_CHILD_COMPLETE {NAME}");
}
