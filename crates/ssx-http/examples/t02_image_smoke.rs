//! Independent Rust wire client for the narrowly scoped Python-free T02 image.
use ssx_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
fn no_python() -> Result<usize, Box<dyn std::error::Error>> {
    let mut directories = vec![
        std::path::PathBuf::from("/usr"),
        std::path::PathBuf::from("/opt"),
    ];
    let mut count = 0;
    while let Some(path) = directories.pop() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            count += 1;
            if count > 200000 {
                return Err("runtime file inventory exceeded explicit limit".into());
            }
            if name.starts_with("libpython")
                || ((name.starts_with("python") || name.starts_with("pypy"))
                    && (kind.is_file() || kind.is_symlink()))
            {
                return Err(
                    format!("Python runtime artifact exists: {}", entry.path().display()).into(),
                );
            }
            if kind.is_dir() {
                directories.push(entry.path());
            }
        }
    }
    for name in ["python", "python2", "python3", "pypy", "pypy3"] {
        match Command::new(name)
            .arg("--version")
            .env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .output()
        {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(format!("Python executable is available: {name}").into()),
        }
    }
    Ok(count)
}
fn query(base: &str, path: &str, params: &[(&str, &str)]) -> String {
    let mut encoded = url::form_urlencoded::Serializer::new(format!("{base}{path}?"));
    for (k, v) in params {
        encoded.append_pair(k, v);
    }
    encoded.finish()
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let inventory = no_python()?;
    let executable = std::env::current_exe()?
        .parent()
        .ok_or("client executable has no parent")?
        .join("ssx-t02-http");
    let calls = Path::new("/tmp/t02-image-engine-calls.jsonl");
    let stderr = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open("/tmp/t02-image-server-stderr.log")?;
    let child = Command::new(executable)
        .args([calls.to_str().unwrap(), "dark", "stdin"])
        .env_clear()
        .env("TZ", "UTC")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()?;
    let mut server = Server(child);
    let output = server.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(output).read_line(&mut line);
        let _ = tx.send((result, line));
    });
    let (result, line) = match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(value) => value,
        Err(error) => {
            let _ = server.0.kill();
            let _ = server.0.wait();
            let _ = reader.join();
            return Err(format!(
                "server startup exceeded owned 5-second deadline: {error}; child and reader joined"
            )
            .into());
        }
    };
    reader.join().map_err(|_| "startup reader failed")?;
    result?;
    let startup: Value = ssx_json::from_str(line.trim())?;
    let base = format!(
        "http://{}",
        startup["address"]
            .as_str()
            .ok_or("missing startup address")?
    );
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()?;
    let mut completed = Vec::new();
    for path in ["/", "/search"] {
        for method in ["GET", "POST"] {
            let request = if method == "GET" {
                client.get(query(&base, path, &[("q", "contract"), ("format", "json")]))
            } else {
                client
                    .post(format!("{base}{path}"))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body("q=contract&format=json")
            };
            let response = request
                .header("x-request-id", "t02-image-fixed")
                .send()
                .await?;
            assert_eq!(response.status(), 200);
            assert_eq!(response.headers()["x-request-id"], "t02-image-fixed");
            let body: Value = ssx_json::from_str(&response.text().await?)?;
            assert_eq!(
                body["results"][0]["parsed_url"],
                json!([
                    "https",
                    "example.test",
                    "/path",
                    "param",
                    "q=one",
                    "section"
                ])
            );
            completed.push(format!("{method} {path} JSON"));
        }
    }
    let controls: Value =
        ssx_json::from_str(include_str!("../tests/formatter_source_controls.json"))?;
    for id in ["epoch:004", "epoch:008", "epoch:013", "epoch:025"] {
        let control = controls["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        let response = client
            .get(query(
                &base,
                "/search",
                &[("q", &format!("formatter:{id}")), ("format", "json")],
            ))
            .send()
            .await?;
        assert_eq!(response.status(), 200);
        let body: Value = ssx_json::from_str(&response.text().await?)?;
        assert_eq!(body["results"][0]["pubdate"], control["output"]);
        completed.push(format!("HTTP formatter {id}"));
    }
    for (format, media, marker) in [
        ("yaml", "text/vnd.yaml+markdown", "## Results Summary"),
        ("csv", "text/csv", "title,url,content,engine,publishedDate"),
        ("rss", "application/rss+xml", "<rss"),
        ("html", "text/html", "Search results for contract"),
    ] {
        let response = client
            .get(query(
                &base,
                "/search",
                &[("q", "contract"), ("format", format)],
            ))
            .send()
            .await?;
        assert_eq!(response.status(), 200);
        assert!(
            response.headers()["content-type"]
                .to_str()?
                .starts_with(media)
        );
        assert!(response.text().await?.contains(marker));
        completed.push(format!("format {format}"));
    }
    for (params, status, error) in [
        (vec![("format", "json")], 400, "query_required"),
        (
            vec![("q", "contract"), ("format", "json"), ("pageno", "0")],
            400,
            "invalid_filter",
        ),
        (
            vec![("q", "contract"), ("format", "json"), ("safesearch", "2")],
            400,
            "invalid_filter",
        ),
        (
            vec![("q", "contract"), ("format", "invalid")],
            400,
            "unsupported_format",
        ),
        (
            vec![
                ("q", "contract"),
                ("format", "json"),
                ("engines", "contract,hibp"),
            ],
            403,
            "engine_restricted",
        ),
        (
            vec![
                ("q", "contract"),
                ("format", "json"),
                ("date_from", "2026-01-01"),
                ("date_from", "2026-01-02"),
            ],
            400,
            "invalid_filter",
        ),
    ] {
        let before = fs::read_to_string(calls)?.lines().count();
        let response = client.get(query(&base, "/search", &params)).send().await?;
        assert_eq!(response.status(), status);
        let body: Value = ssx_json::from_str(&response.text().await?)?;
        assert_eq!(body["error"], error);
        assert_eq!(fs::read_to_string(calls)?.lines().count(), before);
        completed.push(format!("predispatch {error} {status}"));
    }
    let response = client.get(format!("{base}/healthz")).send().await?;
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await?, "OK");
    completed.push("readiness".into());
    let response = client.get(format!("{base}/health")).send().await?;
    assert_eq!(response.status(), 200);
    let body: Value = ssx_json::from_str(&response.text().await?)?;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["engines"]["contract"]["status"], "ok");
    completed.push("passive health".into());
    let response = client.get(format!("{base}/config")).send().await?;
    assert_eq!(response.status(), 200);
    let body: Value = ssx_json::from_str(&response.text().await?)?;
    assert_eq!(body["engines"][0]["name"], "contract");
    assert!(body["categories"].is_array());
    completed.push("redacted discovery".into());
    let response = client.get(format!("{base}/metrics")).send().await?;
    assert_eq!(response.status(), 200);
    let body = response.text().await?;
    assert!(body.contains("# TYPE slopsearx_server_requests_total counter"));
    assert!(body.contains("slopsearx_http_search_completed_total{outcome=\"rejected\"}"));
    completed.push("O01 metrics".into());
    server
        .0
        .stdin
        .take()
        .ok_or("missing owned shutdown input")?
        .write_all(b"STOP\n")?;
    let until = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = server.0.try_wait()? {
            break status;
        }
        if Instant::now() >= until {
            return Err("server shutdown exceeded owned 5-second deadline".into());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert!(status.success());
    assert_eq!(completed.len(), 22);
    println!(
        "{}",
        json!({"scope":"T02 offline real service fixture; not production assembly or deployment","python_executables":false,"libpython":false,"runtime_inventory_entries":inventory,"completed":completed,"server_child_shutdown":"success","network":"container network none; owned loopback only"})
    );
    Ok(())
}
