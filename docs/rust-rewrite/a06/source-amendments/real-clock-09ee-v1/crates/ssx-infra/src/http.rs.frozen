//! Adapter-owned sockets and search-owned identity. No provider retries or parsers.
mod monotonic;
pub(crate) use monotonic::Clock as DeadlineClock;
#[cfg(test)]
pub(crate) use monotonic::Driver;
use reqwest::{
    Client, Method, Url,
    cookie::{CookieStore, Jar},
    header::{self, HeaderMap, HeaderValue},
};
use std::{
    collections::HashMap,
    future::Future,
    io::Read,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, oneshot, watch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Closed,
    Cancelled,
    Deadline,
    RequestLimit,
    RequestBodyLimit,
    WireBodyLimit,
    DecodedBodyLimit,
    RedirectLimit,
    InvalidRequest,
    InvalidResponse,
    UnsupportedEncoding,
    Network,
}
pub type Result<T> = std::result::Result<T, Error>;
/// Reviewed native outbound adapter identity. Explicit request/session values
/// override this static versioned default; no tenant/auth headers are copied.
pub const DEFAULT_USER_AGENT: &str = concat!(
    "SlopSearX/",
    env!("CARGO_PKG_VERSION"),
    " (meta search engine; agent-native)"
);
fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static(DEFAULT_USER_AGENT),
    );
    headers
}

