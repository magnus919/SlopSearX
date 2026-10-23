//! Bounded actual OpenAlex source operations over borrowed SourceJSON.
use super::{
    source_json::{Map, Number, Text, Value},
    source_payload::{self, BudgetError, Field, FieldRef, ProducedBatch, Skeleton},
};
use ssx_owned_budget::{Budget, OwnedString, OwnedVec};
use std::time::Instant;
pub enum Error {
    Source(OwnedString),
    Output(&'static str),
    Resource,
    Deadline,
    Defect,
}

impl From<ssx_owned_budget::Error> for Error {
    fn from(_: ssx_owned_budget::Error) -> Self {
        Self::Resource
    }
}
async fn check(deadline: Instant) -> Result<(), Error> {
    if Instant::now() >= deadline {
        return Err(Error::Deadline);
    }
    tokio::task::yield_now().await;
    if Instant::now() >= deadline {
        Err(Error::Deadline)
    } else {
        Ok(())
    }
}
fn project_error(error: BudgetError) -> Error {
    match error {
        BudgetError::Deadline => Error::Deadline,
        BudgetError::Projection(_) => Error::Defect,
        _ => Error::Resource,
    }
}
fn kind(value: &Value) -> &'static str {
    super::broad_type(value)
}
fn truth(value: &Value) -> bool {
    super::broad_truthy(Some(value))
}
async fn get<'a>(map: &'a Map, name: &str, deadline: Instant) -> Result<Option<&'a Value>, Error> {
    for (i, (key, value)) in map.iter().enumerate() {
        if i.is_multiple_of(128) {
            check(deadline).await?;
        }
        if key.as_str() == Some(name) {
            return Ok(Some(value));
        }
    }
    Ok(None)
}
fn push(out: &mut OwnedString, ch: char) -> Result<(), Error> {
    let wanted = out
        .len()
        .checked_add(ch.len_utf8())
        .ok_or(Error::Resource)?;
    if wanted > out.capacity() {
        let capacity = out
            .capacity()
            .checked_mul(2)
            .and_then(|n| n.checked_add(8))
            .ok_or(Error::Resource)?
            .max(wanted);
        out.try_reserve_exact(capacity - out.len())?;
    }
    out.try_push(ch)?;
    Ok(())
}
async fn append(out: &mut OwnedString, value: &str, deadline: Instant) -> Result<(), Error> {
    for (i, ch) in value.chars().enumerate() {
        if i.is_multiple_of(1024) {
            check(deadline).await?;
        }
        push(out, ch)?;
    }
    Ok(())
}
async fn message(budget: &Budget, parts: &[&str], deadline: Instant) -> Result<Error, Error> {
    let mut out = OwnedString::new(budget)?;
    let capacity = parts
        .iter()
        .try_fold(0usize, |size, part| size.checked_add(part.len()))
        .ok_or(Error::Resource)?;
    out.try_reserve_exact(capacity)?;
    for part in parts {
        append(&mut out, part, deadline).await?;
    }
    Ok(Error::Source(out))
}
async fn repr(value: &Value, budget: &Budget, deadline: Instant) -> Result<OwnedString, Error> {
    super::source_repr::repr_budgeted(value, budget, Some(deadline))
        .await
        .map_err(|error| match error {
            super::source_repr::Error::Resource => Error::Resource,
            super::source_repr::Error::Deadline => Error::Deadline,
            super::source_repr::Error::Unicode => Error::Defect,
        })
}
async fn float(value: Option<&Value>, budget: &Budget, deadline: Instant) -> Result<f64, Error> {
    match value {
        None => Ok(0.0),
        Some(Value::Bool(value)) => Ok(f64::from(u8::from(*value))),
        Some(Value::Number(Number::Float(value))) => Ok(*value),
        Some(Value::Number(value)) => match value.as_f64() {
            Some(value) => Ok(value),
            None => Err(message(budget, &["int too large to convert to float"], deadline).await?),
        },
        Some(value @ Value::String(text)) => {
            match super::scholarly_float::string_float(text, budget, deadline)
                .await
                .map_err(|error| match error {
                    super::scholarly_float::Error::Resource => Error::Resource,
                    super::scholarly_float::Error::Deadline => Error::Deadline,
                })? {
                Some(value) => Ok(value),
                None => {
                    let quoted = repr(value, budget, deadline).await?;
                    Err(message(
                        budget,
                        &["could not convert string to float: ", quoted.as_str()],
                        deadline,
                    )
                    .await?)
                }
            }
        }
        Some(value) => Err(message(
            budget,
            &[
                "float() argument must be a string or a real number, not '",
                kind(value),
                "'",
            ],
            deadline,
        )
        .await?),
    }
}
fn keep(refusal: &mut Option<&'static str>, value: &'static str) {
    if refusal.is_none() {
        *refusal = Some(value);
    }
}
fn scalar<'a>(
    value: Option<&'a Value>,
    optional: bool,
    field: usize,
    refusal: &mut Option<&'static str>,
) -> Option<&'a str> {
    const TYPE: [&str; 3] = [
        "OpenAlex result title must be a string",
        "OpenAlex result url must be a string",
        "OpenAlex result published_date must be a string or null",
    ];
    const UNICODE: [&str; 3] = [
        "OpenAlex result title must contain only Unicode scalar values",
        "OpenAlex result url must contain only Unicode scalar values",
        "OpenAlex result published_date must contain only Unicode scalar values",
    ];
    match value {
        None => {
            if optional {
                None
            } else {
                Some("")
            }
        }
        Some(Value::Null) if optional => None,
        Some(Value::String(text)) => match text.as_str() {
            Some(text) => Some(text),
            None => {
                keep(refusal, UNICODE[field]);
                Some("")
            }
        },
        _ => {
            keep(refusal, TYPE[field]);
            if optional { None } else { Some("") }
        }
    }
}
async fn points(
    out: &mut OwnedString,
    text: &[u32],
    valid: &mut bool,
    deadline: Instant,
) -> Result<(), Error> {
    for (i, point) in text.iter().enumerate() {
        if i.is_multiple_of(1024) {
            check(deadline).await?;
        }
        if let Some(ch) = char::from_u32(*point) {
            push(out, ch)?;
        } else if (0xd800..=0xdfff).contains(point) {
            *valid = false;
        } else {
            return Err(Error::Defect);
        }
    }
    Ok(())
}
async fn trimmed(text: &Text, deadline: Instant) -> Result<&[u32], Error> {
    let points = text.codepoints();
    let mut first = 0;
    for (i, point) in points.iter().enumerate() {
        if i.is_multiple_of(1024) {
            check(deadline).await?;
        }
        if !char::from_u32(*point).is_some_and(super::broad_whitespace) {
            break;
        }
        first = i + 1;
    }
    let mut last = points.len();
    for (i, point) in points[first..].iter().rev().enumerate() {
        if i.is_multiple_of(1024) {
            check(deadline).await?;
        }
        if !char::from_u32(*point).is_some_and(super::broad_whitespace) {
            break;
        }
        last -= 1;
    }
    Ok(&points[first..last])
}
async fn scalar_repr(raw: &str, budget: &Budget, deadline: Instant) -> Result<OwnedString, Error> {
    let (mut single, mut double) = (false, false);
    for (i, ch) in raw.chars().enumerate() {
        if i % 1024 == 0 {
            check(deadline).await?;
        }
        single |= ch == '\'';
        double |= ch == '"';
    }
    let q = if single && !double { '"' } else { '\'' };
    let mut out = OwnedString::new(budget)?;
    push(&mut out, q)?;
    for (i, ch) in raw.chars().enumerate() {
        if i % 1024 == 0 {
            check(deadline).await?;
        }
        let c = ssx_domain::storage::python_repr_character(
            ch,
            if q == '"' {
                ssx_domain::storage::PythonReprQuote::Double
            } else {
                ssx_domain::storage::PythonReprQuote::Single
            },
        );
        append(&mut out, c.as_str(), deadline).await?;
    }
    push(&mut out, q)?;
    Ok(out)
}
async fn slice_key(
    limit: &super::scholarly_config::Scalar,
    budget: &Budget,
    deadline: Instant,
) -> Result<Error, Error> {
    let _numeric = budget.reserve(256)?;
    let raw = match limit {
        super::scholarly_config::Scalar::Text(s) => scalar_repr(s, budget, deadline).await?,
        _ => {
            let mut out = OwnedString::new(budget)?;
            append(&mut out, &limit.python_text(), deadline).await?;
            out
        }
    };
    message(budget, &["slice(None, ", raw.as_str(), ", None)"], deadline).await
}
async fn abstract_text(
    value: Option<&Value>,
    budget: &Budget,
    deadline: Instant,
) -> Result<OwnedVec<u32>, Error> {
    use super::scholarly_order::{Pair, Ref};
    let Some(value) = value.filter(|v| truth(v)) else {
        return Ok(OwnedVec::with_capacity(budget, 0)?);
    };
    let Some(map) = value.as_object() else {
        return Err(message(
            budget,
            &["'", kind(value), "' object has no attribute 'items'"],
            deadline,
        )
        .await?);
    };
    let mut count = 0usize;
    for (i, (_, positions)) in map.iter().enumerate() {
        if i % 128 == 0 {
            check(deadline).await?;
        }
        let n = match positions {
            Value::Array(v) => v.len(),
            Value::String(v) => v.len(),
            Value::Object(v) => v.len(),
            v => {
                return Err(message(
                    budget,
                    &["'", kind(v), "' object is not iterable"],
                    deadline,
                )
                .await?);
            }
        };
        count = count.checked_add(n).ok_or(Error::Resource)?;
    }
    let mut pairs = OwnedVec::with_capacity(budget, count)?;
    for (i, (word, positions)) in map.iter().enumerate() {
        if i % 128 == 0 {
            check(deadline).await?;
        }
        match positions {
            Value::Array(v) => {
                for (j, p) in v.iter().enumerate() {
                    if j % 1024 == 0 {
                        check(deadline).await?;
                    }
                    pairs.try_push(Pair {
                        position: Ref::Value(p),
                        word,
                    })?;
                }
            }
            Value::String(v) => {
                for (j, p) in v.codepoints().iter().enumerate() {
                    if j % 1024 == 0 {
                        check(deadline).await?;
                    }
                    pairs.try_push(Pair {
                        position: Ref::Character(*p),
                        word,
                    })?;
                }
            }
            Value::Object(v) => {
                for (j, (p, _)) in v.iter().enumerate() {
                    if j % 1024 == 0 {
                        check(deadline).await?;
                    }
                    pairs.try_push(Pair {
                        position: Ref::Text(p),
                        word,
                    })?;
                }
            }
            _ => return Err(Error::Defect),
        }
    }
    let permutation = super::scholarly_sort::order_owned(pairs.as_slice(), deadline, budget)
        .await
        .map_err(|e| match e {
            super::scholarly_sort::BoundedError::Source(m) => Error::Source(m),
            super::scholarly_sort::BoundedError::Resource => Error::Resource,
            super::scholarly_sort::BoundedError::Deadline => Error::Deadline,
            super::scholarly_sort::BoundedError::Invariant => Error::Defect,
        })?;
    let mut length = pairs.len().saturating_sub(1);
    for (i, at) in permutation.as_slice().iter().enumerate() {
        if i % 128 == 0 {
            check(deadline).await?;
        }
        length = length
            .checked_add(pairs.as_slice()[*at].word.len())
            .ok_or(Error::Resource)?;
    }
    let mut out = OwnedVec::with_capacity(budget, length)?;
    for (i, at) in permutation.as_slice().iter().enumerate() {
        if i % 128 == 0 {
            check(deadline).await?;
        }
        if i > 0 {
            out.try_push(32)?;
        }
        for (j, p) in pairs.as_slice()[*at].word.codepoints().iter().enumerate() {
            if j % 1024 == 0 {
                check(deadline).await?;
            }
            out.try_push(*p)?;
        }
    }
    Ok(out)
}
/// Actual OpenAlex source row operations. DOM, full joined abstract and produced
/// batch overlap in the same caller root. No invalid scalar output escapes.
pub async fn parse<'a>(
    data: &Value,
    limit: &super::scholarly_config::Scalar,
    bounds: ssx_application::filters::PublicationBounds,
    filter_dates: bool,
    budget: &'a Budget,
    deadline: Instant,
) -> Result<ProducedBatch<'a>, Error> {
    check(deadline).await?;
    let Some(map) = data.as_object() else {
        return Err(message(
            budget,
            &["'", kind(data), "' object has no attribute 'get'"],
            deadline,
        )
        .await?);
    };
    let raw = get(map, "results", deadline).await?;
    let papers = match raw {
        None => {
            if let Err(m) = limit.slice_end(0) {
                return Err(message(budget, &[m], deadline).await?);
            }
            &[][..]
        }
        Some(Value::Array(v)) => {
            let end = match limit.slice_end(v.len()) {
                Ok(n) => n,
                Err(m) => return Err(message(budget, &[m], deadline).await?),
            };
            &v[..end]
        }
        Some(Value::String(v)) => {
            let end = match limit.slice_end(v.len()) {
                Ok(n) => n,
                Err(m) => return Err(message(budget, &[m], deadline).await?),
            };
            if end > 0 {
                return Err(
                    message(budget, &["'str' object has no attribute 'get'"], deadline).await?,
                );
            }
            &[]
        }
        Some(Value::Object(_)) => return Err(slice_key(limit, budget, deadline).await?),
        Some(v) => {
            return Err(message(
                budget,
                &["'", kind(v), "' object is not subscriptable"],
                deadline,
            )
            .await?);
        }
    };
    let empty = Value::String(Text::from(""));
    let mut batch =
        ProducedBatch::new(budget, papers.len(), Some(deadline)).map_err(project_error)?;
    let mut refusal = None;
    for work in papers {
        check(deadline).await?;
        let Some(work) = work.as_object() else {
            return Err(message(
                budget,
                &["'", kind(work), "' object has no attribute 'get'"],
                deadline,
            )
            .await?);
        };
        let date = get(work, "publication_date", deadline).await?;
        if filter_dates && !bounds.contains(date.and_then(Value::as_str)) {
            continue;
        }
        let doi = get(work, "doi", deadline).await?;
        let id = get(work, "id", deadline).await?;
        let mut url_buffer = OwnedString::new(budget)?;
        let url_value = id.unwrap_or(&empty);
        let mut url_from_doi = false;
        if let Some(doi) = doi.filter(|v| truth(v)) {
            let Some(text) = doi.as_text() else {
                return Err(message(
                    budget,
                    &["'", kind(doi), "' object has no attribute 'strip'"],
                    deadline,
                )
                .await?);
            };
            let raw = trimmed(text, deadline).await?;
            if !raw.is_empty() {
                url_from_doi = true;
                let absolute = raw.starts_with(&[104, 116, 116, 112, 115, 58, 47, 47])
                    || raw.starts_with(&[104, 116, 116, 112, 58, 47, 47]);
                if !absolute {
                    append(&mut url_buffer, "https://doi.org/", deadline).await?;
                }
                let mut valid = true;
                points(&mut url_buffer, raw, &mut valid, deadline).await?;
                if !valid {
                    keep(
                        &mut refusal,
                        "OpenAlex result url must contain only Unicode scalar values",
                    );
                }
            }
        }
        let abstract_value = get(work, "abstract_inverted_index", deadline).await?;
        let full_abstract = abstract_text(abstract_value, budget, deadline).await?;
        let mut content = OwnedString::new(budget)?;
        let mut valid_content = true;
        let prefix = &full_abstract.as_slice()[..full_abstract.len().min(500)];
        points(&mut content, prefix, &mut valid_content, deadline).await?;
        let title = get(work, "title", deadline)
            .await?
            .filter(|v| truth(v))
            .unwrap_or(&empty);
        let raw_score = float(
            get(work, "relevance_score", deadline)
                .await?
                .filter(|v| truth(v)),
            budget,
            deadline,
        )
        .await?;
        let score = if raw_score.is_finite() {
            raw_score
        } else {
            keep(&mut refusal, "OpenAlex result score must be finite");
            0.
        };
        let mut fields = OwnedVec::with_capacity(budget, 2)?;
        if let Some(v) = doi.filter(|v| truth(v)) {
            fields.try_push(Field {
                name: "doi",
                value: FieldRef::Value(v),
            })?;
        }
        if let Some(v) = id.filter(|v| truth(v)) {
            fields.try_push(Field {
                name: "openalex_id",
                value: FieldRef::Value(v),
            })?;
        }
        let payload = match source_payload::publication_budgeted(
            "openalex",
            fields.as_slice(),
            budget,
            Some(deadline),
        )
        .await
        {
            Ok(v) => Some(v),
            Err(BudgetError::Projection(super::source_json::ProjectionError::Nonfinite)) => {
                keep(
                    &mut refusal,
                    "OpenAlex payload must contain only finite JSON numbers",
                );
                None
            }
            Err(e) => return Err(project_error(e)),
        };
        let title = scalar(Some(title), false, 0, &mut refusal).unwrap_or("");
        let url = if url_from_doi {
            url_buffer.as_str()
        } else {
            scalar(Some(url_value), false, 1, &mut refusal).unwrap_or("")
        };
        let date = scalar(date, true, 2, &mut refusal);
        if !valid_content {
            keep(
                &mut refusal,
                "OpenAlex result content must contain only Unicode scalar values",
            );
        }
        batch
            .push(
                Skeleton {
                    url,
                    title,
                    content: content.as_str(),
                    engine: "openalex",
                    category: "general",
                    score,
                    position: 0,
                    tier: 1,
                    published_date: date,
                },
                payload,
            )
            .await
            .map_err(project_error)?;
    }
    check(deadline).await?;
    if let Some(m) = refusal {
        Err(Error::Output(m))
    } else {
        Ok(batch)
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(_) => f.write_str("Source"),
            Self::Output(message) => f.debug_tuple("Output").field(message).finish(),
            Self::Resource => f.write_str("Resource"),
            Self::Deadline => f.write_str("Deadline"),
            Self::Defect => f.write_str("Defect"),
        }
    }
}

#[cfg(test)]
mod original_abstract_helper_tests {
    use super::*;
    #[tokio::test]
    async fn actual_abstract_helper_preserves_original_empty_simple_and_multiple_positions() {
        // Exact inputs/assertions from TestOpenAlexAdapter's three helper tests.
        for (input, expected) in [
            ("null", ""),
            ("{}", ""),
            (r#"{"hello":[0],"world":[1]}"#, "hello world"),
            (r#"{"the":[0,3],"cat":[1],"sat":[2]}"#, "the cat sat the"),
        ] {
            let budget = Budget::new(1024 * 1024).unwrap();
            let deadline = Instant::now() + std::time::Duration::from_secs(5);
            let source = super::super::source_json::from_slice_budgeted(
                input.as_bytes(),
                Some(deadline),
                &budget,
            )
            .await
            .unwrap();
            let text = abstract_text(Some(source.value()), &budget, deadline)
                .await
                .unwrap();
            assert_eq!(
                text.as_slice()
                    .iter()
                    .map(|p| char::from_u32(*p).unwrap())
                    .collect::<String>(),
                expected
            );
            drop(text);
            drop(source);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
}
