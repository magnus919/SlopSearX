mod t05_support;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use ssx_application::oauth::OAuthState;
use ssx_domain::oauth::{OAuthSettings, OAuthTtls};
use ssx_infra::valkey::oauth::OAuthStore;
use ssx_mcp::{
    Registry,
    harness::{Host, http_router},
    oauth::{NetworkAuth, OAuthEndpoints},
};
use std::{collections::BTreeMap, sync::Arc};
use t05_support::Server;
use tokio_util::sync::CancellationToken;
const SOURCE: &str = include_str!("../../xtask/fixtures/t05/source.json");
struct Fixture {
    server: Server,
    store: Arc<OAuthStore>,
    host: Host,
    client: reqwest::Client,
}
impl Fixture {
    async fn new() -> Self {
        let server = Server::start();
        let store = Arc::new(
            OAuthStore::new(
                Arc::new(server.store()),
                "t05-qualification",
                OAuthTtls::default(),
            )
            .unwrap(),
        );
        let oauth = OAuthEndpoints::new(
            store.clone(),
            OAuthSettings::new("http://127.0.0.1", OAuthTtls::default()).unwrap(),
        )
        .unwrap();
        let cancel = CancellationToken::new();
        let host = Host::start(
            http_router(Registry::synthetic(), None, cancel.clone())
                .map(|r| NetworkAuth::OAuth(oauth).mount(r)),
            cancel,
        )
        .await
        .unwrap();
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        Self {
            server,
            store,
            host,
            client,
        }
    }
    fn base(&self) -> String {
        format!("http://{}", self.host.address)
    }
}
fn replace(v: &str, aliases: &BTreeMap<String, String>, reverse: bool) -> String {
    if v.get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Basic "))
        && let Ok(raw) = STANDARD.decode(&v[6..])
        && let Ok(raw) = String::from_utf8(raw)
    {
        let mut out = raw;
        for (k, val) in aliases {
            out = if reverse {
                out.replace(val, k)
            } else {
                out.replace(k, val)
            };
        }
        return format!("{}{}", &v[..6], STANDARD.encode(out));
    }
    let mut out = v.to_owned();
    for (k, val) in aliases {
        out = if reverse {
            out.replace(val, k)
        } else {
            out.replace(k, val)
        };
    }
    out
}
fn walk(value: &mut Value, aliases: &BTreeMap<String, String>, reverse: bool) {
    match value {
        Value::String(s) => *s = replace(s, aliases, reverse),
        Value::Array(a) => {
            for v in a {
                walk(v, aliases, reverse)
            }
        }
        Value::Object(m) => {
            for (k, v) in m {
                if k == "client_id_issued_at" {
                    *v = json!("<epoch-seconds>")
                } else {
                    walk(v, aliases, reverse)
                }
            }
        }
        _ => (),
    }
}
fn learn(expected: &Value, actual: &Value, aliases: &mut BTreeMap<String, String>) {
    for k in [
        "client_id",
        "client_secret",
        "access_token",
        "refresh_token",
    ] {
        if let (Some(e), Some(a)) = (expected[k].as_str(), actual[k].as_str()) {
            aliases.insert(a.into(), e.into());
        }
    }
}
#[tokio::test]
async fn pinned_source_wire_corpus_replays_with_only_named_security_corrections() {
    let source: Value = serde_json::from_str(SOURCE).unwrap();
    let f = Fixture::new().await;
    let mut aliases = BTreeMap::new();
    let mut differences = Vec::new();
    let mut corrections = 0;
    for c in source["cases"].as_array().unwrap() {
        let name = c["case"].as_str().unwrap();
        let mut input = c["request"].clone();
        walk(&mut input, &aliases, true);
        let method =
            reqwest::Method::from_bytes(input["method"].as_str().unwrap().as_bytes()).unwrap();
        let mut endpoint =
            url::Url::parse(&format!("{}{}", f.base(), input["path"].as_str().unwrap())).unwrap();
        if let Some(q) = input.get("params").and_then(Value::as_object) {
            endpoint
                .query_pairs_mut()
                .extend_pairs(q.iter().map(|(k, v)| (k.as_str(), v.as_str().unwrap())));
        }
        let mut request = f.client.request(method, endpoint);
        if let Some(h) = input.get("headers").and_then(Value::as_object) {
            for (k, v) in h {
                request = request.header(k, v.as_str().unwrap());
            }
        }
        if let Some(j) = input.get("json") {
            request = request.json(j);
        }
        if let Some(data) = input.get("data") {
            let encoded = url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(
                    data.as_object()
                        .unwrap()
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str().unwrap())),
                )
                .finish();
            request = request
                .header("content-type", "application/x-www-form-urlencoded")
                .body(encoded);
        }
        if let Some(fields) = input.get("files").and_then(Value::as_object) {
            let mut body = String::new();
            for (k, v) in fields {
                body += &format!(
                    "--T05BOUNDARY\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{}\r\n",
                    v[1].as_str().unwrap()
                );
            }
            body += "--T05BOUNDARY--\r\n";
            request = request
                .header("content-type", "multipart/form-data; boundary=T05BOUNDARY")
                .body(body);
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let raw = response.text().await.unwrap();
        let mut body: Value = serde_json::from_str(&raw).unwrap_or(json!(raw));
        let expected = &c["response"];
        if let Some(field) = name.strip_prefix("register:empty-placeholder:") {
            assert_eq!(status, 201);
            assert!(
                body.get(field).is_none(),
                "empty metadata omitted on wire: {field}"
            );
            let persisted = f
                .store
                .get_client(body["client_id"].as_str().unwrap())
                .await
                .unwrap();
            assert_eq!(
                persisted.metadata.get(field),
                Some(&Value::Null),
                "full-client default persisted: {field}"
            );
        }

        if name == "token:implicit-malformed-redirect" {
            assert_eq!(status, 400);
            let code =
                ssx_domain::oauth::Secret::new(input["data"]["code"].as_str().unwrap().into())
                    .unwrap();
            let retained = f.store.load_code(&code).await.unwrap();
            assert!(!retained.grant.redirect_uri_provided_explicitly);
        }
        if matches!(
            name,
            "authorize:empty-challenge" | "authorize:foreign-resource"
        ) {
            assert!(
                expected["headers"]["location"]
                    .as_str()
                    .unwrap()
                    .contains("code=")
            );
            assert_eq!(status, 302);
            let url = url::Url::parse(headers["location"].to_str().unwrap()).unwrap();
            let error = url
                .query_pairs()
                .find(|(k, _)| k == "error")
                .unwrap()
                .1
                .into_owned();
            assert_eq!(
                error,
                if name.ends_with("empty-challenge") {
                    "invalid_request"
                } else {
                    "invalid_target"
                }
            );
            corrections += 1;
            continue;
        }
        if matches!(
            name,
            "protected:missing:GET" | "protected:missing:HEAD" | "protected:missing:PUT"
        ) {
            assert_eq!(expected["status"], 405);
            assert_eq!(status, 401);
            assert!(headers.contains_key("www-authenticate"));
            corrections += 1;
            continue;
        }
        learn(&expected["body"], &body, &mut aliases);
        if let (Some(e), Some(a)) = (
            expected["headers"]["location"].as_str(),
            headers.get("location").and_then(|v| v.to_str().ok()),
        ) {
            let expected = url::Url::parse(e).unwrap();
            let actual = url::Url::parse(a).unwrap();
            let ec = expected.query_pairs().find(|(k, _)| k == "code");
            let ac = actual.query_pairs().find(|(k, _)| k == "code");
            if let (Some((_, e)), Some((_, a))) = (ec, ac) {
                aliases.insert(a.into_owned(), e.into_owned());
            }
        }
        walk(&mut body, &aliases, false);
        let keys = [
            "content-type",
            "cache-control",
            "pragma",
            "location",
            "www-authenticate",
            "allow",
            "access-control-allow-origin",
            "access-control-allow-methods",
            "access-control-allow-headers",
            "access-control-max-age",
            "vary",
        ];
        let actual_headers = keys
            .into_iter()
            .filter_map(|k| {
                headers
                    .get(k)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| (k.to_owned(), json!(replace(v, &aliases, false))))
            })
            .collect::<serde_json::Map<_, _>>();
        let actual = json!({"status":status,"headers":actual_headers,"body":body});
        if actual != *expected {
            differences.push(format!("{name}\nexpected {expected}\nactual   {actual}"));
        }
    }
    assert_eq!(corrections, 5);
    f.host.shutdown().await.unwrap();
    assert!(differences.is_empty(), "{}", differences.join("\n\n"));
}
#[tokio::test]
async fn authoritative_clock_units_and_failure_never_fall_back_to_process_time() {
    let mut f = Fixture::new().await;
    let now = f.store.now_ms().await.unwrap();
    assert!(now > 1_000_000_000_000);
    let later = f.store.now_ms().await.unwrap();
    assert!(later >= now && later - now < 1000);
    f.server.stop();
    assert!(f.store.now_ms().await.is_err());
    let r = f
        .client
        .post(format!("{}/register", f.base()))
        .json(&json!({"redirect_uris":["http://localhost/cb"],"token_endpoint_auth_method":"none"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 503);
    assert!(r.json::<Value>().await.unwrap().get("client_id").is_none());
    f.server.restart();
    assert!(f.store.now_ms().await.unwrap() >= later);
    f.host.shutdown().await.unwrap();
}

fn multipart(fields: &[(&str, &str, bool)], ending: bool) -> String {
    let mut body = String::new();
    for (name, value, file) in fields {
        body += &format!(
            "--T05BOUNDARY\r\nContent-Disposition: form-data; name=\"{name}\"{}\r\n\r\n{value}\r\n",
            if *file {
                "; filename=\"synthetic.txt\""
            } else {
                ""
            }
        );
    }
    if ending {
        body += "--T05BOUNDARY--\r\n";
    }
    body
}
#[tokio::test]
async fn bounded_forms_reject_ambiguity_oversize_and_truncated_multipart_before_authority() {
    let f = Fixture::new().await;
    let post = |typ: String, body: String| {
        f.client
            .post(format!("{}/token", f.base()))
            .header("content-type", typ)
            .body(body)
    };
    let typ = "multipart/form-data; boundary=T05BOUNDARY";
    for (label, body, status) in [
        ("raw body", "x".repeat(65_537), 413),
        (
            "duplicate credential",
            multipart(
                &[("client_id", "a", false), ("client_id", "b", false)],
                true,
            ),
            400,
        ),
        (
            "long value",
            multipart(&[("client_id", &"x".repeat(4097), false)], true),
            400,
        ),
        (
            "long name",
            multipart(&[(&"x".repeat(129), "v", false)], true),
            400,
        ),
        (
            "known file",
            multipart(&[("client_id", "synthetic", true)], true),
            400,
        ),
        (
            "truncated",
            multipart(&[("client_id", "synthetic", false)], false),
            400,
        ),
    ] {
        let response = post(typ.into(), body).send().await.unwrap();
        assert_eq!(response.status().as_u16(), status, "{label}");
    }
    let fields: Vec<_> = (0..65)
        .map(|i| (format!("field{i}"), "v".to_owned(), false))
        .collect();
    let refs: Vec<_> = fields
        .iter()
        .map(|(k, v, f)| (k.as_str(), v.as_str(), *f))
        .collect();
    assert_eq!(
        post(typ.into(), multipart(&refs, true))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        post(
            format!("multipart/form-data; boundary={}", "x".repeat(513)),
            String::new()
        )
        .send()
        .await
        .unwrap()
        .status(),
        400
    );
    let unknown = post(
        typ.into(),
        multipart(&[("unrelated", "ignored", true)], true),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(unknown.status(), 401);
    assert_eq!(
        unknown.json::<Value>().await.unwrap()["error_description"],
        "Missing client_id"
    );
    let duplicate = post(
        "application/x-www-form-urlencoded".into(),
        "client_id=a&client_id=b".into(),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(duplicate.status(), 400);
    let duplicate_type = post(typ.into(), String::new())
        .header("content-type", "application/x-www-form-urlencoded")
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate_type.status(), 400);
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("*")
        .query(&mut f.server.connection())
        .unwrap();
    assert!(
        keys.is_empty(),
        "invalid forms must not issue or consume authority"
    );
    f.host.shutdown().await.unwrap();
}
impl Fixture {
    async fn public_pair(&self) -> (String, Value) {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use sha2::{Digest as _, Sha256};
        let registration=self.client.post(format!("{}/register",self.base())).json(&json!({"redirect_uris":["http://localhost/cb"],"token_endpoint_auth_method":"none","scope":"read write"})).send().await.unwrap();
        assert_eq!(registration.status(), 201);
        let client = registration.json::<Value>().await.unwrap()["client_id"]
            .as_str()
            .unwrap()
            .to_owned();
        let verifier = "v".repeat(48);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("client_id", client.as_str()),
                ("response_type", "code"),
                ("code_challenge", challenge.as_str()),
                ("redirect_uri", "http://localhost/cb"),
                ("scope", "read"),
                ("resource", "http://127.0.0.1/mcp"),
            ])
            .finish();
        let auth = self
            .client
            .get(format!("{}/authorize?{query}", self.base()))
            .send()
            .await
            .unwrap();
        assert_eq!(auth.status(), 302);
        let location = url::Url::parse(auth.headers()["location"].to_str().unwrap()).unwrap();
        let code = location
            .query_pairs()
            .find(|(k, _)| k == "code")
            .unwrap()
            .1
            .into_owned();
        let encoded = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("client_id", client.as_str()),
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", "http://localhost/cb"),
                ("code_verifier", verifier.as_str()),
            ])
            .finish();
        let tokens = self
            .client
            .post(format!("{}/token", self.base()))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(encoded)
            .send()
            .await
            .unwrap();
        assert_eq!(tokens.status(), 200);
        (client, tokens.json().await.unwrap())
    }
    async fn revoke(&self, client: &str, token: &str) {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("client_id", client),
                ("client_secret", ""),
                ("token", token),
            ])
            .finish();
        let response = self
            .client
            .post(format!("{}/revoke", self.base()))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
}
#[tokio::test]
async fn independent_rmcp_clients_authenticate_and_revocation_blocks_existing_sessions() {
    use ssx_mcp::{gateway::Gateway, harness::http_router_mode};
    use std::time::Duration;
    for stateful in [false, true] {
        let f = Fixture::new().await;
        let (client, pair) = f.public_pair().await;
        let access = pair["access_token"].as_str().unwrap();
        let cancel = CancellationToken::new();
        let endpoints = OAuthEndpoints::new(
            f.store.clone(),
            OAuthSettings::new("http://127.0.0.1", OAuthTtls::default()).unwrap(),
        )
        .unwrap();
        let router = http_router_mode(Registry::synthetic(), None, cancel.clone(), stateful)
            .map(|r| NetworkAuth::OAuth(endpoints).mount(r));
        let host = Host::start(router, cancel).await.unwrap();
        let url = format!("http://{}/mcp", host.address);
        assert!(
            Gateway::connect(url.clone(), None, Duration::from_secs(2))
                .await
                .is_err()
        );
        let gateway = Gateway::connect(url.clone(), Some(access.into()), Duration::from_secs(3))
            .await
            .unwrap();
        assert!(!gateway.catalog["tools"].as_array().unwrap().is_empty());
        assert!(gateway.service.list_tools(None).await.is_ok());
        f.revoke(&client, access).await;
        assert!(
            gateway.service.list_tools(None).await.is_err(),
            "revocation must apply to subsequent requests in an established session"
        );
        assert!(
            Gateway::connect(url, Some(access.into()), Duration::from_secs(2))
                .await
                .is_err()
        );
        gateway.service.cancel().await.unwrap();
        host.shutdown().await.unwrap();
        f.host.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn static_bearer_cli_precedence_and_every_registered_network_method_are_guarded() {
    use ssx_domain::oauth::Secret;
    use ssx_mcp::{harness::http_router_mode, legacy::Legacy};
    use std::time::Duration;
    for legacy in [false, true] {
        let cancel = CancellationToken::new();
        let auth = NetworkAuth::Static(Secret::new("synthetic-static-token".into()).unwrap());
        let router = if legacy {
            let l = Legacy::new(
                Registry::synthetic(),
                "/messages/",
                Duration::from_secs(10),
                cancel.clone(),
            )
            .unwrap();
            Host::start(auth.mount(l.router("/sse").unwrap()), cancel)
                .await
                .unwrap()
        } else {
            Host::start(
                http_router_mode(Registry::synthetic(), None, cancel.clone(), true)
                    .map(|r| auth.mount(r)),
                cancel,
            )
            .await
            .unwrap()
        };
        let client = reqwest::Client::new();
        let paths = if legacy {
            vec![
                "/sse",
                "/messages",
                "/messages/",
                "/messages/opaque?session_id=synthetic",
            ]
        } else {
            vec!["/mcp"]
        };
        for path in paths {
            for method in [
                reqwest::Method::GET,
                reqwest::Method::HEAD,
                reqwest::Method::POST,
                reqwest::Method::DELETE,
                reqwest::Method::PUT,
                reqwest::Method::OPTIONS,
            ] {
                let url = format!("http://{}{path}", router.address);
                for credential in [None, Some("Bearer wrong")] {
                    let mut request = client.request(method.clone(), &url);
                    if let Some(value) = credential {
                        request = request.header("authorization", value);
                    }
                    assert_eq!(
                        request.send().await.unwrap().status(),
                        401,
                        "{legacy}/{path}/{method}"
                    );
                }
                assert_eq!(
                    client
                        .request(method.clone(), &url)
                        .header("authorization", "Bearer synthetic-static-token")
                        .header("authorization", "Bearer synthetic-static-token")
                        .send()
                        .await
                        .unwrap()
                        .status(),
                    401
                );
            }
        }
        let allowed = client
            .post(format!(
                "http://{}{}",
                router.address,
                if legacy { "/messages/" } else { "/mcp" }
            ))
            .header("authorization", "bEaReR synthetic-static-token")
            .header("accept", "application/json, text/event-stream")
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"ping"}))
            .send()
            .await
            .unwrap();
        assert_ne!(allowed.status(), 401);
        router.shutdown().await.unwrap();
    }
    let f = Fixture::new().await;
    let oauth = OAuthEndpoints::new(
        f.store.clone(),
        OAuthSettings::new("http://127.0.0.1", OAuthTtls::default()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        NetworkAuth::from_cli(
            Some(oauth),
            Some(Secret::new("synthetic-static".into()).unwrap())
        ),
        NetworkAuth::OAuth(_)
    ));
    f.host.shutdown().await.unwrap();
}
#[tokio::test]
async fn oauth_guards_legacy_sse_and_message_suffix_after_token_revocation() {
    use ssx_mcp::legacy::Legacy;
    use std::time::Duration;
    let f = Fixture::new().await;
    let (client, pair) = f.public_pair().await;
    let access = pair["access_token"].as_str().unwrap();
    let cancel = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/messages/",
        Duration::from_secs(10),
        cancel.clone(),
    )
    .unwrap();
    let oauth = OAuthEndpoints::new(
        f.store.clone(),
        OAuthSettings::new("http://127.0.0.1", OAuthTtls::default()).unwrap(),
    )
    .unwrap();
    let host = Host::start(
        NetworkAuth::OAuth(oauth).mount(legacy.clone().router("/sse").unwrap()),
        cancel,
    )
    .await
    .unwrap();
    let base = format!("http://{}", host.address);
    for path in [
        "/sse",
        "/messages",
        "/messages/",
        "/messages/nested?session_id=synthetic",
    ] {
        for method in [
            reqwest::Method::GET,
            reqwest::Method::HEAD,
            reqwest::Method::POST,
            reqwest::Method::DELETE,
            reqwest::Method::OPTIONS,
            reqwest::Method::PUT,
        ] {
            for credential in [None, Some("Bearer wrong")] {
                let mut request = f.client.request(method.clone(), format!("{base}{path}"));
                if let Some(value) = credential {
                    request = request.header("authorization", value);
                }
                assert_eq!(
                    request.send().await.unwrap().status(),
                    401,
                    "{path}/{method}"
                );
            }
            assert_eq!(
                f.client
                    .request(method.clone(), format!("{base}{path}"))
                    .bearer_auth(access)
                    .header("authorization", format!("Bearer {access}"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                401
            );
        }
    }
    assert_eq!(legacy.active_sessions(), 0);
    let mut stream = f
        .client
        .get(format!("{base}/sse"))
        .bearer_auth(access)
        .send()
        .await
        .unwrap();
    assert_eq!(stream.status(), 200);
    let mut bytes = Vec::new();
    let endpoint = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            bytes.extend_from_slice(&stream.chunk().await.unwrap().unwrap());
            assert!(bytes.len() < 4096);
            let text = String::from_utf8(bytes.clone())
                .unwrap()
                .replace("\r\n", "\n");
            if text.contains("\n\n") {
                break text
                    .lines()
                    .find_map(|l| l.strip_prefix("data: "))
                    .unwrap()
                    .to_owned();
            }
        }
    })
    .await
    .unwrap();
    let response=f.client.post(format!("{base}{endpoint}")).bearer_auth(access).json(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"independent-t05","version":"1"}}})).send().await.unwrap();
    assert_eq!(response.status(), 202);
    f.revoke(&client, access).await;
    for path in [endpoint.as_str(), "/sse"] {
        let response = f
            .client
            .post(format!("{base}{path}"))
            .bearer_auth(access)
            .json(&json!({"jsonrpc":"2.0","id":2,"method":"ping"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
    }
    drop(stream);
    host.shutdown().await.unwrap();
    f.host.shutdown().await.unwrap();
}
#[tokio::test]
async fn concurrent_refresh_has_one_winner_and_expired_secret_never_issues_tokens() {
    let f = Fixture::new().await;
    let (client, pair) = f.public_pair().await;
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("client_id", client.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", pair["refresh_token"].as_str().unwrap()),
        ])
        .finish();
    let send = || {
        f.client
            .post(format!("{}/token", f.base()))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body.clone())
            .send()
    };
    let (a, b) = tokio::join!(send(), send());
    let (a, b) = (a.unwrap(), b.unwrap());
    let mut statuses = vec![a.status().as_u16(), b.status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, vec![200, 401]);
    let success = if a.status() == 200 { a } else { b };
    let next = success.json::<Value>().await.unwrap();
    assert!(
        f.store
            .load_token(
                ssx_domain::oauth::TokenKind::Access,
                &ssx_domain::oauth::Secret::new(pair["access_token"].as_str().unwrap().into())
                    .unwrap()
            )
            .await
            .is_err()
    );
    f.revoke(&client, next["refresh_token"].as_str().unwrap())
        .await;
    assert!(
        f.store
            .load_token(
                ssx_domain::oauth::TokenKind::Access,
                &ssx_domain::oauth::Secret::new(next["access_token"].as_str().unwrap().into())
                    .unwrap()
            )
            .await
            .is_err()
    );
    let registration = f
        .client
        .post(format!("{}/register", f.base()))
        .json(&json!({"redirect_uris":["http://localhost/cb"]}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let id = registration["client_id"].as_str().unwrap();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("*:client:*")
        .query(&mut f.server.connection())
        .unwrap();
    let now = f.store.now_ms().await.unwrap() / 1000;
    let mut altered = false;
    for key in keys {
        let raw: String = redis::cmd("GET")
            .arg(&key)
            .query(&mut f.server.connection())
            .unwrap();
        let mut record: Value = serde_json::from_str(&raw).unwrap();
        if record["data"]["client_id"] == id {
            record["data"]["metadata"]["client_secret_expires_at"] = json!(now - 1);
            redis::cmd("SET")
                .arg(&key)
                .arg(record.to_string())
                .arg("KEEPTTL")
                .query::<()>(&mut f.server.connection())
                .unwrap();
            altered = true;
        }
    }
    assert!(altered);
    let form = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("client_id", id),
            (
                "client_secret",
                registration["client_secret"].as_str().unwrap(),
            ),
            ("grant_type", "refresh_token"),
            ("refresh_token", "unknown"),
        ])
        .finish();
    let response = f
        .client
        .post(format!("{}/token", f.base()))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error_description"],
        "Client secret has expired"
    );
    f.host.shutdown().await.unwrap();
}
/// Test-only fault boundary: real state commits, then its classified unknown
/// outcome reaches the endpoint. T04 separately proves the lost-wire-reply case.
struct UnknownRegistration {
    inner: Arc<OAuthStore>,
    calls: std::sync::atomic::AtomicUsize,
}
impl OAuthState for UnknownRegistration {
    fn now_ms(
        &self,
    ) -> ssx_application::PortFuture<'_, Result<u64, ssx_domain::oauth::OAuthError>> {
        self.inner.now_ms()
    }
    fn register_client(
        &self,
        c: ssx_domain::oauth::ClientRegistration,
    ) -> ssx_application::PortFuture<'_, Result<(), ssx_domain::oauth::OAuthError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.inner.register_client(c).await?;
            Err(ssx_domain::oauth::OAuthError::CompletionUnknown)
        })
    }
    fn get_client<'a>(
        &'a self,
        id: &'a str,
    ) -> ssx_application::PortFuture<
        'a,
        Result<ssx_domain::oauth::ClientRegistration, ssx_domain::oauth::OAuthError>,
    > {
        Box::pin(self.inner.get_client(id))
    }
    fn create_code(
        &self,
        g: ssx_domain::oauth::CodeGrant,
    ) -> ssx_application::PortFuture<
        '_,
        Result<ssx_domain::oauth::IssuedCode, ssx_domain::oauth::OAuthError>,
    > {
        Box::pin(self.inner.create_code(g))
    }
    fn load_code<'a>(
        &'a self,
        c: &'a ssx_domain::oauth::Secret,
    ) -> ssx_application::PortFuture<
        'a,
        Result<ssx_domain::oauth::IssuedCode, ssx_domain::oauth::OAuthError>,
    > {
        Box::pin(self.inner.load_code(c))
    }
    fn redeem_code<'a>(
        &'a self,
        c: &'a ssx_domain::oauth::Secret,
        g: &'a ssx_domain::oauth::CodeGrant,
    ) -> ssx_application::PortFuture<
        'a,
        Result<ssx_domain::oauth::TokenPair, ssx_domain::oauth::OAuthError>,
    > {
        Box::pin(self.inner.redeem_code(c, g))
    }
    fn load_token<'a>(
        &'a self,
        k: ssx_domain::oauth::TokenKind,
        t: &'a ssx_domain::oauth::Secret,
    ) -> ssx_application::PortFuture<
        'a,
        Result<ssx_domain::oauth::LoadedGrant, ssx_domain::oauth::OAuthError>,
    > {
        Box::pin(self.inner.load_token(k, t))
    }
    fn rotate_refresh<'a>(
        &'a self,
        t: &'a ssx_domain::oauth::Secret,
        g: &'a ssx_domain::oauth::TokenGrant,
        s: &'a [String],
    ) -> ssx_application::PortFuture<
        'a,
        Result<ssx_domain::oauth::TokenPair, ssx_domain::oauth::OAuthError>,
    > {
        Box::pin(self.inner.rotate_refresh(t, g, s))
    }
    fn revoke<'a>(
        &'a self,
        k: ssx_domain::oauth::TokenKind,
        t: &'a ssx_domain::oauth::Secret,
        c: &'a str,
    ) -> ssx_application::PortFuture<'a, Result<(), ssx_domain::oauth::OAuthError>> {
        Box::pin(self.inner.revoke(k, t, c))
    }
}
#[tokio::test]
async fn committed_registration_unknown_outcome_returns_no_credentials_and_never_retries() {
    let f = Fixture::new().await;
    let state = Arc::new(UnknownRegistration {
        inner: f.store.clone(),
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let oauth = OAuthEndpoints::new(
        state.clone(),
        OAuthSettings::new("http://127.0.0.1", OAuthTtls::default()).unwrap(),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let host = Host::start(
        http_router(Registry::synthetic(), None, cancel.clone())
            .map(|r| NetworkAuth::OAuth(oauth).mount(r)),
        cancel,
    )
    .await
    .unwrap();
    let response = f
        .client
        .post(format!("http://{}/register", host.address))
        .json(&json!({"redirect_uris":["http://localhost/cb"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    assert!(
        response
            .json::<Value>()
            .await
            .unwrap()
            .get("client_id")
            .is_none()
    );
    assert_eq!(state.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("*:client:*")
        .query(&mut f.server.connection())
        .unwrap();
    assert_eq!(
        keys.len(),
        1,
        "the committed unknown result must not cause a second registration"
    );
    host.shutdown().await.unwrap();
    f.host.shutdown().await.unwrap();
}
#[tokio::test]
async fn endpoint_configuration_accepts_only_https_or_actual_loopback_and_exact_resource() {
    let f = Fixture::new().await;
    for issuer in [
        "http://localhost:8000",
        "http://127.0.0.1:8000",
        "http://127.9.8.7:8000",
        "http://[::1]:8000",
        "http://127.0.0.1/.",
        "http://127.0.0.1/path/..",
        "https://public.example",
    ] {
        let settings = OAuthSettings::new(issuer, OAuthTtls::default()).unwrap();
        assert!(
            OAuthEndpoints::new(f.store.clone(), settings).is_ok(),
            "{issuer}"
        );
    }
    for issuer in [
        "http://192.0.2.1",
        "http://[2001:db8::1]",
        "http://example.com",
    ] {
        let settings = OAuthSettings::new(issuer, OAuthTtls::default()).unwrap();
        assert!(
            OAuthEndpoints::new(f.store.clone(), settings).is_err(),
            "{issuer}"
        );
    }
    let mut altered = OAuthSettings::new("http://127.0.0.1", OAuthTtls::default()).unwrap();
    altered.resource_server_url = "https://other.example/mcp".into();
    assert!(OAuthEndpoints::new(f.store.clone(), altered).is_err());
    f.host.shutdown().await.unwrap();
}
#[tokio::test]
async fn corrupt_persisted_client_authority_denies_without_consuming_code_and_recovers_exactly() {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use sha2::{Digest as _, Sha256};
    use ssx_domain::oauth::{CodeGrant, Secret, TokenKind};
    let f = Fixture::new().await;
    let registration = f
        .client
        .post(format!("{}/register", f.base()))
        .json(&json!({"redirect_uris":["http://localhost/cb"]}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let client = registration["client_id"].as_str().unwrap();
    let secret = registration["client_secret"].as_str().unwrap();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("*:client:*")
        .query(&mut f.server.connection())
        .unwrap();
    assert_eq!(keys.len(), 1);
    let key = &keys[0];
    let original: String = redis::cmd("GET")
        .arg(key)
        .query(&mut f.server.connection())
        .unwrap();
    let verifier = "c".repeat(48);
    let grant = CodeGrant {
        client_id: client.into(),
        scopes: vec![],
        code_challenge: URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
        redirect_uri: "http://localhost/cb".into(),
        redirect_uri_provided_explicitly: true,
        resource: Some("http://127.0.0.1/mcp".into()),
        subject: Some("operator".into()),
    };
    let cases = [
        (
            "expiry-string",
            "client_secret_expires_at",
            Some(json!("NaN")),
        ),
        (
            "expiry-negative",
            "client_secret_expires_at",
            Some(json!(-1)),
        ),
        ("expiry-float", "client_secret_expires_at", Some(json!(1.5))),
        (
            "expiry-bool",
            "client_secret_expires_at",
            Some(json!(false)),
        ),
        ("expiry-array", "client_secret_expires_at", Some(json!([]))),
        (
            "expiry-nonfinite-json",
            "client_secret_expires_at",
            Some(json!(0)),
        ),
        ("method-missing", "token_endpoint_auth_method", None),
        (
            "method-null",
            "token_endpoint_auth_method",
            Some(Value::Null),
        ),
        (
            "method-number",
            "token_endpoint_auth_method",
            Some(json!(1)),
        ),
        (
            "method-unknown",
            "token_endpoint_auth_method",
            Some(json!(secret)),
        ),
    ];
    let mut errors = Vec::new();
    for (label, field, value) in cases {
        let code = f.store.create_code(grant.clone()).await.unwrap();
        let mut record: Value = serde_json::from_str(&original).unwrap();
        if let Some(value) = value {
            record["data"]["metadata"][field] = value;
        } else {
            record["data"]["metadata"]
                .as_object_mut()
                .unwrap()
                .remove(field);
        }
        let corrupt_raw = if label == "expiry-nonfinite-json" {
            record.to_string().replace(
                "\"client_secret_expires_at\":0",
                "\"client_secret_expires_at\":1e999",
            )
        } else {
            record.to_string()
        };
        redis::cmd("SET")
            .arg(key)
            .arg(&corrupt_raw)
            .arg("KEEPTTL")
            .query::<()>(&mut f.server.connection())
            .unwrap();
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("client_id", client),
                ("client_secret", secret),
                ("grant_type", "authorization_code"),
                ("code", code.code.expose()),
                ("code_verifier", verifier.as_str()),
                ("redirect_uri", "http://localhost/cb"),
            ])
            .finish();
        let send = || {
            f.client
                .post(format!("{}/token", f.base()))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(body.clone())
                .send()
        };
        let response = send().await.unwrap();
        let status = response.status().as_u16();
        let text = response.text().await.unwrap();
        if status != 503 {
            errors.push(format!("{label}: expected503 got{status}"));
        }
        if text.contains(secret) {
            errors.push(format!("{label}: stored secret echoed"));
        }
        let persisted: String = redis::cmd("GET")
            .arg(key)
            .query(&mut f.server.connection())
            .unwrap();
        assert_eq!(
            persisted, corrupt_raw,
            "rejected authentication must not rewrite its record"
        );
        if status == 503 {
            let issued: Vec<String> = redis::cmd("KEYS")
                .arg("*:access:*")
                .query(&mut f.server.connection())
                .unwrap();
            assert!(
                issued.is_empty(),
                "corrupt authority must not issue access tokens"
            );
        }
        let retained = f.store.load_code(&code.code).await.is_ok();
        if !retained {
            errors.push(format!("{label}: code was consumed"));
        }
        if status == 200 {
            let pair: Value = serde_json::from_str(&text).unwrap();
            f.store
                .revoke(
                    TokenKind::Access,
                    &Secret::new(pair["access_token"].as_str().unwrap().into()).unwrap(),
                    client,
                )
                .await
                .unwrap();
        }
        redis::cmd("SET")
            .arg(key)
            .arg(&original)
            .arg("KEEPTTL")
            .query::<()>(&mut f.server.connection())
            .unwrap();
        if retained {
            let response = send().await.unwrap();
            assert_eq!(
                response.status(),
                200,
                "corrected record must redeem the original code"
            );
            let pair = response.json::<Value>().await.unwrap();
            f.store
                .revoke(
                    TokenKind::Access,
                    &Secret::new(pair["access_token"].as_str().unwrap().into()).unwrap(),
                    client,
                )
                .await
                .unwrap();
        }
    }
    for value in [None, Some(Value::Null)] {
        let code = f.store.create_code(grant.clone()).await.unwrap();
        let mut record: Value = serde_json::from_str(&original).unwrap();
        if let Some(value) = value {
            record["data"]["metadata"]["client_secret_expires_at"] = value;
        } else {
            record["data"]["metadata"]
                .as_object_mut()
                .unwrap()
                .remove("client_secret_expires_at");
        }
        redis::cmd("SET")
            .arg(key)
            .arg(record.to_string())
            .arg("KEEPTTL")
            .query::<()>(&mut f.server.connection())
            .unwrap();
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("client_id", client),
                ("client_secret", secret),
                ("grant_type", "authorization_code"),
                ("code", code.code.expose()),
                ("code_verifier", verifier.as_str()),
                ("redirect_uri", "http://localhost/cb"),
            ])
            .finish();
        let response = f
            .client
            .post(format!("{}/token", f.base()))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            200,
            "source missing/null optional expiry"
        );
        redis::cmd("SET")
            .arg(key)
            .arg(&original)
            .arg("KEEPTTL")
            .query::<()>(&mut f.server.connection())
            .unwrap();
    }
    f.host.shutdown().await.unwrap();
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[tokio::test]
async fn request_shape_errors_preserve_existing_code_before_valid_redemption() {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use sha2::{Digest as _, Sha256};
    use ssx_domain::oauth::CodeGrant;
    let f = Fixture::new().await;
    let registration = f
        .client
        .post(format!("{}/register", f.base()))
        .json(&json!({"redirect_uris":["http://localhost/cb"],"token_endpoint_auth_method":"none"}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let client = registration["client_id"].as_str().unwrap();
    let verifier = "v".repeat(48);
    let grant = CodeGrant {
        client_id: client.into(),
        scopes: vec![],
        code_challenge: URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
        redirect_uri: "http://localhost/cb".into(),
        redirect_uri_provided_explicitly: true,
        resource: Some("http://127.0.0.1/mcp".into()),
        subject: None,
    };
    let code = f.store.create_code(grant.clone()).await.unwrap();
    let valid = json!({"grant_type":"authorization_code","client_id":client,"code":code.code.expose(),"redirect_uri":"http://localhost/cb","code_verifier":verifier});
    for (field, value, expected) in [
        (
            "redirect_uri",
            Some("not a url"),
            "authorization_code.redirect_uri: Input should be a valid URL, relative URL without a base",
        ),
        (
            "code_verifier",
            None,
            "authorization_code.code_verifier: Field required",
        ),
        ("code", None, "authorization_code.code: Field required"),
    ] {
        let mut data = valid.clone();
        if let Some(value) = value {
            data[field] = json!(value);
        } else {
            data.as_object_mut().unwrap().remove(field);
        }
        let response = f
            .client
            .post(format!("{}/token", f.base()))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(
                url::form_urlencoded::Serializer::new(String::new())
                    .extend_pairs(
                        data.as_object()
                            .unwrap()
                            .iter()
                            .map(|(k, v)| (k.as_str(), v.as_str().unwrap())),
                    )
                    .finish(),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        let body = response.json::<Value>().await.unwrap();
        assert_eq!(
            body,
            json!({"error":"invalid_request","error_description":expected})
        );
        assert_eq!(f.store.load_code(&code.code).await.unwrap().grant, grant);
    }
    let response = f
        .client
        .post(format!("{}/token", f.base()))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(
            url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(
                    valid
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str().unwrap())),
                )
                .finish(),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.json::<Value>().await.unwrap()["access_token"].is_string());
    assert!(f.store.load_code(&code.code).await.is_err());
    f.host.shutdown().await.unwrap();
}

#[tokio::test]
async fn stored_grant_defaults_and_corruption_have_structured_nonconsuming_responses() {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use sha2::{Digest as _, Sha256};
    use ssx_domain::oauth::CodeGrant;
    let f = Fixture::new().await;
    let registration = f
        .client
        .post(format!("{}/register", f.base()))
        .json(&json!({"redirect_uris":["http://localhost/cb"],"token_endpoint_auth_method":"none"}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let client = registration["client_id"].as_str().unwrap();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("*:client:*")
        .query(&mut f.server.connection())
        .unwrap();
    assert_eq!(keys.len(), 1);
    let original: String = redis::cmd("GET")
        .arg(&keys[0])
        .query(&mut f.server.connection())
        .unwrap();
    let verifier = "g".repeat(48);
    let grant = CodeGrant {
        client_id: client.into(),
        scopes: vec![],
        code_challenge: URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
        redirect_uri: "http://localhost/cb".into(),
        redirect_uri_provided_explicitly: true,
        resource: Some("http://127.0.0.1/mcp".into()),
        subject: None,
    };
    for (label, value, status, error, description) in [
        (
            "missing",
            None,
            401,
            "invalid_grant",
            "incorrect code_verifier",
        ),
        (
            "null",
            Some(Value::Null),
            401,
            "invalid_grant",
            "incorrect code_verifier",
        ),
        (
            "placeholder",
            Some(json!("")),
            401,
            "invalid_grant",
            "incorrect code_verifier",
        ),
        (
            "empty",
            Some(json!([])),
            400,
            "unsupported_grant_type",
            "Unsupported grant type (supported grant types are [])",
        ),
        (
            "custom",
            Some(json!(["custom"])),
            400,
            "unsupported_grant_type",
            "Unsupported grant type (supported grant types are ['custom'])",
        ),
        (
            "number",
            Some(json!(1)),
            503,
            "temporarily_unavailable",
            "OAuth authority is unavailable",
        ),
        (
            "boolean",
            Some(json!(true)),
            503,
            "temporarily_unavailable",
            "OAuth authority is unavailable",
        ),
        (
            "object",
            Some(json!({})),
            503,
            "temporarily_unavailable",
            "OAuth authority is unavailable",
        ),
        (
            "mixed",
            Some(json!(["authorization_code", 1])),
            503,
            "temporarily_unavailable",
            "OAuth authority is unavailable",
        ),
    ] {
        let code = f.store.create_code(grant.clone()).await.unwrap();
        let mut record: Value = serde_json::from_str(&original).unwrap();
        if let Some(value) = value {
            record["data"]["metadata"]["grant_types"] = value;
        } else {
            record["data"]["metadata"]
                .as_object_mut()
                .unwrap()
                .remove("grant_types");
        }
        let raw = record.to_string();
        redis::cmd("SET")
            .arg(&keys[0])
            .arg(&raw)
            .arg("KEEPTTL")
            .query::<()>(&mut f.server.connection())
            .unwrap();
        let encode = |verifier: &str| {
            url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs([
                    ("client_id", client),
                    ("grant_type", "authorization_code"),
                    ("code", code.code.expose()),
                    ("redirect_uri", "http://localhost/cb"),
                    ("code_verifier", verifier),
                ])
                .finish()
        };
        let response = f
            .client
            .post(format!("{}/token", f.base()))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(encode("wrong"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status, "{label}");
        assert_eq!(
            response.json::<Value>().await.unwrap(),
            json!({"error":error,"error_description":description}),
            "{label}"
        );
        assert_eq!(
            f.store.load_code(&code.code).await.unwrap().grant,
            grant,
            "{label}"
        );
        let after: String = redis::cmd("GET")
            .arg(&keys[0])
            .query(&mut f.server.connection())
            .unwrap();
        assert_eq!(
            after, raw,
            "projection must not rewrite stored metadata: {label}"
        );
        redis::cmd("SET")
            .arg(&keys[0])
            .arg(&original)
            .arg("KEEPTTL")
            .query::<()>(&mut f.server.connection())
            .unwrap();
        let response = f
            .client
            .post(format!("{}/token", f.base()))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(encode(&verifier))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            200,
            "correct record still redeems: {label}"
        );
        assert!(response.json::<Value>().await.unwrap()["access_token"].is_string());
        assert!(f.store.load_code(&code.code).await.is_err());
    }
    f.host.shutdown().await.unwrap();
}

#[tokio::test]
async fn corrupt_code_only_fields_are_rejected_without_consumption() {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use sha2::{Digest as _, Sha256};
    use ssx_domain::oauth::{CodeGrant, OAuthError};
    let f = Fixture::new().await;
    let registration = f
        .client
        .post(format!("{}/register", f.base()))
        .json(&json!({"redirect_uris":["http://localhost/cb"]}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let client = registration["client_id"].as_str().unwrap();
    let secret = registration["client_secret"].as_str().unwrap();
    let verifier = "v".repeat(48);
    let grant = CodeGrant {
        client_id: client.into(),
        scopes: vec![],
        code_challenge: URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
        redirect_uri: "http://localhost/cb".into(),
        redirect_uri_provided_explicitly: false,
        resource: None,
        subject: None,
    };
    let code = f.store.create_code(grant).await.unwrap();
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("*:code:*")
        .query(&mut f.server.connection())
        .unwrap();
    assert_eq!(keys.len(), 1);
    let key = &keys[0];
    let original: String = redis::cmd("GET")
        .arg(key)
        .query(&mut f.server.connection())
        .unwrap();
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("client_id", client),
            ("client_secret", secret),
            ("grant_type", "authorization_code"),
            ("code", code.code.expose()),
            ("code_verifier", verifier.as_str()),
        ])
        .finish();
    for (field, value) in [
        ("redirect_uri", "http://localhost/\ncb".to_owned()),
        ("redirect_uri", String::new()),
        ("redirect_uri", "r".repeat(4097)),
        ("code_challenge", String::new()),
        ("code_challenge", "bad\nchallenge".into()),
        ("code_challenge", "c".repeat(4097)),
    ] {
        let mut record: Value = serde_json::from_str(&original).unwrap();
        record["data"][field] = json!(value);
        let corrupt = record.to_string();
        redis::cmd("SET")
            .arg(key)
            .arg(&corrupt)
            .arg("KEEPTTL")
            .query::<()>(&mut f.server.connection())
            .unwrap();
        let response = f
            .client
            .post(format!("{}/token", f.base()))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body.clone())
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status().as_u16(),
            503,
            "corrupt {field} must fail closed"
        );
        assert!(matches!(
            f.store.load_code(&code.code).await,
            Err(OAuthError::Malformed)
        ));
        let changed: CodeGrant = serde_json::from_value(record["data"].clone()).unwrap();
        assert!(matches!(
            f.store.redeem_code(&code.code, &changed).await,
            Err(OAuthError::Malformed)
        ));
        let retained: String = redis::cmd("GET")
            .arg(key)
            .query(&mut f.server.connection())
            .unwrap();
        assert_eq!(retained, corrupt);
        for pattern in ["*:access:*", "*:refresh:*"] {
            let tokens: Vec<String> = redis::cmd("KEYS")
                .arg(pattern)
                .query(&mut f.server.connection())
                .unwrap();
            assert!(tokens.is_empty());
        }
    }
    redis::cmd("SET")
        .arg(key)
        .arg(original)
        .arg("KEEPTTL")
        .query::<()>(&mut f.server.connection())
        .unwrap();
    let response = f
        .client
        .post(format!("{}/token", f.base()))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    f.host.shutdown().await.unwrap();
}
