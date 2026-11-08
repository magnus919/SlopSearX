//! O02: native image-pin and Docker base-digest tools.
//!
//! These replace the operational Python helpers that release jobs used to run.
//! The pin parsing/rewriting logic is pure and unit-tested; registry lookups are
//! bounded HTTPS requests performed by `curl` (a portable transport the release
//! runners already provide), with the same allowlisted hosts, accepted manifest
//! media types, three-attempt retry policy and 30-second timeout as before.
//!
//! Evidence boundary: this module reads and rewrites pins and queries registries.
//! It never builds, pushes or promotes an image, and it never touches production
//! state.
use crate::Result;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

/// Digest-pinned application image.
pub const IMAGE: &str = "ghcr.io/magnus919/slopsearx";
/// Deployment artifacts whose combined contents must carry one pinned digest.
pub const PIN_FILES: [&str; 2] = ["docker-compose.yml", "k8s/deployment.yaml"];
pub const TEST_PIN_FILE: &str = "tests/test_deployment_artifacts.py";
pub const DEFAULT_DOCKERFILE: &str = "Dockerfile";
const MAX_ATTEMPTS: u32 = 3;
const TIMEOUT_SECONDS: &str = "30";
const MAX_BODY_BYTES: &str = "1048576";
const RETRYABLE_STATUS: [u16; 5] = [429, 500, 502, 503, 504];
const DOCKER_ACCEPT: &str = "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.docker.distribution.manifest.v2+json";
const GHCR_ACCEPT: &str = "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json";

fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("O02 image pin: {why}"))
    }
}

// ---------------------------------------------------------------------------
// Digest scanning
// ---------------------------------------------------------------------------

fn hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Returns every `IMAGE@sha256:<64 hex>` occurrence in `text`.
fn pinned_references(text: &str) -> Vec<String> {
    let prefix = format!("{IMAGE}@sha256:");
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(index) = rest.find(&prefix) {
        let after = &rest[index + prefix.len()..];
        let candidate = &after[..after.len().min(64)];
        if hex_digest(candidate) {
            let trailing_hex = after[candidate.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_hexdigit());
            if !trailing_hex {
                found.push(format!("sha256:{candidate}"));
                rest = &after[candidate.len()..];
                continue;
            }
        }
        rest = &rest[index + prefix.len()..];
    }
    found
}

/// Collects the unique digest from release deployment manifests.
pub fn extract_pinned_digest(root: &Path) -> Result<String> {
    let mut pins = BTreeSet::new();
    for name in PIN_FILES {
        let path = root.join(name);
        let text = fs::read_to_string(&path)
            .map_err(|_| format!("O02 image pin: missing expected pin file: {name}"))?;
        pins.extend(pinned_references(&text));
    }
    let historical_test_pin = fs::read_to_string(root.join(TEST_PIN_FILE))
        .map_err(|_| format!("O02 image pin: missing expected pin file: {TEST_PIN_FILE}"))?;
    pins.extend(pinned_references(&historical_test_pin));
    need(
        pins.len() == 1,
        &format!(
            "expected exactly one unique pinned digest, found [{}]",
            pins.iter().cloned().collect::<Vec<_>>().join(", ")
        ),
    )?;
    Ok(pins.into_iter().next().expect("one pin"))
}

/// Rewrites every pinned reference to `new_digest`; returns the changed files.
pub fn apply_promotion(root: &Path, new_digest: &str) -> Result<Vec<String>> {
    let replacement = format!("{IMAGE}@{new_digest}");
    let mut changed = Vec::new();
    for name in PIN_FILES {
        let path = root.join(name);
        let text = fs::read_to_string(&path).map_err(|e| format!("O02 image pin: {name}: {e}"))?;
        let mut updated = String::with_capacity(text.len());
        let mut rest = text.as_str();
        let prefix = format!("{IMAGE}@sha256:");
        while let Some(index) = rest.find(&prefix) {
            updated.push_str(&rest[..index]);
            let after = &rest[index + prefix.len()..];
            let candidate = &after[..after.len().min(64)];
            let trailing_hex = after[candidate.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_hexdigit());
            if hex_digest(candidate) && !trailing_hex {
                updated.push_str(&replacement);
                rest = &after[candidate.len()..];
            } else {
                updated.push_str(&prefix);
                rest = after;
            }
        }
        updated.push_str(rest);
        if updated != text {
            fs::write(&path, &updated).map_err(|e| format!("O02 image pin: {name}: {e}"))?;
            changed.push(name.to_owned());
        }
    }
    Ok(changed)
}

