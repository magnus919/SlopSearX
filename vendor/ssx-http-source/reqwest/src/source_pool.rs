//! Explicit finite HTTPX transport mounts over the qualified source HTTP/1 pool.
//! Routing is selected at the innermost Reqwest service on every actual attempt.
mod environment;
mod source_url;
pub use source_url::{
    SourceUrl, SourceUrlChanges, SourceUrlError, SourceUrlErrorKind, SourceUrlOrigin,
};
mod source_cookie_clock;
mod source_cookie_date;
mod source_cookie_headers;
mod source_cookie_jar;
mod source_cookies;
pub use source_cookie_jar::SourceCookieJar;
pub use source_cookies::{
    SourceCookieClock, SourceCookieStore, SourceCookieSystemClock, SourceCookieView,
};
mod os_proxy;
mod platform_api;
mod python_text;
mod source_text;
fn source_lower(text: &str) -> String {
    python_text::lowercase_steps(text)
        .filter_map(|step| match step {
            python_text::LowerStep::Scalar(ch) => Some(ch),
            python_text::LowerStep::Progress => None,
        })
        .collect()
}

use crate::{connect::Connector, proxy::Intercepted};
use http::{header::PROXY_AUTHORIZATION, Uri};
pub use hyper_util::client::legacy::pool::{SourcePoolControl, SourcePoolTimedOut};
use std::{collections::BTreeMap, num::NonZeroUsize, sync::Arc, time::Duration};

