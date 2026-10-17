//! Source operations for Semantic Scholar; HTTP/factory wiring is separate.
use super::source_json::{Map, Number, Text, Value};
use ssx_domain::{FiniteF64, SearchResult};
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Source(String),
    Output(String),
    Deadline,
    Resource,
}
fn source(message: String) -> ParseError {
    ParseError::Source(message)
}
fn kind(v: &Value) -> &'static str {
    super::broad_type(v)
}
fn truthy(v: &Value) -> bool {
    super::broad_truthy(Some(v))
}
fn get<'a>(v: &'a Value, name: &str) -> Result<Option<&'a Value>, ParseError> {
    v.as_object()
        .map(|map| map.get(name))
        .ok_or_else(|| source(format!("'{}' object has no attribute 'get'", kind(v))))
}
fn field<'a>(m: &'a Map, name: &str, default: &'a Value) -> &'a Value {
    m.get(name).unwrap_or(default)
}
fn keep_first(refusal: &mut Option<String>, error: String) {
    if refusal.is_none() {
        *refusal = Some(error);
    }
}
fn scalar(v: &Value, optional: bool, field: &str, refusal: &mut Option<String>) -> Option<String> {
    match v {
        Value::String(text) => match text.scalar() {
            Some(s) => Some(s),
            None => {
                keep_first(
                    refusal,
                    format!(
                        "Semantic Scholar result {field} must contain only Unicode scalar values"
                    ),
                );
                Some(String::new())
            }
        },
        Value::Null if optional => None,
        _ => {
            keep_first(
                refusal,
                format!(
                    "Semantic Scholar result {field} must be a string{}",
                    if optional { " or null" } else { "" }
                ),
            );
            Some(String::new())
        }
    }
}
// Python float accepts Unicode decimal digits, underscores, leading plus and
// decimal forms outside JSON grammar. Keep that lexical boundary separate from
// the shared exact decimal-to-binary conversion.
async fn string_float(text: &Text, deadline: Instant) -> Result<Option<f64>, ParseError> {
    let Some(raw) = text.as_str() else {
        return Ok(None);
    };
    let mut normalized = String::new();
    normalized
        .try_reserve_exact(raw.len())
        .map_err(|_| ParseError::Resource)?;
    let mut previous_digit = false;
    let mut underscore = false;
    for (index, ch) in raw.chars().enumerate() {
        if index % 1024 == 0 {
            checkpoint(deadline).await?;
        }
        // Reuse the accepted Unicode decimal table without scanning a large
        // string synchronously. Whitespace must survive until outer trimming.
        let mut buf = [0; 4];
        let digit = if ch.is_ascii() {
            None
        } else {
            let one =
                ssx_domain::storage::normalize_python_number_text(ch.encode_utf8(&mut buf)).ok();
            one.and_then(|one| {
                (one.len() == 1 && one.as_bytes()[0].is_ascii_digit()).then(|| one.as_bytes()[0])
            })
        };
        let ch = digit.map_or(ch, char::from);
        if underscore && !ch.is_ascii_digit() {
            return Ok(None);
        }
        if ch == '_' {
            if !previous_digit {
                return Ok(None);
            }
            underscore = true;
            previous_digit = false;
        } else {
            normalized.push(ch);
            underscore = false;
            previous_digit = ch.is_ascii_digit();
        }
    }
    if underscore {
        return Ok(None);
    }
    // The loop has already visited all characters cooperatively; whitespace
    // runs are bounded separately so trim does not become a second long scan.
    let bytes = normalized.as_bytes();
    let mut first = 0;
    let mut last = bytes.len();
    for (index, (at, ch)) in normalized.char_indices().enumerate() {
        if index % 1024 == 0 {
            checkpoint(deadline).await?;
        }
        if !float_space(ch) {
            first = at;
            break;
        }
        first = at + ch.len_utf8();
    }
    for (index, (at, ch)) in normalized.char_indices().rev().enumerate() {
        if index % 1024 == 0 {
            checkpoint(deadline).await?;
        }
        if !float_space(ch) {
            last = at + ch.len_utf8();
            break;
        }
        last = at;
    }
    if first >= last {
        return Ok(None);
    }
    let raw = &normalized[first..last];
    let (negative, unsigned) = if let Some(s) = raw.strip_prefix('-') {
        (true, s)
    } else {
        (false, raw.strip_prefix('+').unwrap_or(raw))
    };
    if unsigned.eq_ignore_ascii_case("nan") {
        return Ok(Some(if negative { -f64::NAN } else { f64::NAN }));
    }
    if unsigned.eq_ignore_ascii_case("inf") || unsigned.eq_ignore_ascii_case("infinity") {
        return Ok(Some(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }));
    }
    let mut decimal = String::new();
    decimal
        .try_reserve_exact(raw.len().checked_add(2).ok_or(ParseError::Resource)?)
        .map_err(|_| ParseError::Resource)?;
    if negative {
        decimal.push('-');
    }
    let mut integer_end = 0;
    let mut nonzero = None;
    for (index, byte) in unsigned.bytes().enumerate() {
        if index % 1024 == 0 {
            checkpoint(deadline).await?;
        }
        if !byte.is_ascii_digit() {
            break;
        }
        if byte != b'0' && nonzero.is_none() {
            nonzero = Some(index);
        }
        integer_end = index + 1;
    }
    if integer_end == 0 {
        if !unsigned.starts_with('.') || !unsigned.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
        {
            return Ok(None);
        }
        decimal.push('0');
    } else if let Some(at) = nonzero {
        for chunk in unsigned.as_bytes()[at..integer_end].chunks(1024) {
            checkpoint(deadline).await?;
            decimal.push_str(std::str::from_utf8(chunk).expect("ASCII digits"));
        }
    } else {
        decimal.push('0');
    }
    let tail = &unsigned[integer_end..];
    let trailing_dot =
        tail.starts_with('.') && (tail.len() == 1 || matches!(tail.as_bytes()[1], b'e' | b'E'));
    let tail = if trailing_dot {
        decimal.push_str(".0");
        &tail[1..]
    } else {
        tail
    };
    for chunk in tail.as_bytes().chunks(1024) {
        checkpoint(deadline).await?;
        // Remaining valid grammar is ASCII. Non-ASCII cannot be split into an
        // invalid UTF8 slice and is rejected as a source float lexical error.
        if !chunk.is_ascii() {
            return Ok(None);
        }
        decimal.push_str(std::str::from_utf8(chunk).expect("ASCII grammar"));
    }
    let mut parser = match ssx_json::FloatNormalizer::new(&decimal) {
        Ok(parser) => parser,
        Err(ssx_json::FloatError::ByteLimit) => return Err(ParseError::Resource),
        Err(_) => return Ok(None),
    };
    loop {
        checkpoint(deadline).await?;
        match parser.advance(1024) {
            Ok(true) => break,
            Ok(false) => {}
            Err(_) => return Ok(None),
        }
    }
    checkpoint(deadline).await?;
    let value = parser.finish().ok();
    checkpoint(deadline).await?;
    Ok(value)
}
fn float_space(ch: char) -> bool {
    if ch.is_ascii() {
        ch.is_ascii_whitespace() || ch == '\u{b}'
    } else {
        ch.is_whitespace()
    }
}
async fn float(v: &Value, deadline: Instant) -> Result<f64, ParseError> {
    match v {
        Value::Bool(v) => Ok(f64::from(u8::from(*v))),
        Value::Number(Number::Float(v)) => Ok(*v),
        Value::Number(number) => number
            .as_f64()
            .ok_or_else(|| source("int too large to convert to float".into())),
        Value::String(text) => string_float(text, deadline).await?.ok_or_else(|| {
            source(format!(
                "could not convert string to float: {}",
                text.repr()
            ))
        }),
        _ => Err(source(format!(
            "float() argument must be a string or a real number, not '{}'",
            kind(v)
        ))),
    }
}

