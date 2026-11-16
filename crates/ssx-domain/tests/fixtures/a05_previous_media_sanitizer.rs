// Test-only exact previous native algorithm, extracted from the guarded A05 baseline.
// Production has one shared algorithm in src/media_url.rs.
#[derive(serde::Deserialize)]
struct PreviousTables { nfkc_url_delimiters: Vec<u32> }
fn python_text_tables()-> &'static PreviousTables {
    static TABLE:std::sync::OnceLock<PreviousTables>=std::sync::OnceLock::new();
    TABLE.get_or_init(||serde_json::from_str(include_str!("../../../../contracts/unicode/python-text-16.0.0.json")).unwrap())
}
// Mirrors CPython 3.14 urllib.parse's string URL splitting, validation and
// reconstruction used by adapter.sanitize_url; no WHATWG URL reinterpretation.
struct MediaUrlParts<'a> {
    scheme: Option<String>,
    netloc: Option<&'a str>,
    path: &'a str,
    params: Option<&'a str>,
    query: Option<&'a str>,
    fragment: Option<&'a str>,
}
fn split_media_url(value: &str) -> Option<MediaUrlParts<'_>> {
    let (scheme, mut remainder) = if let Some(colon) = value.find(':').filter(|index| *index > 0) {
        let candidate = &value[..colon];
        if candidate.as_bytes()[0].is_ascii_alphabetic()
            && candidate
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"+-.".contains(&byte))
        {
            (Some(candidate.to_ascii_lowercase()), &value[colon + 1..])
        } else {
            (None, value)
        }
    } else {
        (None, value)
    };
    let netloc = if let Some(after) = remainder.strip_prefix("//") {
        let end = after.find(['/', '?', '#']).unwrap_or(after.len());
        remainder = &after[end..];
        Some(&after[..end])
    } else {
        None
    };
    if let Some(netloc) = netloc
        && !valid_urllib_netloc(netloc)
    {
        return None;
    }
    let fragment = if let Some((before, fragment)) = remainder.split_once('#') {
        remainder = before;
        Some(fragment)
    } else {
        None
    };
    let query = if let Some((before, query)) = remainder.split_once('?') {
        remainder = before;
        Some(query)
    } else {
        None
    };
    const USES_PARAMS: &[&str] = &[
        "", "ftp", "hdl", "prospero", "http", "imap", "https", "shttp", "rtsp", "rtsps", "rtspu",
        "sip", "sips", "mms", "sftp", "tel",
    ];
    let params = if USES_PARAMS.contains(&scheme.as_deref().unwrap_or("")) {
        let last_segment = remainder.rfind('/').unwrap_or(0);
        if let Some(index) = remainder[last_segment..]
            .find(';')
            .map(|index| last_segment + index)
        {
            let params = &remainder[index + 1..];
            remainder = &remainder[..index];
            Some(params)
        } else {
            None
        }
    } else {
        None
    };
    Some(MediaUrlParts {
        scheme,
        netloc,
        path: remainder,
        params,
        query,
        fragment,
    })
}
fn valid_urllib_netloc(netloc: &str) -> bool {
    let open = netloc.contains('[');
    let close = netloc.contains(']');
    if open != close {
        return false;
    }
    if open {
        let hostname_and_port = netloc.rsplit_once('@').map_or(netloc, |(_, host)| host);
        let hostname = if let Some((before, bracketed)) = hostname_and_port.split_once('[') {
            if !before.is_empty() {
                return false;
            }
            let Some((hostname, port)) = bracketed.split_once(']') else {
                return false;
            };
            if !port.is_empty() && !port.starts_with(':') {
                return false;
            }
            hostname
        } else {
            hostname_and_port
                .split_once(':')
                .map_or(hostname_and_port, |(host, _)| host)
        };
        if hostname.starts_with(['v', 'V']) {
            let Some((version, address)) = hostname[1..].split_once('.') else {
                return false;
            };
            if version.is_empty()
                || !version.bytes().all(|byte| byte.is_ascii_hexdigit())
                || address.is_empty()
            {
                return false;
            }
        } else {
            let address = if let Some((address, scope)) = hostname.split_once('%') {
                if scope.is_empty() || scope.contains('%') {
                    return false;
                }
                address
            } else {
                hostname
            };
            if address.parse::<std::net::Ipv6Addr>().is_err() {
                return false;
            }
        }
    }
    // NFKC cannot compose an ASCII URL delimiter from multiple codepoints.
    // The reviewed Unicode16 table contains every non-ASCII character whose
    // full NFKC expansion introduces one; literal @ : # ? are ignored by urllib.
    !netloc.chars().any(|ch| {
        python_text_tables()
            .nfkc_url_delimiters
            .binary_search(&(ch as u32))
            .is_ok()
    })
}
fn reconstruct_media_url(parts: MediaUrlParts<'_>, query: &str) -> String {
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
    let netloc = if parts.netloc.is_some_and(|value| !value.is_empty()) {
        parts.netloc
    } else if parts
        .scheme
        .as_deref()
        .is_some_and(|scheme| USES_NETLOC.contains(&scheme))
        && (parts.path.is_empty() || parts.path.starts_with('/'))
    {
        Some("")
    } else {
        None
    };
    let mut path = parts.path.to_owned();
    if let Some(params) = parts.params.filter(|params| !params.is_empty()) {
        path.push(';');
        path.push_str(params);
    }
    if let Some(netloc) = netloc {
        if !path.is_empty() && !path.starts_with('/') {
            path.insert(0, '/');
        }
        path = format!("//{netloc}{path}");
    } else if path.starts_with("//") {
        path.insert_str(0, "//");
    }
    if let Some(scheme) = parts.scheme {
        path = format!("{scheme}:{path}");
    }
    if !query.is_empty() {
        path.push('?');
        path.push_str(query);
    }
    if let Some(fragment) = parts.fragment.filter(|fragment| !fragment.is_empty()) {
        path.push('#');
        path.push_str(fragment);
    }
    path
}
pub(super) fn sanitize_media_url(original: &str) -> String {
    let cleaned = original
        .trim_start_matches(|ch: char| (ch as u32) <= 32)
        .replace(['\t', '\r', '\n'], "");
    let Some(parts) = split_media_url(&cleaned) else {
        return original.to_owned();
    };
    let Some(query) = parts.query.filter(|query| !query.is_empty()) else {
        return original.trim_end_matches('?').to_owned();
    };
    let sensitive = ["api_key", "key", "apiKey", "token", "access_token"];
    let mut grouped: Vec<(String, Vec<String>)> = Vec::new();
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        if sensitive.contains(&key.as_ref()) {
            continue;
        }
        if let Some((_, values)) = grouped.iter_mut().find(|(existing, _)| existing == &key) {
            values.push(value.into_owned());
        } else {
            grouped.push((key.into_owned(), vec![value.into_owned()]));
        }
    }
    let mut serialized = String::new();
    let mut first_pair = true;
    for (key, values) in grouped {
        for value in values {
            if !first_pair {
                serialized.push('&');
            }
            first_pair = false;
            append_python_quote_plus(&key, &mut serialized);
            serialized.push('=');
            append_python_quote_plus(&value, &mut serialized);
        }
    }
    reconstruct_media_url(parts, &serialized)
}

fn append_python_quote_plus(value: &str, out: &mut String) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"_.-~".contains(&byte) {
            out.push(char::from(byte));
        } else if byte == b' ' {
            out.push('+');
        } else {
            out.push('%');
            out.push(char::from(HEX[(byte >> 4) as usize]));
            out.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }
}

