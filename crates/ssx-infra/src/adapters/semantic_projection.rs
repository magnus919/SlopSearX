//! Bounded actual Semantic Scholar source operations over borrowed SourceJSON.
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
        "Semantic Scholar result title must be a string",
        "Semantic Scholar result url must be a string",
        "Semantic Scholar result published_date must be a string or null",
    ];
    const UNICODE: [&str; 3] = [
        "Semantic Scholar result title must contain only Unicode scalar values",
        "Semantic Scholar result url must contain only Unicode scalar values",
        "Semantic Scholar result published_date must contain only Unicode scalar values",
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
async fn part(out: &mut OwnedString, count: &mut usize, deadline: Instant) -> Result<(), Error> {
    if *count != 0 {
        append(out, " | ", deadline).await?;
    }
    *count += 1;
    Ok(())
}
/// Input DOM ownership must remain charged/live in the same caller root until
/// return. Output refusal is deferred until all source rows finish, preserving
/// later source-error precedence; no invalid scalar result ever escapes.
pub async fn parse<'a>(
    data: &Value,
    budget: &'a Budget,
    deadline: Instant,
) -> Result<ProducedBatch<'a>, Error> {
    check(deadline).await?;
    let map = match data.as_object() {
        Some(map) => map,
        None => {
            return Err(message(
                budget,
                &["'", kind(data), "' object has no attribute 'get'"],
                deadline,
            )
            .await?);
        }
    };
    let raw = get(map, "data", deadline).await?;
    let papers = match raw {
        None => &[][..],
        Some(Value::Array(papers)) => papers.as_slice(),
        Some(Value::String(text)) if text.is_empty() => &[],
        Some(Value::Object(map)) if map.is_empty() => &[],
        Some(Value::String(_) | Value::Object(_)) => {
            return Err(message(budget, &["'str' object has no attribute 'get'"], deadline).await?);
        }
        Some(value) => {
            return Err(message(
                budget,
                &["'", kind(value), "' object is not iterable"],
                deadline,
            )
            .await?);
        }
    };
    let empty = Value::String(Text::from("")); // zero-capacity default, no heap backing
    let mut batch =
        ProducedBatch::new(budget, papers.len(), Some(deadline)).map_err(project_error)?;
    let mut refusal = None;
    for (index, paper) in papers.iter().enumerate() {
        check(deadline).await?;
        let map = match paper.as_object() {
            Some(map) => map,
            None => {
                return Err(message(
                    budget,
                    &["'", kind(paper), "' object has no attribute 'get'"],
                    deadline,
                )
                .await?);
            }
        };
        let title = get(map, "title", deadline).await?;
        let url = get(map, "url", deadline).await?;
        let abstract_value = get(map, "abstract", deadline)
            .await?
            .filter(|value| truth(value));
        let citation = get(map, "citationCount", deadline).await?;
        let date = get(map, "publicationDate", deadline)
            .await?
            .filter(|value| truth(value));
        let external = get(map, "externalIds", deadline)
            .await?
            .and_then(Value::as_object);
        let raw_authors = get(map, "authors", deadline)
            .await?
            .filter(|value| truth(value));
        let authors = match raw_authors {
            None => &[][..],
            Some(Value::Array(authors)) => authors.as_slice(),
            Some(Value::String(_) | Value::Object(_)) => &[],
            Some(value) => {
                return Err(message(
                    budget,
                    &["'", kind(value), "' object is not iterable"],
                    deadline,
                )
                .await?);
            }
        };
        let mut names = OwnedVec::with_capacity(budget, authors.len())?;
        for (i, author) in authors.iter().enumerate() {
            if i.is_multiple_of(128) {
                check(deadline).await?;
            }
            if let Some(map) = author.as_object() {
                names.try_push(get(map, "name", deadline).await?.unwrap_or(&empty))?;
            }
        }
        let mut author = OwnedString::new(budget)?;
        let mut valid_content = true;
        let mut capacity = if names.len() > 3 { 7usize } else { 0 };
        for (i, name) in names.as_slice().iter().take(3).enumerate() {
            let text = match name.as_text() {
                Some(text) => text,
                None => {
                    let index = match i {
                        0 => "0",
                        1 => "1",
                        _ => "2",
                    };
                    return Err(message(
                        budget,
                        &[
                            "sequence item ",
                            index,
                            ": expected str instance, ",
                            kind(name),
                            " found",
                        ],
                        deadline,
                    )
                    .await?);
                }
            };
            capacity = capacity
                .checked_add(text.len().checked_mul(4).ok_or(Error::Resource)?)
                .and_then(|n| n.checked_add(if i == 0 { 0 } else { 2 }))
                .ok_or(Error::Resource)?;
        }
        author.try_reserve_exact(capacity)?;
        for (i, name) in names.as_slice().iter().take(3).enumerate() {
            if i != 0 {
                append(&mut author, ", ", deadline).await?;
            }
            points(
                &mut author,
                name.as_text().ok_or(Error::Defect)?.codepoints(),
                &mut valid_content,
                deadline,
            )
            .await?;
        }
        if names.len() > 3 {
            append(&mut author, " et al.", deadline).await?;
        }
        let mut content = OwnedString::new(budget)?;
        let mut count = 0;
        if let Some(value) = abstract_value {
            let text = match value.as_text() {
                Some(text) => text,
                None => {
                    return Err(message(
                        budget,
                        &["'", kind(value), "' object has no attribute 'strip'"],
                        deadline,
                    )
                    .await?);
                }
            };
            let text_trimmed = trimmed(text, deadline).await?;
            part(&mut content, &mut count, deadline).await?;
            points(
                &mut content,
                &text_trimmed[..text_trimmed.len().min(300)],
                &mut valid_content,
                deadline,
            )
            .await?;
            if text.len() > 300 {
                append(&mut content, "…", deadline).await?;
            }
        }
        if !author.is_empty() {
            part(&mut content, &mut count, deadline).await?;
            append(&mut content, "By: ", deadline).await?;
            append(&mut content, author.as_str(), deadline).await?;
        }
        part(&mut content, &mut count, deadline).await?;
        append(&mut content, "Cited by: ", deadline).await?;
        match citation {
            None => append(&mut content, "0", deadline).await?,
            Some(Value::String(text)) => {
                points(
                    &mut content,
                    text.codepoints(),
                    &mut valid_content,
                    deadline,
                )
                .await?
            }
            Some(value) => {
                let value = repr(value, budget, deadline).await?;
                append(&mut content, value.as_str(), deadline).await?;
            }
        }
        if let Some(external) = external
            && let Some(value) = get(external, "ArXiv", deadline)
                .await?
                .filter(|value| truth(value))
        {
            part(&mut content, &mut count, deadline).await?;
            append(&mut content, "arXiv: ", deadline).await?;
            if let Some(text) = value.as_text() {
                points(
                    &mut content,
                    text.codepoints(),
                    &mut valid_content,
                    deadline,
                )
                .await?;
            } else {
                let value = repr(value, budget, deadline).await?;
                append(&mut content, value.as_str(), deadline).await?;
            }
        }
        let raw_score = float(citation, budget, deadline).await?;
        let score = if raw_score.is_finite() {
            raw_score
        } else {
            keep(&mut refusal, "Semantic Scholar result score must be finite");
            0.0
        };
        let mut fields = OwnedVec::with_capacity(budget, 5)?;
        if let Some(external) = external {
            for (source, name) in [
                ("DOI", "doi"),
                ("PubMed", "pmid"),
                ("PubMedCentral", "pmcid"),
                ("ArXiv", "arxiv_id"),
            ] {
                if let Some(value) = get(external, source, deadline)
                    .await?
                    .filter(|value| truth(value))
                {
                    fields.try_push(Field {
                        name,
                        value: FieldRef::Value(value),
                    })?;
                }
            }
        }
        if !names.is_empty() {
            fields.try_push(Field {
                name: "authors",
                value: FieldRef::SelectedValues(names.as_slice()),
            })?;
        }
        let payload = match source_payload::publication_budgeted(
            "semanticscholar",
            fields.as_slice(),
            budget,
            Some(deadline),
        )
        .await
        {
            Ok(value) => Some(value),
            Err(BudgetError::Projection(super::source_json::ProjectionError::Nonfinite)) => {
                keep(
                    &mut refusal,
                    "Semantic Scholar payload must contain only finite JSON numbers",
                );
                None
            }
            Err(error) => return Err(project_error(error)),
        };
        let title = scalar(title, false, 0, &mut refusal).unwrap_or("");
        let url = scalar(url, false, 1, &mut refusal).unwrap_or("");
        let date = scalar(date, true, 2, &mut refusal);
        if !valid_content {
            keep(
                &mut refusal,
                "Semantic Scholar result content must contain only Unicode scalar values",
            );
        }
        batch
            .push(
                Skeleton {
                    url,
                    title,
                    content: content.as_str(),
                    engine: "semanticscholar",
                    category: "general",
                    score,
                    position: i64::try_from(index)
                        .map_err(|_| Error::Resource)?
                        .checked_add(1)
                        .ok_or(Error::Resource)?,
                    tier: 1,
                    published_date: date,
                },
                payload,
            )
            .await
            .map_err(project_error)?;
    }
    check(deadline).await?;
    if let Some(error) = refusal {
        Err(Error::Output(error))
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
