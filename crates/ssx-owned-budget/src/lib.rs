//! Requested backing-storage accounting, generalized from A05 html_budget.rs.
//! Not an OS RSS limiter. Admission, process containment and cross-process
//! allowance ownership stay in infra. No runtime or allocation dependency.
use std::{
    alloc::Layout,
    fmt,
    sync::{Arc, Mutex},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidLimit,
    Overflow,
    Exhausted,
    Allocation,
    Poisoned,
    Corrupt,
    InvalidReduction,
    WrongBudget,
    UnexpectedCapacity,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLimit => "invalid allocation budget limit",
            Self::Overflow => "allocation size overflow",
            Self::Exhausted => "allocation budget exceeded",
            Self::Allocation => "allocation failed",
            Self::Poisoned => "allocation accountant poisoned",
            Self::Corrupt => "allocation accountant failed closed",
            Self::InvalidReduction => "invalid allocation charge reduction",
            Self::WrongBudget => "allocation charge belongs to another budget",
            Self::UnexpectedCapacity => "allocator returned uncovered capacity",
        })
    }
}
impl std::error::Error for Error {}
#[derive(Clone)]
pub struct Budget(Arc<Mutex<State>>);
struct State {
    limit: usize,
    used: usize,
    peak: usize,
    failed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub limit: usize,
    pub used: usize,
    pub peak: usize,
    pub failed: bool,
}
/// Non-cloneable reservation. Arc<Charge> may share ownership of the SAME
/// allocation/child allowance; it must not cover independent copied buffers.
pub struct Charge {
    budget: Budget,
    bytes: usize,
}
impl Budget {
    pub fn new(limit: usize) -> Result<Self, Error> {
        if limit == 0 || limit > isize::MAX as usize {
            return Err(Error::InvalidLimit);
        }
        Ok(Self(Arc::new(Mutex::new(State {
            limit,
            used: 0,
            peak: 0,
            failed: false,
        }))))
    }
    pub fn reserve(&self, bytes: usize) -> Result<Charge, Error> {
        let mut state = self.0.lock().map_err(|_| Error::Poisoned)?;
        if state.failed {
            return Err(Error::Corrupt);
        }
        let next = state.used.checked_add(bytes).ok_or(Error::Overflow)?;
        if next > state.limit {
            return Err(Error::Exhausted);
        }
        state.used = next;
        state.peak = state.peak.max(next);
        Ok(Charge {
            budget: self.clone(),
            bytes,
        })
    }
    pub fn snapshot(&self) -> Result<Snapshot, Error> {
        let state = self.0.lock().map_err(|_| Error::Poisoned)?;
        Ok(Snapshot {
            limit: state.limit,
            used: state.used,
            peak: state.peak,
            failed: state.failed,
        })
    }
    pub fn fail_closed(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.failed = true;
        }
    }
    fn release(&self, bytes: usize) -> Result<(), Error> {
        let mut state = self.0.lock().map_err(|_| Error::Poisoned)?;
        if let Some(next) = state.used.checked_sub(bytes) {
            state.used = next;
            Ok(())
        } else {
            state.failed = true;
            Err(Error::Corrupt)
        }
    }
}
impl Charge {
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    /// Only reduce after the covered allocation has actually been released.
    pub fn reduce_to(&mut self, bytes: usize) -> Result<(), Error> {
        let released = self
            .bytes
            .checked_sub(bytes)
            .ok_or(Error::InvalidReduction)?;
        self.budget.release(released)?;
        self.bytes = bytes;
        Ok(())
    }
    /// Transfer reservation ownership; does not reserve additional bytes.
    pub fn split(&mut self, bytes: usize) -> Result<Self, Error> {
        let left = self
            .bytes
            .checked_sub(bytes)
            .ok_or(Error::InvalidReduction)?;
        self.bytes = left;
        Ok(Self {
            budget: self.budget.clone(),
            bytes,
        })
    }
    /// Both charges remain intact on error. This never changes total usage.
    pub fn merge_from(&mut self, other: &mut Self) -> Result<(), Error> {
        if !Arc::ptr_eq(&self.budget.0, &other.budget.0) {
            return Err(Error::WrongBudget);
        }
        let bytes = self.bytes.checked_add(other.bytes).ok_or(Error::Overflow)?;
        self.bytes = bytes;
        other.bytes = 0;
        Ok(())
    }
}
impl Drop for Charge {
    fn drop(&mut self) {
        let _ = self.budget.release(self.bytes);
    }
}
fn bytes_for<T>(capacity: usize) -> Result<usize, Error> {
    Layout::array::<T>(capacity)
        .map(|layout| layout.size())
        .map_err(|_| Error::Overflow)
}
/// Accounts backing capacity only, not nested heap ownership inside T. Consumers
/// must use charged element types or account nested objects separately. Storage
/// is declared before its charge so elements/storage drop first. No inner Vec or
/// mutable charge is exposed. This does not claim allocator metadata/RSS bounds.
pub struct OwnedVec<T> {
    data: Vec<T>,
    charge: Charge,
}
impl<T> OwnedVec<T> {
    pub fn new(budget: &Budget) -> Result<Self, Error> {
        Ok(Self {
            data: Vec::new(),
            charge: budget.reserve(0)?,
        })
    }
    pub fn with_capacity(budget: &Budget, capacity: usize) -> Result<Self, Error> {
        let mut out = Self::new(budget)?;
        out.reserve_capacity(capacity)?;
        Ok(out)
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }
    pub fn charged_bytes(&self) -> usize {
        self.charge.bytes()
    }
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), Error> {
        let wanted = self.len().checked_add(additional).ok_or(Error::Overflow)?;
        self.reserve_capacity(wanted)
    }
    pub fn try_push(&mut self, value: T) -> Result<(), Error> {
        if self.len() == self.capacity() {
            let wanted = self
                .capacity()
                .checked_mul(2)
                .and_then(|n| n.checked_add(4))
                .ok_or(Error::Overflow)?;
            self.reserve_capacity(wanted)?;
        }
        self.data.push(value);
        Ok(())
    }
    pub fn pop(&mut self) -> Option<T> {
        self.data.pop()
    }
    pub fn truncate(&mut self, len: usize) {
        self.data.truncate(len);
    }
    fn reserve_capacity(&mut self, wanted: usize) -> Result<(), Error> {
        self.reserve_capacity_with(wanted, |data, capacity| {
            data.try_reserve_exact(capacity)
                .map_err(|_| Error::Allocation)
        })
    }
    fn reserve_capacity_with(
        &mut self,
        wanted: usize,
        allocate: impl FnOnce(&mut Vec<T>, usize) -> Result<(), Error>,
    ) -> Result<(), Error> {
        if wanted <= self.capacity() {
            return Ok(());
        }
        let requested = bytes_for::<T>(wanted)?;
        // Fresh storage makes rollback explicit: the original data and charge
        // remain intact on failed allocation. The full old+new peak is held.
        let mut replacement_charge = self.charge.budget.reserve(requested)?;
        let mut replacement = Vec::new();
        allocate(&mut replacement, wanted)?;
        let actual = bytes_for::<T>(replacement.capacity())?;
        if actual > replacement_charge.bytes() {
            // Never accept uncovered storage or release its reservation before
            // dropping it. An unexpected allocator contract poisons admission.
            self.charge.budget.fail_closed();
            drop(replacement);
            drop(replacement_charge);
            return Err(Error::UnexpectedCapacity);
        }
        replacement_charge.reduce_to(actual)?;
        replacement.append(&mut self.data);
        let old = std::mem::replace(&mut self.data, replacement);
        drop(old);
        let old_charge = std::mem::replace(&mut self.charge, replacement_charge);
        drop(old_charge);
        Ok(())
    }
}
impl<T: Copy> OwnedVec<T> {
    pub fn try_extend_from_slice(&mut self, data: &[T]) -> Result<(), Error> {
        self.try_reserve_exact(data.len())?;
        self.data.extend_from_slice(data);
        Ok(())
    }
}
impl<T> AsRef<[T]> for OwnedVec<T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}
/// UTF8 ownership over a charged byte buffer; bytes cannot be mutated through
/// this wrapper. Validation/error semantics of a codec remain with its owner.
pub struct OwnedString {
    bytes: OwnedVec<u8>,
}
impl OwnedString {
    pub fn new(budget: &Budget) -> Result<Self, Error> {
        Ok(Self {
            bytes: OwnedVec::new(budget)?,
        })
    }
    pub fn try_from_str(budget: &Budget, text: &str) -> Result<Self, Error> {
        let mut out = Self::new(budget)?;
        out.try_push_str(text)?;
        Ok(out)
    }
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(self.bytes.as_slice()).expect("UTF8-only insertion invariant")
    }
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
    pub fn capacity(&self) -> usize {
        self.bytes.capacity()
    }
    pub fn charged_bytes(&self) -> usize {
        self.bytes.charged_bytes()
    }
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), Error> {
        self.bytes.try_reserve_exact(additional)
    }
    pub fn try_push_str(&mut self, text: &str) -> Result<(), Error> {
        self.bytes.try_extend_from_slice(text.as_bytes())
    }
    pub fn try_push(&mut self, ch: char) -> Result<(), Error> {
        let mut buf = [0; 4];
        self.try_push_str(ch.encode_utf8(&mut buf))
    }
}
impl AsRef<str> for OwnedString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reservations_transfer_without_losing_failed_merges() {
        assert!(matches!(Budget::new(0), Err(Error::InvalidLimit)));
        let budget = Budget::new(64).unwrap();
        let mut first = budget.reserve(40).unwrap();
        let mut second = first.split(12).unwrap();
        assert_eq!((first.bytes(), second.bytes()), (28, 12));
        first.merge_from(&mut second).unwrap();
        assert_eq!((first.bytes(), second.bytes()), (40, 0));
        let other_budget = Budget::new(64).unwrap();
        let mut other = other_budget.reserve(8).unwrap();
        assert_eq!(first.merge_from(&mut other), Err(Error::WrongBudget));
        assert_eq!((first.bytes(), other.bytes()), (40, 8));
        assert_eq!(first.reduce_to(41), Err(Error::InvalidReduction));
        first.reduce_to(24).unwrap();
        assert_eq!(budget.snapshot().unwrap().used, 24);
        assert!(matches!(budget.reserve(41), Err(Error::Exhausted)));
        drop(first);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }

    #[test]
    fn replacement_holds_overlap_and_failure_preserves_original() {
        let budget = Budget::new(64).unwrap();
        let mut data = OwnedVec::<u32>::with_capacity(&budget, 4).unwrap();
        data.try_extend_from_slice(&[7, 8]).unwrap();
        let before = budget.snapshot().unwrap().used;
        assert_eq!(
            data.reserve_capacity_with(8, |_, _| {
                assert_eq!(budget.snapshot().unwrap().used, before + 32);
                Err(Error::Allocation)
            }),
            Err(Error::Allocation)
        );
        assert_eq!(data.as_slice(), &[7, 8]);
        assert_eq!(budget.snapshot().unwrap().used, before);
        data.reserve_capacity(8).unwrap();
        assert_eq!(data.as_slice(), &[7, 8]);
        assert_eq!(data.charged_bytes(), data.capacity() * 4);
        assert_eq!(budget.snapshot().unwrap().peak, before + 32);
        drop(data);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }

    #[test]
    fn layout_alignment_zero_size_and_overflow() {
        #[repr(align(64))]
        #[derive(Clone, Copy)]
        struct Aligned;
        #[repr(align(64))]
        struct Nonzero([u8; 64]);
        let budget = Budget::new(1024).unwrap();
        let mut zero = OwnedVec::<Aligned>::with_capacity(&budget, 10).unwrap();
        zero.try_push(Aligned).unwrap();
        assert_eq!(zero.charged_bytes(), 0);
        let mut aligned = OwnedVec::<Nonzero>::with_capacity(&budget, 2).unwrap();
        aligned.try_push(Nonzero([1; 64])).unwrap();
        assert_eq!(aligned.as_slice()[0].0[0], 1);
        assert_eq!(aligned.as_slice().as_ptr() as usize % 64, 0);
        assert_eq!(aligned.charged_bytes(), aligned.capacity() * 64);
        assert_eq!(bytes_for::<u64>(usize::MAX), Err(Error::Overflow));
    }

    #[test]
    fn unexpected_capacity_closes_admission_and_rolls_back() {
        let budget = Budget::new(1024).unwrap();
        let mut data = OwnedVec::<u8>::new(&budget).unwrap();
        assert_eq!(
            data.reserve_capacity_with(4, |v, _| {
                v.try_reserve_exact(8).map_err(|_| Error::Allocation)
            }),
            Err(Error::UnexpectedCapacity)
        );
        assert!(budget.snapshot().unwrap().failed);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert!(matches!(budget.reserve(1), Err(Error::Corrupt)));
    }

    #[test]
    fn poison_refuses_further_reservations() {
        let budget = Budget::new(64).unwrap();
        let cloned = budget.clone();
        assert!(
            std::thread::spawn(move || {
                let _lock = cloned.0.lock().unwrap();
                panic!("injected accountant panic");
            })
            .join()
            .is_err()
        );
        assert!(matches!(budget.reserve(1), Err(Error::Poisoned)));
    }

    #[test]
    fn elements_drop_before_backing_charge() {
        struct Observe(Budget);
        impl Drop for Observe {
            fn drop(&mut self) {
                assert!(self.0.snapshot().unwrap().used >= std::mem::size_of::<Self>());
            }
        }
        let budget = Budget::new(1024).unwrap();
        let mut data = OwnedVec::with_capacity(&budget, 1).unwrap();
        data.try_push(Observe(budget.clone())).unwrap();
        drop(data);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }

    #[test]
    fn utf8_storage_owns_capacity_until_drop() {
        let budget = Budget::new(128).unwrap();
        let mut text = OwnedString::try_from_str(&budget, "é").unwrap();
        text.try_push('界').unwrap();
        assert_eq!(text.as_str(), "é界");
        assert_eq!(text.charged_bytes(), text.capacity());
        assert_eq!(budget.snapshot().unwrap().used, text.capacity());
        drop(text);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
