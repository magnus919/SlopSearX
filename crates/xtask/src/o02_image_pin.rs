//! O02: native image-pin and Docker base-digest tools.
//!
//! These replace the operational Python helpers that release jobs used to run.
//! Pin-state parsing and verification are pure and unit-tested; registry lookups are
//! bounded HTTPS requests performed by `curl` (a portable transport the release
//! runners already provide), with the same allowlisted hosts, accepted manifest
//! media types, three-attempt retry policy and 30-second timeout as before.
//!
//! Evidence boundary: this module verifies pins and queries registries. Its
//! promotion write path only accepts a no-op while source-custody pins are fixed;
//! it never builds, pushes or promotes an image, and it never touches production
//! state.
use crate::Result;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

/// Digest-pinned application image.
pub const IMAGE: &str = "ghcr.io/magnus919/slopsearx";
/// F01/W03-authenticated deployment test source SHA-256.
const FROZEN_TEST_PIN_SOURCE_SHA256: &str =
    "78d9dfccbb97dca38a4149e5f5154ebae18a11589616492234d9817e9d2ea1e2";
/// Literal pin in the exact authenticated source successor.
const FROZEN_TEST_PIN_DIGEST: &str =
    "sha256:91194d146d205b1cf4688c1989da8f5f6b599a9627be23fd1ee7a4e488fda5b7";

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

/// Extracts and validates an immutable image digest from a known image field.
fn digest_reference(value: &str, label: &str) -> Result<String> {
    let (repository, digest) = value
        .split_once('@')
        .ok_or_else(|| format!("O02 image pin: {label} must use an immutable digest"))?;
    need(
        repository == IMAGE && digest.starts_with("sha256:") && hex_digest(&digest[7..]),
        &format!("{label} must equal {IMAGE}@sha256:<64 lowercase hex>"),
    )?;
    Ok(digest.to_owned())
}

fn parse_yaml(text: &str, label: &str) -> Result<serde_json::Value> {
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
        budget: serde_saphyr::budget! {
            max_events: 100_000,
            max_aliases: 16,
            max_anchors: 16,
            flow_nesting_limit: 64,
        },
    };
    serde_saphyr::from_str_with_options(text, options)
        .map_err(|error| format!("O02 image pin: {label}: {error}"))
}

fn deployment_pin_text(name: &str, text: &str) -> Result<String> {
    let value = parse_yaml(text, name)?;
    let image = if name == PIN_FILES[0] {
        value["services"]["slopsearx"]["image"].as_str()
    } else {
        let spec = &value["spec"]["template"]["spec"];
        let Some(containers) = spec["containers"].as_array() else {
            return Err(format!("O02 image pin: {name} has no containers list"));
        };
        let mut named = containers
            .iter()
            .filter(|container| container["name"] == "slopsearx");
        let container = named
            .next()
            .ok_or_else(|| format!("O02 image pin: {name} lacks slopsearx container"))?;
        need(
            named.next().is_none(),
            &format!("{name} has duplicate slopsearx containers"),
        )?;
        container["image"].as_str()
    }
    .ok_or_else(|| format!("O02 image pin: {name} has no active slopsearx image field"))?;
    digest_reference(image, name)
}

const MAX_PIN_INPUT_BYTES: u64 = 64 * 1024;
const MAX_DOCKERFILE_BYTES: u64 = 4 * 1024 * 1024;

#[cfg(unix)]
fn bounded_text_with_limit(path: &Path, label: &str, limit: u64) -> Result<String> {
    use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    let components = path.components().collect::<Vec<_>>();
    let initial = if path.is_absolute() {
        fs::File::open("/").map_err(|error| error.to_string())?
    } else {
        fs::File::open(".").map_err(|error| error.to_string())?
    };
    let mut directory = unsafe { OwnedFd::from_raw_fd(initial.into_raw_fd()) };
    let mut file = None;
    for (index, component) in components.iter().enumerate() {
        match component {
            std::path::Component::RootDir | std::path::Component::CurDir => continue,
            std::path::Component::Normal(name) => {
                let name = std::ffi::CString::new(name.as_bytes())
                    .map_err(|error| format!("O02 image pin: {label} path component: {error}"))?;
                let is_last = components[index + 1..]
                    .iter()
                    .all(|part| matches!(part, std::path::Component::CurDir));
                let flags = if is_last {
                    nix::libc::O_RDONLY
                        | nix::libc::O_NOFOLLOW
                        | nix::libc::O_NONBLOCK
                        | nix::libc::O_CLOEXEC
                } else {
                    nix::libc::O_RDONLY
                        | nix::libc::O_DIRECTORY
                        | nix::libc::O_NOFOLLOW
                        | nix::libc::O_CLOEXEC
                };
                let fd = unsafe { nix::libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
                if fd < 0 {
                    return Err(format!(
                        "O02 image pin: no-follow open failed for {label}: {}",
                        std::io::Error::last_os_error()
                    ));
                }
                if is_last {
                    file = Some(unsafe { OwnedFd::from_raw_fd(fd) });
                } else {
                    directory = unsafe { OwnedFd::from_raw_fd(fd) };
                }
            }
            std::path::Component::ParentDir | std::path::Component::Prefix(_) => {
                return Err(format!(
                    "O02 image pin: non-normal path rejected for {label}"
                ));
            }
        }
    }
    let file = file.ok_or_else(|| format!("O02 image pin: no final file component for {label}"))?;
    let mut file = fs::File::from(file);
    let metadata = file
        .metadata()
        .map_err(|error| format!("O02 image pin: unable to stat {label}: {error}"))?;
    need(
        metadata.is_file(),
        &format!("{label} must be a regular file"),
    )?;
    need(
        metadata.len() <= limit,
        &format!("{label} exceeds {limit} byte limit"),
    )?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    use std::io::Read;
    file.by_ref()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("O02 image pin: unable to read {label}: {error}"))?;
    need(
        bytes.len() as u64 <= limit,
        &format!("{label} exceeds {limit} byte limit"),
    )?;
    String::from_utf8(bytes)
        .map_err(|error| format!("O02 image pin: {label} is not UTF-8: {error}"))
}

