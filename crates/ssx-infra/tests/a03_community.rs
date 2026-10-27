use serde_json::Value;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::valkey::Valkey;
use ssx_infra::{
    adapters::{AllowAllAdmission, CommunityConfig, CommunitySettings, register_community},
    http::AdapterHttp,
};
use ssx_registry::Registry;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use std::{
    fs,
    io::Read as _,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Instant,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
const CASES: &str = include_str!("../../../docs/rust-rewrite/a03/cases.json");
struct Denied;
impl SearchAdmission for Denied {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async { AdmissionDecision::Denied })
    }
}
fn settings<'a>(config: &'a mut CommunityConfig, engine: &str) -> &'a mut CommunitySettings {
    match engine {
        "github" => &mut config.github.0,
        "huggingface" => &mut config.huggingface.0,
        "stackexchange" => &mut config.stackexchange.0,
        "hackernews" => &mut config.hackernews.0,
        _ => panic!("unknown engine"),
    }
}
fn request(case: &Value) -> SearchRequest {
    SearchRequest {
        query: case["query"].as_str().unwrap().into(),
        categories: Some(serde_json::from_value(case["params"]["categories"].clone()).unwrap()),
        page: 7,
        language: "fr".into(),
        safesearch: 2,
        time_range: Some("week".into()),
        media_type: Some("image".into()),
        max_results: Some(1),
        ..SearchRequest::default()
    }
}
async fn read_head(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let mut block = [0; 1024];
        let n = stream.read(&mut block).await.unwrap();
        if n == 0 {
            assert!(bytes.is_empty(), "truncated nonempty request");
            return bytes;
        }
        bytes.extend_from_slice(&block[..n]);
        assert!(bytes.len() <= 16384);
        if bytes.ends_with(b"\r\n\r\n") {
            return bytes;
        }
    }
}
fn relocated(base: &str, address: std::net::SocketAddr) -> String {
    if url::Url::parse(base).is_err() {
        return base.into();
    }
    let rest = base.split_once("://").unwrap().1;
    let suffix = rest.find(['/', '?', '#']).map(|i| &rest[i..]).unwrap_or("");
    format!("http://{address}{suffix}")
}
fn head_parts(head: &str) -> (&str, BTreeMap<String, String>) {
    let target = head
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap();
    let headers = head
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.to_lowercase(), v.trim_start().into()))
        .collect();
    (target, headers)
}

