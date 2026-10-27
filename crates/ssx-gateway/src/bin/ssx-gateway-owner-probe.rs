//! Required-feature qualification subprocess; absent from production packages.
#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    use ssx_windows_owner::{ImageRole, NativeChild, TrustedImage, build_file_digest};
    use tokio::io::AsyncReadExt;
    let executable = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or(std::io::ErrorKind::InvalidInput)?,
    );
    // Complete native package validation precedes this fixture's fixed-role open.
    let _package = ssx_mcp::gateway_client::resolver::install::discover(&executable)?;
    let dns = executable
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or(std::io::ErrorKind::InvalidInput)?
        .join("libexec/slopsearx/ssx-gateway-dns.exe");
    let image = TrustedImage::open(dns.clone(), build_file_digest(&dns)?.sha256, ImageRole::Dns)?;
    let (mut child, mut pipes) = NativeChild::dns(&image)?;
    child.resume()?;
    tokio::time::timeout(std::time::Duration::from_secs(5), pipes.connected()).await??;
    let mut magic = [0; 16];
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        pipes.stdout.read_exact(&mut magic),
    )
    .await??;
    if &magic != b"SSX-RESOLVER-v2\0" {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    // Keep stdin open: child has actually entered, and cannot finish lookup.
    // Parent reports an original handle identity; reviewer opens observation only.
    println!("{} {}", child.pid(), child.fixture_creation_time()?);
    use std::io::Write;
    std::io::stdout().flush()?;
    std::future::pending::<()>().await;
    Ok(())
}
#[cfg(not(windows))]
fn main() -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}
