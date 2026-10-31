//! Private installed-client TLS, proxy and redirect witnesses. No live providers.
#![cfg(feature = "dns-test-fixtures")]
#[cfg(unix)]
#[path = "platform_support/mac_trust.rs"]
mod mac_trust;
mod platform_support;
#[path = "platform_support/tls.rs"]
mod tls;
use platform_support::{BUDGET, Process, root, stage};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tls::{Ca, Fixture};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct TrustGuard {
    root: PathBuf,
    active: bool,
}
impl TrustGuard {
    #[cfg(windows)]
    fn windows_command(root: &Path, mode: &str) -> std::process::Command {
        // Match the already configured trust-store.sh Windows branch exactly,
        // bypassing only ambiguous bash lookup (which can select the WSL launcher).
        let mut command = std::process::Command::new("powershell.exe");
        command
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/platform_support/trust-store.ps1"),
            )
            .arg(mode)
            .arg(root);
        command
    }
    #[cfg(windows)]
    fn command(root: &Path, mode: &str) -> std::io::Result<std::process::ExitStatus> {
        Self::windows_command(root, mode).status()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    fn command(root: &Path, mode: &str) -> std::io::Result<std::process::ExitStatus> {
        std::process::Command::new("bash")
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/platform_support/trust-store.sh"),
            )
            .arg(mode)
            .arg(root)
            .status()
    }
    async fn install(root: &Path) -> Self {
        assert_eq!(
            std::env::var("GITHUB_ACTIONS").as_deref(),
            Ok("true"),
            "never mutate a personal trust store"
        );
        assert_eq!(
            std::env::var("SLOPSEARX_PLATFORM_TRUST_FIXTURE").as_deref(),
            Ok("1")
        );
        let guard = Self {
            root: root.into(),
            active: true,
        };
        #[cfg(target_os = "macos")]
        mac_trust::install(root)
            .await
            .expect("bounded owned macOS trust install");
        #[cfg(not(target_os = "macos"))]
        assert!(Self::command(root, "install").unwrap().success());
        guard
    }
    async fn close(mut self) {
        #[cfg(target_os = "macos")]
        mac_trust::remove(&self.root)
            .await
            .expect("bounded exact macOS trust cleanup");
        #[cfg(not(target_os = "macos"))]
        assert!(Self::command(&self.root, "remove").unwrap().success());
        self.active = false;
    }
}
impl Drop for TrustGuard {
    fn drop(&mut self) {
        if self.active {
            #[cfg(target_os = "macos")]
            eprintln!(
                "owned CI certificate cleanup pending at {}; bounded Rust workflow finalizer must retry",
                self.root.display()
            );
            #[cfg(not(target_os = "macos"))]
            {
                let status = Self::command(&self.root, "remove");
                if !status.is_ok_and(|status| status.success()) {
                    eprintln!("owned CI certificate cleanup failed; workflow finalizer must retry");
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[tokio::test(flavor = "current_thread")]
#[ignore = "exact hosted macOS trust cleanup only; never run on personal hosts"]
async fn finalize_exact_owned_macos_trust_fixtures_with_bounded_security_children() {
    mac_trust::finalize()
        .await
        .expect("all exact owned macOS trust fixtures retired");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit dedicated ephemeral hosted trust-store fixture; never run on personal hosts"]
async fn installed_platform_verifier_rejects_private_ca_then_accepts_exact_system_store_and_still_checks_hostname()
 {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        // Inspect the real constructor without spawning PowerShell or touching
        // a trust store. Structured arguments retain spaces and WTF-16 units.
        let units = [67, 58, 92, 32, 0xD800, 92, 0xDC00];
        let owned = PathBuf::from(std::ffi::OsString::from_wide(&units));
        for mode in ["install", "remove"] {
            let command = TrustGuard::windows_command(&owned, mode);
            assert_eq!(
                command.get_program(),
                std::ffi::OsStr::new("powershell.exe")
            );
            let expected: Vec<std::ffi::OsString> = [
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ]
            .into_iter()
            .map(Into::into)
            .chain([
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/platform_support/trust-store.ps1")
                    .into_os_string(),
                mode.into(),
                owned.clone().into_os_string(),
            ])
            .collect();
            assert_eq!(command.get_args().collect::<Vec<_>>(), expected);
            assert_eq!(
                command
                    .get_args()
                    .last()
                    .unwrap()
                    .encode_wide()
                    .collect::<Vec<_>>(),
                units
            );
        }
    }
    let root = root(if cfg!(windows) {
        "tls-store-command with space"
    } else {
        "tls-store"
    });
    let executable = stage(&root, false);
    let ca = Ca::new();
    std::fs::write(root.join("ca.der"), ca.certificate.der()).unwrap();
    std::fs::write(root.join("ca.pem"), ca.certificate.pem()).unwrap();
    let correct = Fixture::tls(&ca, "127.0.0.1", None).await;
    let mut negative = Process::spawn(&executable, &correct.url, &[]);
    assert!(negative.initialize().await.is_none());
    negative.close(false).await;
    assert!(correct.observed.lock().unwrap().is_empty());
    let trust = TrustGuard::install(&root).await;
    let unrelated = Ca::new();
    let additive = root.join("unrelated.pem");
    std::fs::write(&additive, unrelated.certificate.pem()).unwrap();
    let mut merged = Process::spawn(
        &executable,
        &correct.url,
        &[("MCP_REMOTE_CA_BUNDLE", additive.to_str().unwrap())],
    );
    assert!(merged.initialize().await.unwrap().get("result").is_some());
    merged.close(true).await;

    let mut positive = Process::spawn(&executable, &correct.url, &[]);
    assert!(positive.initialize().await.unwrap().get("result").is_some());
    positive.close(true).await;
    let wrong = Fixture::tls(&ca, "wrong-host.invalid", None).await;
    let mut negative = Process::spawn(&executable, &wrong.url, &[]);
    assert!(negative.initialize().await.is_none());
    negative.close(false).await;
    assert!(wrong.observed.lock().unwrap().is_empty());
    trust.close().await;
    let mut removed = Process::spawn(&executable, &correct.url, &[]);
    assert!(removed.initialize().await.is_none());
    removed.close(false).await;
    wrong.close().await;
    correct.close().await;
}

#[tokio::test(flavor = "current_thread")]
async fn installed_additive_ca_refuses_missing_malformed_bundle_and_wrong_hostname() {
    let root = root("tls-bundle");
    let executable = stage(&root, false);
    let ca = Ca::new();
    let pem = root.join("ca.pem");
    std::fs::write(&pem, ca.certificate.pem()).unwrap();
    let bad = root.join("malformed.pem");
    std::fs::write(&bad, b"not a certificate").unwrap();
    let missing = root.join("missing.pem");
    let empty = root.join("empty.pem");
    std::fs::write(&empty, b"").unwrap();
    let oversized = root.join("oversized.pem");
    std::fs::write(&oversized, vec![b' '; 4 * 1024 * 1024 + 1]).unwrap();
    let partial = root.join("partial.pem");
    std::fs::write(
        &partial,
        format!(
            "{}\n-----BEGIN CERTIFICATE-----\n%%%\n-----END CERTIFICATE-----\n",
            ca.certificate.pem()
        ),
    )
    .unwrap();
    let unrelated = Ca::new();
    // Correct second certificate proves the complete collection is retained.
    std::fs::write(
        &pem,
        format!("{}\n{}", unrelated.certificate.pem(), ca.certificate.pem()),
    )
    .unwrap();

    let correct = Fixture::tls(&ca, "127.0.0.1", None).await;
    for path in [&missing, &bad, &empty, &partial, &oversized] {
        let mut process = Process::spawn(
            &executable,
            &correct.url,
            &[("MCP_REMOTE_CA_BUNDLE", path.to_str().unwrap())],
        );
        assert!(process.initialize().await.is_none());
        process.close(false).await;
    }
    assert!(correct.observed.lock().unwrap().is_empty());
    let mut process = Process::spawn(
        &executable,
        &correct.url,
        &[("MCP_REMOTE_CA_BUNDLE", pem.to_str().unwrap())],
    );
    assert!(process.initialize().await.unwrap().get("result").is_some());
    process.close(true).await;
    let wrong = Fixture::tls(&ca, "wrong-host.invalid", None).await;
    let mut process = Process::spawn(
        &executable,
        &wrong.url,
        &[("MCP_REMOTE_CA_BUNDLE", pem.to_str().unwrap())],
    );
    assert!(process.initialize().await.is_none());
    process.close(false).await;
    assert!(wrong.observed.lock().unwrap().is_empty());
    wrong.close().await;
    correct.close().await;
}

#[tokio::test(flavor = "current_thread")]
async fn installed_tls_connect_proxy_keeps_proxy_credentials_out_of_origin_and_bearer_out_of_connect()
 {
    let root = root("tls-proxy");
    let executable = stage(&root, false);
    let ca = Ca::new();
    let pem = root.join("ca.pem");
    std::fs::write(&pem, ca.certificate.pem()).unwrap();
    let fixture = Fixture::tls(&ca, "127.0.0.1", None).await;
    let address = fixture
        .url
        .strip_prefix("https://")
        .unwrap()
        .strip_suffix("/mcp")
        .unwrap()
        .to_owned();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy = format!(
        "http://fixture-proxy:fixture-password@{}",
        listener.local_addr().unwrap()
    );
    let heads = Arc::new(Mutex::new(Vec::new()));
    let record = heads.clone();
    let (stop, mut stopping) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut connections = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                _=&mut stopping=>break,
                joined=connections.join_next(), if !connections.is_empty()=>{joined.unwrap().unwrap();},
                next=listener.accept()=>{
                    let (mut stream,_)=next.unwrap();let record=record.clone();let address=address.clone();
                    connections.spawn(async move {
                        let mut head=Vec::new();
                        while !head.ends_with(b"\r\n\r\n") { assert!(head.len()<65536);head.push(tokio::time::timeout(BUDGET,stream.read_u8()).await.unwrap().unwrap()); }
                        let head=String::from_utf8(head).unwrap();
                        assert!(head.starts_with(&format!("CONNECT {address} HTTP/1.1\r\n")));
                        assert!(!head.to_ascii_lowercase().contains("\r\nauthorization:"));
                        assert!(head.to_ascii_lowercase().contains("\r\nproxy-authorization: basic "));
                        record.lock().unwrap().push(head);
                        let mut origin=tokio::net::TcpStream::connect(&address).await.unwrap();
                        stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await.unwrap();
                        let _=tokio::time::timeout(BUDGET,tokio::io::copy_bidirectional(&mut stream,&mut origin)).await.unwrap();
                    });
                }
            }
        }
        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result {
                assert!(error.is_cancelled());
            }
        }
    });
    let mut process = Process::spawn(
        &executable,
        &fixture.url,
        &[
            ("MCP_REMOTE_CA_BUNDLE", pem.to_str().unwrap()),
            ("https_proxy", &proxy),
            ("no_proxy", ""),
        ],
    );
    assert!(process.initialize().await.unwrap().get("result").is_some());
    process.close(true).await;
    stop.send(()).unwrap();
    tokio::time::timeout(BUDGET, task).await.unwrap().unwrap();
    assert!(!heads.lock().unwrap().is_empty());
    {
        let observed = fixture.observed.lock().unwrap();
        assert!(!observed.is_empty());
        assert!(observed.iter().all(|request| {
            !request
                .head
                .to_ascii_lowercase()
                .contains("proxy-authorization:")
        }));
        assert!(observed.iter().any(|request| {
            request
                .head
                .to_ascii_lowercase()
                .contains("authorization: bearer platform-sentinel")
        }));
    }
    fixture.close().await;
}

