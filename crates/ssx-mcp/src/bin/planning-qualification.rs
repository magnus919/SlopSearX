//! Explicit W03 qualification process; production never selects fake engines.
#[path = "../../tests/w03_support/mod.rs"]
mod support;
use ssx_mcp::Result;
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [mode, url] = args.as_slice() else {
        return Err("usage: planning-qualification stdio PRIVATE_VALKEY_URL".into());
    };
    if mode != "stdio" {
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

    let (registry, fixture) = support::setup(url).await;
    let bound = registry.clone();
    let result = ssx_mcp::opaque::stdio_with_factory(
        move |wire| bound.bind_connection(wire, None),
        ssx_mcp::research_planning::PlanningRegistry::catalogs(),
    )
    .await;
    registry.close().await;
    fixture.service.shutdown().await;
    result
}
