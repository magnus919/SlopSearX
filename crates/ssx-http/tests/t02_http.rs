//! An independent reqwest client drives a distinct real Rust HTTP process.
//! Source capture bytes remain immutable; only explicit schema nondeterminism is normalized.
use sha2::{Digest, Sha256};
use ssx_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};
// Keep independent process/capture suites in the same controlled order as T02.
static PROCESS_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Process {
    child: Child,
}
impl Drop for Process {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
fn binary() -> PathBuf {
    std::env::var_os("T02_NATIVE_SERVER_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("examples/t02_fixture")
        })
}
fn start(out: &std::path::Path, theme: &str) -> (Process, String, f64) {
    start_scenario(out, theme, "contract", None)
}
fn start_scenario(
    out: &std::path::Path,
    theme: &str,
    scenario: &str,
    store: Option<&str>,
) -> (Process, String, f64) {
    let binary = binary();
    assert!(
        binary.is_file(),
        "build the locked t02_fixture example before actual-process qualification"
    );
    let stderr = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out.join("server-stderr.log"))
        .unwrap();
    let runtime_start = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let server_digest = digest(&fs::read(&binary).unwrap());
    let child = Command::new(&binary)
        .arg(out.join("engine-calls.jsonl"))
        .arg(theme)
        .arg("signal")
        .arg(scenario)
        .args(store)
        .env_clear()
        .env("TZ", "UTC")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .unwrap();
    let mut process = Process { child };
    let stdout = process.child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send((result, line));
    });
    let line = match rx.recv_timeout(Duration::from_secs(5)) {
        Ok((Ok(n), line)) if n > 0 => line,
        other => {
            let _ = process.child.kill();
            let _ = process.child.wait();
            let _ = reader.join();
            fs::write(
                out.join("terminal.json"),
                json!({"startup_failure":format!("{other:?}"),"child_joined":true,"completed":0})
                    .to_string(),
            )
            .unwrap();
            panic!("native HTTP startup failed or exceeded its owned 5-second deadline");
        }
    };
    reader.join().unwrap();
    let startup: Value = ssx_json::from_str(line.trim()).unwrap();
    fs::write(out.join("process-start.json"),json!({"binary":binary.to_string_lossy().as_ref(),"binary_sha256":server_digest,"pid":process.child.id(),"startup":startup,"runtime_start":runtime_start}).to_string()).unwrap();

    (
        process,
        format!("http://{}", startup["address"].as_str().unwrap()),
        runtime_start,
    )
}
async fn stop(process: &mut Process) -> bool {
    let signaled = Command::new("/bin/kill")
        .args(["-INT", &process.child.id().to_string()])
        .status()
        .unwrap()
        .success();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if let Some(status) = process.child.try_wait().unwrap() {
            return signaled && status.success();
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}
fn directory(label: &str) -> PathBuf {
    let mut random = [0u8; 16];
    aws_lc_rs::rand::fill(&mut random).unwrap();
    let id = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let out = std::env::var_os("T02_HTTP_RECEIPTS")
        .map(|base| PathBuf::from(base).join(label))
        .unwrap_or_else(|| std::env::temp_dir().join(format!("t02-http-{label}-{id}")));
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::create_dir(&out).unwrap();
    out
}
fn corpus_texts() -> [&'static str; 9] {
    [
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_records_00.jsonl"),
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_records_01.jsonl"),
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_records_02.jsonl"),
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_records_03.jsonl"),
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_grammar_records.jsonl"),
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_date_records.jsonl"),
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_datebranch_records.jsonl"),
        include_str!(
            "../../../docs/rust-rewrite/t02/oracle/source_http_sensitive_space_records.jsonl"
        ),
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_latin1_records.jsonl"),
    ]
}
fn fixtures() -> Vec<Value> {
    corpus_texts()
        .into_iter()
        .flat_map(str::lines)
        .map(|line| ssx_json::from_str(line).unwrap())
        .collect()
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn source_bindings(texts: &[&str]) -> std::collections::BTreeMap<String, String> {
    texts
        .iter()
        .flat_map(|text| text.lines())
        .map(|line| {
            let record: Value = ssx_json::from_str(line).unwrap();
            (
                record["id"].as_str().unwrap().to_owned(),
                digest(line.as_bytes()),
            )
        })
        .collect()
}
fn bind_receipt(kind: &str, id: &str, path: &std::path::Path, source: Option<&str>) {
    let bytes = fs::read(path).unwrap();
    println!(
        "T02_BINDING {}",
        json!({"kind":kind,"id":id,"source_record_sha256":source,"receipt":path.to_string_lossy().as_ref(),"receipt_sha256":digest(&bytes)})
    );
}
fn normalize_json(value: &mut Value) {
    if let Some(meta) = value.get_mut("meta").filter(|m| m.is_object()) {
        if meta.get("query_id").is_some() {
            meta["query_id"] = json!("<query-id>");
        }
        if meta.get("response_time_ms").is_some() {
            meta["response_time_ms"] = json!(0);
        }
    }
}
fn normalize_yaml(body: &str) -> String {
    let mut in_meta = false;
    let mut output = String::new();
    for part in body.split_inclusive('\n') {
        let line = part.strip_suffix('\n').unwrap_or(part);
        if let Some(rest) = line.strip_prefix("meta: {'cached': True, 'query_id': '") {
            let (id, tail) = rest
                .split_once("', 'advisories': ")
                .expect("typed negative-cache YAML metadata");
            assert!(id.starts_with("ssx-") && id.len() > 4);
            output.push_str("meta: {'cached': True, 'query_id': '<query-id>', 'advisories': ");
            output.push_str(tail);
            if part.ends_with('\n') {
                output.push('\n');
            }
            continue;
        }

        if line == "meta:" {
            in_meta = true;
        } else if !line.starts_with(' ') && !line.is_empty() {
            in_meta = false;
        }
        let line = if in_meta && line.starts_with("  query_id: ") {
            "  query_id: <query-id>"
        } else if in_meta && line.starts_with("  response_time_ms: ") {
            "  response_time_ms: 0"
        } else {
            line
        };
        output.push_str(line);
        if part.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}
fn utc_seconds(value: &str) -> Option<f64> {
    let value = value.strip_suffix("+00:00")?;
    let (whole, fraction) = value.split_once('.').map_or((value, ""), |(a, b)| (a, b));
    if fraction.len() > 6 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let epoch = ssx_http::formatter_metadata::iso_to_epoch(Some(&format!("{whole}Z")))?;
    Some(
        epoch as f64
            + if fraction.is_empty() {
                0.
            } else {
                format!("0.{fraction}").parse::<f64>().ok()?
            },
    )
}
fn metrics_match(source: &str, actual: &str, calls: usize, runtime_seconds: f64) -> bool {
    let native = "slopsearx_search_engine_events_total";
    let headers = |body: &str| {
        body.lines()
            .filter(|line| line.starts_with('#') && !line.contains(native))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    if headers(source) != headers(actual) {
        return false;
    }
    fn samples(body: &str) -> Option<std::collections::BTreeMap<String, f64>> {
        body.lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                let (key, value) = line.rsplit_once(' ')?;
                Some((key.to_owned(), value.parse().ok()?))
            })
            .collect()
    }
    let (Some(source), Some(actual)) = (samples(source), samples(actual)) else {
        return false;
    };
    for (key, value) in &source {
        let Some(observed) = actual.get(key) else {
            return false;
        };
        if key.starts_with("slopsearx_http_search_duration_seconds_")
            && (key.contains("_bucket{") || key.contains("_sum{"))
        {
            if !observed.is_finite() || *observed < 0. {
                return false;
            }
        } else if value != observed {
            return false;
        }
    }
    for (key, value) in &actual {
        if !source.contains_key(key) {
            let expected = match key.as_str() {
                "slopsearx_search_engine_events_total{event=\"observed\"}" => calls as f64,
                "slopsearx_search_engine_events_total{event=\"internal_failure\"}"
                | "slopsearx_search_engine_events_total{event=\"synthetic\"}" => 0.,
                _ => return false,
            };
            if *value != expected {
                return false;
            }
        }
    }
    for outcome in ["success", "rejected", "failure"] {
        let Some(count) = actual.get(&format!(
            "slopsearx_http_search_duration_seconds_count{{outcome=\"{outcome}\"}}"
        )) else {
            // Neither runtime emits histogram samples for an unobserved outcome.
            // A partially emitted histogram is still a contract failure.
            if actual.keys().chain(source.keys()).any(|key| {
                key.starts_with("slopsearx_http_search_duration_seconds_")
                    && key.contains(&format!("outcome=\"{outcome}\""))
            }) {
                return false;
            }
            continue;
        };
        let mut prior = 0.;
        for boundary in [
            "0.005", "0.01", "0.025", "0.05", "0.1", "0.25", "0.5", "1", "2.5", "5", "10", "30",
            "60", "+Inf",
        ] {
            let Some(value)=actual.get(&format!("slopsearx_http_search_duration_seconds_bucket{{outcome=\"{outcome}\",le=\"{boundary}\"}}"))else{return false;};
            if *value < prior || value > count {
                return false;
            }
            prior = *value;
        }
        if prior != *count {
            return false;
        }
        let Some(sum) = actual.get(&format!(
            "slopsearx_http_search_duration_seconds_sum{{outcome=\"{outcome}\"}}"
        )) else {
            return false;
        };
        if *sum > runtime_seconds * count || (*count == 0. && *sum != 0.) {
            return false;
        }
    }
    true
}
fn request_pairs(value: &Value) -> Vec<(String, String)> {
    if let Some(pairs) = value.as_array() {
        pairs
            .iter()
            .map(|p| {
                (
                    p[0].as_str().unwrap().to_owned(),
                    p[1].as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| p[1].to_string()),
                )
            })
            .collect()
    } else {
        value
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string()),
                )
            })
            .collect()
    }
}
#[tokio::test]
async fn original_literal_corpus_through_a_real_native_http_process() {
    let _process_test_guard = PROCESS_TEST_LOCK.lock().await;
    let cases = fixtures();
    let bindings = source_bindings(&corpus_texts());
    assert_eq!(cases.len(), 270);
    let ids = cases
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), cases.len());
    let out = directory("corpus");
    let calls = out.join("engine-calls.jsonl");
    let mut records = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out.join("native-records.jsonl"))
        .unwrap();
    let (mut process, base, runtime_start) = start(&out, "dark");
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let mut completed = 0;
    let mut failures = Vec::new();
    for case in &cases {
        let source_id = case["id"].as_str().unwrap();
        if std::env::var("T02_SKIP_SOURCE_CASE").ok().as_deref() == Some(source_id) {
            println!("T02_INTENTIONAL_SKIP {source_id}");
            continue;
        }
        let input = &case["request"];
        let pairs = request_pairs(&input["params"]);
        let mut encoded = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in &pairs {
            encoded.append_pair(k, v);
        }
        let encoded = encoded.finish();
        let before = fs::read_to_string(&calls).unwrap().lines().count();
        let method = input["method"].as_str().unwrap();
        let mut url = format!("{base}{}", input["path"].as_str().unwrap());
        let explicit_body = input.get("body").and_then(Value::as_str);
        let form = method == "POST" && explicit_body.is_none();
        if !form && !encoded.is_empty() {
            url.push('?');
            url.push_str(&encoded);
        }
        let mut request = client
            .request(reqwest::Method::from_bytes(method.as_bytes()).unwrap(), url)
            .header("X-Request-ID", "t02-source-fixed");
        if let Some(headers) = input.get("headers").and_then(Value::as_object) {
            for (k, v) in headers {
                let bytes = v
                    .as_str()
                    .unwrap()
                    .chars()
                    .map(|ch| {
                        u8::try_from(u32::from(ch))
                            .expect("source http.client header input must encode as Latin-1")
                    })
                    .collect::<Vec<_>>();
                request =
                    request.header(k, reqwest::header::HeaderValue::from_bytes(&bytes).unwrap());
            }
        }
        if form {
            request = request
                .header("content-type", "application/x-www-form-urlencoded")
                .body(encoded);
        } else if let Some(body) = explicit_body {
            request = request.body(body.to_owned());
        }
        let result = request.send().await;
        let (status, headers, body) = match result {
            Ok(response) => {
                let status = response.status().as_u16();
                let headers = response
                    .headers()
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.as_str().to_owned(),
                            Value::String(v.to_str().unwrap_or("<non-utf8>").to_owned()),
                        )
                    })
                    .collect::<ssx_json::Map<_, _>>();
                let body = response.text().await.unwrap();
                (status, headers, body)
            }
            Err(error) => {
                failures.push(format!("{source_id}:transport:{error}"));
                continue;
            }
        };
        let after = fs::read_to_string(&calls).unwrap().lines().count();
        let expected_status = case["status"].as_u64().unwrap();
        let mut differences = Vec::new();
        if u64::from(status) != expected_status {
            differences.push("status");
        }
        if headers.get("content-type") != case["headers"].get("content-type") {
            differences.push("content-type");
        }
        if headers.get("x-request-id").and_then(Value::as_str) != Some("t02-source-fixed") {
            differences.push("request-id");
        }
        for header in [
            "content-security-policy",
            "referrer-policy",
            "x-content-type-options",
            "permissions-policy",
        ] {
            if headers.get(header) != case["headers"].get(header) {
                differences.push(header);
            }
        }
        let expected_calls = case["engine_calls"].as_array().unwrap();
        if after - before != expected_calls.len() {
            differences.push("engine-call-count");
        }
        // Compare the frozen integer that actually crossed the adapter port,
        // independently of the response pagination/enforcement serialization.
        let observed = fs::read_to_string(&calls).unwrap();
        for (line, expected) in observed.lines().skip(before).zip(expected_calls) {
            let actual: Value = ssx_json::from_str(line).unwrap();
            if actual.get("pageno") != expected["params"].get("pageno") {
                differences.push("engine-pageno");
            }
        }
        let media = case["headers"]["content-type"].as_str().unwrap();
        if media.starts_with("application/json") && source_id != "health" {
            match ssx_json::from_str::<Value>(&body) {
                Ok(mut actual) => {
                    let mut expected: Value =
                        ssx_json::from_str(case["body"].as_str().unwrap()).unwrap();
                    if let Some(id) = actual.get("meta").and_then(|m| m.get("query_id")) {
                        assert!(
                            id.as_str()
                                .is_some_and(|id| id.starts_with("ssx-") && id.len() > 4),
                            "HTTP query ID prefix missing before normalization"
                        );
                    }
                    normalize_json(&mut actual);
                    normalize_json(&mut expected);
                    if actual != expected {
                        differences.push("json-body");
                    }
                }
                Err(_) => differences.push("json-parse"),
            }
        } else if source_id == "health" {
            let mut actual: Value = ssx_json::from_str(&body).unwrap();
            let mut expected: Value = ssx_json::from_str(case["body"].as_str().unwrap()).unwrap();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64();
            let observed = utc_seconds(
                actual["engines"]["contract"]["status_at"]
                    .as_str()
                    .unwrap_or(""),
            );
            if !observed.is_some_and(|t| t >= runtime_start && t <= now)
                || utc_seconds(
                    expected["engines"]["contract"]["status_at"]
                        .as_str()
                        .unwrap_or(""),
                )
                .is_none()
            {
                differences.push("health-clock");
            }
            actual["engines"]["contract"]["status_at"] = json!("<observed-at>");
            expected["engines"]["contract"]["status_at"] = json!("<observed-at>");
            if actual != expected {
                differences.push("health-body");
            }
        } else if source_id == "metrics" {
            if !metrics_match(
                case["body"].as_str().unwrap(),
                &body,
                after,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64()
                    - runtime_start,
            ) {
                differences.push("metrics-contract");
            }
        } else if media.starts_with("text/csv")
            || media.starts_with("application/rss+xml")
            || (media.starts_with("text/vnd.yaml+markdown")
                && case["body"].as_str().unwrap().starts_with("error:"))
        {
            if body != case["body"].as_str().unwrap() {
                differences.push("body-bytes");
            }
        } else if media.starts_with("text/vnd.yaml+markdown") {
            if normalize_yaml(&body) != normalize_yaml(case["body"].as_str().unwrap()) {
                differences.push("yaml-body");
            }
        } else if media.starts_with("text/html") {
            if status == 200 && source_id != "root_landing" && !body.contains("Search results for ")
            {
                differences.push("html-search-title");
            }
            if source_id == "root_landing"
                && (!body.contains("Search <em>SlopSearX.</em>")
                    || !body.contains("action=\"/search\"")
                    || !body.contains("data-theme-toggle"))
            {
                differences.push("html-landing-contract");
            }
            if status >= 400
                && case["body"]
                    .as_str()
                    .unwrap()
                    .contains("Search results for ")
                && !body.contains("Search results for ")
            {
                differences.push("html-search-failure-title");
            }
            if status >= 400
                && !case["body"]
                    .as_str()
                    .unwrap()
                    .contains("Search results for ")
                && !body.contains("Search error")
            {
                differences.push("html-error-contract");
            }
        }
        writeln!(records,"{}",json!({"id":source_id,"status":status,"headers":Value::Object(headers.clone()),"body":body,"engine_call_delta":after-before,"intentional_difference":Value::Null,"differences":differences})).unwrap();
        records.flush().unwrap();
        let receipt = out.join(format!("{source_id}.json"));
        fs::write(&receipt,json!({"id":source_id,"status":status,"headers":headers,"body":body,"engine_call_delta":after-before,"differences":differences}).to_string()).unwrap();
        bind_receipt("http", source_id, &receipt, Some(&bindings[source_id]));
        if !differences.is_empty() {
            failures.push(format!("{source_id}:{}", differences.join(",")));
        }
        completed += 1;
    }
    let clean = stop(&mut process).await;
    let cleanup = json!({"completed":completed,"expected":cases.len(),"failures":failures,"graceful_signal_and_child_exit_success":clean,"receipts":out.to_string_lossy().as_ref()});
    fs::write(out.join("terminal.json"), cleanup.to_string()).unwrap();
    println!("{cleanup}");
    assert_eq!(completed, 270);
    assert!(clean);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
