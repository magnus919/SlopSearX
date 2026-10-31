//! Literal port of the pinned CookieJar header split stages; no URL projection.
use super::{
    source_cookie_date::{http2time_with_year, SourceCookieDateError},
    source_cookies::python_whitespace,
};
use crate::header::HeaderMap;
#[derive(Clone)]
pub(super) enum Attribute {
    Text(Option<String>),
    Date(Option<i64>),
}
pub(super) type Pairs = Vec<(String, Attribute)>;
/// HTTPX selects one encoding from ALL raw header values, then multi_items()
/// preserves distinct Set-Cookie values. Do not join cookies with comma.
pub(super) fn values(headers: &HeaderMap, name: http::header::HeaderName) -> Vec<String> {
    let utf8 = headers
        .values()
        .all(|v| std::str::from_utf8(v.as_bytes()).is_ok());
    headers
        .get_all(name)
        .iter()
        .map(|v| {
            if utf8 {
                std::str::from_utf8(v.as_bytes())
                    .expect("validated immutable header values")
                    .to_owned()
            } else {
                v.as_bytes().iter().copied().map(char::from).collect()
            }
        })
        .collect()
}
fn trim(text: &str) -> &str {
    text.trim_matches(python_whitespace)
}
fn strip_quotes(text: &str) -> &str {
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        &text[1..text.len() - 1]
    } else {
        text
    }
}
pub(super) fn netscape(
    headers: &[String],
    mut year: impl FnMut() -> Result<i32, SourceCookieDateError>,
) -> Result<Vec<Pairs>, SourceCookieDateError> {
    let mut result = Vec::new();
    for text in headers {
        let mut pairs = Vec::new();
        let mut version_set = false;
        for (index, param) in text.split(';').enumerate() {
            let param = trim(param);
            let (name, value) = param
                .split_once('=')
                .map_or((param, None), |(k, v)| (k, Some(trim(v))));
            let name = trim(name);
            if name.is_empty() {
                if index == 0 {
                    break;
                } else {
                    continue;
                }
            }
            let mut name = name.to_owned();
            let mut attr = Attribute::Text(value.map(str::to_owned));
            if index != 0 {
                let lower = super::source_lower(&name);
                if [
                    "expires", "domain", "path", "secure", "version", "port", "max-age",
                ]
                .contains(&lower.as_str())
                {
                    name = lower;
                }
                if name == "version" {
                    version_set = true;
                    attr = Attribute::Text(value.map(|v| strip_quotes(v).to_owned()));
                } else if name == "expires" {
                    attr = Attribute::Date(match value {
                        Some(v) => http2time_with_year(strip_quotes(v), &mut year)?,
                        None => None,
                    });
                }
            }
            pairs.push((name, attr));
        }
        if !pairs.is_empty() {
            if !version_set {
                pairs.push(("version".to_owned(), Attribute::Text(Some("0".to_owned()))));
            }
            result.push(pairs);
        }
    }
    Ok(result)
}
/// split_header_words: Unicode16 whitespace, quoted-pair LF exclusion,
/// comma groups and source fallback to an unquoted value when quotes fail.
pub(super) fn rfc2965(headers: &[String]) -> Vec<Pairs> {
    let mut result = Vec::new();
    for input in headers {
        let mut text = input.as_str();
        let mut pairs = Vec::new();
        while !text.is_empty() {
            let start = text.trim_start_matches(python_whitespace);
            let name_end = start
                .char_indices()
                .find(|(_, c)| *c == '=' || *c == ';' || *c == ',' || python_whitespace(*c))
                .map_or(start.len(), |(i, _)| i);
            if name_end != 0 {
                let name = start[..name_end].to_owned();
                text = &start[name_end..];
                let value_start = text.trim_start_matches(python_whitespace);
                let value = if let Some(rest) = value_start.strip_prefix('=') {
                    let rest = rest.trim_start_matches(python_whitespace);
                    if let Some((value, remaining)) = quoted(rest) {
                        text = remaining;
                        Some(value)
                    } else {
                        let end = rest
                            .char_indices()
                            .find(|(_, c)| *c == ';' || *c == ',' || python_whitespace(*c))
                            .map_or(rest.len(), |(i, _)| i);
                        text = &rest[end..];
                        Some(rest[..end].trim_end_matches(python_whitespace).to_owned())
                    }
                } else {
                    None
                };
                pairs.push((name, Attribute::Text(value)));
            } else if let Some(rest) = start.strip_prefix(',') {
                text = rest;
                if !pairs.is_empty() {
                    result.push(std::mem::take(&mut pairs));
                }
            } else {
                let rest =
                    text.trim_start_matches(|c| c == '=' || c == ';' || python_whitespace(c));
                debug_assert!(
                    rest.len() < text.len(),
                    "source header tokenizer must make progress"
                );
                text = rest;
            }
        }
        if !pairs.is_empty() {
            result.push(pairs);
        }
    }
    result
}
fn quoted(text: &str) -> Option<(String, &str)> {
    let text = text.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = text.char_indices();
    while let Some((index, c)) = chars.next() {
        if c == '"' {
            return Some((out, &text[index + 1..]));
        }
        if c == '\\' {
            let (_, next) = chars.next()?;
            if next == '\n' {
                return None;
            }
            out.push(next);
        } else {
            out.push(c);
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    fn plain(pairs: &Pairs) -> Vec<(&str, Option<&str>)> {
        pairs
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str(),
                    match v {
                        Attribute::Text(v) => v.as_deref(),
                        Attribute::Date(_) => None,
                    },
                )
            })
            .collect()
    }
    #[test]
    fn source_cookie_header_split_preserves_ns_comma_and_rfc_quoted_pairs() {
        let ns = netscape(&["a=x,y=z; Path=\"/x\"; Version=\"1\"".into()], || Ok(2026)).unwrap();
        assert_eq!(
            plain(&ns[0]),
            vec![
                ("a", Some("x,y=z")),
                ("path", Some("\"/x\"")),
                ("version", Some("1"))
            ]
        );
        let rfc = rfc2965(&["foo=\"bar\"; port=\"80,81\"; discard, bar=baz".into()]);
        assert_eq!(
            plain(&rfc[0]),
            vec![
                ("foo", Some("bar")),
                ("port", Some("80,81")),
                ("discard", None)
            ]
        );
        assert_eq!(plain(&rfc[1]), vec![("bar", Some("baz"))]);
    }
    #[test]
    fn source_cookie_date_failure_aborts_ns_batch_before_cookie_construction() {
        assert!(netscape(
            &[
                "a=1".into(),
                "b=2; Expires=Mon, 01 Jxx 2024 00:00:00 GMT".into()
            ],
            || Ok(2026)
        )
        .is_err());
        let invalid = netscape(&["a=1; Expires=not a date".into()], || {
            Err(SourceCookieDateError)
        })
        .unwrap();
        assert!(matches!(invalid[0][1].1, Attribute::Date(None)));
    }
}
