//! Native envelope restoration; actual handler text is the only restoration source.
#[path = "t06_support/wire_handler.rs"]
mod wire_handler;
use serde_json::{json, value::RawValue};
use ssx_mcp::{
    Result,
    opaque::{Frames, Overlay},
    output_limits::OutputLimits,
};
use std::collections::BTreeMap;
fn request(id: u64) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"native_wire_probe","arguments":{{}}}}}}"#
    )
}
fn response(id: u64, result: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{result},"literal":18446744073709551617}}"#)
}
#[test]
fn unknown_result_and_unmarked_metadata_remain_literal() -> Result<()> {
    for result in [
        "null",
        "[1,null]",
        "18446744073709551617",
        r#""scalar""#,
        r#"{"_meta":null}"#,
        r#"{"_meta":[1],"structuredContent":null}"#,
    ] {
        let original = response(5, result);
        assert_eq!(Overlay::default().response(&original)?, original);
    }
    Ok(())
}
#[test]
fn marker_is_scrubbed_from_every_result_path_and_only_trusted_objects_restore() -> Result<()> {
    let catalogs = wire_handler::WireHandler::catalogs();
    for admitted in [false, true] {
        for marker in [false, true] {
            for error in [false, true] {
                let mut overlay = Overlay::default();
                if admitted {
                    overlay.request(&catalogs, &request(1))?;
                }
                let result = json!({"_meta":{"ssx/native-structured-text":marker,"literal":[null,"number"]},"isError":error,"structuredContent":{},"content":[{"type":"text","text":"{\"a\":18446744073709551617,\"b\":18446744073709551618,\"null\":null}"}]});
                let raw = overlay.response(&response(1, &result.to_string()))?;
                assert!(!raw.contains("ssx/native-structured-text"));
                assert!(raw.contains("literal"));
                let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&raw)?;
                let result: BTreeMap<String, Box<RawValue>> =
                    serde_json::from_str(fields["result"].get())?;
                assert_eq!(
                    result["structuredContent"]
                        .get()
                        .contains("18446744073709551618"),
                    admitted && marker && !error
                );
                assert!(!overlay.has_pending());
            }
        }
    }
    for content in [
        json!([]),
        json!([{"type":"image","data":"a","mimeType":"image/png"}]),
        json!([{"type":"text","text":"{}"},{"type":"text","text":"{}"}]),
        json!([{"type":"text","text":"null"}]),
        json!([{"type":"text","text":"malformed"}]),
    ] {
        let mut overlay = Overlay::default();
        overlay.request(&catalogs, &request(7))?;
        let result = json!({"_meta":{"ssx/native-structured-text":true},"structuredContent":{},"content":content});
        assert!(overlay.response(&response(7, &result.to_string())).is_err());
        assert!(!overlay.has_pending());
    }
    Ok(())
}
fn verify_lossless(raw: &str, expected: &str) -> Result<()> {
    assert!(!raw.contains("ssx/native-structured-text"));
    let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(raw)?;
    let result: BTreeMap<String, Box<RawValue>> = serde_json::from_str(fields["result"].get())?;
    assert_eq!(result["structuredContent"].get(), expected);
    let content: Vec<serde_json::Value> = serde_json::from_str(result["content"].get())?;
    assert_eq!(content[0]["text"].as_str(), Some(expected));
    Ok(())
}
#[test]
fn large_deep_native_output_uses_raw_envelope_without_default_document_parser() -> Result<()> {
    for kind in ["ordinary", "quotes", "controls"] {
        let h = wire_handler::WireHandler::new(kind);
        let mut overlay = Overlay::default();
        overlay.request(&wire_handler::WireHandler::catalogs(), &request(1))?;
        let result = json!({"content":[{"type":"text","text":h.payload}],"structuredContent":{},"isError":false,"_meta":{"ssx/native-structured-text":true}});
        let restored = overlay.response(&response(1, &result.to_string()))?;
        assert!(restored.len() > 4 * 1024 * 1024);
        assert!(restored.len() < 32 * 1024 * 1024);
        verify_lossless(&restored, &h.payload)?;
    }
    Ok(())
}
#[tokio::test]
async fn typed_output_budget_preserves_default_and_rejects_one_byte_over() -> Result<()> {
    assert_eq!(OutputLimits::HTTP.bytes(), 4 * 1024 * 1024);
    assert_eq!(OutputLimits::LEGACY.bytes(), 16 * 1024 * 1024);
    assert!(OutputLimits::new(0).is_err());
    assert!(OutputLimits::new(64 * 1024 * 1024 + 1).is_err());
    let limits = OutputLimits::new(32 * 1024 * 1024)?;
    let at = vec![b'x'; limits.bytes() - 1];
    let mut at = at;
    at.push(b'\n');
    assert_eq!(
        Frames::with_limit(at.as_slice(), limits)
            .next()
            .await?
            .unwrap()
            .len(),
        limits.bytes() - 1
    );
    let over = vec![b'x'; limits.bytes() + 1];
    let error = Frames::with_limit(over.as_slice(), limits)
        .next()
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains(&limits.bytes().to_string()));
    assert!(
        Frames::new(over.as_slice())
            .next()
            .await
            .unwrap_err()
            .to_string()
            .contains("4194304")
    );
    Ok(())
}
fn init_request() -> String {
    r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"native-wire","version":"1"}}}"#.into()
}
async fn http_send(
    client: &reqwest::Client,
    url: &str,
    request: String,
) -> Result<reqwest::Response> {
    Ok(client
        .post(url)
        .header("accept", "application/json,text/event-stream")
        .header("mcp-protocol-version", "2025-11-25")
        .header("content-type", "application/json")
        .body(request)
        .send()
        .await?)
}
#[tokio::test]
async fn actual_http_native_large_output_and_one_over_profile_fail_closed() -> Result<()> {
    use ssx_mcp::{
        harness::{Host, http_router_for_with_limits},
        oauth::NetworkAuth,
    };
    use tokio_util::sync::CancellationToken;
    for kind in ["ordinary", "quotes", "controls"] {
        let h = wire_handler::WireHandler::new(kind);
        let cancel = CancellationToken::new();
        let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("wire-token".into())?);
        let host = Host::start(
            http_router_for_with_limits(
                h.clone(),
                wire_handler::WireHandler::catalogs(),
                cancel.clone(),
                OutputLimits::new(32 * 1024 * 1024)?,
            )
            .map(|r| auth.mount(r)),
            cancel,
        )
        .await?;
        let url = format!("http://{}/mcp", host.address);
        let client = reqwest::Client::builder().no_proxy().build()?;
        assert_eq!(
            http_send(&client, &url, init_request()).await?.status(),
            401
        );
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, "Bearer wire-token".parse()?);
        let client = reqwest::Client::builder()
            .no_proxy()
            .default_headers(headers)
            .build()?;
        assert_eq!(
            http_send(&client, &url, init_request()).await?.status(),
            200
        );
        let response = http_send(&client, &url, request(1)).await?;
        assert_eq!(response.status(), 200);
        let raw = response.text().await?;
        verify_lossless(&raw, &h.payload)?;
        host.shutdown().await?;
    }
    // A profile below the exact output closes at the transport; marker bytes never escape.
    let h = wire_handler::WireHandler::new("ordinary");
    let cancel = CancellationToken::new();
    let host = Host::start(
        http_router_for_with_limits(
            h,
            wire_handler::WireHandler::catalogs(),
            cancel.clone(),
            OutputLimits::new(1024 * 1024)?,
        ),
        cancel,
    )
    .await?;
    let client = reqwest::Client::builder().no_proxy().build()?;
    let url = format!("http://{}/mcp", host.address);
    http_send(&client, &url, init_request()).await?;
    let response = http_send(&client, &url, request(1)).await?;
    assert_eq!(response.status(), 502);
    assert!(
        !response
            .text()
            .await?
            .contains("ssx/native-structured-text")
    );
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn actual_stdio_native_large_output_with_empty_process_environment() -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    for kind in ["ordinary", "quotes", "controls"] {
        let h = wire_handler::WireHandler::new(kind);
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_wire-qualification"))
            .args(["stdio", kind])
            .env_clear()
            .env("PATH", "")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let mut input = child.stdin.take().unwrap();
        let mut output = tokio::io::BufReader::new(child.stdout.take().unwrap());
        for (i, request) in [init_request(), request(1)].into_iter().enumerate() {
            input.write_all(request.as_bytes()).await?;
            input.write_all(b"\n").await?;
            input.flush().await?;
            let mut raw = String::new();
            assert!(
                tokio::time::timeout(
                    std::time::Duration::from_secs(20),
                    output.read_line(&mut raw)
                )
                .await??
                    > 0
            );
            if i == 1 {
                verify_lossless(&raw, &h.payload)?;
            }
        }
        input.shutdown().await?;
        drop(input);
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(5), child.wait())
                .await??
                .success()
        );
    }
    Ok(())
}
async fn next_event(
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
        let chunk = tokio::time::timeout(std::time::Duration::from_secs(20), stream.chunk())
            .await??
            .ok_or("legacy stream EOF")?;
        pending.extend_from_slice(&chunk);
        assert!(pending.len() < 32 * 1024 * 1024 + 65536);
    }
}
#[tokio::test]
async fn actual_legacy_native_large_output_releases_aggregate_budget_on_disconnect() -> Result<()> {
    use ssx_mcp::{harness::Host, legacy::Legacy, oauth::NetworkAuth};
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;
    for kind in ["ordinary", "quotes", "controls"] {
        let h = wire_handler::WireHandler::new(kind);
        let factory_h = h.clone();
        let cancel = CancellationToken::new();
        let legacy = Legacy::with_factory(
            Arc::new(move |_, _| factory_h.clone()),
            wire_handler::WireHandler::catalogs(),
            "/messages/",
            std::time::Duration::from_secs(60),
            cancel.clone(),
        )?
        .with_output_limits(OutputLimits::new(32 * 1024 * 1024)?);
        let observed = legacy.clone();
        let host = Host::start(
            NetworkAuth::Static(ssx_domain::oauth::Secret::new("wire-token".into()).unwrap())
                .mount(legacy.router("/sse")?),
            cancel,
        )
        .await?;
        let base = format!("http://{}", host.address);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, "Bearer wire-token".parse()?);
        let client = reqwest::Client::builder()
            .no_proxy()
            .default_headers(headers)
            .build()?;
        let mut stream = client.get(format!("{base}/sse")).send().await?;
        assert_eq!(stream.status(), 200);
        let mut pending = Vec::new();
        let (event, path) = next_event(&mut stream, &mut pending).await?;
        assert_eq!(event, "endpoint");
        let url = format!("{base}{path}");
        assert_eq!(
            http_send(&client, &url, init_request()).await?.status(),
            202
        );
        next_event(&mut stream, &mut pending).await?;
        assert_eq!(http_send(&client, &url, request(1)).await?.status(), 202);
        let (_, raw) = next_event(&mut stream, &mut pending).await?;
        verify_lossless(&raw, &h.payload)?;
        drop(stream);
        host.shutdown().await?;
        assert_eq!(observed.available_permits(), 128);
        assert_eq!(observed.available_message_permits(), 32);
    }
    Ok(())
}
#[test]
fn trusted_admission_extracts_only_raw_name_not_arbitrary_argument_numbers() -> Result<()> {
    let catalogs = wire_handler::WireHandler::catalogs();
    let mut overlay = Overlay::default();
    overlay.request(&catalogs,r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"native_wire_probe","arguments":{"unknown":1e400}}}"#)?;
    assert!(overlay.has_pending());
    let result = json!({"content":[{"type":"text","text":"{\"ok\":null}"}],"structuredContent":{},"_meta":{"ssx/native-structured-text":true}});
    assert!(
        overlay
            .response(&response(1, &result.to_string()))?
            .contains("\"structuredContent\":{\"ok\":null}")
    );
    assert!(!overlay.has_pending());
    for params in [
        "null",
        "[]",
        r#"{"name":1e400,"arguments":{}}"#,
        r#"{"name":"untrusted","arguments":{"x":1e400}}"#,
    ] {
        let request = format!(r#"{{"id":2,"method":"tools/call","params":{params}}}"#);
        let mut overlay = Overlay::default();
        overlay.request(&catalogs, &request)?;
        assert!(!overlay.has_pending());
    }
    Ok(())
}
#[test]
fn native_input_codec_is_opt_in_shape_preserving_and_owns_only_existing_pending_ids() -> Result<()>
{
    use ssx_mcp::opaque::{WireState, native_arguments_for_sdk};
    let output_only = wire_handler::WireHandler::catalogs();
    let mut catalogs = (*output_only).clone();
    catalogs.insert(
        "ssx/native-raw-arguments".into(),
        RawValue::from_string("[\"native_wire_probe\"]".into())?,
    );
    let raw = r#"{"jsonrpc":"2.0","id":5,"method":"tools\u002fcall","extension":{"$serde_json::private::Number":"1e400"},"params":{"name":"native\u005fwire_probe","arguments":{"query":1e400},"_meta":{"literal":null}}}"#;
    assert_eq!(native_arguments_for_sdk(&output_only, raw)?, raw);
    let changed = native_arguments_for_sdk(&catalogs, raw)?;
    let before: BTreeMap<String, Box<RawValue>> = serde_json::from_str(raw)?;
    let after: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&changed)?;
    for key in ["jsonrpc", "id", "method", "extension"] {
        assert_eq!(after[key].get(), before[key].get());
    }
    let params: BTreeMap<String, Box<RawValue>> = serde_json::from_str(after["params"].get())?;
    assert_eq!(params["arguments"].get(), "{}");
    assert_eq!(params["_meta"].get(), r#"{"literal":null}"#);
    for request in [
        r#"{"id":null,"method":"tools/call","params":{"name":"native_wire_probe","arguments":{"x":1e400}}}"#,
        r#"{"id":1,"method":"unknown","params":{"name":"native_wire_probe","arguments":{"x":1e400}}}"#,
        r#"{"id":1,"method":"tools/call","params":{"name":"other","arguments":{"x":1e400}}}"#,
        r#"{"id":1,"method":"tools/call","params":null}"#,
        r#"{"id":1,"method":"tools/call","params":{"name":"native_wire_probe","arguments":null}}"#,
        r#"{"id":1,"method":"tools/call","params":{"name":"native_wire_probe","arguments":[]}}"#,
    ] {
        assert_eq!(native_arguments_for_sdk(&catalogs, request)?, request);
    }
    let mut wire = WireState::default();
    let mut overlay = Overlay::default();
    let mut pending = Vec::new();
    for id in 0..32 {
        let raw = format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"native_wire_probe","arguments":{{"x":1e400}}}}}}"#
        );
        let admitted = wire.request(&raw)?;
        overlay.request(&catalogs, &admitted)?;
        let forwarded = native_arguments_for_sdk(&catalogs, &admitted)?;
        let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&forwarded)?;
        let sdk: rmcp::model::RequestId = serde_json::from_str(fields["id"].get())?;
        assert_eq!(wire.original_request(&sdk)?.as_deref(), Some(raw.as_str()));
        pending.push((raw, admitted, forwarded, sdk));
    }
    assert!(wire.request(&pending[0].0).is_err());
    assert!(wire.request(&request(33)).is_err());
    // Cancellation keeps ownership until the real SDK completion; no duplicate reuse.
    let cancel = wire.request(
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":0}}"#,
    )?;
    assert!(cancel.contains("requestId"));
    assert!(wire.original_request(&pending[0].3)?.is_some());
    assert!(wire.request(&pending[0].0).is_err());
    for (_, admitted, forwarded, sdk) in pending.into_iter().rev() {
        let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&forwarded)?;
        let response = format!(
            r#"{{"jsonrpc":"2.0","id":{},"error":{{"code":-32602,"message":"fixture completion"}}}}"#,
            fields["id"].get()
        );
        let response = overlay.response(&response)?;
        wire.response(&response)?;
        assert!(wire.original_request(&sdk)?.is_none());
        wire.abandon(&admitted);
    }
    assert!(!overlay.has_pending());
    for id in 0..32 {
        let admitted = wire.request(&request(id))?;
        wire.abandon(&admitted);
    }
    Ok(())
}

