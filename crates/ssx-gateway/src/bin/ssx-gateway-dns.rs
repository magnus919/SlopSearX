//! Native gateway DNS helper.
#[cfg(any(unix, windows))]
fn main() -> std::io::Result<()> {
    ssx_mcp::gateway_client::resolver::helper_main()
}
#[cfg(not(any(unix, windows)))]
fn main() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "gateway DNS owner is unavailable on this platform",
    ))
}
