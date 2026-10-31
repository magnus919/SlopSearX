//! Parser-private requested backing accounting; surrounding input/output ownership is separate.
use ssx_owned_budget::{Budget, Charge, Error};
use std::alloc::Layout;

pub(super) struct Accounting {
    budget: Option<Budget>,
    retained: Option<Charge>,
}
impl Accounting {
    pub(super) fn new(budget: Option<&Budget>) -> Result<Self, Error> {
        Ok(Self {
            budget: budget.cloned(),
            retained: budget.map(|b| b.reserve(0)).transpose()?,
        })
    }
    pub(super) fn budget(&self) -> Option<&Budget> {
        self.budget.as_ref()
    }
    pub(super) fn charge(self) -> Option<Charge> {
        self.retained
    }
    pub(super) fn push<T>(&mut self, data: &mut Vec<T>, value: T) -> Result<(), Error> {
        if self.budget.is_some() && data.len() == data.capacity() {
            let wanted = data
                .capacity()
                .checked_mul(2)
                .and_then(|n| n.checked_add(4))
                .ok_or(Error::Overflow)?;
            self.grow(data, wanted)?;
        }
        data.push(value);
        Ok(())
    }
    pub(super) fn grow<T>(&mut self, data: &mut Vec<T>, wanted: usize) -> Result<(), Error> {
        if wanted <= data.capacity() {
            return Ok(());
        }
        let Some(budget) = &self.budget else {
            data.reserve(wanted - data.len());
            return Ok(());
        };
        let layout = Layout::array::<T>(wanted).map_err(|_| Error::Overflow)?;
        let old_bytes = Layout::array::<T>(data.capacity())
            .map_err(|_| Error::Overflow)?
            .size();
        let mut charge = budget.reserve(layout.size())?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(wanted)
            .map_err(|_| Error::Allocation)?;
        let actual = Layout::array::<T>(replacement.capacity())
            .map_err(|_| Error::Overflow)?
            .size();
        if actual > charge.bytes() {
            budget.fail_closed();
            drop(replacement);
            return Err(Error::UnexpectedCapacity);
        }
        charge.reduce_to(actual)?;
        replacement.append(data);
        let old = std::mem::replace(data, replacement);
        drop(old);
        let retained = self.retained.as_mut().expect("budget/root charge paired");
        retained.reduce_to(
            retained
                .bytes()
                .checked_sub(old_bytes)
                .ok_or(Error::Corrupt)?,
        )?;
        retained.merge_from(&mut charge)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_tracks_replacement_and_retains_nested_buffers() {
        let budget = Budget::new(1024).unwrap();
        let mut accounting = Accounting::new(Some(&budget)).unwrap();
        // Storage declared after accounting must be freed before its root charge.
        let mut data = Vec::new();
        for value in 0..9u64 {
            accounting.push(&mut data, value).unwrap();
        }
        let capacity_bytes = data.capacity() * std::mem::size_of::<u64>();
        assert_eq!(budget.snapshot().unwrap().used, capacity_bytes);
        assert!(budget.snapshot().unwrap().peak > capacity_bytes);
        drop(data);
        drop(accounting);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[test]
    fn failed_growth_preserves_old_storage_and_charge() {
        let budget = Budget::new(40).unwrap();
        let mut accounting = Accounting::new(Some(&budget)).unwrap();
        let mut data = Vec::new();
        for value in 0..4u64 {
            accounting.push(&mut data, value).unwrap();
        }
        assert_eq!(accounting.push(&mut data, 5), Err(Error::Exhausted));
        assert_eq!(data, [0, 1, 2, 3]);
        assert_eq!(budget.snapshot().unwrap().used, 32);
        drop(data);
        drop(accounting);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
