//! Source MCP/HTTPX redirect and configured URL authentication rules.
use crate::Result;
use base64::Engine;
/// Build HTTPX's candidate before source MCP's method/origin follow predicate.
pub(crate) fn location_url(
    current: &url::Url,
    status: reqwest::StatusCode,
    location: Option<&reqwest::header::HeaderValue>,
) -> Option<url::Url> {
    if !matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
        return None;
    }
    current.join(location?.to_str().ok()?).ok()
}
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
    let location = location_url(current, status, location)?;
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

// Full source-owner lane. The legacy functions above remain until Network's
// separately reviewed adoption replaces every ordinary Request/Response caller.
use reqwest::source_pool::{SourceUrl, SourceUrlChanges};

/// HTTPX chooses one encoding for the complete header collection, then joins
/// duplicate Location fields. A single unrelated invalid UTF-8 value therefore
/// selects Latin-1 for Location too. Header names are already ASCII in HeaderMap.
pub(crate) fn source_location_text(headers: &reqwest::header::HeaderMap) -> Option<String> {
    let utf8 = headers
        .values()
        .all(|value| std::str::from_utf8(value.as_bytes()).is_ok());
    let values: Vec<String> = headers
        .get_all(reqwest::header::LOCATION)
        .iter()
        .map(|value| {
            if utf8 {
                std::str::from_utf8(value.as_bytes())
                    .expect("all header values checked")
                    .to_owned()
            } else {
                value.as_bytes().iter().copied().map(char::from).collect()
            }
        })
        .collect();
    (!values.is_empty()).then(|| values.join(", "))
}

/// Build the actual HTTPX candidate before cookies and the MCP follow predicate.
/// Invalid Location URLs and lazy display-host errors propagate; they are not
/// converted into an absent redirect. SourceUrl Debug never reveals URL data.
pub(crate) fn source_location_url(
    current: &SourceUrl,
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
) -> Result<Option<SourceUrl>> {
    if !matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
        return Ok(None);
    }
    let Some(location) = source_location_text(headers) else {
        return Ok(None);
    };
    let mut candidate = SourceUrl::parse(&location)?;
    if !candidate.scheme().is_empty() && candidate.display_host()?.is_empty() {
        let host = current.display_host()?;
        candidate = candidate.copy_with(SourceUrlChanges {
            host: Some(&host),
            ..Default::default()
        })?;
    }
    if !candidate.is_absolute() {
        candidate = current.join(&candidate.serialized())?;
    }
    let fragment = current.fragment();
    if !fragment.is_empty() && candidate.fragment().is_empty() {
        candidate = candidate.copy_with(SourceUrlChanges {
            fragment: Some(Some(&fragment)),
            ..Default::default()
        })?;
    }
    Ok(Some(candidate))
}

/// Match the pinned HTTPX method rewrite, including QUERY's 302 exception and
/// methods such as DELETE that retain their method on a 301.
pub(crate) fn source_redirect_method(
    method: &reqwest::Method,
    status: reqwest::StatusCode,
) -> reqwest::Method {
    let code = status.as_u16();
    if (code == 303 && *method != reqwest::Method::HEAD)
        || (code == 302 && *method != reqwest::Method::HEAD && method.as_str() != "QUERY")
        || (code == 301 && *method == reqwest::Method::POST)
    {
        reqwest::Method::GET
    } else {
        method.clone()
    }
}

/// This is the MCP follow gate, applied after candidate request preparation.
/// A Location that clears old userinfo is allowed. Nonempty new userinfo must
/// equal the original quoted bytes, rather than merely decode to equal strings.
pub(crate) fn source_follows(
    method: &reqwest::Method,
    current: &SourceUrl,
    candidate: &SourceUrl,
    status: reqwest::StatusCode,
) -> Result<bool> {
    if source_redirect_method(method, status) != *method
        || (!candidate.raw_userinfo().is_empty()
            && candidate.raw_userinfo() != current.raw_userinfo())
    {
        return Ok(false);
    }
    // Python constructs both tuples eagerly, including each lazy display host.
    let current_host = current.display_host()?;
    let candidate_host = candidate.display_host()?;
    let same = (current.scheme(), current_host.as_str(), current.port())
        == (
            candidate.scheme(),
            candidate_host.as_str(),
            candidate.port(),
        );
    let upgrade = current_host == candidate_host
        && current.scheme() == "http"
        && current.port().is_none()
        && candidate.scheme() == "https"
        && candidate.port().is_none();
    Ok(same || upgrade)
}