#[tokio::test]
async fn original_owned_html_assertions_use_real_native_renderer() {
    let _process_test_guard = PROCESS_TEST_LOCK.lock().await;
    for theme in ["dark", "darker"] {
        let out = directory(&format!("html-{theme}"));
        let (mut process, base, _) = start(&out, theme);
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let mut records = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join("native-records.jsonl"))
            .unwrap();
        for (path, status, markers) in [
            (
                "/",
                200,
                vec![
                    "Search <em>SlopSearX.</em>",
                    "action=\"/search\"",
                    "data-theme-toggle",
                ],
            ),
            ("/search?q=test", 200, vec!["Search results for test"]),
            (
                "/search?q=test&format=html",
                200,
                vec!["Search results for test"],
            ),
            ("/search", 400, vec!["query_required"]),
            ("/search?q=headers", 200, vec!["Search results for headers"]),
        ] {
            let response = client.get(format!("{base}{path}")).send().await.unwrap();
            let actual_status = response.status().as_u16();
            let headers = response.headers().clone();
            let body = response.text().await.unwrap();
            writeln!(records,"{}",json!({"path":path,"status":actual_status,"headers":headers.iter().map(|(k,v)|(k.as_str().to_owned(),json!(v.to_str().unwrap()))).collect::<ssx_json::Map<_,_>>(),"body":body})).unwrap();
            records.flush().unwrap();
            assert_eq!(actual_status, status);
            assert!(
                headers["content-type"]
                    .to_str()
                    .unwrap()
                    .contains("text/html")
            );
            for marker in markers {
                assert!(
                    body.contains(marker),
                    "missing original HTML assertion: {marker}"
                );
            }
            assert!(body.contains(&format!("data-default-theme=\"{theme}\"")));
            assert!(
                headers["content-security-policy"]
                    .to_str()
                    .unwrap()
                    .contains("default-src 'self'")
            );
            assert_eq!(headers["referrer-policy"], "no-referrer");
            assert_eq!(headers["x-content-type-options"], "nosniff");
            assert!(
                headers["permissions-policy"]
                    .to_str()
                    .unwrap()
                    .contains("camera=()")
            );
        }
        let clean = stop(&mut process).await;
        fs::write(out.join("terminal.json"),json!({"requests_completed":5,"original_literal_assertions":true,"theme":theme,"child_exit_success":clean,"raw_http_records":out.join("native-records.jsonl").to_string_lossy().as_ref(),"raw_http_records_sha256":digest(&fs::read(out.join("native-records.jsonl")).unwrap())}).to_string()).unwrap();
        assert!(clean);
        bind_receipt("html", theme, &out.join("terminal.json"), None);
    }
}

