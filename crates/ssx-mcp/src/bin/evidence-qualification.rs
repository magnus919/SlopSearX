//! Explicit private evidence qualification process; never a public startup mode.
#[path = "../../tests/w07_support/mcp.rs"]
mod fixture;
#[path = "../../tests/w07_support/observer.rs"]
mod observer;
use ssx_mcp::Result;
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    sync::atomic::Ordering,
};
#[tokio::main]
async fn main() -> Result<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let [mode, url, source] = arguments.as_slice() else {
        return Err(
            "usage: evidence-qualification stdio PRIVATE_VALKEY_URL INERT_SOURCE_URL".into(),
        );
    };
    if mode != "stdio" {
        return Err("qualification requires stdio".into());
    }
    let socket = std::path::Path::new(
        url.strip_prefix("unix://")
            .ok_or("explicit private Unix socket required")?,
    );
    let parent = socket.parent().ok_or("private socket parent")?;
    let metadata = std::fs::symlink_metadata(parent)?;
    let name = parent
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("private directory")?;
    let id = name
        .strip_prefix("ssx-t06-")
        .ok_or("private fixture directory prefix")?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o777 != 0o700
        || metadata.uid() != std::fs::metadata(std::env::current_exe()?)?.uid()
        || id.len() != 32
        || !id.bytes().all(|b| b.is_ascii_hexdigit())
        || socket.file_name().and_then(|n| n.to_str()) != Some("s.sock")
    {
        return Err("qualification requires an owned exclusive private fixture socket".into());
    }
    let source_url = url::Url::parse(source)?;
    if source_url.scheme() != "http"
        || source_url.host_str() != Some("127.0.0.1")
        || source_url.port().is_none()
        || !source_url.username().is_empty()
        || source_url.password().is_some()
    {
        return Err("explicit inert loopback URL required".into());
    }
    let fixture = fixture::Fixture::new(url, source);
    let surface = fixture.factory.surface().clone();
    let catalogs = surface.runtime_catalogs();
    let stop = tokio_util::sync::CancellationToken::new();
    let result = ssx_mcp::opaque::stdio_with_factory_owned(
        move |wire| observer::ObservedSurface {
            inner: surface.bind_connection(wire.clone(), None),
            wire,
        },
        catalogs,
        stop,
    )
    .await;
    // The overall fixture owner closes the existing service exactly once.
    // The borrowed EvidenceRuntime never gets backend shutdown authority.
    fixture.service.shutdown().await;
    eprintln!(
        "{}",
        serde_json::json!({"w07_owner_shutdown":true,"backend_closed":fixture.backend.is_closed(),"dispatches":fixture.dispatches.load(Ordering::SeqCst)})
    );
    result
}
