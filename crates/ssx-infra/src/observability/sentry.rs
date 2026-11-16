//! Optional, isolated, bounded delivery of allowlisted operational errors.
use super::logging::{LogRecord, SentryConfig};
use crate::http::{AdapterHttp, Bounds, Cancellation, Options, Request};
use reqwest::{Method, header};
use ssx_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

#[derive(Default)]
pub struct Delivery {
    pub accepted_response: AtomicU64,
    pub failed: AtomicU64,
    pub dropped: AtomicU64,
    pub sampled: AtomicU64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Report {
    Queued,
    Disabled,
    Sampled,
    FullOrClosed,
    Refused,
}
// Every accepted envelope owns its disposition, including receiver closure or
// cancellation before the worker's first poll. No queue-length estimate is used.
struct Envelope {
    bytes: Vec<u8>,
    delivery: Arc<Delivery>,
    pending: bool,
}
impl Drop for Envelope {
    fn drop(&mut self) {
        if self.pending {
            self.delivery.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}
struct Completion(watch::Sender<bool>);
impl Drop for Completion {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}
pub struct Exporter {
    intake: Mutex<Option<mpsc::Sender<Envelope>>>,
    worker: tokio::sync::Mutex<Option<JoinHandle<()>>>,
    stop: watch::Sender<bool>,
    finished: watch::Sender<bool>,
    http: Option<Arc<AdapterHttp>>,
    rate: f64,
    shutdown_timeout: Duration,
    pub delivery: Arc<Delivery>,
}
impl Exporter {
    /// None creates no HTTP owner, queue or worker. No environment is read here.
    pub fn new(
        config: Option<SentryConfig>,
        capacity: usize,
        operation_timeout: Duration,
        shutdown_timeout: Duration,
    ) -> Result<Option<Self>, &'static str> {
        let Some(config) = config else {
            return Ok(None);
        };
        if capacity == 0
            || capacity > 1024
            || operation_timeout.is_zero()
            || shutdown_timeout.is_zero()
        {
            return Err("invalid monitoring bounds");
        }
        tokio::runtime::Handle::try_current().map_err(|_| "monitoring runtime unavailable")?;
        // Sampling zero has no sender, queue, HTTP owner or worker.
        if config.sample_rate == 0. {
            let (stop, _) = watch::channel(false);
            let (finished, _) = watch::channel(true);
            return Ok(Some(Self {
                intake: Mutex::new(None),
                worker: tokio::sync::Mutex::new(None),
                stop,
                finished,
                http: None,
                rate: 0.,
                shutdown_timeout,
                delivery: Arc::new(Delivery::default()),
            }));
        }
        let authentication = header::HeaderValue::from_str(&config.authentication())
            .map_err(|_| "invalid monitoring credentials")?;
        let (tx, mut rx) = mpsc::channel::<Envelope>(capacity);
        let (stop, mut stopping) = watch::channel(false);
        let (finished, _) = watch::channel(false);
        let http = Arc::new(AdapterHttp::default());
        let owner = http.clone();
        let delivery = Arc::new(Delivery::default());
        let counts = delivery.clone();
        let complete = Completion(finished.clone());
        let worker = tokio::spawn(async move {
            let _completion = complete;
            loop {
                let body = tokio::select! {biased;_=stopping.changed()=>{rx.close();rx.recv().await},body=rx.recv()=>body};
                let Some(mut envelope) = body else { break };
                let body = std::mem::take(&mut envelope.bytes);

                let deadline = tokio::time::Instant::now() + operation_timeout;
                let options = Options {
                    follow_redirects: false,
                    bounds: Bounds {
                        wire_bytes: 4096,
                        decoded_bytes: 4096,
                        request_bytes: 8192,
                        requests: 1,
                        redirects: 0,
                    },
                    ..Options::default()
                };
                let mut headers = header::HeaderMap::new();
                headers.insert(
                    header::CONTENT_TYPE,
                    header::HeaderValue::from_static("application/x-sentry-envelope"),
                );
                headers.insert("x-sentry-auth", authentication.clone());
                let request = Request {
                    method: Method::POST,
                    url: config.endpoint.clone(),
                    headers,
                    body,
                };
                let result = match owner.session(options, deadline, Cancellation::default()) {
                    Ok(mut session) => session
                        .execute(request)
                        .await
                        .map(|r| (200..300).contains(&r.status)),
                    Err(error) => Err(error),
                };
                if matches!(result, Ok(true)) {
                    counts.accepted_response.fetch_add(1, Ordering::Relaxed);
                } else {
                    counts.failed.fetch_add(1, Ordering::Relaxed);
                }
                envelope.pending = false;
            }
        });
        Ok(Some(Self {
            intake: Mutex::new(Some(tx)),
            worker: tokio::sync::Mutex::new(Some(worker)),
            stop,
            finished,
            http: Some(http),
            rate: config.sample_rate,
            shutdown_timeout,
            delivery,
        }))
    }
    pub fn report(&self, record: &LogRecord) -> Report {
        if self.rate == 0. {
            self.delivery.sampled.fetch_add(1, Ordering::Relaxed);
            return Report::Sampled;
        }
        let mut random = [0u8; 16];
        if aws_lc_rs::rand::fill(&mut random).is_err() {
            self.delivery.dropped.fetch_add(1, Ordering::Relaxed);
            return Report::Refused;
        }
        if self.rate < 1.
            && (u64::from_ne_bytes(random[..8].try_into().unwrap()) as f64 / (u64::MAX as f64))
                >= self.rate
        {
            self.delivery.sampled.fetch_add(1, Ordering::Relaxed);
            return Report::Sampled;
        }
        let intake = self.intake.lock().unwrap();
        let Some(sender) = intake.as_ref() else {
            self.delivery.dropped.fetch_add(1, Ordering::Relaxed);
            return Report::FullOrClosed;
        };
        let Ok(permit) = sender.try_reserve() else {
            self.delivery.dropped.fetch_add(1, Ordering::Relaxed);
            return Report::FullOrClosed;
        };
        // Only closed event vocabulary and validated fixed-length correlations reach
        // this encoder. Reserve queue capacity before creating its bounded envelope.
        let id: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let mut event = record.sentry_event();
        event["event_id"] = json!(id);
        event["timestamp"] = json!(chrono::Utc::now().to_rfc3339());
        let payload = event.to_string();
        let envelope = format!(
            "{}\n{}\n{}\n",
            json!({"event_id":id}),
            json!({"type":"event","content_type":"application/json","length":payload.len()}),
            payload
        );
        if envelope.len() > 4096 {
            self.delivery.dropped.fetch_add(1, Ordering::Relaxed);
            return Report::Refused;
        }
        permit.send(Envelope {
            bytes: envelope.into_bytes(),
            delivery: self.delivery.clone(),
            pending: true,
        });
        Report::Queued
    }
    pub fn completed(&self) -> bool {
        *self.finished.borrow()
    }
    /// Explicit shutdown joins the exporter. A cancelled caller retains the handle
    /// in this owner, so another caller can finish the same drain.
    pub async fn shutdown(&self) {
        self.shutdown_until(tokio::time::Instant::now() + self.shutdown_timeout)
            .await;
    }
    pub(super) async fn shutdown_until(&self, deadline: tokio::time::Instant) {
        self.intake.lock().unwrap().take();
        self.stop.send_replace(true);
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
        }
        if let Some(http) = &self.http {
            http.shutdown_and_wait().await;
        }
    }
}
impl Drop for Exporter {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.get_mut().take() {
            worker.abort()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::logging::{Level, LogEvent};
    use super::*;
    use crate::observability::{Options as TelemetryOptions, Telemetry};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };
    async fn request(stream: &mut TcpStream) -> (String, Vec<u8>) {
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 512];
        loop {
            let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut chunk))
                .await
                .unwrap()
                .unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&chunk[..n]);
            assert!(bytes.len() <= 8192);
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
                let length = headers
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(str::parse::<usize>)
                    })
                    .unwrap()
                    .unwrap();
                if bytes.len() >= end + 4 + length {
                    assert_eq!(bytes.len(), end + 4 + length);
                    return (headers, bytes[end + 4..].to_vec());
                }
            }
        }
    }
    async fn observed(counter: &AtomicU64, n: u64) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while counter.load(Ordering::Relaxed) != n {
                tokio::time::sleep(Duration::from_millis(1)).await
            }
        })
        .await
        .unwrap();
    }
    fn record() -> LogRecord {
        LogRecord {
            event: LogEvent::InternalFailure,
            level: Level::Error,
            request_id: None,
            query_id: None,
            duration_ms: None,
        }
    }
    #[tokio::test]
    async fn disabled_and_sample_zero_create_no_sender_http_owner_or_worker() {
        assert!(
            Exporter::new(None, 1, Duration::from_secs(1), Duration::from_secs(1))
                .unwrap()
                .is_none()
        );
        let telemetry = Telemetry::new(TelemetryOptions::default(), None, vec![], vec![]).unwrap();
        assert!(telemetry.sentry.is_none());
        assert_eq!(
            telemetry.report_operational_error(LogEvent::InternalFailure),
            Report::Disabled
        );
        telemetry.shutdown().await;
        let config = SentryConfig::parse(Some("http://synthetic@127.0.0.1:9/12"), 0.).unwrap();
        let exporter = Exporter::new(config, 1, Duration::from_secs(1), Duration::from_secs(1))
            .unwrap()
            .unwrap();
        assert!(exporter.intake.lock().unwrap().is_none());
        assert!(exporter.worker.lock().await.is_none());
        assert!(exporter.http.is_none());
        assert_eq!(exporter.report(&record()), Report::Sampled);
        assert_eq!(exporter.delivery.sampled.load(Ordering::Relaxed), 1);
        exporter.shutdown().await;
    }
    #[tokio::test]
    async fn public_operational_error_posts_exact_envelope_auth_and_native_identity() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let r = request(&mut stream).await;
            stream
                .write_all(
                    b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
            r
        });
        let dsn = format!("http://synthetic:syntheticsecret@{address}/prefix/١٢");
        let config = SentryConfig::parse(Some(&dsn), 1.).unwrap();
        let telemetry =
            Telemetry::new_with_sentry(TelemetryOptions::default(), None, vec![], vec![], config)
                .unwrap();
        assert_eq!(
            telemetry.report_operational_error(LogEvent::StoreUnavailable),
            Report::Queued
        );
        observed(
            &telemetry
                .sentry
                .as_ref()
                .unwrap()
                .delivery
                .accepted_response,
            1,
        )
        .await;
        telemetry.shutdown().await;
        let (headers, body) = server.await.unwrap();
        assert!(headers.starts_with("POST /prefix/api/12/envelope/ HTTP/1.1\r\n"));
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("content-type: application/x-sentry-envelope")
        );
        assert!(headers.contains(concat!(
            "Sentry sentry_key=synthetic, sentry_version=7, sentry_client=slopsearx.rust/",
            env!("CARGO_PKG_VERSION"),
            ", sentry_secret=syntheticsecret"
        )));
        let text = String::from_utf8(body).unwrap();
        assert!(!text.contains("syntheticsecret"));
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);
        let envelope: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let item: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        let event: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(item["type"], "event");
        assert_eq!(item["content_type"], "application/json");
        assert_eq!(item["length"], lines[2].len());
        assert_eq!(envelope["event_id"], event["event_id"]);
        assert_eq!(event["event_id"].as_str().unwrap().len(), 32);
        assert_eq!(event["platform"], "rust");
        assert_eq!(event["message"], "store_unavailable");
        assert_eq!(event["level"], "error");
        assert!(event["tags"]["request_id"].is_null());
    }
    #[tokio::test]
    async fn redirect_is_failed_once_without_forwarding_authentication() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let destination = target.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            request(&mut stream).await;
            stream.write_all(format!("HTTP/1.1 302 Found\r\nLocation: http://{destination}/leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap()
        });
        let exporter = Exporter::new(
            SentryConfig::parse(Some(&format!("http://synthetic@{address}/12")), 1.).unwrap(),
            1,
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .unwrap()
        .unwrap();
        assert_eq!(exporter.report(&record()), Report::Queued);
        observed(&exporter.delivery.failed, 1).await;
        exporter.shutdown().await;
        server.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(25), target.accept())
                .await
                .is_err()
        );
        assert_eq!(
            exporter.delivery.accepted_response.load(Ordering::Relaxed),
            0
        );
    }
    #[tokio::test]
    async fn full_queue_and_hanging_post_shutdown_account_for_discarded_work_and_close_socket() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, started) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            request(&mut stream).await;
            tx.send(()).unwrap();
            let mut byte = [0];
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(3), stream.read(&mut byte))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            )
        });
        let exporter = Exporter::new(
            SentryConfig::parse(Some(&format!("http://synthetic@{address}/12")), 1.).unwrap(),
            1,
            Duration::from_secs(60),
            Duration::from_millis(50),
        )
        .unwrap()
        .unwrap();
        assert_eq!(exporter.report(&record()), Report::Queued);
        tokio::time::timeout(Duration::from_secs(3), started)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(exporter.report(&record()), Report::Queued);
        assert_eq!(exporter.report(&record()), Report::FullOrClosed);
        exporter.shutdown().await;
        server.await.unwrap();
        assert!(exporter.worker.lock().await.is_none());
        assert!(exporter.completed());
        assert_eq!(exporter.delivery.dropped.load(Ordering::Relaxed), 3);
        assert_eq!(exporter.delivery.failed.load(Ordering::Relaxed), 0);
    }
}