#[derive(Clone, Debug)]
struct Pattern {
    scheme: String,
    host: String,
    port: Option<u16>,
}
impl Pattern {
    fn parse(text: &str) -> crate::Result<Self> {
        let (scheme, authority) = text
            .split_once("://")
            .ok_or_else(|| crate::error::builder("source mount pattern needs scheme://"))?;
        if !matches!(scheme, "all" | "http" | "https") {
            return Err(crate::error::builder("unsupported source mount scheme"));
        }
        // HTTPX URLPattern ignores path/query. Normalized ASCII hosts are the
        // current bridge domain; Unicode/IDNA needs the project's source codec.
        let authority = authority.split(['/', '?', '#']).next().unwrap();
        if !authority.is_ascii() || authority.contains('@') {
            return Err(crate::error::builder(
                "source mount host requires normalized ASCII without userinfo",
            ));
        }
        let (host, port) = if authority.starts_with('[') {
            let end = authority
                .find(']')
                .ok_or_else(|| crate::error::builder("invalid source mount IPv6"))?;
            let host = &authority[1..end];
            let host = host
                .parse::<std::net::Ipv6Addr>()
                .map_err(|_| crate::error::builder("invalid source mount IPv6"))?
                .to_string();
            let tail = &authority[end + 1..];
            let port = if tail.is_empty() {
                None
            } else {
                Some(
                    tail.strip_prefix(':')
                        .ok_or_else(|| crate::error::builder("invalid source mount port"))?
                        .parse()
                        .map_err(|_| crate::error::builder("invalid source mount port"))?,
                )
            };
            (host, port)
        } else {
            let (host, port) = match authority.rsplit_once(':') {
                Some((host, port)) => (
                    host,
                    Some(
                        port.parse()
                            .map_err(|_| crate::error::builder("invalid source mount port"))?,
                    ),
                ),
                None => (authority, None),
            };
            (host.to_ascii_lowercase(), port)
        };
        let port = match (scheme, port) {
            ("http", Some(80)) | ("https", Some(443)) => None,
            _ => port,
        };
        Ok(Self {
            scheme: if scheme == "all" {
                String::new()
            } else {
                scheme.into()
            },
            host: if host == "*" { String::new() } else { host },
            port,
        })
    }
    fn priority(&self) -> (bool, std::cmp::Reverse<usize>, std::cmp::Reverse<usize>) {
        (
            self.port.is_none(),
            std::cmp::Reverse(self.host.len()),
            std::cmp::Reverse(self.scheme.len()),
        )
    }
    fn matches(&self, uri: &Uri) -> bool {
        if !self.scheme.is_empty() && Some(self.scheme.as_str()) != uri.scheme_str() {
            return false;
        }
        let host = uri
            .host()
            .unwrap_or("")
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_ascii_lowercase();
        let host_matches = if self.host.is_empty() {
            true
        } else if let Some(suffix) = self.host.strip_prefix("*.") {
            host.len() > suffix.len() + 1 && host.ends_with(&format!(".{suffix}"))
        } else if let Some(suffix) = self.host.strip_prefix('*') {
            host == suffix
                || (host.len() > suffix.len() + 1 && host.ends_with(&format!(".{suffix}")))
        } else {
            host == self.host
        };
        let port = match (uri.scheme_str(), uri.port_u16()) {
            (Some("http"), Some(80)) | (Some("https"), Some(443)) => None,
            (_, p) => p,
        };
        host_matches && (self.port.is_none() || self.port == port)
    }
}
/// Explicit source proxy snapshot. Each nonempty pattern gets its OWN transport
/// mount, even when multiple patterns point at the same proxy URL. Bypass rules
/// use the common default direct mount, as HTTPX's None transports do.
#[derive(Clone)]
pub struct SourceHttp1PoolConfig {
    wait: Duration,
    routes: Vec<(Pattern, usize)>,
    mounts: Vec<Option<Intercepted>>,
}
impl std::fmt::Debug for SourceHttp1PoolConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceHttp1PoolConfig")
            .field("wait", &self.wait)
            .field("mount_count", &self.mounts.len())
            .finish_non_exhaustive()
    }
}
impl SourceHttp1PoolConfig {
    /// Entries must retain source insertion order; stable priority sort follows.
    pub fn new(wait: Duration, entries: Vec<(String, Option<String>)>) -> crate::Result<Self> {
        let mut result = Self {
            wait,
            routes: Vec::new(),
            mounts: vec![None],
        };
        let mut unique: Vec<(String, Option<String>)> = Vec::new();
        for (key, value) in entries {
            if let Some(entry) = unique.iter_mut().find(|(pattern, _)| pattern == &key) {
                entry.1 = value;
            } else {
                unique.push((key, value));
            }
        }
        for (pattern, proxy) in unique {
            let pattern = Pattern::parse(&pattern)?;
            let id = if let Some(proxy) = proxy {
                let matcher = crate::Proxy::all(&proxy)?.into_matcher();
                let intercepted = matcher
                    .intercept(&Uri::from_static("http://source-mount.invalid/"))
                    .ok_or_else(|| crate::error::builder("source proxy did not resolve"))?;
                if !matches!(intercepted.uri().scheme_str(), Some("http" | "https")) {
                    return Err(crate::error::builder(
                        "source bridge requires HTTP/HTTPS proxy",
                    ));
                }
                let id = result.mounts.len();
                result.mounts.push(Some(intercepted));
                id
            } else {
                0
            };
            result.routes.push((pattern, id));
        }
        result.routes.sort_by_key(|(pattern, _)| pattern.priority());
        Ok(result)
    }
    pub(crate) fn ledger(&self) -> hyper_util::client::legacy::pool::SourceHttp1PoolConfig {
        hyper_util::client::legacy::pool::SourceHttp1PoolConfig::new(
            self.wait,
            NonZeroUsize::new(self.mounts.len()).unwrap(),
        )
    }
    fn select(&self, uri: &Uri) -> usize {
        self.routes
            .iter()
            .find(|(p, _)| p.matches(uri))
            .map_or(0, |(_, id)| *id)
    }
    /// Translate an already-resolved urllib proxy-info snapshot. This function
    /// performs no environment access, OS discovery, network activity or IO.
    pub fn from_proxy_info(wait: Duration, info: &BTreeMap<String, String>) -> crate::Result<Self> {
        let mut entries = Vec::new();
        for scheme in ["http", "https", "all"] {
            if let Some(value) = info.get(scheme).filter(|v| !v.is_empty()) {
                entries.push((
                    format!("{scheme}://"),
                    Some(if value.contains("://") {
                        value.clone()
                    } else {
                        format!("http://{value}")
                    }),
                ));
            }
        }
        for host in info
            .get("no")
            .map(String::as_str)
            .unwrap_or("")
            .split(',')
            .map(|host| {
                host.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
            })
            .filter(|v| !v.is_empty())
        {
            if host == "*" {
                return Self::new(wait, Vec::new());
            }
            let pattern = if host.contains("://") {
                host.to_owned()
            } else if host
                .split('/')
                .next()
                .unwrap()
                .parse::<std::net::Ipv4Addr>()
                .is_ok()
            {
                format!("all://{host}")
            } else if host
                .split('/')
                .next()
                .unwrap()
                .parse::<std::net::Ipv6Addr>()
                .is_ok()
            {
                {
                    let (addr, subnet) = host
                        .split_once('/')
                        .map_or((host, None), |(addr, subnet)| (addr, Some(subnet)));
                    match subnet {
                        Some(subnet) => format!("all://[{addr}]/{subnet}"),
                        None => format!("all://[{addr}]"),
                    }
                }
            } else if host.eq_ignore_ascii_case("localhost") {
                format!("all://{host}")
            } else {
                format!("all://*{host}")
            };
            if let Some(existing) = entries.iter_mut().find(|(key, _)| key == &pattern) {
                existing.1 = None;
            } else {
                entries.push((pattern, None));
            }
        }
        Self::new(wait, entries)
    }
}
/// CPython getproxies_environment's two passes, including CGI protection and
/// lowercase empty-value removal. Input preserves environment iteration order.
/// An empty map triggers native platform fallback in current_environment_proxy_info.
pub fn environment_proxy_info(environment: &[(String, String)]) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    for (key, value) in environment {
        if let Some(name) = source_lower(key).strip_suffix("_proxy") {
            if !value.is_empty() {
                result.insert(name.to_owned(), value.clone());
            }
        }
    }
    if environment.iter().any(|(key, _)| key == "REQUEST_METHOD") {
        result.remove("http");
    }
    for (key, value) in environment {
        if key.ends_with("_proxy") {
            let name = source_lower(&key[..key.len() - 6]);
            if value.is_empty() {
                result.remove(&name);
            } else {
                result.insert(name, value.clone());
            }
        }
    }
    result
}
/// Gateway non-OAuth profile: P=C=W=connect_timeout, R=300s; no aggregate timer.
/// OAuth profile uses this with30s. Auth flow/storage/authorization timeout are
/// consumer-owned; this constructor does not implement OAuth.
pub fn gateway_builder(
    connect: Duration,
    proxy_info: &BTreeMap<String, String>,
) -> crate::Result<crate::ClientBuilder> {
    Ok(crate::Client::builder()
        .http1_only()
        .no_proxy()
        .redirect(crate::redirect::Policy::none())
        .retry(crate::retry::never())
        .source_http1_connector_timeouts(connect, Duration::from_secs(300))
        .source_http1_pool(SourceHttp1PoolConfig::from_proxy_info(connect, proxy_info)?))
}