#[tokio::test(flavor = "current_thread")]
async fn installed_tls_cross_origin_redirect_is_rejected_without_destination_request() {
    let root = root("tls-redirect");
    let executable = stage(&root, false);
    let ca = Ca::new();
    let pem = root.join("ca.pem");
    std::fs::write(&pem, ca.certificate.pem()).unwrap();
    // The same installed client, CA and ordinary TLS fixture must succeed first.
    let positive = Fixture::tls(&ca, "127.0.0.1", None).await;
    let mut control = Process::spawn(
        &executable,
        &positive.url,
        &[("MCP_REMOTE_CA_BUNDLE", pem.to_str().unwrap())],
    );
    let initialized = control.initialize().await;
    control.close(true).await;
    let positive_reached = !positive.observed.lock().unwrap().is_empty();
    positive.close().await;
    assert!(initialized.is_some_and(|value| value.get("result").is_some()));
    assert!(positive_reached);

    let destination = Fixture::tls(&ca, "127.0.0.1", None).await;
    let origin = Fixture::tls(&ca, "127.0.0.1", Some(destination.url.clone())).await;
    let mut process = Process::spawn(
        &executable,
        &origin.url,
        &[("MCP_REMOTE_CA_BUNDLE", pem.to_str().unwrap())],
    );
    // MCP follows redirects only within the configured scheme/host/port origin.
    assert!(process.initialize().await.is_none());
    process.close(false).await;
    let origin_bearer_observed = origin.observed.lock().unwrap().iter().any(|request| {
        request
            .head
            .to_ascii_lowercase()
            .contains("authorization: bearer platform-sentinel")
    });
    let destination_requests = destination.observed.lock().unwrap().len();
    origin.close().await;
    destination.close().await;
    assert!(origin_bearer_observed);
    assert_eq!(destination_requests, 0);
}
