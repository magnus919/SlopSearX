#![cfg(all(windows, feature = "dns-test-fixtures"))]
#![forbid(unsafe_code)]
use ssx_mcp::gateway_client::resolver::{SourceResolveStage, SourceRuntime};
#[cfg(feature = "windows-owner-test-fixtures")]
use std::io::Write;
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};
fn stage_case(case: &Path, helper: &Path) -> PathBuf {
    let gateway = Path::new(env!("CARGO_BIN_EXE_ssx-gateway"));
    let input = Path::new(env!("CARGO_BIN_EXE_ssx-gateway-input"));
    #[cfg(feature = "windows-owner-test-fixtures")]
    for (role, path) in [("gateway", gateway), ("dns", helper), ("input", input)] {
        match ssx_windows_owner::fixture_build_file_link_count(path) {
            Ok(links) => {
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "T07_WINDOWS_BUILD_INPUT role={role} links={links}"
                );
            }
            Err(error) => {
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "T07_WINDOWS_BUILD_INPUT role={role} io_kind={:?} os_code={:?}",
                    error.kind(),
                    error.raw_os_error()
                );
            }
        }
    }
    ssx_mcp::gateway_client::resolver::install::stage(gateway, helper, input, case).unwrap()
}
fn runtime(label: &str) -> SourceRuntime {
    let root = PathBuf::from(
        std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT")
            .expect("controller-owned private package root"),
    );
    let case = root.join(format!("windows-{label}-{}", std::process::id()));
    ssx_mcp::gateway_client::resolver::install::prepare_output_root(&case).unwrap();
    let installed = stage_case(
        &case,
        Path::new(env!("CARGO_BIN_EXE_ssx-gateway-dns-fixture")),
    );
    let helper = ssx_mcp::gateway_client::resolver::install::discover(&installed).unwrap();
    SourceRuntime::new(helper)
}
async fn entered(runtime: &SourceRuntime) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if !runtime.ready_process_groups().is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test(flavor = "current_thread")]
async fn windows_numeric_source_ports_use_exact_proactor_mask_without_lookup_child() {
    let owner = runtime("mask");
    for (host, decimal, expected) in [
        ("127.0.0.1", "65536", 0),
        ("::1", "65537", 1),
        ("127.0.0.1", "-1", 65535),
        ("::1", "170141183460469231731687303715884105728", 0),
    ] {
        let rows = owner
            .resolve(host, decimal, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(rows[0].port(), expected);
        assert!(owner.ready_process_groups().is_empty());
    }
    let huge = format!("1{}", "0".repeat(4299));
    assert_eq!(
        owner
            .resolve("127.0.0.1", &huge, Duration::from_secs(1))
            .await
            .unwrap()[0]
            .port(),
        0
    );
    owner.close().await.unwrap();
    assert_eq!(owner.ownership_snapshot(), (true, 0, vec![]));
}
#[tokio::test(flavor = "current_thread")]
async fn windows_dropped_lookup_joins_original_helper_job_without_input_eof() {
    let owner = runtime("drop");
    let lookup = owner.clone();
    let task = tokio::spawn(async move {
        lookup
            .resolve("hold.invalid", "80", Duration::from_secs(30))
            .await
    });
    entered(&owner).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    owner.close().await.unwrap();
    assert_eq!(owner.ownership_snapshot(), (true, 0, vec![]));
}
#[tokio::test(flavor = "current_thread")]
async fn windows_dropped_close_future_retains_registered_original_owner_for_retry() {
    let owner = runtime("close");
    let lookup = owner.clone();
    let task = tokio::spawn(async move {
        lookup
            .resolve("hold.invalid", "80", Duration::from_secs(30))
            .await
    });
    entered(&owner).await;
    let mut close = Box::pin(owner.close());
    // The held child cannot finish spontaneously. On this current-thread
    // runtime the supervisor cannot consume close's cancellation during this
    // first poll, so Pending is an actual finite checkpoint, not a sleep.
    std::future::poll_fn(|cx| {
        assert!(Pin::new(&mut close).poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(close);
    owner.close().await.unwrap();
    assert!(task.await.unwrap().is_err());
    assert_eq!(owner.ownership_snapshot(), (true, 0, vec![]));
}
#[tokio::test(flavor = "current_thread")]
async fn windows_protocol_failure_and_inherited_descendant_do_not_escape_job_retirement() {
    for host in [
        "malformed.invalid",
        "oversize.invalid",
        "stderr-oversize.invalid",
        "crash.invalid",
        "empty.invalid",
        "descendant.invalid",
    ] {
        let owner = runtime(host);
        let error = owner
            .resolve(host, "80", Duration::from_millis(100))
            .await
            .unwrap_err();
        assert!(matches!(
            error.stage,
            SourceResolveStage::Helper
                | SourceResolveStage::OperationalLimit
                | SourceResolveStage::EmptyAddresses
                | SourceResolveStage::TimedOut
        ));
        owner.close().await.unwrap();
        assert_eq!(owner.ownership_snapshot(), (true, 0, vec![]));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn windows_actual_input_idle_eof_backpressure_and_dropped_close_join_original_job() {
    use ssx_mcp::gateway_client::resolver::{WindowsInputControl, windows_input_fixture_control};
    let root = PathBuf::from(
        std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT")
            .expect("controller-owned private package root"),
    );
    let case = root.join(format!("windows-input-{}", std::process::id()));
    ssx_mcp::gateway_client::resolver::install::prepare_output_root(&case).unwrap();
    let executable = stage_case(&case, Path::new(env!("CARGO_BIN_EXE_ssx-gateway-dns")));
    let prefix = executable.parent().unwrap().parent().unwrap();
    let record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(prefix.join("share/slopsearx/native-install.json")).unwrap(),
    )
    .unwrap();
    let text = record["input_helper_sha256"].as_str().unwrap();
    let mut sha = [0u8; 32];
    for (i, b) in sha.iter_mut().enumerate() {
        *b = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap();
    }
    let image =
        ssx_mcp::gateway_client::resolver::install::input_image_for_fixture(&executable, sha)
            .unwrap();
    for control in [
        WindowsInputControl::Idle,
        WindowsInputControl::Eof,
        WindowsInputControl::Backpressure,
        WindowsInputControl::DroppedClose,
    ] {
        windows_input_fixture_control(image.clone(), control)
            .await
            .unwrap();
    }
}
