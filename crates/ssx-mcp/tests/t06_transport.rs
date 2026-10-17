//! Actual loopback/subprocess runtime; no provider calls or production state.
#[path = "t06_support/comparison.rs"]
mod comparison;
#[path = "t06_support/fixture.rs"]
mod fixture;
#[path = "t06_support/server.rs"]
mod private_valkey;
use comparison::Comparison;
use ssx_json::{Value, json};
use ssx_mcp::{
    Result,
    harness::{Host, http_router_for},
    oauth::NetworkAuth,
    search_surface::SearchSurface,
};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;
fn source() -> Value {
    ssx_json::from_str(include_str!("../../../docs/rust-rewrite/t06/source.json")).unwrap()
}
fn matrix() -> Value {
    ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/t06/validation.json"
    ))
    .unwrap()
}
// Diagnostic only: serialize after the timed call has fully completed. No input,
// comparison or deadline behavior changes, and no partially emitted diagnostic.
fn tiny_deadline_diagnostic(raw: (Value, Value), normalized: (&Value, &Value)) -> Result<()> {
    const LIMIT: usize = 1024 * 1024;
    struct Bounded(Vec<u8>);
    impl std::io::Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) >= LIMIT {
                return Err(std::io::Error::other(
                    "tiny deadline diagnostic exceeds 1 MiB",
                ));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Bounded(b"T06_TINY_DEADLINE_DIAGNOSTIC ".to_vec());
    serde_json::to_writer(
        &mut output,
        &json!({
            "name": "slopsearx_search",
            "arguments": {"query": "q", "interactive_timeout_ms": true},
            "raw_actual_result": raw.0,
            "raw_expected_result": raw.1,
            "normalized_actual_result": normalized.0,
            "normalized_expected_result": normalized.1,
        }),
    )?;
    output.0.push(b'\n');
    use std::io::Write as _;
    std::io::stdout().lock().write_all(&output.0)?;
    Ok(())
}
fn first_difference(a: &Value, b: &Value, path: &str) -> String {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in a {
                if b.get(k) != Some(v) {
                    return first_difference(
                        v,
                        b.get(k).unwrap_or(&Value::Null),
                        &format!("{path}.{k}"),
                    );
                }
            }
            for (k, v) in b {
                if !a.contains_key(k) {
                    return format!("{path}.{k}: missing expected={v}");
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return format!("{path}: lengths {} != {}", a.len(), b.len());
            }
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                if a != b {
                    return first_difference(a, b, &format!("{path}[{i}]"));
                }
            }
        }
        (Value::String(a), Value::String(b)) if path.ends_with(".text") => {
            if let (Ok(a), Ok(b)) = (
                ssx_json::from_str::<Value>(a),
                ssx_json::from_str::<Value>(b),
            ) && a != b
            {
                return first_difference(&a, &b, &format!("{path}:json"));
            }
        }
        _ => {}
    }
    let a = a.to_string();
    let b = b.to_string();
    let at = a
        .bytes()
        .zip(b.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(a.len().min(b.len()));
    format!(
        "{path}: first differing byte {at}; actual={} expected={}",
        a.chars()
            .skip(at.saturating_sub(35))
            .take(120)
            .collect::<String>(),
        b.chars()
            .skip(at.saturating_sub(35))
            .take(120)
            .collect::<String>()
    )
}
async fn legacy_event(
    stream: &mut reqwest::Response,
    pending: &mut Vec<u8>,
) -> Result<(String, String)> {
    loop {
        if let Some(i) = pending.windows(2).position(|w| w == b"\n\n") {
            let bytes: Vec<_> = pending.drain(..i + 2).collect();
            let frame = String::from_utf8(bytes)?;
            let kind = frame
                .lines()
                .find_map(|s| s.strip_prefix("event: "))
                .unwrap_or("message")
                .to_owned();
            let data = frame
                .lines()
                .filter_map(|s| s.strip_prefix("data: "))
                .collect::<Vec<_>>()
                .join("\n");
            if !data.is_empty() {
                return Ok((kind, data));
            }
        }
        let chunk = tokio::time::timeout(Duration::from_secs(10), stream.chunk())
            .await??
            .ok_or("legacy stream EOF")?;
        pending.extend_from_slice(&chunk);
        assert!(pending.len() < 4 * 1024 * 1024 + 65536);
    }
}
enum Client {
    Http {
        client: reqwest::Client,
        url: String,
        id: u64,
    },
    Legacy {
        client: reqwest::Client,
        url: String,
        stream: reqwest::Response,
        pending: Vec<u8>,
        id: u64,
    },
    Stdio {
        child: tokio::process::Child,
        input: tokio::process::ChildStdin,
        output: tokio::io::BufReader<tokio::process::ChildStdout>,
        stderr: tokio::task::JoinHandle<std::io::Result<String>>,
        dispatch_receipt: Box<redis::Client>,
        id: u64,
    },
}
impl Client {
    async fn rpc(&mut self, method: &str, params: Value) -> Result<Value> {
        let (request, id) = match self {
            Self::Http { id, .. } | Self::Stdio { id, .. } | Self::Legacy { id, .. } => {
                *id += 1;
                (
                    ssx_json::to_string(
                        &json!({"jsonrpc":"2.0","id":*id,"method":method,"params":params}),
                    )?,
                    *id,
                )
            }
        };
        self.raw(request, id).await
    }
    async fn raw(&mut self, request: String, id: u64) -> Result<Value> {
        let raw = match self {
            Self::Http { client, url, .. } => {
                let response = client
                    .post(&*url)
                    .header("accept", "application/json,text/event-stream")
                    .header("mcp-protocol-version", "2025-11-25")
                    .header("content-type", "application/json")
                    .body(request)
                    .send()
                    .await?;
                assert_eq!(response.status(), 200);
                response.text().await?
            }
            Self::Legacy {
                client,
                url,
                stream,
                pending,
                ..
            } => {
                let response = client
                    .post(&*url)
                    .header("content-type", "application/json")
                    .body(request)
                    .send()
                    .await?;
                assert_eq!(response.status(), 202);
                legacy_event(stream, pending).await?.1
            }
            Self::Stdio { input, output, .. } => {
                input.write_all(request.as_bytes()).await?;
                input.write_all(b"\n").await?;
                input.flush().await?;
                let mut line = String::new();
                let n = tokio::time::timeout(Duration::from_secs(5), output.read_line(&mut line))
                    .await
                    .map_err(|_| format!("stdio RPC deadline id={id}"))??;
                assert!(n > 0, "native process EOF");
                line
            }
        };
        let v: Value = ssx_json::from_str(&raw)?;
        assert_eq!(v["id"].as_u64(), Some(id));
        Ok(v.get("result")
            .cloned()
            .unwrap_or_else(|| json!({"rpc_error":v["error"]})))
    }
    async fn init(&mut self) -> Result<()> {
        self.rpc("initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t06-native-client","version":"1"}})).await?;
        Ok(())
    }
    async fn call(&mut self, name: &str, args: Value) -> Result<Value> {
        self.rpc("tools/call", json!({"name":name,"arguments":args}))
            .await
    }
    fn dispatches(&self, fixture: Option<&fixture::Fixture>) -> usize {
        if let Some(fixture) = fixture {
            return fixture.calls();
        }
        let Self::Stdio {
            dispatch_receipt, ..
        } = self
        else {
            panic!("missing direct dispatch observer");
        };
        let mut connection = dispatch_receipt
            .get_connection()
            .expect("private dispatch receipt read");
        redis::cmd("KEYS")
            .arg("t06:qualification:dispatch:*")
            .query::<Vec<String>>(&mut connection)
            .expect("private dispatch receipt keys")
            .len()
    }
    async fn stop(self) -> Result<()> {
        if let Self::Stdio {
            mut child,
            mut input,
            stderr,
            ..
        } = self
        {
            input.shutdown().await?;
            drop(input);
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            let status = tokio::time::timeout_at(deadline, child.wait())
                .await
                .map_err(|_| "stdio process failed to drain after stdin EOF")??;
            let stderr = tokio::time::timeout_at(deadline, stderr)
                .await
                .map_err(|_| "stdio stderr failed to drain with child")???;
            if !status.success() {
                return Err(
                    format!("stdio process exited {status}; bounded stderr:\n{stderr}").into(),
                );
            }
        }
        Ok(())
    }
}
async fn verify(client: &mut Client, fixture: Option<&fixture::Fixture>) -> Result<()> {
    client.init().await?;
    let source = source();
    let mut comparison = Comparison::default();
    for (method, key) in [
        ("tools/list", "tools"),
        ("resources/list", "resources"),
        ("resources/templates/list", "resourceTemplates"),
        ("prompts/list", "prompts"),
    ] {
        let out = client.rpc(method, json!({})).await?;
        assert_eq!(
            out[key], source["catalog"][key],
            "complete {method} descriptors"
        );
    }
    let mut mismatches = Vec::new();
    let matrix = matrix();
    let boundaries: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/t06/boundaries.json"
    ))?;
    for row in boundaries["resources"].as_array().unwrap() {
        let result = client
            .rpc("resources/read", json!({"uri":row["uri"]}))
            .await?;
        let (actual, expected) = comparison.pair(result, row["result"].clone(), false);
        if actual != expected {
            mismatches.push(format!(
                "resource {} {}",
                row["uri"],
                first_difference(&actual, &expected, "$")
            ));
        }
    }

    let bounds: Value =
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/t06/bounds.json"))?;
    for row in source["calls"]
        .as_array()
        .unwrap()
        .iter()
        .chain(matrix["cases"].as_array().unwrap())
        .chain(bounds["cases"].as_array().unwrap())
    {
        let name = row["name"].as_str().unwrap();
        let before = client.dispatches(fixture);
        let result = client.call(name, row["arguments"].clone()).await?;
        if row["result"]["isError"].as_bool() == Some(true) {
            {
                assert_eq!(
                    client.dispatches(fixture),
                    before,
                    "malformed args dispatched {name}/{}",
                    row["case"]
                );
            }
        }
        let raw_tiny = (name == "slopsearx_search"
            && row["arguments"]
                .as_object()
                .is_some_and(|args| args.len() == 2)
            && row["arguments"]["query"].as_str() == Some("q")
            && row["arguments"]["interactive_timeout_ms"].as_bool() == Some(true))
        .then(|| (result.clone(), row["result"].clone()));
        let (actual, expected) = comparison.pair(
            result,
            row["result"].clone(),
            name.starts_with("slopsearx_search"),
        );
        if let Some(raw) = raw_tiny {
            tiny_deadline_diagnostic(raw, (&actual, &expected))?;
        }
        if actual != expected {
            mismatches.push(format!(
                "{name}/{} args={} {}",
                row["case"],
                row["arguments"],
                first_difference(&actual, &expected, "$")
            ));
        }
    }
    let boundaries: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/t06/boundaries.json"
    ))?;
    for row in boundaries["raw_arguments"].as_array().unwrap() {
        let raw = row["raw_request"].as_str().unwrap();
        let expected_raw = row["body"]
            .as_str()
            .unwrap()
            .lines()
            .find_map(|s| s.strip_prefix("data: "))
            .unwrap();
        let expected: Value = ssx_json::from_str(expected_raw)?;
        let id = expected["id"].as_u64().unwrap();
        for escaped in [false, true] {
            let raw = if escaped {
                raw.replace("tools/call", r"tools\u002fcall")
                    .replace("slopsearx_search", r"slopsearx\u005fsearch")
            } else {
                raw.to_owned()
            };
            let before = client.dispatches(fixture);
            let actual = client.raw(raw, id).await?;
            assert_eq!(
                actual, expected["result"],
                "raw arbitrary numeric validation escaped={escaped}"
            );
            assert_eq!(client.dispatches(fixture), before);
        }
    }
    let snapshots: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/t06/snapshots.json"
    ))?;
    let mut cursor = String::new();
    for row in snapshots["calls"].as_array().unwrap() {
        let mut args = row["arguments"].clone();
        for value in args.as_object_mut().unwrap().values_mut() {
            if let Some(text) = value.as_str() {
                *value = json!(text.replace("<CURSOR>", &cursor));
            }
        }
        let before = client.dispatches(fixture);
        let result = client.call(row["name"].as_str().unwrap(), args).await?;
        if cursor.is_empty() {
            cursor = result["structuredContent"]["meta"]["cursor"]
                .as_str()
                .unwrap()
                .to_owned();
        }
        if row["dispatch_delta"]
            .as_object()
            .unwrap()
            .values()
            .all(|v| v.as_u64() == Some(0))
        {
            assert_eq!(
                client.dispatches(fixture),
                before,
                "snapshot read redispatched"
            );
        }
        let (actual, expected) = comparison.pair(
            result,
            row["result"].clone(),
            row["name"]
                .as_str()
                .unwrap()
                .starts_with("slopsearx_search"),
        );
        if actual != expected {
            mismatches.push(format!(
                "snapshot {} args={} {}",
                row["name"],
                row["arguments"],
                first_difference(&actual, &expected, "$")
            ));
        }
    }
    for (name, prompt) in source["prompt_calls"].as_object().unwrap() {
        let mut args = json!({});
        args[prompt["argument"].as_str().unwrap()] = json!("<CALLER_DATA>");
        let actual = client
            .rpc("prompts/get", json!({"name":name,"arguments":args}))
            .await?;
        assert_eq!(actual, prompt["result"], "prompt {name}");
    }
    let prompts: Value =
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/t06/prompts.json"))?;
    for row in prompts["cases"].as_array().unwrap() {
        let actual = client
            .rpc(
                "prompts/get",
                json!({"name":row["name"],"arguments":row["arguments"]}),
            )
            .await?;
        if actual != row["result"] {
            mismatches.push(format!(
                "promptnegative {} {} {}",
                row["name"],
                row["arguments"],
                first_difference(&actual, &row["result"], "$")
            ));
        }
    }
    for raw_source in [
        include_str!("../../../docs/rust-rewrite/t06/prompts_raw.json"),
        include_str!("../../../docs/rust-rewrite/t06/prompts_unknown.json"),
        include_str!("../../../docs/rust-rewrite/t06/prompts_envelope.json"),
    ] {
        let source: Value = ssx_json::from_str(raw_source)?;
        for row in source["cases"].as_array().unwrap() {
            if row["status"].as_u64() != Some(200) {
                continue;
            } // Explicit invalid-grammar tradeoff tested separately.
            let expected: Value = ssx_json::from_str(row["body"].as_str().unwrap())?;
            for escaped in [false, true] {
                let raw = row["raw_request"].as_str().unwrap();
                let raw = if escaped {
                    raw.replace("prompts/get", r"prompts\u002fget")
                        .replace(
                            "research_with_source_coverage",
                            r"research_with_source\u005fcoverage",
                        )
                        .replace(r#""unknown""#, r#""unk\u006eown""#)
                } else {
                    raw.to_owned()
                };
                let before = client.dispatches(fixture);
                let actual = client.raw(raw, expected["id"].as_u64().unwrap()).await?;
                assert_eq!(
                    actual,
                    json!({"rpc_error":expected["error"]}),
                    "raw prompt {} escaped={escaped}",
                    row["case"]
                );
                assert_eq!(
                    client.dispatches(fixture),
                    before,
                    "prompt dispatched engine"
                );
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} actual runtime mismatches\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
    Ok(())
}
#[tokio::test]
async fn full_frozen_subset_and_argument_calls_over_authenticated_http() -> Result<()> {
    let server = private_valkey::Server::start();
    let fixture = fixture::Fixture::new(server.url(), false).await;
    let cancel = CancellationToken::new();
    let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("t06-token".into()).unwrap());
    let router = http_router_for(
        fixture.surface.clone(),
        SearchSurface::catalogs(),
        cancel.clone(),
    )
    .map(|r| auth.mount(r));
    let host = Host::start(router, cancel).await?;
    let url = format!("http://{}/mcp", host.address);
    let http = reqwest::Client::builder().no_proxy().build()?;
    for token in [None, Some("wrong")] {
        let mut request = http
            .post(&url)
            .header("content-type", "application/json")
            .body(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        assert_eq!(request.send().await?.status(), 401);
        assert_eq!(fixture.calls(), 0);
    }
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, "Bearer t06-token".parse()?);
    let mut client = Client::Http {
        client: reqwest::Client::builder()
            .no_proxy()
            .default_headers(headers)
            .build()?,
        url,
        id: 0,
    };
    let result = verify(&mut client, Some(&fixture)).await;
    client.stop().await?;
    host.shutdown().await?;
    fixture.service.shutdown().await;
    result
}
#[tokio::test]
async fn full_frozen_subset_and_argument_calls_over_native_stdio_process() -> Result<()> {
    let server = private_valkey::Server::start();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_search-qualification"))
        .args(["stdio", server.url()])
        .env_clear()
        .env("PATH", "")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut stderr = child.stderr.take().unwrap();
    let stderr = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut tail = Vec::new();
        let mut chunk = [0; 8192];
        loop {
            let n = stderr.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            tail.extend_from_slice(&chunk[..n]);
            if tail.len() > 65536 {
                tail.drain(..tail.len() - 65536);
            }
        }
        Ok::<_, std::io::Error>(String::from_utf8_lossy(&tail).into_owned())
    });
    let input = child.stdin.take().unwrap();
    let output = tokio::io::BufReader::new(child.stdout.take().unwrap());
    let mut client = Client::Stdio {
        child,
        input,
        output,
        stderr,
        dispatch_receipt: Box::new(redis::Client::open(server.url())?),
        id: 0,
    };
    let result = verify(&mut client, None).await;
    client.stop().await?;
    result
}