pub type ExchangeFuture<'a> = Pin<Box<dyn Future<Output = Result<Response>> + Send + 'a>>;
/// Caller-owned transports are borrowed and never closed by this module.
pub trait Transport: Send + Sync {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_>;
}
#[derive(Clone, Debug)]
pub struct Request {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}
impl Request {
    pub fn get(url: &str) -> Result<Self> {
        Ok(Self {
            method: Method::GET,
            url: Url::parse(url).map_err(|_| Error::InvalidRequest)?,
            headers: HeaderMap::new(),
            body: Vec::new(),
        })
    }
}
#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}
/// HTTPX joins repeated field values; an additional encoding prevents the
/// identity-only early size guard. Integer parsing follows the source's Python
/// text rules without overflowing a machine-sized counter.
pub(crate) fn identity_announced_oversize(headers: &HeaderMap, limit: usize) -> bool {
    let combined = |name| -> Option<String> {
        headers
            .get_all(name)
            .iter()
            .map(|v| v.to_str().ok().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
            .map(|v| v.join(", "))
    };
    let Some(encoding) = combined(header::CONTENT_ENCODING) else {
        return false;
    };
    if !matches!(
        encoding.trim().to_ascii_lowercase().as_str(),
        "" | "identity"
    ) {
        return false;
    }
    let Some(length) = combined(header::CONTENT_LENGTH) else {
        return false;
    };
    let Ok(normalized) = ssx_domain::storage::normalize_python_number_text(&length) else {
        return false;
    };
    let (negative, digits) = if let Some(digits) = normalized.strip_prefix('-') {
        (true, digits)
    } else {
        (false, normalized.strip_prefix('+').unwrap_or(&normalized))
    };
    if digits.is_empty()
        || digits.len() > 4300
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || negative
    {
        return false;
    }
    let digits = digits.trim_start_matches('0');
    let bound = limit.to_string();
    digits.len() > bound.len() || (digits.len() == bound.len() && digits > bound.as_str())
}

/// New migration safety bounds; adapters can select explicit, finite values.
#[derive(Clone, Debug)]
pub struct Bounds {
    pub wire_bytes: usize,
    pub decoded_bytes: usize,
    pub request_bytes: usize,
    pub requests: usize,
    pub redirects: usize,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            wire_bytes: 8 * 1024 * 1024,
            decoded_bytes: 32 * 1024 * 1024,
            request_bytes: 2 * 1024 * 1024,
            requests: 16,
            redirects: 10,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Options {
    pub proxy: Option<String>,
    pub trust_env: bool,
    pub verify_tls: bool,
    pub http2: bool,
    pub root_certificates: Vec<reqwest::Certificate>,
    pub trust_roots_only: bool,
    pub identity: Option<reqwest::Identity>,
    pub follow_redirects: bool,
    pub bounds: Bounds,
    /// Optional decoded identity Content-Length ceiling, checked before body reads.
    pub identity_announced_bytes: Option<usize>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            proxy: None,
            trust_env: true,
            verify_tls: true,
            http2: false,
            root_certificates: Vec::new(),
            trust_roots_only: false,
            identity: None,
            follow_redirects: false,
            bounds: Bounds::default(),
            identity_announced_bytes: None,
        }
    }
}
#[derive(Clone)]
pub struct Cancellation(watch::Sender<bool>);
impl Default for Cancellation {
    fn default() -> Self {
        Self(watch::channel(false).0)
    }
}
impl Cancellation {
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
    async fn cancelled(&self) {
        let mut rx = self.0.subscribe();
        loop {
            if *rx.borrow_and_update() {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    }
}
struct Idle {
    _retention: OwnedSemaphorePermit,
    origin: String,
    client: Client,
    used: Instant,
}
struct Pool {
    idle: Arc<Semaphore>,
    slots: Mutex<Vec<(bool, Option<Idle>)>>,
    permits: Arc<Semaphore>,
    options: Options,
}
impl Pool {
    fn new(options: Options, idle: Arc<Semaphore>) -> Self {
        Self {
            idle,
            slots: Mutex::new((0..10).map(|_| (false, None)).collect()),
            permits: Arc::new(Semaphore::new(20)),
            options,
        }
    }
    fn reserve(
        self: &Arc<Self>,
        origin: String,
        state: Arc<Mutex<State>>,
        adapter_permit: OwnedSemaphorePermit,
        pool_permit: OwnedSemaphorePermit,
    ) -> Result<Checkout> {
        let mut slots = self.slots.lock().unwrap();
        if self.permits.is_closed() {
            return Err(Error::Closed);
        }
        // Origin slots bound each pool; adapter-wide retention permits are acquired
        // when returning clients, so all route and ephemeral pools together
        // retain at most ten idle connections.
        let slot = slots
            .iter()
            .position(|(busy, s)| {
                !busy
                    && s.as_ref().is_some_and(|i| {
                        (i.origin == origin || i.origin.is_empty())
                            && i.used.elapsed() < Duration::from_secs(5)
                    })
            })
            .or_else(|| slots.iter().position(|(busy, s)| !busy && s.is_none()))
            .or_else(|| {
                slots
                    .iter()
                    .enumerate()
                    .filter(|(_, (busy, _))| !*busy)
                    .filter_map(|(n, (_, s))| s.as_ref().map(|i| (n, i.used)))
                    .min_by_key(|(_, t)| *t)
                    .map(|(n, _)| n)
            });
        let client = if let Some(n) = slot {
            let client = match slots[n].1.take() {
                Some(i)
                    if (i.origin == origin || i.origin.is_empty())
                        && i.used.elapsed() < Duration::from_secs(5) =>
                {
                    Some(i.client)
                }
                _ => None,
            };
            slots[n].0 = true;
            client
        } else {
            None
        };
        Ok(Checkout {
            state,
            pool: self.clone(),
            slot,
            origin,
            client,
            retain: true,
            _adapter_permit: adapter_permit,
            _pool_permit: pool_permit,
        })
    }
    fn expire_idle(&self) {
        for (_, idle) in self.slots.lock().unwrap().iter_mut() {
            if idle
                .as_ref()
                .is_some_and(|entry| entry.used.elapsed() >= Duration::from_secs(5))
            {
                *idle = None;
            }
        }
    }
    fn close(&self) {
        self.permits.close();
        self.slots
            .lock()
            .unwrap()
            .iter_mut()
            .for_each(|s| s.1 = None);
    }
}
struct Checkout {
    state: Arc<Mutex<State>>,
    pool: Arc<Pool>,
    slot: Option<usize>,
    origin: String,
    client: Option<Client>,
    // A non-abortable builder owns this whole checkout, including these leases
    // and its reserved slot, until completion even if its waiting future drops.
    retain: bool,
    _adapter_permit: OwnedSemaphorePermit,
    _pool_permit: OwnedSemaphorePermit,
}
impl Drop for Checkout {
    fn drop(&mut self) {
        // Match shutdown's state -> pool lock order. Expiry may have elapsed
        // during the response, so reclaim before attempting idle retention too.
        self.state.lock().unwrap().expire_idle();
        if let Some(n) = self.slot {
            let mut slots = self.pool.slots.lock().unwrap();
            slots[n].0 = false;
            if self.pool.permits.is_closed() || self.client.is_none() || !self.retain {
                return;
            }
            slots[n].1 = self
                .pool
                .idle
                .clone()
                .try_acquire_owned()
                .ok()
                .map(|permit| Idle {
                    _retention: permit,
                    origin: self.origin.clone(),
                    client: self.client.take().unwrap(),
                    used: Instant::now(),
                });
        }
    }
}
/// Startup preparation is deliberately limited to the preexisting ordinary
/// default pool. Ephemeral TLS/proxy profiles retain their cold-session lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preparation {
    Prepared,
    Ephemeral,
    Injected,
    CapacityUnavailable,
}
#[derive(Default)]
struct BuilderState {
    closed: bool,
    jobs: Vec<tokio::task::JoinHandle<()>>,
}
#[cfg(test)]
type TestClientBuilder = Arc<dyn Fn(&Options) -> Result<Client> + Send + Sync>;
#[derive(Default)]
struct ClientBuilders {
    state: Mutex<BuilderState>,
    draining: tokio::sync::Mutex<()>,
    #[cfg(test)]
    build: Mutex<Option<TestClientBuilder>>,
    #[cfg(test)]
    events: Mutex<Option<tokio::sync::mpsc::Sender<&'static str>>>,
}
impl ClientBuilders {
    fn event(&self, event: &'static str) {
        #[cfg(test)]
        if let Some(events) = self.events.lock().unwrap().as_ref() {
            let _ = events.try_send(event);
        }
        #[cfg(not(test))]
        let _ = event;
    }
    fn construct(&self, options: &Options) -> Result<Client> {
        #[cfg(test)]
        if let Some(build) = self.build.lock().unwrap().clone() {
            return build(options);
        }
        build_client(options)
    }
    async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (tx, rx) = oneshot::channel();
        {
            let mut state = self.state.lock().unwrap();
            if state.closed {
                return Err(Error::Closed);
            }
            // Admission belongs to the caller's existing connection and pool
            // leases, already moved into work, not a second worker allowance.
            state.jobs.retain(|job| !job.is_finished());
            state.jobs.push(tokio::task::spawn_blocking(move || {
                // Receiver cancellation drops the completed checkout here,
                // releasing its client, reserved slot and both owned leases.
                let _ = tx.send(work());
            }));
        }
        rx.await.map_err(|_| Error::InvalidResponse)?
    }
    fn close(&self) {
        self.state.lock().unwrap().closed = true;
    }
    async fn drain(&self) {
        let _draining = self.draining.lock().await;
        // Keep join handles in the owner across cancellation of this drain.
        // The async lock serializes concurrent drain waiters without detaching
        // any job when one caller drops its shutdown future.
        std::future::poll_fn(|cx| {
            let mut state = self.state.lock().unwrap();
            state
                .jobs
                .retain_mut(|job| Pin::new(job).poll(cx).is_pending());
            if state.jobs.is_empty() {
                std::task::Poll::Ready(())
            } else {
                std::task::Poll::Pending
            }
        })
        .await;
        self.event("builders_joined");
    }
}
async fn checkout(
    pool: Arc<Pool>,
    builders: Arc<ClientBuilders>,
    state: Arc<Mutex<State>>,
    connections: Arc<Semaphore>,
    origin: String,
) -> Result<Checkout> {
    let adapter_permit = connections
        .acquire_owned()
        .await
        .map_err(|_| Error::Closed)?;
    let pool_permit = pool
        .permits
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| Error::Closed)?;
    state.lock().unwrap().expire_idle();
    let mut checkout = pool.reserve(origin, state, adapter_permit, pool_permit)?;
    if checkout.client.is_none() {
        // Only the receiving operation can authorize idle retention. If its
        // future drops, the worker/result guard drops the new client outright.
        checkout.retain = false;
        let builder_owner = builders.clone();
        let mut checkout = builders
            .run(move || {
                checkout.client = Some(builder_owner.construct(&checkout.pool.options)?);
                Ok(checkout)
            })
            .await?;
        checkout.retain = true;
        Ok(checkout)
    } else {
        Ok(checkout)
    }
}
fn build_client(o: &Options) -> Result<Client> {
    let mut b = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .pool_max_idle_per_host(1)
        .pool_idle_timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(!o.verify_tls)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd();
    if !o.root_certificates.is_empty() {
        b = if o.trust_roots_only {
            b.tls_certs_only(o.root_certificates.clone())
        } else {
            b.tls_certs_merge(o.root_certificates.clone())
        };
    }
    if let Some(identity) = &o.identity {
        b = b.identity(identity.clone());
    }
    if !o.http2 {
        b = b.http1_only();
    }
    if !o.trust_env {
        b = b.no_proxy();
    }
    if let Some(proxy) = &o.proxy {
        b = b.proxy(reqwest::Proxy::all(proxy).map_err(|_| Error::InvalidRequest)?);
    }
    b.build().map_err(|_| Error::InvalidRequest)
}
struct State {
    closed: bool,
    used: bool,
    pools: HashMap<Option<String>, Arc<Pool>>,
    injected: Option<Arc<dyn Transport>>,
    all_pools: Vec<Weak<Pool>>,
}
impl State {
    fn expire_idle(&mut self) {
        self.all_pools.retain(|pool| {
            if let Some(pool) = pool.upgrade() {
                pool.expire_idle();
                true
            } else {
                false
            }
        });
    }
}
pub struct AdapterHttp {
    clock: DeadlineClock,
    #[cfg(test)]
    replay_endpoint: Option<url::Url>,
    #[cfg(test)]
    operation_trace: Option<SystemOperationTrace>,
    idle: Arc<Semaphore>,
    connections: Arc<Semaphore>,
    decoder: Arc<Decoder>,
    builders: Arc<ClientBuilders>,
    state: Arc<Mutex<State>>,
    stop: Cancellation,
}
impl Default for AdapterHttp {
    fn default() -> Self {
        Self {
            clock: DeadlineClock::default(),
            #[cfg(test)]
            replay_endpoint: None,
            #[cfg(test)]
            operation_trace: None,
            idle: Arc::new(Semaphore::new(10)),
            connections: Arc::new(Semaphore::new(20)),
            decoder: Arc::new(Decoder::default()),
            builders: Arc::new(ClientBuilders::default()),
            state: Arc::new(Mutex::new(State {
                closed: false,
                used: false,
                pools: HashMap::new(),
                injected: None,
                all_pools: Vec::new(),
            })),
            stop: Cancellation::default(),
        }
    }
}
impl AdapterHttp {
    pub(crate) fn deadline_now(&self) -> Instant {
        self.clock.now()
    }
    pub(crate) async fn deadline_sleep_until(&self, deadline: Instant) {
        self.clock.sleep_until(deadline).await;
    }
    /// Construct a real-pool owner with a manually advanced deadline clock.
    /// This is private to unit tests; it does not install a transport or expose
    /// a production clock-selection path.
    #[cfg(test)]
    pub(crate) fn clocked_real_pool_owner() -> (Self, Driver) {
        let (clock, driver) = DeadlineClock::replay();
        let mut owner = Self::default();
        owner.clock = clock;
        (owner, driver)
    }
    /// Private A04 delayed-response replay; production cannot select this clock.
    #[cfg(test)]
    pub(crate) fn loopback_timeout_replay(
        base: &str,
        service_context: Option<Instant>,
    ) -> Result<(Self, monotonic::Driver)> {
        let endpoint = url::Url::parse(base).map_err(|_| Error::InvalidRequest)?;
        let loopback = endpoint
            .host_str()
            .and_then(|h| h.parse::<std::net::IpAddr>().ok())
            .is_some_and(|ip| ip.is_loopback());
        if !loopback
            || endpoint.scheme() != "http"
            || service_context.is_some()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
        {
            return Err(Error::InvalidRequest);
        }
        let (mut owner, driver) = Self::clocked_real_pool_owner();
        owner.replay_endpoint = Some(endpoint);
        Ok((owner, driver))
    }
    /// Fixed two-entry metadata observer for the private Wikipedia unit replay.
    /// It retains System time and the real pool; it cannot inject a transport.
    #[cfg(test)]
    pub(crate) fn wikipedia_operation_replay(
        base: &str,
        service_context: Option<Instant>,
    ) -> Result<(Self, SystemOperationTrace)> {
        let endpoint = url::Url::parse(base).map_err(|_| Error::InvalidRequest)?;
        let loopback = endpoint
            .host_str()
            .and_then(|h| h.parse::<std::net::IpAddr>().ok())
            .is_some_and(|ip| ip.is_loopback());
        if !loopback
            || endpoint.scheme() != "http"
            || service_context.is_some()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
        {
            return Err(Error::InvalidRequest);
        }
        let trace = SystemOperationTrace(Arc::new(Mutex::new(Vec::with_capacity(2))));
        let mut owner = Self::default();
        owner.replay_endpoint = Some(endpoint);
        owner.operation_trace = Some(trace.clone());
        Ok((owner, trace))
    }
    pub(crate) fn deadline_clock(&self) -> DeadlineClock {
        self.clock.clone()
    }
    #[cfg(test)]
    pub(crate) fn pending_replay_owner(
        manual: bool,
    ) -> (Self, PendingObserver, Option<monotonic::Driver>) {
        pending_replay::owner(manual)
    }
    pub(crate) async fn admission_timeout<F: std::future::Future>(
        &self,
        deadline: Instant,
        future: F,
    ) -> std::result::Result<F::Output, ()> {
        #[cfg(test)]
        if self.clock.is_manual() {
            assert!(
                self.clock.is_pending_only(),
                "scholarly Manual owner must be sealed pending-only"
            );
            return tokio::select! { biased; value=future=>Ok(value), _=self.clock.sleep_until(deadline)=>Err(()) };
        }
        tokio::time::timeout_at(deadline.into(), future)
            .await
            .map_err(|_| ())
    }
    #[cfg(test)]
    pub(crate) fn record_scholarly_deadlines(&self, network: Instant, operation: Instant) {
        self.clock.record_deadlines(network, operation);
    }
    #[cfg(test)]
    pub(crate) fn assert_direct_replay_context(&self, context: Option<Instant>) {
        assert!(
            !self.clock.is_manual() || context.is_none(),
            "manual HTTP replay is direct-only; context parser/admission clocks remain System"
        );
    }
    /// Sealed unit-test construction always installs an injected transport.
    /// No production/config/feature entry point can select a Manual clock.
    #[cfg(test)]
    pub(crate) fn replay_owner(transport: Arc<dyn Transport>) -> (Self, monotonic::Driver) {
        let (clock, driver) = DeadlineClock::replay();
        let mut owner = Self::default();
        owner.clock = clock;
        owner.set_transport(transport).expect("unused replay owner");
        (owner, driver)
    }
    #[cfg(test)]
    pub(crate) fn replay_workers(&self) -> (usize, usize) {
        (
            self.decoder.state.lock().unwrap().active,
            self.builders.state.lock().unwrap().jobs.len(),
        )
    }
    /// Production startup constructor. A single runtime-owned startup guardian
    /// retains the unpublished owner until receiver acknowledgment, or closes
    /// and drains it if the constructor future disappears. No global reaper or
    /// provider request is involved. Only the owner Arc is shared, never Client.
    ///
    /// Cleanup can outlast the startup deadline: OS proxy discovery cannot be
    /// forcibly canceled. The guardian is not externally joined by a canceled
    /// caller, and its async cleanup requires its Tokio runtime to remain alive.
    /// Publication means this constructor returns Ready. A completed enclosing
    /// task result left unread is normal post-return owner Drop; exact joins
    /// then require an external shutdown supervisor, not this startup guardian.
    pub async fn new_ready(
        deadline: tokio::time::Instant,
        cancel: Cancellation,
    ) -> Result<(Arc<Self>, Preparation)> {
        Self::ready_owner(Arc::new(Self::default()), deadline, cancel).await
    }
    async fn ready_owner(
        owner: Arc<Self>,
        deadline: tokio::time::Instant,
        cancel: Cancellation,
    ) -> Result<(Arc<Self>, Preparation)> {
        let (mut publication, receiver) = oneshot::channel();
        let receiver_cancel = cancel.clone();
        // Exactly one startup guardian per new owner. Its sole cleanup scope is
        // this owner and its existing admitted builders, not a background queue.
        let _guardian = tokio::spawn(async move {
            let result = tokio::select! { biased;
                _=publication.closed()=>Err(Error::Cancelled),
                result=owner.prepare_default(deadline, cancel.clone())=>result,
            };
            match result {
                Ok(preparation) => {
                    // Complete initialization joins before publication; no search
                    // consumer can yet create another builder on this owner.
                    owner.builders.drain().await;
                    let late_error = if *owner.stop.0.borrow() {
                        Some(Error::Closed)
                    } else if *cancel.0.borrow() {
                        Some(Error::Cancelled)
                    } else if tokio::time::Instant::now() >= deadline {
                        Some(Error::Deadline)
                    } else {
                        None
                    };
                    if let Some(error) = late_error {
                        owner.builders.event("startup_cleanup_started");
                        owner.shutdown_and_wait().await;
                        let _ = publication.send(Err(error));
                        return;
                    }
                    let (acknowledge, acknowledgment) = oneshot::channel();
                    let value = Ok((owner.clone(), preparation, acknowledge));
                    match publication.send(value) {
                        Ok(()) => {
                            owner.builders.event("startup_sent");
                            if acknowledgment.await.is_ok() {
                                owner.builders.event("startup_acknowledged");
                                return;
                            }
                        }
                        Err(value) => drop(value),
                    }
                    // A successful send can still be unread and then dropped.
                    // Only receiver acknowledgment authorizes publication.
                    owner.builders.event("startup_cleanup_started");
                    owner.shutdown_and_wait().await;
                }
                Err(error) => {
                    owner.builders.event("startup_cleanup_started");
                    owner.shutdown_and_wait().await;
                    let _ = publication.send(Err(error));
                }
            }
        });
        let (owner, preparation, acknowledge) = receiver.await.map_err(|_| Error::Closed)??;
        // A successfully sent payload may have remained unread while startup
        // cancellation/closure/deadline changed. Recheck before publication in
        // the same priority as execute; rejecting drops the ack and owner Arc,
        // so the guardian still owns and drains the unpublished owner.
        if *owner.stop.0.borrow() {
            return Err(Error::Closed);
        }
        if *receiver_cancel.0.borrow() {
            return Err(Error::Cancelled);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(Error::Deadline);
        }
        // No await/yield between acknowledgment and returning the strong owner.
        acknowledge.send(()).map_err(|_| Error::Closed)?;
        Ok((owner, preparation))
    }
    pub fn set_transport(&self, t: Arc<dyn Transport>) -> Result<()> {
        #[cfg(test)]
        if self.clock.is_pending_only() {
            return Err(Error::InvalidRequest);
        }
        let mut s = self.state.lock().unwrap();
        if s.closed || s.used {
            return Err(Error::Closed);
        }
        s.injected = Some(t);
        Ok(())
    }
    pub fn session(
        &self,
        mut options: Options,
        deadline: tokio::time::Instant,
        cancel: Cancellation,
    ) -> Result<Session> {
        let mut s = self.state.lock().unwrap();
        if s.closed {
            return Err(Error::Closed);
        }
        if options.trust_roots_only && options.root_certificates.is_empty() {
            return Err(Error::InvalidRequest);
        }
        s.used = true;
        if s.injected.is_none()
            && options.verify_tls
            && options.trust_env
            && options.root_certificates.is_empty()
        {
            if let Some(path) = std::env::var_os("SSL_CERT_FILE") {
                let pem = std::fs::read(path).map_err(|_| Error::InvalidRequest)?;
                let certs = reqwest::Certificate::from_pem_bundle(&pem)
                    .map_err(|_| Error::InvalidRequest)?;
                if certs.is_empty() {
                    return Err(Error::InvalidRequest);
                }
                options.root_certificates.extend(certs);
                options.trust_roots_only = true;
            } else if let Some(path) = std::env::var_os("SSL_CERT_DIR") {
                for entry in std::fs::read_dir(path).map_err(|_| Error::InvalidRequest)? {
                    let entry = entry.map_err(|_| Error::InvalidRequest)?;
                    if entry.path().is_file() {
                        let pem = std::fs::read(entry.path()).map_err(|_| Error::InvalidRequest)?;
                        if let Ok(certs) = reqwest::Certificate::from_pem_bundle(&pem) {
                            options.root_certificates.extend(certs);
                        }
                    }
                }
                if options.root_certificates.is_empty() {
                    return Err(Error::InvalidRequest);
                }
                options.trust_roots_only = true;
            }
        }
        let ephemeral = !options.root_certificates.is_empty()
            || options.identity.is_some()
            || options.http2
            || !options.verify_tls
            || !options.trust_env
            || (options.proxy.is_none()
                && [
                    "HTTP_PROXY",
                    "HTTPS_PROXY",
                    "ALL_PROXY",
                    "http_proxy",
                    "https_proxy",
                    "all_proxy",
                ]
                .iter()
                .any(|k| std::env::var(k).is_ok_and(|v| !v.is_empty())));
        let pool = if ephemeral || (!s.pools.contains_key(&options.proxy) && s.pools.len() >= 8) {
            Arc::new(Pool::new(options.clone(), self.idle.clone()))
        } else {
            s.pools
                .entry(options.proxy.clone())
                .or_insert_with(|| Arc::new(Pool::new(options.clone(), self.idle.clone())))
                .clone()
        };
        s.all_pools.retain(|p| p.strong_count() > 0);
        if !s.all_pools.iter().any(|p| p.ptr_eq(&Arc::downgrade(&pool))) {
            s.all_pools.push(Arc::downgrade(&pool));
        }
        Ok(Session {
            clock: self.clock.clone(),
            #[cfg(test)]
            replay_endpoint: self.replay_endpoint.clone(),
            #[cfg(test)]
            operation_trace: self.operation_trace.clone(),
            #[cfg(test)]
            replay_identity: self.operation_trace.as_ref().map(|_| Arc::new(())),
            state: self.state.clone(),
            connections: self.connections.clone(),
            decoder: self.decoder.clone(),
            builders: self.builders.clone(),
            pool,
            injected: s.injected.clone(),
            jar: Jar::default(),
            headers: default_headers(),
            options,
            deadline,
            cancel,
            stop: self.stop.clone(),
            requests: 0,
        })
    }
    /// Prepare one independent default-profile client before publishing native
    /// consumers. This never connects to a URL. Only the already retained
    /// ordinary pool is eligible; custom/environment-derived ephemeral profiles
    /// are reported explicitly and remain cooperatively cold.
    ///
    /// The startup deadline covers admission and construction. A prepared client
    /// shares the existing adapter-wide idle-ten allowance and expires after
    /// five seconds, without extending the later request's absolute deadline.
    pub async fn prepare_default(
        &self,
        deadline: tokio::time::Instant,
        cancel: Cancellation,
    ) -> Result<Preparation> {
        if *self.stop.0.borrow() {
            return Err(Error::Closed);
        }
        if *cancel.0.borrow() {
            return Err(Error::Cancelled);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(Error::Deadline);
        }
        {
            let state = self.state.lock().unwrap();
            if state.closed {
                return Err(Error::Closed);
            }
            if state.injected.is_some() {
                return Ok(Preparation::Injected);
            }
        }
        // Default env-root profiles are already ephemeral after resolution.
        // Report that exclusion before synchronous filesystem/certificate work;
        // their ordinary session behavior remains unchanged.
        if std::env::var_os("SSL_CERT_FILE").is_some() || std::env::var_os("SSL_CERT_DIR").is_some()
        {
            return Ok(Preparation::Ephemeral);
        }
        let session = self.session(Options::default(), deadline, cancel.clone())?;
        self.prepare_resolved(session, deadline, cancel).await
    }
    async fn prepare_resolved(
        &self,
        session: Session,
        deadline: tokio::time::Instant,
        cancel: Cancellation,
    ) -> Result<Preparation> {
        if session.injected.is_some() {
            return Ok(Preparation::Injected);
        }
        if !self
            .state
            .lock()
            .unwrap()
            .pools
            .values()
            .any(|p| Arc::ptr_eq(p, &session.pool))
        {
            return Ok(Preparation::Ephemeral);
        }
        let work = async {
            self.state.lock().unwrap().expire_idle();
            if session
                .pool
                .slots
                .lock()
                .unwrap()
                .iter()
                .any(|(_, entry)| entry.as_ref().is_some_and(|e| e.origin.is_empty()))
            {
                return Ok(Preparation::Prepared);
            }
            let prepared = checkout(
                session.pool.clone(),
                self.builders.clone(),
                self.state.clone(),
                self.connections.clone(),
                String::new(),
            )
            .await?;
            // Return through the ordinary slot/retention guard, with an empty
            // origin meaning an unused client which has never opened a socket.
            drop(prepared);
            let retained = session
                .pool
                .slots
                .lock()
                .unwrap()
                .iter()
                .any(|(_, entry)| entry.as_ref().is_some_and(|e| e.origin.is_empty()));
            Ok(if retained {
                Preparation::Prepared
            } else {
                Preparation::CapacityUnavailable
            })
        };
        tokio::select! { biased;
            _=self.stop.cancelled()=>Err(Error::Closed),
            _=cancel.cancelled()=>Err(Error::Cancelled),
            _=tokio::time::sleep_until(deadline)=>Err(Error::Deadline),
            result=work=>result,
        }
    }
    /// Cancels in-flight operations; existing session handles reject further work.
    /// Synchronous drop closes owned idle clients; it never closes injected transports.
    pub fn shutdown(&self) {
        let mut s = self.state.lock().unwrap();
        s.closed = true;
        self.stop.cancel();
        self.connections.close();
        self.idle.close();
        self.decoder.close();
        self.builders.close();
        for p in s.all_pools.drain(..).filter_map(|p| p.upgrade()) {
            p.close();
        }
        s.pools.clear();
    }
    /// Stop transport work and await completion of every owned blocking decoder.
    /// Caller-owned search tasks still belong to the caller's supervisor.
    pub async fn shutdown_and_wait(&self) {
        self.shutdown();
        self.decoder.drain().await;
        self.builders.drain().await;
        self.builders.event("shutdown_drained");
    }
    pub fn retained_pools(&self) -> usize {
        self.state.lock().unwrap().pools.len()
    }
}
impl Drop for AdapterHttp {
    fn drop(&mut self) {
        self.builders.event("owner_dropped");
        self.shutdown();
    }
}
/// Bounded test metadata, not a transport hook or alternate deadline policy.
#[cfg(test)]
#[derive(Clone)]
pub(crate) struct SystemOperationTrace(Arc<Mutex<Vec<SystemOperationObservation>>>);
#[cfg(test)]
#[derive(Clone)]
pub(crate) struct SystemOperationObservation {
    session: Arc<()>,
    pool: Weak<Pool>,
    pub(crate) deadline: Instant,
    pub(crate) observed_at: Instant,
    pub(crate) preceding_requests: usize,
}
#[cfg(test)]
impl SystemOperationObservation {
    pub(crate) fn same_session(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.session, &other.session)
    }
    pub(crate) fn same_pool(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.pool, &other.pool)
    }
}
#[cfg(test)]
impl SystemOperationTrace {
    fn record(&self, session: &Session) -> Result<()> {
        let mut observations = self.0.lock().unwrap();
        if observations.len() >= 2 {
            return Err(Error::RequestLimit);
        }
        observations.push(SystemOperationObservation {
            session: session
                .replay_identity
                .as_ref()
                .expect("observed Session owns identity")
                .clone(),
            pool: Arc::downgrade(&session.pool),
            deadline: session.deadline.into_std(),
            observed_at: session.clock.now(),
            preceding_requests: session.requests,
        });
        Ok(())
    }
    pub(crate) fn observations(&self) -> Vec<SystemOperationObservation> {
        self.0.lock().unwrap().clone()
    }
}
pub struct Session {
    clock: DeadlineClock,
    #[cfg(test)]
    replay_endpoint: Option<url::Url>,
    #[cfg(test)]
    operation_trace: Option<SystemOperationTrace>,
    #[cfg(test)]
    replay_identity: Option<Arc<()>>,
    state: Arc<Mutex<State>>,
    connections: Arc<Semaphore>,
    decoder: Arc<Decoder>,
    builders: Arc<ClientBuilders>,
    pool: Arc<Pool>,
    injected: Option<Arc<dyn Transport>>,
    jar: Jar,
    pub headers: HeaderMap,
    options: Options,
    deadline: tokio::time::Instant,
    cancel: Cancellation,
    stop: Cancellation,
    requests: usize,
}
impl Session {
    pub(crate) fn deadline_elapsed(&self, deadline: Instant) -> bool {
        self.clock.now() >= deadline
    }
    /// Set the caller-owned deadline for the next operation without replacing
    /// this session's cookies, request budget, cancellation, stop state or pool.
    /// Callers with an overall search budget must cap this at its remaining
    /// deadline; this method does not grant a new end-to-end budget.
    pub fn set_deadline(&mut self, deadline: tokio::time::Instant) {
        self.deadline = deadline;
    }
    pub async fn execute(&mut self, mut request: Request) -> Result<Response> {
        if *self.stop.0.borrow() {
            return Err(Error::Closed);
        }
        if *self.cancel.0.borrow() {
            return Err(Error::Cancelled);
        }
        #[cfg(test)]
        if let Some(trace) = &self.operation_trace {
            trace.record(self)?;
        }
        // Timer readiness may lag the owner's clock (for example within a
        // Tokio timer tick). Reject expired startup before request accounting,
        // transport dispatch or client checkout, retaining stop/cancel priority.
        if self.clock.now() >= self.deadline.into_std() {
            return Err(Error::Deadline);
        }
        let cancel = self.cancel.clone();
        let stop = self.stop.clone();
        let deadline = self.deadline;
        let clock = self.clock.clone();
        for (name, value) in &self.headers {
            if !request.headers.contains_key(name) {
                request.headers.insert(name, value.clone());
            }
        }
        tokio::select! { biased;
            _=stop.cancelled()=>Err(Error::Closed),
            _=cancel.cancelled()=>Err(Error::Cancelled),
            _=clock.sleep_until(deadline.into_std())=>Err(Error::Deadline),
            result=self.execute_inner(request)=>result,
        }
    }
    async fn execute_inner(&mut self, mut request: Request) -> Result<Response> {
        let mut redirects = 0;
        loop {
            #[cfg(test)]
            if let Some(endpoint) = &self.replay_endpoint
                && (request.url.scheme() != endpoint.scheme()
                    || request.url.host() != endpoint.host()
                    || request.url.port_or_known_default() != endpoint.port_or_known_default())
            {
                return Err(Error::InvalidRequest);
            }
            if self.requests >= self.options.bounds.requests {
                return Err(Error::RequestLimit);
            }
            if request.body.len() > self.options.bounds.request_bytes {
                return Err(Error::RequestBodyLimit);
            }
            if !matches!(request.url.scheme(), "http" | "https") {
                return Err(Error::InvalidRequest);
            }
            self.requests += 1;
            let mut outbound = request.clone();
            if !outbound.headers.contains_key(header::COOKIE)
                && let Some(c) = self.jar.cookies(&request.url)
            {
                outbound.headers.insert(header::COOKIE, c);
            }
            let mut response = if let Some(t) = &self.injected {
                t.exchange(outbound).await?
            } else {
                self.network(outbound).await?
            };
            if response.body.len() > self.options.bounds.wire_bytes {
                return Err(Error::WireBodyLimit);
            }
            let mut cookies = response.headers.get_all(header::SET_COOKIE).iter();
            self.jar.set_cookies(&mut cookies, &request.url);
            let headers = response.headers.clone();
            let body = std::mem::take(&mut response.body);
            let bound = self.options.bounds.decoded_bytes;
            response.body = self
                .decoder
                .decode(
                    headers,
                    body,
                    bound,
                    self.cancel.clone(),
                    self.stop.clone(),
                    self.deadline,
                    self.clock.clone(),
                )
                .await?;
            if self.options.follow_redirects
                && matches!(response.status, 301 | 302 | 303 | 307 | 308)
                && let Some(location) = response.headers.get(header::LOCATION)
            {
                if redirects >= self.options.bounds.redirects {
                    return Err(Error::RedirectLimit);
                }
                redirects += 1;
                let next = request
                    .url
                    .join(location.to_str().map_err(|_| Error::InvalidResponse)?)
                    .map_err(|_| Error::InvalidResponse)?;
                if request.url.origin() != next.origin() {
                    request.headers.remove(header::AUTHORIZATION);
                    request.headers.remove(header::PROXY_AUTHORIZATION);
                    request.headers.remove(header::COOKIE);
                    request.headers.remove(header::HOST);
                }
                // HTTPX chooses GET for every non-HEAD 302/303, but only
                // changes the stream/framing if the original method differs.
                request.headers.remove(header::COOKIE);
                if request.method != Method::GET
                    && ((matches!(response.status, 302 | 303) && request.method != Method::HEAD)
                        || (response.status == 301 && request.method == Method::POST))
                {
                    request.method = Method::GET;
                    request.body.clear();
                    for h in [header::CONTENT_LENGTH, header::TRANSFER_ENCODING] {
                        request.headers.remove(h);
                    }
                }
                request.url = next;
                continue;
            }
            return Ok(response);
        }
    }
    async fn network(&self, mut r: Request) -> Result<Response> {
        // Route-specific and ephemeral clients share the adapter's admission
        // budget. Keep ownership until the response body has been consumed.
        let checkout = checkout(
            self.pool.clone(),
            self.builders.clone(),
            self.state.clone(),
            self.connections.clone(),
            r.url.origin().ascii_serialization(),
        )
        .await?;
        r.headers
            .entry(header::ACCEPT_ENCODING)
            .or_insert(HeaderValue::from_static("gzip, deflate"));
        let mut response = checkout
            .client
            .as_ref()
            .unwrap()
            .request(r.method, r.url)
            .headers(r.headers)
            .body(r.body)
            .send()
            .await
            .map_err(classify)?;
        let status = response.status().as_u16();
        let url = response.url().clone();
        let headers = response.headers().clone();
        if self
            .options
            .identity_announced_bytes
            .is_some_and(|limit| identity_announced_oversize(&headers, limit))
        {
            return Err(Error::WireBodyLimit);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(classify)? {
            if chunk.len() > self.options.bounds.wire_bytes.saturating_sub(body.len()) {
                return Err(Error::WireBodyLimit);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(Response {
            status,
            url,
            headers,
            body,
        })
    }
}
fn classify(e: reqwest::Error) -> Error {
    if e.is_timeout() {
        Error::Deadline
    } else {
        Error::Network
    }
}
const DECODE_CONCURRENCY: usize = 4;
#[derive(Default)]
struct DecodeState {
    closed: bool,
    active: usize,
}
#[cfg(test)]
type TestDecodeGate = Arc<dyn Fn() + Send + Sync>;
struct Decoder {
    #[cfg(test)]
    before_work: Mutex<Option<TestDecodeGate>>,
    slots: Arc<Semaphore>,
    state: Mutex<DecodeState>,
    done: Notify,
}
impl Default for Decoder {
    fn default() -> Self {
        Self {
            #[cfg(test)]
            before_work: Mutex::new(None),
            slots: Arc::new(Semaphore::new(DECODE_CONCURRENCY)),
            state: Mutex::new(DecodeState::default()),
            done: Notify::new(),
        }
    }
}
struct DecodeWorker(Arc<Decoder>);
impl Drop for DecodeWorker {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        state.active -= 1;
        if state.active == 0 {
            self.0.done.notify_waiters();
        }
    }
}
impl Decoder {
    async fn run<T: Send + 'static>(
        self: &Arc<Self>,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T> {
        let permit = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| Error::Closed)?;
        {
            let mut state = self.state.lock().unwrap();
            if state.closed {
                return Err(Error::Closed);
            }
            state.active += 1;
        }
        #[cfg(test)]
        let before_work = self.before_work.lock().unwrap().clone();
        let worker = DecodeWorker(self.clone());
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _worker = worker;
            #[cfg(test)]
            if let Some(before_work) = before_work {
                before_work();
            }
            work()
        })
        .await
        .map_err(|_| Error::InvalidResponse)
    }
    // Keep decoding ownership and the same operation clock explicit at this boundary.
    #[allow(clippy::too_many_arguments)]
    async fn decode(
        self: &Arc<Self>,
        headers: HeaderMap,
        body: Vec<u8>,
        bound: usize,
        cancel: Cancellation,
        stop: Cancellation,
        deadline: tokio::time::Instant,
        clock: DeadlineClock,
    ) -> Result<Vec<u8>> {
        self.run(move || {
            decode_with_clock(
                &headers,
                &body,
                bound,
                &cancel,
                &stop,
                deadline.into_std(),
                &clock,
            )
        })
        .await?
    }
    fn close(&self) {
        self.state.lock().unwrap().closed = true;
        self.slots.close();
    }
    async fn drain(&self) {
        loop {
            let notified = self.done.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.state.lock().unwrap().active == 0 {
                return;
            }
            notified.await;
        }
    }
}
fn bounded_read(
    mut reader: impl Read,
    bound: usize,
    cancel: &Cancellation,
    stop: &Cancellation,
    deadline: Instant,
    clock: &DeadlineClock,
) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut chunk = [0; 64 * 1024];
    loop {
        if *stop.0.borrow() {
            return Err(Error::Closed);
        }
        if *cancel.0.borrow() {
            return Err(Error::Cancelled);
        }
        if clock.now() >= deadline {
            return Err(Error::Deadline);
        }
        let size = reader
            .read(&mut chunk)
            .map_err(|_| Error::InvalidResponse)?;
        if size == 0 {
            break;
        }
        if size > bound.saturating_sub(result.len()) {
            return Err(Error::DecodedBodyLimit);
        }
        result.extend_from_slice(&chunk[..size]);
    }
    Ok(result)
}
fn decode_with_clock(
    headers: &HeaderMap,
    body: &[u8],
    bound: usize,
    cancel: &Cancellation,
    stop: &Cancellation,
    deadline: Instant,
    clock: &DeadlineClock,
) -> Result<Vec<u8>> {
    let mut encodings = Vec::new();
    for value in headers.get_all(header::CONTENT_ENCODING) {
        let value = value.to_str().map_err(|_| Error::InvalidResponse)?;
        encodings.extend(
            value
                .split(',')
                .map(|value| value.trim().to_ascii_lowercase()),
        );
    }
    if encodings.is_empty() {
        encodings.push("identity".into());
    }
    let mut result = body.to_vec();
    for encoding in encodings.into_iter().rev() {
        result = match encoding.as_str() {
            "identity" | "" => {
                bounded_read(result.as_slice(), bound, cancel, stop, deadline, clock)?
            }
            "gzip" => bounded_read(
                flate2::read::MultiGzDecoder::new(result.as_slice()),
                bound,
                cancel,
                stop,
                deadline,
                clock,
            )?,
            "deflate" => match bounded_read(
                flate2::read::ZlibDecoder::new(result.as_slice()),
                bound,
                cancel,
                stop,
                deadline,
                clock,
            ) {
                Err(Error::InvalidResponse) => bounded_read(
                    flate2::read::DeflateDecoder::new(result.as_slice()),
                    bound,
                    cancel,
                    stop,
                    deadline,
                    clock,
                )?,
                other => other?,
            },
            _ => return Err(Error::UnsupportedEncoding),
        };
    }
    Ok(result)
}
#[cfg(test)]
fn decode(
    headers: &HeaderMap,
    body: &[u8],
    bound: usize,
    cancel: &Cancellation,
    stop: &Cancellation,
    deadline: Instant,
) -> Result<Vec<u8>> {
    decode_with_clock(
        headers,
        body,
        bound,
        cancel,
        stop,
        deadline,
        &DeadlineClock::default(),
    )
}
/// Offline exchange replay exercises the exact session policy without parsing bodies.
/// Adapters own HTML/XML/JSON parsing and provider-specific status classification.
pub struct Replay {
    exchanges: Mutex<std::collections::VecDeque<(Request, Response)>>,
}
impl Replay {
    pub fn new(exchanges: Vec<(Request, Response)>) -> Self {
        Self {
            exchanges: Mutex::new(exchanges.into()),
        }
    }
    pub fn remaining(&self) -> usize {
        self.exchanges.lock().unwrap().len()
    }
}
impl Transport for Replay {
    fn exchange(&self, r: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let (expected, response) = self
                .exchanges
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(Error::RequestLimit)?;
            if r.method != expected.method
                || r.url != expected.url
                || r.headers != expected.headers
                || r.body != expected.body
            {
                return Err(Error::InvalidRequest);
            }
            Ok(response)
        })
    }
}

