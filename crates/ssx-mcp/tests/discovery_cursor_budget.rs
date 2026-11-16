//! Actual HTTP peers exercise typed discovery cursor retention and forwarding.
use axum::{extract::DefaultBodyLimit, response::IntoResponse};
use serde_json::{Value, json};
use ssx_mcp::{Result, gateway::Gateway, harness::Host};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio_util::sync::CancellationToken;
const METHODS: [&str; 4] = [
    "tools/list",
    "resources/list",
    "prompts/list",
    "resources/templates/list",
];
struct Observed {
    pages: [AtomicUsize; 4],
    forwarded: AtomicUsize,
}
async fn peer(target: Option<usize>, escaped: bool) -> Result<(Host, Arc<Observed>)> {
    let observed = Arc::new(Observed {
        pages: std::array::from_fn(|_| AtomicUsize::new(0)),
        forwarded: AtomicUsize::new(0),
    });
    let state = observed.clone();
    let router = axum::Router::new().route("/mcp", axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
        let state = state.clone();
        async move {
            let method = body["method"].as_str().unwrap_or("");
            if method == "notifications/initialized" { return axum::http::StatusCode::ACCEPTED.into_response(); }
            let id = body["id"].clone();
            if method == "initialize" {
                return axum::Json(json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{},"resources":{},"prompts":{}},"serverInfo":{"name":"cursor-budget-peer","version":"1"}}})).into_response();
            }
            let Some(index) = METHODS.iter().position(|s| *s == method) else { return axum::http::StatusCode::BAD_REQUEST.into_response(); };
            let active = target.is_none_or(|target| target == index);
            let cursor = body["params"]["cursor"].as_str();
            let page = cursor.and_then(|s|s.split_once(':')).and_then(|(p,_)|p.parse::<usize>().ok()).unwrap_or(0);
            if active {
                state.pages[index].fetch_add(1, Ordering::SeqCst);
                if cursor.is_some() { state.forwarded.fetch_add(1, Ordering::SeqCst); }
            }
            let key = ["tools", "resources", "prompts", "resourceTemplates"][index];
            let mut result = json!({key: []});
            // Each response and forwarded request is below 4 MiB. Single-surface
            // cursors exceed 16 MiB on page five; the shared case crosses that
            // bound only after earlier catalogs have completed successfully.
            let limit = if target.is_some() { 7 } else { 3 };
            if active && page < limit {
                let wire_bytes = if target.is_some() { ssx_mcp::http_client::MAX_RPC_BYTES - 1024 } else { 2 * 1024 * 1024 - 256 };
                let payload = if escaped { "\n".repeat(wire_bytes / 2) } else { "x".repeat(wire_bytes) };
                result["nextCursor"] = json!(format!("{}:{payload}", page + 1));
            }
            axum::Json(json!({"jsonrpc":"2.0","id":id,"result":result})).into_response()
        }
    })).layer(DefaultBodyLimit::max(ssx_mcp::http_client::MAX_RPC_BYTES));
    Ok((
        Host::start(router, CancellationToken::new()).await?,
        observed,
    ))
}
async fn rejected(target: Option<usize>, escaped: bool) -> Result<Arc<Observed>> {
    let (host, observed) = peer(target, escaped).await?;
    let outcome = Gateway::connect(
        format!("http://{}/mcp", host.address),
        None,
        Duration::from_secs(20),
    )
    .await;
    let error = match outcome {
        Ok(gateway) => {
            gateway.shutdown().await?;
            None
        }
        Err(error) => Some(error.to_string()),
    };
    host.shutdown().await?;
    assert!(
        error
            .as_deref()
            .is_some_and(|e| e.contains("16 MiB aggregate discovery budget")),
        "target={target:?}, escaped={escaped}: {error:?}"
    );
    Ok(observed)
}
#[tokio::test]
async fn every_typed_catalog_charges_cursor_bytes_before_next_request() -> Result<()> {
    for (target, method) in METHODS.iter().enumerate() {
        let observed = rejected(Some(target), false).await?;
        assert_eq!(
            observed.pages[target].load(Ordering::SeqCst),
            5,
            "{} must stop on the first over-budget cursor",
            method
        );
        assert_eq!(
            observed.forwarded.load(Ordering::SeqCst),
            4,
            "over-budget cursor was forwarded"
        );
    }
    Ok(())
}
#[tokio::test]
async fn escaped_cursor_bytes_are_charged_as_serialized_json() -> Result<()> {
    let observed = rejected(Some(0), true).await?;
    assert_eq!(observed.pages[0].load(Ordering::SeqCst), 5);
    assert_eq!(observed.forwarded.load(Ordering::SeqCst), 4);
    Ok(())
}
#[tokio::test]
async fn cursor_budget_is_shared_across_typed_catalogs() -> Result<()> {
    let observed = rejected(None, false).await?;
    assert_eq!(
        observed
            .pages
            .iter()
            .map(|n| n.load(Ordering::SeqCst))
            .collect::<Vec<_>>(),
        vec![4, 4, 3, 0]
    );
    Ok(())
}
#[tokio::test]
async fn repeated_cursor_is_still_rejected_before_resending() -> Result<()> {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let error = ssx_mcp::gateway::collect_pages(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        async { Ok((json!([]), Some("repeat".into()))) }
    })
    .await
    .unwrap_err();
    assert!(error.to_string().contains("repeated catalog cursor"));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    Ok(())
}
