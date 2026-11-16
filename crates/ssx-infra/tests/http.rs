use reqwest::{
    Method,
    header::{self, HeaderMap, HeaderValue},
};
use ssx_infra::http::*;
use std::{
    io::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};
struct Server {
    url: String,
    accepts: Arc<AtomicUsize>,
    seen: Arc<Mutex<Vec<String>>>,
    open: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
struct Open(Arc<AtomicUsize>);
impl Drop for Open {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn server() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let accepts = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let a = accepts.clone();
    let s = seen.clone();
    let open = Arc::new(AtomicUsize::new(0));
    let open_server = open.clone();
    let barrier = Arc::new(tokio::sync::Barrier::new(20));
    let task = tokio::spawn(async move {
        let mut tasks = tokio::task::JoinSet::new();
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            a.fetch_add(1, Ordering::SeqCst);
            open_server.fetch_add(1, Ordering::SeqCst);
            let open = open_server.clone();
            let seen = s.clone();
            tasks.spawn(serve(stream, open, seen, barrier.clone()));
        }
    });
    Server {
        url,
        accepts,
        seen,
        open,
        task,
    }
}
async fn serve(
    stream: tokio::net::TcpStream,
    open: Arc<AtomicUsize>,
    seen: Arc<Mutex<Vec<String>>>,
    barrier: Arc<tokio::sync::Barrier>,
) {
    let _open = Open(open);
    let mut io = BufReader::new(stream);
    loop {
        let mut request = String::new();
        loop {
            let mut line = String::new();
            if io.read_line(&mut line).await.unwrap_or(0) == 0 {
                return;
            }
            request.push_str(&line);
            if line == "\r\n" {
                break;
            }
        }
        let path = request
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .to_owned();
        // Proxy requests use absolute-form targets; route solely on the URL path.
        let path = if path.starts_with("http://") || path.starts_with("https://") {
            url::Url::parse(&path).unwrap().path().to_owned()
        } else {
            path
        };
        let method = request
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned();
        let length = request
            .lines()
            .find_map(|line| {
                line.split_once(':')
                    .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, value)| value.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);
        let mut body = vec![0; length];
        if io.read_exact(&mut body).await.is_err() {
            return;
        }
        request.push_str(&String::from_utf8_lossy(&body));
        seen.lock().unwrap().push(request);
        if path.contains("idle-expiry") {
            tokio::time::sleep(Duration::from_millis(5200)).await;
        }
        if path.contains("hold") {
            let mut byte = [0; 1];
            let _ = io.read(&mut byte).await;
            return;
        }
        if path.contains("slow") {
            let _ = io
                .get_mut()
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nx\r\n")
                .await;
            tokio::time::sleep(Duration::from_secs(2)).await;
            return;
        }
        if path.contains("delay") {
            barrier.wait().await;
        }
        if path.contains("gzip") {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&vec![b'x'; 4096]).unwrap();
            let compressed = encoder.finish().unwrap();
            let headers = if path.contains("lie") {
                "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 1\r\n\r\n".to_owned()
            } else {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n",
                    compressed.len()
                )
            };
            if io.get_mut().write_all(headers.as_bytes()).await.is_err() {
                return;
            }
            if io.get_mut().write_all(&compressed).await.is_err() {
                return;
            }
            let _ = io.get_mut().write_all(b"\r\n0\r\n\r\n").await;
            return;
        }
        let redirect_status = path
            .strip_prefix("/redirect")
            .filter(|v| !v.is_empty())
            .unwrap_or("302");
        let redirect_response = format!(
            "HTTP/1.1 {redirect_status} Redirect\r\nLocation: /ok\r\nContent-Length: 0\r\n\r\n"
        );
        let response = if path.contains("redirect") {
            redirect_response.as_str()
        } else if path.contains("429") {
            "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 1\r\nContent-Length: 0\r\n\r\n"
        } else if path.contains("truncated") {
            "HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\nx"
        } else if method == "HEAD" {
            "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
        } else {
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nSet-Cookie: session=one; Path=/\r\n\r\nok"
        };
        if io.get_mut().write_all(response.as_bytes()).await.is_err() {
            return;
        }
        if path.contains("truncated") {
            return;
        }
    }
}

async fn wait_seen(s: &Server, count: usize) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while s.seen.lock().unwrap().len() < count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
async fn wait_open(s: &Server, count: usize) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while s.open.load(Ordering::SeqCst) != count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