/// Provider-directed round-robin routing. Reporting failure never resends a
/// request. Caller supplies monotonic time so cooldowns are deterministic.
pub struct ProxyRotation {
    proxies: Vec<String>,
    dynamic: Option<String>,
    next: usize,
    cooldown: Duration,
    failures: HashMap<String, u32>,
    until: HashMap<String, Instant>,
}
impl ProxyRotation {
    pub fn new(proxies: Vec<String>, dynamic: Option<String>, cooldown: Duration) -> Self {
        Self {
            proxies,
            dynamic,
            next: 0,
            cooldown,
            failures: HashMap::new(),
            until: HashMap::new(),
        }
    }
    pub fn select(&mut self, now: Instant) -> Option<String> {
        if let Some(dynamic) = &self.dynamic
            && !dynamic.is_empty()
        {
            return Some(dynamic.clone());
        }
        if self.proxies.is_empty() {
            return None;
        }
        for _ in 0..self.proxies.len() {
            let proxy = self.proxies[self.next].clone();
            self.next = (self.next + 1) % self.proxies.len();
            if !self.until.get(&proxy).is_some_and(|until| now < *until) {
                return Some(proxy);
            }
        }
        let proxy = self.proxies[self.next].clone();
        self.next = (self.next + 1) % self.proxies.len();
        self.until.remove(&proxy);
        Some(proxy)
    }
    pub fn failure(&mut self, proxy: &str, now: Instant) {
        if !self.proxies.iter().any(|p| p == proxy) {
            return;
        }
        let count = self.failures.entry(proxy.to_owned()).or_default();
        *count = count.saturating_add(1);
        self.until.insert(
            proxy.to_owned(),
            now + self.cooldown.saturating_mul(if *count < 3 { 1 } else { 3 }),
        );
    }
    pub fn success(&mut self, proxy: &str) {
        self.failures.remove(proxy);
        self.until.remove(proxy);
    }
    pub fn available(&self, now: Instant) -> usize {
        self.proxies
            .iter()
            .filter(|p| !self.until.get(*p).is_some_and(|until| now < *until))
            .count()
    }
}

