use crate::Result;
use serde_json::Value;
use std::{collections::BTreeMap, io, net::IpAddr, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    time::Instant,
};
use url::Url;

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8080";
const DEFAULT_QUERY: &str = "portal-smoke";
const DEFAULT_THEME: &str = "dark";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_REDIRECTS: usize = 10;
const USAGE: &str = "usage: cargo xtask ci-helper portal-release-smoke [--base-url URL] [--query TEXT] [--expected-theme dark|darker]";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    base_url: String,
    query: String,
    expected_theme: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_owned(),
            query: DEFAULT_QUERY.to_owned(),
            expected_theme: DEFAULT_THEME.to_owned(),
        }
    }
}

fn parse_args(args: &[String]) -> Result<Options> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        let (name, inline_value) = argument
            .split_once('=')
            .map_or((argument, None), |(name, value)| (name, Some(value)));
        if !matches!(name, "--base-url" | "--query" | "--expected-theme") {
            return Err(format!("{USAGE}; unsupported argument {name}"));
        }
        let (value, consumed) = if let Some(value) = inline_value {
            (value, 1)
        } else {
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("{USAGE}; missing value for {name}"))?;
            if value.starts_with("--") {
                return Err(format!("{USAGE}; missing value for {name}"));
            }
            (value.as_str(), 2)
        };
        match name {
            "--base-url" => options.base_url = value.to_owned(),
            "--query" => options.query = value.to_owned(),
            "--expected-theme" if value == "dark" || value == "darker" => {
                options.expected_theme = value.to_owned();
            }
            "--expected-theme" => {
                return Err(format!("{USAGE}; expected theme must be dark or darker"));
            }
            _ => unreachable!("argument names were checked above"),
        }
        index += consumed;
    }
    Ok(options)
}

fn endpoint(base_url: &str, suffix: &str) -> Result<Url> {
    let trimmed = base_url.trim_end_matches('/');
    let url = Url::parse(&format!("{trimmed}{suffix}"))
        .map_err(|_| "portal smoke base URL is invalid".to_owned())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("portal smoke base URL must use HTTP or HTTPS".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("portal smoke base URL must not contain credentials".into());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("portal smoke base URL must not contain a query or fragment".into());
    }
    Ok(url)
}

#[derive(Debug)]
struct Probe {
    status: u16,
    headers: BTreeMap<String, String>,
    body: String,
}

#[derive(Debug)]
struct Once {
    probe: Probe,
    redirect: Option<Url>,
}

#[derive(Debug)]
enum ContentLengthError {
    Invalid,
    TooLarge,
}

#[derive(Debug)]
enum BodyReadError {
    Read,
    TooLarge,
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn redirect_target(current: &Url, location: &str) -> Result<Url> {
    let mut target = current
        .join(location)
        .map_err(|_| "portal smoke redirect target is invalid".to_owned())?;
    if !matches!(target.scheme(), "http" | "https") {
        return Err("portal smoke redirect must use HTTP or HTTPS".into());
    }
    if !target.username().is_empty() || target.password().is_some() {
        return Err("portal smoke redirect must not contain credentials".into());
    }
    target.set_fragment(None);
    Ok(target)
}

fn is_loopback_host(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    host.eq_ignore_ascii_case("localhost")
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn config_quote(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

async fn read_line_bounded<R>(reader: &mut R, total: &mut usize) -> io::Result<Option<Vec<u8>>>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = Vec::new();
    loop {
        let part = {
            let available = reader.fill_buf().await?;
            if available.is_empty() {
                if line.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(line));
            }
            let length = available
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(available.len(), |index| index + 1);
            available[..length].to_vec()
        };
        *total = total
            .checked_add(part.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "header size overflow"))?;
        if *total > MAX_HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "response headers exceed bound",
            ));
        }
        let ended_with_newline = part.last() == Some(&b'\n');
        line.extend_from_slice(&part);
        reader.consume(part.len());
        if ended_with_newline {
            return Ok(Some(line));
        }
    }
}

