//! Legacy HTTP+SSE shim, separate from rmcp's streamable-HTTP SSE responses.
use crate::Registry;
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, FromRequest, RawQuery, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::stream;
use rmcp::ServiceExt;
use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader},
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
#[derive(Clone)]
struct Session {
    input: mpsc::Sender<AdmittedMessage>,
    cancel: CancellationToken,
    owner: Option<ssx_domain::TenantId>,
}
// A permit follows the body into the queue and through pump consumption.
struct AdmittedMessage {
    body: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
}
const MESSAGE_CAPACITY: usize = 32;
const OUTPUT_CAPACITY: usize = 64 * 1024 * 1024;
#[cfg(test)]
const OUTPUT_FRAME_LIMIT: usize = 16 * 1024 * 1024;
struct ChargedLine {
    text: String,
    _permits: Vec<tokio::sync::OwnedSemaphorePermit>,
}
struct OutputEvent {
    bytes: Vec<u8>,
    _permit: Option<tokio::sync::OwnedSemaphorePermit>,
}
// State lives outside next_line's future, preserving partial bytes when select
// chooses incoming input. BufReader's uncharged scratch is fixed at 8 KiB.
struct OutputReader<R> {
    reader: BufReader<R>,
    bytes: Vec<u8>,
    permits: Vec<tokio::sync::OwnedSemaphorePermit>,
    budget: Arc<tokio::sync::Semaphore>,
    frame_limit: crate::output_limits::OutputLimits,
}
impl<R: AsyncRead + Unpin> OutputReader<R> {
    fn new(reader: R, budget: Arc<tokio::sync::Semaphore>) -> Self {
        Self {
            reader: BufReader::with_capacity(8192, reader),
            bytes: Vec::new(),
            permits: Vec::new(),
            budget,
            frame_limit: crate::output_limits::OutputLimits::LEGACY,
        }
    }
    fn with_limit(
        reader: R,
        budget: Arc<tokio::sync::Semaphore>,
        frame_limit: crate::output_limits::OutputLimits,
    ) -> Self {
        Self {
            frame_limit,
            ..Self::new(reader, budget)
        }
    }
    async fn next_line(&mut self) -> crate::Result<Option<ChargedLine>> {
        loop {
            let chunk = self.reader.fill_buf().await?;
            if chunk.is_empty() {
                if self.bytes.is_empty() {
                    return Ok(None);
                }
                return Err("incomplete legacy output frame".into());
            }
            let end = chunk.iter().position(|b| *b == b'\n').map(|n| n + 1);
            let count = end.unwrap_or(chunk.len());
            if self.bytes.len() + count > self.frame_limit.bytes() {
                return Err(format!(
                    "legacy output frame limit: {} bytes",
                    self.frame_limit.bytes()
                )
                .into());
            }
            let permit = self
                .budget
                .clone()
                .try_acquire_many_owned(count as u32)
                .map_err(|_| "legacy aggregate output saturation")?;
            self.bytes.extend_from_slice(&chunk[..count]);
            self.permits.push(permit);
            self.reader.consume(count);
            if end.is_some() {
                let mut bytes = std::mem::take(&mut self.bytes);
                bytes.pop();
                if bytes.last() == Some(&b'\r') {
                    bytes.pop();
                }
                return Ok(Some(ChargedLine {
                    text: String::from_utf8(bytes)?,
                    _permits: std::mem::take(&mut self.permits),
                }));
            }
        }
    }
}
impl AsRef<[u8]> for OutputEvent {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}
fn event_bytes(
    kind: &str,
    text: &str,
    budget: &Arc<tokio::sync::Semaphore>,
) -> crate::Result<OutputEvent> {
    event_bytes_with_limit(
        kind,
        text,
        budget,
        crate::output_limits::OutputLimits::LEGACY,
    )
}
fn event_bytes_with_limit(
    kind: &str,
    text: &str,
    budget: &Arc<tokio::sync::Semaphore>,
    limit: crate::output_limits::OutputLimits,
) -> crate::Result<OutputEvent> {
    if text.len() > limit.bytes() {
        return Err(format!("legacy output frame limit: {} bytes", limit.bytes()).into());
    }
    let count = text.len()
        + text.bytes().filter(|b| matches!(b, b'\n' | b'\r')).count() * 6
        + kind.len()
        + 16;
    let permit = budget
        .clone()
        .try_acquire_many_owned(count.try_into()?)
        .map_err(|_| "legacy aggregate output saturation")?;
    let mut bytes = Vec::with_capacity(count);
    bytes.extend_from_slice(b"event: ");
    bytes.extend_from_slice(kind.as_bytes());
    bytes.extend_from_slice(b"\ndata: ");
    for byte in text.bytes() {
        bytes.push(byte);
        if matches!(byte, b'\n' | b'\r') {
            bytes.extend_from_slice(b"data: ");
        }
    }
    bytes.extend_from_slice(b"\n\n");
    debug_assert_eq!(bytes.len(), count);
    Ok(OutputEvent {
        bytes,
        _permit: Some(permit),
    })
}
fn request_id(text: &str) -> crate::Result<Option<String>> {
    #[derive(serde::Deserialize)]
    struct Id {
        id: Option<Box<serde_json::value::RawValue>>,
    }
    let id: Id = serde_json::from_str(text)?;
    id.id
        .map(|id| {
            serde_json::from_str::<serde_json::Value>(if id.get() == "-0" { "0" } else { id.get() })
                .and_then(|id| serde_json::to_string(&id))
        })
        .transpose()
        .map_err(Into::into)
}
fn output_event(text: &str, budget: &Arc<tokio::sync::Semaphore>) -> crate::Result<OutputEvent> {
    event_bytes("message", text, budget)
}
type NativeHandlerFactory<H> = Arc<
    dyn Fn(Arc<Mutex<crate::opaque::WireState>>, Option<crate::oauth::AuthenticatedClient>) -> H
        + Send
        + Sync,