#[cfg(test)]
mod decode_tests {
    use super::*;
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn decoder_budget_remains_charged_after_waiter_abort_and_drain_joins_workers() {
        let http = Arc::new(AdapterHttp::default());
        let decoder = http.decoder.clone();
        let release = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let started = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..DECODE_CONCURRENCY {
            let decoder = decoder.clone();
            let release = release.clone();
            let started = started.clone();
            tasks.push(tokio::spawn(async move {
                decoder
                    .run(move || {
                        started.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let (lock, cv) = &*release;
                        let mut released = lock.lock().unwrap();
                        while !*released {
                            released = cv.wait(released).unwrap();
                        }
                    })
                    .await
            }));
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while started.load(std::sync::atomic::Ordering::SeqCst) != DECODE_CONCURRENCY {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        for task in tasks {
            task.abort();
            let _ = task.await;
        }
        assert_eq!(decoder.state.lock().unwrap().active, DECODE_CONCURRENCY);
        assert_eq!(decoder.slots.available_permits(), 0);
        assert!(
            tokio::time::timeout(
                Duration::from_millis(20),
                decoder.run(|| panic!("must not exceed worker bound"))
            )
            .await
            .is_err()
        );
        let owner = http.clone();
        let mut shutdown = tokio::spawn(async move {
            owner.shutdown_and_wait().await;
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut shutdown)
                .await
                .is_err()
        );
        {
            let (lock, cv) = &*release;
            *lock.lock().unwrap() = true;
            cv.notify_all();
        }
        tokio::time::timeout(Duration::from_secs(2), shutdown)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(decoder.state.lock().unwrap().active, 0);
        assert_eq!(decoder.run(|| ()).await.unwrap_err(), Error::Closed);
    }
    #[test]
    fn drop_closes_decode_admission_without_claiming_thread_join() {
        let http = AdapterHttp::default();
        let decoder = http.decoder.clone();
        drop(http);
        assert!(decoder.slots.is_closed());
        assert!(decoder.state.lock().unwrap().closed);
    }
    #[test]
    fn decoding_observes_cancel_shutdown_and_expired_budget() {
        let cancel = Cancellation::default();
        let stop = Cancellation::default();
        let deadline = Instant::now() + Duration::from_secs(2);
        cancel.cancel();
        assert_eq!(
            decode(&HeaderMap::new(), b"body", 32, &cancel, &stop, deadline).unwrap_err(),
            Error::Cancelled
        );
        stop.cancel();
        assert_eq!(
            decode(&HeaderMap::new(), b"body", 32, &cancel, &stop, deadline).unwrap_err(),
            Error::Closed
        );
        assert_eq!(
            decode(
                &HeaderMap::new(),
                b"body",
                32,
                &Cancellation::default(),
                &Cancellation::default(),
                Instant::now()
            )
            .unwrap_err(),
            Error::Deadline
        );
    }
    #[test]
    fn decoding_raw_zlib_stacked_and_unknown_encodings() {
        use std::io::Write;
        let body = b"bounded publication body";
        let mut raw =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        raw.write_all(body).unwrap();
        let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        zlib.write_all(body).unwrap();
        let zlib = zlib.finish().unwrap();
        let mut stacked = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        stacked.write_all(&zlib).unwrap();
        for (encoding, encoded) in [
            ("deflate", raw.finish().unwrap()),
            ("deflate", zlib),
            (" DEFLATE, GZIP ", stacked.finish().unwrap()),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_ENCODING,
                HeaderValue::from_str(encoding).unwrap(),
            );
            assert_eq!(
                decode(
                    &headers,
                    &encoded,
                    1024,
                    &Cancellation::default(),
                    &Cancellation::default(),
                    Instant::now() + Duration::from_secs(1)
                )
                .unwrap(),
                body
            );
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_ENCODING,
            HeaderValue::from_static("unknown"),
        );
        assert_eq!(
            decode(
                &headers,
                body,
                1024,
                &Cancellation::default(),
                &Cancellation::default(),
                Instant::now() + Duration::from_secs(1)
            )
            .unwrap_err(),
            Error::UnsupportedEncoding
        );
    }
}

#[cfg(test)]
mod f15_header_tests {
    use super::*;
    #[test]
    fn actual_source_streamed_header_cases_replay_early_admission() {
        let corpus: ssx_json::Value = ssx_json::from_slice(include_bytes!(
            "../../../docs/rust-rewrite/f15/headers.json"
        ))
        .unwrap();
        for case in corpus["cases"].as_array().unwrap() {
            let mut headers = HeaderMap::new();
            for pair in case["headers"].as_array().unwrap() {
                let name =
                    header::HeaderName::from_bytes(pair[0].as_str().unwrap().as_bytes()).unwrap();
                headers.append(
                    name,
                    HeaderValue::from_str(pair[1].as_str().unwrap()).unwrap(),
                );
            }
            assert_eq!(
                identity_announced_oversize(&headers, 2_000_000),
                case["body_reads"] == 0,
                "{}",
                case["name"]
            );
        }
    }
    #[test]
    fn opt_in_identity_length_limit_preserves_encoding_and_python_integer_rules() {
        assert_eq!(Options::default().identity_announced_bytes, None);
        let mut h = HeaderMap::new();
        for (text, expected) in [
            ("2000000", false),
            ("2000001", true),
            ("+2_000_001", true),
            ("-2000001", false),
            ("invalid", false),
            ("2000001x", false),
            ("++2000001", false),
            ("2000001, 2000001", false),
            ("999999999999999999999999999999", true),
        ] {
            h.insert(header::CONTENT_LENGTH, HeaderValue::from_str(text).unwrap());
            assert_eq!(
                identity_announced_oversize(&h, 2_000_000),
                expected,
                "{text}"
            );
        }
        h.insert(header::CONTENT_LENGTH, HeaderValue::from_static("2000001"));
        for (zeros, expected) in [(4293, true), (4294, false)] {
            h.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&format!("{}2000001", "0".repeat(zeros))).unwrap(),
            );
            assert_eq!(identity_announced_oversize(&h, 2_000_000), expected);
        }
        h.insert(header::CONTENT_LENGTH, HeaderValue::from_static("2000001"));
        h.insert(
            header::CONTENT_ENCODING,
            HeaderValue::from_static("identity"),
        );
        assert!(identity_announced_oversize(&h, 2_000_000));
        h.append(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        assert!(!identity_announced_oversize(&h, 2_000_000));
        h.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        assert!(!identity_announced_oversize(&h, 2_000_000));
    }
}