struct PrivateValkey {
    child: Child,
    url: String,
}
impl Drop for PrivateValkey {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(path) = self.url.strip_prefix("unix://") {
            let _ = fs::remove_file(path);
        }
    }
}
impl PrivateValkey {
    fn start(out: &std::path::Path) -> Self {
        let mut random = [0u8; 8];
        aws_lc_rs::rand::fill(&mut random).unwrap();
        // The runner owns a short temporary root so Unix sockets fit
        // sockaddr_un without falling back to the host's default temp path.
        let temp_root = std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT")
            .map(PathBuf::from)
            .expect("SLOPSEARX_TEST_TEMP_ROOT must name an owned short temp directory");
        assert!(temp_root.is_dir(), "T02 temp root must already exist");
        let socket = temp_root.join(format!(
            "t02-http-cache-{}.sock",
            random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ));
        use std::os::unix::ffi::OsStrExt;
        assert!(
            socket.as_os_str().as_bytes().len() < 104,
            "private Valkey socket path exceeds sockaddr_un limit"
        );
        assert!(!socket.exists(), "private socket already exists");
        let executable = std::env::var("SLOPSEARX_TEST_VALKEY_SERVER")
            .unwrap_or_else(|_| "valkey-server".into());
        let stderr = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join("valkey-stderr.log"))
            .unwrap();
        let child = Command::new(executable)
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
            .arg(out)
            .arg("--unixsocket")
            .arg(&socket)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(stderr)
            .spawn()
            .expect("pinned private Valkey required");
        let mut server = Self {
            child,
            url: format!("unix://{}", socket.display()),
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            assert!(
                server.child.try_wait().unwrap().is_none(),
                "private Valkey exited before readiness"
            );
            if std::os::unix::net::UnixStream::connect(&socket).is_ok() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "private Valkey startup timeout"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        server
    }
}
#[tokio::test]
async fn original_fault_bodies_and_private_valkey_cache_match_real_http() {
    let _process_test_guard = PROCESS_TEST_LOCK.lock().await;
    let out = directory("faults");
    let cache_out = out.join("valkey");
    fs::create_dir(&cache_out).unwrap();
    let mut store = PrivateValkey::start(&cache_out);
    let fault_texts = [
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_fault_records.jsonl"),
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_negative_records.jsonl"),
    ];
    let bindings = source_bindings(&fault_texts);
    let cases = fault_texts
        .iter()
        .flat_map(|text| text.lines())
        .map(|line| ssx_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 27);
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let mut completed = std::collections::BTreeSet::new();
    let mut failures = Vec::new();
    for scenario in ["none", "denied", "cache", "negative", "contract"] {
        let process_out = out.join(scenario);
        fs::create_dir(&process_out).unwrap();
        let (mut process, base, _) = start_scenario(
            &process_out,
            "dark",
            scenario,
            if matches!(scenario, "cache" | "negative") {
                Some(&store.url)
            } else {
                None
            },
        );
        if scenario == "none" {
            let (status, _, body) =
                family_wire(&client, &base, "/health", &[], &process_out, "empty-health").await;
            assert_eq!(status, 200);
            let health: Value = ssx_json::from_str(&body).unwrap();
            assert_eq!(health["status"], "ok");
            assert_eq!(health["engines"], json!({}));
        }
        for case in cases
            .iter()
            .filter(|case| case["request"]["runtime"] == scenario)
        {
            let id = case["id"].as_str().unwrap();
            let params = request_pairs(&case["request"]["params"]);
            let mut encoded = url::form_urlencoded::Serializer::new(String::new());
            for (k, v) in &params {
                encoded.append_pair(k, v);
            }
            let url = format!("{base}/search?{}", encoded.finish());
            let calls = process_out.join("engine-calls.jsonl");
            let before = fs::read_to_string(&calls).unwrap().lines().count();
            let response = client
                .get(url)
                .header("x-request-id", "t02-source-fixed")
                .send()
                .await
                .unwrap();
            let status = response.status().as_u16();
            let headers = response
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), json!(v.to_str().unwrap())))
                .collect::<ssx_json::Map<_, _>>();
            let body = response.text().await.unwrap();
            let after = fs::read_to_string(&calls).unwrap().lines().count();
            let media = case["headers"]["content-type"].as_str().unwrap();
            let mut differences = Vec::new();
            if u64::from(status) != case["status"].as_u64().unwrap() {
                differences.push("status");
            }
            if headers.get("content-type") != case["headers"].get("content-type") {
                differences.push("media");
            }
            if after - before != case["engine_calls"].as_array().unwrap().len() {
                differences.push("dispatch-count");
            }
            if media.starts_with("application/json") {
                let mut actual: Value = ssx_json::from_str(&body).unwrap();
                let mut expected: Value =
                    ssx_json::from_str(case["body"].as_str().unwrap()).unwrap();
                if let Some(id) = actual.get("meta").and_then(|m| m.get("query_id"))
                    && !id
                        .as_str()
                        .is_some_and(|id| id.starts_with("ssx-") && id.len() > 4)
                {
                    differences.push("query-id-prefix");
                }
                normalize_json(&mut actual);
                normalize_json(&mut expected);
                if actual != expected {
                    differences.push("json-body");
                }
            } else if media.starts_with("text/vnd.yaml+markdown") {
                if normalize_yaml(&body) != normalize_yaml(case["body"].as_str().unwrap()) {
                    differences.push("yaml-body");
                }
            } else if media.starts_with("text/html") {
                let marker = if id.starts_with("negative_hit") {
                    "service_unavailable"
                } else if status == 429 {
                    "rate_limited"
                } else {
                    "Search results for contract"
                };
                if !body.contains(marker) {
                    differences.push("literal-html");
                }
            } else if body != case["body"].as_str().unwrap() {
                differences.push("body-bytes");
            }
            fs::write(process_out.join(format!("{id}.json")),json!({"id":id,"status":status,"headers":headers,"body":body,"engine_calls":after-before,"differences":differences}).to_string()).unwrap();
            if !differences.is_empty() {
                failures.push(format!("{id}:{differences:?}"));
            }
            bind_receipt(
                "fault",
                id,
                &process_out.join(format!("{id}.json")),
                Some(&bindings[id]),
            );
            assert!(completed.insert(id.to_owned()));
        }
        assert!(stop(&mut process).await, "fault server did not drain");
    }
    let stopped = store.child.kill().is_ok() && store.child.wait().is_ok();
    fs::write(
        out.join("terminal.json"),
        json!({"completed":completed,"failures":failures,"private_valkey_child_joined":stopped})
            .to_string(),
    )
    .unwrap();
    assert!(stopped);
    assert_eq!(completed.len(), 27);
    assert!(
        failures.is_empty(),
        "fault source differences {failures:?}; raw receipt {}",
        out.display()
    );
}