>;
#[derive(Clone)]
pub struct Legacy<H: rmcp::ServerHandler + Clone + 'static = Registry> {
    factory: NativeHandlerFactory<H>,
    catalogs: Arc<crate::opaque::Catalogs>,
    output_limits: crate::output_limits::OutputLimits,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    pub message_path: String,
    pub ttl: Duration,
    cancel: CancellationToken,
    admission: Arc<tokio::sync::Semaphore>,
    messages: Arc<tokio::sync::Semaphore>,
    output: Arc<tokio::sync::Semaphore>,
}
impl Legacy<Registry> {
    pub fn new(
        registry: Registry,
        message_path: &str,
        ttl: Duration,
        cancel: CancellationToken,
    ) -> crate::Result<Self> {
        let catalogs = registry.opaque.clone();
        Self::with_factory(
            Arc::new(move |_, _| registry.clone()),
            catalogs,
            message_path,
            ttl,
            cancel,
        )
    }
}
impl<H: rmcp::ServerHandler + Clone + 'static> Legacy<H> {
    pub fn with_factory(
        factory: NativeHandlerFactory<H>,
        catalogs: Arc<crate::opaque::Catalogs>,
        message_path: &str,
        ttl: Duration,
        cancel: CancellationToken,
    ) -> crate::Result<Self> {
        if !message_path.starts_with('/')
            || !message_path.ends_with('/')
            || message_path.contains('?')
            || message_path.contains('#')
        {
            return Err("legacy message prefix must be an absolute path ending /".into());
        }
        Ok(Self {
            factory,
            catalogs,
            output_limits: crate::output_limits::OutputLimits::LEGACY,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            message_path: message_path.into(),
            ttl,
            cancel,
            admission: Arc::new(tokio::sync::Semaphore::new(128)),
            messages: Arc::new(tokio::sync::Semaphore::new(MESSAGE_CAPACITY)),
            output: Arc::new(tokio::sync::Semaphore::new(OUTPUT_CAPACITY)),
        })
    }
    pub fn with_output_limits(mut self, limits: crate::output_limits::OutputLimits) -> Self {
        self.output_limits = limits;
        self
    }
    pub fn available_permits(&self) -> usize {
        self.admission.available_permits()
    }
    pub fn available_message_permits(&self) -> usize {
        self.messages.available_permits()
    }
    pub fn available_output_bytes(&self) -> usize {
        self.output.available_permits()
    }
    pub fn active_sessions(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }
    pub fn router(self, sse_path: &str) -> crate::Result<Router> {
        if !sse_path.starts_with('/') || sse_path.ends_with('/') {
            return Err("invalid SSE path".into());
        }
        let prefix = self.message_path.trim_end_matches('/');
        Ok(Router::new()
            .route(sse_path, get(connect::<H>))
            .route(prefix, post(message::<H>))
            .route(&format!("{prefix}/"), post(message::<H>))
            .route(&format!("{prefix}/{{*suffix}}"), post(message::<H>))
            .layer(DefaultBodyLimit::max(4 * 1024 * 1024))
            .with_state(self))
    }
}
struct StreamState<H: rmcp::ServerHandler + Clone + 'static> {
    rx: mpsc::Receiver<OutputEvent>,
    session: String,
    state: Legacy<H>,
    _admission: Arc<tokio::sync::OwnedSemaphorePermit>,
}
impl<H: rmcp::ServerHandler + Clone + 'static> Drop for StreamState<H> {
    fn drop(&mut self) {
        if let Some(session) = self.state.sessions.lock().unwrap().remove(&self.session) {
            session.cancel.cancel();
        }
    }
}
async fn connect<H: rmcp::ServerHandler + Clone + 'static>(
    State(state): State<Legacy<H>>,
    request: Request,
) -> Response {
    let owner = request
        .extensions()
        .get::<crate::oauth::AuthenticatedClient>()
        .cloned();
    if state.cancel.is_cancelled() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let admission = match state.admission.clone().try_acquire_owned() {
        Ok(permit) => Arc::new(permit),
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let pump_admission = admission.clone();
    let id = uuid::Uuid::new_v4().simple().to_string();
    let cancel = state.cancel.child_token();
    let (input, mut incoming) = mpsc::channel::<AdmittedMessage>(32);
    let (events, rx) = mpsc::channel(32);
    state.sessions.lock().unwrap().insert(
        id.clone(),
        Session {
            input,
            cancel: cancel.clone(),
            owner: owner.as_ref().map(|identity| identity.0.clone()),
        },
    );
    let endpoint = format!("{}?session_id={id}", state.message_path);
    let endpoint = match event_bytes("endpoint", &endpoint, &state.output) {
        Ok(event) => event,
        Err(_) => {
            state.sessions.lock().unwrap().remove(&id);
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    };
    let _ = events.send(endpoint).await;
    let (server_io, client_io) = tokio::io::duplex(65536);
    let (reader, mut writer) = tokio::io::split(client_io);
    let wire = Arc::new(Mutex::new(crate::opaque::WireState::default()));
    let registry = (state.factory)(wire.clone(), owner);
    let catalogs = state.catalogs.clone();
    let state_cleanup = state.clone();
    let id_cleanup = id.clone();
    let ct = cancel.clone();
    tokio::spawn(async move {
        let _admission = pump_admission;
        let serving = async {
            let service = registry.serve(server_io).await?;
            service.waiting().await?;
            Ok::<(), crate::Error>(())
        };
        let pump = async {
            let mut lines = OutputReader::with_limit(
                reader,
                state_cleanup.output.clone(),
                state_cleanup.output_limits,
            );
            let mut overlay = crate::opaque::Overlay::default();
            let mut lifecycle = crate::opaque::Lifecycle::default();
            let mut pending = HashMap::new();
            let completed = wire.lock().unwrap().completion_notification();
            loop {
                tokio::select! {
                    _ = completed.notified() => {
                        let ids = wire.lock().unwrap().drain_cancelled_native()?;
                        for id in ids { overlay.abandon(&id)?; pending.remove(&id); }
                    }
                    input = incoming.recv() => {
                        let Some(message) = input else { break };
                        let s = &message.body;
                        if let Some(reply) = lifecycle.request(s)? {
                            if let Some(reply) = reply
                                && events.send(output_event(&reply, &state_cleanup.output)?).await.is_err()
                            { break; }
                            continue;
                        }
                        let rewritten = wire.lock().unwrap().request_for(&catalogs, s)?;
                        lifecycle.remap(s, &rewritten)?;
                        let id = request_id(&rewritten)?;
                        if id.as_ref().is_some_and(|id| pending.contains_key(id)) {
                            return Err("duplicate SDK request identity".into());
                        }
                        overlay.request(&catalogs, &rewritten)?;
                        let rewritten = crate::opaque::native_arguments_for_sdk(&catalogs, &rewritten)?;
                        writer.write_all(rewritten.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                        // SDK/opaque state retains request copies until its reply;
                        // do not release aggregate ownership merely after writing.
                        if let Some(id) = id {
                            pending.insert(id, message._permit);
                        }
                    }
                    line = lines.next_line() => {
                        let Some(line) = line? else { break };
                        let response_id = request_id(&line.text)?;
                        if wire.lock().unwrap().stale_native_generation(&line.text)? { continue; }
                        if lifecycle.response(&line.text)? {
                            writer.write_all(crate::opaque::INITIALIZED).await?;
                        }
                        let response = overlay.response(&line.text)?;
                        let Some(response) = wire.lock().unwrap().native_response(&response)? else { continue; };
                        if events.send(event_bytes_with_limit("message", &response, &state_cleanup.output, state_cleanup.output_limits)?).await.is_err() { break; }
                        if let Some(id) = response_id { pending.remove(&id); }
                    }
                }
            }
            Ok::<(), crate::Error>(())
        };
        tokio::select! {_=ct.cancelled()=>{},_=tokio::time::sleep(state_cleanup.ttl)=>{},_=serving=>{},_=pump=>{}}
        state_cleanup.sessions.lock().unwrap().remove(&id_cleanup);
        ct.cancel();
    });
    let state = StreamState {
        rx,
        session: id,
        state,
        _admission: admission,
    };
    // Bytes owns the lease until the HTTP consumer drops its last clone, rather
    // than releasing it when an event is merely dequeued by the stream.
    let mut response = Response::new(Body::from_stream(stream::unfold(
        state,
        |mut s| async move {
            s.rx.recv()
                .await
                .map(|e| (Ok::<_, Infallible>(Bytes::from_owner(e)), s))
        },
    )));
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-cache"));
    response
        .headers_mut()
        .insert("connection", HeaderValue::from_static("keep-alive"));
    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    response
}
async fn message<H: rmcp::ServerHandler + Clone + 'static>(
    State(state): State<Legacy<H>>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    request: Request,
) -> Response {
    let owner = request
        .extensions()
        .get::<crate::oauth::AuthenticatedClient>()
        .map(|identity| identity.0.clone());
    if state.cancel.is_cancelled() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    // The final extractor is the unbuffered request: reject aggregate saturation
    // before polling even one body frame. Never await admission with a body.
    let permit = match state.messages.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let body = tokio::select! {
        biased;
        _ = state.cancel.cancelled() => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        body = String::from_request(request, &state) => body,
    };
    let body = match body {
        Ok(body) => body,
        Err(error) => {
            return if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
                (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large").into_response()
            } else {
                (StatusCode::BAD_REQUEST, "Could not parse message").into_response()
            };
        }
    };
    if headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| !v.starts_with("application/json"))
    {
        return (StatusCode::BAD_REQUEST, "Invalid Content-Type header").into_response();
    }
    let id = query
        .and_then(|q| reqwest::Url::parse(&format!("http://localhost/?{q}")).ok())
        .and_then(|u| {
            u.query_pairs()
                .filter(|(k, _)| k == "session_id")
                .map(|(_, v)| v.into_owned())
                .last()
        });
    let Some(id) = id else {
        return (StatusCode::BAD_REQUEST, "session_id is required").into_response();
    };
    let id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id.simple().to_string(),
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid session ID").into_response(),
    };
    let session = state.sessions.lock().unwrap().get(&id).cloned();
    let Some(session) = session else {
        return (StatusCode::NOT_FOUND, "Could not find session").into_response();
    };
    if session.owner != owner {
        // Match unknown-session behavior: another verified client cannot infer
        // whether this session exists, enqueue a request, or receive its stream.
        return (StatusCode::NOT_FOUND, "Could not find session").into_response();
    }
    // Validate the same envelope after a disposable lossless ID alias; the session
    // pump retains the real alias mapping through response/cancellation.
    let validation = crate::opaque::sdk_validation(&body)
        .and_then(|request| crate::opaque::native_arguments_for_sdk(&state.catalogs, &request));
    if validation
        .as_ref()
        .ok()
        .is_none_or(|text| serde_json::from_str::<rmcp::model::ClientJsonRpcMessage>(text).is_err())
    {
        return (StatusCode::BAD_REQUEST, "Could not parse message").into_response();
    }
    // No sender future may retain a body while waiting for per-session capacity.
    // The global count includes the pump's current message and every queued one.
    match session.input.try_send(AdmittedMessage {
        body,
        _permit: permit,
    }) {
        Ok(()) => {}
        Err(mpsc::error::TrySendError::Closed(_)) => {
            return (StatusCode::NOT_FOUND, "Could not find session").into_response();
        }
        Err(mpsc::error::TrySendError::Full(_)) => {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    }
    (StatusCode::ACCEPTED, "Accepted").into_response()
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    use crate::harness::Host;

    async fn restored(state: &Legacy, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while state.available_message_permits() != count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("message admission recovered");
    }

    #[tokio::test]
    async fn legacy_output_http_saturation_closes_session_and_recovers() -> crate::Result<()> {
        let cancel = CancellationToken::new();
        let mut state = Legacy::new(
            Registry::synthetic(),
            "/messages/",
            Duration::from_secs(30),
            cancel.clone(),
        )?;
        state.output = Arc::new(tokio::sync::Semaphore::new(128));
        let host = Host::start(state.clone().router("/sse")?, cancel).await?;
        let base = format!("http://{}", host.address);
        let client = reqwest::Client::new();
        let mut stream = client.get(format!("{base}/sse")).send().await?;
        let endpoint = stream.chunk().await?.unwrap();
        let endpoint = std::str::from_utf8(&endpoint)?
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap()
            .to_owned();
        assert_eq!(client.post(format!("{base}{endpoint}")).header("content-type", "application/json")
            .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#)
            .send().await?.status(), StatusCode::ACCEPTED);
        assert!(
            tokio::time::timeout(Duration::from_secs(3), stream.chunk())
                .await??
                .is_none(),
            "over-budget output must close, not truncate a reply"
        );
        restored(&state, MESSAGE_CAPACITY).await;
        assert_eq!(state.active_sessions(), 0);
        assert_eq!(state.available_output_bytes(), 128);
        // A fresh real connection can immediately consume its endpoint again.
        let mut next = client.get(format!("{base}/sse")).send().await?;
        assert!(next.chunk().await?.is_some());
        drop(next);
        host.shutdown().await?;
        assert_eq!(state.available_output_bytes(), 128);
        Ok(())
    }

    #[tokio::test]
    async fn legacy_output_socket_saturation_cancellation_and_consumer_ownership()
    -> crate::Result<()> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let mut writer = tokio::net::TcpStream::connect(listener.local_addr()?).await?;
        let (socket, _) = listener.accept().await?;
        let budget = Arc::new(tokio::sync::Semaphore::new(16));
        let mut reader = OutputReader::new(socket, budget.clone());
        writer.write_all(b"12345678").await?;
        // A partial frame stays charged across select cancellation.
        assert!(
            tokio::time::timeout(Duration::from_millis(20), reader.next_line())
                .await
                .is_err()
        );
        assert_eq!(budget.available_permits(), 8);
        writer.write_all(b"abcdefghX").await?;
        assert!(
            reader
                .next_line()
                .await
                .is_err_and(|e| e.to_string().contains("saturation"))
        );
        drop(reader);
        assert_eq!(budget.available_permits(), 16);
        let budget = Arc::new(tokio::sync::Semaphore::new(128));
        let event = output_event("hello\nworld\r", &budget)?;
        assert_eq!(
            event.bytes,
            b"event: message\ndata: hello\ndata: world\rdata: \n\n"
        );
        let used = event.bytes.len();
        let bytes = Bytes::from_owner(event);
        let clone = bytes.clone();
        assert_eq!(budget.available_permits(), 128 - used);
        drop(bytes);
        assert_eq!(budget.available_permits(), 128 - used);
        drop(clone);
        assert_eq!(budget.available_permits(), 128);
        Ok(())
    }

    #[tokio::test]
    async fn legacy_output_frame_limit_rejects_before_unbounded_line_allocation()
    -> crate::Result<()> {
        let budget = Arc::new(tokio::sync::Semaphore::new(OUTPUT_CAPACITY));
        let (mut writer, socket) = tokio::io::duplex(8192);
        let task = tokio::spawn(async move {
            let chunk = [b'x'; 8192];
            for _ in 0..=OUTPUT_FRAME_LIMIT / chunk.len() {
                if writer.write_all(&chunk).await.is_err() {
                    break;
                }
            }
        });
        let mut reader = OutputReader::new(socket, budget.clone());
        assert!(
            reader
                .next_line()
                .await
                .is_err_and(|e| e.to_string().contains("frame limit"))
        );
        assert!(reader.bytes.len() <= OUTPUT_FRAME_LIMIT);
        drop(reader);
        task.await?;
        assert_eq!(budget.available_permits(), OUTPUT_CAPACITY);
        Ok(())
    }

    #[tokio::test]
    async fn legacy_aggregate_ingress_precedes_body_poll_and_owns_queued_messages()
    -> crate::Result<()> {
        let cancel = CancellationToken::new();
        let state = Legacy::new(
            Registry::synthetic(),
            "/messages/",
            Duration::from_secs(30),
            cancel.clone(),
        )?;
        // Real HTTP routing with deliberately stalled consumers. Two sessions
        // prove that admission is global rather than a per-channel limit.
        let mut receivers = Vec::new();
        let mut urls = Vec::new();
        let host = Host::start(state.clone().router("/sse")?, cancel).await?;
        for _ in 0..2 {
            let id = uuid::Uuid::new_v4().simple().to_string();
            let (input, incoming) = mpsc::channel(32);
            state.sessions.lock().unwrap().insert(
                id.clone(),
                Session {
                    input,
                    cancel: CancellationToken::new(),
                    owner: None,
                },
            );
            receivers.push(incoming);
            urls.push(format!("http://{}/messages/?session_id={id}", host.address));
        }
        let client = reqwest::Client::new();
        for i in 0..MESSAGE_CAPACITY {
            let response = client
                .post(&urls[i % 2])
                .header("content-type", "application/json")
                .body(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
                .send()
                .await?;
            assert_eq!(response.status(), StatusCode::ACCEPTED);
            assert_eq!(state.available_message_permits(), MESSAGE_CAPACITY - i - 1);
        }
        // No EOF is ever supplied. A buffered String extractor would hang here.
        let response = client
            .post(&urls[0])
            .header("content-type", "application/json")
            .body(reqwest::Body::wrap_stream(stream::pending::<
                std::io::Result<Vec<u8>>,
            >()))
            .timeout(Duration::from_secs(2))
            .send()
            .await?;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let message = receivers[0].recv().await.unwrap();
        assert_eq!(
            state.available_message_permits(),
            0,
            "dequeue must not release ownership before consumption"
        );
        drop(message);
        restored(&state, 1).await;
        assert_eq!(
            client
                .post(&urls[0])
                .header("content-type", "application/json")
                .body("bad")
                .send()
                .await?
                .status(),
            StatusCode::BAD_REQUEST
        );
        restored(&state, 1).await;
        drop(receivers);
        restored(&state, MESSAGE_CAPACITY).await;
        assert_eq!(
            client
                .post(&urls[0])
                .header("content-type", "application/json")
                .body(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
                .send()
                .await?
                .status(),
            StatusCode::NOT_FOUND
        );
        restored(&state, MESSAGE_CAPACITY).await;
        host.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn legacy_open_partial_body_shutdown_reclaims_admission_before_peer_close()
    -> crate::Result<()> {
        let cancel = CancellationToken::new();
        let state = Legacy::new(
            Registry::synthetic(),
            "/messages/",
            Duration::from_secs(30),
            cancel.clone(),
        )?;
        let host = Host::start(state.clone().router("/sse")?, cancel).await?;
        let mut socket = tokio::net::TcpStream::connect(host.address).await?;
        socket.write_all(b"POST /messages/ HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{").await?;
        restored(&state, MESSAGE_CAPACITY - 1).await;
        host.shutdown().await?;
        // The peer remains open: shutdown itself must cancel the extraction.
        assert_eq!(state.available_message_permits(), MESSAGE_CAPACITY);
        assert_eq!(state.available_output_bytes(), OUTPUT_CAPACITY);
        let request = Request::new(Body::from_stream(
            stream::pending::<std::io::Result<Vec<u8>>>(),
        ));
        let rejected = tokio::time::timeout(
            Duration::from_secs(1),
            message(
                State(state.clone()),
                RawQuery(None),
                HeaderMap::new(),
                request,
            ),
        )
        .await?;
        assert_eq!(rejected.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            connect(State(state.clone()), Request::new(Body::empty()))
                .await
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(state.available_message_permits(), MESSAGE_CAPACITY);
        assert_eq!(state.available_permits(), 128);
        drop(socket);
        Ok(())
    }

    #[tokio::test]
    async fn legacy_aggregate_ingress_canceled_body_recovers_admission() -> crate::Result<()> {
        let cancel = CancellationToken::new();
        let state = Legacy::new(
            Registry::synthetic(),
            "/messages/",
            Duration::from_secs(30),
            cancel.clone(),
        )?;
        let host = Host::start(state.clone().router("/sse")?, cancel).await?;
        let mut socket = tokio::net::TcpStream::connect(host.address).await?;
        socket.write_all(b"POST /messages/ HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{").await?;
        restored(&state, MESSAGE_CAPACITY - 1).await;
        drop(socket);
        restored(&state, MESSAGE_CAPACITY).await;
        host.shutdown().await?;
        Ok(())
    }
}