#[tokio::test]
async fn source_232_observations_replay_over_actual_f08_requests() {
    let corpus: Value = serde_json::from_str(CASES).unwrap();
    assert_eq!(corpus["cases"].as_array().unwrap().len(), 232);
    let mut executed = 0;
    for case in corpus["cases"].as_array().unwrap() {
        let name = case["case"].as_str().unwrap();
        let disposition = case["native_disposition"].as_str().unwrap();
        if disposition == "typed_config_bound_refusal"
            || disposition == "caller_cancelled_no_response"
            || disposition == "real_zero_deadline_not_mock_timing"
        {
            continue;
        }
        let engine = case["engine"].as_str().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let observed = captured.clone();
        let status = case["upstream"]["status"].as_u64().unwrap();
        let body = case["upstream"]["body"].as_str().unwrap().to_owned();
        let fault = case["upstream"]["fault"].as_str().unwrap_or("").to_owned();
        let server = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let head = read_head(&mut stream).await;
                if head.is_empty() {
                    continue;
                } // SDK/transport connection priming is not an HTTP request.
                observed
                    .lock()
                    .unwrap()
                    .push(String::from_utf8(head).unwrap());
                if fault == "timeout" {
                    std::future::pending::<()>().await;
                }
                if fault == "connect" {
                    return;
                }
                let headers = format!(
                    "HTTP/1.1 {status} Fixture\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(headers.as_bytes()).await.unwrap();
                stream.write_all(body.as_bytes()).await.unwrap();
                return;
            }
        });
        let mut config = CommunityConfig::default();
        let cfg = settings(&mut config, engine);
        if let Some(v) = case["config"].get("base_url") {
            cfg.base_url = v.as_str().unwrap().into();
        }
        cfg.base_url = relocated(&cfg.base_url, address);
        if let Some(v) = case["config"].get("max_results") {
            cfg.max_results = usize::try_from(v.as_u64().unwrap()).unwrap();
        }
        if let Some(v) = case["config"]
            .get("api_key")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
        {
            cfg.api_key = Some(ssx_domain::oauth::Secret::new(v.into()).unwrap());
        }
        cfg.timeout_ms = if case["upstream"]["fault"] == "timeout" {
            100.0
        } else {
            2000.0
        };
        let http = Arc::new(AdapterHttp::default());
        let admission: Arc<dyn SearchAdmission> = if case["admission_denied"] == true {
            Arc::new(Denied)
        } else {
            Arc::new(AllowAllAdmission)
        };
        let mut registry = Registry::builtins().unwrap();
        register_community(&mut registry, http.clone(), admission, config).unwrap();
        let adapter = registry.instantiate(engine).unwrap();
        let actual = tokio::time::timeout(Duration::from_secs(4), adapter.search(&request(case)))
            .await
            .unwrap();
        let actual_status = serde_json::to_value(&actual.status).unwrap();
        let invalid_header = name.ends_with(":invalid-header-secret");
        let expected_status = if invalid_header
            || !case["source_exception"].is_null()
            || disposition == "typed_result_or_finite_bound_refusal"
        {
            Value::String("error".into())
        } else {
            case["response"]["status"].clone()
        };
        assert_eq!(actual_status, expected_status, "{name}: status");
        if actual.status == EngineStatus::Ok {
            let results = Value::Array(
                actual
                    .results
                    .iter()
                    .map(|result| {
                        let encoded = ssx_domain::storage::encode_result(result).unwrap();
                        let value: Value =
                            serde_json::from_str(&ssx_json::to_string(&encoded).unwrap()).unwrap();
                        value
                    })
                    .collect(),
            );
            assert_eq!(results, case["response"]["results"], "{name}: results");
        } else {
            assert!(actual.results.is_empty(), "{name}: partial results escaped");
        }
        let expected_requests = case["requests"].as_array().unwrap();
        let heads = captured.lock().unwrap().clone();
        let invalid_base =
            name.ends_with(":empty-base-override") || name.ends_with(":malformed-base");
        assert_eq!(
            heads.len(),
            if invalid_base || invalid_header {
                0
            } else {
                expected_requests.len()
            },
            "{name}: raw request count"
        );
        for (head, expected) in heads.iter().zip(expected_requests) {
            let (target, headers) = head_parts(head);
            assert!(head.starts_with("GET "), "{name}");
            assert_eq!(
                target,
                expected["raw_target"].as_str().unwrap(),
                "{name}: target"
            );
            for key in ["authorization", "x-api-key"] {
                assert_eq!(
                    headers.get(key).map(String::as_str),
                    expected["headers"].get(key).and_then(Value::as_str),
                    "{name}: {key}"
                );
            }
            if engine != "stackexchange" {
                assert_eq!(
                    headers["user-agent"],
                    expected["headers"]["user-agent"].as_str().unwrap(),
                    "{name}: user agent"
                );
            }
            if engine == "github" {
                assert_eq!(
                    headers["accept"],
                    expected["headers"]["accept"].as_str().unwrap(),
                    "{name}: accept"
                );
            }
        }
        if let Some(message) = actual.error_message {
            assert!(
                !message.contains("fixture-token") && !message.contains("fixture-se-key"),
                "{name}: credential leak"
            );
        }
        server.abort();
        let _ = server.await;
        http.shutdown_and_wait().await;
        executed += 1;
    }
    assert_eq!(executed, 196, "exact replay inventory");
}

