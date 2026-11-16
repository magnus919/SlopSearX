use ssx_application::{policy::*, scope::*};
use ssx_domain::{FiniteF64, SearchRequest};
use ssx_infra::{jev::*, valkey::*};
use ssx_json::{Value, json};
use ssx_registry::Registry;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
fn fixture() -> Value {
    ssx_json::from_str(include_str!("../../xtask/fixtures/f12/source.json")).unwrap()
}
fn config() -> JevConfig {
    JevConfig::from_environment(&BTreeMap::from([(
        "TYPESAFE_API_KEY".into(),
        "synthetic".into(),
    )]))
    .unwrap()
}
fn engine(name: &str) -> EngineCapability {
    EngineCapability {
        name: name.into(),
        enabled: true,
        active: true,
        available: true,
        categories: vec!["science".into()],
        media_types: vec![],
        auth_ready: true,
        circuit_open: false,
        cost_class: "free".into(),
        health_status: "ok".into(),
        health_stale: false,
        enforced_filters: BTreeMap::new(),
    }
}
fn service() -> PolicyService {
    PolicyService {
        policy: SearchPolicy::default(),
        catalog: CatalogSnapshot::new(
            ["brave", "pubmed", "arxiv"]
                .into_iter()
                .map(engine)
                .collect(),
        )
        .unwrap(),
        resolver: ScopeResolver {
            topics: Some(vec![]),
            ..Default::default()
        },
    }
}
fn proposal() -> ScopeProposal {
    ScopeProposal::generic(SearchRequest {
        query: " café 🐙 ".into(),
        ..Default::default()
    })
}
struct Provider {
    endpoint: String,
    calls: Arc<Mutex<Vec<(String, String)>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Provider {
    async fn start(body: Vec<u8>, status: u16, delay: Duration) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/v1/systemone", listener.local_addr().unwrap());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let out = calls.clone();
        let task = tokio::spawn(async move {
            loop {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut raw = Vec::new();
                let mut buf = [0; 4096];
                let header_end = loop {
                    let n = sock.read(&mut buf).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    raw.extend_from_slice(&buf[..n]);
                    assert!(raw.len() < 1024 * 1024);
                    if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let head = String::from_utf8(raw[..header_end].to_vec()).unwrap();
                let length: usize = head
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|s| s.trim().parse().unwrap())
                    })
                    .unwrap();
                while raw.len() < header_end + length {
                    let n = sock.read(&mut buf).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    raw.extend_from_slice(&buf[..n]);
                }
                out.lock()
                    .unwrap()
                    .push((head, String::from_utf8(raw[header_end..].to_vec()).unwrap()));
                tokio::time::sleep(delay).await;
                let header = format!(
                    "HTTP/1.1 {status} fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = sock.write_all(header.as_bytes()).await;
                let _ = sock.write_all(&body).await;
            }
        });
        Self {
            endpoint,
            calls,
            task,
        }
    }
    fn router(&self, c: JevConfig) -> JevRouter {
        JevRouter::with_endpoint(c, Registry::builtins().unwrap(), self.endpoint.clone())
    }
    fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn response(score: Value) -> Vec<u8> {
    ssx_json::to_vec(
        &json!({"model":MODEL,"answers":{"arxiv":{"noul":score},"pubmed":{"noul":score}}}),
    )
    .unwrap()
}
#[test]
fn source_configuration_identity_and_keyless_replay() {
    assert!(JevConfig::from_environment(&BTreeMap::new()).is_none());
    for key in ["", " \t\u{1c}"] {
        assert!(
            JevConfig::from_environment(&BTreeMap::from([("TYPESAFE_API_KEY".into(), key.into())]))
                .is_none()
        );
    }
    for row in fixture()["configs"].as_array().unwrap() {
        let env = BTreeMap::from([
            ("TYPESAFE_API_KEY".into(), " synthetic ".into()),
            (
                "JEV_SPECIALIST_THRESHOLD".into(),
                row["threshold_input"].as_str().unwrap().into(),
            ),
            (
                "JEV_TIMEOUT_MS".into(),
                row["timeout_input"].as_str().unwrap().into(),
            ),
        ]);
        let c = JevConfig::from_environment(&env).unwrap();
        assert_eq!(c.threshold(), row["threshold"].as_f64().unwrap());
        assert_eq!(c.timeout.as_secs_f64(), row["timeout"].as_f64().unwrap());
        assert_eq!(c.cache_identity(), row["identity"].as_str().unwrap());
    }
    let c = config();
    let names = vec!["arxiv".into(), "pubmed".into()];
    assert_eq!(
        c.cache_key(&proposal().request.query, &names),
        fixture()["cases"][0]["cache_keys"][0].as_str().unwrap()
    );
    assert_eq!(
        service()
            .preview(&proposal())
            .unwrap()
            .scope
            .selected_engines,
        vec!["brave"]
    );
}
#[tokio::test]
async fn source_provider_replay_exact_body_boolean_threshold_order_and_failure() {
    for case in fixture()["cases"].as_array().unwrap() {
        let mut body: ssx_json::Value =
            ssx_json::from_slice(&response(case["answer"].clone())).unwrap();
        if case["name"] == "wrong_model" {
            body["model"] = json!("other");
        }
        let p = Provider::start(
            ssx_json::to_vec(&body).unwrap(),
            case["status"].as_u64().unwrap() as u16,
            Duration::ZERO,
        )
        .await;
        let router = p.router(config());
        let d = router.route(&proposal(), &service(), None).await;
        let observed = d.map(|d| json!({"engines":d.engines,"scores":d.scores}));
        let expected = if case["decision"].is_null() {
            None
        } else {
            Some(case["decision"].clone())
        };
        assert_eq!(observed, expected, "{}", case["name"]);
        assert_eq!(p.count(), 1);
        {
            let calls = p.calls.lock().unwrap();
            assert!(calls[0].0.starts_with("POST /v1/systemone HTTP/1.1\r\n"));
            assert!(
                calls[0]
                    .0
                    .to_lowercase()
                    .contains("content-type: application/json")
            );
            assert_eq!(calls[0].1, case["requests"][0]["body"].as_str().unwrap());
            assert!(
                calls[0]
                    .0
                    .to_lowercase()
                    .contains("authorization: bearer synthetic")
            );
        }
        router.shutdown().await;
    }
}
#[tokio::test]
async fn malformed_nonfinite_transport_and_total_deadline_fallback() {
    for body in [
        b"invalid".to_vec(),
        b"{\"model\":\"jev-1.13.0\",\"answers\":{\"arxiv\":{\"noul\":NaN}}}".to_vec(),
        response(json!(1e100)),
        json!({"model":MODEL,"answers":[]}).to_string().into_bytes(),
    ] {
        let p = Provider::start(body, 200, Duration::ZERO).await;
        let r = p.router(config());
        assert!(r.route(&proposal(), &service(), None).await.is_none());
        assert_eq!(p.count(), 1);
        r.shutdown().await;
    }
    let mut c = config();
    c.timeout = Duration::from_millis(50);
    let p = Provider::start(response(json!(0.9)), 200, Duration::from_millis(200)).await;
    let r = p.router(c);
    let start = std::time::Instant::now();
    assert!(r.route(&proposal(), &service(), None).await.is_none());
    assert!(start.elapsed() < Duration::from_millis(180));
    assert_eq!(p.count(), 1);
    r.shutdown().await;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let r = JevRouter::with_endpoint(config(), Registry::builtins().unwrap(), endpoint);
    assert!(r.route(&proposal(), &service(), None).await.is_none());
    r.shutdown().await;
}
#[tokio::test]
async fn eligibility_raw_scope_and_preview_reuse_never_double_call() {
    let p = Provider::start(response(json!(0.9)), 200, Duration::ZERO).await;
    let r = p.router(config());
    let s = service();
    for mut prop in [proposal(), proposal(), proposal()] {
        if p.count() > 0 {
            break;
        }
        prop.request.engines = Some(vec![" ".into()]);
        assert!(r.route(&prop, &s, None).await.is_none());
    }
    let mut prop = proposal();
    prop.request.categories = Some(vec![" ".into()]);
    assert!(r.route(&prop, &s, None).await.is_none());
    prop.request.categories = None;
    prop.request.media_type = Some("image".into());
    assert!(r.route(&prop, &s, None).await.is_none());
    assert_eq!(p.count(), 0);
    let prepared = r.prepare(&proposal(), &s, None).await.unwrap();
    assert_eq!(
        prepared.preview.scope.jev_added_engines,
        vec!["arxiv", "pubmed"]
    );
    let token = prepared.authorize(&s).unwrap();
    s.with_revalidated(&token, |cmd| {
        assert_eq!(
            cmd.preview().scope.selected_engines,
            vec!["brave", "arxiv", "pubmed"]
        )
    })
    .unwrap();
    assert_eq!(p.count(), 1);
    let mut engines: Vec<_> = s.catalog.engines().to_vec();
    for e in &mut engines {
        if e.name != "brave" {
            e.circuit_open = true;
        }
    }
    let mut revoked = s.clone();
    revoked.catalog = CatalogSnapshot::new(engines).unwrap();
    assert!(
        revoked
            .with_revalidated(&token, |_| panic!("revoked dispatch"))
            .is_err()
    );
    assert!(r.candidates(&revoked, &proposal()).is_empty());
    assert_eq!(p.count(), 1);
    r.shutdown().await;
}
#[test]
fn automatic_policy_rechecks_advice_budget_and_current_provenance() {
    let mut s = service();
    let advice = vec![
        ("pubmed".into(), FiniteF64::new(0.99).unwrap()),
        ("hibp".into(), FiniteF64::new(1.).unwrap()),
        ("missing".into(), FiniteF64::new(1.).unwrap()),
    ];
    s.resolver.budget = Some(RoutingBudget {
        max_engines: 1,
        max_cost_class: "free".into(),
        coverage_target: 0,
    });
    let v = s.preview_with_advice(&proposal(), &advice).unwrap();
    assert_eq!(v.scope.selected_engines, vec!["brave"]);
    assert!(v.scope.jev_added_engines.is_empty());
    assert!(
        v.scope
            .excluded_engines
            .iter()
            .any(|e| e.engine == "pubmed" && e.stage == "budget")
    );
    s.resolver.budget.as_mut().unwrap().max_engines = 2;
    let token = s.authorize_with_advice(proposal(), advice.clone()).unwrap();
    assert_eq!(
        token.preview().scope.selected_engines,
        vec!["brave", "pubmed"]
    );
    s.resolver.budget.as_mut().unwrap().max_engines = 1;
    assert!(
        s.with_revalidated(&token, |_| panic!("shrunk budget"))
            .is_err()
    );
    s.resolver.budget = None;
    let mut engines = s.catalog.engines().to_vec();
    engines
        .iter_mut()
        .find(|e| e.name == "pubmed")
        .unwrap()
        .auth_ready = false;
    s.catalog = CatalogSnapshot::new(engines).unwrap();
    assert!(
        !s.preview_with_advice(&proposal(), &advice)
            .unwrap()
            .scope
            .selected_engines
            .contains(&"pubmed".into())
    );
    let mut prop = proposal();
    prop.surface = SearchSurface::Http;
    prop.request.engines = Some(vec![" ".into()]);
    assert!(
        s.preview_with_advice(&prop, &advice)
            .unwrap()
            .scope
            .jev_added_engines
            .is_empty()
    );
    let mut s = service();
    let mut engines = s.catalog.engines().to_vec();
    engines
        .iter_mut()
        .find(|e| e.name == "pubmed")
        .unwrap()
        .available = false;
    s.catalog = CatalogSnapshot::new(engines).unwrap();
    assert!(
        s.preview_with_advice(&proposal(), &advice)
            .unwrap()
            .scope
            .selected_engines
            .contains(&"pubmed".into())
    );
    assert!(s.authorize_with_advice(proposal(), advice).is_err());
    // Metadata must retain the score that actually passed eligibility, not an
    // earlier rejected duplicate supplied through the public advice boundary.
    let s = service();
    let duplicate = vec![
        ("arxiv".into(), FiniteF64::new(-1.0).unwrap()),
        ("arxiv".into(), FiniteF64::new(0.9).unwrap()),
        ("arxiv".into(), FiniteF64::new(0.8).unwrap()),
    ];
    let scope = s
        .preview_with_advice(&proposal(), &duplicate)
        .unwrap()
        .scope;
    assert_eq!(scope.selected_engines, vec!["brave", "arxiv"]);
    assert_eq!(scope.jev_added_engines, vec!["arxiv"]);
    assert_eq!(scope.jev_scores["arxiv"].get(), 0.9);
    assert!(!scope.excluded_engines.iter().any(|e| e.engine == "arxiv"));
}

