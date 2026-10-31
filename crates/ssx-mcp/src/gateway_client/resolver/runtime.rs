pub use super::process::SourceResolverHelper;
use super::process::{self, Control, OwnedChild as Child};
use super::protocol::{self, SourceResolveError, SourceResolveStage};
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{oneshot, watch},
    task::JoinSet,
};
/// Failed cleanup is sticky across close retries; semantic GAI/socket failures
/// do not make close fail when every process was reaped and group was absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCloseError {
    pub failed_supervisors: usize,
    pub unretired_rows: usize,
}
impl std::fmt::Display for SourceCloseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "source resolver close: {} failed supervisors, {} unretired rows",
            self.failed_supervisors, self.unretired_rows
        )
    }
}
impl std::error::Error for SourceCloseError {}
struct Row {
    cancel: watch::Sender<bool>,
    control: Arc<Control>,
    ready: Arc<AtomicBool>,
    residual: Option<Child>,
}
struct State {
    closing: bool,
    next: u64,
    rows: BTreeMap<u64, Row>,
    tasks: JoinSet<(u64, Option<Child>)>,
    failures: usize,
}
impl State {
    fn completed(&mut self, result: Result<(u64, Option<Child>), tokio::task::JoinError>) {
        match result {
            Ok((id, None)) => {
                self.rows.remove(&id);
            }
            Ok((id, Some(child))) => {
                self.failures += 1;
                // Keep ownership of a child whose reaping was not established.
                // The next explicit close retries its retirement.
                if let Some(row) = self.rows.get_mut(&id) {
                    row.residual = Some(child);
                }
            }
            Err(_) => {
                self.failures += 1;
            }
        }
    }
    fn reap_ready(&mut self) {
        while let Some(result) = self.tasks.try_join_next() {
            self.completed(result);
        }
    }
}
struct Inner {
    helper: SourceResolverHelper,
    state: Mutex<State>,
    closer: tokio::sync::Mutex<()>,
}
pub(super) fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}
impl Drop for Inner {
    fn drop(&mut self) {
        // Best effort only; application owner MUST await close. JoinSet/Child Drop
        // and this signal cannot prove reaping or group absence.
        let state = self
            .state
            .get_mut()
            .unwrap_or_else(|error| error.into_inner());
        for row in state.rows.values() {
            row.cancel.send_replace(true);
            row.control.cancel_fallback();
        }
    }
}
/// Application lifetime owner. Its outer server owner retains a clone across
/// request aborts and must await close before retiring the application's IO.
#[derive(Clone)]
pub struct SourceRuntime {
    inner: Arc<Inner>,
}
impl std::fmt::Debug for SourceRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceRuntime").finish_non_exhaustive()
    }
}
struct CancelOnDrop(watch::Sender<bool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}
impl SourceRuntime {
    pub fn new(helper: SourceResolverHelper) -> Self {
        Self {
            inner: Arc::new(Inner {
                helper,
                state: Mutex::new(State {
                    closing: false,
                    next: 0,
                    rows: BTreeMap::new(),
                    tasks: JoinSet::new(),
                    failures: 0,
                }),
                closer: tokio::sync::Mutex::new(()),
            }),
        }
    }
    /// Reqwest's ordinary hostname hook supplies no service. Preserve the
    /// native sockaddr order/scope/flow/port; the existing connector applies its
    /// ordinary explicit/default destination-port rules after this hook.
    pub async fn resolve_dns(
        &self,
        host: &str,
        lookup_budget: Duration,
    ) -> Result<Vec<SocketAddr>, SourceResolveError> {
        self.lookup(host, "0", lookup_budget, true).await
    }
    /// Retained source endpoint protocol. Raw gateway URL adoption is separate.
    pub async fn resolve(
        &self,
        host: &str,
        decimal: &str,
        lookup_budget: Duration,
    ) -> Result<Vec<SocketAddr>, SourceResolveError> {
        self.lookup(host, decimal, lookup_budget, false).await
    }
    async fn lookup(
        &self,
        host: &str,
        decimal: &str,
        lookup_budget: Duration,
        native_sockaddrs: bool,
    ) -> Result<Vec<SocketAddr>, SourceResolveError> {
        // Public legacy aggregate API retains its original ordering/contract.
        // Production source endpoints use SourceFlow's explicit lazy stages.
        if locked(&self.inner.state).closing {
            return Err(SourceResolveError::new(SourceResolveStage::Closed));
        }
        let _ = protocol::request(protocol::RequestKind::AnyIoCandidates, host, Some(decimal))?;
        if !host.contains('%')
            && let Ok(ip) = host.parse::<IpAddr>()
        {
            return protocol::socket_port(decimal).map(|p| vec![SocketAddr::new(ip, p)]);
        }
        let scoped_host = scoped_literal(host);
        let scoped = scoped_host.is_some();
        let kind = if native_sockaddrs {
            protocol::RequestKind::DnsOnly
        } else if scoped {
            protocol::RequestKind::AsyncIoCandidate
        } else {
            protocol::RequestKind::AnyIoCandidates
        };
        let input = protocol::request(
            kind,
            scoped_host.as_deref().unwrap_or(host),
            if native_sockaddrs {
                None
            } else {
                Some(decimal)
            },
        )?;
        let reply = self
            .exchange_reply(
                input,
                kind,
                lookup_budget,
                #[cfg(feature = "gateway-source-endpoints")]
                None,
            )
            .await
            .map_err(|mut error| {
                // Retain the old public DNS hook's scoped error label. The new
                // source adapter uses explicit kinds and does not take this path.
                if native_sockaddrs && scoped && error.stage == SourceResolveStage::AnyIoGetAddrInfo
                {
                    error.stage = SourceResolveStage::AsyncIoGetAddrInfo;
                }
                error
            })?;
        if reply.addresses.is_empty() {
            return Err(SourceResolveError::new(SourceResolveStage::EmptyAddresses));
        }
        if scoped || native_sockaddrs {
            return Ok(reply.addresses);
        }
        let port = protocol::socket_port(decimal)?;
        Ok(reply
            .addresses
            .into_iter()
            .map(|a| SocketAddr::new(a.ip(), port))
            .collect())
    }
    /// One global admission/row/JoinSet state machine owns every native child.
    async fn exchange_reply(
        &self,
        input: Vec<u8>,
        kind: protocol::RequestKind,
        budget: Duration,
        #[cfg(feature = "gateway-source-endpoints")] scope: Option<Arc<scoped::ScopeInner>>,
    ) -> Result<protocol::Reply, SourceResolveError> {
        let (send, receive) = oneshot::channel();
        let (cancel, _) = watch::channel(false);
        let request_guard = CancelOnDrop(cancel.clone());
        {
            let mut state = locked(&self.inner.state);
            state.reap_ready();
            if state.closing {
                return Err(SourceResolveError::new(SourceResolveStage::Closed));
            }
            let id = state.next;
            state.next = state
                .next
                .checked_add(1)
                .ok_or_else(|| SourceResolveError::new(SourceResolveStage::OperationalLimit))?;
            #[cfg(feature = "gateway-source-endpoints")]
            let scoped_guard = scope
                .map(|scope| scoped::ScopeTaskGuard::register(scope, id, cancel.clone()))
                .transpose()?;
            let control = Arc::new(Control::new());
            let ready = Arc::new(AtomicBool::new(false));
            state.rows.insert(
                id,
                Row {
                    cancel: cancel.clone(),
                    control: control.clone(),
                    ready: ready.clone(),
                    residual: None,
                },
            );
            let helper = self.inner.helper.clone();
            state.tasks.spawn(async move {
                let (result, residual) =
                    process::supervise(helper, input, cancel.subscribe(), control, ready, budget)
                        .await;
                let result = result.and_then(|bytes| protocol::decode_response(&bytes, kind));
                #[cfg(feature = "gateway-source-endpoints")]
                if let Some(guard) = scoped_guard {
                    guard.finish(residual.is_none());
                }
                let _ = send.send(result);
                (id, residual)
            });
        }
        let result = receive
            .await
            .unwrap_or_else(|_| Err(SourceResolveError::new(SourceResolveStage::Helper)));
        drop(request_guard);
        result
    }
    #[cfg(feature = "gateway-source-endpoints")]
    pub(super) fn source_flow(&self, deadline: tokio::time::Instant) -> SourceFlow {
        SourceFlow::new(self.clone(), deadline)
    }
    /// Close is cancellation safe: dropping this future leaves tasks and rows
    /// registered, closing stays true, and the next close resumes joining them.
    /// Concurrent callers serialize join polling; no overwritten JoinSet waker.
    pub async fn close(&self) -> Result<(), SourceCloseError> {
        {
            let mut state = locked(&self.inner.state);
            state.closing = true;
            for row in state.rows.values() {
                row.cancel.send_replace(true);
            }
        }
        let _closer = self.inner.closer.lock().await;
        {
            let mut state = locked(&self.inner.state);
            let retries: Vec<_> = state
                .rows
                .iter_mut()
                .filter_map(|(id, row)| {
                    row.residual
                        .take()
                        .map(|child| (*id, child, row.control.clone()))
                })
                .collect();
            for (id, mut child, control) in retries {
                state.tasks.spawn(async move {
                    // Unix's signal gate remains disarmed after reaping. Windows
                    // retries retain original process/job handles, never PID authority.
                    let outcome = process::retire(&mut child, &control, false).await;
                    if outcome.group_absent {
                        control.clear_identity();
                    }
                    if outcome.retired() {
                        (id, None)
                    } else {
                        (id, Some(child))
                    }
                });
            }
        }
        loop {
            let result = std::future::poll_fn(|cx| {
                let mut state = locked(&self.inner.state);
                state.tasks.poll_join_next(cx)
            })
            .await;
            match result {
                Some(result) => locked(&self.inner.state).completed(result),
                None => break,
            }
        }
        let state = locked(&self.inner.state);
        if state.failures == 0 && state.rows.is_empty() {
            Ok(())
        } else {
            Err(SourceCloseError {
                failed_supervisors: state.failures,
                unretired_rows: state.rows.len(),
            })
        }
    }
    #[doc(hidden)]
    pub fn ready_process_groups(&self) -> Vec<u32> {
        let state = locked(&self.inner.state);
        state
            .rows
            .values()
            .filter(|row| row.ready.load(Ordering::Acquire))
            .map(|row| row.control.identity())
            .filter(|id| *id != 0)
            .collect()
    }
    #[doc(hidden)]
    pub fn ownership_snapshot(&self) -> (bool, usize, Vec<u32>) {
        let mut state = locked(&self.inner.state);
        state.reap_ready();
        (
            state.closing,
            state.rows.len(),
            state
                .rows
                .values()
                .map(|row| row.control.identity())
                .filter(|id| *id != 0)
                .collect(),
        )
    }
}
// Exact CPython3.14.7 ipaddress.compressed source stages. Native inet_ntop
// belongs only to first getaddrinfo rows; it must not format initial literals.
fn literal_ip_text(address: IpAddr) -> String {
    match address {
        IpAddr::V4(ip) => {
            let b = ip.octets();
            format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
        }
        IpAddr::V6(ip) => {
            let h = ip.segments();
            if h[..5] == [0; 5] && h[5] == 0xffff {
                let b = ip.octets();
                return format!("::ffff:{}.{}.{}.{}", b[12], b[13], b[14], b[15]);
            }
            let (mut start, mut longest, mut current_start, mut current) =
                (0usize, 0usize, 0usize, 0usize);
            for (i, v) in h.iter().enumerate() {
                if *v == 0 {
                    if current == 0 {
                        current_start = i;
                    }
                    current += 1;
                    if current > longest {
                        start = current_start;
                        longest = current;
                    }
                } else {
                    current = 0;
                }
            }
            let text = h.iter().map(|v| format!("{v:x}")).collect::<Vec<_>>();
            if longest > 1 {
                format!(
                    "{}::{}",
                    text[..start].join(":"),
                    text[start + longest..].join(":")
                )
            } else {
                text.join(":")
            }
        }
    }
}
fn scoped_literal(host: &str) -> Option<String> {
    let (address, scope) = host.split_once('%')?;
    if scope.is_empty() || scope.contains('%') {
        return None;
    }
    let address = address.parse::<std::net::Ipv6Addr>().ok()?;
    Some(format!("{}%{scope}", literal_ip_text(IpAddr::V6(address))))
}

