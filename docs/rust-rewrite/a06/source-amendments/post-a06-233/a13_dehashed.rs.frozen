//! Source-derived DeHashed qualification. No Python/backend/provider is executed.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ssx_application::{AdmissionDecision, NoopSink, PortFuture, SearchAdmission, SearchContext};
use ssx_config::{ConfigScalar, Secret};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use ssx_infra::{
    adapters::dehashed::{DeHashedConfig, ENDPOINT, register_dehashed},
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
};
use ssx_registry::Registry;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Reply {
    status: u16,
    body: Vec<u8>,
    error: Option<ssx_infra::http::Error>,
}
impl Reply {
    fn json(body: &[u8]) -> Self {
        Self {
            status: 200,
            body: body.to_vec(),
            error: None,
        }
    }
}
struct Queue {
    replies: Mutex<VecDeque<Reply>>,
    requests: Mutex<Vec<Request>>,
}
impl Queue {
    fn new(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            requests: Mutex::new(Vec::new()),
        })
    }
}
impl Transport for Queue {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let url = request.url.clone();
            self.requests.lock().unwrap().push(request);
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("no unexpected request");
            if let Some(error) = reply.error {
                return Err(error);
            }
            Ok(Response {
                status: reply.status,
                body: reply.body,
                url,
                headers: Default::default(),
            })
        })
    }
}
struct Admission {
    decision: AdmissionDecision,
    names: Mutex<Vec<String>>,
}
impl Admission {
    fn new(decision: AdmissionDecision) -> Arc<Self> {
        Arc::new(Self {
            decision,
            names: Mutex::new(Vec::new()),
        })
    }
}
impl SearchAdmission for Admission {
    fn admit<'a>(&'a self, name: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move {
            self.names.lock().unwrap().push(name.into());
            self.decision
        })
    }
}
fn config() -> DeHashedConfig {
    let mut config = DeHashedConfig::defaults(ENDPOINT);
    config.base_url = "http://127.0.0.1:1/injected".into();
    config.api_key = Secret::new(Some("reader@example.test:synthetic:key".into()));
    config
}
fn query() -> SearchRequest {
    SearchRequest {
        query: "person@example.test".into(),
        ..Default::default()
    }
}
fn factory(
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: DeHashedConfig,
) -> Registry {
    let mut registry = Registry::builtins().unwrap();
    register_dehashed(&mut registry, http, admission, config).unwrap();
    registry
}
async fn execute(
    config: DeHashedConfig,
    request: SearchRequest,
    reply: Reply,
    decision: AdmissionDecision,
) -> (AdapterResponse, Arc<Queue>, Arc<Admission>) {
    let wire = Queue::new(vec![reply]);
    let admission = Admission::new(decision);
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(wire.clone()).unwrap();
    let registry = factory(http.clone(), admission.clone(), config);
    let result = registry
        .instantiate("dehashed")
        .unwrap()
        .search(&request)
        .await;
    http.shutdown_and_wait().await;
    (result, wire, admission)
}
fn row(url: &str, title: &str, content: &str, position: i64) -> SearchResult {
    SearchResult {
        url: url.into(),
        title: title.into(),
        content: content.into(),
        engine: "dehashed".into(),
        position,
        ..Default::default()
    }
}
fn whole(
    mut result: AdapterResponse,
    status: EngineStatus,
    results: Vec<SearchResult>,
    error: Option<&str>,
) {
    assert!(result.latency_ms.get().is_finite() && result.latency_ms.get() >= 0.);
    result.latency_ms = FiniteF64::new(0.).unwrap();
    assert_eq!(
        result,
        AdapterResponse {
            status,
            results,
            error_message: error.map(str::to_owned),
            ..Default::default()
        }
    );
}
fn request(wire: &Queue, q: &str, size: &str, pair: &str) {
    let requests = wire.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let r = &requests[0];
    assert_eq!(r.method, reqwest::Method::GET);
    assert_eq!(r.url.scheme(), "http");
    assert_eq!(r.url.host_str(), Some("127.0.0.1"));
    assert_eq!(r.url.port(), Some(1));
    assert_eq!(r.url.path(), "/injected/search");
    assert_eq!(r.url.fragment(), None);
    assert_eq!(
        r.url
            .query_pairs()
            .map(|(a, b)| (a.into_owned(), b.into_owned()))
            .collect::<Vec<_>>(),
        vec![("q".into(), q.into()), ("size".into(), size.into())]
    );
    let headers: BTreeMap<_, _> = r
        .headers
        .iter()
        .map(|(k, v)| (k.as_str(), v.to_str().unwrap().to_owned()))
        .collect();
    assert_eq!(
        headers,
        expected_default_headers([
            ("accept", "application/json".into()),
            (
                "authorization",
                format!("Basic {}", STANDARD.encode(pair.as_bytes()))
            )
        ])
    );
    assert!(r.body.is_empty());
    assert!(r.headers[reqwest::header::AUTHORIZATION].is_sensitive());
    let debug = format!("{r:?}");
    assert!(
        !debug.contains(&STANDARD.encode(pair.as_bytes())),
        "Debug must redact encoded Basic credentials"
    );
    if pair.len() > 1 {
        assert!(
            !debug.contains(pair),
            "Debug must redact raw credential pair"
        );
    }
}
fn complete(id: &str) {
    println!("A13_DEHASHED_CASE_COMPLETE {id}");
}
const SUCCESS: &[u8] = br#"{"entries":[{"email":"person@example.test","username":"fixture-user","password":"abcd-never-return-this","hashed_password":"0123456789ab-never-return-this","ip_address":"192.0.2.4","name":"Unused","database_name":"FixtureDB","obtained_from":"SyntheticSource"}]}"#;
fn success_row() -> SearchResult {
    row(
        "https://dehashed.com/search?q=person@example.test",
        "person@example.test",
        "Email: person@example.test | Username: fixture-user | Password: abcd*** | Hash: 0123456789ab... | IP: 192.0.2.4 | DB: FixtureDB | Source: SyntheticSource",
        1,
    )
}
#[tokio::test]
async fn full_source_success_and_complete_outbound() {
    let (result, wire, admission) = execute(
        config(),
        query(),
        Reply::json(SUCCESS),
        AdmissionDecision::Allowed,
    )
    .await;
    whole(result, EngineStatus::Ok, vec![success_row()], None);
    request(
        &wire,
        "person@example.test",
        "10",
        "reader@example.test:synthetic:key",
    );
    assert_eq!(*admission.names.lock().unwrap(), vec!["dehashed"]);
    complete("source-success");
}
#[tokio::test]
async fn missing_key_and_admission_precedence() {
    for (id, decision, status, message) in [
        (
            "missing-key",
            AdmissionDecision::Allowed,
            EngineStatus::Error,
            "DeHashed API key not configured (set ENGINE_DEHASHED_API_KEY)",
        ),
        (
            "admission-denied-first",
            AdmissionDecision::Denied,
            EngineStatus::RateLimited,
            "rate limited",
        ),
        (
            "admission-unavailable-first",
            AdmissionDecision::Unavailable,
            EngineStatus::Error,
            "dehashed rate policy unavailable",
        ),
    ] {
        let mut c = config();
        c.api_key = Secret::new(None);
        c.base_url = "not a URL".into();
        let (result, wire, admission) = execute(c, query(), Reply::json(b"bad"), decision).await;
        assert_eq!(result.latency_ms.get(), 0.);
        whole(result, status, vec![], Some(message));
        assert!(wire.requests.lock().unwrap().is_empty());
        assert_eq!(*admission.names.lock().unwrap(), vec!["dehashed"]);
        complete(id);
    }
}
#[tokio::test]
async fn http_status_classification_and_redaction() {
    for status in [301, 401, 403, 404, 429, 500] {
        let reply = Reply {
            status,
            body: b"provider-secret-marker".to_vec(),
            error: None,
        };
        let (result, wire, _) = execute(config(), query(), reply, AdmissionDecision::Allowed).await;
        let expected = match status {
            403 => EngineStatus::Blocked,
            429 => EngineStatus::RateLimited,
            _ => EngineStatus::Error,
        };
        let message =
            (!matches!(status, 403 | 429)).then(|| format!("DeHashed returned HTTP {status}"));
        assert!(!format!("{result:?}").contains("provider-secret-marker"));
        whole(result, expected, vec![], message.as_deref());
        assert_eq!(wire.requests.lock().unwrap().len(), 1);
        complete(&format!("status-{status}"));
    }
    for (id, error, status, message) in [
        (
            "transport-timeout",
            ssx_infra::http::Error::Deadline,
            EngineStatus::Timeout,
            None,
        ),
        (
            "transport-error",
            ssx_infra::http::Error::Network,
            EngineStatus::Error,
            Some("DeHashed request failed"),
        ),
    ] {
        let (result, wire, _) = execute(
            config(),
            query(),
            Reply {
                status: 200,
                body: vec![],
                error: Some(error),
            },
            AdmissionDecision::Allowed,
        )
        .await;
        whole(result, status, vec![], message);
        assert_eq!(wire.requests.lock().unwrap().len(), 1);
        complete(id);
    }
}
#[tokio::test]
async fn basic_auth_first_colon_unicode_and_empty_components() {
    for (id, key, pair) in [
        ("auth-colonless", "key", ":key"),
        ("auth-first-colon", "mail:one:two", "mail:one:two"),
        ("auth-unicode", "mø@example.test:κ:π", "mø@example.test:κ:π"),
        ("auth-empty-user", ":pass", ":pass"),
        ("auth-empty-both", ":", ":"),
    ] {
        let mut c = config();
        c.api_key = Secret::new(Some(key.into()));
        let (result, wire, _) =
            execute(c, query(), Reply::json(b"{}"), AdmissionDecision::Allowed).await;
        whole(result, EngineStatus::Ok, vec![], None);
        request(&wire, "person@example.test", "10", pair);
        complete(id);
    }
    let c = config();
    assert!(!format!("{c:?}").contains("synthetic:key"));
    complete("config-debug-redacted");
    complete("auth-header-debug-redacted");
}
#[tokio::test]
async fn source_slices_and_request_limits_are_independent() {
    let body=br#"{"entries":[{"email":"one@example.test"},{"email":"two@example.test"},{"email":"three@example.test"}]}"#;
    for (id, max, size, end) in [
        ("slice-zero", ConfigScalar::Integer(0), "0", Some(0)),
        ("slice-one", ConfigScalar::Integer(1), "1", Some(1)),
        ("slice-negative", ConfigScalar::Integer(-1), "-1", Some(2)),
        (
            "slice-negative-all",
            ConfigScalar::Integer(-99),
            "-99",
            Some(0),
        ),
        (
            "slice-bool-true",
            ConfigScalar::Boolean(true),
            "true",
            Some(1),
        ),
        (
            "slice-bool-false",
            ConfigScalar::Boolean(false),
            "false",
            Some(0),
        ),
        (
            "slice-string-refused",
            ConfigScalar::Text("2".into()),
            "2",
            None,
        ),
        ("slice-float-refused", ConfigScalar::Float(2.), "2.0", None),
        (
            "slice-nan-refused",
            ConfigScalar::Float(f64::NAN),
            "nan",
            None,
        ),
    ] {
        let mut c = config();
        c.max_results = max;
        let mut q = query();
        q.max_results = Some(1);
        q.page = 17;
        let (result, wire, _) = execute(c, q, Reply::json(body), AdmissionDecision::Allowed).await;
        let all = ["one@example.test", "two@example.test", "three@example.test"]
            .into_iter()
            .enumerate()
            .map(|(i, s)| {
                row(
                    &format!("https://dehashed.com/search?q={s}"),
                    s,
                    &format!("Email: {s}"),
                    i as i64 + 1,
                )
            })
            .collect::<Vec<_>>();
        if let Some(end) = end {
            whole(result, EngineStatus::Ok, all[..end].to_vec(), None)
        } else {
            whole(
                result,
                EngineStatus::Error,
                vec![],
                Some("slice indices must be integers"),
            )
        };
        request(
            &wire,
            "person@example.test",
            size,
            "reader@example.test:synthetic:key",
        );
        complete(id);
    }
}
#[tokio::test]
async fn empty_and_wrong_shape_source_operations() {
    for (id, body, max, status, message) in [
        (
            "missing-entries",
            b"{}".as_slice(),
            10,
            EngineStatus::Ok,
            None,
        ),
        (
            "empty-entries",
            b"{\"entries\":[]}".as_slice(),
            10,
            EngineStatus::Ok,
            None,
        ),
        (
            "empty-string",
            b"{\"entries\":\"\"}".as_slice(),
            10,
            EngineStatus::Ok,
            None,
        ),
        (
            "sliced-string-empty",
            b"{\"entries\":\"abc\"}".as_slice(),
            0,
            EngineStatus::Ok,
            None,
        ),
        (
            "nonempty-string",
            b"{\"entries\":\"abc\"}".as_slice(),
            10,
            EngineStatus::Error,
            Some("DeHashed response element does not support field access"),
        ),
        (
            "null-entries",
            b"{\"entries\":null}".as_slice(),
            10,
            EngineStatus::Error,
            Some("DeHashed entries collection does not support slicing"),
        ),
        (
            "mapping-entries",
            b"{\"entries\":{}}".as_slice(),
            0,
            EngineStatus::Error,
            Some("DeHashed entries mapping cannot be sliced"),
        ),
        (
            "outer-array",
            b"[]".as_slice(),
            10,
            EngineStatus::Error,
            Some("DeHashed response element does not support field access"),
        ),
        (
            "outer-null",
            b"null".as_slice(),
            10,
            EngineStatus::Error,
            Some("DeHashed response element does not support field access"),
        ),
        (
            "scalar-row",
            b"{\"entries\":[1]}".as_slice(),
            10,
            EngineStatus::Error,
            Some("DeHashed response element does not support field access"),
        ),
    ] {
        let mut c = config();
        c.max_results = ConfigScalar::Integer(max);
        let (result, wire, _) =
            execute(c, query(), Reply::json(body), AdmissionDecision::Allowed).await;
        whole(result, status, vec![], message);
        assert_eq!(wire.requests.lock().unwrap().len(), 1);
        complete(id);
    }
}
#[tokio::test]
async fn fallback_titles_literal_url_and_source_content_order() {
    let body=br#"{"entries":[{"id":null},{"username":"user+one","name":"unused"},{"name":"Synthetic Name","ip_address":123},{"email":"a+b@example.test"}]}"#;
    let (result, _, _) = execute(
        config(),
        query(),
        Reply::json(body),
        AdmissionDecision::Allowed,
    )
    .await;
    whole(
        result,
        EngineStatus::Ok,
        vec![
            row(
                "https://dehashed.com/search?q=person@example.test",
                "Entry #None",
                "",
                1,
            ),
            row(
                "https://dehashed.com/search?q=person@example.test",
                "user+one",
                "Username: user+one",
                2,
            ),
            row(
                "https://dehashed.com/search?q=person@example.test",
                "Synthetic Name",
                "IP: 123",
                3,
            ),
            row(
                "https://dehashed.com/search?q=a+b@example.test",
                "a+b@example.test",
                "Email: a+b@example.test",
                4,
            ),
        ],
        None,
    );
    complete("fallback-and-url");
}
#[tokio::test]
async fn used_prefixes_and_unused_unicode_values() {
    for (id, body, content) in [
        (
            "password-tail-surrogate",
            br#"{"entries":[{"password":"abcd\ud800"}]}"#.as_slice(),
            "Password: abcd***",
        ),
        (
            "hash-tail-surrogate",
            br#"{"entries":[{"hashed_password":"0123456789ab\ud800"}]}"#.as_slice(),
            "Hash: 0123456789ab...",
        ),
        (
            "unused-name-surrogate",
            br#"{"entries":[{"email":"person@example.test","name":"\ud800"}]}"#.as_slice(),
            "Email: person@example.test",
        ),
        (
            "unused-extra-surrogate",
            br#"{"entries":[{"extra":"\ud800"}]}"#.as_slice(),
            "",
        ),
        (
            "array-password-slice",
            br#"{"entries":[{"password":["x",1,null,["z"],"unused"]}]}"#.as_slice(),
            "Password: ['x', 1, None, ['z']]***",
        ),
    ] {
        let email = id == "unused-name-surrogate";
        let title = if email {
            "person@example.test"
        } else {
            "Entry #0"
        };
        let (result, _, _) = execute(
            config(),
            query(),
            Reply::json(body),
            AdmissionDecision::Allowed,
        )
        .await;
        whole(
            result,
            EngineStatus::Ok,
            vec![row(
                "https://dehashed.com/search?q=person@example.test",
                title,
                content,
                1,
            )],
            None,
        );
        complete(id);
    }
    let (result, _, _) = execute(
        config(),
        query(),
        Reply::json(br#"{"entries":[{"password":"ab\ud800c"}]}"#),
        AdmissionDecision::Allowed,
    )
    .await;
    whole(
        result,
        EngineStatus::Error,
        vec![],
        Some("source text contains non-scalar Unicode; requires reviewed projection"),
    );
    complete("selected-surrogate-refused");
}
#[tokio::test]
async fn late_failure_is_atomic_and_operation_errors_precede_typed_title() {
    for (id, body, max, message) in [
        (
            "late-row-atomic",
            br#"{"entries":[{"email":"valid@example.test"},false]}"#.as_slice(),
            ConfigScalar::Integer(10),
            "DeHashed response element does not support field access",
        ),
        (
            "prefix-before-title",
            br#"{"entries":[{"email":42,"password":17}]}"#.as_slice(),
            ConfigScalar::Integer(10),
            "source value does not support slicing",
        ),
        (
            "subscription-before-index",
            br#"{"entries":null}"#.as_slice(),
            ConfigScalar::Text("bad".into()),
            "DeHashed entries collection does not support slicing",
        ),
        (
            "typed-title-after-content",
            br#"{"entries":[{"email":42,"password":"abcd"}]}"#.as_slice(),
            ConfigScalar::Integer(10),
            "expected source string",
        ),
    ] {
        let mut c = config();
        c.max_results = max;
        let (result, wire, _) =
            execute(c, query(), Reply::json(body), AdmissionDecision::Allowed).await;
        whole(result, EngineStatus::Error, vec![], Some(message));
        assert_eq!(wire.requests.lock().unwrap().len(), 1);
        complete(id);
    }
}
#[tokio::test]
async fn configured_invalid_inputs_and_codec_refusals_are_atomic() {
    for (id, change, status, message) in [
        (
            "invalid-base",
            0,
            EngineStatus::Error,
            "invalid DeHashed request URL",
        ),
        ("zero-timeout", 1, EngineStatus::Timeout, ""),
        ("text-timeout", 2, EngineStatus::Error, "invalid timeout_ms"),
    ] {
        let mut c = config();
        match change {
            0 => c.base_url = "not a URL".into(),
            1 => c.timeout_ms = ConfigScalar::Integer(0),
            _ => c.timeout_ms = ConfigScalar::Text("10".into()),
        };
        let (result, wire, _) =
            execute(c, query(), Reply::json(SUCCESS), AdmissionDecision::Allowed).await;
        whole(
            result,
            status,
            vec![],
            (!message.is_empty()).then_some(message),
        );
        assert!(wire.requests.lock().unwrap().is_empty());
        complete(id);
    }
    for (id, body) in [
        ("malformed-json", b"provider-secret-marker".to_vec()),
        ("byte-limit", vec![b' '; 4 * 1024 * 1024 + 1]),
    ] {
        let (result, wire, _) = execute(
            config(),
            query(),
            Reply::json(&body),
            AdmissionDecision::Allowed,
        )
        .await;
        assert_eq!(result.status, EngineStatus::Error);
        assert!(result.results.is_empty());
        assert!(!format!("{result:?}").contains("provider-secret-marker"));
        assert!(
            result
                .error_message
                .as_deref()
                .unwrap()
                .starts_with("invalid source JSON:")
        );
        assert_eq!(wire.requests.lock().unwrap().len(), 1);
        complete(id);
    }
}
#[tokio::test]
async fn registry_factory_preserves_all_catalog_metadata() {
    let http = Arc::new(AdapterHttp::default());
    let wire = Queue::new(vec![Reply::json(b"{}")]);
    http.set_transport(wire).unwrap();
    let mut registry = Registry::builtins().unwrap();
    let before = registry.descriptors().cloned().collect::<Vec<_>>();
    register_dehashed(
        &mut registry,
        http.clone(),
        Admission::new(AdmissionDecision::Allowed),
        config(),
    )
    .unwrap();
    assert_eq!(registry.descriptors().cloned().collect::<Vec<_>>(), before);
    assert!(registry.is_available("dehashed"));
    assert!(!registry.is_available("hibp"));
    assert!(!registry.is_available("intelx"));
    assert!(registry.descriptor("dehashed").unwrap().sensitive);
    assert!(!registry.descriptor("intelx").unwrap().sensitive);
    assert!(
        register_dehashed(
            &mut registry,
            http.clone(),
            Admission::new(AdmissionDecision::Allowed),
            config()
        )
        .is_err()
    );
    http.shutdown_and_wait().await;
    complete("real-registration");
}
struct Entered {
    entered: tokio::sync::Notify,
    dropped: Arc<AtomicUsize>,
    block: AtomicBool,
    calls: AtomicUsize,
}
struct DropWitness(Arc<AtomicUsize>);
impl Drop for DropWitness {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
impl Transport for Entered {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.block.swap(false, Ordering::SeqCst) {
                let _owner = DropWitness(self.dropped.clone());
                self.entered.notify_one();
                std::future::pending::<()>().await;
            }
            Ok(Response {
                status: 200,
                body: b"{}".to_vec(),
                url: request.url,
                headers: Default::default(),
            })
        })
    }
}
#[tokio::test]
async fn entered_exchange_cancellation_drops_actual_future_and_keeps_http_owner() {
    let transport = Arc::new(Entered {
        entered: tokio::sync::Notify::new(),
        dropped: Arc::new(AtomicUsize::new(0)),
        block: AtomicBool::new(true),
        calls: AtomicUsize::new(0),
    });
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(transport.clone()).unwrap();
    let registry = factory(
        http.clone(),
        Admission::new(AdmissionDecision::Allowed),
        config(),
    );
    let engine = registry.instantiate("dehashed").unwrap();
    let q = query();
    let mut future = Box::pin(engine.search(&q));
    tokio::time::timeout(Duration::from_secs(3),async{tokio::select!{_ =transport.entered.notified()=>{},_ =&mut future=>panic!("entered operation completed before cancellation")}}).await.unwrap();
    assert_eq!(transport.dropped.load(Ordering::SeqCst), 0);
    drop(future);
    tokio::time::timeout(Duration::from_secs(1), async {
        while transport.dropped.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await
        }
    })
    .await
    .unwrap();
    assert_eq!(transport.dropped.load(Ordering::SeqCst), 1);
    whole(engine.search(&q).await, EngineStatus::Ok, vec![], None);
    assert_eq!(transport.calls.load(Ordering::SeqCst), 2);
    http.shutdown_and_wait().await;
    http.shutdown_and_wait().await;
    complete("entered-cancel-and-reuse");
}
#[tokio::test]
async fn preexpired_caller_context_never_admits_or_dispatches() {
    let http = Arc::new(AdapterHttp::default());
    let wire = Queue::new(vec![Reply::json(SUCCESS)]);
    http.set_transport(wire.clone()).unwrap();
    let admission = Admission::new(AdmissionDecision::Allowed);
    let registry = factory(http.clone(), admission.clone(), config());
    let result = registry
        .instantiate("dehashed")
        .unwrap()
        .search_with_context(
            &query(),
            SearchContext {
                deadline: Instant::now() - Duration::from_secs(1),
            },
        )
        .await;
    whole(result, EngineStatus::Timeout, vec![], None);
    assert!(admission.names.lock().unwrap().is_empty());
    assert!(wire.requests.lock().unwrap().is_empty());
    http.shutdown_and_wait().await;
    complete("preexpired-context");
}
fn service_fixture(
    sensitive_allowed: bool,
    http: Arc<AdapterHttp>,
    registry: Arc<Registry>,
) -> ssx_infra::search::SearchService {
    use ssx_application::{
        cache::RoutingIdentity,
        policy::{PolicyService, SearchPolicy},
        scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
    };
    use ssx_infra::search::{
        EngineBinding, SearchConfig, SearchDependencies, SearchService, SystemSearchClock,
    };
    let mut configuration = ssx_config::load_config_with(
        Some(std::path::Path::new("/no/a13/private-fixture-config")),
        &BTreeMap::new(),
    )
    .unwrap();
    let configured = configuration.engines.get_mut("dehashed").unwrap();
    configured.enabled = true;
    configured.api_key = config().api_key;
    let engines = BTreeMap::from([(
        "dehashed".into(),
        EngineBinding {
            engine: Arc::from(registry.instantiate("dehashed").unwrap()),
            generation: "a13-dehashed-source-v1".into(),
            timeout: Duration::from_secs(10),
            scrape: false,
        },
    )]);
    let capabilities = registry
        .descriptors()
        .map(|d| {
            let (enabled, auth, ready, categories) =
                ssx_config::capability_configured(d, &configuration);
            EngineCapability {
                name: d.name.clone(),
                enabled,
                active: engines.contains_key(&d.name),
                available: registry.is_available(&d.name),
                categories,
                media_types: d.supported_media_types.clone(),
                auth_ready: auth != ssx_registry::AuthClass::Required || ready,
                circuit_open: false,
                cost_class: d.cost_class.clone().unwrap_or_default(),
                health_status: "unknown".into(),
                health_stale: false,
                enforced_filters: d.enforced_filters.clone(),
            }
        })
        .collect();
    let policy = PolicyService {
        policy: SearchPolicy {
            sensitive_allowed,
            grants: ["jobs", "security", "science", "research"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            ..Default::default()
        },
        catalog: CatalogSnapshot::new(capabilities).unwrap(),
        resolver: ScopeResolver::default(),
    };
    let routing_identity = RoutingIdentity {
        catalog: None,
        sensitive: policy.policy.sensitive_engines.clone(),
        tier1: policy.resolver.tier1.clone(),
        budget: None,
        ranking_strategy: "presence".into(),
        work_group_version: "scholarly-work-v3".into(),
        rerank_identity: None,
        routing_extension: None,
    };
    // HTTP is deliberately caller-owned; this service must not close that owner.
    let _caller_owned = &http;
    SearchService::new_with_registry(
        SearchConfig::default(),
        SearchDependencies {
            policy,
            engines,
            routing_identity,
            cache: None,
            client_admission: None,
            suggestions: None,
            jev: None,
            routing_cache: None,
            metrics: Arc::new(NoopSink),
            supervisor: Arc::new(NoopSink),
            audit: Arc::new(NoopSink),
            cooldowns: None,
            owned_http: vec![],
            owned_stores: vec![],
            clock: Arc::new(SystemSearchClock),
            reranker: None,
        },
        registry,
    )
    .unwrap()
}
#[tokio::test]
async fn genuine_factory_crosses_shared_policy_only_with_sensitive_grant() {
    use ssx_application::policy::{ScopeProposal, SearchSurface};
    use ssx_infra::search::SearchError;
    let wire = Queue::new(vec![Reply::json(SUCCESS)]);
    let admission = Admission::new(AdmissionDecision::Allowed);
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(wire.clone()).unwrap();
    let registry = Arc::new(factory(http.clone(), admission.clone(), config()));
    assert!(registry.is_available("dehashed"));
    assert!(!registry.is_available("hibp"));
    assert!(!registry.is_available("intelx"));
    let denied = service_fixture(false, http.clone(), registry.clone());
    for (id, surface) in [
        ("policy-generic", SearchSurface::Generic),
        ("policy-targeted", SearchSurface::Targeted),
        ("policy-jobs", SearchSurface::Jobs),
        ("policy-security", SearchSurface::Security),
        ("policy-science", SearchSurface::Science),
        ("policy-preview", SearchSurface::Preview),
    ] {
        let mut q = query();
        q.engines = Some(vec!["dehashed".into()]);
        let mut proposal = ScopeProposal::generic(q);
        proposal.surface = surface;
        let error = if surface == SearchSurface::Preview {
            denied.preview_scope(&proposal).await.unwrap_err()
        } else {
            denied.search(proposal).await.unwrap_err()
        };
        let SearchError::Policy(error) = error else {
            panic!("shared policy must deny before acquisition")
        };
        assert_eq!(error.code, "tool_disabled");
        assert_eq!(error.field, "engines");
        assert_eq!(
            error.message,
            "sensitive engines are unreachable without the sensitive-engine grant (MCP_TARGETED_SENSITIVE_ALLOWED=1): dehashed"
        );
        assert_eq!(
            *error.details,
            ssx_json::json!({"engines":["dehashed"],"grant":"MCP_TARGETED_SENSITIVE_ALLOWED"})
        );
        assert!(wire.requests.lock().unwrap().is_empty());
        assert!(admission.names.lock().unwrap().is_empty());
        complete(id);
    }
    denied.shutdown().await;
    let allowed = service_fixture(true, http.clone(), registry.clone());
    let mut q = query();
    q.engines = Some(vec!["dehashed".into()]);
    let response = allowed.search(ScopeProposal::generic(q)).await.unwrap();
    let mut expected = success_row();
    expected.tier = 2;
    expected.score = FiniteF64::new(1.).unwrap();
    expected.engines = BTreeSet::from(["dehashed".into()]);
    assert_eq!(response.results, vec![expected]);
    assert_eq!(response.query, "person@example.test");
    assert_eq!(response.engine_outcomes.len(), 1);
    assert_eq!(response.engine_outcomes[0].status, "ok");
    assert!(!response.cached);
    request(
        &wire,
        "person@example.test",
        "10",
        "reader@example.test:synthetic:key",
    );
    assert_eq!(*admission.names.lock().unwrap(), vec!["dehashed"]);
    complete("policy-authorized-real-factory");
    allowed.shutdown().await;
    // Closing one search owner must leave the independently owned HTTP usable.
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
#[tokio::test]
async fn entered_exchange_shutdown_joins_and_rejects_reuse() {
    let transport = Arc::new(Entered {
        entered: tokio::sync::Notify::new(),
        dropped: Arc::new(AtomicUsize::new(0)),
        block: AtomicBool::new(true),
        calls: AtomicUsize::new(0),
    });
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(transport.clone()).unwrap();
    let registry = factory(
        http.clone(),
        Admission::new(AdmissionDecision::Allowed),
        config(),
    );
    let engine = registry.instantiate("dehashed").unwrap();
    let q = query();
    let mut future = Box::pin(engine.search(&q));
    tokio::time::timeout(Duration::from_secs(3),async{tokio::select!{_ =transport.entered.notified()=>{},_ =&mut future=>panic!("operation never held actual transport")}}).await.unwrap();
    let shutdown = http.shutdown_and_wait();
    tokio::pin!(shutdown);
    let (result, _) = tokio::time::timeout(Duration::from_secs(3), async {
        tokio::join!(&mut future, &mut shutdown)
    })
    .await
    .unwrap();
    whole(
        result,
        EngineStatus::Error,
        vec![],
        Some("DeHashed request failed"),
    );
    assert_eq!(transport.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
    assert!(
        http.session(
            Default::default(),
            tokio::time::Instant::now() + Duration::from_secs(1),
            Default::default()
        )
        .is_err()
    );
    http.shutdown_and_wait().await;
    complete("entered-shutdown");
}
// Private qualification-only pipe ownership. The parent retains bytes even if
// the task is aborted; finish always joins before taking that bounded backing.
const PRIVATE_PIPE_LIMIT: usize = 1024 * 1024;
#[derive(Clone, Copy)]
enum PipeReadState {
    Eof,
    Error(std::io::ErrorKind),
    Limit,
}
struct OwnedPipeReader {
    bytes: Arc<Mutex<Vec<u8>>>,
    task: tokio::task::JoinHandle<PipeReadState>,
}
struct PipeCapture {
    bytes: Vec<u8>,
    state: Option<PipeReadState>,
    timed_out: bool,
    task_cancelled: bool,
    task_panicked: bool,
}
impl PipeCapture {
    fn eof(&self) -> bool {
        matches!(self.state, Some(PipeReadState::Eof))
    }
    fn metadata(&self) -> serde_json::Value {
        let error = match self.state {
            Some(PipeReadState::Error(kind)) => Some(format!("{kind:?}")),
            _ => None,
        };
        serde_json::json!({"eof":self.eof(),"read_error_kind":error,"limit_hit":matches!(self.state,Some(PipeReadState::Limit)),"timed_out":self.timed_out,"task_cancelled":self.task_cancelled,"task_panicked":self.task_panicked,"incomplete":!self.eof()})
    }
}
fn spawn_private_pipe<R>(mut reader: R) -> OwnedPipeReader
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    use tokio::io::AsyncReadExt;
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let retained = bytes.clone();
    let task = tokio::spawn(async move {
        let mut buffer = [0u8; 4096];
        loop {
            let remaining = PRIVATE_PIPE_LIMIT + 1 - retained.lock().unwrap().len();
            if remaining == 0 {
                return PipeReadState::Limit;
            }
            let available = remaining.min(buffer.len());
            match reader.read(&mut buffer[..available]).await {
                Ok(0) => return PipeReadState::Eof,
                Ok(n) => retained.lock().unwrap().extend_from_slice(&buffer[..n]),
                Err(error) => return PipeReadState::Error(error.kind()),
            }
        }
    });
    OwnedPipeReader { bytes, task }
}
async fn finish_private_pipe(mut reader: OwnedPipeReader) -> PipeCapture {
    let (joined, timed_out) =
        match tokio::time::timeout(Duration::from_secs(1), &mut reader.task).await {
            Ok(joined) => (joined, false),
            Err(_) => {
                reader.task.abort();
                (reader.task.await, true)
            }
        };
    let (state, task_cancelled, task_panicked) = match joined {
        Ok(state) => (Some(state), false, false),
        Err(error) => (None, error.is_cancelled(), error.is_panic()),
    };
    let bytes = std::mem::take(&mut *reader.bytes.lock().unwrap());
    PipeCapture {
        bytes,
        state,
        timed_out,
        task_cancelled,
        task_panicked,
    }
}
#[tokio::test(start_paused = true)]
async fn private_pipe_failures_retain_actual_read_prefix() {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };
    const PREFIX: &[u8] = b"retained-prefix\0\xff";
    struct Probe {
        sent: bool,
        fail: bool,
        polled: Arc<tokio::sync::Notify>,
        drops: Arc<AtomicUsize>,
    }
    impl tokio::io::AsyncRead for Probe {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buffer: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            if !self.sent {
                self.sent = true;
                buffer.put_slice(PREFIX);
                return Poll::Ready(Ok(()));
            }
            self.polled.notify_one();
            if self.fail {
                Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into()))
            } else {
                Poll::Pending
            }
        }
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }
    for (id, fail, cancel) in [
        ("pipe-prefix-before-read-error", true, false),
        ("pipe-prefix-before-cancel", false, true),
        ("pipe-prefix-before-timeout", false, false),
    ] {
        let polled = Arc::new(tokio::sync::Notify::new());
        let drops = Arc::new(AtomicUsize::new(0));
        let reader = spawn_private_pipe(Probe {
            sent: false,
            fail,
            polled: polled.clone(),
            drops: drops.clone(),
        });
        // This is actual AsyncRead participation after the prefix was retained,
        // not a peer-write notification or a drop before the future was polled.
        polled.notified().await;
        assert_eq!(reader.bytes.lock().unwrap().as_slice(), PREFIX);
        if cancel {
            reader.task.abort();
        }
        let start = tokio::time::Instant::now();
        let mut capture = Box::pin(finish_private_pipe(reader));
        if !fail && !cancel {
            let poll = std::future::poll_fn(|cx| Poll::Ready(capture.as_mut().poll(cx))).await;
            assert!(
                poll.is_pending(),
                "actual pending reader must arm the one-second watchdog"
            );
            assert_eq!(drops.load(Ordering::SeqCst), 0);
            tokio::time::advance(Duration::from_secs(1)).await;
        }
        let capture = capture.await;
        assert_eq!(capture.bytes, PREFIX);
        assert!(!capture.eof());
        assert_eq!(
            drops.load(Ordering::SeqCst),
            1,
            "actual reader is dropped before evidence publication"
        );
        assert_eq!(
            capture.metadata(),
            serde_json::json!({"eof":false,"read_error_kind":if fail {Some("BrokenPipe")}else{None},"limit_hit":false,"timed_out":!fail&&!cancel,"task_cancelled":!fail,"task_panicked":false,"incomplete":true})
        );
        assert_eq!(
            STANDARD.decode(STANDARD.encode(&capture.bytes)).unwrap(),
            PREFIX
        );
        if !fail && !cancel {
            assert_eq!(tokio::time::Instant::now(), start + Duration::from_secs(1));
        }
        complete(id);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn private_literal_loopback_complete_http_contract() {
    use std::io::Read;
    use std::os::unix::fs::DirBuilderExt;
    use std::process::Stdio;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::process::Command;
    const FLAG: &str = "SSX_A13_DEHASHED_PRIVATE_CHILD";
    const TEST: &str = "private_literal_loopback_complete_http_contract";
    if std::env::var(FLAG).is_err() {
        let mut nonce = [0u8; 8];
        std::fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut nonce)
            .unwrap();
        let private = std::env::temp_dir().join(format!(
            "ssx-a13-{}-{:016x}",
            std::process::id(),
            u64::from_ne_bytes(nonce)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&private)
            .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &private)
            .env("TMPDIR", &private)
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env(FLAG, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let out = spawn_private_pipe(stdout);
        let err = spawn_private_pipe(stderr);
        let (status, timed_out) =
            match tokio::time::timeout(Duration::from_secs(20), child.wait()).await {
                Ok(status) => (status, false),
                Err(_) => {
                    let _ = child.kill().await;
                    (child.wait().await, true)
                }
            };
        let stdout_capture = finish_private_pipe(out).await;
        let stderr_capture = finish_private_pipe(err).await;
        let stdout_eof = stdout_capture.eof();
        let stderr_eof = stderr_capture.eof();
        let stdout_metadata = stdout_capture.metadata();
        let stderr_metadata = stderr_capture.metadata();
        let stdout = stdout_capture.bytes;
        let stderr = stderr_capture.bytes;
        println!(
            "A13_DEHASHED_CHILD_STDOUT_BASE64 {}",
            STANDARD.encode(&stdout)
        );
        println!(
            "A13_DEHASHED_CHILD_STDERR_BASE64 {}",
            STANDARD.encode(&stderr)
        );
        println!(
            "A13_DEHASHED_CHILD_LIFECYCLE {}",
            serde_json::json!({"status_code":status.as_ref().ok().and_then(|s|s.code()),"wait_succeeded":status.is_ok(),"success":status.as_ref().is_ok_and(|s|s.success()),"forced_kill":timed_out,"stdout_eof":stdout_eof,"stderr_eof":stderr_eof,"stdout_bytes":stdout.len(),"stderr_bytes":stderr.len(),"stdout_read":stdout_metadata,"stderr_read":stderr_metadata})
        );
        std::fs::remove_dir(&private).unwrap();
        assert!(
            !timed_out,
            "private HTTP child exceeded watchdog; killed child is never success"
        );
        assert!(
            stdout_eof && stderr_eof,
            "both owned pipes must reach actual EOF"
        );
        assert!(
            stdout.len() + stderr.len() <= 1024 * 1024,
            "private child diagnostic bound"
        );
        let status = status.expect("owned child must be reaped");
        assert!(
            status.success(),
            "private child failed: {}",
            String::from_utf8_lossy(&stderr)
        );
        let stdout = String::from_utf8(stdout).unwrap();
        assert_eq!(
            stdout
                .lines()
                .filter(|s| *s == "A13_DEHASHED_PRIVATE_CHILD_COMPLETE")
                .count(),
            1
        );
        assert!(stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"));
        complete("literal-http-parent-exit");
        return;
    }
    assert_eq!(std::env::var(FLAG).unwrap(), "1");
    for name in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "SSLKEYLOGFILE",
        "NODE_OPTIONS",
        "NODE_PATH",
    ] {
        assert!(
            std::env::var_os(name).is_none(),
            "nonliteral child environment"
        );
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/usr/sbin/scutil")
            .arg("--proxy")
            .output()
            .await
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        for name in [
            "HTTPEnable",
            "HTTPSEnable",
            "SOCKSEnable",
            "ProxyAutoConfigEnable",
            "ProxyAutoDiscoveryEnable",
        ] {
            for line in text.lines() {
                if let Some((key, value)) = line.trim().split_once(':')
                    && key.trim() == name
                {
                    assert_eq!(
                        value.trim(),
                        "0",
                        "system proxy refused before literal request"
                    );
                }
            }
        }
    }
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    let mut server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 1024];
        while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut buffer).await?;
            if n == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
            }
            bytes.extend_from_slice(&buffer[..n]);
            if bytes.len() > 16 * 1024 {
                return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
            }
        }
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            SUCCESS.len()
        );
        stream.write_all(header.as_bytes()).await?;
        stream.write_all(SUCCESS).await?;
        stream.shutdown().await?;
        Ok::<_, std::io::Error>(bytes)
    });
    let mut c = config();
    c.base_url = format!("http://{addr}/api");
    let http = Arc::new(AdapterHttp::default());
    let registry = factory(http.clone(), Admission::new(AdmissionDecision::Allowed), c);
    let response = registry
        .instantiate("dehashed")
        .unwrap()
        .search(&query())
        .await;
    let wire = match tokio::time::timeout(Duration::from_secs(3), &mut server).await {
        Ok(join) => join.unwrap(),
        Err(_) => {
            server.abort();
            let _ = server.await;
            Err(std::io::Error::from(std::io::ErrorKind::TimedOut))
        }
    };
    http.shutdown_and_wait().await;
    let bytes = wire.expect("owned server must complete and join");
    whole(response, EngineStatus::Ok, vec![success_row()], None);
    let wire = String::from_utf8(bytes).unwrap();
    let mut lines = wire.split("\r\n");
    assert_eq!(
        lines.next(),
        Some("GET /api/search?q=person%40example.test&size=10 HTTP/1.1")
    );
    let headers = lines
        .take_while(|s| !s.is_empty())
        .map(|l| l.split_once(':').unwrap())
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(headers.get("host"), Some(&addr.to_string()));
    assert_eq!(
        headers.get("accept").map(String::as_str),
        Some("application/json")
    );
    assert_eq!(
        headers.get("authorization"),
        Some(&format!(
            "Basic {}",
            STANDARD.encode("reader@example.test:synthetic:key")
        ))
    );
    assert!(wire.ends_with("\r\n\r\n"));
    println!("A13_DEHASHED_PRIVATE_CHILD_COMPLETE");
}
#[tokio::test]
async fn actual_configured_and_caller_deadlines_drop_entered_exchange_without_retry() {
    // Real std/Tokio clocks only. No paused-time substitution or body/decode claim.
    // The limiting10s is the source default; the other limit is deliberately later.
    for (id, configured_ms, caller_seconds) in [
        ("configured-deadline-entered", 10000, 60),
        ("caller-deadline-entered", 60000, 10),
    ] {
        let transport = Arc::new(Entered {
            entered: tokio::sync::Notify::new(),
            dropped: Arc::new(AtomicUsize::new(0)),
            block: AtomicBool::new(true),
            calls: AtomicUsize::new(0),
        });
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(transport.clone()).unwrap();
        let mut c = config();
        c.timeout_ms = ConfigScalar::Integer(configured_ms);
        let registry = factory(http.clone(), Admission::new(AdmissionDecision::Allowed), c);
        let engine = registry.instantiate("dehashed").unwrap();
        let q = query();
        let start = Instant::now();
        let context = SearchContext {
            deadline: start + Duration::from_secs(caller_seconds),
        };
        let mut future = Box::pin(engine.search_with_context(&q, context));
        tokio::time::timeout(Duration::from_secs(3),async{tokio::select!{_ =transport.entered.notified()=>{},_ =&mut future=>panic!("deadline control must enter actual transport")}}).await.unwrap();
        assert_eq!(transport.dropped.load(Ordering::SeqCst), 0);
        let result = tokio::time::timeout(Duration::from_secs(15), &mut future)
            .await
            .expect("outer watchdog never extends actual limiting deadline");
        assert!(Instant::now() >= start + Duration::from_secs(10));
        whole(result, EngineStatus::Timeout, vec![], None);
        assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
        assert_eq!(transport.dropped.load(Ordering::SeqCst), 1);
        drop(future);
        whole(engine.search(&q).await, EngineStatus::Ok, vec![], None);
        assert_eq!(transport.calls.load(Ordering::SeqCst), 2);
        http.shutdown_and_wait().await;
        complete(id);
    }
}
#[tokio::test]
async fn resolved_settings_and_source_base_query_construction() {
    let raw = ssx_config::EngineConfig {
        base_url: " \t\n".into(),
        api_key: Secret::new(Some("synthetic-key".into())),
        max_results: ConfigScalar::Boolean(true),
        ..Default::default()
    };
    let resolved = DeHashedConfig::from_resolved(&raw, ENDPOINT);
    assert_eq!(resolved.base_url, ENDPOINT);
    assert_eq!(resolved.max_results, ConfigScalar::Boolean(true));
    assert!(resolved.api_key.is_configured());
    complete("resolved-config");
    let mut c = config();
    c.base_url = "http://127.0.0.1:1/base?discard=1#fragment".into();
    let (result, wire, _) =
        execute(c, query(), Reply::json(b"{}"), AdmissionDecision::Allowed).await;
    whole(result, EngineStatus::Ok, vec![], None);
    {
        let requests = wire.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url.as_str(),
            "http://127.0.0.1:1/base?q=person%40example.test&size=10#fragment/search"
        );
    }
    complete("source-base-query-fragment");
    let mut c = config();
    c.base_url.clear();
    let (result, wire, _) =
        execute(c, query(), Reply::json(SUCCESS), AdmissionDecision::Allowed).await;
    whole(
        result,
        EngineStatus::Error,
        vec![],
        Some("invalid DeHashed request URL"),
    );
    assert!(wire.requests.lock().unwrap().is_empty());
    complete("direct-empty-base");
}
#[tokio::test]
async fn unsupported_request_filters_do_not_change_wire_or_claim_media() {
    let mut q = query();
    q.language = "fr".into();
    q.time_range = Some("week".into());
    q.safesearch = 1;
    q.page = 9;
    q.media_type = Some("image".into());
    q.max_results = Some(1);
    q.date_from = Some("2024-01-01".into());
    let (result, wire, _) = execute(
        config(),
        q,
        Reply::json(SUCCESS),
        AdmissionDecision::Allowed,
    )
    .await;
    whole(result, EngineStatus::Ok, vec![success_row()], None);
    request(
        &wire,
        "person@example.test",
        "10",
        "reader@example.test:synthetic:key",
    );
    complete("ignored-request-fields");
}