/// Construct the transport profile from one native process-environment snapshot.
/// Empty snapshots invoke the platform-native urllib-compatible discovery path.
pub fn gateway_builder_from_environment(connect: Duration) -> crate::Result<crate::ClientBuilder> {
    gateway_builder(connect, &current_environment_proxy_info()?)
}

// Keep the original caller operand independently of per-attempt route defaults.
// Retrying/reusing the same request must never promote route A's generated
// credentials into an explicit caller header on route B or direct bypass.
#[derive(Clone)]
struct CallerProxyAuthorization(Vec<http::HeaderValue>);
fn prepare_proxy_authorization(
    req: &mut hyper::Request<crate::async_impl::body::Body>,
    proxy: Option<&Intercepted>,
) {
    let caller = if let Some(original) = req.extensions().get::<CallerProxyAuthorization>() {
        original.0.clone()
    } else {
        let mut original = req
            .headers()
            .get_all(PROXY_AUTHORIZATION)
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        for value in &mut original {
            value.set_sensitive(true);
        }
        req.extensions_mut()
            .insert(CallerProxyAuthorization(original.clone()));
        original
    };
    req.headers_mut().remove(PROXY_AUTHORIZATION);
    if !caller.is_empty() {
        for value in caller {
            req.headers_mut().append(PROXY_AUTHORIZATION, value);
        }
    } else if let (Some("http"), Some(value)) = (
        req.uri().scheme_str(),
        proxy.and_then(|p| p.http_non_tunnel_basic_auth()),
    ) {
        req.headers_mut().insert(PROXY_AUTHORIZATION, value);
    }
}

