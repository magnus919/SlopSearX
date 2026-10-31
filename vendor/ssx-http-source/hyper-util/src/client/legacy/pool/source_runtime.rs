//! Source-only runtime owner; every authority mutation uses PoolInner's mutex.
use super::source::{
    AssignmentKind, EntryId, Lease, MountId, RequestState, SourceLedger, WaiterId,
};
use super::*;
use futures_util::future::{AbortHandle, AbortRegistration, Abortable};
use futures_util::task::AtomicWaker;
use std::future::{Future, poll_fn};
use std::num::NonZeroUsize;
use std::task::{Context, Waker};

mod io;
pub(crate) use io::PhysicalIo;

type BoxError = Box<dyn StdError + Send + Sync>;

fn now() -> Instant {
    tokio::time::Instant::now().into_std()
}

/// Fixed source HTTP/1 limits with an independent assignment wait budget.
#[derive(Clone, Debug)]
pub struct SourceHttp1PoolConfig {
    wait: Duration,
    mounts: NonZeroUsize,
}
impl SourceHttp1PoolConfig {
    /// Configure P and the finite trusted transport-mount registration count.
    pub fn new(wait: Duration, mounts: NonZeroUsize) -> Self {
        Self { wait, mounts }
    }
}
/// Token for one registered transport mount in one concrete pool authority.
#[derive(Clone)]
pub struct SourceMount {
    authority: Arc<()>,
    id: MountId,
}
impl Debug for SourceMount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceMount").finish_non_exhaustive()
    }
}
/// Independent assignment wait P expired before any connection was assigned.
#[derive(Clone, Copy, Debug)]
pub struct SourcePoolTimedOut;
impl fmt::Display for SourcePoolTimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("source pool assignment timed out")
    }
}
impl StdError for SourcePoolTimedOut {}
/// Source pool is closed or a mount token belongs to another authority.
#[derive(Clone, Copy, Debug)]
pub struct SourcePoolClosed;
impl fmt::Display for SourcePoolClosed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("source pool closed or mount unavailable")
    }
}
impl StdError for SourcePoolClosed {}

trait Control: Send + Sync {
    fn close(&self) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}
/// Shared explicit close/join authority. Drop alone is not a join witness.
#[derive(Clone)]
pub struct SourcePoolControl {
    authority: Arc<()>,
    mounts: usize,
    backend: Arc<dyn Control>,
}
impl Debug for SourcePoolControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourcePoolControl").finish_non_exhaustive()
    }
}
impl SourcePoolControl {
    /// Return a token for an already configured mount; this never registers one.
    pub fn mount(&self, index: usize) -> Option<SourceMount> {
        (index < self.mounts).then(|| SourceMount {
            authority: self.authority.clone(),
            id: MountId(index as u64 + 1),
        })
    }
    /// Close physical IO and await all owned resources, including held upgrades.
    /// A caller retaining Upgraded IO can keep this future pending until drop.
    pub async fn close(&self) {
        self.backend.close().await
    }
}
struct Backend<T: Poolable, K: Key>(Pool<T, K>);
impl<T: Poolable, K: Key> Control for Backend<T, K> {
    fn close(&self) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let pool = self.0.clone();
        let mut waiter = CloseWaiter {
            pool: Arc::downgrade(pool.inner.as_ref().unwrap()),
            id: None,
        };
        Box::pin(async move {
            poll_fn(move |cx| {
                let mut actions = Actions::new();
                let mut new_wake = Some(cx.waker().clone());
                let mut replaced_wake = None;
                let done = {
                    let mut inner = pool.inner.as_ref().unwrap().lock().unwrap();
                    let runtime = inner.source.as_mut().unwrap();
                    if !runtime.closed {
                        runtime.closed = true;
                        for index in 0..runtime.mounts {
                            let id = MountId(index as u64 + 1);
                            let effects = runtime.ledger.close_mount(id);
                            runtime.effects(id, effects, &mut actions);
                        }
                    }
                    if runtime.entries.is_empty() {
                        true
                    } else {
                        let id = waiter.registration_id(runtime);
                        if !runtime
                            .close_waiters
                            .get(&id)
                            .is_some_and(|w| w.will_wake(cx.waker()))
                        {
                            replaced_wake =
                                runtime.close_waiters.insert(id, new_wake.take().unwrap());
                        }
                        false
                    }
                };
                drop(new_wake);
                drop(replaced_wake);
                actions.run();
                if done { Poll::Ready(()) } else { Poll::Pending }
            })
            .await;
        })
    }
}