#[tokio::test]
async fn typed_config_bounds_zero_deadline_and_secret_redaction_before_dispatch() {
    for engine in ["github", "huggingface", "stackexchange", "hackernews"] {
        for timeout in [-1.0, f64::NAN, f64::INFINITY, 1e300, 0.0] {
            let mut config = CommunityConfig::default();
            let cfg = settings(&mut config, engine);
            cfg.base_url = "http://127.0.0.1:1".into();
            cfg.timeout_ms = timeout;
            cfg.api_key = Some(ssx_domain::oauth::Secret::new("secret-fixture".into()).unwrap());
            assert!(!format!("{config:?}").contains("secret-fixture"));
            let http = Arc::new(AdapterHttp::default());
            let mut registry = Registry::builtins().unwrap();
            register_community(
                &mut registry,
                http.clone(),
                Arc::new(AllowAllAdmission),
                config,
            )
            .unwrap();
            let response = registry
                .instantiate(engine)
                .unwrap()
                .search(&SearchRequest::default())
                .await;
            assert_eq!(
                response.status,
                if timeout == 0.0 && engine != "stackexchange" {
                    EngineStatus::Timeout
                } else {
                    EngineStatus::Error
                }
            );
            assert!(response.results.is_empty());
            http.shutdown_and_wait().await;
        }
    }
    assert!(ssx_domain::oauth::Secret::new("x".repeat(4097)).is_err());
}

#[tokio::test]
async fn shared_registry_preserves_pilots_and_caller_transport_after_adapter_drop() {
    use ssx_infra::adapters::{PilotConfig, register_pilots};
    let http = Arc::new(AdapterHttp::default());
    // Preserve this batch's original 51-engine parity target.
    let mut registry = Registry::migration_baseline().unwrap();
    let descriptors = registry.descriptors().cloned().collect::<Vec<_>>();
    register_pilots(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        PilotConfig::default(),
    )
    .unwrap();
    register_community(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        CommunityConfig::default(),
    )
    .unwrap();
    assert_eq!(
        registry.descriptors().cloned().collect::<Vec<_>>(),
        descriptors
    );
    assert_eq!(registry.len(), 51);
    for engine in [
        "npm",
        "crates",
        "github",
        "huggingface",
        "stackexchange",
        "hackernews",
    ] {
        assert!(registry.is_available(engine));
        drop(registry.instantiate(engine).unwrap());
    }
    assert!(!registry.is_available("arxiv"));
    // Factory/adapter destruction never closes the borrowed HTTP owner.
    drop(registry);
    assert!(
        http.session(
            Default::default(),
            tokio::time::Instant::now() + Duration::from_secs(1),
            Default::default()
        )
        .is_ok()
    );
    http.shutdown_and_wait().await;
    assert!(
        http.session(
            Default::default(),
            tokio::time::Instant::now() + Duration::from_secs(1),
            Default::default()
        )
        .is_err()
    );
}

#[tokio::test]
async fn dropping_search_cancels_actual_request_without_retry_or_closing_shared_pool() {
    for engine in ["github", "huggingface", "stackexchange", "hackernews"] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let mut ready_tx = Some(ready_tx);
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                if read_head(&mut stream).await.is_empty() {
                    continue;
                }
                assert_eq!(counter.fetch_add(1, Ordering::SeqCst), 0);
                ready_tx.take().unwrap().send(()).unwrap();
                let mut bytes = [0; 1];
                assert_eq!(
                    tokio::time::timeout(Duration::from_secs(2), stream.read(&mut bytes))
                        .await
                        .unwrap()
                        .unwrap(),
                    0,
                    "cancelled request socket must close"
                );
                break;
            }
        });
        let http = Arc::new(AdapterHttp::default());
        let mut config = CommunityConfig::default();
        let cfg = settings(&mut config, engine);
        cfg.base_url = format!("http://{address}");
        cfg.timeout_ms = 10000.5;
        let mut registry = Registry::builtins().unwrap();
        register_community(
            &mut registry,
            http.clone(),
            Arc::new(AllowAllAdmission),
            config,
        )
        .unwrap();
        let adapter = registry.instantiate(engine).unwrap();
        let task = tokio::spawn(async move {
            adapter
                .search(&SearchRequest {
                    query: "cancel fixture".into(),
                    ..SearchRequest::default()
                })
                .await
        });
        tokio::time::timeout(Duration::from_secs(2), ready_rx)
            .await
            .unwrap()
            .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        server.await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            http.session(
                Default::default(),
                tokio::time::Instant::now() + Duration::from_secs(1),
                Default::default()
            )
            .is_ok()
        );
        http.shutdown_and_wait().await;
    }
}

