//! Python urlparse-compatible structural splitting; never uses a WHATWG URL rewrite.
#[derive(Debug)]
pub(crate) struct Parsed {
    pub scheme: String,
    pub authority: String,
    pub path: String,
    pub params: String,
    pub query: String,
    pub fragment: String,
    pub host: String,
    pub port: Option<String>,
    pub userinfo: bool,
}
pub(crate) fn parse(raw: &str) -> Option<Parsed> {
    let cleaned: String = raw
        .trim_start_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\n' | '\r' | '\t'))
        .collect();
    let (scheme, rest) = match cleaned.split_once(':') {
        Some((s, r))
            if s.starts_with(|c: char| c.is_ascii_alphabetic())
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) =>
        {
            (s.to_ascii_lowercase(), r)
        }
        _ => (String::new(), cleaned.as_str()),
    };
    let (before_fragment, fragment) = rest.split_once('#').unwrap_or((rest, ""));
    let (before_query, query) = before_fragment
        .split_once('?')
        .unwrap_or((before_fragment, ""));
    let (authority, path) = if let Some(r) = before_query.strip_prefix("//") {
        let end = r.find('/').unwrap_or(r.len());
        (&r[..end], &r[end..])
    } else {
        ("", before_query)
    };
    // urllib urlparse separates parameters only for its uses_params schemes,
    // and only at the first semicolon in the final path segment. An empty
    // params delimiter disappears during urlunparse; earlier segments survive.
    let params_start = if USES_PARAMS.contains(&scheme.as_str()) {
        let segment = path.rfind('/').unwrap_or(0);
        path[segment..].find(';').map(|i| segment + i)
    } else {
        None
    };
    let (path, params) = params_start.map_or((path, ""), |i| (&path[..i], &path[i + 1..]));
    let userinfo = authority.contains('@');
    let hostport = authority.rsplit('@').next().unwrap_or("");
    let (host, port) = if hostport.starts_with('[') {
        let end = hostport.find(']')?;
        let host = &hostport[1..end];
        // Python validates bracketed literals before scheme classification.
        let scoped_v6 = host.split_once('%').is_some_and(|(address, scope)| {
            !scope.is_empty()
                && !scope.contains('%')
                && address.parse::<std::net::Ipv6Addr>().is_ok()
        });
        if host.parse::<std::net::Ipv6Addr>().is_err() && !scoped_v6 && !ipvfuture(host) {
            return None;
        }
        let tail = &hostport[end + 1..];
        if !tail.is_empty() && !tail.starts_with(':') {
            return None;
        }
        (host, tail.strip_prefix(':').map(str::to_owned))
    } else {
        if hostport.contains(['[', ']']) {
            return None;
        }
        let (host, port) = hostport
            .split_once(':')
            .map_or((hostport, None), |(h, p)| (h, Some(p.to_owned())));
        (host, port)
    };
    // These compatibility characters decompose into authority delimiters.
    if authority.contains([
        '⁇', '⁈', '⁉', '℀', '℁', '℅', '℆', '⩴', '︓', '︖', '﹕', '﹖', '﹟', '﹫', '＃', '／',
        '：', '？', '＠',
    ]) {
        return None;
    }
    Some(Parsed {
        scheme,
        authority: authority.into(),
        path: path.into(),
        params: params.into(),
        query: query.into(),
        fragment: fragment.into(),
        host: host.to_lowercase(),
        port,
        userinfo,
    })
}
/// Six urllib-compatible components for the HTTP formatter boundary.
/// The caller owns the returned strings and metadata/projection lifetime.
pub fn parsed_url(raw: &str) -> Option<Vec<String>> {
    let parsed = parse(raw)?;
    Some(vec![
        parsed.scheme,
        parsed.authority,
        parsed.path,
        parsed.params,
        parsed.query,
        parsed.fragment,
    ])
}
/// Exact lowercase tracking keys only. Repeated values are grouped at first key encounter.
pub fn normalise_url(raw: &str) -> String {
    let Some(p) = parse(raw) else {
        return raw.into();
    };
    let mut pairs: Vec<(String, Vec<String>)> = Vec::new();
    for (key, value) in url::form_urlencoded::parse(p.query.as_bytes()) {
        if [
            "utm_source",
            "utm_medium",
            "utm_campaign",
            "utm_term",
            "utm_content",
            "fbclid",
            "gclid",
        ]
        .contains(&key.as_ref())
        {
            continue;
        }
        if let Some((_, values)) = pairs.iter_mut().find(|(k, _)| k == &key) {
            values.push(value.into_owned());
        } else {
            pairs.push((key.into_owned(), vec![value.into_owned()]));
        }
    }
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, values) in pairs {
        for value in values {
            serializer.append_pair(&key, &value);
        }
    }
    let query = serializer.finish().replace("%7E", "~").replace('*', "%2A");
    let mut output = String::new();
    if !p.scheme.is_empty() {
        output.push_str(&p.scheme);
        output.push(':');
    }
    let authority_present = !p.authority.is_empty()
        || !p.scheme.is_empty()
            && USES_NETLOC.contains(&p.scheme.as_str())
            && (p.path.is_empty() || p.path.starts_with('/'));
    if authority_present {
        output.push_str("//");
        output.push_str(&p.authority);
    }
    let mut path = p.path;
    if !p.params.is_empty() {
        path.push(';');
        path.push_str(&p.params);
    }
    if authority_present && !path.is_empty() && !path.starts_with('/') {
        output.push('/');
    }
    output.push_str(&path);
    if !query.is_empty() {
        output.push('?');
        output.push_str(&query);
    }
    if !p.fragment.is_empty() {
        output.push('#');
        output.push_str(&p.fragment);
    }
    output
}

fn ipvfuture(host: &str) -> bool {
    let Some(rest) = host.strip_prefix(['v', 'V']) else {
        return false;
    };
    let Some((version, address)) = rest.split_once('.') else {
        return false;
    };
    !version.is_empty() && version.bytes().all(|b| b.is_ascii_hexdigit()) && !address.is_empty()
}

// Python 3.14 urllib.parse scheme tables, captured by the F11 source oracle.
const USES_PARAMS: &[&str] = &[
    "", "ftp", "hdl", "prospero", "http", "imap", "https", "shttp", "rtsp", "rtsps", "rtspu",
    "sip", "sips", "mms", "sftp", "tel",
];
const USES_NETLOC: &[&str] = &[
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
