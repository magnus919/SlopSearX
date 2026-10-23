//! Native gateway DNS qualification fixture.
#[cfg(unix)]
fn main() -> std::io::Result<()> {
    ssx_mcp::gateway_client::resolver::fixture_main()
}
#[cfg(not(unix))]
fn main() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "gateway DNS owner requires Unix",
    ))
}
