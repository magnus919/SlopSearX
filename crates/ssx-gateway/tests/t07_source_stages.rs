#![cfg(all(any(unix, windows), feature = "source-endpoint-test-fixtures"))]
#![forbid(unsafe_code)]
use ssx_mcp::gateway_client::resolver::{self, SourceRuntime};
use std::path::{Path, PathBuf};
#[tokio::test(flavor = "current_thread")]
async fn source_endpoint_stages_and_cancelled_preparation_join_original_scope() {
    let root = PathBuf::from(
        std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT")
            .expect("registered private package fixture root"),
    );
    let root = root.join(format!("source-stages-{}", std::process::id()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    #[cfg(windows)]
    resolver::install::prepare_output_root(&root).unwrap();
    let gateway = Path::new(env!("CARGO_BIN_EXE_ssx-gateway"));
    let helper = Path::new(env!("CARGO_BIN_EXE_ssx-gateway-dns-fixture"));
    #[cfg(unix)]
    let package = resolver::install::stage(gateway, helper, &root).unwrap();
    #[cfg(windows)]
    let package = resolver::install::stage(
        gateway,
        helper,
        Path::new(env!("CARGO_BIN_EXE_ssx-gateway-input")),
        &root,
    )
    .unwrap();
    let runtime = SourceRuntime::new(resolver::install::discover(&package).unwrap());
    resolver::source_scope_fixture_controls(runtime)
        .await
        .unwrap();
}