// One registration per close future. Cancellation removes its retained waker;
// replacing or removing a waker must never run its destructor under authority.
struct CloseWaiter<T: Poolable, K: Key> {
    pool: Weak<Mutex<PoolInner<T, K>>>,
    id: Option<u64>,
}
impl<T: Poolable, K: Key> CloseWaiter<T, K> {
    // Using the whole guard keeps its Drop implementation in the close future;
    // capturing only its Copy id field would leave cancellation unregistered.
    fn registration_id(&mut self, runtime: &mut Runtime<T, K>) -> u64 {
        *self.id.get_or_insert_with(|| {
            runtime.next_close_waiter = runtime
                .next_close_waiter
                .checked_add(1)
                .expect("source close waiter identity exhausted");
            runtime.next_close_waiter
        })
    }
}
impl<T: Poolable, K: Key> Drop for CloseWaiter<T, K> {
    fn drop(&mut self) {
        let removed = match (self.pool.upgrade(), self.id.take()) {
            (Some(pool), Some(id)) => {
                let mut inner = pool.lock().unwrap();
                inner.source.as_mut().unwrap().close_waiters.remove(&id)
            }
            _ => None,
        };
        drop(removed);
    }
}

struct Entry {
    lease: Lease,
    resources: HashMap<u64, Option<AbortHandle>>,
    closers: HashMap<u64, io::Closer>,
    retiring: bool,
}
pub(super) struct Runtime<T, K: Eq + Hash> {
    ledger: SourceLedger<K>,
    authority: Arc<()>,
    mounts: usize,
    wait: Duration,
    closed: bool,
    idle: HashMap<(MountId, EntryId), T>,
    entries: HashMap<(MountId, EntryId), Entry>,
    waiters: HashMap<(MountId, WaiterId), Arc<AtomicWaker>>,
    next_resource: u64,
    next_close_waiter: u64,
    close_waiters: HashMap<u64, Waker>,
}
struct Actions<T> {
    values: Vec<T>,
    retired: Vec<Entry>,
    abort: Vec<AbortHandle>,
    close: Vec<io::Closer>,
    wake: Vec<Arc<AtomicWaker>>,
    close_wake: Vec<Waker>,
    retired_waiters: Vec<Arc<AtomicWaker>>,
}
impl<T> Actions<T> {
    fn new() -> Self {
        Self {
            values: Vec::new(),
            retired: Vec::new(),
            abort: Vec::new(),
            close: Vec::new(),
            wake: Vec::new(),
            close_wake: Vec::new(),
            retired_waiters: Vec::new(),
        }
    }
    fn run(self) {
        // No pool mutex is held while any value/future/IO/callback is dropped.
        for abort in self.abort {
            abort.abort();
        }
        for close in self.close {
            close();
        }
        drop(self.values);
        drop(self.retired);
        drop(self.retired_waiters);
        for wake in self.wake {
            wake.wake();
        }
        for wake in self.close_wake {
            wake.wake();
        }
    }
}
impl<T, K: Key> Runtime<T, K> {
    fn effects(&mut self, mount: MountId, effects: source::Effects, actions: &mut Actions<T>) {
        let mut pending = VecDeque::from([effects]);
        while let Some(effects) = pending.pop_front() {
            for waiter in effects.notify {
                if let Some(wake) = self.waiters.get(&(mount, waiter)) {
                    actions.wake.push(wake.clone());
                }
            }
            for lease in effects.retire {
                let key = (mount, lease.entry);
                if let Some(value) = self.idle.remove(&key) {
                    actions.values.push(value);
                }
                if let Some(entry) = self.entries.get_mut(&key) {
                    entry.retiring = true;
                    actions
                        .abort
                        .extend(entry.resources.values().flatten().cloned());
                    actions.close.extend(entry.closers.values().cloned());
                    if !entry.resources.is_empty() {
                        continue;
                    }
                }
                if let Some(entry) = self.entries.remove(&key) {
                    actions.retired.push(entry);
                }
                if let Some(next) = self.ledger.finish_retirement(mount, lease, now()) {
                    pending.push_back(next);
                }
            }
        }
        // Assigned waiters blocked on required physical cleanup also wake when
        // a resource completes, even if their assignment itself did not change.
        actions.wake.extend(self.waiters.values().cloned());
        if self.closed && self.entries.is_empty() {
            actions
                .close_wake
                .extend(self.close_waiters.drain().map(|(_, wake)| wake));
        }
    }
}

