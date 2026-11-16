//! Open Library work → edition → ISBN identity and source book projection.
use super::{catalog_media as m, clinicaltrials as c, source_json::Value};
use ssx_domain::SearchRequest;
use std::time::Instant;
pub const ENDPOINT: &str = "https://openlibrary.org/search.json";
pub type OpenLibraryConfig = m::Settings;
m::adapter!(
    OpenLibraryAdapter,
    OpenLibraryConfig,
    "openlibrary",
    OpenLibraryConfig::defaults(ENDPOINT)
);
fn key(value: &Value, work: bool) -> Option<&str> {
    let raw = value
        .as_text()?
        .as_str()?
        .trim_matches(ssx_json::python_whitespace);
    let raw = raw
        .strip_prefix(if work { "/works/" } else { "/books/" })
        .or_else(|| raw.strip_prefix(if work { "works/" } else { "books/" }))
        .unwrap_or(raw);
    let digits = raw
        .strip_prefix("OL")?
        .strip_suffix(if work { 'W' } else { 'M' })?;
    (!digits.is_empty() && digits.chars().all(ssx_json::decimal_digit)).then_some(raw)
}
async fn identity(
    doc: &Value,
    b: &c::Budget,
    context: Option<Instant>,
) -> m::Result<Option<c::OwnedString>> {
    c::object(doc)?;
    if let Some(id) = doc.get("key").and_then(|v| key(v, true)) {
        return Ok(Some(
            m::join(b, &["https://openlibrary.org/works/", id], None, context).await?,
        ));
    }
    if let Some(v) = doc.get("edition_key") {
        match v {
            Value::String(_) => {
                if let Some(id) = key(v, false) {
                    return Ok(Some(
                        m::join(b, &["https://openlibrary.org/books/", id], None, context).await?,
                    ));
                }
            }
            Value::Array(values) => {
                for value in values {
                    c::row_checkpoint(context).await?;
                    if let Some(id) = key(value, false) {
                        return Ok(Some(
                            m::join(b, &["https://openlibrary.org/books/", id], None, context)
                                .await?,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    let values: &[Value] = match doc.get("isbn") {
        Some(v @ Value::String(_)) => std::slice::from_ref(v),
        Some(Value::Array(values)) => values,
        _ => &[],
    };
    for value in values {
        c::row_checkpoint(context).await?;
        if !matches!(
            value,
            Value::String(_)
                | Value::Number(super::source_json::Number::Integer(_))
                | Value::Bool(_)
        ) {
            continue;
        }
        let raw = c::owned_stringify(b, value, context).await?;
        let mut clean = c::OwnedString::new(b).map_err(c::owned_error)?;
        clean.try_reserve_exact(raw.len()).map_err(c::owned_error)?;
        for (i, ch) in raw.as_str().chars().enumerate() {
            if i % 1024 == 0 {
                c::row_checkpoint(context).await?;
            }
            if ch != '-' && ch != ' ' {
                clean.try_push(ch).map_err(c::owned_error)?
            }
        }
        let s = clean.as_str().trim_matches(ssx_json::python_whitespace);
        if (s.len() == 13 && s.bytes().all(|v| v.is_ascii_digit()))
            || (s.len() == 10
                && s.as_bytes()[..9].iter().all(u8::is_ascii_digit)
                && (s.as_bytes()[9].is_ascii_digit() || matches!(s.as_bytes()[9], b'X' | b'x')))
        {
            return Ok(Some(
                m::join(b, &["https://openlibrary.org/isbn/", s], None, context).await?,
            ));
        }
    }
    Ok(None)
}
impl OpenLibraryAdapter {
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        b: &'b c::Budget,
    ) -> m::Result<m::Batch<'b>> {
        c::admit_with_context(&self.admission, "openlibrary", context).await?;
        let deadline = c::network_deadline(&self.config, false, context)?;
        let max = c::scalar_text(&self.config.max_results);
        let req = c::request(
            &self.config.base_url,
            &[("q", &q.query), ("limit", &max)],
            true,
            false,
        )?;
        let data = m::fetch(
            &self.http,
            req,
            deadline,
            context,
            false,
            false,
            "Open Library",
        )
        .await?;
        let empty = c::empty();
        let list = c::list();
        let zero = Value::Number(super::source_json::Number::Float(0.));
        let rows = c::result_rows(c::field(&data, "docs", &list)?, &self.config.max_results)?;
        let mut batch = m::Batch::new(b, rows.len(), context)?;
        for doc in rows {
            c::row_checkpoint(context).await?;
            let Some(url) = identity(doc, b, context).await? else {
                continue;
            };
            let title = m::text(c::field(doc, "title", &empty)?)?;
            let null = Value::Null;
            let default_author = Value::Array(vec![null]);
            let authors = c::field(doc, "author_name", &default_author)?;
            let (author, author_truthy) = if !c::truthy(authors) {
                (m::owned(b, "", context).await?, false)
            } else {
                match authors {
                    Value::Array(a) => (
                        c::owned_stringify(b, &a[0], context).await?,
                        c::truthy(&a[0]),
                    ),
                    Value::String(s) => (c::owned_prefix(b, s, 1, context).await?, true),
                    _ => return Err(m::error("Open Library author does not support indexing")),
                }
            };
            let by = if author_truthy {
                m::join(b, &["By ", author.as_str()], None, context).await?
            } else {
                m::owned(b, "", context).await?
            };
            let year = c::field(doc, "first_publish_year", &empty)?;
            let year_text = c::owned_stringify(b, year, context).await?;
            let dated = if c::truthy(year) {
                if by.is_empty() {
                    m::join(b, &["Published ", year_text.as_str()], None, context).await?
                } else {
                    m::join(
                        b,
                        &[by.as_str(), " (", year_text.as_str(), ")"],
                        None,
                        context,
                    )
                    .await?
                }
            } else {
                by
            };
            let editions = c::field(doc, "edition_count", &zero)?;
            let edition_text = c::owned_stringify(b, editions, context).await?;
            let content = if c::truthy(editions) {
                m::join(
                    b,
                    &[dated.as_str(), " — ", edition_text.as_str(), " editions"],
                    Some(500),
                    context,
                )
                .await?
            } else {
                m::join(b, &[dated.as_str()], Some(500), context).await?
            };
            let cover = doc.get("cover_i").filter(|v| c::truthy(v));
            let cover_text = if let Some(v) = cover {
                Some(c::owned_stringify(b, v, context).await?)
            } else {
                None
            };
            let thumbnail = if let Some(v) = &cover_text {
                Some(
                    m::join(
                        b,
                        &["https://covers.openlibrary.org/b/id/", v.as_str(), "-M.jpg"],
                        None,
                        context,
                    )
                    .await?,
                )
            } else {
                None
            };
            let score = c::field(doc, "ratings_count", &zero)?;
            let score = if c::truthy(score) {
                m::float(score)?
            } else {
                0.
            };
            batch
                .rows
                .push_with_thumbnail(
                    c::Skeleton {
                        url: url.as_str(),
                        title,
                        content: content.as_str(),
                        engine: "openlibrary",
                        category: "general",
                        score,
                        position: (batch.rows.results().len() + 1) as i64,
                        tier: 1,
                        published_date: None,
                    },
                    None,
                    thumbnail.as_ref().map(c::OwnedString::as_str),
                )
                .await
                .map_err(c::projection_error)?;
        }
        Ok(batch)
    }
}

#[cfg(test)]
fn set_test_key(config: &mut OpenLibraryConfig) {
    let _ = config;
}