async fn family_wire(
    client: &reqwest::Client,
    base: &str,
    path: &str,
    params: &[(&str, &str)],
    out: &std::path::Path,
    id: &str,
) -> (u16, Value, String) {
    let mut encoded = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in params {
        encoded.append_pair(k, v);
    }
    let url = format!("{base}{path}?{}", encoded.finish());
    let before = fs::read_to_string(out.join("engine-calls.jsonl"))
        .unwrap()
        .lines()
        .count();
    let response = client
        .get(&url)
        .header("x-request-id", "t02-family-fixed")
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), json!(v.to_str().unwrap())))
        .collect::<ssx_json::Map<_, _>>();
    let body = response.text().await.unwrap();
    let after = fs::read_to_string(out.join("engine-calls.jsonl"))
        .unwrap()
        .lines()
        .count();
    fs::write(
        out.join(format!("{id}.json")),
        json!({"id":id,"request":{"url":url},"status":status,"headers":headers,"body":body,"engine_call_delta":after-before})
            .to_string(),
    )
    .unwrap();
    bind_receipt(
        "family",
        &format!("{}:{id}", out.file_name().unwrap().to_string_lossy()),
        &out.join(format!("{id}.json")),
        None,
    );
    (status, Value::Object(headers), body)
}
#[tokio::test]
async fn original_assertion_families_use_runtime_policy_config_and_public_discovery() {
    let _process_test_guard = PROCESS_TEST_LOCK.lock().await;
    let out = directory("families");
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let mut completed = Vec::new();
    let mut ordinary_baseline = None;
    for scenario in [
        "mock3",
        "grants",
        "sensitive",
        "restricted",
        "formats-json",
        "scrape-disabled",
        "scrape-enabled",
    ] {
        let process_out = out.join(scenario);
        fs::create_dir(&process_out).unwrap();
        let (mut process, base, _) = start_scenario(&process_out, "dark", scenario, None);
        match scenario {
            "mock3" => {
                let (status, _, body) = family_wire(
                    &client,
                    &base,
                    "/health",
                    &[],
                    &process_out,
                    "unobserved-health",
                )
                .await;
                assert_eq!(status, 200);
                let health: Value = ssx_json::from_str(&body).unwrap();
                let h = &health["engines"]["contract"];
                assert_eq!(h["status"], "unknown");
                assert_eq!(h["status_at"], Value::Null);
                assert_eq!(h["stale"], false);
                assert_eq!(h["configured"], true);
                assert_eq!(h["circuit_open"], false);
                assert_eq!(h["circuit_consecutive_errors"], 0);
                assert!(h.get("auth_class").is_some());
                assert!(h.get("auth_configured").is_some());
                completed.push("unobserved-health");
                let (status, _, body) = family_wire(
                    &client,
                    &base,
                    "/openapi.json",
                    &[],
                    &process_out,
                    "absolute-date-openapi",
                )
                .await;
                assert_eq!(status, 200);
                let schema: Value = ssx_json::from_str(&body).unwrap();
                for method in ["get", "post"] {
                    for date in ["date_from", "date_to"] {
                        assert!(
                            schema["paths"]["/search"][method]["parameters"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|p| p["name"] == date && p["schema"]["format"] == "date")
                        );
                    }
                }
                completed.push("absolute-date-openapi");
                let (status, _, body) = family_wire(
                    &client,
                    &base,
                    "/search",
                    &[("q", "test query"), ("format", "json")],
                    &process_out,
                    "basic-three-results",
                )
                .await;
                assert_eq!(status, 200);
                let data: Value = ssx_json::from_str(&body).unwrap();
                assert_eq!(data["query"], "test query");
                assert_eq!(data["number_of_results"], 3);
                assert_eq!(data["results"].as_array().unwrap().len(), 3);
                assert!(
                    data["meta"]["query_id"]
                        .as_str()
                        .unwrap()
                        .starts_with("ssx-")
                );
                for key in ["cached", "response_time_ms", "engine_status"] {
                    assert!(data["meta"].get(key).is_some());
                }
                assert!(data["suggestions"].is_array());
                ordinary_baseline = Some(data.clone());
                completed.push("basic-three-results");
                let (status, _, body) = family_wire(
                    &client,
                    &base,
                    "/search",
                    &[
                        ("q", "test"),
                        ("language", "fr"),
                        ("pageno", "2"),
                        ("safesearch", "1"),
                        ("time_range", "month"),
                        ("categories", "news,tech"),
                        ("format", "json"),
                    ],
                    &process_out,
                    "query-params-preserved",
                )
                .await;
                assert_eq!(status, 200);
                let data: Value = ssx_json::from_str(&body).unwrap();
                assert_eq!(data["number_of_results"], 3);
                let calls = fs::read_to_string(process_out.join("engine-calls.jsonl")).unwrap();
                let forwarded: Value = ssx_json::from_str(calls.lines().last().unwrap()).unwrap();
                assert_eq!(forwarded["language"], "fr");
                assert_eq!(forwarded["pageno"], 2);
                assert_eq!(forwarded["safesearch"], 1);
                assert_eq!(forwarded["time_range"], "month");
                assert_eq!(forwarded["categories"], json!(["news", "tech"]));
                completed.push("query-params-preserved");
                let (status, _, body) = family_wire(
                    &client,
                    &base,
                    "/search",
                    &[("q", "surrogate"), ("format", "json")],
                    &process_out,
                    "canonical-replacement-payload",
                )
                .await;
                assert_eq!(status, 200);
                let data: Value = ssx_json::from_str(&body).unwrap();
                assert_eq!(
                    data["results"][0]["payload"],
                    json!({"domain":"security","type":"vulnerability","data":{"note":"\u{fffd}"}})
                );
                completed.push("canonical-replacement-payload");
                let (status, _, body) = family_wire(
                    &client,
                    &base,
                    "/search",
                    &[("q", "panic-secret"), ("format", "json")],
                    &process_out,
                    "adapter-panic-redacted",
                )
                .await;
                assert_eq!(status, 503);
                let data: Value = ssx_json::from_str(&body).unwrap();
                assert_eq!(data["unresponsive_engines"].as_array().unwrap().len(), 1);
                assert!(!body.contains("secret-key-12345"));
                assert!(body.contains("internal adapter task failure"));
                completed.push("adapter-panic-redacted");
                for format in ["json", "yaml"] {
                    let id = format!("deadline-{format}");
                    let (status, _, body) = family_wire(
                        &client,
                        &base,
                        "/search",
                        &[
                            ("q", "deadline"),
                            ("format", format),
                            ("interactive_timeout_ms", "10"),
                        ],
                        &process_out,
                        &id,
                    )
                    .await;
                    assert_eq!(status, 503);
                    if format == "json" {
                        let data: Value = ssx_json::from_str(&body).unwrap();
                        assert_eq!(data["meta"]["deadline_exceeded"], true);
                    } else {
                        assert!(body.contains("deadline_exceeded: true"));
                    }
                }
                completed.push("interactive-deadline-json-yaml");
            }
            "grants" => {
                let (status, _, body) = family_wire(
                    &client,
                    &base,
                    "/search",
                    &[("q", "test query"), ("format", "json")],
                    &process_out,
                    "mcp-grants-ordinary-http",
                )
                .await;
                assert_eq!(status, 200);
                let data: Value = ssx_json::from_str(&body).unwrap();
                assert_eq!(data["results"].as_array().unwrap().len(), 3);
                let mut baseline = ordinary_baseline.clone().unwrap();
                let mut enabled = data.clone();
                normalize_json(&mut baseline);
                normalize_json(&mut enabled);
                assert_eq!(
                    enabled, baseline,
                    "additive MCP grants must not change ordinary HTTP projection"
                );
                let before = fs::read_to_string(process_out.join("engine-calls.jsonl"))
                    .unwrap()
                    .lines()
                    .count();
                let (status, _, body) = family_wire(
                    &client,
                    &base,
                    "/search",
                    &[
                        ("q", "private"),
                        ("format", "json"),
                        ("engines", "contract,hibp"),
                    ],
                    &process_out,
                    "mcp-grants-no-sensitive-grant",
                )
                .await;
                assert_eq!(status, 403);
                assert!(body.contains("engine_restricted"));
                assert_eq!(
                    fs::read_to_string(process_out.join("engine-calls.jsonl"))
                        .unwrap()
                        .lines()
                        .count(),
                    before
                );
                completed.push("mcp-grants-ordinary-http-and-sensitive-denial");
            }
            "sensitive" | "restricted" => {
                let (status, _, body) = family_wire(
                    &client,
                    &base,
                    "/search",
                    &[
                        ("q", "private"),
                        ("format", "json"),
                        ("engines", "contract"),
                    ],
                    &process_out,
                    "configured-sensitive-set",
                )
                .await;
                let data: Value = ssx_json::from_str(&body).unwrap();
                if scenario == "sensitive" {
                    assert_eq!(status, 200);
                    assert_eq!(data["number_of_results"], 3);
                } else {
                    assert_eq!(status, 403);
                    assert_eq!(
                        data,
                        json!({"error":"engine_restricted","message":"Explicit selection of one or more sensitive sources requires operator authorization.","field":"engines","engines":["contract"]})
                    );
                    assert_eq!(
                        fs::read_to_string(process_out.join("engine-calls.jsonl")).unwrap(),
                        ""
                    );
                }
                completed.push(if scenario == "sensitive" {
                    "configured-sensitive-allowed"
                } else {
                    "configured-sensitive-restricted"
                });
            }
            "formats-json" => {
                for (format, status) in [
                    ("csv", 403),
                    ("rss", 403),
                    ("html", 403),
                    ("yaml", 200),
                    ("json", 200),
                ] {
                    let id = format!("allowlist-{format}");
                    let (actual, headers, _) = family_wire(
                        &client,
                        &base,
                        "/search",
                        &[("q", "test"), ("format", format)],
                        &process_out,
                        &id,
                    )
                    .await;
                    assert_eq!(actual, status);
                    assert!(
                        headers["content-type"]
                            .as_str()
                            .unwrap()
                            .starts_with(match format {
                                "csv" => "text/csv",
                                "rss" => "application/rss+xml",
                                "html" => "text/html",
                                "yaml" => "text/vnd.yaml+markdown",
                                _ => "application/json",
                            })
                    );
                }
                completed.push("configured-format-allowlist");
            }
            "scrape-disabled" | "scrape-enabled" => {
                let (status, _, body) = family_wire(
                    &client,
                    &base,
                    "/search",
                    &[("q", "empty"), ("format", "json")],
                    &process_out,
                    "empty-scrape-diagnostics",
                )
                .await;
                assert_eq!(status, 200);
                let data: Value = ssx_json::from_str(&body).unwrap();
                assert_eq!(data["unresponsive_engines"], json!([]));
                if scenario == "scrape-enabled" {
                    assert_eq!(
                        data["meta"]["empty_engines"],
                        json!([["contract", "successful scrape returned no results"]])
                    );
                } else {
                    assert!(data["meta"].get("empty_engines").is_none());
                }
                completed.push(if scenario == "scrape-enabled" {
                    "empty-scrape-diagnostic-opt-in"
                } else {
                    "empty-scrape-diagnostic-default-off"
                });
            }
            _ => unreachable!(),
        }
        assert!(
            stop(&mut process).await,
            "assertion-family server failed to drain"
        );
    }
    fs::write(
        out.join("terminal.json"),
        json!({"completed_assertion_families":completed,"all_server_children_joined":true})
            .to_string(),
    )
    .unwrap();
    assert_eq!(completed.len(), 13);
}