#[cfg(not(unix))]
fn bounded_text_with_limit(path: &Path, label: &str, limit: u64) -> Result<String> {
    use std::io::Read;
    if label == TEST_PIN_FILE || label == "Dockerfile" {
        return Err("O02 image pin: no-follow bounded reads require Unix openat".into());
    }
    let file = fs::File::open(path)
        .map_err(|error| format!("O02 image pin: unable to open {label}: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("O02 image pin: unable to stat {label}: {error}"))?;
    need(
        metadata.is_file(),
        &format!("{label} must be a regular file"),
    )?;
    need(
        metadata.len() <= limit,
        &format!("{label} exceeds {limit} byte limit"),
    )?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("O02 image pin: unable to read {label}: {error}"))?;
    need(
        bytes.len() as u64 <= limit,
        &format!("{label} exceeds {limit} byte limit"),
    )?;
    String::from_utf8(bytes)
        .map_err(|error| format!("O02 image pin: {label} is not UTF-8: {error}"))
}

fn bounded_text(path: &Path, label: &str) -> Result<String> {
    bounded_text_with_limit(path, label, MAX_PIN_INPUT_BYTES)
}

fn deployment_pin(root: &Path, name: &str) -> Result<String> {
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("O02 image pin: selected root is unavailable: {error}"))?;
    let path = canonical_root.join(name);
    let text = bounded_text(&path, name)?;
    deployment_pin_text(name, &text)
}

fn oracle_pin_text(text: &str) -> Result<String> {
    need(
        !text.contains('\r'),
        "non-custodied oracle uses unsupported CR/CRLF line endings",
    )?;
    let mut assignments = Vec::new();
    for line in text.lines() {
        let source = line.trim();
        if source.is_empty() || source.starts_with('#') {
            continue;
        }
        need(
            line == line.trim_start(),
            "non-custodied oracle assignment must start at column zero",
        )?;
        need(
            !source.ends_with('\\'),
            "explicit Python line continuation is outside the bounded oracle grammar",
        )?;
        let Some(rest) = source.strip_prefix("GHCR_DIGEST_PIN") else {
            return Err(
                "O02 image pin: non-custodied oracle must use the sealed literal-only grammar"
                    .into(),
            );
        };
        need(
            rest.chars()
                .next()
                .is_none_or(|character| character.is_ascii_whitespace() || character == '='),
            "GHCR_DIGEST_PIN must be a simple assignment target",
        )?;
        let Some(value) = rest.trim_start().strip_prefix('=') else {
            return Err(
                "O02 image pin: non-custodied oracle must use one simple assignment".into(),
            );
        };
        let value = value.trim_start();
        let quoted = value
            .strip_prefix('"')
            .ok_or("GHCR_DIGEST_PIN must use one double-quoted string literal")?;
        let (literal, trailing) = quoted
            .split_once('"')
            .ok_or("GHCR_DIGEST_PIN literal is missing its closing quote")?;
        need(
            !literal.contains('\\'),
            "GHCR_DIGEST_PIN literal escapes are outside the sealed grammar",
        )?;
        let trailing = trailing.trim_start();
        need(
            trailing.is_empty() || trailing.starts_with('#'),
            "GHCR_DIGEST_PIN must be exactly one quoted literal with no executable suffix",
        )?;
        assignments.push(digest_reference(literal, "GHCR_DIGEST_PIN")?);
    }
    need(
        assignments.len() == 1,
        "parity oracle must contain exactly one GHCR_DIGEST_PIN literal assignment",
    )?;
    Ok(assignments.pop().expect("one validated assignment"))
}

