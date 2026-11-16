//! T04 private real-Valkey qualification; no live providers or shared state.
use ssx_domain::oauth::*;
use ssx_infra::valkey::{Valkey, oauth::OAuthStore};
use std::{
    fs,
    io::Read as _,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
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
        let dir = std::env::temp_dir().join(format!("ssx-t04-{id}"));
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
    fn restart(&mut self) {
        self.child = Self::spawn(&self.dir);
        self.wait_ready();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn client(id: &str) -> ClientRegistration {
    ClientRegistration{client_id:id.into(),metadata:serde_json::json!({"redirect_uris":["http://localhost/cb"],"token_endpoint_auth_method":"none","grant_types":["authorization_code","refresh_token"],"response_types":["code"]}).as_object().unwrap().clone(),client_secret:Some(Secret::new("client-secret-fixture".into()).unwrap())}
}

fn grant() -> CodeGrant {
    CodeGrant {
        client_id: "c".into(),
        scopes: vec!["read".into(), "write".into()],
        code_challenge: "pkce-challenge".into(),
        redirect_uri: "http://localhost/cb".into(),
        redirect_uri_provided_explicitly: true,
        resource: Some("https://example.com/mcp".into()),
        subject: Some("operator".into()),
    }
}
fn oauth(s: &Server, t: OAuthTtls) -> OAuthStore {
    OAuthStore::new(Arc::new(s.store()), "test-authority", t).unwrap()
}
async fn pair(s: &OAuthStore) -> TokenPair {
    let code = s.create_code(grant()).await.unwrap();
    s.redeem_code(&code.code, &code.grant).await.unwrap()
}
fn key(kind: &str, token: &Secret) -> String {
    format!(
        "mcp:oauth:v1:{{{}}}:{kind}:{}",
        fingerprint(b"test-authority"),
        token.fingerprint()
    )
}
fn get(s: &Server, key: &str) -> String {
    redis::cmd("GET")
        .arg(key)
        .query(&mut s.connection())
        .unwrap()
}
fn set(s: &Server, key: &str, value: &str) {
    redis::cmd("SET")
        .arg(key)
        .arg(value)
        .query::<()>(&mut s.connection())
        .unwrap();
}
#[tokio::test]
async fn empty_existing_records_are_collisions_and_preflight_is_nonmutating() {
    let server = Server::start();
    let a = oauth(&server, OAuthTtls::default());
    let client_key = format!(
        "mcp:oauth:v1:{{{}}}:client:{}",
        fingerprint(b"test-authority"),
        fingerprint(b"c")
    );
    set(&server, &client_key, "");
    assert_eq!(
        a.register_client(client("c")).await,
        Err(OAuthError::Denied(Denial::Conflict))
    );
    assert_eq!(get(&server, &client_key), "");
    assert_eq!(a.get_client("c").await, Err(OAuthError::Malformed));

    // A collision on a later key must prevent every write in the transition.
    let new_key = "t04-empty-collision:new";
    let result: i64 = redis::cmd("EVAL")
        .arg(include_str!("../src/valkey/oauth/transition.lua"))
        .arg(2)
        .arg(new_key)
        .arg(&client_key)
        .arg(r#"["", ""]"#)
        .arg(r#"["new-record", "replacement"]"#)
        .arg("[4102444800000,4102444800000]")
        .arg("[0,0]")
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(result, -1);
    let absent: Option<String> = redis::cmd("GET")
        .arg(new_key)
        .query(&mut server.connection())
        .unwrap();
    assert_eq!(absent, None);
    assert_eq!(get(&server, &client_key), "");

    redis::cmd("DEL")
        .arg(&client_key)
        .query::<()>(&mut server.connection())
        .unwrap();
    a.register_client(client("c")).await.unwrap();
    let original = get(&server, &client_key);
    assert_eq!(
        a.register_client(client("c")).await,
        Err(OAuthError::Denied(Denial::Conflict))
    );
    assert_eq!(get(&server, &client_key), original);
}
#[tokio::test]
async fn independent_instances_single_use_and_exact_binding() {
    let server = Server::start();
    let a = oauth(&server, OAuthTtls::default());
    let b = oauth(&server, OAuthTtls::default());
    a.register_client(client("c")).await.unwrap();
    assert_eq!(
        a.register_client(client("c")).await,
        Err(OAuthError::Denied(Denial::Conflict))
    );
    let code = a.create_code(grant()).await.unwrap();
    let mut bad = code.grant.clone();
    bad.resource = None;
    assert_eq!(
        b.redeem_code(&code.code, &bad).await,
        Err(OAuthError::Denied(Denial::Binding))
    );
    let (x, y) = tokio::join!(
        a.redeem_code(&code.code, &code.grant),
        b.redeem_code(&code.code, &code.grant)
    );
    assert_eq!(usize::from(x.is_ok()) + usize::from(y.is_ok()), 1);
    let tokens = x.or(y).unwrap();
    assert_eq!(
        a.load_token(TokenKind::Access, &tokens.access_token)
            .await
            .unwrap()
            .grant,
        TokenGrant::from(&code.grant)
    );
    assert!(a.redeem_code(&code.code, &code.grant).await.is_err());
    let before = get(&server, &key("refresh", &tokens.refresh_token));
    let too_wide = vec!["admin".into()];
    assert_eq!(
        a.rotate_refresh(&tokens.refresh_token, &tokens.grant, &too_wide)
            .await,
        Err(OAuthError::Denied(Denial::Scope))
    );
    assert_eq!(get(&server, &key("refresh", &tokens.refresh_token)), before);
    let (x, y) = tokio::join!(
        a.rotate_refresh(&tokens.refresh_token, &tokens.grant, &[]),
        b.rotate_refresh(&tokens.refresh_token, &tokens.grant, &[])
    );
    assert_eq!(usize::from(x.is_ok()) + usize::from(y.is_ok()), 1);
    let next = x.or(y).unwrap();
    assert!(
        a.load_token(TokenKind::Access, &tokens.access_token)
            .await
            .is_err()
    );
    b.revoke(TokenKind::Access, &next.access_token, "c")
        .await
        .unwrap();
    assert!(
        a.load_token(TokenKind::Refresh, &next.refresh_token)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn revoke_rotate_race_never_revives_revoked_or_old_pair() {
    let server = Server::start();
    let a = oauth(&server, OAuthTtls::default());
    let b = oauth(&server, OAuthTtls::default());
    a.register_client(client("c")).await.unwrap();
    for _ in 0..16 {
        let old = pair(&a).await;
        let (rotated, revoked) = tokio::join!(
            a.rotate_refresh(&old.refresh_token, &old.grant, &[]),
            b.revoke(TokenKind::Access, &old.access_token, "c")
        );
        assert!(rotated.is_ok() || revoked.is_ok());
        assert!(!(rotated.is_ok() && revoked.is_ok()));
        assert!(
            a.load_token(TokenKind::Access, &old.access_token)
                .await
                .is_err()
        );
        assert!(
            a.load_token(TokenKind::Refresh, &old.refresh_token)
                .await
                .is_err()
        );
        if let Ok(new) = rotated {
            b.revoke(TokenKind::Refresh, &new.refresh_token, "c")
                .await
                .unwrap();
        }
    }
}
#[tokio::test]
async fn natural_asymmetric_expiry_preserves_refresh_and_reverse_revocation() {
    let server = Server::start();
    let short = OAuthTtls {
        access: 1,
        refresh: 20,
        ..Default::default()
    };
    let a = oauth(&server, short);
    a.register_client(client("c")).await.unwrap();
    let p = pair(&a).await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(
        a.load_token(TokenKind::Access, &p.access_token)
            .await
            .is_err()
    );
    let rotated = a
        .rotate_refresh(&p.refresh_token, &p.grant, &[])
        .await
        .unwrap();
    a.revoke(TokenKind::Refresh, &rotated.refresh_token, "c")
        .await
        .unwrap();
    let b = oauth(
        &server,
        OAuthTtls {
            access: 20,
            refresh: 1,
            ..Default::default()
        },
    );
    let p = pair(&b).await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    b.revoke(TokenKind::Access, &p.access_token, "c")
        .await
        .unwrap();
}
#[tokio::test]
async fn malformed_asymmetric_links_never_delete_unrelated_pair() {
    let server = Server::start();
    let a = oauth(&server, OAuthTtls::default());
    a.register_client(client("c")).await.unwrap();
    let first = pair(&a).await;
    let second = pair(&a).await;
    let fk = key("refresh", &first.refresh_token);
    let before = get(&server, &fk);
    let second_access = get(&server, &key("access", &second.access_token));
    let mut record: serde_json::Value = serde_json::from_str(&before).unwrap();
    record["data"]["counterpart"] = second.access_token.fingerprint().into();
    set(&server, &fk, &record.to_string());
    assert_eq!(
        a.revoke(TokenKind::Refresh, &first.refresh_token, "c")
            .await,
        Err(OAuthError::Malformed)
    );
    assert_eq!(
        a.rotate_refresh(&first.refresh_token, &first.grant, &[])
            .await,
        Err(OAuthError::Malformed)
    );
    assert_eq!(
        get(&server, &key("access", &second.access_token)),
        second_access
    );
    assert!(
        a.load_token(TokenKind::Access, &second.access_token)
            .await
            .is_ok()
    );
}
#[tokio::test]
async fn expiry_is_valkey_authoritative_and_outage_never_falls_back() {
    let mut server = Server::start();
    let a = oauth(&server, OAuthTtls::default());
    a.register_client(client("c")).await.unwrap();
    let code = a.create_code(grant()).await.unwrap();
    let k = key("code", &code.code);
    let mut record: serde_json::Value = serde_json::from_str(&get(&server, &k)).unwrap();
    record["expires_at"] = 1.into();
    set(&server, &k, &record.to_string());
    assert_eq!(
        a.redeem_code(&code.code, &code.grant).await,
        Err(OAuthError::Denied(Denial::Expired))
    );
    server.stop();
    assert_eq!(a.create_code(grant()).await, Err(OAuthError::Unavailable));
    server.restart();
    assert_eq!(
        a.get_client("c").await,
        Err(OAuthError::Denied(Denial::Missing))
    );
    let vk = Arc::new(server.store());
    let closed = OAuthStore::new(vk.clone(), "test-authority", OAuthTtls::default()).unwrap();
    vk.close().await;
    assert_eq!(
        closed.register_client(client("c")).await,
        Err(OAuthError::Closed)
    );
}
#[tokio::test]
async fn credentials_debug_and_durable_grants_do_not_expose_tokens() {
    let server = Server::start();
    let a = oauth(&server, OAuthTtls::default());
    let c = client("c");
    assert!(!format!("{c:?}").contains("client-secret-fixture"));
    a.register_client(c.clone()).await.unwrap();
    assert_eq!(a.get_client("c").await.unwrap(), c);
    let p = pair(&a).await;
    let debug = format!("{p:?}");
    assert!(!debug.contains(p.access_token.expose()));
    assert!(!debug.contains(p.refresh_token.expose()));
    let durable = get(&server, &key("access", &p.access_token));
    assert!(!durable.contains(p.access_token.expose()));
    assert!(!durable.contains(p.refresh_token.expose()));
}
#[tokio::test]
async fn source_registration_model_dump_roundtrips_complete_nullable_metadata() {
    let server = Server::start();
    let a = oauth(&server, OAuthTtls::default());
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../xtask/fixtures/t04/source.json")).unwrap();
    for case in corpus["clients"].as_array().unwrap() {
        let mut metadata = case["expected"].as_object().unwrap().clone();
        let id = metadata
            .remove("client_id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let raw_secret = metadata.remove("client_secret").unwrap();
        let secret = raw_secret.as_str().map(|v| Secret::new(v.into()).unwrap());
        let record = ClientRegistration {
            client_id: id.clone(),
            client_secret: secret,
            metadata,
        };
        a.register_client(record.clone()).await.unwrap();
        assert_eq!(a.get_client(&id).await.unwrap(), record);
    }
}
#[tokio::test]
async fn lost_write_response_is_unknown_and_never_retried() {
    let server = Server::start();
    let vk = Arc::new(Valkey::new(&server.url, Duration::from_millis(50)).unwrap());
    vk.ping().await.unwrap();
    let a = OAuthStore::new(vk, "test-authority", OAuthTtls::default()).unwrap();
    redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(300)
        .arg("WRITE")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        a.register_client(client("unknown")).await,
        Err(OAuthError::CompletionUnknown)
    );
    tokio::time::sleep(Duration::from_millis(400)).await;
    let b = oauth(&server, OAuthTtls::default());
    assert!(b.get_client("unknown").await.is_ok());
    assert_eq!(
        b.register_client(client("unknown")).await,
        Err(OAuthError::Denied(Denial::Conflict))
    );
}
#[tokio::test]
async fn wrong_type_and_invalid_binding_are_nonmutating_and_authorities_isolate() {
    let server = Server::start();
    let a = oauth(&server, OAuthTtls::default());
    a.register_client(client("c")).await.unwrap();
    let code = a.create_code(grant()).await.unwrap();
    let before = get(&server, &key("code", &code.code));
    let mut wrong = code.grant.clone();
    wrong.redirect_uri_provided_explicitly = false;
    assert_eq!(
        a.redeem_code(&code.code, &wrong).await,
        Err(OAuthError::Denied(Denial::Binding))
    );
    wrong = code.grant.clone();
    wrong.subject = None;
    assert_eq!(
        a.redeem_code(&code.code, &wrong).await,
        Err(OAuthError::Denied(Denial::Binding))
    );
    assert_eq!(get(&server, &key("code", &code.code)), before);
    let b = OAuthStore::new(
        Arc::new(server.store()),
        "other-authority",
        OAuthTtls::default(),
    )
    .unwrap();
    assert_eq!(
        b.load_code(&code.code).await,
        Err(OAuthError::Denied(Denial::Missing))
    );
    let tokens = a.redeem_code(&code.code, &code.grant).await.unwrap();
    let access_key = key("access", &tokens.access_token);
    redis::cmd("DEL")
        .arg(&access_key)
        .query::<()>(&mut server.connection())
        .unwrap();
    redis::cmd("LPUSH")
        .arg(&access_key)
        .arg("wrong-type")
        .query::<()>(&mut server.connection())
        .unwrap();
    let refresh = get(&server, &key("refresh", &tokens.refresh_token));
    assert_eq!(
        a.revoke(TokenKind::Refresh, &tokens.refresh_token, "c")
            .await,
        Err(OAuthError::Malformed)
    );
    assert_eq!(
        get(&server, &key("refresh", &tokens.refresh_token)),
        refresh
    );
}
#[tokio::test]
async fn optional_empty_subject_resource_and_scope_narrowing_remain_exact() {
    let server = Server::start();
    let a = oauth(&server, OAuthTtls::default());
    a.register_client(client("c")).await.unwrap();
    let mut binding = grant();
    binding.subject = Some(String::new());
    binding.resource = None;
    let code = a.create_code(binding.clone()).await.unwrap();
    let old = a.redeem_code(&code.code, &binding).await.unwrap();
    assert_eq!(old.grant.subject, Some(String::new()));
    assert_eq!(old.grant.resource, None);
    let next = a
        .rotate_refresh(&old.refresh_token, &old.grant, &["read".into()])
        .await
        .unwrap();
    assert_eq!(next.grant.scopes, vec!["read"]);
    assert_eq!(next.grant.subject, old.grant.subject);
    assert_eq!(next.grant.resource, old.grant.resource);
}
#[tokio::test]
async fn malformed_schema_and_oversize_records_fail_without_writes() {
    let server = Server::start();
    let a = oauth(&server, OAuthTtls::default());
    a.register_client(client("c")).await.unwrap();
    let code = a.create_code(grant()).await.unwrap();
    let k = key("code", &code.code);
    let valid = get(&server, &k);
    for bad in [
        "not-json".to_owned(),
        "x".repeat(65537),
        valid.replace("\"schema\":1", "\"schema\":2"),
        valid.replace("\"kind\":\"code\"", "\"kind\":\"access\""),
    ] {
        set(&server, &k, &bad);
        assert_eq!(
            a.redeem_code(&code.code, &code.grant).await,
            Err(OAuthError::Malformed)
        );
        assert_eq!(get(&server, &k), bad);
    }
    set(&server, &k, &valid);
    assert!(a.redeem_code(&code.code, &code.grant).await.is_ok());
    let client_key = format!(
        "mcp:oauth:v1:{{{}}}:client:{}",
        fingerprint(b"test-authority"),
        fingerprint(b"c")
    );
    let original = get(&server, &client_key);
    for field in ["client_id", "client_secret", "too-many", "wrong-id"] {
        let mut bad: serde_json::Value = serde_json::from_str(&original).unwrap();
        if field == "wrong-id" {
            bad["data"]["client_id"] = "other".into();
        } else if field == "too-many" {
            for i in 0..65 {
                bad["data"]["metadata"][format!("field{i}")] = serde_json::Value::Null;
            }
        } else {
            bad["data"]["metadata"][field] = "synthetic-corruption".into();
        }
        let raw = bad.to_string();
        set(&server, &client_key, &raw);
        assert_eq!(a.get_client("c").await, Err(OAuthError::Malformed));
        assert_eq!(get(&server, &client_key), raw);
    }
    set(&server, &client_key, &original);
}
#[tokio::test]
async fn captured_source_flow_replays_binding_lifetimes_rotation_and_revocation() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../xtask/fixtures/t04/source.json")).unwrap();
    for case in corpus["settings"].as_array().unwrap() {
        for static_bearer in [false, true] {
            let result = OAuthMode::configured(
                case["enabled"].as_bool().unwrap(),
                case["issuer"].as_str().unwrap(),
                static_bearer,
                OAuthTtls::default(),
            );
            if case["expected"]["error"] == true {
                assert!(result.is_err());
            } else if case["expected"].is_null() {
                assert_eq!(
                    result.unwrap(),
                    if static_bearer {
                        OAuthMode::StaticBearer
                    } else {
                        OAuthMode::Disabled
                    }
                );
            } else {
                let settings = match result.unwrap() {
                    OAuthMode::OAuth(s) | OAuthMode::Both(s) => s,
                    _ => panic!("OAuth enabled"),
                };
                assert_eq!(
                    serde_json::json!({"issuer_url":settings.issuer_url,"resource_server_url":settings.resource_server_url}),
                    case["expected"]
                );
            }
        }
    }
    let flow = &corpus["flow"];
    let mut code = flow["code"].as_object().unwrap().clone();
    let source_code = code.remove("code").unwrap();
    let expiry = code.remove("expires_at").unwrap().as_f64().unwrap();
    let binding: CodeGrant = serde_json::from_value(code.into()).unwrap();
    assert_eq!(source_code, "source-code");
    assert_eq!(expiry - 1000., OAuthTtls::default().code as f64);
    let server = Server::start();
    let vk = Arc::new(server.store());
    let a = OAuthStore::new(vk.clone(), "test-authority", OAuthTtls::default()).unwrap();
    a.register_client(client("flow")).await.unwrap();
    let before = vk.quota_time().await.unwrap().milliseconds();
    let code = a.create_code(binding.clone()).await.unwrap();
    let after = vk.quota_time().await.unwrap().milliseconds();
    assert!(code.expires_at >= before + 600000 && code.expires_at <= after + 600000);
    assert_eq!(a.load_code(&code.code).await.unwrap().grant, binding);
    let pair = a.redeem_code(&code.code, &binding).await.unwrap();
    let issued_after = vk.quota_time().await.unwrap().milliseconds();
    assert_eq!(
        pair.expires_in,
        flow["token_response"]["expires_in"].as_u64().unwrap()
    );
    for (kind, name, token) in [
        (TokenKind::Access, "access", &pair.access_token),
        (TokenKind::Refresh, "refresh", &pair.refresh_token),
    ] {
        let mut payload = flow[name].as_object().unwrap().clone();
        payload.remove("token").unwrap();
        let source_expiry = payload.remove("expires_at").unwrap().as_u64().unwrap();
        if name == "access" {
            assert_eq!(payload.remove("claims").unwrap(), serde_json::json!({}));
        }
        assert_eq!(payload["resource"], serde_json::Value::Null);
        payload.insert(
            "resource".into(),
            serde_json::to_value(&binding.resource).unwrap(),
        );
        let expected: TokenGrant = serde_json::from_value(payload.into()).unwrap();
        let loaded = a.load_token(kind, token).await.unwrap();
        assert_eq!(loaded.grant, expected);
        let ttl = source_expiry - 1000;
        assert!(
            loaded.expires_at >= before + ttl * 1000
                && loaded.expires_at <= issued_after + ttl * 1000
        );
    }
    let next = a
        .rotate_refresh(&pair.refresh_token, &pair.grant, &["read".into()])
        .await
        .unwrap();
    assert!(
        a.load_token(TokenKind::Access, &pair.access_token)
            .await
            .is_err()
    );
    assert_eq!(flow["rotation_old_access_missing"], true);
    a.revoke(TokenKind::Access, &next.access_token, "flow")
        .await
        .unwrap();
    assert!(
        a.load_token(TokenKind::Refresh, &next.refresh_token)
            .await
            .is_err()
    );
    assert_eq!(flow["revoked_refresh_missing"], true);
}
#[tokio::test]
async fn narrowed_scope_bound_rejects_without_consuming_live_pair() {
    let server = Server::start();
    let a = oauth(&server, OAuthTtls::default());
    a.register_client(client("c")).await.unwrap();
    let old = pair(&a).await;
    let access_key = key("access", &old.access_token);
    let refresh_key = key("refresh", &old.refresh_token);
    let access_before = get(&server, &access_key);
    let refresh_before = get(&server, &refresh_key);
    let oversized = vec!["read".to_owned(); 129];
    let result = a
        .rotate_refresh(&old.refresh_token, &old.grant, &oversized)
        .await;

    assert_eq!(result, Err(OAuthError::InvalidInput));
    assert_eq!(get(&server, &access_key), access_before);
    assert_eq!(get(&server, &refresh_key), refresh_before);
    let valid = vec!["read".to_owned(); 128];
    let next = a
        .rotate_refresh(&old.refresh_token, &old.grant, &valid)
        .await
        .unwrap();
    assert_eq!(
        a.load_token(TokenKind::Refresh, &next.refresh_token)
            .await
            .unwrap()
            .grant
            .scopes,
        valid
    );
}