#[cfg(feature = "gateway-source-endpoints")]
pub(super) use scoped::SourceFlow;
#[cfg(all(feature = "gateway-source-endpoints", feature = "gateway-dns-fixtures"))]
pub(super) use scoped::fixture_controls;
#[cfg(feature = "gateway-source-endpoints")]
mod scoped {
    use super::*;
    // Per-resolution cancellation registration only. Original child/IO ownership is
    // exclusively the existing Runtime State/Row/JoinSet above, never a second manager.
    struct ScopeRequests {
        closed: bool,
        active: BTreeMap<u64, watch::Sender<bool>>,
        ids: std::collections::BTreeSet<u64>,
        cleanup_deadline: Option<tokio::time::Instant>,
        cleanup_result: Option<Result<(), ScopedCloseError>>,
    }
    pub(super) struct ScopeInner {
        runtime: SourceRuntime,
        deadline: tokio::time::Instant,
        requests: Mutex<ScopeRequests>,
        failed: std::sync::atomic::AtomicUsize,
        timed_out: AtomicBool,
    }
    impl ScopeInner {
        fn cancel(&self) {
            let mut rows = locked(&self.requests);
            if rows.closed {
                return;
            }
            // The first synchronous cancel (including Drop) owns the only grace.
            rows.cleanup_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(5));
            rows.closed = true;
            for send in rows.active.values() {
                send.send_replace(true);
            }
        }
    }
    pub(super) struct ScopeTaskGuard {
        scope: Arc<ScopeInner>,
        id: u64,
        finished: bool,
    }
    impl ScopeTaskGuard {
        pub(super) fn register(
            scope: Arc<ScopeInner>,
            id: u64,
            cancel: watch::Sender<bool>,
        ) -> Result<Self, SourceResolveError> {
            {
                let mut rows = locked(&scope.requests);
                if rows.closed {
                    return Err(SourceResolveError::new(SourceResolveStage::Cancelled));
                }
                rows.active.insert(id, cancel);
                rows.ids.insert(id);
            }
            Ok(Self {
                scope,
                id,
                finished: false,
            })
        }
        pub(super) fn finish(mut self, retired: bool) {
            if !retired {
                self.scope.failed.fetch_add(1, Ordering::AcqRel);
            }
            self.finished = true;
            locked(&self.scope.requests).active.remove(&self.id);
        }
    }
    impl Drop for ScopeTaskGuard {
        fn drop(&mut self) {
            if !self.finished {
                self.scope.failed.fetch_add(1, Ordering::AcqRel);
                locked(&self.scope.requests).active.remove(&self.id);
            }
        }
    }
    #[derive(Clone)]
    pub(crate) struct SourceFlow {
        inner: Arc<ScopeInner>,
    }
    #[derive(Clone, Debug)]
    pub(crate) struct ScopedCloseError {
        failed_supervisors: usize,
        unretired_rows: usize,
        retirement_timed_out: bool,
    }
    impl std::fmt::Display for ScopedCloseError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "source resolution retirement: {} failed supervisors, {} unjoined rows, timeout={}",
                self.failed_supervisors, self.unretired_rows, self.retirement_timed_out
            )
        }
    }
    impl std::error::Error for ScopedCloseError {}
    impl SourceFlow {
        pub(super) fn new(runtime: SourceRuntime, deadline: tokio::time::Instant) -> Self {
            Self {
                inner: Arc::new(ScopeInner {
                    runtime,
                    deadline,
                    requests: Mutex::new(ScopeRequests {
                        closed: false,
                        active: BTreeMap::new(),
                        ids: std::collections::BTreeSet::new(),
                        cleanup_deadline: None,
                        cleanup_result: None,
                    }),
                    failed: std::sync::atomic::AtomicUsize::new(0),
                    timed_out: AtomicBool::new(false),
                }),
            }
        }
        pub(crate) fn cancel(&self) {
            self.inner.cancel();
        }
        fn remaining(&self) -> Result<Duration, SourceResolveError> {
            if locked(&self.inner.runtime.inner.state).closing {
                return Err(SourceResolveError::new(SourceResolveStage::Closed));
            }
            if locked(&self.inner.requests).closed {
                return Err(SourceResolveError::new(SourceResolveStage::Cancelled));
            }
            self.inner
                .deadline
                .checked_duration_since(tokio::time::Instant::now())
                .filter(|d| !d.is_zero())
                .ok_or_else(|| SourceResolveError::new(SourceResolveStage::TimedOut))
        }
        async fn exchange(
            &self,
            kind: protocol::RequestKind,
            host: &str,
            decimal: &str,
        ) -> Result<protocol::Reply, SourceResolveError> {
            let input = protocol::request(kind, host, Some(decimal))?;
            let remaining = self.remaining()?;
            self.inner
                .runtime
                .exchange_reply(input, kind, remaining, Some(self.inner.clone()))
                .await
        }
        pub(crate) async fn prepare(
            &self,
            host: &str,
            decimal: &str,
        ) -> Result<Vec<protocol::Candidate>, SourceResolveError> {
            self.remaining()?;
            let _ = protocol::request(protocol::RequestKind::AnyIoCandidates, host, Some(decimal))?;
            if !host.contains('%')
                && let Ok(ip) = host.parse::<IpAddr>()
            {
                return Ok(vec![protocol::Candidate {
                    family: if ip.is_ipv4() {
                        protocol::Family::V4
                    } else {
                        protocol::Family::V6
                    },
                    native_host_text: super::literal_ip_text(ip),
                }]);
            }
            if let Some(host) = scoped_literal(host) {
                return Ok(vec![protocol::Candidate {
                    family: protocol::Family::V6,
                    native_host_text: host,
                }]);
            }
            // Only first GAI runs here. Native text/scope stay separate from final
            // addresses. The lower connector applies the original family ordering.
            let reply = self
                .exchange(protocol::RequestKind::AnyIoCandidates, host, decimal)
                .await?;
            if reply.candidates.is_empty() {
                return Err(SourceResolveError::new(SourceResolveStage::EmptyAddresses));
            }
            Ok(reply.candidates)
        }
        pub(crate) async fn resolve_candidate(
            &self,
            candidate: &protocol::Candidate,
            decimal: &str,
        ) -> Result<Vec<SocketAddr>, SourceResolveError> {
            self.remaining()?;
            let host = &candidate.native_host_text;
            if !host.contains('%')
                && let Ok(ip) = host.parse::<IpAddr>()
            {
                // This is the platform-specific actual async-connect conversion,
                // after first GAI, never a URL/domain or early DNS validation cast.
                return protocol::socket_port(decimal).map(|p| vec![SocketAddr::new(ip, p)]);
            }
            // Percent host text declines asyncio's numeric shortcut. Resolve only
            // this launched candidate, preserving all FINAL port/flow/scope fields.
            let reply = self
                .exchange(protocol::RequestKind::AsyncIoCandidate, host, decimal)
                .await?;
            if reply.addresses.is_empty() {
                return Err(SourceResolveError::new(SourceResolveStage::EmptyAddresses));
            }
            Ok(reply.addresses)
        }
        pub(crate) async fn cancel_and_wait(&self) -> Result<(), ScopedCloseError> {
            self.cancel();
            // One deadline is committed BEFORE waiting for the global closer. A
            // dropped close future or a repeated outer cleanup cannot renew it.
            let (deadline, ids) = {
                let rows = locked(&self.inner.requests);
                if let Some(result) = &rows.cleanup_result {
                    return result.clone();
                }
                let deadline = rows.cleanup_deadline.expect("synchronous cancel set grace");
                (deadline, rows.ids.clone())
            };
            if ids.is_empty() {
                return locked(&self.inner.requests)
                    .cleanup_result
                    .get_or_insert(Ok(()))
                    .clone();
            }
            if tokio::time::Instant::now() >= deadline {
                self.inner.timed_out.store(true, Ordering::Release);
                let failed = self.inner.failed.load(Ordering::Acquire);
                let pending = {
                    let state = locked(&self.inner.runtime.inner.state);
                    ids.iter().filter(|id| state.rows.contains_key(id)).count()
                };
                return locked(&self.inner.requests)
                    .cleanup_result
                    .get_or_insert(Err(ScopedCloseError {
                        failed_supervisors: failed,
                        unretired_rows: pending,
                        retirement_timed_out: true,
                    }))
                    .clone();
            }
            let joins = async {
                let _serial = self.inner.runtime.inner.closer.lock().await;
                if let Some(result) = &locked(&self.inner.requests).cleanup_result {
                    return Some(result.clone());
                }
                loop {
                    let result = std::future::poll_fn(|cx| {
                        let mut state = locked(&self.inner.runtime.inner.state);
                        state.reap_ready();
                        // Another admission/diagnostic caller can already have
                        // joined our ready task. Check own IDs on EVERY poll.
                        if ids.iter().all(|id| !state.rows.contains_key(id))
                            || self.inner.failed.load(Ordering::Acquire) > 0
                        {
                            std::task::Poll::Ready(None)
                        } else {
                            state.tasks.poll_join_next(cx)
                        }
                    })
                    .await;
                    match result {
                        Some(result) => locked(&self.inner.runtime.inner.state).completed(result),
                        None => break,
                    }
                }
                None
            };
            match tokio::time::timeout_at(deadline, joins).await {
                Ok(Some(result)) => return result,
                Ok(None) => (),
                Err(_) => self.inner.timed_out.store(true, Ordering::Release),
            }
            let failed = self.inner.failed.load(Ordering::Acquire);
            let pending = {
                let state = locked(&self.inner.runtime.inner.state);
                ids.iter().filter(|id| state.rows.contains_key(id)).count()
            };
            let timed = self.inner.timed_out.load(Ordering::Acquire);
            let result = if failed == 0 && pending == 0 && !timed {
                Ok(())
            } else {
                Err(ScopedCloseError {
                    failed_supervisors: failed,
                    unretired_rows: pending,
                    retirement_timed_out: timed,
                })
            };
            // First terminal result wins; no later call starts IO or retries failure.
            // Original residual rows remain owned for explicit application close.
            locked(&self.inner.requests)
                .cleanup_result
                .get_or_insert(result)
                .clone()
        }
    }
    #[cfg(feature = "gateway-dns-fixtures")]
    pub(crate) async fn fixture_controls(runtime: SourceRuntime) -> crate::Result<()> {
        super::super::source_adapter::synchronous_cancel_control(runtime.clone()).await;
        let flow = runtime.source_flow(tokio::time::Instant::now() + Duration::from_secs(20));
        let rows = flow.prepare("staged-source.invalid", "65537").await?;
        assert_eq!(
            rows.len(),
            1,
            "only first native candidate returned before lazy stage"
        );
        assert_eq!(
            rows[0].native_host_text, "::2%19",
            "first scope survives host projection"
        );
        let final_rows = flow.resolve_candidate(&rows[0], "65537").await?;
        assert_eq!(
            final_rows,
            vec![SocketAddr::V6(std::net::SocketAddrV6::new(
                "::2".parse().unwrap(),
                37,
                88,
                23
            ))],
            "secondary native port/flow/scope authoritative"
        );
        flow.cancel_and_wait().await?;
        assert_eq!(runtime.ownership_snapshot().1, 0);
        let flow = runtime.source_flow(tokio::time::Instant::now() + Duration::from_secs(20));
        let work = flow.clone();
        let guard = super::super::source_adapter::preparation_guard(work.clone());
        let unpolled = async move {
            let _guard = guard;
            work.prepare("hold.invalid", "80").await
        };
        let drop_start = tokio::time::Instant::now();
        drop(unpolled);
        let drop_deadline = locked(&flow.inner.requests).cleanup_deadline.unwrap();
        assert!(drop_deadline >= drop_start + Duration::from_secs(5));
        assert!(drop_deadline <= tokio::time::Instant::now() + Duration::from_secs(5));
        flow.cancel();
        assert_eq!(
            locked(&flow.inner.requests).cleanup_deadline,
            Some(drop_deadline),
            "unpolled Drop starts one grace; repeated synchronous cancel cannot reset it"
        );
        assert_eq!(
            flow.prepare("127.0.0.1", "80").await.err().unwrap().stage,
            SourceResolveStage::Cancelled,
            "unpolled first future cancels scope without dispatch"
        );
        flow.cancel_and_wait().await?;
        assert_eq!(runtime.ownership_snapshot().1, 0);
        async fn entered(runtime: &SourceRuntime) -> crate::Result<()> {
            tokio::time::timeout(Duration::from_secs(3), async {
                while runtime.ready_process_groups().is_empty() {
                    tokio::task::yield_now().await;
                }
            })
            .await?;
            Ok(())
        }
        let flow = runtime.source_flow(tokio::time::Instant::now() + Duration::from_secs(20));
        let work = flow.clone();
        let guard = super::super::source_adapter::preparation_guard(work.clone());
        let task = tokio::spawn(async move {
            let _guard = guard;
            work.prepare("hold.invalid", "80").await
        });
        entered(&runtime).await?;
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        flow.cancel_and_wait().await?;
        assert_eq!(
            runtime.ownership_snapshot().1,
            0,
            "dropped first GAI joins original runtime row"
        );
        // A different legitimate reader can reap our completed JoinSet row.
        // Its foreign active row must not keep this flow's close Pending.
        let own = runtime.source_flow(tokio::time::Instant::now() + Duration::from_secs(20));
        let foreign = runtime.source_flow(tokio::time::Instant::now() + Duration::from_secs(20));
        let work = own.clone();
        let guard = super::super::source_adapter::preparation_guard(work.clone());
        let own_task = tokio::spawn(async move {
            let _guard = guard;
            work.prepare("hold.invalid", "80").await
        });
        let work = foreign.clone();
        let guard = super::super::source_adapter::preparation_guard(work.clone());
        let foreign_task = tokio::spawn(async move {
            let _guard = guard;
            work.prepare("hold.invalid", "80").await
        });
        tokio::time::timeout(Duration::from_secs(3), async {
            while runtime.ready_process_groups().len() != 2 {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        let own_ids = locked(&own.inner.requests).ids.clone();
        let foreign_ids = locked(&foreign.inner.requests).ids.clone();
        let mut closing = Box::pin(own.cancel_and_wait());
        std::future::poll_fn(|cx| {
            use std::future::Future;
            assert!(closing.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                runtime.ownership_snapshot();
                if own_ids
                    .iter()
                    .all(|id| !locked(&runtime.inner.state).rows.contains_key(id))
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert!(
            foreign_ids
                .iter()
                .all(|id| locked(&runtime.inner.state).rows.contains_key(id)),
            "foreign helper remains held"
        );
        std::future::poll_fn(|cx| {
            use std::future::Future;
            match closing.as_mut().poll(cx) {
                std::task::Poll::Ready(result) => {
                    assert!(result.is_ok());
                    std::task::Poll::Ready(())
                }
                std::task::Poll::Pending => panic!("joined own flow cannot await foreign helper"),
            }
        })
        .await;
        drop(closing);
        assert!(own_task.await?.is_err());
        foreign_task.abort();
        assert!(foreign_task.await.unwrap_err().is_cancelled());
        foreign.cancel_and_wait().await?;
        assert_eq!(runtime.ownership_snapshot().1, 0);
        let flow = runtime.source_flow(tokio::time::Instant::now() + Duration::from_secs(20));
        let work = flow.clone();
        let guard = super::super::source_adapter::preparation_guard(work.clone());
        let task = tokio::spawn(async move {
            let _guard = guard;
            work.prepare("hold.invalid", "80").await
        });
        entered(&runtime).await?;
        let held = runtime.inner.closer.lock().await;
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let deadline = locked(&flow.inner.requests).cleanup_deadline.unwrap();
        tokio::time::sleep_until(deadline).await;
        // Delaying the FIRST poll must not buy another five seconds, even when
        // the original JoinSet closer is still owned by another operation.
        let mut delayed = Box::pin(flow.cancel_and_wait());
        std::future::poll_fn(|cx| {
            use std::future::Future;
            assert!(
                matches!(delayed.as_mut().poll(cx), std::task::Poll::Ready(Err(error))
                if error.retirement_timed_out),
                "first delayed poll has no fresh grace"
            );
            std::task::Poll::Ready(())
        })
        .await;
        drop(delayed);
        assert_eq!(
            locked(&flow.inner.requests).cleanup_deadline,
            Some(deadline)
        );
        let mut again = Box::pin(flow.cancel_and_wait());
        std::future::poll_fn(|cx| {
            use std::future::Future;
            assert!(
                matches!(again.as_mut().poll(cx), std::task::Poll::Ready(Err(_))),
                "repeat cleanup returns cached failure, not a fresh timeout/IO"
            );
            std::task::Poll::Ready(())
        })
        .await;
        drop(again);
        drop(held);
        runtime.close().await?;
        assert_eq!(runtime.ownership_snapshot(), (true, 0, vec![]));
        let closed = runtime.source_flow(tokio::time::Instant::now() + Duration::from_secs(1));
        assert_eq!(
            closed.prepare("127.0.0.1", "80").await.err().unwrap().stage,
            SourceResolveStage::Closed,
            "numeric source fast path must honor application admission closure"
        );
        closed.cancel_and_wait().await?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn literal_compressed_text_has_its_own_mapped_and_compatible_branches() {
        use std::net::IpAddr;
        assert_eq!(
            super::literal_ip_text("::192.0.2.1".parse::<IpAddr>().unwrap()),
            "::c000:201"
        );
        assert_eq!(
            super::literal_ip_text("::ffff:c000:201".parse::<IpAddr>().unwrap()),
            "::ffff:192.0.2.1"
        );
        assert_eq!(
            super::scoped_literal("::192.0.2.1%zone").as_deref(),
            Some("::c000:201%zone")
        );
    }
    #[test]
    fn anyio_scoped_literal_compression_keeps_source_scope() {
        assert_eq!(
            super::scoped_literal("0:0:0:0:0:0:0:1%lo0").as_deref(),
            Some("::1%lo0")
        );
        assert_eq!(
            super::scoped_literal("0:0:0:0:0:ffff:c000:201%3").as_deref(),
            Some("::ffff:192.0.2.1%3")
        );
        assert_eq!(super::scoped_literal("example.invalid%scope"), None);
    }
}
