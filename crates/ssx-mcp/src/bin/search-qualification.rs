//! Explicit native T06 qualification process. Fake providers are never public startup defaults.
#[path = "../../tests/t06_support/fixture.rs"]
mod fixture;
#[path = "../../tests/t06_support/observer.rs"]
mod observer;
#[path = "../../tests/t06_support/wall_guard.rs"]
mod wall_guard;
use ssx_mcp::{Result, search_surface::SearchSurface};
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (mode, url, endpoint) = match args.as_slice() {
        [mode, url] => (mode, url, None),
        [mode, url, endpoint] => (mode, url, Some(endpoint)),
        _ => return Err(
            "usage: search-qualification stdio PRIVATE_VALKEY_URL [LOOPBACK_FIXTURE_ROUTING_URL]"
                .into(),
        ),
    };
    if !matches!(mode.as_str(), "stdio" | "stdio-frozen")
        || (mode == "stdio-frozen" && endpoint.is_some())
    {
        return Err("qualification requires stdio".into());
    }
    // Accept only this test suite's exclusive private directory, including a
    // platform-specific temp root. Never derive authority from a URL prefix.
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let socket = std::path::Path::new(
        url.strip_prefix("unix://")
            .ok_or("private Unix socket required")?,
    );
    let parent = socket.parent().ok_or("private socket parent required")?;
    let metadata = std::fs::symlink_metadata(parent)?;
    let name = parent
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("private fixture directory name")?;
    let id = name
        .strip_prefix("ssx-t06-")
        .ok_or("private fixture directory prefix")?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o777 != 0o700
        || metadata.uid() != std::fs::metadata(std::env::current_exe()?)?.uid()
        || id.len() != 32
        || !id.bytes().all(|b| b.is_ascii_hexdigit())
        || socket.file_name().and_then(|s| s.to_str()) != Some("s.sock")
    {
        return Err("qualification requires an owned exclusive private fixture socket".into());
    }
    let dispatch_receipt = std::sync::Arc::new(
        ssx_infra::valkey::Valkey::new(url, std::time::Duration::from_millis(300))
            .map_err(|e| format!("private dispatch receipt store: {e:?}"))?,
    );
    let fixture = if let Some(endpoint) = endpoint {
        let endpoint = url::Url::parse(endpoint)?;
        if endpoint.scheme() != "http"
            || endpoint.host_str() != Some("127.0.0.1")
            || endpoint.port().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
        {
            return Err("fixture routing endpoint must be explicit plain loopback".into());
        }
        let router = std::sync::Arc::new(ssx_infra::jev::JevRouter::with_endpoint(
            ssx_infra::jev::JevConfig::from_environment(&std::collections::BTreeMap::from([
                ("TYPESAFE_API_KEY".into(), "fixture-only".into()),
                ("JEV_TIMEOUT_MS".into(), "10000".into()),
            ]))
            .unwrap(),
            ssx_registry::Registry::builtins()?,
            endpoint.into(),
        ));
        let engines = ["wikipedia", "pubmed"]
            .into_iter()
            .map(|name| fixture::FakeEngine {
                name: name.into(),
                calls: Default::default(),
                dispatch_receipt: Some(dispatch_receipt.clone()),
                count: 3,
                status: ssx_domain::EngineStatus::Ok,
                delay: std::time::Duration::ZERO,
                results: None,
            })
            .collect();
        fixture::Fixture::with_options(
            url,
            Default::default(),
            engines,
            &[],
            ssx_infra::search::SearchConfig {
                max_callers: 1,
                optional_port_timeout: std::time::Duration::from_secs(10),
                ..Default::default()
            },
            Some(router),
        )
        .await
    } else {
        let engines = ["wikipedia", "brave", "duckduckgo"]
            .into_iter()
            .map(|name| {
                Ok(fixture::FakeEngine {
                    name: name.into(),
                    calls: Default::default(),
                    dispatch_receipt: None,
                    count: 3,
                    status: ssx_domain::EngineStatus::Ok,
                    delay: std::time::Duration::ZERO,
                    results: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if mode == "stdio-frozen" {
            fixture::Fixture::frozen(url, false).await
        } else {
            fixture::Fixture::with_engines(url, false, engines).await
        }
    };
    let receipts = endpoint
        .is_none()
        .then(|| observer::DispatchReceipts::new(fixture.engines.clone(), dispatch_receipt));
    let surface = fixture.surface.clone();
    let cancel = tokio_util::sync::CancellationToken::new();
    let serving = ssx_mcp::opaque::stdio_with_factory_owned(
        move |wire| observer::ObservedSurface {
            inner: surface.bind_connection(wire, None),
            receipts,
        },
        SearchSurface::catalogs(),
        cancel.clone(),
    );
    let result = if mode == "stdio-frozen" {
        wall_guard::run_owned(serving, cancel).await
    } else {
        serving.await
    };
    fixture.service.shutdown().await;
    result
}

#[cfg(test)]
#[path = "../../tests/t06_support/server.rs"]
mod private_valkey;
#[cfg(test)]
mod tests {
    use super::*;
    use ssx_application::SearchEngine;
    use ssx_domain::{EngineStatus, SearchRequest};
    use std::{
        collections::BTreeMap,
        sync::{Arc, atomic::Ordering},
        time::Duration,
    };

    #[tokio::test]
    async fn receipt_write_latency_is_outside_the_ready_fake_engine_future() -> Result<()> {
        let mut server = private_valkey::Server::start();
        let store = Arc::new(
            ssx_infra::valkey::Valkey::new(server.url(), Duration::from_millis(300)).unwrap(),
        );
        store.ping().await.unwrap();
        let old = fixture::FakeEngine {
            name: "wikipedia".into(),
            calls: Default::default(),
            dispatch_receipt: Some(store.clone()),
            count: 3,
            status: EngineStatus::Ok,
            delay: Duration::ZERO,
            results: None,
        };
        let ready = fixture::FakeEngine {
            name: "brave".into(),
            calls: Default::default(),
            dispatch_receipt: None,
            ..old.clone()
        };
        // Hold the actual owned store until explicitly released. A timed CLIENT
        // PAUSE could expire during the first phase before publication was polled.
        let suspended = server.suspend();
        let request = SearchRequest::default();
        assert!(
            tokio::time::timeout(Duration::from_millis(1), old.search(&request))
                .await
                .is_err(),
            "old receipt instrumentation must consume the one-millisecond engine budget"
        );
        assert_eq!(old.calls.load(Ordering::SeqCst), 1);
        // The fixed fake provider does not yield to receipt I/O at all.
        use futures_util::FutureExt;
        let response = ready
            .search(&request)
            .now_or_never()
            .expect("ready fake engine must finish in its first poll");
        assert_eq!(response.status, EngineStatus::Ok);
        assert_eq!(response.results.len(), 3);
        assert_eq!(ready.calls.load(Ordering::SeqCst), 1);
        let receipts =
            observer::DispatchReceipts::new(BTreeMap::from([(ready.name.clone(), ready)]), store);
        let mut publication = std::pin::pin!(receipts.publish());
        assert!(
            publication.as_mut().now_or_never().is_none(),
            "publication completed while its owned store was suspended"
        );
        // Prove the child remained stopped throughout both phases; Pending alone
        // could otherwise be ordinary connection I/O rather than the control.
        suspended.assert_stopped();
        suspended.resume();
        publication.await?;
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg("t06:qualification:dispatch:brave:*")
            .query(&mut server.connection())?;
        assert_eq!(
            keys.len(),
            1,
            "actual atomic dispatch observations must reach the parent before reply"
        );
        Ok(())
    }
}