async fn read_response_head<R>(reader: &mut R) -> Result<(u16, BTreeMap<String, String>)>
where
    R: AsyncBufRead + Unpin,
{
    let mut total = 0;
    loop {
        let status_line = read_line_bounded(reader, &mut total)
            .await
            .map_err(|_| "portal smoke response headers are invalid or too large".to_owned())?
            .ok_or_else(|| "portal smoke response has no status line".to_owned())?;
        let status_line = trim_header_line(&status_line);
        let mut fields = status_line
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|field| !field.is_empty());
        let protocol = fields.next().unwrap_or_default();
        if !protocol.starts_with(b"HTTP/") {
            return Err("portal smoke response status is invalid".into());
        }
        let status = fields
            .next()
            .and_then(|field| std::str::from_utf8(field).ok())
            .and_then(|field| field.parse::<u16>().ok())
            .filter(|status| (100..=599).contains(status))
            .ok_or_else(|| "portal smoke response status is invalid".to_owned())?;
        let mut headers = BTreeMap::<String, String>::new();
        loop {
            let line = read_line_bounded(reader, &mut total)
                .await
                .map_err(|_| "portal smoke response headers are invalid or too large".to_owned())?
                .ok_or_else(|| "portal smoke response headers are incomplete".to_owned())?;
            let line = trim_header_line(&line);
            if line.is_empty() {
                break;
            }
            if line
                .first()
                .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
            {
                return Err("portal smoke response headers are invalid".into());
            }
            let separator = line
                .iter()
                .position(|byte| *byte == b':')
                .ok_or_else(|| "portal smoke response headers are invalid".to_owned())?;
            let name = std::str::from_utf8(&line[..separator])
                .map_err(|_| "portal smoke response headers are invalid".to_owned())?;
            if name.is_empty()
                || !name.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_\x60|~".contains(&byte)
                })
            {
                return Err("portal smoke response headers are invalid".into());
            }
            let name = name.to_ascii_lowercase();
            let value =
                String::from_utf8_lossy(trim_header_value(&line[separator + 1..])).into_owned();
            if let Some(previous) = headers.get_mut(&name) {
                if name == "content-length" || name == "location" {
                    if previous != &value {
                        return Err("portal smoke response has ambiguous headers".into());
                    }
                } else {
                    previous.push_str(", ");
                    previous.push_str(&value);
                }
            } else {
                headers.insert(name, value);
            }
        }
        if (100..200).contains(&status) && status != 101 {
            continue;
        }
        return Ok((status, headers));
    }
}

fn trim_header_line(mut line: &[u8]) -> &[u8] {
    if let Some(without_lf) = line.strip_suffix(b"\n") {
        line = without_lf;
    }
    if let Some(without_cr) = line.strip_suffix(b"\r") {
        line = without_cr;
    }
    line
}

fn trim_header_value(mut value: &[u8]) -> &[u8] {
    while value
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[1..];
    }
    while value
        .last()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[..value.len() - 1];
    }
    value
}

fn content_length(
    headers: &BTreeMap<String, String>,
) -> std::result::Result<Option<u64>, ContentLengthError> {
    let Some(value) = headers.get("content-length") else {
        return Ok(None);
    };
    let length = value
        .parse::<u64>()
        .map_err(|_| ContentLengthError::Invalid)?;
    if length > MAX_RESPONSE_BYTES as u64 {
        return Err(ContentLengthError::TooLarge);
    }
    Ok(Some(length))
}

async fn read_body<R>(reader: &mut R) -> std::result::Result<Vec<u8>, BodyReadError>
where
    R: AsyncRead + Unpin,
{
    let mut body = Vec::with_capacity(MAX_RESPONSE_BYTES.min(16 * 1024));
    let mut buffer = [0_u8; 8192];
    loop {
        let remaining = MAX_RESPONSE_BYTES + 1 - body.len();
        let limit = remaining.min(buffer.len());
        let count = reader
            .read(&mut buffer[..limit])
            .await
            .map_err(|_| BodyReadError::Read)?;
        if count == 0 {
            return Ok(body);
        }
        if body.len().saturating_add(count) > MAX_RESPONSE_BYTES {
            return Err(BodyReadError::TooLarge);
        }
        body.extend_from_slice(&buffer[..count]);
    }
}

