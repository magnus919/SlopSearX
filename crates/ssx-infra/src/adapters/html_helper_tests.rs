//! Engine-local helper ownership and deterministic successful-bootstrap controls.
use super::AllowAllAdmission;
use super::duckduckgo::{DuckDuckGoAdapter, DuckDuckGoConfig};
use crate::{
    html::ParserPool,
    http::{AdapterHttp, Request},
};
use ssx_application::SearchEngine;
use ssx_domain::{EngineStatus, SearchRequest};
use std::sync::{Arc, Mutex};
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}
fn native_helper() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("ssx-html-parser")
}
async fn read_request(stream: &mut tokio::net::TcpStream) -> String {
    use tokio::io::AsyncReadExt;
    let mut bytes = vec![];
    let mut buf = [0; 4096];
    loop {
        let n = stream.read(&mut buf).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&buf[..n]);
        if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&bytes[..end]);
            let len = head
                .lines()
                .find_map(|s| {
                    s.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|s| s.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            if bytes.len() >= end + 4 + len {
                break;
            }
        }
    }
    String::from_utf8(bytes).unwrap()
}
async fn reply(stream: &mut tokio::net::TcpStream, body: &str, cookie: bool) {
    use tokio::io::AsyncWriteExt;
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n",
        body.len(),
        if cookie {
            "Set-Cookie: local=yes; Path=/\r\n"
        } else {
            ""
        }
    );
    stream.write_all(header.as_bytes()).await.unwrap();
    stream.write_all(body.as_bytes()).await.unwrap();
}

async fn warm(http: &AdapterHttp, base: &str) {
    use crate::http::{Cancellation, Options};
    let mut session = http
        .session(
            Options {
                follow_redirects: true,
                ..Options::default()
            },
            tokio::time::Instant::now() + std::time::Duration::from_secs(5),
            Cancellation::default(),
        )
        .unwrap();
    session
        .execute(Request::get(&format!("{base}/warm")).unwrap())
        .await
        .unwrap();
}

