//! Complete normalized source-result replay. Raw captures and source expectations
//! are immutable inputs; transport-only message differences are explicit below.
use serde_json::Value;
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission};
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{AllowAllAdmission, HtmlConfig, register_html},
    http::{AdapterHttp, Error, ExchangeFuture, Request, Response, Transport},
};
use ssx_registry::Registry;
use std::sync::{Arc, Mutex};

struct Replay {
    case: Value,
    seen: Mutex<Vec<Request>>,
}
impl Transport for Replay {
    fn exchange(&self, r: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let i = {
                let mut seen = self.seen.lock().unwrap();
                let i = seen.len();
                seen.push(r.clone());
                i
            };
            let ex = &self.case["exchanges"][i];
            assert!(!ex.is_null(), "unexpected request {}", self.case["id"]);
            if ex["timeout"] == true || (i == 0 && self.case["bootstrap_failure"] == true) {
                return Err(Error::Deadline);
            }
            if ex["network"] == true {
                return Err(Error::Network);
            }
            let mut headers = reqwest::header::HeaderMap::new();
            if let Some(pairs) = ex["headers"].as_array() {
                for pair in pairs {
                    headers.append(
                        reqwest::header::HeaderName::from_bytes(
                            pair[0].as_str().unwrap().as_bytes(),
                        )
                        .unwrap(),
                        reqwest::header::HeaderValue::from_bytes(
                            pair[1].as_str().unwrap().as_bytes(),
                        )
                        .unwrap(),
                    );
                }
            } else {
                for (k, v) in ex["headers"].as_object().unwrap() {
                    headers.insert(
                        reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                        v.as_str().unwrap().parse().unwrap(),
                    );
                }
            }
            Ok(Response {
                status: ex["status"].as_u64().unwrap() as u16,
                url: r.url,
                headers,
                body: hex(ex["body_hex"].as_str().unwrap()),
            })
        })
    }
}
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}
async fn run(case: &Value) {
    let id = case["id"].as_str().unwrap();
    let transport = Arc::new(Replay {
        case: case.clone(),
        seen: Mutex::new(vec![]),
    });
    let http = Arc::new(AdapterHttp::default());
    http.set_transport(transport.clone()).unwrap();
    let mut config = HtmlConfig::default();
    let max = case.get("limit").cloned().unwrap_or(10.into());
    config.duckduckgo.max_results = ssx_json::from_str(&max.to_string()).unwrap();
    config.google.max_results = ssx_json::from_str(&max.to_string()).unwrap();
    config.google.timeout_ms = 5000.into();
    config.duckduckgo.timeout_ms = 5000.into();
    if let Some(c) = case.get("config") {
        if let Some(base) = c["base_url"].as_str() {
            config.google.base_url = base.into();
            config.duckduckgo.base_url = base.into();
        }
        if let Some(home) = c["bootstrap_url"].as_str() {
            config.duckduckgo.bootstrap_url = home.into();
        }
    }
    let mut registry = Registry::builtins().unwrap();
    register_html(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        config,
    )
    .unwrap();
    let engine = registry
        .instantiate(case["engine"].as_str().unwrap())
        .unwrap();
    let response = engine
        .search(&SearchRequest {
            query: case["query"].as_str().unwrap().into(),
            categories: case.get("categories").map(|v| {
                v.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().into())
                    .collect()
            }),
            page: ssx_domain::SearchPage::from_u64(7).expect("positive fixture page"),
            language: "fr".into(),
            safesearch: 2,
            time_range: Some("day".into()),
            max_results: Some(1),
            ..SearchRequest::default()
        })
        .await;
    let results:Vec<Value>=response.results.iter().map(|r|{let mut value:Value=serde_json::from_str(&ssx_json::to_string(&ssx_domain::storage::encode_result(r).unwrap()).unwrap()).unwrap();if let Some(media)=&r.media {value["media"]=serde_json::json!({"media_type":media.media_type,"url":media.url,"thumbnail":media.thumbnail,"source":media.source,"width":media.width,"height":media.height,"duration":media.duration.as_ref().map(|v|v.get())});}value}).collect();
    let actual = serde_json::json!({"status":response.status,"results":results,"error_message":response.error_message,"answers":response.answers,"corrections":response.corrections,"infoboxes":response.infoboxes,"synthetic":response.synthetic});
    let mut expected = case["expected"].clone();
    // Explicit transport exception diagnostics differ because F08 owns a typed
    // native error rather than an HTTPX exception. No result/status is normalized.
    if id == "google_network" {
        assert_eq!(expected["error_message"], "source oracle network");
        expected["error_message"] = "Network".into();
    }
    assert_eq!(actual, expected, "{id}: complete normalized outcome");
    let seen = transport.seen.lock().unwrap().clone();
    let requests = case["requests"].as_array().unwrap();
    assert_eq!(seen.len(), requests.len(), "{id}: count");
    for (actual, expected) in seen.iter().zip(requests) {
        assert_eq!(
            actual.method.as_str(),
            expected["method"].as_str().unwrap(),
            "{id}"
        );
        assert_eq!(
            actual.url.as_str(),
            expected["url"].as_str().unwrap(),
            "{id}"
        );
        assert_eq!(
            actual.body,
            hex(expected["body_hex"].as_str().unwrap()),
            "{id}"
        );
        // F08 Transport receives adapter headers and cookie state. Host,
        // connection, encoding and body length are added by the network client.
        let mut expected: std::collections::BTreeMap<_, _> = expected["headers"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(k, _)| {
                !["host", "connection", "accept-encoding", "content-length"].contains(&k.as_str())
            })
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
            .collect();
        if id == "ddg_cookie_continuity" && actual.method == reqwest::Method::POST {
            assert_eq!(expected.remove("cookie").as_deref(), Some("a05=one"));
        }
        let actual: std::collections::BTreeMap<_, _> = actual
            .headers
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), v.to_str().unwrap().to_owned()))
            .collect();
        assert_eq!(actual, expected, "{id}: complete adapter-owned headers");
    }
    http.shutdown_and_wait().await;
    println!("\nA05_CASE {id}");
}
#[tokio::test]
async fn source_corpus_complete_outcomes_and_requests() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../docs/rust-rewrite/a05/corpus.json")).unwrap();
    let review: Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a05/corpus-review.json"
    ))
    .unwrap();
    let original = corpus["cases"].as_array().unwrap();
    let appended = review["cases"].as_array().unwrap();
    assert_eq!(original.len(), 115);
    assert_eq!(appended.len(), 43);
    let review_2: Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a05/corpus-review-2.json"
    ))
    .unwrap();
    let second = review_2["cases"].as_array().unwrap();
    assert_eq!(second.len(), 28);
    let unicode: Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a05/surrogate-pipeline-observations.json"
    ))
    .unwrap();
    let unicode_cases = unicode["cases"].as_array().unwrap();
    assert_eq!(unicode_cases.len(), 44);
    let nul: Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a05/nul-version-observations.json"
    ))
    .unwrap();
    let nul_expanded: Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a05/nul-expanded-observations.json"
    ))
    .unwrap();
    let nul_cases = nul["cases"].as_array().unwrap();
    let expanded_cases = nul_expanded["cases"].as_array().unwrap();
    assert_eq!(nul_cases.len(), 180);
    assert_eq!(expanded_cases.len(), 168);
    let long: Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a05/long-query-observations.json"
    ))
    .unwrap();
    let long_cases = long["cases"].as_array().unwrap();
    assert_eq!(long_cases.len(), 9);
    let cases = original
        .iter()
        .chain(appended)
        .chain(second)
        .chain(unicode_cases)
        .chain(nul_cases)
        .chain(expanded_cases)
        .chain(long_cases);
    let mut failures = vec![];
    for case in cases {
        let c = case.clone();
        if tokio::spawn(async move { run(&c).await }).await.is_err() {
            failures.push(case["id"].clone());
        }
    }
    assert!(failures.is_empty(), "source mismatches: {failures:?}");
}
#[tokio::test]
async fn source_charset_pipeline_complete_outcomes_and_requests() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a05/http-charset-pipeline-source.json"
    ))
    .unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 26);
    for case in cases {
        run(case).await;
    }
}
struct Deny;
impl SearchAdmission for Deny {
    fn admit<'a>(&'a self, _: &'static str) -> PortFuture<'a, AdmissionDecision> {
        Box::pin(async { AdmissionDecision::Denied })
    }
}
#[tokio::test]
async fn admission_denies_both_engines_before_bootstrap() {
    let http = Arc::new(AdapterHttp::default());
    let mut registry = Registry::builtins().unwrap();
    register_html(
        &mut registry,
        http.clone(),
        Arc::new(Deny),
        HtmlConfig::default(),
    )
    .unwrap();
    for name in ["duckduckgo", "google"] {
        let result = registry
            .instantiate(name)
            .unwrap()
            .search(&SearchRequest::default())
            .await;
        assert_eq!(result.status, EngineStatus::RateLimited);
        assert!(result.results.is_empty());
    }
    http.shutdown_and_wait().await;
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

#[tokio::test]
async fn cancellation_during_bootstrap_sends_no_search_or_fallback() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (notify, received) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut warm, _) = listener.accept().await.unwrap();
        assert!(read_request(&mut warm).await.starts_with("GET /warm"));
        reply(&mut warm, "warm", false).await;
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_request(&mut stream).await;
        notify.send(request).unwrap();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), listener.accept())
                .await
                .is_err()
        );
    });
    let mut config = HtmlConfig::default();
    config.duckduckgo.bootstrap_url = format!("{base}/home");
    config.duckduckgo.base_url = format!("{base}/html");
    config.duckduckgo.fallback_base_url = format!("{base}/lite");
    let http = Arc::new(AdapterHttp::default());
    warm(&http, &base).await;
    let mut registry = Registry::builtins().unwrap();
    register_html(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        config,
    )
    .unwrap();
    let engine = registry.instantiate("duckduckgo").unwrap();
    let search = tokio::spawn(async move { engine.search(&SearchRequest::default()).await });
    assert!(received.await.unwrap().starts_with("GET /home"));
    search.abort();
    assert!(search.await.unwrap_err().is_cancelled());
    server.await.unwrap();
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn loopback_proxy_rotation_keeps_one_route_per_search_and_applies_feedback() {
    // Both proxy listeners receive absolute HTTP target URLs; no provider DNS or
    // network call occurs. A blocked endpoint cools route one, next search uses two.
    let first = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let second = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxies = vec![
        format!("http://{}", first.local_addr().unwrap()),
        format!("http://{}", second.local_addr().unwrap()),
    ];
    let seen = Arc::new(Mutex::new(vec![]));
    let a = seen.clone();
    let one = tokio::spawn(async move {
        let (mut s, _) = first.accept().await.unwrap();
        let r = read_request(&mut s).await;
        a.lock().unwrap().push(r);
        reply(&mut s, "challenge", false).await;
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), first.accept())
                .await
                .is_err()
        );
    });
    let b = seen.clone();
    let two = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut s, _) = second.accept().await.unwrap();
            let r = read_request(&mut s).await;
            b.lock().unwrap().push(r);
            reply(
                &mut s,
                "<div class='g'><a href='https://a'><h3>A</h3></a></div>",
                false,
            )
            .await;
        }
    });
    let mut config = HtmlConfig::default();
    config.google.base_url = "http://provider.invalid/search".into();
    config.google.scrape.proxy_pool = proxies;
    let http = Arc::new(AdapterHttp::default());
    let mut registry = Registry::builtins().unwrap();
    register_html(
        &mut registry,
        http.clone(),
        Arc::new(AllowAllAdmission),
        config,
    )
    .unwrap();
    let engine = registry.instantiate("google").unwrap();
    let first = engine
        .search(&SearchRequest {
            query: "one".into(),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(first.status, EngineStatus::Blocked);
    let second = engine
        .search(&SearchRequest {
            query: "two".into(),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(second.status, EngineStatus::Ok);
    assert_eq!(second.results[0].title, "A");
    let third = engine
        .search(&SearchRequest {
            query: "three".into(),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(third.status, EngineStatus::Ok);
    one.await.unwrap();
    two.await.unwrap();
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 3);
    assert!(seen[0].starts_with("GET http://provider.invalid/search?q=one&hl=en&num=10 HTTP/1.1"));
    assert!(seen[1].starts_with("GET http://provider.invalid/search?q=two&hl=en&num=10 HTTP/1.1"));
    assert!(
        seen[2].starts_with("GET http://provider.invalid/search?q=three&hl=en&num=10 HTTP/1.1")
    );
    drop(seen);
    http.shutdown_and_wait().await;
}

async fn warm(http: &AdapterHttp, base: &str) {
    use ssx_infra::http::{Cancellation, Options};
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

#[test]
fn html_factories_expose_live_availability_and_preserve_f06_metadata() {
    let mut r = Registry::builtins().unwrap();
    let descriptors: Vec<_> = ["duckduckgo", "google"]
        .iter()
        .map(|n| r.descriptor(n).unwrap().clone())
        .collect();
    assert!(!r.is_available("duckduckgo"));
    assert!(!r.is_available("google"));
    register_html(
        &mut r,
        Arc::new(AdapterHttp::default()),
        Arc::new(AllowAllAdmission),
        HtmlConfig::default(),
    )
    .unwrap();
    for (name, before) in ["duckduckgo", "google"].iter().zip(descriptors) {
        assert!(r.is_available(name));
        assert_eq!(r.descriptor(name).unwrap(), &before);
        assert_eq!(before.engine_type, "scrape");
        assert!(before.categories.contains(&"general".into()));
    }
}

#[cfg(unix)]
fn blocked_helper_fixture(
    label: &str,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    ssx_infra::html::ParserPool,
) {
    use std::os::unix::fs::PermissionsExt;
    let directory = std::env::temp_dir().join(format!("a05-helper-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let script = directory.join("helper");
    let pid = directory.join("pid");
    let _ = std::fs::remove_file(&pid);
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$$\" > '{}'\nexec /bin/sleep 30\n",
            pid.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let pool =
        ssx_infra::html::ParserPool::new(script, std::num::NonZeroUsize::new(1).unwrap()).unwrap();
    (directory, pid, pool)
}
#[cfg(unix)]
fn fixture_task_outcome<T>(outcome: Result<Result<T, String>, tokio::task::JoinError>) -> String {
    match outcome {
        Ok(Ok(_)) => "unexpected successful helper completion before PID readiness".into(),
        Ok(Err(message)) => format!("helper returned error before PID readiness: {message:?}"),
        Err(error) => format!("helper caller join failed before PID readiness: {error}"),
    }
}
#[cfg(unix)]
async fn fixture_pid<T>(
    path: &std::path::Path,
    pool: &ssx_infra::html::ParserPool,
    task: &mut tokio::task::JoinHandle<Result<T, String>>,
) -> String {
    enum Observation {
        Pid(String),
        Early(String),
    }
    let observed = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let Ok(pid) = std::fs::read_to_string(path)
                && pid.ends_with('\n')
                && pid.trim().parse::<u32>().is_ok_and(|value| value > 0)
            {
                break Observation::Pid(pid);
            }
            tokio::select! {
                biased;
                outcome = &mut *task => break Observation::Early(fixture_task_outcome(outcome)),
                _ = tokio::time::sleep(std::time::Duration::from_millis(5)) => {}
            }
        }
    })
    .await;
    if let Ok(Observation::Pid(pid)) = observed {
        return pid;
    }
    // A spawn/protocol/deadline error must not be hidden behind PID polling.
    // Close and bound supervisor drain before reporting the actual observation.
    let cleanup = tokio::time::timeout(std::time::Duration::from_secs(2), pool.shutdown()).await;
    let detail = match observed {
        Ok(Observation::Early(message)) => message,
        Err(error) => {
            let caller = match tokio::time::timeout(std::time::Duration::from_secs(2), &mut *task)
                .await
            {
                Ok(outcome) => fixture_task_outcome(outcome),
                Err(join_timeout) => {
                    task.abort();
                    let aborted =
                        tokio::time::timeout(std::time::Duration::from_secs(2), &mut *task).await;
                    format!(
                        "caller join watchdog: {join_timeout}; abort completion: {}",
                        match aborted {
                            Ok(outcome) => fixture_task_outcome(outcome),
                            Err(error) => format!("abort join unverified: {error}"),
                        }
                    )
                }
            };
            format!("PID readiness deadline: {error}; {caller}")
        }
        Ok(Observation::Pid(_)) => unreachable!("returned above"),
    };
    panic!("complete owned PID missing at {path:?}: {detail}; owned pool shutdown={cleanup:?}");
}

#[cfg(unix)]
fn assert_reaped(pid: &str) {
    assert!(
        !std::process::Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success(),
        "helper PID {pid} remains alive or unreaped"
    );
}
#[cfg(unix)]
#[tokio::test]
async fn helper_deadline_kills_and_reaps_before_releasing_slot() {
    let (directory, pid, pool) = blocked_helper_fixture("deadline");
    let parser = pool.clone();
    let mut task = tokio::spawn(async move {
        parser
            .parse(
                "<html/>",
                tokio::time::Instant::now() + std::time::Duration::from_millis(500),
            )
            .await
    });
    let pid = fixture_pid(&pid, &pool, &mut task).await;
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.unwrap_err(), "HTML parser deadline exceeded");
    assert_eq!(pool.available_slots(), 1);
    assert_reaped(&pid);
    pool.shutdown().await.unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
#[cfg(unix)]
#[tokio::test]
async fn helper_future_cancellation_kills_and_reaps_before_releasing_slot() {
    let (directory, pid, pool) = blocked_helper_fixture("cancel");
    let parser = pool.clone();
    let mut task = tokio::spawn(async move {
        parser
            .parse(
                "<html/>",
                tokio::time::Instant::now() + std::time::Duration::from_secs(20),
            )
            .await
    });
    let pid = fixture_pid(&pid, &pool, &mut task).await;
    task.abort();
    let _ = task.await;
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while pool.available_slots() == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_reaped(&pid);
    pool.shutdown().await.unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
#[cfg(unix)]
#[tokio::test]
async fn helper_shutdown_kills_and_reaps_and_closes_admission() {
    let (directory, pid, pool) = blocked_helper_fixture("shutdown");
    let parser = pool.clone();
    let mut task = tokio::spawn(async move {
        parser
            .parse(
                "<html/>",
                tokio::time::Instant::now() + std::time::Duration::from_secs(20),
            )
            .await
    });
    let pid = fixture_pid(&pid, &pool, &mut task).await;
    tokio::time::timeout(std::time::Duration::from_secs(2), pool.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task.await.unwrap().unwrap_err(), "HTML parser shutdown");
    assert_eq!(pool.available_slots(), 1);
    assert_reaped(&pid);
    assert_eq!(
        pool.parse(
            "<html/>",
            tokio::time::Instant::now() + std::time::Duration::from_secs(1)
        )
        .await
        .unwrap_err(),
        "HTML parser admission closed"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
fn native_codec_helper_fixture(
    label: &str,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    ssx_infra::html::ParserPool,
) {
    use std::os::unix::fs::PermissionsExt;
    let (directory, pid, _) = blocked_helper_fixture(label);
    let helper = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("ssx-html-parser");
    let script = directory.join("helper");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$$\" > '{}'\nexec '{}'\n",
            pid.display(),
            helper.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let pool =
        ssx_infra::html::ParserPool::new(script, std::num::NonZeroUsize::new(1).unwrap()).unwrap();
    (directory, pid, pool)
}
#[cfg(unix)]
#[tokio::test]
async fn stateful_codec_deadline_kills_reaps_and_releases_permit() {
    let (directory, pid, pool) = native_codec_helper_fixture("codec-deadline");
    let mut bytes = vec![b'A'; 32 * 1024 * 1024];
    bytes[0] = b'+';
    let parser = pool.clone();
    let mut task = tokio::spawn(async move {
        parser
            .decode(
                &bytes,
                "utf-7",
                tokio::time::Instant::now() + std::time::Duration::from_millis(150),
            )
            .await
    });
    let pid = fixture_pid(&pid, &pool, &mut task).await;
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.unwrap_err(), "HTML parser deadline exceeded");
    assert_eq!(pool.available_slots(), 1);
    assert_reaped(&pid);
    assert_eq!(
        pool.decode(
            b"+AEI-",
            "utf-7",
            tokio::time::Instant::now() + std::time::Duration::from_secs(2)
        )
        .await
        .unwrap()
        .codepoints(),
        [66]
    );
    pool.shutdown().await.unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
#[cfg(unix)]
#[tokio::test]
async fn stateful_codec_cancellation_kills_reaps_and_releases_permit() {
    let (directory, pid, pool) = native_codec_helper_fixture("codec-cancel");
    let mut bytes = vec![b'A'; 32 * 1024 * 1024];
    bytes[0] = b'+';
    let parser = pool.clone();
    let mut task = tokio::spawn(async move {
        parser
            .decode(
                &bytes,
                "utf-7",
                tokio::time::Instant::now() + std::time::Duration::from_secs(20),
            )
            .await
    });
    let pid = fixture_pid(&pid, &pool, &mut task).await;
    // Permit enough time for real helper admission/input delivery; no sleep
    // executable substitutes for the native stateful decoder in this fixture.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(!task.is_finished());
    task.abort();
    let _ = task.await;
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while pool.available_slots() == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_reaped(&pid);
    pool.shutdown().await.unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn native_helper_rejects_noncanonical_frames() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let helper = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("ssx-html-parser");
    let frame = |version: u8, operation: u8, count: u64, payload: &[u8]| {
        let mut bytes = b"A05H".to_vec();
        bytes.extend([version, operation, 0]);
        bytes.extend(count.to_be_bytes());
        bytes.extend(payload);
        bytes
    };
    for (name, bytes, diagnostic) in [
        (
            "version",
            frame(5, 0, 0, &[]),
            "invalid HTML helper protocol version",
        ),
        ("operation", frame(1, 2, 0, &[]), "invalid helper operation"),
        (
            "oversize",
            frame(1, 0, 32 * 1024 * 1024 + 1, &[]),
            "oversize HTML helper input",
        ),
        (
            "trailing",
            frame(1, 0, 0, &[0]),
            "HTML helper trailing input",
        ),
    ] {
        let mut child = Command::new(&helper)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success(), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap().trim(),
            diagnostic,
            "{name}"
        );
    }
}
