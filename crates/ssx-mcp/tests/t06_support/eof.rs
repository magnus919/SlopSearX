//! Causal EOF fixture: every Result path retires each owned resource before return.
use super::{Provider, private_valkey};
use ssx_mcp::Result;
use std::{process::ExitStatus, sync::atomic::Ordering, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

const LINE_CAP: u64 = 16 * 1024;
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Fault {
    None,
    BeforeInitialize,
    AfterProviderReady,
    InvalidInitialize,
    EarlyExit,
    ClosedStdout,
}
struct Outcome {
    primary: Option<String>,
    cleanup: Vec<String>,
    child_status: Option<ExitStatus>,
    provider_retirement_ok: bool,
    store_removed: bool,
    endpoint: String,
    stderr_bytes: usize,
}
impl Outcome {
    fn finish(self, name: &str) -> Result<()> {
        if self.primary.is_none() && self.cleanup.is_empty() {
            return Ok(());
        }
        Err(format!(
            "T06 tool={name}; primary={}; cleanup={:?}; child_status={:?}; provider_retirement_ok={}; store_removed={}; stderr_bytes={}",
            self.primary.as_deref().unwrap_or("none"), self.cleanup, self.child_status,
            self.provider_retirement_ok, self.store_removed, self.stderr_bytes,
        ).into())
    }
}
fn phase(name: &str, phase: &str, reason: impl std::fmt::Display) -> ssx_mcp::Error {
    format!("T06 tool={name} phase={phase}: {reason}").into()
}

async fn initialize(
    name: &str,
    child: &mut tokio::process::Child,
    output: &mut tokio::io::BufReader<tokio::process::ChildStdout>,
) -> Result<()> {
    // Bound allocation as well as time. Never print arbitrary protocol content.
    tokio::time::timeout(Duration::from_secs(1), async {
        let mut line = Vec::new();
        let mut bounded = output.take(LINE_CAP + 1);
        let n = tokio::select! {
            biased;
            status = child.wait() => {
                let status = status.map_err(|e| phase(name, "initialize-child-status", format!("{:?}", e.kind())))?;
                return Err(phase(name, "initialize-early-child-exit", status));
            }
            n = bounded.read_until(b'\n', &mut line) => n
                .map_err(|e| phase(name, "initialize-read", format!("{:?}", e.kind())))?,
        };
        if n == 0 {
            return match child.try_wait() {
                Ok(Some(status)) => Err(phase(name, "initialize-early-child-exit", status)),
                Ok(None) => Err(phase(name, "initialize-stdout-EOF", "child still running")),
                Err(e) => Err(phase(name, "initialize-child-status", format!("{:?}", e.kind()))),
            };
        }
        if n as u64 > LINE_CAP || line.last() != Some(&b'\n') {
            return Err(phase(
                name,
                "initialize-protocol",
                "line exceeds bound or lacks delimiter",
            ));
        }
        let response: serde_json::Value = serde_json::from_slice(&line)
            .map_err(|_| phase(name, "initialize-protocol", "malformed JSON"))?;
        if response["jsonrpc"] != "2.0"
            || response["id"] != 0
            || response.get("error").is_some()
            || !response["result"]["protocolVersion"].is_string()
            || !response["result"]["capabilities"].is_object()
        {
            return Err(phase(
                name,
                "initialize-protocol",
                "unexpected response envelope",
            ));
        }
        Ok(())
    })
    .await
    .map_err(|_| phase(name, "initialize-observation", "timeout (1s)"))?
}

async fn exercise(
    name: &str,
    fault: Fault,
    child: &mut tokio::process::Child,
    input: &mut Option<tokio::process::ChildStdin>,
    output: &mut tokio::io::BufReader<tokio::process::ChildStdout>,
    provider: &Provider,
    server: &private_valkey::Server,
) -> Result<()> {
    if fault == Fault::BeforeInitialize {
        return Err(phase(
            name,
            "injected-before-initialize",
            "negative control",
        ));
    }
    if fault == Fault::EarlyExit {
        // Establish actual child completion before asking the classifier to
        // distinguish it from the separate live-child stdout-EOF control.
        tokio::time::timeout(Duration::from_secs(1), child.wait())
            .await
            .map_err(|_| phase(name, "early-exit-control-setup", "timeout (1s)"))?
            .map_err(|e| phase(name, "early-exit-control-setup", format!("{:?}", e.kind())))?;
        return initialize(name, child, output).await;
    }
    // A valid request with the wrong identity makes the real child reply
    // with id=17; the owner requires its id=0 initialization exchange.
    let init = if fault == Fault::InvalidInitialize {
        r#"{"jsonrpc":"2.0","id":17,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"owned-eof","version":"1"}}}"#
    } else {
        r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"owned-eof","version":"1"}}}"#
    };
    let stdin = input
        .as_mut()
        .ok_or_else(|| phase(name, "initialize-write", "missing stdin"))?;
    stdin
        .write_all(format!("{init}\n").as_bytes())
        .await
        .map_err(|e| phase(name, "initialize-write", format!("{:?}", e.kind())))?;
    stdin
        .flush()
        .await
        .map_err(|e| phase(name, "initialize-flush", format!("{:?}", e.kind())))?;
    initialize(name, child, output).await?;
    let request = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":{"query":"unrelated public query"}}});
    stdin
        .write_all(format!("{request}\n").as_bytes())
        .await
        .map_err(|e| phase(name, "tool-write", format!("{:?}", e.kind())))?;
    stdin
        .flush()
        .await
        .map_err(|e| phase(name, "tool-flush", format!("{:?}", e.kind())))?;
    let mut unexpected = Vec::new();
    let mut bounded = output.take(LINE_CAP + 1);
    tokio::select! {
        biased;
        status = child.wait() => {
            let status = status.map_err(|e| phase(name, "provider-child-status", format!("{:?}", e.kind())))?;
            return Err(phase(name, "provider-early-child-exit", status));
        }
        n = bounded.read_until(b'\n', &mut unexpected) => {
            let n = n.map_err(|e| phase(name, "provider-protocol-read", format!("{:?}", e.kind())))?;
            return Err(phase(name, "provider-protocol-before-readiness",
                format!("unexpected output bytes={n} (cap={LINE_CAP}); request not observed")));
        }
        ready = provider.observed() => ready.map_err(|e| phase(name, "provider-request-readiness", e))?,
    }
    if fault == Fault::AfterProviderReady {
        return Err(phase(
            name,
            "injected-after-provider-readiness",
            "negative control",
        ));
    }
    stdin
        .shutdown()
        .await
        .map_err(|e| phase(name, "stdin-eof", format!("{:?}", e.kind())))?;
    drop(input.take());
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .map_err(|_| phase(name, "eof-child-retirement", "timeout (5s)"))?
        .map_err(|e| phase(name, "eof-child-retirement", format!("{:?}", e.kind())))?;
    if !status.success() {
        return Err(phase(name, "eof-child-status", status));
    }
    // This observes the independently scheduled socket EOF after process exit.
    // It is distinct from the unchanged five-second EOF process contract.
    tokio::time::timeout(Duration::from_secs(1), async {
        while provider.closed.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .map_err(|_| phase(name, "eof-provider-close-observation", "timeout (1s)"))?;
    if provider.closed.load(Ordering::SeqCst) != 1 {
        return Err(phase(
            name,
            "eof-provider-close-count",
            "expected exactly one peer EOF",
        ));
    }
    let dispatched = redis::cmd("KEYS")
        .arg("t06:qualification:dispatch:*")
        .query::<Vec<String>>(&mut server.connection())
        .map_err(|_| phase(name, "dispatch-receipt-read", "private store read failed"))?;
    if !dispatched.is_empty() {
        return Err(phase(
            name,
            "dispatch-receipt-count",
            "EOF dispatched an engine",
        ));
    }
    Ok(())
}

async fn attempt(name: &str, fault: Fault) -> Result<Outcome> {
    let provider = Provider::new().await?;
    let endpoint = provider.endpoint.clone();
    let server = private_valkey::Server::start();
    let store_dir = std::path::Path::new(server.url().strip_prefix("unix://").unwrap())
        .parent()
        .unwrap()
        .to_path_buf();
    let mut child = None;
    let mut input = None;
    let mut stderr = None;
    let primary = async {
        let mut command = if fault == Fault::ClosedStdout {
            // Real owned process with closed stdout and live stdin. Shell
            // built-ins keep it alive without a sleep or a descendant process.
            let mut command = tokio::process::Command::new("/bin/sh");
            command.args(["-c", "exec 1>&-; while IFS= read -r line; do :; done"]);
            command
        } else {
            let mut command =
                tokio::process::Command::new(env!("CARGO_BIN_EXE_search-qualification"));
            command.args([
                if fault == Fault::EarlyExit {
                    "invalid-mode"
                } else {
                    "stdio"
                },
                server.url(),
                &endpoint,
            ]);
            command
        };
        let process = command
            .env_clear()
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| phase(name, "child-spawn", format!("{:?}", e.kind())))?;
        child = Some(process);
        let process = child.as_mut().unwrap();
        input = process.stdin.take();
        let mut output = tokio::io::BufReader::new(process.stdout.take().unwrap());
        let mut err = process.stderr.take().unwrap();
        stderr = Some(tokio::spawn(async move {
            // Drain continuously with fixed memory. Arbitrary stderr is never
            // interpolated into errors (it can contain provider credentials).
            let mut count = 0usize;
            let mut buf = [0; 4096];
            loop {
                match err.read(&mut buf).await {
                    Ok(0) => return Ok(count),
                    Ok(n) => count = count.saturating_add(n),
                    Err(e) => return Err(e.kind()),
                }
            }
        }));
        exercise(
            name,
            fault,
            process,
            &mut input,
            &mut output,
            &provider,
            &server,
        )
        .await
    }
    .await
    .err()
    .map(|e| e.to_string());
    // No early Result return below: preserve the primary failure and attempt
    // all cleanup even when another resource's retirement fails.
    drop(input.take());
    let mut cleanup = Vec::new();
    let mut child_status = None;
    if let Some(mut child) = child {
        let already_reaped = match child.try_wait() {
            Ok(Some(status)) => {
                child_status = Some(status);
                true
            }
            Ok(None) => false,
            Err(e) => {
                cleanup.push(format!("child-status: {:?}", format!("{:?}", e.kind())));
                false
            }
        };
        if !already_reaped {
            if let Err(e) = child.start_kill() {
                cleanup.push(format!("child-kill: {:?}", format!("{:?}", e.kind())));
            }
            match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
                Ok(Ok(status)) => child_status = Some(status),
                Ok(Err(e)) => cleanup.push(format!("child-reap: {:?}", format!("{:?}", e.kind()))),
                Err(_) => cleanup.push("child-reap timeout (5s)".into()),
            }
        }
    }
    let provider_retirement_ok = match provider.close().await {
        Ok(()) => true,
        Err(e) => {
            cleanup.push(format!("provider-retirement: {e}"));
            false
        }
    };
    let mut stderr_bytes = 0;
    if let Some(mut task) = stderr {
        match tokio::time::timeout(Duration::from_secs(2), &mut task).await {
            Ok(Ok(Ok(count))) => stderr_bytes = count,
            Ok(_) => cleanup.push("stderr-reader failed".into()),
            Err(_) => {
                task.abort();
                let _ = task.await;
                cleanup.push("stderr-reader timeout (2s); aborted and joined".into());
            }
        }
    }
    drop(server); // Server::drop kills AND waits before deleting its private directory.
    let store_removed = !store_dir.exists();
    if !store_removed {
        cleanup.push("private-store directory remains".into());
    }
    Ok(Outcome {
        primary,
        cleanup,
        child_status,
        provider_retirement_ok,
        store_removed,
        endpoint,
        stderr_bytes,
    })
}