struct Unavailable(Arc<AtomicUsize>);
impl SearchAdmission for Unavailable {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { AdmissionDecision::Unavailable })
    }
}
#[tokio::test]
async fn unavailable_admission_is_fail_closed_before_http_and_auth_is_separate() {
    for engine in ["github", "huggingface", "stackexchange", "hackernews"] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let mut config = CommunityConfig::default();
        settings(&mut config, engine).base_url =
            format!("http://{}", listener.local_addr().unwrap());
        let http = Arc::new(AdapterHttp::default());
        let mut registry = Registry::builtins().unwrap();
        register_community(
            &mut registry,
            http.clone(),
            Arc::new(Unavailable(count.clone())),
            config,
        )
        .unwrap();
        let actual = registry
            .instantiate(engine)
            .unwrap()
            .search(&SearchRequest::default())
            .await;
        assert_eq!(actual.status, EngineStatus::Error);
        assert!(actual.results.is_empty());
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), listener.accept())
                .await
                .is_err()
        );
        http.shutdown_and_wait().await;
    }
}

struct PrivateValkey {
    child: Child,
    dir: PathBuf,
    url: String,
}
impl PrivateValkey {
    fn start() -> Self {
        let mut bytes = [0; 16];
        fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut bytes)
            .unwrap();
        bytes[6] = (bytes[6] & 15) | 64;
        bytes[8] = (bytes[8] & 63) | 128;
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("ssx-a03-{id}"));
        fs::create_dir(&dir).unwrap();
        let url = format!("unix://{}", dir.join("s.sock").display());
        let mut s = Self {
            child: Self::spawn(&dir),
            dir,
            url,
        };
        s.wait_ready();
        s
    }
    fn spawn(dir: &std::path::Path) -> Child {
        Command::new(
            std::env::var("SLOPSEARX_TEST_VALKEY_SERVER")
                .unwrap_or_else(|_| "valkey-server".into()),
        )
        .args([
            "--port",
            "0",
            "--bind",
            "127.0.0.1",
            "--save",
            "",
            "--appendonly",
            "no",
            "--daemonize",
            "no",
            "--unixsocketperm",
            "700",
            "--dir",
        ])
        .arg(dir)
        .arg("--unixsocket")
        .arg(dir.join("s.sock"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("disposable valkey-server required")
    }
    fn wait_ready(&mut self) {
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "Valkey startup failed"
            );
            if redis::Client::open(self.url.as_str())
                .unwrap()
                .get_connection()
                .is_ok()
            {
                break;
            }
            assert!(Instant::now() < until, "Valkey startup deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn store(&self) -> Valkey {
        Valkey::new(&self.url, Duration::from_millis(300)).unwrap()
    }
}
impl Drop for PrivateValkey {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[tokio::test]
async fn actual_private_valkey_f10_quota_and_cooldown_stop_all_four_before_dispatch() {
    use ssx_application::rate::{RateLimiter, RateStore};
    use ssx_domain::rate::{EngineKey, QuotaConfig};
    use ssx_infra::{adapters::F10Admission, valkey::rate::DistributedQuota};
    let server = PrivateValkey::start();
    let quota = Arc::new(DistributedQuota::new(
        Arc::new(server.store()),
        QuotaConfig::new(1.0, 3600.0).unwrap(),
    ));
    let limiter = Arc::new(tokio::sync::Mutex::new(RateLimiter::new(
        quota.clone(),
        true,
    )));
    let http = Arc::new(AdapterHttp::default());
    let mut registry = Registry::builtins().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let responder = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            if read_head(&mut stream).await.is_empty() {
                continue;
            }
            counter.fetch_add(1, Ordering::SeqCst);
            let body = b"[]"; // GH/HN/SE classify shape; quota applies independently.
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            stream.write_all(body).await.unwrap();
        }
    });
    let mut config = CommunityConfig::default();
    for engine in ["github", "huggingface", "stackexchange", "hackernews"] {
        settings(&mut config, engine).base_url = format!("http://{address}");
    }
    register_community(
        &mut registry,
        http.clone(),
        Arc::new(F10Admission::new(limiter.clone())),
        config,
    )
    .unwrap();
    for engine in ["github", "huggingface", "stackexchange", "hackernews"] {
        let adapter = registry.instantiate(engine).unwrap();
        let first = adapter.search(&SearchRequest::default()).await;
        assert_ne!(first.status, EngineStatus::RateLimited);
        let second = adapter.search(&SearchRequest::default()).await;
        assert_eq!(second.status, EngineStatus::RateLimited);
        assert!(second.results.is_empty());
        let key = EngineKey::new(engine).unwrap();
        assert_eq!(
            quota.acquire(&key, 0).await.unwrap().count,
            2,
            "denied quota acquisition is conservatively charged; provider cooldown must add no third acquisition"
        );
        quota.publish_cooldown(&key, 60.0).await.unwrap();
        let third = adapter.search(&SearchRequest::default()).await;
        assert_eq!(third.status, EngineStatus::RateLimited);
        assert_eq!(
            quota.acquire(&key, 0).await.unwrap().count,
            2,
            "denied quota acquisition is conservatively charged; provider cooldown must add no third acquisition"
        );
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        4,
        "exactly one actual HTTP request per engine"
    );
    responder.abort();
    let _ = responder.await;
    http.shutdown_and_wait().await;
    quota.shutdown().await;
}

