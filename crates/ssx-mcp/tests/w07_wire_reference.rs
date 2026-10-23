//! Actual private HTTP against a measured, frozen CPython/FastMCP wire subset.
#[path = "w07_support/mcp.rs"]
mod fixture;
#[allow(dead_code)] // Preserve the accepted shared fixture, including unused public control methods.
#[path = "../../ssx-infra/tests/w07_support/valkey.rs"]
mod private_valkey;
use ssx_json::{Value, json};
use ssx_mcp::{Result, harness::Host, oauth::NetworkAuth};
use std::{collections::BTreeSet, sync::atomic::Ordering, time::Duration};
use tokio_util::sync::CancellationToken;
async fn rpc(client: &reqwest::Client, url: &str, method: &str, params: Value) -> Result<Value> {
    let response = client
        .post(url)
        .header("content-type", "application/json")
        .header("accept", "application/json,text/event-stream")
        .header("mcp-protocol-version", "2025-11-25")
        .body(ssx_json::to_string(
            &json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}),
        )?)
        .send()
        .await?;
    if response.status() != 200 {
        return Err(format!("native HTTP status: {}", response.status()).into());
    }
    let envelope: Value = ssx_json::from_str(&response.text().await?)?;
    if envelope.get("error").is_some() {
        return Err(format!("native RPC error: {envelope}").into());
    }
    Ok(envelope["result"].clone())
}
#[tokio::test]
async fn measured_python_descriptors_and_negative_calls_match_actual_native_http() -> Result<()> {
    let reference: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w07/wire-reference.json"
    ))?;
    let successor: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w07/wire-reference-packaging263.json"
    ))?;
    for field in ["tools", "full_catalog", "cases"] {
        if reference[field] != successor[field] {
            return Err(format!("packaging reference changed measured {field}").into());
        }
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .default_headers(reqwest::header::HeaderMap::from_iter([(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_static("Bearer w07-wire-reference"),
        )]))
        .build()?;
    let server = private_valkey::Server::start();
    let fixture = fixture::Fixture::new(server.url(), "https://example.invalid/inert");
    let stop = CancellationToken::new();
    let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("w07-wire-reference".into())?);
    let host = match Host::start(fixture.factory.http(auth, stop.clone()), stop).await {
        Ok(host) => host,
        Err(error) => {
            fixture.service.shutdown().await;
            return Err(error);
        }
    };
    let endpoint = format!("http://{}/mcp", host.address);
    let journey = tokio::time::timeout(Duration::from_secs(30), async {
        let catalog = rpc(&client, &endpoint, "tools/list", json!({})).await?;
        let expected = reference["tools"]
            .as_array()
            .ok_or("measured tools missing")?;
        let actual = catalog["tools"].as_array().ok_or("native tools missing")?;
        if !actual.ends_with(expected) {
            return Err("actual native descriptors differ from measured Python tools/list".into());
        }
        let cases = reference["cases"]
            .as_array()
            .ok_or("measured cases missing")?;
        let mut complete = BTreeSet::new();
        for case in cases {
            let id = case["id"]
                .as_str()
                .ok_or("measured case identity missing")?;
            let actual = rpc(
                &client,
                &endpoint,
                "tools/call",
                json!({"name":case["name"],"arguments":case["arguments"]}),
            )
            .await?;
            if actual != case["result"] {
                return Err(format!(
                    "measured wire case {id} differs\nexpected={}\nactual={actual}",
                    case["result"]
                )
                .into());
            }
            if !complete.insert(id) {
                return Err("duplicate measured wire case".into());
            }
            if fixture.dispatches.load(Ordering::SeqCst) != 0 {
                return Err("negative reference case dispatched an engine".into());
            }
            println!("W07_WIRE_CASE_COMPLETE {id}");
        }
        if complete.is_empty() {
            return Err("empty measured wire case replay".into());
        }
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })
    .await;
    // Retire the same owners even when a comparison returns an error.
    let transport = host.shutdown().await;
    fixture.service.shutdown().await;
    transport?;
    journey??;
    Ok(())
}