#[tokio::test]
async fn cancelled_native_generations_retain_capacity_until_drain_and_ignore_late_output()
-> Result<()> {
    use ssx_mcp::opaque::WireState;
    let mut catalogs = (*wire_handler::WireHandler::catalogs()).clone();
    catalogs.insert(
        "ssx/native-raw-arguments".into(),
        RawValue::from_string("[\"native_wire_probe\"]".into())?,
    );
    let mut wire = WireState::default();
    let mut overlay = Overlay::default();
    let notify = wire.completion_notification();
    let mut aliases = Vec::new();
    for id in 0..32 {
        let rewritten = wire.request_for(&catalogs, &request(id))?;
        overlay.request(&catalogs, &rewritten)?;
        let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&rewritten)?;
        aliases.push(fields["id"].get().to_owned());
    }
    assert!(wire.request_for(&catalogs, &request(32)).is_err());
    assert!(wire.request_for(&catalogs, &request(1)).is_err());
    wire.request_for(
        &catalogs,
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"unknown"}}"#,
    )?;
    for id in 0..32 {
        wire.request_for(&catalogs,&format!(r#"{{"jsonrpc":"2.0","method":"notifications/cancelled","params":{{"requestId":{id}}}}}"#))?;
    }
    assert!(
        wire.drain_cancelled_native()?.is_empty(),
        "cancel must not release undrained work"
    );
    assert!(wire.request_for(&catalogs, &request(32)).is_err());
    for alias in &aliases {
        wire.native_handler_complete(&serde_json::from_str(alias)?)?;
    }
    // Notify is retained if completion happens before the pump starts waiting.
    tokio::time::timeout(std::time::Duration::from_millis(100), notify.notified()).await?;
    let ids = wire.drain_cancelled_native()?;
    assert_eq!(ids.len(), 32);
    for id in ids {
        overlay.abandon(&id)?;
    }
    assert!(!overlay.has_pending());
    let stale = |alias: &str| {
        format!(
            r#"{{"jsonrpc":"2.0","id":{alias},"result":{{"_meta":{{"ssx/native-structured-text":true}},"structuredContent":{{}},"content":[{{"type":"text","text":"MALFORMED"}}]}}}}"#
        )
    };
    assert!(wire.stale_native_generation(&stale(&aliases[0]))?);
    assert!(wire.native_response(&stale(&aliases[0]))?.is_none());
    for id in 0..32 {
        let r = wire.request_for(&catalogs, &request(id))?;
        overlay.request(&catalogs, &r)?;
    }
    assert!(
        wire.stale_native_generation(&stale(&aliases[0]))?,
        "external id reuse revived old generation"
    );
    assert!(wire.native_response(&stale(&aliases[0]))?.is_none());
    // A caller may use an old private alias literally. Its admitted ordinary
    // response remains live even though it lies in our issued alias namespace.
    let mut one = WireState::default();
    let r = one.request_for(&catalogs, &request(1))?;
    let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&r)?;
    let alias = fields["id"].get();
    one.response(&format!(r#"{{"id":{alias},"result":null}}"#))?;
    one.request(&format!(r#"{{"id":{alias},"method":"tools/list"}}"#))?;
    let ordinary = format!(r#"{{"id":{alias},"result":{{"tools":[]}}}}"#);
    assert!(!one.stale_native_generation(&ordinary)?);
    assert!(one.native_response(&ordinary)?.is_some());
    // Complete-before-cancel covers a queued SDK result racing cancellation.
    let r = one.request_for(&catalogs, &request(2))?;
    let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&r)?;
    let alias = fields["id"].get();
    one.native_handler_complete(&serde_json::from_str(alias)?)?;
    assert!(one.drain_cancelled_native()?.is_empty());
    one.request(r#"{"method":"notifications/cancelled","params":{"requestId":2}}"#)?;
    assert!(one.stale_native_generation(&stale(alias))?);
    assert!(one.native_response(&stale(alias))?.is_none());
    assert_eq!(one.drain_cancelled_native()?.len(), 1);
    assert!(one.drain_cancelled_native()?.is_empty());
    Ok(())
}

#[path = "t06_support/cancel_wire_handler.rs"]
mod cancel_wire_handler;
fn cancel_probe(id: u64, scenario: &str) -> String {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"native_cancel_probe","arguments":{"scenario":scenario}}}).to_string()
}
const CANCEL_INIT: &str = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"late-output","version":"1"}}}"#;
#[tokio::test]
async fn actual_stdio_pump_discards_late_malformed_sdk_generation_before_and_after_external_id_reuse()
-> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    for after in [false, true] {
        let mut child =
            tokio::process::Command::new(env!("CARGO_BIN_EXE_cancel-wire-qualification"))
                .arg("stdio")
                .env_clear()
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn()?;
        let mut input = child.stdin.take().unwrap();
        let mut output = tokio::io::BufReader::new(child.stdout.take().unwrap());
        let mut stderr = tokio::io::BufReader::new(child.stderr.take().unwrap());
        input
            .write_all(format!("{CANCEL_INIT}\n").as_bytes())
            .await?;
        input.flush().await?;
        let mut line = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            output.read_line(&mut line),
        )
        .await??;
        assert_eq!(serde_json::from_str::<serde_json::Value>(&line)?["id"], 0);
        input
            .write_all(
                format!(
                    "{}\n",
                    cancel_probe(1, if after { "old-after" } else { "old-before" })
                )
                .as_bytes(),
            )
            .await?;
        input.flush().await?;
        line.clear();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            stderr.read_line(&mut line),
        )
        .await??;
        assert_eq!(line.trim(), "T06_STALE_GENERATION_READY");
        for (id, scenario) in [
            (
                if after { 1 } else { 2 },
                if after { "fresh-after" } else { "fresh-before" },
            ),
            (3, "barrier"),
        ] {
            input
                .write_all(format!("{}\n", cancel_probe(id, scenario)).as_bytes())
                .await?;
            input.flush().await?;
            line.clear();
            let n = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                output.read_line(&mut line),
            )
            .await??;
            assert!(n > 0, "late malformed SDK output terminated stdio pump");
            let reply: serde_json::Value = serde_json::from_str(&line)?;
            assert_eq!(reply["id"], id);
            assert_eq!(reply["result"]["structuredContent"]["generation"], scenario);
        }
        input.shutdown().await?;
        drop(input);
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(5), child.wait())
                .await??
                .success()
        );
    }
    Ok(())
}
#[tokio::test]
async fn actual_legacy_pump_discards_late_malformed_sdk_generation_before_and_after_external_id_reuse()
-> Result<()> {
    use ssx_mcp::{harness::Host, legacy::Legacy};
    use std::{sync::Arc, time::Duration};
    use tokio_util::sync::CancellationToken;
    for after in [false, true] {
        let ready = Arc::new(tokio::sync::Semaphore::new(0));
        let r = ready.clone();
        let cancel = CancellationToken::new();
        let legacy = Legacy::with_factory(
            Arc::new(move |wire, _| cancel_wire_handler::CancelWire::new(wire, r.clone())),
            cancel_wire_handler::CancelWire::catalogs(),
            "/messages/",
            Duration::from_secs(60),
            cancel.clone(),
        )?;
        let host = Host::start(legacy.clone().router("/sse")?, cancel).await?;
        let base = format!("http://{}", host.address);
        let client = reqwest::Client::builder().no_proxy().build()?;
        let mut stream = client.get(format!("{base}/sse")).send().await?;
        let mut pending = Vec::new();
        let (_, path) = next_event(&mut stream, &mut pending).await?;
        let url = format!("{base}{path}");
        let post = |body: String| {
            client
                .post(&url)
                .header("content-type", "application/json")
                .body(body)
                .send()
        };
        assert_eq!(post(CANCEL_INIT.into()).await?.status(), 202);
        next_event(&mut stream, &mut pending).await?;
        assert_eq!(
            post(cancel_probe(
                1,
                if after { "old-after" } else { "old-before" }
            ))
            .await?
            .status(),
            202
        );
        tokio::time::timeout(Duration::from_secs(1), ready.acquire())
            .await??
            .forget();
        for (id, scenario) in [
            (
                if after { 1 } else { 2 },
                if after { "fresh-after" } else { "fresh-before" },
            ),
            (3, "barrier"),
        ] {
            assert_eq!(post(cancel_probe(id, scenario)).await?.status(), 202);
            let (_, line) = tokio::time::timeout(
                Duration::from_secs(1),
                next_event(&mut stream, &mut pending),
            )
            .await??;
            let reply: serde_json::Value = serde_json::from_str(&line)?;
            assert_eq!(reply["id"], id);
            assert_eq!(reply["result"]["structuredContent"]["generation"], scenario);
        }
        drop(stream);
        host.shutdown().await?;
        assert_eq!(legacy.available_message_permits(), 32);
        assert_eq!(legacy.available_permits(), 128);
    }
    Ok(())
}