#[tokio::test]
async fn loopback_cookie_isolation_bootstrap_fallback_and_exact_order() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(vec![]));
    let captured = seen.clone();
    let completed = Arc::new(tokio::sync::Notify::new());
    let signal = completed.clone();
    let mut server = Server(tokio::spawn(async move {
        let (mut warm, _) = listener.accept().await.unwrap();
        assert!(read_request(&mut warm).await.starts_with("GET /warm"));
        reply(&mut warm, "warm", false).await;
        for i in 0..6 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            captured.lock().unwrap().push(request);
            reply(&mut stream,match i%3 {0=>"<html/>",1=>"challenge",_=>"<table><tr><td><a class='result-link' href='/a'>A</a></td></tr><tr><td class='result-snippet'>S</td></tr></table>"},i%3==0).await;
            if i % 3 == 0 {
                signal.notify_one();
            }
        }
    }));
    let config = DuckDuckGoConfig {
        bootstrap_url: format!("{base}/home"),
        base_url: format!("{base}/html"),
        fallback_base_url: format!("{base}/lite"),
        ..Default::default()
    };
    let http = Arc::new(AdapterHttp::default());
    warm(&http, &base).await;
    let pool = Arc::new(
        ParserPool::new(native_helper(), std::num::NonZeroUsize::new(1).unwrap()).unwrap(),
    );
    let engine = DuckDuckGoAdapter::with_parser(
        http.clone(),
        Arc::new(AllowAllAdmission),
        config,
        pool.clone(),
    )
    .with_test_bootstrap_budget(std::time::Duration::from_secs(2));
    for _ in 0..2 {
        let request = SearchRequest {
            query: "A B".into(),
            ..SearchRequest::default()
        };
        let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            tokio::join!(engine.search(&request), completed.notified())
        })
        .await
        .expect("bounded bootstrap/search handshake");
        assert_eq!(result.status, EngineStatus::Ok);
        assert_eq!(result.results[0].url, "/a");
        assert_eq!(result.results[0].content, "S");
    }
    tokio::time::timeout(std::time::Duration::from_secs(2), &mut server.0)
        .await
        .expect("bounded server cleanup")
        .unwrap();
    pool.shutdown().await.unwrap();
    let requests = seen.lock().unwrap().clone();
    assert_eq!(requests.len(), 6);
    for (i, r) in requests.iter().enumerate() {
        assert!(r.starts_with(match i % 3 {
            0 => "GET /home HTTP/1.1",
            1 => "POST /html HTTP/1.1",
            _ => "POST /lite HTTP/1.1",
        }));
        assert_eq!(
            r.to_lowercase().contains("cookie: local=yes"),
            i % 3 != 0,
            "request {i}: {r:?}"
        );
        if i % 3 != 0 {
            assert!(r.ends_with("q=A+B"));
            assert!(r.contains("sec-fetch-site: same-origin"));
        }
    }
    drop(requests);
    http.shutdown_and_wait().await;
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Deadline,
    Resource,
    Shutdown,
    Source,
    Quota,
    Native,
    Held,
}
struct Fixture {
    dir: std::path::PathBuf,
    script: std::path::PathBuf,
    pid: std::path::PathBuf,
    spawned: std::path::PathBuf,
}
impl Fixture {
    fn new(mode: Mode) -> Self {
        use std::{
            fs,
            os::unix::fs::PermissionsExt,
            sync::atomic::{AtomicU64, Ordering},
        };
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = loop {
            let candidate = std::env::temp_dir().join(format!(
                "a05-helper-owned-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("fixture directory: {error}"),
            }
        };
        let script = dir.join("worker");
        let pid = dir.join("pids");
        let spawned = dir.join("spawned");
        let quote = |path: &std::path::Path| {
            format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
        };
        let mut content = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$$\" >>{}\nframe={}/frame-$$\n/bin/cat >\"$frame\"\n",
            quote(&spawned),
            quote(&dir)
        );
        if matches!(mode, Mode::Deadline) {
            content.push_str(&format!("mode=$(/usr/bin/od -An -tu1 -j5 -N1 \"$frame\" | /usr/bin/tr -d ' ')\nif [ \"$mode\" = 1 ]; then exec {} <\"$frame\"; fi\n", quote(&native_helper())));
        }
        content.push_str(&format!("printf '%s\\n' \"$$\" >>{}\n", quote(&pid)));
        match mode {
            Mode::Resource => {
                content.push_str("printf '\\377\\377\\377\\377\\377\\377\\377\\377'\n")
            }
            Mode::Source => {
                let payload = [b'A', b'0', b'5', b'D', 1, 2, 3];
                let bytes: String = (payload.len() as u64)
                    .to_be_bytes()
                    .into_iter()
                    .chain(payload)
                    .map(|byte| format!("\\{byte:03o}"))
                    .collect();
                content.push_str(&format!("printf '{bytes}'\n"));
            }
            Mode::Native => {
                content.push_str(&format!("exec {} <\"$frame\"\n", quote(&native_helper())))
            }
            _ => content.push_str("exec /bin/sleep 30\n"),
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&script)
            .unwrap();
        std::io::Write::write_all(&mut file, content.as_bytes()).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            dir,
            script,
            pid,
            spawned,
        }
    }
    fn records(path: &std::path::Path) -> Vec<u32> {
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let Some(complete) = raw.strip_suffix('\n') else {
            return Vec::new();
        };
        complete
            .lines()
            .map(|line| line.parse::<u32>().ok().filter(|&pid| pid > 0))
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default()
    }
    fn pids(&self) -> Vec<u32> {
        Self::records(&self.pid)
    }
    fn spawns(&self) -> Vec<u32> {
        Self::records(&self.spawned)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).expect("only owned reaped fixture directory");
    }
}
enum Engine {
    Ddg(Arc<DuckDuckGoAdapter>),
    Google(Arc<super::google::GoogleAdapter>),
}
impl Engine {
    fn port(&self) -> Arc<dyn SearchEngine> {
        match self {
            Self::Ddg(value) => value.clone(),
            Self::Google(value) => value.clone(),
        }
    }
    fn events(&self) -> Vec<bool> {
        match self {
            Self::Ddg(value) => value.test_feedback(),
            Self::Google(value) => value.test_feedback(),
        }
    }
}
#[tokio::test]
async fn typed_helper_terminal_outcomes_preserve_request_counts_feedback_and_reap() {
    use std::time::Duration;
    let mut completed = 0;
    for google in [false, true] {
        for mode in [
            Mode::Deadline,
            Mode::Resource,
            Mode::Shutdown,
            Mode::Source,
            Mode::Quota,
        ] {
            let fixture = Fixture::new(mode);
            let pool = Arc::new(
                ParserPool::new(
                    fixture.script.clone(),
                    std::num::NonZeroUsize::new(1).unwrap(),
                )
                .unwrap(),
            );
            assert!(!Arc::ptr_eq(&pool, &crate::html::shared_pool()));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let seen = Arc::new(Mutex::new(Vec::new()));
            let capture = seen.clone();
            let mut server = Server(tokio::spawn(async move {
                loop {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let request = read_request(&mut stream).await;
                    capture.lock().unwrap().push(request);
                    reply(&mut stream, "<html/>", false).await;
                }
            }));
            let http = Arc::new(AdapterHttp::default());
            let budget = if matches!(mode, Mode::Quota) {
                1
            } else {
                crate::html_budget::MAX_SEARCH_BYTES
            };
            let engine = if google {
                Engine::Google(Arc::new(super::google::GoogleAdapter::with_parser(
                    http.clone(),
                    Arc::new(AllowAllAdmission),
                    super::google::GoogleConfig {
                        base_url: format!("{base}/search"),
                        allocation_budget_bytes: budget,
                        ..Default::default()
                    },
                    pool.clone(),
                )))
            } else {
                Engine::Ddg(Arc::new(
                    DuckDuckGoAdapter::with_parser(
                        http.clone(),
                        Arc::new(AllowAllAdmission),
                        DuckDuckGoConfig {
                            bootstrap_url: format!("{base}/home"),
                            base_url: format!("{base}/html"),
                            fallback_base_url: format!("{base}/lite"),
                            allocation_budget_bytes: budget,
                            ..Default::default()
                        },
                        pool.clone(),
                    )
                    .with_test_bootstrap_budget(Duration::from_secs(2)),
                ))
            };
            let port = engine.port();
            let mut task = tokio::spawn(async move {
                port.search_with_context(
                    &SearchRequest {
                        query: "owned".into(),
                        ..Default::default()
                    },
                    ssx_application::SearchContext {
                        deadline: std::time::Instant::now() + Duration::from_secs(2),
                    },
                )
                .await
            });
            let mut ready = true;
            let mut before_events = None;
            if matches!(mode, Mode::Deadline | Mode::Shutdown) {
                ready = tokio::time::timeout(Duration::from_secs(1), async {
                    while fixture.pids().is_empty() {
                        tokio::time::sleep(Duration::from_millis(2)).await;
                    }
                })
                .await
                .is_ok();
                before_events = Some(engine.events());
                if matches!(mode, Mode::Shutdown) {
                    pool.shutdown().await.unwrap();
                }
            }
            let outcome = tokio::time::timeout(Duration::from_secs(4), &mut task).await;
            if outcome.is_err() {
                // Cleanup owns the child before any failing assertion or fixture
                // removal, including regression failures in this test itself.
                pool.shutdown().await.unwrap();
                task.abort();
                let _ = task.await;
                http.shutdown_and_wait().await;
                server.0.abort();
                let _ = (&mut server.0).await;
                panic!("engine outcome exceeded bound {google}/{mode:?}");
            }
            let result = outcome.unwrap();
            pool.shutdown().await.unwrap();
            http.shutdown_and_wait().await;
            server.0.abort();
            let joined = tokio::time::timeout(Duration::from_secs(1), &mut server.0)
                .await
                .expect("bounded server abort");
            assert!(
                joined.as_ref().is_err_and(|error| error.is_cancelled()),
                "server failed unexpectedly: {joined:?}"
            );
            let result = result.expect("engine completed without panic after cleanup");
            assert!(
                ready,
                "worker did not publish complete owned PID {google}/{mode:?}: {:?}",
                fixture.pid
            );
            if let Some(events) = before_events {
                assert_eq!(
                    events,
                    if google && matches!(mode, Mode::Deadline) {
                        vec![true]
                    } else {
                        vec![]
                    },
                    "before outcome {google}/{mode:?}"
                );
            }
            let requests = seen.lock().unwrap().clone();
            let expected_requests = if matches!(mode, Mode::Quota) {
                0
            } else if google {
                1
            } else if matches!(mode, Mode::Source) {
                3
            } else {
                2
            };
            assert_eq!(
                requests.len(),
                expected_requests,
                "{google}/{mode:?}: {requests:?}"
            );
            if !google && !requests.is_empty() {
                assert!(requests[0].starts_with("GET /home HTTP/1.1"));
                assert!(requests[1].starts_with("POST /html HTTP/1.1"));
                if matches!(mode, Mode::Source) {
                    assert!(requests[2].starts_with("POST /lite HTTP/1.1"));
                }
            }
            assert_eq!(
                result.status,
                if matches!(mode, Mode::Deadline) {
                    EngineStatus::Timeout
                } else {
                    EngineStatus::Error
                },
                "{google}/{mode:?}"
            );
            if matches!(mode, Mode::Source) {
                let message = result
                    .error_message
                    .as_deref()
                    .expect("ordinary codec diagnostic retained");
                assert_eq!(
                    message.matches("undefined encoding").count(),
                    if google { 1 } else { 2 }
                );
            }
            let expected_events = match mode {
                Mode::Deadline if google => vec![true],
                Mode::Deadline => vec![false],
                Mode::Source if !google => vec![false, false],
                _ => vec![],
            };
            assert_eq!(engine.events(), expected_events, "{google}/{mode:?}");
            let pids = fixture.pids();
            assert_eq!(
                pids.len(),
                if matches!(mode, Mode::Quota) {
                    0
                } else if !google && matches!(mode, Mode::Source) {
                    2
                } else {
                    1
                },
                "{google}/{mode:?}"
            );
            let spawns = fixture.spawns();
            assert_eq!(
                spawns.len(),
                if matches!(mode, Mode::Quota) {
                    0
                } else if matches!(mode, Mode::Deadline)
                    || (!google && matches!(mode, Mode::Source))
                {
                    2
                } else {
                    1
                },
                "worker entry count {google}/{mode:?}"
            );
            for pid in pids.into_iter().chain(spawns) {
                assert!(
                    !std::process::Command::new("/bin/kill")
                        .args(["-0", &pid.to_string()])
                        .output()
                        .unwrap()
                        .status
                        .success(),
                    "owned worker {pid} still live {google}/{mode:?}"
                );
            }
            completed += 1;
            println!(
                "A05_HELPER {}/{mode:?}",
                if google { "google" } else { "duckduckgo" }
            );
        }
    }
    assert_eq!(completed, 10);
}