#[tokio::test]
async fn absolute_date_proof_and_forwarding_reach_actual_http_without_claiming_a06() {
    let _process_test_guard = PROCESS_TEST_LOCK.lock().await;
    let out = directory("date-proof");
    let (mut process, base, _) = start_scenario(&out, "dark", "date-proof", None);
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let mut completed = Vec::new();
    for path in ["/", "/search"] {
        for method in ["GET", "POST"] {
            for bounds in [
                vec![("date_from", "2024-02-29")],
                vec![("date_to", "2024-03-01")],
                vec![("date_from", "2024-02-29"), ("date_to", "2024-03-01")],
            ] {
                for format in ["json", "yaml"] {
                    let id = format!("date-proof-{:02}", completed.len());
                    let mut params = vec![
                        ("q", "bounded"),
                        ("engines", "openalex"),
                        ("format", format),
                    ];
                    params.extend(bounds.iter().copied());
                    let mut encoder = url::form_urlencoded::Serializer::new(String::new());
                    for (k, v) in &params {
                        encoder.append_pair(k, v);
                    }
                    let encoded = encoder.finish();
                    let request = if method == "GET" {
                        client.get(format!("{base}{path}?{encoded}"))
                    } else {
                        client
                            .post(format!("{base}{path}"))
                            .header("content-type", "application/x-www-form-urlencoded")
                            .body(encoded)
                    };
                    let before = fs::read_to_string(out.join("engine-calls.jsonl"))
                        .unwrap()
                        .lines()
                        .count();
                    let response = request.send().await.unwrap();
                    let status = response.status().as_u16();
                    let headers = response
                        .headers()
                        .iter()
                        .map(|(k, v)| (k.to_string(), json!(v.to_str().unwrap())))
                        .collect::<ssx_json::Map<_, _>>();
                    let body = response.text().await.unwrap();
                    let after = fs::read_to_string(out.join("engine-calls.jsonl"))
                        .unwrap()
                        .lines()
                        .count();
                    assert_eq!(after - before, 1);
                    fs::write(out.join(format!("{id}.json")),json!({"id":id,"method":method,"path":path,"params":params,"status":status,"headers":headers,"body":body,"engine_call_delta":after-before}).to_string()).unwrap();
                    assert_eq!(status, 200);
                    let data: serde_json::Value = if format == "json" {
                        serde_json::from_str(&body).unwrap()
                    } else {
                        serde_saphyr::from_str(body.split_once("---\n").unwrap().0).unwrap()
                    };
                    let from = bounds
                        .iter()
                        .find(|(k, _)| *k == "date_from")
                        .map(|(_, v)| *v);
                    let to = bounds
                        .iter()
                        .find(|(k, _)| *k == "date_to")
                        .map(|(_, v)| *v);
                    assert_eq!(data["meta"]["engine_status"]["openalex"]["status"], "ok");
                    assert_eq!(
                        data["meta"]["date_filter"],
                        serde_json::json!({"requested":{"date_from":from,"date_to":to,"time_range":null},"applied":{"date_from":from,"date_to":to},"status":"enforced","reason":"Publication dates enforced by the audited OpenAlex adapter.","engines":["openalex"],"enforced_by":["upstream:openalex"],"warnings":[]})
                    );
                    let calls = fs::read_to_string(out.join("engine-calls.jsonl")).unwrap();
                    let forwarded: Value =
                        ssx_json::from_str(calls.lines().last().unwrap()).unwrap();
                    assert_eq!(forwarded["date_from"], json!(from));
                    assert_eq!(forwarded["date_to"], json!(to));
                    bind_receipt("date-proof", &id, &out.join(format!("{id}.json")), None);
                    completed.push(id);
                }
            }
        }
    }
    let response = client
        .get(format!(
            "{base}/search?q=timeout&engines=openalex&date_from=2024-02-29&format=json"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    let body = response.text().await.unwrap();
    fs::write(out.join("upstream-timeout.json"), &body).unwrap();
    let data: Value = ssx_json::from_str(&body).unwrap();
    assert_eq!(
        data["meta"]["engine_status"]["openalex"]["status"],
        "timeout"
    );
    assert_eq!(completed.len(), 24);
    let clean = stop(&mut process).await;
    fs::write(out.join("terminal.json"),json!({"completed":completed,"upstream_timeout":true,"child_exit_success":clean,"scope":"synthetic audited capability proves HTTP forwarding only; real A06/#984 integration remains open"}).to_string()).unwrap();
    assert!(clean);
}

#[tokio::test]
async fn reviewed_offsets_category_labels_and_duplicate_queries_match_original_http() {
    let _process_test_guard = PROCESS_TEST_LOCK.lock().await;
    let text =
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_review_records.jsonl");
    let cases = text
        .lines()
        .map(|line| ssx_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let bindings = source_bindings(&[text]);
    assert_eq!(cases.len(), 9);
    let out = directory("review");
    let (mut process, base, runtime_start) = start(&out, "dark");
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let mut completed = std::collections::BTreeSet::new();
    let mut snapshots = std::collections::BTreeMap::new();
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        let input = &case["request"];
        let pairs = request_pairs(&input["params"]);
        let mut encoded = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in &pairs {
            encoded.append_pair(key, value);
        }
        let target = format!(
            "{base}{}?{}",
            input["path"].as_str().unwrap(),
            encoded.finish()
        );
        let mut request = client
            .get(target)
            .header("x-request-id", "t02-source-fixed");
        for (key, value) in input["headers"].as_object().into_iter().flatten() {
            request = request.header(key, value.as_str().unwrap());
        }
        let before = fs::read_to_string(out.join("engine-calls.jsonl"))
            .unwrap()
            .lines()
            .count();
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v.to_str().unwrap())))
            .collect::<ssx_json::Map<_, _>>();
        let body = response.text().await.unwrap();
        let after = fs::read_to_string(out.join("engine-calls.jsonl"))
            .unwrap()
            .lines()
            .count();
        let mut differences = Vec::new();
        if u64::from(status) != case["status"].as_u64().unwrap() {
            differences.push("status");
        }
        if after - before != case["engine_calls"].as_array().unwrap().len() {
            differences.push("dispatch-count");
        }
        for name in [
            "content-type",
            "x-request-id",
            "content-security-policy",
            "referrer-policy",
            "x-content-type-options",
            "permissions-policy",
        ] {
            if headers.get(name) != case["headers"].get(name) {
                differences.push(name);
            }
        }
        if input["path"] == "/metrics" {
            let elapsed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64()
                - runtime_start;
            if !metrics_match(case["body"].as_str().unwrap(), &body, after, elapsed) {
                differences.push("whole-typed-metrics");
            }
            snapshots.insert(id.to_owned(), body.clone());
        } else if case["headers"]["content-type"]
            .as_str()
            .unwrap()
            .starts_with("application/json")
        {
            let mut actual: Value = ssx_json::from_str(&body).unwrap();
            let mut expected: Value = ssx_json::from_str(case["body"].as_str().unwrap()).unwrap();
            if let Some(id) = actual["meta"].get("query_id") {
                assert!(id.as_str().unwrap().starts_with("ssx-"));
            }
            if input.get("formatter").is_some() {
                assert!(actual["results"][0]["pubdate"].is_null());
            }
            normalize_json(&mut actual);
            normalize_json(&mut expected);
            if actual != expected {
                differences.push("whole-json-body");
            }
        } else {
            assert_eq!(id, "review_duplicate_test_then_empty");
            for marker in [
                "Search <em>SlopSearX.</em>",
                "action=\"/search\"",
                "data-theme-toggle",
                "data-default-theme=\"dark\"",
            ] {
                if !body.contains(marker) {
                    differences.push("landing-literal");
                }
            }
            if body.contains("Search results for ") {
                differences.push("landing-became-search");
            }
        }
        let receipt = out.join(format!("{id}.json"));
        fs::write(&receipt,json!({"id":id,"request":input,"status":status,"headers":headers,"body":body,"engine_call_delta":after-before,"differences":differences}).to_string()).unwrap();
        bind_receipt("review", id, &receipt, Some(&bindings[id]));
        assert!(differences.is_empty(), "{id}: {differences:?}");
        assert!(completed.insert(id.to_owned()));
    }
    let sample = |id: &str, key: &str| -> f64 {
        snapshots[id]
            .lines()
            .find_map(|line| {
                line.strip_prefix(key)
                    .and_then(|v| v.strip_prefix(' '))
                    .map(|v| v.parse().unwrap())
            })
            .unwrap_or(0.)
    };
    let initial = "review_metrics_initial";
    let category = "review_metrics_category";
    let search = "review_metrics_duplicate_search";
    let landing = "review_metrics_duplicate_landing";
    let other = "slopsearx_server_requests_by_category_total{category=\"other\"}";
    assert_eq!(sample(category, other) - sample(initial, other), 1.);
    for key in [
        "slopsearx_server_requests_total{}",
        "slopsearx_http_search_completed_total{outcome=\"success\"}",
        "slopsearx_http_search_duration_seconds_count{outcome=\"success\"}",
    ] {
        assert_eq!(
            sample(search, key) - sample(category, key),
            1.,
            "empty then test must count a search: {key}"
        );
        assert_eq!(
            sample(landing, key) - sample(search, key),
            0.,
            "test then empty must exclude landing: {key}"
        );
    }
    assert_eq!(
        snapshots[landing], snapshots[search],
        "landing must leave the entire metrics snapshot unchanged"
    );
    let failure = "slopsearx_http_search_duration_seconds_count{outcome=\"failure\"}";
    assert_eq!(sample(search, failure) - sample(category, failure), 0.);
    assert_eq!(sample(landing, failure) - sample(search, failure), 0.);
    let clean = stop(&mut process).await;
    fs::write(out.join("terminal.json"),json!({"completed":completed,"expected":9,"metric_deltas_verified":true,"child_joined":clean}).to_string()).unwrap();
    assert!(clean);
    assert_eq!(completed.len(), 9);
}