#[cfg(test)]
#[path = "../tests/support/a10_http_clock.rs"]
mod a10_owner_clock_tests;

#[cfg(test)]
mod pypi_operation_timeout_tests {
    use super::AdapterHttp;
    use crate::adapters::{AllowAllAdmission, PackageConfig, PyPIConfig, register_packages};
    use ssx_domain::{EngineStatus, SearchRequest};
    use ssx_registry::Registry;
    use std::{sync::Arc, time::Duration};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn run_pypi_per_operation_timeout_scenario() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (owner, driver) = AdapterHttp::clocked_real_pool_owner();
        let http = Arc::new(owner);
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (index, body) in [
                "",
                "<a href='/simple/python-one/'>x</a><a href='/simple/python-two/'>y</a>",
                "{\"info\":{}}",
                "{\"info\":{}}",
            ]
            .iter()
            .enumerate()
            {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0; 4096];
                let mut read = 0;
                while !buf[..read].windows(4).any(|window| window == b"\r\n\r\n") {
                    assert!(read < buf.len(), "bounded HTTP request headers");
                    let count = stream.read(&mut buf[read..]).await.unwrap();
                    assert!(count > 0, "loopback request closed before its headers");
                    read += count;
                }
                let request = std::str::from_utf8(&buf[..read])
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap()
                    .to_owned();
                requests.push(request);
                if index > 0 {
                    // Model a logical 180 ms server operation without letting
                    // host scheduling consume part of the HTTP timeout budget.
                    driver.advance_to(Duration::from_millis(180 * index as u64));
                }
                let status = if index == 0 { 404 } else { 200 };
                let head = format!(
                    "HTTP/1.1 {status} Response\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).await.unwrap();
                stream.write_all(body.as_bytes()).await.unwrap();
            }
            requests
        });

        let mut registry = Registry::builtins().unwrap();
        register_packages(
            &mut registry,
            http.clone(),
            Arc::new(AllowAllAdmission),
            PackageConfig {
                pypi: PyPIConfig {
                    base_url: base,
                    timeout_ms: 400,
                    max_results: 2,
                },
                ..PackageConfig::default()
            },
        )
        .unwrap();
        let response = registry
            .instantiate("pypi")
            .unwrap()
            .search(&SearchRequest {
                query: "python".into(),
                ..SearchRequest::default()
            })
            .await;
        assert_eq!(response.status, EngineStatus::Ok);
        assert_eq!(
            response
                .results
                .iter()
                .map(|result| result.title.as_str())
                .collect::<Vec<_>>(),
            ["python-one ", "python-two "]
        );
        assert_eq!(
            server.await.unwrap(),
            [
                "GET /pypi/python/json HTTP/1.1",
                "GET /simple/ HTTP/1.1",
                "GET /pypi/python-one/json HTTP/1.1",
                "GET /pypi/python-two/json HTTP/1.1",
            ]
        );
        {
            let state = http.state.lock().unwrap();
            assert!(state.used, "the caller-owned HTTP pool was exercised");
            assert!(state.injected.is_none(), "the real transport was retained");
        }
        http.shutdown();
    }

    #[tokio::test]
    async fn pypi_uses_fresh_per_operation_timeouts_on_the_caller_pool() {
        tokio::time::timeout(
            Duration::from_secs(35),
            run_pypi_per_operation_timeout_scenario(),
        )
        .await
        .expect("loopback timeout scenario exceeded its wall-clock watchdog");
    }
}

