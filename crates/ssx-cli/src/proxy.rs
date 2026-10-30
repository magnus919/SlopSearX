//! Source urllib environment and OS proxy policy. OS access is read-only.
//! CPython3.13.6 urllib.request/fnmatch, PSF license retained in T08A sources.
use crate::Failure;
use std::{collections::BTreeMap, ffi::OsString};
#[derive(Default)]
struct Settings {
    proxies: BTreeMap<String, String>,
    bypass: Bypass,
}
#[derive(Default)]
enum Bypass {
    #[default]
    None,
    Environment(String),
    #[cfg(any(target_os = "macos", test))]
    Mac {
        simple: bool,
        exceptions: Vec<String>,
    },
    #[cfg(any(windows, test))]
    Windows(String),
}
pub struct Policy(Settings);
fn environment_values(
    variables: Vec<(OsString, OsString)>,
    cgi: bool,
) -> Result<(BTreeMap<String, String>, bool), Failure> {
    let mut raw = BTreeMap::<String, OsString>::new();
    for (key, value) in &variables {
        let Some(key) = key.to_str() else { continue };
        if let Some(name) = key.to_lowercase().strip_suffix("_proxy")
            && !value.is_empty()
        {
            raw.insert(name.into(), value.clone());
        }
    }
    if cgi {
        raw.remove("http");
    }
    for (key, value) in &variables {
        let Some(key) = key.to_str() else { continue };
        if let Some(name) = key.strip_suffix("_proxy") {
            let name = name.to_lowercase();
            if value.is_empty() {
                raw.remove(&name);
            } else {
                raw.insert(name, value.clone());
            }
        }
    }
    // Even an unknown nonempty *_proxy key disables the source OS fallback.
    let present = !raw.is_empty();
    let mut out = BTreeMap::new();
    for name in ["http", "https", "no"] {
        if let Some(value) = raw.remove(name) {
            out.insert(
                name.into(),
                value.into_string().map_err(|_| Failure::Transport)?,
            );
        }
    }
    Ok((out, present))
}
impl Policy {
    pub fn from_environment() -> Result<Self, Failure> {
        let (mut proxies, present) = environment_values(
            std::env::vars_os().collect(),
            std::env::var_os("REQUEST_METHOD").is_some(),
        )?;
        if present {
            let no = proxies.remove("no").unwrap_or_default();
            Ok(Self(Settings {
                proxies,
                bypass: Bypass::Environment(no),
            }))
        } else {
            Ok(Self(system()?))
        }
    }
    pub fn route(&self, scheme: &str, authority: &str) -> Result<Option<String>, Failure> {
        let bypass = match &self.0.bypass {
            Bypass::None => false,
            Bypass::Environment(no) => environment_bypass(authority, no),
            #[cfg(any(target_os = "macos", test))]
            Bypass::Mac { simple, exceptions } => mac_bypass(authority, *simple, exceptions),
            #[cfg(any(windows, test))]
            Bypass::Windows(overrides) => windows_bypass(authority, overrides),
        };
        Ok(if bypass {
            None
        } else {
            self.0.proxies.get(scheme).cloned()
        })
    }
}
fn strip_port(host: &str) -> &str {
    host.rsplit_once(':')
        .filter(|(_, port)| port.bytes().all(|b| b.is_ascii_digit()))
        .map_or(host, |(h, _)| h)
}
fn environment_bypass(authority: &str, no: &str) -> bool {
    if no == "*" {
        return true;
    }
    let full = authority.to_lowercase();
    let host = strip_port(&full);
    no.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .any(|s| {
            let s = s.trim_start_matches('.').to_lowercase();
            host == s
                || full == s
                || host.ends_with(&format!(".{s}"))
                || full.ends_with(&format!(".{s}"))
        })
}
#[cfg(any(target_os = "macos", windows, test))]
#[derive(Debug)]
enum Token {
    Star,
    Any,
    Literal(char),
    Class {
        negative: bool,
        ranges: Vec<(char, char)>,
    },
}
// Source fnmatch's bracket/range cleanup, represented as tokens rather than a
// custom regex engine. Iterative glob matching avoids recursion and backtracking.
#[cfg(any(target_os = "macos", windows, test))]
fn tokens(pattern: &str) -> Vec<Token> {
    let pat = pattern.chars().collect::<Vec<_>>();
    let mut out = Vec::new();
    let mut i = 0;
    while i < pat.len() {
        let c = pat[i];
        i += 1;
        match c {
            '*' => {
                if !matches!(out.last(), Some(Token::Star)) {
                    out.push(Token::Star)
                }
            }
            '?' => out.push(Token::Any),
            '[' => {
                let mut j = i;
                if pat.get(j) == Some(&'!') {
                    j += 1
                }
                if pat.get(j) == Some(&']') {
                    j += 1
                }
                while j < pat.len() && pat[j] != ']' {
                    j += 1
                }
                if j == pat.len() {
                    out.push(Token::Literal('['));
                    continue;
                }
                let mut chunks = Vec::<Vec<char>>::new();
                if pat[i..j].contains(&'-') {
                    let mut k = i + if pat[i] == '!' { 2 } else { 1 };
                    while let Some(offset) =
                        pat.get(k..j).and_then(|s| s.iter().position(|c| *c == '-'))
                    {
                        k += offset;
                        chunks.push(pat[i..k].to_vec());
                        i = k + 1;
                        k += 3;
                    }
                    if i < j {
                        chunks.push(pat[i..j].to_vec())
                    } else if let Some(last) = chunks.last_mut() {
                        last.push('-')
                    }
                    for k in (1..chunks.len()).rev() {
                        if chunks[k - 1].last() > chunks[k].first() {
                            let mut joined = chunks[k - 1][..chunks[k - 1].len() - 1].to_vec();
                            joined.extend_from_slice(&chunks[k][1..]);
                            chunks[k - 1] = joined;
                            chunks.remove(k);
                        }
                    }
                } else {
                    chunks.push(pat[i..j].to_vec())
                }
                i = j + 1;
                let mut chars = Vec::new();
                for (index, chunk) in chunks.into_iter().enumerate() {
                    if index != 0 {
                        chars.push(('-', true))
                    }
                    chars.extend(chunk.into_iter().map(|c| (c, false)));
                }
                let negative = chars.first() == Some(&('!', false));
                if negative {
                    chars.remove(0);
                }
                let mut ranges = Vec::new();
                let mut k = 0;
                while k < chars.len() {
                    if k + 2 < chars.len() && chars[k + 1].1 {
                        ranges.push((chars[k].0, chars[k + 2].0));
                        k += 3
                    } else {
                        ranges.push((chars[k].0, chars[k].0));
                        k += 1
                    }
                }
                out.push(Token::Class { negative, ranges });
            }
            c => out.push(Token::Literal(c)),
        }
    }
    out
}
#[cfg(any(target_os = "macos", windows, test))]
fn glob(host: &str, pattern: &str, windows: bool) -> bool {
    let normalized = |s: &str| {
        if windows {
            s.to_lowercase().replace('/', "\\")
        } else {
            s.into()
        }
    };
    let host = normalized(host).chars().collect::<Vec<_>>();
    let pattern = tokens(&normalized(pattern));
    let mut previous = vec![false; host.len() + 1];
    previous[0] = true;
    for token in pattern {
        let mut next = vec![false; host.len() + 1];
        if matches!(token, Token::Star) {
            next[0] = previous[0];
            for i in 1..next.len() {
                next[i] = previous[i] || next[i - 1];
            }
        } else {
            for (i, c) in host.iter().enumerate() {
                let admitted = match &token {
                    Token::Any => true,
                    Token::Literal(v) => v == c,
                    Token::Class { negative, ranges } => {
                        ranges.iter().any(|(a, b)| a <= c && c <= b) != *negative
                    }
                    Token::Star => unreachable!(),
                };
                next[i + 1] = previous[i] && admitted;
            }
        }
        previous = next;
    }
    previous[host.len()]
}
#[cfg(any(windows, test))]
fn windows_bypass(host: &str, overrides: &str) -> bool {
    let host = strip_port(host);
    overrides.split(';').map(str::trim).any(|item| {
        if item == "<local>" {
            !host.contains('.')
        } else {
            glob(host, item, true)
        }
    })
}
#[cfg(any(target_os = "macos", test))]
fn mac_bypass(host: &str, simple: bool, exceptions: &[String]) -> bool {
    if simple && !host.contains('.') {
        return true;
    }
    let ip = strip_port(host)
        .parse::<std::net::Ipv4Addr>()
        .ok()
        .map(u32::from);
    for value in exceptions.iter().filter(|v| !v.is_empty()) {
        // CPython re.match accepts a numeric prefix before arbitrary suffixes.
        let bytes = value.as_bytes();
        let mut end = 0;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1
        }
        if end != 0 {
            loop {
                let start = end;
                if bytes.get(end) == Some(&b'.')
                    && bytes.get(end + 1).is_some_and(u8::is_ascii_digit)
                {
                    end += 1;
                    while end < bytes.len() && bytes[end].is_ascii_digit() {
                        end += 1
                    }
                }
                if end == start {
                    break;
                }
            }
        }
        if end != 0
            && let Some(ip) = ip
        {
            let parts = value[..end].split('.').collect::<Vec<_>>();
            let mut numbers = [0u64; 4];
            let mut valid = true;
            for (i, p) in parts.iter().take(4).enumerate() {
                // Components are nonnegative. Any component aboveu32 makes
                // Python's arbitrary-int base exceed every IPv4 prefix for
                // every mask0..32. Reject before shifting; retain overlapping
                // components above255 rather than imposing IPv4 octet syntax.
                if let Ok(n) = p.parse::<u32>() {
                    numbers[i] = u64::from(n)
                } else {
                    valid = false
                }
            }
            let base = (numbers[0] << 24) | (numbers[1] << 16) | (numbers[2] << 8) | numbers[3];
            let mut mask = parts.len().saturating_mul(8);
            if bytes.get(end) == Some(&b'/') && bytes.get(end + 1).is_some_and(u8::is_ascii_digit) {
                let start = end + 1;
                end = start;
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1
                }
                mask = value[start..end].parse().unwrap_or(usize::MAX);
            }
            if valid
                && base <= u64::from(u32::MAX)
                && mask <= 32
                && (u64::from(ip) >> (32 - mask)) == (base >> (32 - mask))
            {
                return true;
            }
        } else if glob(host, value, false) {
            return true;
        }
    }
    false
}
#[cfg(target_os = "macos")]
fn system() -> Result<Settings, Failure> {
    use core_foundation::{
        base::CFType, dictionary::CFDictionary, number::CFNumber, string::CFString,
    };
    use system_configuration::dynamic_store::SCDynamicStoreBuilder;
    fn value(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<CFType> {
        dict.find(CFString::new(key)).map(|v| (*v).clone())
    }
    fn string(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<String> {
        value(dict, key)?
            .downcast::<CFString>()
            .map(|v| v.to_string())
    }
    fn number(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<i64> {
        value(dict, key)?.downcast::<CFNumber>()?.to_i64()
    }
    let store = SCDynamicStoreBuilder::new("ssx-cli-source-proxy")
        .build()
        .ok_or(Failure::Transport)?;
    let Some(dict) = store.get_proxies() else {
        return Ok(Settings::default());
    };
    let mut proxies = BTreeMap::new();
    for (scheme, prefix) in [("http", "HTTP"), ("https", "HTTPS")] {
        if number(&dict, &format!("{prefix}Enable")).is_some_and(|n| n != 0)
            && let Some(host) = string(&dict, &format!("{prefix}Proxy"))
        {
            let port = number(&dict, &format!("{prefix}Port"))
                .filter(|n| *n != 0)
                .map_or(String::new(), |n| format!(":{n}"));
            proxies.insert(scheme.into(), format!("http://{host}{port}"));
        }
    }
    let simple = number(&dict, "ExcludeSimpleHostnames").is_some_and(|n| n != 0);
    let exceptions = value(&dict, "ExceptionsList")
        .map(|value| mac_exception_strings(&value))
        .transpose()?
        .unwrap_or_default();
    Ok(Settings {
        proxies,
        bypass: Bypass::Mac { simple, exceptions },
    })
}
// The vetted CF API intentionally does not expose a safe typed downcast to
// CFArray<CFType>. Serialize the OS-owned property list through its safe API,
// then project strings with the existing native XML parser. No raw element
// pointer casts, external DTD fetch, custom XML parser or lossy conversion.
#[cfg(target_os = "macos")]
fn mac_exception_strings(value: &core_foundation::base::CFType) -> Result<Vec<String>, Failure> {
    use core_foundation::{
        array::CFArray,
        base::TCFType,
        propertylist::{create_data, kCFPropertyListXMLFormat_v1_0},
    };
    use quick_xml::{Reader, events::Event, name::QName};
    let array = value.downcast::<CFArray>().ok_or(Failure::Transport)?;
    let data = create_data(array.as_CFTypeRef(), kCFPropertyListXMLFormat_v1_0)
        .map_err(|_| Failure::Transport)?;
    let mut reader = Reader::from_reader(data.bytes());
    let mut out = Vec::new();
    let (mut plist, mut array, mut complete) = (false, false, false);
    loop {
        match reader.read_event().map_err(|_| Failure::Transport)? {
            Event::Start(element)
                if element.name().as_ref() == "plist" && !plist && !array && !complete =>
            {
                plist = true
            }
            Event::Start(element)
                if element.name().as_ref() == "array" && plist && !array && !complete =>
            {
                array = true
            }
            Event::Start(element)
                if element.name().as_ref() == "string" && plist && array && !complete =>
            {
                let text = reader
                    .read_text(QName("string"))
                    .map_err(|_| Failure::Transport)?;
                out.push(
                    quick_xml::escape::unescape(text.as_ref())
                        .map_err(|_| Failure::Transport)?
                        .into_owned(),
                );
            }
            Event::Empty(element)
                if element.name().as_ref() == "string" && plist && array && !complete =>
            {
                out.push(String::new())
            }
            Event::Empty(element)
                if element.name().as_ref() == "array" && plist && !array && !complete =>
            {
                complete = true
            }
            Event::End(element)
                if element.name().as_ref() == "array" && plist && array && !complete =>
            {
                array = false;
                complete = true;
            }
            Event::End(element)
                if element.name().as_ref() == "plist" && plist && !array && complete =>
            {
                plist = false
            }
            Event::Text(text) if text.chars().all(|c| matches!(c, ' ' | '\r' | '\n' | '\t')) => {}
            Event::Decl(_) | Event::DocType(_) if !plist && !array && !complete => {}
            Event::Eof if !plist && !array && complete => return Ok(out),
            // Only a root plist/array and its direct strings are admitted. A nested
            // dictionary/array, number/bool or other malformed setting fails closed;
            // descendants never become bypass exceptions.
            _ => return Err(Failure::Transport),
        }
    }
}

#[cfg(windows)]
fn system() -> Result<Settings, Failure> {
    let Ok(key) = windows_registry::CURRENT_USER
        .open(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings")
    else {
        return Ok(Settings::default());
    };
    if !key.get_u32("ProxyEnable").is_ok_and(|n| n != 0) {
        return Ok(Settings::default());
    }
    let Ok(server) = key.get_string("ProxyServer") else {
        return Ok(Settings::default());
    };
    let proxies = windows_servers(&server);
    let overrides = key.get_string("ProxyOverride").unwrap_or_default();
    Ok(Settings {
        proxies,
        bypass: Bypass::Windows(overrides),
    })
}
#[cfg(any(windows, test))]
fn windows_servers(server: &str) -> BTreeMap<String, String> {
    let server = if !server.contains(['=', ';']) {
        format!("http={server};https={server};ftp={server}")
    } else {
        server.into()
    };
    let mut proxies = BTreeMap::new();
    for item in server.split(';') {
        let Some((protocol, address)) = item.split_once('=') else {
            break;
        };
        let address = if address
            .split_once("://")
            .is_some_and(|(s, _)| !s.is_empty() && !s.contains(['/', ':']))
        {
            address.into()
        } else if matches!(protocol, "http" | "https" | "ftp") {
            format!("http://{address}")
        } else if protocol == "socks" {
            format!("socks://{address}")
        } else {
            address.into()
        };
        proxies.insert(protocol.into(), address);
    }
    if let Some(socks) = proxies.get("socks").filter(|s| !s.is_empty()).cloned() {
        let address = socks
            .strip_prefix("socks://")
            .map_or(socks.clone(), |s| format!("socks4://{s}"));
        for protocol in ["http", "https"] {
            if proxies.get(protocol).is_none_or(String::is_empty) {
                proxies.insert(protocol.into(), address.clone());
            }
        }
    }
    proxies
}
#[cfg(not(any(windows, target_os = "macos")))]
fn system() -> Result<Settings, Failure> {
    Ok(Settings::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_os_bypass_and_glob_controls() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/rust-rewrite/t08a/proxy-policy-source-controls.json"
        ))
        .unwrap();
        assert_eq!(source["mac"].as_array().unwrap().len(), 9);
        assert_eq!(source["windows"].as_array().unwrap().len(), 5);
        assert_eq!(source["glob"].as_array().unwrap().len(), 10);
        for c in source["mac"].as_array().unwrap() {
            let exceptions = c["exceptions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            assert_eq!(
                mac_bypass(
                    c["host"].as_str().unwrap(),
                    c["simple"].as_bool().unwrap(),
                    &exceptions
                ),
                c["expected"].as_bool().unwrap(),
                "{c}"
            );
        }
        for c in source["windows"].as_array().unwrap() {
            assert_eq!(
                windows_bypass(c["host"].as_str().unwrap(), c["override"].as_str().unwrap()),
                c["expected"].as_bool().unwrap(),
                "{c}"
            );
        }
        for c in source["glob"].as_array().unwrap() {
            assert_eq!(
                glob(
                    c["host"].as_str().unwrap(),
                    c["pattern"].as_str().unwrap(),
                    false
                ),
                c["expected"].as_bool().unwrap(),
                "{c}"
            );
        }
        for bypass in [
            Bypass::Mac {
                simple: true,
                exceptions: vec![],
            },
            Bypass::Windows("<local>".into()),
        ] {
            let p = Policy(Settings {
                proxies: BTreeMap::from([("http".into(), "http://proxy.invalid".into())]),
                bypass,
            });
            assert_eq!(p.route("http", "printer").unwrap(), None);
        }
    }
    #[test]
    fn environment_precedence_and_registry_shapes_preserve_routes() {
        let vars = |v: Vec<(&str, &str)>| {
            v.into_iter()
                .map(|(k, v)| (OsString::from(k), OsString::from(v)))
                .collect()
        };
        let (p, present) = environment_values(
            vars(vec![("HTTP_PROXY", "upper"), ("HtTp_proxy", "mixed")]),
            true,
        )
        .unwrap();
        assert!(present);
        assert_eq!(p["http"], "mixed");
        assert!(
            !environment_values(
                vars(vec![("HTTP_PROXY", "upper"), ("http_proxy", "")]),
                false
            )
            .unwrap()
            .1
        );
        let (p, present) =
            environment_values(vars(vec![("all_proxy", "source-disables-os")]), false).unwrap();
        assert!(present && p.is_empty());
        assert_eq!(windows_servers("proxy:8080")["https"], "http://proxy:8080");
        assert_eq!(
            windows_servers("socks=proxy:1080")["http"],
            "socks4://proxy:1080"
        );
        assert_eq!(
            windows_servers("http=one:80;https=two:81")["https"],
            "http://two:81"
        );
        assert!(environment_bypass("host:00080", "host:00080"));
        assert!(!environment_bypass("host:00080", "host:80"));
        assert!(!mac_bypass("0.0.0.0", false, &["1099511627776/0".into()]));
        assert!(mac_bypass("1.0.0.0", false, &["0.256/32".into()]));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn native_cf_proxy_exception_strings_roundtrip_without_raw_pointer_casts() {
        use core_foundation::{array::CFArray, base::TCFType, string::CFString};
        let expected = [
            "*.local",
            "169.254/16",
            "A&B<angle>\"quoted\"",
            "snowman☃",
            "line\r\nending",
            "",
        ];
        let strings = expected
            .iter()
            .map(|s| CFString::new(s))
            .collect::<Vec<_>>();
        let array = CFArray::from_CFTypes(&strings).as_CFType();
        assert_eq!(mac_exception_strings(&array).unwrap(), expected);
        assert!(mac_exception_strings(&CFString::new("wrong shape").as_CFType()).is_err());
        let nested =
            CFArray::from_CFTypes(&[CFString::new("*.must-not-become-an-exception")]).as_CFType();
        let nested = CFArray::from_CFTypes(&[nested]).as_CFType();
        assert!(mac_exception_strings(&nested).is_err());
        let number = core_foundation::number::CFNumber::from(1i32).as_CFType();
        let mixed =
            CFArray::from_CFTypes(&[CFString::new("valid.local").as_CFType(), number]).as_CFType();
        assert!(mac_exception_strings(&mixed).is_err());
    }
}