async fn stop_child(child: &mut Child) -> (bool, Option<std::process::ExitStatus>) {
    if let Ok(Some(exit)) = child.try_wait() {
        return (false, Some(exit));
    }
    let killed = child.kill().await.is_ok();
    let exit = child.wait().await.ok();
    (killed, exit)
}

fn transport_error(label: &str, status: std::process::ExitStatus) -> String {
    let reason = match status.code() {
        Some(6 | 7) => "could not connect",
        Some(28) => "timed out",
        Some(63) => "response exceeded size limit",
        _ => "transport failed",
    };
    format!("portal smoke {label} request {reason}")
}

fn curl_config(url: &Url, accept: &str) -> String {
    format!(
        "url = {}\nheader = {}\n{}",
        config_quote(url.as_str()),
        config_quote(&format!("Accept: {accept}")),
        if is_loopback_host(url) {
            "noproxy = \"*\"\n"
        } else {
            ""
        }
    )
}

async fn request_once(
    url: Url,
    accept: &'static str,
    label: &str,
    remaining: Duration,
) -> Result<Once> {
    if remaining.is_zero() {
        return Err(format!("portal smoke {label} request timed out"));
    }
    let max_time = format!("{:.6}", remaining.as_secs_f64().max(0.001));
    let mut command = Command::new("curl");
    command
        .arg("--disable")
        .args([
            "--silent",
            "--show-error",
            "--globoff",
            "--suppress-connect-headers",
            "--max-time",
            &max_time,
            "--max-filesize",
            "1048576",
            "--proto",
            "=http,https",
            "--dump-header",
            "-",
            "--output",
            "-",
            "--config",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|_| format!("portal smoke {label} request could not start curl"))?;
    let config = curl_config(&url, accept);
    let write_result = match child.stdin.take() {
        Some(mut stdin) => stdin.write_all(config.as_bytes()).await,
        None => Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "curl configuration pipe unavailable",
        )),
    };
    if write_result.is_err() {
        let _ = stop_child(&mut child).await;
        return Err(format!("portal smoke {label} request could not start curl"));
    }

    let Some(stdout) = child.stdout.take() else {
        let _ = stop_child(&mut child).await;
        return Err(format!("portal smoke {label} request failed"));
    };
    let mut reader = BufReader::new(stdout);
    let (status, headers) = match read_response_head(&mut reader).await {
        Ok(head) => head,
        Err(_) => {
            // EOF can arrive just before Tokio observes curl's exit status.
            // Give a completed transport failure a short chance to be reaped
            // before terminating curl for a malformed or incomplete response.
            match tokio::time::timeout(Duration::from_millis(100).min(remaining), child.wait())
                .await
            {
                Ok(Ok(exit)) if !exit.success() => {
                    return Err(transport_error(label, exit));
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) | Err(_) => {
                    let _ = stop_child(&mut child).await;
                }
            }
            return Err(format!(
                "portal smoke {label} response headers are invalid or too large"
            ));
        }
    };

    let redirect = if is_redirect(status) {
        match headers.get("location") {
            Some(location) => match redirect_target(&url, location) {
                Ok(target) => Some(target),
                Err(error) => {
                    let _ = stop_child(&mut child).await;
                    return Err(error);
                }
            },
            None => None,
        }
    } else {
        None
    };
    if redirect.is_some() {
        let _ = stop_child(&mut child).await;
        return Ok(Once {
            probe: Probe {
                status,
                headers,
                body: String::new(),
            },
            redirect,
        });
    }

    match content_length(&headers) {
        Ok(_) => {}
        Err(ContentLengthError::TooLarge) => {
            let _ = stop_child(&mut child).await;
            return Err(format!(
                "portal smoke {label} response exceeded {MAX_RESPONSE_BYTES} bytes"
            ));
        }
        Err(ContentLengthError::Invalid) => {
            let _ = stop_child(&mut child).await;
            return Err(format!("portal smoke {label} response length is invalid"));
        }
    }

    let body = match read_body(&mut reader).await {
        Ok(body) => body,
        Err(BodyReadError::TooLarge) => {
            let _ = stop_child(&mut child).await;
            return Err(format!(
                "portal smoke {label} response exceeded {MAX_RESPONSE_BYTES} bytes"
            ));
        }
        Err(BodyReadError::Read) => {
            let _ = stop_child(&mut child).await;
            return Err(format!("portal smoke {label} response read failed"));
        }
    };
    let exit = child
        .wait()
        .await
        .map_err(|_| format!("portal smoke {label} request failed"))?;
    if !exit.success() {
        return Err(transport_error(label, exit));
    }
    Ok(Once {
        probe: Probe {
            status,
            headers,
            body: String::from_utf8_lossy(&body).into_owned(),
        },
        redirect: None,
    })
}