fn frozen_oracle_pin_text(text: &str) -> Result<String> {
    let actual = Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    need(
        actual == FROZEN_TEST_PIN_SOURCE_SHA256,
        "fixture-bound Python deployment oracle source hash changed",
    )?;
    let expected_invocation = "GHCR_DIGEST_PIN = \"ghcr.io/magnus919/slopsearx@sha256:91194d146d205b1cf4688c1989da8f5f6b599a9627be23fd1ee7a4e488fda5b7\"";
    let count = text
        .lines()
        .filter(|line| line.trim_start() == expected_invocation)
        .count();
    need(
        count == 1,
        "frozen test oracle contains exactly one literal pin declaration",
    )?;
    let source = include_str!("o02_health_retirement.rs");
    let expected_successor_row = format!(
        "        \"tests/test_deployment_artifacts.py\",\n        \"649650e35656bfff78d7930052085270811343fd\",\n        12945,\n        \"{FROZEN_TEST_PIN_SOURCE_SHA256}\",\n        12872,\n        \"e7634a07ab53f6409386a9515f46ab65ebccfb0747235d6ad43a07dcf54f431c\","
    );
    need(
        source.contains(&expected_successor_row),
        "frozen oracle W03/O02 successor metadata fragment matches",
    )?;
    let literal_digest = expected_invocation
        .split_once("@sha256:")
        .and_then(|(_, suffix)| suffix.strip_suffix('"'))
        .ok_or("frozen test oracle literal has digest")?;
    need(
        FROZEN_TEST_PIN_DIGEST == format!("sha256:{literal_digest}"),
        "frozen compiled digest matches the exact custody-authenticated source literal",
    )?;
    digest_reference(
        &format!("{IMAGE}@{FROZEN_TEST_PIN_DIGEST}"),
        "fixture-bound GHCR_DIGEST_PIN",
    )
}

fn oracle_pin(root: &Path) -> Result<String> {
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("O02 image pin: selected root is unavailable: {error}"))?;
    let path = canonical_root.join(TEST_PIN_FILE);
    let text = bounded_text(&path, TEST_PIN_FILE)?;
    let repository_oracle = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/test_deployment_artifacts.py")
        .canonicalize()
        .map_err(|error| format!("O02 image pin: canonical build oracle: {error}"))?;
    let build_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|error| format!("O02 image pin: canonical build root: {error}"))?;
    if path == repository_oracle && canonical_root == build_root {
        return frozen_oracle_pin_text(&text);
    }
    oracle_pin_text(&text)
}

/// Collects the unique active image digest across manifests and parity oracle.
pub fn extract_pinned_digest(root: &Path) -> Result<String> {
    let mut pins = BTreeSet::new();
    for name in PIN_FILES {
        pins.insert(deployment_pin(root, name)?);
    }
    pins.insert(oracle_pin(root)?);
    need(
        pins.len() == 1,
        &format!(
            "expected exactly one unique pinned digest, found [{}]",
            pins.iter().cloned().collect::<Vec<_>>().join(", ")
        ),
    )?;
    Ok(pins.into_iter().next().expect("one pin"))
}

/// Refuses digest drift while the Python source hash is fixture-bound; same-pin
/// invocation is a no-op. Drift requires the sealed source-promotion workflow.
pub fn apply_promotion(root: &Path, new_digest: &str) -> Result<Vec<String>> {
    let current = extract_pinned_digest(root)?;
    if new_digest != current {
        return Err(
            "native --write refuses drift while the fixture-bound test pin would remain stale; use the sealed promotion workflow".into(),
        );
    }
    Ok(Vec::new())
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
    let payload: serde_json::Value = serde_json::from_str(body).map_err(|_| LookupError {
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

fn promotion_mode(args: &[String]) -> Result<(bool, Option<String>)> {
    let mut check = false;
    let mut write = false;
    let mut root = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--check" => {
                need(!check, "duplicate --check flag")?;
                check = true;
            }
            "--write" => {
                need(!write, "duplicate --write flag")?;
                write = true;
            }
            "--root" => {
                need(root.is_none(), "duplicate --root flag")?;
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or("--root requires a directory value")?;
                root = Some(value.clone());
                index += 1;
            }
            other => {
                return Err(format!(
                    "O02 image pin: unsupported promotion argument {other}"
                ));
            }
        }
        index += 1;
    }
    need(
        check ^ write,
        "image-pin-promote requires exactly one of --check or --write",
    )?;
    Ok((check, root))
}

fn dockerfile_value(args: &[String]) -> Result<String> {
    let mut dockerfile = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dockerfile" => {
                need(dockerfile.is_none(), "duplicate --dockerfile flag")?;
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or("--dockerfile requires a path")?;
                dockerfile = Some(value.clone());
                index += 1;
            }
            other => {
                return Err(format!(
                    "O02 image pin: unsupported verifier argument {other}"
                ));
            }
        }
        index += 1;
    }
    Ok(dockerfile.unwrap_or_else(|| DEFAULT_DOCKERFILE.to_owned()))
}

fn verify_base_with(
    root: &Path,
    args: &[String],
    transport: &dyn Transport,
    sleep: &mut dyn FnMut(u64),
) -> Result<i32> {
    let dockerfile = dockerfile_value(args)?;
    let path = if Path::new(&dockerfile).is_absolute() {
        Path::new(&dockerfile).to_path_buf()
    } else {
        root.join(&dockerfile)
    };
    let text = bounded_text_with_limit(&path, "Dockerfile", MAX_DOCKERFILE_BYTES)?;
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

fn promote_with_mode(root: &Path, check: bool) -> Result<i32> {
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

// Preserve the Python helper contract: drift is 1, malformed input/lookup errors are 2.
fn command_exit_code(result: Result<i32>) -> i32 {
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            2
        }
    }
}

