//! Portable replay and structural differential boundary, independent of backend imports.
use crate::{Result, command, load_json};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Exchange {
    pub request: Request,
    pub response: Response,
}

// JSON equality deliberately retains integer/float distinctions and absent/null fields.
pub fn compare(expected: &Value, actual: &Value, volatile: &[String]) -> Result<()> {
    validate_numbers(expected)?;
    validate_numbers(actual)?;
    let mut a = expected.clone();
    let mut b = actual.clone();
    for pointer in volatile {
        if !matches!(
            pointer.as_str(),
            "/body/meta/response_time" | "/body/meta/response_time_ms" | "/body/latency_ms"
        ) {
            return Err(format!("unapproved volatile pointer {pointer}"));
        }
        let x = a
            .pointer_mut(pointer)
            .ok_or("volatile expected field missing")?;
        let y = b
            .pointer_mut(pointer)
            .ok_or("volatile actual field missing")?;
        if !x.is_number()
            || !y.is_number()
            || x.as_f64().is_none_or(|v| v < 0.0)
            || y.as_f64().is_none_or(|v| v < 0.0)
        {
            return Err("invalid volatile numeric value".into());
        }
        *x = Value::Null;
        *y = Value::Null;
    }
    if a != b {
        return Err(format!(
            "structural differential mismatch at {}",
            difference(&a, &b, "")
        ));
    }
    Ok(())
}
fn read_request(stream: &mut TcpStream) -> Result<Request> {
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream.take(65536));
    let mut first = String::new();
    reader.read_line(&mut first).map_err(|e| e.to_string())?;
    let parts: Vec<_> = first.split_whitespace().collect();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" {
        return Err("invalid replay request line".into());
    }
    let method = parts[0].to_owned();
    let target = parts[1].to_owned();
    let mut headers = BTreeMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        if line == "\r\n" {
            break;
        }
        if line.is_empty() {
            return Err("incomplete headers".into());
        }
        let (k, v) = line.split_once(':').ok_or("invalid header")?;
        let k = k.to_ascii_lowercase();
        if headers.insert(k, v.trim().to_owned()).is_some() {
            return Err("duplicate header".into());
        }
    }
    let host = headers.remove("host").ok_or("missing replay Host")?;
    if host != "localhost" && !host.starts_with("127.0.0.1:") {
        return Err("non-loopback transport Host".into());
    }
    if headers.contains_key("transfer-encoding") {
        return Err("chunked request unsupported in recorded replay".into());
    }
    headers.remove("connection");
    let len = headers
        .remove("content-length")
        .map(|v| v.parse::<usize>())
        .transpose()
        .map_err(|_| "invalid content length")?
        .unwrap_or(0);
    if len > 1024 * 1024 {
        return Err("oversize replay body".into());
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    Ok(Request {
        method,
        target,
        headers,
        body: String::from_utf8(body).map_err(|e| e.to_string())?,
    })
}
/// Runs a real loopback upstream. Client receives only its bound address, never a live URL.
pub fn replay<F>(exchanges: &[Exchange], client: F) -> Result<Vec<Request>>
where
    F: FnOnce(std::net::SocketAddr) -> Result<()>,
{
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let address = listener.local_addr().map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let exchanges = exchanges.to_vec();
    let count = exchanges.len();
    let (tx, rx) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || -> Result<Vec<Request>> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut observed = Vec::new();
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    // Darwin accept inherits O_NONBLOCK from the listener; timeouts alone
                    // do not make the accepted stream blocking. Avoid scheduling-dependent EAGAIN.
                    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
                    let request = read_request(&mut stream)?;
                    let index = observed.len();
                    observed.push(request.clone());
                    let matched = exchanges.get(index).filter(|e| e.request == request);
                    let response = matched.map(|e| e.response.clone()).unwrap_or(Response {
                        status: 599,
                        headers: BTreeMap::new(),
                        body: "unmatched replay request".into(),
                    });
                    let mut wire = format!(
                        "HTTP/1.1 {} Replay\r\nContent-Length: {}\r\nConnection: close\r\n",
                        response.status,
                        response.body.len()
                    );
                    for (k, v) in &response.headers {
                        if k.contains(['\r', '\n']) || v.contains(['\r', '\n']) {
                            return Err("unsafe response header".into());
                        }
                        wire.push_str(&format!("{k}: {v}\r\n"));
                    }
                    wire.push_str("\r\n");
                    wire.push_str(&response.body);
                    stream
                        .write_all(wire.as_bytes())
                        .map_err(|e| e.to_string())?;
                    if matched.is_none() {
                        return Err(format!("unmatched/out-of-order request {index}"));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e.to_string()),
            }
            if rx.try_recv().is_ok() {
                if observed.len() != count {
                    return Err(format!("request count {} != {count}", observed.len()));
                }
                return Ok(observed);
            }
            if Instant::now() > deadline {
                return Err("replay deadline exceeded".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    let result = client(address);
    let _ = tx.send(());
    let observed = server.join().map_err(|_| "replay server panic")?;
    if let Err(error) = result {
        return Err(format!("client: {error}; server: {observed:?}"));
    }
    observed
}
pub fn send(address: std::net::SocketAddr, request: &Request) -> Result<Response> {
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(2)).map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    if !request.target.starts_with('/')
        || request.target.contains(['\r', '\n'])
        || request.method.contains(char::is_whitespace)
    {
        return Err("unsafe request".into());
    }
    let mut wire = format!(
        "{} {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n",
        request.method,
        request.target,
        request.body.len()
    );
    for (k, v) in &request.headers {
        if k.contains(['\r', '\n']) || v.contains(['\r', '\n']) {
            return Err("unsafe request header".into());
        }
        wire.push_str(&format!("{k}: {v}\r\n"));
    }
    wire.push_str("\r\n");
    wire.push_str(&request.body);
    stream
        .write_all(wire.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut bytes = String::new();
    stream
        .read_to_string(&mut bytes)
        .map_err(|e| e.to_string())?;
    let (head, body) = bytes
        .split_once("\r\n\r\n")
        .ok_or("invalid replay response")?;
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|v| v.split_whitespace().nth(1))
        .ok_or("missing status")?
        .parse()
        .map_err(|_| "invalid status")?;
    let headers = lines
        .filter_map(|v| v.split_once(':'))
        .filter(|(k, _)| {
            !k.eq_ignore_ascii_case("content-length") && !k.eq_ignore_ascii_case("connection")
        })
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    Ok(Response {
        status,
        headers,
        body: body.into(),
    })
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let corpus = load_json(&root.join("crates/xtask/fixtures/replay/corpus.json"))?;
    if corpus["schema_version"] != 1 {
        return Err("unsupported corpus version".into());
    }
    let sources = corpus["provenance"]["sources"]
        .as_array()
        .ok_or("missing provenance sources")?;
    if sources.is_empty()
        || corpus["provenance"]["license"]
            .as_str()
            .is_none_or(str::is_empty)
        || corpus["provenance"]["redaction"]
            .as_str()
            .is_none_or(str::is_empty)
    {
        return Err("missing licensing/redaction".into());
    }
    let baseline = corpus["baseline_commit"]
        .as_str()
        .ok_or("missing baseline")?;
    if baseline != "7b2d876307189d7a62000d1f289b3ef4b65eaff6" {
        return Err("unexpected accepted baseline".into());
    }
    validate_source_coverage(sources)?;
    let mut source_paths = std::collections::BTreeSet::new();
    for source in sources {
        let path = source["path"].as_str().ok_or("missing source path")?;
        if !source_paths.insert(path) {
            return Err("duplicate source provenance".into());
        }
        if path.starts_with('/') || path.split('/').any(|p| p == "..") {
            return Err("unsafe source path".into());
        }
        let blob = command(
            root,
            "git",
            &["rev-parse", &format!("{baseline}:{path}")],
            records,
        )?;
        if source["git_blob"] != blob.trim() {
            return Err(format!("source provenance differs: {path}"));
        }
    }
    let cases = corpus["cases"].as_array().ok_or("missing cases")?;
    if cases.is_empty() {
        return Err("empty corpus".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for case in cases {
        let id = case["id"].as_str().ok_or("missing case id")?;
        if !ids.insert(id) {
            return Err("duplicate case id".into());
        }
        if case["input"].is_null()
            || !case["clock"]["unix_seconds"].is_u64()
            || case["clock"]["ids"].as_array().is_none_or(Vec::is_empty)
        {
            return Err("missing deterministic input/clock/IDs".into());
        }
        validate_case(case)?;
        let ids: Vec<String> =
            serde_json::from_value(case["clock"]["ids"].clone()).map_err(|e| e.to_string())?;
        let mut context = FixedContext::new(
            case["clock"]["unix_seconds"]
                .as_u64()
                .ok_or("invalid clock")?,
            ids,
        )?;
        let fixed_id = context.next_id()?;
        records.push(json!({"program":"fixed-context","args":[id],"exit_code":0,"stdout":format!("clock={} id={fixed_id}",context.unix_seconds),"stderr":""}));
        let exchanges: Vec<Exchange> =
            serde_json::from_value(case["exchanges"].clone()).map_err(|e| e.to_string())?;
        if case["kind"] == "adapter_http" {
            let engine = case["input"]["engine"].as_str().ok_or("missing engine")?;
            let source = command(
                root,
                "git",
                &[
                    "show",
                    &format!("{baseline}:tests/fixtures/upstream_contracts/{engine}.json"),
                ],
                records,
            )?;
            let source: Value = serde_json::from_str(&source).map_err(|e| e.to_string())?;
            if exchanges.len() != 1
                || exchanges[0].response.status != 200
                || serde_json::from_str::<Value>(&exchanges[0].response.body)
                    .map_err(|e| e.to_string())?
                    != source["payload"]
            {
                return Err(format!(
                    "case {id}: recorded upstream body differs from pinned source"
                ));
            }
            let query = case["input"]["query"].as_str().ok_or("missing query")?;
            let encoded = query
                .bytes()
                .map(|b| {
                    if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                        (b as char).to_string()
                    } else {
                        format!("%{b:02X}")
                    }
                })
                .collect::<String>();
            let max = case["input"]["max_results"].as_u64().ok_or("missing max")?;
            let target = match engine {
                "npm" => format!("/-/v1/search?text={encoded}&size={max}"),
                "crates" => format!("/api/v1/crates?q={encoded}&per_page={max}"),
                "openalex" => format!("/works?search={encoded}&per_page={max}"),
                _ => return Err("unknown canary engine".into()),
            };
            if exchanges[0].request.method != "GET"
                || exchanges[0].request.target != target
                || !exchanges[0].request.body.is_empty()
            {
                return Err(format!(
                    "case {id}: source-derived request method/URL differs"
                ));
            }
        } else if !exchanges.is_empty() {
            return Err("fake-engine surface case must not dispatch HTTP".into());
        }
        let observed = replay(&exchanges, |address| {
            for exchange in &exchanges {
                let actual = send(address, &exchange.request)?;
                if actual != exchange.response {
                    return Err("upstream response differs".into());
                }
            }
            Ok(())
        })?;
        let volatile: Vec<String> =
            serde_json::from_value(case["volatility"].clone()).map_err(|e| e.to_string())?;
        compare(&case["expected"], &case["candidate"], &volatile)
            .map_err(|e| format!("case {id}: {e}"))?;
        records.push(json!({"program":"native-replay","args":[id],"exit_code":0,"stdout":json!({"case":id,"upstream_origin":case["upstream_origin"],"request_count":observed.len(),"requests":observed,"scope":"recorded transcript and candidate observations; no adapter parity certification"}).to_string(),"stderr":""}));
    }
    validate_coverage(cases)?;
    network_checks(records)?;
    mutation_checks(records)?;
    Ok(())
}

/// Standalone native comparator: the candidate is an observation emitted by a backend.
pub fn cli() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_none_or(|v| !v.starts_with("replay-")) {
        return false;
    }
    let result = (|| -> Result<()> {
        if args.first().is_some_and(|v| v == "replay-sleep-child") {
            std::thread::sleep(Duration::from_secs(20));
            std::process::exit(0);
        }
        if args
            .first()
            .is_some_and(|v| v == "replay-held-pipes-candidate")
        {
            std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                .arg("replay-sleep-child")
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .map_err(|e| e.to_string())?;
            std::process::exit(0);
        }
        if args
            .first()
            .is_some_and(|v| v == "replay-isolation-candidate")
        {
            candidate_isolation_canary(&args)?;
            std::process::exit(0);
        }
        if args.first().is_some_and(|v| v == "replay-network-probe") {
            #[cfg(target_os = "linux")]
            verify_linux_namespace()?;
            return network_probe();
        }
        if args.first().is_some_and(|v| v == "replay-run-isolated") {
            #[cfg(target_os = "linux")]
            {
                verify_linux_namespace()?;
                network_probe()?;
                return run_backend_inner(&args);
            }
            #[cfg(not(target_os = "linux"))]
            return Err("isolated namespace entrypoint is Linux-only".into());
        }
        if args.first().is_some_and(|v| v == "replay-run") {
            return run_backend(&args);
        }
        if args.len() != 3 {
            return Err("usage: cargo xtask replay-compare EXPECTED.json ACTUAL.json".into());
        }
        let (expected, actual) = if args[0] == "replay-compare-json" {
            (
                serde_json::from_str(&args[1]).map_err(|e| e.to_string())?,
                serde_json::from_str(&args[2]).map_err(|e| e.to_string())?,
            )
        } else if args[0] == "replay-compare" {
            (
                load_json(Path::new(&args[1]))?,
                load_json(Path::new(&args[2]))?,
            )
        } else {
            return Err("unknown replay command".into());
        };
        compare(&expected, &actual, &[])
    })();
    match result {
        Ok(()) => println!("{{\"status\":\"passed\"}}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_structure_and_numeric_contract() {
        for (a, b) in [
            (json!({}), json!({"x":null})),
            (json!({"x":1}), json!({"x":1.0})),
            (json!([1, 2]), json!([2, 1])),
            (json!(0), json!(-1)),
        ] {
            assert!(compare(&a, &b, &[]).is_err());
        }
        assert!(
            compare(
                &json!({"body":{"latency_ms":1}}),
                &json!({"body":{"latency_ms":2.5}}),
                &["/body/latency_ms".into()]
            )
            .is_ok()
        );
        assert!(
            compare(
                &json!({"body":{"latency_ms":1}}),
                &json!({"body":{"latency_ms":-1}}),
                &["/body/latency_ms".into()]
            )
            .is_err()
        );
        assert!(
            compare(
                &json!({"status":200}),
                &json!({"status":500}),
                &["/status".into()]
            )
            .is_err()
        );
    }
    fn exchange(target: &str) -> Exchange {
        Exchange {
            request: Request {
                method: "GET".into(),
                target: target.into(),
                headers: BTreeMap::new(),
                body: String::new(),
            },
            response: Response {
                status: 200,
                headers: BTreeMap::new(),
                body: "{}".into(),
            },
        }
    }
    #[test]
    fn accepted_socket_waits_for_a_delayed_request_within_deadline() {
        let expected = vec![exchange("/delayed")];
        replay(&expected, |address| {
            let mut socket = TcpStream::connect(address).map_err(|e| e.to_string())?;
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .map_err(|e| e.to_string())?;
            std::thread::sleep(Duration::from_millis(30));
            socket
                .write_all(b"GET /delayed HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .map_err(|e| e.to_string())?;
            let mut response = String::new();
            socket
                .read_to_string(&mut response)
                .map_err(|e| e.to_string())?;
            if !response.starts_with("HTTP/1.1 200 ") {
                return Err("delayed response missing".into());
            }
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn real_loopback_rejects_missing_extra_and_wrong_order() {
        let expected = vec![exchange("/one"), exchange("/two")];
        let positive = replay(&expected, |a| {
            for e in &expected {
                assert_eq!(send(a, &e.request)?, e.response);
            }
            Ok(())
        });
        assert!(positive.is_ok(), "{positive:?}");
        assert!(
            replay(&expected, |a| {
                send(a, &expected[0].request)?;
                Ok(())
            })
            .is_err()
        );
        assert!(
            replay(&expected, |a| {
                send(a, &expected[1].request)?;
                Ok(())
            })
            .is_err()
        );
        assert!(
            replay(&[], |a| {
                send(a, &expected[0].request)?;
                Ok(())
            })
            .is_err()
        );
    }
}

fn exact_keys(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or("expected object")?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(format!("complete schema differs; expected {keys:?}"));
    }
    Ok(())
}
fn validate_case(case: &Value) -> Result<()> {
    let kind = case["kind"].as_str().ok_or("missing case kind")?;
    if !matches!(kind, "adapter_http" | "search_http" | "mcp_search") {
        return Err("unsupported replay kind".into());
    }
    match kind {
        "adapter_http" => {
            exact_keys(
                &case["input"],
                &["engine", "query", "params", "max_results"],
            )?;
            if !case["input"]["query"].is_string()
                || !case["input"]["params"].is_object()
                || case["input"]["max_results"].as_u64().is_none_or(|v| v == 0)
            {
                return Err("invalid adapter input".into());
            }
        }
        "search_http" => {
            exact_keys(&case["input"], &["method", "path", "params"])?;
            if case["input"]["method"] != "GET"
                || case["input"]["path"] != "/search"
                || !case["input"]["params"]["q"].is_string()
            {
                return Err("invalid HTTP search input".into());
            }
        }
        "mcp_search" => {
            exact_keys(
                &case["input"],
                &[
                    "tool",
                    "query",
                    "engines",
                    "language",
                    "time_range",
                    "safesearch",
                ],
            )?;
            if case["input"]["tool"] != "slopsearx_search"
                || !case["input"]["query"].is_string()
                || !case["input"]["engines"].is_array()
            {
                return Err("invalid MCP search input".into());
            }
        }
        _ => return Err("unsupported kind".into()),
    }
    if case["oracle"]["baseline_commit"] != "7b2d876307189d7a62000d1f289b3ef4b65eaff6"
        || case["oracle"]["command"].as_str().is_none_or(str::is_empty)
    {
        return Err("missing oracle provenance".into());
    }
    for envelope in [&case["expected"], &case["candidate"]] {
        exact_keys(envelope, &["status", "body"])?;
        if kind == "adapter_http" {
            let origin = case["upstream_origin"]
                .as_str()
                .ok_or("missing logical upstream origin")?;
            let required_origin = match case["input"]["engine"].as_str() {
                Some("npm") => "https://registry.npmjs.org",
                Some("crates") => "https://crates.io",
                Some("openalex") => "https://api.openalex.org",
                _ => return Err("unsupported adapter canary".into()),
            };
            if origin != required_origin {
                return Err("engine/logical origin mismatch".into());
            }
            if !matches!(
                origin,
                "https://registry.npmjs.org" | "https://crates.io" | "https://api.openalex.org"
            ) {
                return Err("unrecognized logical upstream origin".into());
            }
            if !matches!(
                envelope["status"].as_str(),
                Some("ok" | "rate_limited" | "blocked" | "error" | "timeout" | "unavailable")
            ) {
                return Err("invalid adapter status".into());
            }
            let body = &envelope["body"];
            exact_keys(body, &["results", "latency_ms", "error_message"])?;
            if body["latency_ms"].as_f64().is_none_or(|v| v < 0.0)
                || !(body["error_message"].is_string() || body["error_message"].is_null())
            {
                return Err("invalid adapter numeric/error field".into());
            }
            let results = body["results"].as_array().ok_or("invalid results")?;
            for result in results {
                exact_keys(
                    result,
                    &[
                        "url",
                        "title",
                        "content",
                        "engine",
                        "engines",
                        "score",
                        "position",
                        "category",
                        "published_date",
                        "thumbnail",
                        "img_src",
                        "tier",
                        "media",
                        "payload",
                        "work_group",
                    ],
                )?;
                for key in ["url", "title", "content", "engine", "category"] {
                    if !result[key].is_string() {
                        return Err(format!("invalid result {key}"));
                    }
                }
                for key in ["published_date", "thumbnail", "img_src"] {
                    if !(result[key].is_string() || result[key].is_null()) {
                        return Err(format!("invalid nullable result {key}"));
                    }
                }
                if result["score"].as_f64().is_none_or(|v| !v.is_finite())
                    || !result["position"].is_u64()
                    || !matches!(result["tier"].as_u64(), Some(1 | 2))
                {
                    return Err("invalid result numeric constraints".into());
                }
                let engines = result["engines"]
                    .as_array()
                    .ok_or("engines must be array")?;
                if engines.iter().any(|v| !v.is_string())
                    || engines.windows(2).any(|w| w[0].as_str() >= w[1].as_str())
                {
                    return Err("engines must be sorted unique strings".into());
                }
                for key in ["media", "payload", "work_group"] {
                    if !(result[key].is_null() || result[key].is_object()) {
                        return Err(format!("invalid result object {key}"));
                    }
                }
            }
        } else {
            let body = &envelope["body"];
            if kind == "search_http" {
                exact_keys(
                    body,
                    &[
                        "query",
                        "results",
                        "engines",
                        "number_of_results",
                        "answers",
                        "corrections",
                        "infoboxes",
                        "suggestions",
                        "unresponsive_engines",
                        "meta",
                    ],
                )?;
                exact_keys(
                    &body["meta"],
                    &[
                        "response_time_ms",
                        "cached",
                        "query_id",
                        "engine_status",
                        "partial",
                        "deadline_exceeded",
                    ],
                )?;
                if !body["number_of_results"].is_u64()
                    || !body["meta"]["cached"].is_boolean()
                    || !body["meta"]["partial"].is_boolean()
                    || !body["meta"]["deadline_exceeded"].is_boolean()
                {
                    return Err("invalid HTTP numeric/boolean contract".into());
                }
            } else {
                exact_keys(
                    body,
                    &[
                        "query",
                        "results",
                        "scope",
                        "answers",
                        "corrections",
                        "infoboxes",
                        "empty_engines",
                        "enforcement",
                        "engine_outcomes",
                        "meta",
                        "warnings",
                    ],
                )?;
                exact_keys(
                    &body["enforcement"],
                    &["language", "time_range", "safesearch"],
                )?;
                for entry in body["enforcement"]
                    .as_object()
                    .ok_or("missing enforcement")?
                    .values()
                {
                    exact_keys(entry, &["requested", "status", "reason", "enforced_by"])?;
                    if !matches!(
                        entry["status"].as_str(),
                        Some("enforced" | "partially_enforced" | "unsupported" | "rejected")
                    ) || entry["reason"].as_str().is_none_or(str::is_empty)
                        || entry["enforced_by"]
                            .as_array()
                            .is_none_or(|a| a.iter().any(|v| !v.is_string()))
                    {
                        return Err("invalid enforcement schema".into());
                    }
                }
            }
            validate_surface(kind, body)?;
            if body["meta"]["response_time_ms"]
                .as_f64()
                .is_none_or(|v| !v.is_finite() || v < 0.0)
                || !body["query"].is_string()
                || !body["results"].is_array()
            {
                return Err("invalid surface scalar schema".into());
            }
            if envelope["status"]
                .as_u64()
                .is_none_or(|v| !(100..=599).contains(&v))
                || !envelope["body"].is_object()
            {
                return Err("invalid transport envelope".into());
            }
        }
    }
    Ok(())
}

/// Injectable deterministic seams. Exhausted IDs fail rather than inventing random values.
#[derive(Debug)]
pub struct FixedContext {
    pub unix_seconds: u64,
    ids: std::collections::VecDeque<String>,
}
impl FixedContext {
    pub fn new(unix_seconds: u64, ids: Vec<String>) -> Result<Self> {
        if ids.is_empty() || ids.iter().any(String::is_empty) {
            return Err("empty deterministic IDs".into());
        }
        Ok(Self {
            unix_seconds,
            ids: ids.into(),
        })
    }
    pub fn next_id(&mut self) -> Result<String> {
        self.ids
            .pop_front()
            .ok_or("fixed ID source exhausted".into())
    }
}

fn mutation_checks(records: &mut Vec<Value>) -> Result<()> {
    let expected = json!({"status":200,"body":{"enforcement":{"language":{"status":"unsupported"}},"results":[{"url":"https://example.org/a","score":1.0},{"url":"https://example.org/b","score":2.0}],"provenance":{"source":"pinned"},"optional":null},"requests":["GET /one","GET /two"]});
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    for (name, pointer, replacement) in [
        ("status", "/status", json!(500)),
        (
            "enforcement",
            "/body/enforcement/language/status",
            json!("enforced"),
        ),
        ("schema", "/body/optional", json!(false)),
        ("provenance", "/body/provenance/source", json!("other")),
        ("request-count", "/requests", json!(["GET /one"])),
        (
            "request-order",
            "/requests",
            json!(["GET /two", "GET /one"]),
        ),
        (
            "result-order",
            "/body/results",
            json!([{ "url":"https://example.org/b","score":2.0},{"url":"https://example.org/a","score":1.0}]),
        ),
        ("numeric-type", "/body/results/0/score", json!(1)),
    ] {
        let mut actual = expected.clone();
        *actual
            .pointer_mut(pointer)
            .ok_or("missing mutation pointer")? = replacement;
        let output = std::process::Command::new(&executable)
            .args([
                "replay-compare-json",
                &expected.to_string(),
                &actual.to_string(),
            ])
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            return Err(format!("comparator accepted {name} mutation"));
        }
        records.push(json!({"program":"negative-comparator","args":[name],"exit_code":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)}));
    }
    let a = Exchange {
        request: Request {
            method: "GET".into(),
            target: "/one".into(),
            headers: BTreeMap::new(),
            body: String::new(),
        },
        response: Response {
            status: 200,
            headers: BTreeMap::new(),
            body: "{}".into(),
        },
    };
    let mut b = a.clone();
    b.request.target = "/two".into();
    let exchanges = vec![a, b];
    for mode in ["missing", "extra", "order", "method", "url"] {
        let result = replay(&exchanges, |address| {
            let mut requests: Vec<_> = exchanges.iter().map(|e| e.request.clone()).collect();
            match mode {
                "missing" => {
                    requests.pop();
                }
                "extra" => requests.push(requests[0].clone()),
                "order" => requests.reverse(),
                "method" => requests[0].method = "POST".into(),
                "url" => requests[0].target = "/other".into(),
                _ => return Err("unknown mutation".into()),
            }
            for request in requests {
                send(address, &request)?;
            }
            Ok(())
        });
        if result.is_ok() {
            return Err(format!("replay accepted {mode} mutation"));
        }
        records.push(json!({"program":"negative-replay","args":[mode],"exit_code":1,"stdout":"","stderr":result.unwrap_err()}));
    }
    Ok(())
}

fn difference(a: &Value, b: &Value, path: &str) -> String {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for key in x.keys().chain(y.keys()) {
                let p = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                match (x.get(key), y.get(key)) {
                    (Some(v), Some(w)) if v != w => return difference(v, w, &p),
                    (None, Some(_)) | (Some(_), None) => return p,
                    _ => {}
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            for i in 0..x.len().max(y.len()) {
                let p = format!("{path}/{i}");
                match (x.get(i), y.get(i)) {
                    (Some(v), Some(w)) if v != w => return difference(v, w, &p),
                    (None, Some(_)) | (Some(_), None) => return p,
                    _ => {}
                }
            }
        }
        _ => {}
    }
    if path.is_empty() {
        "/".into()
    } else {
        path.into()
    }
}

/// Run an independently implemented client/backend against fixture-only loopback.
/// Backend must print one observation envelope as JSON and use the supplied seams.
fn run_backend(args: &[String]) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut inner = args.to_vec();
        inner[0] = "replay-run-isolated".into();
        let mut command = isolated_command(&executable, &inner)?;
        let output = bounded_candidate(&mut command, Duration::from_secs(9))?;
        if !output.status.success() {
            return Err(format!(
                "OS-isolated replay failed before/inside candidate: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        verify_os_isolation(&mut Vec::new())?;
        run_backend_inner(args)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = args;
        Err("OS-enforced offline replay is unsupported on this platform".into())
    }
}
fn run_backend_inner(args: &[String]) -> Result<()> {
    if args.len() < 5 || args[3] != "--" {
        return Err("usage: cargo xtask replay-run CORPUS.json CASE -- PROGRAM [ARGS...]".into());
    }
    let corpus = load_json(Path::new(&args[1]))?;
    let case = corpus["cases"]
        .as_array()
        .ok_or("missing cases")?
        .iter()
        .find(|c| c["id"] == args[2])
        .ok_or("unknown replay case")?;
    validate_case(case)?;
    let exchanges: Vec<Exchange> =
        serde_json::from_value(case["exchanges"].clone()).map_err(|e| e.to_string())?;
    let mut actual = None;
    replay(&exchanges, |address| {
        #[cfg(target_os = "macos")]
        let mut command = isolated_command(Path::new(&args[4]), &args[5..])?;
        #[cfg(not(target_os = "macos"))]
        let mut command = {
            let mut c = std::process::Command::new(&args[4]);
            c.args(&args[5..]);
            c
        };
        command
            .env("SSX_REPLAY_URL", format!("http://{address}"))
            .env(
                "SSX_REPLAY_ORIGIN",
                case["upstream_origin"].as_str().unwrap_or(""),
            )
            .env(
                "SSX_FIXED_UNIX_SECONDS",
                case["clock"]["unix_seconds"].to_string(),
            )
            .env("SSX_FIXED_IDS", case["clock"]["ids"].to_string())
            .env("SSX_REPLAY_INPUT", case["input"].to_string());
        let output = bounded_candidate(&mut command, Duration::from_secs(4))?;
        if !output.status.success() {
            return Err(format!(
                "candidate backend failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        actual = Some(
            serde_json::from_slice::<Value>(&output.stdout)
                .map_err(|e| format!("candidate observation: {e}"))?,
        );
        Ok(())
    })?;
    let actual = actual.ok_or("missing candidate observation")?;
    let mut candidate_case = case.clone();
    candidate_case["candidate"] = actual.clone();
    validate_case(&candidate_case)?;
    let volatility: Vec<String> =
        serde_json::from_value(case["volatility"].clone()).map_err(|e| e.to_string())?;
    compare(&case["expected"], &actual, &volatility)
}

fn validate_coverage(cases: &[Value]) -> Result<()> {
    let expected = std::collections::BTreeMap::from([
        ("upstream-npm-captured", "adapter_http"),
        ("upstream-crates-captured", "adapter_http"),
        ("upstream-openalex-captured", "adapter_http"),
        ("search-http-json-contract", "search_http"),
        ("mcp-search-filter-enforcement", "mcp_search"),
    ]);
    let mut seen = std::collections::BTreeMap::new();
    for case in cases {
        let id = case["id"].as_str().ok_or("missing case ID")?;
        let kind = case["kind"].as_str().ok_or("missing case kind")?;
        if seen.insert(id, kind).is_some() {
            return Err("duplicate corpus case".into());
        }
    }
    if seen != expected {
        return Err("required reviewed F04 corpus coverage differs".into());
    }
    Ok(())
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    #[test]
    fn promised_corpus_cannot_shrink_grow_duplicate_or_change_kind() {
        let cases: Vec<Value> =
            serde_json::from_str::<Value>(include_str!("../fixtures/replay/corpus.json")).unwrap()
                ["cases"]
                .as_array()
                .unwrap()
                .clone();
        assert!(validate_coverage(&cases).is_ok());
        let mut missing = cases.clone();
        missing.pop();
        assert!(validate_coverage(&missing).is_err());
        let mut duplicate = cases.clone();
        duplicate.push(cases[0].clone());
        assert!(validate_coverage(&duplicate).is_err());
        let mut unknown = cases.clone();
        unknown[0]["id"] = json!("other");
        assert!(validate_coverage(&unknown).is_err());
        let mut wrong = cases.clone();
        wrong[0]["kind"] = json!("search_http");
        assert!(validate_coverage(&wrong).is_err());
    }
}

fn validate_numbers(value: &Value) -> Result<()> {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if matches!(
                    key.as_str(),
                    "latency_ms" | "response_time_ms" | "response_time"
                ) && value.as_f64().is_none_or(|v| !v.is_finite() || v < 0.0)
                {
                    return Err(format!("invalid nonnegative numeric field {key}"));
                }
                if matches!(
                    key.as_str(),
                    "position" | "source_count" | "result_count" | "number_of_results"
                ) && !value.is_u64()
                {
                    return Err(format!("invalid integer field {key}"));
                }
                if key == "score" && value.as_f64().is_none_or(|v| !v.is_finite()) {
                    return Err("invalid score".into());
                }
                validate_numbers(value)?;
            }
        }
        Value::Array(array) => {
            for value in array {
                validate_numbers(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn strings(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|v| v.iter().all(Value::is_string))
}
fn artifact(value: &Value, kind: &str) -> Result<()> {
    exact_keys(value, &["contract", "version", "kind", "id"])?;
    if value["contract"] != "slopsearx.artifact_ref"
        || value["version"] != 1
        || value["kind"] != kind
        || value["id"].as_str().is_none_or(str::is_empty)
    {
        return Err("invalid artifact schema".into());
    }
    Ok(())
}
fn validate_surface(kind: &str, body: &Value) -> Result<()> {
    for key in ["answers", "corrections", "infoboxes"] {
        if !body[key].is_array() {
            return Err(format!("invalid surface array {key}"));
        }
    }
    if kind == "search_http" {
        if body["engines"].as_array().is_none_or(|a| {
            a.iter()
                .any(|v| v["engine"].as_str().is_none() || !v["results"].is_u64())
        }) || !strings(&body["suggestions"])
            || !body["unresponsive_engines"].is_array()
        {
            return Err("invalid HTTP arrays".into());
        }
        for status in body["meta"]["engine_status"]
            .as_object()
            .ok_or("missing engine status")?
            .values()
        {
            exact_keys(status, &["results", "latency_ms", "status"])?;
            if !status["results"].is_u64() || !status["status"].is_string() {
                return Err("invalid engine status".into());
            }
        }
    } else {
        exact_keys(
            &body["meta"],
            &[
                "query_id",
                "cached",
                "cached_error",
                "response_time_ms",
                "partial",
                "deadline_exceeded",
                "ranking",
                "cursor",
                "artifact",
                "suggestions",
                "total",
                "has_more",
            ],
        )?;
        for key in [
            "cached",
            "cached_error",
            "partial",
            "deadline_exceeded",
            "has_more",
        ] {
            if !body["meta"][key].is_boolean() {
                return Err(format!("invalid MCP meta boolean {key}"));
            }
        }
        if !body["meta"]["total"].is_u64()
            || !strings(&body["meta"]["suggestions"])
            || !strings(&body["warnings"])
            || !strings(&body["empty_engines"])
        {
            return Err("invalid MCP metadata".into());
        }
        artifact(&body["meta"]["artifact"], "snapshot")?;
        exact_keys(
            &body["scope"],
            &[
                "requested_intent",
                "resolved_categories",
                "selected_engines",
                "routing_reason",
                "excluded_engines",
                "routing",
            ],
        )?;
        for key in [
            "resolved_categories",
            "selected_engines",
            "excluded_engines",
        ] {
            if !strings(&body["scope"][key]) {
                return Err(format!("invalid MCP scope {key}"));
            }
        }
        for outcome in body["engine_outcomes"]
            .as_array()
            .ok_or("invalid outcomes")?
        {
            exact_keys(
                outcome,
                &["engine", "status", "result_count", "latency_ms", "message"],
            )?;
            if !outcome["engine"].is_string()
                || !outcome["status"].is_string()
                || !outcome["result_count"].is_u64()
            {
                return Err("invalid engine outcome".into());
            }
        }
    }
    if !body["meta"]["query_id"].is_string() {
        return Err("invalid query ID".into());
    }
    for result in body["results"].as_array().ok_or("invalid results")? {
        if kind == "search_http" {
            exact_keys(
                result,
                &[
                    "url",
                    "title",
                    "content",
                    "engine",
                    "engines",
                    "score",
                    "positions",
                    "category",
                    "publishedDate",
                    "pubdate",
                    "tier",
                    "length",
                    "thumbnail",
                    "img_src",
                    "media",
                    "payload",
                    "iframe_src",
                    "audio_src",
                    "views",
                    "author",
                    "metadata",
                    "template",
                    "parsed_url",
                    "open_group",
                    "close_group",
                    "priority",
                ],
            )?;
            if !strings(&result["engines"])
                || !strings(&result["parsed_url"])
                || result["positions"]
                    .as_array()
                    .is_none_or(|a| a.iter().any(|v| !v.is_u64()))
                || !result["open_group"].is_boolean()
                || !result["close_group"].is_boolean()
            {
                return Err("invalid HTTP result arrays/boolean".into());
            }
        } else {
            exact_keys(
                result,
                &[
                    "title",
                    "url",
                    "snippet",
                    "source_engines",
                    "source_count",
                    "primary_engine",
                    "category",
                    "published_at",
                    "score",
                    "position",
                    "tier",
                    "citation",
                    "retrieval",
                    "result_id",
                    "artifact",
                ],
            )?;
            if !strings(&result["source_engines"])
                || !result["source_count"].is_u64()
                || !result["position"].is_u64()
                || !result["result_id"].is_string()
            {
                return Err("invalid MCP result provenance".into());
            }
            exact_keys(&result["citation"], &["label", "url"])?;
            exact_keys(
                &result["retrieval"],
                &[
                    "contract",
                    "version",
                    "eligible",
                    "url_status",
                    "url_reason",
                    "scheme",
                ],
            )?;
            if result["retrieval"]["contract"] != "slopsearx.retrieval_handoff"
                || result["retrieval"]["version"] != 1
                || !result["retrieval"]["eligible"].is_boolean()
            {
                return Err("invalid retrieval schema".into());
            }
            artifact(&result["artifact"], "result")?;
        }
        for key in ["url", "title", "category"] {
            if !result[key].is_string() {
                return Err(format!("invalid result text {key}"));
            }
        }
        if !matches!(result["tier"].as_u64(), Some(1 | 2)) {
            return Err("invalid result tier".into());
        }
    }
    Ok(())
}

fn validate_source_coverage(sources: &[Value]) -> Result<()> {
    let required_sources = [
        "tests/fixtures/upstream_contracts/npm.json",
        "tests/fixtures/upstream_contracts/crates.json",
        "tests/fixtures/upstream_contracts/openalex.json",
        "engines/npm.py",
        "engines/crates.py",
        "engines/openalex.py",
        "slopsearx/adapter.py",
        "slopsearx/config.py",
        "slopsearx/payload.py",
        "tests/fixtures/searxng_api_contract.json",
        "tests/test_searxng_api_contract.py",
        "tests/test_mcp_harness.py",
        "slopsearx/mcp/harness.py",
    ];
    let mut paths = std::collections::BTreeSet::new();
    for source in sources {
        let path = source["path"].as_str().ok_or("missing source path")?;
        if !paths.insert(path) {
            return Err("duplicate source provenance".into());
        }
    }
    if required_sources.iter().any(|p| !paths.contains(p)) {
        return Err("missing required source provenance".into());
    }
    Ok(())
}

#[cfg(test)]
mod source_tests {
    use super::*;
    #[test]
    fn required_provenance_cannot_be_removed_or_duplicated() {
        let corpus: Value =
            serde_json::from_str(include_str!("../fixtures/replay/corpus.json")).unwrap();
        let sources = corpus["provenance"]["sources"].as_array().unwrap();
        assert!(validate_source_coverage(sources).is_ok());
        let missing: Vec<_> = sources
            .iter()
            .filter(|s| s["path"] != "engines/npm.py")
            .cloned()
            .collect();
        assert!(validate_source_coverage(&missing).is_err());
        let mut duplicate = sources.clone();
        duplicate.push(sources[0].clone());
        assert!(validate_source_coverage(&duplicate).is_err());
    }
}

#[cfg(unix)]
fn kill_group(pid: u32) {
    let _ = std::process::Command::new("/bin/kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}
fn bounded_read(mut reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|e| format!("candidate pipe read: {e}"))?;
        if count == 0 {
            return Ok(bytes);
        }
        if bytes.len() + count > limit {
            return Err(format!("candidate output exceeds {limit} bytes"));
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
}
fn bounded_candidate(
    command: &mut std::process::Command,
    budget: Duration,
) -> Result<std::process::Output> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("candidate spawn: {e}"))?;
    let pid = child.id();
    let stdout = child.stdout.take().ok_or("missing stdout")?;
    let stderr = child.stderr.take().ok_or("missing stderr")?;
    let (tx, rx) = std::sync::mpsc::channel();
    let tx2 = tx.clone();
    let out_thread = std::thread::spawn(move || {
        let _ = tx.send((true, bounded_read(stdout, 1024 * 1024)));
    });
    let err_thread = std::thread::spawn(move || {
        let _ = tx2.send((false, bounded_read(stderr, 65536)));
    });
    let deadline = Instant::now() + budget;
    let mut status = None;
    let mut out = None;
    let mut err = None;
    let result = (|| -> Result<std::process::Output> {
        loop {
            while let Ok((is_stdout, result)) = rx.try_recv() {
                let bytes = result?;
                if is_stdout {
                    out = Some(bytes);
                } else {
                    err = Some(bytes);
                }
            }
            if status.is_none() {
                status = child
                    .try_wait()
                    .map_err(|e| format!("candidate wait: {e}"))?;
            }
            if let Some(status) = status
                && out.is_some()
                && err.is_some()
            {
                return Ok(std::process::Output {
                    status,
                    stdout: out.take().ok_or("missing stdout")?,
                    stderr: err.take().ok_or("missing stderr")?,
                });
            }
            if Instant::now() >= deadline {
                return Err("candidate process/output drain timeout".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    })();
    // Group cleanup also runs after a direct child exits, releasing descendants' pipe handles.
    #[cfg(unix)]
    kill_group(pid);
    #[cfg(not(unix))]
    let _ = pid;
    let _ = child.kill();
    let _ = child.wait();
    let cleanup = Instant::now() + Duration::from_secs(1);
    while !(out_thread.is_finished() && err_thread.is_finished()) && Instant::now() < cleanup {
        std::thread::sleep(Duration::from_millis(2));
    }
    if !(out_thread.is_finished() && err_thread.is_finished()) {
        return Err("candidate pipes remain open after process cleanup".into());
    }
    out_thread.join().map_err(|_| "stdout reader panic")?;
    err_thread.join().map_err(|_| "stderr reader panic")?;
    result
}

#[cfg(all(test, unix))]
mod process_tests {
    use super::*;
    #[test]
    fn concurrently_drains_and_rejects_oversize_output() {
        let output = bounded_candidate(
            std::process::Command::new("/bin/sh").args([
                "-c",
                "dd if=/dev/zero bs=32768 count=1 >&2 2>/dev/null; printf '{}' ",
            ]),
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(output.stdout, b"{}");
        assert_eq!(output.stderr.len(), 32768);
        assert!(
            bounded_candidate(
                std::process::Command::new("/bin/sh").args([
                    "-c",
                    "printf '{}'; dd if=/dev/zero bs=1048576 count=2 2>/dev/null"
                ]),
                Duration::from_secs(2)
            )
            .unwrap_err()
            .contains("exceeds")
        );
    }
    #[test]
    fn exited_child_with_inherited_pipes_still_has_bounded_deadline() {
        let start = Instant::now();
        let error = bounded_candidate(
            std::process::Command::new("/bin/sh").args(["-c", "sleep 20 & printf '{}' "]),
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert!(error.contains("timeout"));
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}

// Every candidate is contained by an OS policy, including arbitrary non-HTTP clients.
// Linux isolates runner + fixture server together so the private loopback remains usable.
fn isolated_command(program: &Path, args: &[String]) -> Result<std::process::Command> {
    #[cfg(target_os = "macos")]
    {
        let sandbox = Path::new("/usr/bin/sandbox-exec");
        require_isolation_tool(sandbox)?;
        let mut command = std::process::Command::new(sandbox);
        command.args(["-p","(version 1)(allow default)(deny network*)(allow network-outbound (remote ip \"localhost:*\"))(allow network-inbound (local ip \"localhost:*\"))"]).arg(program).args(args);
        Ok(command)
    }
    #[cfg(target_os = "linux")]
    {
        let sandbox = Path::new("/usr/bin/bwrap");
        require_isolation_tool(sandbox)?;
        let parent = std::fs::read_link("/proc/self/ns/net")
            .map_err(|e| format!("cannot identify parent network namespace: {e}"))?;
        let mut command = std::process::Command::new(sandbox);
        command
            .args([
                "--unshare-user",
                "--unshare-net",
                "--unshare-pid",
                "--disable-userns",
                "--cap-drop",
                "ALL",
                "--new-session",
                "--die-with-parent",
                "--ro-bind",
                "/",
                "/",
                "--proc",
                "/proc",
                "--dev",
                "/dev",
                "--",
            ])
            .arg(program)
            .args(args)
            .env("SSX_PARENT_NETNS", parent);
        Ok(command)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (program, args);
        Err("OS-enforced offline replay is unsupported on this platform".into())
    }
}
#[cfg(target_os = "linux")]
fn verify_linux_namespace() -> Result<()> {
    let parent = std::env::var_os("SSX_PARENT_NETNS")
        .ok_or("missing parent namespace identity; candidate not started")?;
    let own = std::fs::read_link("/proc/self/ns/net").map_err(|e| e.to_string())?;
    if own.as_os_str() == parent {
        return Err("network namespace was not isolated; candidate not started".into());
    }
    let devices = std::fs::read_to_string("/proc/net/dev").map_err(|e| e.to_string())?;
    let names: Vec<_> = devices
        .lines()
        .skip(2)
        .filter_map(|line| line.split_once(':').map(|(name, _)| name.trim()))
        .collect();
    if names != ["lo"] {
        return Err("isolated namespace must contain only loopback; candidate not started".into());
    }
    let status = std::fs::read_to_string("/proc/self/status").map_err(|e| e.to_string())?;
    if status
        .lines()
        .find_map(|line| line.strip_prefix("CapEff:"))
        .is_none_or(|v| v.trim() != "0000000000000000")
    {
        return Err("candidate namespace retains effective capabilities".into());
    }
    Ok(())
}
fn network_probe() -> Result<()> {
    let exchange = Exchange {
        request: Request {
            method: "GET".into(),
            target: "/isolation-probe".into(),
            headers: BTreeMap::new(),
            body: String::new(),
        },
        response: Response {
            status: 200,
            headers: BTreeMap::new(),
            body: "offline-loopback".into(),
        },
    };
    replay(std::slice::from_ref(&exchange), |address| {
        if send(address, &exchange.request)? != exchange.response {
            return Err("isolated loopback replay differs".into());
        }
        Ok(())
    })?;
    println!("{}", non_loopback_denied()?);
    Ok(())
}
fn non_loopback_denied() -> Result<Value> {
    // RFC 5737 documentation-only address; callers establish OS isolation first.
    let target = "203.0.113.1:9"
        .parse()
        .map_err(|_| "invalid probe target")?;
    let error = TcpStream::connect_timeout(&target, Duration::from_millis(300))
        .err()
        .ok_or("non-loopback socket unexpectedly connected")?;
    let denied = match std::env::consts::OS {
        "macos" => matches!(error.raw_os_error(), Some(1 | 13)),
        "linux" => matches!(error.raw_os_error(), Some(1 | 13 | 101)),
        _ => false,
    };
    if !denied {
        return Err(format!("OS did not prove non-loopback denial: {error}"));
    }
    Ok(
        json!({"isolation":"os-enforced","loopback":"passed","non_loopback":"denied","target":"203.0.113.1:9","os_error":error.raw_os_error()}),
    )
}
fn verify_os_isolation(records: &mut Vec<Value>) -> Result<()> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = isolated_command(&executable, &["replay-network-probe".into()])?;
    let output = bounded_candidate(&mut command, Duration::from_secs(4))?;
    records.push(json!({"program":"os-network-isolation","args":["loopback-positive","documentation-ip-denial"],"exit_code":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)}));
    if !output.status.success() {
        return Err(format!(
            "mandatory OS network isolation unavailable/rejected; candidate not started: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

// Explicit plumbing canary: sends fixture requests and echoes the pinned observation.
// It proves runner containment and transcript enforcement, never adapter transformation parity.
fn candidate_isolation_canary(args: &[String]) -> Result<()> {
    if args.len() != 3 {
        return Err("isolation canary requires corpus and case".into());
    }
    non_loopback_denied()?;
    let corpus = load_json(Path::new(&args[1]))?;
    let case = corpus["cases"]
        .as_array()
        .ok_or("missing cases")?
        .iter()
        .find(|c| c["id"] == args[2])
        .ok_or("unknown canary case")?;
    let address = std::env::var("SSX_REPLAY_URL")
        .map_err(|e| e.to_string())?
        .strip_prefix("http://")
        .ok_or("invalid loopback fixture URL")?
        .parse::<std::net::SocketAddr>()
        .map_err(|e| e.to_string())?;
    if !address.ip().is_loopback() {
        return Err("canary URL must be loopback".into());
    }
    let exchanges: Vec<Exchange> =
        serde_json::from_value(case["exchanges"].clone()).map_err(|e| e.to_string())?;
    for exchange in exchanges {
        if send(address, &exchange.request)? != exchange.response {
            return Err("isolated candidate transcript differs".into());
        }
    }
    println!("{}", case["expected"]);
    Ok(())
}
fn network_checks(records: &mut Vec<Value>) -> Result<()> {
    verify_os_isolation(records)?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/replay/corpus.json");
    let args = vec![
        "replay-run".into(),
        corpus.to_string_lossy().into_owned(),
        "upstream-npm-captured".into(),
        "--".into(),
        executable.to_string_lossy().into_owned(),
        "replay-isolation-candidate".into(),
        corpus.to_string_lossy().into_owned(),
        "upstream-npm-captured".into(),
    ];
    run_backend(&args)?;
    for mode in ["replay-sleep-child", "replay-held-pipes-candidate"] {
        let mut timeout_args = args[..5].to_vec();
        timeout_args.push(mode.into());
        let started = Instant::now();
        let error = run_backend(&timeout_args)
            .err()
            .ok_or("isolated timeout candidate unexpectedly passed")?;
        if !error.contains("timeout") || started.elapsed() > Duration::from_secs(11) {
            return Err(format!(
                "isolated candidate cleanup did not meet deadline: {error}"
            ));
        }
        records.push(json!({"program":"isolated-candidate-deadline","args":[mode],"exit_code":1,"stdout":"expected timeout and bounded process/pipe cleanup","stderr":error}));
    }
    records.push(json!({"program":"isolated-replay-candidate-canary","args":["upstream-npm-captured","documentation-ip-denial","loopback-transcript"],"exit_code":0,"stdout":"runner containment canary passed; observation echoed deliberately, not adapter parity","stderr":""}));
    Ok(())
}

fn require_isolation_tool(path: &Path) -> Result<()> {
    if !path.is_absolute() || !path.is_file() {
        return Err(format!(
            "mandatory OS network isolation executable {} unavailable; candidate not started",
            path.display()
        ));
    }
    Ok(())
}
#[cfg(test)]
mod isolation_tests {
    use super::*;
    #[test]
    fn unavailable_isolation_fails_before_candidate() {
        assert!(
            require_isolation_tool(Path::new("/nonexistent/ssx-isolation-executable"))
                .unwrap_err()
                .contains("candidate not started")
        );
        assert!(require_isolation_tool(Path::new("sandbox-exec")).is_err());
    }
}
