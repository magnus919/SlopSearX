//! Structural handoff classification, without DNS, sockets, or HTTP.
use crate::urls::parse;
use std::net::{Ipv4Addr, Ipv6Addr};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retrieval {
    pub status: &'static str,
    pub reason: Option<String>,
    pub scheme: Option<String>,
    pub url: Option<String>,
}
fn component(s: &str) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    let (digits, base) = if s.starts_with("0x") || s.starts_with("0X") {
        (&s[2..], 16)
    } else if s.starts_with('0') {
        (s, 8)
    } else {
        (s, 10)
    };
    if digits.is_empty() {
        return Some(0);
    }
    if !digits.bytes().all(|b| match base {
        16 => b.is_ascii_hexdigit(),
        8 => (b'0'..=b'7').contains(&b),
        _ => b.is_ascii_digit(),
    }) {
        return None;
    }
    u32::from_str_radix(digits, base).ok()
}
/// WHATWG and native inet_aton numeric grammar, including octal/hex and abbreviated final octets.
/// Unlike ToSocketAddrs, this cannot resolve a DNS name or perform I/O.
fn numeric_v4(host: &str) -> Option<Ipv4Addr> {
    let host = host.strip_suffix('.').unwrap_or(host);
    let parts = host.split('.').map(component).collect::<Option<Vec<_>>>()?;
    if parts.is_empty() || parts.len() > 4 {
        return None;
    }
    let last = *parts.last()? as u64;
    if last >= 1u64 << (8 * (5 - parts.len())) || parts[..parts.len() - 1].iter().any(|v| *v > 255)
    {
        return None;
    }
    let mut value = last;
    for (i, n) in parts[..parts.len() - 1].iter().enumerate() {
        value += (*n as u64) << (8 * (3 - i));
    }
    Some(Ipv4Addr::from(value as u32))
}
fn in4(ip: u32, base: u32, bits: u32) -> bool {
    ip >> (32 - bits) == base >> (32 - bits)
}
fn global4(ip: Ipv4Addr) -> bool {
    let n = u32::from(ip);
    if n == 0xc0000009 || n == 0xc000000a {
        return true;
    }
    ![
        (0, 8),
        (0x0a000000, 8),
        (0x64400000, 10),
        (0x7f000000, 8),
        (0xa9fe0000, 16),
        (0xac100000, 12),
        (0xc0000000, 24),
        (0xc0000200, 24),
        (0xc0a80000, 16),
        (0xc6120000, 15),
        (0xc6336400, 24),
        (0xcb007100, 24),
        (0xf0000000, 4),
    ]
    .iter()
    .any(|&(b, k)| in4(n, b, k))
}
fn in6(ip: u128, base: &str, bits: u32) -> bool {
    ip >> (128 - bits)
        == u128::from(base.parse::<Ipv6Addr>().expect("static prefix")) >> (128 - bits)
}
fn global6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return global4(v4);
    }
    let n = u128::from(ip);
    if [
        ("2001:1::1", 128),
        ("2001:1::2", 128),
        ("2001:3::", 32),
        ("2001:4:112::", 48),
        ("2001:20::", 28),
        ("2001:30::", 28),
    ]
    .iter()
    .any(|&(b, k)| in6(n, b, k))
    {
        return true;
    }
    ![
        ("::", 128),
        ("::1", 128),
        ("64:ff9b:1::", 48),
        ("100::", 64),
        ("2001::", 23),
        ("2001:db8::", 32),
        ("2002::", 16),
        ("3fff::", 20),
        ("fc00::", 7),
        ("fe80::", 10),
    ]
    .iter()
    .any(|&(b, k)| in6(n, b, k))
}
fn reserved6(ip: Ipv6Addr) -> bool {
    let n = u128::from(ip);
    [
        ("::", 8),
        ("100::", 8),
        ("200::", 7),
        ("400::", 6),
        ("800::", 5),
        ("1000::", 4),
        ("4000::", 3),
        ("6000::", 3),
        ("8000::", 3),
        ("a000::", 3),
        ("c000::", 3),
        ("e000::", 4),
        ("f000::", 5),
        ("f800::", 6),
        ("fe00::", 9),
    ]
    .iter()
    .any(|&(b, k)| in6(n, b, k))
}
pub fn classify_retrieval_url(raw: &str) -> Retrieval {
    let make = |status, reason: Option<&str>, scheme: Option<&str>, url: Option<&str>| Retrieval {
        status,
        reason: reason.map(str::to_owned),
        scheme: scheme.map(str::to_owned),
        url: url.map(str::to_owned),
    };
    if raw.is_empty() || raw.chars().all(ssx_json::python_whitespace) {
        return make("missing", Some("result has no URL to retrieve"), None, None);
    }
    let Some(p) = parse(raw) else {
        return make(
            "ambiguous",
            Some("URL cannot be parsed unambiguously"),
            None,
            None,
        );
    };
    if p.scheme.is_empty() {
        return make(
            "ambiguous",
            Some("URL has no scheme; canonicalization is ambiguous"),
            None,
            None,
        );
    }
    let scheme = Some(p.scheme.as_str());
    let fail = |reason| make("ambiguous", Some(reason), scheme, None);
    if !["http", "https"].contains(&p.scheme.as_str()) {
        let unsafe_scheme = ["file", "data", "javascript", "vbscript", "gopher", "ftp"]
            .contains(&p.scheme.as_str());
        let reason = if unsafe_scheme {
            format!(
                "scheme '{}' is unsafe for downstream HTTP retrieval",
                p.scheme
            )
        } else {
            format!(
                "scheme '{}' is not HTTP(S); not retrievable over HTTP",
                p.scheme
            )
        };
        return make(
            if unsafe_scheme {
                "unsafe_scheme"
            } else {
                "non_http"
            },
            Some(&reason),
            scheme,
            None,
        );
    }
    if p.host.is_empty() {
        return fail("URL has no host; canonicalization is ambiguous");
    }
    if p.authority
        .chars()
        .any(|c| c == '\\' || c < ' ' || c == '\u{7f}' || ssx_json::python_whitespace(c))
    {
        return fail(
            "URL authority contains a backslash, whitespace, or control character; canonicalization is ambiguous",
        );
    }
    if p.userinfo {
        return fail(
            "URL authority contains userinfo credentials; not handed off as a fetch target",
        );
    }
    if p.host.contains('%') || !p.host.is_ascii() {
        return fail(
            "URL host contains a percent-encoded or non-ASCII character; canonicalization is ambiguous",
        );
    }
    let reserved =
        "URL host is a reserved or translation-prefix IP address; not a safe fetch target";
    let nonglobal = "URL host is a non-global literal IP address; not a safe fetch target";
    if let Ok(ip) = p.host.parse::<Ipv6Addr>() {
        let n = u128::from(ip);
        if in6(n, "2002::", 16)
            || in6(n, "fec0::", 10)
            || ip.to_ipv4_mapped().is_some_and(|v| !global4(v))
        {
            return fail(reserved);
        }
        if !global6(ip) {
            return fail(nonglobal);
        }
        if ip.to_ipv4_mapped().is_none() && reserved6(ip) {
            return fail(reserved);
        }
    } else {
        // Linux AI_NUMERICHOST's IPv4 grammar is inet_aton: bounded
        // decimal/hex/octal components and abbreviated final octets. WHATWG
        // covers these forms and additionally accepts its own trailing dot
        // and bare 0x zero. Do not import Darwin-only decimal or overflow
        // candidates: they can loosen Linux eligibility for malformed input.
        if let Some(ip) = numeric_v4(&p.host) {
            if !global4(ip) {
                return fail(nonglobal);
            }
        } else if p.host.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return fail(
                "URL host is a malformed dotted numeric literal; canonicalization is ambiguous",
            );
        }
    }
    if let Some(port) = p.port.filter(|p| !p.is_empty()) {
        if !port.bytes().all(|b| b.is_ascii_digit())
            || port.parse::<u32>().map_or(true, |n| n > 65535)
        {
            return fail("URL port is invalid; canonicalization is ambiguous");
        }
        if port.parse::<u32>() == Ok(0) {
            return fail(
                "URL port is outside the sane range (1-65535); canonicalization is ambiguous",
            );
        }
    }
    make("ok", None, scheme, Some(raw))
}