async fn get(url: Url, accept: &'static str, label: &'static str) -> Result<Probe> {
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let mut current = url;
    for redirects in 0..=MAX_REDIRECTS {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let response = request_once(current.clone(), accept, label, remaining).await?;
        if let Some(target) = response.redirect {
            if redirects == MAX_REDIRECTS {
                return Err(format!("portal smoke {label} exceeded redirect limit"));
            }
            current = target;
            continue;
        }
        return Ok(response.probe);
    }
    unreachable!("the redirect loop always returns or errors")
}

fn validate_landing(probe: &Probe, expected_theme: &str) -> Result<()> {
    if probe.status != 200 {
        return Err(format!(
            "portal smoke landing returned HTTP {}",
            probe.status
        ));
    }
    if !probe.body.contains("<form class=\"search-form\"") {
        return Err("portal smoke landing page has no search form".into());
    }
    if !probe
        .body
        .contains(&format!("data-default-theme=\"{expected_theme}\""))
    {
        return Err("portal smoke landing page has an unexpected default theme".into());
    }
    if !probe.headers.contains_key("content-security-policy") {
        return Err("portal smoke landing page is missing CSP".into());
    }
    Ok(())
}

fn validate_healthz(probe: &Probe) -> Result<()> {
    if probe.status != 200 || probe.body.trim() != "OK" {
        return Err("portal smoke healthz did not return HTTP 200 and OK".into());
    }
    Ok(())
}

fn validate_search(probe: &Probe, query: &str) -> Result<()> {
    if !matches!(probe.status, 200 | 503) {
        return Err(format!(
            "portal smoke search returned HTTP {}",
            probe.status
        ));
    }
    let payload: Value = serde_json::from_str(&probe.body)
        .map_err(|_| "portal smoke search response was not JSON".to_owned())?;
    if payload.get("query").and_then(Value::as_str) != Some(query) {
        return Err("portal smoke search response did not preserve the query".into());
    }
    if !payload.get("results").is_some_and(Value::is_array) {
        return Err("portal smoke search response has no results list".into());
    }
    Ok(())
}

async fn run(options: &Options) -> Result<(u16, String)> {
    let landing = get(endpoint(&options.base_url, "/")?, "text/html", "landing").await?;
    validate_landing(&landing, &options.expected_theme)?;

    let healthz = get(
        endpoint(&options.base_url, "/healthz")?,
        "text/plain",
        "healthz",
    )
    .await?;
    validate_healthz(&healthz)?;

    let mut search_url = endpoint(&options.base_url, "/search")?;
    search_url
        .query_pairs_mut()
        .append_pair("q", &options.query)
        .append_pair("format", "json")
        .append_pair("categories", "__portal_smoke__")
        .append_pair("interactive_timeout_ms", "1000");
    let search = get(search_url, "application/json", "search").await?;
    validate_search(&search, &options.query)?;
    Ok((search.status, options.expected_theme.clone()))
}

