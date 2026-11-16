//! Explicit deterministic wire harness; never public server startup.
#[path = "../../tests/t06_support/wire_handler.rs"]
mod wire_handler;
#[tokio::main]
async fn main() -> ssx_mcp::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [mode, kind] = args.as_slice() else {
        return Err("usage: wire-qualification stdio ordinary|quotes|controls".into());
    };
    if mode != "stdio" || !["ordinary", "quotes", "controls"].contains(&kind.as_str()) {
        return Err("invalid qualification mode".into());
    }
    let handler = wire_handler::WireHandler::new(kind);
    ssx_mcp::opaque::stdio_with_factory_and_limits(
        move |_| handler,
        wire_handler::WireHandler::catalogs(),
        ssx_mcp::output_limits::OutputLimits::new(32 * 1024 * 1024)?,
    )
    .await
}
