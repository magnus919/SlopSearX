//! Source IOC grammar: Python Unicode16 word/digit boundaries, no IP validation.
use super::threat_intel::{self as t, Result};
use std::{sync::OnceLock, time::Instant};
async fn work(d: Option<Instant>) -> Result<()> {
    t::checkpoint(d).await?;
    #[cfg(test)]
    t::query_step();
    Ok(())
}
fn word(ch: char) -> bool {
    static RANGES: OnceLock<Vec<(u32, u32)>> = OnceLock::new();
    let ranges = RANGES.get_or_init(|| {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a11/word-ranges-v2.json"
        ))
        .expect("frozen Unicode16 word ranges");
        value["ranges"]
            .as_array()
            .expect("ranges")
            .iter()
            .map(|v| {
                (
                    v[0].as_u64().expect("start") as u32,
                    v[1].as_u64().expect("end") as u32,
                )
            })
            .collect()
    });
    let point = ch as u32;
    let at = ranges.partition_point(|(start, _)| *start <= point);
    at > 0 && point <= ranges[at - 1].1
}
fn ip_at(text: &str, at: usize) -> Option<&str> {
    let mut chars = text[at..].char_indices().peekable();
    let mut end = at;
    for group in 0..4 {
        let mut n = 0;
        while n < 3
            && chars
                .peek()
                .is_some_and(|(_, c)| ssx_json::decimal_digit(*c))
        {
            let (i, c) = chars.next()?;
            end = at + i + c.len_utf8();
            n += 1;
        }
        if n == 0 {
            return None;
        }
        if group < 3 {
            let (_, c) = chars.next()?;
            if c != '.' {
                return None;
            }
        }
    }
    if chars.peek().is_some_and(|(_, c)| word(*c)) {
        None
    } else {
        Some(&text[at..end])
    }
}
pub(super) async fn ip(text: &str, prefix: bool, d: Option<Instant>) -> Result<Option<&str>> {
    let mut previous = None;
    for (n, (at, ch)) in text.char_indices().enumerate() {
        if n % 128 == 0 {
            work(d).await?
        }
        if ssx_json::decimal_digit(ch)
            && previous.is_none_or(|c| !word(c))
            && let Some(found) = ip_at(text, at)
        {
            return Ok(Some(found));
        }
        if prefix {
            break;
        }
        previous = Some(ch);
    }
    Ok(None)
}
pub(super) async fn hash(text: &str, d: Option<Instant>) -> Result<Option<&str>> {
    let mut previous = None;
    for (n, (at, ch)) in text.char_indices().enumerate() {
        if n % 128 == 0 {
            work(d).await?
        }
        if ch.is_ascii_hexdigit() && previous.is_none_or(|c| !word(c)) {
            for width in [32, 40, 64] {
                if let Some(raw) = text.get(at..at.saturating_add(width))
                    && raw.bytes().all(|b| b.is_ascii_hexdigit())
                    && text[at + width..].chars().next().is_none_or(|c| !word(c))
                {
                    return Ok(Some(raw));
                }
            }
        }
        previous = Some(ch);
    }
    Ok(None)
}
pub(super) async fn cve(text: &str, d: Option<Instant>) -> Result<Option<&str>> {
    for (n, (at, _)) in text.char_indices().enumerate() {
        if n % 128 == 0 {
            work(d).await?
        }
        if !text
            .get(at..at + 4)
            .is_some_and(|s| s.eq_ignore_ascii_case("cve-"))
        {
            continue;
        }
        let mut chars = text[at + 4..].char_indices();
        let mut valid = true;
        for _ in 0..4 {
            if chars
                .next()
                .is_none_or(|(_, c)| !ssx_json::decimal_digit(c))
            {
                valid = false;
                break;
            }
        }
        if !valid || chars.next().is_none_or(|(_, c)| c != '-') {
            continue;
        }
        let mut count = 0;
        let mut end = at + 4;
        for (i, ch) in chars {
            if !ssx_json::decimal_digit(ch) {
                break;
            }
            if count % 128 == 0 {
                work(d).await?
            }
            count += 1;
            end = at + 4 + i + ch.len_utf8();
        }
        if count >= 4 {
            return Ok(Some(&text[at..end]));
        }
    }
    Ok(None)
}
pub(super) async fn url(text: &str, d: Option<Instant>) -> Result<Option<&str>> {
    for (n, (at, _)) in text.char_indices().enumerate() {
        if n % 128 == 0 {
            work(d).await?
        }
        if !text
            .get(at..at + 4)
            .is_some_and(|s| s.eq_ignore_ascii_case("http"))
        {
            continue;
        }
        // Python re.IGNORECASE's optional ASCII s also matches long-s U+017F.
        // Preserve the original matched bytes; only recognition folds the case.
        let suffix = &text[at + 4..];
        let secure = suffix
            .chars()
            .next()
            .filter(|ch| matches!(ch, 's' | 'S' | '\u{017f}'))
            .map_or(0, char::len_utf8);
        let width = 4 + secure + 3;
        if text.get(at + 4 + secure..at + width) != Some("://") {
            continue;
        }
        let mut end = at + width;
        for (n, (i, ch)) in text[at + width..].char_indices().enumerate() {
            if ssx_json::python_whitespace(ch) {
                break;
            }
            if n % 1024 == 0 {
                work(d).await?
            }
            end = at + width + i + ch.len_utf8();
        }
        if end > at + width {
            return Ok(Some(&text[at..end]));
        }
    }
    Ok(None)
}
pub(super) fn md5(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|b| b.is_ascii_hexdigit())
}
pub(super) async fn hostname(text: &str, d: Option<Instant>) -> Result<bool> {
    let mut count = 0;
    let mut last = 0u8;
    for (n, b) in text.bytes().enumerate() {
        if n % 1024 == 0 {
            t::checkpoint(d).await?;
        }
        if b == b'.' {
            if count == 0 || !last.is_ascii_alphanumeric() {
                return Ok(false);
            }
            count = 0;
        } else {
            if count >= 63
                || !(b.is_ascii_alphanumeric() || b == b'-')
                || (count == 0 && !b.is_ascii_alphanumeric())
            {
                return Ok(false);
            }
            count += 1;
            last = b;
        }
    }
    Ok(count > 0 && last.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn native_unicode16_word_table_and_all_ioc_grammars_keep_source_boundaries() {
        let ranges: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a11/word-ranges-v2.json"
        ))
        .unwrap();
        let ranges = ranges["ranges"].as_array().unwrap();
        let mut at = 0;
        let mut members = 0;
        for point in 0..=0x10ffffu32 {
            if point % 1024 == 0 {
                tokio::task::yield_now().await;
            }
            while at < ranges.len() && point > ranges[at][1].as_u64().unwrap() as u32 {
                at += 1;
            }
            let expected = at < ranges.len() && point >= ranges[at][0].as_u64().unwrap() as u32;
            if expected {
                members += 1;
            }
            if let Some(ch) = char::from_u32(point) {
                assert_eq!(word(ch), expected, "Unicode16 U+{point:04X}");
            }
        }
        assert_eq!(members, 142940);
        println!("A11_IOC_COMPLETE unicode16-all-scalars");
        for (query, expected) in [
            ("🙂８.８.８.８", Some("８.８.８.８")),
            ("é8.8.8.8", None),
            ("_8.8.8.8", None),
            ("8.8.8.8x", None),
            ("x 999.999.999.999 y", Some("999.999.999.999")),
        ] {
            assert_eq!(ip(query, false, None).await.unwrap(), expected);
        }
        assert_eq!(ip(" 8.8.8.8", true, None).await.unwrap(), None);
        assert_eq!(
            ip("8.8.8.8 text", true, None).await.unwrap(),
            Some("8.8.8.8")
        );
        for n in [32, 40, 64] {
            let text = "a".repeat(n);
            assert_eq!(hash(&text, None).await.unwrap(), Some(text.as_str()));
            assert!(hash(&format!("é{text}"), None).await.unwrap().is_none());
        }
        assert_eq!(
            cve("prefixcve-２０２４-１２３４５suffix", None)
                .await
                .unwrap(),
            Some("cve-２０２４-１２３４５")
        );
        assert!(!hostname("é.example", None).await.unwrap());
        assert!(hostname("EXAMPLE.org", None).await.unwrap());
        assert!(!hostname(&"a".repeat(64), None).await.unwrap());
        assert!(md5(&"A".repeat(32)));
        assert!(!md5(&"Ａ".repeat(32)));
        println!("A11_IOC_COMPLETE ip-hash-cve-hostname-source-grammar");
        for q in [
            "httpſ://evil.example.org/a",
            "HTTPſ://evil.example.org/a?q=1&b=2#frag",
            "https://evil.example.org/a",
        ] {
            assert_eq!(url(q, None).await.unwrap(), Some(q));
        }
        assert_eq!(
            url("prefix httpſ://evil.example/a\u{001c}ignored", None)
                .await
                .unwrap(),
            Some("httpſ://evil.example/a")
        );
        assert!(url("httſ://evil.example/a", None).await.unwrap().is_none());
        println!("A11_IOC_COMPLETE source-ignorecase-long-s-url");
    }
}
