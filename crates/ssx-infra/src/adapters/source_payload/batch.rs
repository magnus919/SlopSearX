//! Narrow science-result assembler. Source-row/scratch ownership is separate.
//! Requested backing accounting ends explicitly at into_adapter_response return;
//! G02 owns retained public response/clone/flight/cache lifetime qualification.
use super::{BudgetError, BudgetPayload, Projection};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchResult};
use ssx_owned_budget::Budget;
use std::{collections::BTreeSet, time::Instant};

// Test-only per-future causal phases; completely absent from production builds.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BatchPhase {
    Stored(usize),
    FieldCopied {
        retained_rows: usize,
        field: &'static str,
    },
}
#[cfg(test)]
tokio::task_local! { pub(crate) static BATCH_PHASE: std::sync::Arc<dyn Fn(BatchPhase)->bool+Send+Sync>; }
#[cfg(test)]
async fn observe_phase(phase: BatchPhase) {
    if BATCH_PHASE
        .try_with(|observer| observer(phase))
        .unwrap_or(false)
    {
        // A test can suspend this exact operation after an observed copy.
        std::future::pending::<()>().await;
    }
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeadlinePhase {
    Operation(Instant),
    Handoff(Option<(Instant, Instant)>),
}
#[cfg(test)]
tokio::task_local! { pub(crate) static DEADLINE_PHASE: std::sync::Arc<dyn Fn(DeadlinePhase)+Send+Sync>; }
#[cfg(test)]
pub(crate) fn observe_deadline_phase(phase: DeadlinePhase) {
    let _ = DEADLINE_PHASE.try_with(|observer| observer(phase));
}
pub struct Skeleton<'a> {
    pub url: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub engine: &'a str,
    pub category: &'a str,
    pub score: f64,
    pub position: i64,
    pub tier: i64,
    pub published_date: Option<&'a str>,
}
pub struct Outcome<'a> {
    pub status: EngineStatus,
    pub error_message: Option<&'a str>,
    pub latency_ms: f64,
    pub synthetic: bool,
}
/// Capacity is the caller's exact retained-source-row upper bound, checked and
/// reserved before allocation. Skipped rows leave conservative capacity charged
/// until handoff. This does not account earlier arbitrary String construction.
pub struct ProducedBatch<'a> {
    results: Vec<SearchResult>,
    state: Projection<'a>,
}
impl<'a> ProducedBatch<'a> {
    pub fn new(
        budget: &'a Budget,
        capacity: usize,
        deadline: Option<Instant>,
    ) -> Result<Self, BudgetError> {
        let bytes = std::alloc::Layout::array::<SearchResult>(capacity)
            .map_err(|_| BudgetError::Resource)?
            .size();
        let charge = budget
            .reserve(bytes.checked_add(25).ok_or(BudgetError::Resource)?)
            .map_err(|_| BudgetError::Resource)?;
        let mut results = Vec::new();
        results
            .try_reserve_exact(capacity)
            .map_err(|_| BudgetError::Resource)?;
        if results.capacity() != capacity {
            return Err(BudgetError::Resource);
        }
        Ok(Self {
            results,
            state: Projection {
                budget,
                charge,
                deadline,
            },
        })
    }
    /// Read-only access for inspection. Cloning outside accounting is forbidden
    /// in mandatory consumers; borrowed public types cannot prevent all clones.
    pub fn results(&self) -> &[SearchResult] {
        &self.results
    }
    pub fn charged_bytes(&self) -> usize {
        self.state.charge.bytes()
    }
    /// Fields are copied into charged storage while the caller retains their
    /// original owners: both copies must remain in the SAME operation budget.
    pub async fn push(
        &mut self,
        row: Skeleton<'_>,
        payload: Option<BudgetPayload>,
    ) -> Result<(), BudgetError> {
        self.push_with_thumbnail(row, payload, None).await
    }
    /// Add a source thumbnail through the same charged projection boundary.
    pub async fn push_with_thumbnail(
        &mut self,
        row: Skeleton<'_>,
        mut payload: Option<BudgetPayload>,
        thumbnail: Option<&str>,
    ) -> Result<(), BudgetError> {
        self.state.checkpoint().await?;
        if self.results.len() == self.results.capacity() {
            return Err(BudgetError::Resource);
        }
        if !row.score.is_finite() {
            return Err(BudgetError::Projection(super::ProjectionError::Nonfinite));
        }
        if let Some(payload) = payload.as_mut() {
            self.state
                .charge
                .merge_from(&mut payload._charge)
                .map_err(|_| BudgetError::Resource)?;
        }
        let url = self.state.scalar(row.url).await?;
        #[cfg(test)]
        observe_phase(BatchPhase::FieldCopied {
            retained_rows: self.results.len(),
            field: "url",
        })
        .await;
        let title = self.state.scalar(row.title).await?;
        #[cfg(test)]
        observe_phase(BatchPhase::FieldCopied {
            retained_rows: self.results.len(),
            field: "title",
        })
        .await;
        let content = self.state.scalar(row.content).await?;
        let engine = self.state.scalar(row.engine).await?;
        let category = self.state.scalar(row.category).await?;
        let published_date = match row.published_date {
            Some(value) => Some(self.state.scalar(value).await?),
            None => None,
        };
        let thumbnail = match thumbnail {
            Some(value) => Some(self.state.scalar(value).await?),
            None => None,
        };
        #[cfg(test)]
        if thumbnail.is_some() {
            observe_phase(BatchPhase::FieldCopied {
                retained_rows: self.results.len(),
                field: "thumbnail",
            })
            .await;
        }
        self.state.reserve(25)?;
        let score = FiniteF64::from_python_float(row.score).map_err(|_| BudgetError::Resource)?;
        let result = SearchResult {
            url,
            title,
            content,
            engine,
            engines: BTreeSet::new(),
            score,
            position: row.position,
            category,
            published_date,
            thumbnail,
            img_src: None,
            tier: row.tier,
            media: None,
            payload: payload.map(|payload| payload.value),
            work_group: None,
        };
        self.state.checkpoint().await?;
        self.results.push(result);
        #[cfg(test)]
        observe_phase(BatchPhase::Stored(self.results.len())).await;
        Ok(())
    }
    /// Named final ownership boundary. Moves existing storage without clone or
    /// reallocation; its public lifetime and clones are then owned by G02.
    /// No serialization or response-retention bound is claimed beyond return.
    pub async fn into_adapter_response(
        mut self,
        outcome: Outcome<'_>,
    ) -> Result<AdapterResponse, BudgetError> {
        self.state.checkpoint().await?;
        if !outcome.latency_ms.is_finite() {
            return Err(BudgetError::Projection(super::ProjectionError::Nonfinite));
        }
        let error_message = match outcome.error_message {
            Some(value) => Some(self.state.scalar(value).await?),
            None => None,
        };
        let latency_ms =
            FiniteF64::from_python_float(outcome.latency_ms).map_err(|_| BudgetError::Resource)?;
        if outcome.status != EngineStatus::Ok {
            self.results.clear();
        }
        let _handoff = self.state.checkpoint_sample().await?;
        #[cfg(test)]
        observe_deadline_phase(DeadlinePhase::Handoff(_handoff));
        let results = std::mem::take(&mut self.results);
        Ok(AdapterResponse {
            results,
            status: outcome.status,
            error_message,
            latency_ms,
            answers: Vec::new(),
            corrections: Vec::new(),
            infoboxes: Vec::new(),
            synthetic: outcome.synthetic,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::source_json::{Text, Value};
    // Poll the existing two cooperative yields separately. Expire the real
    // absolute deadline only after entering the terminal checkpoint; no corpus
    // replay is delayed or clock-modified by this focused control.
    #[tokio::test]
    async fn expired_terminal_handoff_refuses_rows_and_drains_without_success_stamp() {
        use std::{future::Future, task::Poll};
        let budget = Budget::new(8192).unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        let mut batch = ProducedBatch::new(&budget, 1, Some(deadline)).unwrap();
        batch.push(row("content"), None).await.unwrap();
        let stamps = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = stamps.clone();
        let observer: std::sync::Arc<dyn Fn(DeadlinePhase) + Send + Sync> =
            std::sync::Arc::new(move |phase| captured.lock().unwrap().push(phase));
        let mut future = Box::pin(DEADLINE_PHASE.scope(
            observer,
            batch.into_adapter_response(outcome(EngineStatus::Ok)),
        ));
        for phase in ["initial", "terminal"] {
            std::future::poll_fn(|cx| {
                assert!(
                    matches!(future.as_mut().poll(cx), Poll::Pending),
                    "missing {phase} handoff yield"
                );
                Poll::Ready(())
            })
            .await;
        }
        assert!(stamps.lock().unwrap().is_empty());
        tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
        assert!(matches!(future.await, Err(BudgetError::Deadline)));
        assert!(stamps.lock().unwrap().is_empty());
        assert_eq!(budget.snapshot().unwrap().used, 0);
        println!("A10_DEADLINE_COMPLETE expired-terminal-handoff");
    }
    fn row(content: &str) -> Skeleton<'_> {
        Skeleton {
            url: "https://example.test/1",
            title: "title",
            content,
            engine: "test",
            category: "general",
            score: 1.0,
            position: 1,
            tier: 1,
            published_date: Some("2024-02-29"),
        }
    }
    fn outcome(status: EngineStatus) -> Outcome<'static> {
        Outcome {
            status,
            error_message: None,
            latency_ms: 0.0,
            synthetic: false,
        }
    }
    #[tokio::test]
    async fn thumbnail_uses_same_budget_and_named_handoff_release() {
        let b = Budget::new(8192).unwrap();
        let mut batch = ProducedBatch::new(&b, 1, None).unwrap();
        let original =
            ssx_owned_budget::OwnedString::try_from_str(&b, "https://images.test/a.jpg").unwrap();
        let before = b.snapshot().unwrap().used;
        batch
            .push_with_thumbnail(row("content"), None, Some(original.as_str()))
            .await
            .unwrap();
        assert_eq!(
            batch.results()[0].thumbnail.as_deref(),
            Some(original.as_str())
        );
        assert!(b.snapshot().unwrap().used >= before + original.len());
        assert!(batch.results()[0].img_src.is_none());
        assert!(batch.results()[0].media.is_none());
        drop(original);
        let response = batch
            .into_adapter_response(outcome(EngineStatus::Ok))
            .await
            .unwrap();
        assert_eq!(
            response.results[0].thumbnail.as_deref(),
            Some("https://images.test/a.jpg")
        );
        assert_eq!(b.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn thumbnail_capacity_refusal_and_cancel_release_projection_storage() {
        let b = Budget::new(4096).unwrap();
        let mut batch = ProducedBatch::new(&b, 1, None).unwrap();
        let huge = "x".repeat(8192);
        assert!(matches!(
            batch.push_with_thumbnail(row("x"), None, Some(&huge)).await,
            Err(BudgetError::Resource)
        ));
        drop(batch);
        assert_eq!(b.snapshot().unwrap().used, 0);
        let b = Budget::new(4 * 1024 * 1024).unwrap();
        let mut batch = ProducedBatch::new(&b, 1, None).unwrap();
        let large = "x".repeat(100000);
        let reached = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed = reached.clone();
        let observer: std::sync::Arc<dyn Fn(BatchPhase) -> bool + Send + Sync> =
            std::sync::Arc::new(move |phase| {
                let stop = phase
                    == BatchPhase::FieldCopied {
                        retained_rows: 0,
                        field: "thumbnail",
                    };
                if stop {
                    observed.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                stop
            });
        let mut pending = Box::pin(BATCH_PHASE.scope(
            observer,
            batch.push_with_thumbnail(row("x"), None, Some(&large)),
        ));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !reached.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::select! {_= &mut pending=>panic!("thumbnail projection must suspend after actual copied field"),_=tokio::task::yield_now()=>{}}
            assert!(std::time::Instant::now() < deadline);
        }
        assert!(reached.load(std::sync::atomic::Ordering::SeqCst));
        drop(pending);
        drop(batch);
        assert_eq!(b.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn batch_owns_projection_capacity_overlap_and_named_handoff() {
        let budget = Budget::new(1024 * 1024).unwrap();
        let value = Value::String(Text::from_points(vec![0xd800]));
        let payload = super::super::project_field_budgeted_optional(&value, &budget, None)
            .await
            .unwrap();
        let field_charge = payload.charged_bytes();
        let mut batch = ProducedBatch::new(&budget, 2, None).unwrap();
        batch.push(row("content"), Some(payload)).await.unwrap();
        assert_eq!(budget.snapshot().unwrap().used, batch.charged_bytes());
        assert!(batch.charged_bytes() > field_charge);
        // A simultaneous backend reply buffer must reserve against the SAME
        // ledger while source results remain live; no serializer proof implied.
        let reply = ssx_owned_budget::OwnedVec::<u8>::with_capacity(&budget, 4096).unwrap();
        assert_eq!(
            budget.snapshot().unwrap().used,
            batch.charged_bytes() + 4096
        );
        drop(reply);
        let response = batch
            .into_adapter_response(outcome(EngineStatus::Ok))
            .await
            .unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(
            response.results[0].payload,
            Some(ssx_json::Value::String("\u{fffd}".into()))
        );
        assert_eq!(budget.snapshot().unwrap().used, 0); // Explicit public handoff.
    }
    #[tokio::test]
    async fn wrong_ledger_refusal_and_fixed_capacity_preserve_storage_ownership() {
        let a = Budget::new(1024 * 1024).unwrap();
        let b = Budget::new(1024 * 1024).unwrap();
        let value = Value::String(Text::from("payload"));
        let payload = super::super::project_field_budgeted_optional(&value, &b, None)
            .await
            .unwrap();
        let mut batch = ProducedBatch::new(&a, 1, None).unwrap();
        let before = a.snapshot().unwrap().used;
        assert!(matches!(
            batch.push(row("x"), Some(payload)).await,
            Err(BudgetError::Resource)
        ));
        assert_eq!(a.snapshot().unwrap().used, before);
        assert_eq!(b.snapshot().unwrap().used, 0);
        assert!(batch.results().is_empty());
        batch.push(row("x"), None).await.unwrap();
        assert!(matches!(
            batch.push(row("y"), None).await,
            Err(BudgetError::Resource)
        ));
        assert_eq!(batch.results().len(), 1);
        drop(batch);
        assert_eq!(a.snapshot().unwrap().used, 0);
        let tiny = Budget::new(1).unwrap();
        assert!(matches!(
            ProducedBatch::new(&tiny, 1, None),
            Err(BudgetError::Resource)
        ));
        assert_eq!(tiny.snapshot().unwrap().used, 0);
        assert!(matches!(
            ProducedBatch::new(&a, usize::MAX, None),
            Err(BudgetError::Resource)
        ));
        assert_eq!(a.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn cancelled_push_drops_partial_storage_before_batch_charge() {
        let budget = Budget::new(16 * 1024 * 1024).unwrap();
        let content = "x".repeat(4 * 1024 * 1024);
        let mut batch = ProducedBatch::new(&budget, 1, None).unwrap();
        let initial = batch.charged_bytes();
        {
            let job = batch.push(row(&content), None);
            tokio::pin!(job);
            tokio::select! {
                r=&mut job=>panic!("must observe allocation before completion: {}",r.is_ok()),
                _=async{loop{if budget.snapshot().unwrap().used>=initial+content.len(){break;}tokio::task::yield_now().await;}}=>{}
            }
        }
        assert!(batch.results().is_empty());
        assert!(batch.charged_bytes() >= initial + content.len());
        drop(batch);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn non_success_handoff_has_no_partial_rows_and_charges_error_copy() {
        let budget = Budget::new(1024 * 1024).unwrap();
        let mut batch = ProducedBatch::new(&budget, 1, None).unwrap();
        batch.push(row("x"), None).await.unwrap();
        let mut result = outcome(EngineStatus::Error);
        result.error_message = Some("source error");
        let response = batch.into_adapter_response(result).await.unwrap();
        assert!(response.results.is_empty());
        assert_eq!(response.error_message.as_deref(), Some("source error"));
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
