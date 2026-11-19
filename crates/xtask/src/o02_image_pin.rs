//! O02: native image-pin and Docker base-digest tools.
//!
//! These replace the operational Python helpers that release jobs used to run.
//! Pin-state parsing and verification are pure and unit-tested; registry lookups are
//! bounded HTTPS requests performed by `curl` (a portable transport the release
//! runners already provide), with the same allowlisted hosts, accepted manifest
//! media types, three-attempt retry policy and 30-second timeout as before.
//!
//! Evidence boundary: this module verifies pins and queries registries. Its
//! promotion writer prepares four exact, validated outputs before replacement;
//! it never builds, pushes or promotes an image, and it never touches production
//! state.
use crate::Result;
use crate::o02_health_retirement::image_pin_custody;
#[cfg(unix)]
#[path = "o02_image_files.rs"]
mod promotion_files;
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
        let operand = crate::o02_health_retirement::deployment_source_operand(&canonical_root)?;
        let operand = std::str::from_utf8(&operand).map_err(|error| error.to_string())?;
        frozen_oracle_pin_text(operand)?;
        return image_pin_custody::current_digest(text.as_bytes());
    }
    if let Ok(digest) = image_pin_custody::current_digest(text.as_bytes()) {
        // A copied real oracle still needs the independently authenticated
        // runtime capsule and historical inverse, never a Python scanner.
        let operand = crate::o02_health_retirement::deployment_source_operand(&canonical_root)?;
        frozen_oracle_pin_text(std::str::from_utf8(&operand).map_err(|e| e.to_string())?)?;
        return Ok(digest);
    }
    oracle_pin_text(&text)
}

/// Collects the unique active image digest across manifests and parity oracle.
pub fn extract_pinned_digest(root: &Path) -> Result<String> {
    #[cfg(unix)]
    let _guard = promotion_files::read_guard(root)?;
    let mut pins = BTreeSet::new();
    for name in PIN_FILES {
        pins.insert(deployment_pin(root, name)?);
    }
    pins.insert(oracle_pin(root)?);
    pins.insert(spec_pin_text(&bounded_text(
        &root
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join("spec.md"),
        "spec.md",
    )?)?);
    need(
        pins.len() == 1,
        &format!(
            "expected exactly one unique pinned digest, found [{}]",
            pins.iter().cloned().collect::<Vec<_>>().join(", ")
        ),
    )?;
    Ok(pins.into_iter().next().expect("one pin"))
}

const PROMOTION_FILES: [&str; 4] = [PIN_FILES[0], PIN_FILES[1], TEST_PIN_FILE, "spec.md"];

#[derive(Clone, Debug, PartialEq, Eq)]
struct PinText {
    path: &'static str,
    bytes: Vec<u8>,
}
struct PromotionPlan {
    originals: Vec<PinText>,
    outputs: Vec<PinText>,
    current: String,
    latest: String,
}
impl PromotionPlan {
    fn changed(&self) -> bool {
        self.current != self.latest
    }
}
#[derive(Debug)]
struct ApplyError {
    message: &'static str,
    replaced: Vec<&'static str>,
    cleanup_failed: Vec<&'static str>,
}
impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}; replaced=[{}]; staging_cleanup_failed=[{}]",
            self.message,
            self.replaced.join(", "),
            self.cleanup_failed.join(", ")
        )
    }
}

// Preserve YAML formatting while proving the semantic delta is exactly the
// named active image field. Comments, aliases, escaped scalars and unrelated
// references cannot authorize a replacement or broaden its scope.
fn replace_deployment(name: &str, text: &str, current: &str, latest: &str) -> Result<String> {
    need(
        deployment_pin_text(name, text)? == current,
        "manifest pin changed during planning",
    )?;
    let old = format!("{IMAGE}@{current}");
    let new = format!("{IMAGE}@{latest}");
    need(
        text.matches(&old).count() == 1,
        "manifest requires one unambiguous literal image reference",
    )?;
    let updated = text.replacen(&old, &new, 1);
    let mut expected = parse_yaml(text, name)?;
    if name == PIN_FILES[0] {
        expected["services"]["slopsearx"]["image"] = new.into();
    } else {
        for container in expected["spec"]["template"]["spec"]["containers"]
            .as_array_mut()
            .ok_or("O02 image pin: containers required")?
        {
            if container["name"] == "slopsearx" {
                container["image"] = new.clone().into();
            }
        }
    }
    need(
        parse_yaml(&updated, name)? == expected,
        "replacement must change only the active image field",
    )?;
    need(
        deployment_pin_text(name, &updated)? == latest,
        "updated active image digest",
    )?;
    Ok(updated)
}

fn spec_pin_text(text: &str) -> Result<String> {
    need(
        text.matches(IMAGE).count() == 2,
        "spec.md must contain exactly two application image references",
    )?;
    let blocks: Vec<&str> = text
        .split("```yaml\n")
        .skip(1)
        .filter_map(|tail| tail.split_once("\n```").map(|(block, _)| block))
        .filter(|block| block.contains(IMAGE))
        .collect();
    need(
        blocks.len() == 2,
        "spec.md requires two active YAML examples",
    )?;
    let kubernetes = deployment_pin_text(PIN_FILES[1], blocks[0])?;
    let compose = parse_yaml(blocks[1], "spec.md Compose example")?;
    let compose = digest_reference(
        compose["slopsearx"]["image"]
            .as_str()
            .ok_or("O02 image pin: spec.md lacks active Compose image")?,
        "spec.md Compose example",
    )?;
    need(kubernetes == compose, "spec.md example pins must agree")?;
    Ok(kubernetes)
}

fn replace_spec(text: &str, current: &str, latest: &str) -> Result<String> {
    need(
        spec_pin_text(text)? == current,
        "spec.md pin must match active deployment",
    )?;
    let old = format!("{IMAGE}@{current}");
    let new = format!("{IMAGE}@{latest}");
    need(
        text.matches(&old).count() == 2,
        "spec.md requires exactly two literal active references",
    )?;
    let updated = text.replace(&old, &new);
    need(
        spec_pin_text(&updated)? == latest,
        "updated spec.md active example pins",
    )?;
    need(
        updated.replace(&new, &old) == text,
        "spec.md bytes outside the two references remain exact",
    )?;
    Ok(updated)
}