pub(super) async fn run(name: &str, fault: Fault) -> Result<()> {
    attempt(name, fault).await?.finish(name)
}

pub(super) async fn early_failure_controls() -> Result<()> {
    for name in ["slopsearx_search", "slopsearx_explain_search_scope"] {
        for (fault, expected) in [
            (Fault::BeforeInitialize, "injected-before-initialize"),
            (
                Fault::AfterProviderReady,
                "injected-after-provider-readiness",
            ),
        ] {
            let outcome = attempt(name, fault).await?;
            assert!(outcome.primary.as_deref().unwrap().contains(expected));
            assert!(outcome.cleanup.is_empty(), "{:?}", outcome.cleanup);
            assert!(outcome.child_status.is_some(), "child was not reaped");
            assert!(outcome.provider_retirement_ok && outcome.store_removed);
            let url = url::Url::parse(&outcome.endpoint)?;
            assert!(
                tokio::net::TcpStream::connect(("127.0.0.1", url.port().unwrap()))
                    .await
                    .is_err(),
                "provider listener survived joined teardown"
            );
        }
    }
    Ok(())
}

pub(super) async fn initialization_controls() -> Result<()> {
    for (fault, expected) in [
        (Fault::InvalidInitialize, "initialize-protocol"),
        (Fault::EarlyExit, "initialize-early-child-exit"),
        (Fault::ClosedStdout, "initialize-stdout-EOF"),
    ] {
        let outcome = attempt("slopsearx_search", fault).await?;
        assert!(
            outcome.primary.as_deref().unwrap().contains(expected),
            "{:?}",
            outcome.primary
        );
        assert!(outcome.cleanup.is_empty(), "{:?}", outcome.cleanup);
        assert!(outcome.child_status.is_some());
        assert!(outcome.provider_retirement_ok && outcome.store_removed);
    }
    Ok(())
}