#[test]
fn unobserved_metric_outcomes_require_joint_complete_absence() {
    let first =
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_review_records.jsonl")
            .lines()
            .next()
            .unwrap();
    let row: Value = ssx_json::from_str(first).unwrap();
    let initial = row["body"].as_str().unwrap();
    assert!(metrics_match(initial, initial, 0, 1.));
    let partial =
        format!("{initial}slopsearx_http_search_duration_seconds_sum{{outcome=\"success\"}} 0\n");
    assert!(
        !metrics_match(&partial, &partial, 0, 1.),
        "joint partial histograms must fail"
    );
    assert!(
        !metrics_match(initial, &partial, 0, 1.),
        "extra native histogram must fail"
    );
    assert!(
        !metrics_match(&partial, initial, 0, 1.),
        "missing native histogram must fail"
    );
}

#[tokio::test]
async fn yaml_invalid_date_field_escapes_nonprintable_unicode_before_dispatch() {
    let _process_test_guard = PROCESS_TEST_LOCK.lock().await;
    let out = directory("unicode-repr");
    let (mut process, base, _) = start(&out, "dark");
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let before = fs::read(out.join("engine-calls.jsonl")).unwrap();
    let response = client
        .get(format!("{base}/search?q=x&format=yaml&date%E2%80%A8x=1"))
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let content_type = response.headers()["content-type"]
        .to_str()
        .unwrap()
        .to_owned();
    let body = response.text().await.unwrap();
    let after = fs::read(out.join("engine-calls.jsonl")).unwrap();
    fs::write(
        out.join("response.json"),
        json!({"status":status,"content_type":content_type,"body":body}).to_string(),
    )
    .unwrap();
    let clean = stop(&mut process).await;
    fs::write(
        out.join("terminal.json"),
        json!({"child_exit_success":clean,"source_control":true}).to_string(),
    )
    .unwrap();
    assert!(clean);
    assert_eq!(status, 400);
    assert_eq!(content_type, "text/vnd.yaml+markdown; charset=utf-8");
    assert_eq!(
        body,
        "error: 'invalid_filter'\nmessage: 'Unknown or repeated date parameter.'\nfield: 'date\\u2028x'\n"
    );
    assert_eq!(
        after, before,
        "invalid date field must not dispatch an engine"
    );
}

