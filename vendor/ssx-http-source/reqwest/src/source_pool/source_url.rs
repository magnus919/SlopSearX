//! Immutable HTTPX2 URL owner before any WHATWG/URI transport projection.
//! Request/wire ownership is a separate integration obligation; this parser is
//! not a replacement for Reqwest's default Url API.
/// Ordered source URL validation stage, without retaining secret URL text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceUrlErrorKind {
    /// The source URL codepoint length limit.
    Length,
    /// A forbidden ASCII control character.
    Control,
    /// Source IPv4 address validation.
    Ipv4,
    /// Source IPv6 address validation.
    Ipv6,
    /// Strict source IDNA or host display validation.
    Idna,
    /// Source decimal integer parsing.
    Port,
    /// Source relative or authority path validation.
    Path,
    /// A nonempty proxy pattern omitted the source colon form.
    PatternForm,
    /// Invalid typed copy component.
    Component,
    /// A later source ASCII accessor cannot encode an otherwise constructed URL.
    Ascii,
    /// CPython ValueError from urljoin operand bracket validation, after construction.
    JoinBracket,
    /// CPython ValueError from urljoin operand Unicode16 NFKC netloc validation.
    JoinNetloc,
}
/// Typed source URL validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceUrlError {
    /// The source validation stage that rejected the input.
    pub kind: SourceUrlErrorKind,
}
impl std::fmt::Display for SourceUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTPX2 source URL validation: {:?}", self.kind)
    }
}
impl std::error::Error for SourceUrlError {}
fn invalid(kind: SourceUrlErrorKind) -> SourceUrlError {
    SourceUrlError { kind }
}

fn whitespace(ch: char) -> bool {
    // CPython16 _PyUnicode_IsWhitespace; URL controls are rejected first.
    matches!(ch, '\u{9}'..='\u{d}' | '\u{1c}'..='\u{20}' | '\u{85}' | '\u{a0}'
        | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}'
        | '\u{202f}' | '\u{205f}' | '\u{3000}')
}
fn port(text: &str, original_scheme: &str) -> Result<Option<String>, SourceUrlError> {
    if text.is_empty() {
        return Ok(None);
    }
    let text = text.trim_matches(whitespace);
    let (negative, digits) = if let Some(tail) = text.strip_prefix('-') {
        (true, tail)
    } else {
        (false, text.strip_prefix('+').unwrap_or(text))
    };
    let mut normalized = String::new();
    let mut previous_digit = false;
    let mut digit_count = 0;
    for ch in digits.chars() {
        if ch == '_' {
            if !previous_digit {
                return Err(invalid(SourceUrlErrorKind::Port));
            }
            previous_digit = false;
        } else {
            let digit = ssx_source_codecs::python_text::decimal_digit(ch)
                .ok_or(invalid(SourceUrlErrorKind::Port))?;
            normalized.push(char::from(b'0' + digit));
            digit_count += 1;
            previous_digit = true;
        }
    }
    // Pinned CPython's default non-power-of-two int-string digit limit.
    if !previous_digit || digit_count > 4300 {
        return Err(invalid(SourceUrlErrorKind::Port));
    }
    let significant = normalized.trim_start_matches('0');
    let normalized = if significant.is_empty() {
        "0".to_owned()
    } else if negative {
        format!("-{significant}")
    } else {
        significant.to_owned()
    };
    let default = match original_scheme {
        "ftp" => "21",
        "http" | "ws" => "80",
        "https" | "wss" => "443",
        _ => "",
    };
    Ok((normalized != default).then_some(normalized))
}
fn quote_host(host: &str) -> String {
    let mut result = String::with_capacity(host.len());
    for b in host.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=\"`{}%|\\".contains(&b) {
            result.push(char::from(b.to_ascii_lowercase()));
        } else {
            use std::fmt::Write;
            write!(result, "%{b:02X}").expect("writing to String");
        }
    }
    result
}
fn host(host: &str) -> Result<String, SourceUrlError> {
    let encoded = if host.is_empty() {
        String::new()
    } else if host.split('.').count() == 4
        && host
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    {
        host.parse::<std::net::Ipv4Addr>()
            .map_err(|_| invalid(SourceUrlErrorKind::Ipv4))?;
        host.to_owned()
    } else if host.starts_with('[') && host.ends_with(']') {
        let inside = &host[1..host.len() - 1];
        let address = if let Some((address, scope)) = inside.split_once('%') {
            if scope.is_empty() || scope.contains('%') {
                return Err(invalid(SourceUrlErrorKind::Ipv6));
            }
            address
        } else {
            inside
        };
        address
            .parse::<std::net::Ipv6Addr>()
            .map_err(|_| invalid(SourceUrlErrorKind::Ipv6))?;
        inside.to_owned() // Python validates but preserves IPv6 spelling and scope.
    } else if host.is_ascii() {
        quote_host(host)
    } else {
        ssx_source_codecs::idna2008::encode(&super::source_lower(host))
            .map_err(|_| invalid(SourceUrlErrorKind::Idna))?
    };
    Ok(encoded)
}