pub fn cli(args: &[String]) -> Result<()> {
    if matches!(args, [help] if help == "--help" || help == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_args(args)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "portal smoke async runtime could not be created".to_owned())?;
    let (search_status, theme) = runtime.block_on(run(&options))?;
    println!("portal smoke passed: landing=200 healthz=200 search={search_status} theme={theme}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    fn probe(status: u16, headers: &[(&str, &str)], body: &str) -> Probe {
        Probe {
            status,
            headers: headers
                .iter()
                .map(|(name, value)| (name.to_ascii_lowercase(), (*value).to_owned()))
                .collect(),
            body: body.to_owned(),
        }
    }

    #[test]
    fn defaults_and_argument_validation_match_the_operator_contract() {
        assert_eq!(parse_args(&[]).unwrap(), Options::default());
        assert_eq!(
            parse_args(&[
                "--base-url".into(),
                "http://localhost:8080/".into(),
                "--query".into(),
                "unicode λ & spaces".into(),
                "--expected-theme".into(),
                "darker".into(),
            ])
            .unwrap(),
            Options {
                base_url: "http://localhost:8080/".into(),
                query: "unicode λ & spaces".into(),
                expected_theme: "darker".into(),
            }
        );
        assert!(parse_args(&["--unknown".into(), "x".into()]).is_err());
        assert!(parse_args(&["--query".into()]).is_err());
        assert!(parse_args(&["--expected-theme".into(), "light".into()]).is_err());
        assert_eq!(
            parse_args(&[
                "--base-url=http://localhost:8080/".into(),
                "--query=first".into(),
                "--query".into(),
                "unicode λ & spaces".into(),
                "--expected-theme=darker".into(),
            ])
            .unwrap(),
            Options {
                base_url: "http://localhost:8080/".into(),
                query: "unicode λ & spaces".into(),
                expected_theme: "darker".into(),
            }
        );
    }

    #[test]
    fn endpoints_reject_credentials_and_non_http_schemes_without_echoing_urls() {
        assert!(endpoint("file:///etc", "/").is_err());
        assert!(endpoint("http://user:secret@localhost:8080", "/").is_err());
        assert!(endpoint("http://localhost:8080/?token=secret", "/").is_err());
    }

    #[test]
    fn redirects_and_curl_configuration_preserve_safe_transport_rules() {
        let local = Url::parse("http://[::1]:8080/search?q=unicode%20%CE%BB").unwrap();
        assert!(is_loopback_host(&local));
        assert!(curl_config(&local, "application/json").contains("noproxy = \"*\""));
        assert!(
            !curl_config(
                &Url::parse("https://search.example.test/healthz").unwrap(),
                "text/plain"
            )
            .contains("noproxy")
        );
        assert_eq!(
            redirect_target(&local, "/healthz").unwrap().path(),
            "/healthz"
        );
        assert!(redirect_target(&local, "ftp://example.test/file").is_err());
        assert!(redirect_target(&local, "http://user:secret@example.test/").is_err());
    }

    #[tokio::test]
    async fn response_headers_accept_opaque_values_but_keep_ascii_names() {
        let (mut sender, receiver) = tokio::io::duplex(256);
        let mut response = b"HTTP/1.1 200 ".to_vec();
        response.push(0xff);
        response.extend_from_slice(b"\r\nX-opaque: ");
        response.push(0xff);
        response.extend_from_slice(b"\r\nContent-Length: 0\r\n\r\n");
        sender.write_all(&response).await.unwrap();
        sender.shutdown().await.unwrap();
        let mut reader = BufReader::new(receiver);
        let (status, headers) = read_response_head(&mut reader).await.unwrap();
        assert_eq!(status, 200);
        assert_eq!(headers.get("x-opaque").map(String::as_str), Some("�"));
        assert_eq!(headers.get("content-length").map(String::as_str), Some("0"));
    }

    #[test]
    fn landing_requires_status_form_theme_and_csp() {
        let good = probe(
            200,
            &[("content-security-policy", "default-src 'self'")],
            "<form class=\"search-form\" data-default-theme=\"dark\">",
        );
        assert!(validate_landing(&good, "dark").is_ok());
        assert!(validate_landing(&probe(503, &[], ""), "dark").is_err());
        assert!(
            validate_landing(
                &probe(
                    200,
                    &[],
                    "<form class=\"search-form\" data-default-theme=\"dark\">"
                ),
                "dark"
            )
            .is_err()
        );
        assert!(
            validate_landing(
                &probe(
                    200,
                    &[("content-security-policy", "x")],
                    "<form class=\"search-form\" data-default-theme=\"light\">"
                ),
                "dark"
            )
            .is_err()
        );
    }

    #[test]
    fn healthz_requires_http_200_and_trimmed_ok() {
        assert!(validate_healthz(&probe(200, &[], " OK\n")).is_ok());
        assert!(validate_healthz(&probe(503, &[], "OK")).is_err());
        assert!(validate_healthz(&probe(200, &[], "healthy")).is_err());
    }

    #[test]
    fn search_accepts_503_but_rejects_invalid_or_untruthful_envelopes() {
        for status in [200, 503] {
            let body = json!({"query":"unicode λ","results":[]}).to_string();
            assert!(validate_search(&probe(status, &[], &body), "unicode λ").is_ok());
        }
        assert!(validate_search(&probe(500, &[], "{}"), "q").is_err());
        assert!(validate_search(&probe(200, &[], "not-json"), "q").is_err());
        assert!(
            validate_search(&probe(200, &[], r#"{"query":"other","results":[]}"#), "q").is_err()
        );
        assert!(validate_search(&probe(200, &[], r#"{"query":"q","results":null}"#), "q").is_err());
    }

    async fn fixture(
        theme: &str,
        search_status: u16,
        requests: usize,
    ) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let theme = theme.to_owned();
        let server = tokio::spawn(async move {
            let mut captured = Vec::new();
            for _ in 0..requests {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut chunk = [0_u8; 1024];
                loop {
                    let count = stream.read(&mut chunk).await.unwrap();
                    if count == 0 {
                        break;
                    }
                    request.extend_from_slice(&chunk[..count]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                    assert!(
                        request.len() <= 16 * 1024,
                        "fixture request headers bounded"
                    );
                }
                let request = String::from_utf8(request).unwrap();
                let first_line = request.lines().next().unwrap_or_default();
                let path = first_line.split_whitespace().nth(1).unwrap_or_default();
                let (status, headers, body) = if path == "/redirect" {
                    (302, "Location: /\r\n", String::new())
                } else if path == "/" {
                    (
                        200,
                        "Content-Type: text/html\r\nContent-Security-Policy: default-src 'self'\r\n",
                        format!("<form class=\"search-form\" data-default-theme=\"{theme}\">"),
                    )
                } else if path == "/healthz" {
                    (200, "Content-Type: text/plain\r\n", "OK\n".to_owned())
                } else if path.starts_with("/search?") {
                    let url = Url::parse(&format!("http://localhost{path}")).unwrap();
                    let query = url
                        .query_pairs()
                        .find(|(key, _)| key == "q")
                        .map(|(_, value)| value.into_owned())
                        .unwrap_or_default();
                    (
                        search_status,
                        "Content-Type: application/json\r\n",
                        json!({"query":query,"results":[]}).to_string(),
                    )
                } else {
                    (404, "Content-Type: text/plain\r\n", "missing".to_owned())
                };
                let reason = match status {
                    200 => "OK",
                    302 => "Found",
                    503 => "Service Unavailable",
                    _ => "Not Found",
                };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
                stream.shutdown().await.unwrap();
                captured.push(request);
            }
            captured
        });
        (format!("http://{address}"), server)
    }

    #[tokio::test]
    async fn loopback_smoke_preserves_encoded_query_and_accepts_unavailable_search() {
        let query = "unicode λ & spaces";
        let (base_url, server) = fixture("darker", 503, 3).await;
        let options = Options {
            base_url,
            query: query.to_owned(),
            expected_theme: "darker".to_owned(),
        };
        assert_eq!(run(&options).await.unwrap(), (503, "darker".to_owned()));
        let requests = server.await.unwrap();
        assert!(
            requests[0]
                .to_ascii_lowercase()
                .contains("accept: text/html")
        );
        assert!(
            requests[1]
                .to_ascii_lowercase()
                .contains("accept: text/plain")
        );
        assert!(
            requests[2]
                .to_ascii_lowercase()
                .contains("accept: application/json")
        );
        assert!(requests[2].contains("categories=__portal_smoke__"));
        assert!(requests[2].contains("interactive_timeout_ms=1000"));
        let url = Url::parse(&format!(
            "http://localhost{}",
            requests[2]
                .lines()
                .next()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap()
        ))
        .unwrap();
        assert_eq!(
            url.query_pairs().find(|(key, _)| key == "q").unwrap().1,
            query
        );
    }

    #[tokio::test]
    async fn get_follows_bounded_relative_redirects() {
        let (base_url, server) = fixture("dark", 200, 2).await;
        let landing = get(
            endpoint(&base_url, "/redirect").unwrap(),
            "text/html",
            "landing",
        )
        .await
        .unwrap();
        assert!(validate_landing(&landing, "dark").is_ok());
        let requests = server.await.unwrap();
        assert!(requests[0].starts_with("GET /redirect "));
        assert!(requests[1].starts_with("GET / "));
    }

    #[tokio::test]
    async fn connection_refusal_is_reported_as_a_sanitized_transport_error() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let url = Url::parse(&format!("http://{address}/private-query")).unwrap();
        let error = get(url, "text/html", "landing").await.unwrap_err();
        assert!(error.contains("could not connect"), "{error}");
        assert!(!error.contains("private-query"), "{error}");
        assert!(!error.contains(&address.to_string()), "{error}");
    }

    #[tokio::test]
    async fn timeout_before_headers_is_reported_as_a_sanitized_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).await.unwrap();
            tokio::time::sleep(Duration::from_millis(300)).await;
            let _ = stream.shutdown().await;
        });
        let url = Url::parse(&format!("http://{address}/private-query")).unwrap();
        let error = request_once(url, "text/html", "landing", Duration::from_millis(100))
            .await
            .unwrap_err();
        assert!(error.contains("timed out"), "{error}");
        assert!(!error.contains("private-query"), "{error}");
        server.await.unwrap();
    }

    async fn oversized_response_fixture(declare_length: bool) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).await.unwrap();
            let body = vec![b'x'; MAX_RESPONSE_BYTES + 1];
            let length = if declare_length {
                format!("Content-Length: {}\r\n", body.len())
            } else {
                String::new()
            };
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n{length}Connection: close\r\n\r\n"
            );
            stream.write_all(head.as_bytes()).await.unwrap();
            let _ = stream.write_all(&body).await;
            let _ = stream.shutdown().await;
        });
        (format!("http://{address}"), server)
    }

    async fn assert_oversized_response_rejected(declare_length: bool) {
        let (base_url, server) = oversized_response_fixture(declare_length).await;
        let options = Options {
            base_url,
            ..Options::default()
        };
        let error = run(&options).await.unwrap_err();
        assert!(error.contains("exceeded"), "{error}");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn declared_oversized_response_is_rejected_before_reading() {
        assert_oversized_response_rejected(true).await;
    }

    #[tokio::test]
    async fn unknown_length_response_is_rejected_before_accumulation_exceeds_bound() {
        assert_oversized_response_rejected(false).await;
    }

    #[test]
    fn docker_workflow_uses_both_native_themes_and_no_python_smoke_invocation() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let workflow = std::fs::read_to_string(root.join(".github/workflows/docker.yml")).unwrap();
        assert_eq!(
            workflow.matches("ci-helper portal-release-smoke").count(),
            2
        );
        assert!(workflow.contains("--expected-theme dark\n"));
        assert!(workflow.contains("--expected-theme darker\n"));
        assert!(!workflow.contains("python3 scripts/portal_release_smoke.py"));
    }
}