pub(crate) struct LeaseOwner<T: Poolable, K: Key> {
    pool: Pool<T, K>,
    mount: MountId,
    lease: Lease,
}
impl<T: Poolable, K: Key> Clone for LeaseOwner<T, K> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            mount: self.mount,
            lease: self.lease,
        }
    }
}
struct Completion<T: Poolable, K: Key> {
    pool: Weak<Mutex<PoolInner<T, K>>>,
    mount: MountId,
    entry: EntryId,
    resource: u64,
    physical: bool,
}
impl<T: Poolable, K: Key> Drop for Completion<T, K> {
    fn drop(&mut self) {
        let Some(pool) = self.pool.upgrade() else {
            return;
        };
        let mut actions = Actions::new();
        {
            let mut inner = pool.lock().unwrap();
            let runtime = inner.source.as_mut().unwrap();
            let key = (self.mount, self.entry);
            let Some(entry) = runtime.entries.get_mut(&key) else {
                return;
            };
            if entry.resources.remove(&self.resource).is_none() {
                return;
            }
            if let Some(closer) = entry.closers.remove(&self.resource) {
                actions.close.push(closer);
            }
            let lease = entry.lease;
            let retiring = entry.retiring;
            if self.physical && !retiring {
                if let Some(effects) = runtime.ledger.retire_connection(self.mount, lease) {
                    runtime.effects(self.mount, effects, &mut actions);
                }
            } else if retiring
                && runtime
                    .entries
                    .get(&key)
                    .is_some_and(|e| e.resources.is_empty())
            {
                let mut effects = source::Effects::default();
                effects.retire.push(lease);
                runtime.effects(self.mount, effects, &mut actions);
            } else {
                actions.wake.extend(runtime.waiters.values().cloned());
            }
        }
        actions.run();
    }
}
pub(crate) struct SourceJob<T: Poolable, K: Key> {
    registration: AbortRegistration,
    completion: Completion<T, K>,
}
// Explicit owner order also covers an executor dropping an UNPOLLED job.
// Async argument/capture drop order must not release a completion token first.
pub(crate) struct OwnedJob<F: Future, T: Poolable, K: Key> {
    future: Option<Pin<Box<Abortable<F>>>>,
    completion: Option<Completion<T, K>>,
}
impl<T: Poolable, K: Key> SourceJob<T, K> {
    pub(crate) fn run<F: Future>(self, future: F) -> OwnedJob<F, T, K> {
        OwnedJob {
            future: Some(Box::pin(Abortable::new(future, self.registration))),
            completion: Some(self.completion),
        }
    }
}
impl<F: Future, T: Poolable, K: Key> Future for OwnedJob<F, T, K> {
    type Output = Result<F::Output, SourcePoolClosed>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match this
            .future
            .as_mut()
            .expect("source job polled after completion")
            .as_mut()
            .poll(cx)
        {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                drop(this.future.take());
                drop(this.completion.take());
                Poll::Ready(result.map_err(|_| SourcePoolClosed))
            }
        }
    }
}
impl<F: Future, T: Poolable, K: Key> Drop for OwnedJob<F, T, K> {
    fn drop(&mut self) {
        drop(self.future.take());
        drop(self.completion.take());
    }
}