// Pool-contract cases exercise the default trust store. Environment CA overrides
// deliberately select isolated clients; test those separately. Re-exec rather
// than mutating process-global environment alongside parallel Tokio tests.
fn default_ca_environment(test: &str) -> bool {
    if std::env::var_os("SSL_CERT_FILE").is_none() && std::env::var_os("SSL_CERT_DIR").is_none() {
        return false;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env_remove("SSL_CERT_FILE")
        .env_remove("SSL_CERT_DIR")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "default-CA pool subprocess failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

fn session(http: &AdapterHttp) -> Session {
    http.session(
        Options {
            trust_env: false,
            ..Options::default()
        },
        tokio::time::Instant::now() + Duration::from_secs(3),
        Cancellation::default(),
    )
    .unwrap()
}
// Default trust_env false intentionally uses per-session clients, matching Python explicit overrides.
// Use a caller-specified explicit proxy (same local server) to prove adapter-owned cross-search reuse
// independently of the test runner's global environment proxy variables.
fn pooled(http: &AdapterHttp, server: &Server) -> Session {
    http.session(
        Options {
            proxy: Some(server.url.clone()),
            ..Options::default()
        },
        tokio::time::Instant::now() + Duration::from_secs(3),
        Cancellation::default(),
    )
    .unwrap()
}
#[tokio::test]
async fn tcp_reuse_and_search_identity_isolation() {
    if default_ca_environment("tcp_reuse_and_search_identity_isolation") {
        return;
    }
    let s = server().await;
    let http = AdapterHttp::default();
    for n in 0..20 {
        let mut client = pooled(&http, &s);
        if n == 0 {
            client.headers.insert(
                header::AUTHORIZATION,
                HeaderValue::from_static("Bearer tenant-one"),
            );
            client
                .headers
                .insert("x-tenant", HeaderValue::from_static("one"));
        }
        client
            .execute(Request::get("http://upstream.invalid/ok").unwrap())
            .await
            .unwrap();
        if n == 0 {
            client
                .execute(Request::get("http://upstream.invalid/ok").unwrap())
                .await
                .unwrap();
        }
    }
    assert_eq!(s.accepts.load(Ordering::SeqCst), 1);
    let seen = s.seen.lock().unwrap();
    assert!(seen[0].contains("authorization: Bearer tenant-one"));
    assert!(!seen[0].contains("cookie:"));
    assert!(seen[1].contains("cookie: session=one"));
    for request in &seen[2..] {
        assert!(!request.contains("cookie:"));
        assert!(!request.contains("authorization:"));
        assert!(!request.contains("x-tenant:"));
    }
    drop(seen);
    http.shutdown();
    http.shutdown();
    assert!(matches!(
        http.session(
            Options::default(),
            tokio::time::Instant::now(),
            Cancellation::default()
        ),
        Err(Error::Closed)
    ));
}
#[tokio::test]
async fn deadline_cancel_shutdown_and_429_never_retry() {
    let s = server().await;
    let http = AdapterHttp::default();
    let mut c = session(&http);
    let r = c
        .execute(Request::get(&format!("{}/429", s.url)).unwrap())
        .await
        .unwrap();
    assert_eq!(r.status, 429);
    assert_eq!(s.seen.lock().unwrap().len(), 1);
    let mut c = http
        .session(
            Options {
                trust_env: false,
                ..Options::default()
            },
            tokio::time::Instant::now() + Duration::from_millis(300),
            Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        c.execute(Request::get(&format!("{}/slow", s.url)).unwrap())
            .await
            .unwrap_err(),
        Error::Deadline
    );
    let cancel = Cancellation::default();
    let mut c = http
        .session(
            Options {
                trust_env: false,
                ..Options::default()
            },
            tokio::time::Instant::now() + Duration::from_secs(3),
            cancel.clone(),
        )
        .unwrap();
    let url = format!("{}/slow", s.url);
    let task = tokio::spawn(async move { c.execute(Request::get(&url).unwrap()).await });
    wait_seen(&s, 3).await;
    cancel.cancel();
    assert_eq!(task.await.unwrap().unwrap_err(), Error::Cancelled);
    let mut c = session(&http);
    let http = Arc::new(http);
    let h = http.clone();
    let url = format!("{}/slow", s.url);
    let task = tokio::spawn(async move { c.execute(Request::get(&url).unwrap()).await });
    wait_seen(&s, 4).await;
    h.shutdown();
    assert_eq!(task.await.unwrap().unwrap_err(), Error::Closed);
    assert_eq!(s.seen.lock().unwrap().len(), 4);
}
#[tokio::test]
async fn truncated_and_wire_limit_no_retry() {
    let s = server().await;
    let http = AdapterHttp::default();
    assert_eq!(
        session(&http)
            .execute(Request::get(&format!("{}/truncated", s.url)).unwrap())
            .await
            .unwrap_err(),
        Error::Network
    );
    let mut c = http
        .session(
            Options {
                trust_env: false,
                bounds: Bounds {
                    wire_bytes: 1,
                    ..Bounds::default()
                },
                ..Options::default()
            },
            tokio::time::Instant::now() + Duration::from_secs(3),
            Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        c.execute(Request::get(&s.url).unwrap()).await.unwrap_err(),
        Error::WireBodyLimit
    );
    assert_eq!(s.seen.lock().unwrap().len(), 2);
}
#[tokio::test]
async fn proxy_churn_bounded_overflow_ephemeral() {
    if default_ca_environment("proxy_churn_bounded_overflow_ephemeral") {
        return;
    }
    let http = AdapterHttp::default();
    let mut servers = Vec::new();
    for _ in 0..10 {
        let s = server().await;
        for _ in 0..2 {
            pooled(&http, &s)
                .execute(Request::get("http://upstream.invalid/").unwrap())
                .await
                .unwrap();
        }
        servers.push(s);
    }
    assert_eq!(http.retained_pools(), 8);
    for (n, s) in servers.iter().enumerate() {
        assert_eq!(s.accepts.load(Ordering::SeqCst), if n < 8 { 1 } else { 2 });
    }
    http.shutdown();
}
fn response(url: &str, status: u16, headers: HeaderMap, body: Vec<u8>) -> Response {
    Response {
        url: reqwest::Url::parse(url).unwrap(),
        status,
        headers,
        body,
    }
}
#[tokio::test]
async fn replay_malformed_bodies_and_caller_ownership() {
    let http = AdapterHttp::default();
    let bodies = [
        b"<html><broken".to_vec(),
        b"<xml><".to_vec(),
        b"{bad json".to_vec(),
    ];
    let replay = Arc::new(Replay::new(
        bodies
            .iter()
            .map(|b| {
                (
                    native_expected(Request::get("https://fixture.invalid/").unwrap()),
                    response("https://fixture.invalid/", 200, HeaderMap::new(), b.clone()),
                )
            })
            .collect(),
    ));
    http.set_transport(replay.clone()).unwrap();
    let mut c = session(&http);
    for body in bodies {
        assert_eq!(
            c.execute(Request::get("https://fixture.invalid/").unwrap())
                .await
                .unwrap()
                .body,
            body
        );
    }
    assert_eq!(replay.remaining(), 0);
    http.shutdown();
    assert_eq!(Arc::strong_count(&replay), 3);
    assert_eq!(
        c.execute(Request::get("https://fixture.invalid/").unwrap())
            .await
            .unwrap_err(),
        Error::Closed
    );
    assert!(http.set_transport(replay).is_err());
}
#[tokio::test]
async fn gzip_expansion_and_request_bounds() {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&vec![b'x'; 4096]).unwrap();
    let compressed = encoder.finish().unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("1"));
    let replay = Arc::new(Replay::new(vec![(
        native_expected(Request::get("https://fixture.invalid/").unwrap()),
        response("https://fixture.invalid/", 200, headers, compressed),
    )]));
    let http = AdapterHttp::default();
    http.set_transport(replay.clone()).unwrap();
    let mut c = http
        .session(
            Options {
                bounds: Bounds {
                    decoded_bytes: 100,
                    ..Bounds::default()
                },
                ..Options::default()
            },
            tokio::time::Instant::now() + Duration::from_secs(2),
            Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        c.execute(Request::get("https://fixture.invalid/").unwrap())
            .await
            .unwrap_err(),
        Error::DecodedBodyLimit
    );
    let mut r = Request::get("https://fixture.invalid/").unwrap();
    r.body = vec![0; 2 * 1024 * 1024 + 1];
    assert_eq!(c.execute(r).await.unwrap_err(), Error::RequestBodyLimit);
    assert_eq!(replay.remaining(), 0);
}
#[tokio::test]
async fn redirect_rules_strip_credentials_and_preserve_307_body() {
    for status in [301, 302, 303, 307, 308] {
        let http = AdapterHttp::default();
        let mut request = Request::get("https://one.invalid/").unwrap();
        request.method = Method::POST;
        request.body = b"payload".to_vec();
        request.headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer secret"),
        );
        request
            .headers
            .insert(header::COOKIE, HeaderValue::from_static("secret=one"));
        let mut second = Request::get("https://two.invalid/").unwrap();
        if status == 307 || status == 308 {
            second.method = Method::POST;
            second.body = b"payload".to_vec();
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            header::LOCATION,
            HeaderValue::from_static("https://two.invalid/"),
        );
        let replay = Arc::new(Replay::new(vec![
            (
                native_expected(request.clone()),
                response("https://one.invalid/", status, headers, vec![]),
            ),
            (
                native_expected(second),
                response("https://two.invalid/", 200, HeaderMap::new(), vec![]),
            ),
        ]));
        http.set_transport(replay.clone()).unwrap();
        let mut c = http
            .session(
                Options {
                    follow_redirects: true,
                    ..Options::default()
                },
                tokio::time::Instant::now() + Duration::from_secs(2),
                Cancellation::default(),
            )
            .unwrap();
        assert_eq!(c.execute(request).await.unwrap().status, 200);
        assert_eq!(replay.remaining(), 0);
    }
}
#[tokio::test]
async fn request_count_and_redirect_limits() {
    let http = AdapterHttp::default();
    let mut headers = HeaderMap::new();
    headers.insert(header::LOCATION, HeaderValue::from_static("/"));
    let replay = Arc::new(Replay::new(vec![(
        native_expected(Request::get("https://fixture.invalid/").unwrap()),
        response("https://fixture.invalid/", 302, headers, vec![]),
    )]));
    http.set_transport(replay).unwrap();
    let mut c = http
        .session(
            Options {
                follow_redirects: true,
                bounds: Bounds {
                    redirects: 0,
                    requests: 1,
                    ..Bounds::default()
                },
                ..Options::default()
            },
            tokio::time::Instant::now() + Duration::from_secs(2),
            Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        c.execute(Request::get("https://fixture.invalid/").unwrap())
            .await
            .unwrap_err(),
        Error::RedirectLimit
    );
    assert_eq!(
        c.execute(Request::get("https://fixture.invalid/").unwrap())
            .await
            .unwrap_err(),
        Error::RequestLimit
    );
}
#[tokio::test]
async fn twenty_active_ten_idle_and_shutdown_closes_sockets() {
    if default_ca_environment("twenty_active_ten_idle_and_shutdown_closes_sockets") {
        return;
    }
    let s = server().await;
    let http = AdapterHttp::default();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..20 {
        let mut c = pooled(&http, &s);
        tasks.spawn(async move {
            c.execute(Request::get("http://upstream.invalid/delay").unwrap())
                .await
                .unwrap();
        });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }
    wait_open(&s, 10).await;
    assert_eq!(s.accepts.load(Ordering::SeqCst), 20);
    assert_eq!(s.open.load(Ordering::SeqCst), 10);
    http.shutdown();
    wait_open(&s, 0).await;
    assert_eq!(s.open.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn queue_time_counts_toward_aggregate_deadline() {
    if default_ca_environment("queue_time_counts_toward_aggregate_deadline") {
        return;
    }
    // Establish actual saturation before starting the queued request's clock.
    // Creating all deadlines before the first poll assumed scheduler timing.
    let s = server().await;
    let http = AdapterHttp::default();
    let cancel = Cancellation::default();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..20 {
        let mut c = http
            .session(
                Options {
                    proxy: Some(s.url.clone()),
                    ..Options::default()
                },
                tokio::time::Instant::now() + Duration::from_secs(30),
                cancel.clone(),
            )
            .unwrap();
        tasks.spawn(async move {
            c.execute(Request::get("http://upstream.invalid/hold").unwrap())
                .await
        });
    }
    tokio::time::timeout(Duration::from_secs(10), async {
        while s.seen.lock().unwrap().len() < 20 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("twenty holder requests must reach the peer before timing queue wait");
    assert_eq!(s.seen.lock().unwrap().len(), 20);
    assert_eq!(s.open.load(Ordering::SeqCst), 20);
    let queued_deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let mut queued = http
        .session(
            Options {
                proxy: Some(s.url.clone()),
                ..Options::default()
            },
            queued_deadline,
            Cancellation::default(),
        )
        .unwrap();
    let before_execute = tokio::time::Instant::now();
    assert!(
        queued_deadline.saturating_duration_since(before_execute) >= Duration::from_secs(1),
        "queued request must enter execute with a meaningful unexpired budget"
    );
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(5),
            queued.execute(Request::get("http://upstream.invalid/queued").unwrap())
        )
        .await
        .expect("queued deadline must complete while holders stay active")
        .unwrap_err(),
        Error::Deadline
    );
    assert!(
        before_execute.elapsed() >= Duration::from_secs(1),
        "deadline must expire while execute waits for occupied capacity"
    );
    assert_eq!(
        s.seen.lock().unwrap().len(),
        20,
        "queued request must never dispatch"
    );
    assert_eq!(s.open.load(Ordering::SeqCst), 20);
    cancel.cancel();
    let mut completed = 0;
    while let Some(result) = tasks.join_next().await {
        assert_eq!(result.unwrap().unwrap_err(), Error::Cancelled);
        completed += 1;
    }
    assert_eq!(completed, 20);
    http.shutdown();
    wait_open(&s, 0).await;
}
#[tokio::test]
async fn idle_expiry_reconnects() {
    if default_ca_environment("idle_expiry_reconnects") {
        return;
    }
    let s = server().await;
    let http = AdapterHttp::default();
    pooled(&http, &s)
        .execute(Request::get("http://upstream.invalid/").unwrap())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(5100)).await;
    pooled(&http, &s)
        .execute(Request::get("http://upstream.invalid/").unwrap())
        .await
        .unwrap();
    assert_eq!(s.accepts.load(Ordering::SeqCst), 2);
}
#[tokio::test]
#[ignore = "subprocess helper; parent supplies isolated environment"]
async fn env_proxy_child() {
    let url = std::env::var("F08_TEST_URL").unwrap();
    let http = AdapterHttp::default();
    let mode = std::env::var("F08_CA_MODE").unwrap_or_default();
    let options = if mode == "ignore" {
        Options {
            verify_tls: false,
            trust_env: false,
            ..Options::default()
        }
    } else {
        Options::default()
    };
    let result = http.session(
        options,
        tokio::time::Instant::now() + Duration::from_secs(2),
        Cancellation::default(),
    );
    if mode == "invalid" {
        assert!(matches!(result, Err(Error::InvalidRequest)));
        return;
    }
    let mut c = result.unwrap();
    assert_eq!(
        c.execute(Request::get(&url).unwrap()).await.unwrap().status,
        200
    );
    assert_eq!(http.retained_pools(), if mode == "direct" { 1 } else { 0 });
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_proxy_and_no_proxy_use_real_subprocesses() {
    // Absolute-form proxy targets must not route on digits in their authority.
    let authority_probe = server().await;
    let mut probe =
        tokio::net::TcpStream::connect(authority_probe.url.strip_prefix("http://").unwrap())
            .await
            .unwrap();
    probe
        .write_all(b"GET http://127.0.0.1:42999/ HTTP/1.1\r\nHost: 127.0.0.1:42999\r\n\r\n")
        .await
        .unwrap();
    let mut status = String::new();
    BufReader::new(probe).read_line(&mut status).await.unwrap();
    assert_eq!(status, "HTTP/1.1 200 OK\r\n");
    let target = server().await;
    let proxy = server().await;
    for bypass in [false, true] {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--ignored", "--exact", "env_proxy_child"])
            .env("F08_TEST_URL", &target.url)
            .env("HTTP_PROXY", &proxy.url)
            .env("NO_PROXY", if bypass { "127.0.0.1" } else { "" });
        for key in [
            "http_proxy",
            "https_proxy",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "all_proxy",
            "no_proxy",
        ] {
            command.env_remove(key);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(proxy.seen.lock().unwrap().len(), 1);
    assert_eq!(target.seen.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn tls_verification_default_override_and_custom_ca() {
    // Fresh native CA/leaf avoids a calendar-expiring fixture or an openssl dependency.
    let now = time::OffsetDateTime::now_utc();
    let mut ca = rcgen::CertificateParams::default();
    ca.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca.not_before = now - time::Duration::days(1);
    ca.not_after = now + time::Duration::days(3650);
    ca.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
    ];
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let ca_cert = ca.self_signed(&ca_key).unwrap();
    let issuer = rcgen::Issuer::new(ca, ca_key);
    let mut leaf =
        rcgen::CertificateParams::new(vec!["localhost".into(), "127.0.0.1".into()]).unwrap();
    leaf.not_before = now - time::Duration::days(1);
    leaf.not_after = now + time::Duration::days(1);
    leaf.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    let leaf_key = rcgen::KeyPair::generate().unwrap();
    let leaf_cert = leaf.signed_by(&leaf_key, &issuer).unwrap();
    let cert = leaf_cert.der().clone();
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(leaf_key.serialize_der());
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key.into())
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("https://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..3 {
            let (socket, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            tasks.spawn(async move {
                if let Ok(stream) = acceptor.accept(socket).await {
                    let mut stream = BufReader::new(stream);
                    loop {
                        let mut line = String::new();
                        if stream.read_line(&mut line).await.unwrap_or(0) == 0 {
                            return;
                        }
                        if line == "\r\n" {
                            break;
                        }
                    }
                    stream
                        .get_mut()
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                        .await
                        .unwrap();
                }
            });
        }
        while tasks.join_next().await.is_some() {}
    });
    let http = AdapterHttp::default();
    let opts = Options {
        trust_env: false,
        ..Options::default()
    };
    let mut c = http
        .session(
            opts.clone(),
            tokio::time::Instant::now() + Duration::from_secs(3),
            Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        c.execute(Request::get(&url).unwrap()).await.unwrap_err(),
        Error::Network
    );
    for opts in [
        Options {
            verify_tls: false,
            ..opts.clone()
        },
        Options {
            root_certificates: vec![
                reqwest::Certificate::from_pem(ca_cert.pem().as_bytes()).unwrap(),
            ],
            ..opts
        },
    ] {
        let mut c = http
            .session(
                opts,
                tokio::time::Instant::now() + Duration::from_secs(3),
                Cancellation::default(),
            )
            .unwrap();
        assert_eq!(
            c.execute(Request::get(&url).unwrap()).await.unwrap().body,
            b"ok"
        );
    }
    server.await.unwrap();
}
#[test]
fn proxy_rotation_cooldown_failopen_and_dynamic_override() {
    let now = std::time::Instant::now();
    let mut p = ProxyRotation::new(vec!["a".into(), "b".into()], None, Duration::from_secs(2));
    assert_eq!(p.select(now).as_deref(), Some("a"));
    p.failure("a", now);
    assert_eq!(p.select(now).as_deref(), Some("b"));
    p.failure("b", now);
    assert_eq!(p.available(now), 0);
    assert_eq!(p.select(now).as_deref(), Some("a"));
    assert_eq!(p.available(now), 1);
    p.failure("a", now);
    p.failure("a", now);
    assert_eq!(p.available(now + Duration::from_secs(3)), 1);
    p.success("a");
    assert_eq!(p.available(now + Duration::from_secs(3)), 2);
    let mut p = ProxyRotation::new(
        vec!["a".into()],
        Some("dynamic".into()),
        Duration::from_secs(2),
    );
    p.failure("a", now);
    assert_eq!(p.select(now).as_deref(), Some("dynamic"));
}
#[tokio::test]
async fn replay_denies_request_method_url_header_body_mutations() {
    for variant in 0..4 {
        let expected = Request::get("https://fixture.invalid/").unwrap();
        let mut actual = expected.clone();
        match variant {
            0 => actual.method = Method::POST,
            1 => actual.url.set_path("/other"),
            2 => {
                actual
                    .headers
                    .insert("x-mutation", HeaderValue::from_static("yes"));
            }
            _ => actual.body = b"other".to_vec(),
        };
        let http = AdapterHttp::default();
        http.set_transport(Arc::new(Replay::new(vec![(
            native_expected(expected),
            response("https://fixture.invalid/", 200, HeaderMap::new(), vec![]),
        )])))
        .unwrap();
        assert_eq!(
            session(&http).execute(actual).await.unwrap_err(),
            Error::InvalidRequest
        );
    }
}
#[tokio::test]
async fn chunked_compression_expansion_and_false_length() {
    let s = server().await;
    let http = AdapterHttp::default();
    let mut c = http
        .session(
            Options {
                trust_env: false,
                bounds: Bounds {
                    decoded_bytes: 100,
                    ..Bounds::default()
                },
                ..Options::default()
            },
            tokio::time::Instant::now() + Duration::from_secs(3),
            Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        c.execute(Request::get(&format!("{}/gzip", s.url)).unwrap())
            .await
            .unwrap_err(),
        Error::DecodedBodyLimit
    );
    assert_eq!(
        c.execute(Request::get(&format!("{}/gzip-lie", s.url)).unwrap())
            .await
            .unwrap_err(),
        Error::InvalidResponse
    );
    assert_eq!(s.seen.lock().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_ca_override_explicit_ignore_and_no_proxy_only() {
    let target = server().await;
    for mode in ["invalid", "ignore", "direct"] {
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args(["--ignored", "--exact", "env_proxy_child"])
            .env("F08_TEST_URL", &target.url)
            .env("F08_CA_MODE", mode)
            .env("NO_PROXY", "127.0.0.1");
        for key in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "no_proxy",
            "SSL_CERT_DIR",
        ] {
            cmd.env_remove(key);
        }
        if mode == "direct" {
            cmd.env_remove("SSL_CERT_FILE");
        } else {
            cmd.env("SSL_CERT_FILE", "/nonexistent-f08-test-ca.pem");
        }
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(target.seen.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn empty_roots_only_rejected_before_pool_reuse() {
    if default_ca_environment("empty_roots_only_rejected_before_pool_reuse") {
        return;
    }
    for prior_use in [false, true] {
        let server = server().await;
        let http = AdapterHttp::default();
        if prior_use {
            pooled(&http, &server)
                .execute(Request::get("http://upstream.invalid/").unwrap())
                .await
                .unwrap();
        }
        let result = http.session(
            Options {
                proxy: Some(server.url.clone()),
                trust_roots_only: true,
                ..Options::default()
            },
            tokio::time::Instant::now() + Duration::from_secs(2),
            Cancellation::default(),
        );
        assert!(matches!(result, Err(Error::InvalidRequest)));
        assert_eq!(http.retained_pools(), usize::from(prior_use));
        pooled(&http, &server)
            .execute(Request::get("http://upstream.invalid/").unwrap())
            .await
            .unwrap();
        assert_eq!(server.accepts.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn socket_redirect_method_and_body_matrix() {
    let server = server().await;
    let http = AdapterHttp::default();
    for status in [301, 302, 303, 307, 308] {
        for method in [
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::HEAD,
        ] {
            let mut client = http
                .session(
                    Options {
                        trust_env: false,
                        follow_redirects: true,
                        ..Options::default()
                    },
                    tokio::time::Instant::now() + Duration::from_secs(3),
                    Cancellation::default(),
                )
                .unwrap();
            let mut request = Request::get(&format!("{}/redirect{status}", server.url)).unwrap();
            request.method = method.clone();
            request.body = b"payload".to_vec();
            request
                .headers
                .insert(header::COOKIE, HeaderValue::from_static("explicit=initial"));
            request.headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            assert_eq!(client.execute(request).await.unwrap().status, 200);
            let seen = server.seen.lock().unwrap();
            let before = &seen[seen.len() - 2];
            let after = &seen[seen.len() - 1];
            assert!(before.starts_with(method.as_str()));
            assert!(before.ends_with("payload"));
            let rewritten = method != Method::GET
                && ((status == 301 && method == Method::POST)
                    || (matches!(status, 302 | 303) && method != Method::HEAD));
            let (headers, body) = after.split_once("\r\n\r\n").unwrap();
            assert!(!headers.contains("cookie:"));
            if rewritten {
                assert!(headers.starts_with("GET "));
                assert_eq!(body, "");
                assert!(!headers.contains("content-length:"));
                assert!(!headers.contains("transfer-encoding:"));
                assert!(headers.contains("content-type: application/json"));
            } else {
                assert!(headers.starts_with(&format!("{} ", method)));
                assert_eq!(body, "payload");
                assert!(headers.contains("content-type: application/json"));
            }
        }
    }
    assert_eq!(server.seen.lock().unwrap().len(), 60);
}
#[tokio::test]
async fn repeated_content_encoding_headers_decode_in_wire_order() {
    let body = b"publication response";
    let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    zlib.write_all(body).unwrap();
    let zlib = zlib.finish().unwrap();
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&zlib).unwrap();
    let encoded = gzip.finish().unwrap();
    for (values, expected) in [
        (vec!["deflate", "gzip"], Ok(body.to_vec())),
        (vec!["gzip", "deflate"], Err(Error::InvalidResponse)),
        (
            vec!["deflate", "gzip, unknown"],
            Err(Error::UnsupportedEncoding),
        ),
    ] {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append(
                header::CONTENT_ENCODING,
                HeaderValue::from_str(value).unwrap(),
            );
        }
        let http = AdapterHttp::default();
        http.set_transport(Arc::new(Replay::new(vec![(
            native_expected(Request::get("https://fixture.invalid/").unwrap()),
            response("https://fixture.invalid/", 200, headers, encoded.clone()),
        )])))
        .unwrap();
        assert_eq!(
            session(&http)
                .execute(Request::get("https://fixture.invalid/").unwrap())
                .await
                .map(|r| r.body),
            expected
        );
    }
}

#[tokio::test]
async fn adapter_admission_spans_routes_and_ephemeral_sessions() {
    if default_ca_environment("adapter_admission_spans_routes_and_ephemeral_sessions") {
        return;
    }
    let first = server().await;
    let second = server().await;
    let http = AdapterHttp::default();
    let cancel = Cancellation::default();
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..20 {
        let options = if index < 10 {
            Options {
                proxy: Some(if index % 2 == 0 {
                    first.url.clone()
                } else {
                    second.url.clone()
                }),
                ..Options::default()
            }
        } else {
            Options {
                trust_env: false,
                ..Options::default()
            }
        };
        let mut session = http
            .session(
                options,
                tokio::time::Instant::now() + Duration::from_secs(10),
                if index == 0 {
                    cancel.clone()
                } else {
                    Cancellation::default()
                },
            )
            .unwrap();
        let url = format!("{}/hold", first.url);
        tasks.spawn(async move { session.execute(Request::get(&url).unwrap()).await });
    }
    wait_seen(&first, 15).await;
    wait_seen(&second, 5).await;
    let mut queued = http
        .session(
            Options {
                trust_env: false,
                ..Options::default()
            },
            tokio::time::Instant::now() + Duration::from_millis(100),
            Cancellation::default(),
        )
        .unwrap();
    assert_eq!(
        queued
            .execute(Request::get(&format!("{}/hold", first.url)).unwrap())
            .await
            .unwrap_err(),
        Error::Deadline
    );
    assert_eq!(
        first.accepts.load(Ordering::SeqCst) + second.accepts.load(Ordering::SeqCst),
        20
    );
    cancel.cancel();
    assert_eq!(
        tasks.join_next().await.unwrap().unwrap().unwrap_err(),
        Error::Cancelled
    );
    let mut replacement = session(&http);
    assert_eq!(
        replacement
            .execute(Request::get(&first.url).unwrap())
            .await
            .unwrap()
            .status,
        200
    );
    http.shutdown();
    while let Some(result) = tasks.join_next().await {
        assert_eq!(result.unwrap().unwrap_err(), Error::Closed);
    }
    wait_open(&first, 0).await;
    wait_open(&second, 0).await;
}

#[tokio::test]
async fn adapter_idle_budget_spans_routes_and_ephemeral_sessions() {
    if default_ca_environment("adapter_idle_budget_spans_routes_and_ephemeral_sessions") {
        return;
    }
    let first = server().await;
    let second = server().await;
    let http = AdapterHttp::default();
    let mut sessions = Vec::new();
    for index in 0..20 {
        let mut session = http
            .session(
                Options {
                    proxy: Some(if index % 2 == 0 {
                        first.url.clone()
                    } else {
                        second.url.clone()
                    }),
                    trust_env: index < 12,
                    ..Options::default()
                },
                tokio::time::Instant::now() + Duration::from_secs(10),
                Cancellation::default(),
            )
            .unwrap();
        session
            .execute(Request::get(&format!("http://origin{index}.invalid/ok")).unwrap())
            .await
            .unwrap();
        sessions.push(session);
    }
    tokio::time::timeout(Duration::from_secs(2), async {
        while first.open.load(Ordering::SeqCst) + second.open.load(Ordering::SeqCst) != 10 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        first.accepts.load(Ordering::SeqCst) + second.accepts.load(Ordering::SeqCst),
        20
    );
    http.shutdown();
    wait_open(&first, 0).await;
    wait_open(&second, 0).await;
    drop(sessions);
}

#[tokio::test]
async fn expired_idle_routes_release_adapter_retention_before_checkout_and_return() {
    if default_ca_environment(
        "expired_idle_routes_release_adapter_retention_before_checkout_and_return",
    ) {
        return;
    }
    for expire_during_response in [false, true] {
        let old = server().await;
        let next = server().await;
        let http = AdapterHttp::default();
        for n in 0..10 {
            pooled(&http, &old)
                .execute(Request::get(&format!("http://old{n}.invalid/ok")).unwrap())
                .await
                .unwrap();
        }
        assert_eq!(old.accepts.load(Ordering::SeqCst), 10);
        if !expire_during_response {
            tokio::time::sleep(Duration::from_millis(5200)).await;
            wait_open(&old, 0).await;
        }
        let mut first = http
            .session(
                Options {
                    proxy: Some(next.url.clone()),
                    ..Options::default()
                },
                tokio::time::Instant::now() + Duration::from_secs(10),
                Cancellation::default(),
            )
            .unwrap();
        let path = if expire_during_response {
            "idle-expiry"
        } else {
            "ok"
        };
        first
            .execute(Request::get(&format!("http://new.invalid/{path}")).unwrap())
            .await
            .unwrap();
        wait_open(&old, 0).await;
        for _ in 0..2 {
            pooled(&http, &next)
                .execute(Request::get("http://new.invalid/ok").unwrap())
                .await
                .unwrap();
        }
        assert_eq!(next.accepts.load(Ordering::SeqCst), 1);
        http.shutdown();
        wait_open(&next, 0).await;
    }
}

// Native replay templates declare the reviewed shared default explicitly;
// Replay itself still compares complete headers without normalizing expectations.
fn native_expected(mut request: Request) -> Request {
    if !request.headers.contains_key(header::USER_AGENT) {
        request.headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static(DEFAULT_USER_AGENT),
        );
    }
    request
}

#[tokio::test]
async fn native_user_agent_socket_precedence_redirect_and_session_identity_isolation() {
    if default_ca_environment(
        "native_user_agent_socket_precedence_redirect_and_session_identity_isolation",
    ) {
        return;
    }
    assert_eq!(
        DEFAULT_USER_AGENT,
        concat!(
            "SlopSearX/",
            env!("CARGO_PKG_VERSION"),
            " (meta search engine; agent-native)"
        )
    );
    let s = server().await;
    let http = AdapterHttp::default();
    let url = format!("{}/ok", s.url);
    let mut first = session(&http);
    first.execute(Request::get(&url).unwrap()).await.unwrap();
    first.headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("session-agent"),
    );
    first.headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer tenant-one"),
    );
    let mut request = Request::get(&url).unwrap();
    request.headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("request-agent"),
    );
    first.execute(request).await.unwrap();
    first.execute(Request::get(&url).unwrap()).await.unwrap();
    drop(first);
    session(&http)
        .execute(Request::get(&url).unwrap())
        .await
        .unwrap();
    let mut redirect = http
        .session(
            Options {
                trust_env: false,
                follow_redirects: true,
                ..Default::default()
            },
            tokio::time::Instant::now() + Duration::from_secs(5),
            Cancellation::default(),
        )
        .unwrap();
    redirect.headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("session-redirect"),
    );
    redirect.headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer route"),
    );
    let mut request = Request::get(&format!("{}/redirect307", s.url)).unwrap();
    request.headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("request-redirect"),
    );
    redirect.execute(request).await.unwrap();
    drop(redirect);
    session(&http)
        .execute(Request::get(&url).unwrap())
        .await
        .unwrap();
    {
        let seen = s.seen.lock().unwrap();
        assert_eq!(seen.len(), 7);
        let value = |at: usize, name: &str| {
            seen[at].lines().find_map(|line| {
                line.split_once(':')
                    .filter(|(n, _)| n.eq_ignore_ascii_case(name))
                    .map(|(_, v)| v.trim().to_owned())
            })
        };
        for at in [0, 3, 6] {
            assert_eq!(value(at, "user-agent").as_deref(), Some(DEFAULT_USER_AGENT));
            assert!(value(at, "authorization").is_none());
            assert!(value(at, "cookie").is_none());
        }
        assert_eq!(value(1, "user-agent").as_deref(), Some("request-agent"));
        assert_eq!(value(2, "user-agent").as_deref(), Some("session-agent"));
        for at in [1, 2] {
            assert_eq!(
                value(at, "authorization").as_deref(),
                Some("Bearer tenant-one")
            );
            assert_eq!(value(at, "cookie").as_deref(), Some("session=one"));
        }
        for at in [4, 5] {
            assert_eq!(value(at, "user-agent").as_deref(), Some("request-redirect"));
            assert_eq!(value(at, "authorization").as_deref(), Some("Bearer route"));
        }
    }
    http.shutdown_and_wait().await;
}
