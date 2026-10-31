#![cfg(all(unix, feature = "dns-test-fixtures"))]
#![deny(unsafe_code)]
use sha2::{Digest, Sha256};
use ssx_mcp::gateway_client::resolver::{SourceResolveStage, SourceResolverHelper, SourceRuntime};
use std::{future::Future, path::PathBuf, time::Duration};
fn runtime() -> SourceRuntime {
    let path = PathBuf::from(env!("CARGO_BIN_EXE_ssx-gateway-dns-fixture"));
    let digest: [u8; 32] = Sha256::digest(std::fs::read(&path).unwrap()).into();
    SourceRuntime::new(SourceResolverHelper::new(path, digest).unwrap())
}

use futures_util::FutureExt;
async fn owned<F, Fut>(body: F)
where
    F: FnOnce(SourceRuntime) -> Fut,
    Fut: Future<Output = ()>,
{
    owned_with(runtime(), body).await;
}
async fn owned_with<F, Fut>(runtime: SourceRuntime, body: F)
where
    F: FnOnce(SourceRuntime) -> Fut,
    Fut: Future<Output = ()>,
{
    let observed = std::panic::AssertUnwindSafe(body(runtime.clone()))
        .catch_unwind()
        .await;
    let closed = runtime.close().await;
    // Resume assertions only after every retained Child/supervisor was joined.
    if let Err(panic) = observed {
        std::panic::resume_unwind(panic);
    }
    closed.expect("owned DNS fixture close must prove retirement");
}
async fn spawned(runtime: &SourceRuntime) -> u32 {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(pid) = runtime.ready_process_groups().first() {
                return *pid;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}
fn absent(pid: u32) -> bool {
    matches!(
        nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(pid as i32),
            None::<nix::sys::signal::Signal>
        ),
        Err(nix::errno::Errno::ESRCH)
    )
}
#[tokio::test]
async fn request_abort_cancels_held_helper_and_close_reaps_group() {
    owned(|runtime| async move {
        let request_runtime = runtime.clone();
        let request = tokio::spawn(async move {
            request_runtime
                .resolve("hold.invalid", "65536", Duration::from_secs(60))
                .await
        });
        let pid = spawned(&runtime).await;
        request.abort();
        let _ = request.await;
        runtime.close().await.unwrap();
        assert!(absent(pid));
        assert_eq!(runtime.ownership_snapshot().1, 0);
    })
    .await;
}
#[tokio::test]
async fn outer_request_owner_abort_retains_independent_runtime_close_obligation() {
    owned(|runtime| async move {
        let owner_runtime = runtime.clone();
        let outer = tokio::spawn(async move {
            owner_runtime
                .resolve("::1%hold", "65536", Duration::from_secs(60))
                .await
        });
        let pid = spawned(&runtime).await;
        outer.abort();
        let _ = outer.await;
        runtime.close().await.unwrap();
        assert!(absent(pid));
    })
    .await;
}
#[tokio::test]
async fn prespawn_admission_and_close_do_not_leave_untracked_child() {
    // First poll admits+registers the supervisor, but this current-thread runtime
    // cannot poll that spawned task until the parent yields. Close cancels first.
    owned(|runtime| async move {
        let mut request =
            Box::pin(runtime.resolve("hold.invalid", "65536", Duration::from_secs(60)));
        std::future::poll_fn(|cx| {
            assert!(request.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        assert_eq!(runtime.ownership_snapshot().1, 1);
        assert!(runtime.ownership_snapshot().2.is_empty());
        runtime.close().await.unwrap();
        assert_eq!(
            request.await.unwrap_err().stage,
            SourceResolveStage::Cancelled
        );
        assert_eq!(runtime.ownership_snapshot().1, 0);
    })
    .await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_registration_close_and_close_callers_are_owned() {
    owned(|runtime| async move {
        use std::sync::Arc;
        use tokio::sync::{Barrier, oneshot};
        // Both workers exist before the controller releases admission. The
        // acknowledged first poll commits a real row under the production gate;
        // close cannot win accidentally because spawned tasks were never polled.
        let admission = Arc::new(Barrier::new(2));
        let (registered, registration) = oneshot::channel();
        let before_runtime = runtime.clone();
        let before_admission = admission.clone();
        let before = tokio::spawn(async move {
            before_admission.wait().await;
            let mut request =
                Box::pin(before_runtime.resolve("hold.invalid", "65536", Duration::from_secs(60)));
            std::future::poll_fn(|cx| {
                assert!(request.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            registered.send(()).unwrap();
            request.await
        });
        admission.wait().await;
        registration.await.unwrap();
        let pid = spawned(&runtime).await;
        assert_eq!(runtime.ownership_snapshot().1, 1);
        // A second spawned worker waits for close's actual admission commit,
        // rather than a yield/sleep intended to influence scheduler order.
        let closing = Arc::new(Barrier::new(2));
        let after_runtime = runtime.clone();
        let after_closing = closing.clone();
        let after = tokio::spawn(async move {
            after_closing.wait().await;
            after_runtime
                .resolve("hold.invalid", "65536", Duration::from_secs(60))
                .await
        });
        let closer_runtime = runtime.clone();
        let (closed_admission, admission_closed) = oneshot::channel();
        let (second_polled, second_entered) = oneshot::channel();
        let first = tokio::spawn(async move {
            let mut close = Box::pin(closer_runtime.close());
            let first_poll =
                std::future::poll_fn(|cx| std::task::Poll::Ready(close.as_mut().poll(cx))).await;
            assert!(closer_runtime.ownership_snapshot().0);
            closed_admission.send(()).unwrap();
            second_entered.await.unwrap();
            closing.wait().await;
            match first_poll {
                std::task::Poll::Ready(result) => result,
                std::task::Poll::Pending => close.await,
            }
        });
        admission_closed.await.unwrap();
        // This caller competes for the join owner while the first close owns
        // the same retained registry. Neither may overwrite/drop a join handle.
        let other_runtime = runtime.clone();
        let second = tokio::spawn(async move {
            let mut close = Box::pin(other_runtime.close());
            let first_poll =
                std::future::poll_fn(|cx| std::task::Poll::Ready(close.as_mut().poll(cx))).await;
            second_polled.send(()).unwrap();
            match first_poll {
                std::task::Poll::Ready(result) => result,
                std::task::Poll::Pending => close.await,
            }
        });
        first.await.unwrap().unwrap();
        second.await.unwrap().unwrap();
        assert_eq!(
            before.await.unwrap().unwrap_err().stage,
            SourceResolveStage::Cancelled
        );
        assert_eq!(
            after.await.unwrap().unwrap_err().stage,
            SourceResolveStage::Closed
        );
        assert!(absent(pid));
        assert_eq!(runtime.ownership_snapshot(), (true, 0, Vec::new()));
    })
    .await;
}
#[tokio::test]
async fn close_future_abort_keeps_join_registry_for_next_close() {
    owned(|runtime| async move {
        let r = runtime.clone();
        let request = tokio::spawn(async move {
            r.resolve("hold.invalid", "65536", Duration::from_secs(60))
                .await
        });
        let pid = spawned(&runtime).await;
        let closer = runtime.clone();
        let close = tokio::spawn(async move { closer.close().await });
        tokio::task::yield_now().await;
        assert!(!close.is_finished());
        close.abort();
        let _ = close.await;
        runtime.close().await.unwrap();
        let _ = request.await;
        assert!(absent(pid));
    })
    .await;
}
#[tokio::test]
async fn helper_crash_malformed_oversize_and_timeout_all_retire() {
    for (host, expected) in [
        ("crash.invalid", SourceResolveStage::Helper),
        ("malformed.invalid", SourceResolveStage::Helper),
        ("oversize.invalid", SourceResolveStage::OperationalLimit),
        (
            "stderr-oversize.invalid",
            SourceResolveStage::OperationalLimit,
        ),
        ("hold.invalid", SourceResolveStage::TimedOut),
        ("descendant.invalid", SourceResolveStage::TimedOut),
    ] {
        owned(|runtime| async move {
            assert_eq!(
                runtime
                    .resolve(host, "65536", Duration::from_secs(2))
                    .await
                    .unwrap_err()
                    .stage,
                expected
            );
            runtime.close().await.unwrap();
            assert_eq!(runtime.ownership_snapshot().1, 0);
        })
        .await;
    }
}
#[tokio::test]
async fn multiple_held_lookups_shutdown_cancels_and_reaps_every_row() {
    owned(|runtime| async move {
        let mut requests = Vec::new();
        for _ in 0..6 {
            let r = runtime.clone();
            requests.push(tokio::spawn(async move {
                r.resolve("hold.invalid", "65536", Duration::from_secs(60))
                    .await
            }));
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while runtime.ready_process_groups().len() != 6 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let groups = runtime.ownership_snapshot().2;
        runtime.close().await.unwrap();
        for request in requests {
            assert_eq!(
                request.await.unwrap().unwrap_err().stage,
                SourceResolveStage::Cancelled
            );
        }
        assert!(groups.into_iter().all(absent));
        assert_eq!(runtime.ownership_snapshot().1, 0);
    })
    .await;
}
#[tokio::test]
async fn numeric_skips_helper_but_hostname_lookup_precedes_original_port_failure() {
    owned(|runtime| async move {
        assert_eq!(
            runtime
                .resolve("127.0.0.1", "65536", Duration::from_secs(2))
                .await
                .unwrap_err()
                .stage,
            SourceResolveStage::SocketPort
        );
        assert_eq!(
            runtime
                .resolve("::1", "2147483648", Duration::from_secs(2))
                .await
                .unwrap_err()
                .stage,
            SourceResolveStage::SocketCInt
        );
        assert_eq!(runtime.ownership_snapshot().1, 0);
        assert_eq!(
            runtime
                .resolve("gai-error.invalid", "2147483648", Duration::from_secs(2))
                .await
                .unwrap_err()
                .stage,
            SourceResolveStage::AnyIoGetAddrInfo
        );
        assert_eq!(
            runtime
                .resolve("good.invalid", "2147483648", Duration::from_secs(2))
                .await
                .unwrap_err()
                .stage,
            SourceResolveStage::SocketCInt
        );
        assert_eq!(
            runtime
                .resolve("empty.invalid", "65536", Duration::from_secs(2))
                .await
                .unwrap_err()
                .stage,
            SourceResolveStage::EmptyAddresses
        );
        runtime.close().await.unwrap();
    })
    .await;
}
#[tokio::test]
async fn scoped_ipv6_retains_returned_ports_flow_scope_and_sequence() {
    owned(|runtime| async move {
        let addresses = runtime
            .resolve("::1%fixture", "65536", Duration::from_secs(2))
            .await
            .unwrap();
        assert_eq!(
            addresses[0],
            "[::1%4]:7"
                .parse::<std::net::SocketAddr>()
                .map(|mut a| {
                    if let std::net::SocketAddr::V6(ref mut v6) = a {
                        v6.set_flowinfo(17);
                    }
                    a
                })
                .unwrap()
        );
        assert_eq!(
            addresses[1],
            "127.0.0.1:9".parse::<std::net::SocketAddr>().unwrap()
        );
        runtime.close().await.unwrap();
    })
    .await;
}
#[test]
fn native_artifact_identity_is_explicit_and_digest_checked() {
    let path = PathBuf::from(env!("CARGO_BIN_EXE_ssx-gateway-dns-fixture"));
    assert!(SourceResolverHelper::new(path, [0; 32]).is_err());
    assert!(SourceResolverHelper::new(PathBuf::from("ssx-gateway-dns"), [0; 32]).is_err());
}

#[tokio::test]
async fn pipe_eof_before_normal_exit_preserves_original_success() {
    owned(|runtime| async move {
        let result = runtime
            .resolve("::1%delayed", "65536", Duration::from_secs(2))
            .await;
        assert!(
            result.is_ok(),
            "EOF is not a substitute for successful leader exit"
        );
        runtime.close().await.unwrap();
        assert_eq!(runtime.ownership_snapshot().1, 0);
    })
    .await;
}
#[tokio::test]
async fn pipe_eof_while_leader_lives_obeys_lookup_budget_then_retires() {
    owned(|runtime| async move {
        let r = runtime.clone();
        let request = tokio::spawn(async move {
            r.resolve("::1%eof-hold", "65536", Duration::from_millis(50))
                .await
        });
        let pid = spawned(&runtime).await;
        assert_eq!(
            request.await.unwrap().unwrap_err().stage,
            SourceResolveStage::TimedOut
        );
        runtime.close().await.unwrap();
        assert!(absent(pid));
    })
    .await;
}
#[tokio::test]
async fn pipe_conversion_failure_keeps_exact_spawned_child_until_reaped() {
    let path = PathBuf::from(env!("CARGO_BIN_EXE_ssx-gateway-dns-fixture"));
    let digest: [u8; 32] = Sha256::digest(std::fs::read(&path).unwrap()).into();
    let helper = SourceResolverHelper::new(path, digest)
        .unwrap()
        .inject_pipe_conversion_failure_for_test();
    let runtime = SourceRuntime::new(helper);
    owned_with(runtime, |runtime| async move {
        let r = runtime.clone();
        let request = tokio::spawn(async move {
            r.resolve("hold.invalid", "65536", Duration::from_secs(2))
                .await
        });
        let pid = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(pid) = runtime.ownership_snapshot().2.first() {
                    break *pid;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            request.await.unwrap().unwrap_err().stage,
            SourceResolveStage::Helper
        );
        runtime.close().await.unwrap();
        assert!(absent(pid));
    })
    .await;
}

#[tokio::test]
async fn repeated_close_after_retirement_keeps_group_absence_and_closed_admission() {
    owned(|runtime| async move {
        assert!(
            runtime
                .resolve("::1%fixture", "65536", Duration::from_secs(2))
                .await
                .is_ok()
        );
        runtime.close().await.unwrap();
        runtime.close().await.unwrap();
        assert_eq!(runtime.ownership_snapshot(), (true, 0, Vec::new()));
        assert_eq!(
            runtime
                .resolve("hold.invalid", "65536", Duration::from_secs(2))
                .await
                .unwrap_err()
                .stage,
            SourceResolveStage::Closed
        );
    })
    .await;
}

#[tokio::test]
async fn reqwest_hook_preserves_native_sockaddr_sequence_scope_flow_and_ports() {
    use reqwest::dns::Resolve;
    use ssx_mcp::gateway_client::resolver::GatewayResolver;
    let path = PathBuf::from(env!("CARGO_BIN_EXE_ssx-gateway-dns-fixture"));
    let digest: [u8; 32] = Sha256::digest(std::fs::read(&path).unwrap()).into();
    let resolver = GatewayResolver::new(
        SourceResolverHelper::new(path, digest).unwrap(),
        Duration::from_secs(2),
    );
    owned_with(resolver.runtime().clone(), |_| async move {
        let addresses: Vec<_> = resolver
            .resolve("adapter.invalid".parse().unwrap())
            .await
            .unwrap()
            .collect();
        resolver.close().await.unwrap();
        let std::net::SocketAddr::V6(first) = addresses[0] else {
            panic!("IPv6 order");
        };
        assert_eq!(
            (first.port(), first.flowinfo(), first.scope_id()),
            (7, 17, 4)
        );
        assert_eq!(
            addresses[1],
            "127.0.0.1:9".parse::<std::net::SocketAddr>().unwrap()
        );
    })
    .await;
}
#[tokio::test]
async fn dropping_reqwest_lookup_cancels_original_child_and_explicit_close_joins() {
    use reqwest::dns::Resolve;
    use ssx_mcp::gateway_client::resolver::GatewayResolver;
    let path = PathBuf::from(env!("CARGO_BIN_EXE_ssx-gateway-dns-fixture"));
    let digest: [u8; 32] = Sha256::digest(std::fs::read(&path).unwrap()).into();
    let resolver = GatewayResolver::new(
        SourceResolverHelper::new(path, digest).unwrap(),
        Duration::from_secs(60),
    );
    owned_with(resolver.runtime().clone(), |runtime| async move {
        let pending = resolver.resolve("hold.invalid".parse().unwrap());
        let request = tokio::spawn(pending);
        let pid = spawned(&runtime).await;
        request.abort();
        let _ = request.await;
        resolver.close().await.unwrap();
        assert!(absent(pid));
        assert_eq!(runtime.ownership_snapshot().1, 0);
    })
    .await;
}

// Real executable cases reuse the native helper protocol; no external DNS or
// provider is contacted. Each HTTP response closes its connection so the held
// lookup is reached during a live RPC after successful gateway initialization.
#[derive(Clone, Copy)]
enum ProcessStop {
    CancelRequest,
    Eof,
    Sigterm,
    EofDeleteLookup,
    SigtermDeleteLookup,
}
async fn process_case(stop: ProcessStop) {
    use axum::{Router, extract::State, response::IntoResponse, routing::post};
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let delete_lookup = matches!(
        stop,
        ProcessStop::EofDeleteLookup | ProcessStop::SigtermDeleteLookup
    );
    #[derive(Clone)]
    struct Peer {
        session: bool,
        methods: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }
    async fn mcp(State(peer): State<Peer>, body: axum::body::Bytes) -> axum::response::Response {
        let value: Value = serde_json::from_slice(&body).unwrap();
        let method = value["method"].as_str().unwrap();
        peer.methods.lock().unwrap().push(method.to_owned());
        if method.starts_with("notifications/") {
            return (axum::http::StatusCode::ACCEPTED, [("connection", "close")]).into_response();
        }
        let result = match method {
            "initialize" => {
                json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"owned-dns-native-peer","version":"1"}})
            }
            "tools/list" => {
                json!({"tools":[{"name":"echo","inputSchema":{"type":"object","properties":{"query":{"type":"integer"}},"required":["query"]}}]})
            }
            "tools/call" => {
                json!({"content":[{"type":"text","text":"ok"}],"structuredContent":value["params"]["arguments"]})
            }
            _ => panic!("unexpected native fixture method"),
        };
        let mut response = (
            [("connection", "close")],
            axum::Json(json!({"jsonrpc":"2.0","id":value["id"],"result":result})),
        )
            .into_response();
        // The cleanup cases establish a real session, then close each socket so
        // final DELETE has to enter a fresh ordinary hostname resolution.
        if peer.session && method == "initialize" {
            response
                .headers_mut()
                .insert("mcp-session-id", "dns-cleanup-session".parse().unwrap());
        }
        response
    }
    let root = PathBuf::from(
        std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT")
            .expect("preregistered private native fixture root"),
    );
    assert!(root.is_absolute() && root.is_dir());
    let case = root.join(format!(
        "dns-process-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&case).unwrap();
    let control = case.join("control");
    std::fs::write(&control, b"pass").unwrap();
    let installed = ssx_mcp::gateway_client::resolver::install::stage(
        std::path::Path::new(env!("CARGO_BIN_EXE_ssx-gateway")),
        std::path::Path::new(env!("CARGO_BIN_EXE_ssx-gateway-dns-fixture")),
        &root,
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let close = tokio_util::sync::CancellationToken::new();
    let signal = close.clone();
    let peer = Peer {
        session: delete_lookup,
        methods: Default::default(),
    };
    let observed_peer = peer.clone();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/mcp", post(mcp)).with_state(peer),
        )
        .with_graceful_shutdown(signal.cancelled_owned())
        .await
    });
    let mut child = tokio::process::Command::new(installed)
        .env_clear()
        .env("no_proxy", "*")
        .env("SLOPSEARX_DNS_FIXTURE_CONTROL", &control)
        .args([
            "--remote",
            &format!("http://gateway.fixture.invalid:{port}/mcp"),
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take();
    let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
    let stderr = child.stderr.take().unwrap();
    let stderr = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr
            .take(65537)
            .read_to_end(&mut bytes)
            .await
            .map(|_| bytes)
    });
    let mut terminal = None;
    let mut helper = None;
    let outcome = std::panic::AssertUnwindSafe(async {
        let init=json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"owned-dns-client","version":"1"}}});
        input.as_mut().unwrap().write_all(format!("{init}\n").as_bytes()).await.unwrap();
        let reply=tokio::time::timeout(Duration::from_secs(10),output.next_line()).await.unwrap().unwrap().unwrap();
        assert!(serde_json::from_str::<Value>(&reply).unwrap()["result"].is_object());
        assert_eq!(*observed_peer.methods.lock().unwrap(), ["initialize", "notifications/initialized", "tools/list"]);
        std::fs::write(&control,b"hold-once").unwrap();
        let stopped_at = tokio::time::Instant::now();
        if delete_lookup {
            // No new RPC is sent. With initialization/discovery already done,
            // the only remaining upstream request is the session DELETE.
            match stop {
                ProcessStop::EofDeleteLookup => { input.take(); },
                ProcessStop::SigtermDeleteLookup => {
                    nix::sys::signal::kill(nix::unistd::Pid::from_raw(child.id().unwrap() as i32),nix::sys::signal::Signal::SIGTERM).unwrap();
                },
                _ => unreachable!(),
            }
        } else {
            let call=json!({"jsonrpc":"2.0","id":31,"method":"tools/call","params":{"name":"echo","arguments":{"query":99}}});
            input.as_mut().unwrap().write_all(format!("{call}\n").as_bytes()).await.unwrap();
        }
        let pid=tokio::time::timeout(Duration::from_secs(3),async {
            loop {
                if let Ok(text)=std::fs::read_to_string(control.with_extension("ready"))
                    && let Ok(pid)=text.trim().parse::<u32>() { break pid; }
                tokio::task::yield_now().await;
            }
        }).await.unwrap();
        helper=Some(pid); assert!(!absent(pid),"original held lookup must actually exist");
        assert_eq!(std::fs::read_to_string(control.with_extension("host")).unwrap(), "gateway.fixture.invalid");
        match stop {
            ProcessStop::CancelRequest => {
                let cancel=json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":31}});
                input.as_mut().unwrap().write_all(format!("{cancel}\n").as_bytes()).await.unwrap();
                tokio::time::timeout(Duration::from_secs(3),async { while !absent(pid) { tokio::task::yield_now().await; } }).await.unwrap();
                // The original held child is retired before EOF and a new
                // ordinary RPC completes on the same initialized gateway.
                let next=json!({"jsonrpc":"2.0","id":32,"method":"tools/call","params":{"name":"echo","arguments":{"query":7}}});
                input.as_mut().unwrap().write_all(format!("{next}\n").as_bytes()).await.unwrap();
                let reply=tokio::time::timeout(Duration::from_secs(5),async {
                    loop { let line=output.next_line().await.unwrap().unwrap(); let v:Value=serde_json::from_str(&line).unwrap(); if v["id"]==32 {break v;} }
                }).await.unwrap(); assert!(reply["result"].is_object());
                input.take();
            },
            ProcessStop::Eof => { input.take(); },
            ProcessStop::EofDeleteLookup | ProcessStop::SigtermDeleteLookup => {},
            ProcessStop::Sigterm => {
                // Keep the parent stdin writer owned and OPEN through exit.
                nix::sys::signal::kill(nix::unistd::Pid::from_raw(child.id().unwrap() as i32),nix::sys::signal::Signal::SIGTERM).unwrap();
            },
        }
        // The final DELETE gets rmcp's five-second grace. Allow the existing
        // five-second process watchdog for native retirement (100ms TERM grace,
        // <=2s leader reap, <=2s group absence); this is test headroom, not policy.
        let grace = ssx_mcp::opaque::GATEWAY_SESSION_CLEANUP_TIMEOUT;
        terminal=Some(tokio::time::timeout(grace + Duration::from_secs(5),child.wait()).await.unwrap().unwrap());
        if delete_lookup {
            assert!(stopped_at.elapsed() >= grace, "held DELETE must expire its real grace, never be skipped");
            assert!(!terminal.as_ref().unwrap().success(), "unconfirmed session cleanup must fail closed");
            assert_eq!(*observed_peer.methods.lock().unwrap(), ["initialize", "notifications/initialized", "tools/list"], "held DNS must prevent DELETE from reaching the peer");
            if matches!(stop, ProcessStop::SigtermDeleteLookup) {
                assert!(input.is_some(), "parent stdin remains open through signal exit");
            }
            // No release exists: the exact fixture only terminates through the
            // retained production supervisor. Process exit must include its join.
            assert!(absent(pid));
        }
    }).catch_unwind().await;
    // Drain original process, stderr and local fixture before reporting any
    // protocol assertion/panic. Do not signal the child after its wait succeeded.
    if terminal.is_none() {
        if let Some(pid) = child.id() {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGTERM,
            );
        }
        terminal = match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
            Ok(Ok(status)) => Some(status),
            _ => {
                let _ = child.start_kill();
                child.wait().await.ok()
            }
        };
    }
    input.take();
    let stderr_result = stderr.await;
    close.cancel();
    let server_result = server.await;
    if let Some(pid) = helper {
        let _ = tokio::time::timeout(Duration::from_secs(2), async {
            while !absent(pid) {
                tokio::task::yield_now().await;
            }
        })
        .await;
    }
    let removed = std::fs::remove_dir_all(&case);
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    assert_eq!(terminal.unwrap().success(), !delete_lookup);
    assert!(helper.is_some_and(absent));
    let bytes = stderr_result.unwrap().unwrap();
    assert!(bytes.len() <= 65536);
    assert!(server_result.unwrap().is_ok());
    removed.unwrap();
}
#[tokio::test]
async fn actual_gateway_cancelled_held_dns_retires_child_before_next_rpc_and_eof() {
    process_case(ProcessStop::CancelRequest).await;
}
#[tokio::test]
async fn actual_gateway_eof_cancels_held_dns_and_joins_original_child() {
    process_case(ProcessStop::Eof).await;
}
#[tokio::test]
async fn actual_gateway_sigterm_cancels_held_dns_with_parent_stdin_open() {
    process_case(ProcessStop::Sigterm).await;
}

#[tokio::test]
async fn actual_gateway_eof_bounds_session_delete_held_fresh_dns_and_joins_original_child() {
    process_case(ProcessStop::EofDeleteLookup).await;
}
#[tokio::test]
async fn actual_gateway_sigterm_bounds_session_delete_held_fresh_dns_with_stdin_open() {
    process_case(ProcessStop::SigtermDeleteLookup).await;
}