fn whitespace(point: u32) -> bool {
    char::from_u32(point).is_some_and(super::broad_whitespace)
}
fn trim(text: &Text) -> Text {
    let p = text.codepoints();
    let start = p.iter().position(|p| !whitespace(*p)).unwrap_or(p.len());
    let end = p
        .iter()
        .rposition(|p| !whitespace(*p))
        .map_or(start, |p| p + 1);
    Text::from_points(p[start..end].to_vec())
}
fn join(parts: &[Text], separator: &str) -> Text {
    let mut out = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if index != 0 {
            out.extend(separator.chars().map(u32::from));
        }
        out.extend_from_slice(part.codepoints());
    }
    Text::from_points(out)
}
fn sanitize_text(text: &Text) -> Result<String, ParseError> {
    let mut out = String::new();
    out.try_reserve_exact(text.len().checked_mul(4).ok_or(ParseError::Resource)?)
        .map_err(|_| ParseError::Resource)?;
    for point in text.codepoints() {
        out.push(char::from_u32(*point).unwrap_or('\u{fffd}'));
    }
    Ok(out)
}
fn sanitized(
    value: &Value,
    depth: usize,
    refusal: &mut Option<String>,
) -> Result<ssx_json::Value, ParseError> {
    use ssx_json::Value as Out;
    if depth > 100 {
        return Ok(Out::String("<max depth exceeded>".into()));
    }
    Ok(match value {
        Value::String(text) => Out::String(sanitize_text(text)?),
        Value::Array(items) => {
            let mut out = Vec::new();
            out.try_reserve_exact(items.len())
                .map_err(|_| ParseError::Resource)?;
            for item in items {
                out.push(sanitized(item, depth + 1, refusal)?);
            }
            Out::Array(out)
        }
        Value::Object(map) => {
            let mut out = ssx_json::Map::new();
            out.try_reserve_exact(map.len())
                .map_err(|_| ParseError::Resource)?;
            for (key, value) in map.iter() {
                out.insert(sanitize_text(key)?, sanitized(value, depth + 1, refusal)?);
            }
            Out::Object(out)
        }
        _ => match value.project() {
            Ok(out) => out,
            Err(_) => {
                keep_first(
                    refusal,
                    "Semantic Scholar payload must contain only finite JSON numbers".into(),
                );
                Out::Null
            }
        },
    })
}
fn payload(
    fields: &[(&str, &Value)],
    authors: &[&Value],
    refusal: &mut Option<String>,
) -> Result<ssx_json::Value, ParseError> {
    use ssx_json::Value as Out;
    let count = fields.len() + usize::from(!authors.is_empty());
    let mut data = ssx_json::Map::new();
    data.try_reserve_exact(count)
        .map_err(|_| ParseError::Resource)?;
    let mut names = Vec::new();
    names
        .try_reserve_exact(count)
        .map_err(|_| ParseError::Resource)?;
    for (name, value) in fields {
        names.push(Out::String((*name).into()));
        data.insert((*name).into(), sanitized(value, 1, refusal)?);
    }
    if !authors.is_empty() {
        let mut out = Vec::new();
        out.try_reserve_exact(authors.len())
            .map_err(|_| ParseError::Resource)?;
        for value in authors {
            out.push(sanitized(value, 2, refusal)?);
        }
        names.push(Out::String("authors".into()));
        data.insert("authors".into(), Out::Array(out));
    }
    let mut provenance = ssx_json::Map::new();
    provenance
        .try_reserve_exact(4)
        .map_err(|_| ParseError::Resource)?;
    provenance.insert("engine".into(), Out::String("semanticscholar".into()));
    provenance.insert("adapter_fields".into(), Out::Array(names));
    provenance.insert("normalized_fields".into(), Out::Array(vec![]));
    provenance.insert("inferred_fields".into(), Out::Array(vec![]));
    let mut out = ssx_json::Map::new();
    out.try_reserve_exact(5).map_err(|_| ParseError::Resource)?;
    out.insert("domain".into(), Out::String("science".into()));
    out.insert("type".into(), Out::String("publication".into()));
    out.insert(
        "schema_version".into(),
        Out::Number(ssx_json::Number::new("1").unwrap()),
    );
    out.insert("data".into(), Out::Object(data));
    out.insert("provenance".into(), Out::Object(provenance));
    Ok(Out::Object(out))
}
async fn checkpoint(deadline: Instant) -> Result<(), ParseError> {
    tokio::task::yield_now().await;
    if Instant::now() >= deadline {
        Err(ParseError::Deadline)
    } else {
        Ok(())
    }
}
/// Complete source operations run before a pending typed-output refusal wins.
/// No result escapes a later source error. No registry activation occurs here.
pub async fn parse(data: &Value, deadline: Instant) -> Result<Vec<SearchResult>, ParseError> {
    checkpoint(deadline).await?;
    let empty_list = Value::Array(vec![]);
    let empty = Value::String(Text::from(""));
    let zero = Value::integer(0);
    let null = Value::Null;
    let papers = get(data, "data")?.unwrap_or(&empty_list);
    let papers = match papers {
        Value::Array(p) => p.as_slice(),
        Value::String(t) if t.is_empty() => &[],
        Value::Object(o) if o.is_empty() => &[],
        Value::String(_) | Value::Object(_) => {
            return Err(source("'str' object has no attribute 'get'".into()));
        }
        _ => return Err(source(format!("'{}' object is not iterable", kind(papers)))),
    };
    let mut results = Vec::new();
    let mut refusal = None;
    for (index, paper) in papers.iter().enumerate() {
        checkpoint(deadline).await?;
        let m = paper
            .as_object()
            .ok_or_else(|| source(format!("'{}' object has no attribute 'get'", kind(paper))))?;
        let title = field(m, "title", &empty);
        let url = field(m, "url", &empty);
        let raw_abstract = field(m, "abstract", &null);
        let abstract_value = if truthy(raw_abstract) {
            raw_abstract
        } else {
            &empty
        };
        let citation = field(m, "citationCount", &zero);
        let raw_date = field(m, "publicationDate", &null);
        let date = if truthy(raw_date) { raw_date } else { &null };
        let external = m.get("externalIds").and_then(Value::as_object);
        let raw_authors = field(m, "authors", &empty_list);
        let mut names = Vec::new();
        if truthy(raw_authors) {
            match raw_authors {
                Value::Array(authors) => {
                    for (index, author) in authors.iter().enumerate() {
                        if index % 128 == 0 {
                            checkpoint(deadline).await?;
                        }
                        if let Some(author) = author.as_object() {
                            names.push(field(author, "name", &empty));
                        }
                    }
                }
                Value::String(_) | Value::Object(_) => {}
                _ => {
                    return Err(source(format!(
                        "'{}' object is not iterable",
                        kind(raw_authors)
                    )));
                }
            }
        }
        let first = names
            .iter()
            .take(3)
            .enumerate()
            .map(|(i, v)| match v {
                Value::String(text) => Ok(text.clone()),
                _ => Err(source(format!(
                    "sequence item {i}: expected str instance, {} found",
                    kind(v)
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut author = join(&first, ", ");
        if names.len() > 3 {
            author = Text::concat(&[author, Text::from(" et al.")]);
        }
        let mut parts = Vec::new();
        if truthy(abstract_value) {
            let text = abstract_value.as_text().ok_or_else(|| {
                source(format!(
                    "'{}' object has no attribute 'strip'",
                    kind(abstract_value)
                ))
            })?;
            let mut clean = trim(text).take(300);
            if text.len() > 300 {
                clean = Text::concat(&[clean, Text::from("…")]);
            }
            parts.push(clean);
        }
        if !author.is_empty() {
            parts.push(Text::concat(&[Text::from("By: "), author]));
        }
        parts.push(Text::concat(&[
            Text::from("Cited by: "),
            citation.python_string(),
        ]));
        if let Some(arxiv) = external.and_then(|m| m.get("ArXiv")).filter(|v| truthy(v)) {
            parts.push(Text::concat(&[
                Text::from("arXiv: "),
                arxiv.python_string(),
            ]));
        }
        let content = join(&parts, " | ");
        // float(citation_count) precedes build_payload in the source constructor.
        let raw_score = float(citation, deadline).await?;
        let score = match FiniteF64::new(raw_score) {
            Ok(v) => v,
            Err(_) => {
                keep_first(
                    &mut refusal,
                    "Semantic Scholar result score must be finite".into(),
                );
                FiniteF64::default()
            }
        };
        let mut fields = Vec::new();
        for (source_name, target) in [
            ("DOI", "doi"),
            ("PubMed", "pmid"),
            ("PubMedCentral", "pmcid"),
            ("ArXiv", "arxiv_id"),
        ] {
            if let Some(v) = external
                .and_then(|m| m.get(source_name))
                .filter(|v| truthy(v))
            {
                fields.push((target, v));
            }
        }
        let payload = payload(&fields, &names, &mut refusal)?;
        let title = scalar(title, false, "title", &mut refusal).unwrap();
        let url = scalar(url, false, "url", &mut refusal).unwrap();
        let date = scalar(date, true, "published_date", &mut refusal);
        let content = scalar(&Value::String(content), false, "content", &mut refusal).unwrap();
        results.push(SearchResult {
            title,
            url,
            content,
            engine: "semanticscholar".into(),
            position: index as i64 + 1,
            score,
            published_date: date,
            payload: Some(payload),
            ..Default::default()
        });
    }
    checkpoint(deadline).await?;
    if let Some(error) = refusal {
        Err(ParseError::Output(error))
    } else {
        Ok(results)
    }
}

#[cfg(test)]
mod numeric_tests {
    use super::*;
    use std::time::Duration;
    #[tokio::test]
    async fn python_float_lexical_forms_before_shared_decimal_conversion() {
        let fixture: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-float-lexical.json"
        ))
        .unwrap();
        for row in fixture["records"].as_array().unwrap() {
            let text = row["text"].as_str().unwrap();
            let actual = string_float(&Text::from(text), Instant::now() + Duration::from_secs(5))
                .await
                .unwrap();
            if let Some(bits) = row["bits"].as_str() {
                assert_eq!(
                    format!("{:016x}", actual.unwrap().to_bits()),
                    bits,
                    "{text:?}"
                );
            } else {
                assert_eq!(actual, None, "{text:?}");
            }
        }
    }
    #[tokio::test]
    async fn long_mixed_width_float_whitespace_observes_deadline() {
        let text = Text::from(format!("{}1", "\u{a0}\u{2000}".repeat(400_000)).as_str());
        assert_eq!(
            string_float(&text, Instant::now() + Duration::from_millis(1)).await,
            Err(ParseError::Deadline)
        );
    }
}
