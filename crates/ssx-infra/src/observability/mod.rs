//! Bounded, native operational telemetry. Transport owners drain after search shutdown.
mod event_metrics;
pub mod metrics;
pub mod sentry;
use chrono::{DateTime, Utc};
use ssx_application::{Audit, Metrics, Supervisor, TaskEvent, TaskState};
use ssx_domain::{AdapterResponse, EngineStatus, SearchRequest, SearchResponse};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

/// Audit redacts query/client fields by default. Raw retention is an explicit opt-in; logs/metrics never contain them.
#[derive(Clone, Copy)]
pub enum QueryPrivacy {
    Redacted,
    RetainRaw,
}
pub struct Options {
    pub monitoring: bool,
    pub audit: bool,
    pub privacy: QueryPrivacy,
    pub queue_capacity: usize,
    pub operation_timeout: Duration,
    pub shutdown_timeout: Duration,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            monitoring: true,
            audit: true,
            privacy: QueryPrivacy::Redacted,
            queue_capacity: 256,
            operation_timeout: Duration::from_secs(1),
            shutdown_timeout: Duration::from_secs(2),
        }
    }
}
struct AuditEntry {
    at: DateTime<Utc>,
    fields: Vec<(&'static str, String)>,
}
struct EngineEntry {
    at: DateTime<Utc>,
    engine: String,
    count: usize,
    latency: f64,
    score: i64,
    status: EngineStatus,
}
enum Event {
    Audit(AuditEntry),
    Engine(EngineEntry),
}
#[derive(Default)]
pub struct Delivery {
    pub dropped: AtomicU64,
    pub written: AtomicU64,
    pub failed: AtomicU64,
}
struct Completion(watch::Sender<bool>);
impl Drop for Completion {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}
pub struct Telemetry {
    pub metrics: metrics::Metrics,
    options: Options,
    persistence_enabled: bool,
    sentry: Option<sentry::Exporter>,
    tx: Mutex<Option<mpsc::Sender<Event>>>,
    stop: watch::Sender<bool>,
    worker: tokio::sync::Mutex<Option<JoinHandle<()>>>,
    pub delivery: Arc<Delivery>,
    finished: watch::Sender<bool>,
    spans: Mutex<std::collections::BTreeMap<&'static str, u64>>,
}
impl Telemetry {
    pub fn new(
        options: Options,
        valkey_url: Option<&str>,
        engines: Vec<String>,
        categories: Vec<String>,
    ) -> Result<Arc<Self>, &'static str> {
        Self::new_with_sentry(options, valkey_url, engines, categories, None)
    }
    /// Explicit optional monitoring composition; no implicit endpoint or environment.
    pub fn new_with_sentry(
        options: Options,
        valkey_url: Option<&str>,
        engines: Vec<String>,
        categories: Vec<String>,
        config: Option<logging::SentryConfig>,
    ) -> Result<Arc<Self>, &'static str> {
        if options.queue_capacity == 0
            || options.queue_capacity > 65536
            || options.operation_timeout.is_zero()
            || options.shutdown_timeout.is_zero()
        {
            return Err("invalid telemetry bounds");
        }
        let metrics = metrics::Metrics::new(options.monitoring, engines, categories)?;
        let client = valkey_url
            .map(redis::Client::open)
            .transpose()
            .map_err(|_| "invalid telemetry store")?;
        let persistence_enabled = client.is_some();
        let (tx, mut rx) = mpsc::channel(options.queue_capacity);
        let (stop, mut stopping) = watch::channel(false);
        let (finished, _) = watch::channel(false);
        let completed = finished.clone();
        tokio::runtime::Handle::try_current().map_err(|_| "telemetry runtime unavailable")?;
        let delivery = Arc::new(Delivery::default());
        let counters = delivery.clone();
        let timeout = options.operation_timeout;
        let sentry = sentry::Exporter::new(
            config,
            options.queue_capacity,
            options.operation_timeout,
            options.shutdown_timeout,
        )?;
        let worker = tokio::spawn(async move {
            let _completion = Completion(completed);
            let mut conn = None;
            loop {
                let event = tokio::select! {biased;_=stopping.changed()=>{rx.close();rx.recv().await},event=rx.recv()=>event};
                let Some(event) = event else { break };
                let Some(client) = &client else {
                    counters.failed.fetch_add(1, Ordering::Relaxed);
                    continue;
                };
                let result = tokio::time::timeout(timeout, async {
                    if conn.is_none() {
                        conn = Some(client.get_multiplexed_async_connection().await?);
                    }
                    let mut pipe = redis::pipe();
                    pipe.atomic();
                    match event {
                        Event::Audit(e) => {
                            let key = format!(
                                "query_audit:{}",
                                e.at.with_timezone(&chrono::Local).format("%Y-%m-%d")
                            );
                            pipe.cmd("XADD")
                                .arg(&key)
                                .arg("MAXLEN")
                                .arg("~")
                                .arg(10000)
                                .arg("*");
                            for (k, v) in e.fields {
                                pipe.arg(k).arg(v);
                            }
                            pipe.ignore().cmd("EXPIRE").arg(key).arg(7776000).ignore();
                        }
                        Event::Engine(e) => {
                            let key = format!(
                                "engine_stats:{}:{}",
                                e.engine,
                                e.at.with_timezone(&chrono::Local).format("%Y-%m-%d")
                            );
                            for (field, n) in [
                                ("queries", 1),
                                ("results_returned", e.count as i64),
                                (
                                    "errors",
                                    i64::from(matches!(
                                        e.status,
                                        EngineStatus::Error | EngineStatus::Timeout
                                    )),
                                ),
                                (
                                    "rate_limited",
                                    i64::from(e.status == EngineStatus::RateLimited),
                                ),
                                ("total_latency_ms", e.latency as i64),
                                ("total_score", e.score),
                            ] {
                                pipe.cmd("HINCRBY").arg(&key).arg(field).arg(n).ignore();
                            }
                            pipe.cmd("EXPIRE").arg(key).arg(7776000).ignore();
                        }
                    }
                    pipe.query_async::<()>(conn.as_mut().unwrap()).await
                })
                .await;
                if matches!(result, Ok(Ok(()))) {
                    counters.written.fetch_add(1, Ordering::Relaxed);
                } else {
                    conn = None;
                    counters.failed.fetch_add(1, Ordering::Relaxed);
                } // Ambiguous writes are never retried.
            }
        });
        Ok(Arc::new(Self {
            metrics,
            options,
            persistence_enabled,
            sentry,
            tx: Mutex::new(Some(tx)),
            stop,
            worker: tokio::sync::Mutex::new(Some(worker)),
            delivery,
            finished,
            spans: Mutex::new(Default::default()),
        }))
    }
    fn send(&self, event: Event) {
        let tx = self.tx.lock().unwrap();
        if tx.as_ref().is_none_or(|tx| tx.try_send(event).is_err()) {
            self.delivery.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
    /// Stop intake, drain within the configured budget, abort and join a stuck exporter.
    pub async fn shutdown(&self) {
        let deadline = tokio::time::Instant::now() + self.options.shutdown_timeout;
        self.tx.lock().unwrap().take();
        self.stop.send_replace(true);
        // Keep the JoinHandle in the supervisor across every cancellation point.
        // Aborting this caller releases the async guard, never detaches the exporter.
        let mut slot = self.worker.lock().await;
        if let Some(worker) = slot.as_mut() {
            if tokio::time::timeout_at(deadline, &mut *worker)
                .await
                .is_err()
            {
                worker.abort();
                let _ = worker.await;
            }
            slot.take();
            self.finished.send_replace(true);
        }
        if let Some(exporter) = &self.sentry {
            exporter.shutdown_until(deadline).await;
        }
    }
    /// Emit a closed-vocabulary operational error through the actual optional sender.
    /// Raw query, exception and credential text are never accepted by this API.
    pub fn report_operational_error(&self, event: logging::LogEvent) -> sentry::Report {
        self.sentry
            .as_ref()
            .map_or(sentry::Report::Disabled, |exporter| {
                exporter.report(&logging::LogRecord {
                    event,
                    level: logging::Level::Error,
                    request_id: None,
                    query_id: None,
                    duration_ms: None,
                })
            })
    }
    pub fn active_spans(&self) -> u64 {
        self.spans.lock().unwrap().values().sum()
    }
    pub fn span(self: &Arc<Self>, kind: SpanKind) -> Span {
        let name = match kind {
            SpanKind::Request => "request",
            SpanKind::Job => "job",
        };
        self.observed(TaskEvent {
            task: name,
            state: TaskState::Started,
        });
        Span {
            owner: self.clone(),
            name,
            start: Instant::now(),
            completed: false,
        }
    }
}
impl Drop for Telemetry {
    fn drop(&mut self) {
        if let Some(task) = self.worker.get_mut().take() {
            task.abort();
        }
    }
}
pub enum SpanKind {
    Request,
    Job,
}
pub struct Span {
    owner: Arc<Telemetry>,
    name: &'static str,
    start: Instant,
    completed: bool,
}
impl Span {
    pub fn finish(mut self) {
        self.completed = true;
    }
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        self.owner.observed(TaskEvent {
            task: self.name,
            state: if self.completed {
                TaskState::Completed
            } else {
                TaskState::Cancelled
            },
        });
    }
}
impl Metrics for Telemetry {
    fn workflow_accepted(&self, workflow: &'static str, mode: &'static str) {
        let _ = self.metrics.workflow_accepted(workflow, mode);
    }
    fn workflow_rejected(&self, workflow: &'static str, reason: &'static str) {
        let _ = self.metrics.update(
            "slopsearx_workflow_rejections_total",
            &[("workflow", workflow), ("reason", reason)],
            1.,
        );
    }
    fn workflow_terminal(&self, workflow: &'static str, outcome: &'static str) {
        let _ = self.metrics.workflow_terminal(workflow, outcome, None);
    }
    fn cache_lookup(&self, observation: ssx_application::CacheObservation) {
        let kind = match observation {
            ssx_application::CacheObservation::Miss => "miss",
            ssx_application::CacheObservation::Hit => "hit",
            ssx_application::CacheObservation::Negative => "negative",
        };
        let _ = self
            .metrics
            .update("slopsearx_cache_hit_total", &[("type", kind)], 1.);
    }
    fn increment(&self, name: &'static str) {
        self.metrics.internal_event(name);
    }
    fn engine_outcome(
        &self,
        engine: &str,
        response: &AdapterResponse,
        origin: ssx_application::health::OutcomeOrigin,
    ) {
        if !self.metrics.accepts_engine(engine) {
            return;
        }
        self.metrics.engine_outcome(engine, response, origin);
        if self.persistence_enabled
            && self.options.monitoring
            && origin != ssx_application::health::OutcomeOrigin::CallerCancelled
        {
            self.send(Event::Engine(EngineEntry {
                at: Utc::now(),
                engine: engine.into(),
                count: response.results.len(),
                score: if response.results.is_empty() {
                    0
                } else {
                    (response.results.iter().map(|r| r.score.get()).sum::<f64>()
                        / response.results.len() as f64
                        * 1000.0) as i64
                },
                latency: response.latency_ms.get(),
                status: response.status.clone(),
            }));
        }
    }
}
enum AuditLatency {
    Fractional(Duration),
    Integer(i64),
}
impl Audit for Telemetry {
    fn record(&self, _: &'static str) {}
    fn query(&self, request: &SearchRequest, response: &SearchResponse, elapsed: Duration) {
        self.audit_query(request, response, AuditLatency::Fractional(elapsed));
    }
    fn query_completed(&self, request: &SearchRequest, response: &SearchResponse, elapsed_ms: i64) {
        self.audit_query(request, response, AuditLatency::Integer(elapsed_ms));
    }
}
impl Telemetry {
    fn audit_query(
        &self,
        request: &SearchRequest,
        response: &SearchResponse,
        elapsed: AuditLatency,
    ) {
        if !self.persistence_enabled || !self.options.audit || response.cached {
            return;
        }
        if request.query.len() > 65536
            || request
                .client_identifier
                .as_ref()
                .is_some_and(|x| x.len() > 1024)
        {
            self.delivery.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        if response.engine_outcomes.len() > 256
            || response
                .engine_outcomes
                .iter()
                .any(|o| o.engine.len() > 128)
        {
            self.delivery.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        // The public audit port may receive caller-constructed outcome counts.
        // Refuse an unrepresentable aggregate before allocating an event. Source
        // Python integers are unbounded; this finite native limit is explicit.
        let Some(total_results) = response
            .engine_outcomes
            .iter()
            .try_fold(0_i64, |total, outcome| {
                total.checked_add(outcome.result_count.max(0))
            })
        else {
            self.delivery.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let at = Utc::now();
        let raw = matches!(self.options.privacy, QueryPrivacy::RetainRaw);
        let fields = vec![
            (
                "query",
                if raw {
                    request.query.clone()
                } else {
                    "[redacted]".into()
                },
            ),
            (
                "client_ip",
                if raw {
                    request
                        .client_identifier
                        .clone()
                        .unwrap_or("unknown".into())
                } else {
                    "[redacted]".into()
                },
            ),
            (
                "timestamp",
                at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            ),
            (
                "engines",
                response
                    .engine_outcomes
                    .iter()
                    .map(|o| o.engine.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            (
                "engines_ok",
                response
                    .engine_outcomes
                    .iter()
                    .filter(|o| o.status == "ok")
                    .count()
                    .to_string(),
            ),
            (
                "engines_error",
                response
                    .engine_outcomes
                    .iter()
                    .filter(|o| matches!(o.status.as_str(), "error" | "blocked" | "rate_limited"))
                    .count()
                    .to_string(),
            ),
            (
                "engines_timeout",
                response
                    .engine_outcomes
                    .iter()
                    .filter(|o| o.status == "timeout")
                    .count()
                    .to_string(),
            ),
            ("total_results", total_results.to_string()),
            (
                "latency_ms",
                match elapsed {
                    AuditLatency::Fractional(duration) => {
                        format!("{:.1}", duration.as_secs_f64() * 1000.0)
                    }
                    AuditLatency::Integer(milliseconds) => milliseconds.to_string(),
                },
            ),
        ];
        self.send(Event::Audit(AuditEntry { at, fields }));
    }
}
impl Supervisor for Telemetry {
    fn observed(&self, event: TaskEvent) {
        if ![
            "request",
            "job",
            "search_flight",
            "search_engine",
            "search_suggestion",
        ]
        .contains(&event.task)
        {
            return;
        }
        let mut spans = self.spans.lock().unwrap();
        let active = spans.entry(event.task).or_default();
        if event.state == TaskState::Started {
            *active = active.saturating_add(1);
        } else {
            *active = active.saturating_sub(1);
        }
    }
}

pub mod logging;
#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    #[tokio::test]
    async fn cancelled_shutdown_caller_retains_exporter_and_next_shutdown_joins_socket() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (accepted, waiting) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            accepted.send(()).unwrap();
            let mut bytes = [0; 4096];
            loop {
                match socket.read(&mut bytes).await {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });
        let telemetry = Telemetry::new(
            Options {
                audit: true,
                operation_timeout: Duration::from_secs(30),
                shutdown_timeout: Duration::from_millis(50),
                ..Default::default()
            },
            Some(&format!("redis://{address}")),
            vec![],
            vec![],
        )
        .unwrap();
        telemetry.query(
            &SearchRequest::default(),
            &SearchResponse::default(),
            Duration::ZERO,
        );
        tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .unwrap()
            .unwrap();
        let first = {
            let telemetry = telemetry.clone();
            tokio::spawn(async move { telemetry.shutdown().await })
        };
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if telemetry.worker.try_lock().is_err() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        assert!(telemetry.worker.lock().await.as_ref().is_some());
        tokio::time::timeout(Duration::from_secs(1), telemetry.shutdown())
            .await
            .unwrap();
        assert!(telemetry.worker.lock().await.is_none());
        assert!(*telemetry.finished.borrow());
        tokio::time::timeout(Duration::from_secs(1), peer)
            .await
            .unwrap()
            .unwrap();
    }
    #[tokio::test]
    async fn exporter_panic_still_signals_completion() {
        let (tx, rx) = watch::channel(false);
        let task = tokio::spawn(async move {
            let _guard = Completion(tx);
            panic!("intentional isolated exporter panic")
        });
        assert!(task.await.unwrap_err().is_panic());
        assert!(*rx.borrow());
    }
}
