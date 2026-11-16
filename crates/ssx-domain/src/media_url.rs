//! Shared source-compatible media URL algorithm with private storage policies.
//! Bounded owners retain the caller's charge through use; no independent quota.
use crate::FiniteF64;
use ssx_owned_budget::{Budget, OwnedString, OwnedVec};
use std::convert::Infallible;
#[path = "media_nfkc.rs"]
mod nfkc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaBudgetError {
    Resource,
}
impl std::fmt::Display for MediaBudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("media sanitizer resource budget exceeded")
    }
}
impl std::error::Error for MediaBudgetError {}
/// Affine media URL ownership; backing remains charged until this value drops.
pub struct BudgetMediaInfo {
    pub media_type: OwnedString,
    pub url: Option<OwnedString>,
    pub thumbnail: Option<OwnedString>,
    pub source: Option<OwnedString>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration: Option<FiniteF64>,
}
trait Policy {
    type Text: AsRef<str>;
    type Error;
    type List<T>: AsRef<[T]> + AsMut<[T]>;
    fn list<T>(&self) -> Result<Self::List<T>, Self::Error>;
    fn push_list<T>(&self, values: &mut Self::List<T>, value: T) -> Result<(), Self::Error>;
    fn allocate(&self, capacity: usize) -> Result<Self::Text, Self::Error>;
    fn push(&self, out: &mut Self::Text, text: &str) -> Result<(), Self::Error>;
    fn overflow(&self) -> Self::Error;
    fn copy(&self, text: &str) -> Result<Self::Text, Self::Error> {
        let mut out = self.allocate(text.len())?;
        self.push(&mut out, text)?;
        Ok(out)
    }
}
struct Plain;
impl Policy for Plain {
    type Text = String;
    type Error = Infallible;
    type List<T> = Vec<T>;
    fn list<T>(&self) -> Result<Vec<T>, Infallible> {
        Ok(Vec::new())
    }
    fn push_list<T>(&self, values: &mut Vec<T>, value: T) -> Result<(), Infallible> {
        values.push(value);
        Ok(())
    }
    fn allocate(&self, capacity: usize) -> Result<String, Infallible> {
        Ok(String::with_capacity(capacity))
    }
    fn push(&self, out: &mut String, text: &str) -> Result<(), Infallible> {
        out.push_str(text);
        Ok(())
    }
    fn overflow(&self) -> Infallible {
        panic!("legacy media URL exceeds address space")
    }
}
struct ChargedList<T>(OwnedVec<T>);
impl<T> AsRef<[T]> for ChargedList<T> {
    fn as_ref(&self) -> &[T] {
        self.0.as_slice()
    }
}
impl<T> AsMut<[T]> for ChargedList<T> {
    fn as_mut(&mut self) -> &mut [T] {
        self.0.as_mut_slice()
    }
}
struct Charged<'a>(&'a Budget);
impl Policy for Charged<'_> {
    type Text = OwnedString;
    type Error = MediaBudgetError;
    type List<T> = ChargedList<T>;
    fn list<T>(&self) -> Result<ChargedList<T>, MediaBudgetError> {
        OwnedVec::new(self.0)
            .map(ChargedList)
            .map_err(|_| MediaBudgetError::Resource)
    }
    fn push_list<T>(&self, values: &mut ChargedList<T>, value: T) -> Result<(), MediaBudgetError> {
        values
            .0
            .try_push(value)
            .map_err(|_| MediaBudgetError::Resource)
    }
    fn allocate(&self, capacity: usize) -> Result<OwnedString, MediaBudgetError> {
        let mut out = OwnedString::new(self.0).map_err(|_| MediaBudgetError::Resource)?;
        out.try_reserve_exact(capacity)
            .map_err(|_| MediaBudgetError::Resource)?;
        Ok(out)
    }
    fn push(&self, out: &mut OwnedString, text: &str) -> Result<(), MediaBudgetError> {
        out.try_push_str(text)
            .map_err(|_| MediaBudgetError::Resource)
    }
    fn overflow(&self) -> MediaBudgetError {
        MediaBudgetError::Resource
    }
}
pub(super) fn sanitize_legacy(original: &str) -> String {
    match sanitize(&Plain, original) {
        Ok(value) => value,
        Err(never) => match never {},
    }
}
/// Same source sanitizer as build_media, with caller-owned allocation budget.
#[allow(clippy::too_many_arguments)]
pub fn build_media_bounded(
    budget: &Budget,
    media_type: &str,
    url: Option<&str>,
    thumbnail: Option<&str>,
    source: Option<&str>,
    width: Option<i64>,
    height: Option<i64>,
    duration: Option<FiniteF64>,
) -> Result<Option<BudgetMediaInfo>, MediaBudgetError> {
    if !matches!(media_type, "image" | "video") {
        return Ok(None);
    }
    let policy = Charged(budget);
    let field = |value: Option<&str>| {
        value
            .filter(|s| !s.is_empty())
            .map(|s| sanitize(&policy, s))
            .transpose()
    };
    Ok(Some(BudgetMediaInfo {
        media_type: policy.copy(media_type)?,
        url: field(url)?,
        thumbnail: field(thumbnail)?,
        source: field(source)?,
        width,
        height,
        duration,
    }))
}
struct Parts<'a> {
    scheme: Option<&'a str>,
    netloc: Option<&'a str>,
    path: &'a str,
    params: Option<&'a str>,
    query: Option<&'a str>,
    fragment: Option<&'a str>,
}
fn contains_ascii(items: &[&str], value: &str) -> bool {
    items.iter().any(|item| item.eq_ignore_ascii_case(value))
}
fn split(value: &str) -> Option<Parts<'_>> {
    let (scheme, mut remainder) = if let Some(colon) = value.find(':').filter(|i| *i > 0) {
        let candidate = &value[..colon];
        if candidate.as_bytes()[0].is_ascii_alphabetic()
            && candidate
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
        {
            (Some(candidate), &value[colon + 1..])
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
        && !valid_netloc(netloc)
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
    let params = if contains_ascii(USES_PARAMS, scheme.unwrap_or("")) {
        let last_segment = remainder.rfind('/').unwrap_or(0);
        if let Some(index) = remainder[last_segment..]
            .find(';')
            .map(|i| last_segment + i)
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
    Some(Parts {
        scheme,
        netloc,
        path: remainder,
        params,
        query,
        fragment,
    })
}
pub(super) fn valid_netloc(netloc: &str) -> bool {
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
                || !version.bytes().all(|b| b.is_ascii_hexdigit())
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
    !netloc
        .chars()
        .any(|ch| nfkc::DELIMITERS.binary_search(&(ch as u32)).is_ok())
}
fn pairs(query: &str) -> impl Iterator<Item = (&str, &str)> + Clone {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| pair.split_once('=').unwrap_or((pair, "")))
}
fn bytes(value: &str) -> impl Iterator<Item = u8> + '_ {
    let mut rest = value.as_bytes();
    std::iter::from_fn(move || {
        let (&first, after) = rest.split_first()?;
        rest = after;
        if first == b'%' && rest.len() >= 2 {
            let hex = |c: u8| match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            };
            if let (Some(a), Some(b)) = (hex(rest[0]), hex(rest[1])) {
                rest = &rest[2..];
                return Some(a * 16 + b);
            }
        }
        Some(if first == b'+' { b' ' } else { first })
    })
}
// Fixed-size UTF8 carry preserves from_utf8_lossy's invalid-byte partition.
// An incomplete suffix waits for more bytes; only a final incomplete suffix is
// replaced once. No heap buffer or replacement string is allocated.
fn decode_into<E>(value: &str, mut emit: impl FnMut(&str) -> Result<(), E>) -> Result<(), E> {
    let mut input = bytes(value);
    let mut carry = [0; 4];
    let mut used = 0;
    loop {
        while used < carry.len() {
            let Some(byte) = input.next() else {
                break;
            };
            carry[used] = byte;
            used += 1;
        }
        if used == 0 {
            break;
        }
        match std::str::from_utf8(&carry[..used]) {
            Ok(valid) => {
                emit(valid)?;
                used = 0;
            }
            Err(error) => {
                let prefix = error.valid_up_to();
                // The valid_up_to prefix is guaranteed valid UTF8 by std.
                if prefix != 0 {
                    emit(std::str::from_utf8(&carry[..prefix]).expect("UTF8 validated prefix"))?;
                }
                let consumed = if let Some(invalid) = error.error_len() {
                    emit("�")?;
                    prefix + invalid
                } else if used < carry.len() {
                    emit("�")?;
                    used
                } else {
                    prefix
                };
                carry.copy_within(consumed..used, 0);
                used -= consumed;
            }
        }
    }
    Ok(())
}
fn decoded<P: Policy>(policy: &P, value: &str) -> Result<P::Text, P::Error> {
    let mut count = 0usize;
    decode_into(value, |part| {
        count = count
            .checked_add(part.len())
            .ok_or_else(|| policy.overflow())?;
        Ok(())
    })?;
    let mut out = policy.allocate(count)?;
    decode_into(value, |part| policy.push(&mut out, part))?;
    Ok(out)
}
fn quote<E>(value: &str, mut emit: impl FnMut(&str) -> Result<(), E>) -> Result<(), E> {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"_.-~".contains(&byte) {
            let mut scratch = [0; 4];
            emit(char::from(byte).encode_utf8(&mut scratch))?;
        } else if byte == b' ' {
            emit("+")?;
        } else {
            let encoded = [b'%', HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]];
            emit(std::str::from_utf8(&encoded).expect("ASCII quote bytes"))?;
        }
    }
    Ok(())
}
struct Group<P: Policy> {
    key: P::Text,
    values: P::List<P::Text>,
}
fn parse_query<P: Policy>(policy: &P, query: &str) -> Result<P::List<Group<P>>, P::Error> {
    let mut grouped = policy.list::<Group<P>>()?;
    for (key, value) in pairs(query) {
        let key = decoded(policy, key)?;
        let value = decoded(policy, value)?;
        if ["api_key", "key", "apiKey", "token", "access_token"].contains(&key.as_ref()) {
            continue;
        }
        if let Some(group) = grouped
            .as_mut()
            .iter_mut()
            .find(|group| group.key.as_ref() == key.as_ref())
        {
            policy.push_list(&mut group.values, value)?;
        } else {
            let mut values = policy.list()?;
            policy.push_list(&mut values, value)?;
            policy.push_list(&mut grouped, Group { key, values })?;
        }
    }
    Ok(grouped)
}
fn query_into<P: Policy>(
    groups: &[Group<P>],
    mut emit: impl FnMut(&str) -> Result<(), P::Error>,
) -> Result<(), P::Error> {
    let mut first = true;
    for group in groups {
        for value in group.values.as_ref() {
            if !first {
                emit("&")?;
            }
            first = false;
            quote(group.key.as_ref(), &mut emit)?;
            emit("=")?;
            quote(value.as_ref(), &mut emit)?;
        }
    }
    Ok(())
}
fn reconstruct_into<E>(
    parts: &Parts<'_>,
    query: &str,
    mut emit: impl FnMut(&str) -> Result<(), E>,
) -> Result<(), E> {
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
    let netloc = if parts.netloc.is_some_and(|v| !v.is_empty()) {
        parts.netloc
    } else if parts.scheme.is_some_and(|s| contains_ascii(USES_NETLOC, s))
        && (parts.path.is_empty() || parts.path.starts_with('/'))
    {
        Some("")
    } else {
        None
    };
    if let Some(scheme) = parts.scheme {
        for byte in scheme.bytes() {
            let mut scratch = [0; 4];
            emit(char::from(byte.to_ascii_lowercase()).encode_utf8(&mut scratch))?;
        }
        emit(":")?;
    }
    if let Some(netloc) = netloc {
        emit("//")?;
        emit(netloc)?;
        if (!parts.path.is_empty() || parts.params.is_some_and(|v| !v.is_empty()))
            && !parts.path.starts_with('/')
        {
            emit("/")?;
        }
    } else if parts.path.starts_with("//") {
        emit("//")?;
    }
    emit(parts.path)?;
    if let Some(params) = parts.params.filter(|v| !v.is_empty()) {
        emit(";")?;
        emit(params)?;
    }
    if !query.is_empty() {
        emit("?")?;
        emit(query)?;
    }
    if let Some(fragment) = parts.fragment.filter(|v| !v.is_empty()) {
        emit("#")?;
        emit(fragment)?;
    }
    Ok(())
}
fn sanitize<P: Policy>(policy: &P, original: &str) -> Result<P::Text, P::Error> {
    let trimmed = original.trim_start_matches(|ch: char| (ch as u32) <= 32);
    let cleaned = if trimmed.contains(['\t', '\r', '\n']) {
        let count = trimmed
            .chars()
            .filter(|ch| !matches!(ch, '\t' | '\r' | '\n'))
            .try_fold(0usize, |n, ch| n.checked_add(ch.len_utf8()))
            .ok_or_else(|| policy.overflow())?;
        let mut value = policy.allocate(count)?;
        for ch in trimmed
            .chars()
            .filter(|ch| !matches!(ch, '\t' | '\r' | '\n'))
        {
            let mut scratch = [0; 4];
            policy.push(&mut value, ch.encode_utf8(&mut scratch))?;
        }
        Some(value)
    } else {
        None
    };
    let cleaned = cleaned.as_ref().map_or(trimmed, AsRef::as_ref);
    let Some(parts) = split(cleaned) else {
        return policy.copy(original);
    };
    let Some(query) = parts.query.filter(|q| !q.is_empty()) else {
        return policy.copy(original.trim_end_matches('?'));
    };
    let grouped = parse_query(policy, query)?;
    let mut count = 0usize;
    query_into::<P>(grouped.as_ref(), |part| {
        count = count
            .checked_add(part.len())
            .ok_or_else(|| policy.overflow())?;
        Ok(())
    })?;
    let mut serialized = policy.allocate(count)?;
    query_into::<P>(grouped.as_ref(), |part| policy.push(&mut serialized, part))?;
    drop(grouped);
    let mut count = 0usize;
    reconstruct_into(&parts, serialized.as_ref(), |part| {
        count = count
            .checked_add(part.len())
            .ok_or_else(|| policy.overflow())?;
        Ok(())
    })?;
    let mut out = policy.allocate(count)?;
    reconstruct_into(&parts, serialized.as_ref(), |part| {
        policy.push(&mut out, part)
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    mod previous {
        include!("../tests/fixtures/a05_previous_media_sanitizer.rs");
    }
    #[test]
    fn static_nfkc_subset_matches_entire_accepted_membership_domain() {
        let table: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/unicode/python-text-16.0.0.json"
        ))
        .unwrap();
        let original: Vec<u32> = table["nfkc_url_delimiters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u32)
            .collect();
        assert_eq!(nfkc::DELIMITERS, original);
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/rust-rewrite/a05/media-nfkc-source.json"
        ))
        .unwrap();
        assert_eq!(source["point_count"], 0x110000);
        let encoded = source["membership_bits_hex"].as_str().unwrap();
        let bits: Vec<u8> = encoded
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(bits.len(), 0x110000 / 8);
        use sha2::Digest;
        assert_eq!(
            sha2::Sha256::digest(&bits)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            source["membership_bits_sha256"]
        );
        for point in 0..=0x10ffff {
            assert_eq!(
                nfkc::DELIMITERS.binary_search(&point).is_ok(),
                bits[point as usize / 8] & (1 << (point % 8)) != 0,
                "actual source U+{point:04X}"
            );
            assert_eq!(
                nfkc::DELIMITERS.binary_search(&point).is_ok(),
                original.contains(&point),
                "U+{point:04X}"
            );
        }
    }
    #[test]
    fn fixed_carry_decoder_preserves_invalid_partition_and_final_suffix() {
        let mut values = vec![
            "%F0%9F%92%A9".to_owned(),
            "%F0%9F%92".into(),
            "x%F0%9F%92y".into(),
            "%ED%A0%80".into(),
            "%E0%80%80".into(),
            "%F4%90%80%80".into(),
            "%E2%82".into(),
            "%C2".into(),
            "💩%FFé+%2B".into(),
            "%%%2%00".into(),
        ];
        for first in 0..=255u8 {
            for second in 0..=255u8 {
                values.push(format!("%{first:02X}%{second:02X}"));
            }
        }
        let budget = Budget::new(4096).unwrap();
        for value in values {
            let encoded = format!("x={value}");
            let expected = url::form_urlencoded::parse(encoded.as_bytes())
                .next()
                .unwrap()
                .1
                .into_owned();
            let mut plain = String::new();
            decode_into(&value, |part| {
                plain.push_str(part);
                Ok::<(), Infallible>(())
            })
            .unwrap();
            assert_eq!(plain, expected, "{value}");
            let charged = decoded(&Charged(&budget), &value).unwrap();
            assert_eq!(charged.as_str(), expected, "{value}");
            assert_eq!(budget.snapshot().unwrap().used, charged.charged_bytes());
            drop(charged);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
    #[test]
    fn shared_plain_and_bounded_sanitizer_match_previous_full_algorithm() {
        let roots = [
            "https://x.test/a",
            "HTTP:/foo",
            "http:foo",
            "custom://",
            "A/B:C",
            "http:////x/a;",
            "https://[::1]/a",
            "https://[vF.example]/a",
            "https://[bad]/a",
            "https://x℀y/a",
            "\0 \tHTTPS://x.test/a",
        ];
        let queries = [
            "",
            "?",
            "?a=1&b=2&a=3",
            "?b=1&a=2&b=3&a=4",
            "?token=secret&api_key=x&apiKey=x&access_token=x&key=x&size=small",
            "?Token=keep&%74oken=remove&%6Bey=remove&KEY=keep",
            "?a=%FF%ED%A0%80&b=%F0%9F%92&a=%E0%80%80",
            "?a=hello+world&star=*&tilde=~&a=x%2By",
            "?&&a&=v&&a=2",
            "?token=x",
            "?to\nken=x&x=1",
            "?a=1#fragment?token=keep",
        ];
        let budget = Budget::new(1024 * 1024).unwrap();
        for root in roots {
            for query in queries {
                let input = format!("{root}{query}");
                let expected = previous::sanitize_media_url(&input);
                assert_eq!(sanitize_legacy(&input), expected, "{input:?}");
                let media = build_media_bounded(
                    &budget,
                    "image",
                    Some(&input),
                    Some(&input),
                    Some(&input),
                    Some(0),
                    Some(-1),
                    Some(FiniteF64::new(0.5).unwrap()),
                )
                .unwrap()
                .unwrap();
                assert_eq!(media.media_type.as_str(), "image");
                assert_eq!(media.url.as_ref().unwrap().as_str(), expected, "{input:?}");
                assert_eq!(media.thumbnail.as_ref().unwrap().as_str(), expected);
                assert_eq!(media.source.as_ref().unwrap().as_str(), expected);
                assert_eq!((media.width, media.height), (Some(0), Some(-1)));
                assert_eq!(media.duration.as_ref().unwrap().get(), 0.5);
                drop(media);
                assert_eq!(budget.snapshot().unwrap().used, 0);
            }
        }
        assert!(
            build_media_bounded(&budget, "audio", Some("x"), None, None, None, None, None)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            sanitize_legacy("https://x/a?b=1&a=2&b=3&a=4"),
            "https://x/a?b=1&b=3&a=2&a=4"
        );
    }
    #[test]
    fn sanitizer_quota_sweep_never_turns_refusal_into_source_fallback() {
        for original in [
            "https://x/a?b=1&a=2&b=3&token=remove#frag",
            "https://[bad]/a?token=retained",
        ] {
            let budget = Budget::new(1024 * 1024).unwrap();
            let value = build_media_bounded(
                &budget,
                "image",
                Some(original),
                Some(original),
                Some(original),
                None,
                None,
                None,
            )
            .unwrap()
            .unwrap();
            let expected = sanitize_legacy(original);
            assert_eq!(value.url.as_ref().unwrap().as_str(), expected);
            let peak = budget.snapshot().unwrap().peak;
            drop(value);
            assert_eq!(budget.snapshot().unwrap().used, 0);
            let mut refused = 0;
            let mut succeeded = 0;
            for quota in 1..=peak {
                let budget = Budget::new(quota).unwrap();
                match build_media_bounded(
                    &budget,
                    "image",
                    Some(original),
                    Some(original),
                    Some(original),
                    None,
                    None,
                    None,
                ) {
                    Ok(Some(value)) => {
                        assert_eq!(value.url.as_ref().unwrap().as_str(), expected);
                        drop(value);
                        succeeded += 1;
                    }
                    Err(MediaBudgetError::Resource) => refused += 1,
                    _ => panic!("unexpected media vocabulary result"),
                }
                assert_eq!(budget.snapshot().unwrap().used, 0, "quota {quota}");
            }
            assert!(refused > 0 && succeeded > 0);
        }
    }
    #[test]
    fn nested_distinct_and_duplicate_group_lists_charge_growth_overlap_and_drop() {
        for duplicate in [false, true] {
            let count = if duplicate { 1000 } else { 256 };
            let query = (0..count)
                .map(|n| {
                    if duplicate {
                        format!("same=v{n}")
                    } else {
                        format!("key{n}=v{n}")
                    }
                })
                .collect::<Vec<_>>()
                .join("&");
            let budget = Budget::new(1024 * 1024).unwrap();
            let groups = parse_query(&Charged(&budget), &query).unwrap();
            assert_eq!(groups.as_ref().len(), if duplicate { 1 } else { count });
            let mut retained = groups.0.charged_bytes();
            for group in groups.as_ref() {
                retained += group.key.charged_bytes() + group.values.0.charged_bytes();
                retained += group
                    .values
                    .as_ref()
                    .iter()
                    .map(OwnedString::charged_bytes)
                    .sum::<usize>();
            }
            let snapshot = budget.snapshot().unwrap();
            assert_eq!(snapshot.used, retained);
            assert!(
                snapshot.peak > retained,
                "must reserve old+new backing overlap"
            );
            if duplicate {
                let values = groups.as_ref()[0].values.as_ref();
                assert_eq!(values.len(), 1000);
                assert_eq!(values[0].as_str(), "v0");
                assert_eq!(values[999].as_str(), "v999");
            } else {
                assert_eq!(groups.as_ref()[255].key.as_str(), "key255");
            }
            let peak = snapshot.peak;
            drop(groups);
            assert_eq!(budget.snapshot().unwrap().used, 0);
            // A final-retained-size-only budget must refuse the actual transient
            // growth peak, and release every nested value/list on that failure.
            let limited = Budget::new(retained).unwrap();
            assert!(matches!(
                parse_query(&Charged(&limited), &query),
                Err(MediaBudgetError::Resource)
            ));
            assert_eq!(limited.snapshot().unwrap().used, 0);
            for quota in [1, 8, 32, 128, 1024, retained / 2, peak] {
                let limited = Budget::new(quota).unwrap();
                match parse_query(&Charged(&limited), &query) {
                    Ok(groups) => drop(groups),
                    Err(MediaBudgetError::Resource) => {}
                }
                assert_eq!(
                    limited.snapshot().unwrap().used,
                    0,
                    "duplicate={duplicate} quota={quota}"
                );
                assert!(limited.snapshot().unwrap().peak <= quota);
            }
        }
    }
}