/// Where the parser's input text came from; derived text is not caller spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceUrlOrigin {
    /// Direct caller input, retained verbatim.
    Input,
    /// Reconstructed current components through copy_with.
    Copy,
    /// CPython3.14.7 urljoin over both source-normalized serializations.
    Join,
    /// HTTPX client base/raw-path concatenation, distinct from join.
    ClientMerge,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Parts {
    scheme: String,
    userinfo: String,
    host: String,
    port: Option<String>,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
}
/// Full immutable source URL. Debug omits all URL data, including credentials.
/// No Request/URI conversion or transport authority is granted by construction.
#[derive(Clone)]
pub struct SourceUrl {
    input: String,
    origin: SourceUrlOrigin,
    lexical_authority: bool,
    lexical_userinfo: bool,
    parts: Parts,
}
impl std::fmt::Debug for SourceUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceUrl")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}
/// Typed copy inputs. None keeps a component; Some(None) clears query/fragment/port.
/// Empty strings clear nonnullable components. Defaults come from current parsed
/// fields, never the original input. raw_path and userinfo aliases follow HTTPX.
#[derive(Clone, Copy, Default)]
pub struct SourceUrlChanges<'a> {
    /// Scheme spelling before source lowercase/default-port handling.
    pub scheme: Option<&'a str>,
    /// Full authority replacement before individual authority overrides.
    pub authority: Option<&'a str>,
    /// Quoted or unquoted joint user information; must not contain literal @.
    pub userinfo: Option<&'a str>,
    /// Host replacement; colon-containing hosts are bracketed as source kwargs.
    pub host: Option<&'a str>,
    /// Lossless signed decimal source input; no u16 restriction.
    pub port: Option<Option<&'a str>>,
    /// Path replacement, excluding ? and #.
    pub path: Option<&'a str>,
    /// Query replacement, excluding #; Some("") retains the delimiter.
    pub query: Option<Option<&'a str>>,
    /// Fragment replacement; Some("") retains the delimiter.
    pub fragment: Option<Option<&'a str>>,
    /// Source raw_path alias; replaces path and query together.
    pub raw_path: Option<&'a str>,
    /// Username alias; when either alias is given, the other defaults to empty.
    pub username: Option<&'a str>,
    /// Password alias, never rendered by Debug or errors.
    pub password: Option<&'a str>,
    /// Source netloc alias partitions at the first colon before host validation.
    pub netloc: Option<&'a str>,
}
impl std::fmt::Debug for SourceUrlChanges<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SourceUrlChanges(<redacted>)")
    }
}
fn validate(text: &str) -> Result<(), SourceUrlError> {
    if text.chars().count() > 65536 {
        return Err(invalid(SourceUrlErrorKind::Length));
    }
    if text.bytes().any(|b| b.is_ascii_control()) {
        return Err(invalid(SourceUrlErrorKind::Control));
    }
    Ok(())
}
fn scheme_valid(s: &str) -> bool {
    s.is_empty()
        || (s.as_bytes()[0].is_ascii_alphabetic()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+.-".contains(&b)))
}
fn quote(text: &str, excluded: &[u8]) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if (b' '..=b'~').contains(&b) && !excluded.contains(&b) {
            out.push(char::from(b));
        } else {
            use std::fmt::Write;
            write!(out, "%{b:02X}").expect("String write");
        }
    }
    out
}
const PATH_EXCLUDED: &[u8] = b" \"#<>?`{}";
const QUERY_EXCLUDED: &[u8] = b" \"#<>";
const FRAGMENT_EXCLUDED: &[u8] = b" \"<>`";
const USERINFO_EXCLUDED: &[u8] = b" \"#<>?`{}/;=@[\\]^|";
const USERNAME_EXCLUDED: &[u8] = b" \"#<>?`{}/:;=@[\\]^|";
fn unquote(text: &str) -> String {
    fn hex(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    }
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < b.len() {
        if b[at] == b'%' && at + 2 < b.len() {
            if let (Some(a), Some(c)) = (hex(b[at + 1]), hex(b[at + 2])) {
                out.push(a * 16 + c);
                at += 3;
                continue;
            }
        }
        out.push(b[at]);
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
fn split_authority(authority: &str) -> (&str, &str, &str) {
    let (userinfo, authority) = authority.rsplit_once('@').unwrap_or(("", authority));
    let (host, port) = if authority.starts_with('[') {
        if let Some(end) = authority.rfind(']') {
            let tail = &authority[end + 1..];
            (&authority[..=end], tail.strip_prefix(':').unwrap_or(tail))
        } else {
            authority.split_once(':').unwrap_or((authority, ""))
        }
    } else {
        authority.split_once(':').unwrap_or((authority, ""))
    };
    (userinfo, host, port)
}
fn normalized_path(path: &str) -> String {
    let parts: Vec<_> = path.split('/').collect();
    if !parts.iter().any(|p| matches!(*p, "." | "..")) {
        return path.to_owned();
    }
    let mut out = Vec::new();
    for part in parts {
        if part == "." {
        } else if part == ".." {
            if !out.is_empty() && out != [""] {
                out.pop();
            }
        } else {
            out.push(part);
        }
    }
    out.join("/")
}
impl Parts {
    fn netloc(&self) -> String {
        let h = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        match &self.port {
            Some(p) => format!("{h}:{p}"),
            None => h,
        }
    }
    fn authority(&self) -> String {
        let netloc = self.netloc();
        if self.userinfo.is_empty() {
            netloc
        } else {
            format!("{}@{netloc}", self.userinfo)
        }
    }
    fn serialized(&self) -> String {
        let mut out = String::new();
        if !self.scheme.is_empty() {
            out.push_str(&self.scheme);
            out.push(':');
        }
        let authority = self.authority();
        if !authority.is_empty() {
            out.push_str("//");
            out.push_str(&authority);
        }
        out.push_str(&self.path);
        if let Some(q) = &self.query {
            out.push('?');
            out.push_str(q);
        }
        if let Some(f) = &self.fragment {
            out.push('#');
            out.push_str(f);
        }
        out
    }
}
fn normalize(
    scheme: &str,
    userinfo: &str,
    raw_host: &str,
    raw_port: &str,
    path: &str,
    query: Option<&str>,
    fragment: Option<&str>,
) -> Result<Parts, SourceUrlError> {
    let scheme_lower = scheme.to_ascii_lowercase();
    let userinfo = quote(userinfo, USERINFO_EXCLUDED);
    let host = host(raw_host)?; // Host errors precede port, which precedes path.
    let port = port(raw_port, scheme)?; // ORIGINAL scheme spelling is causal.
    let authority = !userinfo.is_empty() || !host.is_empty() || port.is_some();
    if (authority && !path.is_empty() && !path.starts_with('/'))
        || (scheme_lower.is_empty()
            && !authority
            && (path.starts_with("//") || path.starts_with(':')))
    {
        return Err(invalid(SourceUrlErrorKind::Path));
    }
    let path = if !scheme_lower.is_empty() || authority {
        normalized_path(path)
    } else {
        path.into()
    };
    Ok(Parts {
        scheme: scheme_lower,
        userinfo,
        host,
        port,
        path: quote(&path, PATH_EXCLUDED),
        query: query.map(|s| quote(s, QUERY_EXCLUDED)),
        fragment: fragment.map(|s| quote(s, FRAGMENT_EXCLUDED)),
    })
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
fn join_bracketed_host(host: &str) -> Result<(), SourceUrlError> {
    if let Some(future) = host.strip_prefix(['v', 'V']) {
        let good = future.split_once('.').is_some_and(|(version, tail)| {
            !version.is_empty()
                && version.bytes().all(|b| b.is_ascii_hexdigit())
                && !tail.is_empty()
        });
        return if good {
            Ok(())
        } else {
            Err(invalid(SourceUrlErrorKind::JoinBracket))
        };
    }
    // Match ipaddress's scoped IPv6 prefix check, including empty/% scope errors.
    let address = if let Some((a, scope)) = host.split_once('%') {
        if scope.is_empty() || scope.contains('%') {
            return Err(invalid(SourceUrlErrorKind::JoinBracket));
        }
        a
    } else {
        host
    };
    address
        .parse::<std::net::Ipv6Addr>()
        .map(|_| ())
        .map_err(|_| invalid(SourceUrlErrorKind::JoinBracket))
}
fn join_check_brackets(netloc: &str) -> Result<(), SourceUrlError> {
    if netloc.contains('[') != netloc.contains(']') {
        return Err(invalid(SourceUrlErrorKind::JoinBracket));
    }
    if netloc.contains('[') && netloc.contains(']') {
        let host_port = netloc.rsplit_once('@').map_or(netloc, |(_, tail)| tail);
        let host = if let Some((before, rest)) = host_port.split_once('[') {
            if !before.is_empty() {
                return Err(invalid(SourceUrlErrorKind::JoinBracket));
            }
            let (host, tail) = rest.split_once(']').unwrap_or((rest, ""));
            // CPython checks ONLY that the tail starts ':', never its port syntax.
            if !tail.is_empty() && !tail.starts_with(':') {
                return Err(invalid(SourceUrlErrorKind::JoinBracket));
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
fn join_split(text: &str) -> Result<JoinParts, SourceUrlError> {
    let clean = text
        .trim_start_matches(|ch: char| ch <= ' ')
        .replace(['\t', '\r', '\n'], "");
    let mut rest = clean.as_str();
    let mut scheme = None;
    if let Some((candidate, tail)) = rest.split_once(':') {
        if !candidate.is_empty() && scheme_valid(candidate) {
            scheme = Some(candidate.to_ascii_lowercase());
            rest = tail;
        }
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
    // the pinned Unicode16 closure/composition proof is in python_text's tests.
    // No Unicode17 host normalizer or global constructor restriction is used.
    if netloc.as_ref().is_some_and(|n| {
        !n.is_ascii()
            && n.chars()
                .any(ssx_source_codecs::python_text::url_join_nfkc_introduces_delimiter)
    }) {
        return Err(invalid(SourceUrlErrorKind::JoinNetloc));
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

/// Raw urllib.parse.urlparse observations only; not a transport-authorized URL.
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
/// Observe original text with pinned urllib.parse.urlparse, not SourceUrl::parse.
/// Reuses the same exact private _urlsplit brackets/NFKC/C0 handling as raw join.
pub fn source_raw_urlparse(text: &str) -> Result<RawSourceUrlParts, SourceUrlError> {
    let mut parts = source_raw_urlsplit(text)?;
    if RAW_USES_PARAMS.contains(&parts.scheme.as_str()) {
        let start = parts.path.rfind('/').unwrap_or(0);
        if let Some(offset) = parts.path[start..].find(';') {
            parts.path.truncate(start + offset);
        }
    }
    Ok(parts)
}
/// Raw urllib.parse.urlsplit observations without urlparse's params removal.
/// This never normalizes through HTTPX or grants endpoint/model authority.
pub fn source_raw_urlsplit(text: &str) -> Result<RawSourceUrlParts, SourceUrlError> {
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
/// Raw urllib.parse.urljoin over original strings, not HTTPX normalization.
/// The returned text grants no routing, validation or model authority.
pub fn source_raw_urljoin(base: &str, reference: &str) -> Result<String, SourceUrlError> {
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

impl SourceUrl {
    /// Parse exact HTTPX2 string input, preserving original text separately.
    pub fn parse(text: &str) -> Result<Self, SourceUrlError> {
        validate(text)?;
        let (scheme, mut rest) = match text.split_once(':') {
            Some((candidate, tail)) if scheme_valid(candidate) => (candidate, tail),
            _ => ("", text),
        };
        let lexical_authority = rest.starts_with("//");
        let authority = if let Some(after) = rest.strip_prefix("//") {
            let end = after.find(['/', '?', '#']).unwrap_or(after.len());
            rest = &after[end..];
            &after[..end]
        } else {
            ""
        };
        let (userinfo, raw_host, raw_port) = split_authority(authority);
        let (before_fragment, fragment) = rest
            .split_once('#')
            .map_or((rest, None), |(a, b)| (a, Some(b)));
        let (path, query) = before_fragment
            .split_once('?')
            .map_or((before_fragment, None), |(a, b)| (a, Some(b)));
        Ok(Self {
            input: text.into(),
            origin: SourceUrlOrigin::Input,
            lexical_authority,
            lexical_userinfo: authority.contains('@'),
            parts: normalize(scheme, userinfo, raw_host, raw_port, path, query, fragment)?,
        })
    }
    /// The exact parser input. For derived owners, inspect origin before attribution.
    pub fn input(&self) -> &str {
        &self.input
    }
    /// Distinguish caller input from constructed copy/join/client-merge input.
    pub fn origin(&self) -> SourceUrlOrigin {
        self.origin
    }
    /// Original parser input contained an authority delimiter, even if it normalized away.
    pub fn lexical_authority_present(&self) -> bool {
        self.lexical_authority
    }
    /// Original parser authority contained a userinfo delimiter, even if empty.
    pub fn lexical_userinfo_present(&self) -> bool {
        self.lexical_userinfo
    }
    /// Source-normalized scheme; always ASCII.
    pub fn scheme(&self) -> &str {
        &self.parts.scheme
    }
    /// Source-normalized raw host; IPv6 has no surrounding brackets.
    pub fn raw_host(&self) -> &str {
        &self.parts.host
    }
    /// Source raw_scheme ASCII bytes.
    pub fn raw_scheme(&self) -> &[u8] {
        self.scheme().as_bytes()
    }
    /// Source raw_host ASCII boundary; scoped IPv6 construction may retain Unicode.
    pub fn raw_host_bytes(&self) -> Result<&[u8], SourceUrlError> {
        if self.raw_host().is_ascii() {
            Ok(self.raw_host().as_bytes())
        } else {
            Err(invalid(SourceUrlErrorKind::Ascii))
        }
    }
    /// Display decoding is lazy and fallible, unlike URL construction.
    pub fn display_host(&self) -> Result<String, SourceUrlError> {
        if self.parts.host.contains("xn--") {
            ssx_source_codecs::idna2008::decode_display(&self.parts.host)
                .map_err(|_| invalid(SourceUrlErrorKind::Idna))
        } else {
            Ok(self.parts.host.clone())
        }
    }
    /// Canonical arbitrary signed source integer, never clamped/cast to u16.
    pub fn port(&self) -> Option<&str> {
        self.parts.port.as_deref()
    }
    /// httpcore URL.origin uses truthy port or scheme default, so explicit zero defaults.
    /// Unsupported schemes without a truthy port have no supported default here.
    pub fn effective_origin_port(&self) -> Option<&str> {
        match self.port() {
            Some(p) if p != "0" => Some(p),
            _ => match self.scheme() {
                "http" | "ws" => Some("80"),
                "https" | "wss" => Some("443"),
                "socks5" | "socks5h" => Some("1080"),
                _ => None,
            },
        }
    }
    /// Quoted raw source userinfo. This explicit accessor may contain credentials.
    pub fn raw_userinfo(&self) -> &str {
        &self.parts.userinfo
    }
    /// Whether the normalized username contains any bytes, independent of password.
    pub fn username_present(&self) -> bool {
        !self
            .parts
            .userinfo
            .split_once(':')
            .map_or(self.parts.userinfo.as_str(), |(a, _)| a)
            .is_empty()
    }
    /// Whether the normalized userinfo carries the first-colon password marker.
    pub fn password_present(&self) -> bool {
        self.parts.userinfo.contains(':')
    }
    /// Username with UTF-8 replacement unquote, exactly the source display boundary.
    pub fn username(&self) -> String {
        unquote(
            self.parts
                .userinfo
                .split_once(':')
                .map_or(self.parts.userinfo.as_str(), |(a, _)| a),
        )
    }
    /// Password with UTF-8 replacement unquote; no plus-to-space conversion.
    pub fn password(&self) -> String {
        unquote(self.parts.userinfo.split_once(':').map_or("", |(_, b)| b))
    }
    /// Source path without query; an empty path remains empty here.
    pub fn path(&self) -> &str {
        &self.parts.path
    }
    /// Preserve absent versus explicitly empty query internally and at this accessor.
    pub fn query_component(&self) -> Option<&str> {
        self.parts.query.as_deref()
    }
    /// Preserve absent versus explicitly empty fragment.
    pub fn fragment_component(&self) -> Option<&str> {
        self.parts.fragment.as_deref()
    }
    /// Public raw query bytes collapse absent and empty; component accessor retains them.
    pub fn query(&self) -> &[u8] {
        self.query_component().unwrap_or("").as_bytes()
    }
    /// Source public fragment display collapses None/empty and applies UTF-8 unquote.
    pub fn fragment(&self) -> String {
        unquote(self.fragment_component().unwrap_or(""))
    }
    /// Source netloc excludes userinfo, brackets IPv6, and retains explicit zero port.
    pub fn netloc(&self) -> String {
        self.parts.netloc()
    }
    /// Source netloc ASCII boundary, separate from its retained textual form.
    pub fn netloc_bytes(&self) -> Result<Vec<u8>, SourceUrlError> {
        let value = self.netloc();
        if value.is_ascii() {
            Ok(value.into_bytes())
        } else {
            Err(invalid(SourceUrlErrorKind::Ascii))
        }
    }
    /// Source wire-target text: (path or '/') plus every present query delimiter.
    /// Fragment/userinfo never enter this target. This does not authorize sending it.
    pub fn raw_path(&self) -> String {
        let mut out = if self.parts.path.is_empty() {
            "/".into()
        } else {
            self.parts.path.clone()
        };
        if let Some(q) = &self.parts.query {
            out.push('?');
            out.push_str(q);
        }
        out
    }
    /// Source __str__, including explicit empty ?/#. May contain credentials.
    pub fn serialized(&self) -> String {
        self.parts.serialized()
    }
    /// HTTPX considers a URL absolute only when BOTH scheme and host are nonempty.
    pub fn is_absolute(&self) -> bool {
        !self.parts.scheme.is_empty() && !self.parts.host.is_empty()
    }
    /// Copy current normalized components using source kwargs normalization.
    pub fn copy_with(&self, c: SourceUrlChanges<'_>) -> Result<Self, SourceUrlError> {
        if c.scheme.is_none()
            && c.authority.is_none()
            && c.userinfo.is_none()
            && c.host.is_none()
            && c.port.is_none()
            && c.path.is_none()
            && c.query.is_none()
            && c.fragment.is_none()
            && c.raw_path.is_none()
            && c.username.is_none()
            && c.password.is_none()
            && c.netloc.is_none()
        {
            return Ok(self.clone());
        }
        let scheme = c.scheme.unwrap_or(self.scheme());
        let authority = c
            .authority
            .map(str::to_owned)
            .unwrap_or_else(|| self.parts.authority());
        let (default_user, default_host, default_port) = split_authority(&authority);
        let mut user = c
            .userinfo
            .map(str::to_owned)
            .unwrap_or_else(|| default_user.into());
        let mut h = c
            .host
            .map(str::to_owned)
            .unwrap_or_else(|| default_host.into());
        let mut p = c
            .port
            .map(|v| v.unwrap_or("").to_owned())
            .unwrap_or_else(|| default_port.into());
        if let Some(netloc) = c.netloc {
            let (a, b) = netloc.split_once(':').unwrap_or((netloc, ""));
            h = a.into();
            p = b.into();
        }
        if c.username.is_some() || c.password.is_some() {
            let username = quote(c.username.unwrap_or(""), USERNAME_EXCLUDED);
            let password = quote(c.password.unwrap_or(""), USERNAME_EXCLUDED);
            user = if password.is_empty() {
                username
            } else {
                format!("{username}:{password}")
            };
        }
        if h.contains(':') && !(h.starts_with('[') && h.ends_with(']')) {
            h = format!("[{h}]");
        }
        let mut path = c.path.unwrap_or(self.path());
        let mut query = c.query.unwrap_or(self.query_component());
        if let Some(raw) = c.raw_path {
            let (a, b) = raw
                .split_once('?')
                .map_or((raw, None), |(a, b)| (a, Some(b)));
            path = a;
            query = b;
        }
        let fragment = c.fragment.unwrap_or(self.fragment_component());
        // Component guards run before host/port normalization, as source kwargs do.
        for text in [
            Some(scheme),
            Some(authority.as_str()),
            Some(user.as_str()),
            Some(h.as_str()),
            Some(p.as_str()),
            Some(path),
            query,
            fragment,
        ]
        .into_iter()
        .flatten()
        {
            validate(text)?;
        }
        if !scheme_valid(scheme)
            || authority.contains(['/', '?', '#'])
            || (c.username.is_none()
                && c.password.is_none()
                && c.userinfo.is_some_and(|u| u.contains('@')))
            || (!(h.starts_with('[') && h.ends_with(']')) && h.contains(':'))
            || path.contains(['?', '#'])
            || query.is_some_and(|q| q.contains('#'))
        {
            return Err(invalid(SourceUrlErrorKind::Component));
        }
        let parts = normalize(scheme, &user, &h, &p, path, query, fragment)?;
        let input = parts.serialized();
        Ok(Self {
            input,
            origin: SourceUrlOrigin::Copy,
            lexical_authority: !parts.authority().is_empty(),
            lexical_userinfo: !parts.userinfo.is_empty(),
            parts,
        })
    }
    /// Join through CPython's private split/unsplit, after reference construction.
    /// JoinBracket/JoinNetloc represent source ValueError, not constructor InvalidURL.
    pub fn join(&self, reference: &str) -> Result<Self, SourceUrlError> {
        let reference = Self::parse(reference)?;
        let joined = source_raw_urljoin(&self.serialized(), &reference.serialized())?;
        let mut result = Self::parse(&joined)?;
        result.origin = SourceUrlOrigin::Join;
        Ok(result)
    }
    /// HTTPX Client first enforces a trailing slash on raw_path (including query).
    pub fn client_base(&self) -> Result<Self, SourceUrlError> {
        let mut raw = self.raw_path();
        if raw.ends_with('/') {
            return Ok(self.clone());
        }
        raw.push('/');
        self.copy_with(SourceUrlChanges {
            raw_path: Some(&raw),
            ..Default::default()
        })
    }
    /// Client._merge_url appends raw_path for ANY URL lacking either scheme or host.
    /// Reference authority/scheme/fragment are not RFC-joined on that path.
    pub fn merge_client_reference(&self, reference: &str) -> Result<Self, SourceUrlError> {
        let r = Self::parse(reference)?;
        if r.is_absolute() {
            return Ok(r);
        }
        let base = self.client_base()?;
        let raw = format!(
            "{}{}",
            base.raw_path(),
            r.raw_path().trim_start_matches('/')
        );
        let mut out = base.copy_with(SourceUrlChanges {
            raw_path: Some(&raw),
            ..Default::default()
        })?;
        out.origin = SourceUrlOrigin::ClientMerge;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn url(s: &str) -> SourceUrl {
        SourceUrl::parse(s).unwrap()
    }
    // HTTPX _urlparse ParseResult.__str__/authority/netloc + _urls raw_path.
    #[test]
    fn complete_owner_separates_provenance_normalized_fields_and_wire_target() {
        let input = "HTTP://User:p%FF@EXAMPLE.test:080/a/./b/../c%2f?x=%2f#frag";
        let u = url(input);
        assert_eq!(u.input(), input);
        assert_eq!(
            u.serialized(),
            "http://User:p%FF@example.test:80/a/c%2f?x=%2f#frag"
        );
        assert_eq!(u.raw_path(), "/a/c%2f?x=%2f");
        assert_eq!(u.netloc(), "example.test:80");
        assert_eq!(u.username(), "User");
        assert_eq!(u.password(), "p\u{fffd}");
        assert!(u.password_present());
        assert!(!format!("{u:?}").contains("p%FF"));
        assert!(!format!("{u:?}").contains("x=%2f"));
        // Copy reconstructs with parsed lowercase scheme; default port now drops.
        assert_eq!(
            u.copy_with(SourceUrlChanges {
                path: Some("/next"),
                ..Default::default()
            })
            .unwrap()
            .serialized(),
            "http://User:p%FF@example.test/next?x=%2f#frag"
        );
        assert_eq!(
            u.copy_with(Default::default()).unwrap().serialized(),
            u.serialized()
        );
    }
    #[test]
    fn absent_empty_query_fragment_and_empty_authority_are_not_conflated() {
        let u = url("http://host?#");
        assert_eq!(u.path(), "");
        assert_eq!(u.raw_path(), "/?");
        assert_eq!(u.query_component(), Some(""));
        assert_eq!(u.fragment_component(), Some(""));
        assert_eq!(u.serialized(), "http://host?#");
        let absent = u
            .copy_with(SourceUrlChanges {
                query: Some(None),
                fragment: Some(None),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(absent.serialized(), "http://host");
        assert_eq!(absent.raw_path(), "/");
        let empty = url("//");
        assert!(empty.lexical_authority_present());
        assert_eq!(empty.serialized(), "");
        let user = url("http://@host");
        assert!(user.lexical_userinfo_present());
        assert_eq!(user.serialized(), "http://host");
    }
    #[test]
    fn quoting_literal_dot_segments_and_utf8_userinfo_follow_source_stages() {
        assert_eq!(
            url("https://bücher.example/é a/%2e/../x\\y?q=é #é").serialized(),
            "https://xn--bcher-kva.example/%C3%A9%20a/x\\y?q=%C3%A9%20#%C3%A9"
        );
        // Relative paths keep literal dot segments until a later absolute join.
        assert_eq!(url("a/../b").path(), "a/../b");
        assert_eq!(url("http://host/%2e/%2E%2e").path(), "/%2e/%2E%2e");
        let u = url("http://host")
            .copy_with(SourceUrlChanges {
                username: Some("a:b@c"),
                password: Some("p/q"),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(u.raw_userinfo(), "a%3Ab%40c:p%2Fq");
        assert_eq!(u.username(), "a:b@c");
        assert_eq!(u.password(), "p/q");
        let authority = url("http://host")
            .copy_with(SourceUrlChanges {
                authority: Some("a@b@other"),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(authority.serialized(), "http://a%40b@other");
        assert!(
            url("http://host")
                .copy_with(SourceUrlChanges {
                    userinfo: Some("a@b"),
                    ..Default::default()
                })
                .is_err()
        );
        let raw = u
            .copy_with(SourceUrlChanges {
                raw_path: Some("/p?"),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(raw.path(), "/p");
        assert_eq!(raw.query_component(), Some(""));
        assert!(
            u.copy_with(SourceUrlChanges {
                path: Some("/p?q"),
                ..Default::default()
            })
            .is_err()
        );
    }
    // httpcore2 URL.origin: truthy port OR default, not None-only default.
    #[test]
    fn signed_zero_default_and_arbitrary_integer_ports_never_become_u16() {
        for p in ["0", "-000", "+٠_٠"] {
            let u = url(&format!("http://host:{p}"));
            assert_eq!(u.port(), Some("0"));
            assert_eq!(u.netloc(), "host:0");
            assert_eq!(u.effective_origin_port(), Some("80"));
        }
        for p in ["-1", "65536", "999999999999999999999999999999999999999"] {
            let u = url(&format!("https://host:{p}"));
            assert_eq!(u.port(), Some(p));
            assert_eq!(u.effective_origin_port(), Some(p));
        }
        assert_eq!(url("https://host:00443").port(), None);
        assert_eq!(url("HTTPS://host:00443").port(), Some("443"));
        assert_eq!(url("custom://host:+١_٢٣٤٥٦").port(), Some("123456"));
        assert_eq!(
            SourceUrl::parse("http://host: ").unwrap_err().kind,
            SourceUrlErrorKind::Port
        );
        assert_eq!(
            SourceUrl::parse("http://host:1__2").unwrap_err().kind,
            SourceUrlErrorKind::Port
        );
        assert!(SourceUrl::parse(&format!("http://host:{}", "1".repeat(4300))).is_ok());
        assert_eq!(
            SourceUrl::parse(&format!("http://host:{}", "1".repeat(4301)))
                .unwrap_err()
                .kind,
            SourceUrlErrorKind::Port
        );
    }
    #[test]
    fn strict_idna_context_bidi_and_lazy_ascii_ace_display_stay_distinct() {
        assert_eq!(url("http://faß.de").raw_host(), "xn--fa-hia.de");
        assert_eq!(
            url("http://BÜCHER.example").display_host().unwrap(),
            "bücher.example"
        );
        assert_eq!(url("http://l·l").display_host().unwrap(), "l·l");
        for bad in [
            "http://a·a",
            "http://a\u{200d}b",
            "http://\u{088f}",
            "http://e\u{301}.example",
        ] {
            assert_eq!(
                SourceUrl::parse(bad).unwrap_err().kind,
                SourceUrlErrorKind::Idna
            );
        }
        // ASCII construction bypasses IDNA, including invalid ACE. Display is later.
        let ace = url("http://xn--.bad_label");
        assert_eq!(ace.raw_host(), "xn--.bad_label");
        assert!(ace.display_host().is_err());
        let scoped = url("http://[fe80::1%é]");
        assert!(scoped.raw_host_bytes().is_err());
        let ipv6 = url("http://[2001:0DB8::1%eth0]:0");
        assert_eq!(ipv6.raw_host(), "2001:0DB8::1%eth0");
        assert_eq!(ipv6.netloc(), "[2001:0DB8::1%eth0]:0");
    }
    #[test]
    fn source_length_controls_host_port_path_validation_precedence() {
        assert_eq!(
            SourceUrl::parse(&"é".repeat(65537)).unwrap_err().kind,
            SourceUrlErrorKind::Length
        );
        assert_eq!(
            SourceUrl::parse("http://[bad]:x/\n").unwrap_err().kind,
            SourceUrlErrorKind::Control
        );
        assert_eq!(
            SourceUrl::parse("http://[bad]:x").unwrap_err().kind,
            SourceUrlErrorKind::Ipv6
        );
        assert_eq!(
            SourceUrl::parse("http://001.2.3.4:x").unwrap_err().kind,
            SourceUrlErrorKind::Ipv4
        );
        assert_eq!(
            url("custom:")
                .copy_with(SourceUrlChanges {
                    host: Some("[bad]"),
                    port: Some(Some("x")),
                    path: Some("relative"),
                    ..Default::default()
                })
                .unwrap_err()
                .kind,
            SourceUrlErrorKind::Ipv6
        );
        assert_eq!(
            url("custom:")
                .copy_with(SourceUrlChanges {
                    host: Some("host"),
                    port: Some(Some("x")),
                    path: Some("relative"),
                    ..Default::default()
                })
                .unwrap_err()
                .kind,
            SourceUrlErrorKind::Port
        );
        assert_eq!(
            url("custom:")
                .copy_with(SourceUrlChanges {
                    host: Some("host"),
                    path: Some("relative"),
                    ..Default::default()
                })
                .unwrap_err()
                .kind,
            SourceUrlErrorKind::Path
        );
    }
    // CPython3.14.7 urllib.parse.urljoin582–650, called on str(URL(ref)).
    #[test]
    fn join_inherits_none_but_not_empty_delimiters_and_preserves_source_scheme_tables() {
        let base = url("https://host/a/b?old#prior");
        for (reference, expected) in [
            ("", "https://host/a/b?old#prior"),
            ("?", "https://host/a/b?"),
            ("#", "https://host/a/b?old#"),
            ("?new", "https://host/a/b?new"),
            ("../c", "https://host/c"),
            ("./", "https://host/a/"),
            ("/a//b", "https://host/a//b"),
            ("c//d", "https://host/a/c/d"),
            ("//other/x?", "https://other/x?"),
            ("http://other/x", "http://other/x"),
        ] {
            assert_eq!(
                base.join(reference).unwrap().serialized(),
                expected,
                "{reference}"
            );
        }
        // HTTPX's outer URL construction also runs on urllib early returns.
        assert_eq!(
            url("HTTP://host:80/a").join("").unwrap().serialized(),
            "http://host/a"
        );
        assert_eq!(
            url("").join("HTTP://host:80/a").unwrap().serialized(),
            "http://host/a"
        );
        assert_eq!(
            base.join("HTTP://other:80/a").unwrap().serialized(),
            "http://other/a"
        );
        assert_eq!(url("custom://host/a").join("b").unwrap().serialized(), "b");
        assert_eq!(
            url("http:base").join("next").unwrap().serialized(),
            "http:next"
        );
    }
    #[test]
    fn client_base_raw_path_merge_is_not_urljoin_or_authority_replacement() {
        let base = url("https://host/api");
        assert_eq!(
            base.client_base().unwrap().serialized(),
            "https://host/api/"
        );
        assert_eq!(
            base.join("/next").unwrap().serialized(),
            "https://host/next"
        );
        assert_eq!(
            base.merge_client_reference("/next").unwrap().serialized(),
            "https://host/api/next"
        );
        assert_eq!(
            base.merge_client_reference("//other/next#drop")
                .unwrap()
                .serialized(),
            "https://host/api/next"
        );
        assert_eq!(
            base.merge_client_reference("http:next")
                .unwrap()
                .serialized(),
            "https://host/api/next"
        );
        assert_eq!(
            base.merge_client_reference("https://other/next#keep")
                .unwrap()
                .serialized(),
            "https://other/next#keep"
        );
        assert_eq!(
            url("https://host/api?q")
                .client_base()
                .unwrap()
                .serialized(),
            "https://host/api?q/"
        );
        assert_eq!(
            url("https://host/api?q")
                .merge_client_reference("/next")
                .unwrap()
                .serialized(),
            "https://host/api?q/next"
        );
    }
    #[test]
    fn join_nfkc_value_errors_follow_both_nonempty_operand_splits_only() {
        let base = url("http://host/a");
        for input in ["http://[::1%／]/a", "http://[::1%℀]/a"] {
            let bad = url(input); // Source construction remains valid.
            assert_eq!(
                bad.join("b").unwrap_err().kind,
                SourceUrlErrorKind::JoinNetloc
            );
            assert_eq!(
                base.join(input).unwrap_err().kind,
                SourceUrlErrorKind::JoinNetloc
            );
            assert_eq!(
                bad.join("mailto:x").unwrap_err().kind,
                SourceUrlErrorKind::JoinNetloc
            );
            assert_eq!(bad.join("").unwrap().serialized(), bad.serialized());
            assert_eq!(url("").join(input).unwrap().serialized(), bad.serialized());
        }
    }
    #[test]
    fn join_bracket_prefix_value_errors_do_not_validate_the_colon_port_tail() {
        let base = url("http://host/a");
        for input in ["http://[::1%x]y]/a", "http://[::1%]:y]/a"] {
            let bad = url(input);
            assert_eq!(
                bad.join("b").unwrap_err().kind,
                SourceUrlErrorKind::JoinBracket
            );
            assert_eq!(
                base.join(input).unwrap_err().kind,
                SourceUrlErrorKind::JoinBracket
            );
            assert_eq!(bad.join("").unwrap().serialized(), bad.serialized());
            assert_eq!(url("").join(input).unwrap().serialized(), bad.serialized());
        }
        // First closing-bracket tail only needs ':', even if not a port integer.
        let allowed = "http://[::1%x]:y]/a";
        assert_eq!(
            url(allowed).join("b").unwrap().serialized(),
            "http://[::1%x]:y]/b"
        );
        assert_eq!(base.join(allowed).unwrap().serialized(), allowed);
        // Bracket checks precede the NFKC netloc check.
        assert_eq!(
            url("http://[::1%／]y]/a").join("b").unwrap_err().kind,
            SourceUrlErrorKind::JoinBracket
        );
    }
    #[test]
    fn join_resplits_serialized_authority_scheme_and_semicolon_paths() {
        let u = url("http:////x");
        assert_eq!(u.raw_host(), "");
        assert_eq!(u.path(), "//x");
        assert_eq!(u.serialized(), "http://x");
        assert_eq!(u.join("b").unwrap().serialized(), "http://x/b");
        assert_eq!(url("http:////").join("b").unwrap().serialized(), "http:/b");
        assert_eq!(url(":foo").serialized(), "foo");
        assert_eq!(
            url("http://host/a/b").join(":foo").unwrap().serialized(),
            "http://host/a/foo"
        );
        let copied = url("a")
            .copy_with(SourceUrlChanges {
                path: Some("http:a/b"),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(copied.scheme(), "");
        assert_eq!(copied.serialized(), "http:a/b");
        assert_eq!(copied.join("/c").unwrap().serialized(), "http:/c");
        let params = url("http://host/a;b/c;d?old");
        assert_eq!(
            params.join(";p").unwrap().serialized(),
            "http://host/a;b/;p"
        );
        assert_eq!(
            params.join("?new").unwrap().serialized(),
            "http://host/a;b/c;d?new"
        );
        assert_eq!(
            url("file:///a/b").join("/c").unwrap().serialized(),
            "file:/c"
        );
    }
}

impl hyper::ext::SourceIdentity for SourceUrl {
    fn scheme(&self) -> &str {
        self.scheme()
    }
    fn raw_host(&self) -> &str {
        self.raw_host()
    }
    fn origin_port(&self) -> Option<&str> {
        self.effective_origin_port()
    }
    fn explicit_port(&self) -> Option<&str> {
        self.port()
    }
    fn authority(&self) -> String {
        self.netloc()
    }
    fn origin_target(&self) -> String {
        self.raw_path()
    }
}

#[cfg(test)]
mod source_wire_identity_tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn ipv6_host_header_proxy_target_and_connect_port_have_distinct_source_forms() {
        let owner = Arc::new(SourceUrl::parse("http://u:p@[2001:0DB8::1]:0/a?#fragment").unwrap());
        let identity = hyper::ext::SourceRequestIdentity(owner.clone());
        assert_eq!(identity.0.authority(), "[2001:0DB8::1]:0");
        let wire = |form| {
            hyper::ext::SourceRequestTarget {
                identity: identity.clone(),
                form,
            }
            .bytes()
            .unwrap()
        };
        assert_eq!(wire(hyper::ext::SourceTargetForm::Origin), b"/a?");
        assert_eq!(
            wire(hyper::ext::SourceTargetForm::Absolute),
            b"http://2001:0DB8::1:0/a?"
        );
        assert_eq!(
            wire(hyper::ext::SourceTargetForm::Connect),
            b"2001:0DB8::1:80"
        );
        let expected: Arc<dyn hyper::ext::SourceIdentity> = owner;
        assert!(Arc::ptr_eq(&identity.0, &expected));
    }
}

#[cfg(test)]
mod raw_reference_tests {
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
        assert!(source_raw_urlparse("http://[::1").is_err());
        assert!(source_raw_urlparse("http://exa／mple/").is_err());
        assert!(SourceUrl::parse("\0http://Host/").is_err());
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
