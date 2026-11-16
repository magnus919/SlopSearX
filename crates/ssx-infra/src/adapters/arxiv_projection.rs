//! Direct borrowed Expat fields to the shared charged ProducedBatch boundary.
use super::source_payload::{self, BudgetError, Field, FieldRef, ProducedBatch, Skeleton};
use ssx_owned_budget::{Budget, OwnedString, OwnedVec};
use ssx_source_xml::Entry;
use std::time::Instant;
fn space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
async fn checkpoint(deadline: Instant) -> Result<(), BudgetError> {
    tokio::task::yield_now().await;
    if Instant::now() >= deadline {
        Err(BudgetError::Deadline)
    } else {
        Ok(())
    }
}
async fn trim(text: &str, deadline: Instant) -> Result<&str, BudgetError> {
    let mut first = 0;
    for (i, (at, c)) in text.char_indices().enumerate() {
        if i % 1024 == 0 {
            checkpoint(deadline).await?
        }
        if !space(c) {
            first = at;
            break;
        }
        first = at + c.len_utf8();
    }
    let mut last = text.len();
    for (i, (at, c)) in text[first..].char_indices().rev().enumerate() {
        if i % 1024 == 0 {
            checkpoint(deadline).await?
        }
        if !space(c) {
            break;
        }
        last = first + at;
    }
    Ok(&text[first..last])
}
async fn append(out: &mut OwnedString, text: &str, deadline: Instant) -> Result<(), BudgetError> {
    for (i, c) in text.chars().enumerate() {
        if i % 1024 == 0 {
            checkpoint(deadline).await?
        }
        out.try_push(c).map_err(|_| BudgetError::Resource)?;
    }
    Ok(())
}
/// Caller retains parsed entries and their reservation until this future returns.
/// All temporary normalized strings and borrowed descriptor arrays use budget.
pub(super) async fn project<'a>(
    entries: &[Entry],
    budget: &'a Budget,
    deadline: Instant,
) -> Result<ProducedBatch<'a>, BudgetError> {
    checkpoint(deadline).await?;
    let mut batch = ProducedBatch::new(budget, entries.len(), Some(deadline))?;
    for entry in entries {
        checkpoint(deadline).await?;
        let title = trim(entry.fields[0].as_deref().unwrap_or(""), deadline).await?;
        let summary = entry.fields[1].as_deref().unwrap_or("");
        let mut abstract_text = OwnedString::new(budget).map_err(|_| BudgetError::Resource)?;
        abstract_text
            .try_reserve_exact(summary.len())
            .map_err(|_| BudgetError::Resource)?;
        let mut pending = false;
        for (i, c) in summary.chars().enumerate() {
            if i % 1024 == 0 {
                checkpoint(deadline).await?
            }
            if space(c) {
                pending = !abstract_text.is_empty();
            } else {
                if pending {
                    abstract_text
                        .try_push(' ')
                        .map_err(|_| BudgetError::Resource)?;
                    pending = false;
                }
                abstract_text
                    .try_push(c)
                    .map_err(|_| BudgetError::Resource)?;
            }
        }
        let mut id = trim(entry.fields[2].as_deref().unwrap_or(""), deadline).await?;
        for (i, (at, c)) in id.char_indices().rev().enumerate() {
            if i % 1024 == 0 {
                checkpoint(deadline).await?
            }
            if c == '/' {
                id = &id[at + 1..];
                break;
            }
        }
        let mut authors = OwnedVec::with_capacity(budget, entry.authors.len())
            .map_err(|_| BudgetError::Resource)?;
        for (index, author) in entry.authors.iter().enumerate() {
            if index % 128 == 0 {
                checkpoint(deadline).await?;
            }
            let author = trim(author, deadline).await?;
            if !author.is_empty() {
                authors
                    .try_push(author)
                    .map_err(|_| BudgetError::Resource)?;
            }
        }
        let mut fields = OwnedVec::with_capacity(budget, 8).map_err(|_| BudgetError::Resource)?;
        if !id.is_empty() {
            for name in ["publication_id", "arxiv_id"] {
                fields
                    .try_push(Field {
                        name,
                        value: FieldRef::Scalar(id),
                    })
                    .map_err(|_| BudgetError::Resource)?;
            }
        }
        if let Some(doi) = &entry.fields[4] {
            fields
                .try_push(Field {
                    name: "doi",
                    value: FieldRef::Scalar(doi),
                })
                .map_err(|_| BudgetError::Resource)?;
        }
        if !authors.is_empty() {
            fields
                .try_push(Field {
                    name: "authors",
                    value: FieldRef::SelectedScalars(authors.as_slice()),
                })
                .map_err(|_| BudgetError::Resource)?;
        }
        for (index, name) in [(5, "journal"), (6, "comments"), (7, "revision_date")] {
            if let Some(text) = &entry.fields[index] {
                fields
                    .try_push(Field {
                        name,
                        value: FieldRef::Scalar(text),
                    })
                    .map_err(|_| BudgetError::Resource)?;
            }
        }
        if !abstract_text.is_empty() {
            fields
                .try_push(Field {
                    name: "abstract",
                    value: FieldRef::Scalar(abstract_text.as_str()),
                })
                .map_err(|_| BudgetError::Resource)?;
        }
        let payload = source_payload::publication_budgeted(
            "arxiv",
            fields.as_slice(),
            budget,
            Some(deadline),
        )
        .await?;
        let mut url = OwnedString::new(budget).map_err(|_| BudgetError::Resource)?;
        url.try_reserve_exact(
            "https://arxiv.org/abs/"
                .len()
                .checked_add(id.len())
                .ok_or(BudgetError::Resource)?,
        )
        .map_err(|_| BudgetError::Resource)?;
        append(&mut url, "https://arxiv.org/abs/", deadline).await?;
        append(&mut url, id, deadline).await?;
        let mut content = OwnedString::new(budget).map_err(|_| BudgetError::Resource)?;
        content
            .try_reserve_exact(301 * 4)
            .map_err(|_| BudgetError::Resource)?;
        for (i, c) in abstract_text.as_str().chars().enumerate() {
            if i == 300 {
                content.try_push('…').map_err(|_| BudgetError::Resource)?;
                break;
            }
            content.try_push(c).map_err(|_| BudgetError::Resource)?;
        }
        let date = match entry.fields[3].as_deref() {
            Some(value) if !value.is_empty() => Some(trim(value, deadline).await?),
            _ => None,
        };
        let position = i64::try_from(batch.results().len())
            .ok()
            .and_then(|n| n.checked_add(1))
            .ok_or(BudgetError::Resource)?;
        batch
            .push(
                Skeleton {
                    url: url.as_str(),
                    title,
                    content: content.as_str(),
                    engine: "arxiv",
                    category: "general",
                    score: 0.0,
                    position,
                    tier: 1,
                    published_date: date,
                },
                Some(payload),
            )
            .await?;
    }
    Ok(batch)
}
