//! Actual installed client package: no Python, provider keys or server feature.
#![cfg(feature = "dns-test-fixtures")]
mod platform_support;
use platform_support::{root, stage};
#[test]
fn native_distribution_installs_and_invokes_exact_target_without_backend_runtime() {
    let root = root("install");
    let executable = stage(&root, false);
    let helper = ssx_mcp::gateway_client::resolver::install::discover(&executable).unwrap();
    drop(helper);
    for argument in ["--version", "--help"] {
        let output = std::process::Command::new(&executable)
            .env_clear()
            .arg(argument)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let output = String::from_utf8(output.stdout).unwrap();
        assert!(output.contains(if argument == "--version" {
            env!("CARGO_PKG_VERSION")
        } else {
            "--remote"
        }));
    }
    // Moving the entire prefix retains its relative fixed-role discovery.
    let prefix = executable.parent().unwrap().parent().unwrap();
    let moved = root.join("relocated");
    std::fs::rename(prefix, &moved).unwrap();
    let executable = moved.join("bin").join(if cfg!(windows) {
        "ssx-gateway.exe"
    } else {
        "ssx-gateway"
    });
    assert!(ssx_mcp::gateway_client::resolver::install::discover(&executable).is_ok());
}