fn promotion_plan(inputs: &[PinText], latest: Option<&str>) -> Result<PromotionPlan> {
    need(
        inputs.len() == PROMOTION_FILES.len(),
        "exactly four promotion operands required",
    )?;
    for (source, path) in inputs.iter().zip(PROMOTION_FILES) {
        need(
            source.path == path && source.bytes.len() <= MAX_PIN_INPUT_BYTES as usize,
            "ordered bounded promotion operands",
        )?;
    }
    let current = image_pin_custody::current_digest(&inputs[2].bytes)?;
    let latest = latest.unwrap_or(&current);
    digest_reference(&format!("{IMAGE}@{latest}"), "target digest")?;
    let mut outputs = Vec::new();
    for (index, source) in inputs.iter().enumerate() {
        let bytes = if index < 2 {
            replace_deployment(
                source.path,
                std::str::from_utf8(&source.bytes).map_err(|e| e.to_string())?,
                &current,
                latest,
            )?
            .into_bytes()
        } else if index == 2 {
            image_pin_custody::replace(&source.bytes, latest)?
        } else {
            replace_spec(
                std::str::from_utf8(&source.bytes).map_err(|e| e.to_string())?,
                &current,
                latest,
            )?
            .into_bytes()
        };
        outputs.push(PinText {
            path: source.path,
            bytes,
        });
    }
    Ok(PromotionPlan {
        originals: inputs.to_vec(),
        outputs,
        current: current.clone(),
        latest: latest.into(),
    })
}

