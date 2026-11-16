//! Explicit transport fault-injection process; never public startup.
#[path = "../../tests/t06_support/cancel_wire_handler.rs"]
mod handler;
#[tokio::main]
async fn main() -> ssx_mcp::Result<()> {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["stdio"] {
        return Err("usage: cancel-wire-qualification stdio".into());
    }
    ssx_mcp::opaque::stdio_with_factory(
        |wire| handler::CancelWire::new(wire, std::sync::Arc::new(tokio::sync::Semaphore::new(0))),
        handler::CancelWire::catalogs(),
    )
    .await
}
