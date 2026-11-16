//! Pure shared publication codecs; no domain, policy, transport, or I/O.
use ssx_json::Value;
use std::collections::BTreeMap;
pub fn paper_fields(work_group: Option<&Value>) -> BTreeMap<String, Value> {
    let Some(paper) = work_group
        .and_then(Value::as_object)
        .and_then(|group| group.get("paper"))
        .and_then(Value::as_object)
    else {
        return BTreeMap::new();
    };
    let mut fields = BTreeMap::new();
    if let Some(doi) = paper
        .get("doi")
        .and_then(Value::as_str)
        .and_then(normalize_doi)
    {
        fields.insert("doi".into(), Value::String(doi));
    }
    for key in [
        "journal",
        "publisher",
        "editor",
        "pages",
        "number",
        "comments",
        "type",
        "pdf_url",
        "html_url",
    ] {
        if let Some(value) = paper
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 2048)
        {
            fields.insert(key.into(), Value::String(value.into()));
        }
    }
    for key in ["authors", "issn", "isbn", "tags"] {
        if let Some(values) = paper.get(key).and_then(Value::as_array).filter(|values| {
            values.len() <= 64
                && values
                    .iter()
                    .all(|value| value.as_str().is_some_and(|s| s.chars().count() <= 256))
        }) {
            fields.insert(key.into(), Value::Array(values.clone()));
        }
    }
    if let Some(value) = paper.get("volume") {
        let allowed = match value {
            Value::String(text) => text.chars().count() <= 256,
            Value::Number(number) => !number.is_f64() && number.as_str().len() <= 256,
            _ => false,
        };
        if allowed {
            fields.insert("volume".into(), value.clone());
        }
    }
    fields
}

// Python re.IGNORECASE adds these four Unicode letters to ASCII letter matching.
fn prefix_end(text: &str, prefix: &str) -> Option<usize> {
    let mut chars = text.char_indices();
    let mut end = 0;
    for expected in prefix.chars() {
        let (index, actual) = chars.next()?;
        let folded = match actual {
            'İ' | 'ı' => 'i',
            'ſ' => 's',
            'K' => 'k',
            ch => ch.to_ascii_lowercase(),
        };
        if folded != expected {
            return None;
        }
        end = index + actual.len_utf8();
    }
    Some(end)
}
pub fn normalize_doi(raw: &str) -> Option<String> {
    if raw.chars().count() > 512 {
        return None;
    }
    let mut text = raw.trim_matches(ssx_json::python_whitespace);
    for prefix in [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "http://dx.doi.org/",
        "doi:",
    ] {
        if let Some(end) = prefix_end(text, prefix) {
            text = &text[end..];
            if prefix == "doi:" {
                text = text.trim_start_matches(ssx_json::python_whitespace);
            }
            break;
        }
    }
    let (registrant, suffix) = text.split_once('/')?;
    let digits = registrant.strip_prefix("10.")?;
    if !(4..=9).contains(&digits.chars().count())
        || !digits.chars().all(ssx_json::decimal_digit)
        || suffix.is_empty()
        || suffix.chars().any(ssx_json::python_whitespace)
        || suffix.contains(['<', '>'])
    {
        return None;
    }
    Some(ssx_json::casefold(text))
}
