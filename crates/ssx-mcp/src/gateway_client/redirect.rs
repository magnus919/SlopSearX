//! Source MCP/HTTPX redirect and configured URL authentication rules.
use crate::Result;
use base64::Engine;
pub fn next_url(
    method: &reqwest::Method,
    current: &url::Url,
    status: reqwest::StatusCode,
    location: Option<&reqwest::header::HeaderValue>,
) -> Option<url::Url> {
    if !matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
        return None;
    }
    if !matches!(status.as_u16(), 307 | 308)
        && *method != reqwest::Method::GET
        && *method != reqwest::Method::HEAD
    {
        return None;
    }
    let location = current.join(location?.to_str().ok()?).ok()?;
    let same = current.scheme() == location.scheme()
        && current.host_str() == location.host_str()
        && current.port() == location.port();
    let upgrade = current.scheme() == "http"
        && location.scheme() == "https"
        && current.port().is_none()
        && location.port().is_none()
        && current.host_str() == location.host_str();
    if (!same && !upgrade)
        || location.username() != current.username()
        || location.password() != current.password()
    {
        return None;
    }
    Some(location)
}
fn unquote(value: &str) -> String {
    let mut bytes = Vec::new();
    let source = value.as_bytes();
    let mut i = 0;
    while i < source.len() {
        if source[i] == b'%'
            && i + 2 < source.len()
            && let (Some(a), Some(b)) = (
                (source[i + 1] as char).to_digit(16),
                (source[i + 2] as char).to_digit(16),
            )
        {
            bytes.push(((a << 4) | b) as u8);
            i += 3;
            continue;
        }
        bytes.push(source[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
pub fn wire_url(
    url: &str,
    oauth: bool,
    headers: &mut reqwest::header::HeaderMap,
) -> Result<String> {
    let mut parsed = url::Url::parse(url)?;
    if !oauth && (!parsed.username().is_empty() || parsed.password().is_some()) {
        let value = format!(
            "{}:{}",
            unquote(parsed.username()),
            unquote(parsed.password().unwrap_or_default())
        );
        let mut value = reqwest::header::HeaderValue::from_str(&format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(value)
        ))?;
        value.set_sensitive(true);
        headers.insert(reqwest::header::AUTHORIZATION, value);
    }
    parsed
        .set_username("")
        .map_err(|_| "gateway URL username cannot be cleared")?;
    parsed
        .set_password(None)
        .map_err(|_| "gateway URL password cannot be cleared")?;
    parsed.set_fragment(None);
    Ok(parsed.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_redirects_keep_method_origin_and_userinfo() {
        let start = url::Url::parse("http://a.test/mcp").unwrap();
        let location = reqwest::header::HeaderValue::from_static("/mcp/");
        assert!(
            next_url(
                &reqwest::Method::POST,
                &start,
                reqwest::StatusCode::TEMPORARY_REDIRECT,
                Some(&location)
            )
            .is_some()
        );
        assert!(
            next_url(
                &reqwest::Method::POST,
                &start,
                reqwest::StatusCode::FOUND,
                Some(&location)
            )
            .is_none()
        );
        let other = reqwest::header::HeaderValue::from_static("http://b.test/mcp");
        assert!(
            next_url(
                &reqwest::Method::GET,
                &start,
                reqwest::StatusCode::FOUND,
                Some(&other)
            )
            .is_none()
        );
        let secret = reqwest::header::HeaderValue::from_static("http://user:secret@a.test/mcp");
        assert!(
            next_url(
                &reqwest::Method::GET,
                &start,
                reqwest::StatusCode::FOUND,
                Some(&secret)
            )
            .is_none()
        );
        let upgrade = reqwest::header::HeaderValue::from_static("https://a.test/mcp");
        assert!(
            next_url(
                &reqwest::Method::POST,
                &start,
                reqwest::StatusCode::PERMANENT_REDIRECT,
                Some(&upgrade)
            )
            .is_some()
        );
    }
    #[test]
    fn source_static_url_basic_overrides_bearer_but_oauth_does_not_use_url_basic() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_static("Bearer static"),
        );
        assert_eq!(
            wire_url(
                "http://us%65r:p%2Bass@a.test/mcp?q=1#fragment",
                false,
                &mut headers
            )
            .unwrap(),
            "http://a.test/mcp?q=1"
        );
        assert_eq!(
            headers[reqwest::header::AUTHORIZATION],
            "Basic dXNlcjpwK2Fzcw=="
        );
        headers.clear();
        let _ = wire_url("http://user:pass@a.test/mcp#fragment", true, &mut headers).unwrap();
        assert!(!headers.contains_key(reqwest::header::AUTHORIZATION));
    }
}