// ---------------------------------------------------------------------------
// Dockerfile base parsing
// ---------------------------------------------------------------------------

fn from_tokens(line: &str) -> Option<Vec<&str>> {
    let mut tokens = line.split_whitespace();
    let first = tokens.next()?;
    if !first.eq_ignore_ascii_case("FROM") {
        return None;
    }
    Some(tokens.collect())
}

fn image_parts(token: &str, repository: &str) -> Option<(String, String)> {
    let prefix = format!("{repository}:");
    let rest = token.strip_prefix(&prefix)?;
    let (tag, digest) = rest.split_once("@sha256:")?;
    if tag.is_empty() || tag.contains('@') || !hex_digest(digest) {
        return None;
    }
    Some((tag.to_owned(), format!("sha256:{digest}")))
}

/// Recognized `(repository, tag, digest)` bases in a Dockerfile.
pub fn pinned_bases(dockerfile: &str) -> Result<Vec<(String, String, String)>> {
    let lines: Vec<&str> = dockerfile
        .lines()
        .filter(|line| from_tokens(line).is_some())
        .collect();
    let recognized =
        "Dockerfile must contain exactly one runtime FROM or two recognized FROM stages";
    match lines.len() {
        1 => {
            let tokens = from_tokens(lines[0]).expect("FROM line");
            let runtime = match tokens.len() {
                1 => image_parts(tokens[0], "python"),
                3 if tokens[1].eq_ignore_ascii_case("AS") => image_parts(tokens[0], "python"),
                _ => None,
            };
            let (tag, digest) = runtime.ok_or_else(|| {
                "O02 image pin: Dockerfile must pin python base image by tag and digest".to_owned()
            })?;
            Ok(vec![("library/python".to_owned(), tag, digest)])
        }
        2 => {
            let first = from_tokens(lines[0]).expect("FROM line");
            let second = from_tokens(lines[1]).expect("FROM line");
            let builder = (first.len() == 3
                && first[1].eq_ignore_ascii_case("AS")
                && first[2] == "healthcheck-builder")
                .then(|| image_parts(first[0], "rust"))
                .flatten()
                .filter(|(tag, _)| tag.as_str() == "1.99.0-bookworm");
            let runtime = (second.len() == 1)
                .then(|| image_parts(second[0], "python"))
                .flatten();
            match (builder, runtime) {
                (Some((tag, digest)), Some((runtime_tag, runtime_digest))) => Ok(vec![
                    ("library/rust".to_owned(), tag, digest),
                    ("library/python".to_owned(), runtime_tag, runtime_digest),
                ]),
                _ => Err(format!("O02 image pin: {recognized}")),
            }
        }
        _ => Err(format!("O02 image pin: {recognized}")),
    }
}

#[cfg(test)]
fn pinned_python_base(dockerfile: &str) -> Result<(String, String)> {
    let bases = pinned_bases(dockerfile)?;
    let (_, tag, digest) = bases.last().cloned().expect("at least one base");
    Ok((tag, digest))
}

// ---------------------------------------------------------------------------
// Bounded HTTP transport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

impl HttpResponse {
    fn header(&self, name: &str) -> &str {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
            .unwrap_or_default()
    }
}

pub trait Transport {
    fn get(&self, url: &str, headers: &[(&str, &str)])
    -> std::result::Result<HttpResponse, String>;
}

/// Transport that shells out to `curl` with a bounded timeout and body size.
pub struct CurlTransport;