impl<T: Poolable, K: Key> LeaseOwner<T, K> {
    pub(crate) fn job(&self) -> Result<SourceJob<T, K>, SourcePoolClosed> {
        let (abort, registration) = AbortHandle::new_pair();
        let completion = self.register(Some(abort), false)?;
        Ok(SourceJob {
            registration,
            completion,
        })
    }
    fn register(
        &self,
        abort: Option<AbortHandle>,
        physical: bool,
    ) -> Result<Completion<T, K>, SourcePoolClosed> {
        let pool = self.pool.inner.as_ref().unwrap();
        let mut inner = pool.lock().unwrap();
        let runtime = inner.source.as_mut().unwrap();
        if runtime.closed {
            return Err(SourcePoolClosed);
        }
        let entry = runtime
            .entries
            .get_mut(&(self.mount, self.lease.entry))
            .ok_or(SourcePoolClosed)?;
        if entry.retiring || entry.lease != self.lease {
            return Err(SourcePoolClosed);
        }
        runtime.next_resource = runtime
            .next_resource
            .checked_add(1)
            .expect("source resource identity exhausted");
        let resource = runtime.next_resource;
        entry.resources.insert(resource, abort);
        Ok(Completion {
            pool: Arc::downgrade(pool),
            mount: self.mount,
            entry: self.lease.entry,
            resource,
            physical,
        })
    }
    pub(crate) fn wrap_io<I: Send + 'static>(
        &self,
        io: I,
    ) -> Result<PhysicalIo<I>, SourcePoolClosed> {
        let completion = self.register(None, true)?;
        let resource = completion.resource;
        let (io, closer) = PhysicalIo::new(io, move || drop(completion));
        let accepted = {
            let mut inner = self.pool.inner.as_ref().unwrap().lock().unwrap();
            let runtime = inner.source.as_mut().unwrap();
            if let Some(entry) = runtime.entries.get_mut(&(self.mount, self.lease.entry)) {
                if !entry.retiring && !runtime.closed {
                    entry.closers.insert(resource, closer.clone());
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };
        if accepted {
            Ok(io)
        } else {
            closer();
            drop(io);
            Err(SourcePoolClosed)
        }
    }
    pub(crate) fn retire(&self) {
        let mut actions = Actions::new();
        {
            let mut inner = self.pool.inner.as_ref().unwrap().lock().unwrap();
            let runtime = inner.source.as_mut().unwrap();
            if let Some(effects) = runtime.ledger.retire_connection(self.mount, self.lease) {
                runtime.effects(self.mount, effects, &mut actions);
            }
        }
        actions.run();
    }
    pub(crate) fn return_idle(self, value: T) {
        // Readiness checks may poll native state and are outside the authority.
        if !value.is_open() {
            self.retire();
            drop(value);
            return;
        }
        let mut value = Some(value);
        let mut actions = Actions::new();
        {
            let mut inner = self.pool.inner.as_ref().unwrap().lock().unwrap();
            let runtime = inner.source.as_mut().unwrap();
            if let Some(effects) = runtime.ledger.release_idle(self.mount, self.lease, now()) {
                if let Some(old) = runtime
                    .idle
                    .insert((self.mount, self.lease.entry), value.take().unwrap())
                {
                    actions.values.push(old);
                }
                runtime.effects(self.mount, effects, &mut actions);
            }
        }
        drop(value);
        actions.run();
    }
    pub(crate) fn upgraded(&self) {
        let mut inner = self.pool.inner.as_ref().unwrap().lock().unwrap();
        inner
            .source
            .as_mut()
            .unwrap()
            .ledger
            .upgrade(self.mount, self.lease);
    }
}
pub(crate) struct Reservation<T: Poolable, K: Key> {
    owner: Option<LeaseOwner<T, K>>,
    key: Option<K>,
}
impl<T: Poolable, K: Key> Reservation<T, K> {
    pub(crate) fn owner(&self) -> &LeaseOwner<T, K> {
        self.owner.as_ref().unwrap()
    }
    pub(crate) fn pooled(mut self, value: T) -> Result<Pooled<T, K>, SourcePoolClosed> {
        let owner = self.owner.as_ref().unwrap();
        let active = {
            let mut inner = owner.pool.inner.as_ref().unwrap().lock().unwrap();
            inner
                .source
                .as_mut()
                .unwrap()
                .ledger
                .activate(owner.mount, owner.lease)
        };
        if !active {
            drop(value);
            return Err(SourcePoolClosed);
        }
        let owner = self.owner.take().unwrap();
        Ok(Pooled {
            value: Some(value),
            is_reused: false,
            key: self.key.take().unwrap(),
            pool: WeakOpt::none(),
            source: Some(owner),
        })
    }
}
impl<T: Poolable, K: Key> Drop for Reservation<T, K> {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            owner.retire();
        }
    }
}

