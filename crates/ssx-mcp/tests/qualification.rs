//! Real subprocess/HTTP qualification; synthetic application state, no provider or store.
use rmcp::{ServiceExt, model::*, transport::TokioChildProcess};
use serde_json::{Value, json};
use ssx_mcp::{
    Registry, Result, compare_descriptor,
    gateway::{Gateway, collect_pages},
    harness::{Host, http_router},
    legacy::Legacy,
};
use std::{sync::atomic::Ordering, time::Duration};
use tokio_util::sync::CancellationToken;
fn baseline() -> Value {
    serde_json::from_str(include_str!("../fixtures/python-sse.json")).unwrap()
}
fn config() -> ClientConfig {
    ClientConfig::default().with_protocol_version(ProtocolVersion::V_2025_11_25)
}
async fn verify<S: rmcp::Service<rmcp::RoleClient>>(
    client: &rmcp::service::RunningService<rmcp::RoleClient, S>,
) -> Result<()> {
    let fixture = baseline();
    let expected = &fixture["observations"];
    compare_descriptor(
        &expected["tools"],
        &serde_json::to_value(client.list_tools(None).await?.tools)?,
    )?;
    compare_descriptor(
        &expected["resources"],
        &serde_json::to_value(client.list_resources(None).await?.resources)?,
    )?;
    compare_descriptor(
        &expected["prompts"],
        &serde_json::to_value(client.list_prompts(None).await?.prompts)?,
    )?;
    let result =
        client
            .call_tool(CallToolRequestParams::new("t01_echo").with_arguments(
                serde_json::from_value(json!({"payload":{"items":[1,2]},"note":null}))?,
            ))
            .await?;
    compare_descriptor(&expected["echo_result"], &serde_json::to_value(result)?)?;
    let resource = client
        .read_resource(ReadResourceRequestParams::new("t01://text"))
        .await?;
    compare_descriptor(
        &expected["resource_result"],
        &serde_json::to_value(resource)?,
    )?;
    let prompt = client
        .get_prompt(
            GetPromptRequestParams::new("t01_prompt")
                .with_arguments(serde_json::from_value(json!({"topic":"test"}))?),
        )
        .await?;
    compare_descriptor(&expected["prompt_result"], &serde_json::to_value(prompt)?)?;
    assert!(
        client
            .call_tool(CallToolRequestParams::new("not-real"))
            .await
            .is_err()
    );
    assert!(
        client
            .call_tool(CallToolRequestParams::new("t01_echo").with_arguments(
                serde_json::from_value(json!({"payload":{"items":["wrong"]}}))?
            ))
            .await
            .is_err()
    );
    let error =
        client
            .call_tool(CallToolRequestParams::new("t01_echo").with_arguments(
                serde_json::from_value(json!({"payload":{"items":[]},"note":"error"}))?,
            ))
            .await?;
    assert_eq!(error.is_error, Some(true));
    Ok(())
}
async fn process(
    args: &[&str],
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ClientConfig>> {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_mcp-qualification"));
    command
        .args(args)
        .env_clear()
        .env("PATH", "")
        .kill_on_drop(true);
    let transport = TokioChildProcess::new(command)?;
    Ok(tokio::time::timeout(Duration::from_secs(5), config().serve(transport)).await??)
}
#[tokio::test]
async fn real_stdio_subprocess_preserves_complete_contract_and_clean_shutdown() -> Result<()> {
    let client = process(&["stdio"]).await?;
    verify(&client).await?;
    tokio::time::timeout(Duration::from_secs(3), client.cancel()).await??;
    Ok(())
}
#[tokio::test]
async fn real_streamable_http_bearer_and_stateless_contract() -> Result<()> {
    let ct = CancellationToken::new();
    let registry = Registry::synthetic();
    let host = Host::start(
        http_router(registry.clone(), Some("fixture-token".into()), ct.clone()),
        ct,
    )
    .await?;
    let url = format!("http://{}/mcp", host.address);
    let http = reqwest::Client::builder().no_proxy().build()?;
    for token in [None, Some("wrong")] {
        let mut request = http
            .post(&url)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = request.send().await?;
        assert_eq!(response.status(), 401);
        assert_eq!(
            response.headers()["content-type"],
            "text/plain; charset=utf-8"
        );
        assert_eq!(response.text().await?, "unauthorized");
    }
    let gateway = Gateway::connect(
        url.clone(),
        Some("fixture-token".into()),
        Duration::from_secs(3),
    )
    .await?;
    verify(&gateway.service).await?;
    assert_eq!(registry.calls.load(Ordering::SeqCst), 2);
    let response = http
        .post(&url)
        .bearer_auth("fixture-token")
        .header("accept", "application/json, text/event-stream")
        .json(&json!({"jsonrpc":"2.0","id":9,"method":"tools/list"}))
        .send()
        .await?;
    assert!(response.headers().get("mcp-session-id").is_none());
    assert_eq!(response.status(), 200);
    gateway.shutdown().await?;
    let address = host.address;
    host.shutdown().await?;
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
    Ok(())
}
#[tokio::test]
async fn real_stdio_gateway_forwards_to_remote_and_owns_session() -> Result<()> {
    let ct = CancellationToken::new();
    let registry = Registry::synthetic();
    let host = Host::start(http_router(registry.clone(), None, ct.clone()), ct).await?;
    let url = format!("http://{}/mcp", host.address);
    let client = process(&["gateway", &url]).await?;
    verify(&client).await?;
    assert_eq!(registry.calls.load(Ordering::SeqCst), 2);
    tokio::time::timeout(Duration::from_secs(3), client.cancel()).await??;
    host.shutdown().await?;
    Ok(())
}
#[test]
fn descriptor_comparison_rejects_nested_schema_defaults_nullable_and_metadata_mutation() {
    let expected = baseline()["observations"]["tools"].clone();
    for (pointer, replacement) in [
        (
            "/0/inputSchema/properties/payload/properties/items/items/type",
            json!("string"),
        ),
        ("/0/inputSchema/properties/note/default", json!("new")),
        (
            "/0/inputSchema/properties/note/anyOf/1/type",
            json!("integer"),
        ),
        ("/0/_meta/fastmcp/tags", json!(["changed"])),
        ("/0/description", json!("changed")),
        ("/0/outputSchema/type", json!("array")),
    ] {
        let mut changed = expected.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        assert!(compare_descriptor(&expected, &changed).is_err());
    }
    let mut changed = expected.clone();
    changed[0].as_object_mut().unwrap().remove("_meta");
    assert!(compare_descriptor(&expected, &changed).is_err());
}
#[tokio::test]
async fn discovery_limits_are_fail_closed_and_empty_cursor_is_valid() -> Result<()> {
    let mut seen = Vec::new();
    let result = collect_pages(|cursor| {
        seen.push(cursor.clone());
        std::future::ready(Ok(if cursor.is_none() {
            (json!([{"name":"first"}]), Some("".into()))
        } else {
            (json!([{"name":"second"}]), None)
        }))
    })
    .await?;
    assert_eq!(result.len(), 2);
    assert_eq!(seen, vec![None, Some("".into())]);
    assert!(
        collect_pages(|_| std::future::ready(Ok((
            json!([{"name":"duplicate"}]),
            Some("next".into())
        ))))
        .await
        .is_err()
    );
    assert!(
        collect_pages(|_| std::future::ready(Ok((json!([]), Some("same".into())))))
            .await
            .is_err()
    );
    let mut pages = 0;
    assert!(
        collect_pages(|_| {
            pages += 1;
            std::future::ready(Ok((json!([]), Some(pages.to_string()))))
        })
        .await
        .is_err()
    );
    assert_eq!(pages, 128);
    let large: Vec<_> = (0..4097).map(|n| json!({"name":n.to_string()})).collect();
    assert!(
        collect_pages(|_| std::future::ready(Ok((json!(large), None))))
            .await
            .is_err()
    );
    assert!(
        collect_pages(|_| std::future::ready(Ok((json!([{}]), None))))
            .await
            .is_err()
    );
    Ok(())
}
struct SseClient {
    http: reqwest::Client,
    base: String,
    endpoint: String,
    response: reqwest::Response,
    buffer: String,
}
impl SseClient {
    async fn connect(base: String, path: &str) -> Result<Self> {
        let http = reqwest::Client::builder().no_proxy().build()?;
        let response = http.get(format!("{base}{path}")).send().await?;
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers()["content-type"],
            baseline()["observations"]["content_type"].as_str().unwrap()
        );
        let mut client = Self {
            http,
            base,
            endpoint: String::new(),
            response,
            buffer: String::new(),
        };
        let (kind, data) = client.event().await?;
        assert_eq!(kind, "endpoint");
        client.endpoint = data;
        Ok(client)
    }
    async fn event(&mut self) -> Result<(String, String)> {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                self.buffer = self.buffer.replace("\r\n", "\n");
                if let Some((raw, rest)) = self.buffer.split_once("\n\n") {
                    let raw = raw.to_owned();
                    self.buffer = rest.to_owned();
                    let mut kind = String::new();
                    let mut data = String::new();
                    for line in raw.lines() {
                        if let Some(value) = line.strip_prefix("event:") {
                            kind = value.trim().into();
                        }
                        if let Some(value) = line.strip_prefix("data:") {
                            if !data.is_empty() {
                                data.push('\n');
                            }
                            data.push_str(value.strip_prefix(' ').unwrap_or(value));
                        }
                    }
                    if !data.is_empty() {
                        return Ok((kind, data));
                    }
                } else {
                    let chunk = self.response.chunk().await?.ok_or("SSE disconnected")?;
                    self.buffer
                        .push_str(&String::from_utf8(chunk.to_vec())?.replace("\r\n", "\n"));
                    if self.buffer.len() > 1024 * 1024 {
                        return Err("SSE event too large".into());
                    }
                }
            }
        })
        .await?
    }
    async fn post_raw(&self, body: &str) -> Result<Value> {
        let r = self
            .http
            .post(format!("{}{}", self.base, self.endpoint))
            .header("content-type", "application/json")
            .body(body.to_owned())
            .send()
            .await?;
        Ok(json!({"status":r.status().as_u16(),"body":r.text().await?}))
    }
    async fn post(&self, body: Value) -> Result<Value> {
        let r = self
            .http
            .post(format!("{}{}", self.base, self.endpoint))
            .json(&body)
            .send()
            .await?;
        let status = r.status().as_u16();
        Ok(json!({"status":status,"body":r.text().await?}))
    }
    async fn request(&mut self, id: u32, method: &str, params: Value) -> Result<Value> {
        assert_eq!(
            self.post(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
                .await?,
            baseline()["observations"]["accepted"]
        );
        let (_, body) = self.event().await?;
        let value: Value = serde_json::from_str(&body)?;
        assert_eq!(value["id"], id);
        Ok(value)
    }
}
#[tokio::test]
async fn real_legacy_sse_matches_observed_python_status_and_payloads() -> Result<()> {
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/messages/",
        Duration::from_secs(30),
        ct.clone(),
    )?;
    let host = Host::start(legacy.clone().router("/sse")?, ct).await?;
    let base = format!("http://{}", host.address);
    let mut client = SseClient::connect(base.clone(), "/sse").await?;
    let observation = baseline()["observations"].clone();
    assert_eq!(
        client.endpoint.split('?').next().unwrap(),
        observation["message_prefix"]
    );
    for (key, path, body) in [
        ("missing_session", "/messages/".to_string(), "{}"),
        (
            "invalid_session",
            "/messages/?session_id=invalid".into(),
            "{}",
        ),
        (
            "unknown_session",
            "/messages/?session_id=00000000000000000000000000000000".into(),
            "{}",
        ),
        ("invalid_json", client.endpoint.clone(), "{"),
        ("invalid_rpc", client.endpoint.clone(), "{}"),
        ("oversized", client.endpoint.clone(), "oversize"),
    ] {
        let body = if key == "oversized" {
            "x".repeat(4 * 1024 * 1024 + 1)
        } else {
            body.to_string()
        };
        let r = client
            .http
            .post(format!("{base}{path}"))
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await?;
        let status = r.status().as_u16();
        assert_eq!(
            json!({"status":status,"body":r.text().await?}),
            observation[key],
            "{key}"
        );
    }
    let result=client.request(1,"initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}})).await?;
    assert_eq!(result["result"]["protocolVersion"], "2025-11-25");
    client
        .post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await?;
    for (id, method, params, key, inner) in [
        (2, "tools/list", json!({}), "tools", Some("tools")),
        (
            3,
            "resources/list",
            json!({}),
            "resources",
            Some("resources"),
        ),
        (4, "prompts/list", json!({}), "prompts", Some("prompts")),
        (
            5,
            "tools/call",
            json!({"name":"t01_echo","arguments":{"payload":{"items":[1,2]},"note":null}}),
            "echo_result",
            None,
        ),
        (
            6,
            "resources/read",
            json!({"uri":"t01://text"}),
            "resource_result",
            None,
        ),
        (
            7,
            "prompts/get",
            json!({"name":"t01_prompt","arguments":{"topic":"test"}}),
            "prompt_result",
            None,
        ),
    ] {
        let actual = client.request(id, method, params).await?;
        let actual = if let Some(inner) = inner {
            &actual["result"][inner]
        } else {
            &actual["result"]
        };
        compare_descriptor(&observation[key], actual)?;
    }
    let head = client.http.head(format!("{base}/sse")).send().await?;
    assert_eq!(head.status(), 200);
    assert!(head.text().await?.is_empty());
    drop(client);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(legacy.active_sessions(), 0);
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn legacy_custom_paths_suffixes_method_content_type_and_expiry() -> Result<()> {
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/prefix/inbox/",
        Duration::from_millis(200),
        ct.clone(),
    )?;
    let host = Host::start(legacy.clone().router("/prefix/events")?, ct).await?;
    let base = format!("http://{}", host.address);
    let client = SseClient::connect(base.clone(), "/prefix/events").await?;
    assert!(client.endpoint.starts_with("/prefix/inbox/?session_id="));
    let observation = baseline()["observations"].clone();
    for (method, content_type, key) in [
        (reqwest::Method::GET, "application/json", "wrong_method"),
        (reqwest::Method::POST, "text/plain", "wrong_content_type"),
    ] {
        let r = client
            .http
            .request(method, format!("{base}{}", client.endpoint))
            .header("content-type", content_type)
            .body("{}")
            .send()
            .await?;
        let status = r.status().as_u16();
        assert_eq!(
            json!({"status":status,"body":r.text().await?}),
            observation[key]
        );
    }
    let suffix = client
        .endpoint
        .replacen("/prefix/inbox/", "/prefix/inbox/arbitrary/suffix", 1);
    let r = client
        .http
        .post(format!("{base}{suffix}"))
        .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .send()
        .await?;
    assert_eq!(r.status(), 202);
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(legacy.active_sessions(), 0);
    let r = client
        .http
        .post(format!("{base}{}", client.endpoint))
        .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .send()
        .await?;
    assert_eq!(r.status(), 404);
    drop(client);
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn real_http_discovery_limits_and_startup_deadlines_leave_no_ready_gateway() -> Result<()> {
    for mode in [
        ssx_mcp::Discovery::RepeatedCursor,
        ssx_mcp::Discovery::Duplicate,
        ssx_mcp::Discovery::TooManyPages,
        ssx_mcp::Discovery::TooManyItems,
        ssx_mcp::Discovery::Delayed,
    ] {
        let mut registry = Registry::synthetic();
        registry.discovery = mode;
        let ct = CancellationToken::new();
        let host = Host::start(http_router(registry, None, ct.clone()), ct).await?;
        let start = std::time::Instant::now();
        let deadline = if matches!(mode, ssx_mcp::Discovery::Delayed) {
            Duration::from_millis(50)
        } else {
            Duration::from_secs(4)
        };
        assert!(
            Gateway::connect(format!("http://{}/mcp", host.address), None, deadline)
                .await
                .is_err()
        );
        assert!(start.elapsed() < Duration::from_secs(5));
        host.shutdown().await?;
    }
    let mut registry = Registry::synthetic();
    registry.initialize_delay = Duration::from_secs(1);
    let ct = CancellationToken::new();
    let host = Host::start(http_router(registry, None, ct.clone()), ct).await?;
    assert!(
        Gateway::connect(
            format!("http://{}/mcp", host.address),
            None,
            Duration::from_millis(30)
        )
        .await
        .is_err()
    );
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn real_http_request_disconnect_cancels_handler_and_releases_state() -> Result<()> {
    let registry = Registry::synthetic();
    let ct = CancellationToken::new();
    let host = Host::start(http_router(registry.clone(), None, ct.clone()), ct).await?;
    let url = format!("http://{}/mcp", host.address);
    let pending = tokio::spawn(async move {
        reqwest::Client::builder().no_proxy().pool_max_idle_per_host(0).build().unwrap().post(url).header("accept","application/json, text/event-stream").header("mcp-protocol-version","2025-11-25").json(&json!({"jsonrpc":"2.0","id":99,"method":"tools/call","params":{"name":"t01_echo","arguments":{"payload":{"items":[]},"note":"wait"}}})).send().await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while registry.in_flight.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    pending.abort();
    let _ = pending.await;
    tokio::time::timeout(Duration::from_secs(2), async {
        while registry.in_flight.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn legacy_unsupported_version_negotiates_and_jsonrpc_errors_remain_errors() -> Result<()> {
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/messages/",
        Duration::from_secs(10),
        ct.clone(),
    )?;
    let host = Host::start(legacy.clone().router("/sse")?, ct).await?;
    let mut client = SseClient::connect(format!("http://{}", host.address), "/sse").await?;
    let response=client.request(1,"initialize",json!({"protocolVersion":"1900-01-01","capabilities":{},"clientInfo":{"name":"test","version":"1"}})).await?;
    assert_eq!(response["result"]["protocolVersion"], "2025-11-25");
    client
        .post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await?;
    let error = client.request(2, "not-a-method", json!({})).await?;
    assert!(error.get("error").is_some());
    assert!(error.get("result").is_none());
    drop(client);
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn complete_captured_public_catalog_roundtrips_through_pinned_sdk_and_http() -> Result<()> {
    let observed = baseline()["observations"].clone();
    assert_eq!(observed["public_tools"].as_array().unwrap().len(), 38);
    let mut registry = Registry::synthetic();
    registry.descriptors = std::sync::Arc::new(
        json!({"tools":observed["public_tools"],"resources":observed["public_resources"],"prompts":observed["public_prompts"]}),
    );
    let ct = CancellationToken::new();
    let host = Host::start(http_router(registry, None, ct.clone()), ct).await?;
    let gateway = Gateway::connect(
        format!("http://{}/mcp", host.address),
        None,
        Duration::from_secs(3),
    )
    .await?;
    for key in ["tools", "resources", "prompts"] {
        compare_descriptor(&observed[format!("public_{key}")], &gateway.catalog[key])?;
    }
    gateway.shutdown().await?;
    host.shutdown().await?;
    Ok(())
}
#[test]
fn unknown_schema_metadata_objects_and_refs_are_preserved_without_numeric_reinterpretation()
-> Result<()> {
    let mut descriptor = baseline()["observations"]["tools"][0].clone();
    descriptor["inputSchema"]["$defs"] = json!({"Value":{"type":"object","properties":{"$serde_json::private::Number":{"type":"string"}}}});
    descriptor["inputSchema"]["properties"]["vendor"] =
        json!({"$ref":"#/$defs/Value","x-vendor":{"$serde_json::private::Number":"123"}});
    descriptor["_meta"]["vendor"] =
        json!({"$serde_json::private::Number":"123","null":null,"empty":{}});
    let typed: Tool = serde_json::from_value(descriptor.clone())?;
    compare_descriptor(&descriptor, &serde_json::to_value(typed)?)?;
    Ok(())
}
#[tokio::test]
async fn legacy_request_cancellation_releases_inflight_work_and_sessions() -> Result<()> {
    let registry = Registry::synthetic();
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        registry.clone(),
        "/messages/",
        Duration::from_secs(10),
        ct.clone(),
    )?;
    let host = Host::start(legacy.clone().router("/sse")?, ct).await?;
    let mut client = SseClient::connect(format!("http://{}", host.address), "/sse").await?;
    client.request(1,"initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}})).await?;
    client
        .post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await?;
    assert_eq!(client.post_raw(r#"{"jsonrpc":"2.0","id":18446744073709551617,"method":"tools/call","params":{"name":"t01_echo","arguments":{"payload":{"items":[]},"note":"wait"}}}"#).await?,baseline()["observations"]["accepted"]);
    tokio::time::timeout(Duration::from_secs(2), async {
        while registry.in_flight.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    assert_eq!(
        legacy.available_message_permits(),
        31,
        "active handler retains global ingress ownership"
    );
    assert_eq!(client.post_raw(r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":18446744073709551617,"reason":"test"}}"#).await?,baseline()["observations"]["accepted"]);
    tokio::time::timeout(Duration::from_secs(2), async {
        while registry.in_flight.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    drop(client);
    host.shutdown().await?;
    assert_eq!(legacy.active_sessions(), 0);
    assert_eq!(legacy.available_message_permits(), 32);
    assert_eq!(legacy.available_output_bytes(), 64 * 1024 * 1024);
    Ok(())
}
fn raw_large_tools() -> Box<serde_json::value::RawValue> {
    let root: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_str(include_str!("../fixtures/python-sse.json")).unwrap();
    let mut observations: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_str(root["observations"].get()).unwrap();
    observations.remove("large_tools").unwrap()
}
fn raw_tools(response: &str) -> Result<String> {
    let root: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_str(response)?;
    let result: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_str(root["result"].get())?;
    let tools: Vec<Box<serde_json::value::RawValue>> = serde_json::from_str(result["tools"].get())?;
    Ok(serde_json::to_string(&tools)?)
}
fn assert_large_catalog(response: &str) -> Result<()> {
    let expected: Vec<Box<serde_json::value::RawValue>> =
        serde_json::from_str(raw_large_tools().get())?;
    let actual = raw_tools(response)?;
    assert_eq!(
        ssx_mcp::opaque::compact(&actual)?,
        ssx_mcp::opaque::compact(&serde_json::to_string(&expected)?)?
    );
    assert!(actual.contains("18446744073709551617"));
    assert!(actual.contains("$serde_json::private::Number"));
    Ok(())
}
#[test]
fn typed_sdk_large_number_limitation_is_characterized_and_not_mistaken_for_parity() -> Result<()> {
    let raw = raw_large_tools();
    let typed: Vec<Tool> = serde_json::from_str(raw.get())?;
    let rounded = serde_json::to_string(&typed)?;
    assert!(
        !rounded.contains("18446744073709551617"),
        "SDK changed; requalify the opaque bridge"
    );
    assert!(rounded.contains("$serde_json::private::Number"));
    assert!(raw.get().contains("18446744073709551617"));
    Ok(())
}
async fn raw_process_catalog(args: &[&str]) -> Result<String> {
    raw_process_request(
        args,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
    )
    .await
}
async fn raw_process_request(args: &[&str], request: &str) -> Result<String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_mcp-qualification"))
        .args(args)
        .env_clear()
        .env("PATH", "")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;
    let mut input = child.stdin.take().ok_or("child stdin")?;
    let mut output = BufReader::new(child.stdout.take().ok_or("child stdout")?).lines();
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"raw-client\",\"version\":\"1\"}}}\n").await?;
    let init = tokio::time::timeout(Duration::from_secs(3), output.next_line())
        .await??
        .ok_or("initialize missing")?;
    assert!(init.contains("result"));
    input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
        .await?;
    input.write_all(request.as_bytes()).await?;
    input.write_all(b"\n").await?;
    let response = tokio::time::timeout(Duration::from_secs(3), output.next_line())
        .await??
        .ok_or("catalog missing")?;
    input.shutdown().await?;
    drop(input);
    assert!(
        tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await??
            .success()
    );
    Ok(response)
}
#[tokio::test]
async fn opaque_large_integer_and_reserved_object_survive_real_http_stdio_sse_and_gateway()
-> Result<()> {
    let registry = Registry::large_catalog();
    let ct = CancellationToken::new();
    let host = Host::start(http_router(registry.clone(), None, ct.clone()), ct).await?;
    let url = format!("http://{}/mcp", host.address);
    let response = reqwest::Client::builder()
        .no_proxy()
        .build()?
        .post(&url)
        .header("accept", "application/json, text/event-stream")
        .json(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
        .send()
        .await?
        .text()
        .await?;
    assert_large_catalog(&response)?;
    assert_large_catalog(&raw_process_catalog(&["stdio-opaque"]).await?)?;
    assert_large_catalog(&raw_process_catalog(&["gateway", &url]).await?)?;
    host.shutdown().await?;
    let ct = CancellationToken::new();
    let legacy = Legacy::new(registry, "/messages/", Duration::from_secs(10), ct.clone())?;
    let host = Host::start(legacy.clone().router("/sse")?, ct).await?;
    let mut client = SseClient::connect(format!("http://{}", host.address), "/sse").await?;
    client.request(1,"initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"raw-client","version":"1"}})).await?;
    client
        .post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await?;
    client
        .post(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await?;
    let (_, response) = client.event().await?;
    assert_large_catalog(&response)?;
    drop(client);
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn selected_raw_gateway_owns_stateful_session_and_closes_it() -> Result<()> {
    let ct = CancellationToken::new();
    let deletes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = deletes.clone();
    let router =
        ssx_mcp::harness::http_router_mode(Registry::large_catalog(), None, ct.clone(), true).map(
            |router| {
                router.layer(axum::middleware::from_fn(
                    move |request: axum::http::Request<axum::body::Body>,
                          next: axum::middleware::Next| {
                        let observed = observed.clone();
                        async move {
                            if request.method() == axum::http::Method::DELETE
                                && request.headers().contains_key("mcp-session-id")
                            {
                                observed.fetch_add(1, Ordering::SeqCst);
                            }
                            next.run(request).await
                        }
                    },
                ))
            },
        );
    let host = Host::start(router, ct).await?;
    let url = format!("http://{}/mcp", host.address);
    assert_large_catalog(&raw_process_catalog(&["gateway", &url]).await?)?;
    assert_eq!(deletes.load(Ordering::SeqCst), 1);
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn selected_raw_gateway_request_cancellation_reaches_remote_handler() -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let registry = Registry::synthetic();
    let ct = CancellationToken::new();
    let host = Host::start(
        ssx_mcp::harness::http_router_mode(registry.clone(), None, ct.clone(), true),
        ct,
    )
    .await?;
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_mcp-qualification"))
        .args(["gateway", &format!("http://{}/mcp", host.address)])
        .env_clear()
        .env("PATH", "")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"client\",\"version\":\"1\"}}}\n").await?;
    tokio::time::timeout(Duration::from_secs(3), output.next_line())
        .await??
        .ok_or("initialize missing")?;
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":18446744073709551617,\"method\":\"tools/call\",\"params\":{\"name\":\"t01_echo\",\"arguments\":{\"payload\":{\"items\":[]},\"note\":\"wait\"}}}\n").await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        while registry.in_flight.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/cancelled\",\"params\":{\"requestId\":18446744073709551617}}\n").await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        while registry.in_flight.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    input.shutdown().await?;
    drop(input);
    assert!(
        tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await??
            .success()
    );
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn selected_raw_gateway_accepts_incremental_sse_and_rejects_bad_remote_version() -> Result<()>
{
    use axum::{response::IntoResponse, routing::post};
    for bad in [false, true] {
        let router=axum::Router::new().route("/mcp",post(move |body:String|async move {
            let request:Value=serde_json::from_str(&body).unwrap();let method=request["method"].as_str().unwrap_or("");
            if request.get("id").is_none(){return axum::http::StatusCode::ACCEPTED.into_response();}
            let result=match method {"initialize"=>json!({"protocolVersion":if bad{"2099-01-01"}else{"2025-11-25"},"capabilities":{"tools":{},"resources":{},"prompts":{}},"serverInfo":{"name":"fixture","version":"1"}}),"tools/list"=>json!({"tools":[]}),"resources/list"=>json!({"resources":[]}),"prompts/list"=>json!({"prompts":[]}),_=>json!({"resourceTemplates":[]})};
            let response=json!({"jsonrpc":"2.0","id":request["id"],"result":result}).to_string();
            let frames=vec![Ok::<_,std::convert::Infallible>(axum::response::sse::Event::default().event("message").data(r#"{"jsonrpc":"2.0","method":"notifications/progress","params":{}}"#)),Ok(axum::response::sse::Event::default().event("message").data(response))];
            axum::response::Sse::new(futures_util::stream::iter(frames)).into_response()
        }));
        let ct = CancellationToken::new();
        let host = Host::start(router, ct).await?;
        let result = ssx_mcp::opaque::RawGateway::connect(
            format!("http://{}/mcp", host.address),
            None,
            Duration::from_secs(2),
        )
        .await;
        if bad {
            assert!(result.is_err());
        } else {
            let gateway = result?;
            assert_eq!(gateway.catalogs.len(), 4);
            gateway.close().await?;
        }
        host.shutdown().await?;
    }
    Ok(())
}
#[tokio::test]
async fn selected_raw_gateway_real_discovery_limits_and_deadline_fail_closed() -> Result<()> {
    for mode in [
        ssx_mcp::Discovery::RepeatedCursor,
        ssx_mcp::Discovery::Duplicate,
        ssx_mcp::Discovery::TooManyPages,
        ssx_mcp::Discovery::TooManyItems,
        ssx_mcp::Discovery::Delayed,
    ] {
        let mut registry = Registry::synthetic();
        registry.discovery = mode;
        let ct = CancellationToken::new();
        let host = Host::start(http_router(registry, None, ct.clone()), ct).await?;
        let deadline = if matches!(mode, ssx_mcp::Discovery::Delayed) {
            Duration::from_millis(50)
        } else {
            Duration::from_secs(4)
        };
        assert!(
            ssx_mcp::opaque::RawGateway::connect(
                format!("http://{}/mcp", host.address),
                None,
                deadline
            )
            .await
            .is_err()
        );
        host.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn selected_stdio_source_lifecycle_and_adjacent_big_ids() -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let ct = CancellationToken::new();
    let host = Host::start(http_router(Registry::large_catalog(), None, ct.clone()), ct).await?;
    let url = format!("http://{}/mcp", host.address);
    for args in [vec!["stdio-opaque"], vec!["gateway", url.as_str()]] {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_mcp-qualification"))
            .args(args)
            .env_clear()
            .kill_on_drop(true)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        let mut input = child.stdin.take().unwrap();
        let mut output = ssx_mcp::opaque::Frames::new(child.stdout.take().unwrap());
        let cases = [
            r#"{"jsonrpc":"2.0","id":100,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":101,"method":"initialize","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":102,"method":"initialize","params":{"protocolVersion":"unsupported","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
            r#"{"jsonrpc":"2.0","id":18446744073709551616,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":18446744073709551617,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":104,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
        ];
        for (index, request) in cases.iter().enumerate() {
            input.write_all(request.as_bytes()).await?;
            input.write_all(b"\n").await?;
            let raw = tokio::time::timeout(Duration::from_secs(3), output.next())
                .await??
                .ok_or("stdio ended")?;
            let response: Value = serde_json::from_str(&raw)?;
            match index {
                0 | 1 => assert_eq!(
                    response,
                    baseline()["observations"]["lifecycle"][if index == 0 {
                        "before_initialize"
                    } else {
                        "malformed_initialize"
                    }]
                ),
                2 => assert_eq!(response["result"]["protocolVersion"], "2025-11-25"),
                3 | 4 => {
                    let fields: std::collections::BTreeMap<
                        String,
                        Box<serde_json::value::RawValue>,
                    > = serde_json::from_str(&raw)?;
                    assert_eq!(
                        fields["id"].get(),
                        if index == 3 {
                            "18446744073709551616"
                        } else {
                            "18446744073709551617"
                        }
                    );
                    assert!(raw.contains("18446744073709551617"));
                    assert!(response.get("error").is_none());
                }
                5 => assert_eq!(response["result"]["protocolVersion"], "2024-11-05"),
                _ => unreachable!(),
            }
        }
        drop(input);
        tokio::time::timeout(Duration::from_secs(3), child.wait()).await??;
    }
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn legacy_source_lifecycle_matches_captured_phase_errors() -> Result<()> {
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/messages/",
        Duration::from_secs(10),
        ct.clone(),
    )?;
    let host = Host::start(legacy.router("/sse")?, ct).await?;
    let mut client = SseClient::connect(format!("http://{}", host.address), "/sse").await?;
    for (id, method, params, expected) in [
        (100, "tools/list", json!({}), "before_initialize"),
        (101, "initialize", json!({}), "malformed_initialize"),
    ] {
        assert_eq!(
            client.request(id, method, params).await?,
            baseline()["observations"]["lifecycle"][expected]
        );
    }
    let init = client.request(102, "initialize", json!({"protocolVersion":"unsupported","capabilities":{},"clientInfo":{"name":"test","version":"1"}})).await?;
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    assert!(client.request(103, "tools/list", json!({})).await?["result"]["tools"].is_array());
    let init = client.request(104, "initialize", json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}})).await?;
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    drop(client);
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn real_http_and_legacy_preserve_adjacent_large_request_ids() -> Result<()> {
    let ct = CancellationToken::new();
    let host = Host::start(http_router(Registry::large_catalog(), None, ct.clone()), ct).await?;
    let http = reqwest::Client::new();
    for id in ["18446744073709551616", "18446744073709551617"] {
        for (method, params) in [
            (
                "initialize",
                r#"{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}"#,
            ),
            ("tools/list", "{}"),
            (
                "tools/call",
                r#"{"name":"t01_echo","arguments":{"payload":{"items":[]}}}"#,
            ),
        ] {
            let response = http.post(format!("http://{}/mcp",host.address)).header("accept","application/json, text/event-stream").header("content-type","application/json")
                .body(format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"{method}\",\"params\":{params}}}")).send().await?;
            assert_eq!(response.status(), 200);
            let fields: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
                serde_json::from_str(&response.text().await?)?;
            assert_eq!(fields["id"].get(), id);
            assert!(!fields.contains_key("error"));
        }
    }
    host.shutdown().await?;
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::large_catalog(),
        "/messages/",
        Duration::from_secs(10),
        ct.clone(),
    )?;
    let host = Host::start(legacy.router("/sse")?, ct).await?;
    let mut client = SseClient::connect(format!("http://{}", host.address), "/sse").await?;
    for id in ["18446744073709551616", "18446744073709551617"] {
        for (method, params) in [
            (
                "initialize",
                r#"{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}"#,
            ),
            ("tools/list", "{}"),
            (
                "tools/call",
                r#"{"name":"t01_echo","arguments":{"payload":{"items":[]}}}"#,
            ),
        ] {
            let response=client.http.post(format!("{}{}",client.base,client.endpoint)).header("content-type","application/json").body(format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"{method}\",\"params\":{params}}}")).send().await?;
            assert_eq!(response.status(), 202);
            let (_, raw) = client.event().await?;
            let fields: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
                serde_json::from_str(&raw)?;
            assert_eq!(fields["id"].get(), id);
            assert!(!fields.contains_key("error"));
        }
    }
    drop(client);
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_http_session_flood_does_not_consume_accepted_session_capacity() -> Result<()> {
    let ct = CancellationToken::new();
    let host = Host::start(
        ssx_mcp::harness::http_router_mode(Registry::synthetic(), None, ct.clone(), true),
        ct,
    )
    .await?;
    let url = format!("http://{}/mcp", host.address);
    let http = reqwest::Client::new();
    for n in 0..160 {
        let response = http
            .post(&url)
            .header("mcp-session-id", format!("invalid-{n}"))
            .header("accept", "application/json, text/event-stream")
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .send()
            .await?;
        assert!(!response.status().is_success());
        assert_ne!(response.status(), 503);
    }
    let gateway = ssx_mcp::opaque::RawGateway::connect(url, None, Duration::from_secs(3)).await?;
    assert!(gateway.catalogs.contains_key("tools/list"));
    gateway.close().await?;
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn typed_gateway_cancels_tools_resources_and_prompts_upstream() -> Result<()> {
    let mut registry = Registry::synthetic();
    registry.response_delay = Duration::from_secs(10);
    let ct = CancellationToken::new();
    let host = Host::start(
        ssx_mcp::harness::http_router_mode(registry.clone(), None, ct.clone(), true),
        ct,
    )
    .await?;
    let gateway = Gateway::connect(
        format!("http://{}/mcp", host.address),
        None,
        Duration::from_secs(3),
    )
    .await?;
    let handler = gateway.handler();
    let (server_io, client_io) = tokio::io::duplex(65536);
    let server = tokio::spawn(async move {
        handler.serve(server_io).await?.waiting().await?;
        Ok::<(), ssx_mcp::Error>(())
    });
    let client = config().serve(client_io).await?;
    for request in [
        ClientRequest::CallToolRequest(CallToolRequest::new(
            CallToolRequestParams::new("t01_echo").with_arguments(serde_json::from_value(
                json!({"payload":{"items":[]},"note":"wait"}),
            )?),
        )),
        ClientRequest::ReadResourceRequest(ReadResourceRequest::new(
            ReadResourceRequestParams::new("t01://text"),
        )),
        ClientRequest::GetPromptRequest(GetPromptRequest::new(GetPromptRequestParams::new(
            "t01_prompt",
        ))),
    ] {
        let handle = client
            .peer()
            .send_cancellable_request(request, rmcp::service::PeerRequestOptions::no_options())
            .await?;
        tokio::time::timeout(Duration::from_secs(2), async {
            while registry.in_flight.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await?;
        handle.cancel(Some("test downstream cancel".into())).await?;
        tokio::time::timeout(Duration::from_secs(2), async {
            while registry.in_flight.load(Ordering::SeqCst) != 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await?;
    }
    client.cancel().await?;
    tokio::time::timeout(Duration::from_secs(3), server).await???;
    gateway.shutdown().await?;
    host.shutdown().await?;
    Ok(())
}
async fn initialize_http_session(http: &reqwest::Client, url: &str) -> Result<String> {
    let response=http.post(url).header("accept","application/json, text/event-stream").json(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"expiry-test","version":"1"}}})).send().await?;
    assert_eq!(response.status(), 200);
    let session = response.headers()["mcp-session-id"].to_str()?.to_owned();
    let _ = response.bytes().await?;
    let response = http
        .post(url)
        .header("mcp-session-id", &session)
        .header("accept", "application/json, text/event-stream")
        .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .send()
        .await?;
    assert_eq!(response.status(), 202);
    Ok(session)
}
#[tokio::test]
async fn stateful_idle_expiry_reclaims_sdk_aliases_and_permits_but_keeps_active_work() -> Result<()>
{
    let mut registry = Registry::synthetic();
    registry.response_delay = Duration::from_millis(400);
    let ct = CancellationToken::new();
    let (router, probe) = ssx_mcp::harness::http_router_with_session_expiry(
        registry.clone(),
        None,
        ct.clone(),
        Duration::from_millis(100),
    );
    let host = Host::start(router, ct).await?;
    let http = reqwest::Client::new();
    let url = format!("http://{}/mcp", host.address);
    let session = initialize_http_session(&http, &url).await?;
    let request = async {
        let response=http.post(&url).header("mcp-session-id",&session).header("accept","application/json, text/event-stream").json(&json!({"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"t01://text"}})).send().await?;
        assert_eq!(response.status(), 200);
        Ok::<_, ssx_mcp::Error>(response.text().await?)
    };
    tokio::pin!(request);
    tokio::select! {r=&mut request=>panic!("request completed before injected delay: {r:?}"),_=tokio::time::sleep(Duration::from_millis(250))=>{}}
    assert_eq!(registry.in_flight.load(Ordering::SeqCst), 1);
    assert_eq!(probe.aliases(), 1);
    assert_eq!(probe.sdk_sessions().await, 1);
    let response = tokio::time::timeout(Duration::from_secs(2), request).await??;
    let data = response
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .find(|data| !data.trim().is_empty())
        .ok_or("missing SSE response")?;
    assert_eq!(serde_json::from_str::<Value>(data)?["id"], 2);
    let stream = http
        .get(&url)
        .header("mcp-session-id", &session)
        .header("accept", "text/event-stream")
        .send()
        .await?;
    assert_eq!(stream.status(), 200);
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(probe.aliases(), 1);
    assert_eq!(probe.sdk_sessions().await, 1);
    drop(stream);
    tokio::time::timeout(Duration::from_secs(3), async {
        while probe.aliases() != 0
            || probe.sdk_sessions().await != 0
            || probe.available_permits() != 128
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert_eq!(probe.available_permits(), 128);
    let response = http
        .post(&url)
        .header("mcp-session-id", &session)
        .header("accept", "application/json, text/event-stream")
        .json(&json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}))
        .send()
        .await?;
    assert_eq!(response.status(), 404);
    let replacement = initialize_http_session(&http, &url).await?;
    assert_ne!(replacement, session);
    host.shutdown().await?;
    tokio::time::timeout(Duration::from_secs(2), probe.stopped()).await??;
    assert_eq!(probe.aliases(), 0);
    assert_eq!(probe.sdk_sessions().await, 0);
    Ok(())
}

fn assert_integer_echo(raw: &str) -> Result<()> {
    type RawMap = std::collections::BTreeMap<String, Box<serde_json::value::RawValue>>;
    let fields: RawMap = serde_json::from_str(raw)?;
    assert!(!fields.contains_key("error"));
    let result: RawMap = serde_json::from_str(fields["result"].get())?;
    assert_eq!(
        serde_json::from_str::<Value>(result["content"].get())?,
        baseline()["observations"]["integer_echo_result"]["content"]
    );
    let structured: RawMap = serde_json::from_str(result["structuredContent"].get())?;
    let payload: RawMap = serde_json::from_str(structured["payload"].get())?;
    let items: Vec<Box<serde_json::value::RawValue>> =
        serde_json::from_str(payload["items"].get())?;
    assert_eq!(
        items.iter().map(|v| v.get()).collect::<Vec<_>>(),
        [
            "9223372036854775808",
            "18446744073709551617",
            "-18446744073709551617"
        ]
    );
    Ok(())
}
#[tokio::test]
async fn unbounded_integer_echo_retains_source_tokens_and_rejects_bool_fractional_and_failure()
-> Result<()> {
    let ct = CancellationToken::new();
    let host = Host::start(http_router(Registry::synthetic(), None, ct.clone()), ct).await?;
    let url = format!("http://{}/mcp", host.address);
    let http = reqwest::Client::new();
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/messages/",
        Duration::from_secs(10),
        ct.clone(),
    )?;
    let sse_host = Host::start(legacy.router("/sse")?, ct).await?;
    let mut sse = SseClient::connect(format!("http://{}", sse_host.address), "/sse").await?;
    sse.request(1,"initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"integer-test","version":"1"}})).await?;
    for (items, note, success) in [
        (
            "[9223372036854775808,18446744073709551617,-18446744073709551617]",
            "null",
            true,
        ),
        ("[18446744073709551617,true]", "null", false),
        ("[18446744073709551617,1.5]", "null", false),
        ("[18446744073709551617]", "\"error\"", false),
    ] {
        let request = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{{\"name\":\"t01_echo\",\"arguments\":{{\"payload\":{{\"items\":{items}}},\"note\":{note}}}}}}}"
        );
        let response = http
            .post(&url)
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json")
            .body(request.clone())
            .send()
            .await?;
        assert_eq!(response.status(), 200);
        let direct = response.text().await?;
        assert_eq!(
            sse.post_raw(&request).await?,
            baseline()["observations"]["accepted"]
        );
        let (_, legacy) = sse.event().await?;
        let stdio = raw_process_request(&["stdio"], &request).await?;
        let gateway = raw_process_request(&["gateway", &url], &request).await?;
        for raw in [direct, legacy, stdio, gateway] {
            if success {
                assert_integer_echo(&raw)?;
            } else {
                let response: Value = serde_json::from_str(&raw)?;
                assert!(response.get("error").is_some() || response["result"]["isError"] == true);
            }
        }
    }
    let response=http.post(&url).header("accept","application/json, text/event-stream").header("content-type","application/json").body(r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"t01_echo","arguments":{"payload":{"items":[01]}}}}"#).send().await?;
    assert_eq!(response.status(), 400);
    drop(sse);
    sse_host.shutdown().await?;
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn sdk_termination_and_abrupt_host_or_router_drop_drain_actual_session_state() -> Result<()> {
    let ct = CancellationToken::new();
    let (router, probe) = ssx_mcp::harness::http_router_with_session_expiry(
        Registry::synthetic(),
        None,
        ct.clone(),
        Duration::from_secs(10),
    );
    let host = Host::start(router, ct).await?;
    let url = format!("http://{}/mcp", host.address);
    let http = reqwest::Client::new();
    let session = initialize_http_session(&http, &url).await?;
    assert_eq!(probe.sdk_sessions().await, 1);
    probe.terminate_sdk_session(&session).await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        while probe.aliases() != 0
            || probe.sdk_sessions().await != 0
            || probe.available_permits() != 128
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    initialize_http_session(&http, &url).await?;
    assert_eq!(probe.sdk_sessions().await, 1);
    drop(host);
    tokio::time::timeout(Duration::from_secs(3), probe.stopped()).await??;
    assert_eq!(probe.sdk_sessions().await, 0);
    assert_eq!(probe.aliases(), 0);
    assert_eq!(probe.available_permits(), 128);
    let ct = CancellationToken::new();
    let (router, probe) = ssx_mcp::harness::http_router_with_session_expiry(
        Registry::synthetic(),
        None,
        ct.clone(),
        Duration::from_secs(10),
    );
    drop(router);
    assert!(!ct.is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), probe.stopped()).await??;
    assert_eq!(probe.sdk_sessions().await, 0);
    assert_eq!(probe.aliases(), 0);
    Ok(())
}
#[tokio::test]
async fn malformed_call_shapes_do_not_poison_stateful_request_admission() -> Result<()> {
    let ct = CancellationToken::new();
    let host = Host::start(
        ssx_mcp::harness::http_router_mode(Registry::synthetic(), None, ct.clone(), true),
        ct,
    )
    .await?;
    let url = format!("http://{}/mcp", host.address);
    let http = reqwest::Client::new();
    let session = initialize_http_session(&http, &url).await?;
    for id in 2..82 {
        let params = if id % 2 == 0 {
            "[]"
        } else {
            r#"{"name":"t01_echo","arguments":[]}"#
        };
        let response=http.post(&url).header("mcp-session-id",&session).header("accept","application/json, text/event-stream").header("content-type","application/json").body(format!("{{\"jsonrpc\":\"2.0\",\"id\":\"bad-{id}\",\"method\":\"tools/call\",\"params\":{params}}}")).send().await?;
        if response.status() == 200 {
            assert!(response.text().await?.contains("\"error\""));
        } else {
            assert!(response.status().is_client_error());
        }
    }
    let gateway =
        ssx_mcp::opaque::RawGateway::connect(url.clone(), None, Duration::from_secs(3)).await?;
    gateway.close().await?;
    let response = http
        .post(&url)
        .header("mcp-session-id", &session)
        .header("accept", "application/json, text/event-stream")
        .json(&json!({"jsonrpc":"2.0","id":"valid-after-malformed","method":"tools/list"}))
        .send()
        .await?;
    assert_eq!(response.status(), 200);
    let body = response.text().await?;
    assert!(body.contains("valid-after-malformed"));
    assert!(!body.contains("\"error\""));
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn typed_and_raw_catalogs_use_resource_uri_identity() -> Result<()> {
    for duplicate_uri in [false, true] {
        let mut registry = Registry::synthetic();
        let mut descriptors = registry.descriptors.as_ref().clone();
        descriptors["resources"] = json!([{"name":"same display name","uri":"t01://one"},{"name":if duplicate_uri{"another display name"}else{"same display name"},"uri":if duplicate_uri{"t01://one"}else{"t01://two"}}]);
        registry.descriptors = std::sync::Arc::new(descriptors);
        let ct = CancellationToken::new();
        let host = Host::start(http_router(registry, None, ct.clone()), ct).await?;
        let url = format!("http://{}/mcp", host.address);
        let typed = Gateway::connect(url.clone(), None, Duration::from_secs(3)).await;
        let raw = ssx_mcp::opaque::RawGateway::connect(url, None, Duration::from_secs(3)).await;
        if duplicate_uri {
            assert!(typed.is_err());
            assert!(raw.is_err());
        } else {
            let typed = typed?;
            assert_eq!(typed.catalog["resources"].as_array().unwrap().len(), 2);
            typed.shutdown().await?;
            let raw = raw?;
            let catalog: Value = serde_json::from_str(raw.catalogs["resources/list"].get())?;
            assert_eq!(catalog["resources"].as_array().unwrap().len(), 2);
            raw.close().await?;
        }
        host.shutdown().await?;
    }
    Ok(())
}
#[tokio::test]
async fn raw_gateway_uses_negotiated_older_protocol_on_every_http_operation() -> Result<()> {
    use axum::{response::IntoResponse, routing::post};
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let posts = seen.clone();
    let deletes = seen.clone();
    let router=axum::Router::new().route("/mcp",post(move |headers:axum::http::HeaderMap,axum::Json(body):axum::Json<Value>|{
        let seen=posts.clone();async move {
            let method=body["method"].as_str().unwrap_or("");
            if method!="initialize" && headers.get("mcp-protocol-version").and_then(|v|v.to_str().ok())!=Some("2024-11-05") {return axum::http::StatusCode::BAD_REQUEST.into_response();}
            seen.lock().unwrap().push(method.to_owned());
            if method=="notifications/initialized" {return axum::http::StatusCode::ACCEPTED.into_response();}
            let result=match method {
                "initialize"=>json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{},"resources":{},"prompts":{}},"serverInfo":{"name":"older-peer","version":"1"}}),
                "tools/list"=>json!({"tools":[]}),"prompts/list"=>json!({"prompts":[]}),
                "resources/list"=>json!({"resources":[{"name":"same","uri":"t01://one"},{"name":"same","uri":"t01://two"}]}),
                "resources/templates/list"=>json!({"resourceTemplates":[{"name":"same","uriTemplate":"t01://one/{id}"},{"name":"same","uriTemplate":"t01://two/{id}"}]}),
                "tools/call"=>baseline()["observations"]["echo_result"].clone(),
                _=>return axum::http::StatusCode::BAD_REQUEST.into_response(),
            };
            let mut response=axum::Json(json!({"jsonrpc":"2.0","id":body["id"],"result":result})).into_response();
            if method=="initialize"{response.headers_mut().insert("mcp-session-id",axum::http::HeaderValue::from_static("older-session"));}response
        }
    }).delete(move |headers:axum::http::HeaderMap|{let seen=deletes.clone();async move{
        if headers.get("mcp-protocol-version").and_then(|v|v.to_str().ok())!=Some("2024-11-05") {return axum::http::StatusCode::BAD_REQUEST;}
        seen.lock().unwrap().push("DELETE".into());axum::http::StatusCode::NO_CONTENT
    }}));
    let ct = CancellationToken::new();
    let host = Host::start(router, ct).await?;
    let url = format!("http://{}/mcp", host.address);
    let gateway =
        ssx_mcp::opaque::RawGateway::connect(url.clone(), None, Duration::from_secs(3)).await?;
    let templates: Value =
        serde_json::from_str(gateway.catalogs["resources/templates/list"].get())?;
    assert_eq!(templates["resourceTemplates"].as_array().unwrap().len(), 2);
    gateway.close().await?;
    let result=raw_process_request(&["gateway",&url],r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"t01_echo","arguments":{"payload":{"items":[1,2]},"note":null}}}"#).await?;
    assert_eq!(
        serde_json::from_str::<Value>(&result)?["result"],
        baseline()["observations"]["echo_result"]
    );
    let seen = seen.lock().unwrap().clone();
    for method in [
        "notifications/initialized",
        "tools/list",
        "resources/list",
        "resources/templates/list",
        "prompts/list",
        "tools/call",
        "DELETE",
    ] {
        assert!(seen.iter().any(|value| value == method));
    }
    assert_eq!(seen.iter().filter(|m| m.as_str() == "DELETE").count(), 2);
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn rejected_delete_retains_actual_sdk_session_aliases_and_capacity() -> Result<()> {
    let cancel = CancellationToken::new();
    let (router, probe) = ssx_mcp::harness::http_router_with_session_expiry(
        Registry::synthetic(),
        None,
        cancel.clone(),
        Duration::from_secs(30),
    );
    let host = Host::start(router, cancel).await?;
    let url = format!("http://{}/mcp", host.address);
    let http = reqwest::Client::new();
    let mut first = None;
    for index in 0..128 {
        let session = initialize_http_session(&http, &url).await?;
        if first.is_none() {
            first = Some(session.clone());
        }
        let rejected = http
            .delete(&url)
            .header("mcp-session-id", &session)
            .header("mcp-protocol-version", "unsupported")
            .send()
            .await?;
        assert_eq!(rejected.status(), 400);
        let _ = rejected.bytes().await?;
        assert_eq!(probe.sdk_sessions().await, index + 1);
        assert_eq!(probe.aliases(), index + 1);
        assert_eq!(probe.available_permits(), 127 - index);
    }
    let first = first.unwrap();
    let valid=http.post(&url).header("mcp-session-id",&first).header("mcp-protocol-version","2025-11-25").header("accept","application/json, text/event-stream").json(&json!({"jsonrpc":"2.0","id":"still-works-after-rejected-delete","method":"tools/list"})).send().await?;
    assert_eq!(valid.status(), 200);
    let body = valid.text().await?;
    assert!(body.contains("still-works-after-rejected-delete"));
    assert!(!body.contains("\"error\""));
    let saturated=http.post(&url).header("accept","application/json, text/event-stream").json(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"capacity-test","version":"1"}}})).send().await?;
    assert_eq!(saturated.status(), 503);
    assert_eq!(probe.sdk_sessions().await, 128);
    let closed = http
        .delete(&url)
        .header("mcp-session-id", &first)
        .header("mcp-protocol-version", "2025-11-25")
        .send()
        .await?;
    assert_eq!(closed.status(), 202);
    let _ = closed.bytes().await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        while probe.sdk_sessions().await != 127
            || probe.aliases() != 127
            || probe.available_permits() != 1
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    initialize_http_session(&http, &url).await?;
    assert_eq!(probe.sdk_sessions().await, 128);
    assert_eq!(probe.aliases(), 128);
    assert_eq!(probe.available_permits(), 0);
    host.shutdown().await?;
    assert_eq!(probe.sdk_sessions().await, 0);
    assert_eq!(probe.aliases(), 0);
    assert_eq!(probe.available_permits(), 128);
    Ok(())
}

#[derive(Clone, Copy)]
enum CatalogSizeFault {
    ContentLength,
    Chunked,
    ManyPages,
    CrossCatalog,
    TemplatePages,
}
async fn catalog_size_fixture(
    fault: CatalogSizeFault,
) -> Result<(Host, std::sync::Arc<std::sync::atomic::AtomicUsize>)> {
    use axum::response::IntoResponse;
    let pages = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = pages.clone();
    let router=axum::Router::new().route("/mcp",axum::routing::post(move |axum::Json(body):axum::Json<Value>|{let pages=observed.clone();async move{
        let method=body["method"].as_str().unwrap_or("");
        if method=="notifications/initialized" {return axum::http::StatusCode::ACCEPTED.into_response();}
        let id=body["id"].clone();
        if method=="initialize" {return axum::Json(json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{},"resources":{},"prompts":{}},"serverInfo":{"name":"size-fixture","version":"1"}}})).into_response();}
        if method=="tools/list" && matches!(fault,CatalogSizeFault::ContentLength|CatalogSizeFault::Chunked) {
            let stream=futures_util::stream::repeat_with(||Ok::<_,std::io::Error>(axum::body::Bytes::from_static(&[b'x';16384])));
            let mut response=axum::body::Body::from_stream(stream).into_response();response.headers_mut().insert("content-type",axum::http::HeaderValue::from_static("application/json"));
            if matches!(fault,CatalogSizeFault::ContentLength){response.headers_mut().insert("content-length",axum::http::HeaderValue::from_str(&(ssx_mcp::http_client::MAX_RPC_BYTES+1).to_string()).unwrap());}
            return response;
        }
        let page=body["params"]["cursor"].as_str().and_then(|s|s.parse::<usize>().ok()).unwrap_or(0);
        let active=if matches!(fault,CatalogSizeFault::TemplatePages){method=="resources/templates/list"}else{method=="tools/list" || matches!(fault,CatalogSizeFault::CrossCatalog) && matches!(method,"resources/list"|"prompts/list")};
        let last=if matches!(fault,CatalogSizeFault::ManyPages|CatalogSizeFault::TemplatePages){63}else{11};
        let description="x".repeat(512*1024);
        let result=match method {
            "tools/list"=>if active{json!({"tools":[{"name":format!("tool-{page}"),"description":description,"inputSchema":{"type":"object"}}]})}else{json!({"tools":[]})},
            "resources/list"=>if active{json!({"resources":[{"name":format!("resource-{page}"),"uri":format!("t01://resource/{page}"),"description":description}]})}else{json!({"resources":[]})},
            "prompts/list"=>if active{json!({"prompts":[{"name":format!("prompt-{page}"),"description":description}]})}else{json!({"prompts":[]})},
            "resources/templates/list"=>if active{json!({"resourceTemplates":[{"name":format!("template-{page}"),"uriTemplate":format!("t01://{page}/{{id}}"),"description":description}]})}else{json!({"resourceTemplates":[]})},
            _=>return axum::http::StatusCode::BAD_REQUEST.into_response(),
        };
        let mut result=result;
        if active {pages.fetch_add(1,Ordering::SeqCst);if page<last{result["nextCursor"]=json!((page+1).to_string());}}
        axum::Json(json!({"jsonrpc":"2.0","id":id,"result":result})).into_response()
    }}));
    let host = Host::start(router, CancellationToken::new()).await?;
    Ok((host, pages))
}
#[tokio::test]
async fn real_catalog_response_and_shared_discovery_byte_limits_fail_closed() -> Result<()> {
    for fault in [
        CatalogSizeFault::ContentLength,
        CatalogSizeFault::Chunked,
        CatalogSizeFault::ManyPages,
        CatalogSizeFault::CrossCatalog,
        CatalogSizeFault::TemplatePages,
    ] {
        for typed in [false, true] {
            let (host, pages) = catalog_size_fixture(fault).await?;
            let url = format!("http://{}/mcp", host.address);
            let error = if typed {
                match Gateway::connect(url, None, Duration::from_secs(10)).await {
                    Ok(gateway) => {
                        gateway.shutdown().await?;
                        panic!("oversized catalog accepted");
                    }
                    Err(error) => error.to_string(),
                }
            } else {
                match ssx_mcp::opaque::RawGateway::connect(url, None, Duration::from_secs(10)).await
                {
                    Ok(gateway) => {
                        gateway.close().await?;
                        panic!("oversized raw catalog accepted");
                    }
                    Err(error) => error.to_string(),
                }
            };
            let expected = if matches!(
                fault,
                CatalogSizeFault::ContentLength | CatalogSizeFault::Chunked
            ) {
                "4 MiB"
            } else {
                "16 MiB"
            };
            assert!(
                error.contains(expected),
                "wrong failure for typed={typed}: {error}"
            );
            if matches!(
                fault,
                CatalogSizeFault::ManyPages
                    | CatalogSizeFault::CrossCatalog
                    | CatalogSizeFault::TemplatePages
            ) {
                let count = pages.load(Ordering::SeqCst);
                assert!(
                    (32..128).contains(&count),
                    "byte limit must precede page/item limit: {count}"
                );
            }
            host.shutdown().await?;
        }
    }
    Ok(())
}
#[tokio::test]
async fn legacy_global_admission_recovers_after_disconnect_expiry_and_shutdown() -> Result<()> {
    let cancel = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/messages/",
        Duration::from_secs(30),
        cancel.clone(),
    )?;
    let host = Host::start(legacy.clone().router("/sse")?, cancel).await?;
    let base = format!("http://{}", host.address);
    let mut clients = Vec::new();
    for _ in 0..128 {
        clients.push(SseClient::connect(base.clone(), "/sse").await?);
    }
    assert_eq!(legacy.active_sessions(), 128);
    assert_eq!(legacy.available_permits(), 0);
    let rejected = reqwest::get(format!("{base}/sse")).await?;
    assert_eq!(rejected.status(), 503);
    assert_eq!(legacy.active_sessions(), 128);
    // A malformed handshake does not free a live connection's admission.
    let mut client = clients.pop().unwrap();
    assert_eq!(
        client
            .post(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}))
            .await?["status"],
        202
    );
    let (_, body) = client.event().await?;
    assert_eq!(
        serde_json::from_str::<Value>(&body)?["error"]["code"],
        -32602
    );
    assert_eq!(legacy.available_permits(), 0);
    drop(client);
    tokio::time::timeout(Duration::from_secs(3), async {
        while legacy.available_permits() != 1 || legacy.active_sessions() != 127 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    clients.push(SseClient::connect(base, "/sse").await?);
    assert_eq!(legacy.active_sessions(), 128);
    host.shutdown().await?;
    assert_eq!(legacy.active_sessions(), 0);
    assert_eq!(legacy.available_permits(), 128);
    drop(clients);
    let cancel = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/messages/",
        Duration::from_millis(50),
        cancel.clone(),
    )?;
    let host = Host::start(legacy.clone().router("/sse")?, cancel).await?;
    let client = SseClient::connect(format!("http://{}", host.address), "/sse").await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        while legacy.active_sessions() != 0 || legacy.available_permits() != 128 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    drop(client);
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn typed_and_raw_template_pagination_forwarding_and_uri_template_identity() -> Result<()> {
    use axum::response::IntoResponse;
    for duplicate in [false, true] {
        let expected = json!([{"name":"same","uriTemplate":"t01://one/{id}","description":"first template","mimeType":"text/plain","_meta":{"nested":{"nullable":null}}},{"name":if duplicate{"different"}else{"same"},"uriTemplate":if duplicate{"t01://one/{id}"}else{"t01://two/{id}"},"description":"second template"}]);
        let rows = expected.clone();
        let pages = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = pages.clone();
        let router=axum::Router::new().route("/mcp",axum::routing::post(move |axum::Json(body):axum::Json<Value>|{let rows=rows.clone();let observed=observed.clone();async move{
            let result=match body["method"].as_str().unwrap_or("") {
                "initialize"=>json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{},"resources":{},"prompts":{}},"serverInfo":{"name":"templates-peer","version":"1"}}),
                "notifications/initialized"=>return axum::http::StatusCode::ACCEPTED.into_response(),
                "tools/list"=>json!({"tools":[]}),"resources/list"=>json!({"resources":[]}),"prompts/list"=>json!({"prompts":[]}),
                "resources/templates/list"=>{observed.fetch_add(1,Ordering::SeqCst);if body["params"]["cursor"]=="next"{json!({"resourceTemplates":[rows[1]]})}else{json!({"resourceTemplates":[rows[0]],"nextCursor":"next"})}},
                _=>return axum::http::StatusCode::BAD_REQUEST.into_response(),
            };axum::Json(json!({"jsonrpc":"2.0","id":body["id"],"result":result})).into_response()
        }}));
        let host = Host::start(router, CancellationToken::new()).await?;
        let url = format!("http://{}/mcp", host.address);
        let typed = Gateway::connect(url.clone(), None, Duration::from_secs(3)).await;
        if duplicate {
            assert!(typed.is_err());
            assert!(
                ssx_mcp::opaque::RawGateway::connect(url, None, Duration::from_secs(3))
                    .await
                    .is_err()
            );
        } else {
            let typed = typed?;
            assert_eq!(typed.catalog["resourceTemplates"], expected);
            let handler = typed.handler();
            let (server_io, client_io) = tokio::io::duplex(65536);
            let server = tokio::spawn(async move {
                handler.serve(server_io).await?.waiting().await?;
                Ok::<_, ssx_mcp::Error>(())
            });
            let client = config().serve(client_io).await?;
            assert_eq!(
                serde_json::to_value(
                    client
                        .list_resource_templates(None)
                        .await?
                        .resource_templates
                )?,
                expected
            );
            client.cancel().await?;
            server.await??;
            typed.shutdown().await?;
            let raw = raw_process_request(
                &["gateway", &url],
                r#"{"jsonrpc":"2.0","id":2,"method":"resources/templates/list","params":{}}"#,
            )
            .await?;
            assert_eq!(
                serde_json::from_str::<Value>(&raw)?["result"]["resourceTemplates"],
                expected
            );
        }
        assert_eq!(pages.load(Ordering::SeqCst), 4);
        host.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn selected_raw_gateway_close_retries_failed_and_timed_out_delete() -> Result<()> {
    use axum::response::IntoResponse;
    for fault in ["500", "timeout", "405", "404"] {
        let ct = CancellationToken::new();
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let ids = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = attempts.clone();
        let seen = ids.clone();
        let router =
            ssx_mcp::harness::http_router_mode(Registry::synthetic(), None, ct.clone(), true).map(
                |router| {
                    router.layer(axum::middleware::from_fn(
                        move |request: axum::http::Request<axum::body::Body>,
                              next: axum::middleware::Next| {
                            let observed = observed.clone();
                            let seen = seen.clone();
                            async move {
                                if request.method() == axum::http::Method::DELETE {
                                    seen.lock().unwrap().push(
                                        request.headers()["mcp-session-id"]
                                            .to_str()
                                            .unwrap()
                                            .to_owned(),
                                    );
                                    if observed.fetch_add(1, Ordering::SeqCst) == 0 {
                                        match fault {
                                            "500" => {
                                                return axum::http::StatusCode::INTERNAL_SERVER_ERROR
                                                .into_response();
                                            }
                                            "405" => {
                                                return axum::http::StatusCode::METHOD_NOT_ALLOWED
                                                    .into_response();
                                            }
                                            "timeout" => {
                                                tokio::time::sleep(Duration::from_millis(350))
                                                    .await;
                                                return axum::http::StatusCode::INTERNAL_SERVER_ERROR
                                                .into_response();
                                            }
                                            // Close the real SDK session, but expose its already absent status.
                                            "404" => {
                                                let _ = next.run(request).await;
                                                return axum::http::StatusCode::NOT_FOUND
                                                    .into_response();
                                            }
                                            _ => unreachable!(),
                                        }
                                    }
                                }
                                next.run(request).await
                            }
                        },
                    ))
                },
            );
        let host = Host::start(router, ct).await?;
        let gateway = ssx_mcp::opaque::RawGateway::connect(
            format!("http://{}/mcp", host.address),
            None,
            Duration::from_millis(150),
        )
        .await?;
        let first = gateway.close().await;
        assert_eq!(first.is_err(), matches!(fault, "500" | "timeout"));
        if fault == "404" {
            gateway.close().await?;
            assert_eq!(attempts.load(Ordering::SeqCst), 1);
        } else {
            // Both concurrent retries retain the original ID. One may observe 404
            // after the other successfully terminates the actual SDK session.
            let (a, b) = tokio::join!(gateway.close(), gateway.close());
            a?;
            b?;
            let after = attempts.load(Ordering::SeqCst);
            assert!((2..=3).contains(&after));
            gateway.close().await?;
            assert_eq!(attempts.load(Ordering::SeqCst), after);
        }
        let ids = ids.lock().unwrap().clone();
        assert!(!ids.is_empty());
        assert!(ids.iter().all(|id| id == &ids[0]));
        host.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn legacy_near_ingress_limit_echo_keeps_complete_amplified_response() -> Result<()> {
    let cancel = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/messages/",
        Duration::from_secs(30),
        cancel.clone(),
    )?;
    let host = Host::start(legacy.clone().router("/sse")?, cancel).await?;
    let mut client = SseClient::connect(format!("http://{}", host.address), "/sse").await?;
    client.request(1, "initialize", json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}})).await?;
    let note = "a".repeat(4 * 1024 * 1024 - 1024);
    let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"t01_echo","arguments":{"payload":{"items":[]},"note":note}}}).to_string();
    assert!(request.len() < 4 * 1024 * 1024);
    assert_eq!(client.post_raw(&request).await?["status"], 202);
    let raw = tokio::time::timeout(Duration::from_secs(15), async {
        let mut raw = Vec::new();
        loop {
            let chunk = client
                .response
                .chunk()
                .await?
                .ok_or("missing amplified echo")?;
            raw.extend_from_slice(&chunk);
            assert!(raw.len() < 16 * 1024 * 1024);
            if raw.ends_with(b"\n\n") {
                return Ok::<_, ssx_mcp::Error>(raw);
            }
        }
    })
    .await??;
    assert!(raw.len() > 4 * 1024 * 1024);
    let raw = std::str::from_utf8(&raw)?;
    let data = raw
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .ok_or("missing event data")?;
    let reply: Value = serde_json::from_str(data)?;
    assert_eq!(reply["result"]["structuredContent"]["note"], note);
    let text: Value =
        serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap())?;
    assert_eq!(text["note"], note);
    drop(client);
    host.shutdown().await?;
    assert_eq!(legacy.available_message_permits(), 32);
    assert_eq!(legacy.available_output_bytes(), 64 * 1024 * 1024);
    Ok(())
}

/// Native replacement for the unused upstream Python development smoke's SDK
/// child-process APIs. The fixture has one catalog page; this does not claim
/// live Python interoperability or multi-page list_all coverage.
#[tokio::test]
async fn native_sdk_child_builder_captures_stderr_and_one_page_catalogs() -> Result<()> {
    use tokio::io::AsyncReadExt;

    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_mcp-qualification"));
    command
        .arg("stdio-stderr")
        .env_clear()
        .env("PATH", "")
        .kill_on_drop(true);
    let (transport, stderr) = TokioChildProcess::builder(command)
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let stderr = stderr.ok_or("SDK builder did not return the requested stderr handle")?;
    let mut capture = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr.take(65_537).read_to_end(&mut bytes).await?;
        if bytes.len() > 65_536 {
            return Err(std::io::Error::other(
                "native fixture stderr exceeded its bound",
            ));
        }
        Ok::<_, std::io::Error>(bytes)
    });
    let observation: Result<()> = async {
        let client =
            tokio::time::timeout(Duration::from_secs(5), config().serve(transport)).await??;
        let catalogs = tokio::time::timeout(Duration::from_secs(5), async {
            let expected = baseline();
            compare_descriptor(
                &expected["observations"]["resources"],
                &serde_json::to_value(client.list_all_resources().await?)?,
            )?;
            compare_descriptor(
                &expected["observations"]["tools"],
                &serde_json::to_value(client.list_all_tools().await?)?,
            )?;
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        })
        .await;
        // Await SDK transport.close/graceful child shutdown even if a catalog
        // operation failed. Do not cancel this cleanup future with a timeout.
        let closed = client.cancel().await;
        catalogs??;
        closed?;
        Ok(())
    }
    .await;
    // Retain and join the actual stderr reader on success, protocol failure,
    // handshake failure and timeout. A failed drain never becomes a pass.
    let stderr = match tokio::time::timeout(Duration::from_secs(3), &mut capture).await {
        Ok(result) => result?,
        Err(error) => {
            capture.abort();
            let _ = capture.await;
            return Err(error.into());
        }
    };
    observation?;
    assert_eq!(stderr?.as_slice(), b"T01_NATIVE_STDERR_READY\n");
    Ok(())
}
