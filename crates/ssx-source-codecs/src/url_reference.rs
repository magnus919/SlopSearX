//! Raw gateway URL references following CPython 3.14.7 urllib.parse.
//!
//! Extracted from the gateway URL owner at 884d50f222792cbce82d282fef01afec33d93191
//! (vendor/ssx-http-source/reqwest/src/source_pool/source_url.rs).

/// Source validation failure while splitting or joining URL references.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawSourceUrlError {
    /// The authority contains malformed bracket syntax or an invalid literal.
    Bracket,
    /// Unicode 16 NFKC would introduce a delimiter into the authority.
    Netloc,
}

impl std::fmt::Display for RawSourceUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bracket => f.write_str("invalid bracketed URL authority"),
            Self::Netloc => {
                f.write_str("Unicode normalization introduces a URL authority delimiter")
            }
        }
    }
}

impl std::error::Error for RawSourceUrlError {}

fn scheme_valid(s: &str) -> bool {
    s.is_empty()
        || (s.as_bytes()[0].is_ascii_alphabetic()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+.-".contains(&b)))
}

// Exact CPython3.14.7 urllib.parse tables; urljoin uses private _urlsplit,
// NOT urlparse (so semicolon params remain part of path).
const JOIN_USES_RELATIVE: &[&str] = &[
    "", "ftp", "http", "gopher", "nntp", "imap", "wais", "file", "https", "shttp", "mms",
    "prospero", "rtsp", "rtsps", "rtspu", "sftp", "svn", "svn+ssh", "ws", "wss",
];
const JOIN_USES_NETLOC: &[&str] = &[
    "",
    "ftp",
    "http",
    "gopher",
    "nntp",
    "telnet",
    "imap",
    "wais",
    "file",
    "mms",
    "https",
    "shttp",
    "snews",
    "prospero",
    "rtsp",
    "rtsps",
    "rtspu",
    "rsync",
    "svn",
    "svn+ssh",
    "sftp",
    "nfs",
    "git",
    "git+ssh",
    "ws",
    "wss",
    "itms-services",
];
struct JoinParts {
    scheme: Option<String>,
    netloc: Option<String>,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
}
fn join_bracketed_host(host: &str) -> Result<(), RawSourceUrlError> {
    if let Some(future) = host.strip_prefix(['v', 'V']) {
        let good = future.split_once('.').is_some_and(|(version, tail)| {
            !version.is_empty()
                && version.bytes().all(|b| b.is_ascii_hexdigit())
                && !tail.is_empty()
        });
        return if good {
            Ok(())
        } else {
            Err(RawSourceUrlError::Bracket)
        };
    }
    // Match ipaddress's scoped IPv6 prefix check, including empty/% scope errors.
    let address = if let Some((a, scope)) = host.split_once('%') {
        if scope.is_empty() || scope.contains('%') {
            return Err(RawSourceUrlError::Bracket);
        }
        a
    } else {
        host
    };
    address
        .parse::<std::net::Ipv6Addr>()
        .map(|_| ())
        .map_err(|_| RawSourceUrlError::Bracket)
}
fn join_check_brackets(netloc: &str) -> Result<(), RawSourceUrlError> {
    if netloc.contains('[') != netloc.contains(']') {
        return Err(RawSourceUrlError::Bracket);
    }
    if netloc.contains('[') && netloc.contains(']') {
        let host_port = netloc.rsplit_once('@').map_or(netloc, |(_, tail)| tail);
        let host = if let Some((before, rest)) = host_port.split_once('[') {
            if !before.is_empty() {
                return Err(RawSourceUrlError::Bracket);
            }
            let (host, tail) = rest.split_once(']').unwrap_or((rest, ""));
            // CPython checks ONLY that the tail starts ':', never its port syntax.
            if !tail.is_empty() && !tail.starts_with(':') {
                return Err(RawSourceUrlError::Bracket);
            }
            host
        } else {
            host_port
                .split_once(':')
                .map_or(host_port, |(host, _)| host)
        };
        join_bracketed_host(host)?;
    }
    Ok(())
}
fn join_split(text: &str) -> Result<JoinParts, RawSourceUrlError> {
    let clean = text
        .trim_start_matches(|ch: char| ch <= ' ')
        .replace(['\t', '\r', '\n'], "");
    let mut rest = clean.as_str();
    let mut scheme = None;
    if let Some((candidate, tail)) = rest.split_once(':')
        && !candidate.is_empty()
        && scheme_valid(candidate)
    {
        scheme = Some(candidate.to_ascii_lowercase());
        rest = tail;
    }
    let netloc = if let Some(after) = rest.strip_prefix("//") {
        let end = after.find(['/', '?', '#']).unwrap_or(after.len());
        rest = &after[end..];
        let n = &after[..end];
        join_check_brackets(n)?;
        Some(n.to_owned())
    } else {
        None
    };
    let (before_fragment, fragment) = rest
        .split_once('#')
        .map_or((rest, None), |(a, b)| (a, Some(b.to_owned())));
    let (path, query) = before_fragment
        .split_once('?')
        .map_or((before_fragment, None), |(a, b)| (a, Some(b.to_owned())));
    // CPython removes literal @:#? before NFKC; '/' already ended netloc.
    // These literals cannot affect introduction of ASCII guards by other scalars:
    // the pinned Unicode 16 closure proof is in url_join's tests.
    if netloc.as_ref().is_some_and(|n| {
        !n.is_ascii()
            && n.chars()
                .any(crate::url_join::url_join_nfkc_introduces_delimiter)
    }) {
        return Err(RawSourceUrlError::Netloc);
    }
    Ok(JoinParts {
        scheme,
        netloc,
        path: path.into(),
        query,
        fragment,
    })
}
fn join_unsplit(
    scheme: Option<&str>,
    netloc: Option<&str>,
    path: &str,
    query: Option<&str>,
    fragment: Option<&str>,
) -> String {
    let mut out = String::new();
    if let Some(s) = scheme.filter(|s| !s.is_empty()) {
        out.push_str(s);
        out.push(':');
    }
    if let Some(n) = netloc {
        out.push_str("//");
        out.push_str(n);
        if !path.is_empty() && !path.starts_with('/') {
            out.push('/');
        }
    } else if path.starts_with("//") {
        out.push_str("//");
    }
    out.push_str(path);
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }
    out
}