#[tokio::test]
async fn successful_yaml_break_scalars_match_pinned_emitter_on_actual_http() {
    let _process_test_guard = PROCESS_TEST_LOCK.lock().await;
    let controls: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/t02/yaml-break-source-controls-v1.json"
    ))
    .unwrap();
    let rows = controls["records"].as_array().unwrap();
    assert_eq!(rows.len(), 18);
    let out = directory("yaml-break-scalars");
    let (mut process, base, _) = start(&out, "dark");
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let mut differences = Vec::new();
    for (index, case) in rows.iter().enumerate() {
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query.append_pair("q", case["query"].as_str().unwrap());
        query.append_pair("format", "yaml");
        let request_target = format!("{base}/search?{}", query.finish());
        let response = client.get(&request_target).send().await.unwrap();
        let status = response.status().as_u16();
        let media = response.headers()["content-type"]
            .to_str()
            .unwrap()
            .to_owned();
        let body = response.text().await.unwrap();
        let document = body
            .split_once("---\n## Results Summary\n")
            .map(|(yaml, _)| yaml)
            .unwrap_or("");
        let mut mismatches = Vec::new();
        for (field, expected) in case["fragments"].as_object().unwrap() {
            if !document.contains(expected.as_str().unwrap()) {
                mismatches.push(field.clone());
            }
        }
        if status != 200
            || media != "text/vnd.yaml+markdown; charset=utf-8"
            || !mismatches.is_empty()
        {
            differences
                .push(json!({"id":case["id"],"status":status,"media":media,"fields":mismatches}));
        }
        fs::write(out.join(format!("{index:02}.json")),json!({"id":case["id"],"request_target":request_target,"status":status,"media":media,"body":body,"mismatches":mismatches}).to_string()).unwrap();
    }
    let calls = fs::read_to_string(out.join("engine-calls.jsonl")).unwrap();
    let calls = calls
        .lines()
        .map(|line| ssx_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let clean = stop(&mut process).await;
    fs::write(out.join("terminal.json"),json!({"expected":rows.len(),"completed":rows.len(),"differences":differences,"engine_calls":calls.len(),"child_joined":clean}).to_string()).unwrap();
    assert!(clean, "owned actual HTTP child must shut down successfully");
    assert_eq!(
        calls.len(),
        rows.len(),
        "every successful input must reach the counted engine"
    );
    for (call, case) in calls.iter().zip(rows) {
        assert_eq!(call["query"], case["query"]);
    }
    assert!(
        differences.is_empty(),
        "successful YAML scalar source differences: {differences:?}"
    );
}

