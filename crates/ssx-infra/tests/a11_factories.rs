//! Native F06 factories on private wire, with joined HTTP ownership and no providers.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchContext};
use ssx_domain::{AdapterResponse, EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{ThreatIntelSettings, register_threat_intelligence},
    http::AdapterHttp,
};
use ssx_registry::Registry;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
const NAMES: [&str; 4] = ["abuseipdb", "greynoise", "otx", "urlhaus"];
const CHILD: &str = "SSX_A11_PRIVATE_FACTORY_CHILD";
const ENV_KEYS: [&str; 11] = [
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
];
fn isolated(name: &str) -> bool {
    if let Ok(actual) = std::env::var(CHILD) {
        assert_eq!(actual, name);
        return false;
    }
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", name, "--nocapture"])
        .env(CHILD, name);
    for key in ENV_KEYS {
        command.env_remove(key);
    }
    let out = command
        .output()
        .expect("private factory child must execute");
    let stdout = String::from_utf8(out.stdout).unwrap();
    print!("{stdout}");
    eprint!("{}", String::from_utf8_lossy(&out.stderr));
    assert!(
        out.status.success(),
        "isolated factory child failed: {name}"
    );
    assert_eq!(
        stdout
            .lines()
            .filter(|s| *s == format!("A11_PRIVATE_CHILD_COMPLETE {name}"))
            .count(),
        1,
        "one actual joined child completion required"
    );
    assert!(stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"));
    true
}
fn endpoint(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "http"
            && matches!(u.host(),Some(url::Host::Ipv4(ip))if ip.is_loopback())
            && u.port().is_some_and(|p| p != 0)
            && u.username().is_empty()
            && u.password().is_none()
    })
}
fn guard(url: &str, env: &BTreeMap<String, String>) {
    assert!(
        endpoint(url),
        "endpoint refused before factory registration"
    );
    for key in ENV_KEYS {
        assert!(
            env.get(key).is_none_or(|v| v.is_empty()),
            "environment refused before network: {key}"
        );
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/usr/sbin/scutil")
            .arg("--proxy")
            .output()
            .expect("system proxy audit must execute");
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        for name in [
            "HTTPEnable",
            "HTTPSEnable",
            "SOCKSEnable",
            "ProxyAutoConfigEnable",
            "ProxyAutoDiscoveryEnable",
        ] {
            let values: Vec<i32> = text
                .lines()
                .filter_map(|l| {
                    let (k, v) = l.trim().split_once(':')?;
                    (k.trim() == name).then(|| v.trim().parse().expect("valid proxy flag"))
                })
                .collect();
            assert!(
                values.len() <= 1 && values.iter().all(|v| *v == 0),
                "system proxy refused"
            );
        }
    }
}
fn private(url: &str) {
    guard(url, &std::env::vars().collect());
}
#[derive(Clone, Copy)]
enum Mode {
    Allow,
    Deny,
    Unavailable,
    Pending,
}
struct Admission {
    mode: Mode,
    calls: Mutex<Vec<String>>,
}
impl SearchAdmission for Admission {
    fn admit<'a>(&'a self, n: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(n.into());
            match self.mode {
                Mode::Allow => AdmissionDecision::Allowed,
                Mode::Deny => AdmissionDecision::Denied,
                Mode::Unavailable => AdmissionDecision::Unavailable,
                Mode::Pending => std::future::pending().await,
            }
        })
    }
}
fn admission(mode: Mode) -> Arc<Admission> {
    Arc::new(Admission {
        mode,
        calls: Mutex::new(Vec::new()),
    })
}
fn resolved(base: &str, timeout: i64, key: bool) -> ThreatIntelSettings {
    resolved_key(base, timeout, key.then_some("fixture-only-key"))
}
fn resolved_key(base: &str, timeout: i64, key: Option<&str>) -> ThreatIntelSettings {
    private(base);
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let p = std::env::temp_dir().join(format!(
        "a11-private-config-{}-{}.yaml",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let yaml = format!(
        "engines:\n{}",
        NAMES
            .iter()
            .map(|n| format!("  {n}: {{}}\n"))
            .collect::<String>()
    );
    std::fs::write(&p, yaml).unwrap();
    let mut env = BTreeMap::new();
    for n in NAMES {
        let prefix = format!("ENGINE_{}", n.to_ascii_uppercase());
        env.insert(format!("{prefix}_BASE_URL"), base.into());
        env.insert(format!("{prefix}_TIMEOUT_MS"), timeout.to_string());
        env.insert(format!("{prefix}_MAX_RESULTS"), "10".into());
        if let Some(key) = key {
            env.insert(format!("{prefix}_API_KEY"), key.into());
        }
    }
    let cfg = ssx_config::load_config_with(Some(&p), &env).unwrap();
    std::fs::remove_file(p).unwrap();
    let settings = ThreatIntelSettings::from_resolved(&cfg);
    for c in [
        &settings.abuseipdb,
        &settings.greynoise,
        &settings.otx,
        &settings.urlhaus,
    ] {
        assert_eq!(c.base_url, base);
        assert_eq!(c.api_key.expose(), key);
        private(&c.base_url);
    }
    settings
}
fn wired(
    base: &str,
    timeout: i64,
    key: bool,
    a: Arc<dyn SearchAdmission>,
) -> (Registry, Arc<AdapterHttp>) {
    let mut r = Registry::builtins().unwrap();
    let h = Arc::new(AdapterHttp::default());
    register_threat_intelligence(&mut r, h.clone(), a, resolved(base, timeout, key)).unwrap();
    (r, h)
}
async fn read_request(s: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let (end, length) = loop {
        let mut part = [0; 4096];
        let n = s.read(&mut part).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&part[..n]);
        assert!(bytes.len() <= 65536, "bounded fixture request");
        if let Some(at) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
            let h = std::str::from_utf8(&bytes[..at]).unwrap();
            let len = h
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            break (at + 4, len);
        }
    };
    while bytes.len() < end + length {
        let mut part = [0; 4096];
        let n = s.read(&mut part).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&part[..n]);
        assert!(bytes.len() <= 65536);
    }
    assert_eq!(bytes.len(), end + length);
    String::from_utf8(bytes).unwrap()
}
fn complete(response: &AdapterResponse) -> ssx_json::Value {
    let results = response
        .results
        .iter()
        .map(|r| {
            let v = ssx_domain::storage::encode_result(r).unwrap();
            assert_eq!(ssx_domain::storage::decode_result(v.clone()).unwrap(), *r);
            v
        })
        .collect::<Vec<_>>();
    ssx_json::to_value(serde_json::json!({"results":results,"status":response.status,"error_message":response.error_message,"answers":response.answers,"infoboxes":response.infoboxes,"corrections":response.corrections,"synthetic":response.synthetic})).unwrap()
}
#[derive(Deserialize)]
struct Capture {
    records: Vec<Box<serde_json::value::RawValue>>,
}
fn source_cases() -> Vec<ssx_json::Value> {
    [
        include_str!("../../../docs/rust-rewrite/a11/source-boundaries-v2.json"),
        include_str!("../../../docs/rust-rewrite/a11/source-supplement-v1.json"),
    ]
    .into_iter()
    .flat_map(|source| {
        serde_json::from_str::<Capture>(source)
            .unwrap()
            .records
            .into_iter()
            .map(|r| ssx_json::from_str(r.get()).unwrap())
            .collect::<Vec<_>>()
    })
    .collect()
}
#[test]
fn guards_refuse_public_proxy_and_ca_before_registration() {
    let registered = std::sync::atomic::AtomicUsize::new(0);
    let sent = std::sync::atomic::AtomicUsize::new(0);
    let mut fixtures = vec![
        ("https://api.abuseipdb.com/api/v2", BTreeMap::new()),
        ("http://public.invalid:1234", BTreeMap::new()),
        ("http://127.0.0.1/", BTreeMap::new()),
        ("http://user:secret@127.0.0.1:1234/", BTreeMap::new()),
    ];
    for k in ENV_KEYS {
        fixtures.push((
            "http://127.0.0.1:1234/",
            BTreeMap::from([(k.into(), "synthetic-nondefault".into())]),
        ));
    }
    for (base, e) in fixtures {
        let result = std::panic::catch_unwind(|| {
            guard(base, &e);
            registered.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            sent.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        assert!(result.is_err());
        assert_eq!(registered.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(sent.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
    let cfg = ssx_config::load_config_with(
        None,
        &BTreeMap::from([(
            "ENGINE_OTX_BASE_URL".into(),
            "http://127.0.0.1:1/explicit-override".into(),
        )]),
    )
    .unwrap();
    assert_eq!(
        ThreatIntelSettings::from_resolved(&cfg).otx.base_url,
        "http://127.0.0.1:1/explicit-override"
    );
    let default = ssx_config::load_config_with(None, &BTreeMap::new()).unwrap();
    assert_eq!(
        ThreatIntelSettings::from_resolved(&default).otx.base_url,
        ssx_infra::adapters::otx::ENDPOINT
    );
    println!("A11_FACTORY_COMPLETE endpoint-proxy-ca-refusal");
}
#[tokio::test]
async fn four_f06_factories_preserve_full_original_outputs_and_actual_wire_headers() {
    const NAME: &str = "four_f06_factories_preserve_full_original_outputs_and_actual_wire_headers";
    if isolated(NAME) {
        return;
    }
    let cases = source_cases();
    let mut wires = Vec::new();
    for (name, variant, keyed) in [
        ("abuseipdb", "full-success", true),
        ("greynoise", "full-success", true),
        ("otx", "full-success", true),
        ("urlhaus", "full-success", true),
        ("greynoise", "missing-key", false),
        ("otx", "routing-hash32", true),
        ("otx", "routing-cve", true),
        ("otx", "routing-pulses", true),
        ("urlhaus", "routing-url", true),
        ("urlhaus", "routing-md5", true),
        ("urlhaus", "long-s-lower", true),
        ("urlhaus", "long-s-upper", true),
        ("urlhaus", "ascii-control", true),
    ] {
        let profile = if ["long-s-lower", "long-s-upper", "ascii-control"].contains(&variant) {
            "a11-supplement"
        } else {
            "a11-boundary"
        };
        let row = cases
            .iter()
            .find(|r| r["id"].as_str() == Some(format!("{profile}/{name}/{variant}").as_str()))
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}/base");
        private(&base);
        let source_response = row["exchanges"][0]["response"].clone();
        let server = tokio::spawn(async move {
            let (mut s, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let wire = read_request(&mut s).await;
            let body = STANDARD
                .decode(source_response["body_base64"].as_str().unwrap())
                .unwrap();
            s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.unwrap();
            s.write_all(&body).await.unwrap();
            wire
        });
        let a = admission(Mode::Allow);
        let (r, h) = wired(&base, 5000, keyed, a.clone());
        let e = r.instantiate(name).unwrap();
        let response = e
            .search(&SearchRequest {
                query: row["query"].as_str().unwrap().into(),
                page: ssx_domain::SearchPage::from_u64(7).expect("positive fixture page"),
                max_results: Some(1),
                ..Default::default()
            })
            .await;
        drop(e);
        h.shutdown_and_wait().await;
        let wire = server.await.unwrap();
        assert_eq!(
            complete(&response),
            row["output"],
            "all original public fields and actual storage consumer: {name}"
        );
        assert_eq!(*a.calls.lock().unwrap(), [name]);
        let (head, body) = wire.split_once("\r\n\r\n").unwrap();
        let mut lines = head.lines();
        let request_line = lines.next().unwrap();
        let headers: BTreeMap<_, _> = lines
            .map(|l| {
                let (k, v) = l.split_once(':').unwrap();
                (k.to_ascii_lowercase(), v.trim().to_owned())
            })
            .collect();
        let expected = &row["exchanges"][0]["request"];
        let source_url = url::Url::parse(expected["url"].as_str().unwrap()).unwrap();
        let prefix = match name {
            "abuseipdb" => "/api/v2",
            "otx" => "/api/v1",
            _ => "",
        };
        let suffix = source_url.path().strip_prefix(prefix).unwrap();
        let query = source_url
            .query()
            .map(|q| format!("?{q}"))
            .unwrap_or_default();
        assert_eq!(
            request_line,
            format!(
                "{} /base{suffix}{query} HTTP/1.1",
                expected["method"].as_str().unwrap()
            )
        );
        assert_eq!(
            STANDARD.encode(body),
            expected["body_base64"].as_str().unwrap()
        );
        assert_eq!(headers["host"], addr.to_string());
        assert_eq!(
            headers["user-agent"],
            "SlopSearX/0.1.0 (meta search engine; agent-native)"
        );
        for pair in expected["headers"].as_array().unwrap() {
            let k = pair[0].as_str().unwrap();
            if ![
                "host",
                "accept-encoding",
                "connection",
                "user-agent",
                "content-length",
            ]
            .contains(&k)
            {
                assert_eq!(
                    headers.get(k).map(String::as_str),
                    pair[1].as_str(),
                    "custom header {k}"
                );
            }
        }
        let standard = [
            "host",
            "accept-encoding",
            "connection",
            "user-agent",
            "content-length",
        ];
        let actual_custom: BTreeMap<_, _> = headers
            .iter()
            .filter(|(k, _)| !standard.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let expected_custom: BTreeMap<_, _> = expected["headers"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| !standard.contains(&p[0].as_str().unwrap()))
            .map(|p| {
                (
                    p[0].as_str().unwrap().to_owned(),
                    p[1].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(
            actual_custom, expected_custom,
            "complete custom wire header map"
        );
        if name == "greynoise" && variant == "missing-key" {
            for key in ["key", "x-api-key", "authorization"] {
                assert!(
                    !headers.contains_key(key),
                    "community must omit credential headers"
                );
            }
        }
        assert!(!wire.contains("api_key="));
        assert!(
            !response
                .error_message
                .as_deref()
                .unwrap_or("")
                .contains("fixture-only-key")
        );
        if name == "urlhaus" {
            assert_eq!(headers["content-length"], body.len().to_string());
        }
        wires.push(serde_json::json!({"engine":name,"source_id":row["id"],"actual_wire":wire,"complete_public_output":complete(&response)}));
        println!("A11_FACTORY_COMPLETE real-wire:{name}:{variant}");
    }
    if let Ok(path) = std::env::var("SSX_A11_WIRE_OUTPUT") {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(serde_json::to_string_pretty(&wires).unwrap().as_bytes())
            .unwrap();
    }
    println!("A11_PRIVATE_CHILD_COMPLETE {NAME}");
}
#[tokio::test]
async fn four_missing_credentials_and_admission_order_have_zero_http() {
    const NAME: &str = "four_missing_credentials_and_admission_order_have_zero_http";
    if isolated(NAME) {
        return;
    }
    for name in NAMES {
        // GreyNoise permits a missing key via the community route, proven on real wire above.
        let modes: &[Mode] = if name == "greynoise" {
            &[Mode::Deny, Mode::Unavailable, Mode::Pending]
        } else {
            &[Mode::Allow, Mode::Deny, Mode::Unavailable, Mode::Pending]
        };
        for &mode in modes {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}/blocked", listener.local_addr().unwrap());
            let a = admission(mode);
            let (r, h) = wired(&base, 5000, false, a.clone());
            let response = r
                .instantiate(name)
                .unwrap()
                .search_with_context(
                    &SearchRequest {
                        query: "8.8.8.8".into(),
                        ..Default::default()
                    },
                    SearchContext {
                        deadline: Instant::now() + Duration::from_millis(30),
                    },
                )
                .await;
            h.shutdown_and_wait().await;
            assert!(
                tokio::time::timeout(Duration::from_millis(5), listener.accept())
                    .await
                    .is_err()
            );
            assert!(response.results.is_empty());
            if name == "urlhaus" {
                assert_eq!(response.status, EngineStatus::Unavailable);
                assert!(a.calls.lock().unwrap().is_empty());
            } else {
                assert_eq!(*a.calls.lock().unwrap(), [name]);
                assert_eq!(
                    response.status,
                    match mode {
                        Mode::Allow | Mode::Unavailable => EngineStatus::Error,
                        Mode::Deny => EngineStatus::RateLimited,
                        Mode::Pending => EngineStatus::Timeout,
                    }
                );
            }
            println!(
                "A11_FACTORY_COMPLETE no-key-admission:{name}:{}",
                match mode {
                    Mode::Allow => "allow",
                    Mode::Deny => "deny",
                    Mode::Unavailable => "unavailable",
                    Mode::Pending => "pending",
                }
            );
        }
    }
    println!("A11_PRIVATE_CHILD_COMPLETE {NAME}");
}
#[tokio::test]
async fn four_cancellation_shutdown_and_real_deadline_join_pending_sockets() {
    const NAME: &str = "four_cancellation_shutdown_and_real_deadline_join_pending_sockets";
    if isolated(NAME) {
        return;
    }
    for name in NAMES {
        for mode in ["drop", "shutdown", "deadline"] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let base = format!("http://{addr}/pending");
            private(&base);
            let (tx, rx) = tokio::sync::oneshot::channel();
            let server = tokio::spawn(async move {
                let (mut s, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
                let _wire = read_request(&mut s).await;
                tx.send(()).unwrap();
                let mut b = [0; 1];
                assert_eq!(
                    tokio::time::timeout(Duration::from_secs(5), s.read(&mut b))
                        .await
                        .unwrap()
                        .unwrap(),
                    0
                );
            });
            let (r, h) = wired(&base, 5000, true, admission(Mode::Allow));
            let e = r.instantiate(name).unwrap();
            let q = SearchRequest {
                query: "8.8.8.8".into(),
                ..Default::default()
            };
            let mut future = e.search_with_context(
                &q,
                SearchContext {
                    deadline: Instant::now()
                        + Duration::from_millis(if mode == "deadline" { 500 } else { 4000 }),
                },
            );
            tokio::select! {r=rx=>r.unwrap(),_=&mut future=>panic!("engine returned before actual request observed: {name}/{mode}")};
            let result = match mode {
                "drop" => {
                    drop(future);
                    None
                }
                "shutdown" => {
                    h.shutdown();
                    Some(future.await)
                }
                _ => Some(future.await),
            };
            drop(e);
            h.shutdown_and_wait().await;
            server.await.unwrap();
            if let Some(response) = result {
                assert_eq!(
                    response.status,
                    if mode == "deadline" {
                        EngineStatus::Timeout
                    } else {
                        EngineStatus::Error
                    }
                );
                assert!(response.results.is_empty());
            }
            println!("A11_FACTORY_COMPLETE owned-socket:{name}:{mode}");
        }
    }
    println!("A11_PRIVATE_CHILD_COMPLETE {NAME}");
}
#[tokio::test]
async fn four_nonpositive_and_expired_deadlines_dispatch_nothing() {
    const NAME: &str = "four_nonpositive_and_expired_deadlines_dispatch_nothing";
    if isolated(NAME) {
        return;
    }
    for name in NAMES {
        for timeout in [0, -1] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}/no-send", listener.local_addr().unwrap());
            let a = admission(Mode::Allow);
            let (r, h) = wired(&base, timeout, true, a.clone());
            let response = r
                .instantiate(name)
                .unwrap()
                .search(&SearchRequest {
                    query: "8.8.8.8".into(),
                    ..Default::default()
                })
                .await;
            h.shutdown_and_wait().await;
            assert_eq!(response.status, EngineStatus::Timeout);
            assert!(response.results.is_empty());
            assert_eq!(*a.calls.lock().unwrap(), [name]);
            assert!(
                tokio::time::timeout(Duration::from_millis(5), listener.accept())
                    .await
                    .is_err()
            );
            println!("A11_FACTORY_COMPLETE deadline-no-dispatch:{name}:{timeout}");
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/expired", listener.local_addr().unwrap());
        let a = admission(Mode::Allow);
        let (r, h) = wired(&base, 5000, true, a.clone());
        let response = r
            .instantiate(name)
            .unwrap()
            .search_with_context(
                &SearchRequest {
                    query: "8.8.8.8".into(),
                    ..Default::default()
                },
                SearchContext {
                    deadline: Instant::now() - Duration::from_millis(1),
                },
            )
            .await;
        h.shutdown_and_wait().await;
        assert_eq!(response.status, EngineStatus::Timeout);
        assert!(a.calls.lock().unwrap().is_empty());
        assert!(
            tokio::time::timeout(Duration::from_millis(5), listener.accept())
                .await
                .is_err()
        );
        println!("A11_FACTORY_COMPLETE expired-no-dispatch:{name}");
    }
    println!("A11_PRIVATE_CHILD_COMPLETE {NAME}");
}
#[tokio::test]
async fn actual_registration_source_metadata_and_secret_debug_contracts() {
    const NAME: &str = "actual_registration_source_metadata_and_secret_debug_contracts";
    if isolated(NAME) {
        return;
    }
    let settings = resolved("http://127.0.0.1:1/metadata-only", 5000, true);
    let debug = format!("{settings:?}");
    assert!(!debug.contains("fixture-only-key"));
    assert!(debug.contains("[redacted]"));
    let mut registry = Registry::builtins().unwrap();
    let http = Arc::new(AdapterHttp::default());
    register_threat_intelligence(
        &mut registry,
        http.clone(),
        admission(Mode::Allow),
        settings,
    )
    .unwrap();
    let mut completed = Vec::new();
    for (name, source) in [
        (
            "abuseipdb",
            "tests/test_abuseipdb.py::TestAbuseIPDBAdapter::test_adapter_registered",
        ),
        (
            "greynoise",
            "tests/test_greynoise.py::TestGreyNoiseAdapter::test_adapter_registered",
        ),
        (
            "otx",
            "tests/test_otx.py::TestOTXAdapter::test_adapter_registered",
        ),
        (
            "urlhaus",
            "tests/test_urlhaus.py::TestURLhausAdapter::test_adapter_registered",
        ),
    ] {
        assert!(registry.is_available(name));
        assert!(registry.instantiate(name).is_some());
        assert_eq!(registry.descriptor(name).unwrap().engine_type, "api");
        completed.push(source);
    }
    for (name, source) in [
        (
            "greynoise",
            "tests/test_greynoise.py::TestGreyNoiseAdapter::test_adapter_categories",
        ),
        (
            "urlhaus",
            "tests/test_urlhaus.py::TestURLhausAdapter::test_adapter_categories",
        ),
    ] {
        assert_eq!(
            registry.descriptor(name).unwrap().categories,
            ["security", "threat-intel"]
        );
        completed.push(source);
    }
    http.shutdown_and_wait().await;
    for source in completed {
        println!("A11_SOURCE_TEST_COMPLETE {source}");
    }
    println!("A11_FACTORY_COMPLETE config-secret-debug-redacted");
    println!("A11_PRIVATE_CHILD_COMPLETE {NAME}");
}

#[test]
fn f06_rejects_original_raw_yaml_maxima_before_a11_factory_construction() {
    let source: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a11/source-config-entry-v1.json"
    ))
    .unwrap();
    let scope: ssx_json::Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a11/config-entry-differences-v1.json"
    ))
    .unwrap();
    let records = source["records"].as_array().unwrap();
    assert_eq!(records.len(), 4);
    assert_eq!(scope["entries"].as_array().unwrap().len(), 4);
    let mut observations = Vec::new();
    for (index, row) in records.iter().enumerate() {
        let id = row["id"].as_str().unwrap();
        assert_eq!(row["search_performed"].as_bool(), Some(false));
        assert_eq!(row["adapter_shutdown_joined"].as_bool(), Some(true));
        assert_eq!(row["loaded_max_results"], row["asdict_max_results"]);
        assert_eq!(row["loaded_max_results"], row["normalized_max_results"]);
        assert_eq!(
            row["loaded_max_results"],
            row["otx_object_config"]["max_results"]
        );
        let path = std::env::temp_dir().join(format!(
            "a11-f06-raw-maximum-{}-{index}.yaml",
            std::process::id()
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::Write::write_all(&mut file, row["yaml"].as_str().unwrap().as_bytes()).unwrap();
        drop(file);
        let resolved = ssx_config::load_config_with(Some(&path), &BTreeMap::new());
        std::fs::remove_file(&path).unwrap();
        let message = match resolved {
            Err(error) => error.to_string(),
            Ok(config) => {
                let _settings = ThreatIntelSettings::from_resolved(&config);
                panic!("untyped/out-of-range setting reached A11 settings construction: {id}")
            }
        };
        let binding = scope["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["identity"].as_str() == Some(id))
            .unwrap();
        assert_eq!(binding["source_entry_record"], *row);
        assert_eq!(
            binding["native_entry_error"].as_str(),
            Some(message.as_str())
        );
        assert!(!message.contains("fixture-only-key"));
        observations.push(ssx_json::json!({
            "id": id,
            "source_entry_record": row,
            "native_entry_error": message,
            "adapter_settings_constructed": false,
            "source_search_performed": false
        }));
        println!("A11_CONFIG_COMPLETE original-yaml-rejected-before-factory:{id}");
    }
    if let Ok(path) = std::env::var("SSX_A11_CONFIG_OUTPUT") {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        std::io::Write::write_all(
            &mut file,
            ssx_json::to_string(&observations).unwrap().as_bytes(),
        )
        .unwrap();
    }
}

#[tokio::test]
async fn four_f06_unicode_credentials_reject_before_http_with_full_source_refusal_bindings() {
    const NAME: &str =
        "four_f06_unicode_credentials_reject_before_http_with_full_source_refusal_bindings";
    if isolated(NAME) {
        return;
    }
    let captured: Capture = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a11/source-credential-ascii-v1.json"
    ))
    .unwrap();
    let original: Vec<ssx_json::Value> = captured
        .records
        .iter()
        .map(|record| ssx_json::from_str(record.get()).unwrap())
        .collect();
    assert_eq!(original.len(), NAMES.len());
    let bindings: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a11/credential-refusal-bindings-v1.json"
    ))
    .unwrap();
    assert_eq!(bindings["entries"].as_array().unwrap().len(), NAMES.len());
    let positive = source_cases();
    let mut observations = Vec::new();
    for name in NAMES {
        let id = format!("a11-credential/{name}/unicode-key-ascii-refusal");
        let source = original
            .iter()
            .find(|record| record["id"].as_str() == Some(&id))
            .unwrap();
        assert_eq!(source["config"]["api_key"].as_str(), Some("clé"));
        assert_eq!(source["output"]["status"].as_str(), Some("error"));
        assert_eq!(source["exchanges"].as_array().unwrap().len(), 0);
        assert_eq!(
            source["admission_calls"],
            ssx_json::to_value(vec![name]).unwrap()
        );
        assert_eq!(
            source["header_normalizer_observations"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            source["header_normalizer_observations"][0]["class"].as_str(),
            Some("UnicodeEncodeError")
        );
        assert_eq!(
            source["header_normalizer_observations"][0]["encoding"].as_str(),
            Some("ascii")
        );
        assert_eq!(source["adapter_shutdown_joined"].as_bool(), Some(true));
        assert_eq!(source["transport_close_joined"].as_bool(), Some(true));
        let response_fixture = positive
            .iter()
            .find(|record| {
                record["id"].as_str() == Some(format!("a11-boundary/{name}/full-success").as_str())
            })
            .unwrap()["exchanges"][0]["response"]
            .clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/credential", listener.local_addr().unwrap());
        let settings = resolved_key(&base, 5000, Some("clé"));
        assert!(!format!("{settings:?}").contains("clé"));
        let a = admission(Mode::Allow);
        let h = Arc::new(AdapterHttp::default());
        let mut registry = Registry::builtins().unwrap();
        register_threat_intelligence(&mut registry, h.clone(), a.clone(), settings).unwrap();
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = accepted.clone();
        let server = tokio::spawn(async move {
            let connection = tokio::select! {
                value = listener.accept() => Some(value.unwrap()),
                _ = stopped => None,
            };
            if let Some((mut socket, _)) = connection {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let wire = read_request(&mut socket).await;
                let body = STANDARD
                    .decode(response_fixture["body_base64"].as_str().unwrap())
                    .unwrap();
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await.unwrap();
                socket.write_all(&body).await.unwrap();
                Some(wire)
            } else {
                None
            }
        });
        let engine = registry.instantiate(name).unwrap();
        let response = engine
            .search(&SearchRequest {
                query: source["query"].as_str().unwrap().into(),
                ..Default::default()
            })
            .await;
        drop(engine);
        h.shutdown_and_wait().await;
        let _ = stop.send(());
        let wire = server.await.unwrap();
        observations.push(serde_json::json!({
            "id": id,
            "engine": name,
            "source_record": source,
            "native_output": complete(&response),
            "actual_wire": wire,
            "accepted_connections": accepted.load(std::sync::atomic::Ordering::SeqCst),
            "admission_calls": a.calls.lock().unwrap().clone(),
            "F06_unicode_credential_preserved": true,
            "settings_debug_redacted": true,
            "http_shutdown_joined": true,
            "server_joined": true,
        }));
    }
    // Preserve all four actual observations before any expected-red semantic assertion.
    if let Ok(path) = std::env::var("SSX_A11_CREDENTIAL_OUTPUT") {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(&serde_json::to_vec_pretty(&observations).unwrap())
            .unwrap();
    }
    for record in &observations {
        let bound = bindings["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == record["id"])
            .unwrap();
        assert_eq!(record["source_record"], bound["source_record"]);
        assert_eq!(record["native_output"], bound["native_output"]);
        assert_eq!(
            record["accepted_connections"], 0,
            "Unicode credential crossed the actual native HTTP boundary"
        );
        assert!(record["actual_wire"].is_null());
        let mut expected = record["source_record"]["output"].clone();
        expected["error_message"] =
            serde_json::json!("invalid intelligence credential configuration");
        assert_eq!(
            record["native_output"], expected,
            "full public refusal differs beyond the safe constant diagnostic"
        );
        assert_eq!(
            record["admission_calls"],
            serde_json::json!([record["engine"]])
        );
        assert!(
            !record["native_output"]["error_message"]
                .as_str()
                .unwrap()
                .contains("clé")
        );
        println!("A11_CREDENTIAL_COMPLETE {}", record["id"].as_str().unwrap());
    }
    println!("A11_PRIVATE_CHILD_COMPLETE {NAME}");
}
