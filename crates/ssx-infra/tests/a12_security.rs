//! A12 native components; fixture answers are static derivations, not Python runs.
use ssx_infra::adapters::{http_charset, http_headers, source_json, source_payload, source_repr};
pub use ssx_infra::http;
#[path = "../src/adapters/censys.rs"]
mod censys;
#[path = "../src/adapters/crtsh.rs"]
mod crtsh;
#[path = "../src/adapters/infrastructure_security.rs"]
mod infrastructure_security;
#[path = "../src/adapters/shodan.rs"]
mod shodan;
#[path = "../src/adapters/virustotal.rs"]
mod virustotal;
#[path = "../src/adapters/vulncheck.rs"]
mod vulncheck;
use http::{AdapterHttp, ExchangeFuture, Request, Response, Transport};
use infrastructure_security::{
    Kind, SecurityConfig, SecuritySettings, register_infrastructure_security,
};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission};
use ssx_domain::{EngineStatus, SearchRequest};
use std::sync::{Arc, Mutex};
struct Admission {
    result: AdmissionDecision,
    calls: Mutex<Vec<String>>,
}
impl SearchAdmission for Admission {
    fn admit<'a>(&'a self, name: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(name.into());
            self.result
        })
    }
}
struct Queue {
    body: Vec<u8>,
    status: u16,
    content_type: Option<String>,
    calls: Mutex<Vec<Request>>,
}
impl Transport for Queue {
    fn exchange(&self, req: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let url = req.url.clone();
            self.calls.lock().unwrap().push(req);
            let mut headers = reqwest::header::HeaderMap::new();
            if let Some(ct) = &self.content_type {
                headers.insert("content-type", ct.parse().unwrap());
            }
            Ok(Response {
                body: self.body.clone(),
                status: self.status,
                url,
                headers,
            })
        })
    }
}
const KINDS: [Kind; 5] = [
    Kind::Censys,
    Kind::Shodan,
    Kind::VirusTotal,
    Kind::VulnCheck,
    Kind::CrtSh,
];
fn settings() -> SecuritySettings {
    let mut s = SecuritySettings::default();
    for c in [
        &mut s.censys,
        &mut s.shodan,
        &mut s.virustotal,
        &mut s.vulncheck,
        &mut s.crtsh,
    ] {
        c.api_key = ssx_config::Secret::new(Some("synthetic-key".into()));
        c.api_secret = ssx_config::Secret::new(Some("synthetic-secret".into()));
    }
    s
}
fn config(s: &mut SecuritySettings, kind: Kind) -> &mut SecurityConfig {
    match kind {
        Kind::Censys => &mut s.censys,
        Kind::Shodan => &mut s.shodan,
        Kind::VirusTotal => &mut s.virustotal,
        Kind::VulnCheck => &mut s.vulncheck,
        Kind::CrtSh => &mut s.crtsh,
    }
}
fn fixture(kind: Kind) -> serde_json::Value {
    let fixtures: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a12/static-fixtures-v1.json"
    ))
    .unwrap();
    assert_eq!(fixtures["provenance"], "STATIC_ORIGINAL_TEST_DERIVATION");
    assert_eq!(fixtures["source_executed"], false);
    fixtures["payloads"][kind.name()].clone()
}
async fn execute(
    kind: Kind,
    body: Vec<u8>,
    status: u16,
    ct: Option<&str>,
    settings: SecuritySettings,
    q: SearchRequest,
) -> (ssx_domain::AdapterResponse, Arc<Queue>, Arc<Admission>) {
    let http = Arc::new(AdapterHttp::default());
    let wire = Arc::new(Queue {
        body,
        status,
        content_type: ct.map(str::to_owned),
        calls: Mutex::new(Vec::new()),
    });
    http.set_transport(wire.clone()).unwrap();
    let admission = Arc::new(Admission {
        result: AdmissionDecision::Allowed,
        calls: Mutex::new(Vec::new()),
    });
    let mut registry = ssx_registry::Registry::builtins().unwrap();
    register_infrastructure_security(&mut registry, http.clone(), admission.clone(), settings)
        .unwrap();
    let engine = registry.instantiate(kind.name()).unwrap();
    let response = engine.search(&q).await;
    http.shutdown_and_wait().await;
    (response, wire, admission)
}
#[test]
fn resolved_config_normalization_and_secret_debug() {
    let c = ssx_config::Config::default();
    let settings = SecuritySettings::from_resolved(&c);
    assert_eq!(settings.crtsh.base_url, Kind::CrtSh.base());
    let mut settings = settings;
    config(&mut settings, Kind::Shodan).api_key =
        ssx_config::Secret::new(Some("sensitive-a12-value".into()));
    assert!(!format!("{settings:?}").contains("sensitive-a12-value"));
}
fn query(kind: Kind) -> SearchRequest {
    SearchRequest {
        query: if kind == Kind::VulnCheck {
            "CVE-2024-3094"
        } else {
            "example query"
        }
        .into(),
        ..Default::default()
    }
}
fn row(
    kind: Kind,
    url: &str,
    title: &str,
    content: &str,
    score: f64,
    position: i64,
    date: Option<&str>,
) -> ssx_domain::SearchResult {
    ssx_domain::SearchResult {
        url: url.into(),
        title: title.into(),
        content: content.into(),
        engine: kind.name().into(),
        score: ssx_domain::FiniteF64::new(score).unwrap(),
        position,
        published_date: date.map(str::to_owned),
        ..Default::default()
    }
}
fn expected(kind: Kind) -> Vec<ssx_domain::SearchResult> {
    match kind {
        Kind::Censys => vec![row(
            kind,
            "https://search.censys.io/hosts/1.2.3.4",
            "1.2.3.4",
            "HTTP:80 | Mountain View, US",
            0.,
            1,
            None,
        )],
        Kind::Shodan => vec![row(
            kind,
            "https://www.shodan.io/host/1.2.3.4",
            "1.2.3.4:80",
            "nginx | tcp | Example ISP | www.example.com",
            0.,
            1,
            None,
        )],
        Kind::VirusTotal => vec![row(
            kind,
            "https://virustotal.com/gui/search/d41d8cd98f00b204e9800998ecf8427e",
            "bad.exe",
            "Detection: 5/67 engines (2 suspicious) | Reputation: -5 | VT link: https://virustotal.com/gui/search/d41d8cd98f00b204e9800998ecf8427e",
            5.,
            1,
            None,
        )],
        Kind::VulnCheck => vec![row(
            kind,
            "https://api.vulncheck.com/v3/community/cve/CVE-2024-3094",
            "CVE-2024-3094",
            "Exploit State: exploited | Added: 2024-04-01 | Affected: apache | Exploits: 1 available",
            0.,
            1,
            None,
        )],
        Kind::CrtSh => vec![
            row(
                kind,
                "https://crt.sh/?id=12345",
                "example.com",
                "Issuer: C=US, O=Let's Encrypt | SANs: example.com | Valid: 2024-01-01 | Expires: 2025-01-01",
                0.,
                1,
                Some("2024-01-01"),
            ),
            row(
                kind,
                "https://crt.sh/?id=12346",
                "test.org",
                "Issuer: C=US, O=DigiCert | SANs: test.org | Valid: 2024-06-01 | Expires: 2025-06-01",
                0.,
                2,
                Some("2024-06-01"),
            ),
        ],
    }
}
fn whole(
    mut actual: ssx_domain::AdapterResponse,
    status: EngineStatus,
    rows: Vec<ssx_domain::SearchResult>,
    message: Option<&str>,
) {
    assert!(actual.latency_ms.get().is_finite() && actual.latency_ms.get() >= 0.);
    actual.latency_ms = Default::default();
    assert_eq!(
        actual,
        ssx_domain::AdapterResponse {
            results: rows,
            status,
            error_message: message.map(str::to_owned),
            ..Default::default()
        }
    );
}
fn assert_request(kind: Kind, r: &Request, q: &str, limit: &str, key: &str) {
    assert_eq!(r.method, reqwest::Method::GET);
    assert!(r.body.is_empty());
    let (suffix, params) = match kind {
        Kind::Censys => ("/hosts/search", vec![("q", q), ("per_page", limit)]),
        Kind::Shodan => ("/shodan/host/search", vec![("query", q), ("limit", limit)]),
        Kind::VirusTotal => ("/search", vec![("query", q), ("limit", limit)]),
        Kind::VulnCheck => ("/community/cve/CVE-2024-3094", vec![]),
        Kind::CrtSh => ("/", vec![("q", q)]),
    };
    let mut expected = reqwest::Url::parse(&format!("{}{suffix}", kind.base())).unwrap();
    if !params.is_empty() {
        expected.query_pairs_mut().extend_pairs(params);
    }
    assert_eq!(r.url, expected);
    let mut headers = reqwest::header::HeaderMap::new();
    if matches!(kind, Kind::Censys | Kind::VirusTotal | Kind::VulnCheck) {
        headers.insert("accept", "application/json".parse().unwrap());
    }
    match kind {
        Kind::Censys => {
            use base64::Engine as _;
            headers.insert(
                "authorization",
                format!(
                    "Basic {}",
                    base64::engine::general_purpose::STANDARD
                        .encode(format!("{key}:synthetic-secret"))
                )
                .parse()
                .unwrap(),
            );
        }
        Kind::Shodan | Kind::VulnCheck => {
            headers.insert("authorization", format!("Bearer {key}").parse().unwrap());
        }
        Kind::VirusTotal => {
            headers.insert("x-apikey", key.parse().unwrap());
        }
        Kind::CrtSh => {}
    }
    assert_eq!(r.headers, headers);
}
#[tokio::test]
async fn all_five_complete_results_and_exact_requests() {
    for kind in KINDS {
        let q = query(kind);
        let (result, wire, admission) = execute(
            kind,
            serde_json::to_vec(&fixture(kind)).unwrap(),
            200,
            Some("application/json"),
            settings(),
            q.clone(),
        )
        .await;
        whole(result, EngineStatus::Ok, expected(kind), None);
        let calls = wire.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_request(kind, &calls[0], &q.query, "10", "synthetic-key");
        assert_eq!(*admission.calls.lock().unwrap(), vec![kind.name()]);
        println!(
            "A12_STATIC_CASE {}.complete_output_and_request",
            kind.name()
        );
    }
}
#[tokio::test]
async fn original_named_auth_errors_have_no_outbound_and_admit_first() {
    for kind in KINDS.into_iter().filter(|k| *k != Kind::CrtSh) {
        for missing_secret in [false, true] {
            if missing_secret && kind != Kind::Censys {
                continue;
            }
            let mut s = settings();
            if missing_secret {
                config(&mut s, kind).api_secret = ssx_config::Secret::new(None);
            } else {
                config(&mut s, kind).api_key = ssx_config::Secret::new(None);
            }
            let (response, wire, admission) =
                execute(kind, b"not JSON".to_vec(), 200, None, s, query(kind)).await;
            assert_eq!(response.status, EngineStatus::Error);
            assert!(
                response
                    .error_message
                    .as_ref()
                    .unwrap()
                    .contains("not configured")
            );
            assert!(response.results.is_empty());
            assert!(wire.calls.lock().unwrap().is_empty());
            assert_eq!(*admission.calls.lock().unwrap(), vec![kind.name()]);
        }
    }
    for key in ["", " \t\n", "\u{2003}\u{85}"] {
        let mut s = settings();
        config(&mut s, Kind::Shodan).api_key = ssx_config::Secret::new(Some(key.into()));
        let (r, w, _) = execute(
            Kind::Shodan,
            b"{}".to_vec(),
            200,
            None,
            s,
            query(Kind::Shodan),
        )
        .await;
        whole(
            r,
            EngineStatus::Error,
            vec![],
            Some("Shodan API key not configured (set ENGINE_SHODAN_API_KEY)"),
        );
        assert!(w.calls.lock().unwrap().is_empty());
    }
    let mut s = settings();
    config(&mut s, Kind::Shodan).api_key =
        ssx_config::Secret::new(Some("  synthetic-key\u{2003}".into()));
    let (r, w, _) = execute(
        Kind::Shodan,
        b"{}".to_vec(),
        200,
        None,
        s,
        query(Kind::Shodan),
    )
    .await;
    whole(r, EngineStatus::Ok, vec![], None);
    assert_request(
        Kind::Shodan,
        &w.calls.lock().unwrap()[0],
        "example query",
        "10",
        "synthetic-key",
    );
}
#[tokio::test]
async fn original_status_classes_are_empty_and_redacted_for_every_engine() {
    for kind in KINDS {
        for (code, status) in [
            (429, EngineStatus::RateLimited),
            (403, EngineStatus::Blocked),
            (401, EngineStatus::Error),
            (404, EngineStatus::Error),
            (500, EngineStatus::Error),
        ] {
            let (r, w, _) = execute(
                kind,
                b"synthetic-key synthetic-secret raw-provider-body".to_vec(),
                code,
                Some("text/html"),
                settings(),
                query(kind),
            )
            .await;
            let message = if status == EngineStatus::Error {
                Some(format!("{} returned HTTP {code}", kind.display()))
            } else {
                None
            };
            whole(r, status, vec![], message.as_deref());
            assert_eq!(w.calls.lock().unwrap().len(), 1);
        }
    }
}
#[tokio::test]
async fn original_crtsh_exact_classification_messages_and_no_body_disclosure() {
    let cases: &[(&[u8], &str, EngineStatus, &str)] = &[
        (
            br#"{"error":"temporarily unavailable"}"#,
            "application/json",
            EngineStatus::Error,
            "CRT.sh returned an unexpected JSON shape; expected a list (HTTP 200, content-type application/json)",
        ),
        (
            b"not json",
            "text/plain",
            EngineStatus::Error,
            "CRT.sh returned malformed JSON (HTTP 200, content-type text/plain)",
        ),
        (
            b"<html><body><div class='captcha'>verify you are human</div></body></html>",
            "text/html; charset=UTF-8",
            EngineStatus::Blocked,
            "CRT.sh returned an upstream challenge/block page (HTTP 200, content-type text/html; markers=captcha,verify_human)",
        ),
        (
            b"<html><body>maintenance</body></html>",
            "text/html; charset=UTF-8",
            EngineStatus::Error,
            "CRT.sh returned non-JSON HTML (HTTP 200, content-type text/html; no challenge markers)",
        ),
        (
            br#"[{"id":1},"not-a-certificate-row"]"#,
            "application/json",
            EngineStatus::Error,
            "CRT.sh returned a malformed certificate row (HTTP 200, content-type application/json)",
        ),
    ];
    for (body, ct, status, message) in cases {
        let (r, w, _) = execute(
            Kind::CrtSh,
            body.to_vec(),
            200,
            Some(ct),
            settings(),
            query(Kind::CrtSh),
        )
        .await;
        whole(r, status.clone(), vec![], Some(message));
        assert_eq!(w.calls.lock().unwrap().len(), 1);
    }
    let (r, _, _) = execute(
        Kind::CrtSh,
        b"[]".to_vec(),
        200,
        Some("application/json"),
        settings(),
        query(Kind::CrtSh),
    )
    .await;
    whole(r, EngineStatus::Ok, vec![], None);
}
#[tokio::test]
async fn registered_factories_preserve_f06_descriptor_metadata() {
    let http = Arc::new(AdapterHttp::default());
    let a = Arc::new(Admission {
        result: AdmissionDecision::Allowed,
        calls: Mutex::new(vec![]),
    });
    let mut registry = ssx_registry::Registry::builtins().unwrap();
    let before = registry.descriptors().cloned().collect::<Vec<_>>();
    register_infrastructure_security(&mut registry, http.clone(), a, settings()).unwrap();
    assert_eq!(registry.descriptors().cloned().collect::<Vec<_>>(), before);
    for kind in KINDS {
        assert!(registry.is_available(kind.name()));
        assert!(registry.instantiate(kind.name()).is_some());
        let d = registry.descriptor(kind.name()).unwrap();
        assert!(d.categories.contains(&"security".into()));
        assert!(d.enforced_filters.is_empty());
    }
    let d = registry.descriptor("crtsh").unwrap();
    assert!(d.categories.contains(&"it".into()));
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn source_slicing_and_request_limits_are_distinct() {
    use ssx_config::ConfigScalar as S;
    for (limit, count) in [
        (S::Integer(0), 0),
        (S::Integer(1), 1),
        (S::Integer(-1), 1),
        (S::Boolean(true), 1),
        (S::Boolean(false), 0),
    ] {
        for kind in [Kind::Shodan, Kind::CrtSh] {
            let body = if kind == Kind::Shodan {
                serde_json::json!({"matches":[{"ip_str":"one"},{"ip_str":"two"}]})
            } else {
                serde_json::json!([{"common_name":"one"},{"common_name":"two"}])
            };
            let mut s = settings();
            config(&mut s, kind).max_results = limit.clone();
            let (r, _, _) = execute(
                kind,
                serde_json::to_vec(&body).unwrap(),
                200,
                None,
                s,
                query(kind),
            )
            .await;
            assert_eq!(r.status, EngineStatus::Ok);
            assert_eq!(r.results.len(), count);
        }
    }
    for kind in [Kind::Censys, Kind::VirusTotal, Kind::VulnCheck] {
        let mut s = settings();
        config(&mut s, kind).max_results = S::Integer(0);
        let (r, _, _) = execute(
            kind,
            serde_json::to_vec(&fixture(kind)).unwrap(),
            200,
            None,
            s,
            query(kind),
        )
        .await;
        whole(r, EngineStatus::Ok, expected(kind), None);
    }
    for kind in [Kind::Shodan, Kind::CrtSh] {
        let mut s = settings();
        config(&mut s, kind).max_results = S::Integer(0);
        let body = if kind == Kind::Shodan {
            br#"{"matches":[{},null]}"#.to_vec()
        } else {
            b"[{},null]".to_vec()
        };
        let (r, _, _) = execute(kind, body, 200, None, s, query(kind)).await;
        assert_eq!(
            r.status,
            if kind == Kind::CrtSh {
                EngineStatus::Error
            } else {
                EngineStatus::Ok
            }
        );
        assert!(r.results.is_empty());
    }
}
#[tokio::test]
async fn vulncheck_query_routing_first_match_unicode_digits_and_no_cve() {
    for (q, expected) in [
        ("arbitrary host name", None),
        ("CVE-2024-123", None),
        ("CVE-202-12345", None),
        (
            "prefixcve-2024-3094tail CVE-2025-9999",
            Some("CVE-2024-3094"),
        ),
        ("cve-٢٠٢٤-٣٠٩٤", Some("CVE-٢٠٢٤-٣٠٩٤")),
    ] {
        let (r, w, a) = execute(
            Kind::VulnCheck,
            br#"{"data":{"exploit_state":"known"}}"#.to_vec(),
            200,
            None,
            settings(),
            SearchRequest {
                query: q.into(),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(r.status, EngineStatus::Ok);
        assert_eq!(*a.calls.lock().unwrap(), vec!["vulncheck"]);
        let calls = w.calls.lock().unwrap();
        match expected {
            None => {
                assert!(r.results.is_empty());
                assert!(calls.is_empty());
            }
            Some(id) => {
                assert_eq!(r.results[0].title, id);
                assert_eq!(calls.len(), 1);
                assert_eq!(
                    calls[0].url.path(),
                    format!(
                        "/v3/community/cve/{}",
                        reqwest::Url::parse(&format!("https://a/{id}"))
                            .unwrap()
                            .path()
                            .trim_start_matches('/')
                    )
                );
            }
        }
    }
}
use ssx_application::{SearchContext, SearchEngine};
use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::{Duration, Instant},
};
struct ExchangeOwner<'a>(&'a AtomicUsize);
impl Drop for ExchangeOwner<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
struct ControlledWire {
    active: AtomicUsize,
    calls: AtomicUsize,
    block: AtomicBool,
    entered: tokio::sync::Notify,
    status: u16,
    body: Vec<u8>,
}
impl Transport for ControlledWire {
    fn exchange(&self, req: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            self.active.fetch_add(1, Ordering::SeqCst);
            let _owner = ExchangeOwner(&self.active);
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.entered.notify_one();
            if self.block.load(Ordering::SeqCst) {
                std::future::pending::<()>().await;
            }
            Ok(Response {
                body: self.body.clone(),
                status: self.status,
                url: req.url,
                headers: Default::default(),
            })
        })
    }
}
struct ControlledAdmission {
    active: AtomicUsize,
    calls: AtomicUsize,
    entered: tokio::sync::Notify,
    block: AtomicBool,
    decision: AdmissionDecision,
}
impl SearchAdmission for ControlledAdmission {
    fn admit(&self, _: &'static str) -> PortFuture<'_, AdmissionDecision> {
        Box::pin(async move {
            self.active.fetch_add(1, Ordering::SeqCst);
            let _owner = ExchangeOwner(&self.active);
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.entered.notify_one();
            if self.block.load(Ordering::SeqCst) {
                std::future::pending::<()>().await;
            }
            self.decision
        })
    }
}
fn wire(kind: Kind) -> Arc<ControlledWire> {
    Arc::new(ControlledWire {
        active: AtomicUsize::new(0),
        calls: AtomicUsize::new(0),
        block: AtomicBool::new(false),
        entered: Default::default(),
        status: 200,
        body: serde_json::to_vec(&fixture(kind)).unwrap(),
    })
}
fn admission(decision: AdmissionDecision) -> Arc<ControlledAdmission> {
    Arc::new(ControlledAdmission {
        active: AtomicUsize::new(0),
        calls: AtomicUsize::new(0),
        entered: Default::default(),
        block: AtomicBool::new(false),
        decision,
    })
}
fn engine(
    kind: Kind,
    http: Arc<AdapterHttp>,
    a: Arc<dyn SearchAdmission>,
    s: SecuritySettings,
) -> Box<dyn SearchEngine> {
    let mut r = ssx_registry::Registry::builtins().unwrap();
    register_infrastructure_security(&mut r, http, a, s).unwrap();
    r.instantiate(kind.name()).unwrap()
}
#[tokio::test]
async fn admission_failure_precedes_credentials_url_and_parse_with_zero_dispatch() {
    for kind in KINDS {
        for decision in [AdmissionDecision::Denied, AdmissionDecision::Unavailable] {
            let h = Arc::new(AdapterHttp::default());
            let w = wire(kind);
            h.set_transport(w.clone()).unwrap();
            let a = admission(decision);
            let mut s = settings();
            config(&mut s, kind).api_key = ssx_config::Secret::new(None);
            config(&mut s, kind).base_url = "not a URL".into();
            let e = engine(kind, h.clone(), a.clone(), s);
            let r = e.search(&query(kind)).await;
            if decision == AdmissionDecision::Denied {
                whole(r, EngineStatus::RateLimited, vec![], Some("rate limited"));
            } else {
                whole(
                    r,
                    EngineStatus::Error,
                    vec![],
                    Some(&format!("{} rate policy unavailable", kind.display())),
                );
            }
            assert_eq!(a.calls.load(Ordering::SeqCst), 1);
            assert_eq!(a.active.load(Ordering::SeqCst), 0);
            assert_eq!(w.calls.load(Ordering::SeqCst), 0);
            h.shutdown_and_wait().await;
        }
    }
}
#[tokio::test]
async fn expired_absolute_context_does_not_admit_or_dispatch() {
    for kind in KINDS {
        let h = Arc::new(AdapterHttp::default());
        let w = wire(kind);
        h.set_transport(w.clone()).unwrap();
        let a = admission(AdmissionDecision::Allowed);
        let e = engine(kind, h.clone(), a.clone(), settings());
        let r = e
            .search_with_context(
                &query(kind),
                SearchContext {
                    deadline: Instant::now() - Duration::from_secs(1),
                },
            )
            .await;
        whole(r, EngineStatus::Timeout, vec![], None);
        assert_eq!(a.calls.load(Ordering::SeqCst), 0);
        assert_eq!(w.calls.load(Ordering::SeqCst), 0);
        h.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn absolute_context_bounds_admission_and_releases_its_future() {
    for kind in KINDS {
        let h = Arc::new(AdapterHttp::default());
        let w = wire(kind);
        h.set_transport(w.clone()).unwrap();
        let a = admission(AdmissionDecision::Allowed);
        a.block.store(true, Ordering::SeqCst);
        let e = engine(kind, h.clone(), a.clone(), settings());
        let r = tokio::time::timeout(
            Duration::from_secs(2),
            e.search_with_context(
                &query(kind),
                SearchContext {
                    deadline: Instant::now() + Duration::from_millis(20),
                },
            ),
        )
        .await
        .expect("absolute admission deadline must terminate");
        whole(r, EngineStatus::Timeout, vec![], None);
        assert_eq!(a.calls.load(Ordering::SeqCst), 1);
        assert_eq!(a.active.load(Ordering::SeqCst), 0);
        assert_eq!(w.calls.load(Ordering::SeqCst), 0);
        h.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn context_and_configured_deadline_each_drop_actual_inflight_exchange() {
    for kind in KINDS {
        for configured in [false, true] {
            let h = Arc::new(AdapterHttp::default());
            let w = wire(kind);
            w.block.store(true, Ordering::SeqCst);
            h.set_transport(w.clone()).unwrap();
            let a = admission(AdmissionDecision::Allowed);
            let mut s = settings();
            if configured {
                config(&mut s, kind).timeout_ms = ssx_config::ConfigScalar::Integer(20);
            }
            let e = engine(kind, h.clone(), a, s);
            let q = query(kind);
            let context = SearchContext {
                deadline: Instant::now()
                    + if configured {
                        Duration::from_secs(5)
                    } else {
                        Duration::from_millis(20)
                    },
            };
            let r =
                tokio::time::timeout(Duration::from_secs(2), e.search_with_context(&q, context))
                    .await
                    .expect("minimum absolute/configured deadline must terminate");
            whole(r, EngineStatus::Timeout, vec![], None);
            assert_eq!(w.calls.load(Ordering::SeqCst), 1);
            assert_eq!(w.active.load(Ordering::SeqCst), 0);
            // The adapter releases only its own session, preserving its caller's HTTP owner.
            w.block.store(false, Ordering::SeqCst);
            let r = e.search(&q).await;
            whole(r, EngineStatus::Ok, expected(kind), None);
            assert_eq!(w.calls.load(Ordering::SeqCst), 2);
            h.shutdown_and_wait().await;
        }
    }
}
#[tokio::test]
async fn drop_and_shutdown_cancel_only_owned_exchange_and_join_cleanup() {
    for kind in KINDS {
        for shutdown in [false, true] {
            let h = Arc::new(AdapterHttp::default());
            let w = wire(kind);
            w.block.store(true, Ordering::SeqCst);
            h.set_transport(w.clone()).unwrap();
            let a = admission(AdmissionDecision::Allowed);
            let e = engine(kind, h.clone(), a, settings());
            let q = query(kind);
            let mut f = e.search_with_context(
                &q,
                SearchContext {
                    deadline: Instant::now() + Duration::from_secs(5),
                },
            );
            tokio::time::timeout(Duration::from_secs(2),async{tokio::select!{_ = w.entered.notified()=>{},r=&mut f=>panic!("exchange must be observed before control: {r:?}")}}).await.unwrap();
            assert_eq!(w.active.load(Ordering::SeqCst), 1);
            if shutdown {
                h.shutdown();
                let r = tokio::time::timeout(Duration::from_secs(2), f)
                    .await
                    .unwrap();
                assert_eq!(r.status, EngineStatus::Error);
                assert!(r.results.is_empty());
            } else {
                drop(f);
            }
            assert_eq!(w.active.load(Ordering::SeqCst), 0);
            assert_eq!(w.calls.load(Ordering::SeqCst), 1);
            if !shutdown {
                w.block.store(false, Ordering::SeqCst);
                let r = e.search(&q).await;
                whole(r, EngineStatus::Ok, expected(kind), None);
            }
            tokio::time::timeout(Duration::from_secs(2), h.shutdown_and_wait())
                .await
                .unwrap();
        }
    }
}
#[tokio::test]
async fn f08_transport_failure_is_redacted_and_does_not_substitute_provider_payload() {
    struct Failing;
    impl Transport for Failing {
        fn exchange(&self, _: Request) -> ExchangeFuture<'_> {
            Box::pin(async { Err(http::Error::Network) })
        }
    }
    for kind in KINDS {
        let h = Arc::new(AdapterHttp::default());
        h.set_transport(Arc::new(Failing)).unwrap();
        let a = admission(AdmissionDecision::Allowed);
        let e = engine(kind, h.clone(), a, settings());
        whole(
            e.search(&query(kind)).await,
            EngineStatus::Error,
            vec![],
            Some(&format!("{} request failed", kind.display())),
        );
        h.shutdown_and_wait().await;
    }
}
#[tokio::test]
async fn source_empty_wrong_shape_and_atomic_failure_matrix() {
    use serde_json::json;
    for kind in KINDS {
        let empty = match kind {
            Kind::Censys => json!({"result":{}}),
            Kind::Shodan => json!({"matches":[]}),
            Kind::VirusTotal => json!({"data":[]}),
            Kind::VulnCheck => json!({"data":{}}),
            Kind::CrtSh => json!([]),
        };
        let (r, _, _) = execute(
            kind,
            serde_json::to_vec(&empty).unwrap(),
            200,
            None,
            settings(),
            query(kind),
        )
        .await;
        whole(r, EngineStatus::Ok, vec![], None);
        for body in [b"null".as_slice(), b"true", b"7", br#""root""#, b"[]"] {
            let (r, _, _) = execute(kind, body.to_vec(), 200, None, settings(), query(kind)).await;
            if kind == Kind::CrtSh && body == b"[]" {
                whole(r, EngineStatus::Ok, vec![], None);
            } else {
                assert_eq!(r.status, EngineStatus::Error, "{kind:?} {body:?}");
                assert!(r.results.is_empty());
            }
        }
        let body = match kind {
            Kind::Censys => json!({"result":{"hits":[{"ip":"valid"},null]}}),
            Kind::Shodan => json!({"matches":[{"ip_str":"valid"},null]}),
            Kind::VirusTotal => json!({"data":[{"id":"valid"},null]}),
            Kind::VulnCheck => json!({"data":[{}]}),
            Kind::CrtSh => json!([{}, null]),
        };
        let (r, _, _) = execute(
            kind,
            serde_json::to_vec(&body).unwrap(),
            200,
            None,
            settings(),
            query(kind),
        )
        .await;
        assert_eq!(r.status, EngineStatus::Error);
        assert!(
            r.results.is_empty(),
            "{kind:?} must discard already constructed rows"
        );
        let (r, _, _) = execute(
            kind,
            b"not-json synthetic-key".to_vec(),
            200,
            None,
            settings(),
            query(kind),
        )
        .await;
        assert_eq!(r.status, EngineStatus::Error);
        assert!(r.results.is_empty());
        assert!(!r.error_message.unwrap().contains("synthetic-key"));
    }
    for result in [
        json!(null),
        json!(false),
        json!(7),
        json!("text"),
        json!([]),
    ] {
        let (r, _, _) = execute(
            Kind::Censys,
            serde_json::to_vec(&json!({"result":result})).unwrap(),
            200,
            None,
            settings(),
            query(Kind::Censys),
        )
        .await;
        whole(r, EngineStatus::Ok, vec![], None);
    }
    for data in [json!(null), json!(false), json!(0), json!(""), json!([])] {
        let (r, _, _) = execute(
            Kind::VulnCheck,
            serde_json::to_vec(&json!({"data":data})).unwrap(),
            200,
            None,
            settings(),
            query(Kind::VulnCheck),
        )
        .await;
        whole(r, EngineStatus::Ok, vec![], None);
    }
}
#[tokio::test]
async fn numeric_unicode_container_and_limit_semantics_use_source_operations() {
    use serde_json::json;
    let shodan = r#"{"matches":[{"ip_str":"::1","port":false,"hostnames":"αβγδ","product":"nginx","version":7,"cvss":"\u2003９_１.５\u0085","vulns":{"CVE-B":{},"CVE-A":{}}}]}"#;
    let (r, _, _) = execute(
        Kind::Shodan,
        shodan.as_bytes().to_vec(),
        200,
        None,
        settings(),
        query(Kind::Shodan),
    )
    .await;
    whole(
        r,
        EngineStatus::Ok,
        vec![row(
            Kind::Shodan,
            "https://www.shodan.io/host/::1",
            "::1",
            "nginx 7 | α, β, γ | CVSS \u{2003}９_１.５\u{85} | CVEs: CVE-B, CVE-A",
            91.5,
            1,
            None,
        )],
        None,
    );
    // Exact raw integer total exceeds u64; addition and cancellation occur before f64 score conversion.
    let body=br#"{"data":[{"id":"id","attributes":{"last_analysis_stats":{"malicious":1,"negative":-99999999999999999999999999999999,"positive":100000000000000000000000000000000,"suspicious":true}}}]}"#;
    let (r, _, _) = execute(
        Kind::VirusTotal,
        body.to_vec(),
        200,
        None,
        settings(),
        query(Kind::VirusTotal),
    )
    .await;
    whole(
        r,
        EngineStatus::Ok,
        vec![row(
            Kind::VirusTotal,
            "https://virustotal.com/gui/search/id",
            "id",
            "Detection: 1/3 engines (True suspicious) | VT link: https://virustotal.com/gui/search/id",
            1.,
            1,
            None,
        )],
        None,
    );
    let body=br#"{"data":[{"id":"f","attributes":{"last_analysis_stats":{"malicious":1.0,"a":1e16,"b":-1e16,"suspicious":0}}}]}"#;
    let (r, _, _) = execute(
        Kind::VirusTotal,
        body.to_vec(),
        200,
        None,
        settings(),
        query(Kind::VirusTotal),
    )
    .await;
    whole(
        r,
        EngineStatus::Ok,
        vec![row(
            Kind::VirusTotal,
            "https://virustotal.com/gui/search/f",
            "f",
            "Detection: 1.0/1.0 engines (0 suspicious) | VT link: https://virustotal.com/gui/search/f",
            1.,
            1,
            None,
        )],
        None,
    );
    let body = json!({"data":{"date_added":"","vulncheck_date_added":"2026-02-03T12:34:56","vendor_data":[{"vendor":"z"},{"vendor":"a"},{"vendor":"a"},{"vendor":""},{"vendor":"b"},{"vendor":"ignored"}],"exploit_urls":"😀é","cpe":{"x":1,"y":2}}});
    let (r, _, _) = execute(
        Kind::VulnCheck,
        serde_json::to_vec(&body).unwrap(),
        200,
        None,
        settings(),
        query(Kind::VulnCheck),
    )
    .await;
    whole(
        r,
        EngineStatus::Ok,
        vec![row(
            Kind::VulnCheck,
            "https://api.vulncheck.com/v3/community/cve/CVE-2024-3094",
            "CVE-2024-3094",
            "Added: 2026-02-03 | Affected: , a, b, z | Exploits: 2 available | CPEs: 2",
            0.,
            1,
            None,
        )],
        None,
    );
    // Serial slicing is lazy: an otherwise invalid serial is untouched when issuer content exists.
    let body=br#"[{"issuer_name":7,"serial_number":7},{"serial_number":"1234567890123456789012345"},{"id":null}]"#;
    let (r, _, _) = execute(
        Kind::CrtSh,
        body.to_vec(),
        200,
        None,
        settings(),
        query(Kind::CrtSh),
    )
    .await;
    whole(
        r,
        EngineStatus::Ok,
        vec![
            row(
                Kind::CrtSh,
                "https://crt.sh/?q=",
                "Certificate #",
                "Issuer: 7",
                0.,
                1,
                None,
            ),
            row(
                Kind::CrtSh,
                "https://crt.sh/?q=",
                "Certificate #",
                "Serial: 12345678901234567890",
                0.,
                2,
                None,
            ),
            row(
                Kind::CrtSh,
                "https://crt.sh/?q=",
                "Certificate #None",
                "Certificate result",
                0.,
                3,
                None,
            ),
        ],
        None,
    );
}
#[tokio::test]
async fn crtsh_marker_order_sniff_window_and_decoded_scan_boundaries() {
    let marker_body = "VERIFY YOU ARE HUMAN unusual traffic recaptcha hcaptcha cloudflare challenge cf-chl- captcha access denied";
    let (r, _, _) = execute(
        Kind::CrtSh,
        marker_body.as_bytes().to_vec(),
        200,
        Some("TEXT/HTML; charset=utf-8"),
        settings(),
        query(Kind::CrtSh),
    )
    .await;
    whole(
        r,
        EngineStatus::Blocked,
        vec![],
        Some(
            "CRT.sh returned an upstream challenge/block page (HTTP 200, content-type text/html; markers=access_denied,captcha,cf_challenge,challenge,cloudflare,hcaptcha,recaptcha,unusual_traffic,verify_human)",
        ),
    );
    for prefix in ["<HTML>", "\t\r\n<!DOCTYPE html>", "<html-"] {
        let (r, _, _) = execute(
            Kind::CrtSh,
            format!("{prefix} CAPTCHA").into_bytes(),
            200,
            None,
            settings(),
            query(Kind::CrtSh),
        )
        .await;
        whole(
            r,
            EngineStatus::Blocked,
            vec![],
            Some(
                "CRT.sh returned an upstream challenge/block page (HTTP 200, content-type unknown; markers=captcha)",
            ),
        );
    }
    for prefix in ["<htmlx>", "<!doctypehtml>"] {
        let (r, _, _) = execute(
            Kind::CrtSh,
            format!("{prefix} captcha").into_bytes(),
            200,
            None,
            settings(),
            query(Kind::CrtSh),
        )
        .await;
        whole(
            r,
            EngineStatus::Error,
            vec![],
            Some("CRT.sh returned malformed JSON (HTTP 200, content-type unknown)"),
        );
    }
    for (padding, status) in [(65529, EngineStatus::Blocked), (65530, EngineStatus::Error)] {
        let body = format!("{}captcha", "é".repeat(padding));
        let (r, _, _) = execute(
            Kind::CrtSh,
            body.into_bytes(),
            200,
            Some("text/html; charset=utf-8"),
            settings(),
            query(Kind::CrtSh),
        )
        .await;
        assert_eq!(r.status, status);
        assert!(r.results.is_empty());
    }
    let body = format!("{}<html>captcha", " ".repeat(256));
    let (r, _, _) = execute(
        Kind::CrtSh,
        body.into_bytes(),
        200,
        None,
        settings(),
        query(Kind::CrtSh),
    )
    .await;
    whole(
        r,
        EngineStatus::Error,
        vec![],
        Some("CRT.sh returned malformed JSON (HTTP 200, content-type unknown)"),
    );
}
#[tokio::test]
async fn legal_config_scalars_credential_encoding_and_base_override_are_not_silently_coerced() {
    use ssx_config::ConfigScalar as S;
    for kind in KINDS {
        for timeout in [S::Integer(0), S::Integer(-1), S::Boolean(false)] {
            let mut s = settings();
            config(&mut s, kind).timeout_ms = timeout;
            let (r, w, _) = execute(kind, b"{}".to_vec(), 200, None, s, query(kind)).await;
            whole(r, EngineStatus::Timeout, vec![], None);
            assert!(w.calls.lock().unwrap().is_empty());
        }
        let mut s = settings();
        config(&mut s, kind).base_url = "".into();
        let (r, w, _) = execute(kind, b"{}".to_vec(), 200, None, s, query(kind)).await;
        assert_eq!(r.status, EngineStatus::Error);
        assert!(w.calls.lock().unwrap().is_empty());
    }
    for kind in [Kind::Shodan, Kind::VirusTotal, Kind::VulnCheck] {
        let mut s = settings();
        config(&mut s, kind).api_key = ssx_config::Secret::new(Some("clé-秘密".into()));
        let (r, w, _) = execute(kind, b"{}".to_vec(), 200, None, s, query(kind)).await;
        whole(
            r,
            EngineStatus::Error,
            vec![],
            Some("invalid infrastructure credential configuration"),
        );
        assert!(w.calls.lock().unwrap().is_empty());
    }
    // HTTP Basic credentials encode UTF-8 before producing the ASCII header.
    let mut s = settings();
    config(&mut s, Kind::Censys).api_key = ssx_config::Secret::new(Some("clé".into()));
    let (r, w, _) = execute(
        Kind::Censys,
        b"{}".to_vec(),
        200,
        None,
        s,
        query(Kind::Censys),
    )
    .await;
    whole(r, EngineStatus::Ok, vec![], None);
    assert_request(
        Kind::Censys,
        &w.calls.lock().unwrap()[0],
        "example query",
        "10",
        "clé",
    );
}
#[test]
fn private_loopback_wire_contract() {
    const CHILD: &str = "SSX_A12_PRIVATE_WIRE_CHILD";
    if std::env::var_os(CHILD).is_some() {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            use tokio::io::{AsyncReadExt,AsyncWriteExt};
            for kind in KINDS {
                let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let addr=listener.local_addr().unwrap();
                let mut s=settings();config(&mut s,kind).base_url=format!("http://{addr}/private");
                let h=Arc::new(AdapterHttp::default());let a=admission(AdmissionDecision::Allowed);let e=engine(kind,h.clone(),a.clone(),s);let q=query(kind);
                let server=async {
                    let(mut socket,peer)=listener.accept().await.unwrap();assert!(peer.ip().is_loopback());let mut bytes=Vec::new();let mut buf=[0u8;2048];
                    while !bytes.ends_with(b"\r\n\r\n") {let n=socket.read(&mut buf).await.unwrap();assert!(n>0,"complete headers required");assert!(bytes.len()+n<=65536,"bounded test wire");bytes.extend_from_slice(&buf[..n]);}
                    let body=serde_json::to_vec(&fixture(kind)).unwrap();let headers=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());socket.write_all(headers.as_bytes()).await.unwrap();socket.write_all(&body).await.unwrap();socket.shutdown().await.unwrap();bytes
                };
                let(r,bytes)=tokio::time::timeout(Duration::from_secs(3),async{tokio::join!(e.search(&q),server)}).await.expect("private request and server must finish together");whole(r,EngineStatus::Ok,expected(kind),None);
                let text=std::str::from_utf8(&bytes).unwrap();let mut lines=text.split("\r\n");let request_line=lines.next().unwrap();
                let path=match kind {Kind::Censys=>"/private/hosts/search?q=example+query&per_page=10",Kind::Shodan=>"/private/shodan/host/search?query=example+query&limit=10",Kind::VirusTotal=>"/private/search?query=example+query&limit=10",Kind::VulnCheck=>"/private/community/cve/CVE-2024-3094",Kind::CrtSh=>"/private/?q=example+query"};assert_eq!(request_line,format!("GET {path} HTTP/1.1"));
                let mut headers=std::collections::BTreeMap::new();for line in lines.take_while(|l|!l.is_empty()){let(k,v)=line.split_once(':').unwrap();assert!(headers.insert(k.to_ascii_lowercase(),v.trim().to_string()).is_none(),"no repeated test header");}
                assert_eq!(headers.get("host"),Some(&addr.to_string()));assert!(!headers.contains_key("x-api-key"));
                match kind {Kind::Censys=>{use base64::Engine as _;assert_eq!(headers.get("authorization"),Some(&format!("Basic {}",base64::engine::general_purpose::STANDARD.encode("synthetic-key:synthetic-secret"))));},Kind::Shodan|Kind::VulnCheck=>assert_eq!(headers.get("authorization").map(String::as_str),Some("Bearer synthetic-key")),Kind::VirusTotal=>assert_eq!(headers.get("x-apikey").map(String::as_str),Some("synthetic-key")),Kind::CrtSh=>{assert!(!headers.contains_key("authorization"));assert!(!headers.contains_key("x-apikey"));}}
                assert_eq!(a.calls.load(Ordering::SeqCst),1);
                use base64::Engine as _;println!("\nA12_PRIVATE_WIRE {} {}",kind.name(),base64::engine::general_purpose::STANDARD.encode(&bytes));
                h.shutdown_and_wait().await;
            }
        });
        return;
    }
    struct OwnedChild {
        child: std::process::Child,
        dir: std::path::PathBuf,
    }
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ssx-a12-wire-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir(&dir).unwrap();
    let out = dir.join("stdout");
    let err = dir.join("stderr");
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "private_loopback_wire_contract", "--nocapture"])
        .env(CHILD, "1")
        .stdout(std::fs::File::create(&out).unwrap())
        .stderr(std::fs::File::create(&err).unwrap());
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "NO_PROXY",
        "no_proxy",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
    ] {
        command.env_remove(key);
    }
    let mut owned = OwnedChild {
        child: command.spawn().unwrap(),
        dir,
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(s) = owned.child.try_wait().unwrap() {
            break s;
        }
        assert!(
            Instant::now() < deadline,
            "private child exceeded monotonic watchdog"
        );
        for p in [&out, &err] {
            assert!(
                std::fs::metadata(p).unwrap().len() <= 1048576,
                "private log cap"
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = std::fs::read(&out).unwrap();
    let stderr = std::fs::read(&err).unwrap();
    assert!(
        stdout.len() + stderr.len() <= 1048576,
        "private combined log cap"
    );
    let text = std::str::from_utf8(&stdout).unwrap();
    assert!(
        status.success(),
        "private child {status}: {text}\n{}",
        String::from_utf8_lossy(&stderr)
    );
    assert!(text.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"));
    for kind in KINDS {
        assert_eq!(
            text.lines()
                .filter(|l| l.starts_with(&format!("A12_PRIVATE_WIRE {} ", kind.name())))
                .count(),
            1
        );
    }
    use base64::Engine as _;
    println!(
        "A12_CHILD_STDOUT_BASE64 {}",
        base64::engine::general_purpose::STANDARD.encode(&stdout)
    );
    println!(
        "A12_CHILD_STDERR_BASE64 {}",
        base64::engine::general_purpose::STANDARD.encode(&stderr)
    );
}
async fn parse_direct(
    kind: Kind,
    v: &source_json::Value,
    batch: &mut source_payload::ProducedBatch<'_>,
    b: &ssx_owned_budget::Budget,
    d: Instant,
) -> infrastructure_security::Result<()> {
    let limit = ssx_config::ConfigScalar::Integer(10);
    match kind {
        Kind::Censys => censys::parse(v, batch, b, d).await,
        Kind::Shodan => shodan::parse(v, &limit, batch, b, d).await,
        Kind::VirusTotal => virustotal::parse(v, batch, b, d).await,
        Kind::VulnCheck => vulncheck::parse(v, "CVE-2024-3094", batch, b, d).await,
        Kind::CrtSh => crtsh::parse(v, &limit, batch, b, d).await,
    }
}
#[tokio::test]
async fn owned_projection_charges_release_and_refusals_are_atomic() {
    for kind in KINDS {
        let v = source_json::from_slice(&serde_json::to_vec(&fixture(kind)).unwrap(), None)
            .await
            .unwrap();
        let b = ssx_owned_budget::Budget::new(32 * 1024 * 1024).unwrap();
        let d = Instant::now() + Duration::from_secs(5);
        let mut batch =
            source_payload::ProducedBatch::new(&b, expected(kind).len(), Some(d)).unwrap();
        parse_direct(kind, &v, &mut batch, &b, d).await.unwrap();
        assert!(b.snapshot().unwrap().used > 0);
        assert_eq!(batch.results(), expected(kind));
        drop(batch);
        assert_eq!(b.snapshot().unwrap().used, 0);
        let tiny = ssx_owned_budget::Budget::new(1).unwrap();
        assert!(source_payload::ProducedBatch::new(&tiny, 1, Some(d)).is_err());
        assert_eq!(tiny.snapshot().unwrap().used, 0);
    }
}
#[tokio::test]
async fn dropping_actual_projection_work_releases_scratch_at_observed_charge() {
    use std::{
        future::Future,
        task::{Context, Poll},
    };
    for kind in KINDS {
        let long = "x".repeat(16384);
        let body = match kind {
            Kind::Censys => serde_json::json!({"result":{"hits":[{"ip":long}]}}),
            Kind::Shodan => serde_json::json!({"matches":[{"ip_str":"ip","org":long}]}),
            Kind::VirusTotal => {
                serde_json::json!({"data":[{"id":"id","attributes":{"meaningful_name":long}}]})
            }
            Kind::VulnCheck => serde_json::json!({"data":{"exploit_state":long}}),
            Kind::CrtSh => serde_json::json!([{"issuer_name":long}]),
        };
        let v = source_json::from_slice(&serde_json::to_vec(&body).unwrap(), None)
            .await
            .unwrap();
        let b = ssx_owned_budget::Budget::new(32 * 1024 * 1024).unwrap();
        let d = Instant::now() + Duration::from_secs(5);
        let mut batch = source_payload::ProducedBatch::new(&b, 1, Some(d)).unwrap();
        let initial = b.snapshot().unwrap().used;
        let mut f = Box::pin(parse_direct(kind, &v, &mut batch, &b, d));
        let waker = std::task::Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut observed = false;
        for _ in 0..10000 {
            match f.as_mut().poll(&mut context) {
                Poll::Ready(r) => {
                    panic!("must observe allocated scratch before completion: {kind:?} {r:?}")
                }
                Poll::Pending => {
                    if b.snapshot().unwrap().used > initial + 1024 {
                        observed = true;
                        break;
                    }
                }
            }
        }
        assert!(
            observed,
            "actual allocated projection work must be observed: {kind:?}"
        );
        drop(f);
        assert_eq!(b.snapshot().unwrap().used, initial);
        assert!(batch.results().is_empty());
        drop(batch);
        assert_eq!(b.snapshot().unwrap().used, 0);
    }
}
#[tokio::test]
async fn unsupported_request_filters_and_paging_do_not_claim_or_change_enforcement() {
    for kind in KINDS {
        let mut q = query(kind);
        q.language = "fr".into();
        q.page = 9;
        q.max_results = Some(0);
        q.time_range = Some("week".into());
        q.safesearch = 2;
        q.categories = Some(vec!["security".into()]);
        q.date_from = Some("2020-01-01".into());
        q.date_to = Some("2020-01-02".into());
        let (r, w, _) = execute(
            kind,
            serde_json::to_vec(&fixture(kind)).unwrap(),
            200,
            None,
            settings(),
            q.clone(),
        )
        .await;
        whole(r, EngineStatus::Ok, expected(kind), None);
        assert_request(
            kind,
            &w.calls.lock().unwrap()[0],
            &q.query,
            "10",
            "synthetic-key",
        );
    }
}
#[tokio::test]
async fn inherited_f05_refusals_are_whole_responses_and_unused_values_remain_legal() {
    for token in ["NaN", "Infinity", "-Infinity"] {
        for kind in [Kind::Shodan, Kind::VirusTotal] {
            let body = if kind == Kind::Shodan {
                format!(r#"{{"matches":[{{"ip_str":"ok","cvss":{token}}}]}}"#)
            } else {
                format!(
                    r#"{{"data":[{{"id":"ok","attributes":{{"last_analysis_stats":{{"malicious":{token}}}}}}}]}}"#
                )
            };
            let (r, _, _) =
                execute(kind, body.into_bytes(), 200, None, settings(), query(kind)).await;
            whole(
                r,
                EngineStatus::Error,
                vec![],
                Some("infrastructure result cannot be projected within native bounds"),
            );
        }
    }
    for kind in KINDS {
        let mut body = fixture(kind);
        if kind == Kind::CrtSh {
            body[0]["unused"] = serde_json::Value::String("unused".into());
        } else {
            body["unused"] = serde_json::Value::String("unused".into());
        }
        let bytes = serde_json::to_string(&body)
            .unwrap()
            .replace("\"unused\"", r#""\ud800""#);
        let (r, _, _) = execute(kind, bytes.into_bytes(), 200, None, settings(), query(kind)).await;
        whole(r, EngineStatus::Ok, expected(kind), None);
    }
    for (kind, body) in [
        (
            Kind::Censys,
            br#"{"result":{"hits":[{"ip":7}]}}"#.as_slice(),
        ),
        (Kind::VirusTotal, br#"{"data":[{"id":7}]}"#),
        (Kind::CrtSh, br#"[{"common_name":"\ud800"}]"#),
    ] {
        let (r, w, _) = execute(kind, body.to_vec(), 200, None, settings(), query(kind)).await;
        assert_eq!(r.status, EngineStatus::Error);
        assert!(r.results.is_empty());
        assert_eq!(w.calls.lock().unwrap().len(), 1);
    }
}
// Literal source-derived selection regressions; no original engine was executed.
#[tokio::test]
async fn review_source_selection_vulncheck_date_tail() {
    let kind = Kind::VulnCheck;
    let q = query(kind);
    let (actual, wire, admission) = execute(
        kind,
        br#"{"data":{"date_added":"2024-01-01\ud800"}}"#.to_vec(),
        200,
        Some("application/json"),
        settings(),
        q.clone(),
    )
    .await;
    let calls = wire.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_request(kind, &calls[0], &q.query, "10", "synthetic-key");
    assert_eq!(*admission.calls.lock().unwrap(), vec![kind.name()]);
    println!(
        "A12_SELECTION_NATIVE_ACTUAL case=vulncheck_date_tail response={actual:?} request={:?}",
        calls[0]
    );
    drop(calls);
    whole(
        actual,
        EngineStatus::Ok,
        vec![row(
            kind,
            "https://api.vulncheck.com/v3/community/cve/CVE-2024-3094",
            "CVE-2024-3094",
            "Added: 2024-01-01",
            0.,
            1,
            None,
        )],
        None,
    );
}
#[tokio::test]
async fn review_source_selection_crtsh_date_tail() {
    let kind = Kind::CrtSh;
    let q = query(kind);
    let (actual, wire, admission) = execute(
        kind,
        br#"[{"id":1,"common_name":"valid.example","not_before":"2024-01-01\ud800"}]"#.to_vec(),
        200,
        Some("application/json"),
        settings(),
        q.clone(),
    )
    .await;
    let calls = wire.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_request(kind, &calls[0], &q.query, "10", "synthetic-key");
    assert_eq!(*admission.calls.lock().unwrap(), vec![kind.name()]);
    println!(
        "A12_SELECTION_NATIVE_ACTUAL case=crtsh_date_tail response={actual:?} request={:?}",
        calls[0]
    );
    drop(calls);
    whole(
        actual,
        EngineStatus::Ok,
        vec![row(
            kind,
            "https://crt.sh/?id=1",
            "valid.example",
            "Valid: 2024-01-01",
            0.,
            1,
            Some("2024-01-01"),
        )],
        None,
    );
}
#[tokio::test]
async fn review_source_selection_crtsh_san_tail() {
    let kind = Kind::CrtSh;
    let q = query(kind);
    let (actual, wire, admission) = execute(
        kind,
        br#"[{"id":1,"common_name":"valid.example","name_value":"valid.example\n\ud800"}]"#
            .to_vec(),
        200,
        Some("application/json"),
        settings(),
        q.clone(),
    )
    .await;
    let calls = wire.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_request(kind, &calls[0], &q.query, "10", "synthetic-key");
    assert_eq!(*admission.calls.lock().unwrap(), vec![kind.name()]);
    println!(
        "A12_SELECTION_NATIVE_ACTUAL case=crtsh_san_tail response={actual:?} request={:?}",
        calls[0]
    );
    drop(calls);
    whole(
        actual,
        EngineStatus::Ok,
        vec![row(
            kind,
            "https://crt.sh/?id=1",
            "valid.example",
            "SANs: valid.example",
            0.,
            1,
            None,
        )],
        None,
    );
}
#[tokio::test]
async fn review_source_selection_crtsh_vertical_tab_html() {
    let kind = Kind::CrtSh;
    let q = query(kind);
    let (actual, wire, admission) = execute(
        kind,
        b"\x0b<html>captcha".to_vec(),
        200,
        Some("application/json"),
        settings(),
        q.clone(),
    )
    .await;
    let calls = wire.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_request(kind, &calls[0], &q.query, "10", "synthetic-key");
    assert_eq!(*admission.calls.lock().unwrap(), vec![kind.name()]);
    println!(
        "A12_SELECTION_NATIVE_ACTUAL case=crtsh_vertical_tab_html response={actual:?} request={:?}",
        calls[0]
    );
    drop(calls);
    whole(
        actual,
        EngineStatus::Blocked,
        vec![],
        Some(
            "CRT.sh returned an upstream challenge/block page (HTTP 200, content-type application/json; markers=captcha)",
        ),
    );
}
