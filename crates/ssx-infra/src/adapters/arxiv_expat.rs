//! Candidate Expat-backed Atom extraction; no registry/factory activation.
use ssx_domain::{AdapterResponse, EngineStatus, SearchResult};
use ssx_json::{Map, Value};
use ssx_source_xml::Entry;
use std::time::Instant;
fn ws(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExtractFailure {
    Deadline,
    Resource,
}
const PROJECTION: usize = 128 * 1024 * 1024;
const DATA_KEYS: [&str; 8] = [
    "publication_id",
    "arxiv_id",
    "doi",
    "authors",
    "journal",
    "comments",
    "revision_date",
    "abstract",
];
fn map_storage_bound(n: usize) -> Option<usize> {
    // Locked IndexMap stores a Vec<Bucket{hash,key,value}> plus hash-table
    // indices/control bytes. First reservation Vec growth <=2*max(n,4);
    // hashbrown buckets <=2*(n+1), with16control-group bytes. Only n<=8 here.
    let entries =
        n.max(4)
            .checked_mul(2)?
            .checked_mul(std::mem::size_of::<(usize, String, Value)>())?;
    let indices = n
        .checked_add(1)?
        .checked_mul(2)?
        .checked_mul(std::mem::size_of::<usize>() + 1)?
        .checked_add(16)?;
    entries.checked_add(indices)
}
fn projection_fixed() -> Option<usize> {
    let fixed = std::mem::size_of::<SearchResult>()
        .checked_mul(4)?
        .checked_add(map_storage_bound(8)?)?
        .checked_add(map_storage_bound(5)?)?
        .checked_add(map_storage_bound(4)?)?
        .checked_add(16usize.checked_mul(std::mem::size_of::<Value>())?)?
        // Exact known map keys/adapter-fields names, scalar values and URL/
        // snippet bytes. The snippet is at most301Unicode scalars.
        .checked_add(DATA_KEYS.iter().map(|k| k.len() * 2).sum::<usize>())?
        .checked_add(
            [
                "domain",
                "type",
                "schema_version",
                "data",
                "provenance",
                "engine",
                "adapter_fields",
                "normalized_fields",
                "inferred_fields",
                "science",
                "publication",
                "arxiv",
                "https://arxiv.org/abs/",
            ]
            .iter()
            .map(|s| s.len())
            .sum::<usize>(),
        )?
        .checked_add(301 * 4)?;
    Some(fixed)
}
async fn projection_cost(e: &Entry, deadline: Instant) -> Result<usize, ExtractFailure> {
    let fixed = projection_fixed().ok_or(ExtractFailure::Resource)?;
    let mut strings = e
        .fields
        .iter()
        .flatten()
        .map(String::capacity)
        .try_fold(0usize, |n, v| n.checked_add(v))
        .ok_or(ExtractFailure::Resource)?;
    for (index, author) in e.authors.iter().enumerate() {
        if index % 128 == 0 {
            extraction_checkpoint(deadline).await?;
        }
        strings = strings
            .checked_add(author.capacity())
            .ok_or(ExtractFailure::Resource)?;
    }
    fixed
        .checked_add(strings.checked_mul(6).ok_or(ExtractFailure::Resource)?)
        .and_then(|n| {
            n.checked_add(
                e.authors
                    .len()
                    .checked_mul(2 * std::mem::size_of::<Value>())?,
            )
        })
        .ok_or(ExtractFailure::Resource)
}
fn object(n: usize) -> Result<Map<String, Value>, ExtractFailure> {
    let mut map = Map::new();
    map.try_reserve_exact(n)
        .map_err(|_| ExtractFailure::Resource)?;
    Ok(map)
}
async fn extraction_checkpoint(deadline: Instant) -> Result<(), ExtractFailure> {
    tokio::task::yield_now().await;
    if Instant::now() >= deadline {
        Err(ExtractFailure::Deadline)
    } else {
        Ok(())
    }
}
async fn trim_text(s: &str, deadline: Instant) -> Result<&str, ExtractFailure> {
    let mut start = 0;
    for (i, (offset, c)) in s.char_indices().enumerate() {
        if i % 4096 == 0 {
            extraction_checkpoint(deadline).await?
        }
        if !ws(c) {
            start = offset;
            break;
        }
        start = offset + c.len_utf8();
    }
    if start == s.len() {
        return Ok(&s[start..]);
    }
    let mut end = s.len();
    for (i, (offset, c)) in s[start..].char_indices().rev().enumerate() {
        if i % 4096 == 0 {
            extraction_checkpoint(deadline).await?
        }
        if !ws(c) {
            break;
        }
        end = start + offset;
    }
    Ok(&s[start..end])
}
async fn append_text(
    out: &mut String,
    text: &str,
    deadline: Instant,
) -> Result<(), ExtractFailure> {
    let mut start = 0;
    while start < text.len() {
        let mut end = start.saturating_add(16384).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1
        }
        out.push_str(&text[start..end]);
        start = end;
        extraction_checkpoint(deadline).await?;
    }
    Ok(())
}
async fn copy_text(text: &str, deadline: Instant) -> Result<String, ExtractFailure> {
    let mut out = String::new();
    out.try_reserve_exact(text.len())
        .map_err(|_| ExtractFailure::Resource)?;
    append_text(&mut out, text, deadline).await?;
    Ok(out)
}
async fn last_segment(text: &str, deadline: Instant) -> Result<&str, ExtractFailure> {
    for (i, (offset, c)) in text.char_indices().rev().enumerate() {
        if i % 4096 == 0 {
            extraction_checkpoint(deadline).await?
        }
        if c == '/' {
            return Ok(&text[offset + 1..]);
        }
    }
    Ok(text)
}
/// Known-shape projection uses checked pre-reservation accounting; no generic
/// json! interpolation round-trip or global ssx_json size-policy change.
pub async fn extract(
    entries: Vec<Entry>,
    deadline: Instant,
) -> Result<Vec<SearchResult>, ExtractFailure> {
    let mut results = Vec::new();
    let mut projected = 0usize;
    for (index, mut e) in entries.into_iter().enumerate() {
        if index % 128 == 0 {
            extraction_checkpoint(deadline).await?
        }
        projected = projected
            .checked_add(projection_cost(&e, deadline).await?)
            .filter(|n| *n <= PROJECTION)
            .ok_or(ExtractFailure::Resource)?;
        results
            .try_reserve(1)
            .map_err(|_| ExtractFailure::Resource)?;
        let title = e.fields[0].take().unwrap_or_default();
        let title = copy_text(trim_text(&title, deadline).await?, deadline).await?;
        let summary = e.fields[1].take().unwrap_or_default();
        let mut abstract_text = String::new();
        abstract_text
            .try_reserve_exact(summary.len())
            .map_err(|_| ExtractFailure::Resource)?;
        let mut pending = false;
        for (i, c) in summary.chars().enumerate() {
            if i % 4096 == 0 {
                extraction_checkpoint(deadline).await?
            }
            if ws(c) {
                pending = !abstract_text.is_empty()
            } else {
                if pending {
                    abstract_text.push(' ');
                    pending = false
                }
                abstract_text.push(c)
            }
        }
        let id = e.fields[2].take().unwrap_or_default();
        let id = trim_text(&id, deadline).await?;
        let id = last_segment(id, deadline).await?;
        let mut data = object(8)?;
        if !id.is_empty() {
            data.insert(
                "publication_id".into(),
                Value::String(copy_text(id, deadline).await?),
            );
            data.insert(
                "arxiv_id".into(),
                Value::String(copy_text(id, deadline).await?),
            );
        }
        if let Some(v) = e.fields[4].take() {
            data.insert("doi".into(), Value::String(v));
        }
        let mut authors = Vec::new();
        authors
            .try_reserve_exact(e.authors.len())
            .map_err(|_| ExtractFailure::Resource)?;
        for (i, a) in e.authors.into_iter().enumerate() {
            if i % 128 == 0 {
                extraction_checkpoint(deadline).await?
            }
            let a = trim_text(&a, deadline).await?;
            if !a.is_empty() {
                authors.push(Value::String(copy_text(a, deadline).await?))
            }
        }
        if !authors.is_empty() {
            data.insert("authors".into(), Value::Array(authors));
        }
        for (source, target) in [(5, "journal"), (6, "comments"), (7, "revision_date")] {
            if let Some(v) = e.fields[source].take() {
                data.insert(target.into(), Value::String(v));
            }
        }
        if !abstract_text.is_empty() {
            data.insert(
                "abstract".into(),
                Value::String(copy_text(&abstract_text, deadline).await?),
            );
        }
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(8)
            .map_err(|_| ExtractFailure::Resource)?;
        fields.extend(data.keys().cloned().map(Value::String));
        let mut provenance = object(4)?;
        provenance.insert("engine".into(), Value::String("arxiv".into()));
        provenance.insert("adapter_fields".into(), Value::Array(fields));
        provenance.insert("normalized_fields".into(), Value::Array(Vec::new()));
        provenance.insert("inferred_fields".into(), Value::Array(Vec::new()));
        let mut payload = object(5)?;
        payload.insert("domain".into(), Value::String("science".into()));
        payload.insert("type".into(), Value::String("publication".into()));
        payload.insert("schema_version".into(), ssx_json::json!(1));
        payload.insert("data".into(), Value::Object(data));
        payload.insert("provenance".into(), Value::Object(provenance));
        let mut content = String::new();
        content
            .try_reserve_exact(301 * 4)
            .map_err(|_| ExtractFailure::Resource)?;
        content.extend(abstract_text.chars().take(300));
        if abstract_text.chars().nth(300).is_some() {
            content.push('…')
        }
        let published_date = match e.fields[3].take() {
            Some(date) if !date.is_empty() => {
                Some(copy_text(trim_text(&date, deadline).await?, deadline).await?)
            }
            _ => None,
        };
        let mut url = String::new();
        url.try_reserve_exact(
            "https://arxiv.org/abs/"
                .len()
                .checked_add(id.len())
                .ok_or(ExtractFailure::Resource)?,
        )
        .map_err(|_| ExtractFailure::Resource)?;
        url.push_str("https://arxiv.org/abs/");
        append_text(&mut url, id, deadline).await?;
        results.push(SearchResult {
            url,
            title,
            content,
            engine: "arxiv".into(),
            position: results.len() as i64 + 1,
            published_date,
            payload: Some(Value::Object(payload)),
            ..Default::default()
        });
    }
    extraction_checkpoint(deadline).await?;
    Ok(results)
}
pub fn syntax() -> AdapterResponse {
    AdapterResponse::default()
}
pub fn resource() -> AdapterResponse {
    AdapterResponse {
        status: EngineStatus::Error,
        error_message: Some("arXiv native XML resource limit exceeded".into()),
        ..Default::default()
    }
}