#[test]
fn native_prompt_input_profile_preserves_other_methods_nooptin_and_original_identity() -> Result<()>
{
    use ssx_mcp::opaque::{WireState, native_arguments_for_sdk};
    let output_only = wire_handler::WireHandler::catalogs();
    assert_eq!(
        native_arguments_for_sdk(&output_only, "not JSON")?,
        "not JSON"
    );
    let mut catalogs = (*output_only).clone();
    catalogs.insert(
        "ssx/native-raw-prompts".into(),
        RawValue::from_string(r#"{"names":["known"],"validate_unknown_names":true}"#.into())?,
    );
    let mut wire = WireState::default();
    for (id, name) in (0..32).map(|id| (id, if id % 2 == 0 { "known" } else { "unknown" })) {
        let raw = format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"prompts\u002fget","literal":18446744073709551617,"params":{{"name":"{name}","arguments":{{"x":1e400}},"_meta":{{"literal":null}}}}}}"#
        );
        assert_eq!(native_arguments_for_sdk(&output_only, &raw)?, raw);
        let admitted = wire.request_for(&catalogs, &raw)?;
        let safe = native_arguments_for_sdk(&catalogs, &admitted)?;
        let original: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&admitted)?;
        let changed: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&safe)?;
        assert_eq!(original["id"].get(), changed["id"].get());
        assert_eq!(original["literal"].get(), changed["literal"].get());
        let params: BTreeMap<String, Box<RawValue>> =
            serde_json::from_str(changed["params"].get())?;
        assert_eq!(params["arguments"].get(), "{}");
        assert!(params.contains_key("_meta"));
        let internal: rmcp::model::RequestId = serde_json::from_str(original["id"].get())?;
        assert_eq!(wire.original_request(&internal)?, Some(raw));
    }
    let request = r#"{"id":33,"method":"prompts/get","params":{"name":"known","arguments":null}}"#;
    assert!(wire.request_for(&catalogs, request).is_err());
    for params in [
        "null",
        "[]",
        r#"{"name":null,"arguments":{"x":1e400}}"#,
        r#"{"name":1,"arguments":{"x":1e400}}"#,
    ] {
        let raw = format!(r#"{{"id":1,"method":"prompts/get","params":{params}}}"#);
        assert_eq!(native_arguments_for_sdk(&catalogs, &raw)?, raw);
    }
    let raw = r#"{"id":1,"method":"unknown","params":{"name":"known","arguments":{"x":1e400}}}"#;
    assert_eq!(native_arguments_for_sdk(&catalogs, raw)?, raw);
    for args in ["null", "1", "[]", r#"{"x":1e400}"#] {
        let raw = format!(
            r#"{{"id":1,"method":"prompts/get","params":{{"name":"known","arguments":{args}}}}}"#
        );
        assert!(native_arguments_for_sdk(&catalogs, &raw)?.contains(r#""arguments":{}"#));
    }
    let mut known_only = catalogs.clone();
    known_only.insert(
        "ssx/native-raw-prompts".into(),
        RawValue::from_string(r#"{"names":["known"],"validate_unknown_names":false}"#.into())?,
    );
    let unknown =
        r#"{"id":1,"method":"prompts/get","params":{"name":"unknown","arguments":{"x":1e400}}}"#;
    assert_eq!(native_arguments_for_sdk(&known_only, unknown)?, unknown);
    Ok(())
}
