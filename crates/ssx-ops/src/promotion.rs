//! Fixed application data-pin promotion. Planning and registry resolution are portable;
//! Unix filesystem application is separate and never mutates a registry.
use crate::{
    AdapterHttp, MANIFEST_BYTES, TOKEN_BYTES, exchange, manifest_digest, registry_token, request,
};
#[cfg(not(unix))]
use std::path::Path;
use url::Url;
pub const IMAGE: &str = "ghcr.io/magnus919/slopsearx";
pub const PIN_PATHS: [&str; 3] = ["docker-compose.yml", "k8s/deployment.yaml", "spec.md"];
pub const FILE_BYTES: usize = 1024 * 1024;
const GHCR_ACCEPT: &str = "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinText {
    pub path: &'static str,
    pub bytes: Vec<u8>,
}
#[derive(Debug)]
pub struct PromotionPlan {
    originals: Vec<PinText>,
    outputs: Vec<PinText>,
    counts: [usize; 3],
    current: String,
    latest: String,
}
impl PromotionPlan {
    pub fn originals(&self) -> &[PinText] {
        &self.originals
    }
    pub fn outputs(&self) -> &[PinText] {
        &self.outputs
    }
    pub fn counts(&self) -> &[usize; 3] {
        &self.counts
    }
    pub fn current(&self) -> &str {
        &self.current
    }
    pub fn latest(&self) -> &str {
        &self.latest
    }
    pub fn changed(&self) -> bool {
        self.current != self.latest
    }
}
fn pins(bytes: &[u8]) -> Result<Vec<&str>, String> {
    if bytes.len() > FILE_BYTES {
        return Err("pin source exceeds its byte bound".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "pin source is not UTF-8")?;
    let prefix = format!("{IMAGE}@");
    let mut found = Vec::new();
    for (at, _) in text.match_indices(&prefix) {
        let tail = &text[at + prefix.len()..];
        let end = tail
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '\'' | '"' | ')' | ']' | ',' | ';' | '`')
            })
            .unwrap_or(tail.len());
        let digest = &tail[..end];
        if !crate::digest_valid(digest) {
            return Err("malformed fixed-image digest reference".into());
        }
        found.push(digest);
    }
    Ok(found)
}
fn source_state(inputs: &[PinText]) -> Result<(String, [usize; 3]), String> {
    if inputs.len() != PIN_PATHS.len() {
        return Err("exactly three pin data sources are required".into());
    }
    let mut current: Option<String> = None;
    let mut counts = [0; 3];
    for (i, (source, expected)) in inputs.iter().zip(PIN_PATHS).enumerate() {
        if source.path != expected {
            return Err("pin source paths or order differ".into());
        }
        let found = pins(&source.bytes)?;
        if found.len() < if i == 2 { 2 } else { 1 } {
            return Err(format!("required pin references missing: {expected}"));
        }
        counts[i] = found.len();
        for digest in found {
            match &current {
                Some(value) if value != digest => {
                    return Err("conflicting application image pins".into());
                }
                None => current = Some(digest.to_owned()),
                _ => {}
            }
        }
    }
    Ok((current.ok_or("application pin missing")?, counts))
}
pub fn plan(inputs: &[PinText], latest: &str) -> Result<PromotionPlan, String> {
    if !crate::digest_valid(latest) {
        return Err("invalid resolved application digest".into());
    }
    let (current, counts) = source_state(inputs)?;
    let old = format!("{IMAGE}@{current}");
    let new = format!("{IMAGE}@{latest}");
    let mut outputs = Vec::with_capacity(3);
    for (i, source) in inputs.iter().enumerate() {
        let original = std::str::from_utf8(&source.bytes).map_err(|_| "pin source is not UTF-8")?;
        let updated = original.replace(&old, &new);
        let references = pins(updated.as_bytes())?;
        if references.len() != counts[i]
            || references.iter().any(|value| *value != latest)
            || updated.replace(&new, &old) != original
        {
            return Err("pin replacement did not preserve exact source/counts".into());
        }
        outputs.push(PinText {
            path: source.path,
            bytes: updated.into_bytes(),
        });
    }
    Ok(PromotionPlan {
        originals: inputs.to_vec(),
        outputs,
        counts,
        current,
        latest: latest.to_owned(),
    })
}
pub async fn resolve_latest(owner: &AdapterHttp) -> Result<String, String> {
    let mut endpoint =
        Url::parse("https://ghcr.io/token").map_err(|_| "invalid fixed GHCR endpoint")?;
    endpoint
        .query_pairs_mut()
        .append_pair("scope", "repository:magnus919/slopsearx:pull");
    let mut token_request = request(endpoint.as_str(), None)?;
    token_request.headers.insert(
        "accept",
        "application/json"
            .parse()
            .map_err(|_| "invalid token media type")?,
    );
    let token_response = exchange(owner, token_request, TOKEN_BYTES).await?;
    let token = registry_token(&token_response)?;
    let mut manifest = request(
        "https://ghcr.io/v2/magnus919/slopsearx/manifests/latest",
        Some(&token),
    )?;
    manifest.headers.insert(
        "accept",
        GHCR_ACCEPT
            .parse()
            .map_err(|_| "invalid GHCR media types")?,
    );
    let response = exchange(owner, manifest, MANIFEST_BYTES).await?;
    manifest_digest(&response)
}
#[derive(Debug)]
pub struct ApplyError {
    pub message: &'static str,
    pub replaced: Vec<&'static str>,
    pub cleanup_failed: Vec<&'static str>,
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
impl std::error::Error for ApplyError {}
#[cfg(unix)]
mod files;
#[cfg(unix)]
pub use files::PinSnapshot;
#[cfg(not(unix))]
pub struct PinSnapshot;
#[cfg(not(unix))]
impl PinSnapshot {
    pub fn read(_: &Path) -> Result<Self, String> {
        Err("application pin filesystem qualification requires Unix".into())
    }
    pub fn plan(&self, _: &str) -> Result<PromotionPlan, String> {
        Err("application pin filesystem qualification requires Unix".into())
    }
    pub fn apply(&self, _: &PromotionPlan) -> Result<Vec<&'static str>, ApplyError> {
        Err(ApplyError {
            message: "application pin filesystem qualification requires Unix",
            replaced: vec![],
            cleanup_failed: vec![],
        })
    }
}
#[cfg(test)]
mod tests;