#[test]
fn operator_numeric_projection_refuses_all_28_captured_invalid_configurations() {
    let corpus: Value = serde_json::from_str(CASES).unwrap();
    let mut refused = 0;
    for case in corpus["cases"].as_array().unwrap() {
        if case["native_disposition"] != "typed_config_bound_refusal" {
            continue;
        }
        let max = ssx_json::from_str::<ssx_json::Value>(
            &case["config"]
                .get("max_results")
                .unwrap_or(&serde_json::json!(5))
                .to_string(),
        )
        .unwrap();
        let timeout = case["config"]
            .get("timeout_ms")
            .and_then(Value::as_f64)
            .unwrap_or(5000.0);
        let error =
            CommunitySettings::resolve("https://fixture.invalid".into(), timeout, &max, None)
                .unwrap_err();
        assert_eq!(error.to_string(), "invalid community adapter settings");
        refused += 1;
    }
    assert_eq!(refused, 28);
    for text in ["0", "1", &usize::MAX.to_string()] {
        let value = ssx_json::from_str::<ssx_json::Value>(text).unwrap();
        assert!(
            CommunitySettings::resolve("https://fixture.invalid".into(), 1.5, &value, None).is_ok()
        );
    }
    for text in [
        "1.0",
        "1e0",
        "-1",
        "null",
        "true",
        "\"1\"",
        "[]",
        "{}",
        &(usize::MAX as u128 + 1).to_string(),
    ] {
        let value = ssx_json::from_str::<ssx_json::Value>(text).unwrap();
        assert!(
            CommunitySettings::resolve("https://fixture.invalid".into(), 1.5, &value, None)
                .is_err(),
            "{text}"
        );
    }
}

#[test]
fn four_native_factories_keep_source_f06_capabilities_and_jev_cards() {
    let registry = Registry::builtins().unwrap();
    for (engine, fragment) in [
        (
            "github",
            include_str!("../../../docs/rust-rewrite/f06/fragments/github.json"),
        ),
        (
            "huggingface",
            include_str!("../../../docs/rust-rewrite/f06/fragments/huggingface.json"),
        ),
        (
            "stackexchange",
            include_str!("../../../docs/rust-rewrite/f06/fragments/stackexchange.json"),
        ),
        (
            "hackernews",
            include_str!("../../../docs/rust-rewrite/f06/fragments/hackernews.json"),
        ),
    ] {
        let descriptor = registry.descriptor(engine).unwrap();
        assert_eq!(
            descriptor,
            &serde_json::from_str::<ssx_registry::EngineDescriptor>(fragment).unwrap(),
            "{engine}: accepted metadata is unchanged"
        );
        assert!(descriptor.enforced_filters.is_empty());
        assert!(descriptor.supported_media_types.is_empty());
        assert_eq!(descriptor.supported_result_types, ["text"]);
        if engine == "github" {
            assert!(descriptor.caveats.iter().any(|v| v.contains("page 1")));
        }
        if descriptor.routing.role == ssx_registry::RoutingRole::Specialist {
            assert!(
                descriptor
                    .routing
                    .purpose
                    .as_ref()
                    .is_some_and(|v| !v.is_empty())
            );
            assert!(
                descriptor
                    .routing
                    .use_when
                    .as_ref()
                    .is_some_and(|v| !v.is_empty())
            );
        }
    }
}
