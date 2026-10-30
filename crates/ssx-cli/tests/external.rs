//! Real native CLI child against an owned loopback TCP fixture. No Python/backend.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
const CASES: &[u8] = include_bytes!("../../../docs/rust-rewrite/t08a/cases.json");
const INVENTORY: &str = include_str!("../../../docs/rust-rewrite/t08a/inventory.json");
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema: u32,
    plan: String,
    cases: Vec<Case>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    argv: Vec<String>,
    env: BTreeMap<String, String>,
    #[serde(default)]
    env_os_invalid: BTreeMap<String, String>,
    #[serde(default)]
    base_suffix: String,
    #[serde(default)]
    tls: String,
    #[serde(default)]
    tls_protocol: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    proxy: Option<ProxyConfig>,
    #[serde(default)]
    raw_ca_file: bool,
    #[serde(default)]
    proxy_requests: Vec<Request>,
    replies: Vec<Reply>,
    requests: Vec<Request>,
    status: i32,
    stdout: String,
    #[serde(default)]
    stdout_sha256: Option<String>,
    #[serde(default)]
    stdout_bytes: Option<usize>,
    #[serde(default)]
    stdout_contains: Vec<String>,
    stderr: String,
    reference: String,
    #[serde(default)]
    intentional_difference: Option<String>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProxyConfig {
    tls: bool,
    tunnel: bool,
    status: u16,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    status: u16,
    headers: BTreeMap<String, String>,
    body_hex: String,
    #[serde(default)]
    body_repeat: usize,
    #[serde(default)]
    body_kind: String,
    #[serde(default)]
    body_nesting: usize,
    #[serde(default)]
    wire: String,
    close: bool,
    delay_ms: u64,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Request {
    #[serde(default = "get_method")]
    method: String,
    target: String,
    accept: String,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    referer: String,
    #[serde(default)]
    proxy_auth: String,
}
fn get_method() -> String {
    "GET".into()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(s: &str) -> Vec<u8> {
    assert_eq!(s.len() % 2, 0);
    (0..s.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&s[at..at + 2], 16).unwrap())
        .collect()
}
fn validate(corpus: &Corpus) -> Result<(), String> {
    if corpus.schema != 1 || corpus.plan != "T08A" {
        return Err("corpus identity".into());
    }
    let ids = corpus
        .cases
        .iter()
        .map(|c| c.id.as_str())
        .collect::<Vec<_>>();
    let expected: Vec<String> = serde_json::from_str(INVENTORY).unwrap();
    if ids != expected.iter().map(String::as_str).collect::<Vec<_>>() {
        return Err("finite case inventory changed".into());
    }
    if ids
        .iter()
        .map(|id| id.to_ascii_lowercase())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != ids.len()
    {
        return Err("case-insensitive artifact collision".into());
    }
    if ids.iter().collect::<std::collections::BTreeSet<_>>().len() != ids.len() {
        return Err("duplicate cases".into());
    }
    if corpus.cases.iter().any(|c| {
        c.intentional_difference.as_deref().is_some_and(|d| {
            !matches!(
                d,
                "unordered-NaN-category-comparison"
                    | "sanitized-diagnostics-help"
                    | "proxy-credential-hop-scope"
                    | "strict-chunk-framing"
            )
        })
    }) {
        return Err("undeclared intentional difference".into());
    }
    if corpus.cases.iter().any(|c| c.reference.is_empty()) {
        return Err("missing provenance".into());
    }
    Ok(())
}
struct Fixture {
    address: std::net::SocketAddr,
    done: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<Request>>>,
    worker: Option<thread::JoinHandle<()>>,
    producer: Arc<Mutex<Vec<serde_json::Value>>>,
}
impl Fixture {
    fn start(
        mut replies: Vec<Reply>,
        tls: &str,
        protocol: &str,
        tunnel: Option<(std::net::SocketAddr, u16)>,
    ) -> Self {
        let acceptor = if tls.is_empty() {
            None
        } else {
            use openssl::{
                pkey::PKey,
                ssl::{SslAcceptor, SslMethod},
                x509::X509,
            };
            let pem = match tls {
                "server" => {
                    include_bytes!("../../../docs/rust-rewrite/t08a/tls/server.pem").as_slice()
                }
                "expired" => {
                    include_bytes!("../../../docs/rust-rewrite/t08a/tls/expired.pem").as_slice()
                }
                "critical" => {
                    include_bytes!("../../../docs/rust-rewrite/t08a/tls/critical.pem").as_slice()
                }
                "missing-intermediate" => {
                    include_bytes!("../../../docs/rust-rewrite/t08a/tls/missing-intermediate.pem")
                        .as_slice()
                }
                "with-intermediate" => {
                    include_bytes!("../../../docs/rust-rewrite/t08a/tls/with-intermediate.pem")
                        .as_slice()
                }
                "weak-chain" => {
                    include_bytes!("../../../docs/rust-rewrite/t08a/tls/weak-chain.pem").as_slice()
                }
                "wrong-purpose" => {
                    include_bytes!("../../../docs/rust-rewrite/t08a/tls/wrong-purpose.pem")
                        .as_slice()
                }
                "cn-fallback" => {
                    include_bytes!("../../../docs/rust-rewrite/t08a/tls/cn-fallback.pem").as_slice()
                }
                "partial-wildcard" => {
                    include_bytes!("../../../docs/rust-rewrite/t08a/tls/partial-wildcard.pem")
                        .as_slice()
                }
                _ => panic!("unknown TLS fixture"),
            };
            let mut a = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls()).unwrap();
            a.set_private_key(
                &PKey::private_key_from_pem(include_bytes!(
                    "../../../docs/rust-rewrite/t08a/tls/server.key"
                ))
                .unwrap(),
            )
            .unwrap();
            let mut chain = X509::stack_from_pem(pem).unwrap().into_iter();
            a.set_certificate(&chain.next().unwrap()).unwrap();
            for cert in chain {
                a.add_extra_chain_cert(cert).unwrap();
            }
            if !protocol.is_empty() {
                let v = match protocol {
                    "TLSv1_2" => openssl::ssl::SslVersion::TLS1_2,
                    "TLSv1_3" => openssl::ssl::SslVersion::TLS1_3,
                    _ => panic!("unknown protocol"),
                };
                a.set_min_proto_version(Some(v)).unwrap();
                a.set_max_proto_version(Some(v)).unwrap();
            }
            a.check_private_key().unwrap();
            Some(a.build())
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        for reply in &mut replies {
            for value in reply.headers.values_mut() {
                *value = value
                    .replace("$ADDRESS", &address.to_string())
                    .replace("$BASE", &format!("http://{address}"))
                    .replace("$PORT", &address.port().to_string());
            }
        }
        listener.set_nonblocking(true).unwrap();
        let done = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let producer = Arc::new(Mutex::new(Vec::new()));
        let producer_events = producer.clone();
        let d = done.clone();
        let r = requests.clone();
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(27);
            while !d.load(Ordering::SeqCst) && Instant::now() < deadline {
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream
                            .set_nonblocking(false)
                            .expect("accepted fixture socket is blocking");
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let raw_read = stream.try_clone().unwrap();
                        trait Io: Read + Write {}
                        impl<T: Read + Write> Io for T {}
                        let mut stream: Box<dyn Io> = if let Some(a) = &acceptor {
                            match a.accept(stream) {
                                Ok(s) => Box::new(s),
                                Err(error) => {
                                    let kind = match error {
                                        openssl::ssl::HandshakeError::SetupFailure(_) => {
                                            "setup".into()
                                        }
                                        openssl::ssl::HandshakeError::Failure(mid) => {
                                            format!("tls-code-{}", mid.error().code().as_raw())
                                        }
                                        openssl::ssl::HandshakeError::WouldBlock(_) => {
                                            "would-block".into()
                                        }
                                    };
                                    producer_events.lock().unwrap().push(serde_json::json!({"stage":"tls-handshake","ok":false,"kind":kind}));
                                    continue;
                                }
                            }
                        } else {
                            Box::new(stream)
                        };
                        let mut bytes = Vec::new();
                        let mut byte = [0; 1];
                        while !bytes.ends_with(b"\r\n\r\n") && bytes.len() < 131072 {
                            if stream.read_exact(&mut byte).is_err() {
                                break;
                            }
                            bytes.push(byte[0]);
                        }
                        let header = bytes.iter().map(|b| char::from(*b)).collect::<String>();
                        let method = header
                            .lines()
                            .next()
                            .unwrap_or("")
                            .split(' ')
                            .next()
                            .unwrap_or("")
                            .to_owned();
                        let target = header
                            .lines()
                            .next()
                            .unwrap_or("")
                            .split(' ')
                            .nth(1)
                            .unwrap_or("")
                            .to_owned();
                        let accept = header
                            .lines()
                            .find_map(|s| {
                                s.split_once(':')
                                    .filter(|(k, _)| k.eq_ignore_ascii_case("accept"))
                                    .map(|(_, v)| v.trim().to_owned())
                            })
                            .unwrap_or_default();
                        let mut requests = r.lock().unwrap();
                        let index = requests.len();
                        let proxy_auth = header
                            .lines()
                            .find_map(|s| {
                                s.split_once(':')
                                    .filter(|(k, _)| k.eq_ignore_ascii_case("proxy-authorization"))
                                    .map(|(_, v)| v.trim().to_owned())
                            })
                            .unwrap_or_default();
                        let field = |name: &str| {
                            header
                                .lines()
                                .find_map(|s| {
                                    s.split_once(':')
                                        .filter(|(k, _)| k.eq_ignore_ascii_case(name))
                                        .map(|(_, v)| v.trim().to_owned())
                                })
                                .unwrap_or_default()
                        };
                        requests.push(Request {
                            method: method.clone(),
                            target,
                            accept,
                            proxy_auth,
                            host: Some(field("host")),
                            referer: field("referer"),
                        });
                        drop(requests);
                        if let Some((origin, status)) = tunnel {
                            if method != "CONNECT" {
                                stream
                                    .write_all(b"HTTP/1.1 400 Fixture\r\nContent-Length: 0\r\n\r\n")
                                    .ok();
                                continue;
                            }
                            stream
                                .write_all(format!("HTTP/1.1 {status} Fixture\r\n\r\n").as_bytes())
                                .ok();
                            if status != 200 {
                                continue;
                            }
                            let mut origin_write =
                                TcpStream::connect_timeout(&origin, Duration::from_secs(2))
                                    .unwrap();
                            origin_write
                                .set_read_timeout(Some(Duration::from_secs(2)))
                                .unwrap();
                            origin_write
                                .set_write_timeout(Some(Duration::from_secs(2)))
                                .unwrap();
                            let mut origin_read = origin_write.try_clone().unwrap();
                            let origin_abort = origin_write.try_clone().unwrap();
                            let proxy_abort = raw_read.try_clone().unwrap();
                            let upstream = thread::spawn(move || {
                                let mut raw_read = raw_read;
                                let _ = std::io::copy(&mut raw_read, &mut origin_write);
                                let _ = origin_write.shutdown(std::net::Shutdown::Write);
                            });
                            let _ = std::io::copy(&mut origin_read, &mut stream);
                            let _ = origin_abort.shutdown(std::net::Shutdown::Both);
                            let _ = proxy_abort.shutdown(std::net::Shutdown::Both);
                            upstream.join().expect("owned CONNECT copy joined");
                            producer_events.lock().unwrap().push(serde_json::json!({"stage":"connect-relay","origin":origin,"status":status,"both_copy_directions_joined":true}));
                            continue;
                        }

                        let reply = replies.get(index).cloned().unwrap_or(Reply {
                            status: 500,
                            headers: BTreeMap::new(),
                            body_hex: String::new(),
                            body_repeat: 0,
                            body_kind: String::new(),
                            body_nesting: 0,
                            wire: String::new(),
                            close: false,
                            delay_ms: 0,
                        });
                        if reply.close {
                            continue;
                        }
                        let until = Instant::now() + Duration::from_millis(reply.delay_ms);
                        while Instant::now() < until && !d.load(Ordering::SeqCst) {
                            thread::sleep(Duration::from_millis(5));
                        }
                        let body = if reply.body_repeat > 0 {
                            assert!(
                                reply.body_repeat <= 5 * 1024 * 1024,
                                "bounded repeat fixture"
                            );
                            match reply.body_kind.as_str() {
                                "float-fraction" => {
                                    format!("1.{}1e0", "0".repeat(reply.body_repeat)).into_bytes()
                                }
                                "float-exponent" => {
                                    format!("1e{}", "9".repeat(reply.body_repeat)).into_bytes()
                                }
                                "" => {
                                    let mut b = vec![b'"'];
                                    b.extend(std::iter::repeat_n(b'x', reply.body_repeat));
                                    b.push(b'"');
                                    b
                                }
                                _ => panic!("unknown repeat fixture"),
                            }
                        } else if reply.body_nesting > 0 {
                            assert!(reply.body_nesting <= 1000, "bounded nesting fixture");
                            format!(
                                "{}0{}",
                                "[".repeat(reply.body_nesting),
                                "]".repeat(reply.body_nesting)
                            )
                            .into_bytes()
                        } else {
                            unhex(&reply.body_hex)
                        };
                        let mut response = format!(
                            "HTTP/1.1 {} Fixture\r\nConnection: close\r\nContent-Length: {}\r\n",
                            reply.status,
                            body.len()
                        );
                        for (k, v) in reply.headers {
                            response.push_str(&format!("{k}: {v}\r\n"));
                        }
                        response.push_str("\r\n");
                        let response = if reply.wire.is_empty() {
                            response
                                .chars()
                                .map(|c| u8::try_from(u32::from(c)).expect("Latin1 source header"))
                                .collect::<Vec<_>>()
                        } else {
                            fixture_wire(&reply.wire)
                        };
                        let wire = !reply.wire.is_empty();
                        let header = stream.write_all(&response);
                        let mut sent = 0;
                        let mut failure = None;
                        if header.is_ok() {
                            while !wire && sent < body.len() {
                                match stream.write(&body[sent..]) {
                                    Ok(0) => {
                                        failure = Some("WriteZero".to_owned());
                                        break;
                                    }
                                    Ok(n) => sent += n,
                                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                                    Err(e) => {
                                        failure = Some(format!("{:?}", e.kind()));
                                        break;
                                    }
                                }
                            }
                        }
                        producer_events.lock().unwrap().push(serde_json::json!({"stage":"response","declared":body.len(),"header_ok":header.is_ok(),"sent":sent,"write_error_kind":failure}));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("fixture accept: {e}"),
                }
            }
        });
        Self {
            address,
            done,
            requests,
            producer,
            worker: Some(worker),
        }
    }
    fn finish(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            worker.join().expect("fixture joined");
        }
        assert!(
            TcpStream::connect_timeout(&self.address, Duration::from_millis(50)).is_err(),
            "fixture listener retired"
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
struct OwnedCa(std::path::PathBuf);
impl Drop for OwnedCa {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
impl OwnedCa {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        for _ in 0..32 {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("t08a-ca-{}-{now}-{n}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("owned CA fixture: {e}"),
            }
        }
        panic!("owned CA fixture collision budget")
    }
    fn file(&self) -> std::path::PathBuf {
        #[cfg(all(unix, not(target_os = "macos")))]
        let name = {
            use std::os::unix::ffi::OsStrExt;
            std::ffi::OsStr::from_bytes(b"ca-\xff.pem")
        };
        #[cfg(any(windows, target_os = "macos"))]
        let name = std::ffi::OsStr::new("ca-unicode-☃.pem");
        let path = self.0.join(name);
        fs::write(
            &path,
            include_bytes!("../../../docs/rust-rewrite/t08a/tls/ca.pem"),
        )
        .unwrap();
        path
    }
}
fn execute(case: &Case) -> Result<serde_json::Value, String> {
    let mut fixture = Fixture::start(case.replies.clone(), &case.tls, &case.tls_protocol, None);
    let mut proxy_fixture = case.proxy.as_ref().map(|p| {
        Fixture::start(
            case.replies.clone(),
            if p.tls { "server" } else { "" },
            "",
            if p.tunnel {
                Some((fixture.address, p.status))
            } else {
                None
            },
        )
    });
    let image = std::env::var_os("SSX_T08A_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_ssx")));
    let image_before = fs::read(&image).map_err(|e| e.to_string())?;
    let root = std::env::var_os("SSX_T08A_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let cwd = std::env::current_dir().unwrap();
            cwd.ancestors()
                .find(|p| p.join("docs/rust-rewrite/t08a/cases.json").is_file())
                .expect("source root")
                .to_owned()
        });
    let base = format!("http://{}", fixture.address);
    let url = if case.tls.is_empty() {
        format!("{base}{}", case.base_suffix)
    } else {
        format!(
            "https://{}:{}",
            if case.host.is_empty() {
                "localhost"
            } else {
                &case.host
            },
            fixture.address.port()
        )
    };
    let env = case
        .env
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                v.replace(
                    "$TLS",
                    &root.join("docs/rust-rewrite/t08a/tls").to_string_lossy(),
                )
                .replace("$BASE", &base)
                .replace("$ADDRESS", &fixture.address.to_string())
                .replace("$PATHSEP", if cfg!(windows) { ";" } else { ":" })
                .replace("$PORT", &fixture.address.port().to_string())
                .replace("$TARGETPORT", &fixture.address.port().to_string())
                .replace(
                    "$PROXYPORT",
                    &proxy_fixture
                        .as_ref()
                        .map_or(0, |f| f.address.port())
                        .to_string(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let owned_ca = case.raw_ca_file.then(OwnedCa::new);
    let mut cmd = Command::new(&image);
    cmd.args(&case.argv)
        .env_clear()
        .env("PATH", "/nonexistent")
        .env("all_proxy", "t08a-disable-ambient-os-proxy")
        .env("SSX_URL", url)
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(ca) = &owned_ca {
        cmd.env("SSL_CERT_FILE", ca.file());
    }
    #[cfg(windows)]
    for name in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(name) {
            cmd.env(name, value);
        }
    }
    #[cfg(feature = "source-transport-test-fixtures")]
    if let Some(dir) = std::env::var_os("SSX_T08A_EVIDENCE") {
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        cmd.env(
            "SSX_T08A_DIAGNOSTIC_FILE",
            dir.join(format!("{}.stages.txt", case.id)),
        );
    }
    for (name, raw) in &case.env_os_invalid {
        let bytes = unhex(raw);
        #[cfg(unix)]
        let value = {
            use std::os::unix::ffi::OsStringExt;
            std::ffi::OsString::from_vec(bytes)
        };
        #[cfg(windows)]
        let value = {
            use std::os::windows::ffi::OsStringExt;
            std::ffi::OsString::from_wide(
                &bytes
                    .into_iter()
                    .map(|b| if b < 128 { u16::from(b) } else { 0xd800 })
                    .collect::<Vec<_>>(),
            )
        };
        cmd.env(name, value);
    }
    if let Some(tmp) = std::env::var_os("TMPDIR") {
        cmd.env("TMPDIR", tmp);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || {
        let mut v = Vec::new();
        stdout.read_to_end(&mut v).unwrap();
        v
    });
    let err = thread::spawn(move || {
        let mut v = Vec::new();
        stderr.read_to_end(&mut v).unwrap();
        v
    });
    let deadline = Instant::now() + Duration::from_secs(25);
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(s) => break s,
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            None => {
                child.kill().map_err(|e| e.to_string())?;
                child.wait().map_err(|e| e.to_string())?;
                fixture.finish();
                let _ = out.join();
                let _ = err.join();
                return Err(format!("{} child deadline", case.id));
            }
        }
    };
    let stdout = out.join().map_err(|_| "stdout join")?;
    let stderr = err.join().map_err(|_| "stderr join")?;
    if let Some(proxy) = &mut proxy_fixture {
        proxy.finish();
    }
    fixture.finish();
    if let Some(ca) = owned_ca {
        let path = ca.0.clone();
        drop(ca);
        assert!(!path.exists(), "owned CA fixture retired");
    }
    let requests = fixture.requests.lock().unwrap().clone();
    let proxy_requests = proxy_fixture
        .as_ref()
        .map(|f| f.requests.lock().unwrap().clone())
        .unwrap_or_default();
    if fs::read(&image).map_err(|e| e.to_string())? != image_before {
        return Err("native image changed during execution".into());
    }
    let (stdout_data, stdout_encoding) = if stdout.len() > 65536 {
        use flate2::{Compression, write::GzEncoder};
        let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
        gzip.write_all(&stdout).map_err(|e| e.to_string())?;
        (gzip.finish().map_err(|e| e.to_string())?, "gzip-hex")
    } else {
        (stdout.clone(), "hex")
    };
    let evidence = serde_json::json!({"case":case.id,"argv":case.argv,"declared_environment":case.env,"os_invalid_environment":case.env_os_invalid,"fixture_address":fixture.address.to_string(),"producer":fixture.producer.lock().unwrap().clone(),"image_sha256":hex(&Sha256::digest(&image_before)),"pid":pid,"status":status.code(),"stdout_hex":hex(&stdout_data),"stdout_encoding":stdout_encoding,"stdout_bytes":stdout.len(),"stdout_sha256":hex(&Sha256::digest(&stdout)),"stderr_hex":hex(&stderr),"requests":requests.iter().map(|r|serde_json::json!({"target":r.target,"accept":r.accept,"proxy_auth":r.proxy_auth,"host":r.host,"referer":r.referer,"method":r.method})).collect::<Vec<_>>(),"proxy_requests":proxy_requests.iter().map(|r|serde_json::json!({"target":r.target,"accept":r.accept,"proxy_auth":r.proxy_auth,"host":r.host,"referer":r.referer,"method":r.method})).collect::<Vec<_>>(),"proxy_fixture_joined_listener_absent":proxy_fixture.is_some(),"child_waited":true,"fixture_joined_listener_absent":true,"python_path":"/nonexistent","openssl_runtime":openssl::version::version()});
    if let Some(dir) = std::env::var_os("SSX_T08A_EVIDENCE") {
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        fs::write(
            dir.join(format!("{}.json", case.id)),
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .map_err(|e| e.to_string())?;
    }
    if status.code() != Some(case.status) {
        return Err(format!(
            "{} status {:?} expected {}; stderr {}",
            case.id,
            status.code(),
            case.status,
            String::from_utf8_lossy(&stderr)
        ));
    }
    if !case.stdout_contains.is_empty() {
        let text = String::from_utf8_lossy(&stdout);
        for required in &case.stdout_contains {
            if !text.contains(required) {
                return Err(format!("{} missing help field {required}", case.id));
            }
        }
    } else if let Some(expected) = &case.stdout_sha256 {
        if hex(&Sha256::digest(&stdout)) != *expected || Some(stdout.len()) != case.stdout_bytes {
            return Err(format!("{} stdout length/digest differs", case.id));
        }
    } else if stdout != case.stdout.as_bytes() {
        return Err(format!(
            "{} stdout differs (actual {} bytes, expected {} bytes; actual prefix {:?})",
            case.id,
            stdout.len(),
            case.stdout.len(),
            String::from_utf8_lossy(&stdout[..stdout.len().min(256)])
        ));
    }
    if case.stderr == "native-missing-command-help" {
        if !case.argv.is_empty()
            || case.status != 2
            || !case.stdout.is_empty()
            || !case.requests.is_empty()
            || stderr
                != include_bytes!("../../../docs/rust-rewrite/t08a/native-missing-command-help.txt")
        {
            return Err(format!("{} exact missing-command help differs", case.id));
        }
    } else if case.stderr == "usage" {
        let diagnostic = String::from_utf8_lossy(&stderr).to_lowercase();
        if !diagnostic.contains("error:")
            || !(diagnostic.contains("usage:") || diagnostic.contains("--help"))
        {
            return Err(format!("{} missing argument diagnostic", case.id));
        }
    } else if stderr != case.stderr.as_bytes() {
        return Err(format!(
            "{} stderr differs: {}",
            case.id,
            String::from_utf8_lossy(&stderr)
        ));
    }
    let expand = |s: &str| {
        s.replace("$BASE", &base)
            .replace("$ADDRESS", &fixture.address.to_string())
            .replace("$PORT", &fixture.address.port().to_string())
            .replace("$TARGETPORT", &fixture.address.port().to_string())
            .replace(
                "$PROXYPORT",
                &proxy_fixture
                    .as_ref()
                    .map_or(0, |f| f.address.port())
                    .to_string(),
            )
    };
    let same = |actual: &[Request], expected: &[Request]| {
        actual.len() == expected.len()
            && actual.iter().zip(expected).all(|(actual, expected)| {
                actual.method == expected.method
                    && actual.target == expand(&expected.target)
                    && actual.accept == expected.accept
                    && actual.proxy_auth == expected.proxy_auth
                    && actual.referer == expected.referer
                    && expected
                        .host
                        .as_ref()
                        .is_none_or(|h| actual.host.as_deref() == Some(expand(h).as_str()))
            })
    };
    if !same(&requests, &case.requests) || !same(&proxy_requests, &case.proxy_requests) {
        return Err(format!(
            "{} request order/bytes differ: origin {:?} expected {:?}; proxy {:?} expected {:?}",
            case.id, requests, case.requests, proxy_requests, case.proxy_requests
        ));
    }
    Ok(evidence)
}
#[test]
fn native_external_child_executes_complete_frozen_contract() {
    let bytes = std::env::var_os("SSX_T08A_CORPUS")
        .map(|p| fs::read(p).unwrap())
        .unwrap_or_else(|| CASES.to_vec());
    assert_eq!(
        Sha256::digest(&bytes),
        Sha256::digest(CASES),
        "source-bound case bytes changed"
    );
    let corpus: Corpus = serde_json::from_slice(&bytes).unwrap();
    validate(&corpus).unwrap();
    let mut errors = Vec::new();
    for c in &corpus.cases {
        if let Err(e) = execute(c) {
            errors.push(e);
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
#[test]
fn missing_case_and_mutated_native_reply_are_rejected() {
    let mut corpus: Corpus = serde_json::from_slice(CASES).unwrap();
    validate(&corpus).unwrap();
    corpus.cases.pop();
    assert_eq!(
        validate(&corpus).unwrap_err(),
        "finite case inventory changed"
    );
    let mut case: Case = serde_json::from_slice::<Corpus>(CASES)
        .unwrap()
        .cases
        .into_iter()
        .find(|c| c.id == "reference-health-json")
        .unwrap();
    case.id = "negative-mutated-native-response".into();
    case.replies[0].body_hex = hex(b"{\"mutated\":true}");
    let error = execute(&case).unwrap_err();
    assert!(error.contains("stdout differs"), "{error}");
}

#[cfg(feature = "source-transport-test-fixtures")]
#[test]
fn source_transport_diagnostic_controls() {
    let corpus: Corpus = serde_json::from_slice(CASES).unwrap();
    let mut failures = Vec::new();
    for id in [
        "tls-file-correct",
        "json-above-shared-four-mib",
        "float-token-fraction",
    ] {
        let case = corpus.cases.iter().find(|c| c.id == id).unwrap();
        if let Err(error) = execute(case) {
            failures.push(error);
        }
    }
    assert!(
        failures.is_empty(),
        "diagnostic positives remain blocked: {}",
        failures.join("\n")
    );
}

fn fixture_wire(id: &str) -> Vec<u8> {
    use flate2::read::GzDecoder;
    let compressed: &[u8] = match id {
        "headers-99" => include_bytes!("../../../docs/rust-rewrite/t08a/wire/headers-99.bin.gz"),
        "headers-100" => include_bytes!("../../../docs/rust-rewrite/t08a/wire/headers-100.bin.gz"),
        "header-line-65536" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/header-line-65536.bin.gz")
        }
        "header-line-65537" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/header-line-65537.bin.gz")
        }
        "headers-eight-60000" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/headers-eight-60000.bin.gz")
        }
        "trailers-1-20000" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/trailers-1-20000.bin.gz")
        }
        "trailers-101-100" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/trailers-101-100.bin.gz")
        }
        "trailers-1024-100" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/trailers-1024-100.bin.gz")
        }
        "trailers-1-65532" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/trailers-1-65532.bin.gz")
        }
        "informational-100" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/informational-100.bin.gz")
        }
        "informational-103" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/informational-103.bin.gz")
        }
        "chunk-delimiter-malformed" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/chunk-delimiter-malformed.bin.gz")
        }
        "chunk-trailer-eof-no-blank" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/chunk-trailer-eof-no-blank.bin.gz")
        }
        "chunk-extension-single-20000" => include_bytes!(
            "../../../docs/rust-rewrite/t08a/wire/chunk-extension-single-20000.bin.gz"
        ),
        "chunk-extensions-total-20000" => include_bytes!(
            "../../../docs/rust-rewrite/t08a/wire/chunk-extensions-total-20000.bin.gz"
        ),
        "chunk-size-line-65536" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/chunk-size-line-65536.bin.gz")
        }
        "chunk-size-line-65537" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/chunk-size-line-65537.bin.gz")
        }
        "chunk-size-plus" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/chunk-size-plus.bin.gz")
        }
        "chunk-size-prefix" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/chunk-size-prefix.bin.gz")
        }
        "chunk-size-underscore" => {
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/chunk-size-underscore.bin.gz")
        }
        _ => panic!("unknown finite wire fixture"),
    };
    let mut bytes = Vec::new();
    GzDecoder::new(compressed).read_to_end(&mut bytes).unwrap();
    bytes
}
