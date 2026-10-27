//! Private installed-client TLS, proxy and redirect witnesses. No live providers.
#![cfg(feature = "dns-test-fixtures")]
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
    fn command(root: &Path, mode: &str) -> std::io::Result<std::process::ExitStatus> {
        std::process::Command::new("bash")
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/platform_support/trust-store.sh"),
            )
            .arg(mode)
            .arg(root)
            .status()
    }
    fn install(root: &Path) -> Self {
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
        assert!(Self::command(root, "install").unwrap().success());
        guard
    }
    fn close(mut self) {
        assert!(Self::command(&self.root, "remove").unwrap().success());
        self.active = false;
    }
}
impl Drop for TrustGuard {
    fn drop(&mut self) {
        if self.active {
            let status = Self::command(&self.root, "remove");
            if !status.is_ok_and(|status| status.success()) {
                eprintln!("owned CI certificate cleanup failed; workflow finalizer must retry");
            }
        }
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires explicit dedicated ephemeral hosted trust-store fixture; never run on personal hosts"]
async fn installed_platform_verifier_rejects_private_ca_then_accepts_exact_system_store_and_still_checks_hostname()
 {
    let root = root("tls-store");
    let executable = stage(&root, false);
    let ca = Ca::new();
    std::fs::write(root.join("ca.der"), ca.certificate.der()).unwrap();
    std::fs::write(root.join("ca.pem"), ca.certificate.pem()).unwrap();
    let correct = Fixture::tls(&ca, "127.0.0.1", None).await;
    let mut negative = Process::spawn(&executable, &correct.url, &[]);
    assert!(negative.initialize().await.is_none());
    negative.close(false).await;
    assert!(correct.observed.lock().unwrap().is_empty());
    let trust = TrustGuard::install(&root);
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
    trust.close();
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