impl Transport for CurlTransport {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> std::result::Result<HttpResponse, String> {
        need(
            url.starts_with("https://"),
            &format!("only allowlisted HTTPS lookups are permitted: {url}"),
        )?;
        let mut command = Command::new("curl");
        command.args([
            "--silent",
            "--show-error",
            "--max-time",
            TIMEOUT_SECONDS,
            "--max-filesize",
            MAX_BODY_BYTES,
            "--dump-header",
            "-",
            "--output",
            "-",
        ]);
        for (name, value) in headers {
            command.arg("--header").arg(format!("{name}: {value}"));
        }
        command.arg(url);
        let output = command
            .output()
            .map_err(|e| format!("curl unavailable: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "curl exited {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let raw = String::from_utf8_lossy(&output.stdout);
        let (head, body) = raw
            .split_once("\r\n\r\n")
            .or_else(|| raw.split_once("\n\n"))
            .ok_or("curl response carried no header block")?;
        let mut lines = head.lines();
        let status_line = lines.next().ok_or("curl response carried no status line")?;
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<u16>().ok())
            .ok_or_else(|| format!("unrecognized status line: {status_line}"))?;
        let mut parsed = BTreeMap::new();
        for line in lines {
            if let Some((name, value)) = line.split_once(':') {
                parsed.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
            }
        }
        Ok(HttpResponse {
            status,
            headers: parsed,
            body: body.to_owned(),
        })
    }
}

struct LookupError {
    detail: String,
}

fn get_with_retries(
    transport: &dyn Transport,
    url: &str,
    headers: &[(&str, &str)],
    label: &str,
    sleep: &mut dyn FnMut(u64),
) -> std::result::Result<HttpResponse, LookupError> {
    let mut detail = "unknown error".to_owned();
    let mut attempts = 0;
    for attempt in 0..MAX_ATTEMPTS {
        attempts = attempt + 1;
        match transport.get(url, headers) {
            Ok(response) if (200..300).contains(&response.status) => return Ok(response),
            Ok(response) => {
                detail = format!("HTTP {}", response.status);
                if !RETRYABLE_STATUS.contains(&response.status) || attempt + 1 == MAX_ATTEMPTS {
                    break;
                }
            }
            Err(error) => {
                detail = error;
                if attempt + 1 == MAX_ATTEMPTS {
                    break;
                }
            }
        }
        sleep(1u64 << attempt);
    }
    Err(LookupError {
        detail: format!("{label} failed with {detail} after {attempts} attempt(s)"),
    })
}

fn token_from(body: &str) -> std::result::Result<String, LookupError> {
    let payload: Value = serde_json::from_str(body).map_err(|_| LookupError {
        detail: "token response was not JSON".to_owned(),
    })?;
    payload["token"]
        .as_str()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| LookupError {
            detail: "token response did not contain a token".to_owned(),
        })
}

/// Docker Hub manifest digest for `repository:tag`.
pub fn registry_digest(
    transport: &dyn Transport,
    repository: &str,
    tag: &str,
    sleep: &mut dyn FnMut(u64),
) -> Result<String> {
    let scope = format!("repository:{repository}:pull")
        .replace(':', "%3A")
        .replace('/', "%2F");
    let token_url =
        format!("https://auth.docker.io/token?service=registry.docker.io&scope={scope}");
    let response = get_with_retries(
        transport,
        &token_url,
        &[],
        "Docker Hub token request",
        sleep,
    )
    .map_err(|e| format!("O02 image pin: {}", e.detail))?;
    let token = token_from(&response.body).map_err(|e| format!("O02 image pin: {}", e.detail))?;
    let authorization = format!("Bearer {token}");
    let manifest_url = format!("https://registry-1.docker.io/v2/{repository}/manifests/{tag}");
    let response = get_with_retries(
        transport,
        &manifest_url,
        &[("Accept", DOCKER_ACCEPT), ("Authorization", &authorization)],
        &format!("Docker Hub manifest request for {repository}:{tag}"),
        sleep,
    )
    .map_err(|e| format!("O02 image pin: {}", e.detail))?;
    let digest = response.header("docker-content-digest").to_owned();
    need(
        digest.starts_with("sha256:") && hex_digest(&digest["sha256:".len()..]),
        "Docker Hub did not return a valid manifest digest",
    )?;
    Ok(digest)
}

/// `latest` manifest digest for the pinned application image.
pub fn latest_digest(transport: &dyn Transport, sleep: &mut dyn FnMut(u64)) -> Result<String> {
    let repository = IMAGE.strip_prefix("ghcr.io/").expect("ghcr image");
    let token_url = format!("https://ghcr.io/token?scope=repository:{repository}:pull");
    let response = get_with_retries(transport, &token_url, &[], "token request", sleep)
        .map_err(|e| format!("O02 image pin: {}", e.detail))?;
    let token = token_from(&response.body).map_err(|e| format!("O02 image pin: {}", e.detail))?;
    let authorization = format!("Bearer {token}");
    let manifest_url = format!("https://ghcr.io/v2/{repository}/manifests/latest");
    let response = get_with_retries(
        transport,
        &manifest_url,
        &[("Accept", GHCR_ACCEPT), ("Authorization", &authorization)],
        "manifest request",
        sleep,
    )
    .map_err(|e| format!("O02 image pin: {}", e.detail))?;
    let digest = response.header("docker-content-digest").to_owned();
    need(
        digest.starts_with("sha256:") && hex_digest(&digest["sha256:".len()..]),
        &format!("registry returned no usable digest: {digest:?}"),
    )?;
    Ok(digest)
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

fn real_sleep(seconds: u64) {
    std::thread::sleep(std::time::Duration::from_secs(seconds));
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn verify_base_with(
    root: &Path,
    args: &[String],
    transport: &dyn Transport,
    sleep: &mut dyn FnMut(u64),
) -> Result<i32> {
    let dockerfile =
        flag_value(args, "--dockerfile").unwrap_or_else(|| DEFAULT_DOCKERFILE.to_owned());
    let path = if Path::new(&dockerfile).is_absolute() {
        Path::new(&dockerfile).to_path_buf()
    } else {
        root.join(&dockerfile)
    };
    let text = fs::read_to_string(&path)
        .map_err(|e| format!("O02 image pin: unable to read Dockerfile: {e}"))?;
    let bases = pinned_bases(&text)?;
    let mut observations = Vec::new();
    for (repository, tag, expected) in bases {
        let actual = registry_digest(transport, &repository, &tag, sleep)?;
        println!("{repository}:{tag} expected={expected} registry={actual}");
        observations.push((expected, actual));
    }
    if observations
        .iter()
        .any(|(expected, actual)| expected != actual)
    {
        println!("Pinned Docker base digest is stale; update tag and digest together.");
        return Ok(1);
    }
    Ok(0)
}

fn verify_base(root: &Path, args: &[String]) -> Result<i32> {
    let mut sleep = real_sleep;
    verify_base_with(root, args, &CurlTransport, &mut sleep)
}

fn promote(root: &Path, args: &[String]) -> Result<i32> {
    let check = args.iter().any(|arg| arg == "--check");
    let pinned = extract_pinned_digest(root)?;
    let transport = CurlTransport;
    let mut sleep = real_sleep;
    let latest = latest_digest(&transport, &mut sleep)?;
    if pinned == latest {
        println!("{IMAGE} pin is current: {pinned}");
        return Ok(0);
    }
    println!("{IMAGE} drift detected:\n  pinned: {pinned}\n  latest: {latest}");
    if check {
        return Ok(1);
    }
    let changed = apply_promotion(root, &latest)?;
    println!("updated pin in: {}", changed.join(", "));
    Ok(10)
}

/// Dispatches `image-pin-verify-base` and `image-pin-promote`.
pub fn cli(root: &Path, args: &[String]) -> Result<()> {
    let (command, rest) = args
        .split_first()
        .ok_or("usage: cargo xtask ci-helper image-pin-{verify-base|promote}")?;
    let effective = flag_value(rest, "--root")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.to_path_buf());
    let code = match command.as_str() {
        "image-pin-verify-base" => verify_base(&effective, rest)?,
        "image-pin-promote" => match promote(&effective, rest) {
            Ok(code) => code,
            Err(error) => {
                eprintln!("error: {error}");
                2
            }
        },
        other => return Err(format!("O02 image pin: unknown command {other}")),
    };
    if code == 0 {
        Ok(())
    } else {
        std::process::exit(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    struct Fake {
        responses: std::cell::RefCell<
            std::collections::VecDeque<std::result::Result<HttpResponse, String>>,
        >,
        calls: std::cell::RefCell<Vec<String>>,
    }

    impl Fake {
        fn new(responses: Vec<std::result::Result<HttpResponse, String>>) -> Self {
            Self {
                responses: std::cell::RefCell::new(responses.into_iter().collect()),
                calls: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl Transport for Fake {
        fn get(
            &self,
            url: &str,
            _headers: &[(&str, &str)],
        ) -> std::result::Result<HttpResponse, String> {
            self.calls.borrow_mut().push(url.to_owned());
            self.responses
                .borrow_mut()
                .pop_front()
                .expect("scripted response")
        }
    }

    fn response(status: u16, header: Option<(&str, &str)>, body: &str) -> HttpResponse {
        let mut headers = BTreeMap::new();
        if let Some((name, value)) = header {
            headers.insert(name.to_ascii_lowercase(), value.to_owned());
        }
        HttpResponse {
            status,
            headers,
            body: body.to_owned(),
        }
    }

    fn dismiss(_: u64) {}

    #[test]
    fn generic_python_base_is_accepted() {
        let digest = "a".repeat(64);
        let (tag, parsed) =
            pinned_python_base(&format!("FROM python:3.13-slim-bookworm@sha256:{digest}\n"))
                .expect("accepted");
        assert_eq!(tag, "3.13-slim-bookworm");
        assert_eq!(parsed, format!("sha256:{digest}"));
    }

    #[test]
    fn multiple_unrecognized_from_lines_are_rejected() {
        let digest = "a".repeat(64);
        for second in [
            "FROM scratch",
            "FROM --platform=linux/amd64 python:3.12-slim",
            "FROM python:3.12-slim  # runtime stage",
        ] {
            let dockerfile = format!("FROM python:3.12-slim@sha256:{digest}\n{second}\n");
            let error = pinned_python_base(&dockerfile).expect_err("must reject");
            assert!(error.contains("recognized"), "{error}");
        }
    }

    #[test]
    fn health_builder_and_runtime_bases_are_both_verified() {
        let builder = "b".repeat(64);
        let runtime = "c".repeat(64);
        let dockerfile = format!(
            "FROM rust:1.99.0-bookworm@sha256:{builder} AS healthcheck-builder\nFROM python:3.13-slim-bookworm@sha256:{runtime}\n"
        );
        let bases = pinned_bases(&dockerfile).expect("two stages");
        assert_eq!(bases[0].0, "library/rust");
        assert_eq!(bases[0].1, "1.99.0-bookworm");
        assert_eq!(bases[1].0, "library/python");
        assert_eq!(bases[1].2, format!("sha256:{runtime}"));
    }

    fn build_dockerfile(root: &Path) -> std::path::PathBuf {
        let builder = "b".repeat(64);
        let runtime = "c".repeat(64);
        let path = root.join("Dockerfile");
        fs::write(
            &path,
            format!(
                "FROM rust:1.99.0-bookworm@sha256:{builder} AS healthcheck-builder\nFROM python:3.13-slim-bookworm@sha256:{runtime}\n"
            ),
        )
        .expect("fixture Dockerfile");
        path
    }

    #[test]
    fn docker_workflow_runs_native_base_check_before_multiplatform_image_build() {
        let workflow = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/docker.yml"),
        )
        .expect("docker workflow");
        let native = workflow
            .find("cargo xtask ci-helper image-pin-verify-base")
            .expect("native verifier invocation");
        let build = workflow
            .find("name: Build and load image")
            .expect("build step");
        assert!(native < build, "base verifier must run before Docker build");
        assert!(!workflow.contains("python3 scripts/verify_docker_base_digest.py"));
        assert!(workflow.contains("rustup toolchain install \"$toolchain\""));
        assert!(workflow.contains("test \"$toolchain\" = \"1.99.0\""));
    }

    #[test]
    fn verify_base_accepts_matching_injected_registry_digests() {
        let root = fixture_root("verify-base-match");
        let _dockerfile = build_dockerfile(&root);
        let builder = format!("sha256:{}", "b".repeat(64));
        let runtime = format!("sha256:{}", "c".repeat(64));
        let transport = Fake::new(vec![
            Ok(response(200, None, r#"{"token":"registry-token"}"#)),
            Ok(response(200, Some(("Docker-Content-Digest", &builder)), "")),
            Ok(response(200, None, r#"{"token":"registry-token"}"#)),
            Ok(response(200, Some(("Docker-Content-Digest", &runtime)), "")),
        ]);
        let mut sleeps = Vec::new();
        let mut sleep = |seconds| sleeps.push(seconds);
        assert_eq!(
            verify_base_with(
                &root,
                &["--dockerfile".into(), "Dockerfile".into()],
                &transport,
                &mut sleep,
            )
            .expect("matching registry manifests"),
            0
        );
        assert_eq!(transport.calls.borrow().len(), 4);
        assert!(sleeps.is_empty());
    }

    #[test]
    fn verify_base_reports_digest_drift_without_mutating_sources() {
        let root = fixture_root("verify-base-drift");
        let dockerfile = build_dockerfile(&root);
        let original = fs::read(&dockerfile).expect("read fixture Dockerfile");
        let builder = format!("sha256:{}", "d".repeat(64));
        let runtime = format!("sha256:{}", "e".repeat(64));
        let transport = Fake::new(vec![
            Ok(response(200, None, r#"{"token":"registry-token"}"#)),
            Ok(response(200, Some(("Docker-Content-Digest", &builder)), "")),
            Ok(response(200, None, r#"{"token":"registry-token"}"#)),
            Ok(response(200, Some(("Docker-Content-Digest", &runtime)), "")),
        ]);
        let mut sleep = dismiss;
        assert_eq!(
            verify_base_with(&root, &[], &transport, &mut sleep).expect("drift result"),
            1
        );
        assert_eq!(fs::read(dockerfile).expect("source unchanged"), original);
        assert_eq!(transport.calls.borrow().len(), 4);
    }

    #[test]
    fn verify_base_classifies_registry_failure_without_sleeping_or_mutating() {
        let root = fixture_root("verify-base-error");
        let dockerfile = build_dockerfile(&root);
        let original = fs::read(&dockerfile).expect("read fixture Dockerfile");
        let transport = Fake::new(vec![Ok(response(404, None, ""))]);
        let mut sleeps = Vec::new();
        let mut sleep = |seconds| sleeps.push(seconds);
        let error = verify_base_with(&root, &[], &transport, &mut sleep)
            .expect_err("registry failure is an error, not digest drift");
        assert!(error.contains("HTTP 404"), "{error}");
        assert_eq!(transport.calls.borrow().len(), 1);
        assert!(sleeps.is_empty());
        assert_eq!(fs::read(dockerfile).expect("source unchanged"), original);
    }

    #[test]
    fn repository_dockerfile_pins_are_recognized() {
        let dockerfile =
            fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Dockerfile"))
                .expect("Dockerfile");
        let bases = pinned_bases(&dockerfile).expect("recognized bases");
        assert!(!bases.is_empty());
        assert!(
            bases
                .iter()
                .any(|(repository, _, _)| repository == "library/python")
        );
    }

    fn fixture_root(name: &str) -> std::path::PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/o02-image-pin")
            .join(format!(
                "{name}-{}",
                FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&root).expect("fixture root");
        root
    }

    #[test]
    fn unique_pinned_digest_is_extracted() {
        let root = fixture_root("unique");
        let digest = "d".repeat(64);
        for name in PIN_FILES {
            let path = root.join(name);
            fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
            fs::write(&path, format!("image: {IMAGE}@sha256:{digest}\n")).expect("write");
        }
        fs::write(
            root.join(TEST_PIN_FILE),
            format!("PIN = \"{IMAGE}@sha256:{digest}\"\n"),
        )
        .expect("historical pytest fixture");
        assert_eq!(
            extract_pinned_digest(&root).expect("one pin"),
            format!("sha256:{digest}")
        );
    }

    #[test]
    fn conflicting_pins_fail_closed() {
        let root = fixture_root("conflict");
        fs::write(
            root.join(PIN_FILES[0]),
            format!("{IMAGE}@sha256:{}", "d".repeat(64)),
        )
        .expect("write");
        for name in &PIN_FILES[1..] {
            let path = root.join(name);
            fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
            fs::write(&path, "").expect("write");
        }
        fs::create_dir_all(root.join(TEST_PIN_FILE).parent().expect("test parent"))
            .expect("test dirs");
        fs::write(
            root.join(TEST_PIN_FILE),
            format!("PIN = \"{IMAGE}@sha256:{}\"\n", "e".repeat(64)),
        )
        .expect("fixture assertion");
        let error = extract_pinned_digest(&root).expect_err("must reject");
        assert!(
            error.contains("exactly one unique pinned digest"),
            "{error}"
        );
    }

    #[test]
    fn missing_pin_file_fails_closed() {
        let root = fixture_root("missing");
        fs::write(root.join(PIN_FILES[0]), "").expect("write");
        let error = extract_pinned_digest(&root).expect_err("must reject");
        assert!(error.contains("missing expected pin file"), "{error}");
    }

    #[test]
    fn promotion_rewrites_every_file() {
        let root = fixture_root("apply");
        let old = "d".repeat(64);
        let new = format!("sha256:{}", "f".repeat(64));
        for name in PIN_FILES {
            let path = root.join(name);
            fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
            fs::write(&path, format!("a {IMAGE}@sha256:{old} b\n")).expect("write");
        }
        let test_pin = root.join(TEST_PIN_FILE);
        fs::create_dir_all(test_pin.parent().expect("test parent")).expect("test dirs");
        fs::write(&test_pin, format!("PIN = \"{IMAGE}@sha256:{old}\"\n")).expect("write");
        let original_test_pin = fs::read(&test_pin).expect("original test pin");
        let changed = apply_promotion(&root, &new).expect("applies");
        assert_eq!(changed.len(), PIN_FILES.len());
        assert_eq!(
            fs::read(&test_pin).expect("historical test pin unchanged"),
            original_test_pin
        );
        for name in PIN_FILES {
            let text = fs::read_to_string(root.join(name)).expect("read");
            assert!(text.contains(&format!("{IMAGE}@{new}")));
            assert!(!text.contains(&old));
        }
    }

    #[test]
    fn registry_lookup_retries_transient_failures() {
        let digest = format!("sha256:{}", "a".repeat(64));
        let transport = Fake::new(vec![
            Err("connection reset".to_owned()),
            Ok(response(503, None, "")),
            Ok(response(200, None, r#"{"token":"t"}"#)),
            Ok(response(200, Some(("Docker-Content-Digest", &digest)), "")),
        ]);
        assert_eq!(
            registry_digest(&transport, "library/python", "3.13-slim", &mut dismiss)
                .expect("digest"),
            digest
        );
        assert_eq!(transport.calls.borrow().len(), 4);
    }

    #[test]
    fn registry_lookup_classifies_permanent_failure() {
        let transport = Fake::new(vec![Ok(response(404, None, ""))]);
        let error = registry_digest(&transport, "library/python", "gone", &mut dismiss)
            .expect_err("must fail");
        assert!(error.contains("HTTP 404"), "{error}");
        assert_eq!(transport.calls.borrow().len(), 1);
    }

    #[test]
    fn registry_lookup_rejects_unusable_digest() {
        let transport = Fake::new(vec![
            Ok(response(200, None, r#"{"token":"t"}"#)),
            Ok(response(200, Some(("Docker-Content-Digest", "nope")), "")),
        ]);
        let error = registry_digest(&transport, "library/python", "tag", &mut dismiss)
            .expect_err("must fail");
        assert!(error.contains("valid manifest digest"), "{error}");
    }

    #[test]
    fn curl_transport_rejects_non_https_targets() {
        let error = CurlTransport
            .get("http://registry-1.docker.io/v2/", &[])
            .expect_err("must reject");
        assert!(error.contains("HTTPS"), "{error}");
    }
}