#[test]
fn source_inventory_is_closed_and_bound_without_original_execution() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let default_headers = expected_default_headers([("accept", "application/json".into())]);
    assert_eq!(default_headers.len(), 2);
    assert_eq!(default_headers["accept"], "application/json");
    assert_eq!(
        default_headers["user-agent"],
        ssx_infra::http::DEFAULT_USER_AGENT
    );
    assert!(
        std::panic::catch_unwind(|| {
            expected_default_headers([("user-agent", "unreviewed-override".into())])
        })
        .is_err()
    );
    source_amendment::causal_successor::a06_source_successor::a13_header_layout_controls(&root)
        .unwrap();
    let hash = |data: &[u8]| {
        Sha256::digest(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let closed = |paths: &[&str], expected: &BTreeSet<&str>| {
        let actual: BTreeSet<_> = paths.iter().copied().collect();
        paths.len() == expected.len() && actual.len() == paths.len() && actual == *expected
    };
    let bytes =
        std::fs::read(root.join("docs/rust-rewrite/a13/dehashed-source-map-v1.json")).unwrap();
    assert_eq!(
        hash(&bytes),
        "226174da4be26ff9e902e73dc79a6d5344e5405abfae3a051ed19339a51a4b2d"
    );
    let map: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(map["kind"], "STATIC_SOURCE_DERIVATION_AND_NATIVE_COMPONENT");
    assert_eq!(map["python_executed"], false);
    assert_eq!(map["provider_calls"], 0);
    assert_eq!(map["source_runs"].as_array().unwrap().len(), 0);
    let expected_paths: BTreeSet<&str> = [
        "engines/dehashed.py",
        "tests/test_dehashed.py",
        "slopsearx/adapter.py",
        "slopsearx/config.py",
        "slopsearx/jev.py",
        "slopsearx/mcp/tools.py",
        "slopsearx/filters.py",
        "crates/ssx-infra/src/adapters/threat_intel.rs",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
        "crates/ssx-infra/src/adapters/source_json.rs",
        "crates/ssx-infra/src/adapters/source_repr.rs",
        "crates/ssx-infra/src/adapters/source_payload.rs",
        "crates/ssx-infra/src/http.rs",
        "crates/ssx-infra/src/search.rs",
        "crates/ssx-application/src/policy.rs",
        "crates/ssx-domain/src/records.rs",
        "crates/ssx-config/src/lib.rs",
        "crates/ssx-registry/src/lib.rs",
        "crates/ssx-owned-budget/src/lib.rs",
        "crates/ssx-infra/Cargo.toml",
        "Cargo.lock",
        "docs/rust-rewrite/f08/README.md",
        "docs/rust-rewrite/f05/README.md",
        "docs/rust-rewrite/f09/README.md",
        "crates/ssx-infra/src/adapters/mod.rs",
        "crates/ssx-infra/src/adapters/dehashed.rs",
        "docs/rust-rewrite/a13/dehashed-static-fixture-v1.json",
        "docs/rust-rewrite/a13/README.md",
        "docs/rust-rewrite/a13/PARITY.md",
        "docs/rust-rewrite/a13/BOUNDS_REVIEW.md",
        "docs/rust-rewrite/a13/PORTAL_IMPACT.md",
    ]
    .into_iter()
    .collect();
    let sources = map["sources"].as_array().unwrap();
    let paths: Vec<_> = sources
        .iter()
        .map(|source| source["path"].as_str().unwrap())
        .collect();
    let expected_container = "docs/rust-rewrite/a13/dehashed-source-map-v1.json";
    let expected_container_sha = "226174da4be26ff9e902e73dc79a6d5344e5405abfae3a051ed19339a51a4b2d";
    assert!(closed(&paths, &expected_paths));
    let mut duplicate = paths.clone();
    duplicate.push(paths[0]);
    assert!(!closed(&duplicate, &expected_paths));
    let mut unlisted = paths.clone();
    unlisted.push("crates/ssx-infra/src/not-in-the-frozen-map.rs");
    assert!(!closed(&unlisted, &expected_paths));
    let mut missing = paths.clone();
    missing.pop();
    assert!(!closed(&missing, &expected_paths));
    for source in sources {
        source_amendment::verify_current(
            &root,
            expected_container,
            expected_container_sha,
            source["path"].as_str().unwrap(),
            source["sha256"].as_str().unwrap(),
            source["bytes"].as_u64().map(|bytes| bytes as usize),
        )
        .unwrap();
    }
    source_amendment::causal_successor::control_bridge(&root).unwrap();
    let remaining_path = "docs/rust-rewrite/a13/remaining/source-map.json";
    let remaining_sha = "af359782abc772c727a885e66a87d519aea5393bcec31e0e105b2de46a8c5d22";
    let remaining = std::fs::read(root.join(remaining_path)).unwrap();
    assert_eq!(hash(&remaining), remaining_sha);
    let remaining_map: serde_json::Value = serde_json::from_slice(&remaining).unwrap();
    let remaining_http = remaining_map["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["path"] == "crates/ssx-infra/src/http.rs")
        .unwrap();
    source_amendment::verify_current(
        &root,
        remaining_path,
        remaining_sha,
        "crates/ssx-infra/src/http.rs",
        remaining_http["sha256"].as_str().unwrap(),
        remaining_http["bytes"].as_u64().map(|bytes| bytes as usize),
    )
    .unwrap();
    let originals = map["original_tests"].as_array().unwrap();
    assert_eq!(originals.len(), 4);
    let expected_originals: BTreeSet<&str> = [
        "test_search_returns_entries",
        "test_missing_api_key",
        "test_rate_limited",
        "test_adapter_registered",
    ]
    .into_iter()
    .collect();
    let mut names = BTreeSet::new();
    for row in originals {
        let name = row["function"].as_str().unwrap();
        assert!(names.insert(name));
        assert_eq!(row["source_executed"], false);
        let file = std::fs::read_to_string(root.join("tests/test_dehashed.py")).unwrap();
        assert!(file.contains(row["source_excerpt"].as_str().unwrap()));
    }
    assert_eq!(names, expected_originals);
    let expected_tests: BTreeSet<&str> = [
        "full_source_success_and_complete_outbound",
        "missing_key_and_admission_precedence",
        "http_status_classification_and_redaction",
        "basic_auth_first_colon_unicode_and_empty_components",
        "source_slices_and_request_limits_are_independent",
        "empty_and_wrong_shape_source_operations",
        "fallback_titles_literal_url_and_source_content_order",
        "used_prefixes_and_unused_unicode_values",
        "late_failure_is_atomic_and_operation_errors_precede_typed_title",
        "configured_invalid_inputs_and_codec_refusals_are_atomic",
        "registry_factory_preserves_all_catalog_metadata",
        "entered_exchange_cancellation_drops_actual_future_and_keeps_http_owner",
        "preexpired_caller_context_never_admits_or_dispatches",
        "genuine_factory_crosses_shared_policy_only_with_sensitive_grant",
        "entered_exchange_shutdown_joins_and_rejects_reuse",
        "private_pipe_failures_retain_actual_read_prefix",
        "private_literal_loopback_complete_http_contract",
        "actual_configured_and_caller_deadlines_drop_entered_exchange_without_retry",
        "resolved_settings_and_source_base_query_construction",
        "unsupported_request_filters_do_not_change_wire_or_claim_media",
        "source_inventory_is_closed_and_bound_without_original_execution",
    ]
    .into_iter()
    .collect();
    let native = map["native_tests"].as_array().unwrap();
    assert_eq!(native.len(), expected_tests.len());
    assert_eq!(
        native
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        expected_tests
    );
    let expected_cases: BTreeSet<&str> = [
        "source-success",
        "missing-key",
        "admission-denied-first",
        "admission-unavailable-first",
        "status-301",
        "status-401",
        "status-403",
        "status-404",
        "status-429",
        "status-500",
        "transport-timeout",
        "transport-error",
        "auth-colonless",
        "auth-first-colon",
        "auth-unicode",
        "auth-empty-user",
        "auth-empty-both",
        "config-debug-redacted",
        "auth-header-debug-redacted",
        "slice-zero",
        "slice-one",
        "slice-negative",
        "slice-negative-all",
        "slice-bool-true",
        "slice-bool-false",
        "slice-string-refused",
        "slice-float-refused",
        "slice-nan-refused",
        "missing-entries",
        "empty-entries",
        "empty-string",
        "sliced-string-empty",
        "nonempty-string",
        "null-entries",
        "mapping-entries",
        "outer-array",
        "outer-null",
        "scalar-row",
        "fallback-and-url",
        "password-tail-surrogate",
        "hash-tail-surrogate",
        "unused-name-surrogate",
        "unused-extra-surrogate",
        "array-password-slice",
        "selected-surrogate-refused",
        "late-row-atomic",
        "prefix-before-title",
        "subscription-before-index",
        "typed-title-after-content",
        "invalid-base",
        "zero-timeout",
        "text-timeout",
        "malformed-json",
        "byte-limit",
        "real-registration",
        "entered-cancel-and-reuse",
        "preexpired-context",
        "policy-generic",
        "policy-targeted",
        "policy-jobs",
        "policy-security",
        "policy-science",
        "policy-preview",
        "policy-authorized-real-factory",
        "entered-shutdown",
        "pipe-prefix-before-read-error",
        "pipe-prefix-before-cancel",
        "pipe-prefix-before-timeout",
        "literal-http-parent-exit",
        "configured-deadline-entered",
        "caller-deadline-entered",
        "resolved-config",
        "source-base-query-fragment",
        "direct-empty-base",
        "ignored-request-fields",
        "source-bindings-closed",
    ]
    .into_iter()
    .collect();
    let cases = map["case_ids"].as_array().unwrap();
    assert_eq!(cases.len(), expected_cases.len());
    assert_eq!(
        cases
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        expected_cases
    );
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("docs/rust-rewrite/a13/dehashed-static-fixture-v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        fixture["kind"],
        "STATIC_SOURCE_DERIVATION_NOT_PYTHON_OBSERVATION"
    );
    assert_eq!(fixture["body_json"].as_str().unwrap().as_bytes(), SUCCESS);
    assert_eq!(fixture["native_expected_content"], success_row().content);
    complete("source-bindings-closed");
}

// Keep the frozen selector line identities above stable; this shared verifier
// declaration is intentionally at the end of the module.
#[path = "../../../docs/rust-rewrite/a13/source-amendments/validator.rs"]
mod source_amendment;

// Keep the immutable A13 selector locations above stable. Header expectations
// still include the versioned native identity on every request assertion.
fn expected_default_headers<const N: usize>(
    entries: [(&'static str, String); N],
) -> BTreeMap<&'static str, String> {
    let mut headers = BTreeMap::from(entries);
    assert!(!headers.contains_key("user-agent"));
    headers.insert("user-agent", ssx_infra::http::DEFAULT_USER_AGENT.into());
    headers
}