#[tokio::test]
async fn full_frozen_subset_and_argument_calls_over_authenticated_legacy() -> Result<()> {
    use ssx_mcp::legacy::Legacy;
    use std::sync::Arc;
    let server = private_valkey::Server::start();
    let fixture = fixture::Fixture::new(server.url(), false).await;
    let cancel = CancellationToken::new();
    let surface = fixture.surface.clone();
    let legacy = Legacy::with_factory(
        Arc::new(move |wire, owner| surface.clone().bind_connection(wire, owner)),
        SearchSurface::catalogs(),
        "/messages/",
        Duration::from_secs(60),
        cancel.clone(),
    )?;
    let observed = legacy.clone();
    let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("t06-token".into())?);
    let host = Host::start(auth.mount(legacy.router("/sse")?), cancel).await?;
    let base = format!("http://{}", host.address);
    let http = reqwest::Client::builder().no_proxy().build()?;
    assert_eq!(http.get(format!("{base}/sse")).send().await?.status(), 401);
    assert_eq!(fixture.calls(), 0);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, "Bearer t06-token".parse()?);
    let http = reqwest::Client::builder()
        .no_proxy()
        .default_headers(headers)
        .build()?;
    let mut stream = http.get(format!("{base}/sse")).send().await?;
    assert_eq!(stream.status(), 200);
    let mut pending = Vec::new();
    let (event, path) = legacy_event(&mut stream, &mut pending).await?;
    assert_eq!(event, "endpoint");
    let mut client = Client::Legacy {
        client: http,
        url: format!("{base}{path}"),
        stream,
        pending,
        id: 0,
    };
    let result = verify(&mut client, Some(&fixture)).await;
    client.stop().await?;
    host.shutdown().await?;
    assert_eq!(observed.available_permits(), 128);
    assert_eq!(observed.available_message_permits(), 32);
    fixture.service.shutdown().await;
    result
}
#[tokio::test]
async fn source_specialist_success_and_failure_envelopes_use_actual_pipeline() -> Result<()> {
    let source: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/t06/boundaries.json"
    ))?;
    let server = private_valkey::Server::start();
    let engines = [
        "wikipedia",
        "brave",
        "duckduckgo",
        "greenhouse",
        "cve",
        "openalex",
    ]
    .into_iter()
    .map(|name| fixture::FakeEngine {
        name: name.into(),
        calls: Default::default(),
        dispatch_receipt: None,
        count: 3,
        status: ssx_domain::EngineStatus::Ok,
        delay: Duration::ZERO,
        results: None,
    })
    .collect();
    let fixture = fixture::Fixture::with_engines(server.url(), true, engines).await;
    let cancel = CancellationToken::new();
    let host = Host::start(
        http_router_for(
            fixture.surface.clone(),
            SearchSurface::catalogs(),
            cancel.clone(),
        ),
        cancel,
    )
    .await?;
    let mut client = Client::Http {
        client: reqwest::Client::builder().no_proxy().build()?,
        url: format!("http://{}/mcp", host.address),
        id: 0,
    };
    client.init().await?;
    let mut failures = Vec::new();
    let mut comparison = Comparison::default();
    for row in source["specialists"].as_array().unwrap() {
        let before = fixture.calls();
        let result = client
            .call(row["name"].as_str().unwrap(), row["arguments"].clone())
            .await?;
        let (actual, expected) = comparison.pair(result, row["result"].clone(), true);
        if row["dispatch_delta"]
            .as_object()
            .unwrap()
            .values()
            .all(|v| v.as_u64() == Some(0))
        {
            assert_eq!(fixture.calls(), before);
        }
        if actual != expected {
            failures.push(format!(
                "{} {} {}",
                row["name"],
                row["arguments"],
                first_difference(&actual, &expected, "$")
            ));
        }
    }
    client.stop().await?;
    host.shutdown().await?;
    fixture.service.shutdown().await;
    assert!(
        failures.is_empty(),
        "specialist source differences:\n{}",
        failures.join("\n")
    );
    Ok(())
}
async fn oauth_pair(
    store: &ssx_infra::valkey::oauth::OAuthStore,
    id: &str,
) -> Result<ssx_domain::oauth::TokenPair> {
    use ssx_domain::oauth::{ClientRegistration, CodeGrant};
    store.register_client(ClientRegistration{client_id:id.into(),client_secret:None,metadata:serde_json::json!({"redirect_uris":["http://127.0.0.1/callback"],"token_endpoint_auth_method":"none"}).as_object().unwrap().clone()}).await.unwrap();
    let grant = CodeGrant {
        client_id: id.into(),
        scopes: vec![],
        code_challenge: "a".repeat(43),
        redirect_uri: "http://127.0.0.1/callback".into(),
        redirect_uri_provided_explicitly: true,
        resource: None,
        subject: Some("operator".into()),
    };
    let code = store.create_code(grant.clone()).await.unwrap();
    Ok(store.redeem_code(&code.code, &grant).await.unwrap())
}
fn oauth_http(token: &ssx_domain::oauth::Secret) -> Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {}", token.expose()).parse()?,
    );
    Ok(reqwest::Client::builder()
        .no_proxy()
        .default_headers(headers)
        .build()?)
}
#[tokio::test]
async fn oauth_native_snapshots_and_legacy_sessions_follow_stable_client_ownership() -> Result<()> {
    use ssx_domain::oauth::{OAuthSettings, OAuthTtls};
    use ssx_infra::valkey::oauth::OAuthStore;
    use ssx_mcp::{legacy::Legacy, oauth::OAuthEndpoints};
    use std::sync::Arc;
    let server = private_valkey::Server::start();
    let fixture = fixture::Fixture::new(server.url(), false).await;
    let store = Arc::new(
        OAuthStore::new(
            Arc::new(server.store()),
            "t06-native-oauth",
            OAuthTtls::default(),
        )
        .unwrap(),
    );
    let a = oauth_pair(&store, "client-A").await?;
    let b = oauth_pair(&store, "client-B").await?;
    let cancel = CancellationToken::new();
    let surface = fixture.surface.clone();
    let legacy = Legacy::with_factory(
        Arc::new(move |wire, owner| surface.clone().bind_connection(wire, owner)),
        SearchSurface::catalogs(),
        "/messages/",
        Duration::from_secs(60),
        cancel.clone(),
    )?;
    let observed = legacy.clone();
    let legacy_router = legacy.router("/sse")?;
    let auth = NetworkAuth::OAuth(
        OAuthEndpoints::new(
            store.clone(),
            OAuthSettings::new("http://127.0.0.1", OAuthTtls::default())?,
        )
        .unwrap(),
    );
    let host = Host::start(
        http_router_for(
            fixture.surface.clone(),
            SearchSurface::catalogs(),
            cancel.clone(),
        )
        .map(|router| auth.mount(router.merge(legacy_router))),
        cancel,
    )
    .await?;
    let base = format!("http://{}", host.address);
    let client_a = oauth_http(&a.access_token)?;
    let client_b = oauth_http(&b.access_token)?;
    let mut wire_a = Client::Http {
        client: client_a.clone(),
        url: format!("{base}/mcp"),
        id: 0,
    };
    wire_a.init().await?;
    let first = wire_a
        .call(
            "slopsearx_search",
            json!({"query":"tenant snapshot","engines":["wikipedia"],"max_results":1,"include":[]}),
        )
        .await?;
    let first = &first["structuredContent"];
    assert_eq!(first["results"].as_array().unwrap().len(), 1);
    assert_eq!(first["meta"]["total"], 3);
    let cursor = first["meta"]["cursor"].as_str().unwrap().to_owned();
    let before = fixture.calls();
    let mut wire_b = Client::Http {
        client: client_b.clone(),
        url: format!("{base}/mcp"),
        id: 0,
    };
    wire_b.init().await?;
    for name in [
        "slopsearx_read_results",
        "slopsearx_read_entities",
        "slopsearx_read_result",
    ] {
        let args = if name == "slopsearx_read_result" {
            json!({"result_id":format!("{cursor}:0")})
        } else {
            json!({"cursor":cursor})
        };
        let value = wire_b.call(name, args).await?;
        assert_eq!(
            value["structuredContent"]["error"]["code"],
            "invalid_cursor"
        );
        assert_eq!(fixture.calls(), before);
    }
    // Session A is established under a verified stable client, then its access token rotates.
    let mut stream = client_a.get(format!("{base}/sse")).send().await?;
    assert_eq!(stream.status(), 200);
    let mut pending = Vec::new();
    let (_, path) = legacy_event(&mut stream, &mut pending).await?;
    let post = format!("{base}{path}");
    let init = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"tenant","version":"1"}}}"#;
    assert_eq!(
        client_a
            .post(&post)
            .header("content-type", "application/json")
            .body(init)
            .send()
            .await?
            .status(),
        202
    );
    legacy_event(&mut stream, &mut pending).await?;
    let rotated = store
        .rotate_refresh(&a.refresh_token, &a.grant, &a.grant.scopes)
        .await
        .unwrap();
    let client_a2 = oauth_http(&rotated.access_token)?;
    let mut wire_a2 = Client::Http {
        client: client_a2.clone(),
        url: format!("{base}/mcp"),
        id: 0,
    };
    wire_a2.init().await?;
    let all = wire_a2
        .call(
            "slopsearx_read_results",
            json!({"cursor":cursor,"max_results":50}),
        )
        .await?;
    assert_eq!(
        all["structuredContent"]["results"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(fixture.calls(), before);
    let request = ssx_json::to_string(
        &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"slopsearx_search","arguments":{"query":"owner guarded","engines":["wikipedia"]}}}),
    )?;
    let wrong = client_b
        .post(&post)
        .header("content-type", "application/json")
        .body(request.clone())
        .send()
        .await?;
    assert_eq!(wrong.status(), 404);
    let wrong = wrong.text().await?;
    let unknown = client_b
        .post(format!(
            "{base}/messages/?session_id=00000000-0000-0000-0000-000000000000"
        ))
        .header("content-type", "application/json")
        .body(request.clone())
        .send()
        .await?;
    assert_eq!(unknown.status(), 404);
    assert_eq!(unknown.text().await?, wrong);
    assert_eq!(fixture.calls(), before);
    assert!(
        tokio::time::timeout(
            Duration::from_millis(100),
            legacy_event(&mut stream, &mut pending)
        )
        .await
        .is_err()
    );
    assert_eq!(
        client_a
            .post(&post)
            .header("content-type", "application/json")
            .body(request.clone())
            .send()
            .await?
            .status(),
        401
    );
    assert_eq!(fixture.calls(), before);
    assert_eq!(
        client_a2
            .post(&post)
            .header("content-type", "application/json")
            .body(request)
            .send()
            .await?
            .status(),
        202
    );
    let (_, raw) = legacy_event(&mut stream, &mut pending).await?;
    let out: Value = ssx_json::from_str(&raw)?;
    assert_eq!(out["result"]["structuredContent"]["query"], "owner guarded");
    assert_eq!(fixture.calls(), before + 1);
    drop(stream);
    wire_a.stop().await?;
    wire_a2.stop().await?;
    wire_b.stop().await?;
    host.shutdown().await?;
    assert_eq!(observed.available_permits(), 128);
    assert_eq!(observed.available_message_permits(), 32);
    assert_eq!(observed.available_output_bytes(), 64 * 1024 * 1024);
    fixture.service.shutdown().await;
    Ok(())
}
#[tokio::test]
async fn native_policy_order_sensitive_authorization_precedes_runtime_availability_atomically()
-> Result<()> {
    use ssx_application::policy::SearchPolicy;
    for allowed in [false, true] {
        for disabled in [false, true] {
            let server = private_valkey::Server::start();
            let engines = ["wikipedia", "brave", "duckduckgo"]
                .into_iter()
                .map(|name| fixture::FakeEngine {
                    name: name.into(),
                    calls: Default::default(),
                    dispatch_receipt: None,
                    count: 3,
                    status: ssx_domain::EngineStatus::Ok,
                    delay: Duration::ZERO,
                    results: None,
                })
                .collect();
            let fixture = fixture::Fixture::with_policy(
                server.url(),
                SearchPolicy {
                    sensitive_allowed: allowed,
                    ..Default::default()
                },
                engines,
                if disabled { &["hibp"] } else { &[] },
            )
            .await;
            let cancel = CancellationToken::new();
            let host = Host::start(
                http_router_for(
                    fixture.surface.clone(),
                    SearchSurface::catalogs(),
                    cancel.clone(),
                ),
                cancel,
            )
            .await?;
            let mut client = Client::Http {
                client: reqwest::Client::builder().no_proxy().build()?,
                url: format!("http://{}/mcp", host.address),
                id: 0,
            };
            client.init().await?;
            for engines in [
                vec!["wikipedia", "hibp"],
                vec!["hibp"],
                vec!["unknown", "hibp"],
            ] {
                for tool in [
                    "slopsearx_search",
                    "slopsearx_search_targeted",
                    "slopsearx_explain_search_scope",
                ] {
                    let before = fixture.calls();
                    let result = client
                        .call(tool, json!({"query":"policy boundary","engines":engines}))
                        .await?;
                    let error = &result["structuredContent"]["error"];
                    let config_failure = disabled || engines.contains(&"unknown");
                    assert_eq!(
                        error["code"],
                        if !config_failure && !allowed {
                            "tool_disabled"
                        } else {
                            "invalid_scope"
                        }
                    );
                    assert_eq!(
                        fixture.calls(),
                        before,
                        "sensitive/inactive mixed scope dispatched"
                    );
                    if config_failure {
                        assert!(
                            error["valid_alternatives"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|v| v.as_str() == Some("openalex")),
                            "config validation alternatives exclude enabled inactive adapters"
                        );
                        if disabled {
                            assert!(
                                !error["valid_alternatives"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .any(|v| v.as_str() == Some("hibp"))
                            );
                        }
                    } else if allowed {
                        assert_eq!(
                            error["valid_alternatives"],
                            json!(["brave", "duckduckgo", "wikipedia"])
                        );
                    } else {
                        assert_eq!(error["grant"], "MCP_TARGETED_SENSITIVE_ALLOWED");
                    }
                }
            }
            let before = fixture.calls();
            let strict = client
                .call(
                    "slopsearx_search",
                    json!({"query":"strict","engines":["wikipedia"],"safesearch":"strict"}),
                )
                .await?;
            assert_eq!(
                strict["structuredContent"]["error"]["code"],
                "safesearch_unenforced"
            );
            assert_eq!(fixture.calls(), before);
            client.stop().await?;
            host.shutdown().await?;
            fixture.service.shutdown().await;
        }
    }
    Ok(())
}
#[tokio::test]
async fn actual_entity_payloads_survive_search_cache_snapshot_and_member_reads() -> Result<()> {
    let corpus: Value =
        ssx_json::from_str(include_str!("../../../docs/rust-rewrite/t06/entities.json"))?;
    let row = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["case"] == "candidate_and_advisory" && r["version"] == 2)
        .unwrap();
    let mut engines = Vec::new();
    for (i, r) in row["results"].as_array().unwrap().iter().enumerate() {
        let mut r = ssx_domain::storage::decode_result(r.clone())?;
        r.url = format!("https://entity{i}.example");
        r.content = "Full source snippet. ".repeat(50);
        engines.push(fixture::FakeEngine {
            name: r.engine.clone(),
            calls: Default::default(),
            dispatch_receipt: None,
            count: 1,
            status: ssx_domain::EngineStatus::Ok,
            delay: Duration::ZERO,
            results: Some(vec![r]),
        });
    }
    let server = private_valkey::Server::start();
    let fixture = fixture::Fixture::with_engines(server.url(), false, engines).await;
    let cancel = CancellationToken::new();
    let host = Host::start(
        http_router_for(
            fixture.surface.clone(),
            SearchSurface::catalogs(),
            cancel.clone(),
        ),
        cancel,
    )
    .await?;
    let mut client = Client::Http {
        client: reqwest::Client::builder().no_proxy().build()?,
        url: format!("http://{}/mcp", host.address),
        id: 0,
    };
    client.init().await?;
    let first=client.call("slopsearx_search",json!({"query":"entity pipeline","engines":["pypi","nvd"],"max_results":1,"include":["results","payload"]})).await?;
    let out = &first["structuredContent"];
    assert_eq!(out["results"].as_array().unwrap().len(), 1);
    assert_eq!(out["meta"]["total"], 2);
    let cursor = out["meta"]["cursor"].as_str().unwrap();
    let before = fixture.calls();
    assert_eq!(before, 2);
    let groups = client
        .call(
            "slopsearx_read_entities",
            json!({"cursor":cursor,"version":2}),
        )
        .await?;
    let groups = &groups["structuredContent"];
    assert_eq!(groups["entities"].as_array().unwrap().len(), 4);
    assert_eq!(groups["relationships"].as_array().unwrap().len(), 2);
    assert_eq!(groups["meta"]["total_results"], 2);
    let mut members = std::collections::BTreeSet::new();
    for entity in groups["entities"].as_array().unwrap() {
        assert_eq!(entity["artifact"]["kind"], "entity_group");
        for result_id in entity["result_ids"].as_array().unwrap() {
            members.insert(result_id.as_str().unwrap().to_owned());
        }
    }
    assert_eq!(members.len(), 2);
    for result_id in members {
        let record = client
            .call("slopsearx_read_result", json!({"result_id":result_id}))
            .await?;
        let record = &record["structuredContent"];
        assert_eq!(record["result_id"].as_str(), Some(result_id.as_str()));
        assert!(record["content"].as_str().unwrap().len() > 512);
        assert!(record["payload"].is_object());
        assert_eq!(record["retrieval"].get("verified"), Some(&json!(false)));
    }
    let cached = client
        .call(
            "slopsearx_search",
            json!({"query":"entity pipeline","engines":["pypi","nvd"],"max_results":50}),
        )
        .await?;
    assert_eq!(cached["structuredContent"]["meta"]["cached"], true);
    assert_eq!(
        cached["structuredContent"]["results"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let again = client
        .call(
            "slopsearx_read_entities",
            json!({"cursor":cursor,"version":2}),
        )
        .await?;
    assert_eq!(again["structuredContent"], *groups);
    assert_eq!(fixture.calls(), before, "read/cache path dispatched");
    client.stop().await?;
    host.shutdown().await?;
    fixture.service.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn outer_scalar_array_prompt_params_are_rejected_before_dispatch_with_explicit_framework_difference()
-> Result<()> {
    let differences: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/t06/framework-differences.json"
    ))?;
    let source: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/t06/prompts_envelope.json"
    ))?;
    let server = private_valkey::Server::start();
    let fixture = fixture::Fixture::new(server.url(), false).await;
    let cancel = CancellationToken::new();
    let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("grammar-token".into())?);
    let host = Host::start(
        http_router_for(
            fixture.surface.clone(),
            SearchSurface::catalogs(),
            cancel.clone(),
        )
        .map(|router| auth.mount(router)),
        cancel,
    )
    .await?;
    let http = reqwest::Client::builder().no_proxy().build()?;
    for difference in differences["cases"].as_array().unwrap() {
        let row = source["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["case"] == difference["case"])
            .unwrap();
        assert_eq!(row["status"], difference["source_status"]);
        assert_eq!(row["body"], difference["source_body"]);
        for token in ["wrong", "grammar-token"] {
            let response = http
                .post(format!("http://{}/mcp", host.address))
                .bearer_auth(token)
                .header("accept", "application/json,text/event-stream")
                .header("content-type", "application/json")
                .header("mcp-protocol-version", "2025-11-25")
                .body(row["raw_request"].as_str().unwrap().to_owned())
                .send()
                .await?;
            if token == "wrong" {
                assert_eq!(response.status(), 401);
            } else {
                assert_eq!(
                    u64::from(response.status().as_u16()),
                    difference["native_status"].as_u64().unwrap()
                );
                assert_eq!(
                    response.text().await?,
                    difference["native_body"].as_str().unwrap()
                );
            }
            assert_eq!(
                fixture.calls(),
                0,
                "invalid JSONRPC grammar reached engine dispatch"
            );
        }
    }
    host.shutdown().await?;
    fixture.service.shutdown().await;
    Ok(())
}