pub(crate) struct Prepared {
    config: SourceHttp1PoolConfig,
    pub(crate) control: SourcePoolControl,
    connector: Connector,
}
impl Prepared {
    pub(crate) fn new(
        config: SourceHttp1PoolConfig,
        control: SourcePoolControl,
        connector: Connector,
    ) -> Arc<Self> {
        Arc::new(Self {
            config,
            control,
            connector,
        })
    }
    pub(crate) fn attempt(
        &self,
        req: &mut hyper::Request<crate::async_impl::body::Body>,
    ) -> Result<(hyper_util::client::legacy::pool::SourceMount, Connector), crate::Error> {
        let id = self.config.select(req.uri());
        let proxy = self.config.mounts[id].clone();
        prepare_proxy_authorization(req, proxy.as_ref());
        let connector = self
            .connector
            .source_pool_route(proxy)
            .map_err(crate::error::builder)?;
        let mount = self
            .control
            .mount(id)
            .ok_or_else(|| crate::error::builder("source mount unavailable"))?;
        Ok((mount, connector))
    }
}

/// Read one process-environment snapshot without mutating it. On macOS/Windows,
/// an empty environment proxy map invokes native OS discovery. Unsupported
/// non-Unicode environment keys/values are preserved through source precedence.
pub fn current_environment_proxy_info() -> crate::Result<BTreeMap<String, String>> {
    environment::current(os_proxy::discover)
}

