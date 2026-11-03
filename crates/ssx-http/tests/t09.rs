//! Actual TLS loopback IdP/browser peers and private Valkey only. No provider calls.
#[path = "support/t03_peer.rs"]
mod t03_peer;
use aws_lc_rs::{
    rand::SystemRandom,
    rsa::KeySize,
    signature::{self, KeyPair as _},
};
use axum::{
    Json, Router,
    extract::{Query, Request, State},
    response::IntoResponse,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use ssx_application::{PortFuture, browser_identity::*};
use ssx_domain::browser_identity::*;
use ssx_http::browser_identity::*;
use ssx_infra::{
    browser_identity::*,
    valkey::{Valkey, browser_identity::BrowserStore},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read as _,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
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
        let dir = std::env::temp_dir().join(format!("ssx-t09-{id}"));
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
impl Drop for PrivateValkey {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}
#[derive(Clone)]
struct Directory(Arc<Mutex<BrowserPrincipal>>);
impl BrowserDirectory for Directory {
    fn subject<'a>(
        &'a self,
        issuer: &'a str,
        subject: &'a str,
    ) -> PortFuture<'a, Result<Option<BrowserPrincipal>, BrowserError>> {
        Box::pin(async move {
            let p = self.0.lock().unwrap().clone();
            Ok((p.issuer == issuer && p.subject == subject).then_some(p))
        })
    }
    fn principal<'a>(
        &'a self,
        id: &'a str,
    ) -> PortFuture<'a, Result<Option<BrowserPrincipal>, BrowserError>> {
        Box::pin(async move {
            let p = self.0.lock().unwrap().clone();
            Ok((p.principal_id == id).then_some(p))
        })
    }
}
#[derive(Default)]
struct Workflows {
    calls: AtomicUsize,
    received: Mutex<Vec<Vec<(String, String)>>>,
    debug: Mutex<Vec<String>>,
    fail: std::sync::atomic::AtomicBool,
}
impl BrowserWorkflow for Workflows {
    fn read<'a>(
        &'a self,
        c: &'a BrowserContext,
        kind: Option<&'a str>,
        id: Option<&'a str>,
    ) -> PortFuture<'a, Result<String, BrowserError>> {
        Box::pin(async move {
            if kind.is_some()
                && id
                    != Some(if c.tenant_id == "a" {
                        "a-object"
                    } else {
                        "b-object"
                    })
            {
                return Err(BrowserError::Missing);
            }
            Ok(format!("{}:{}", c.tenant_label, c.tenant_id))
        })
    }
    fn mutate<'a>(
        &'a self,
        c: &'a BrowserContext,
        command: BrowserMutation<'a>,
    ) -> PortFuture<'a, Result<(), BrowserError>> {
        Box::pin(async move {
            if command.id
                != if c.tenant_id == "a" {
                    "a-object"
                } else {
                    "b-object"
                }
            {
                return Err(BrowserError::Missing);
            }
            if command.revision != 1 {
                return Err(BrowserError::Conflict);
            }
            self.received.lock().unwrap().push(command.fields.to_vec());
            self.debug.lock().unwrap().push(format!("{command:?}"));
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                Err(BrowserError::Unavailable)
            } else {
                Ok(())
            }
        })
    }
}
type FakeCodes = Arc<Mutex<BTreeMap<String, (String, String, String)>>>;
#[derive(Clone)]
struct FakeIdp {
    issuer: String,
    key: Arc<signature::RsaKeyPair>,
    codes: FakeCodes,
    claims: Arc<Mutex<Value>>,
    token_calls: Arc<AtomicUsize>,
    discovery_calls: Arc<AtomicUsize>,
}
fn sign(key: &signature::RsaKeyPair, header: &str, claims: &str) -> String {
    let input = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header),
        URL_SAFE_NO_PAD.encode(claims)
    );
    let mut sig = vec![0; key.public_modulus_len()];
    key.sign(
        &signature::RSA_PKCS1_SHA256,
        &SystemRandom::new(),
        input.as_bytes(),
        &mut sig,
    )
    .unwrap();
    format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig))
}
async fn metadata(State(s): State<FakeIdp>) -> Json<Value> {
    s.discovery_calls.fetch_add(1, Ordering::SeqCst);
    Json(
        json!({"issuer":s.issuer,"authorization_endpoint":format!("{}/authorize",s.issuer),"token_endpoint":format!("{}/token",s.issuer),"jwks_uri":format!("{}/jwks",s.issuer),"code_challenge_methods_supported":["S256"]}),
    )
}
async fn jwks(State(s): State<FakeIdp>) -> Json<Value> {
    let key = s.key.public_key();
    Json(
        json!({"keys":[{"kty":"RSA","kid":"test","use":"sig","alg":"RS256","n":URL_SAFE_NO_PAD.encode(key.modulus().big_endian_without_leading_zero()),"e":URL_SAFE_NO_PAD.encode(key.exponent().big_endian_without_leading_zero())}]}),
    )
}
async fn authorize(
    State(s): State<FakeIdp>,
    Query(q): Query<BTreeMap<String, String>>,
) -> axum::response::Response {
    assert_eq!(q["client_id"], "client");
    assert_eq!(q["response_type"], "code");
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(q["scope"], "openid");
    let code = format!("code-{}", s.codes.lock().unwrap().len());
    s.codes.lock().unwrap().insert(
        code.clone(),
        (
            q["nonce"].clone(),
            q["code_challenge"].clone(),
            q["redirect_uri"].clone(),
        ),
    );
    let mut location = url::Url::parse(&q["redirect_uri"]).unwrap();
    location
        .query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", &q["state"]);
    axum::response::Redirect::to(location.as_str()).into_response()
}
async fn token(State(s): State<FakeIdp>, request: Request) -> axum::response::Response {
    s.token_calls.fetch_add(1, Ordering::SeqCst);
    assert_eq!(
        request.headers()["authorization"],
        "Basic Y2xpZW50OnN5bnRoZXRpYy1jbGllbnQtc2VjcmV0"
    );
    let b = axum::body::to_bytes(request.into_body(), 16384)
        .await
        .unwrap();
    let q: BTreeMap<_, _> = url::form_urlencoded::parse(&b)
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let code = match s.codes.lock().unwrap().remove(&q["code"]) {
        Some(v) => v,
        None => return axum::http::StatusCode::BAD_REQUEST.into_response(),
    };
    use aws_lc_rs::digest;
    assert_eq!(q["redirect_uri"], code.2);
    assert_eq!(
        URL_SAFE_NO_PAD
            .encode(digest::digest(&digest::SHA256, q["code_verifier"].as_bytes()).as_ref()),
        code.1
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut claims = json!({"iss":s.issuer,"aud":"client","sub":"subject","iat":now,"exp":now+3600,"nonce":code.0});
    for (k, v) in s.claims.lock().unwrap().as_object().unwrap() {
        claims[k] = v.clone()
    }
    Json(json!({"id_token":sign(&s.key,r#"{"alg":"RS256","kid":"test"}"#,&claims.to_string())}))
        .into_response()
}
struct Running {
    url: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn listener() -> tokio::net::TcpListener {
    tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap()
}
fn tls() -> (Arc<rustls::ServerConfig>, reqwest::Certificate) {
    let c =
        rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()]).unwrap();
    let root = reqwest::Certificate::from_der(c.cert.der()).unwrap();
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![c.cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(c.signing_key.serialize_der()).into(),
        )
        .unwrap();
    (Arc::new(config), root)
}
fn run(
    listener: tokio::net::TcpListener,
    tls: Arc<rustls::ServerConfig>,
    router: Router,
) -> Running {
    let url = format!("https://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(
            BrowserTlsListener::new(listener, tls),
            router.into_make_service_with_connect_info::<BrowserTlsPeer>(),
        )
        .await
        .unwrap()
    });
    Running { url, task }
}
struct Fixture {
    valkey: PrivateValkey,
    idp: FakeIdp,
    _idp_server: Running,
    portal: Running,
    directory: Directory,
    workflows: Arc<Workflows>,
    authority: Arc<BrowserAuthority>,
    client: reqwest::Client,
    root: reqwest::Certificate,
    settings: OidcSettings,
}
impl Fixture {
    async fn new(multi: bool) -> Self {
        let valkey = PrivateValkey::start();
        let (config, root) = tls();
        let idp_listener = listener().await;
        let issuer = format!("https://{}", idp_listener.local_addr().unwrap());
        let idp = FakeIdp {
            issuer: issuer.clone(),
            key: Arc::new(signature::RsaKeyPair::generate(KeySize::Rsa2048).unwrap()),
            codes: Arc::new(Mutex::new(BTreeMap::new())),
            claims: Arc::new(Mutex::new(json!({}))),
            token_calls: Arc::new(AtomicUsize::new(0)),
            discovery_calls: Arc::new(AtomicUsize::new(0)),
        };
        let idp_router = Router::new()
            .route("/.well-known/openid-configuration", get(metadata))
            .route("/authorize", get(authorize))
            .route("/token", post(token))
            .route("/jwks", get(jwks))
            .with_state(idp.clone());
        let idp_server = run(idp_listener, config.clone(), idp_router);
        let portal_listener = listener().await;
        let portal_origin = format!("https://{}", portal_listener.local_addr().unwrap());
        let mut memberships = vec![BrowserMembership {
            tenant_id: "a".into(),
            display_name: "Tenant A".into(),
            grants: ["workflow.read", "workflow.research.cancel"]
                .into_iter()
                .map(String::from)
                .collect(),
            revision: 1,
            enabled: true,
        }];
        if multi {
            let mut b = memberships[0].clone();
            b.tenant_id = "b".into();
            b.display_name = "Tenant B".into();
            memberships.push(b)
        }
        let directory = Directory(Arc::new(Mutex::new(BrowserPrincipal {
            principal_id: "principal".into(),
            issuer: issuer.clone(),
            subject: "subject".into(),
            memberships,
            revision: 1,
            enabled: true,
        })));
        let settings = OidcSettings::new(
            issuer,
            "client".into(),
            secret("synthetic-client-secret"),
            format!("{portal_origin}/auth/callback"),
        )
        .unwrap();
        let authority = Arc::new(
            BrowserAuthority::new(
                BrowserStore::new(Arc::new(valkey.store())),
                Arc::new(directory.clone()),
                BrowserAuthorityConfig {
                    settings: settings.clone(),
                    keys: BrowserKeys::new(secret(&"s".repeat(32)), None).unwrap(),
                    audit_key: secret(&"a".repeat(32)),
                    roots: vec![root.clone()],
                    idle_seconds: 60,
                    absolute_seconds: 120,
                },
            )
            .await
            .unwrap(),
        );
        let workflows = Arc::new(Workflows::default());
        let runtime = BrowserRuntime::new(
            authority.clone(),
            workflows.clone(),
            &portal_origin,
            actions(),
            ["workflow.research.cancel", "workflow.compose"]
                .into_iter()
                .map(String::from)
                .collect(),
            false,
        )
        .unwrap();
        // Public routes are independent stand-ins proving mounting/absence of auth side effects,
        // not a claim that T02's production search rendering is complete.
        let app = browser_router(runtime)
            .route("/", get(|| async { "public search" }))
            .route("/search", get(|| async { "public results" }));
        let portal = run(portal_listener, config, app);
        let client = reqwest::Client::builder()
            .add_root_certificate(root.clone())
            .redirect(reqwest::redirect::Policy::none())
            .cookie_store(true)
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        Self {
            valkey,
            idp,
            _idp_server: idp_server,
            portal,
            directory,
            workflows,
            authority,
            client,
            root,
            settings,
        }
    }
    async fn login(&self) -> reqwest::Response {
        let start = self
            .client
            .get(format!("{}/auth/login", self.portal.url))
            .send()
            .await
            .unwrap();
        assert_eq!(start.status(), 303);
        let authorization = start.headers()["location"].to_str().unwrap();
        let redirect = self.client.get(authorization).send().await.unwrap();
        assert_eq!(redirect.status(), 303);
        self.client
            .get(redirect.headers()["location"].to_str().unwrap())
            .send()
            .await
            .unwrap()
    }
    async fn get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(format!("{}{path}", self.portal.url))
            .send()
            .await
            .unwrap()
    }
    async fn post(&self, path: &str, fields: &[(&str, &str)], origin: &str) -> reqwest::Response {
        let b = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(fields.iter().copied())
            .finish();
        self.client
            .post(format!("{}{path}", self.portal.url))
            .header("origin", origin)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(b)
            .send()
            .await
            .unwrap()
    }
}
fn secret(s: &str) -> BrowserSecret {
    BrowserSecret::new(s.into()).unwrap()
}
fn actions() -> BTreeSet<String> {
    BROWSER_ACTIONS.iter().map(|s| (*s).into()).collect()
}
fn csrf(html: &str) -> String {
    html.split("name=\"csrf\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .into()
}
#[tokio::test]
async fn actual_oidc_tls_cookie_flow_and_public_search_equivalence() {
    let f = Fixture::new(false).await;
    let before = f.get("/").await.text().await.unwrap();
    let without = f.get("/search").await.text().await.unwrap();
    let denied = f.get("/workflows").await;
    assert_eq!(denied.status(), 303);
    let login = f.login().await;
    assert_eq!(login.status(), 303);
    let cookies: Vec<_> = login
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap().to_owned())
        .collect();
    assert!(
        cookies
            .iter()
            .any(|v| v.starts_with("__Host-slopsearx_session=")
                && v.contains("Secure")
                && v.contains("HttpOnly")
                && v.contains("SameSite=Lax")
                && !v.contains("Domain"))
    );
    assert_eq!(f.idp.token_calls.load(Ordering::SeqCst), 1);
    let body = f.get("/workflows").await.text().await.unwrap();
    assert!(body.contains("Tenant A"));
    assert_eq!(f.get("/").await.text().await.unwrap(), before);
    assert_eq!(f.get("/search").await.text().await.unwrap(), without);
    let session_cookie = cookies
        .iter()
        .find(|value| value.starts_with("__Host-slopsearx_session="))
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    t03_peer::public_cookie_equivalence(session_cookie).await;
    let invalid = f
        .client
        .get(format!("{}/", f.portal.url))
        .header("cookie", "__Host-slopsearx_session=invalid")
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.text().await.unwrap(), before);
    assert_eq!(f.idp.discovery_calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn mutations_require_origin_csrf_and_done_replays_do_not_repeat_action() {
    let f = Fixture::new(false).await;
    assert_eq!(f.login().await.status(), 303);
    let token = csrf(&f.get("/workflows").await.text().await.unwrap());
    let fields = [
        ("csrf", token.as_str()),
        ("expected_revision", "1"),
        ("idempotency_key", "synthetic-idempotency"),
    ];
    assert_eq!(
        f.post(
            "/workflows/research/a-object/cancel",
            &fields,
            "https://evil.example"
        )
        .await
        .status(),
        403
    );
    assert_eq!(f.workflows.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        f.post(
            "/workflows/research/a-object/cancel",
            &fields,
            &f.portal.url
        )
        .await
        .status(),
        303
    );
    assert_eq!(
        f.post(
            "/workflows/research/a-object/cancel",
            &fields,
            &f.portal.url
        )
        .await
        .status(),
        303
    );
    assert_eq!(f.workflows.calls.load(Ordering::SeqCst), 1);
    assert!(
        f.workflows
            .received
            .lock()
            .unwrap()
            .iter()
            .flatten()
            .all(|(k, v)| k != "csrf" && v != &token)
    );
    assert!(
        f.workflows
            .debug
            .lock()
            .unwrap()
            .iter()
            .all(|v| !v.contains(&token))
    );
    let altered = [
        ("csrf", token.as_str()),
        ("expected_revision", "2"),
        ("idempotency_key", "synthetic-idempotency"),
    ];
    assert_eq!(
        f.post(
            "/workflows/research/a-object/cancel",
            &altered,
            &f.portal.url
        )
        .await
        .status(),
        409
    );
    assert_eq!(
        f.post("/auth/logout", &[("csrf", token.as_str())], &f.portal.url)
            .await
            .status(),
        303
    );
    assert_eq!(f.get("/workflows").await.status(), 303);
}
#[tokio::test]
async fn two_tenants_rotate_cookie_and_foreign_objects_are_uniformly_unavailable() {
    let f = Fixture::new(true).await;
    assert_eq!(f.login().await.status(), 303);
    assert_eq!(f.get("/workflows").await.status(), 303);
    let chooser = f.get("/auth/tenant").await.text().await.unwrap();
    assert!(chooser.contains("Tenant A") && chooser.contains("Tenant B"));
    let token = csrf(&chooser);
    assert_eq!(
        f.post(
            "/auth/tenant",
            &[("csrf", &token), ("tenant", "foreign")],
            &f.portal.url
        )
        .await
        .status(),
        403
    );
    let rotated = f
        .post(
            "/auth/tenant",
            &[("csrf", &token), ("tenant", "b")],
            &f.portal.url,
        )
        .await;
    assert_eq!(rotated.status(), 303);
    let own = f.get("/workflows/research/b-object").await;
    assert_eq!(own.status(), 200);
    let foreign = f.get("/workflows/research/a-object").await;
    let unknown = f.get("/workflows/research/unknown").await;
    assert_eq!(foreign.status(), 404);
    assert_eq!(unknown.status(), 404);
    assert_eq!(foreign.text().await.unwrap(), unknown.text().await.unwrap());
    let page = f.get("/workflows").await.text().await.unwrap();
    assert!(page.contains("Tenant B"));
}
#[tokio::test]
async fn signed_invalid_claims_and_replayed_callback_never_create_session() {
    let f = Fixture::new(false).await;
    *f.idp.claims.lock().unwrap() = json!({"aud":"attacker"});
    assert_eq!(f.login().await.status(), 401);
    assert_eq!(f.get("/workflows").await.status(), 303);
    *f.idp.claims.lock().unwrap() = json!({});
    let start = f
        .client
        .get(format!("{}/auth/login", f.portal.url))
        .send()
        .await
        .unwrap();
    let auth = f
        .client
        .get(start.headers()["location"].to_str().unwrap())
        .send()
        .await
        .unwrap();
    let location = auth.headers()["location"].to_str().unwrap();
    let success = f.client.get(location).send().await.unwrap();
    assert_eq!(success.status(), 303);
    let n = f.idp.token_calls.load(Ordering::SeqCst);
    assert_eq!(f.client.get(location).send().await.unwrap().status(), 401);
    assert_eq!(f.idp.token_calls.load(Ordering::SeqCst), n);
}
#[tokio::test]
async fn revoked_membership_and_valkey_failure_deny_protected_but_not_public() {
    let mut f = Fixture::new(false).await;
    assert_eq!(f.login().await.status(), 303);
    f.directory.0.lock().unwrap().memberships[0].revision = 2;
    assert_eq!(f.get("/workflows").await.status(), 303);
    f.directory.0.lock().unwrap().memberships[0].revision = 1;
    f.valkey.stop();
    let denied = f
        .client
        .get(format!("{}/workflows", f.portal.url))
        .header("accept", "application/json")
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 401);
    assert_eq!(f.get("/").await.status(), 200);
}
fn handle(response: &reqwest::Response) -> BrowserSecret {
    let field = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap())
        .find(|s| s.starts_with("__Host-slopsearx_session="))
        .unwrap();
    secret(field.split(';').next().unwrap().split_once('=').unwrap().1)
}
impl Fixture {
    async fn second(&self, keys: BrowserKeys) -> Arc<BrowserAuthority> {
        Arc::new(
            BrowserAuthority::new(
                BrowserStore::new(Arc::new(self.valkey.store())),
                Arc::new(self.directory.clone()),
                BrowserAuthorityConfig {
                    settings: self.settings.clone(),
                    keys,
                    audit_key: secret(&"a".repeat(32)),
                    roots: vec![self.root.clone()],
                    idle_seconds: 60,
                    absolute_seconds: 120,
                },
            )
            .await
            .unwrap(),
        )
    }
    fn session_records(&self) -> Vec<(String, String)> {
        let mut c = self.valkey.connection();
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg("portal:native:v1:*:session:*")
            .query(&mut c)
            .unwrap();
        keys.into_iter()
            .map(|k| {
                let raw: String = redis::cmd("GET").arg(&k).query(&mut c).unwrap();
                (k, raw)
            })
            .collect()
    }
}
#[tokio::test]
async fn two_independent_authorities_refresh_rotate_logout_without_session_revival() {
    let f = Fixture::new(true).await;
    let h = handle(&f.login().await);
    let other = f
        .second(BrowserKeys::new(secret(&"s".repeat(32)), None).unwrap())
        .await;
    let pending = f.authority.resolve(&h, &actions()).await.unwrap();
    let selected = f.authority.rotate(&h, &pending, "a").await.unwrap();
    let h = selected.handle;
    let a = f.authority.clone();
    let b = other.clone();
    let allowed = actions();
    let (x, y) = tokio::join!(a.resolve(&h, &allowed), b.resolve(&h, &allowed));
    assert!(x.is_ok() && y.is_ok());
    let s = x.unwrap();
    let (rotate, stale) = tokio::join!(a.rotate(&h, &s, "b"), b.resolve(&h, &allowed));
    let rotated = match rotate {
        Ok(rotated) => rotated,
        Err(BrowserError::Conflict) => {
            let current = a.resolve(&h, &allowed).await.unwrap();
            assert_eq!(current.active_tenant, s.active_tenant);
            assert_eq!(current.generation, s.generation);
            a.rotate(&h, &current, "b").await.unwrap()
        }
        Err(error) => panic!("unexpected rotation error: {error:?}"),
    };
    assert!(matches!(
        stale,
        Ok(_) | Err(BrowserError::Missing) | Err(BrowserError::Conflict)
    ));
    assert_eq!(
        other.resolve(&h, &actions()).await.unwrap_err(),
        BrowserError::Missing
    );
    assert_eq!(
        f.authority
            .resolve(&rotated.handle, &actions())
            .await
            .unwrap()
            .active_tenant
            .as_deref(),
        Some("b")
    );
    let (refresh, logout) = tokio::join!(
        a.resolve(&rotated.handle, &allowed),
        b.revoke(&rotated.handle)
    );
    assert!(logout.is_ok());
    assert!(matches!(
        refresh,
        Ok(_) | Err(BrowserError::Missing) | Err(BrowserError::Conflict)
    ));
    for authority in [a, b] {
        assert_eq!(
            authority
                .resolve(&rotated.handle, &actions())
                .await
                .unwrap_err(),
            BrowserError::Missing
        );
    }
    assert!(f.session_records().is_empty());
}
#[tokio::test]
async fn stale_exact_record_refresh_and_rotation_never_restore_deleted_state() {
    let f = Fixture::new(false).await;
    let h = handle(&f.login().await);
    let before = f.session_records().remove(0);
    let record: Value = serde_json::from_str(&before.1).unwrap();
    let store = BrowserStore::new(Arc::new(f.valkey.store()));
    f.authority.resolve(&h, &actions()).await.unwrap();
    let ttl = record["absolute_expires_ms"].as_u64().unwrap();
    assert_eq!(
        store
            .change("refresh", &[&before.0], &before.1, &record, ttl)
            .await,
        Err(BrowserError::Conflict)
    );
    let refreshed = f.session_records();
    assert_eq!(
        store
            .change("delete", &[&before.0], &before.1, &record, ttl)
            .await,
        Err(BrowserError::Conflict)
    );
    assert_eq!(f.session_records(), refreshed);
    f.authority.revoke(&h).await.unwrap();
    assert_eq!(
        store
            .change("refresh", &[&before.0], &before.1, &record, ttl)
            .await,
        Err(BrowserError::Missing)
    );
    assert_eq!(
        store
            .change(
                "rotate",
                &[&before.0, "t09-never-created"],
                &before.1,
                &record,
                ttl
            )
            .await,
        Err(BrowserError::Missing)
    );
    let mut c = f.valkey.connection();
    assert_eq!(
        redis::cmd("EXISTS")
            .arg("t09-never-created")
            .query::<u64>(&mut c)
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn revision_revocation_is_durable_and_operator_grants_only_intersect() {
    let f = Fixture::new(false).await;
    let h = handle(&f.login().await);
    let view = f
        .authority
        .resolve(&h, &["workflow.read".into()].into_iter().collect())
        .await
        .unwrap();
    assert_eq!(
        view.context.unwrap().grants,
        ["workflow.read".into()].into_iter().collect()
    );
    f.directory.0.lock().unwrap().memberships[0].revision = 2;
    assert_eq!(
        f.authority.resolve(&h, &actions()).await.unwrap_err(),
        BrowserError::Revoked
    );
    assert!(f.session_records().is_empty());
    f.directory.0.lock().unwrap().memberships[0].revision = 1;
    assert_eq!(
        f.authority.resolve(&h, &actions()).await.unwrap_err(),
        BrowserError::Missing
    );
    let h = handle(&f.login().await);
    f.directory.0.lock().unwrap().revision = 2;
    assert_eq!(
        f.authority.resolve(&h, &actions()).await.unwrap_err(),
        BrowserError::Revoked
    );
    f.directory.0.lock().unwrap().revision = 1;
    assert_eq!(
        f.authority.resolve(&h, &actions()).await.unwrap_err(),
        BrowserError::Missing
    );
}
#[tokio::test]
async fn previous_key_drain_is_verification_only_and_expiry_cannot_extend_absolute_lifetime() {
    let f = Fixture::new(false).await;
    let h = handle(&f.login().await);
    let previous = f
        .second(
            BrowserKeys::new(secret(&"n".repeat(32)), Some((secret(&"s".repeat(32)), 60))).unwrap(),
        )
        .await;
    let s = previous.resolve(&h, &actions()).await.unwrap();
    let rotated = previous.rotate(&h, &s, "a").await.unwrap();
    assert_eq!(rotated.session.created_ms, s.created_ms);
    assert_eq!(rotated.session.absolute_expires_ms, s.absolute_expires_ms);
    assert_eq!(
        f.authority
            .resolve(&rotated.handle, &actions())
            .await
            .unwrap_err(),
        BrowserError::Missing
    );
    assert!(BrowserKeys::new(secret(&"n".repeat(32)), Some((secret(&"s".repeat(32)), 0))).is_err());
    assert!(
        BrowserKeys::new(
            secret(&"n".repeat(32)),
            Some((secret(&"s".repeat(32)), 601))
        )
        .is_err()
    );
    let (key, raw) = f.session_records().remove(0);
    let mut record: Value = serde_json::from_str(&raw).unwrap();
    let now = BrowserStore::new(Arc::new(f.valkey.store()))
        .now()
        .await
        .unwrap();
    record["idle_expires_ms"] = json!(now + 20);
    let mut c = f.valkey.connection();
    redis::cmd("SET")
        .arg(&key)
        .arg(record.to_string())
        .query::<()>(&mut c)
        .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        previous
            .resolve(&rotated.handle, &actions())
            .await
            .unwrap_err(),
        BrowserError::Expired
    );
    let current: String = redis::cmd("GET").arg(&key).query(&mut c).unwrap();
    assert_eq!(current, record.to_string());
}
#[tokio::test]
async fn malformed_wrong_type_oversized_and_duplicate_session_records_deny_without_write() {
    let f = Fixture::new(false).await;
    let h = handle(&f.login().await);
    let (key, raw) = f.session_records().remove(0);
    let mut c = f.valkey.connection();
    for bad in [
        "{".to_owned(),
        "x".repeat(65537),
        raw.replacen("\"schema\":1", "\"schema\":1,\"schema\":1", 1),
        raw.replace("\"schema\":1", "\"schema\":2"),
    ] {
        redis::cmd("SET")
            .arg(&key)
            .arg(&bad)
            .query::<()>(&mut c)
            .unwrap();
        assert_eq!(
            f.authority.resolve(&h, &actions()).await.unwrap_err(),
            BrowserError::Malformed
        );
        let after: String = redis::cmd("GET").arg(&key).query(&mut c).unwrap();
        assert_eq!(after, bad);
    }
    redis::cmd("DEL").arg(&key).query::<()>(&mut c).unwrap();
    redis::cmd("LPUSH")
        .arg(&key)
        .arg("synthetic")
        .query::<()>(&mut c)
        .unwrap();
    assert_eq!(
        f.authority.resolve(&h, &actions()).await.unwrap_err(),
        BrowserError::Malformed
    );
    assert_eq!(
        redis::cmd("LRANGE")
            .arg(&key)
            .arg(0)
            .arg(-1)
            .query::<Vec<String>>(&mut c)
            .unwrap(),
        vec!["synthetic"]
    );
}
#[tokio::test]
async fn competing_admission_and_stale_finish_fence_actions_and_unknown_failures() {
    let f = Fixture::new(false).await;
    let h = handle(&f.login().await);
    let c = f
        .authority
        .resolve(&h, &actions())
        .await
        .unwrap()
        .context
        .unwrap();
    let other = f
        .second(BrowserKeys::new(secret(&"s".repeat(32)), None).unwrap())
        .await;
    let digest = "d".repeat(64);
    let (a, b) = tokio::join!(
        f.authority
            .claim_mutation(&c, "workflow.research.cancel", "same-idempotency", &digest),
        other.claim_mutation(&c, "workflow.research.cancel", "same-idempotency", &digest)
    );
    let claim = match (a, b) {
        (Ok(BrowserAdmission::Claimed(v)), Err(BrowserError::Conflict))
        | (Err(BrowserError::Conflict), Ok(BrowserAdmission::Claimed(v))) => v,
        other => panic!("unexpected {other:?}"),
    };
    assert_eq!(
        f.authority
            .finish_mutation(
                &c,
                "workflow.research.cancel",
                "same-idempotency",
                &digest,
                &secret("stale-claim")
            )
            .await,
        Err(BrowserError::Conflict)
    );
    f.authority
        .finish_mutation(
            &c,
            "workflow.research.cancel",
            "same-idempotency",
            &digest,
            &claim,
        )
        .await
        .unwrap();
    assert!(matches!(
        other
            .claim_mutation(&c, "workflow.research.cancel", "same-idempotency", &digest)
            .await
            .unwrap(),
        BrowserAdmission::Replay
    ));
    f.workflows.fail.store(true, Ordering::SeqCst);
    let page = f.get("/workflows").await.text().await.unwrap();
    let token = csrf(&page);
    let fields = [
        ("csrf", token.as_str()),
        ("expected_revision", "1"),
        ("idempotency_key", "ambiguous-operation"),
    ];
    assert_eq!(
        f.post(
            "/workflows/research/a-object/cancel",
            &fields,
            &f.portal.url
        )
        .await
        .status(),
        404
    );
    assert_eq!(
        f.post(
            "/workflows/research/a-object/cancel",
            &fields,
            &f.portal.url
        )
        .await
        .status(),
        409
    );
    assert_eq!(f.workflows.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn callback_state_and_duplicate_inputs_reject_before_token_exchange_and_tampering() {
    let f = Fixture::new(false).await;
    let started = f
        .client
        .get(format!("{}/auth/login", f.portal.url))
        .send()
        .await
        .unwrap();
    let authorization = f
        .client
        .get(started.headers()["location"].to_str().unwrap())
        .send()
        .await
        .unwrap();
    let location = authorization.headers()["location"].to_str().unwrap();
    let mut wrong = url::Url::parse(location).unwrap();
    let code = wrong
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    wrong.set_query(Some(&format!("code={code}&state=attacker")));
    assert_eq!(f.client.get(wrong).send().await.unwrap().status(), 401);
    assert_eq!(f.idp.token_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        f.client
            .get(format!("{location}&code=second"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(f.idp.token_calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.client.get(location).send().await.unwrap().status(), 303);
    assert_eq!(f.idp.token_calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn host_origin_form_and_rate_boundaries_fail_closed() {
    let f = Fixture::new(false).await;
    let wrong = f
        .client
        .get(format!("{}/auth/login", f.portal.url))
        .header("host", "evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 400);
    assert_eq!(f.idp.discovery_calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.login().await.status(), 303);
    let token = csrf(&f.get("/workflows").await.text().await.unwrap());
    let bad = f
        .client
        .post(format!("{}/auth/logout", f.portal.url))
        .header("origin", &f.portal.url)
        .header("content-type", "application/json")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 403);
    let duplicate = f
        .client
        .post(format!("{}/auth/logout", f.portal.url))
        .header("origin", &f.portal.url)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("csrf={token}&csrf={token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), 403);
    let referer = f
        .client
        .post(format!("{}/auth/logout", f.portal.url))
        .header("referer", format!("{}/workflows", f.portal.url))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("csrf={token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(referer.status(), 303);
    for _ in 0..9 {
        assert_eq!(f.get("/auth/login").await.status(), 303)
    }
    assert_eq!(f.get("/auth/login").await.status(), 429);
}
fn session_projection(value: Result<BrowserSession, BrowserError>) -> Value {
    match value {
        Ok(s) => {
            json!({"active":s.active_tenant,"generation":s.generation,"grants":s.context.map(|c|c.grants.into_iter().collect::<Vec<_>>()),"expired":false,"revoked":false,"unavailable":false})
        }
        Err(e) => {
            json!({"active":null,"generation":null,"grants":null,"expired":e==BrowserError::Expired,"revoked":e==BrowserError::Revoked,"unavailable":matches!(e,BrowserError::Unavailable|BrowserError::Closed)})
        }
    }
}
#[tokio::test]
async fn exact_source_session_transition_projection_replay() {
    let mut f = Fixture::new(true).await;
    let corpus: Value =
        serde_json::from_str(include_str!("../../xtask/fixtures/t09/source.json")).unwrap();
    let cases = corpus["sessions"].as_array().unwrap();
    let expected = |id: &str| cases.iter().find(|c| c["id"] == id).unwrap()["source"].clone();
    let actions = ["workflow.read".into()].into_iter().collect();
    let h = handle(&f.login().await);
    let pending = f.authority.resolve(&h, &actions).await.unwrap();
    assert_eq!(session_projection(Ok(pending.clone())), expected("pending"));
    let new = f.authority.rotate(&h, &pending, "a").await.unwrap();
    assert_eq!(
        session_projection(f.authority.resolve(&new.handle, &actions).await),
        expected("rotated")
    );
    assert_eq!(
        session_projection(f.authority.resolve(&h, &actions).await),
        expected("old_missing")
    );
    // Retain an expired record so logical expiry is tested independently of key eviction.
    let (key, raw) = f.session_records().remove(0);
    let mut expired: Value = serde_json::from_str(&raw).unwrap();
    let now = BrowserStore::new(Arc::new(f.valkey.store()))
        .now()
        .await
        .unwrap();
    expired["absolute_expires_ms"] = json!(now);
    expired["idle_expires_ms"] = json!(now);
    redis::cmd("SET")
        .arg(&key)
        .arg(expired.to_string())
        .query::<()>(&mut f.valkey.connection())
        .unwrap();
    assert_eq!(
        session_projection(f.authority.resolve(&new.handle, &actions).await),
        expected("absolute_expired")
    );
    let h = handle(&f.login().await);
    let pending = f.authority.resolve(&h, &actions).await.unwrap();
    let new = f.authority.rotate(&h, &pending, "a").await.unwrap();
    f.directory.0.lock().unwrap().revision = 2;
    assert_eq!(
        session_projection(f.authority.resolve(&new.handle, &actions).await),
        expected("revision_revoked")
    );
    assert_eq!(
        session_projection(f.authority.resolve(&new.handle, &actions).await),
        expected("revocation_missing")
    );
    f.valkey.stop();
    assert_eq!(
        session_projection(f.authority.resolve(&new.handle, &actions).await),
        expected("unavailable")
    );
    assert_eq!(cases.len(), 7);
}
#[tokio::test]
async fn stalled_tls_peer_does_not_serialize_other_browser_handshakes() {
    let f = Fixture::new(false).await;
    let url = url::Url::parse(&f.portal.url).unwrap();
    let _stalled = tokio::net::TcpStream::connect(("127.0.0.1", url.port().unwrap()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let response = tokio::time::timeout(Duration::from_millis(500), f.get("/"))
        .await
        .expect("stalled peer must not block independent TLS clients");
    assert_eq!(response.status(), 200);
}
#[tokio::test]
async fn composition_only_accepts_staged_search_kind_and_semantic_replay_ignores_csrf_rotation() {
    let f = Fixture::new(false).await;
    f.directory.0.lock().unwrap().memberships[0]
        .grants
        .insert("workflow.compose".into());
    assert_eq!(f.login().await.status(), 303);
    let token = csrf(&f.get("/workflows").await.text().await.unwrap());
    let fields = [
        ("csrf", token.as_str()),
        ("expected_revision", "1"),
        ("idempotency_key", "semantic-replay-identity"),
    ];
    let invalid = f
        .post(
            "/workflows/research/a-object/compose",
            &fields,
            &f.portal.url,
        )
        .await;
    assert_eq!(
        invalid.status(),
        403,
        "composition must never reach workflow for an unsupported kind"
    );
    assert_eq!(f.workflows.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn semantic_mutation_digest_replays_explicit_defaults_and_rotated_csrf() {
    let f = Fixture::new(false).await;
    assert_eq!(f.login().await.status(), 303);
    let token = csrf(&f.get("/workflows").await.text().await.unwrap());
    let fields = [
        ("csrf", token.as_str()),
        ("expected_revision", "1"),
        ("idempotency_key", "semantic-replay-identity"),
    ];
    assert_eq!(
        f.post(
            "/workflows/research/a-object/cancel",
            &fields,
            &f.portal.url
        )
        .await
        .status(),
        303
    );
    let explicit = [
        ("csrf", token.as_str()),
        ("expected_revision", "1"),
        ("idempotency_key", "semantic-replay-identity"),
        ("strategy", "triangulate"),
    ];
    assert_eq!(
        f.post(
            "/workflows/research/a-object/cancel",
            &explicit,
            &f.portal.url
        )
        .await
        .status(),
        303
    );
    let chooser = f.get("/auth/tenant").await.text().await.unwrap();
    let tenant_csrf = csrf(&chooser);
    assert_eq!(
        f.post(
            "/auth/tenant",
            &[("csrf", &tenant_csrf), ("tenant", "a")],
            &f.portal.url
        )
        .await
        .status(),
        303
    );
    let newtoken = csrf(&f.get("/workflows").await.text().await.unwrap());
    assert_ne!(newtoken, token);
    let rotated = [
        ("csrf", newtoken.as_str()),
        ("expected_revision", "1"),
        ("idempotency_key", "semantic-replay-identity"),
    ];
    assert_eq!(
        f.post(
            "/workflows/research/a-object/cancel",
            &rotated,
            &f.portal.url
        )
        .await
        .status(),
        303
    );
    let changed = [
        ("csrf", newtoken.as_str()),
        ("expected_revision", "1"),
        ("idempotency_key", "semantic-replay-identity"),
        ("question", "changed"),
    ];
    assert_eq!(
        f.post(
            "/workflows/research/a-object/cancel",
            &changed,
            &f.portal.url
        )
        .await
        .status(),
        409
    );
    assert_eq!(f.workflows.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn dropping_listener_closes_owned_pending_tls_peers_promptly() {
    use tokio::io::AsyncReadExt as _;
    let (config, _) = tls();
    let bound = listener().await;
    let addr = bound.local_addr().unwrap();
    let mut listener = BrowserTlsListener::new(bound, config);
    let task = tokio::spawn(async move {
        use axum::serve::Listener as _;
        let _ = listener.accept().await;
    });
    let mut peer = tokio::net::TcpStream::connect(addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let mut byte = [0];
    let read = tokio::time::timeout(Duration::from_millis(500), peer.read(&mut byte))
        .await
        .expect("pending peer must close when listener is dropped");
    assert!(matches!(read, Ok(0) | Err(_)));
}
#[tokio::test]
async fn unicode_idempotency_counts_source_characters_not_utf8_bytes() {
    let f = Fixture::new(false).await;
    assert_eq!(f.login().await.status(), 303);
    let token = csrf(&f.get("/workflows").await.text().await.unwrap());
    let too_short = "é".repeat(6);
    let valid = "é".repeat(12);
    assert_eq!(
        f.post(
            "/workflows/research/a-object/cancel",
            &[
                ("csrf", &token),
                ("expected_revision", "1"),
                ("idempotency_key", &too_short)
            ],
            &f.portal.url
        )
        .await
        .status(),
        400
    );
    assert_eq!(
        f.post(
            "/workflows/research/a-object/cancel",
            &[
                ("csrf", &token),
                ("expected_revision", "1"),
                ("idempotency_key", &valid)
            ],
            &f.portal.url
        )
        .await
        .status(),
        303
    );
}
#[tokio::test]
async fn csrf_header_is_supported_but_conflicting_values_deny_and_audit_is_pseudonymous() {
    let f = Fixture::new(false).await;
    let logged = f.login().await;
    let handle = handle(&logged);
    let token = csrf(&f.get("/workflows").await.text().await.unwrap());
    let conflict = f
        .client
        .post(format!("{}/auth/logout", f.portal.url))
        .header("origin", &f.portal.url)
        .header("x-csrf-token", &token)
        .header("content-type", "application/x-www-form-urlencoded")
        .body("csrf=wrong")
        .send()
        .await
        .unwrap();
    assert_eq!(conflict.status(), 403);
    let allowed = f
        .client
        .post(format!("{}/auth/logout", f.portal.url))
        .header("origin", &f.portal.url)
        .header("x-csrf-token", &token)
        .header("content-type", "application/x-www-form-urlencoded")
        .body("")
        .send()
        .await
        .unwrap();
    assert_eq!(allowed.status(), 303);
    let mut connection = f.valkey.connection();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("portal:native:v1:*:audit:*")
        .query(&mut connection)
        .unwrap();
    assert_eq!(keys.len(), 1);
    let value: redis::Value = redis::cmd("XRANGE")
        .arg(&keys[0])
        .arg("-")
        .arg("+")
        .query(&mut connection)
        .unwrap();
    let encoded = format!("{value:?}");
    for forbidden in [
        "synthetic-client-secret",
        "subject",
        handle.expose(),
        token.as_str(),
    ] {
        assert!(
            !encoded.contains(forbidden),
            "audit leaked credential or subject"
        );
    }
    assert!(encoded.contains("csrf_denied") && encoded.contains("logout"));
    let ttl: i64 = redis::cmd("TTL")
        .arg(&keys[0])
        .query(&mut connection)
        .unwrap();
    assert!((7_775_990..=7_776_000).contains(&ttl));
}
#[tokio::test]
async fn audit_stream_is_bounded_and_wrong_type_audit_failure_does_not_authorize_read() {
    let f = Fixture::new(false).await;
    assert_eq!(f.login().await.status(), 303);
    let mut c = f.valkey.connection();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("portal:native:v1:*:audit:*")
        .query(&mut c)
        .unwrap();
    let key = &keys[0];
    let mut pipeline = redis::pipe();
    for _ in 0..10002 {
        pipeline
            .cmd("XADD")
            .arg(key)
            .arg("*")
            .arg("synthetic")
            .arg("bounded")
            .ignore();
    }
    pipeline.query::<()>(&mut c).unwrap();
    assert_eq!(f.get("/workflows").await.status(), 200);
    let length: u64 = redis::cmd("XLEN").arg(key).query(&mut c).unwrap();
    assert_eq!(length, 10000);
    redis::cmd("DEL").arg(key).query::<()>(&mut c).unwrap();
    redis::cmd("SET")
        .arg(key)
        .arg("wrong-type")
        .query::<()>(&mut c)
        .unwrap();
    assert_eq!(f.get("/workflows").await.status(), 503);
    assert_eq!(
        redis::cmd("GET").arg(key).query::<String>(&mut c).unwrap(),
        "wrong-type"
    );
    assert_eq!(f.get("/").await.status(), 200);
}
#[tokio::test]
async fn authority_namespaces_isolate_sessions_quotas_and_closed_store_is_terminal() {
    let f = Fixture::new(false).await;
    let h = handle(&f.login().await);
    let mut settings = f.settings.clone();
    settings.client_id = "other-client".into();
    let valkey = Arc::new(f.valkey.store());
    let other = BrowserAuthority::new(
        BrowserStore::new(valkey.clone()),
        Arc::new(f.directory.clone()),
        BrowserAuthorityConfig {
            settings,
            keys: BrowserKeys::new(secret(&"s".repeat(32)), None).unwrap(),
            audit_key: secret(&"a".repeat(32)),
            roots: vec![f.root.clone()],
            idle_seconds: 60,
            absolute_seconds: 120,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        other.resolve(&h, &actions()).await.unwrap_err(),
        BrowserError::Missing
    );
    for _ in 0..10 {
        assert!(
            f.authority
                .rate(BrowserRate::Login, &["network"])
                .await
                .unwrap()
        )
    }
    assert!(
        !f.authority
            .rate(BrowserRate::Login, &["network"])
            .await
            .unwrap()
    );
    assert!(other.rate(BrowserRate::Login, &["network"]).await.unwrap());
    valkey.close().await;
    assert_eq!(
        other.rate(BrowserRate::Login, &["network"]).await,
        Err(BrowserError::Closed)
    );
    assert_eq!(
        other.resolve(&h, &actions()).await.unwrap_err(),
        BrowserError::Closed
    );
}
#[tokio::test]
async fn audit_failure_during_reauthentication_never_issues_unlogged_session_cookie() {
    let f = Fixture::new(false).await;
    assert_eq!(f.login().await.status(), 303);
    let mut c = f.valkey.connection();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("portal:native:v1:*:audit:*")
        .query(&mut c)
        .unwrap();
    redis::cmd("DEL").arg(&keys[0]).query::<()>(&mut c).unwrap();
    redis::cmd("SET")
        .arg(&keys[0])
        .arg("wrong-type")
        .query::<()>(&mut c)
        .unwrap();
    let response = f.login().await;
    assert_eq!(response.status(), 503);
    assert_eq!(response.headers().get_all("set-cookie").iter().count(), 0);
    assert!(f.session_records().is_empty());
}
#[tokio::test]
async fn committed_write_with_lost_reply_is_unknown_and_never_retried() {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let server = PrivateValkey::start();
    let now = BrowserStore::new(Arc::new(server.store()))
        .now()
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let socket = server.dir.join("s.sock");
    let evals = Arc::new(AtomicUsize::new(0));
    let counter = evals.clone();
    let task = tokio::spawn(async move {
        let (client, _) = listener.accept().await.unwrap();
        let upstream = tokio::net::UnixStream::connect(socket).await.unwrap();
        let (mut cr, mut cw) = client.into_split();
        let (mut ur, mut uw) = upstream.into_split();
        let dispatched = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed = dispatched.clone();
        let forward = async move {
            let mut buffer = [0; 8192];
            let mut tail = Vec::new();
            loop {
                let n = cr.read(&mut buffer).await.unwrap();
                if n == 0 {
                    break;
                }
                tail.extend_from_slice(&buffer[..n]);
                if tail.windows(8).any(|v| v == b"\r\nEVAL\r\n") {
                    counter.fetch_add(1, Ordering::SeqCst);
                    observed.store(true, Ordering::SeqCst);
                    tail.clear();
                } else if tail.len() > 16 {
                    tail = tail[tail.len() - 16..].to_vec()
                }
                uw.write_all(&buffer[..n]).await.unwrap();
            }
        };
        let reply = async move {
            let mut buffer = [0; 8192];
            loop {
                let n = ur.read(&mut buffer).await.unwrap();
                if n == 0 {
                    break;
                }
                if dispatched.load(Ordering::SeqCst) {
                    break;
                }
                cw.write_all(&buffer[..n]).await.unwrap();
            }
        };
        tokio::select! {_=forward=>(),_=reply=>()}
    });
    let store = BrowserStore::new(Arc::new(
        Valkey::new(&format!("redis://{address}"), Duration::from_secs(2)).unwrap(),
    ));
    let record = json!({"schema":1,"expires_ms":now+60000});
    let result = store
        .change("create", &["t09:unknown-write"], "", &record, now + 60000)
        .await;
    assert_eq!(result, Err(BrowserError::CompletionUnknown));
    task.await.unwrap();
    assert_eq!(evals.load(Ordering::SeqCst), 1);
    let persisted: String = redis::cmd("GET")
        .arg("t09:unknown-write")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&persisted).unwrap(), record);
}
struct MisdirectedDirectory(BrowserPrincipal);
impl BrowserDirectory for MisdirectedDirectory {
    fn subject<'a>(
        &'a self,
        _issuer: &'a str,
        _subject: &'a str,
    ) -> PortFuture<'a, Result<Option<BrowserPrincipal>, BrowserError>> {
        Box::pin(async move { Ok(Some(self.0.clone())) })
    }
    fn principal<'a>(
        &'a self,
        _id: &'a str,
    ) -> PortFuture<'a, Result<Option<BrowserPrincipal>, BrowserError>> {
        Box::pin(async move { Ok(Some(self.0.clone())) })
    }
}
#[tokio::test]
async fn directory_port_cannot_substitute_another_principal_or_verified_subject() {
    let f = Fixture::new(false).await;
    let h = handle(&f.login().await);
    let session = f.authority.resolve(&h, &actions()).await.unwrap();
    let mut wrong = f.directory.0.lock().unwrap().clone();
    wrong.principal_id = "another-principal".into();
    wrong.subject = "another-subject".into();
    let authority = BrowserAuthority::new(
        BrowserStore::new(Arc::new(f.valkey.store())),
        Arc::new(MisdirectedDirectory(wrong)),
        BrowserAuthorityConfig {
            settings: f.settings.clone(),
            keys: BrowserKeys::new(secret(&"s".repeat(32)), None).unwrap(),
            audit_key: secret(&"a".repeat(32)),
            roots: vec![f.root.clone()],
            idle_seconds: 60,
            absolute_seconds: 120,
        },
    )
    .await
    .unwrap();
    let before = f.session_records();
    assert_eq!(
        authority.tenants(&session).await.unwrap_err(),
        BrowserError::Revoked
    );
    assert_eq!(f.session_records(), before);
    assert_eq!(
        authority.resolve(&h, &actions()).await.unwrap_err(),
        BrowserError::Revoked
    );
    assert!(f.session_records().is_empty());
    let start = authority.begin("/workflows").await.unwrap();
    let response = f.client.get(&start.location).send().await.unwrap();
    let callback = url::Url::parse(response.headers()["location"].to_str().unwrap()).unwrap();
    assert_eq!(
        authority
            .complete(callback.query().unwrap(), &start.transaction, None)
            .await
            .unwrap_err(),
        BrowserError::Denied
    );
    assert!(f.session_records().is_empty());
}

/// Holds the rotating authority after its raw read, before its exact-record CAS.
struct PausedDirectory {
    inner: Directory,
    reached: tokio::sync::Notify,
    resume: tokio::sync::Notify,
}
impl BrowserDirectory for PausedDirectory {
    fn subject<'a>(
        &'a self,
        issuer: &'a str,
        subject: &'a str,
    ) -> PortFuture<'a, Result<Option<BrowserPrincipal>, BrowserError>> {
        self.inner.subject(issuer, subject)
    }
    fn principal<'a>(
        &'a self,
        id: &'a str,
    ) -> PortFuture<'a, Result<Option<BrowserPrincipal>, BrowserError>> {
        Box::pin(async move {
            self.reached.notify_one();
            self.resume.notified().await;
            self.inner.principal(id).await
        })
    }
}
#[tokio::test]
async fn refresh_between_rotation_read_and_cas_conflicts_without_issuing_handle() {
    let f = Fixture::new(true).await;
    let h = handle(&f.login().await);
    let pending = f.authority.resolve(&h, &actions()).await.unwrap();
    let selected = f.authority.rotate(&h, &pending, "a").await.unwrap();
    let h = selected.handle;
    let directory = Arc::new(PausedDirectory {
        inner: f.directory.clone(),
        reached: tokio::sync::Notify::new(),
        resume: tokio::sync::Notify::new(),
    });
    let rotating = BrowserAuthority::new(
        BrowserStore::new(Arc::new(f.valkey.store())),
        directory.clone(),
        BrowserAuthorityConfig {
            settings: f.settings.clone(),
            keys: BrowserKeys::new(secret(&"s".repeat(32)), None).unwrap(),
            audit_key: secret(&"a".repeat(32)),
            roots: vec![f.root.clone()],
            idle_seconds: 60,
            absolute_seconds: 120,
        },
    )
    .await
    .unwrap();
    let expected = selected.session;
    let attempt = rotating.rotate(&h, &expected, "b");
    let interleave = async {
        directory.reached.notified().await;
        let current = f.authority.resolve(&h, &actions()).await.unwrap();
        assert_eq!(current.active_tenant.as_deref(), Some("a"));
        let preserved = f.session_records();
        assert_eq!(preserved.len(), 1);
        directory.resume.notify_one();
        preserved
    };
    let (result, preserved) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(attempt, interleave)
    })
    .await
    .unwrap();
    assert_eq!(result.unwrap_err(), BrowserError::Conflict);
    assert_eq!(f.session_records(), preserved);
    let fresh = f.authority.resolve(&h, &actions()).await.unwrap();
    assert_eq!(fresh.active_tenant.as_deref(), Some("a"));
    assert_eq!(fresh.generation, expected.generation);
    let successor = f.authority.rotate(&h, &fresh, "b").await.unwrap();
    assert_eq!(
        f.authority.resolve(&h, &actions()).await.unwrap_err(),
        BrowserError::Missing
    );
    assert_eq!(
        f.authority
            .resolve(&successor.handle, &actions())
            .await
            .unwrap()
            .active_tenant
            .as_deref(),
        Some("b")
    );
}

#[tokio::test]
async fn decoded_nul_semantic_components_deny_before_claim_or_workflow() {
    let f = Fixture::new(false).await;
    f.directory.0.lock().unwrap().memberships[0]
        .grants
        .insert("workflow.compose".into());
    assert_eq!(f.login().await.status(), 303);
    let token = csrf(&f.get("/workflows").await.text().await.unwrap());
    let cases = [
        (
            "collision-first",
            "/workflows/staged_search/a-object/compose",
            "",
            "alpha\0beta",
            "triangulate",
        ),
        (
            "collision-second",
            "/workflows/staged_search/a-object/compose",
            "\0alpha",
            "beta",
            "triangulate",
        ),
        (
            "kind",
            "/workflows/staged_search%00/a-object/compose",
            "",
            "question",
            "triangulate",
        ),
        (
            "id",
            "/workflows/staged_search/a-object%00/compose",
            "",
            "question",
            "triangulate",
        ),
        (
            "action",
            "/workflows/staged_search/a-object/compose%00",
            "",
            "question",
            "triangulate",
        ),
        (
            "cursor",
            "/workflows/staged_search/a-object/compose",
            "cursor\0",
            "question",
            "triangulate",
        ),
        (
            "question",
            "/workflows/staged_search/a-object/compose",
            "",
            "question\0",
            "triangulate",
        ),
        (
            "strategy",
            "/workflows/staged_search/a-object/compose",
            "",
            "question",
            "triangulate\0",
        ),
    ];
    for (label, path, cursor, question, strategy) in cases {
        let response = f
            .post(
                path,
                &[
                    ("csrf", token.as_str()),
                    ("expected_revision", "1"),
                    ("idempotency_key", "nul-replay-regression"),
                    ("cursor", cursor),
                    ("question", question),
                    ("strategy", strategy),
                ],
                &f.portal.url,
            )
            .await;
        assert_eq!(response.status(), 403, "{label}");
        assert_eq!(f.workflows.calls.load(Ordering::SeqCst), 0, "{label}");
        assert!(f.workflows.received.lock().unwrap().is_empty(), "{label}");
        let claims: Vec<String> = redis::cmd("KEYS")
            .arg("portal:native:v1:*:mutation:*")
            .query(&mut f.valkey.connection())
            .unwrap();
        assert!(
            claims
                .iter()
                .all(|key| key.split(':').nth(4) != Some("mutation")),
            "{label}: invalid semantic input must never claim"
        );
    }
    // Refusal must not consume the idempotency key or block an ordinary command.
    assert_eq!(
        f.post(
            "/workflows/staged_search/a-object/compose",
            &[
                ("csrf", token.as_str()),
                ("expected_revision", "1"),
                ("idempotency_key", "nul-replay-regression"),
                ("question", "ordinary question"),
            ],
            &f.portal.url
        )
        .await
        .status(),
        303
    );
    assert_eq!(f.workflows.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn request_ids_correlate_real_audit_events_without_cross_request_leaks() {
    let f = Fixture::new(false).await;
    let url = format!("{}/workflows", f.portal.url);
    let (first, second, generated, empty) = tokio::join!(
        f.client
            .get(&url)
            .header("x-request-id", "first-request")
            .send(),
        f.client
            .get(&url)
            .header("x-request-id", "second-request")
            .send(),
        f.client.get(&url).send(),
        f.client.get(&url).header("x-request-id", "").send(),
    );
    let mut expected = Vec::new();
    for (response, supplied) in [
        (first.unwrap(), Some("first-request")),
        (second.unwrap(), Some("second-request")),
        (generated.unwrap(), None),
        (empty.unwrap(), None),
    ] {
        assert_eq!(response.status(), 303);
        let id = response.headers()["x-request-id"]
            .to_str()
            .unwrap()
            .to_owned();
        if let Some(supplied) = supplied {
            assert_eq!(id, supplied);
        } else {
            assert_eq!(id.len(), 36);
            assert_eq!(&id[14..15], "4");
            assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"));
        }
        expected.push(id);
    }
    assert_eq!(expected.iter().collect::<BTreeSet<_>>().len(), 4);
    assert_eq!(f.login().await.status(), 303);
    let read = f
        .client
        .get(&url)
        .header("x-request-id", "authorized-read")
        .send()
        .await
        .unwrap();
    assert_eq!(read.status(), 200);
    assert_eq!(read.headers()["x-request-id"], "authorized-read");
    let denied = f
        .client
        .post(format!("{}/auth/logout", f.portal.url))
        .header("x-request-id", "csrf-denial")
        .header("origin", &f.portal.url)
        .header("content-type", "application/x-www-form-urlencoded")
        .body("csrf=wrong")
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);
    assert_eq!(denied.headers()["x-request-id"], "csrf-denial");
    let long = "x".repeat(160);
    let bounded = f
        .client
        .get(&url)
        .header("x-request-id", &long)
        .send()
        .await
        .unwrap();
    assert_eq!(bounded.status(), 200);
    assert_eq!(bounded.headers()["x-request-id"], long.as_str());

    let mut connection = f.valkey.connection();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("portal:native:v1:*:audit:*")
        .query(&mut connection)
        .unwrap();
    let mut events = Vec::<Value>::new();
    for key in keys {
        let rows: redis::streams::StreamRangeReply = redis::cmd("XRANGE")
            .arg(key)
            .arg("-")
            .arg("+")
            .query(&mut connection)
            .unwrap();
        for row in rows.ids {
            let raw: String =
                redis::from_redis_value(row.map.get("event").unwrap().clone()).unwrap();
            events.push(serde_json::from_str(&raw).unwrap());
        }
    }
    assert!(
        events
            .iter()
            .all(|e| !e["request_id"].as_str().unwrap().is_empty())
    );
    for id in expected {
        let matching: Vec<_> = events.iter().filter(|e| e["request_id"] == id).collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0]["outcome"], "denied");
        assert_eq!(matching[0]["reason"], "authentication_required");
    }
    for (id, outcome) in [("authorized-read", "allowed"), ("csrf-denial", "denied")] {
        let matching: Vec<_> = events.iter().filter(|e| e["request_id"] == id).collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0]["outcome"], outcome);
    }
    assert!(events.iter().any(|e| e["request_id"] == "x".repeat(128)));
}