#[cfg(test)]
mod source_owner_tests {
    use super::*;
    fn candidate(current: &SourceUrl, location: &str) -> SourceUrl {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::LOCATION,
            reqwest::header::HeaderValue::from_bytes(location.as_bytes()).unwrap(),
        );
        source_location_url(current, reqwest::StatusCode::TEMPORARY_REDIRECT, &headers)
            .unwrap()
            .unwrap()
    }
    #[test]
    fn source_redirect_preserves_empty_query_and_inherits_truthy_fragment() {
        let current = SourceUrl::parse("http://a.test/path?old#ke%65p").unwrap();
        let next = candidate(&current, "/next?#");
        assert_eq!(next.query_component(), Some(""));
        assert_eq!(next.fragment(), "keep");
        assert_eq!(next.raw_path(), "/next?");
    }
    #[test]
    fn source_redirect_scheme_without_host_uses_display_host() {
        let current = SourceUrl::parse("http://xn--bcher-kva.example/start").unwrap();
        let next = candidate(&current, "https:/next");
        assert_eq!(next.scheme(), "https");
        assert_eq!(next.display_host().unwrap(), "bücher.example");
        assert_eq!(next.path(), "/next");
    }
    #[test]
    fn source_redirect_method_gate_uses_actual_rewrite_rules() {
        let current = SourceUrl::parse("http://a.test/start").unwrap();
        let next = candidate(&current, "/next");
        for (method, code, follows) in [
            ("DELETE", 301, true),
            ("PUT", 301, true),
            ("POST", 301, false),
            ("QUERY", 302, true),
            ("DELETE", 302, false),
            ("HEAD", 303, true),
            ("POST", 307, true),
            ("POST", 308, true),
        ] {
            assert_eq!(
                source_follows(
                    &reqwest::Method::from_bytes(method.as_bytes()).unwrap(),
                    &current,
                    &next,
                    reqwest::StatusCode::from_u16(code).unwrap()
                )
                .unwrap(),
                follows,
                "{method}/{code}"
            );
        }
    }
    #[test]
    fn source_redirect_userinfo_compares_raw_bytes_and_allows_clear() {
        let current = SourceUrl::parse("http://us%65r:pass@a.test/start").unwrap();
        for (location, follows) in [
            ("http://a.test/next", true),
            ("/next", true),
            ("http://user:pass@a.test/next", false),
        ] {
            let next = candidate(&current, location);
            assert_eq!(
                source_follows(
                    &reqwest::Method::GET,
                    &current,
                    &next,
                    reqwest::StatusCode::FOUND
                )
                .unwrap(),
                follows
            );
        }
    }
    #[test]
    fn source_redirect_uses_source_ports_and_default_upgrade() {
        let current = SourceUrl::parse("http://a.test:80/start").unwrap();
        let next = candidate(&current, "https://a.test:443/next");
        assert!(
            source_follows(
                &reqwest::Method::GET,
                &current,
                &next,
                reqwest::StatusCode::FOUND
            )
            .unwrap()
        );
        let zero = SourceUrl::parse("http://a.test:0/start").unwrap();
        assert!(
            !source_follows(
                &reqwest::Method::GET,
                &zero,
                &next,
                reqwest::StatusCode::FOUND
            )
            .unwrap()
        );
    }
    #[test]
    fn source_redirect_header_encoding_is_collection_wide_and_duplicates_join() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.append(
            reqwest::header::LOCATION,
            reqwest::header::HeaderValue::from_bytes("/café".as_bytes()).unwrap(),
        );
        assert_eq!(source_location_text(&headers).unwrap(), "/café");
        headers.insert(
            "x-other",
            reqwest::header::HeaderValue::from_bytes(&[255]).unwrap(),
        );
        assert_eq!(source_location_text(&headers).unwrap(), "/cafÃ©");
        headers.append(
            reqwest::header::LOCATION,
            reqwest::header::HeaderValue::from_static("/next"),
        );
        assert_eq!(source_location_text(&headers).unwrap(), "/cafÃ©, /next");
    }
    #[test]
    fn source_redirect_invalid_location_propagates_but_nonredirect_ignores_it() {
        let current = SourceUrl::parse("http://a.test/start").unwrap();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::LOCATION,
            reqwest::header::HeaderValue::from_static("http://[invalid/"),
        );
        assert!(source_location_url(&current, reqwest::StatusCode::FOUND, &headers).is_err());
        assert!(
            source_location_url(&current, reqwest::StatusCode::OK, &headers)
                .unwrap()
                .is_none()
        );
    }
}
