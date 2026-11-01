#[cfg(unix)]
mod unix {
    //! Actual external ssx -> production HTTP/service/npm/Valkey composition.
    //! The provider is an owned deterministic loopback; no paid/live calls occur.

    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::{
        fs::{self, File, OpenOptions},
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
            }
            let _ = self.0.wait();
        }
    }
    fn wait(child: &mut OwnedChild, seconds: u64) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "owned child {} exceeded deadline",
                child.0.id()
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn file(path: &Path) -> File {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
    }
    fn binary(variable: &str) -> PathBuf {
        let path = PathBuf::from(
            std::env::var_os(variable).expect("explicit qualified binary path required"),
        );
        assert!(path.is_absolute() && path.is_file());
        path
    }
    fn sha(path: &Path) -> String {
        Sha256::digest(fs::read(path).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
    fn lines(path: &Path) -> Vec<Value> {
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    }
    struct Upstream {
        address: std::net::SocketAddr,
        stop: Arc<AtomicBool>,
        requests: Arc<Mutex<Vec<String>>>,
        thread: Option<thread::JoinHandle<()>>,
    }
    impl Upstream {
        fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            listener.set_nonblocking(true).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let requests = Arc::new(Mutex::new(Vec::new()));
            let (halt, capture) = (stop.clone(), requests.clone());
            let thread = thread::spawn(move || {
                while !halt.load(Ordering::SeqCst) {
                    let (mut stream, _) = match listener.accept() {
                        Ok(v) => v,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                        Err(e) => panic!("owned upstream accept: {e}"),
                    };
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut raw = Vec::new();
                    while raw.len() < 65536 && !raw.windows(4).any(|s| s == b"\r\n\r\n") {
                        let mut buf = [0; 1024];
                        match stream.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => raw.extend_from_slice(&buf[..n]),
                        }
                    }
                    if raw.is_empty() {
                        continue;
                    }
                    assert!(
                        raw.windows(4).any(|s| s == b"\r\n\r\n"),
                        "bounded upstream headers incomplete"
                    );
                    let request = String::from_utf8(raw).unwrap();
                    let target = request
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap();
                    let url = url::Url::parse(&format!("http://127.0.0.1{target}")).unwrap();
                    let query = url
                        .query_pairs()
                        .find(|(k, _)| k == "text")
                        .unwrap()
                        .1
                        .into_owned();
                    capture.lock().unwrap().push(request);
                    let (status, body) = if query == "upstream-error" {
                        (503, "{}".to_owned())
                    } else if query == "upstream-malformed" {
                        (200, "{".to_owned())
                    } else {
                        (200, json!({"objects":[{"package":{"name":"t08-package","version":"1.0.0","description":query,"links":{"npm":"https://www.npmjs.com/package/t08-package"}},"score":{"detail":{"popularity":0.5}}}]}).to_string())
                    };
                    let reply = format!(
                        "HTTP/1.1 {status} Response\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream.write_all(reply.as_bytes()).unwrap();
                }
            });
            Self {
                address,
                stop,
                requests,
                thread: Some(thread),
            }
        }
    }
    impl Drop for Upstream {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(100));
            if let Some(handle) = self.thread.take() {
                let _ = handle.join();
            }
        }
    }
    fn cli(out: &Path, image: &Path, base: &str, id: &str, args: &[&str], expected: i32) -> Value {
        let stdout = out.join(format!("{id}.stdout"));
        let stderr = out.join(format!("{id}.stderr"));
        let mut child = OwnedChild(
            Command::new(image)
                .args(args)
                .env_clear()
                .env("PATH", "/nonexistent")
                .env("SSX_URL", base)
                .env("TZ", "UTC")
                .stdin(Stdio::null())
                .stdout(file(&stdout))
                .stderr(file(&stderr))
                .spawn()
                .unwrap(),
        );
        let status = wait(&mut child, 20);
        let output = fs::read_to_string(&stdout).unwrap();
        let errors = fs::read_to_string(&stderr).unwrap();
        assert_eq!(status.code(), Some(expected), "{id}: {errors}");
        if expected == 0 {
            assert!(errors.is_empty(), "{id}: {errors}");
        } else {
            assert!(output.is_empty(), "{id}: failure leaked stdout");
            assert!(!errors.is_empty());
        }
        json!({"id":id,"args":args,"status":status.code(),"stdout":output,"stderr":errors,"binary_sha256":sha(image),"child_waited":true})
    }
    pub fn run() {
        let out =
            PathBuf::from(std::env::var_os("T08_RECEIPTS").expect("owned T08_RECEIPTS required"));
        assert!(out.is_absolute());
        fs::create_dir(&out).expect("T08 receipt directory must be fresh");
        let temp = PathBuf::from(
            std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT").expect("owned short temp root required"),
        );
        assert!(temp.is_absolute() && temp.is_dir());
        let mut random = [0; 8];
        aws_lc_rs::rand::fill(&mut random).unwrap();
        let suffix = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let socket = temp.join(format!("t08-{suffix}.sock"));
        use std::os::unix::ffi::OsStrExt;
        assert!(socket.as_os_str().as_bytes().len() < 104 && !socket.exists());
        struct Socket(PathBuf);
        impl Drop for Socket {
            fn drop(&mut self) {
                let _ = fs::remove_file(&self.0);
            }
        }
        let _socket_cleanup = Socket(socket.clone());
        let server = binary("T08_NATIVE_SERVER_BINARY");
        let image = binary("T08_NATIVE_CLI_BINARY");
        let valkey = binary("SLOPSEARX_TEST_VALKEY_SERVER");
        let mut store = OwnedChild(
            Command::new(&valkey)
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
                .arg(&out)
                .arg("--unixsocket")
                .arg(&socket)
                .env_clear()
                .stdin(Stdio::null())
                .stdout(file(&out.join("valkey.stdout")))
                .stderr(file(&out.join("valkey.stderr")))
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while std::os::unix::net::UnixStream::connect(&socket).is_err() {
            assert!(
                store.0.try_wait().unwrap().is_none(),
                "Valkey exited before readiness"
            );
            assert!(Instant::now() < deadline, "Valkey startup timeout");
            thread::sleep(Duration::from_millis(10));
        }
        let upstream = Upstream::start();
        let startup_path = out.join("server.stdout");
        let mut native = OwnedChild(
            Command::new(&server)
                .arg(out.join("adapter-calls.jsonl"))
                .arg(format!("http://{}/search", upstream.address))
                .arg(format!("unix://{}", socket.display()))
                .arg(out.join("absent.yaml"))
                .env_clear()
                .env("PATH", "/nonexistent")
                .stdin(Stdio::null())
                .stdout(file(&startup_path))
                .stderr(file(&out.join("server.stderr")))
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        let startup: Value = loop {
            let raw = fs::read_to_string(&startup_path).unwrap();
            if let Some(line) = raw.lines().next().filter(|_| raw.contains('\n')) {
                break serde_json::from_str(line).unwrap();
            }
            assert!(
                native.0.try_wait().unwrap().is_none(),
                "native HTTP startup exited"
            );
            assert!(Instant::now() < deadline, "native HTTP startup timeout");
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(startup["mode"], "t08-production-components");
        let base = format!("http://{}", startup["address"].as_str().unwrap());
        let mut records = Vec::new();
        for command in ["health", "config", "engines"] {
            for json in [false, true] {
                let args = if json {
                    vec![command, "--json"]
                } else {
                    vec![command]
                };
                let record = cli(
                    &out,
                    &image,
                    &base,
                    &format!("{command}-{}", if json { "json" } else { "text" }),
                    &args,
                    0,
                );
                assert!(
                    record["stdout"].as_str().unwrap().contains("npm"),
                    "discovery must expose actual npm"
                );
                if json {
                    let _: Value =
                        serde_json::from_str(record["stdout"].as_str().unwrap()).unwrap();
                }
                records.push(record);
            }
        }
        for (id, args) in [
            (
                "search-yaml",
                vec!["search", "Unicode café 🦀", "--engines", "npm"],
            ),
            (
                "search-json",
                vec!["search", "Unicode café 🦀", "--engines", "npm", "--json"],
            ),
        ] {
            let record = cli(&out, &image, &base, id, &args, 0);
            assert!(record["stdout"].as_str().unwrap().contains("t08-package"));
            if id.ends_with("json") {
                let v: Value = serde_json::from_str(record["stdout"].as_str().unwrap()).unwrap();
                assert_eq!(v["results"][0]["engine"], "npm");
            }
            records.push(record);
        }
        let pages = ["18446744073709551616", "18446744073709551617"];
        let before = lines(&out.join("adapter-calls.jsonl")).len();
        let upstream_before = upstream.requests.lock().unwrap().len();
        for (i, page) in pages.iter().enumerate() {
            for repeat in 0..2 {
                let record = cli(
                    &out,
                    &image,
                    &base,
                    &format!("page-{i}-{repeat}"),
                    &[
                        "search",
                        "cache-page",
                        "--engines",
                        "npm",
                        "--pageno",
                        page,
                        "--json",
                    ],
                    0,
                );
                let reply: Value =
                    serde_json::from_str(record["stdout"].as_str().unwrap()).unwrap();
                assert_eq!(reply["meta"]["cached"], repeat == 1);
                records.push(record);
                assert_eq!(
                    lines(&out.join("adapter-calls.jsonl")).len(),
                    before + i + 1,
                    "repeat must hit private Valkey, distinct high page must miss"
                );
                assert_eq!(
                    upstream.requests.lock().unwrap().len(),
                    upstream_before + i + 1
                );
            }
        }
        let calls = lines(&out.join("adapter-calls.jsonl"));
        assert_eq!(calls[before]["page_text"].as_str(), Some(pages[0]));
        assert_eq!(calls[before + 1]["page_text"].as_str(), Some(pages[1]));
        // The generic receipt reader uses serde_json; use the already native
        // lossless parser for raw numeric lexemes instead of narrowing via f64.
        let exact_calls = fs::read_to_string(out.join("adapter-calls.jsonl"))
            .unwrap()
            .lines()
            .map(|line| ssx_json::from_str::<ssx_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        for (offset, page) in pages.iter().enumerate() {
            let expected = ssx_json::from_str::<ssx_json::Value>(page).unwrap();
            assert_eq!(exact_calls[before + offset]["page"], expected);
        }
        for (id, args, status) in [
            (
                "unknown-engine",
                vec![
                    "search",
                    "scope",
                    "--engines",
                    "not-a-real-engine",
                    "--json",
                ],
                503,
            ),
            (
                "negative-page",
                vec![
                    "search",
                    "scope",
                    "--engines",
                    "npm",
                    "--pageno",
                    "-1",
                    "--json",
                ],
                400,
            ),
        ] {
            let before = lines(&out.join("adapter-calls.jsonl")).len();
            let upstream_before = upstream.requests.lock().unwrap().len();
            let record = cli(&out, &image, &base, id, &args, 1);
            assert_eq!(
                record["stderr"],
                format!("error: ssx: HTTP failure: {status}\n")
            );
            records.push(record);
            assert_eq!(upstream.requests.lock().unwrap().len(), upstream_before);
            assert_eq!(
                lines(&out.join("adapter-calls.jsonl")).len(),
                before,
                "denied request dispatched adapter"
            );
        }
        // All failed production engines yield HTTP 503, retained as CLI failure.
        for (query, message) in [
            ("upstream-error", "npm returned an unsuccessful status"),
            ("upstream-malformed", "npm returned invalid JSON"),
        ] {
            let record = cli(
                &out,
                &image,
                &base,
                query,
                &["search", query, "--engines", "npm", "--json"],
                1,
            );
            assert_eq!(record["stderr"], "error: ssx: HTTP failure: 503\n");
            let calls = lines(&out.join("adapter-calls.jsonl"));
            let outcome = calls.last().unwrap();
            assert_eq!(outcome["query"], query);
            assert_eq!(outcome["status"], "error");
            assert_eq!(outcome["error_message"], message);
            records.push(record);
        }
        let adapter_calls_raw = fs::read_to_string(out.join("adapter-calls.jsonl")).unwrap();
        let calls = lines(&out.join("adapter-calls.jsonl"));
        native.0.kill().unwrap();
        let native_exit = wait(&mut native, 5);
        assert!(native.0.try_wait().unwrap().is_some());
        records.push(cli(
            &out,
            &image,
            &base,
            "unavailable-server",
            &["health", "--json"],
            1,
        ));
        store.0.kill().unwrap();
        let valkey_exit = wait(&mut store, 5);
        assert!(store.0.try_wait().unwrap().is_some());
        fs::remove_file(&socket)
            .or_else(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(e)
                }
            })
            .unwrap();
        assert!(!socket.exists());
        let requests = upstream.requests.lock().unwrap().clone();
        let decoded_queries = requests
            .iter()
            .map(|request| {
                let target = request
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap();
                let url = url::Url::parse(&format!("http://127.0.0.1{target}")).unwrap();
                url.query_pairs()
                    .find(|(key, _)| key == "text")
                    .unwrap()
                    .1
                    .into_owned()
            })
            .collect::<Vec<_>>();
        assert!(decoded_queries.iter().any(|q| q == "Unicode café 🦀"));
        assert_eq!(
            decoded_queries
                .iter()
                .filter(|q| q.as_str() == "cache-page")
                .count(),
            2
        );
        upstream.stop.store(true, Ordering::SeqCst);
        let mut upstream = upstream;
        upstream.thread.take().unwrap().join().unwrap();
        drop(upstream);
        fs::write(out.join("completed.json"),serde_json::to_vec_pretty(&json!({"scope":"native production-component composition, not production startup/distribution","cli_binary":image,"cli_sha256":sha(&image),"server_binary":server,"server_sha256":sha(&server),"valkey_sha256":sha(&valkey),"records":records,"adapter_calls":calls,"adapter_calls_raw":adapter_calls_raw,"upstream_requests":requests,"server_exit":native_exit.to_string(),"valkey_exit":valkey_exit.to_string(),"private_socket_retired":!socket.exists(),"server_and_valkey_waited":true,"upstream_joined":true,"upstream_page_support":false})).unwrap()).unwrap();
    }
}
#[cfg(unix)]
fn main() {
    unix::run();
}
#[cfg(not(unix))]
fn main() {
    eprintln!(
        "T08 backend integration requires a supported Unix backend host; Windows CLI qualification is T08A"
    );
    std::process::exit(2);
}