use std::{
    fs,
    io::Read as _,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Instant,
};
struct Server {
    child: Child,
    dir: PathBuf,
    url: String,
}
impl Server {
    fn start() -> Self {
        let mut bytes = [0; 16];
        fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut bytes)
            .unwrap();
        bytes[6] = (bytes[6] & 15) | 64;
        bytes[8] = (bytes[8] & 63) | 128;
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("ssx-f07-{id}"));
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
    fn connection(&self) -> redis::Connection {
        redis::Client::open(self.url.as_str())
            .unwrap()
            .get_connection()
            .unwrap()
    }
    fn store(&self) -> Valkey {
        Valkey::new(&self.url, Duration::from_millis(300)).unwrap()
    }
    fn stop(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[tokio::test]
async fn real_valkey_cache_exact_bytes_ttl_malformed_miss_and_outage() {
    let mut server = Server::start();
    let store = server.store();
    let p = Provider::start(response(json!(0.9)), 200, Duration::ZERO).await;
    let mut c = config();
    c.cache_ttl = std::num::NonZeroU64::new(1);
    let r = p.router(c);
    let names = r.candidates(&service(), &proposal());
    let key = r.config().cache_key(&proposal().request.query, &names);
    let d = r
        .route(&proposal(), &service(), Some(&store))
        .await
        .unwrap();
    assert_eq!(p.count(), 1);
    let raw = match store.get(&key).await {
        Read::Found(v) => v,
        _ => panic!("cache missing"),
    };
    assert_eq!(
        String::from_utf8(raw).unwrap(),
        "{\"engines\": [\"arxiv\", \"pubmed\"], \"scores\": {\"arxiv\": 0.9, \"pubmed\": 0.9}}"
    );
    let ttl: i64 = redis::cmd("TTL")
        .arg(&key)
        .query(&mut server.connection())
        .unwrap();
    assert!((0..=1).contains(&ttl));
    let hit = r
        .route(&proposal(), &service(), Some(&store))
        .await
        .unwrap();
    assert_eq!(hit.engines, d.engines);
    assert_eq!(hit.latency_ms, 0.0);
    assert_eq!(p.count(), 1);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(matches!(store.get(&key).await, Read::Missing));
    r.route(&proposal(), &service(), Some(&store))
        .await
        .unwrap();
    assert_eq!(p.count(), 2);
    for invalid in [
        json!({"engines":["pubmed","pubmed"],"scores":{"arxiv":0.9,"pubmed":0.9}}),
        json!({"engines":["pubmed","arxiv"],"scores":{"arxiv":0.9,"pubmed":0.9}}),
        json!({"engines":["pubmed"],"scores":{"arxiv":0.9,"pubmed":0.3}}),
        json!({"engines":["arxiv","pubmed"],"scores":{"arxiv":"nan","pubmed":0.9}}),
        json!({"engines":["hibp"],"scores":{"arxiv":0.9,"pubmed":0.9}}),
        json!({"engines":[],"scores":{"arxiv":0.9}}),
        json!({"engines":[],"scores":{"arxiv":true,"pubmed":false}}),
        json!({"engines":["arxiv"],"scores":{"arxiv":1.1,"pubmed":0.0}}),
    ] {
        store
            .put(
                &key,
                &ssx_json::to_vec(&invalid).unwrap(),
                std::num::NonZeroU64::new(2).unwrap(),
            )
            .await
            .unwrap();
        let before = p.count();
        assert!(
            r.route(&proposal(), &service(), Some(&store))
                .await
                .is_some()
        );
        assert_eq!(p.count(), before + 1);
    }
    server.stop();
    let before = p.count();
    assert!(
        r.route(&proposal(), &service(), Some(&store))
            .await
            .is_some()
    );
    assert_eq!(p.count(), before + 1);
    store.close().await;
    r.shutdown().await;
}
#[tokio::test]
async fn invalid_ttl_skips_write_cache_failure_keeps_decision_and_empty_candidates_no_call() {
    let server = Server::start();
    let store = server.store();
    let p = Provider::start(response(json!(0.9)), 200, Duration::ZERO).await;
    let c = JevConfig::from_environment(&BTreeMap::from([
        ("TYPESAFE_API_KEY".into(), "synthetic".into()),
        ("JEV_ROUTING_CACHE_TTL_SECONDS".into(), "bad".into()),
    ]))
    .unwrap();
    assert!(c.cache_ttl.is_none());
    let r = p.router(c);
    r.route(&proposal(), &service(), Some(&store))
        .await
        .unwrap();
    let key = r.config().cache_key(
        &proposal().request.query,
        &r.candidates(&service(), &proposal()),
    );
    assert!(matches!(store.get(&key).await, Read::Missing));
    r.route(&proposal(), &service(), Some(&store))
        .await
        .unwrap();
    assert_eq!(p.count(), 2);
    let mut s = service();
    s.catalog = CatalogSnapshot::new(vec![engine("brave")]).unwrap();
    let d = r.route(&proposal(), &s, Some(&store)).await.unwrap();
    assert!(d.engines.is_empty());
    assert_eq!(d.latency_ms, 0.0);
    assert_eq!(p.count(), 2);
    store.close().await;
    r.shutdown().await;
}
#[test]
fn final_explanations_recompute_coverage_without_duplicate_tradeoffs() {
    let mut s = service();
    s.resolver.budget = Some(RoutingBudget {
        max_engines: 2,
        max_cost_class: "free".into(),
        coverage_target: 2,
    });
    assert_eq!(
        s.preview(&proposal())
            .unwrap()
            .scope
            .routing_tradeoffs
            .iter()
            .filter(|t| t.kind == "coverage")
            .count(),
        1
    );
    let advice = vec![
        ("arxiv".into(), FiniteF64::new(0.9).unwrap()),
        ("pubmed".into(), FiniteF64::new(0.8).unwrap()),
    ];
    let d = s.preview_with_advice(&proposal(), &advice).unwrap().scope;
    assert_eq!(d.selected_engines, vec!["brave", "arxiv"]);
    assert_eq!(d.jev_added_engines, vec!["arxiv"]);
    assert!(!d.routing_tradeoffs.iter().any(|t| t.kind == "coverage"));
    assert_eq!(
        d.routing_tradeoffs
            .iter()
            .filter(|t| t.kind == "cost")
            .count(),
        1
    );
    assert!(
        !d.warnings
            .iter()
            .any(|w| w.contains("below the coverage target"))
    );
    assert!(
        d.excluded_engines
            .iter()
            .any(|e| e.engine == "pubmed" && e.stage == "budget")
    );
    let mut disabled = service();
    disabled.resolver.catalog_routing = false;
    disabled.resolver.topics = None;
    disabled.resolver.budget = Some(RoutingBudget {
        max_engines: 1,
        max_cost_class: "free".into(),
        coverage_target: 2,
    });
    let mut engines = disabled.catalog.engines().to_vec();
    engines
        .iter_mut()
        .find(|e| e.name == "pubmed")
        .unwrap()
        .auth_ready = false;
    disabled.catalog = CatalogSnapshot::new(engines).unwrap();
    let baseline = disabled.preview(&proposal()).unwrap().scope;
    let rejected = vec![("hibp".into(), FiniteF64::new(1.0).unwrap())];
    let advised = disabled
        .preview_with_advice(&proposal(), &rejected)
        .unwrap()
        .scope;
    assert_eq!(
        advised.selected_engines, baseline.selected_engines,
        "rejected advice must not evict disabled-routing baseline"
    );
    assert_eq!(advised.routing_fallback, baseline.routing_fallback);
    assert!(advised.jev_added_engines.is_empty());
    assert!(advised.jev_scores.is_empty());

    disabled.resolver.topics = Some(vec![]);
    disabled.resolver.budget.as_mut().unwrap().max_engines = 2;
    let mut engines = disabled.catalog.engines().to_vec();
    let brave = engines.iter_mut().find(|e| e.name == "brave").unwrap();
    brave.auth_ready = false;
    brave.cost_class = "paid".into();
    disabled.catalog = CatalogSnapshot::new(engines).unwrap();
    let baseline = disabled.preview(&proposal()).unwrap().scope;
    let addition = vec![("arxiv".into(), FiniteF64::new(0.9).unwrap())];
    let advised = disabled
        .preview_with_advice(&proposal(), &addition)
        .unwrap()
        .scope;
    assert_eq!(baseline.selected_engines, vec!["brave"]);
    assert_eq!(advised.selected_engines, vec!["brave", "arxiv"]);
    assert_eq!(advised.jev_added_engines, vec!["arxiv"]);
    assert_eq!(
        advised
            .jev_scores
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["arxiv"]
    );
    assert!(
        advised
            .jev_added_engines
            .iter()
            .all(|n| advised.selected_engines.contains(n))
    );
}

#[tokio::test]
async fn source_scope_choices_explanations_and_keyless_fallback_replay() {
    for row in fixture()["scopes"].as_array().unwrap() {
        if row["name"] == "budget_bypass" {
            continue;
        }
        let mut prop = proposal();
        if row["name"] == "explicit" {
            prop.request.engines = Some(vec!["brave".into()]);
        }
        let s = service();
        let advice = if row["name"] == "enhanced" {
            vec![
                ("arxiv".into(), FiniteF64::new(0.9).unwrap()),
                ("pubmed".into(), FiniteF64::new(0.9).unwrap()),
            ]
        } else {
            vec![]
        };
        let preview = s.preview_with_advice(&prop, &advice).unwrap();
        assert_eq!(
            ssx_json::to_value(preview.scope).unwrap(),
            row["scope"],
            "{}",
            row["name"]
        );
        if row["name"] == "keyless" {
            let prepared = prepare_optional(None, &prop, &s, None).await.unwrap();
            assert_eq!(
                ssx_json::to_value(&prepared.preview.scope).unwrap(),
                row["scope"]
            );
            prepared.authorize(&s).unwrap();
        }
    }
    let hazards: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f12/hazards.json")).unwrap();
    assert_eq!(hazards["cached_nan_duplicate"]["score_repr"], "nan");
    assert_eq!(
        hazards["budget_bypass"]["selected"],
        json!(["brave", "pubmed"])
    );
}
#[test]
fn revoked_grants_sensitive_policy_and_strict_filters_cannot_admit_advice() {
    let mut s = service();
    let advice = vec![("arxiv".into(), FiniteF64::new(0.9).unwrap())];
    let token = s.authorize_with_advice(proposal(), advice.clone()).unwrap();
    s.policy.sensitive_engines.insert("arxiv".into());
    s.policy.sensitive_allowed = true;
    assert!(
        s.with_revalidated(&token, |_| panic!("sensitive advice"))
            .is_err()
    );
    assert!(
        !s.preview_with_advice(&proposal(), &advice)
            .unwrap()
            .scope
            .selected_engines
            .contains(&"arxiv".into())
    );
    let mut prop = proposal();
    prop.intent = "science".into();
    s.policy.grants.insert("science".into());
    let token = s
        .authorize_with_advice(prop.clone(), advice.clone())
        .unwrap();
    s.policy.grants.clear();
    assert!(
        s.with_revalidated(&token, |_| panic!("revoked grant"))
            .is_err()
    );
    let mut prop = proposal();
    prop.request.safesearch = 2;
    assert!(s.preview_with_advice(&prop, &advice).is_err());
}

#[tokio::test]
async fn source_overflow_per_field_corrections_and_signed_zero_order() {
    let source: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/f12/overflow.json")).unwrap();
    let server = Server::start();
    let store = server.store();
    let p = Provider::start(response(json!(0.9)), 200, Duration::ZERO).await;
    for row in source["cases"].as_array().unwrap() {
        let field = row["field"].as_str().unwrap();
        let text = row["input"].as_str().unwrap();
        let mut env = BTreeMap::from([
            ("TYPESAFE_API_KEY".into(), "synthetic".into()),
            ("JEV_SPECIALIST_THRESHOLD".into(), "-0.0".into()),
        ]);
        env.insert(
            if field == "timeout" {
                "JEV_TIMEOUT_MS"
            } else {
                "JEV_ROUTING_CACHE_TTL_SECONDS"
            }
            .into(),
            text.into(),
        );
        let config = JevConfig::from_environment(&env);
        if field == "timeout" {
            if text.len() > 19 && !text.starts_with('-') {
                assert!(config.is_none());
            } else {
                let c = config.unwrap();
                assert_eq!(c.threshold(), 0.0);
                assert!(!c.threshold().is_sign_negative());
                assert_eq!(c.cache_identity(), row["identity"].as_str().unwrap());
                assert_eq!(
                    c.timeout.as_secs_f64(),
                    row["timeout_repr"]
                        .as_str()
                        .unwrap()
                        .parse::<f64>()
                        .unwrap()
                );
            }
        } else {
            let c = config.unwrap();
            let r = p.router(c);
            let mut s = service();
            s.catalog = CatalogSnapshot::new(vec![engine("pubmed")]).unwrap();
            let before = p.count();
            let d = r.route(&proposal(), &s, Some(&store)).await.unwrap();
            assert_eq!(
                json!({"engines":d.engines,"scores":d.scores}),
                row["decision"]
            );
            assert_eq!(p.count(), before + 1);
            let key = r
                .config()
                .cache_key(&proposal().request.query, &r.candidates(&s, &proposal()));
            if text.starts_with('-') {
                assert!(matches!(store.get(&key).await, Read::Found(_)));
            } else {
                assert!(matches!(store.get(&key).await, Read::Missing));
            }
            r.shutdown().await;
        }
    }
    let body = json!({"model":MODEL,"answers":{"arxiv":{"noul":-0.0},"pubmed":{"noul":0.0},"hibp":{"noul":1.0}}});
    let zero = Provider::start(ssx_json::to_vec(&body).unwrap(), 200, Duration::ZERO).await;
    let c = JevConfig::from_environment(&BTreeMap::from([
        ("TYPESAFE_API_KEY".into(), "synthetic".into()),
        ("JEV_SPECIALIST_THRESHOLD".into(), "-0.0".into()),
    ]))
    .unwrap();
    let r = zero.router(c);
    let d = r.route(&proposal(), &service(), None).await.unwrap();
    assert_eq!(d.engines, vec!["arxiv", "pubmed"]);
    assert_eq!(zero.count(), 1);
    r.shutdown().await;
    store.close().await;
}
