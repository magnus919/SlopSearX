//! Loopback-only, injected-state host with explicit cancellation ownership.
use crate::{Registry, Result};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use std::{net::SocketAddr, time::Duration};
use subtle::ConstantTimeEq;
use tokio_util::sync::CancellationToken;
// Keep unconsumed bytes in a ring: scanning and draining are linear even when
// one frame arrives bytewise or one chunk contains many short frames.
#[derive(Default)]
struct SseFrames {
    bytes: std::collections::VecDeque<u8>,
    scanned: usize,
    limit: Option<crate::output_limits::OutputLimits>,
    #[cfg(test)]
    inspected: usize,
}
impl SseFrames {
    fn push(&mut self, chunk: &[u8]) -> std::io::Result<()> {
        if chunk.len()
            > self
                .limit
                .unwrap_or(crate::output_limits::OutputLimits::HTTP)
                .bytes()
                .saturating_sub(self.bytes.len())
        {
            return Err(std::io::Error::other(format!(
                "MCP SSE frame exceeds {} bytes",
                self.limit
                    .unwrap_or(crate::output_limits::OutputLimits::HTTP)
                    .bytes()
            )));
        }
        self.bytes.extend(chunk);
        Ok(())
    }
    fn push_pending(&mut self, pending: &mut axum::body::Bytes) -> std::io::Result<()> {
        let room = self
            .limit
            .unwrap_or(crate::output_limits::OutputLimits::HTTP)
            .bytes()
            .saturating_sub(self.bytes.len());
        if room == 0 {
            return Err(std::io::Error::other(format!(
                "MCP SSE frame exceeds {} bytes",
                self.limit
                    .unwrap_or(crate::output_limits::OutputLimits::HTTP)
                    .bytes()
            )));
        }
        let take = room.min(pending.len());
        self.push(&pending.split_to(take))
    }
    fn next_frame(&mut self) -> Option<Vec<u8>> {
        while self.scanned < self.bytes.len() {
            self.scanned += 1;
            #[cfg(test)]
            {
                self.inspected += 1;
            }
            let end = self.scanned;
            let lf = end >= 2 && self.bytes[end - 2] == b'\n' && self.bytes[end - 1] == b'\n';
            let crlf = end >= 4
                && self.bytes[end - 4] == b'\r'
                && self.bytes[end - 3] == b'\n'
                && self.bytes[end - 2] == b'\r'
                && self.bytes[end - 1] == b'\n';
            if lf || crlf {
                self.scanned = 0;
                return Some(self.bytes.drain(..end).collect());
            }
        }
        None
    }
    fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

async fn bearer(
    axum::extract::State(token): axum::extract::State<String>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let authorized = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split_once(' '))
        .is_some_and(|(scheme, value)| {
            scheme.eq_ignore_ascii_case("bearer")
                && bool::from(value.as_bytes().ct_eq(token.as_bytes()))
        });
    if authorized {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [("content-type", "text/plain; charset=utf-8")],
            "unauthorized",
        )
            .into_response()
    }
}
pub fn http_router(
    registry: Registry,
    token: Option<String>,
    cancel: CancellationToken,
) -> ManagedRouter {
    http_router_mode(registry, token, cancel, false)
}
/// Additional SDK session qualification seam; production baseline remains stateless.
pub fn http_router_mode(
    registry: Registry,
    token: Option<String>,
    cancel: CancellationToken,
    stateful: bool,
) -> ManagedRouter {
    configured_router(registry, token, cancel, stateful, Duration::from_secs(300)).0
}
/// Injectable idle expiry exercises the actual SDK manager, without a second worker timer.
pub fn http_router_with_session_expiry(
    registry: Registry,
    token: Option<String>,
    cancel: CancellationToken,
    expiry: Duration,
) -> (ManagedRouter, SessionProbe) {
    configured_router(registry, token, cancel, true, expiry)
}
struct ExpiryWorkerConfig {
    expiry: Duration,
    #[cfg(test)]
    setup_gate: Option<std::sync::Arc<tokio::sync::Notify>>,
}
/// Test-only setup seam: hold the real idle-expiry worker until the test has
/// completed protocol initialization and established its SDK lock barrier.
#[cfg(test)]
fn http_router_with_gated_session_expiry(
    registry: Registry,
    token: Option<String>,
    cancel: CancellationToken,
    expiry: Duration,
) -> (ManagedRouter, SessionProbe) {
    configured_router_with_gate(
        registry,
        token,
        cancel,
        true,
        ExpiryWorkerConfig {
            expiry,
            setup_gate: Some(std::sync::Arc::new(tokio::sync::Notify::new())),
        },
    )
}
fn configured_router(
    registry: Registry,
    token: Option<String>,
    cancel: CancellationToken,
    stateful: bool,
    expiry: Duration,
) -> (ManagedRouter, SessionProbe) {
    configured_router_with_gate(
        registry,
        token,
        cancel,
        stateful,
        ExpiryWorkerConfig {
            expiry,
            #[cfg(test)]
            setup_gate: None,
        },
    )
}
fn configured_router_with_gate(
    registry: Registry,
    token: Option<String>,
    cancel: CancellationToken,
    stateful: bool,
    expiry_worker: ExpiryWorkerConfig,
) -> (ManagedRouter, SessionProbe) {
    let catalogs = registry.opaque.clone();
    configured_router_for_with_gate(
        registry,
        catalogs,
        token,
        cancel,
        stateful,
        expiry_worker,
        crate::output_limits::OutputLimits::HTTP,
    )
}
/// Mount a real injected handler using exactly the qualified lifecycle/opaque transport.
pub fn http_router_for<H: rmcp::ServerHandler + Clone + 'static>(
    handler: H,
    catalogs: std::sync::Arc<crate::opaque::Catalogs>,
    cancel: CancellationToken,
) -> ManagedRouter {
    configured_router_for(
        handler,
        catalogs,
        None,
        cancel,
        false,
        Duration::from_secs(300),
        crate::output_limits::OutputLimits::HTTP,
    )
    .0
}
pub fn http_router_for_with_limits<H: rmcp::ServerHandler + Clone + 'static>(
    handler: H,
    catalogs: std::sync::Arc<crate::opaque::Catalogs>,
    cancel: CancellationToken,
    limits: crate::output_limits::OutputLimits,
) -> ManagedRouter {
    configured_router_for(
        handler,
        catalogs,
        None,
        cancel,
        false,
        Duration::from_secs(300),
        limits,
    )
    .0
}
fn configured_router_for<H: rmcp::ServerHandler + Clone + 'static>(
    registry: H,
    catalogs: std::sync::Arc<crate::opaque::Catalogs>,
    token: Option<String>,
    cancel: CancellationToken,
    stateful: bool,
    expiry: Duration,
    output_limits: crate::output_limits::OutputLimits,
) -> (ManagedRouter, SessionProbe) {
    configured_router_for_with_gate(
        registry,
        catalogs,
        token,
        cancel,
        stateful,
        ExpiryWorkerConfig {
            expiry,
            #[cfg(test)]
            setup_gate: None,
        },
        output_limits,
    )
}
fn configured_router_for_with_gate<H: rmcp::ServerHandler + Clone + 'static>(
    registry: H,
    catalogs: std::sync::Arc<crate::opaque::Catalogs>,
    token: Option<String>,
    cancel: CancellationToken,
    stateful: bool,
    expiry_worker: ExpiryWorkerConfig,
    output_limits: crate::output_limits::OutputLimits,
) -> (ManagedRouter, SessionProbe) {
    let mut manager = LocalSessionManager::default();
    // One owner governs idle expiry. The SDK timer counts time inside active
    // operations; stream/request leases below suspend idle expiry correctly.
    manager.session_config.keep_alive = None;
    let manager = std::sync::Arc::new(manager);
    let state = OpaqueState {
        catalogs,
        output_limits,
        sessions: Default::default(),
        admission: stateful.then(|| std::sync::Arc::new(tokio::sync::Semaphore::new(128))),
    };
    let control = std::sync::Arc::new(CleanupControl {
        stop: CancellationToken::new(),
        done: CancellationToken::new(),
        task: std::sync::Mutex::new(None),
        result: Default::default(),
    });
    let expiry = expiry_worker.expiry;
    #[cfg(test)]
    let setup_gate = expiry_worker.setup_gate;
    #[cfg(test)]
    let expiry_close_observer = setup_gate.as_ref().map(|_| {
        std::sync::Arc::new(ExpiryCloseObserver {
            timeouts: std::sync::atomic::AtomicUsize::new(0),
            notify: tokio::sync::Notify::new(),
        })
    });
    let probe = SessionProbe {
        state: state.clone(),
        manager: manager.clone(),
        control: control.clone(),
        #[cfg(test)]
        setup_gate: setup_gate.clone(),
        #[cfg(test)]
        expiry_close_observer: expiry_close_observer.clone(),
    };
    let worker_state = state.clone();
    let worker_manager = manager.clone();
    let worker_cancel = cancel.clone();
    let worker_stopped = control.done.clone();
    let worker_stop = control.stop.clone();
    let worker_result = control.result.clone();
    #[cfg(test)]
    let worker_setup_gate = setup_gate;
    #[cfg(test)]
    let worker_expiry_close_observer = expiry_close_observer;
    let task = tokio::spawn(async move {
        let _stopped = worker_stopped.drop_guard();
        #[cfg(test)]
        if let Some(gate) = worker_setup_gate {
            tokio::select! {
                _ = gate.notified() => {},
                _ = worker_cancel.cancelled() => {},
                _ = worker_stop.cancelled() => {},
            }
        }
        let mut interval = tokio::time::interval(
            (expiry / 4).clamp(Duration::from_millis(5), Duration::from_secs(1)),
        );
        loop {
            tokio::select! { _=worker_cancel.cancelled()=>break, _=worker_stop.cancelled()=>break, _=interval.tick()=>{} }
            let entries: Vec<_> = worker_state
                .sessions
                .lock()
                .unwrap()
                .iter()
                .map(|(id, entry)| (id.clone(), entry.clone()))
                .collect();
            // SDK worker failure removes its manager entry in spawn_session_worker.
            let live: std::collections::HashSet<String> = match tokio::time::timeout(
                Duration::from_millis(100),
                worker_manager.sessions.read(),
            )
            .await
            {
                Ok(sessions) => sessions.keys().map(ToString::to_string).collect(),
                Err(_) => continue,
            };
            for (id, entry) in entries {
                if worker_stop.is_cancelled() || worker_cancel.is_cancelled() {
                    break;
                }
                let close = {
                    let mut activity = entry.activity.lock().unwrap();
                    if !live.contains(&id)
                        || (activity.active == 0 && activity.last.elapsed() >= expiry)
                    {
                        activity.closing = true;
                        true
                    } else {
                        false
                    }
                };
                if close {
                    // Never hold alias/activity locks while waiting for SDK manager locks.
                    let (removed, timed_out) =
                        close_and_confirm_observed(&worker_manager, &id).await;
                    if removed {
                        worker_state.sessions.lock().unwrap().remove(&id);
                    } else if timed_out {
                        #[cfg(test)]
                        if let Some(observer) = &worker_expiry_close_observer {
                            observer
                                .timeouts
                                .fetch_add(1, std::sync::atomic::Ordering::Release);
                            observer.notify.notify_one();
                        }
                    }
                }
            }
        }
        let drain = tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                let ids: Vec<_> = worker_manager
                    .sessions
                    .read()
                    .await
                    .keys()
                    .map(ToString::to_string)
                    .collect();
                if ids.is_empty() {
                    break;
                }
                futures_util::future::join_all(
                    ids.iter().map(|id| close_and_confirm(&worker_manager, id)),
                )
                .await;
                tokio::task::yield_now().await;
            }
        })
        .await;
        let result = if drain.is_ok() {
            worker_state.sessions.lock().unwrap().clear();
            Ok(())
        } else {
            Err("SDK session cleanup deadline; aliases and permits retained".to_owned())
        };
        *worker_result.lock().unwrap() = Some(result);
    });
    *control.task.lock().unwrap() = Some(task);
    let owner = std::sync::Arc::new(ExpiryOwner {
        control: control.clone(),
    });
    let service: StreamableHttpService<H, LocalSessionManager> = StreamableHttpService::new(
        move || Ok(registry.clone()),
        manager,
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(stateful)
            .with_json_response(true)
            .with_cancellation_token(cancel),
    );
    let router = Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(state, opaque_catalog))
        .layer(axum::Extension(owner));
    let router = if let Some(token) = token.filter(|s| !s.is_empty()) {
        router.layer(middleware::from_fn_with_state(token, bearer))
    } else {
        router
    };
    (
        ManagedRouter {
            router,
            cleanup: Some(control),
        },
        probe,
    )
}
async fn close_and_confirm(manager: &LocalSessionManager, id: &str) -> bool {
    close_and_confirm_observed(manager, id).await.0
}
async fn close_and_confirm_observed(manager: &LocalSessionManager, id: &str) -> (bool, bool) {
    use rmcp::transport::streamable_http_server::session::SessionManager;
    let close = tokio::time::timeout(
        Duration::from_secs(1),
        manager.close_session(&id.to_owned().into()),
    )
    .await;
    if matches!(close, Ok(Ok(()))) {
        return (true, false);
    }
    let timed_out = close.is_err();
    let removed = tokio::time::timeout(Duration::from_millis(100), manager.sessions.read())
        .await
        .is_ok_and(|sessions| !sessions.contains_key(id));
    (removed, timed_out)
}
struct CleanupControl {
    stop: CancellationToken,
    done: CancellationToken,
    task: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    result: std::sync::Arc<std::sync::Mutex<Option<std::result::Result<(), String>>>>,
}
impl CleanupControl {
    async fn finish(&self) -> Result<()> {
        self.stop.cancel();
        self.join().await
    }
    async fn join(&self) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(7), self.done.cancelled()).await?;
        let task = self.task.lock().unwrap().take();
        if let Some(task) = task {
            task.await?;
        }
        self.result
            .lock()
            .unwrap()
            .clone()
            .ok_or("cleanup task did not report completion")?
            .map_err(Into::into)
    }
}
struct ExpiryOwner {
    control: std::sync::Arc<CleanupControl>,
}
impl Drop for ExpiryOwner {
    fn drop(&mut self) {
        self.control.stop.cancel();
    }
}
/// Owns cleanup through explicit Host shutdown; dropping an unhosted router or
/// abrupt Host triggers the same bounded drain. Probe can join and verify it.
pub struct ManagedRouter {
    router: Router,
    cleanup: Option<std::sync::Arc<CleanupControl>>,
}
impl ManagedRouter {
    pub fn map(self, map: impl FnOnce(Router) -> Router) -> Self {
        Self {
            router: map(self.router),
            cleanup: self.cleanup,
        }
    }
}
impl From<Router> for ManagedRouter {
    fn from(router: Router) -> Self {
        Self {
            router,
            cleanup: None,
        }
    }
}
#[derive(Clone)]
pub struct SessionProbe {
    state: OpaqueState,
    manager: std::sync::Arc<LocalSessionManager>,
    control: std::sync::Arc<CleanupControl>,
    #[cfg(test)]
    setup_gate: Option<std::sync::Arc<tokio::sync::Notify>>,
    #[cfg(test)]
    expiry_close_observer: Option<std::sync::Arc<ExpiryCloseObserver>>,
}
#[cfg(test)]
struct ExpiryCloseObserver {
    timeouts: std::sync::atomic::AtomicUsize,
    notify: tokio::sync::Notify,
}
impl SessionProbe {
    pub fn available_permits(&self) -> usize {
        self.state
            .admission
            .as_ref()
            .map_or(128, |s| s.available_permits())
    }
    pub fn aliases(&self) -> usize {
        self.state.sessions.lock().unwrap().len()
    }
    pub async fn sdk_sessions(&self) -> usize {
        self.manager.sessions.read().await.len()
    }
    pub async fn stopped(&self) -> Result<()> {
        self.control.join().await
    }
    pub async fn terminate_sdk_session(&self, id: &str) -> Result<()> {
        use rmcp::transport::streamable_http_server::session::SessionManager;
        tokio::time::timeout(
            Duration::from_secs(1),
            self.manager.close_session(&id.to_owned().into()),
        )
        .await??;
        Ok(())
    }
    #[cfg(test)]
    fn release_expiry_worker(&self) {
        if let Some(gate) = &self.setup_gate {
            gate.notify_one();
        }
    }
    #[cfg(test)]
    async fn wait_for_expiry_close_timeout(&self) -> usize {
        loop {
            let Some(observer) = &self.expiry_close_observer else {
                return 0;
            };
            let count = observer.timeouts.load(std::sync::atomic::Ordering::Acquire);
            if count > 0 {
                return count;
            }
            observer.notify.notified().await;
        }
    }
}
type SharedWire = std::sync::Arc<std::sync::Mutex<crate::opaque::WireState>>;
struct WireLease {
    wire: SharedWire,
    request: String,
    activity: Option<SessionActivity>,
}
impl Drop for WireLease {
    fn drop(&mut self) {
        self.wire.lock().unwrap().abandon(&self.request);
    }
}
struct Activity {
    active: usize,
    last: std::time::Instant,
    closing: bool,
}
struct SessionEntry {
    wire: SharedWire,
    _permit: tokio::sync::OwnedSemaphorePermit,
    activity: std::sync::Mutex<Activity>,
}
struct SessionActivity(std::sync::Arc<SessionEntry>);
impl SessionActivity {
    fn acquire(entry: std::sync::Arc<SessionEntry>) -> Option<Self> {
        {
            let mut state = entry.activity.lock().unwrap();
            if state.closing {
                return None;
            }
            state.active += 1;
        }
        Some(Self(entry))
    }
}
impl Drop for SessionActivity {
    fn drop(&mut self) {
        let mut state = self.0.activity.lock().unwrap();
        state.active -= 1;
        state.last = std::time::Instant::now();
    }
}
#[derive(Clone)]
struct OpaqueState {
    admission: Option<std::sync::Arc<tokio::sync::Semaphore>>,
    catalogs: std::sync::Arc<crate::opaque::Catalogs>,
    output_limits: crate::output_limits::OutputLimits,
    sessions: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<SessionEntry>>>,
    >,
}
fn hold_activity(response: Response, activity: Option<SessionActivity>) -> Response {
    use futures_util::StreamExt;
    let (parts, body) = response.into_parts();
    let stream = futures_util::stream::unfold(
        (body.into_data_stream(), activity),
        |(mut body, activity)| async move { body.next().await.map(|chunk| (chunk, (body, activity))) },
    );
    Response::from_parts(parts, Body::from_stream(stream))
}
async fn opaque_catalog(
    axum::extract::State(state): axum::extract::State<OpaqueState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let session = request
        .headers()
        .get("mcp-session-id")
        .and_then(|s| s.to_str().ok())
        .map(str::to_owned);
    // Only server-issued sessions are retained; arbitrary client headers never
    // allocate persistent alias state or consume session admission capacity.
    let entry = session
        .as_ref()
        .and_then(|id| state.sessions.lock().unwrap().get(id).cloned());
    let wire = entry
        .as_ref()
        .map(|entry| entry.wire.clone())
        .unwrap_or_default();
    let activity = if let Some(entry) = entry {
        match SessionActivity::acquire(entry) {
            Some(activity) => Some(activity),
            None => return StatusCode::NOT_FOUND.into_response(),
        }
    } else {
        None
    };
    if request.method() == axum::http::Method::DELETE {
        let response = next.run(request).await;
        // The pinned SDK closes the manager entry before returning a successful
        // DELETE response. Rejections (including invalid protocol headers) do
        // not close it, so must retain both aliases and the admission permit.
        if response.status().is_success()
            && let Some(session) = session
        {
            state.sessions.lock().unwrap().remove(&session);
        }
        return response;
    }
    if request.method() != axum::http::Method::POST {
        return hold_activity(next.run(request).await, activity);
    }
    let (mut parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, 4 * 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };
    if let Ok(original) = std::str::from_utf8(&bytes) {
        parts
            .extensions
            .insert(crate::opaque::OriginalRequest::new(original.to_owned()));
    }
    let mut overlay = crate::opaque::Overlay::default();
    let bytes = if let Ok(text) = std::str::from_utf8(&bytes) {
        match wire.lock().unwrap().request_for(&state.catalogs, text) {
            Ok(rewritten) => {
                if overlay.request(&state.catalogs, &rewritten).is_err() {
                    return StatusCode::BAD_REQUEST.into_response();
                }
                let rewritten =
                    match crate::opaque::native_arguments_for_sdk(&state.catalogs, &rewritten) {
                        Ok(request) => request,
                        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
                    };
                axum::body::Bytes::from(rewritten)
            }
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        }
    } else {
        bytes
    };
    let initializing = session.is_none()
        && serde_json::from_slice::<
            std::collections::BTreeMap<String, Box<serde_json::value::RawValue>>,
        >(&bytes)
        .ok()
        .is_some_and(|fields| {
            fields
                .get("method")
                .is_some_and(|m| m.get() == "\"initialize\"")
        });
    let admission = if initializing {
        match &state.admission {
            Some(semaphore) => match semaphore.clone().try_acquire_owned() {
                Ok(permit) => Some(permit),
                Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
            },
            None => None,
        }
    } else {
        None
    };
    let mut lease = WireLease {
        wire: wire.clone(),
        request: String::from_utf8_lossy(&bytes).into_owned(),
        activity,
    };
    let response = next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await;
    if response.status() != StatusCode::OK {
        return response;
    }
    if let Some(admission) = admission
        && let Some(issued) = response
            .headers()
            .get("mcp-session-id")
            .and_then(|s| s.to_str().ok())
    {
        let entry = std::sync::Arc::new(SessionEntry {
            wire: wire.clone(),
            _permit: admission,
            activity: std::sync::Mutex::new(Activity {
                active: 0,
                last: std::time::Instant::now(),
                closing: false,
            }),
        });
        lease.activity = SessionActivity::acquire(entry.clone());
        state
            .sessions
            .lock()
            .unwrap()
            .insert(issued.to_owned(), entry);
    }
    let (mut parts, body) = response.into_parts();
    if parts
        .headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.starts_with("text/event-stream"))
    {
        use futures_util::StreamExt;
        let stream = futures_util::stream::try_unfold(
            (
                body.into_data_stream(),
                SseFrames {
                    limit: Some(state.output_limits),
                    ..Default::default()
                },
                axum::body::Bytes::new(),
                overlay,
                wire,
                lease,
                state.output_limits,
            ),
            |(mut stream, mut buffer, mut pending, mut overlay, wire, lease, output_limits)| async move {
                loop {
                    if let Some(frame) = buffer.next_frame() {
                        let text = String::from_utf8(frame).map_err(std::io::Error::other)?;
                        let rewritten = crate::opaque::overlay_event(&mut overlay, &text)
                            .map_err(|e| std::io::Error::other(e.to_string()))?;
                        let rewritten =
                            crate::opaque::wire_event(&mut wire.lock().unwrap(), &rewritten)
                                .map_err(|e| std::io::Error::other(e.to_string()))?;
                        if rewritten.len() > output_limits.bytes() {
                            return Err(std::io::Error::other(
                                "native output exceeds configured byte budget",
                            ));
                        }
                        return Ok::<_, std::io::Error>(Some((
                            axum::body::Bytes::from(rewritten),
                            (stream, buffer, pending, overlay, wire, lease, output_limits),
                        )));
                    }
                    // Transport chunks can coalesce several standalone events.
                    // Keep the remaining Bytes without copying and extract frames
                    // before copying more than one frame's capacity.
                    if !pending.is_empty() {
                        buffer.push_pending(&mut pending)?;
                        continue;
                    }
                    let Some(chunk) = stream.next().await else {
                        if buffer.is_empty() {
                            return Ok(None);
                        }
                        return Err(std::io::Error::other("incomplete MCP SSE frame"));
                    };
                    let chunk = chunk.map_err(std::io::Error::other)?;
                    pending = chunk;
                }
            },
        );
        parts.headers.remove("content-length");
        return Response::from_parts(parts, Body::from_stream(stream));
    }
    let bytes = match axum::body::to_bytes(body, state.output_limits.bytes()).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
    };
    let replacement = match std::str::from_utf8(&bytes)
        .map_err(|e| e.to_string())
        .and_then(|s| overlay.response(s).map_err(|e| e.to_string()))
        .and_then(|s| wire.lock().unwrap().response(&s).map_err(|e| e.to_string()))
    {
        Ok(value) => value,
        Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
    };
    if replacement.len() > state.output_limits.bytes() {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    parts.headers.remove("content-length");
    Response::from_parts(parts, Body::from(replacement))
}
pub struct Host {
    pub address: SocketAddr,
    pub cancel: CancellationToken,
    task: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
    cleanup: Option<std::sync::Arc<CleanupControl>>,
}
impl Host {
    pub async fn start(
        router: impl Into<ManagedRouter>,
        cancel: CancellationToken,
    ) -> Result<Self> {
        let ManagedRouter { router, cleanup } = router.into();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let ct = cancel.clone();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(ct.cancelled_owned())
                .await
        });
        Ok(Self {
            address,
            cancel,
            task: Some(task),
            cleanup,
        })
    }
    pub async fn shutdown(mut self) -> Result<()> {
        self.cancel.cancel();
        let mut server_deadline = false;
        if let Some(mut task) = self.task.take() {
            match tokio::time::timeout(Duration::from_secs(3), &mut task).await {
                Ok(result) => {
                    result??;
                }
                Err(_) => {
                    task.abort();
                    let _ = task.await;
                    server_deadline = true;
                }
            }
        }
        if let Some(cleanup) = &self.cleanup {
            cleanup.finish().await?;
        }
        if server_deadline {
            return Err("host shutdown deadline".into());
        }
        Ok(())
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(cleanup) = &self.cleanup {
            cleanup.stop.cancel();
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[tokio::test]
    async fn sdk_close_lock_timeout_retains_alias_admission_until_actual_removal() -> Result<()> {
        let cancel = CancellationToken::new();
        let (router, probe) = http_router_with_gated_session_expiry(
            Registry::synthetic(),
            None,
            cancel.clone(),
            Duration::from_millis(50),
        );
        let host = Host::start(router, cancel).await?;
        let url = format!("http://{}/mcp", host.address);
        let http = reqwest::Client::new();
        let response=http.post(&url).header("accept","application/json, text/event-stream").json(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"lock-test","version":"1"}}})).send().await?;
        let session = response.headers()["mcp-session-id"].to_str()?.to_owned();
        let _ = response.bytes().await?;
        tokio::time::sleep(Duration::from_millis(125)).await;
        let response = http
            .post(&url)
            .header("mcp-session-id", &session)
            .header("accept", "application/json, text/event-stream")
            .json(&serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .send()
            .await?;
        assert_eq!(response.status(), 202);
        let sdk_lock = probe.manager.sessions.read().await;
        probe.release_expiry_worker();
        assert!(
            tokio::time::timeout(
                Duration::from_secs(3),
                probe.wait_for_expiry_close_timeout()
            )
            .await?
                >= 1
        );
        tokio::time::sleep(Duration::from_millis(1250)).await;
        assert_eq!(probe.aliases(), 1);
        assert_eq!(probe.available_permits(), 127);
        assert_eq!(sdk_lock.len(), 1);
        drop(sdk_lock);
        tokio::time::timeout(Duration::from_secs(3), async {
            while probe.aliases() != 0
                || probe.sdk_sessions().await != 0
                || probe.available_permits() != 128
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await?;
        host.shutdown().await?;
        Ok(())
    }
}

#[cfg(test)]
mod sse_frame_tests {
    use super::SseFrames;
    #[test]
    fn fragmented_frames_scan_each_byte_once_and_preserve_mixed_boundaries() {
        let mut frames = SseFrames::default();
        let mut expected = vec![b'x'; 4 * 1024 * 1024 - 4];
        expected.extend_from_slice(b"\r\n\r\n");
        for byte in &expected[..expected.len() - 1] {
            frames.push(&[*byte]).unwrap();
            assert!(frames.next_frame().is_none());
        }
        frames.push(&expected[expected.len() - 1..]).unwrap();
        assert_eq!(frames.next_frame().unwrap(), expected);
        assert_eq!(frames.inspected, expected.len());
        assert!(frames.is_empty());
        let mixed = b"data: first\n\ndata: second\r\n\r\ndata: third\n\n";
        frames.push(mixed).unwrap();
        for expected in [
            b"data: first\n\n".as_slice(),
            b"data: second\r\n\r\n",
            b"data: third\n\n",
        ] {
            assert_eq!(frames.next_frame().unwrap(), expected);
        }
        assert_eq!(frames.inspected, 4 * 1024 * 1024 + mixed.len());
        assert!(frames.is_empty());
        // A ring buffer must also work after repeated wraparound and draining.
        for _ in 0..1000 {
            frames.push(b"data: small\n\n").unwrap();
            assert_eq!(frames.next_frame().unwrap(), b"data: small\n\n");
        }
    }
    #[test]
    fn coalesced_near_limit_frames_obey_individual_event_limit() {
        let limit = 4 * 1024 * 1024;
        for delimiter in [b"\n\n".as_slice(), b"\r\n\r\n"] {
            let mut first = vec![b'x'; limit - delimiter.len()];
            first.extend_from_slice(delimiter);
            let mut frames = SseFrames::default();
            frames.push(&first[..limit - 1]).unwrap();
            assert!(frames.next_frame().is_none());
            let mut tail = first[limit - 1..].to_vec();
            tail.extend_from_slice(b"data: next\n\n");
            let mut pending = axum::body::Bytes::from(tail);
            frames.push_pending(&mut pending).unwrap();
            assert_eq!(frames.next_frame().unwrap(), first);
            frames.push_pending(&mut pending).unwrap();
            assert_eq!(frames.next_frame().unwrap(), b"data: next\n\n");
            assert!(pending.is_empty());
            assert_eq!(frames.inspected, limit + b"data: next\n\n".len());
            let mut oversized = axum::body::Bytes::from(vec![b'x'; limit + 1]);
            frames.push_pending(&mut oversized).unwrap();
            assert!(frames.next_frame().is_none());
            assert!(frames.push_pending(&mut oversized).is_err());
        }
    }
    #[test]
    fn incomplete_frames_and_limit_remain_explicit() {
        let mut frames = SseFrames::default();
        frames.push(&vec![b'x'; 4 * 1024 * 1024]).unwrap();
        assert!(frames.next_frame().is_none());
        assert!(!frames.is_empty());
        assert!(frames.push(b"x").is_err());
        assert_eq!(frames.inspected, 4 * 1024 * 1024);
        assert!(frames.next_frame().is_none());
        assert_eq!(frames.inspected, 4 * 1024 * 1024);
    }
}
