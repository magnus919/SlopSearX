//! Synthetic protocol process used by native transport qualification only.
use ssx_mcp::{Registry, Result, opaque::RawGateway};
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let [mode, url] = args.as_slice() {
        if mode != "gateway" {
            return Err("unknown qualification mode".into());
        }
        RawGateway::connect(url.clone(), None, std::time::Duration::from_secs(3))
            .await?
            .stdio()
            .await?;
    } else if args == ["stdio"] {
        ssx_mcp::opaque::stdio(Registry::synthetic()).await?;
    } else if args == ["stdio-stderr"] {
        eprintln!("T01_NATIVE_STDERR_READY");
        ssx_mcp::opaque::stdio(Registry::synthetic()).await?;
    } else if args == ["stdio-opaque"] {
        ssx_mcp::opaque::stdio(Registry::large_catalog()).await?;
    } else {
        return Err("usage: mcp-qualification stdio | gateway URL".into());
    }
    Ok(())
}
