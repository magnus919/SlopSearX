//! No environment store URL is accepted. Each test owns a newly spawned, private
//! Unix-socket Valkey process and UUID namespace; no global flush is needed.
use serde_json::Value as Json;
use ssx_application::{
    health::*,
    rate::{RateLimiter, RateStore},
    retry_after::retry_after,
};
use ssx_domain::{AdapterResponse, EngineStatus, WallClockTimestamp, rate::*};
use ssx_infra::valkey::{Valkey, rate::DistributedQuota};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::{
    fs,
    io::Read as _,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Server {
    child: Child,
    dir: PathBuf,
    url: String,
}
impl Server {
    fn start() -> Self {
        let mut bytes = [0; 16];
        fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut bytes)
            .unwrap();
        bytes[6] = (bytes[6] & 15) | 64;
        bytes[8] = (bytes[8] & 63) | 128;
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("ssx-f10-{id}"));
        fs::create_dir(&dir).unwrap();
        let url = format!("unix://{}", dir.join("s.sock").display());
        let mut s = Self {
            child: Self::spawn(&dir),
            dir,
            url,
        };
        s.wait_ready();
        s
    }
    fn spawn(dir: &std::path::Path) -> Child {
        Command::new(
            std::env::var("SLOPSEARX_TEST_VALKEY_SERVER")
                .unwrap_or_else(|_| "valkey-server".into()),
        )
        .args([
            "--port",
            "0",
            "--bind",
            "127.0.0.1",
            "--save",
            "",
            "--appendonly",
            "no",
            "--daemonize",
            "no",
            "--unixsocketperm",
            "700",
            "--dir",
        ])
        .arg(dir)
        .arg("--unixsocket")
        .arg(dir.join("s.sock"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("disposable valkey-server required")
    }
    fn wait_ready(&mut self) {
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "Valkey startup failed"
            );
            if redis::Client::open(self.url.as_str())
                .unwrap()
                .get_connection()
                .is_ok()
            {
                break;
            }
            assert!(Instant::now() < until, "Valkey startup deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn connection(&self) -> redis::Connection {
        redis::Client::open(self.url.as_str())
            .unwrap()
            .get_connection()
            .unwrap()
    }
    fn store(&self) -> Valkey {
        Valkey::new(&self.url, Duration::from_millis(300)).unwrap()
    }
    fn stop(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
    fn restart(&mut self) {
        self.child = Self::spawn(&self.dir);
        self.wait_ready();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

struct Worker(Option<Child>);
impl Worker {
    fn output(mut self) -> std::process::Output {
        self.0.take().unwrap().wait_with_output().unwrap()
    }
    fn stop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
    }
}
fn engine(name: &str) -> EngineKey {
    EngineKey::new(name).unwrap()
}
fn quota(server: &Server, rate: f64, window: f64) -> DistributedQuota {
    DistributedQuota::new(
        Arc::new(server.store()),
        QuotaConfig::new(rate, window).unwrap(),
    )
}
fn keys(server: &Server, pattern: &str) -> Vec<String> {
    redis::cmd("KEYS")
        .arg(pattern)
        .query(&mut server.connection())
        .unwrap()
}
fn ttl_ms(server: &Server, key: &str) -> i64 {
    redis::cmd("PTTL")
        .arg(key)
        .query(&mut server.connection())
        .unwrap()
}
fn read(server: &Server, key: &str) -> Option<String> {
    redis::cmd("GET")
        .arg(key)
        .query(&mut server.connection())
        .unwrap()
}
#[tokio::test]
async fn actual_quota_law_zero_cost_fractional_rate_and_expiry() {
    let server = Server::start();
    let data: Json =
        serde_json::from_str(include_str!("../../xtask/fixtures/f10/corpus.json")).unwrap();
    for (i, case) in data["quota_law"].as_array().unwrap().iter().enumerate() {
        let q = quota(&server, case["rate"].as_f64().unwrap(), 60.0);
        let name = engine(&format!("law-{i}"));
        let mut key = None;
        for (cost, expected) in case["costs"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["outputs"].as_array().unwrap())
        {
            let receipt = q.acquire(&name, cost.as_u64().unwrap()).await.unwrap();
            assert_eq!(receipt.allowed, expected["allowed"].as_bool().unwrap());
            assert_eq!(receipt.count, expected["count"].as_u64().unwrap());
            assert_eq!(
                receipt.bucket_key,
                format!(
                    "ratelimit:{}:{}",
                    name.as_str(),
                    receipt.server_time.milliseconds() / 60000
                )
            );
            assert!(receipt.remaining_ttl_ms > 0 && receipt.remaining_ttl_ms <= 120000);
            if let Some(k) = &key {
                assert_eq!(k, &receipt.bucket_key)
            }
            key = Some(receipt.bucket_key);
        }
    }
    let q = quota(&server, 1.0, 0.05);
    let receipt = q.acquire(&engine("expiry"), 1).await.unwrap();
    assert!(receipt.allowed);
    assert!(ttl_ms(&server, &receipt.bucket_key) > 0);
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(read(&server, &receipt.bucket_key).is_none());
    assert!(q.acquire(&engine("expiry"), 1).await.unwrap().allowed);
}
#[tokio::test]
async fn quota_process_worker() {
    let Ok(url) = std::env::var("F10_PRIVATE_WORKER_URL") else {
        return;
    };
    assert!(url.starts_with("unix://") && url.contains("ssx-f10-"));
    let clock: f64 = std::env::var("F10_WORKER_CLOCK")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .unwrap();
    let once = std::env::var("F10_WORKER_MODE").is_ok();
    let store = Arc::new(Valkey::new(&url, Duration::from_secs(60)).unwrap());
    let q = Arc::new(DistributedQuota::new(
        store,
        QuotaConfig::new(7.0, if once { 1.0 } else { 60.0 }).unwrap(),
    ));
    if once {
        q.acquire(&engine("interrupted"), 1).await.unwrap();
        panic!("test proxy must withhold acknowledgment")
    }
    let mut limiter = RateLimiter::new(q, true);
    let mut allowed = 0;
    for i in 0..32 {
        allowed += u64::from(
            limiter
                .acquire(&engine("cross-process"), 1, clock + i as f64 / 1000.0)
                .await
                .unwrap()
                .allowed,
        );
    }
    println!("F10_ALLOWED:{allowed}");
    println!("F10_CLOCK:{clock}");
}
#[tokio::test]
async fn two_native_processes_with_divergent_local_clock_origins_share_one_quota() {
    let server = Server::start();
    let t = server.store().quota_time().await.unwrap().milliseconds();
    if t % 60000 > 55000 {
        tokio::time::sleep(Duration::from_millis(60100 - t % 60000)).await;
    }
    let children = [-900000.0, 900000.0].map(|clock| {
        Worker(Some(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "quota_process_worker", "--nocapture"])
                .env("F10_PRIVATE_WORKER_URL", &server.url)
                .env("F10_WORKER_CLOCK", clock.to_string())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        ))
    });
    let mut total = 0;
    let mut origins = Vec::new();
    for child in children {
        let out = child.output();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8(out.stdout).unwrap();
        origins.push(
            text.lines()
                .find_map(|l| l.strip_prefix("F10_CLOCK:"))
                .unwrap()
                .parse::<f64>()
                .unwrap(),
        );
        let n = text
            .lines()
            .find_map(|l| l.strip_prefix("F10_ALLOWED:"))
            .unwrap()
            .parse::<u64>()
            .unwrap();
        total += n;
    }
    assert_eq!(total, 7);
    assert_eq!(origins, vec![-900000.0, 900000.0]);
    println!("F10 native process clock origins={origins:?}; total admitted={total}");
    let keys = keys(&server, "ratelimit:cross-process:*");
    assert_eq!(keys.len(), 1);
    assert!(ttl_ms(&server, &keys[0]) > 0);
    assert!(read(&server, &keys[0]).unwrap().parse::<u64>().unwrap() >= 7);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn killed_native_waiter_after_commit_cannot_leave_an_immortal_bucket() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{UnixListener, UnixStream},
        sync::oneshot,
    };
    let server = Server::start();
    let proxy = server.dir.join("p");
    let listener = UnixListener::bind(&proxy).unwrap();
    let upstream = server.dir.join("s.sock");
    let (tx, rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        let (client, _) = listener.accept().await.unwrap();
        let upstream = UnixStream::connect(upstream).await.unwrap();
        let (mut cr, mut cw) = client.into_split();
        let (mut ur, mut uw) = upstream.into_split();
        let block = Arc::new(AtomicBool::new(false));
        let b = block.clone();
        let forward = tokio::spawn(async move {
            let mut buf = [0; 8192];
            let mut tail = Vec::new();
            loop {
                let n = cr.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                tail.extend_from_slice(&buf[..n]);
                if tail.windows(9).any(|w| w == b"F10: TIME") {
                    b.store(true, Ordering::SeqCst);
                }
                if tail.len() > 256 {
                    tail.drain(..tail.len() - 256);
                }
                if uw.write_all(&buf[..n]).await.is_err() {
                    break;
                }
            }
        });
        let mut tx = Some(tx);
        let mut buf = [0; 8192];
        loop {
            let n = ur.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                break;
            }
            if block.load(Ordering::SeqCst) {
                if let Some(tx) = tx.take() {
                    let _ = tx.send(());
                }
                continue;
            }
            if cw.write_all(&buf[..n]).await.is_err() {
                break;
            }
        }
        forward.abort();
    });
    let mut child = Worker(Some(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "quota_process_worker", "--nocapture"])
            .env(
                "F10_PRIVATE_WORKER_URL",
                format!("unix://{}", proxy.display()),
            )
            .env("F10_WORKER_MODE", "one")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    ));
    tokio::time::timeout(Duration::from_secs(10), rx)
        .await
        .expect("private proxy saw committed response")
        .unwrap();
    child.stop();
    task.abort();
    let entries = keys(&server, "ratelimit:interrupted:*");
    assert_eq!(entries.len(), 1);
    assert_eq!(read(&server, &entries[0]).as_deref(), Some("1"));
    let ttl = ttl_ms(&server, &entries[0]);
    assert!((1..=2000).contains(&ttl));
    println!("F10 killed unacknowledged native waiter: committed_count=1 ttl_ms={ttl}");
    tokio::time::sleep(Duration::from_millis(2100)).await;
    assert!(read(&server, &entries[0]).is_none());
}
#[tokio::test]
async fn shared_cooldown_extension_recovery_and_unavailable_distinction() {
    let mut server = Server::start();
    let a = Arc::new(quota(&server, 10.0, 1.0));
    let b = Arc::new(quota(&server, 10.0, 1.0));
    let name = engine("provider");
    assert_eq!(a.cooldown(&name).await.unwrap(), SharedCooldown::Missing);
    assert_eq!(a.publish_cooldown(&name, 1.01).await.unwrap(), 2);
    assert_eq!(b.publish_cooldown(&name, 1.0).await.unwrap(), 2);
    assert_eq!(a.publish_cooldown(&name, 3.0).await.unwrap(), 3);
    let mut limiter = RateLimiter::new(b.clone(), true);
    let denied = limiter.acquire(&name, 1, -9000.0).await.unwrap();
    assert_eq!(denied.reason, RateReason::ProviderCooldown);
    assert_eq!(limiter.state(&name).unwrap().consecutive_denials, 0);
    assert!(keys(&server, "ratelimit:provider:*").is_empty());
    // A subsecond live key must deny; explicit expiry avoids a sleep boundary.
    let boundary = engine("subsecond");
    redis::cmd("SET")
        .arg("ratelimit:upstream-cooldown:subsecond")
        .arg("1")
        .arg("PX")
        .arg(400)
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        b.cooldown(&boundary).await.unwrap(),
        SharedCooldown::Active { seconds: 1 }
    );
    let mut boundary_limiter = RateLimiter::new(b.clone(), true);
    assert_eq!(
        boundary_limiter
            .acquire(&boundary, 1, 0.0)
            .await
            .unwrap()
            .reason,
        RateReason::ProviderCooldown
    );
    redis::cmd("PEXPIRE")
        .arg("ratelimit:upstream-cooldown:subsecond")
        .arg(0)
        .query::<()>(&mut server.connection())
        .unwrap();
    boundary_limiter.reactivate(&boundary);
    assert!(
        boundary_limiter
            .acquire(&boundary, 1, 2.0)
            .await
            .unwrap()
            .allowed
    );
    assert_eq!(a.publish_cooldown(&name, 9000.0).await.unwrap(), 300);
    redis::cmd("PEXPIRE")
        .arg("ratelimit:upstream-cooldown:provider")
        .arg(10)
        .query::<()>(&mut server.connection())
        .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    limiter.reactivate(&name);
    assert!(limiter.acquire(&name, 1, 9999999.0).await.unwrap().allowed);
    server.stop();
    assert!(b.cooldown(&name).await.is_err());
    let decision = limiter.acquire(&engine("outage"), 1, 1.0).await.unwrap();
    assert!(!decision.allowed);
    assert!(decision.authority_failure.is_some());
    assert!(decision.cooldown_failure.is_some());
    server.restart();
    assert!(b.cooldown(&name).await.is_ok());
    assert!(b.acquire(&engine("recovery"), 1).await.unwrap().allowed);
    b.shutdown().await;
    assert_eq!(b.acquire(&name, 1).await.unwrap_err(), RateFailure::Closed);
}
#[tokio::test]
async fn invalid_requests_do_not_write_and_persistent_counter_is_repaired() {
    let server = Server::start();
    let q = quota(&server, 10.0, 60.0);
    assert_eq!(
        q.acquire(&engine("bad-cost"), u64::MAX).await.unwrap_err(),
        RateFailure::InvalidInput
    );
    assert!(keys(&server, "ratelimit:*").is_empty());
    let receipt = q.acquire(&engine("repair"), 1).await.unwrap();
    redis::cmd("PERSIST")
        .arg(&receipt.bucket_key)
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(ttl_ms(&server, &receipt.bucket_key), -1);
    let next = q.acquire(&engine("repair"), 1).await.unwrap();
    assert_eq!(next.count, 2);
    assert!(ttl_ms(&server, &receipt.bucket_key) > 0);
    redis::cmd("LPUSH")
        .arg("ratelimit:upstream-cooldown:wrongtype")
        .arg("x")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        q.cooldown(&engine("wrongtype")).await.unwrap_err(),
        RateFailure::Malformed
    );
}
#[tokio::test]
async fn explicit_fail_open_is_separate_from_authority_and_terminal_close() {
    let server = Server::start();
    let q = Arc::new(quota(&server, 1.0, 1.0));
    let mut limiter = RateLimiter::new(q.clone(), false);
    q.shutdown().await;
    let out = limiter.acquire(&engine("closed"), 1, 0.0).await.unwrap();
    assert!(!out.allowed);
    assert_eq!(out.authority_failure, Some(RateFailure::Closed));
    assert!(
        limiter
            .acquire(&engine("cost"), u64::MAX, 0.0)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn actual_f08_429_cooldown_prevents_redispatch_and_health_stays_observed() {
    use ssx_infra::http::{AdapterHttp, Cancellation, Options, Request};
    use std::sync::atomic::AtomicUsize;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let peer = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/fixture", peer.local_addr().unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let task = tokio::spawn(async move {
        let mut i = 0;
        while i < 2 {
            let (mut stream, _) = peer.accept().await.unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                match stream.read_exact(&mut byte).await {
                    Ok(_) => request.push(byte[0]),
                    Err(error)
                        if error.kind() == std::io::ErrorKind::UnexpectedEof
                            && request.is_empty() =>
                    {
                        break;
                    }
                    Err(error) => panic!("truncated mock request: {error}"),
                }
            }
            if request.is_empty() {
                continue;
            }
            counter.fetch_add(1, Ordering::SeqCst);
            stream.write_all(if i==0{b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"}else{b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"}).await.unwrap();
            i += 1;
        }
    });
    // A deadline may close an accepted socket before any HTTP bytes arrive.
    drop(
        tokio::net::TcpStream::connect(
            url.trim_start_matches("http://")
                .trim_end_matches("/fixture"),
        )
        .await
        .unwrap(),
    );
    let server = Server::start();
    let mut limiter = RateLimiter::new(Arc::new(quota(&server, 10.0, 1.0)), true);
    let name = engine("loopback");
    let http = AdapterHttp::default();
    let options = Options {
        trust_env: false,
        ..Default::default()
    };
    let mut health = HealthState::default();
    let wall = WallClockTimestamp::new(100.0).unwrap();
    assert!(limiter.acquire(&name, 1, 0.0).await.unwrap().allowed);
    let mut session = http
        .session(
            options.clone(),
            tokio::time::Instant::now() + Duration::from_secs(2),
            Cancellation::default(),
        )
        .unwrap();
    let began = Instant::now();
    let response = session.execute(Request::get(&url).unwrap()).await.unwrap();
    let measured = began.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(response.status, 429);
    let seconds = retry_after(
        response
            .headers
            .get("retry-after")
            .and_then(|h| h.to_str().ok()),
        100.0,
    );
    limiter
        .set_upstream_cooldown(&name, seconds, 0.0)
        .await
        .unwrap()
        .1
        .unwrap();
    let actual = AdapterResponse {
        status: EngineStatus::RateLimited,
        latency_ms: ssx_domain::FiniteF64::new(measured).unwrap(),
        ..Default::default()
    };
    health.observe(OutcomeOrigin::AdapterReturned, &actual, wall);
    let observed = build_engine_health(Some(&health), None, wall, 300.0).unwrap();
    assert_eq!(observed["status"], "rate_limited");
    assert!(observed["last_observed_latency_ms"].as_f64().unwrap() > 0.0);
    assert_eq!(
        limiter.acquire(&name, 1, 0.1).await.unwrap().reason,
        RateReason::ProviderCooldown
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let fabricated = AdapterResponse {
        status: EngineStatus::Timeout,
        synthetic: true,
        latency_ms: ssx_domain::FiniteF64::new(9999.0).unwrap(),
        ..Default::default()
    };
    let mut expired = http
        .session(
            options.clone(),
            tokio::time::Instant::now() - Duration::from_millis(1),
            Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        expired
            .execute(Request::get(&url).unwrap())
            .await
            .unwrap_err(),
        ssx_infra::http::Error::Deadline
    );
    health.observe(OutcomeOrigin::Undispatched, &fabricated, wall);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut cancelled = http
        .session(
            options.clone(),
            tokio::time::Instant::now() + Duration::from_secs(1),
            cancellation,
        )
        .unwrap();
    assert_eq!(
        cancelled
            .execute(Request::get(&url).unwrap())
            .await
            .unwrap_err(),
        ssx_infra::http::Error::Cancelled
    );
    health.observe(OutcomeOrigin::CallerCancelled, &fabricated, wall);
    assert_eq!(health.consecutive_errors, 0);
    assert_eq!(
        build_engine_health(Some(&health), None, wall, 300.0).unwrap(),
        observed
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(limiter.acquire(&name, 1, 1.2).await.unwrap().allowed);
    let mut recovered = http
        .session(
            options,
            tokio::time::Instant::now() + Duration::from_secs(2),
            Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        recovered
            .execute(Request::get(&url).unwrap())
            .await
            .unwrap()
            .status,
        200
    );
    task.await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn absent_url_warmup_is_noop_and_fail_setting_is_explicit() {
    for fail_closed in [false, true] {
        let config = QuotaConfig::new(10.0, 1.0).unwrap();
        let store = Arc::new(
            DistributedQuota::from_url("", None, config, Duration::from_millis(100)).unwrap(),
        );
        assert!(!store.configured());
        store.warmup().await.unwrap();
        let mut limiter = RateLimiter::new(store.clone(), fail_closed);
        let out = limiter.acquire(&engine("no-url"), 1, 0.0).await.unwrap();
        assert_eq!(out.allowed, !fail_closed);
        assert_eq!(out.reason, RateReason::QuotaUnavailable);
        assert_eq!(out.authority_failure, Some(RateFailure::Unavailable));
        assert_eq!(
            store.cooldown(&engine("no-url")).await.unwrap_err(),
            RateFailure::Unavailable
        );
        store.shutdown().await;
        assert_eq!(
            store.acquire(&engine("no-url"), 1).await.unwrap_err(),
            RateFailure::Closed
        );
    }
}
#[tokio::test]
async fn lost_acknowledgment_is_unknown_and_does_not_retry_increment() {
    let server = Server::start();
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(20)).unwrap());
    store.quota_time().await.unwrap();
    let q = DistributedQuota::new(store, QuotaConfig::new(10.0, 60.0).unwrap());
    redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(150)
        .arg("WRITE")
        .query::<()>(&mut server.connection())
        .unwrap();
    assert_eq!(
        q.acquire(&engine("unknown"), 1).await.unwrap_err(),
        RateFailure::CompletionUnknown
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    let keys = keys(&server, "ratelimit:unknown:*");
    assert_eq!(keys.len(), 1);
    assert_eq!(read(&server, &keys[0]).as_deref(), Some("1"));
    assert!(ttl_ms(&server, &keys[0]) > 0);
}

#[tokio::test]
async fn shutdown_does_not_close_borrowed_f07_authority() {
    let server = Server::start();
    let store = Arc::new(server.store());
    let q = DistributedQuota::new(store.clone(), QuotaConfig::new(1.0, 1.0).unwrap());
    q.shutdown().await;
    assert_eq!(
        q.acquire(&engine("closed"), 1).await.unwrap_err(),
        RateFailure::Closed
    );
    assert!(store.quota_time().await.is_ok());
    let owned = DistributedQuota::from_url(
        &server.url,
        None,
        QuotaConfig::new(1.0, 1.0).unwrap(),
        Duration::from_millis(100),
    )
    .unwrap();
    assert!(owned.configured());
    owned.warmup().await.unwrap();
    owned.shutdown().await;
    assert_eq!(owned.warmup().await.unwrap_err(), RateFailure::Closed);
}

#[tokio::test]
async fn a06_optional_adapter_cooldown_feedback_reuses_shared_authority_and_epoch() {
    use ssx_application::{AdmissionDecision, SearchAdmission};
    use ssx_infra::adapters::{AllowAllAdmission, F10Admission, f10_monotonic_seconds};
    let server = Server::start();
    let local = Arc::new(tokio::sync::Mutex::new(RateLimiter::new(
        Arc::new(quota(&server, 10.0, 1.0)),
        true,
    )));
    let remote = Arc::new(tokio::sync::Mutex::new(RateLimiter::new(
        Arc::new(quota(&server, 10.0, 1.0)),
        true,
    )));
    let a = F10Admission::new(local.clone());
    let b = F10Admission::new(remote);
    assert_eq!(a.admit("semanticscholar").await, AdmissionDecision::Allowed);
    let before = f10_monotonic_seconds();
    a.set_upstream_cooldown("semanticscholar", 12.5).await;
    let state = local
        .lock()
        .await
        .state(&engine("semanticscholar"))
        .unwrap()
        .clone();
    assert!(state.upstream_cooldown_until >= before + 12.5);
    assert!(state.upstream_cooldown_until <= f10_monotonic_seconds() + 12.5);
    assert_eq!(a.admit("semanticscholar").await, AdmissionDecision::Denied);
    assert_eq!(b.admit("semanticscholar").await, AdmissionDecision::Denied);
    let ttl = ttl_ms(&server, "ratelimit:upstream-cooldown:semanticscholar");
    assert!((1..=13_000).contains(&ttl));
    // Optional absence is the source no-limiter case, not a shared cooldown.
    let absent = AllowAllAdmission;
    absent.set_upstream_cooldown("semanticscholar", 12.5).await;
    assert_eq!(
        absent.admit("semanticscholar").await,
        AdmissionDecision::Allowed
    );
    // Publication failure is ignored, retaining the local marker already set.
    let store = Arc::new(
        DistributedQuota::from_url(
            "",
            None,
            QuotaConfig::new(10.0, 1.0).unwrap(),
            Duration::from_millis(100),
        )
        .unwrap(),
    );
    let failed = Arc::new(tokio::sync::Mutex::new(RateLimiter::new(store, true)));
    let bridge = F10Admission::new(failed.clone());
    bridge.set_upstream_cooldown("semanticscholar", 12.5).await;
    assert_eq!(
        bridge.admit("semanticscholar").await,
        AdmissionDecision::Denied
    );
    assert!(
        failed
            .lock()
            .await
            .state(&engine("semanticscholar"))
            .unwrap()
            .upstream_cooldown_until
            > 0.0
    );
}