#[tokio::test]
async fn actual_bounded_child_quota_and_non_emitting_codec_deadline_cancel_reap() {
    use crate::{html::OperationFailure, html_budget::Operation};
    use std::time::Duration;
    // Capture raw fixture bytes before starting the operation timer. Actual
    // native child input/decoder work is supervised; no sleep substitutes for it.
    let mut completed = 0;
    for kind in ["quota", "deadline", "cancel"] {
        let fixture = Fixture::new(Mode::Native);
        let pool = Arc::new(
            ParserPool::new(
                fixture.script.clone(),
                std::num::NonZeroUsize::new(1).unwrap(),
            )
            .unwrap(),
        );
        let body = if kind == "quota" {
            vec![b'A'; 4 * 1024 * 1024]
        } else {
            let count = (20 * 1024 * 1024 / 3) * 3;
            let mut body = vec![0; count + 2];
            for chunk in body[..count].as_chunks_mut::<3>().0 {
                chunk.copy_from_slice(b"\x1b(B");
            }
            body[count..].copy_from_slice(b"\x1b(");
            body
        };
        let limit = if kind == "quota" {
            7 * body.len() + 512
        } else {
            crate::html_budget::MAX_SEARCH_BYTES
        };
        let duration = if kind == "deadline" {
            Duration::from_millis(150)
        } else {
            Duration::from_secs(20)
        };
        let operation =
            match Operation::start(limit, 0, tokio::time::Instant::now() + duration).await {
                Ok(operation) => operation,
                Err(crate::html_budget::OperationError::Deadline) if kind == "deadline" => {
                    pool.shutdown().await.unwrap();
                    assert!(fixture.spawns().is_empty());
                    assert!(fixture.pids().is_empty());
                    assert_eq!(pool.available_slots(), 1);
                    println!("A05_DECODE_DEADLINE entry=0 consumed=0 admission_expired=true");
                    println!("A05_BOUNDED_CPU deadline");
                    completed += 1;
                    continue;
                }
                Err(error) => {
                    pool.shutdown().await.unwrap();
                    panic!("unexpected fixture admission failure: {error:?}");
                }
            };
        let ledger = operation.budget.0.clone();
        struct Input {
            bytes: Vec<u8>,
            _charge: crate::html_budget::Charge,
        }
        let charge = ledger.reserve(body.capacity()).unwrap();
        let input = Input {
            bytes: body,
            _charge: charge,
        };
        let parser = pool.clone();
        let codec = if kind == "quota" {
            "utf-8"
        } else {
            "iso2022-jp"
        };
        let mut task = tokio::spawn(async move {
            let input = input;
            let result = parser
                .decode_bounded_with_operation(&input.bytes, codec, operation)
                .await;
            drop(input);
            result
        });
        let fixture_started = std::time::Instant::now();
        let mut active_before_cancel = true;
        let outcome = if kind == "cancel" {
            let ready = tokio::time::timeout(Duration::from_secs(2), async {
                while fixture.pids().is_empty() {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
            })
            .await
            .is_ok();
            if ready {
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
            active_before_cancel = ready && !task.is_finished();
            task.abort();
            match task.await {
                Err(error) if error.is_cancelled() => None,
                other => {
                    drop(other);
                    active_before_cancel = false;
                    None
                }
            }
        } else {
            match tokio::time::timeout(Duration::from_secs(4), &mut task).await {
                Ok(joined) => Some(joined),
                Err(_) => {
                    let spawned = fixture.spawns();
                    let consumed = fixture.pids();
                    let before_cleanup = ledger.snapshot().unwrap();
                    let slots_before = pool.available_slots();
                    let elapsed = fixture_started.elapsed();
                    let shutdown = pool.shutdown().await;
                    task.abort();
                    let joined = task.await;
                    let joined_outcome = match &joined {
                        Ok(Ok(_)) => "completed-success",
                        Ok(Err(_)) => "completed-operation-error",
                        Err(error) if error.is_cancelled() => "cancelled",
                        Err(_) => "task-panic",
                    };
                    drop(joined);
                    let after_cleanup = ledger.snapshot().unwrap();
                    let slots_after = pool.available_slots();
                    let live: Vec<_> = spawned
                        .iter()
                        .chain(&consumed)
                        .copied()
                        .filter(|pid| {
                            std::process::Command::new("/bin/kill")
                                .args(["-0", &pid.to_string()])
                                .output()
                                .unwrap()
                                .status
                                .success()
                        })
                        .collect();
                    eprintln!(
                        "A05_DECODE_WATCHDOG kind={kind} elapsed={elapsed:?} spawned={spawned:?} consumed={consumed:?} slots_before={slots_before} used_before={} shutdown={shutdown:?} joined={joined_outcome} slots_after={slots_after} used_after={} live_after={live:?} fixture={}",
                        before_cleanup.used,
                        after_cleanup.used,
                        fixture.dir.display()
                    );
                    assert!(shutdown.is_ok(), "watchdog cleanup failed: {kind}");
                    assert!(
                        live.is_empty(),
                        "watchdog retained live owned worker: {kind}"
                    );
                    assert_eq!(slots_after, 1, "watchdog retained worker permit: {kind}");
                    assert_eq!(
                        after_cleanup.used, 0,
                        "watchdog retained operation charge: {kind}"
                    );
                    panic!("bounded native worker exceeded fixture deadline: {kind}");
                }
            }
        };
        let drained = tokio::time::timeout(Duration::from_secs(2), async {
            while pool.available_slots() != 1 || ledger.snapshot().unwrap().used != 0 {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .is_ok();
        // Verified shutdown also precedes failing assertions/fixture removal.
        pool.shutdown().await.unwrap();
        assert!(drained, "worker lease/accounting not drained: {kind}");
        assert!(
            active_before_cancel,
            "cancellation did not interrupt a live native operation"
        );
        if let Some(joined) = outcome {
            let result = joined.expect("native operation task did not panic");
            assert!(
                matches!(
                    (kind, result),
                    ("quota", Err(OperationFailure::Resource))
                        | ("deadline", Err(OperationFailure::Deadline))
                ),
                "wrong native outcome {kind}"
            );
        }
        if kind == "deadline" {
            // The absolute end-to-end deadline may expire before payload
            // consumption. This is valid Timeout evidence, not codec entry.
            assert!(fixture.spawns().len() <= 1);
            assert!(fixture.pids().len() <= 1);
            assert!(fixture.pids().len() <= fixture.spawns().len());
            println!(
                "A05_DECODE_DEADLINE entry={} consumed={}",
                fixture.spawns().len(),
                fixture.pids().len()
            );
        } else {
            assert_eq!(fixture.spawns().len(), 1, "actual worker entry {kind}");
            assert_eq!(fixture.pids().len(), 1, "actual request consumed {kind}");
        }
        for pid in fixture.spawns() {
            assert!(
                !std::process::Command::new("/bin/kill")
                    .args(["-0", &pid.to_string()])
                    .output()
                    .unwrap()
                    .status
                    .success(),
                "native worker {pid} live after shutdown: {kind}"
            );
        }
        assert_eq!(ledger.snapshot().unwrap().used, 0);
        completed += 1;
        println!("A05_BOUNDED_CPU {kind}");
    }
    assert_eq!(completed, 3);
}

#[tokio::test]
async fn actual_rows_worker_quota_deadline_cancel_and_reap() {
    use crate::{html::OperationFailure, html_budget::Operation};
    use std::time::Duration;
    let mut completed = 0;
    for kind in ["quota", "deadline", "cancel"] {
        let fixture = Fixture::new(Mode::Native);
        let pool = Arc::new(
            ParserPool::new(
                fixture.script.clone(),
                std::num::NonZeroUsize::new(1).unwrap(),
            )
            .unwrap(),
        );
        // Deep selected contexts with a wide subtree make real child parsing /
        // CSS work nontrivial without synthetic worker sleeps or network calls.
        let mut raw = String::new();
        for i in 0..200 {
            raw.push_str(&format!("<div class=g><a href='http:/{i}'>x</a>"));
        }
        raw.push_str(&"<span>x</span>".repeat(100_000));
        raw.push_str(&"</div>".repeat(200));
        let points: Vec<u32> = raw.bytes().map(u32::from).collect();
        drop(raw);
        let limit = if kind == "quota" {
            points.len() * 12 + 512
        } else {
            crate::html_budget::MAX_SEARCH_BYTES
        };
        let duration = if kind == "deadline" {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(20)
        };
        let operation = Operation::start(limit, 0, tokio::time::Instant::now() + duration)
            .await
            .unwrap();
        let ledger = operation.budget.0.clone();
        struct Input {
            points: Vec<u32>,
            _charge: crate::html_budget::Charge,
        }
        let input = Input {
            _charge: ledger.reserve(points.capacity() * 4).unwrap(),
            points,
        };
        let parser = pool.clone();
        let mut task = tokio::spawn(async move {
            let result = parser
                .parse_rows_with_operation(
                    &input.points,
                    crate::html::row_wire::Layout::Google,
                    super::RowLimit::Unlimited,
                    operation,
                )
                .await;
            drop(input);
            result
        });
        let mut interrupted = true;
        let result = if kind == "cancel" {
            let ready = tokio::time::timeout(Duration::from_secs(4), async {
                while fixture.pids().is_empty() {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
            })
            .await
            .is_ok();
            if ready {
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
            interrupted = ready && !task.is_finished();
            task.abort();
            match task.await {
                Err(error) if error.is_cancelled() => None,
                other => {
                    drop(other);
                    interrupted = false;
                    None
                }
            }
        } else {
            match tokio::time::timeout(Duration::from_secs(5), &mut task).await {
                Ok(result) => Some(result),
                Err(_) => {
                    pool.shutdown().await.unwrap();
                    task.abort();
                    let _ = task.await;
                    panic!("rows fixture exceeded bound: {kind}");
                }
            }
        };
        // Unexpected successful frames must drop their retained reply charge
        // before the ledger drainage assertion below.
        let outcome = match result {
            Some(Ok(Err(error))) => Some(error),
            Some(other) => {
                drop(other);
                None
            }
            None => None,
        };
        let drained = tokio::time::timeout(Duration::from_secs(2), async {
            while pool.available_slots() != 1 || ledger.snapshot().unwrap().used != 0 {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .is_ok();
        pool.shutdown().await.unwrap();
        assert!(drained, "rows lease did not drain: {kind}");
        assert!(
            interrupted,
            "rows cancellation did not interrupt a live worker"
        );
        if kind == "quota" {
            assert!(matches!(outcome, Some(OperationFailure::Resource)));
            assert!(fixture.spawns().is_empty());
            assert!(fixture.pids().is_empty());
        } else {
            if kind == "deadline" {
                assert!(matches!(outcome, Some(OperationFailure::Deadline)));
            }
            assert_eq!(fixture.spawns().len(), 1, "native rows worker entry {kind}");
            assert_eq!(fixture.pids().len(), 1, "native rows input consumed {kind}");
            for pid in fixture.spawns() {
                assert!(
                    !std::process::Command::new("/bin/kill")
                        .args(["-0", &pid.to_string()])
                        .output()
                        .unwrap()
                        .status
                        .success(),
                    "rows worker {pid} live after cleanup"
                );
            }
        }
        assert_eq!(ledger.snapshot().unwrap().used, 0);
        completed += 1;
        println!("A05_ROWS_CPU {kind}");
    }
    assert_eq!(completed, 3);
}

#[tokio::test]
async fn held_decoder_handoff_deadline_retains_ownership_until_verified_reap() {
    use crate::{html::OperationFailure, html_budget::Operation};
    use std::time::Duration;
    // Deliberately held AFTER the complete real request is consumed. This is
    // handoff ownership evidence, never labelled native codec CPU evidence.
    let fixture = Fixture::new(Mode::Held);
    let pool = Arc::new(
        ParserPool::new(
            fixture.script.clone(),
            std::num::NonZeroUsize::new(1).unwrap(),
        )
        .unwrap(),
    );
    let bytes = vec![b'A'; 1024 * 1024];
    let operation = Operation::start(
        crate::html_budget::MAX_SEARCH_BYTES,
        0,
        tokio::time::Instant::now() + Duration::from_secs(5),
    )
    .await
    .unwrap();
    let ledger = operation.budget.0.clone();
    struct Input {
        bytes: Vec<u8>,
        _charge: crate::html_budget::Charge,
    }
    let input = Input {
        _charge: ledger.reserve(bytes.capacity()).unwrap(),
        bytes,
    };
    let parser = pool.clone();
    let mut task = tokio::spawn(async move {
        let result = parser
            .decode_bounded_with_operation(&input.bytes, "utf-8", operation)
            .await;
        drop(input);
        result
    });
    let ready = tokio::time::timeout(Duration::from_secs(2), async {
        while fixture.pids().is_empty() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .is_ok();
    let owned = ready
        && !task.is_finished()
        && pool.available_slots() == 0
        && ledger.snapshot().unwrap().used > 0;
    let outcome = match tokio::time::timeout(Duration::from_secs(7), &mut task).await {
        Ok(result) => result,
        Err(_) => {
            pool.shutdown().await.unwrap();
            task.abort();
            let _ = task.await;
            panic!("held decoder handoff exceeded bounded cleanup watchdog");
        }
    };
    pool.shutdown().await.unwrap();
    assert!(
        owned,
        "request did not enter the fixture-controlled held handoff"
    );
    assert!(matches!(outcome, Ok(Err(OperationFailure::Deadline))));
    assert_eq!(fixture.spawns().len(), 1);
    assert_eq!(fixture.pids().len(), 1);
    assert_eq!(pool.available_slots(), 1);
    assert_eq!(ledger.snapshot().unwrap().used, 0);
    for pid in fixture.spawns() {
        assert!(
            !std::process::Command::new("/bin/kill")
                .args(["-0", &pid.to_string()])
                .output()
                .unwrap()
                .status
                .success(),
            "held worker {pid} live after verified reap"
        );
    }
    println!("A05_HELD_HANDOFF deadline");
}

#[tokio::test]
async fn actual_raw_header_selector_cancellation_retains_lease_until_reap() {
    use crate::html_budget::Operation;
    use std::time::Duration;
    let fixture = Fixture::new(Mode::Native);
    let pool = Arc::new(
        ParserPool::new(
            fixture.script.clone(),
            std::num::NonZeroUsize::new(1).unwrap(),
        )
        .unwrap(),
    );
    let mut bytes = b"text/html; charset=".to_vec();
    bytes.resize(12 * 1024 * 1024, b'a');
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "content-type",
        reqwest::header::HeaderValue::from_bytes(&bytes).unwrap(),
    );
    drop(bytes);
    let operation = Operation::start(
        crate::html_budget::MAX_SEARCH_BYTES,
        0,
        tokio::time::Instant::now() + Duration::from_secs(20),
    )
    .await
    .unwrap();
    let ledger = operation.budget.0.clone();
    // Logical public bytes are charged for this controlled fixture. This does
    // not infer HeaderValue's opaque allocation layout or close F08 backing.
    struct Input {
        headers: reqwest::header::HeaderMap,
        _charge: crate::html_budget::Charge,
    }
    let charge = ledger
        .reserve(headers["content-type"].as_bytes().len())
        .unwrap();
    let input = Input {
        headers,
        _charge: charge,
    };
    let parser = pool.clone();
    let task = tokio::spawn(async move {
        let input = input;
        let out = parser
            .decode_http_with_operation(b"A", &input.headers, operation)
            .await;
        drop(input);
        out
    });
    let ready = tokio::time::timeout(Duration::from_secs(3), async {
        while fixture.pids().is_empty() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .is_ok();
    let active = ready && !task.is_finished();
    let charged = ledger.snapshot().unwrap().used > 12 * 1024 * 1024;
    task.abort();
    let cancelled = task.await.is_err_and(|e| e.is_cancelled());
    let drained = tokio::time::timeout(Duration::from_secs(2), async {
        while pool.available_slots() != 1 || ledger.snapshot().unwrap().used != 0 {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .is_ok();
    pool.shutdown().await.unwrap();
    assert!(
        active && charged && cancelled && drained,
        "ready={ready} active={active} charged={charged} cancelled={cancelled} drained={drained}"
    );
    assert_eq!(fixture.spawns().len(), 1);
    assert_eq!(fixture.pids().len(), 1);
    for pid in fixture.pids() {
        assert!(
            !std::process::Command::new("/bin/kill")
                .args(["-0", &pid.to_string()])
                .output()
                .unwrap()
                .status
                .success(),
            "raw-header worker {pid} remains live"
        );
    }
    assert_eq!(ledger.snapshot().unwrap().used, 0);
    println!("A05_HEADER_CPU cancel");
}

#[tokio::test]
async fn registered_factories_share_owned_pool_and_await_teardown_after_caller_drop() {
    use std::time::{Duration, Instant};
    let fixture = Fixture::new(Mode::Held);
    let pool = Arc::new(
        ParserPool::new(
            fixture.script.clone(),
            std::num::NonZeroUsize::new(1).unwrap(),
        )
        .unwrap(),
    );
    let independent = Arc::new(
        ParserPool::new(native_helper(), std::num::NonZeroUsize::new(1).unwrap()).unwrap(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let capture = requests.clone();
    let server = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            capture.lock().unwrap().push(request);
            reply(&mut stream, "<html/>", false).await;
        }
    });
    let http = Arc::new(AdapterHttp::default());
    let mut config = super::HtmlConfig::default();
    config.google.base_url = format!("{base}/google");
    config.duckduckgo.bootstrap_url = format!("{base}/home");
    config.duckduckgo.base_url = format!("{base}/ddg");
    config.duckduckgo.fallback_base_url = format!("{base}/lite");
    config.google.timeout_ms = ssx_config::ConfigScalar::Float(f64::INFINITY);
    let mut registry = ssx_registry::Registry::builtins().unwrap();
    super::register_html_with_parser(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        config,
        pool.clone(),
    )
    .unwrap();
    // Both executable factories exist independently of static availability flags.
    let google = registry.instantiate("google").unwrap();
    let ddg = registry.instantiate("duckduckgo").unwrap();
    let task = tokio::spawn(async move {
        google
            .search_with_context(
                &SearchRequest {
                    query: "owned factory".into(),
                    ..Default::default()
                },
                ssx_application::SearchContext {
                    deadline: Instant::now() + Duration::from_secs(20),
                },
            )
            .await
    });
    let entered = tokio::time::timeout(Duration::from_secs(5), async {
        while fixture.pids().is_empty() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .is_ok();
    task.abort();
    let _ = task.await;
    let closed = tokio::time::timeout(Duration::from_secs(5), pool.shutdown()).await;
    assert!(entered, "factory dispatch did not consume its helper frame");
    closed.unwrap().unwrap();
    assert_eq!(fixture.spawns().len(), 1);
    for pid in fixture.spawns() {
        assert!(
            !std::process::Command::new("/bin/kill")
                .args(["-0", &pid.to_string()])
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap()
                .success(),
            "owned worker still live after awaited shutdown"
        );
    }
    // A second factory retains this same closed lifecycle, never a default pool.
    let result = ddg
        .search_with_context(
            &SearchRequest {
                query: "closed".into(),
                ..Default::default()
            },
            ssx_application::SearchContext {
                deadline: Instant::now() + Duration::from_secs(2),
            },
        )
        .await;
    assert_eq!(result.status, EngineStatus::Error);
    assert_eq!(
        result.error_message,
        Some(format!(
            "DuckDuckGo served no parsable results on any endpoint: {base}/ddg ({base}/ddg: HTML parser admission closed); {base}/lite ({base}/lite: HTML parser admission closed)"
        ))
    );
    server.abort();
    let _ = server.await;
    assert_eq!(fixture.spawns().len(), 1);
    assert_eq!(independent.available_slots(), 1);
    let decoded = independent
        .decode(
            b"still independent",
            "utf-8",
            tokio::time::Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert_eq!(
        decoded.codepoints(),
        &b"still independent"
            .iter()
            .map(|&byte| u32::from(byte))
            .collect::<Vec<_>>()
    );
    independent.shutdown().await.unwrap();
    http.shutdown_and_wait().await;
    assert!(requests.lock().unwrap()[0].starts_with("GET /google?"));
}

// Successor of the frozen 3f1b9cca integration control. Only the HTTP owner's
// Manual clock advances; Tokio/HTML parser deadlines retain their System clock.
// A pending bootstrap has no Set-Cookie response: successful-cookie behavior is
// separately covered by the seven unchanged helper controls above.
struct OwnerClockColdBootstrap {
    seen: Mutex<Vec<Request>>,
    started: tokio::sync::Notify,
    driver: Mutex<Option<Arc<crate::http::Driver>>>,
    bootstrap_started_at: Mutex<Option<std::time::Instant>>,
    elapsed: Mutex<Vec<std::time::Duration>>,
    dropped_bootstraps: std::sync::atomic::AtomicUsize,
}
struct PendingColdBootstrap<'a>(&'a std::sync::atomic::AtomicUsize);
impl Drop for PendingColdBootstrap<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}
impl crate::http::Transport for OwnerClockColdBootstrap {
    fn exchange(&self, request: Request) -> crate::http::ExchangeFuture<'_> {
        Box::pin(async move {
            let driver = self
                .driver
                .lock()
                .unwrap()
                .as_ref()
                .expect("Manual owner installed before search")
                .clone();
            self.seen.lock().unwrap().push(request.clone());
            if request.url.path() == "/home" {
                *self.bootstrap_started_at.lock().unwrap() = Some(driver.now());
                let _pending = PendingColdBootstrap(&self.dropped_bootstraps);
                self.started.notify_one();
                std::future::pending::<()>().await;
                unreachable!("bootstrap must expire through its HTTP owner")
            }
            let primary = request.url.path() == "/html";
            let second_search = self
                .seen
                .lock()
                .unwrap()
                .iter()
                .filter(|request| request.url.path() == "/home")
                .count()
                == 2;
            if primary {
                let started = self
                    .bootstrap_started_at
                    .lock()
                    .unwrap()
                    .expect("primary must follow an actually dispatched bootstrap");
                self.elapsed
                    .lock()
                    .unwrap()
                    .push(driver.now().duration_since(started));
            }
            let body = if !primary {
                "<table><tr><td><a class='result-link' href='/cold-lite'>Cold Lite</a></td></tr></table>"
            } else if second_search {
                "challenge"
            } else {
                "<div class='result'><a class='result__a' href='/cold'>Cold</a></div>"
            };
            Ok(crate::http::Response {
                status: 200,
                url: request.url,
                headers: reqwest::header::HeaderMap::new(),
                body: body.as_bytes().to_vec(),
            })
        })
    }
}
#[tokio::test]
async fn cold_bootstrap_expires_without_spending_search_deadline_or_leaking_cookies() {
    use std::{future::Future, task::Poll, time::Duration};
    let transport = Arc::new(OwnerClockColdBootstrap {
        seen: Mutex::new(vec![]),
        started: tokio::sync::Notify::new(),
        driver: Mutex::new(None),
        bootstrap_started_at: Mutex::new(None),
        elapsed: Mutex::new(vec![]),
        dropped_bootstraps: std::sync::atomic::AtomicUsize::new(0),
    });
    let (owner, driver) = AdapterHttp::replay_owner(transport.clone());
    let driver = Arc::new(driver);
    *transport.driver.lock().unwrap() = Some(driver.clone());
    let http = Arc::new(owner);
    let pool = Arc::new(
        ParserPool::new(native_helper(), std::num::NonZeroUsize::new(1).unwrap()).unwrap(),
    );
    let config = DuckDuckGoConfig {
        bootstrap_url: "http://cold.invalid/home".into(),
        base_url: "http://cold.invalid/html".into(),
        fallback_base_url: "http://cold.invalid/lite".into(),
        ..Default::default()
    };
    assert_eq!(config.bootstrap_timeout_ms, 2500.0);
    let engine = DuckDuckGoAdapter::with_parser(
        http.clone(),
        Arc::new(AllowAllAdmission),
        config,
        pool.clone(),
    )
    .with_test_bootstrap_owner_clock();
    for index in 0..2 {
        let request = SearchRequest::default();
        let mut search = Box::pin(engine.search(&request));
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            // This is readiness, never an assumption that a 5ms System deadline
            // left enough host scheduling time to enter the transport.
            tokio::select! {
                _ = transport.started.notified() => {},
                result = &mut search => panic!("search completed before bootstrap readiness: {:?}", result.status),
            }
            let origin = Duration::from_millis(index as u64 * 5);
            driver.advance_to(origin + Duration::from_millis(4));
            let early = std::future::poll_fn(|cx| Poll::Ready(search.as_mut().poll(cx))).await;
            assert!(early.is_pending(), "bootstrap completed before its 5ms budget");
            assert_eq!(transport.elapsed.lock().unwrap().len(), index);
            assert_eq!(
                transport.dropped_bootstraps.load(std::sync::atomic::Ordering::SeqCst),
                index,
            );
            driver.advance_to(origin + Duration::from_millis(5));
            let boundary = std::future::poll_fn(|cx| Poll::Ready(search.as_mut().poll(cx))).await;
            assert_eq!(
                transport.dropped_bootstraps.load(std::sync::atomic::Ordering::SeqCst),
                index + 1,
                "default bootstrap clamp did not expire and drop its future at exactly 5ms",
            );
            // Hold the owner at exactly 5ms while normal continuation/parser
            // scheduling completes under the unchanged System harness bound.
            match boundary {
                Poll::Ready(result) => result,
                Poll::Pending => search.await,
            }
        })
        .await
        .expect("bounded owner-clock cold bootstrap/search control");
        assert_eq!(
            result.status,
            EngineStatus::Ok,
            "search must restore its search deadline after bootstrap expiry",
        );
        assert_eq!(transport.elapsed.lock().unwrap().len(), index + 1);
        assert_eq!(
            result.results[0].url,
            if index == 0 { "/cold" } else { "/cold-lite" },
        );
    }
    assert_eq!(
        *transport.elapsed.lock().unwrap(),
        vec![Duration::from_millis(5); 2],
    );
    let requests = transport.seen.lock().unwrap().clone();
    assert_eq!(requests.len(), 5);
    for (request, path) in requests
        .iter()
        .zip(["/home", "/html", "/home", "/html", "/lite"])
    {
        assert_eq!(request.url.path(), path);
        assert_eq!(
            request.method,
            if path == "/home" {
                reqwest::Method::GET
            } else {
                reqwest::Method::POST
            },
        );
        assert!(!request.headers.contains_key("cookie"));
    }
    drop(requests);
    pool.shutdown().await.unwrap();
    http.shutdown_and_wait().await;
}
