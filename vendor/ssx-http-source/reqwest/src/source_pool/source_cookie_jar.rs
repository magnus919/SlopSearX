//! Pinned default CookieJar policy/state; ordinary Reqwest CookieStore is separate.
use super::{
    source_cookie_date::SourceCookieDateError,
    source_cookie_headers::{self as headers, Attribute, Pairs},
    source_cookies::{
        self, SourceCookieClock, SourceCookieStore, SourceCookieSystemClock, SourceCookieView,
    },
};
use crate::header::{HeaderMap, HeaderValue};
use num_bigint::BigInt;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};
#[derive(Clone)]
struct Cookie {
    name: String,
    value: Option<String>,
    version: Option<BigInt>,
    domain: String,
    domain_specified: bool,
    path: String,
    port: Option<String>,
    port_specified: bool,
    secure: bool,
    expires: Option<BigInt>,
}
#[derive(Clone, Default)]
struct State {
    domains: Vec<(String, Vec<(String, Vec<Cookie>)>)>,
}
impl State {
    fn insert(&mut self, cookie: Cookie) {
        let di = match self.domains.iter().position(|(d, _)| *d == cookie.domain) {
            Some(i) => i,
            None => {
                self.domains.push((cookie.domain.clone(), Vec::new()));
                self.domains.len() - 1
            }
        };
        let paths = &mut self.domains[di].1;
        let pi = match paths.iter().position(|(p, _)| *p == cookie.path) {
            Some(i) => i,
            None => {
                paths.push((cookie.path.clone(), Vec::new()));
                paths.len() - 1
            }
        };
        let names = &mut paths[pi].1;
        if let Some(i) = names.iter().position(|c| c.name == cookie.name) {
            names[i] = cookie;
        } else {
            names.push(cookie);
        }
    }
    fn clear(&mut self, domain: &str, path: &str, name: &str) {
        if let Some((_, paths)) = self.domains.iter_mut().find(|(d, _)| d == domain) {
            if let Some((_, names)) = paths.iter_mut().find(|(p, _)| p == path) {
                names.retain(|c| c.name != name);
            }
        }
        // Source clear(domain,path,name) retains empty parent dictionaries.
    }
    fn purge(&mut self, now: &BigInt) {
        for (_, paths) in &mut self.domains {
            for (_, names) in paths {
                names.retain(|c| c.expires.as_ref().is_none_or(|e| e > now));
            }
        }
    }
    fn has_cookies(&self) -> bool {
        self.domains
            .iter()
            .any(|(_, p)| p.iter().any(|(_, n)| !n.is_empty()))
    }
}
/// Native pinned-default source jar. It stores every accepted record without an
/// added domain/count cap; pool budgets do not claim to pay for this state.
pub struct SourceCookieJar {
    state: Mutex<State>,
    clock: Arc<dyn SourceCookieClock>,
}
impl std::fmt::Debug for SourceCookieJar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceCookieJar").finish_non_exhaustive()
    }
}
impl Default for SourceCookieJar {
    fn default() -> Self {
        Self::with_clock(Arc::new(SourceCookieSystemClock))
    }
}
impl SourceCookieJar {
    /// Explicit trusted clock seam for deterministic source-cookie controls.
    pub fn with_clock(clock: Arc<dyn SourceCookieClock>) -> Self {
        Self {
            state: Mutex::new(State::default()),
            clock,
        }
    }
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn integer_now(&self, floor: bool) -> crate::Result<BigInt> {
        let t = self.clock.now()?;
        if !t.is_finite() || t < i64::MIN as f64 || t >= -(i64::MIN as f64) {
            return Err(crate::error::request(
                "source cookie clock integer conversion failed",
            ));
        }
        Ok(BigInt::from(if floor {
            t.floor() as i64
        } else {
            t.trunc() as i64
        }))
    }
    fn create_batch(
        &self,
        state: &mut State,
        pairs: Vec<Pairs>,
        view: &SourceCookieView,
        now: &BigInt,
        netscape: bool,
    ) -> Result<Vec<Cookie>, SourceCookieDateError> {
        // CPython normalizes the entire attrs set before constructing any
        // Cookie. A TypeError in a later tuple must prevent earlier expiry
        // deletion; construction failures retain their separate source stage.
        let normalized = pairs
            .into_iter()
            .map(|pairs| normalize(pairs, now))
            .collect::<Result<Vec<_>, SourceCookieDateError>>()?;
        let mut result = Vec::new();
        for (name, value, standard) in normalized.into_iter().flatten() {
            let version = match standard.get("version") {
                Some(Standard::Text(Some(v))) => match integer(v) {
                    Some(v) => Some(v),
                    None => continue,
                },
                _ => None,
            };
            let path = match standard.get("path") {
                Some(Standard::Text(Some(p))) if !p.is_empty() => source_cookies::escape_path(p),
                _ => {
                    let mut p = view.request_path().map_err(|_| SourceCookieDateError)?;
                    if let Some(i) = p.rfind('/') {
                        p.truncate(i + usize::from(version.as_ref() != Some(&BigInt::from(0))));
                    }
                    if p.is_empty() {
                        p.push('/');
                    }
                    p
                }
            };
            let domain_specified = standard.contains_key("domain");
            let domain = match standard.get("domain") {
                Some(Standard::Text(Some(d))) => {
                    if d.starts_with('.') {
                        d.clone()
                    } else {
                        format!(".{d}")
                    }
                }
                _ => view.effective_host().map_err(|_| SourceCookieDateError)?,
            };
            let (port, port_specified) = match standard.get("port") {
                Some(Standard::Text(Some(p))) => (
                    Some(
                        p.chars()
                            .filter(|c| !source_cookies::python_whitespace(*c))
                            .collect(),
                    ),
                    true,
                ),
                Some(Standard::Text(None)) => (view.request_port(), false),
                _ => (None, false),
            };
            let expires = match standard.get("expires") {
                Some(Standard::Expires(e)) => {
                    if e <= now {
                        state.clear(&domain, &path, &name);
                        continue;
                    }
                    Some(float_rounded_integer(e)?)
                }
                Some(Standard::WrongExpiry) => return Err(SourceCookieDateError),
                _ => None,
            };
            let secure = match standard.get("secure") {
                Some(Standard::Flag) => true,
                Some(Standard::Text(Some(v))) => !v.is_empty(),
                _ => false,
            };
            // RFC2109 downgrade occurs AFTER default path and Cookie construction.
            let version = if netscape && version.as_ref() == Some(&BigInt::from(1)) {
                Some(BigInt::from(0))
            } else {
                version
            };
            result.push(Cookie {
                name,
                value,
                version,
                domain,
                domain_specified,
                path,
                port,
                port_specified,
                secure,
                expires,
            });
        }
        Ok(result)
    }
}
impl SourceCookieStore for SourceCookieJar {
    fn has_cookies(&self) -> bool {
        self.lock().has_cookies()
    }
    fn cookies(&self, view: &SourceCookieView) -> crate::Result<Option<HeaderValue>> {
        let mut selected = Vec::new();
        // HTTPX copies client cookies at Request construction; expiry purge and
        // header generation act on that ordered snapshot, not the shared jar.
        let mut snapshot = self.lock().clone();
        let result = {
            let state = &snapshot;
            let now = self.integer_now(false)?;
            for (domain, paths) in &state.domains {
                let req = view.request_host().map_err(crate::error::request)?;
                let effective = view.effective_host().map_err(crate::error::request)?;
                let dotdomain = if domain.starts_with('.') {
                    domain.clone()
                } else {
                    format!(".{domain}")
                };
                let dotted_req = if req.starts_with('.') {
                    req
                } else {
                    format!(".{req}")
                };
                let dotted_effective = if effective.starts_with('.') {
                    effective
                } else {
                    format!(".{effective}")
                };
                if !dotted_req.ends_with(&dotdomain) && !dotted_effective.ends_with(&dotdomain) {
                    continue;
                }
                for (path, names) in paths {
                    let request_path = view.request_path().map_err(crate::error::request)?;
                    if !path_matches(path, &request_path) {
                        continue;
                    }
                    for cookie in names {
                        if return_ok(cookie, view, &now)? {
                            selected.push(cookie.clone());
                        }
                    }
                }
            }
            // Stable sorting preserves domain/path/name dictionary enumeration.
            selected.sort_by_key(|c| std::cmp::Reverse(c.path.chars().count()));
            if selected.is_empty() || view.has_cookie_header() {
                None
            } else {
                let value = selected
                    .iter()
                    .map(|c| {
                        c.value
                            .as_ref()
                            .map_or_else(|| c.name.clone(), |v| format!("{}={v}", c.name))
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                if !value.is_ascii() {
                    return Err(crate::error::request(
                        "source cookie header ASCII encoding failed",
                    ));
                }
                let mut header = HeaderValue::from_str(&value).map_err(crate::error::request)?;
                header.set_sensitive(true);
                Some(header)
            }
        };
        // Source releases its header-operation lock before clear_expired_cookies,
        // observes wall time again, and does not purge after a header error.
        let now = self.integer_now(true)?;
        snapshot.purge(&now);
        Ok(result)
    }
    fn extract(&self, view: &SourceCookieView, response: &HeaderMap) -> crate::Result<()> {
        let ns = headers::values(response, http::header::SET_COOKIE);
        let rfc = headers::values(
            response,
            http::header::HeaderName::from_static("set-cookie2"),
        );
        let mut state = self.lock();
        let now = self.integer_now(false)?;
        // Default rfc2965=false: Set-Cookie2-only is an early exit. If Netscape
        // headers exist, source still constructs the RFC2965 batch first.
        if ns.is_empty() {
            return Ok(());
        }
        let rfc = self
            .create_batch(&mut state, headers::rfc2965(&rfc), view, &now, false)
            .unwrap_or_else(|_| {
                log::warn!("source RFC2965 cookie batch rejected at parser stage");
                Vec::new()
            });
        let ns = headers::netscape(&ns, || {
            self.clock.local_year().map_err(|_| SourceCookieDateError)
        })
        .and_then(|p| self.create_batch(&mut state, p, view, &now, true))
        .unwrap_or_else(|_| {
            log::warn!("source Netscape cookie batch rejected at parser stage");
            Vec::new()
        });
        for cookie in rfc.into_iter().chain(ns) {
            if set_ok(&cookie, view)? {
                state.insert(cookie);
            }
        }
        Ok(())
    }
}
#[derive(Clone)]
enum Standard {
    Text(Option<String>),
    Flag,
    Expires(BigInt),
    WrongExpiry,
}
fn normalize(
    pairs: Pairs,
    now: &BigInt,
) -> Result<Option<(String, Option<String>, HashMap<String, Standard>)>, SourceCookieDateError> {
    let mut iter = pairs.into_iter();
    let Some((name, value)) = iter.next() else {
        return Ok(None);
    };
    let Attribute::Text(value) = value else {
        return Ok(None);
    };
    let mut map = HashMap::new();
    let mut max_age = false;
    for (key, attr) in iter {
        let key = super::source_lower(&key);
        if map.contains_key(&key) {
            continue;
        }
        let normalized = match key.as_str() {
            "secure" | "discard" => match attr {
                Attribute::Text(None) => Standard::Flag,
                Attribute::Text(v) => Standard::Text(v),
                _ => continue,
            },
            "max-age" => {
                max_age = true;
                let Attribute::Text(Some(v)) = attr else {
                    // int(None) raises TypeError; source catches only ValueError
                    // here, so make_cookies rejects this entire batch.
                    return Err(SourceCookieDateError);
                };
                let Some(age) = integer(&v) else {
                    return Ok(None);
                };
                let e = age + now;
                map.insert("expires".to_owned(), Standard::Expires(e));
                continue;
            }
            "expires" => {
                if max_age {
                    continue;
                }
                match attr {
                    Attribute::Date(Some(e)) => Standard::Expires(BigInt::from(e)),
                    Attribute::Date(None) | Attribute::Text(None) => continue,
                    Attribute::Text(Some(_)) => Standard::WrongExpiry,
                }
            }
            "domain" => {
                let Attribute::Text(Some(v)) = attr else {
                    return Ok(None);
                };
                Standard::Text(Some(super::source_lower(&v)))
            }
            "version" | "path" => {
                let Attribute::Text(Some(v)) = attr else {
                    return Ok(None);
                };
                Standard::Text(Some(v))
            }
            "port" | "comment" | "commenturl" => {
                let Attribute::Text(v) = attr else {
                    return Ok(None);
                };
                Standard::Text(v)
            }
            _ => continue,
        };
        map.insert(key, normalized);
    }
    Ok(Some((name, value, map)))
}
fn integer(text: &str) -> Option<BigInt> {
    if !source_cookies::python_integer(text) {
        return None;
    }
    let text = text.trim_matches(source_cookies::python_whitespace);
    let mut normalized = String::new();
    for (i, c) in text.chars().enumerate() {
        if i == 0 && (c == '+' || c == '-') {
            normalized.push(c);
        } else if c != '_' {
            normalized.push(char::from(b'0' + super::python_text::decimal_digit(c)?));
        }
    }
    normalized.parse().ok()
}
fn float_rounded_integer(value: &BigInt) -> Result<BigInt, SourceCookieDateError> {
    let value = value
        .to_string()
        .parse::<f64>()
        .map_err(|_| SourceCookieDateError)?;
    if !value.is_finite() {
        return Err(SourceCookieDateError);
    }
    format!("{value:.0}")
        .parse()
        .map_err(|_| SourceCookieDateError)
}
fn path_matches(path: &str, request: &str) -> bool {
    request == path
        || (request.starts_with(path)
            && (path.ends_with('/') || request[path.len()..].starts_with('/')))
}
fn set_ok(c: &Cookie, view: &SourceCookieView) -> crate::Result<bool> {
    let Some(version) = &c.version else {
        return Ok(false);
    };
    if version > &BigInt::from(0) {
        return Ok(false);
    }
    if c.domain_specified {
        let req = view.request_host().map_err(crate::error::request)?;
        let effective = view.effective_host().map_err(crate::error::request)?;
        let undotted = c.domain.strip_prefix('.').unwrap_or(&c.domain);
        if !undotted.contains('.') && !effective.ends_with(".local") {
            return Ok(false);
        }
        if version == &BigInt::from(0)
            && !(effective.ends_with(&c.domain)
                || effective.ends_with(&format!("{undotted}.local"))
                || (!effective.starts_with('.') && format!(".{effective}").ends_with(&c.domain)))
        {
            return Ok(false);
        }
        let _ = req; // negative versions do not enable the strict Netscape switches.
    }
    if c.port_specified {
        let req = view.request_port().unwrap_or_else(|| "80".to_owned());
        let Some(port) = &c.port else {
            return Ok(false);
        };
        let mut found = false;
        for part in port.split(',') {
            if integer(part).is_none() {
                return Ok(false);
            }
            if part == req {
                found = true;
                break;
            }
        }
        if !found {
            return Ok(false);
        }
    }
    Ok(true)
}
fn return_ok(c: &Cookie, view: &SourceCookieView, now: &BigInt) -> crate::Result<bool> {
    let Some(version) = &c.version else {
        return Ok(false);
    };
    if version > &BigInt::from(0) {
        return Ok(false);
    }
    if c.secure && !matches!(view.scheme(), "https" | "wss") {
        return Ok(false);
    }
    if c.expires.as_ref().is_some_and(|e| e <= now) {
        return Ok(false);
    }
    if let Some(port) = &c.port {
        if !port.is_empty()
            && !port
                .split(',')
                .any(|p| p == view.request_port().as_deref().unwrap_or("80"))
        {
            return Ok(false);
        }
    }
    let effective = view.effective_host().map_err(crate::error::request)?;
    let domain = if c.domain.starts_with('.') {
        c.domain.clone()
    } else {
        format!(".{}", c.domain)
    };
    if version == &BigInt::from(0) && !format!(".{effective}").ends_with(&domain) {
        return Ok(false);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixed;
    impl SourceCookieClock for Fixed {
        fn now(&self) -> crate::Result<f64> {
            Ok(1_700_000_000.0)
        }
        fn local_year(&self) -> crate::Result<i32> {
            Ok(2023)
        }
    }
    fn jar() -> SourceCookieJar {
        SourceCookieJar::with_clock(Arc::new(Fixed))
    }
    fn view(url: &str) -> SourceCookieView {
        SourceCookieView::parse(url, &HeaderMap::new()).unwrap()
    }
    fn extract(jar: &SourceCookieJar, url: &str, values: &[&str]) {
        let mut headers = HeaderMap::new();
        for v in values {
            headers.append(http::header::SET_COOKIE, HeaderValue::from_str(v).unwrap());
        }
        jar.extract(&view(url), &headers).unwrap();
    }
    fn cookie(jar: &SourceCookieJar, url: &str) -> Option<String> {
        jar.cookies(&view(url))
            .unwrap()
            .map(|v| v.to_str().unwrap().to_owned())
    }
    #[test]
    fn raw_percent_cookie_domain_does_not_leak_to_normalized_host() {
        let jar = jar();
        extract(
            &jar,
            "http://exa%6dple.test:70000/a/b",
            &["sid=fake-secret"],
        );
        assert_eq!(
            cookie(&jar, "http://exa%6dple.test:1/a/c"),
            Some("sid=fake-secret".into())
        );
        assert_eq!(cookie(&jar, "http://example.test:70000/a/c"), None);
        assert!(!format!("{jar:?}").contains("fake-secret"));
        assert!(jar
            .cookies(&view("http://exa%6dple.test/a/c"))
            .unwrap()
            .unwrap()
            .is_sensitive());
    }
    #[test]
    fn source_cookie_order_overwrite_and_rfc2109_default_path_are_observable() {
        let jar = jar();
        extract(
            &jar,
            "http://a.test/deep/file",
            &[
                "a=old; Path=/",
                "b=two; Path=/deep",
                "a=new; Path=/",
                "zero=z",
                "one=o; Version=1",
            ],
        );
        assert_eq!(
            cookie(&jar, "http://a.test/deep"),
            Some("b=two; zero=z; a=new".into())
        );
        assert_eq!(
            cookie(&jar, "http://a.test/deep/next"),
            Some("one=o; b=two; zero=z; a=new".into())
        );
    }
    #[test]
    fn set_cookie2_only_is_ignored_but_ns_presence_reaches_default_version_zero() {
        let jar = jar();
        let mut headers = HeaderMap::new();
        headers.insert("set-cookie2", "r=two; Version=0; Path=/".parse().unwrap());
        jar.extract(&view("http://a.test/"), &headers).unwrap();
        assert!(!jar.has_cookies());
        headers.insert(http::header::SET_COOKIE, "n=one; Path=/".parse().unwrap());
        jar.extract(&view("http://a.test/"), &headers).unwrap();
        assert_eq!(cookie(&jar, "http://a.test/"), Some("r=two; n=one".into()));
    }
    #[test]
    fn max_age_overflow_drops_batch_but_prior_deletion_survives_and_digit_refusal_is_per_cookie() {
        let jar = jar();
        extract(
            &jar,
            "http://a.test/",
            &["delete=old; Path=/", "keep=old; Path=/"],
        );
        let overflow = format!("bad=x; Max-Age={}; Path=/", "9".repeat(400));
        extract(
            &jar,
            "http://a.test/",
            &[
                "delete=x; Max-Age=0; Path=/",
                "pending=no; Path=/",
                &overflow,
            ],
        );
        assert_eq!(cookie(&jar, "http://a.test/"), Some("keep=old".into()));
        let refused = format!("bad=x; Max-Age={}; Path=/", "9".repeat(4301));
        extract(&jar, "http://a.test/", &["good=yes; Path=/", &refused]);
        assert_eq!(
            cookie(&jar, "http://a.test/"),
            Some("keep=old; good=yes".into())
        );
        let negative = format!("keep=x; Max-Age=-{}; Path=/", "9".repeat(400));
        extract(&jar, "http://a.test/", &[&negative, "after=yes; Path=/"]);
        assert_eq!(
            cookie(&jar, "http://a.test/"),
            Some("good=yes; after=yes".into())
        );
    }
    #[test]
    fn finite_integer_float_roundtrip_and_duplicate_max_age_semantics() {
        assert_eq!(
            float_rounded_integer(&"9007199254740993".parse().unwrap()).unwrap(),
            "9007199254740992".parse::<BigInt>().unwrap()
        );
        assert!(float_rounded_integer(&BigInt::from(10).pow(400)).is_err());
        let jar = jar();
        extract(
            &jar,
            "http://a.test/",
            &[
                "a=gone; Max-Age=60; Max-Age=0; Path=/",
                "b=live; Max-Age=0; Max-Age=60; Path=/",
            ],
        );
        assert_eq!(cookie(&jar, "http://a.test/"), Some("b=live".into()));
    }
    #[test]
    fn secure_port_and_caller_cookie_rules_keep_source_views() {
        let jar = jar();
        extract(
            &jar,
            "https://[2001:0DB8::1]:70000/",
            &["p=yes; Port=80; Path=/", "s=yes; Secure; Path=/"],
        );
        assert_eq!(
            cookie(&jar, "http://[2001:0DB8::1]:99/"),
            Some("p=yes".into())
        );
        assert_eq!(
            cookie(&jar, "wss://[2001:0DB8::1]:99/"),
            Some("p=yes; s=yes".into())
        );
        let url = "https://[2001:0DB8::1]:99/";
        let mut headers = HeaderMap::new();
        headers.insert(http::header::COOKIE, "caller=kept".parse().unwrap());
        assert!(jar
            .cookies(&SourceCookieView::parse(url, &headers).unwrap())
            .unwrap()
            .is_none());
    }
    #[test]
    fn valueless_max_age_rejects_whole_batch_before_any_cookie_construction() {
        let jar = jar();
        let url = "http://example.test/path";
        extract(&jar, url, &["good=1; Path=/", "bad=2; Max-Age; Path=/"]);
        assert!(!jar.has_cookies());
    }
    #[test]
    fn later_normalization_type_error_prevents_earlier_expiry_deletion() {
        let jar = jar();
        let url = "http://example.test/path";
        extract(&jar, url, &["keep=original; Path=/"]);
        extract(
            &jar,
            url,
            &["keep=deleted; Max-Age=0; Path=/", "bad=2; Max-Age; Path=/"],
        );
        assert_eq!(cookie(&jar, url).as_deref(), Some("keep=original"));
    }

    #[test]
    fn underscored_numeric_month_expiry_is_not_silently_session_cookie() {
        let jar = jar();
        // The fixed clock is after1970: a parsed Jan1970 cookie deletes rather
        // than becoming a session cookie when the month spelling contains '_'.
        extract(
            &jar,
            "http://example.test/",
            &["expired=bad; Expires=01-0_1-1970; Path=/"],
        );
        assert!(!jar.has_cookies());
    }
}