#[cfg(test)]
#[path = "../tests/support/f10_peer_timeout.rs"]
mod f10_peer_timeout_tests;

#[cfg(test)]
#[path = "../tests/support/http_expired_startup.rs"]
mod expired_startup_tests;

#[cfg(test)]
#[path = "../tests/support/a04_broad_api.rs"]
mod a04_broad_api;

#[cfg(test)]
mod loopback_timeout_replay_controls {
    use super::*;
    #[test]
    fn manual_network_owner_rejects_remote_and_service_context() {
        for base in [
            "https://127.0.0.1:1234",
            "http://example.com",
            "http://localhost:1234",
            "http://user@127.0.0.1:1234",
        ] {
            assert!(AdapterHttp::loopback_timeout_replay(base, None).is_err());
            assert!(AdapterHttp::wikipedia_operation_replay(base, None).is_err());
        }
        assert!(
            AdapterHttp::loopback_timeout_replay("http://127.0.0.1:1234", Some(Instant::now()))
                .is_err()
        );
        assert!(AdapterHttp::loopback_timeout_replay("http://127.0.0.1:1234", None).is_ok());
        assert!(
            AdapterHttp::wikipedia_operation_replay("http://127.0.0.1:1234", Some(Instant::now()))
                .is_err()
        );
        assert!(AdapterHttp::wikipedia_operation_replay("http://127.0.0.1:1234", None).is_ok());
    }
    #[tokio::test]
    async fn manual_network_session_rejects_different_endpoint_before_wire() {
        let (owner, driver) =
            AdapterHttp::loopback_timeout_replay("http://127.0.0.1:1234", None).unwrap();
        let mut session = owner
            .session(
                Options::default(),
                (driver.now() + std::time::Duration::from_secs(1)).into(),
                Cancellation::default(),
            )
            .unwrap();
        assert!(matches!(
            session
                .execute(Request::get("http://127.0.0.1:1235/").unwrap())
                .await,
            Err(Error::InvalidRequest)
        ));
        owner.shutdown_and_wait().await;
        let (owner, trace) =
            AdapterHttp::wikipedia_operation_replay("http://127.0.0.1:1234", None).unwrap();
        let mut session = owner
            .session(
                Options::default(),
                (Instant::now() + Duration::from_secs(1)).into(),
                Cancellation::default(),
            )
            .unwrap();
        for _ in 0..2 {
            assert!(matches!(
                session
                    .execute(Request::get("http://127.0.0.1:1235/").unwrap())
                    .await,
                Err(Error::InvalidRequest)
            ));
        }
        assert!(matches!(
            session
                .execute(Request::get("http://127.0.0.1:1235/").unwrap())
                .await,
            Err(Error::RequestLimit)
        ));
        assert_eq!(trace.observations().len(), 2);
        owner.shutdown_and_wait().await;
    }
}