pub(super) async fn incomplete_readiness_controls() -> Result<()> {
    for bytes in [
        b"POST /v1/systemone HTTP/1.1\r\nContent-Length: 200\r\n\r\n{}".to_vec(),
        b"POST /v1/systemone HTTP/1.1\r\nContent-Length: 2\r\n".to_vec(),
        b"POST /v1/systemone HTTP/1.1\r\n\r\n{}".to_vec(),
        b"POST /v1/systemone HTTP/1.1\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}".to_vec(),
        b"POST /v1/systemone HTTP/1.1\r\nContent-Length: 65536\r\n\r\n".to_vec(),
        vec![b'x'; 8 * 1024 + 1],
    ] {
        let provider = Provider::new().await?;
        let observation: Result<_> = async {
            let url = url::Url::parse(&provider.endpoint)?;
            let mut socket =
                tokio::net::TcpStream::connect(("127.0.0.1", url.port().unwrap())).await?;
            socket.write_all(&bytes).await?;
            socket.shutdown().await?;
            let ready = provider.observed().await;
            Ok((ready.is_err(), provider.started.available_permits()))
        }
        .await;
        let retired = provider.close().await;
        let (rejected, permits) = observation.map_err(|primary| {
            format!(
                "incomplete-control setup primary={primary}; cleanup={:?}",
                retired.as_ref().err().map(|e| e.to_string()),
            )
        })?;
        assert!(
            rejected,
            "incomplete/malformed input falsely announced readiness"
        );
        assert_eq!(permits, 0, "invalid input created readiness permit");
        assert!(
            retired.is_err(),
            "protocol failure was hidden during joined teardown"
        );
    }
    // A connected, incomplete peer must also be cancellable without peer EOF.
    let provider = Provider::new().await?;
    let setup: Result<_> = async {
        let url = url::Url::parse(&provider.endpoint)?;
        let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", url.port().unwrap())).await?;
        socket.write_all(b"POST /v1/systemone HTTP/1.1\r\n").await?;
        let read =
            tokio::time::timeout(Duration::from_secs(1), provider.request_read.acquire()).await;
        let read_observed = matches!(&read, Ok(Ok(_)));
        drop(read);
        Ok((socket, read_observed))
    }
    .await;
    let retired = provider.close().await;
    // Teardown precedes returns/assertions even when setup or observation failed.
    let (mut socket, read_observed) = setup.map_err(|primary| {
        format!(
            "incomplete-read cancellation setup primary={primary}; cleanup={:?}",
            retired.as_ref().err().map(|e| e.to_string()),
        )
    })?;
    retired?;
    assert!(
        read_observed,
        "incomplete-read cancellation control never reached actual reader"
    );
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), socket.read(&mut byte)).await??,
        0
    );
    Ok(())
}