#[cfg(test)]
fn resolve_proxy_info(
    environment: &[(String, String)],
    discovery: impl FnOnce() -> crate::Result<BTreeMap<String, String>>,
) -> crate::Result<BTreeMap<String, String>> {
    let raw = environment
        .iter()
        .map(|(k, v)| (source_text::text(k), source_text::text(v)))
        .collect::<Vec<_>>();
    environment::resolve(&raw, discovery)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caller_proxy_authorization_precedes_route_defaults_and_generated_values_never_become_callers(
    ) {
        let config = SourceHttp1PoolConfig::new(
            Duration::from_secs(1),
            vec![
                (
                    "http://a.test".into(),
                    Some("http://first:one@127.0.0.1:1".into()),
                ),
                (
                    "http://b.test".into(),
                    Some("http://second:two@127.0.0.1:2".into()),
                ),
            ],
        )
        .unwrap();
        let mut request = hyper::Request::builder()
            .uri("http://a.test/")
            .body(crate::async_impl::Body::empty())
            .unwrap();
        prepare_proxy_authorization(&mut request, config.mounts[1].as_ref());
        assert_eq!(request.headers()[PROXY_AUTHORIZATION], "Basic Zmlyc3Q6b25l");
        *request.uri_mut() = "http://b.test/".parse().unwrap();
        prepare_proxy_authorization(&mut request, config.mounts[2].as_ref());
        assert_eq!(
            request.headers()[PROXY_AUTHORIZATION],
            "Basic c2Vjb25kOnR3bw==",
            "mutant: stale route A authorization survived route B"
        );
        prepare_proxy_authorization(&mut request, None);
        assert!(
            !request.headers().contains_key(PROXY_AUTHORIZATION),
            "mutant: generated proxy credentials leaked to direct origin"
        );
        let mut explicit = hyper::Request::builder()
            .uri("http://a.test/")
            .header(PROXY_AUTHORIZATION, "Bearer caller")
            .body(crate::async_impl::Body::empty())
            .unwrap();
        for proxy in [config.mounts[1].as_ref(), config.mounts[2].as_ref(), None] {
            prepare_proxy_authorization(&mut explicit, proxy);
            assert_eq!(
                explicit.headers()[PROXY_AUTHORIZATION],
                "Bearer caller",
                "mutant: configured route overwrote explicit caller"
            );
            assert!(explicit.headers()[PROXY_AUTHORIZATION].is_sensitive());
        }
        let mut duplicate = hyper::Request::builder()
            .uri("http://a.test/")
            .body(crate::async_impl::Body::empty())
            .unwrap();
        duplicate
            .headers_mut()
            .append(PROXY_AUTHORIZATION, http::HeaderValue::from_static("one"));
        duplicate
            .headers_mut()
            .append(PROXY_AUTHORIZATION, http::HeaderValue::from_static("two"));
        prepare_proxy_authorization(&mut duplicate, config.mounts[1].as_ref());
        assert_eq!(
            duplicate
                .headers()
                .get_all(PROXY_AUTHORIZATION)
                .iter()
                .map(|v| v.as_bytes())
                .collect::<Vec<_>>(),
            vec![b"one".as_slice(), b"two".as_slice()],
            "duplicate caller authorization collapsed mutant"
        );
        *explicit.uri_mut() = "https://b.test/".parse().unwrap();
        prepare_proxy_authorization(&mut explicit, config.mounts[2].as_ref());
        assert_eq!(
            explicit.headers()[PROXY_AUTHORIZATION],
            "Bearer caller",
            "CONNECT credentials must not replace origin caller"
        );
    }
    #[test]
    fn distinct_scheme_mounts_keep_distinct_credit_even_same_proxy_url() {
        let info = BTreeMap::from([
            ("http".into(), "http://127.0.0.1:8888".into()),
            ("https".into(), "http://127.0.0.1:8888".into()),
            ("all".into(), "http://127.0.0.1:8888".into()),
        ]);
        let pool = SourceHttp1PoolConfig::from_proxy_info(Duration::from_secs(30), &info).unwrap();
        assert_eq!(pool.mounts.len(), 4);
        assert_ne!(
            pool.select(&"http://one.test".parse().unwrap()),
            pool.select(&"https://one.test".parse().unwrap())
        );
    }
    #[test]
    fn bypasses_share_direct_and_priority_matches_httpx() {
        let info = BTreeMap::from([
            ("all".into(), "http://127.0.0.1:8888".into()),
            (
                "no".into(),
                ".one.test,two.test,localhost,127.0.0.1,::1".into(),
            ),
        ]);
        let pool = SourceHttp1PoolConfig::from_proxy_info(Duration::from_secs(30), &info).unwrap();
        for url in [
            "http://sub.one.test",
            "http://two.test",
            "https://sub.two.test",
            "http://localhost",
            "http://127.0.0.1",
            "http://[::1]",
        ] {
            assert_eq!(pool.select(&url.parse().unwrap()), 0, "{url}");
        }
        for url in [
            "http://one.test",
            "http://nottwo.test",
            "http://sub.localhost",
        ] {
            assert_ne!(pool.select(&url.parse().unwrap()), 0, "{url}");
        }
    }
    #[test]
    fn cgi_lowercase_empty_and_nonhttp_env_fallback_semantics() {
        let env = vec![
            ("HTTP_PROXY".into(), "upper".into()),
            ("http_proxy".into(), "lower".into()),
            ("REQUEST_METHOD".into(), "GET".into()),
            ("HTTPS_PROXY".into(), "tls".into()),
            ("https_proxy".into(), "".into()),
            ("FTP_PROXY".into(), "ftp".into()),
        ];
        let info = environment_proxy_info(&env);
        assert_eq!(info.get("http").map(String::as_str), Some("lower"));
        assert!(!info.contains_key("https"));
        assert!(info.contains_key("ftp"));
        let pool = SourceHttp1PoolConfig::from_proxy_info(
            Duration::from_secs(30),
            &BTreeMap::from([
                ("all".into(), "http://127.0.0.1:8888".into()),
                ("no".into(), "one.test,*".into()),
            ]),
        )
        .unwrap();
        assert_eq!(pool.mounts.len(), 1);
    }
    #[test]
    fn unicode_environment_kelvin_removal_reaches_real_fallback_seam() {
        let env = vec![
            ("K_PROXY".into(), "x".into()),
            ("k_proxy".into(), "".into()),
        ];
        let mut called = false;
        let map = resolve_proxy_info(&env, || {
            called = true;
            Ok(BTreeMap::from([("http".into(), "http://os-proxy".into())]))
        })
        .unwrap();
        assert!(
            called,
            "empty Unicode-normalized environment must invoke native discovery seam"
        );
        assert_eq!(map["http"], "http://os-proxy");
    }
    #[test]
    fn any_environment_proxy_entry_suppresses_native_discovery() {
        let env = vec![("FTP_PROXY".into(), "x".into())];
        let map = resolve_proxy_info(&env, || {
            panic!("nonempty complete map must suppress OS discovery")
        })
        .unwrap();
        assert_eq!(map["ftp"], "x");
        assert!(SourceHttp1PoolConfig::from_proxy_info(Duration::ZERO, &map).is_ok());
    }
    #[test]
    fn environment_full_lower_expansion_sigma_and_cgi_are_source_owned() {
        let env = vec![
            ("İ_PROXY".into(), "x".into()),
            ("ΟΣ_PROXY".into(), "y".into()),
            ("HTTP_PROXY".into(), "bad".into()),
            ("REQUEST_METHOD".into(), "GET".into()),
        ];
        let map = environment_proxy_info(&env);
        assert_eq!(map["i\u{307}"], "x");
        assert_eq!(map["ος"], "y");
        assert!(!map.contains_key("http"));
    }
}
