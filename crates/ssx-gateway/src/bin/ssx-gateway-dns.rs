//! Native gateway DNS helper.
#[cfg(unix)]
fn main() -> std::io::Result<()> {
    ssx_mcp::gateway_client::resolver::helper_main()
}
#[cfg(not(unix))]
fn main() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "gateway DNS owner requires Unix",
    ))
}