#[cfg(test)]
#[path = "../tests/support/default_ca_environment.rs"]
mod default_ca_environment;
#[cfg(test)]
pub(crate) use default_ca_environment::default_ca_test_environment;

#[cfg(test)]
mod client_initialization_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn block_builder(http: &AdapterHttp) -> (oneshot::Receiver<()>, std::sync::mpsc::Sender<()>) {
        let (started_tx, started_rx) = oneshot::channel();
        let started = Mutex::new(Some(started_tx));
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release = Mutex::new(release_rx);
        *http.builders.build.lock().unwrap() = Some(Arc::new(move |_| {
            started.lock().unwrap().take().unwrap().send(()).unwrap();
            release.lock().unwrap().recv().unwrap();
            // Only the scheduling control changes: the normal builder still
            // produces an independent client; no HTTP or real proxy is used.
            build_client(&Options {
                trust_env: false,
                ..Options::default()
            })
        }));
        (started_rx, release_tx)
    }
    fn busy(http: &AdapterHttp) -> usize {
        http.state
            .lock()
            .unwrap()
            .all_pools
            .iter()
            .filter_map(Weak::upgrade)
            .map(|p| {
                p.slots
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|(busy, _)| *busy)
                    .count()
            })
            .sum()
    }
    #[tokio::test]
    async fn deadline_keeps_builder_permits_and_slot_until_actual_completion_without_dispatch() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let http = Arc::new(AdapterHttp::default());
        let (started, release) = block_builder(&http);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        let mut session = http
            .session(Options::default(), deadline, Cancellation::default())
            .unwrap();
        let pool = session.pool.clone();
        let request = Request::get(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let operation = tokio::spawn(async move { session.execute(request).await });
        tokio::time::timeout(Duration::from_secs(5), started)
            .await
            .unwrap()
            .unwrap();
        // A deterministic parent Tokio deadline after observed worker startup;
        // actual A08 fifty-millisecond wall-clock control stays separate.
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(61)).await;
        assert!(tokio::time::Instant::now() > deadline);
        // Resume before the qualification timeout: blocked workers inhibit
        // paused-clock auto-advance, but cannot stop this real running clock.
        tokio::time::resume();
        assert_eq!(http.connections.available_permits(), 19);
        assert_eq!(pool.permits.available_permits(), 19);
        assert_eq!(busy(&http), 1);
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), operation)
                .await
                .unwrap()
                .unwrap()
                .unwrap_err(),
            Error::Deadline,
        );
        assert_eq!(http.connections.available_permits(), 19);
        assert_eq!(pool.permits.available_permits(), 19);
        assert_eq!(busy(&http), 1);
        assert!(
            tokio::time::timeout(Duration::from_millis(1), listener.accept())
                .await
                .is_err()
        );
        let owner = http.clone();
        let drain = tokio::spawn(async move { owner.shutdown_and_wait().await });
        tokio::task::yield_now().await;
        assert!(!drain.is_finished());
        release.send(()).unwrap();
        drain.await.unwrap();
        assert_eq!(busy(&http), 0);
        // Closed semaphore still reports released ownership after exact join.
        assert_eq!(http.connections.available_permits(), 20);
        assert_eq!(pool.permits.available_permits(), 20);
        assert!(pool.slots.lock().unwrap().iter().all(|(busy, _)| !busy));
        assert!(http.state.lock().unwrap().pools.is_empty());
    }
    #[tokio::test]
    async fn waiter_abort_does_not_release_blocking_builder_ownership_or_retain_late_client() {
        let http = Arc::new(AdapterHttp::default());
        let (started, release) = block_builder(&http);
        let mut session = http
            .session(
                Options::default(),
                tokio::time::Instant::now() + Duration::from_secs(10),
                Cancellation::default(),
            )
            .unwrap();
        let operation = tokio::spawn(async move {
            session
                .execute(Request::get("http://127.0.0.1:1/").unwrap())
                .await
        });
        tokio::time::timeout(Duration::from_secs(5), started)
            .await
            .unwrap()
            .unwrap();
        operation.abort();
        assert!(operation.await.unwrap_err().is_cancelled());
        assert_eq!(http.connections.available_permits(), 19);
        assert_eq!(busy(&http), 1);
        let owner = http.clone();
        let first_drain = tokio::spawn(async move { owner.builders.drain().await });
        tokio::task::yield_now().await;
        assert!(!first_drain.is_finished());
        first_drain.abort();
        assert!(first_drain.await.unwrap_err().is_cancelled());
        assert_eq!(http.builders.state.lock().unwrap().jobs.len(), 1);
        release.send(()).unwrap();
        http.builders.drain().await;
        assert_eq!(http.connections.available_permits(), 20);
        assert_eq!(busy(&http), 0);
        assert_eq!(http.idle.available_permits(), 10);
        http.shutdown_and_wait().await;
    }
    #[tokio::test]
    async fn prepared_default_client_is_moved_once_and_reuses_existing_retention_and_expiry() {
        if default_ca_test_environment(
            "http::client_initialization_tests::prepared_default_client_is_moved_once_and_reuses_existing_retention_and_expiry",
        ) {
            return;
        }
        let http = AdapterHttp::default();
        *http.builders.build.lock().unwrap() = Some(Arc::new(|_| {
            build_client(&Options {
                trust_env: false,
                ..Options::default()
            })
        }));
        let result = http
            .prepare_default(
                tokio::time::Instant::now() + Duration::from_secs(2),
                Cancellation::default(),
            )
            .await
            .unwrap();
        // Launcher must provide an ordinary default-profile environment;
        // a nonordinary host profile fails visibly rather than skipping controls.
        assert_eq!(result, Preparation::Prepared);
        assert_eq!(http.idle.available_permits(), 9);
        let pool = http.state.lock().unwrap().pools.get(&None).unwrap().clone();
        assert_eq!(
            pool.slots
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, i)| i.as_ref().is_some_and(|i| i.origin.is_empty()))
                .count(),
            1
        );
        // Unused prepared clients use the same five-second idle expiry.
        for (_, entry) in pool.slots.lock().unwrap().iter_mut() {
            if let Some(entry) = entry {
                entry.used = Instant::now() - Duration::from_secs(5);
            }
        }
        pool.expire_idle();
        assert_eq!(http.idle.available_permits(), 10);
        assert_eq!(
            http.prepare_default(
                tokio::time::Instant::now() + Duration::from_secs(2),
                Cancellation::default()
            )
            .await
            .unwrap(),
            Preparation::Prepared
        );
        assert_eq!(http.idle.available_permits(), 9);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 8192];
            assert!(stream.read(&mut bytes).await.unwrap() > 0);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
        });
        // Any second construction is an actual regression, not a timing check.
        *http.builders.build.lock().unwrap() =
            Some(Arc::new(|_| panic!("prepared client was not moved")));
        let mut session = http
            .session(
                Options::default(),
                tokio::time::Instant::now() + Duration::from_secs(2),
                Cancellation::default(),
            )
            .unwrap();
        assert_eq!(
            session
                .execute(Request::get(&format!("http://{address}/")).unwrap())
                .await
                .unwrap()
                .body,
            b"ok"
        );
        server.await.unwrap();
        assert_eq!(http.idle.available_permits(), 9);
        assert!(
            pool.slots
                .lock()
                .unwrap()
                .iter()
                .all(|(_, i)| i.as_ref().is_none_or(|i| !i.origin.is_empty()))
        );
        for (_, idle) in pool.slots.lock().unwrap().iter_mut() {
            if let Some(idle) = idle {
                idle.used = Instant::now() - Duration::from_secs(5);
            }
        }
        pool.expire_idle();
        assert_eq!(http.idle.available_permits(), 10);
        http.shutdown_and_wait().await;
    }
    #[tokio::test]
    async fn builder_error_releases_both_leases_and_reserved_slot_without_idle_client() {
        if default_ca_test_environment(
            "http::client_initialization_tests::builder_error_releases_both_leases_and_reserved_slot_without_idle_client",
        ) {
            return;
        }
        let http = AdapterHttp::default();
        *http.builders.build.lock().unwrap() = Some(Arc::new(|_| Err(Error::InvalidRequest)));
        assert_eq!(
            http.prepare_default(
                tokio::time::Instant::now() + Duration::from_secs(5),
                Cancellation::default()
            )
            .await
            .unwrap_err(),
            Error::InvalidRequest
        );
        let mut session = http
            .session(
                Options::default(),
                tokio::time::Instant::now() + Duration::from_secs(5),
                Cancellation::default(),
            )
            .unwrap();
        let pool = session.pool.clone();
        assert_eq!(
            session
                .execute(Request::get("http://127.0.0.1:1/").unwrap())
                .await
                .unwrap_err(),
            Error::InvalidRequest
        );
        http.builders.drain().await;
        assert_eq!(http.connections.available_permits(), 20);
        assert_eq!(pool.permits.available_permits(), 20);
        assert_eq!(busy(&http), 0);
        assert_eq!(http.idle.available_permits(), 10);
        http.shutdown_and_wait().await;
    }
    #[tokio::test]
    async fn owner_close_cancels_waiter_but_retains_build_ownership_until_drain() {
        let http = Arc::new(AdapterHttp::default());
        let (started, release) = block_builder(&http);
        let mut session = http
            .session(
                Options::default(),
                tokio::time::Instant::now() + Duration::from_secs(60),
                Cancellation::default(),
            )
            .unwrap();
        let pool = session.pool.clone();
        let operation = tokio::spawn(async move {
            session
                .execute(Request::get("http://127.0.0.1:1/").unwrap())
                .await
        });
        tokio::time::timeout(Duration::from_secs(5), started)
            .await
            .unwrap()
            .unwrap();
        http.shutdown();
        assert_eq!(operation.await.unwrap().unwrap_err(), Error::Closed);
        assert_eq!(http.connections.available_permits(), 19);
        assert_eq!(pool.permits.available_permits(), 19);
        assert!(pool.slots.lock().unwrap().iter().any(|(busy, _)| *busy));
        release.send(()).unwrap();
        http.shutdown_and_wait().await;
        assert_eq!(http.connections.available_permits(), 20);
        assert_eq!(pool.permits.available_permits(), 20);
        assert!(pool.slots.lock().unwrap().iter().all(|(busy, _)| !*busy));
        assert_eq!(http.idle.available_permits(), 10);
    }
    struct NeverExchange;
    impl Transport for NeverExchange {
        fn exchange(&self, _: Request) -> ExchangeFuture<'_> {
            Box::pin(async { panic!("preparation dispatched HTTP") })
        }
    }
    #[tokio::test]
    async fn injected_and_ephemeral_profiles_never_start_preparation_workers() {
        let http = AdapterHttp::default();
        *http.builders.build.lock().unwrap() = Some(Arc::new(|_| panic!("excluded profile built")));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        // These are actual existing ephemeral Session profiles. The same private
        // classification is called by the public default-only preparation API.
        for options in [
            Options {
                trust_env: false,
                ..Options::default()
            },
            Options {
                http2: true,
                ..Options::default()
            },
            Options {
                verify_tls: false,
                ..Options::default()
            },
        ] {
            let session = http
                .session(options, deadline, Cancellation::default())
                .unwrap();
            assert_eq!(
                http.prepare_resolved(session, deadline, Cancellation::default())
                    .await
                    .unwrap(),
                Preparation::Ephemeral
            );
        }
        assert!(http.builders.state.lock().unwrap().jobs.is_empty());
        assert_eq!(http.connections.available_permits(), 20);
        assert_eq!(http.idle.available_permits(), 10);
        http.shutdown_and_wait().await;
        let injected = AdapterHttp::default();
        injected.set_transport(Arc::new(NeverExchange)).unwrap();
        *injected.builders.build.lock().unwrap() =
            Some(Arc::new(|_| panic!("injected profile built")));
        assert_eq!(
            injected
                .prepare_default(deadline, Cancellation::default())
                .await
                .unwrap(),
            Preparation::Injected
        );
        assert!(injected.builders.state.lock().unwrap().jobs.is_empty());
        assert_eq!(injected.connections.available_permits(), 20);
        assert_eq!(injected.idle.available_permits(), 10);
        injected.shutdown_and_wait().await;
    }
    async fn event(events: &mut tokio::sync::mpsc::Receiver<&'static str>, expected: &'static str) {
        let actual = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(actual, expected);
    }
    #[tokio::test]
    async fn aborted_unpublished_constructor_guardian_joins_blocked_builder_without_external_owner()
    {
        if default_ca_test_environment(
            "http::client_initialization_tests::aborted_unpublished_constructor_guardian_joins_blocked_builder_without_external_owner",
        ) {
            return;
        }
        let owner = Arc::new(AdapterHttp::default());
        let weak = Arc::downgrade(&owner);
        let connections = owner.connections.clone();
        let idle = owner.idle.clone();
        let (events_tx, mut events) = tokio::sync::mpsc::channel(16);
        *owner.builders.events.lock().unwrap() = Some(events_tx);
        let (started, release) = block_builder(&owner);
        let mut constructor = Box::pin(AdapterHttp::ready_owner(
            owner,
            tokio::time::Instant::now() + Duration::from_secs(60),
            Cancellation::default(),
        ));
        std::future::poll_fn(|cx| {
            assert!(constructor.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        tokio::time::timeout(Duration::from_secs(5), started)
            .await
            .unwrap()
            .unwrap();
        let pool = {
            let owner = weak.upgrade().unwrap();
            owner
                .state
                .lock()
                .unwrap()
                .pools
                .get(&None)
                .unwrap()
                .clone()
        };
        // Only the unpublished guardian now owns AdapterHttp, not this test.
        assert_eq!(weak.strong_count(), 1);
        drop(constructor);
        event(&mut events, "startup_cleanup_started").await;
        assert_eq!(connections.available_permits(), 19);
        assert_eq!(pool.permits.available_permits(), 19);
        assert!(pool.slots.lock().unwrap().iter().any(|(busy, _)| *busy));
        assert!(events.try_recv().is_err());
        release.send(()).unwrap();
        event(&mut events, "builders_joined").await;
        event(&mut events, "shutdown_drained").await;
        event(&mut events, "owner_dropped").await;
        assert!(weak.upgrade().is_none());
        assert_eq!(connections.available_permits(), 20);
        assert_eq!(pool.permits.available_permits(), 20);
        assert!(
            pool.slots
                .lock()
                .unwrap()
                .iter()
                .all(|(busy, entry)| !busy && entry.is_none())
        );
        assert_eq!(idle.available_permits(), 10);
    }
    #[tokio::test]
    async fn unread_successful_publication_still_requires_ack_and_drains_on_receiver_drop() {
        if default_ca_test_environment(
            "http::client_initialization_tests::unread_successful_publication_still_requires_ack_and_drains_on_receiver_drop",
        ) {
            return;
        }
        let owner = Arc::new(AdapterHttp::default());
        let weak = Arc::downgrade(&owner);
        let connections = owner.connections.clone();
        let idle = owner.idle.clone();
        let (events_tx, mut events) = tokio::sync::mpsc::channel(16);
        *owner.builders.events.lock().unwrap() = Some(events_tx);
        *owner.builders.build.lock().unwrap() = Some(Arc::new(|_| {
            build_client(&Options {
                trust_env: false,
                ..Options::default()
            })
        }));
        let mut constructor = Box::pin(AdapterHttp::ready_owner(
            owner,
            tokio::time::Instant::now() + Duration::from_secs(60),
            Cancellation::default(),
        ));
        std::future::poll_fn(|cx| {
            assert!(constructor.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        event(&mut events, "builders_joined").await;
        event(&mut events, "startup_sent").await;
        // Do not poll constructor again: send succeeded, but result is unread.
        assert_eq!(weak.strong_count(), 2); // guardian plus channel payload
        assert_eq!(idle.available_permits(), 9);
        drop(constructor);
        event(&mut events, "startup_cleanup_started").await;
        event(&mut events, "builders_joined").await;
        event(&mut events, "shutdown_drained").await;
        event(&mut events, "owner_dropped").await;
        assert!(weak.upgrade().is_none());
        assert_eq!(connections.available_permits(), 20);
        assert_eq!(idle.available_permits(), 10);
    }
    async fn reject_late_unread_publication(cancelled: bool) {
        let owner = Arc::new(AdapterHttp::default());
        let weak = Arc::downgrade(&owner);
        let connections = owner.connections.clone();
        let idle = owner.idle.clone();
        let (events_tx, mut events) = tokio::sync::mpsc::channel(16);
        *owner.builders.events.lock().unwrap() = Some(events_tx);
        *owner.builders.build.lock().unwrap() = Some(Arc::new(|_| {
            build_client(&Options {
                trust_env: false,
                ..Options::default()
            })
        }));
        let cancel = Cancellation::default();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        let mut constructor = Box::pin(AdapterHttp::ready_owner(owner, deadline, cancel.clone()));
        std::future::poll_fn(|cx| {
            assert!(constructor.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        event(&mut events, "builders_joined").await;
        event(&mut events, "startup_sent").await;
        assert_eq!(weak.strong_count(), 2);
        if cancelled {
            cancel.cancel();
        } else {
            tokio::time::pause();
            tokio::time::advance(Duration::from_secs(61)).await;
            assert!(tokio::time::Instant::now() > deadline);
            tokio::time::resume();
        }
        let result = tokio::time::timeout(Duration::from_secs(5), constructor)
            .await
            .unwrap();
        if cancelled {
            assert!(matches!(result, Err(Error::Cancelled)));
        } else {
            assert!(matches!(result, Err(Error::Deadline)));
        }
        event(&mut events, "startup_cleanup_started").await;
        event(&mut events, "builders_joined").await;
        event(&mut events, "shutdown_drained").await;
        event(&mut events, "owner_dropped").await;
        assert!(weak.upgrade().is_none());
        assert_eq!(connections.available_permits(), 20);
        assert_eq!(idle.available_permits(), 10);
    }
    #[tokio::test]
    async fn unread_publication_cancelled_before_final_poll_is_rejected_and_drained() {
        reject_late_unread_publication(true).await;
    }
    #[tokio::test]
    async fn unread_publication_expired_before_final_poll_is_rejected_and_drained() {
        reject_late_unread_publication(false).await;
    }
}

#[cfg(test)]
#[path = "../tests/support/a07_http_clock.rs"]
mod owner_clock_tests;

#[cfg(test)]
#[path = "http/pending_replay.rs"]
mod pending_replay;
#[cfg(test)]
pub(crate) use pending_replay::PendingObserver;
#[cfg(test)]
#[path = "../tests/support/a06_timeout_clock.rs"]
mod scholarly_timeout_clock;

#[cfg(test)]
#[path = "../tests/support/a07_cold_context.rs"]
mod cold_context_tests;
