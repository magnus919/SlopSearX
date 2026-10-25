//! Pinned CookieJar observations kept separate from the transport URL.
//! Routing still uses the gateway's existing Url parser; raw routing/IDNA is
//! a separate unfinished contract. Cookie observation never normalizes a host.
use crate::header::{HeaderMap, HeaderValue};
#[derive(Clone)]
/// Request URL and header observations used by the pinned cookie policy.
pub struct SourceCookieView {
    scheme: String,
    authority: String,
    path: String,
    headers: HeaderMap,
}
impl std::fmt::Debug for SourceCookieView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceCookieView").finish_non_exhaustive()
    }
}
impl SourceCookieView {
    /// Observe the HTTPX request URL before configured userinfo is removed.
    pub fn parse(raw: &str, headers: &HeaderMap) -> crate::Result<Self> {
        let (scheme, rest) = raw
            .split_once(':')
            .ok_or_else(|| crate::error::builder("cookie URL type absent"))?;
        if scheme.is_empty() {
            return Err(crate::error::builder("cookie URL type absent"));
        }
        let (authority, path) = if let Some(rest) = rest.strip_prefix("//") {
            let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
            (&rest[..end], &rest[end..])
        } else {
            ("", rest)
        };
        let path = path.split(['?', '#']).next().unwrap_or_default();
        Ok(Self {
            // HTTPX normalizes the ASCII scheme before cookie policy sees it.
            // Keep authority, userinfo and path observations byte-for-byte.
            scheme: scheme.to_ascii_lowercase(),
            authority: authority.into(),
            path: path.into(),
            headers: headers.clone(),
        })
    }
    pub(super) fn has_cookie_header(&self) -> bool {
        self.headers.contains_key(http::header::COOKIE)
    }
    /// Return the ASCII-lowercased scheme.
    pub fn scheme(&self) -> &str {
        &self.scheme
    }
    /// Return the lowercased request-host observation.
    pub fn request_host(&self) -> crate::Result<String> {
        let host = if self.authority.is_empty() {
            let utf8 = self
                .headers
                .values()
                .all(|v| std::str::from_utf8(v.as_bytes()).is_ok());
            self.headers
                .get(http::header::HOST)
                .map(|v| {
                    if utf8 {
                        std::str::from_utf8(v.as_bytes())
                            .expect("checked headers")
                            .into()
                    } else {
                        v.as_bytes().iter().copied().map(char::from).collect()
                    }
                })
                .unwrap_or_default()
        } else {
            self.authority.clone()
        };
        Ok(super::source_lower(cut_decimal_port(&host)))
    }
    /// Return the request host, adding `.local` when it contains no dot.
    pub fn effective_host(&self) -> crate::Result<String> {
        let host = self.request_host()?;
        Ok(if host.contains('.') {
            host
        } else {
            format!("{host}.local")
        })
    }
    /// Return the escaped, slash-prefixed request path.
    pub fn request_path(&self) -> crate::Result<String> {
        let mut p = escape_path(&self.path);
        if !p.starts_with('/') {
            p.insert(0, '/');
        }
        Ok(p)
    }
    /// Return the source-valid port observation, defaulting to `80` when absent.
    /// Return `None` when the observed port is invalid.
    pub fn request_port(&self) -> Option<String> {
        let bytes = self.authority.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                if let (Some(a), Some(b)) = (
                    (bytes[i + 1] as char).to_digit(16),
                    (bytes[i + 2] as char).to_digit(16),
                ) {
                    out.push((a * 16 + b) as u8);
                    i += 3;
                    continue;
                }
            }
            out.push(bytes[i]);
            i += 1;
        }
        let host = String::from_utf8_lossy(&out);
        if let Some((_, port)) = host.split_once(':') {
            python_integer(port).then(|| port.into())
        } else {
            Some("80".into())
        }
    }
}
fn cut_decimal_port(host: &str) -> &str {
    match host.rsplit_once(':') {
        Some((head, tail)) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => host,
    }
}
pub(super) fn python_whitespace(c: char) -> bool {
    matches!(u32::from(c), 9..=13 | 28..=32 | 133 | 160 | 5760 | 8192..=8202 | 8232 | 8233 | 8239 | 8287 | 12288)
}
pub(super) fn python_integer(text: &str) -> bool {
    let text = text.trim_matches(|c: char| matches!(c, '\u{9}'..='\u{d}' | '\u{1c}'..='\u{20}' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'));
    let digits = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    let mut previous = false;
    let mut count = 0;
    for c in digits.chars() {
        if c == '_' {
            if !previous {
                return false;
            }
            previous = false;
        } else if super::python_text::decimal_digit(c).is_some() {
            count += 1;
            previous = true;
        } else {
            return false;
        }
    }
    previous && count <= 4300
}
pub(super) fn escape_path(path: &str) -> String {
    let mut out = String::new();
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            out.push('%');
            out.push(char::from(bytes[i + 1].to_ascii_uppercase()));
            out.push(char::from(bytes[i + 2].to_ascii_uppercase()));
            i += 3;
            continue;
        }
        if b.is_ascii_alphanumeric() || b"_-.%/;:@&=+$,!~*'()".contains(&b) {
            out.push(char::from(b));
        } else {
            use std::fmt::Write;
            write!(out, "%{b:02X}").expect("String formatting is infallible");
        }
        i += 1;
    }
    out
}
/// Explicit native store interface; the ordinary CookieStore API is unchanged.
/// Implementations must use this source view, never reconstruct a url::Url.
pub trait SourceCookieStore: Send + Sync {
    /// Includes expired entries until the source header operation purges them.
    fn has_cookies(&self) -> bool;
    /// Generate at the source request stage; existing caller Cookie wins.
    fn cookies(&self, view: &SourceCookieView) -> crate::Result<Option<HeaderValue>>;
    /// Process response headers before redirect request construction.
    fn extract(&self, view: &SourceCookieView, headers: &HeaderMap) -> crate::Result<()>;
}
/// Source cookie wall-clock observations, independently injected for controls.
pub trait SourceCookieClock: Send + Sync {
    /// Floating wall seconds, before source int()/floor conversion at each stage.
    fn now(&self) -> crate::Result<f64>;
    /// Independently observed local-current year, with platform TZ/error semantics.
    fn local_year(&self) -> crate::Result<i32>;
}
/// Native platform clock; no timezone parser/cache or UTC fallback.
#[derive(Clone, Copy, Debug, Default)]
pub struct SourceCookieSystemClock;
impl SourceCookieClock for SourceCookieSystemClock {
    fn now(&self) -> crate::Result<f64> {
        super::source_cookie_clock::seconds_double().map_err(crate::error::request)
    }
    fn local_year(&self) -> crate::Result<i32> {
        super::source_cookie_clock::local_year().map_err(crate::error::request)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cookie_observations_preserve_raw_userinfo_path_port_and_local_host() {
        let view = SourceCookieView::parse(
            "http://u:p@exa%6dple.test:70000/a%2f?ignored#fragment",
            &HeaderMap::new(),
        )
        .unwrap();
        assert_eq!(view.request_host().unwrap(), "u:p@exa%6dple.test");
        assert_eq!(view.request_path().unwrap(), "/a%2F");
        assert_eq!(view.request_port(), None);
        let view = SourceCookieView::parse("https://host:00080/a", &HeaderMap::new()).unwrap();
        assert_eq!(view.effective_host().unwrap(), "host.local");
        assert_eq!(view.request_port().as_deref(), Some("00080"));
        let view = SourceCookieView::parse("https://host/a", &HeaderMap::new()).unwrap();
        assert_eq!(view.request_port().as_deref(), Some("80"));
        assert!(!format!("{view:?}").contains("host"));
    }
    #[test]
    fn cookie_integer_language_uses_frozen_decimal_digits_and_underscores() {
        for text in [" +١_٢ ", "３４", "00080"] {
            assert!(python_integer(text));
        }
        for text in ["_1", "1__2", "1_", "1.0", ""] {
            assert!(!python_integer(text));
        }
    }
}

#[cfg(test)]
mod scheme_controls {
    use super::*;
    #[test]
    fn cookie_scheme_is_ascii_lowercase_without_normalizing_authority() {
        for (raw, expected) in [("HTTPS", "https"), ("WsS", "wss"), ("HTTP", "http")] {
            let view = SourceCookieView::parse(
                &format!("{raw}://User:Pass@ExA%6dple.test:00080/a%2f"),
                &HeaderMap::new(),
            )
            .unwrap();
            assert_eq!(
                view.scheme(),
                expected,
                "configured scheme normalization mutant"
            );
            assert_eq!(view.authority, "User:Pass@ExA%6dple.test:00080");
            assert_eq!(view.path, "/a%2f");
        }
    }
}