use super::arxiv_expat_wire::{self as wire, MEMORY, OUTPUT, Reply};
use crate::process_pool::{ProcessError, ProcessPool};
use std::{
    io::{Read, Write},
    num::NonZeroUsize,
    path::PathBuf,
};
const MAX_INPUT: usize = 128 * 1024 * 1024;
const MAX_POINTS: usize = 32 * 1024 * 1024;
#[derive(Clone)]
pub struct AtomPool {
    workers: ProcessPool,
    admission: std::sync::Arc<tokio::sync::Semaphore>,
    drained: std::sync::Arc<tokio::sync::Notify>,
    closing: tokio::sync::watch::Sender<bool>,
}
struct Operation {
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
    drained: std::sync::Arc<tokio::sync::Notify>,
}
impl Drop for Operation {
    fn drop(&mut self) {
        drop(self.permit.take());
        self.drained.notify_waiters();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawAdmissionError {
    Deadline,
    Saturated,
    Shutdown,
}
/// Must be acquired before the adapter's HTTP call. The same operation stays
/// alive through response framing, supervised disposal and parent projection.
pub struct RawOperation {
    pool: AtomPool,
    operation: std::sync::Arc<Operation>,
    budget: ssx_owned_budget::Budget,
    deadline: tokio::time::Instant,
    #[cfg(test)]
    clock: Option<crate::http::DeadlineClock>,
}
struct RawLease {
    _operation: std::sync::Arc<Operation>,
    _child: ssx_owned_budget::Charge,
    _output: std::sync::Arc<ssx_owned_budget::Charge>,
}
// The acquire future may own a semaphore reservation before its next poll.
// Keep both that future and a delivered permit inside one cancellation guard.
// Notification MUST follow both drops so shutdown can observe restored capacity.
struct AdmissionAttempt<F> {
    waiting: Option<F>,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
    drained: std::sync::Arc<tokio::sync::Notify>,
}
impl<F> Drop for AdmissionAttempt<F> {
    fn drop(&mut self) {
        drop(self.waiting.take());
        drop(self.permit.take());
        self.drained.notify_waiters();
    }
}

impl RawOperation {
    fn expired(&self) -> bool {
        #[cfg(test)]
        if let Some(clock) = &self.clock {
            return clock.expired(self.deadline.into_std());
        }
        tokio::time::Instant::now() >= self.deadline
    }
    async fn wait_deadline(&self) {
        #[cfg(test)]
        if let Some(clock) = &self.clock {
            clock.sleep_until(self.deadline.into_std()).await;
            return;
        }
        tokio::time::sleep_until(self.deadline).await;
    }
    /// Scope pre-helper work (including HTTP) to this operation's existing
    /// absolute deadline and pool lifetime. No additional admission is acquired.
    /// Already-closing pools are checked before the supplied future is polled.
    pub async fn run_until_cancelled<F: std::future::Future>(
        &self,
        future: F,
    ) -> Result<F::Output, RawAdmissionError> {
        let mut closing = self.pool.closing.subscribe();
        if *closing.borrow() {
            return Err(RawAdmissionError::Shutdown);
        }
        if self.expired() {
            return Err(RawAdmissionError::Deadline);
        }
        tokio::select! {
            biased;
            // Sender closure also means this owner can no longer authorize work.
            _ = closing.changed() => Err(RawAdmissionError::Shutdown),
            _ = self.wait_deadline() => Err(RawAdmissionError::Deadline),
            result = future => Ok(result),
        }
    }
    pub fn budget(&self) -> &ssx_owned_budget::Budget {
        &self.budget
    }
    pub async fn parse<'a, I>(self, headers: I, body: &[u8]) -> AdapterResponse
    where
        I: Iterator<Item = super::scholarly_frame::Header<'a>> + Clone + Send,
    {
        #[cfg(test)]
        assert!(
            !self.clock.as_ref().is_some_and(|clock| clock.is_manual()),
            "pending Manual owner cannot enter a parser worker"
        );
        let mut closing = self.pool.closing.subscribe();
        tokio::select! {
            result=self.parse_admitted(headers,body)=>result,
            _=tokio::time::sleep_until(self.deadline)=>timeout(),
            _=async{if *closing.borrow(){return}let _=closing.changed().await;}=>parser_error("arXiv native XML parser shutdown"),
        }
    }
    async fn source_error(&self, message: &str) -> AdapterResponse {
        use super::source_payload::{Outcome, ProducedBatch};
        let result = async {
            let batch = ProducedBatch::new(&self.budget, 0, Some(self.deadline.into_std()))?;
            batch
                .into_adapter_response(Outcome {
                    status: EngineStatus::Error,
                    error_message: Some(message),
                    latency_ms: 0.0,
                    synthetic: false,
                })
                .await
        }
        .await;
        match result {
            Ok(response) => response,
            Err(super::source_payload::BudgetError::Deadline) => timeout(),
            Err(_) => resource(),
        }
    }
    async fn parse_admitted<'a, I>(&self, headers: I, body: &[u8]) -> AdapterResponse
    where
        I: Iterator<Item = super::scholarly_frame::Header<'a>> + Clone + Send,
    {
        use super::source_payload::{BudgetError, Outcome};
        use super::{scholarly_frame as frame, scholarly_reply as reply};
        if tokio::time::Instant::now() >= self.deadline {
            return timeout();
        }
        let child = match self.budget.reserve(frame::CANDIDATE_CHILD_BYTES) {
            Ok(value) => value,
            Err(_) => return resource(),
        };
        let output_charge = match self.budget.reserve(reply::MAX_BYTES) {
            Ok(value) => std::sync::Arc::new(value),
            Err(_) => return resource(),
        };
        let payload = match frame::encode(
            frame::Operation::Atom,
            headers,
            body,
            &self.budget,
            self.deadline.into_std(),
        )
        .await
        {
            Ok(frame) => frame,
            Err(frame::Error::Deadline) => return timeout(),
            Err(_) => return resource(),
        };
        let lease = RawLease {
            _operation: self.operation.clone(),
            _child: child,
            _output: output_charge.clone(),
        };
        let output = match self
            .pool
            .workers
            .exchange_with_lease(self.deadline, Vec::new(), payload, reply::MAX_BYTES, lease)
            .await
        {
            Ok(output) => output,
            Err(ProcessError::Deadline) => return timeout(),
            Err(ProcessError::OutputBound | ProcessError::Allocation) => return resource(),
            Err(ProcessError::Shutdown | ProcessError::AdmissionClosed) => {
                return parser_error("arXiv native XML parser shutdown");
            }
            Err(_) => {
                return self
                    .source_error("arXiv native scholarly helper failed")
                    .await;
            }
        };
        let view = match reply::view(&output) {
            Ok(view) => view,
            Err(_) => {
                return self
                    .source_error("arXiv native scholarly helper invalid response")
                    .await;
            }
        };
        let bytes = match view {
            reply::View::Atom(bytes) => bytes,
            reply::View::Resource(_) => return resource(),
            reply::View::Deadline(_) => return timeout(),
            reply::View::Source { message, .. } => return self.source_error(message).await,
            reply::View::Defect(_) => {
                return self
                    .source_error("arXiv native scholarly helper defect")
                    .await;
            }
        };
        // Parent wire/string/entry allocations share the existing quota formula;
        // reserve its full upper bound BEFORE decoding, while reply remains live.
        let entries_charge = match self.budget.reserve(MEMORY) {
            Ok(charge) => charge,
            Err(_) => return resource(),
        };
        let decoded = wire::decode(bytes, self.deadline).await;
        drop(output);
        drop(output_charge);
        let result = match decoded {
            Err(wire::Error::Deadline) => return timeout(),
            Ok(Reply::Syntax) => return syntax(),
            Ok(Reply::Resource) => return resource(),
            Ok(Reply::Entries(entries)) => {
                let batch = super::arxiv_projection::project(
                    &entries,
                    &self.budget,
                    self.deadline.into_std(),
                )
                .await;
                drop(entries);
                drop(entries_charge);
                match batch {
                    Ok(batch) => {
                        batch
                            .into_adapter_response(Outcome {
                                status: EngineStatus::Ok,
                                error_message: None,
                                latency_ms: 0.0,
                                synthetic: false,
                            })
                            .await
                    }
                    Err(error) => Err(error),
                }
            }
            _ => {
                return self
                    .source_error("arXiv native scholarly helper invalid Atom response")
                    .await;
            }
        };
        match result {
            Ok(response) => response,
            Err(BudgetError::Deadline) => timeout(),
            Err(_) => resource(),
        }
    }
}

impl AtomPool {
    /// Acquire before HTTP so admission and cancellation ownership span the
    /// entire actual adapter operation, not just subprocess execution.
    pub fn begin_raw(
        &self,
        budget: ssx_owned_budget::Budget,
        deadline: tokio::time::Instant,
    ) -> Result<RawOperation, RawAdmissionError> {
        if tokio::time::Instant::now() >= deadline {
            return Err(RawAdmissionError::Deadline);
        }
        self.begin_raw_admitted(budget, deadline)
    }
    fn begin_raw_admitted(
        &self,
        budget: ssx_owned_budget::Budget,
        deadline: tokio::time::Instant,
    ) -> Result<RawOperation, RawAdmissionError> {
        let permit = self.admission.clone().try_acquire_owned().map_err(|_| {
            if self.admission.is_closed() {
                RawAdmissionError::Shutdown
            } else {
                RawAdmissionError::Saturated
            }
        })?;
        Ok(RawOperation {
            pool: self.clone(),
            operation: std::sync::Arc::new(Operation {
                permit: Some(permit),
                drained: self.drained.clone(),
            }),
            budget,
            deadline,
            #[cfg(test)]
            clock: None,
        })
    }
    /// Wait for the same single operation before HTTP. Queueing consumes the
    /// already selected network deadline, never a renewed timeout; parsing keeps
    /// the original caller operation deadline. Dropping this future removes its
    /// semaphore waiter without creating a worker or retaining a permit.
    pub(crate) async fn begin_raw_for_http(
        &self,
        budget: ssx_owned_budget::Budget,
        deadline: tokio::time::Instant,
        network_deadline: tokio::time::Instant,
        http: &crate::http::AdapterHttp,
    ) -> Result<RawOperation, RawAdmissionError> {
        let clock = http.deadline_clock();
        #[cfg(test)]
        assert!(
            !clock.is_manual() || clock.is_pending_only(),
            "scholarly Manual owner is pending-only"
        );
        let wait_deadline = deadline.min(network_deadline).into_std();
        let mut closing = self.closing.subscribe();
        if *closing.borrow() || self.admission.is_closed() {
            return Err(RawAdmissionError::Shutdown);
        }
        if clock.expired(wait_deadline) {
            return Err(RawAdmissionError::Deadline);
        }
        let mut attempt = AdmissionAttempt {
            waiting: Some(Box::pin(self.admission.clone().acquire_owned())),
            permit: None,
            drained: self.drained.clone(),
        };
        attempt.permit = Some(tokio::select! {
            biased;
            _ = closing.changed() => return Err(RawAdmissionError::Shutdown),
            _ = clock.sleep_until(wait_deadline) => return Err(RawAdmissionError::Deadline),
            permit = attempt.waiting.as_mut().expect("admission acquire retained").as_mut() =>
                permit.map_err(|_| RawAdmissionError::Shutdown)?,
        });
        // Closure or expiry can become ready together with the permit. Recheck
        // before exposing operation ownership or polling any HTTP/source work.
        if *closing.borrow() || self.admission.is_closed() {
            return Err(RawAdmissionError::Shutdown);
        }
        if clock.expired(wait_deadline) {
            return Err(RawAdmissionError::Deadline);
        }
        Ok(RawOperation {
            pool: self.clone(),
            operation: std::sync::Arc::new(Operation {
                permit: attempt.permit.take(),
                drained: self.drained.clone(),
            }),
            budget,
            deadline,
            #[cfg(test)]
            clock: clock.observed().then_some(clock),
        })
    }
    pub fn new(helper: PathBuf, capacity: NonZeroUsize) -> Result<Self, ProcessError> {
        // Initial reviewed operational policy is exactly one operation. Raising
        // concurrency requires a matching aggregate application/container budget.
        if capacity.get() != 1 {
            return Err(ProcessError::InvalidCapacity);
        }
        Ok(Self {
            workers: ProcessPool::new(helper, capacity)?,
            admission: std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
            drained: std::sync::Arc::new(tokio::sync::Notify::new()),
            closing: tokio::sync::watch::channel(false).0,
        })
    }
    pub fn available_slots(&self) -> usize {
        self.workers.available_slots()
    }
    pub fn available_operations(&self) -> usize {
        self.admission.available_permits()
    }
    /// Explicitly await before runtime teardown; drains preflight/projection too.
    pub async fn shutdown(&self) -> Result<(), ProcessError> {
        self.admission.close();
        self.closing.send_replace(true);
        self.workers.shutdown().await?;
        loop {
            let notified = self.drained.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.admission.available_permits() == 1 {
                return Ok(());
            }
            notified.await;
        }
    }
    pub async fn parse(&self, text: &str, deadline: tokio::time::Instant) -> AdapterResponse {
        if tokio::time::Instant::now() >= deadline {
            return timeout();
        }
        let permit = match self.admission.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                return parser_error(if self.admission.is_closed() {
                    "arXiv native XML parser shutdown"
                } else {
                    "arXiv native XML parser admission saturated"
                });
            }
        };
        let operation = std::sync::Arc::new(Operation {
            permit: Some(permit),
            drained: self.drained.clone(),
        });
        let mut closing = self.closing.subscribe();
        tokio::select! {
            result=self.parse_admitted(text,deadline,operation.clone())=>result,
            _=tokio::time::sleep_until(deadline)=>timeout(),
            _=async{if *closing.borrow(){return}let _=closing.changed().await;}=>parser_error("arXiv native XML parser shutdown"),
        }
    }
    async fn parse_admitted(
        &self,
        text: &str,
        deadline: tokio::time::Instant,
        operation: std::sync::Arc<Operation>,
    ) -> AdapterResponse {
        if tokio::time::Instant::now() >= deadline {
            return timeout();
        }
        if text.len() > MAX_INPUT {
            return resource();
        }
        for (i, _) in text.chars().enumerate() {
            if i >= MAX_POINTS {
                return resource();
            }
            if i % 4096 == 0 {
                tokio::task::yield_now().await;
                if tokio::time::Instant::now() >= deadline {
                    return timeout();
                }
            }
        }
        let mut payload = Vec::new();
        if payload.try_reserve_exact(text.len()).is_err() {
            return resource();
        }
        for chunk in text.as_bytes().chunks(65536) {
            payload.extend_from_slice(chunk);
            tokio::task::yield_now().await;
            if tokio::time::Instant::now() >= deadline {
                return timeout();
            }
        }
        let header = (text.len() as u64).to_be_bytes().to_vec();
        let output = match self
            .workers
            .exchange_with_lease(deadline, header, payload, OUTPUT, operation)
            .await
        {
            Ok(out) => out,
            Err(ProcessError::Deadline) => return timeout(),
            Err(e) => {
                return AdapterResponse {
                    status: EngineStatus::Error,
                    error_message: Some(format!("arXiv native XML helper: {e}")),
                    ..Default::default()
                };
            }
        };
        let decoded = wire::decode(&output, deadline).await;
        drop(output);
        match decoded {
            Err(wire::Error::Deadline) => timeout(),
            Ok(Reply::Syntax) => syntax(),
            Ok(Reply::Resource) => resource(),
            Ok(Reply::Entries(entries)) => match extract(entries, deadline.into_std()).await {
                Ok(results) => AdapterResponse {
                    results,
                    ..Default::default()
                },
                Err(ExtractFailure::Deadline) => timeout(),
                Err(ExtractFailure::Resource) => resource(),
            },
            _ => AdapterResponse {
                status: EngineStatus::Error,
                error_message: Some("arXiv native XML helper invalid response".into()),
                ..Default::default()
            },
        }
    }
}
fn parser_error(message: &str) -> AdapterResponse {
    AdapterResponse {
        status: EngineStatus::Error,
        error_message: Some(message.into()),
        ..Default::default()
    }
}
fn timeout() -> AdapterResponse {
    AdapterResponse {
        status: EngineStatus::Timeout,
        ..Default::default()
    }
}
/// Single-thread, post-exec helper entry. Parent owns deadline/kill/reap.
/// Never called from the application server: C allocations stay in this child.
pub fn helper_main() -> Result<(), String> {
    ssx_source_xml::prepare_worker().map_err(|_| "XML helper resource setup failed")?;
    let mut input = std::io::stdin().lock();
    let mut prefix = [0; 4];
    input
        .read_exact(&mut prefix)
        .map_err(|error| error.to_string())?;
    let mut input = std::io::Cursor::new(prefix).chain(input);
    if &prefix == b"SSXS" {
        let mut output = std::io::stdout().lock();
        return super::scholarly_helper::worker(
            &mut input,
            &mut output,
            super::scholarly_frame::CANDIDATE_CHILD_BYTES,
        )
        .map(|_| ())
        .map_err(|error| error.to_string());
    }
    let mut header = [0; 8];
    input.read_exact(&mut header).map_err(|e| e.to_string())?;
    let length = usize::try_from(u64::from_be_bytes(header)).map_err(|_| "XML input overflow")?;
    if length > MAX_INPUT {
        return Err("XML input bound".into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| "XML input allocation")?;
    bytes.resize(length, 0);
    input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    let mut trailing = [0];
    if input.read(&mut trailing).map_err(|e| e.to_string())? != 0 {
        return Err("XML input trailing data".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "XML input UTF8")?;
    if text.chars().count() > MAX_POINTS {
        return Err("XML codepoint bound".into());
    }
    let reply = match ssx_source_xml::parse(text, MEMORY) {
        Ok(parsed) => Reply::Entries(parsed.entries),
        Err(ssx_source_xml::Error::Syntax) => Reply::Syntax,
        Err(ssx_source_xml::Error::NativeMemory | ssx_source_xml::Error::OutputMemory) => {
            Reply::Resource
        }
        Err(_) => Reply::Defect,
    };
    drop(bytes);
    let output = wire::encode(&reply)
        .or_else(|_| wire::encode(&Reply::Resource))
        .map_err(|_| "XML output allocation")?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&(output.len() as u64).to_be_bytes())
        .map_err(|e| e.to_string())?;
    stdout.write_all(&output).map_err(|e| e.to_string())?;
    stdout.flush().map_err(|e| e.to_string())
}

#[cfg(test)]
mod allocation_tests {
    use super::*;
    // A fast runner may finish a valid copy before a one-millisecond timer.
    // Compete in the same poll instead: removing the projection's repeated
    // checkpoints must let it finish before the cancellation probe can settle.
    async fn drop_after_cooperative_yields<T>(
        future: impl std::future::Future<Output = Result<T, ExtractFailure>>,
        context: &str,
    ) {
        let mut future = Box::pin(future);
        tokio::select! {
            biased;
            _ = future.as_mut() => panic!("{context} completed before cooperative cancellation"),
            _ = async {
                for _ in 0..32 {
                    tokio::task::yield_now().await;
                }
            } => {}
        }
        // Cancel the actual pending future; its caller retains any borrowed input.
        drop(future);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn large_projection_scans_and_copies_yield_to_cancellation() {
        for field in [0, 2, 3] {
            let mut e = Entry::default();
            e.fields[field] = Some(format!("{}X", " ".repeat(8 * 1024 * 1024)));
            drop_after_cooperative_yields(
                extract(vec![e], Instant::now() + std::time::Duration::from_secs(30)),
                &format!("large field {field}"),
            )
            .await;
        }
        let mut e = Entry::default();
        e.authors.push("X".repeat(8 * 1024 * 1024));
        drop_after_cooperative_yields(
            extract(vec![e], Instant::now() + std::time::Duration::from_secs(30)),
            "large author copy",
        )
        .await;
    }
    #[tokio::test(flavor = "current_thread")]
    async fn many_empty_author_slots_yield_during_projection_cost_preflight() {
        let e = Entry {
            authors: vec![String::new(); 1_000_000],
            ..Entry::default()
        };
        drop_after_cooperative_yields(
            projection_cost(&e, Instant::now() + std::time::Duration::from_secs(30)),
            "author-cost preflight",
        )
        .await;
    }
    #[tokio::test]
    async fn closed_schema_struct_and_map_reservations_fit_projection_formula() {
        assert!(
            std::mem::size_of::<Entry>() >= 80,
            "binary per-entry headers must fit retained representation"
        );
        assert!(std::mem::size_of::<String>() >= 8, "author header budget");
        for n in [4, 5, 8] {
            let m = object(n).unwrap();
            assert!(m.capacity() <= 2 * n.max(4));
            assert!(
                map_storage_bound(n).unwrap()
                    >= m.capacity() * std::mem::size_of::<(usize, String, Value)>()
            );
        }
        let e = Entry::default();
        assert!(
            projection_cost(&e, Instant::now() + std::time::Duration::from_secs(30))
                .await
                .unwrap()
                > 4 * std::mem::size_of::<SearchResult>()
        );
        assert!(map_storage_bound(usize::MAX).is_none());
    }
}
