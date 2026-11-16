//! One trusted-composition byte account. No wire/config/environment constructor
//! selects a capacity or asserts that a supported operation class fits it.
//! Coupled composition MUST validate its full pre-growth operation-class bound.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Overflow,
    Saturated,
    Allocation,
    InvalidTransfer,
}
/// Shared once across requests AND all three transports of one composition.
/// Capacity is requested backing, not RSS or proof of OS physical availability.
pub struct Account {
    capacity: usize,
    used: AtomicUsize,
    high_water: AtomicUsize,
}
impl Account {
    pub fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            capacity,
            used: AtomicUsize::new(0),
            high_water: AtomicUsize::new(0),
        })
    }
    pub fn used(&self) -> usize {
        self.used.load(Ordering::SeqCst)
    }
    pub fn high_water(&self) -> usize {
        self.high_water.load(Ordering::SeqCst)
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    /// Acquire BEFORE allocation, never measure-then-admit. This call cannot
    /// wait while retaining a partly built capture; saturation is explicit.
    pub fn reserve(self: &Arc<Self>, bytes: usize) -> Result<Reservation, Refusal> {
        let mut previous = self.used.load(Ordering::SeqCst);
        loop {
            let next = previous.checked_add(bytes).ok_or(Refusal::Overflow)?;
            if next > self.capacity {
                return Err(Refusal::Saturated);
            }
            match self.used.compare_exchange_weak(
                previous,
                next,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    self.high_water.fetch_max(next, Ordering::SeqCst);
                    return Ok(Reservation {
                        account: self.clone(),
                        bytes,
                    });
                }
                Err(actual) => previous = actual,
            }
        }
    }
}
/// Affine credit; not Clone. Sharing an allocation means sharing the owner that
/// contains this reservation, never cloning/refunding its charge independently.
pub struct Reservation {
    account: Arc<Account>,
    bytes: usize,
}
impl Reservation {
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn account(&self) -> &Arc<Account> {
        &self.account
    }
    /// Transfer already reserved bytes to a disjoint allocation owner. This
    /// neither increases global credit nor returns credit while memory is live.
    pub fn split(&mut self, bytes: usize) -> Result<Self, Refusal> {
        if bytes > self.bytes {
            return Err(Refusal::InvalidTransfer);
        }
        self.bytes -= bytes;
        Ok(Self {
            account: self.account.clone(),
            bytes,
        })
    }
    /// Only after the corresponding old allocation is destroyed/never allocated.
    /// Vec::clear and an outstanding Bytes clone do NOT authorize this call.
    pub fn release_unused(&mut self, bytes: usize) -> Result<(), Refusal> {
        if bytes > self.bytes {
            return Err(Refusal::InvalidTransfer);
        }
        self.bytes -= bytes;
        let previous = self.account.used.fetch_sub(bytes, Ordering::SeqCst);
        assert!(
            previous >= bytes,
            "private reservation accounting invariant"
        );
        Ok(())
    }
    pub fn same_account(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.account, &other.account)
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let previous = self.account.used.fetch_sub(self.bytes, Ordering::SeqCst);
        assert!(
            previous >= self.bytes,
            "private reservation accounting invariant"
        );
    }
}
/// Per-operation suballocation of ONE reservation acquired before any backend
/// dispatch. This is not another global quota/authority. Claim drops can reuse
/// pre-admitted scratch while the global reservation stays owned by Scope.
/// Backend driver, capture owner and emitted chunks keep this same Arc alive.
/// There is no small default/class-size inference in this primitive.
pub struct Scope {
    credit: Reservation,
    claimed: AtomicUsize,
    high_water: AtomicUsize,
}
impl Scope {
    /// Caller must first reserve the complete reviewed operation-class peak,
    /// progress and baseline-retirement bands. No wire or tool can mint Scope.
    /// Constructor/header backing itself belongs to that source-derived class.
    pub fn from_pre_reserved(credit: Reservation) -> Arc<Self> {
        Arc::new(Self {
            credit,
            claimed: AtomicUsize::new(0),
            high_water: AtomicUsize::new(0),
        })
    }
    pub fn claimed(&self) -> usize {
        self.claimed.load(Ordering::SeqCst)
    }
    pub fn high_water(&self) -> usize {
        self.high_water.load(Ordering::SeqCst)
    }
    pub fn reserved(&self) -> usize {
        self.credit.bytes()
    }
    pub fn claim(self: &Arc<Self>, bytes: usize) -> Result<Claim, Refusal> {
        let mut previous = self.claimed.load(Ordering::SeqCst);
        loop {
            let next = previous.checked_add(bytes).ok_or(Refusal::Overflow)?;
            if next > self.credit.bytes() {
                return Err(Refusal::InvalidTransfer);
            }
            match self.claimed.compare_exchange_weak(
                previous,
                next,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    self.high_water.fetch_max(next, Ordering::SeqCst);
                    return Ok(Claim {
                        scope: self.clone(),
                        bytes,
                    });
                }
                Err(actual) => previous = actual,
            }
        }
    }
}
/// Affine requested backing within the already admitted operation scope.
/// Fields owning allocations must be dropped BEFORE this token. Returning a
/// claim never returns global bytes while ANY driver/capture/chunk holds Scope.
pub struct Claim {
    scope: Arc<Scope>,
    bytes: usize,
}
impl Claim {
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn scope(&self) -> &Arc<Scope> {
        &self.scope
    }
    pub fn reserve_replacement(
        &self,
        new_capacity: usize,
        element_bytes: usize,
    ) -> Result<Self, Refusal> {
        self.scope.claim(
            new_capacity
                .checked_mul(element_bytes)
                .ok_or(Refusal::Overflow)?,
        )
    }
}
impl Drop for Claim {
    fn drop(&mut self) {
        let previous = self.scope.claimed.fetch_sub(self.bytes, Ordering::SeqCst);
        assert!(previous >= self.bytes, "private claim invariant");
    }
}
// Poll-local ferry for trusted native operation composition. It does not spawn
// a task or put authority in wire data. Every Pending/Ready/unwind restores the
// previous poll's scope; a different task on the same thread cannot inherit it.
// Spawned backend drivers must explicitly retain the captured Arc separately.
thread_local! {
    static POLL_SCOPE: std::cell::RefCell<Option<Arc<Scope>>> = const { std::cell::RefCell::new(None) };
}
pub fn current_scope() -> Option<Arc<Scope>> {
    POLL_SCOPE.with(|slot| slot.borrow().clone())
}
struct RestorePollScope(Option<Arc<Scope>>);
impl Drop for RestorePollScope {
    fn drop(&mut self) {
        POLL_SCOPE.with(|slot| {
            *slot.borrow_mut() = self.0.take();
        });
    }
}
/// One existing owned operation future is wrapped, rather than replaced by an
/// independently scheduled dispatcher. Scope remains in this future at Pending.
pub async fn with_scope<F: std::future::Future>(scope: Arc<Scope>, future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|cx| {
        let previous = POLL_SCOPE.with(|slot| slot.replace(Some(scope.clone())));
        let _restore = RestorePollScope(previous);
        future.as_mut().poll(cx)
    })
    .await
}
/// A replacement reservation deliberately overlaps old backing. The caller
/// first constructs the new allocation, destroys old allocation+reservation,
/// then transfers this reservation to the new owner. No refund on clear().
pub fn reserve_replacement(
    old: &Reservation,
    new_capacity: usize,
    element_bytes: usize,
) -> Result<Reservation, Refusal> {
    let bytes = new_capacity
        .checked_mul(element_bytes)
        .ok_or(Refusal::Overflow)?;
    old.account.reserve(bytes)
}
/// Heap scratch bound for existing FloatNormalizer::normalized String growth:
/// at most raw_len significant bytes + punctuation/exponent suffix, doubled
/// growth, and simultaneous old/new buffers. The pinned allocator/Vec growth
/// assumptions and std float-conversion scratch remain explicit proof bindings.
/// This is NOT a claim that F05's32-byte formatter bounds decimal conversion.
pub fn decimal_growth_reservation(raw_len: usize) -> Result<usize, Refusal> {
    raw_len
        .checked_add(32)
        .and_then(usize::checked_next_power_of_two)
        .and_then(|n| n.checked_mul(2))
        .ok_or(Refusal::Overflow)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reserve_before_growth_overlaps_old_and_new_and_refuses_without_mutation() {
        let a = Account::new(100);
        let old = a.reserve(40).unwrap();
        let new = reserve_replacement(&old, 15, 4).unwrap();
        assert_eq!(a.used(), 100);
        assert_eq!(a.high_water(), 100);
        assert!(matches!(a.reserve(1), Err(Refusal::Saturated)));
        assert_eq!(a.used(), 100);
        drop(old);
        assert_eq!(a.used(), 60);
        drop(new);
        assert_eq!(a.used(), 0);
        assert!(matches!(a.reserve(usize::MAX), Err(Refusal::Saturated)));
        let held = a.reserve(1).unwrap();
        assert!(matches!(a.reserve(usize::MAX), Err(Refusal::Overflow)));
        drop(held);
        assert_eq!(a.used(), 0);
    }
    #[test]
    fn affine_split_and_shared_owner_release_only_at_actual_last_drop() {
        struct Owner {
            bytes: Box<[u8]>,
            _credit: Reservation,
        }
        let a = Account::new(64);
        let mut admitted = a.reserve(64).unwrap();
        let credit = admitted.split(16).unwrap();
        let owner = Arc::new(Owner {
            bytes: vec![0; 16].into_boxed_slice(),
            _credit: credit,
        });
        let observer = Arc::downgrade(&owner);
        let clone = owner.clone();
        assert_eq!(owner.bytes.len(), 16);
        drop(owner);
        assert_eq!(a.used(), 64);
        admitted.release_unused(48).unwrap();
        assert_eq!(a.used(), 16);
        assert!(observer.upgrade().is_some());
        drop(clone);
        assert!(observer.upgrade().is_none());
        assert_eq!(a.used(), 0);
    }
    #[test]
    fn driver_and_capture_share_pre_dispatch_reservation_until_actual_last_owner() {
        let a = Account::new(128);
        let scope = Scope::from_pre_reserved(a.reserve(128).unwrap());
        let driver = scope.clone();
        let input = scope.claim(48).unwrap();
        let replacement = input.reserve_replacement(64, 1).unwrap();
        assert_eq!(scope.claimed(), 112);
        assert_eq!(scope.high_water(), 112);
        assert!(matches!(scope.claim(17), Err(Refusal::InvalidTransfer)));
        drop(input);
        drop(replacement);
        assert_eq!(scope.claimed(), 0);
        let capture = scope.clone();
        drop(scope);
        assert_eq!(a.used(), 128);
        drop(capture);
        assert_eq!(a.used(), 128);
        drop(driver);
        assert_eq!(a.used(), 0);
    }
    #[test]
    fn checked_capacity_and_normalizer_growth_are_not_float_formatter_length() {
        let a = Account::new(128);
        let credit = a.reserve(1).unwrap();
        assert!(matches!(
            reserve_replacement(&credit, usize::MAX, 2),
            Err(Refusal::Overflow)
        ));
        assert!(matches!(
            decimal_growth_reservation(usize::MAX),
            Err(Refusal::Overflow)
        ));
        assert!(decimal_growth_reservation(4 * 1024 * 1024).unwrap() > 4 * 1024 * 1024);
        assert_eq!(a.used(), 1);
    }
    #[test]
    fn interleaved_owned_futures_restore_scope_on_every_pending_ready_and_drop() {
        use std::{
            future::Future,
            task::{Context, Poll, Waker},
        };
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let a = Account::new(256);
        let first = Scope::from_pre_reserved(a.reserve(64).unwrap());
        let second = Scope::from_pre_reserved(a.reserve(128).unwrap());
        let make = |expected: Arc<Scope>| {
            let mut polls = 0;
            std::future::poll_fn(move |_| {
                assert!(Arc::ptr_eq(&current_scope().unwrap(), &expected));
                polls += 1;
                if polls == 1 {
                    Poll::Pending
                } else {
                    Poll::Ready(polls)
                }
            })
        };
        let mut one = std::pin::pin!(with_scope(first.clone(), make(first.clone())));
        let mut two = std::pin::pin!(with_scope(second.clone(), make(second.clone())));
        assert!(current_scope().is_none());
        assert!(one.as_mut().poll(&mut cx).is_pending());
        assert!(current_scope().is_none());
        assert!(two.as_mut().poll(&mut cx).is_pending());
        assert!(current_scope().is_none());
        assert_eq!(one.as_mut().poll(&mut cx), Poll::Ready(2));
        assert!(current_scope().is_none());
        assert_eq!(two.as_mut().poll(&mut cx), Poll::Ready(2));
        assert!(current_scope().is_none());
        // Nested scope restoration uses the outer identity, never a last-writer
        // slot shared by concurrently dispatched Redis pipeline messages.
        let outer = first.clone();
        let inner = second.clone();
        let nested = async {
            assert!(Arc::ptr_eq(&current_scope().unwrap(), &outer));
            with_scope(inner.clone(), async {
                assert!(Arc::ptr_eq(&current_scope().unwrap(), &inner));
            })
            .await;
            assert!(Arc::ptr_eq(&current_scope().unwrap(), &outer));
        };
        let mut nested = std::pin::pin!(with_scope(first.clone(), nested));
        assert!(nested.as_mut().poll(&mut cx).is_ready());
        assert!(current_scope().is_none());
    }
}