pub(crate) enum Acquired<T: Poolable, K: Key> {
    Reused(Pooled<T, K>),
    Connect(Reservation<T, K>),
}
pub(crate) struct SourceAcquire<T: Poolable, K: Key> {
    pool: Pool<T, K>,
    mount: MountId,
    key: K,
    waiter: Option<WaiterId>,
    wake: Arc<AtomicWaker>,
    timer: Option<(u64, Pin<Box<tokio::time::Sleep>>)>,
}
impl<T: Poolable, K: Key> Future for SourceAcquire<T, K> {
    type Output = Result<Acquired<T, K>, BoxError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        this.wake.register(cx.waker());
        let mut actions = Actions::new();
        let mut assigned = None;
        let state = {
            let mut inner = this.pool.inner.as_ref().unwrap().lock().unwrap();
            let runtime = inner.source.as_mut().unwrap();
            if runtime.closed {
                None
            } else {
                if this.waiter.is_none() {
                    let (waiter, effects) = runtime
                        .ledger
                        .request(this.mount, this.key.clone(), now())
                        .map_err(|_| SourcePoolClosed)?;
                    this.waiter = Some(waiter);
                    runtime
                        .waiters
                        .insert((this.mount, waiter), this.wake.clone());
                    runtime.effects(this.mount, effects, &mut actions);
                }
                let waiter = this.waiter.unwrap();
                let state = runtime.ledger.state(this.mount, waiter);
                if let Some(RequestState::Assigned(assignment)) = state {
                    // Required retirement work finishes before any actual dial.
                    if !runtime
                        .entries
                        .iter()
                        .any(|((mount, _), entry)| *mount == this.mount && entry.retiring)
                    {
                        runtime
                            .entries
                            .entry((this.mount, assignment.lease.entry))
                            .and_modify(|entry| entry.lease = assignment.lease)
                            .or_insert_with(|| Entry {
                                lease: assignment.lease,
                                resources: HashMap::new(),
                                closers: HashMap::new(),
                                retiring: false,
                            });
                        match assignment.kind {
                            AssignmentKind::Connect => {
                                if runtime.ledger.begin_connect(this.mount, assignment.lease) {
                                    assigned = Some((assignment, None));
                                }
                            }
                            AssignmentKind::Reuse => {
                                if let Some(value) =
                                    runtime.idle.remove(&(this.mount, assignment.lease.entry))
                                {
                                    if runtime.ledger.activate_reused(this.mount, assignment.lease)
                                    {
                                        assigned = Some((assignment, Some(value)));
                                    } else {
                                        actions.values.push(value);
                                    }
                                }
                            }
                        }
                    }
                }
                state
            }
        };
        actions.run();
        if let Some((assignment, value)) = assigned {
            let waiter = this.waiter.take().unwrap();
            this.timer = None;
            let owner = LeaseOwner {
                pool: this.pool.clone(),
                mount: this.mount,
                lease: assignment.lease,
            };
            let retired_wake = {
                let mut inner = this.pool.inner.as_ref().unwrap().lock().unwrap();
                inner
                    .source
                    .as_mut()
                    .unwrap()
                    .waiters
                    .remove(&(this.mount, waiter))
            };
            drop(retired_wake);
            return Poll::Ready(Ok(match value {
                Some(value) => Acquired::Reused(Pooled {
                    value: Some(value),
                    is_reused: true,
                    key: this.key.clone(),
                    pool: WeakOpt::none(),
                    source: Some(owner),
                }),
                None => Acquired::Connect(Reservation {
                    owner: Some(owner),
                    key: Some(this.key.clone()),
                }),
            }));
        }
        match state {
            None => Poll::Ready(Err(Box::new(SourcePoolClosed))),
            Some(RequestState::Queued { epoch }) => {
                if this.timer.as_ref().is_none_or(|(old, _)| *old != epoch) {
                    let wait = this
                        .pool
                        .inner
                        .as_ref()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .source
                        .as_ref()
                        .unwrap()
                        .wait;
                    this.timer = Some((epoch, Box::pin(tokio::time::sleep(wait))));
                }
                if this.timer.as_mut().unwrap().1.as_mut().poll(cx).is_ready() {
                    let waiter = this.waiter.unwrap();
                    let expired = this
                        .pool
                        .inner
                        .as_ref()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .source
                        .as_mut()
                        .unwrap()
                        .ledger
                        .expire_wait(this.mount, waiter, epoch);
                    if expired {
                        return Poll::Ready(Err(Box::new(SourcePoolTimedOut)));
                    }
                    cx.waker().wake_by_ref();
                }
                Poll::Pending
            }
            _ => {
                this.timer = None;
                Poll::Pending
            }
        }
    }
}
impl<T: Poolable, K: Key> Drop for SourceAcquire<T, K> {
    fn drop(&mut self) {
        let Some(waiter) = self.waiter.take() else {
            return;
        };
        let mut actions = Actions::new();
        {
            let mut inner = self.pool.inner.as_ref().unwrap().lock().unwrap();
            let runtime = inner.source.as_mut().unwrap();
            if let Some(wake) = runtime.waiters.remove(&(self.mount, waiter)) {
                actions.retired_waiters.push(wake);
            }
            if let Some(state) = runtime.ledger.state(self.mount, waiter) {
                let effects = runtime.ledger.cancel(self.mount, waiter, state, now());
                runtime.effects(self.mount, effects, &mut actions);
            }
        }
        actions.run();
    }
}
impl<T: Poolable, K: Key> Pool<T, K> {
    pub(crate) fn source_enable(&self, config: SourceHttp1PoolConfig) -> SourcePoolControl {
        let authority = Arc::new(());
        let mounts = config.mounts.get();
        let mut ledger = SourceLedger::new();
        for index in 0..mounts {
            ledger.register_mount(MountId(index as u64 + 1));
        }
        self.inner.as_ref().unwrap().lock().unwrap().source = Some(Runtime {
            ledger,
            authority: authority.clone(),
            mounts,
            wait: config.wait,
            closed: false,
            idle: HashMap::new(),
            entries: HashMap::new(),
            waiters: HashMap::new(),
            next_resource: 0,
            next_close_waiter: 0,
            close_waiters: HashMap::new(),
        });
        SourcePoolControl {
            authority,
            mounts,
            backend: Arc::new(Backend(self.clone())),
        }
    }
    pub(crate) fn source_enabled(&self) -> bool {
        self.inner
            .as_ref()
            .is_some_and(|p| p.lock().unwrap().source.is_some())
    }
    pub(crate) fn source_acquire(
        &self,
        key: K,
        mount: &SourceMount,
    ) -> Result<SourceAcquire<T, K>, SourcePoolClosed> {
        let valid = {
            let inner = self.inner.as_ref().unwrap().lock().unwrap();
            inner.source.as_ref().is_some_and(|runtime| {
                !runtime.closed
                    && Arc::ptr_eq(&runtime.authority, &mount.authority)
                    && mount.id.0 > 0
                    && mount.id.0 <= runtime.mounts as u64
            })
        };
        if !valid {
            return Err(SourcePoolClosed);
        }
        Ok(SourceAcquire {
            pool: self.clone(),
            mount: mount.id,
            key,
            waiter: None,
            wake: Arc::new(AtomicWaker::new()),
            timer: None,
        })
    }
}