/// Source URL components with the original authority spelling.
#[derive(Clone, PartialEq, Eq)]
pub struct RawSourceUrlParts {
    scheme: String,
    netloc: String,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
}
impl std::fmt::Debug for RawSourceUrlParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RawSourceUrlParts(<raw observations>)")
    }
}
impl RawSourceUrlParts {
    /// Lowercased recognized scheme from raw urlparse.
    pub fn scheme(&self) -> &str {
        &self.scheme
    }
    /// Untouched authority spelling, including userinfo and arbitrary port text.
    pub fn netloc(&self) -> &str {
        &self.netloc
    }
    /// Raw path; urlparse strips scheme-dependent final params, urlsplit retains them.
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Raw query, preserving its explicit empty delimiter observation.
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }
    /// Raw fragment, preserving its explicit empty delimiter observation.
    pub fn fragment(&self) -> Option<&str> {
        self.fragment.as_deref()
    }
}
const RAW_USES_PARAMS: &[&str] = &[
    "", "ftp", "hdl", "prospero", "http", "imap", "https", "shttp", "rtsp", "rtsps", "rtspu",
    "sip", "sips", "mms", "sftp", "tel",
];
/// Parse original text with the pinned urllib.parse URL and parameter rules.
pub fn source_raw_urlparse(text: &str) -> Result<RawSourceUrlParts, RawSourceUrlError> {
    let mut parts = source_raw_urlsplit(text)?;
    if RAW_USES_PARAMS.contains(&parts.scheme.as_str()) {
        let start = parts.path.rfind('/').unwrap_or(0);
        if let Some(offset) = parts.path[start..].find(';') {
            parts.path.truncate(start + offset);
        }
    }
    Ok(parts)
}
/// Split original text, including path parameters and empty query delimiters.
pub fn source_raw_urlsplit(text: &str) -> Result<RawSourceUrlParts, RawSourceUrlError> {
    let parts = join_split(text)?;
    Ok(RawSourceUrlParts {
        scheme: parts.scheme.unwrap_or_default(),
        netloc: parts.netloc.unwrap_or_default(),
        path: parts.path,
        query: parts.query,
        fragment: parts.fragment,
    })
}
/// Public urllib.parse.urlunsplit: empty query/fragment delimiters collapse.
/// Empty authority presence follows the pinned scheme table, unlike _urlunsplit.
pub fn source_raw_urlunsplit(
    scheme: &str,
    netloc: &str,
    path: &str,
    query: Option<&str>,
    fragment: Option<&str>,
) -> String {
    let authority = if !netloc.is_empty()
        || (!scheme.is_empty()
            && JOIN_USES_NETLOC.contains(&scheme)
            && (path.is_empty() || path.starts_with('/')))
    {
        Some(netloc)
    } else {
        None
    };
    join_unsplit(
        Some(scheme),
        authority,
        path,
        query.filter(|q| !q.is_empty()),
        fragment.filter(|f| !f.is_empty()),
    )
}
/// Resolve a raw reference against a base with the pinned urllib.parse rules.
/// A transport caller must validate the resulting URL before routing.
pub fn source_raw_urljoin(base: &str, reference: &str) -> Result<String, RawSourceUrlError> {
    let base = base.to_owned();
    let target = reference.to_owned();
    // CPython returns original text before splitting if either operand is empty.
    let joined = if base.is_empty() {
        target
    } else if target.is_empty() {
        base
    } else {
        let b = join_split(&base)?;
        let r = join_split(&target)?;
        let scheme = r.scheme.as_deref().or(b.scheme.as_deref());
        if scheme != b.scheme.as_deref() || scheme.is_some_and(|s| !JOIN_USES_RELATIVE.contains(&s))
        {
            target
        } else {
            let mut netloc = r.netloc.clone();
            if scheme.is_none() || scheme.is_some_and(|s| JOIN_USES_NETLOC.contains(&s)) {
                if netloc.as_ref().is_some_and(|n| !n.is_empty()) {
                    let text = join_unsplit(
                        scheme,
                        netloc.as_deref(),
                        &r.path,
                        r.query.as_deref(),
                        r.fragment.as_deref(),
                    );
                    return Ok(text);
                }
                netloc = b.netloc.clone();
            }
            let mut path = r.path.clone();
            let mut query = r.query.as_deref();
            let mut fragment = r.fragment.as_deref();
            if path.is_empty() {
                path = b.path.clone();
                if query.is_none() {
                    query = b.query.as_deref();
                    if fragment.is_none() {
                        fragment = b.fragment.as_deref();
                    }
                }
            } else {
                let mut base_parts: Vec<_> = b.path.split('/').collect();
                if base_parts.last() != Some(&"") {
                    base_parts.pop();
                }
                let segments = if path.starts_with('/') {
                    path.split('/').collect::<Vec<_>>()
                } else {
                    base_parts.extend(path.split('/'));
                    let len = base_parts.len();
                    base_parts
                        .into_iter()
                        .enumerate()
                        .filter(|(i, v)| *i == 0 || *i + 1 == len || !v.is_empty())
                        .map(|(_, v)| v)
                        .collect()
                };
                let trailing = segments.last().is_some_and(|s| matches!(*s, "." | ".."));
                let mut resolved = Vec::new();
                for seg in segments {
                    match seg {
                        ".." => {
                            resolved.pop();
                        }
                        "." => {}
                        _ => resolved.push(seg),
                    }
                }
                if trailing {
                    resolved.push("");
                }
                path = resolved.join("/");
                if path.is_empty() {
                    path = "/".into();
                }
            }
            join_unsplit(scheme, netloc.as_deref(), &path, query, fragment)
        }
    };
    Ok(joined)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_parse_preserves_case_userinfo_and_only_strips_final_params_for_table_schemes() {
        let parts = source_raw_urlparse(" \0HTTP://User:Pass@EXAMPLE:80/a;b/c;d?q?#f").unwrap();
        assert_eq!(parts.scheme(), "http");
        assert_eq!(parts.netloc(), "User:Pass@EXAMPLE:80");
        assert_eq!(parts.path(), "/a;b/c");
        assert_eq!(parts.query(), Some("q?"));
        assert_eq!(parts.fragment(), Some("f"));
        assert_eq!(
            source_raw_urlparse("custom://Host/a;b").unwrap().path(),
            "/a;b"
        );
        assert_eq!(
            source_raw_urlsplit("http://Host/a;b?").unwrap().path(),
            "/a;b"
        );
    }
    #[test]
    fn raw_join_preserves_early_returns_empty_delimiters_and_unknown_scheme_rules() {
        assert_eq!(
            source_raw_urljoin("http://Case/p?old#f", "?").unwrap(),
            "http://Case/p?"
        );
        assert_eq!(source_raw_urljoin("\0RAW?", "").unwrap(), "\0RAW?");
        assert_eq!(source_raw_urljoin("", "\0RAW?").unwrap(), "\0RAW?");
        assert_eq!(
            source_raw_urljoin("custom://Host/a", "/next").unwrap(),
            "/next"
        );
        assert_eq!(
            source_raw_urlparse("http://[::1"),
            Err(RawSourceUrlError::Bracket)
        );
        assert_eq!(
            source_raw_urlparse("http://exa／mple/"),
            Err(RawSourceUrlError::Netloc)
        );
        assert_eq!(
            source_raw_urlparse("\0http://Host/").unwrap().netloc(),
            "Host"
        );
    }
    #[test]
    fn public_unsplit_empty_delimiters_are_distinct_from_private_join() {
        assert_eq!(
            source_raw_urlunsplit("http", "", "/p", Some(""), Some("")),
            "http:///p"
        );
        assert_eq!(
            source_raw_urlunsplit("custom", "", "/p", Some("q"), None),
            "custom:/p?q"
        );
        assert_eq!(
            source_raw_urljoin("http:///p?", "#").unwrap(),
            "http:///p?#"
        );
    }
}
