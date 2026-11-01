//! Narrow native image operations. No arbitrary registry, credentials or deployment.
use sha2::{Digest, Sha256};
use ssx_infra::http::{
    AdapterHttp, Bounds, Cancellation, Error as HttpError, Options, Request, Response,
};
use ssx_json::Value;
use std::{
    collections::BTreeSet,
    fs::File,
    io::{self, Read},
    path::Path,
    time::Duration,
};
use tokio::time::Instant;
use url::Url;
const TOKEN_BYTES: usize = 64 * 1024;
const MANIFEST_BYTES: usize = 1024 * 1024;
const TOKEN_LENGTH: usize = 16 * 1024;
const ACCEPT: &str = "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.docker.distribution.manifest.v2+json";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
    pub repository: &'static str,
    pub tag: String,
    pub digest: String,
}
#[derive(Debug)]
pub struct Observation {
    pub base: Base,
    pub actual: String,
}
pub fn bounded_file(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operational input too large",
        ));
    }
    Ok(bytes)
}
fn fail(message: &str) -> String {
    message.to_owned()
}
pub fn digest_valid(text: &str) -> bool {
    text.strip_prefix("sha256:").is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn base_ref(text: &str, image: &str) -> Result<Base, String> {
    let (reference, digest) = text
        .split_once('@')
        .ok_or_else(|| fail("base must pin tag and digest"))?;
    let (name, tag) = reference
        .split_once(':')
        .ok_or_else(|| fail("base tag missing"))?;
    if !name.eq_ignore_ascii_case(image)
        || !digest_valid(digest)
        || tag.len() > 128
        || tag.is_empty()
        || !tag.bytes().enumerate().all(|(i, b)| {
            b.is_ascii_alphanumeric() || b == b'_' || (i > 0 && matches!(b, b'.' | b'-'))
        })
    {
        return Err(fail("invalid recognized base reference"));
    }
    Ok(Base {
        repository: if image == "rust" {
            "library/rust"
        } else {
            "library/python"
        },
        tag: tag.into(),
        digest: digest.into(),
    })
}
pub fn pinned_bases(source: &str) -> Result<Vec<Base>, String> {
    let lines: Vec<Vec<&str>> = source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let first = line.get(..4)?;
            (first.eq_ignore_ascii_case("FROM")
                && line
                    .as_bytes()
                    .get(4)
                    .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_'))
            .then(|| line.split_ascii_whitespace().collect())
        })
        .collect();
    match lines.as_slice() {
        [runtime]
            if runtime.len() == 2
                || (runtime.len() == 4 && runtime[2].eq_ignore_ascii_case("AS")) =>
        {
            Ok(vec![base_ref(runtime[1], "python")?])
        }
        [builder, runtime]
            if builder.len() == 4
                && builder[2].eq_ignore_ascii_case("AS")
                && builder[3].eq_ignore_ascii_case("healthcheck-builder")
                && runtime.len() == 2 =>
        {
            let builder = base_ref(builder[1], "rust")?;
            if builder.tag != "1.99.0-bookworm" {
                return Err(fail("unrecognized health builder tag"));
            }
            Ok(vec![builder, base_ref(runtime[1], "python")?])
        }
        _ => Err(fail("unrecognized or additional Docker FROM stages")),
    }
}
pub fn verify_platforms(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MANIFEST_BYTES {
        return Err(fail("manifest too large"));
    }
    let value: Value = ssx_json::from_slice(bytes).map_err(|_| fail("manifest is not JSON"))?;
    let rows = value["manifests"]
        .as_array()
        .ok_or_else(|| fail("manifest index missing"))?;
    let mut platforms = BTreeSet::new();
    for row in rows {
        let os = row["platform"]["os"]
            .as_str()
            .ok_or_else(|| fail("manifest platform OS missing"))?;
        let arch = row["platform"]["architecture"]
            .as_str()
            .ok_or_else(|| fail("manifest platform architecture missing"))?;
        platforms.insert((os, arch));
    }
    if platforms != BTreeSet::from([("linux", "amd64"), ("linux", "arm64")]) {
        return Err(fail("manifest must contain exactly Linux amd64 and arm64"));
    }
    Ok(())
}
fn token_request(base: &Base) -> Result<Request, String> {
    let mut url = Url::parse("https://auth.docker.io/token")
        .map_err(|_| fail("invalid fixed token endpoint"))?;
    url.query_pairs_mut()
        .append_pair("service", "registry.docker.io")
        .append_pair("scope", &format!("repository:{}:pull", base.repository));
    let mut request = request(url.as_str(), None)?;
    request.headers.insert(
        "accept",
        "application/json"
            .parse()
            .map_err(|_| fail("invalid token media type"))?,
    );
    Ok(request)
}
fn manifest_request(base: &Base, token: &str) -> Result<Request, String> {
    request(
        &format!(
            "https://registry-1.docker.io/v2/{}/manifests/{}",
            base.repository, base.tag
        ),
        Some(token),
    )
}
fn request(url: &str, token: Option<&str>) -> Result<Request, String> {
    let mut r = Request::get(url).map_err(|_| fail("invalid fixed registry request"))?;
    r.headers.insert(
        "accept",
        ACCEPT
            .parse()
            .map_err(|_| fail("invalid manifest media types"))?,
    );
    r.headers.insert(
        "accept-encoding",
        "identity"
            .parse()
            .map_err(|_| fail("invalid encoding policy"))?,
    );
    if let Some(token) = token {
        r.headers.insert(
            "authorization",
            format!("Bearer {token}")
                .parse()
                .map_err(|_| fail("invalid registry token"))?,
        );
    }
    Ok(r)
}
async fn exchange(owner: &AdapterHttp, request: Request, limit: usize) -> Result<Response, String> {
    for attempt in 0..3 {
        let options = Options {
            trust_env: false,
            follow_redirects: false,
            bounds: Bounds {
                wire_bytes: limit,
                decoded_bytes: limit,
                request_bytes: 32 * 1024,
                requests: 1,
                redirects: 0,
            },
            identity_announced_bytes: Some(limit),
            ..Options::default()
        };
        let mut session = owner
            .session(
                options,
                Instant::now() + Duration::from_secs(30),
                Cancellation::default(),
            )
            .map_err(|_| fail("registry session unavailable"))?;
        let result = session.execute(request.clone()).await;
        match result {
            Ok(response) if (200..300).contains(&response.status) => {
                if response.url != request.url
                    || response
                        .headers
                        .get("content-encoding")
                        .is_some_and(|v| v.as_bytes() != b"identity")
                {
                    return Err(fail("registry endpoint or encoding changed"));
                }
                return Ok(response);
            }
            Ok(response)
                if matches!(response.status, 429 | 500 | 502 | 503 | 504) && attempt < 2 => {}
            Err(HttpError::Network | HttpError::Deadline) if attempt < 2 => {}
            _ => return Err(fail("registry lookup failed")),
        }
        tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
    }
    Err(fail("registry lookup failed"))
}
pub async fn verify_bases(owner: &AdapterHttp, bases: &[Base]) -> Result<Vec<Observation>, String> {
    let mut observations = Vec::new();
    for base in bases {
        // Base values are parsed again at the boundary; callers cannot select arbitrary repositories.
        let image = match base.repository {
            "library/rust" => "rust",
            "library/python" => "python",
            _ => return Err(fail("unrecognized registry repository")),
        };
        if base_ref(&format!("{image}:{}@{}", base.tag, base.digest), image)? != *base {
            return Err(fail("invalid base"));
        }
        let token_response = exchange(owner, token_request(base)?, TOKEN_BYTES).await?;
        let token = registry_token(&token_response)?;
        let response = exchange(owner, manifest_request(base, &token)?, MANIFEST_BYTES).await?;
        let actual = manifest_digest(&response)?;
        observations.push(Observation {
            base: base.clone(),
            actual,
        });
    }
    Ok(observations)
}

#[cfg(test)]
mod tests;

fn registry_token(response: &Response) -> Result<String, String> {
    let value: Value =
        ssx_json::from_slice(&response.body).map_err(|_| fail("invalid token response"))?;
    value["token"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= TOKEN_LENGTH)
        .map(str::to_owned)
        .ok_or_else(|| fail("invalid registry token"))
}
fn manifest_digest(response: &Response) -> Result<String, String> {
    let mut headers = response.headers.get_all("docker-content-digest").iter();
    let actual = headers
        .next()
        .and_then(|h| h.to_str().ok())
        .filter(|s| digest_valid(s))
        .ok_or_else(|| fail("invalid registry manifest digest"))?;
    if headers.next().is_some() {
        return Err(fail("ambiguous registry manifest digest"));
    }
    let actual_body = format!(
        "sha256:{}",
        Sha256::digest(&response.body)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    if actual != actual_body {
        return Err(fail("manifest bytes do not match registry digest"));
    }
    Ok(actual.to_owned())
}
pub mod promotion;