#[cfg(test)]
mod lifecycle_controls {
    use super::super::logging::{Level, LogEvent};
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    fn record() -> LogRecord {
        LogRecord {
            event: LogEvent::InternalFailure,
            level: Level::Error,
            request_id: None,
            query_id: None,
            duration_ms: None,
        }
    }
    async fn received(stream: &mut tokio::net::TcpStream) {
        let mut bytes = Vec::new();
        let mut buf = [0; 512];
        loop {
            let n = stream.read(&mut buf).await.unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
            assert!(bytes.len() <= 8192);
            if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                let h = String::from_utf8_lossy(&bytes[..end]);
                let length = h
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(str::parse::<usize>)
                    })
                    .unwrap()
                    .unwrap();
                if bytes.len() == end + 4 + length {
                    break;
                }
            }
        }
    }
    async fn failures(exporter: &Exporter, n: u64) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while exporter.delivery.failed.load(Ordering::Relaxed) != n {
                tokio::time::sleep(Duration::from_millis(1)).await
            }
        })
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn non_success_and_connection_refusal_are_nonfatal_without_retry() {
        for status in [429, 500] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                received(&mut stream).await;
                stream.write_all(format!("HTTP/1.1 {status} Synthetic\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
                assert!(
                    tokio::time::timeout(Duration::from_millis(25), listener.accept())
                        .await
                        .is_err()
                )
            });
            let exporter = Exporter::new(
                SentryConfig::parse(Some(&format!("http://synthetic@{address}/12")), 1.).unwrap(),
                1,
                Duration::from_secs(1),
                Duration::from_secs(1),
            )
            .unwrap()
            .unwrap();
            assert_eq!(exporter.report(&record()), Report::Queued);
            failures(&exporter, 1).await;
            exporter.shutdown().await;
            server.await.unwrap();
            assert_eq!(
                exporter.delivery.accepted_response.load(Ordering::Relaxed),
                0
            );
            assert_eq!(exporter.delivery.dropped.load(Ordering::Relaxed), 0);
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let exporter = Exporter::new(
            SentryConfig::parse(Some(&format!("http://synthetic@{address}/12")), 1.).unwrap(),
            1,
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .unwrap()
        .unwrap();
        assert_eq!(exporter.report(&record()), Report::Queued);
        failures(&exporter, 1).await;
        exporter.shutdown().await;
    }
    #[tokio::test]
    async fn cancelled_then_concurrent_shutdown_retains_handle_until_real_response_and_join() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (start_tx, started) = tokio::sync::oneshot::channel();
        let (release, release_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            received(&mut stream).await;
            start_tx.send(()).unwrap();
            release_rx.await.unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap()
        });
        let exporter = Arc::new(
            Exporter::new(
                SentryConfig::parse(Some(&format!("http://synthetic@{address}/12")), 1.).unwrap(),
                1,
                Duration::from_secs(60),
                Duration::from_secs(2),
            )
            .unwrap()
            .unwrap(),
        );
        assert_eq!(exporter.report(&record()), Report::Queued);
        tokio::time::timeout(Duration::from_secs(3), started)
            .await
            .unwrap()
            .unwrap();
        let owner = exporter.clone();
        let caller = tokio::spawn(async move { owner.shutdown().await });
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if exporter.worker.try_lock().is_err() {
                    break;
                }
                tokio::task::yield_now().await
            }
        })
        .await
        .unwrap();
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        assert!(exporter.worker.lock().await.is_some());
        assert!(!exporter.completed());
        release.send(()).unwrap();
        let a = exporter.clone();
        let b = exporter.clone();
        tokio::time::timeout(Duration::from_secs(3), async {
            tokio::join!(a.shutdown(), b.shutdown());
        })
        .await
        .unwrap();
        server.await.unwrap();
        assert!(exporter.worker.lock().await.is_none());
        assert!(exporter.completed());
        assert_eq!(
            exporter.delivery.accepted_response.load(Ordering::Relaxed),
            1
        );
        assert_eq!(exporter.delivery.failed.load(Ordering::Relaxed), 0);
        assert_eq!(exporter.delivery.dropped.load(Ordering::Relaxed), 0);
        assert_eq!(exporter.http.as_ref().unwrap().retained_pools(), 0);
    }
    #[tokio::test]
    async fn completion_guard_signals_even_when_its_task_panics() {
        let (finished, mut receiver) = watch::channel(false);
        let task = tokio::spawn(async move {
            let _guard = Completion(finished);
            panic!("synthetic completion-guard panic")
        });
        assert!(task.await.unwrap_err().is_panic());
        tokio::time::timeout(Duration::from_secs(1), receiver.changed())
            .await
            .unwrap()
            .unwrap();
        assert!(*receiver.borrow());
    }
}

#[cfg(test)]
mod never_polled_control {
    use super::super::logging::{Level, LogEvent};
    use super::*;
    #[tokio::test]
    async fn never_polled_worker_abort_drops_each_envelope_and_signals_completion() {
        // This current-thread future does not yield before aborting its new worker.
        let exporter = Exporter::new(
            SentryConfig::parse(Some("http://synthetic@127.0.0.1:9/12"), 1.).unwrap(),
            1,
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            exporter.report(&LogRecord {
                event: LogEvent::InternalFailure,
                level: Level::Error,
                request_id: None,
                query_id: None,
                duration_ms: None
            }),
            Report::Queued
        );
        let handle = exporter.worker.lock().await.take().unwrap();
        handle.abort();
        assert!(handle.await.unwrap_err().is_cancelled());
        assert!(exporter.completed());
        assert_eq!(exporter.delivery.dropped.load(Ordering::Relaxed), 1);
        assert_eq!(exporter.delivery.failed.load(Ordering::Relaxed), 0);
        assert_eq!(exporter.http.as_ref().unwrap().retained_pools(), 0);
        exporter.shutdown().await;
    }
}