#[tokio::test]
async fn lossless_large_page_html_links_preserve_filters_and_exact_decimal_neighbors() {
    let _process_test_guard = PROCESS_TEST_LOCK.lock().await;
    let out = directory("html-lossless-pages");
    let (mut process, base, _) = start(&out, "darker");
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let cases = [
        (
            "18446744073709551616",
            "18446744073709551615",
            "18446744073709551617",
        ),
        (
            "170141183460469231731687303715884105727",
            "170141183460469231731687303715884105726",
            "170141183460469231731687303715884105728",
        ),
        (
            "170141183460469231731687303715884105728",
            "170141183460469231731687303715884105727",
            "170141183460469231731687303715884105729",
        ),
    ];
    let page_url = |pairs: &[(&str, &str)]| {
        let mut url = reqwest::Url::parse(&format!("{base}/search")).unwrap();
        url.query_pairs_mut().extend_pairs(pairs.iter().copied());
        url
    };
    let mut captures = Vec::new();
    for (page, _, _) in cases {
        let response = client
            .get(page_url(&[
                ("q", "large <page> & scope"),
                ("format", "html"),
                ("pageno", page),
                ("categories", "general"),
                ("engines", "contract"),
                ("language", "fr"),
                ("safesearch", "0"),
                ("time_range", "day"),
            ]))
            .send()
            .await;
        match response {
            Ok(response) => {
                let status = response.status().as_u16();
                let headers = response
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.as_str().to_owned(), json!(v.to_str().unwrap())))
                    .collect::<ssx_json::Map<_, _>>();
                captures.push((status, headers, response.text().await));
            }
            Err(error) => {
                captures.push((0, ssx_json::Map::new(), Err(error)));
                break;
            }
        }
    }
    let boundary = "9".repeat(4300);
    let boundary_previous = format!("{}8", "9".repeat(4299));
    let boundary_requests = async {
        let html = client
            .get(page_url(&[
                ("q", "boundary"),
                ("format", "html"),
                ("pageno", boundary.as_str()),
            ]))
            .send()
            .await?;
        let html_status = html.status().as_u16();
        let html_headers = html
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), json!(v.to_str().unwrap())))
            .collect::<ssx_json::Map<_, _>>();
        let html = html.text().await?;
        let json = client
            .get(page_url(&[
                ("q", "boundary"),
                ("format", "json"),
                ("pageno", boundary.as_str()),
            ]))
            .send()
            .await?;
        let json_status = json.status().as_u16();
        let json_headers = json
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), json!(v.to_str().unwrap())))
            .collect::<ssx_json::Map<_, _>>();
        let json = json.text().await?;
        Ok::<_, reqwest::Error>((
            html_status,
            html_headers,
            html,
            json_status,
            json_headers,
            json,
        ))
    }
    .await;
    let clean = stop(&mut process).await;
    assert!(clean);
    assert_eq!(captures.len(), cases.len());
    let mut records = Vec::new();
    for ((status, headers, body), (page, previous, next)) in captures.into_iter().zip(cases) {
        assert_eq!(status, 200);
        let body = body.unwrap();
        let mut pages = Vec::new();
        for part in body.split("href=\"").skip(1) {
            let href = part.split_once('"').unwrap().0.replace("&amp;", "&");
            if !href.starts_with("/search?") {
                continue;
            }
            let parsed = reqwest::Url::parse(&format!("http://fixture{href}")).unwrap();
            let fields = parsed
                .query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect::<std::collections::BTreeMap<_, _>>();
            let Some(link_page) = fields.get("pageno") else {
                continue;
            };
            assert_eq!(fields["q"], "large <page> & scope");
            assert_eq!(fields["categories"], "general");
            assert_eq!(fields["engines"], "contract");
            assert_eq!(fields["language"], "fr");
            assert_eq!(fields["safesearch"], "0");
            assert_eq!(fields["time_range"], "day");
            pages.push(link_page.clone());
        }
        assert_eq!(pages, [previous, next]);
        records
            .push(json!({"page":page,"status":status,"headers":headers,"body":body,"previous":previous,"next":next}));
    }
    let (html_status, html_headers, html, json_status, json_headers, json_body) =
        boundary_requests.unwrap();
    assert_eq!(html_status, 200);
    assert!(html.contains("aria-disabled=\"true\""));
    assert!(html.contains("Next page unavailable (page number limit)"));
    assert!(html.contains(&format!("aria-current=\"page\">Page {boundary}</span>")));
    assert!(html.contains(&format!("pageno={boundary_previous}\"")));
    assert!(!html.contains(&format!("pageno=1{}", "0".repeat(4300))));
    assert_eq!(json_status, 200);
    assert!(
        ssx_json::from_str::<Value>(&json_body).unwrap()["results"]
            .as_array()
            .is_some()
    );
    let call_text = fs::read_to_string(out.join("engine-calls.jsonl")).unwrap();
    let last: Value = ssx_json::from_str(call_text.lines().last().unwrap()).unwrap();
    assert_eq!(last["pageno"].to_string(), boundary);
    records.push(json!({"page":boundary,"status":html_status,"headers":html_headers,"body":html,"adapter_page":last["pageno"].to_string()}));
    records.push(json!({"page":boundary,"status":json_status,"headers":json_headers,"body":json_body,"adapter_page":last["pageno"].to_string()}));
    let raw_http_records = out.join("native-records.jsonl");
    let raw = records
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&raw_http_records, &raw).unwrap();
    let receipt = out.join("terminal.json");
    fs::write(
        &receipt,
        json!({"child_exit_success":clean,"requests_completed":5,"raw_http_records":raw_http_records.to_string_lossy().as_ref(),"raw_http_records_sha256":digest(raw.as_bytes())}).to_string(),
    )
    .unwrap();
    bind_receipt("html", "lossless-pages", &receipt, None);
}