/// Dispatches `image-pin-verify-base` and `image-pin-promote`.
pub fn cli(root: &Path, args: &[String]) -> Result<()> {
    let (command, rest) = args
        .split_first()
        .ok_or("usage: cargo xtask ci-helper image-pin-{verify-base|promote}")?;
    let code = match command.as_str() {
        "image-pin-verify-base" => {
            let effective = root.to_path_buf();
            command_exit_code(verify_base(&effective, rest))
        }
        "image-pin-promote" => {
            let (check, selected_root) = promotion_mode(rest)?;
            let effective =
                selected_root.map_or_else(|| root.to_path_buf(), std::path::PathBuf::from);
            command_exit_code(promote_with_mode(&effective, check))
        }
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
    use serde_json::json;
    use std::{
        fs,
        ops::Deref,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
    const FIXTURE_MAX_DEPTH: usize = 8;
    const FIXTURE_MAX_ENTRIES: usize = 128;

    struct FixtureDir {
        base: PathBuf,
        path: PathBuf,
        device: u64,
        inode: u64,
    }

    impl FixtureDir {
        fn new(name: &str) -> Self {
            let configured = std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT")
                .expect("registered external SLOPSEARX_TEST_TEMP_ROOT");
            let configured = PathBuf::from(configured);
            assert!(configured.is_absolute(), "fixture root must be absolute");
            let root = fs::canonicalize(&configured).expect("canonical external fixture root");
            assert_eq!(root, configured, "fixture root may not traverse symlinks");
            let metadata = fs::symlink_metadata(&root).expect("external fixture root");
            assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
            let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .expect("canonical repository root");
            assert!(
                !root.starts_with(checkout),
                "fixture root must be outside checkout"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                assert_eq!(metadata.uid(), unsafe { nix::libc::geteuid() } as u32);
            }
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock after Unix epoch")
                .as_nanos();
            for _ in 0..8 {
                let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
                let path = root.join(format!(
                    "o02-image-pin-{}-{id}-{nonce}-{name}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::{MetadataExt, PermissionsExt};
                            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                                .expect("restrict fixture directory");
                            let owned =
                                fs::symlink_metadata(&path).expect("owned fixture metadata");
                            return Self {
                                base: root.clone(),
                                path,
                                device: owned.dev(),
                                inode: owned.ino(),
                            };
                        }
                        #[cfg(not(unix))]
                        {
                            return Self {
                                base: root.clone(),
                                path,
                                device: 0,
                                inode: 0,
                            };
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("create owned fixture directory: {error}"),
                }
            }
            panic!("unable to allocate a unique fixture directory")
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Deref for FixtureDir {
        type Target = Path;
        fn deref(&self) -> &Self::Target {
            self.path()
        }
    }

    impl Drop for FixtureDir {
        fn drop(&mut self) {
            remove_fixture_tree(&self.base, &self.path, self.device, self.inode)
                .expect("owned fixture cleanup");
        }
    }

    #[cfg(unix)]
    fn remove_fixture_tree(
        base: &Path,
        path: &Path,
        device: u64,
        inode: u64,
    ) -> std::io::Result<()> {
        use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
        fn walk(
            parent: &OwnedFd,
            name: &std::ffi::OsStr,
            root_dev: u64,
            depth: usize,
            count: &mut usize,
        ) -> std::io::Result<()> {
            use std::os::unix::ffi::OsStrExt;
            assert!(depth <= FIXTURE_MAX_DEPTH, "fixture cleanup depth exceeded");
            let c_name = std::ffi::CString::new(name.as_bytes())
                .map_err(|_| std::io::Error::other("NUL in owned fixture name"))?;
            let mut stat = std::mem::MaybeUninit::<nix::libc::stat>::uninit();
            let result = unsafe {
                nix::libc::fstatat(
                    parent.as_raw_fd(),
                    c_name.as_ptr(),
                    stat.as_mut_ptr(),
                    nix::libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if result != 0 {
                return Err(std::io::Error::last_os_error());
            }
            let stat = unsafe { stat.assume_init() };
            assert!(
                i128::from(stat.st_dev) == i128::from(root_dev),
                "fixture cleanup crossed filesystem"
            );
            assert!(
                (stat.st_mode & nix::libc::S_IFMT) != nix::libc::S_IFLNK,
                "fixture cleanup symlink refused"
            );
            *count += 1;
            assert!(
                *count <= FIXTURE_MAX_ENTRIES,
                "fixture cleanup entry limit exceeded"
            );
            if (stat.st_mode & nix::libc::S_IFMT) == nix::libc::S_IFDIR {
                let child_fd = unsafe {
                    nix::libc::openat(
                        parent.as_raw_fd(),
                        c_name.as_ptr(),
                        nix::libc::O_RDONLY
                            | nix::libc::O_DIRECTORY
                            | nix::libc::O_NOFOLLOW
                            | nix::libc::O_CLOEXEC,
                    )
                };
                if child_fd < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let child = unsafe { OwnedFd::from_raw_fd(child_fd) };
                let duplicate = unsafe { nix::libc::dup(child.as_raw_fd()) };
                if duplicate < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let read_fd = unsafe { OwnedFd::from_raw_fd(duplicate) };
                let dir = unsafe { nix::libc::fdopendir(read_fd.as_raw_fd()) };
                if dir.is_null() {
                    return Err(std::io::Error::last_os_error());
                }
                std::mem::forget(read_fd);
                loop {
                    let entry = unsafe { nix::libc::readdir(dir) };
                    if entry.is_null() {
                        break;
                    }
                    let entry_name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) };
                    let bytes = entry_name.to_bytes();
                    if bytes == b"." || bytes == b".." {
                        continue;
                    }
                    let child_name = std::ffi::OsStr::from_bytes(bytes);
                    walk(&child, child_name, root_dev, depth + 1, count)?;
                }
                unsafe {
                    nix::libc::closedir(dir);
                }
                let removed = unsafe {
                    nix::libc::unlinkat(
                        parent.as_raw_fd(),
                        c_name.as_ptr(),
                        nix::libc::AT_REMOVEDIR,
                    )
                };
                if removed != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            } else {
                let removed =
                    unsafe { nix::libc::unlinkat(parent.as_raw_fd(), c_name.as_ptr(), 0) };
                if removed != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        }
        use std::os::unix::ffi::OsStrExt;
        assert_eq!(
            path.parent(),
            Some(base),
            "fixture must remain a direct child"
        );
        let base_file = fs::File::open(base)?;
        let base_fd = unsafe { OwnedFd::from_raw_fd(base_file.into_raw_fd()) };
        let name = path
            .file_name()
            .ok_or_else(|| std::io::Error::other("fixture lacks final component"))?;
        let c_name = std::ffi::CString::new(name.as_bytes())
            .map_err(|_| std::io::Error::other("NUL in fixture name"))?;
        let mut stat = std::mem::MaybeUninit::<nix::libc::stat>::uninit();
        let result = unsafe {
            nix::libc::fstatat(
                base_fd.as_raw_fd(),
                c_name.as_ptr(),
                stat.as_mut_ptr(),
                nix::libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if result != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let stat = unsafe { stat.assume_init() };
        assert!(
            i128::from(stat.st_dev) == i128::from(device) && stat.st_ino == inode,
            "fixture root identity changed"
        );
        let mut count = 0;
        walk(&base_fd, name, device, 0, &mut count)
    }

    #[cfg(not(unix))]
    fn remove_fixture_tree(
        _base: &Path,
        _path: &Path,
        _device: u64,
        _inode: u64,
    ) -> std::io::Result<()> {
        Err(std::io::Error::other("O02 fixture cleanup requires Unix"))
    }

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

    fn build_dockerfile(root: &FixtureDir) -> std::path::PathBuf {
        let builder = "b".repeat(64);
        let runtime = "c".repeat(64);
        fs::create_dir_all(&**root).expect("fixture root exists");
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
        assert_eq!(command_exit_code(Err(error)), 2);
        assert_eq!(command_exit_code(Ok(1)), 1);
        assert_eq!(command_exit_code(Ok(0)), 0);
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

    fn fixture_root(name: &str) -> FixtureDir {
        FixtureDir::new(name)
    }

    fn write_pin_fixtures(root: &FixtureDir, digest: &str) {
        let manifest = format!("ghcr.io/magnus919/slopsearx@{digest}");
        fs::create_dir_all(root.join("services")).expect("compose fixture directory");
        let compose = json!({"services":{"slopsearx":{"image":manifest}}});
        fs::write(
            root.join(PIN_FILES[0]),
            serde_json::to_string(&compose).expect("compose JSON fixture"),
        )
        .expect("compose fixture");
        fs::create_dir_all(root.join("k8s")).expect("kubernetes fixture directory");
        let deployment = json!({"spec":{"template":{"spec":{"containers":[{"name":"slopsearx","image":manifest}]}}}});
        fs::write(
            root.join(PIN_FILES[1]),
            serde_json::to_string(&deployment).expect("deployment JSON fixture"),
        )
        .expect("kubernetes fixture");
        fs::create_dir_all(root.join(TEST_PIN_FILE).parent().expect("test parent"))
            .expect("test fixture directory");
        fs::write(
            root.join(TEST_PIN_FILE),
            format!("GHCR_DIGEST_PIN = \"{manifest}\"\n"),
        )
        .expect("oracle fixture");
    }

    #[test]
    fn frozen_checkout_oracle_uses_authenticated_fixture_pin() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source_path = root.join(TEST_PIN_FILE);
        let original = fs::read(&source_path).expect("authenticated tracked oracle source");
        assert_eq!(
            oracle_pin(&root).expect("authenticated tracked oracle"),
            FROZEN_TEST_PIN_DIGEST
        );

        let mut mutated = original.clone();
        mutated.extend_from_slice(b"# mutation\n");
        assert!(frozen_oracle_pin_text(std::str::from_utf8(&mutated).unwrap()).is_err());

        let fixture = fixture_root("modified-source");
        write_pin_fixtures(&fixture, FROZEN_TEST_PIN_DIGEST);
        fs::write(fixture.join(TEST_PIN_FILE), &mutated).expect("modified synthetic source");
        assert!(
            oracle_pin(&fixture).is_err(),
            "modified bytes cannot inherit the source-bound exception"
        );
    }

    #[test]
    fn unique_pinned_digest_is_extracted() {
        let root = fixture_root("unique");
        let digest = "d".repeat(64);
        write_pin_fixtures(&root, &format!("sha256:{digest}"));
        assert_eq!(
            extract_pinned_digest(&root).expect("one pin"),
            format!("sha256:{digest}")
        );
    }

    #[test]
    fn conflicting_pins_fail_closed() {
        let root = fixture_root("conflict");
        write_pin_fixtures(&root, &format!("sha256:{}", "d".repeat(64)));
        let k8s_path = root.join(PIN_FILES[1]);
        let text = fs::read_to_string(&k8s_path).expect("read Kubernetes fixture");
        fs::write(&k8s_path, text.replace(&"d".repeat(64), &"e".repeat(64)))
            .expect("write conflicting Kubernetes pin");
        let oracle_path = root.join(TEST_PIN_FILE);
        let text = fs::read_to_string(&oracle_path).expect("read oracle fixture");
        fs::write(&oracle_path, text.replace(&"d".repeat(64), &"e".repeat(64)))
            .expect("write conflicting oracle pin");
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
        assert!(error.contains(PIN_FILES[0]), "{error}");
    }

    #[test]
    fn mutable_active_image_with_comment_decoy_fails_closed() {
        let root = fixture_root("compose-decoy");
        write_pin_fixtures(&root, &format!("sha256:{}", "a".repeat(64)));
        let decoy = format!("# {IMAGE}@sha256:{}", "a".repeat(64));
        let text = json!({"services":{"slopsearx":{"image":"ghcr.io/magnus919/slopsearx:latest"}},"note":decoy});
        fs::write(
            root.join(PIN_FILES[0]),
            serde_json::to_string(&text).expect("compose decoy JSON"),
        )
        .expect("mutable image with stale unrelated reference");
        let error = extract_pinned_digest(&root)
            .expect_err("unrelated reference cannot stand in for active pin");
        assert!(error.contains("immutable digest"), "{error}");
    }

    #[test]
    fn duplicate_oracle_assignment_without_spaces_fails_closed() {
        let root = fixture_root("oracle-duplicate");
        write_pin_fixtures(&root, &format!("sha256:{}", "a".repeat(64)));
        let path = root.join(TEST_PIN_FILE);
        let mut text = fs::read_to_string(&path).expect("oracle fixture");
        text.push_str("GHCR_DIGEST_PIN=\"ghcr.io/magnus919/slopsearx@sha256:");
        text.push_str(&"a".repeat(64));
        text.push_str("\"\n");
        fs::write(path, text).expect("duplicate alternate assignment");
        assert!(oracle_pin(&root).is_err());
        let mut augmented = fs::read_to_string(root.join(TEST_PIN_FILE)).expect("duplicate oracle");
        augmented.push_str("GHCR_DIGEST_PIN += \"x\"\n");
        assert!(oracle_pin_text(&augmented).is_err());
        assert!(
            oracle_pin_text(&format!(
                "{}\nsys._getframe().f_globals[\"GHCR_DIGEST_PIN\"] = \"x\"\n",
                fs::read_to_string(root.join(TEST_PIN_FILE)).expect("valid pin")
            ))
            .is_err()
        );
    }

    #[test]
    fn over_limit_pin_input_is_rejected_before_parse() {
        let root = fixture_root("oversized-input");
        let path = root.join(PIN_FILES[0]);
        fs::write(&path, vec![b' '; (MAX_PIN_INPUT_BYTES + 1) as usize])
            .expect("oversized fixture");
        let error = bounded_text(&path, PIN_FILES[0]).expect_err("must enforce cap");
        assert!(error.contains("byte limit"), "{error}");
    }

    #[test]
    fn non_custodied_fstring_reads_are_outside_sealed_grammar() {
        let root = fixture_root("real-oracle-fstrings");
        write_pin_fixtures(&root, &format!("sha256:{}", "a".repeat(64)));
        let path = root.join(TEST_PIN_FILE);
        let mut source = fs::read_to_string(&path).expect("oracle fixture");
        source.push_str("def reads_pin():\n    return f\"{GHCR_DIGEST_PIN}\"\n");
        fs::write(&path, source).expect("ordinary f-string read");
        assert!(
            oracle_pin(&root).is_err(),
            "non-custodied function code is rejected"
        );
    }

    #[test]
    fn f_string_assignment_expressions_are_rejected() {
        let valid = format!(
            "GHCR_DIGEST_PIN = \"{}@sha256:{}\"\n",
            IMAGE,
            "a".repeat(64)
        );
        assert!(
            oracle_pin_text(&format!(
                "{valid}GHCR_DIGEST_PIN = \"{}@sha256:{}\" \"suffix\"\n",
                IMAGE,
                "b".repeat(64)
            ))
            .is_err()
        );
        let source = format!(
            "{valid}f\"{{(GHCR_DIGEST_PIN := '{}@sha256:{}')}}\"\n",
            IMAGE,
            "b".repeat(64)
        );
        assert!(oracle_pin_text(&source).is_err());
        let reflective = format!(
            "{valid}globals[\"GHCR_DIGEST_PIN\"] = \"{}@sha256:{}\"\n",
            IMAGE,
            "b".repeat(64)
        );
        assert!(oracle_pin_text(&reflective).is_err());
        assert!(oracle_pin_text(&format!("{valid}exec (\"GHCR_DIGEST_PIN = x\")\n")).is_err());
        assert!(
            oracle_pin_text(&format!(
                "{valid}setattr (object, \"GHCR_DIGEST_PIN\", \"x\")\n"
            ))
            .is_err()
        );
        assert!(
            oracle_pin_text(&format!(
                "{valid}vars () .update({{\"GHCR_DIGEST_PIN\": \"x\"}})\n"
            ))
            .is_err()
        );
        assert!(oracle_pin_text(&format!("{valid}__import__ (\"os\")\n")).is_err());
        assert!(
            oracle_pin_text(&format!(
                "{valid}globals () .update({{\"GHCR_DIGEST_PIN\": \"x\"}})\n"
            ))
            .is_err()
        );
        assert!(
            oracle_pin_text(&format!(
                "{valid}setattr(sys.modules[__name__], \"GHCR_DIGEST_PIN\", \"x\")\n"
            ))
            .is_err()
        );
        assert!(
            oracle_pin_text(&format!(
                "{valid}locals().update({{\"GHCR_DIGEST_PIN\": \"x\"}})\n"
            ))
            .is_err()
        );
        assert!(
            oracle_pin_text(&format!(
                "{valid}sys.modules[__name__].__dict__[\"GHCR_DIGEST_PIN\"] = \"x\"\n"
            ))
            .is_err()
        );
        assert!(
            oracle_pin_text(&format!(
                "{valid}__builtins__[\"exec\"](\"GHCR_DIGEST_PIN = 'x'\")\n"
            ))
            .is_err()
        );
        let b = "b".repeat(64);
        let builtin_exec =
            format!("{valid}__builtins__[\"exec\"](\"GHCR_DIGEST_PIN = '{IMAGE}@sha256:{b}'\")\n");
        assert!(oracle_pin_text(&builtin_exec).is_err());
        let computed_builtin_exec =
            format!("{valid}__builtins__['ex'+'ec'](\"GHCR_DIGEST_PIN = '{IMAGE}@sha256:{b}'\")\n");
        assert!(oracle_pin_text(&computed_builtin_exec).is_err());
        let continued = format!("{valid}GHCR_DIGEST_PIN \\\n    = \"{IMAGE}@sha256:{b}\"\n");
        assert!(oracle_pin_text(&continued).is_err());
        assert!(oracle_pin_text(&valid).is_ok());
        let dynamic_fstring = format!(
            "{valid}f\"{{__builtins__['ex'+'ec']('GHCR_DIGEST_PIN = ' + chr(39) + '{IMAGE}@sha256:{b}' + chr(39))}}\"\n"
        );
        assert!(oracle_pin_text(&dynamic_fstring).is_err());
    }

    #[test]
    fn sealed_non_custodied_grammar_rejects_executable_statements() {
        let digest = "a".repeat(64);
        let valid = format!("GHCR_DIGEST_PIN = \"{IMAGE}@sha256:{digest}\"\n");
        for unsupported in [
            format!("    {valid}"),
            format!("\t{valid}"),
            format!(" \t{valid}"),
            format!("{valid}note = \"GHCR_DIGEST_PIN = x\"\n"),
            format!("{valid}def reads_pin():\n    return GHCR_DIGEST_PIN\n"),
            format!("{valid}if False:\n    GHCR_DIGEST_PIN = \"x\"\n"),
            format!("{valid}sys._getframe().f_globals[\"GHCR_DIGEST_PIN\"] = \"x\"\n"),
            format!("{valid}__builtins__['ex'+'ec'](\"GHCR_DIGEST_PIN = 'x'\")\n"),
            format!("{valid}f\"{{'text := ordinary string content'}}\"\n"),
            format!("{valid}f\"{{GHCR_DIGEST_PIN}}\" # ordinary comment = example\n"),
            format!("{valid}# ordinary comment\rimport os\n"),
            format!("{valid}GHCR_DIGEST_PIN = \"{IMAGE}@sha256:{digest}\\\"#decoy\"\n"),
        ] {
            assert!(
                oracle_pin_text(&unsupported).is_err(),
                "untrusted synthetic source must fail closed: {unsupported:?}"
            );
        }
        assert_eq!(
            oracle_pin_text(&valid).expect("sealed literal"),
            format!("sha256:{digest}")
        );
    }

    #[test]
    fn oversized_dockerfile_is_rejected_before_registry_queries() {
        let root = fixture_root("oversized-dockerfile");
        fs::create_dir_all(&*root).expect("Dockerfile fixture root");
        let path = root.join("Dockerfile");
        fs::write(&path, vec![b' '; (MAX_DOCKERFILE_BYTES + 1) as usize])
            .expect("oversized Dockerfile fixture");
        let transport = Fake::new(Vec::new());
        let mut sleep = dismiss;
        let error = verify_base_with(&root, &[], &transport, &mut sleep).expect_err("must reject");
        assert!(error.contains("byte limit"), "{error}");
        assert!(transport.calls.borrow().is_empty());
    }

    #[test]
    fn non_custodied_function_code_is_rejected() {
        let root = fixture_root("indented-python");
        write_pin_fixtures(&root, &format!("sha256:{}", "a".repeat(64)));
        let path = root.join(TEST_PIN_FILE);
        let mut text = fs::read_to_string(&path).expect("oracle fixture");
        text.push_str("def helper():\n    value = 1\n    return value\n");
        text.push_str("def reads_pin():\n    return GHCR_DIGEST_PIN\n");
        fs::write(&path, text).expect("ordinary indented function");
        assert!(
            oracle_pin(&root).is_err(),
            "non-custodied function code is rejected"
        );
    }

    #[test]
    fn nested_oracle_assignment_is_not_a_module_level_pin() {
        let root = fixture_root("nested-oracle");
        write_pin_fixtures(&root, &format!("sha256:{}", "a".repeat(64)));
        let path = root.join(TEST_PIN_FILE);
        let mut text = fs::read_to_string(&path).expect("oracle fixture");
        text.push_str("if False:\n    GHCR_DIGEST_PIN = \"ghcr.io/magnus919/slopsearx@sha256:");
        text.push_str(&"b".repeat(64));
        text.push_str("\"\n");
        fs::write(&path, &text).expect("indented nested assignment");
        assert!(
            oracle_pin(&root).is_err(),
            "indented target assignment rejected"
        );
        for nested in [
            format!(
                "GHCR_DIGEST_PIN = \"{}@sha256:{}\" \"suffix\"",
                IMAGE,
                "a".repeat(64)
            ),
            format!(
                "if False: GHCR_DIGEST_PIN = \"{}@sha256:{}\"",
                IMAGE,
                "b".repeat(64)
            ),
            format!(
                "x = 1; GHCR_DIGEST_PIN = \"{}@sha256:{}\"",
                IMAGE,
                "b".repeat(64)
            ),
        ] {
            let mut inline = fs::read_to_string(&path).expect("reset oracle fixture");
            inline.push_str(&nested);
            inline.push('\n');
            fs::write(&path, inline).expect("inline nested assignment");
            assert!(
                oracle_pin(&root).is_err(),
                "inline target assignment rejected: {nested}"
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn symlinked_parent_component_is_rejected() {
        use std::os::unix::fs::symlink;
        let root = fixture_root("parent-link");
        write_pin_fixtures(&root, &format!("sha256:{}", "a".repeat(64)));
        let k8s = root.join("k8s");
        let moved = root.join("k8s-real");
        fs::rename(&k8s, &moved).expect("move owned k8s fixture");
        symlink(&moved, &k8s).expect("link parent component");
        let result = deployment_pin(&root, PIN_FILES[1]);
        fs::remove_file(&k8s).expect("remove owned symlink before fixture drop");
        fs::rename(&moved, &k8s).expect("restore owned directory before fixture drop");
        let error = result.expect_err("must refuse parent symlink");
        assert!(error.contains("no-follow open failed"), "{error}");
    }

    #[test]
    fn mutable_kubernetes_container_with_decoy_fails_closed() {
        let root = fixture_root("k8s-decoy");
        write_pin_fixtures(&root, &format!("sha256:{}", "a".repeat(64)));
        let manifest = json!({
            "spec":{"template":{"spec":{"containers":[{"name":"slopsearx","image":"ghcr.io/magnus919/slopsearx:latest"}]}}},
            "note":format!("{}@sha256:{}", IMAGE, "a".repeat(64))
        });
        fs::write(
            root.join(PIN_FILES[1]),
            serde_json::to_string(&manifest).expect("Kubernetes decoy JSON"),
        )
        .expect("write mutable Kubernetes container");
        let error = extract_pinned_digest(&root)
            .expect_err("unrelated field cannot stand in for active Kubernetes image");
        assert!(error.contains("immutable digest"), "{error}");
    }

    #[test]
    fn no_pin_in_one_present_manifest_fails_closed() {
        let root = fixture_root("empty-manifest-pin");
        write_pin_fixtures(&root, &format!("sha256:{}", "d".repeat(64)));
        fs::write(root.join(PIN_FILES[0]), "services: {}\n").expect("empty compose pin");
        let error = extract_pinned_digest(&root).expect_err("manifest without a pin is incomplete");
        assert!(error.contains(PIN_FILES[0]), "{error}");
    }

    #[test]
    fn native_write_refuses_drift_without_mutating_manifest_or_test_oracle() {
        let root = fixture_root("apply");
        let old = "d".repeat(64);
        let new = format!("sha256:{}", "f".repeat(64));
        write_pin_fixtures(&root, &format!("sha256:{old}"));
        let test_pin = root.join(TEST_PIN_FILE);
        let original_manifests =
            PIN_FILES.map(|name| fs::read(root.join(name)).expect("manifest before"));
        let original_test_pin = fs::read(&test_pin).expect("test pin before");
        let error = apply_promotion(&root, &new).expect_err("must not create split-brain pins");
        assert!(error.contains("fixture-bound"), "{error}");
        for (name, original) in PIN_FILES.into_iter().zip(original_manifests) {
            assert_eq!(fs::read(root.join(name)).expect("manifest after"), original);
        }
        assert_eq!(
            fs::read(&test_pin).expect("test oracle after"),
            original_test_pin
        );
        assert_eq!(
            extract_pinned_digest(&root).expect("pin remains consistent"),
            format!("sha256:{old}")
        );
        assert_eq!(
            apply_promotion(&root, &format!("sha256:{old}")).expect("same-digest no-op"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn promotion_requires_exactly_one_explicit_mode() {
        assert!(promotion_mode(&[]).is_err());
        assert!(promotion_mode(&["--check".into(), "--write".into()]).is_err());
        assert!(promotion_mode(&["--check".into()]).expect("check mode").0);
        assert!(!promotion_mode(&["--write".into()]).expect("write mode").0);
        assert!(promotion_mode(&["--check".into(), "--surprise".into()]).is_err());
        assert!(
            promotion_mode(&[
                "--check".into(),
                "--root".into(),
                "a".into(),
                "--root".into(),
                "b".into()
            ])
            .is_err()
        );
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
