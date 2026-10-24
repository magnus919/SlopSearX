//! A13-owned deterministic fixtures. All identities and secrets are synthetic.
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission};
use ssx_config::Secret;
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use ssx_infra::{
    adapters::{
        hibp::{HibpConfig, register_hibp},
        intelx::{IntelXConfig, register_intelx},
    },
    http::{AdapterHttp, ExchangeFuture, Request, Response, Transport},
};
use ssx_registry::Registry;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[path = "../../../../docs/rust-rewrite/a13/source-amendments/validator.rs"]
mod source_amendment;
pub const KEY: &str = "synthetic-a13-key";
pub const HIBP_BODY: &[u8] = br#"[{"Name":"FabricatedBreach","Domain":"fabricated.example.test","BreachDate":"2024-01-02","PwnCount":1234567,"DataClasses":["Synthetic email","Synthetic password"],"Description":"Synthetic description."}]"#;
pub const INTELX_BODY: &[u8] = br#"{"records":[{"record":{"systemid":"synthetic-system","name":"Synthetic record","type":2,"date":"2024-02-03T00:00:00Z","preview":"Synthetic preview","bucket":"synthetic.bucket"}}]}"#;
#[derive(Clone)]
pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
    pub error: Option<ssx_infra::http::Error>,
    pub delay: Duration,
    pub block: bool,
    pub headers: reqwest::header::HeaderMap,
}
impl Reply {
    pub fn json(body: &[u8]) -> Self {
        Self {
            status: 200,
            body: body.to_vec(),
            error: None,
            delay: Duration::ZERO,
            block: false,
            headers: Default::default(),
        }
    }
    pub fn status(status: u16) -> Self {
        Self {
            status,
            ..Self::json(b"not JSON; synthetic diagnostic")
        }
    }
    pub fn delayed(body: &[u8], delay: Duration) -> Self {
        Self {
            delay,
            ..Self::json(body)
        }
    }
    pub fn blocked() -> Self {
        Self {
            block: true,
            ..Self::json(b"{}")
        }
    }
}
#[derive(Default)]
pub struct Queue {
    replies: Mutex<VecDeque<Reply>>,
    pub requests: Mutex<Vec<Request>>,
    pub entered_at: Mutex<Vec<Instant>>,
    pub terminal_at: Mutex<Vec<Option<Instant>>>,
    changed: tokio::sync::Notify,
}
impl Queue {
    pub fn new(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            ..Default::default()
        })
    }
    pub fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
    pub fn terminals(&self) -> usize {
        self.terminal_at
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v.is_some())
            .count()
    }
    pub async fn entered(&self, count: usize) {
        while self.calls() < count {
            self.changed.notified().await;
        }
    }
}
struct ExchangeDrop<'a>(&'a Queue, usize);
impl Drop for ExchangeDrop<'_> {
    fn drop(&mut self) {
        self.0.terminal_at.lock().unwrap()[self.1] = Some(Instant::now());
        self.0.changed.notify_one();
    }
}
impl Transport for Queue {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let url = request.url.clone();
            let id = {
                let mut requests = self.requests.lock().unwrap();
                let id = requests.len();
                requests.push(request);
                id
            };
            self.entered_at.lock().unwrap().push(Instant::now());
            self.terminal_at.lock().unwrap().push(None);
            let _actual_owner = ExchangeDrop(self, id);
            self.changed.notify_one();
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected provider dispatch");
            if reply.block {
                std::future::pending::<()>().await;
            }
            if !reply.delay.is_zero() {
                tokio::time::sleep(reply.delay).await;
            }
            if let Some(error) = reply.error {
                return Err(error);
            }
            Ok(Response {
                status: reply.status,
                body: reply.body,
                headers: reply.headers,
                url,
            })
        })
    }
}
pub struct Admission {
    pub decision: AdmissionDecision,
    pub names: Mutex<Vec<String>>,
}
impl Admission {
    pub fn new(decision: AdmissionDecision) -> Arc<Self> {
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
pub fn hibp_config() -> HibpConfig {
    let mut c = HibpConfig::defaults(ssx_infra::adapters::hibp::ENDPOINT);
    c.base_url = "http://127.0.0.1:1/injected".into();
    c.api_key = Secret::new(Some(KEY.into()));
    c
}
pub fn intelx_config() -> IntelXConfig {
    let mut c = IntelXConfig::defaults();
    c.base_url = "http://127.0.0.1:1/injected".into();
    c.api_key = Secret::new(Some(KEY.into()));
    c
}
pub fn query() -> SearchRequest {
    SearchRequest {
        query: "fiction@example.test".into(),
        ..Default::default()
    }
}
pub struct Harness {
    pub http: Arc<AdapterHttp>,
    pub wire: Arc<Queue>,
    pub admission: Arc<Admission>,
    pub registry: Arc<Registry>,
    pub engine: &'static str,
}
impl Harness {
    pub fn hibp(config: HibpConfig, replies: Vec<Reply>, decision: AdmissionDecision) -> Self {
        let wire = Queue::new(replies);
        let admission = Admission::new(decision);
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(wire.clone()).unwrap();
        let mut registry = Registry::builtins().unwrap();
        let before = registry.descriptors().cloned().collect::<Vec<_>>();
        register_hibp(&mut registry, http.clone(), admission.clone(), config).unwrap();
        assert_eq!(registry.descriptors().cloned().collect::<Vec<_>>(), before);
        Self {
            http,
            wire,
            admission,
            registry: Arc::new(registry),
            engine: "hibp",
        }
    }
    pub fn intelx(config: IntelXConfig, replies: Vec<Reply>, decision: AdmissionDecision) -> Self {
        let wire = Queue::new(replies);
        let admission = Admission::new(decision);
        let http = Arc::new(AdapterHttp::default());
        http.set_transport(wire.clone()).unwrap();
        let mut registry = Registry::builtins().unwrap();
        let before = registry.descriptors().cloned().collect::<Vec<_>>();
        register_intelx(&mut registry, http.clone(), admission.clone(), config).unwrap();
        assert_eq!(registry.descriptors().cloned().collect::<Vec<_>>(), before);
        Self {
            http,
            wire,
            admission,
            registry: Arc::new(registry),
            engine: "intelx",
        }
    }
    pub async fn run(&self, q: &SearchRequest) -> AdapterResponse {
        self.registry
            .instantiate(self.engine)
            .unwrap()
            .search(q)
            .await
    }
    pub async fn close(&self) {
        self.http.shutdown_and_wait().await;
        assert_eq!(self.wire.calls(), self.wire.terminals());
    }
}
pub fn row(
    engine: &str,
    url: &str,
    title: &str,
    content: &str,
    position: i64,
    score: f64,
    date: Option<&str>,
) -> SearchResult {
    SearchResult {
        engine: engine.into(),
        url: url.into(),
        title: title.into(),
        content: content.into(),
        position,
        score: FiniteF64::new(score).unwrap(),
        published_date: date.map(str::to_owned),
        ..Default::default()
    }
}
pub fn hibp_row() -> SearchResult {
    row(
        "hibp",
        "https://haveibeenpwned.com/breaches/FabricatedBreach",
        "FabricatedBreach",
        "Synthetic description. | Domain: fabricated.example.test | Accounts: 1,234,567 | Data: Synthetic email, Synthetic password",
        1,
        1.234567,
        Some("2024-01-02"),
    )
}
pub fn intelx_row() -> SearchResult {
    row(
        "intelx",
        "https://intelx.io/?did=synthetic-system",
        "Synthetic record",
        "Darknet | Synthetic preview | synthetic.bucket",
        1,
        0.,
        Some("2024-02-03"),
    )
}
pub fn whole(
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
pub fn codec_error(result: AdapterResponse) {
    let error = result
        .error_message
        .clone()
        .expect("redacted codec diagnostic");
    assert!(error.starts_with("invalid source JSON:"));
    assert!(!error.contains("provider-secret"));
    whole(result, EngineStatus::Error, vec![], Some(&error));
}
pub fn headers(request: &Request, engine: &str, post: bool, cookie: bool) {
    let mut expected = BTreeMap::from([
        (
            "accept",
            if engine == "hibp" {
                "application/vnd.haveibeenpwned.v3+json"
            } else {
                "application/json"
            },
        ),
        ("user-agent", "SlopSearX/0.1.0"),
        (
            if engine == "hibp" {
                "hibp-api-key"
            } else {
                "x-key"
            },
            KEY,
        ),
    ]);
    if post {
        expected.insert("content-type", "application/json");
    }
    if cookie {
        expected.insert("cookie", "a13=synthetic-cookie");
    }
    assert_eq!(
        request
            .headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.to_str().unwrap()))
            .collect::<BTreeMap<_, _>>(),
        expected
    );
    assert!(
        request.headers[if engine == "hibp" {
            "hibp-api-key"
        } else {
            "x-key"
        }]
        .is_sensitive()
    );
    assert!(!format!("{request:?}").contains(KEY));
}
pub fn hibp_wire(wire: &Queue, account: &str) {
    let requests = wire.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let r = &requests[0];
    assert_eq!(r.method, reqwest::Method::GET);
    assert_eq!(
        r.url.as_str(),
        format!("http://127.0.0.1:1/injected/breachedaccount/{account}")
    );
    assert!(r.body.is_empty());
    headers(r, "hibp", false, false);
}
pub fn intelx_wire(
    wire: &Queue,
    term: &str,
    max_json: &str,
    parameters: Option<Vec<(&str, &str)>>,
    cookie: bool,
) {
    let requests = wire.requests.lock().unwrap();
    assert_eq!(requests.len(), if parameters.is_some() { 2 } else { 1 });
    let first = &requests[0];
    assert_eq!(first.method, reqwest::Method::POST);
    assert_eq!(
        first.url.as_str(),
        "http://127.0.0.1:1/injected/intelligent/search"
    );
    headers(first, "intelx", true, false);
    let quoted = serde_json::to_string(term).unwrap();
    assert_eq!(
        first.body,
        format!("{{\"term\":{quoted},\"maxresults\":{max_json},\"target\":0,\"sort\":2}}")
            .as_bytes()
    );
    if let Some(parameters) = parameters {
        let second = &requests[1];
        assert_eq!(second.method, reqwest::Method::GET);
        assert_eq!(second.url.path(), "/injected/intelligent/result");
        assert_eq!(
            second
                .url
                .query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect::<Vec<_>>(),
            parameters
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect::<Vec<_>>()
        );
        assert!(second.body.is_empty());
        headers(second, "intelx", false, cookie);
        let entered = wire.entered_at.lock().unwrap();
        let terminal = wire.terminal_at.lock().unwrap();
        assert!(entered[1].duration_since(terminal[0].unwrap()) >= Duration::from_secs(2));
    }
}
pub fn id_reply() -> Reply {
    Reply::json(br#"{"id":"synthetic-search"}"#)
}
pub fn default_parameters() -> Vec<(&'static str, &'static str)> {
    vec![("id", "synthetic-search"), ("limit", "10")]
}
pub fn complete(id: &str) {
    println!("A13_REMAINING_CASE_COMPLETE {id}");
}

fn service(sensitive_allowed: bool, registry: Arc<Registry>) -> ssx_infra::search::SearchService {
    use ssx_application::{
        NoopSink,
        cache::RoutingIdentity,
        policy::{PolicyService, SearchPolicy},
        scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
    };
    use ssx_infra::search::{
        EngineBinding, SearchConfig, SearchDependencies, SearchService, SystemSearchClock,
    };
    let mut configuration = ssx_config::load_config_with(
        Some(std::path::Path::new("/no/a13/remaining-fixture-config")),
        &BTreeMap::new(),
    )
    .unwrap();
    for name in ["hibp", "intelx"] {
        let configured = configuration.engines.get_mut(name).unwrap();
        configured.enabled = true;
        configured.api_key = Secret::new(Some(KEY.into()));
    }
    let engines = ["hibp", "intelx"]
        .into_iter()
        .map(|name| {
            (
                name.into(),
                EngineBinding {
                    engine: Arc::from(registry.instantiate(name).unwrap()),
                    generation: "a13-remaining-source-v1".into(),
                    timeout: Duration::from_secs(15),
                    scrape: false,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
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
    assert!(policy.policy.sensitive_engines.contains("hibp"));
    assert!(!policy.policy.sensitive_engines.contains("intelx"));
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
pub async fn policy() {
    use ssx_application::policy::{ScopeProposal, SearchSurface};
    use ssx_infra::search::SearchError;
    let wire = Queue::new(vec![
        id_reply(),
        Reply::json(INTELX_BODY),
        Reply::json(HIBP_BODY),
    ]);
    let admission = Admission::new(AdmissionDecision::Allowed);
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(wire.clone()).unwrap();
    let mut registry = Registry::builtins().unwrap();
    register_hibp(
        &mut registry,
        http.clone(),
        admission.clone(),
        hibp_config(),
    )
    .unwrap();
    register_intelx(
        &mut registry,
        http.clone(),
        admission.clone(),
        intelx_config(),
    )
    .unwrap();
    let registry = Arc::new(registry);
    assert!(!registry.is_available("dehashed"));
    let denied = service(false, registry.clone());
    for surface in [
        SearchSurface::Generic,
        SearchSurface::Targeted,
        SearchSurface::Jobs,
        SearchSurface::Security,
        SearchSurface::Science,
        SearchSurface::Preview,
    ] {
        for names in [vec!["hibp".into()], vec!["hibp".into(), "intelx".into()]] {
            let mut q = query();
            q.engines = Some(names);
            let mut p = ScopeProposal::generic(q);
            p.surface = surface;
            let result = if surface == SearchSurface::Preview {
                denied.preview_scope(&p).await.map(|_| ())
            } else {
                denied.search(p).await.map(|_| ())
            };
            let Err(SearchError::Policy(error)) = result else {
                panic!("sensitive or mixed scope must fail atomically")
            };
            assert_eq!(error.code, "tool_disabled");
            assert_eq!(error.field, "engines");
            assert_eq!(
                *error.details,
                ssx_json::json!({"engines":["hibp"],"grant":"MCP_TARGETED_SENSITIVE_ALLOWED"})
            );
            assert_eq!(wire.calls(), 0);
            assert!(admission.names.lock().unwrap().is_empty());
        }
    }
    let recovered = ssx_domain::research::ResearchQuery {
        query: query().query,
        engines: vec!["hibp".into(), "intelx".into()],
        intent: "security".into(),
        ..Default::default()
    };
    assert!(matches!(
        denied
            .search(ScopeProposal::from_research_query(&recovered))
            .await,
        Err(SearchError::Policy(_))
    ));
    assert_eq!(wire.calls(), 0);
    let mut q = query();
    q.engines = Some(vec!["intelx".into()]);
    let response = denied.search(ScopeProposal::generic(q)).await.unwrap();
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].engine, "intelx");
    assert_eq!(response.engine_outcomes[0].status, "ok");
    assert_eq!(wire.calls(), 2);
    intelx_wire(
        &wire,
        "fiction@example.test",
        "10",
        Some(default_parameters()),
        false,
    );
    denied.shutdown().await;
    let allowed = service(true, registry);
    let mut q = query();
    q.engines = Some(vec!["hibp".into()]);
    let response = allowed.search(ScopeProposal::generic(q)).await.unwrap();
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].engine, "hibp");
    assert_eq!(response.engine_outcomes[0].status, "ok");
    allowed.shutdown().await;
    assert_eq!(*admission.names.lock().unwrap(), vec!["intelx", "hibp"]);
    assert!(
        http.session(
            Default::default(),
            tokio::time::Instant::now() + Duration::from_secs(1),
            Default::default()
        )
        .is_ok()
    );
    http.shutdown_and_wait().await;
    assert_eq!(wire.calls(), wire.terminals());
    let fresh = Harness::intelx(
        intelx_config(),
        vec![Reply::json(b"{}")],
        AdmissionDecision::Allowed,
    );
    whole(fresh.run(&query()).await, EngineStatus::Ok, vec![], None);
    intelx_wire(&fresh.wire, "fiction@example.test", "10", None, false);
    fresh.close().await;
}

#[cfg(unix)]
pub async fn literal(engine: &str, selector: &str) {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use std::{io::Read, os::unix::fs::DirBuilderExt, process::Stdio};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        process::Command,
    };
    const FLAG: &str = "SSX_A13_REMAINING_PRIVATE_CHILD";
    if std::env::var_os(FLAG).is_none() {
        let mut nonce = [0u8; 8];
        std::fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut nonce)
            .unwrap();
        let private = std::env::temp_dir().join(format!(
            "ssx-a13-{engine}-{}-{:016x}",
            std::process::id(),
            u64::from_ne_bytes(nonce)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&private)
            .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", selector, "--nocapture"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &private)
            .env("TMPDIR", &private)
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env(FLAG, engine)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        async fn drain<R: tokio::io::AsyncRead + Unpin>(
            mut reader: R,
            bytes: Arc<Mutex<Vec<u8>>>,
        ) -> (&'static str, bool) {
            let mut buffer = [0u8; 16384];
            loop {
                match reader.read(&mut buffer).await {
                    Ok(0) => return ("eof", true),
                    Ok(n) => {
                        let mut output = bytes.lock().unwrap();
                        let take = n.min((1024 * 1024 + 1usize).saturating_sub(output.len()));
                        output.extend_from_slice(&buffer[..take]);
                        if output.len() > 1024 * 1024 {
                            return ("limit", false);
                        }
                    }
                    Err(_) => return ("read-error", false),
                }
            }
        }
        let output = Arc::new(Mutex::new(Vec::new()));
        let errors = Arc::new(Mutex::new(Vec::new()));
        let mut out = tokio::spawn(drain(child.stdout.take().unwrap(), output.clone()));
        let mut err = tokio::spawn(drain(child.stderr.take().unwrap(), errors.clone()));
        let (status, killed) =
            match tokio::time::timeout(Duration::from_secs(20), child.wait()).await {
                Ok(s) => (s, false),
                Err(_) => {
                    let _ = child.kill().await;
                    (child.wait().await, true)
                }
            };
        async fn joined(
            task: &mut tokio::task::JoinHandle<(&'static str, bool)>,
        ) -> (&'static str, bool) {
            match tokio::time::timeout(Duration::from_secs(1), &mut *task).await {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => ("join-error", false),
                Err(_) => {
                    task.abort();
                    let _ = task.await;
                    ("drain-timeout", false)
                }
            }
        }
        let (outcome_out, eof_out) = joined(&mut out).await;
        let (outcome_err, eof_err) = joined(&mut err).await;
        let output = output.lock().unwrap().clone();
        let errors = errors.lock().unwrap().clone();
        println!(
            "A13_REMAINING_CHILD_STDOUT_BASE64 {}",
            STANDARD.encode(&output)
        );
        println!(
            "A13_REMAINING_CHILD_STDERR_BASE64 {}",
            STANDARD.encode(&errors)
        );
        println!(
            "A13_REMAINING_CHILD_LIFECYCLE {}",
            serde_json::json!({"engine":engine,"status_code":status.as_ref().ok().and_then(|s|s.code()),"wait_succeeded":status.is_ok(),"success":status.as_ref().is_ok_and(|s|s.success()),"forced_kill":killed,"stdout_outcome":outcome_out,"stderr_outcome":outcome_err,"stdout_eof":eof_out,"stderr_eof":eof_err,"stdout_bytes":output.len(),"stderr_bytes":errors.len()})
        );
        std::fs::remove_dir(&private).unwrap();
        assert!(!killed);
        assert!(eof_out && eof_err);
        assert!(output.len() + errors.len() <= 1024 * 1024);
        assert!(
            status.unwrap().success(),
            "private child failed; exact bounded raw prefixes retained as Base64"
        );
        let text = String::from_utf8(output).unwrap();
        assert_eq!(
            text.lines()
                .filter(|line| *line == format!("A13_REMAINING_PRIVATE_CHILD_COMPLETE {engine}"))
                .count(),
            1
        );
        assert!(text.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"));
        return;
    }
    assert_eq!(std::env::var(FLAG).unwrap(), engine);
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
        assert!(std::env::var_os(name).is_none());
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
    let address = listener.local_addr().unwrap();
    let is_hibp = engine == "hibp";
    let mut server = tokio::spawn(async move {
        let mut captures = Vec::new();
        for index in 0..if is_hibp { 1 } else { 2 } {
            let (mut stream, _) = listener.accept().await?;
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 1024];
            let header_end = loop {
                if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
                let n = stream.read(&mut buffer).await?;
                if n == 0 {
                    return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
                }
                bytes.extend_from_slice(&buffer[..n]);
                if bytes.len() > 16384 {
                    return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
                }
            };
            let header = std::str::from_utf8(&bytes[..header_end])
                .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
            let length = header
                .split("\r\n")
                .filter_map(|line| line.split_once(':'))
                .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                .map(|(_, v)| v.trim().parse::<usize>())
                .transpose()
                .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?
                .unwrap_or(0);
            if header_end + length > 16384 {
                return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
            }
            while bytes.len() < header_end + length {
                let n = stream.read(&mut buffer).await?;
                if n == 0 {
                    return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
                }
                bytes.extend_from_slice(&buffer[..n]);
            }
            let received = Instant::now();
            let body = if is_hibp {
                HIBP_BODY
            } else if index == 0 {
                br#"{"id":"synthetic-search"}"#
            } else {
                INTELX_BODY
            };
            let cookie = if !is_hibp && index == 0 {
                "Set-Cookie: a13=synthetic-cookie; Path=/\r\n"
            } else {
                ""
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{cookie}Connection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).await?;
            stream.write_all(body).await?;
            stream.shutdown().await?;
            captures.push((bytes, received));
        }
        Ok::<_, std::io::Error>(captures)
    });
    let http = Arc::new(AdapterHttp::default());
    let admission = Admission::new(AdmissionDecision::Allowed);
    let mut registry = Registry::builtins().unwrap();
    if is_hibp {
        let mut c = hibp_config();
        c.base_url = format!("http://{address}/api");
        register_hibp(&mut registry, http.clone(), admission, c).unwrap();
    } else {
        let mut c = intelx_config();
        c.base_url = format!("http://{address}/api");
        register_intelx(&mut registry, http.clone(), admission, c).unwrap();
    }
    let result = registry.instantiate(engine).unwrap().search(&query()).await;
    let captures = match tokio::time::timeout(Duration::from_secs(4), &mut server).await {
        Ok(result) => result.unwrap(),
        Err(_) => {
            server.abort();
            let _ = server.await;
            Err(std::io::Error::from(std::io::ErrorKind::TimedOut))
        }
    };
    http.shutdown_and_wait().await;
    let captures = captures.expect("all owned literal server exchanges must join");
    whole(
        result,
        EngineStatus::Ok,
        vec![if is_hibp { hibp_row() } else { intelx_row() }],
        None,
    );
    for (index, (bytes, _)) in captures.iter().enumerate() {
        let wire = std::str::from_utf8(bytes).unwrap();
        let (header, body) = wire.split_once("\r\n\r\n").unwrap();
        let mut lines = header.split("\r\n");
        let expected = if is_hibp {
            "GET /api/breachedaccount/fiction@example.test HTTP/1.1"
        } else if index == 0 {
            "POST /api/intelligent/search HTTP/1.1"
        } else {
            "GET /api/intelligent/result?id=synthetic-search&limit=10 HTTP/1.1"
        };
        assert_eq!(lines.next(), Some(expected));
        let headers = lines
            .map(|line| line.split_once(':').unwrap())
            .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(headers.get("host"), Some(&address.to_string()));
        assert_eq!(
            headers
                .get(if is_hibp { "hibp-api-key" } else { "x-key" })
                .map(String::as_str),
            Some(KEY)
        );
        assert_eq!(
            headers.get("user-agent").map(String::as_str),
            Some("SlopSearX/0.1.0")
        );
        assert_eq!(
            headers.get("accept").map(String::as_str),
            Some(if is_hibp {
                "application/vnd.haveibeenpwned.v3+json"
            } else {
                "application/json"
            })
        );
        if !is_hibp && index == 0 {
            assert_eq!(
                body,
                r#"{"term":"fiction@example.test","maxresults":10,"target":0,"sort":2}"#
            );
        } else {
            assert!(body.is_empty());
        }
        if !is_hibp && index == 1 {
            assert_eq!(
                headers.get("cookie").map(String::as_str),
                Some("a13=synthetic-cookie")
            );
            assert!(captures[1].1.duration_since(captures[0].1) >= Duration::from_secs(2));
        }
    }
    println!("A13_REMAINING_PRIVATE_CHILD_COMPLETE {engine}");
}

pub fn inventory(engine: &str) {
    use sha2::{Digest, Sha256};
    use std::collections::BTreeSet;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    source_amendment::causal_successor::control_bridge(&root).unwrap();
    let bytes =
        std::fs::read(root.join("docs/rust-rewrite/a13/remaining/source-map.json")).unwrap();
    let digest = |bytes: &[u8]| {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    assert_eq!(
        digest(&bytes),
        "9f75190c18606f2e9416e4db9a99392c7cb9aa0836dc3ff41a56263f05af975f"
    );
    let map: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        map["kind"],
        "STATIC_SOURCE_DERIVATION_AND_PROPOSED_NATIVE_SLICE"
    );
    assert_eq!(map["python_executed"], false);
    assert_eq!(map["provider_calls"], 0);
    assert!(map["source_runs"].as_array().unwrap().is_empty());
    let expected: BTreeSet<&str> = [
        "Cargo.lock",
        "contracts/unicode/provenance.json",
        "contracts/unicode/python-text-source/Include__cpython__unicodeobject.h.txt",
        "contracts/unicode/python-text-source/Include__patchlevel.h.txt",
        "contracts/unicode/python-text-source/LICENSE.txt",
        "contracts/unicode/python-text-source/Modules___sre__sre.c.txt",
        "contracts/unicode/python-text-source/Modules__unicodedata_db.h.txt",
        "contracts/unicode/python-text-source/Objects__unicodectype.c.txt",
        "contracts/unicode/python-text-source/Objects__unicodeobject.c.txt",
        "contracts/unicode/python-text-source/Objects__unicodetype_db.h.txt",
        "crates/ssx-application/src/policy.rs",
        "crates/ssx-config/src/lib.rs",
        "crates/ssx-domain/src/records.rs",
        "crates/ssx-infra/Cargo.toml",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
        "crates/ssx-infra/src/adapters/crates_io.rs",
        "crates/ssx-infra/src/adapters/hibp.rs",
        "crates/ssx-infra/src/adapters/intelx.rs",
        "crates/ssx-infra/src/adapters/mod.rs",
        "crates/ssx-infra/src/adapters/source_json.rs",
        "crates/ssx-infra/src/adapters/source_payload.rs",
        "crates/ssx-infra/src/adapters/source_repr.rs",
        "crates/ssx-infra/src/adapters/threat_intel.rs",
        "crates/ssx-infra/src/http.rs",
        "crates/ssx-infra/src/search.rs",
        "crates/ssx-json/Cargo.toml",
        "crates/ssx-json/src/lib.rs",
        "crates/ssx-json/src/python_text.rs",
        "crates/ssx-json/src/python_text_data.rs",
        "crates/ssx-owned-budget/src/lib.rs",
        "crates/ssx-registry/src/lib.rs",
        "crates/ssx-source-codecs/provenance/idna/inputs/SpecialCasing-16.0.0.txt",
        "crates/ssx-source-codecs/provenance/idna/inputs/UnicodeData-16.0.0.txt",
        "crates/xtask/src/a09.rs",
        "crates/xtask/src/a11.rs",
        "docs/rust-rewrite/CASE_STUDY.md",
        "docs/rust-rewrite/a13/remaining/BOUNDS_REVIEW.md",
        "docs/rust-rewrite/a13/remaining/PARITY.md",
        "docs/rust-rewrite/a13/remaining/PORTAL_IMPACT.md",
        "docs/rust-rewrite/a13/remaining/README.md",
        "docs/rust-rewrite/a13/remaining/cpython-input-pins.json",
        "docs/rust-rewrite/a13/remaining/native-plan.json",
        "docs/rust-rewrite/a13/remaining/original-tests.json",
        "docs/rust-rewrite/a13/remaining/unicode-derivation-proof.json",
        "docs/rust-rewrite/f05/README.md",
        "docs/rust-rewrite/f06/PARITY.md",
        "docs/rust-rewrite/f06/fragments/hibp.json",
        "docs/rust-rewrite/f06/fragments/intelx.json",
        "docs/rust-rewrite/f08/README.md",
        "docs/rust-rewrite/f08/parity.md",
        "docs/rust-rewrite/f09/README.md",
        "engines/dehashed.py",
        "engines/hibp.py",
        "engines/intelx.py",
        "slopsearx/adapter.py",
        "slopsearx/config.py",
        "slopsearx/filters.py",
        "slopsearx/jev.py",
        "slopsearx/mcp/tools.py",
        "tests/test_dehashed.py",
        "tests/test_hibp.py",
        "tests/test_intelx.py",
        "tests/test_mcp_security.py",
    ]
    .into_iter()
    .collect();
    let mut seen = BTreeSet::new();
    for source in map["sources"].as_array().unwrap() {
        let p = source["path"].as_str().unwrap();
        assert!(seen.insert(p));
        source_amendment::verify_current(
            &root,
            "docs/rust-rewrite/a13/remaining/source-map.json",
            "af359782abc772c727a885e66a87d519aea5393bcec31e0e105b2de46a8c5d22",
            p,
            source["sha256"].as_str().unwrap(),
            source["bytes"].as_u64().map(|bytes| bytes as usize),
        )
        .unwrap();
    }
    assert_eq!(seen, expected);
    let originals = map["original_tests"].as_array().unwrap();
    assert_eq!(originals.len(), 8);
    for row in originals {
        assert_eq!(row["source_executed"], false);
        let text =
            std::fs::read_to_string(root.join(row["source_file"].as_str().unwrap())).unwrap();
        assert!(text.contains(row["source_excerpt"].as_str().unwrap()));
    }
    let expected: BTreeSet<&str> = match engine {
        "hibp" => [
            "hibp_source_success_complete_outbound",
            "hibp_404_empty_before_json",
            "hibp_missing_key_and_admission_precedence",
            "hibp_auth_redaction_and_resolved_configuration",
            "hibp_email_first_match_word_boundaries_and_case",
            "hibp_fallback_strip_unicode16_lowercase",
            "hibp_description_prefix_and_source_type_errors",
            "hibp_grouped_count_numeric_and_error_order",
            "hibp_data_classes_slice_and_text_join",
            "hibp_source_operations_before_typed_projection",
            "hibp_dates_unsliced_and_positions",
            "hibp_wrong_shapes_and_late_atomicity",
            "hibp_configured_max_and_request_filters_ignored",
            "hibp_http_transport_url_and_codec_failures",
            "hibp_entered_cancel_and_http_reuse",
            "hibp_entered_shutdown_rejects_reuse",
            "hibp_caller_and_configured_deadlines",
            "hibp_real_policy_and_mixed_intelx_atomicity",
            "hibp_private_literal_http_contract",
            "hibp_static_source_inventory_closed",
        ]
        .into_iter()
        .collect(),
        "intelx" => [
            "intelx_success_two_requests_exact_delay",
            "intelx_falsey_ids_stop_before_delay",
            "intelx_start_status_classification",
            "intelx_result_statuses_are_generic_errors",
            "intelx_start_shape_and_json_errors",
            "intelx_id_parameter_source_type_semantics",
            "intelx_post_json_scalar_order_and_nonfinite_refusal",
            "intelx_configured_slices_and_invalid_indices",
            "intelx_result_shape_normalization_before_slice",
            "intelx_wrapped_skipped_rows_preserve_positions",
            "intelx_type_label_key_semantics_and_precedence",
            "intelx_preview_bucket_and_title_operation_order",
            "intelx_id_url_fallback_and_dates",
            "intelx_late_failure_and_quota_atomicity",
            "intelx_header_redaction_settings_and_filters",
            "intelx_one_session_cookie_continuity",
            "intelx_fresh_phase_timeout_excludes_source_delay",
            "intelx_caller_context_bounds_each_phase_and_delay",
            "intelx_cancel_post_delay_and_get",
            "intelx_shutdown_entered_and_delayed_no_dispatch",
            "intelx_private_literal_two_phase_http",
            "intelx_static_source_inventory_closed",
        ]
        .into_iter()
        .collect(),
        _ => panic!("unknown engine"),
    };
    let actual = map["native_tests"][engine]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
}
