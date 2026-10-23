//! Adapter-owned sockets and search-owned identity. No provider retries or parsers.
mod monotonic;
use monotonic::Clock as DeadlineClock;
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
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, watch};

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
    fn checkout(self: &Arc<Self>, url: &Url, state: Arc<Mutex<State>>) -> Result<Checkout> {
        let origin = url.origin().ascii_serialization();
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
                        i.origin == origin && i.used.elapsed() < Duration::from_secs(5)
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
                Some(i) if i.origin == origin && i.used.elapsed() < Duration::from_secs(5) => {
                    i.client
                }
                _ => build_client(&self.options)?,
            };
            slots[n].0 = true;
            client
        } else {
            build_client(&self.options)?
        };
        Ok(Checkout {
            state,
            pool: self.clone(),
            slot,
            origin,
            client: Some(client),
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
}
impl Drop for Checkout {
    fn drop(&mut self) {
        // Match shutdown's state -> pool lock order. Expiry may have elapsed
        // during the response, so reclaim before attempting idle retention too.
        self.state.lock().unwrap().expire_idle();
        if let Some(n) = self.slot {
            let mut slots = self.pool.slots.lock().unwrap();
            if self.pool.permits.is_closed() {
                return;
            }
            slots[n].0 = false;
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
    pub fn set_transport(&self, t: Arc<dyn Transport>) -> Result<()> {
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
            pool,
            injected: s.injected.clone(),
            jar: Jar::default(),
            headers: HeaderMap::new(),
            options,
            deadline,
            cancel,
            stop: self.stop.clone(),
            requests: 0,
        })
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
    }
    pub fn retained_pools(&self) -> usize {
        self.state.lock().unwrap().pools.len()
    }
}
impl Drop for AdapterHttp {
    fn drop(&mut self) {
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
        let _adapter_permit = self
            .connections
            .acquire()
            .await
            .map_err(|_| Error::Closed)?;
        let _permit = self
            .pool
            .permits
            .acquire()
            .await
            .map_err(|_| Error::Closed)?;
        self.state.lock().unwrap().expire_idle();
        let checkout = self.pool.checkout(&r.url, self.state.clone())?;
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
