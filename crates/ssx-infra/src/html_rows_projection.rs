//! Engine-local produced rows. Requested backing charges survive until the
//! explicit AdapterResponse handoff; retained caller/clone lifetime is G02.
use super::{
    OperationFailure,
    row_wire::Layout,
    rows_borrowed::{Field, Media},
    rows_parent::OwnedRows,
};
use crate::html_budget::{Charge, Operation};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, MediaInfo, SearchResult};
use std::sync::Arc;
pub(crate) struct Batch {
    results: Vec<SearchResult>,
    _charge: Charge,
    _operation: Arc<Operation>,
}
impl Batch {
    pub(crate) fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
    pub(crate) fn into_adapter_response(mut self, latency_ms: FiniteF64) -> AdapterResponse {
        let results = std::mem::take(&mut self.results);
        // Named ownership transfer: the public response now owns its storage.
        // No clone, serialization or caller-lifetime bound is implied.
        AdapterResponse {
            results,
            status: EngineStatus::Ok,
            latency_ms,
            ..AdapterResponse::default()
        }
    }
}
fn resource(_: ssx_owned_budget::Error) -> OperationFailure {
    OperationFailure::Resource
}
fn check(operation: &Operation) -> Result<(), OperationFailure> {
    if tokio::time::Instant::now() >= operation.deadline {
        Err(OperationFailure::Deadline)
    } else {
        Ok(())
    }
}
fn reserve(batch: &mut Batch, n: usize) -> Result<(), OperationFailure> {
    let mut charge = batch._operation.budget.0.reserve(n).map_err(resource)?;
    batch._charge.merge_from(&mut charge).map_err(resource)
}
async fn copy_field(batch: &mut Batch, field: Field<'_>) -> Result<String, OperationFailure> {
    check(&batch._operation)?;
    reserve(batch, field.len())?;
    let mut output = String::new();
    output
        .try_reserve_exact(field.len())
        .map_err(|_| OperationFailure::Resource)?;
    if output.capacity() > field.len() {
        batch._operation.budget.0.fail_closed();
        return Err(OperationFailure::Resource);
    }
    for part in field.parts() {
        check(&batch._operation)?;
        output.push_str(
            part.map_err(|_| OperationFailure::Invariant("HTML helper invalid row UTF8"))?,
        );
        tokio::task::yield_now().await;
        check(&batch._operation)?;
    }
    Ok(output)
}
async fn copy_str(batch: &mut Batch, value: &str) -> Result<String, OperationFailure> {
    check(&batch._operation)?;
    reserve(batch, value.len())?;
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| OperationFailure::Resource)?;
    if output.capacity() > value.len() {
        batch._operation.budget.0.fail_closed();
        return Err(OperationFailure::Resource);
    }
    let mut offset = 0;
    while offset < value.len() {
        let end = value.floor_char_boundary(offset.saturating_add(4096).min(value.len()));
        check(&batch._operation)?;
        output.push_str(&value[offset..end]);
        offset = end;
        tokio::task::yield_now().await;
        check(&batch._operation)?;
    }
    Ok(output)
}
async fn optional(
    batch: &mut Batch,
    value: Option<Field<'_>>,
) -> Result<Option<String>, OperationFailure> {
    match value {
        Some(field) => Ok(Some(copy_field(batch, field).await?)),
        None => Ok(None),
    }
}
async fn media(
    batch: &mut Batch,
    value: Option<Media<'_>>,
) -> Result<Option<MediaInfo>, OperationFailure> {
    match value {
        None => Ok(None),
        Some(value) => Ok(Some(MediaInfo {
            media_type: copy_str(batch, "image").await?,
            url: optional(batch, value.url).await?,
            thumbnail: optional(batch, value.thumbnail).await?,
            source: optional(batch, value.source).await?,
            width: None,
            height: None,
            duration: None,
        })),
    }
}
pub(super) async fn project(
    owned: OwnedRows,
    layout: Layout,
    query: &str,
) -> Result<Batch, OperationFailure> {
    if layout.is_a10() {
        return Err(OperationFailure::Invariant(
            "A10 rows require typed projection",
        ));
    }
    let operation = owned.operation().clone();
    check(&operation)?;
    let rows = owned.rows().await?;
    let capacity = rows.count();
    let bytes = std::alloc::Layout::array::<SearchResult>(capacity)
        .map_err(|_| OperationFailure::Resource)?
        .size();
    // Same fixed 25-byte finite-score backing allowance as the reviewed shared
    // science batch. Constant constructor/transient runtime overhead remains in
    // the separately open native-allocation qualification.
    let bytes = capacity
        .checked_mul(25)
        .and_then(|n| n.checked_add(bytes))
        .ok_or(OperationFailure::Resource)?;
    let charge = operation.budget.0.reserve(bytes).map_err(resource)?;
    let mut results = Vec::new();
    results
        .try_reserve_exact(capacity)
        .map_err(|_| OperationFailure::Resource)?;
    if results.capacity() != capacity {
        operation.budget.0.fail_closed();
        return Err(OperationFailure::Resource);
    }
    let mut batch = Batch {
        results,
        _charge: charge,
        _operation: operation,
    };
    for row in rows.iter() {
        check(&batch._operation)?;
        let row =
            row.map_err(|_| OperationFailure::Invariant("HTML helper invalid row projection"))?;
        let url = copy_field(&mut batch, row.url).await?;
        let title = if row.fallback {
            copy_str(&mut batch, query).await?
        } else {
            copy_field(&mut batch, row.title).await?
        };
        let content = copy_field(&mut batch, row.content).await?;
        let img_src = optional(&mut batch, row.image).await?;
        let media = media(&mut batch, row.media).await?;
        let engine = copy_str(
            &mut batch,
            if layout == Layout::Google {
                "google"
            } else {
                "duckduckgo"
            },
        )
        .await?;
        let category = copy_str(
            &mut batch,
            if layout == Layout::DdgImage {
                "images"
            } else {
                "general"
            },
        )
        .await?;
        batch.results.push(SearchResult {
            url,
            title,
            content,
            engine,
            engines: Default::default(),
            score: FiniteF64::default(),
            position: row.position,
            category,
            published_date: None,
            thumbnail: None,
            img_src,
            tier: 1,
            media,
            payload: None,
            work_group: None,
        });
        tokio::task::yield_now().await;
        check(&batch._operation)?;
    }
    check(&batch._operation)?;
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::super::row_wire::{ExtendedRow, ImageMedia, Row, write_extended};
    use super::*;
    fn frame() -> Vec<u8> {
        let title = "é𐀀".repeat(3000);
        let row = ExtendedRow {
            base: Row {
                position: 7,
                query_title_fallback: false,
                url: "http://raw/?token=secret",
                title: &title,
                content: "\u{1c}body",
                img_src: Some("//raw/?key=secret"),
            },
            media: Some(ImageMedia {
                url: Some("//raw/"),
                thumbnail: Some(""),
                source: Some("http://raw/"),
            }),
        };
        let mut encoded = Vec::new();
        write_extended(Layout::DdgImage, &[row], &mut encoded).unwrap();
        encoded[8..].to_vec()
    }
    async fn operation(limit: usize) -> Arc<Operation> {
        Operation::start(
            limit,
            0,
            tokio::time::Instant::now() + std::time::Duration::from_secs(5),
        )
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn produced_batch_retains_fields_until_named_handoff_and_refusal_drops_partial_copy() {
        let bytes = frame();
        let op = operation(1024 * 1024).await;
        let ledger = op.budget.0.clone();
        let owned =
            super::super::rows_parent::test_frame(bytes.clone(), Layout::DdgImage, op).unwrap();
        let batch = project(owned, Layout::DdgImage, "").await.unwrap();
        assert!(ledger.snapshot().unwrap().used > 0);
        let peak = ledger.snapshot().unwrap().peak;
        let response = batch.into_adapter_response(FiniteF64::default());
        assert_eq!(ledger.snapshot().unwrap().used, 0);
        let row = &response.results[0];
        assert_eq!(row.url, "http://raw/?token=secret");
        assert_eq!(row.img_src.as_deref(), Some("//raw/?key=secret"));
        assert_eq!(row.title, "é𐀀".repeat(3000));
        assert_eq!(row.position, 7);
        assert_eq!(row.category, "images");
        let media = row.media.as_ref().unwrap();
        assert_eq!(media.url.as_deref(), Some("//raw/"));
        assert_eq!(media.thumbnail.as_deref(), Some(""));
        assert_eq!(media.source.as_deref(), Some("http://raw/"));
        drop(response);
        let op = operation(peak - 1).await;
        let ledger = op.budget.0.clone();
        let owned = super::super::rows_parent::test_frame(bytes, Layout::DdgImage, op).unwrap();
        assert!(matches!(
            project(owned, Layout::DdgImage, "").await,
            Err(OperationFailure::Resource)
        ));
        assert_eq!(ledger.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn cancellation_during_private_result_projection_releases_frame_and_batch() {
        let op = operation(1024 * 1024).await;
        let ledger = op.budget.0.clone();
        let owned = super::super::rows_parent::test_frame(frame(), Layout::DdgImage, op).unwrap();
        let before = ledger.snapshot().unwrap().used;
        let baseline = before + std::mem::size_of::<SearchResult>() + 25;
        let task = tokio::spawn(project(owned, Layout::DdgImage, ""));
        let reached = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while ledger.snapshot().unwrap().used <= baseline && !task.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .is_ok();
        let active = reached && !task.is_finished() && ledger.snapshot().unwrap().used > baseline;
        task.abort();
        let cancelled = task.await.is_err_and(|error| error.is_cancelled());
        assert!(
            active && cancelled,
            "observer did not interrupt active private row projection"
        );
        assert_eq!(ledger.snapshot().unwrap().used, 0);
    }
}
