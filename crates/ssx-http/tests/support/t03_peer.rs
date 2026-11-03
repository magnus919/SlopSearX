//! Owned real HTTP peer used by T03 and the actual OIDC cookie test.
use sha2::{Digest, Sha256};
use ssx_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn directory(name: &str) -> PathBuf {
    let base = std::env::var_os("T03_EVIDENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    assert!(base.is_absolute());
    fs::create_dir_all(&base).unwrap();
    assert!(
        !fs::symlink_metadata(&base)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let path = base.join(format!(
        "t03-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn binary() -> PathBuf {
    std::env::var_os("T03_NATIVE_SERVER_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join(if cfg!(windows) {
                    "examples/t03_fixture.exe"
                } else {
                    "examples/t03_fixture"
                })
        })
}
pub struct Process {
    child: Child,
    base: String,
    pub out: PathBuf,
}
impl Drop for Process {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
impl Process {
    pub fn start(name: &str, scenario: &str) -> Self {
        let out = directory(name);
        let binary = binary();
        assert!(
            binary.is_file(),
            "build the locked t03_fixture example before T03 acceptance"
        );
        let stderr = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join("server-stderr.log"))
            .unwrap();
        let child = Command::new(&binary)
            .arg(out.join("calls.jsonl"))
            .arg("dark")
            .arg(scenario)
            .env_clear()
            .env("TZ", "UTC")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn()
            .unwrap();
        let mut p = Self {
            child,
            base: String::new(),
            out,
        };
        let stdout = p.child.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let reader = std::thread::spawn(move || {
            let mut line = String::new();
            let result = BufReader::new(stdout).read_line(&mut line);
            let _ = tx.send((result, line));
        });
        let line = match rx.recv_timeout(Duration::from_secs(5)) {
            Ok((Ok(n), line)) if n > 0 => line,
            other => {
                let _ = p.child.kill();
                let _ = p.child.wait();
                let _ = reader.join();
                panic!("T03 fixture startup failed: {other:?}")
            }
        };
        reader.join().unwrap();
        let startup: Value = ssx_json::from_str(line.trim()).unwrap();
        assert_eq!(startup["mode"], "offline-t03-fixture");
        assert_eq!(startup["renderer"], "PublicPortal");
        p.base = format!("http://{}", startup["address"].as_str().unwrap());
        fs::write(p.out.join("process-start.json"),json!({"binary":binary.to_string_lossy().as_ref(),"binary_sha256":digest(&fs::read(binary).unwrap()),"pid":p.child.id(),"startup":startup}).to_string()).unwrap();
        p
    }
    pub fn calls(&self) -> usize {
        fs::read_to_string(self.out.join("calls.jsonl"))
            .unwrap()
            .lines()
            .count()
    }
    pub async fn stop(&mut self) {
        writeln!(self.child.stdin.take().unwrap(), "STOP").unwrap();
        for _ in 0..100 {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                let terminal = self.out.join("terminal.json");
                fs::write(&terminal,json!({"child_joined":true,"child_exit_success":true,"engine_calls":self.calls()}).to_string()).unwrap();
                println!(
                    "T03_TERMINAL {}",
                    json!({"path":terminal.to_string_lossy().as_ref(),"sha256":digest(&fs::read(&terminal).unwrap())})
                );
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("T03 fixture did not join after owned shutdown");
    }
    pub async fn get(&self, target: &str, id: &str) -> (u16, String) {
        self.request("GET", target, None, None, id).await
    }
    pub async fn request(
        &self,
        method: &str,
        target: &str,
        form: Option<&str>,
        cookie: Option<&str>,
        id: &str,
    ) -> (u16, String) {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let before = self.calls();
        let mut request = if method == "POST" {
            client
                .post(format!("{}{target}", self.base))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(form.unwrap_or("").to_owned())
        } else {
            client.get(format!("{}{target}", self.base))
        };
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v.to_str().unwrap())))
            .collect::<ssx_json::Map<_, _>>();
        let body = response.text().await.unwrap();
        let after = self.calls();
        let path = self.out.join(format!("{id}.json"));
        fs::write(&path,json!({"id":id,"method":method,"target":target,"status":status,"headers":headers,"body":body,"engine_call_delta":after-before}).to_string()).unwrap();
        println!(
            "T03_BINDING {}",
            json!({"id":id,"receipt":path.to_string_lossy().as_ref(),"sha256":digest(&fs::read(&path).unwrap()),"status":status,"engine_call_delta":after-before})
        );
        (status, body)
    }
}
pub fn yaml(body: &str) -> Value {
    let header = body.split_once("---\n## Results Summary\n").unwrap().0;
    let value: serde_json::Value = serde_saphyr::from_str(header).unwrap();
    ssx_json::from_str(&value.to_string()).unwrap()
}
pub fn params(pairs: &[(&str, &str)]) -> String {
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        s.append_pair(k, v);
    }
    s.finish()
}

// Keep this helper usable by the real OIDC test, which supplies a freshly issued
// valid workflow cookie rather than inventing a string labelled as valid.
pub async fn public_cookie_equivalence(valid_cookie: &str) {
    let mut p = Process::start("cookies", "contract");
    let (status, _) = p.get("/", "cookie-landing").await;
    assert_eq!(status, 200);
    for path in ["/", "/search"] {
        for method in ["GET", "POST"] {
            for format in ["html", "json", "yaml", "csv", "rss"] {
                let fields = params(&[
                    ("q", "cookie-journey"),
                    ("categories", "general"),
                    ("format", format),
                ]);
                let target = if method == "GET" {
                    format!("{path}?{fields}")
                } else {
                    path.into()
                };
                let mut expected = None;
                for (index, cookie) in [
                    None,
                    Some(valid_cookie),
                    Some("__Host-slopsearx_session=invalid"),
                ]
                .into_iter()
                .enumerate()
                {
                    let (status, body) = p
                        .request(
                            method,
                            &target,
                            (method == "POST").then_some(fields.as_str()),
                            cookie,
                            &format!(
                                "cookie-{}-{method}-{format}-{index}",
                                if path == "/" { "root" } else { "search" }
                            ),
                        )
                        .await;
                    assert_eq!(status, 200);
                    let normalized = normalize_public(format, &body);
                    if let Some(expected) = &expected {
                        assert_eq!(&normalized, expected);
                    } else {
                        expected = Some(normalized);
                    }
                }
            }
        }
    }
    assert_eq!(p.calls(), 120);
    p.stop().await;
}
fn normalize_public(format: &str, body: &str) -> String {
    match format {
        "json" | "yaml" => {
            let mut value = if format == "json" {
                ssx_json::from_str(body).unwrap()
            } else {
                yaml(body)
            };
            let meta = value["meta"].as_object_mut().unwrap();
            meta.shift_remove("query_id");
            meta.shift_remove("response_time_ms");
            value.to_string()
        }
        "html" => {
            let start = body.find(" results · ").unwrap() + " results · ".len();
            let end = start + body[start..].find(" ms").unwrap();
            format!("{}<elapsed>{}", &body[..start], &body[end..])
        }
        _ => body.into(),
    }
}