/// Prepare all four operands before per-file atomic replacement. A failed
/// replacement reports every completed path and never claims set-wide atomicity.
#[cfg(unix)]
pub fn apply_promotion(root: &Path, new_digest: &str) -> Result<Vec<String>> {
    let snapshot = promotion_files::PinSnapshot::read(root)?;
    // Authenticate the runtime capsule and existing F01/W03 inverse as well as
    // the digest-slot template validated by the snapshot planner.
    let operand = crate::o02_health_retirement::deployment_source_operand(root)?;
    frozen_oracle_pin_text(std::str::from_utf8(&operand).map_err(|e| e.to_string())?)?;
    let plan = snapshot.plan(new_digest)?;
    snapshot
        .apply(&plan)
        .map(|paths| paths.into_iter().map(String::from).collect())
        .map_err(|error| format!("O02 image pin: {error}"))
}
#[cfg(not(unix))]
pub fn apply_promotion(_: &Path, _: &str) -> Result<Vec<String>> {
    Err("O02 image pin: promotion writes require Unix no-follow filesystem controls".into())
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
                .filter(|(tag, _)| matches!(tag.as_str(), "1.99.0-bookworm" | "1.99.1-bookworm"));
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

fn promotion_mode(args: &[String]) -> Result<(&'static str, Option<String>)> {
    let mut check = false;
    let mut write = false;
    let mut recover = false;
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
            "--recover" => {
                need(!recover, "duplicate --recover flag")?;
                recover = true;
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
        usize::from(check) + usize::from(write) + usize::from(recover) == 1,
        "image-pin-promote requires exactly one of --check, --write or --recover",
    )?;
    Ok((
        if check {
            "check"
        } else if write {
            "write"
        } else {
            "recover"
        },
        root,
    ))
}

/// Explicit offline recovery; --check never calls this mutating path.
#[cfg(unix)]
pub fn recover_promotion(root: &Path) -> Result<()> {
    let _guard = promotion_files::recover(root)?;
    Ok(())
}
#[cfg(not(unix))]
pub fn recover_promotion(_: &Path) -> Result<()> {
    Err("O02 image pin: recovery requires Unix filesystem controls".into())
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
    promote_with_transport(root, check, &CurlTransport, &mut real_sleep)
}
fn promote_with_transport(
    root: &Path,
    check: bool,
    transport: &dyn Transport,
    sleep: &mut dyn FnMut(u64),
) -> Result<i32> {
    if !check {
        recover_promotion(root)?;
    }
    let pinned = extract_pinned_digest(root)?;
    let latest = latest_digest(transport, sleep)?;
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
        "image-pin-promote" => command_exit_code((|| {
            let (mode, selected_root) = promotion_mode(rest)?;
            let effective =
                selected_root.map_or_else(|| root.to_path_buf(), std::path::PathBuf::from);
            if mode == "recover" {
                recover_promotion(&effective).map(|()| 0)
            } else {
                promote_with_mode(&effective, mode == "check")
            }
        })()),
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
    fn health_builder_patch_tags_remain_explicitly_reviewed() {
        let digest = "a".repeat(64);
        for tag in ["1.99.0-bookworm", "1.99.1-bookworm"] {
            let dockerfile = format!(
                "FROM rust:{tag}@sha256:{digest} AS healthcheck-builder\nFROM python:3.14-slim@sha256:{digest}\n"
            );
            let bases = pinned_bases(&dockerfile).expect("reviewed patch tag");
            assert_eq!(bases[0].1, tag);
        }
        for tag in [
            "1.99.2-bookworm",
            "1.100.0-bookworm",
            "1.99.1-alpine",
            "latest",
        ] {
            let dockerfile = format!(
                "FROM rust:{tag}@sha256:{digest} AS healthcheck-builder\nFROM python:3.14-slim@sha256:{digest}\n"
            );
            assert!(pinned_bases(&dockerfile).is_err(), "unreviewed tag: {tag}");
        }
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
        fs::write(root.join("spec.md"),format!("```yaml\nspec:\n  template:\n    spec:\n      containers:\n        - name: slopsearx\n          image: {manifest}\n```\n\n```yaml\nslopsearx:\n  image: {manifest}\n```\n")).unwrap();
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

    #[cfg(unix)]
    fn promotion_fixture(name: &str) -> FixtureDir {
        let root = fixture_root(name);
        write_pin_fixtures(&root, FROZEN_TEST_PIN_DIGEST);
        fs::write(
            root.join(TEST_PIN_FILE),
            include_bytes!("../../../tests/test_deployment_artifacts.py"),
        )
        .unwrap();
        let capsule = root.join("docs/rust-rewrite/o02/builder-patch-v2/capsule.json");
        fs::create_dir_all(capsule.parent().unwrap()).unwrap();
        fs::write(
            capsule,
            include_bytes!("../../../docs/rust-rewrite/o02/builder-patch-v2/capsule.json"),
        )
        .unwrap();
        root
    }

    #[test]
    #[cfg(unix)]
    fn native_write_updates_only_digest_slots_and_is_idempotent() {
        let root = promotion_fixture("apply");
        let original = fs::read(root.join(TEST_PIN_FILE)).unwrap();
        let new = format!("sha256:{}", "f".repeat(64));
        assert_eq!(apply_promotion(&root, &new).unwrap(), PROMOTION_FILES);
        assert_eq!(extract_pinned_digest(&root).unwrap(), new);
        let current = fs::read(root.join(TEST_PIN_FILE)).unwrap();
        assert_eq!(
            image_pin_custody::normalize(&current).unwrap(),
            image_pin_custody::normalize(&original).unwrap()
        );
        assert_eq!(apply_promotion(&root, &new).unwrap(), Vec::<String>::new());
        // Existing strict F01/W03 inverse accepts the parameterized successor.
        let operand = crate::o02_health_retirement::deployment_source_operand(&root).unwrap();
        assert_eq!(
            frozen_oracle_pin_text(std::str::from_utf8(&operand).unwrap()).unwrap(),
            FROZEN_TEST_PIN_DIGEST
        );
    }

    #[test]
    #[cfg(unix)]
    fn malformed_promotion_operands_and_targets_do_not_write() {
        for kind in 0..6 {
            let root = promotion_fixture("bad-plan");
            let oracle = root.join(TEST_PIN_FILE);
            let manifest = root.join(PIN_FILES[0]);
            let mut target = format!("sha256:{}", "f".repeat(64));
            match kind {
                0 => target = "sha256:nope".into(),
                1 => {
                    let mut raw = fs::read(&oracle).unwrap();
                    raw[0] ^= 1;
                    fs::write(&oracle, raw).unwrap();
                }
                2 => {
                    let text = fs::read_to_string(&manifest).unwrap();
                    fs::write(
                        &manifest,
                        format!("{text}\n# {IMAGE}@{FROZEN_TEST_PIN_DIGEST}\n"),
                    )
                    .unwrap();
                }
                3 => fs::write(&manifest, "services: {slopsearx: {image: latest}}\n").unwrap(),
                4 => fs::write(
                    &oracle,
                    format!("GHCR_DIGEST_PIN = \"{IMAGE}@{FROZEN_TEST_PIN_DIGEST}\"\n"),
                )
                .unwrap(),
                _ => {
                    let capsule = root.join("docs/rust-rewrite/o02/builder-patch-v2/capsule.json");
                    fs::write(capsule, "{}\n").unwrap();
                }
            }
            let before = PROMOTION_FILES.map(|p| fs::read(root.join(p)).unwrap());
            assert!(apply_promotion(&root, &target).is_err(), "case {kind}");
            for (path, bytes) in PROMOTION_FILES.into_iter().zip(before) {
                assert_eq!(
                    fs::read(root.join(path)).unwrap(),
                    bytes,
                    "case {kind}: {path}"
                );
            }
        }
    }

    #[test]
    fn replacement_proves_semantic_delta_and_rejects_alias_and_comment_decoys() {
        let old = format!("{IMAGE}@{FROZEN_TEST_PIN_DIGEST}");
        let new = format!("sha256:{}", "f".repeat(64));
        for text in [
            format!("pin: &pin {old}\nservices: {{slopsearx: {{image: *pin}}}}\n"),
            format!("# {old}\nservices: {{slopsearx: {{image: latest}}}}\n"),
            format!("services: {{slopsearx: {{image: {old}, image: latest}}}}\n"),
        ] {
            assert!(replace_deployment(PIN_FILES[0], &text, FROZEN_TEST_PIN_DIGEST, &new).is_err());
        }
        let text =
            format!("# retained comment\nservices:\n  slopsearx:\n    image: '{old}' # active\n");
        let output = replace_deployment(PIN_FILES[0], &text, FROZEN_TEST_PIN_DIGEST, &new).unwrap();
        assert_eq!(output, text.replace(FROZEN_TEST_PIN_DIGEST, &new));
    }

    #[test]
    #[cfg(unix)]
    fn promotion_snapshot_rejects_changed_source_and_cleans_staging_failures() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        let new = format!("sha256:{}", "f".repeat(64));
        for stage_failure in [true, false] {
            let root = promotion_fixture("stage-failure");
            let snapshot = PinSnapshot::read(&root).unwrap();
            let plan = snapshot.plan(&new).unwrap();
            let before = PROMOTION_FILES.map(|p| fs::read(root.join(p)).unwrap());
            let error = snapshot
                .apply_with_test_hook(&plan, |point, index, _, _| {
                    if stage_failure && point == ApplyPoint::BeforeStage && index == 2 {
                        return Err(std::io::Error::other("injected staging failure"));
                    }
                    if !stage_failure && point == ApplyPoint::BeforeReplace && index == 0 {
                        fs::write(root.join(PIN_FILES[0]), "changed concurrently\n")?;
                    }
                    Ok(())
                })
                .unwrap_err();
            assert!(error.replaced.is_empty());
            assert_eq!(error.cleanup_failed.is_empty(), stage_failure);
            drop(snapshot);
            if !stage_failure {
                assert!(extract_pinned_digest(&root).is_err());
                fs::write(root.join(PIN_FILES[0]), &before[0]).unwrap();
                recover_promotion(&root).unwrap();
                assert_eq!(
                    extract_pinned_digest(&root).unwrap(),
                    FROZEN_TEST_PIN_DIGEST
                );
            }
            for (index, (path, bytes)) in PROMOTION_FILES.into_iter().zip(before).enumerate() {
                if stage_failure || index != 0 {
                    assert_eq!(fs::read(root.join(path)).unwrap(), bytes);
                }
            }
            for dir in [root.to_path_buf(), root.join("k8s"), root.join("tests")] {
                assert!(fs::read_dir(dir).unwrap().all(|e| {
                    !e.unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".ssx-image-promotion")
                }));
            }
        }
    }

    #[test]
    #[cfg(unix)]
    fn promotion_preserves_an_edit_after_the_last_source_check() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        let root = promotion_fixture("late-source-edit");
        let original = root.join(PIN_FILES[0]);
        let saved = root.join("saved-original");
        let incoming = root.join("incoming-edit");
        let before = fs::read(&original).unwrap();
        let foreign = b"a concurrent editor's replacement";
        fs::write(&incoming, foreign).unwrap();
        let snapshot = PinSnapshot::read(&root).unwrap();
        let plan = snapshot
            .plan(&format!("sha256:{}", "f".repeat(64)))
            .unwrap();
        let error = snapshot
            .apply_with_test_hook(&plan, |point, index, _, _| {
                if point == ApplyPoint::BeforeRename && index == 0 {
                    fs::rename(&original, &saved)?;
                    fs::rename(&incoming, &original)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(error.replaced.is_empty());
        assert!(!error.cleanup_failed.is_empty());
        drop(snapshot);
        assert_eq!(fs::read(&original).unwrap(), foreign);
        assert_eq!(fs::read(&saved).unwrap(), before);
        assert!(recover_promotion(&root).is_err());
        assert_eq!(fs::read(&original).unwrap(), foreign);
        let preserved_edit = root.join("preserved-edit");
        fs::rename(&original, &preserved_edit).unwrap();
        fs::rename(&saved, &original).unwrap();
        recover_promotion(&root).unwrap();
        assert_eq!(fs::read(&preserved_edit).unwrap(), foreign);
        assert_eq!(fs::read(&original).unwrap(), before);
    }

    #[test]
    #[cfg(unix)]
    fn recovery_preserves_foreign_source_displaced_at_interruption() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        let root = promotion_fixture("foreign-displaced");
        let source = root.join(PIN_FILES[0]);
        let saved = root.join("saved-original");
        let incoming = root.join("incoming-edit");
        let before = fs::read(&source).unwrap();
        let foreign = b"a concurrent editor's replacement";
        fs::write(&incoming, foreign).unwrap();
        let snapshot = PinSnapshot::read(&root).unwrap();
        let plan = snapshot
            .plan(&format!("sha256:{}", "f".repeat(64)))
            .unwrap();
        let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = snapshot.apply_with_test_hook(&plan, |point, index, _, _| {
                if index == 0 && point == ApplyPoint::BeforeRename {
                    fs::rename(&source, &saved)?;
                    fs::rename(&incoming, &source)?;
                }
                if index == 0 && point == ApplyPoint::AfterExchange {
                    panic!("interrupted before verifying the displaced entry");
                }
                Ok(())
            });
        }));
        assert!(stopped.is_err());
        drop(snapshot);
        let journal = fs::read(root.join(".ssx-image-promotion/journal.json")).unwrap();
        let record: serde_json::Value = serde_json::from_slice(&journal).unwrap();
        let stage = source.with_file_name(record["stages"][0].as_str().unwrap());
        assert!(recover_promotion(&root).is_err());
        assert_eq!(fs::read(&stage).unwrap(), foreign);
        assert_eq!(fs::read(&source).unwrap(), plan.outputs[0].bytes);
        assert_eq!(fs::read(&saved).unwrap(), before);
        let preserved_edit = root.join("preserved-edit");
        fs::rename(&stage, &preserved_edit).unwrap();
        fs::rename(&saved, &stage).unwrap();
        recover_promotion(&root).unwrap();
        assert_eq!(fs::read(&source).unwrap(), before);
        assert_eq!(fs::read(&preserved_edit).unwrap(), foreign);
    }

    #[test]
    #[cfg(unix)]
    fn recovery_retires_the_empty_private_draft_after_interruption() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        let root = promotion_fixture("empty-private-draft");
        let before = PROMOTION_FILES.map(|path| fs::read(root.join(path)).unwrap());
        let unrelated = root.join(".ssx-image-promotion-unrelated.tmp");
        fs::write(&unrelated, []).unwrap();
        let snapshot = PinSnapshot::read(&root).unwrap();
        let plan = snapshot
            .plan(&format!("sha256:{}", "f".repeat(64)))
            .unwrap();
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = snapshot.apply_with_test_hook(&plan, |point, index, _, _| {
                if point == ApplyPoint::StageCreated && index == 0 {
                    panic!("interrupted after creation and before the first write");
                }
                Ok(())
            });
        }));
        assert!(interrupted.is_err());
        drop(snapshot);

        let directory = root.join(".ssx-image-promotion");
        let journal: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join("journal.json")).unwrap()).unwrap();
        let stage = journal["stages"][0].as_str().unwrap();
        let draft = directory.join(format!("{stage}.draft"));
        assert_eq!(fs::metadata(&draft).unwrap().len(), 0);
        assert!(!root.join(PROMOTION_FILES[0]).with_file_name(stage).exists());

        recover_promotion(&root).unwrap();
        recover_promotion(&root).unwrap();
        assert!(!directory.exists());
        assert_eq!(fs::read(&unrelated).unwrap(), b"");
        assert_eq!(
            PROMOTION_FILES.map(|path| fs::read(root.join(path)).unwrap()),
            before
        );
    }

    #[test]
    #[cfg(unix)]
    fn promotion_rolls_back_every_rename_boundary_and_retains_failed_rollback() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        let new = format!("sha256:{}", "f".repeat(64));
        for boundary in 0..=PROMOTION_FILES.len() {
            for fail_rollback in [false, true] {
                let root = promotion_fixture("rollback");
                let before = PROMOTION_FILES.map(|p| fs::read(root.join(p)).unwrap());
                let snapshot = PinSnapshot::read(&root).unwrap();
                let plan = snapshot.plan(&new).unwrap();
                let error = snapshot
                    .apply_with_test_hook(&plan, |point, index, _, _| {
                        if (point == ApplyPoint::BeforeRename && index == boundary)
                            || (point == ApplyPoint::BeforeCommit
                                && boundary == PROMOTION_FILES.len())
                            || (point == ApplyPoint::BeforeRollback && fail_rollback)
                        {
                            return Err(std::io::Error::other("injected failure"));
                        }
                        Ok(())
                    })
                    .unwrap_err();
                drop(snapshot);
                let retained = fail_rollback && boundary > 0;
                assert_eq!(
                    !error.cleanup_failed.is_empty(),
                    retained,
                    "boundary {boundary}"
                );
                assert_eq!(
                    root.join(".ssx-image-promotion/journal.json").exists(),
                    retained
                );
                if retained {
                    let journal = fs::read(root.join(".ssx-image-promotion/journal.json")).unwrap();
                    let state = PROMOTION_FILES.map(|p| fs::read(root.join(p)).unwrap());
                    assert!(extract_pinned_digest(&root).is_err());
                    assert_eq!(
                        fs::read(root.join(".ssx-image-promotion/journal.json")).unwrap(),
                        journal
                    );
                    for (path, bytes) in PROMOTION_FILES.into_iter().zip(state) {
                        assert_eq!(fs::read(root.join(path)).unwrap(), bytes);
                    }
                    // Compensation uses the original preserved by exchange.
                    // An unknown entry at that name must block recovery and survive it.
                    let record: serde_json::Value = serde_json::from_slice(&journal).unwrap();
                    let displaced = root
                        .join(PROMOTION_FILES[0])
                        .with_file_name(record["stages"][0].as_str().unwrap());
                    assert_eq!(fs::read(&displaced).unwrap(), before[0]);
                    let unrelated = b"foreign staging file";
                    fs::write(&displaced, unrelated).unwrap();
                    assert!(recover_promotion(&root).is_err());
                    assert_eq!(fs::read(&displaced).unwrap(), unrelated);
                    assert_eq!(
                        fs::read(root.join(".ssx-image-promotion/journal.json")).unwrap(),
                        journal
                    );
                    fs::write(&displaced, &before[0]).unwrap();
                    recover_promotion(&root).unwrap();
                    assert!(!displaced.exists());
                }
                assert_eq!(
                    extract_pinned_digest(&root).unwrap(),
                    FROZEN_TEST_PIN_DIGEST
                );
                for (path, bytes) in PROMOTION_FILES.into_iter().zip(before) {
                    assert_eq!(fs::read(root.join(path)).unwrap(), bytes);
                }
                assert!(!root.join(".ssx-image-promotion").exists());
            }
        }
    }

    #[test]
    #[cfg(unix)]
    fn interrupted_transaction_requires_explicit_recovery_before_check() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        let new = format!("sha256:{}", "f".repeat(64));
        for boundary in 0..=PROMOTION_FILES.len() {
            let root = promotion_fixture("interrupted");
            let before = PROMOTION_FILES.map(|p| fs::read(root.join(p)).unwrap());
            let snapshot = PinSnapshot::read(&root).unwrap();
            let plan = snapshot.plan(&new).unwrap();
            let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = snapshot.apply_with_test_hook(&plan, |point, index, _, _| {
                    if (point == ApplyPoint::BeforeRename && index == boundary)
                        || (point == ApplyPoint::BeforeCommit && boundary == PROMOTION_FILES.len())
                    {
                        panic!("simulated process interruption after {boundary} renames");
                    }
                    Ok(())
                });
            }));
            assert!(interrupted.is_err());
            assert!(root.join(".ssx-image-promotion/journal.json").exists());
            drop(snapshot);
            let journal = fs::read(root.join(".ssx-image-promotion/journal.json")).unwrap();
            let state = PROMOTION_FILES.map(|p| fs::read(root.join(p)).unwrap());
            assert!(extract_pinned_digest(&root).is_err());
            assert_eq!(
                fs::read(root.join(".ssx-image-promotion/journal.json")).unwrap(),
                journal
            );
            for (path, bytes) in PROMOTION_FILES.into_iter().zip(state) {
                assert_eq!(fs::read(root.join(path)).unwrap(), bytes);
            }
            // Simulate leftover output files from a hard interruption (unlike
            // unwinding, SIGKILL would not run Stage::drop).
            use std::os::unix::fs::PermissionsExt;
            let record: serde_json::Value = serde_json::from_slice(&journal).unwrap();
            let stages: Vec<_> = PROMOTION_FILES
                .into_iter()
                .zip(record["stages"].as_array().unwrap())
                .map(|(path, name)| {
                    root.join(path)
                        .parent()
                        .unwrap()
                        .join(name.as_str().unwrap())
                })
                .collect();
            let unrelated = b"foreign staging data";
            fs::write(&stages[0], unrelated).unwrap();
            fs::set_permissions(&stages[0], fs::Permissions::from_mode(0o600)).unwrap();
            assert!(recover_promotion(&root).is_err());
            assert_eq!(fs::read(&stages[0]).unwrap(), unrelated);
            assert_eq!(
                fs::read(root.join(".ssx-image-promotion/journal.json")).unwrap(),
                journal
            );
            for (index, (stage, document)) in stages
                .iter()
                .zip(record["documents"].as_array().unwrap())
                .enumerate()
            {
                if index < boundary {
                    // Put back the displaced original that would survive a crash.
                    if index == 0 {
                        fs::write(stage, &before[index]).unwrap();
                        fs::set_permissions(stage, fs::Permissions::from_mode(0o644)).unwrap();
                    }
                    assert_eq!(fs::read(stage).unwrap(), before[index]);
                } else {
                    let expected = document["updated"].as_str().unwrap().as_bytes();
                    fs::write(stage, &expected[..16]).unwrap();
                    fs::set_permissions(stage, fs::Permissions::from_mode(0o644)).unwrap();
                    assert!(recover_promotion(&root).is_err());
                    assert_eq!(fs::read(stage).unwrap(), &expected[..16]);
                    fs::write(stage, expected).unwrap();
                }
            }
            recover_promotion(&root).unwrap();
            if boundary % 2 == 0 {
                assert_eq!(
                    extract_pinned_digest(&root).unwrap(),
                    FROZEN_TEST_PIN_DIGEST
                );
            } else {
                assert!(
                    apply_promotion(&root, FROZEN_TEST_PIN_DIGEST)
                        .unwrap()
                        .is_empty()
                );
            }
            for (path, bytes) in PROMOTION_FILES.into_iter().zip(before) {
                assert_eq!(fs::read(root.join(path)).unwrap(), bytes);
            }
            assert!(!root.join(".ssx-image-promotion").exists());
        }
    }

    #[test]
    #[cfg(unix)]
    fn every_publication_and_cleanup_boundary_is_recoverable() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        let points = [
            ApplyPoint::InitCreated,
            ApplyPoint::InitWritten,
            ApplyPoint::InitSynced,
            ApplyPoint::InitRenamed,
            ApplyPoint::InitPublished,
            ApplyPoint::CommitCreated,
            ApplyPoint::CommitWritten,
            ApplyPoint::CommitSynced,
            ApplyPoint::CommitRenamed,
            ApplyPoint::CleanupReceiptCreated,
            ApplyPoint::CleanupReceiptWritten,
            ApplyPoint::CleanupReceiptSynced,
            ApplyPoint::CleanupReceiptPublished,
            ApplyPoint::CleanupMarkersRemoved,
            ApplyPoint::CleanupDirectoryRemoved,
        ];
        let new = format!("sha256:{}", "f".repeat(64));
        for point in points {
            let root = promotion_fixture("publication-crash");
            let snapshot = PinSnapshot::read(&root).unwrap();
            let plan = snapshot.plan(&new).unwrap();
            let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = snapshot.apply_with_test_hook(&plan, |at, _, _, _| {
                    if at == point {
                        if at == ApplyPoint::CommitCreated || at == ApplyPoint::CommitWritten {
                            let journal =
                                fs::read(root.join(".ssx-image-promotion/journal.json")).unwrap();
                            fs::write(
                                root.join(".ssx-image-promotion/commit.pending"),
                                &journal[..17],
                            )
                            .unwrap();
                        }
                        panic!("injected publication/cleanup interruption");
                    }
                    Ok(())
                });
            }));
            assert!(interrupted.is_err());
            drop(snapshot);
            let state = PROMOTION_FILES.map(|p| fs::read(root.join(p)).unwrap());
            let _ = extract_pinned_digest(&root);
            for (path, raw) in PROMOTION_FILES.into_iter().zip(state) {
                assert_eq!(fs::read(root.join(path)).unwrap(), raw);
            }
            recover_promotion(&root).unwrap();
            recover_promotion(&root).unwrap();
            let committed = matches!(
                point,
                ApplyPoint::CommitRenamed
                    | ApplyPoint::CleanupReceiptCreated
                    | ApplyPoint::CleanupReceiptWritten
                    | ApplyPoint::CleanupReceiptSynced
                    | ApplyPoint::CleanupReceiptPublished
                    | ApplyPoint::CleanupMarkersRemoved
                    | ApplyPoint::CleanupDirectoryRemoved
            );
            assert_eq!(
                extract_pinned_digest(&root).unwrap(),
                if committed {
                    new.clone()
                } else {
                    FROZEN_TEST_PIN_DIGEST.into()
                }
            );
            assert!(!root.join(".ssx-image-promotion").exists());
            assert!(!root.join(".ssx-image-promotion-cleanup.json").exists());
        }
    }

    #[test]
    #[cfg(unix)]
    fn commit_errors_compensate_only_before_marker_publication() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        let new = format!("sha256:{}", "f".repeat(64));
        for point in [
            ApplyPoint::CommitCreated,
            ApplyPoint::CommitWritten,
            ApplyPoint::CommitSynced,
            ApplyPoint::CommitRenamed,
        ] {
            let root = promotion_fixture("commit-error");
            let before = PROMOTION_FILES.map(|path| fs::read(root.join(path)).unwrap());
            let snapshot = PinSnapshot::read(&root).unwrap();
            let plan = snapshot.plan(&new).unwrap();
            let mut rollback_calls = 0;
            let error = snapshot
                .apply_with_test_hook(&plan, |at, _, _, _| {
                    if at == ApplyPoint::BeforeRollback && point == ApplyPoint::CommitRenamed {
                        rollback_calls += 1;
                        return Err(std::io::Error::other("injected rollback refusal"));
                    }
                    if at == point {
                        return Err(std::io::Error::other("commit I/O failure"));
                    }
                    Ok(())
                })
                .unwrap_err();
            let published = point == ApplyPoint::CommitRenamed;
            assert_eq!(!error.replaced.is_empty(), published);
            assert_eq!(!error.cleanup_failed.is_empty(), published);
            assert_eq!(rollback_calls, 0);
            drop(snapshot);
            recover_promotion(&root).unwrap();
            for (path, raw) in PROMOTION_FILES.into_iter().zip(before) {
                if published {
                    assert_eq!(
                        fs::read(root.join(path)).unwrap(),
                        plan.outputs.iter().find(|p| p.path == path).unwrap().bytes
                    );
                } else {
                    assert_eq!(fs::read(root.join(path)).unwrap(), raw);
                }
            }
        }
    }

    #[test]
    #[cfg(unix)]
    fn reserved_publication_never_replaces_a_foreign_artifact() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        use std::os::unix::fs::PermissionsExt;
        let new = format!("sha256:{}", "f".repeat(64));
        for point in [
            ApplyPoint::JournalBeforePublish,
            ApplyPoint::InitBeforePublish,
            ApplyPoint::CommitBeforePublish,
            ApplyPoint::CleanupBeforePublish,
        ] {
            let root = promotion_fixture("foreign-publication");
            let before = PROMOTION_FILES.map(|p| fs::read(root.join(p)).unwrap());
            let snapshot = PinSnapshot::read(&root).unwrap();
            let plan = snapshot.plan(&new).unwrap();
            let mut foreign = None;
            let result = snapshot.apply_with_test_hook(&plan, |at, _, _, _| {
                if at == point {
                    let path = match point {
                        ApplyPoint::JournalBeforePublish => fs::read_dir(&*root)
                            .unwrap()
                            .map(|e| e.unwrap().path())
                            .find(|p| {
                                p.file_name()
                                    .unwrap()
                                    .to_string_lossy()
                                    .starts_with(".ssx-image-promotion-init-")
                            })
                            .unwrap()
                            .join("journal.json"),
                        ApplyPoint::InitBeforePublish => root.join(".ssx-image-promotion"),
                        ApplyPoint::CommitBeforePublish => {
                            root.join(".ssx-image-promotion/committed")
                        }
                        _ => root.join(".ssx-image-promotion-cleanup.json"),
                    };
                    if point == ApplyPoint::InitBeforePublish {
                        fs::create_dir(&path).unwrap();
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                    } else {
                        fs::write(&path, b"foreign artifact").unwrap();
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                    }
                    foreign = Some(path);
                }
                Ok(())
            });
            drop(snapshot);
            let foreign = foreign.unwrap();
            if point == ApplyPoint::InitBeforePublish {
                assert!(foreign.is_dir());
                assert_eq!(fs::read_dir(&foreign).unwrap().count(), 0);
            } else {
                assert_eq!(fs::read(&foreign).unwrap(), b"foreign artifact");
            }
            if point == ApplyPoint::CleanupBeforePublish {
                assert!(result.is_ok());
            } else {
                assert!(result.is_err());
            }
            if matches!(
                point,
                ApplyPoint::JournalBeforePublish | ApplyPoint::InitBeforePublish
            ) {
                for (path, raw) in PROMOTION_FILES.into_iter().zip(before) {
                    assert_eq!(fs::read(root.join(path)).unwrap(), raw);
                }
            } else {
                assert!(recover_promotion(&root).is_err());
                assert_eq!(fs::read(&foreign).unwrap(), b"foreign artifact");
            }
        }
    }

    #[test]
    #[cfg(unix)]
    fn direct_write_recovers_before_extraction_without_mutating_check() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        let root = promotion_fixture("direct-write");
        let new = format!("sha256:{}", "f".repeat(64));
        let snapshot = PinSnapshot::read(&root).unwrap();
        let plan = snapshot.plan(&new).unwrap();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = snapshot.apply_with_test_hook(&plan, |point, index, _, _| {
                if point == ApplyPoint::BeforeRename && index == 1 {
                    panic!("interrupt");
                }
                Ok(())
            });
        }));
        drop(snapshot);
        let check = Fake::new(vec![]);
        assert!(promote_with_transport(&root, true, &check, &mut dismiss).is_err());
        assert!(check.calls.borrow().is_empty());
        let transport = Fake::new(vec![
            Ok(response(200, None, r#"{"token":"t"}"#)),
            Ok(response(
                200,
                Some(("Docker-Content-Digest", FROZEN_TEST_PIN_DIGEST)),
                "",
            )),
        ]);
        assert_eq!(
            promote_with_transport(&root, false, &transport, &mut dismiss).unwrap(),
            0
        );
        assert_eq!(
            extract_pinned_digest(&root).unwrap(),
            FROZEN_TEST_PIN_DIGEST
        );
    }

    #[test]
    fn promotion_cli_subprocess_helper() {
        if let Ok(raw) = std::env::var("SLOPSEARX_O02_INVALID_ARGS") {
            let mut args = vec!["image-pin-promote".to_string()];
            args.extend(raw.split_whitespace().map(String::from));
            cli(Path::new(env!("CARGO_MANIFEST_DIR")), &args).unwrap();
            panic!("invalid modes must exit before returning");
        }
    }

    #[test]
    fn invalid_modes_exit_two_in_the_real_cli_dispatch() {
        for args in [
            "",
            "--check --write",
            "--check --recover",
            "--check --surprise",
            "--write --root",
        ] {
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "o06::o02_image_pin::tests::promotion_cli_subprocess_helper",
                    "--nocapture",
                ])
                .env("SLOPSEARX_O02_INVALID_ARGS", args)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2), "args {args:?}: {output:?}");
        }
    }

    #[test]
    #[cfg(unix)]
    fn inert_initialization_and_full_root_scan_do_not_block_recovery() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        let root = promotion_fixture("inert-init-scan");
        let snapshot = PinSnapshot::read(&root).unwrap();
        let plan = snapshot
            .plan(&format!("sha256:{}", "f".repeat(64)))
            .unwrap();
        snapshot
            .apply_with_test_hook(&plan, |point, index, _, _| {
                if (point == ApplyPoint::BeforeRename && index == 1)
                    || point == ApplyPoint::BeforeRollback
                {
                    return Err(std::io::Error::other("injected interruption"));
                }
                Ok(())
            })
            .unwrap_err();
        drop(snapshot);
        let journal = root.join(".ssx-image-promotion/journal.json");
        assert!(journal.is_file());
        let inert = root.join(".ssx-image-promotion-init-inert");
        fs::create_dir(&inert).unwrap();
        for index in 0..=4096 {
            fs::write(root.join(format!("unrelated-{index}")), b"").unwrap();
        }

        let recovered = recover_promotion(&root);
        assert!(
            inert.is_dir(),
            "an empty initialization has no journal authority"
        );
        for index in 0..=4096 {
            let unrelated = root.join(format!("unrelated-{index}"));
            assert_eq!(fs::read(&unrelated).unwrap(), b"");
            fs::remove_file(unrelated).unwrap();
        }
        recovered.unwrap();
        assert!(!journal.exists());
        assert_eq!(
            extract_pinned_digest(&root).unwrap(),
            FROZEN_TEST_PIN_DIGEST
        );
    }

    #[test]
    #[cfg(unix)]
    fn recovery_preserves_unknown_or_invalid_transaction_material() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        for kind in 0..7 {
            let root = promotion_fixture("unknown-transaction");
            let transaction = root.join(".ssx-image-promotion");
            fs::create_dir(&transaction).unwrap();
            fs::set_permissions(&transaction, fs::Permissions::from_mode(0o700)).unwrap();
            let pending = transaction.join("journal.pending");
            let external = root.join("unrelated-original");
            fs::write(&external, b"user data").unwrap();
            match kind {
                0 => {}
                1 => fs::write(transaction.join("unrelated"), b"user data").unwrap(),
                2 => fs::write(&pending, b"{broken journal").unwrap(),
                3 => symlink(&external, &pending).unwrap(),
                4 => fs::hard_link(&external, &pending).unwrap(),
                5 => {
                    fs::write(&pending, b"invalid data").unwrap();
                    fs::set_permissions(&pending, fs::Permissions::from_mode(0o644)).unwrap();
                }
                _ => fs::set_permissions(&transaction, fs::Permissions::from_mode(0o755)).unwrap(),
            }
            let before = PROMOTION_FILES.map(|path| fs::read(root.join(path)).unwrap());
            let names: Vec<_> = fs::read_dir(&transaction)
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            assert!(extract_pinned_digest(&root).is_err());
            assert!(recover_promotion(&root).is_err(), "case {kind}");
            assert!(transaction.exists());
            assert_eq!(fs::read(&external).unwrap(), b"user data");
            assert_eq!(
                fs::read_dir(&transaction)
                    .unwrap()
                    .map(|e| e.unwrap().file_name())
                    .collect::<Vec<_>>(),
                names
            );
            for (path, raw) in PROMOTION_FILES.into_iter().zip(before) {
                assert_eq!(fs::read(root.join(path)).unwrap(), raw);
            }
            if kind == 3 {
                fs::remove_file(&pending).unwrap();
            }
            fs::set_permissions(&transaction, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }

    #[test]
    #[cfg(unix)]
    fn authenticated_pending_journal_recovers_without_inheriting_name_authority() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        let root = promotion_fixture("pending");
        let new = format!("sha256:{}", "f".repeat(64));
        let snapshot = PinSnapshot::read(&root).unwrap();
        let plan = snapshot.plan(&new).unwrap();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = snapshot.apply_with_test_hook(&plan, |point, _, _, _| {
                if point == ApplyPoint::BeforeStage {
                    panic!("interrupted with durable originals");
                }
                Ok(())
            });
        }));
        drop(snapshot);
        let directory = root.join(".ssx-image-promotion");
        fs::rename(
            directory.join("journal.json"),
            directory.join("journal.pending"),
        )
        .unwrap();
        recover_promotion(&root).unwrap();
        assert!(!directory.exists());
        assert_eq!(
            extract_pinned_digest(&root).unwrap(),
            FROZEN_TEST_PIN_DIGEST
        );
    }

    #[test]
    #[cfg(unix)]
    fn committed_transaction_cleanup_preserves_successful_pins() {
        use promotion_files::{ApplyPoint, PinSnapshot};
        let root = promotion_fixture("committed");
        let new = format!("sha256:{}", "f".repeat(64));
        let snapshot = PinSnapshot::read(&root).unwrap();
        let plan = snapshot.plan(&new).unwrap();
        let changed = snapshot
            .apply_with_test_hook(&plan, |point, _, _, _| {
                if point == ApplyPoint::AfterCommit {
                    return Err(std::io::Error::other("injected cleanup failure"));
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(changed, PROMOTION_FILES);
        drop(snapshot);
        assert!(root.join(".ssx-image-promotion/committed").exists());
        assert!(extract_pinned_digest(&root).is_err());
        recover_promotion(&root).unwrap();
        assert_eq!(extract_pinned_digest(&root).unwrap(), new);
        assert!(!root.join(".ssx-image-promotion").exists());
        recover_promotion(&root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn promotion_rejects_symlinks_and_wrong_modes() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let root = promotion_fixture("write-links");
        let path = root.join(PIN_FILES[0]);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(apply_promotion(&root, FROZEN_TEST_PIN_DIGEST).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let saved = root.join("saved-compose");
        fs::rename(&path, &saved).unwrap();
        symlink(&saved, &path).unwrap();
        assert!(apply_promotion(&root, FROZEN_TEST_PIN_DIGEST).is_err());
        fs::remove_file(&path).unwrap();
        fs::rename(&saved, &path).unwrap();
    }

    #[test]
    fn promotion_workflow_uses_native_modes_and_rejects_unexpected_exits() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let text = fs::read_to_string(root.join(".github/workflows/image-promotion.yml")).unwrap();
        let workflow = parse_yaml(&text, "promotion workflow").unwrap();
        let ledger: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("docs/rust-rewrite/ci/gates.json")).unwrap(),
        )
        .unwrap();
        let entry = &ledger["gates"]["image-promotion.yml:promote"]["source"];
        assert_eq!(entry["job"], workflow["jobs"]["promote"]);
        let steps = workflow["jobs"]["promote"]["steps"].as_array().unwrap();
        let recovery = steps
            .iter()
            .position(|step| {
                step["run"]
                    .as_str()
                    .is_some_and(|run| run == "cargo xtask ci-helper image-pin-promote --recover")
            })
            .unwrap();
        let check = steps
            .iter()
            .position(|step| {
                step["run"]
                    .as_str()
                    .is_some_and(|run| run.contains("image-pin-promote --check"))
            })
            .unwrap();
        let write = steps
            .iter()
            .position(|step| {
                step["run"]
                    .as_str()
                    .is_some_and(|run| run.contains("image-pin-promote --write --root ."))
            })
            .unwrap();
        assert!(recovery < check && check < write);
        assert!(!text.contains("scripts/promote_image_digest.py"));
        let fixture = fixture_root("workflow-exits");
        for (index, accepted) in [(check, [0, 1]), (write, [0, 10])] {
            let run = steps[index]["run"].as_str().unwrap();
            for code in [0, 1, 2, 10, 101, 130] {
                let output = Command::new("bash")
                    .args(["-ec", &format!("cargo() {{ if [ \"$4\" = --recover ]; then return 0; fi; return {code}; }}\n{run}")])
                    .env("GITHUB_OUTPUT", fixture.join("output"))
                    .output()
                    .unwrap();
                assert_eq!(
                    output.status.success(),
                    accepted.contains(&code),
                    "mode {index}, exit {code}: {:?}",
                    output
                );
            }
        }
    }

    #[test]
    fn promotion_workflow_blocks_all_pending_transaction_artifacts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let workflow = parse_yaml(
            &fs::read_to_string(root.join(".github/workflows/image-promotion.yml")).unwrap(),
            "workflow",
        )
        .unwrap();
        let run = workflow["jobs"]["promote"]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|step| step["id"] == "promote")
            .unwrap()["run"]
            .as_str()
            .unwrap();
        assert!(run.contains("cargo xtask ci-helper image-pin-promote --recover"));
        for (name, create) in [
            (".ssx-image-promotion", "mkdir"),
            (".ssx-image-promotion-cleanup.json", "touch"),
            (".ssx-image-promotion-cleanup.pending", "touch"),
        ] {
            let fixture = fixture_root("publication-block");
            let script = format!(
                "cargo() {{ if [ \"$4\" = --write ]; then {create} {name}; return 10; fi; return 0; }}\n{run}"
            );
            let output = Command::new("bash")
                .args(["-ec", &script])
                .current_dir(&*fixture)
                .env("GITHUB_OUTPUT", fixture.join("output"))
                .output()
                .unwrap();
            assert!(!output.status.success(), "{name}");
            assert!(!fixture.join("output").exists());
        }
    }

    #[test]
    fn actual_four_operands_plan_and_custody_spans_remain_exact() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let inputs: Vec<PinText> = PROMOTION_FILES
            .into_iter()
            .map(|path| PinText {
                path,
                bytes: fs::read(root.join(path)).unwrap(),
            })
            .collect();
        let new = format!("sha256:{}", "f".repeat(64));
        let plan = promotion_plan(&inputs, Some(&new)).unwrap();
        assert_eq!(plan.outputs.len(), 4);
        assert_eq!(
            spec_pin_text(std::str::from_utf8(&plan.outputs[3].bytes).unwrap()).unwrap(),
            new
        );
        let normalized = image_pin_custody::normalize(&plan.outputs[2].bytes).unwrap();
        assert_eq!(normalized.len(), 13732);
        assert_eq!(
            Sha256::digest(&normalized)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            "e9d53620f211325296cea8f3723c11a013621a8eb7fc642e077f06bf235035b8"
        );
        let text = std::str::from_utf8(&inputs[2].bytes).unwrap();
        let assertion =
            "    assert \"cargo xtask ci-helper image-pin-promote --write\" in step_text\n";
        assert_eq!(text.matches(assertion).count(), 1);
        assert!(image_pin_custody::normalize(text.replace(assertion, "").as_bytes()).is_err());
        assert!(
            image_pin_custody::normalize(
                text.replace("not in step_text", "in step_text").as_bytes()
            )
            .is_err()
        );
        let spec = std::str::from_utf8(&inputs[3].bytes).unwrap();
        for invalid in [
            format!("{spec}\n# {IMAGE}@{FROZEN_TEST_PIN_DIGEST}\n"),
            spec.replacen(FROZEN_TEST_PIN_DIGEST, &new, 1),
        ] {
            assert!(spec_pin_text(&invalid).is_err());
        }
    }

    #[test]
    fn promotion_requires_exactly_one_explicit_mode() {
        assert!(promotion_mode(&[]).is_err());
        assert!(promotion_mode(&["--check".into(), "--write".into()]).is_err());
        assert_eq!(promotion_mode(&["--check".into()]).unwrap().0, "check");
        assert_eq!(promotion_mode(&["--write".into()]).unwrap().0, "write");
        assert_eq!(promotion_mode(&["--recover".into()]).unwrap().0, "recover");
        assert!(promotion_mode(&["--recover".into(), "--check".into()]).is_err());
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
