//! Cooperative translation of CPython3.14.7 list sort operations for OpenAlex.
//! Source: Objects/listobject.c SHA256
//! 9bea0c5d2101a2619f34cab86b4895a1236e4cc19f55ce55694b5e7909bf6127.
//! PSF license: crates/ssx-source-codecs/LICENSE-PYTHON. No Python runtime.
use super::scholarly_order::{Error, Pair, pair_less_budgeted};
use std::time::Instant;
#[cfg(test)]
static TRACE: [std::sync::atomic::AtomicUsize; 6] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; 6];
macro_rules! trace {
    ($n:expr) => {
        #[cfg(test)]
        {
            TRACE[$n].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    };
}
// Backing storage remains charge-owned throughout sorting and result use.
// Caller pair storage is separate; comparator futures use this same budget.
enum Buffer<T> {
    Plain(Vec<T>),
    Charged(ssx_owned_budget::OwnedVec<T>),
}
impl<T> Buffer<T> {
    fn reserve(budget: Option<&ssx_owned_budget::Budget>, capacity: usize) -> Result<Self, Error> {
        if let Some(budget) = budget {
            return ssx_owned_budget::OwnedVec::with_capacity(budget, capacity)
                .map(Self::Charged)
                .map_err(|_| Error::Resource);
        }
        let mut v = Vec::new();
        v.try_reserve_exact(capacity).map_err(|_| Error::Resource)?;
        Ok(Self::Plain(v))
    }
    fn capacity(&self) -> usize {
        match self {
            Self::Plain(v) => v.capacity(),
            Self::Charged(v) => v.capacity(),
        }
    }
    fn push(&mut self, value: T) -> Result<(), Error> {
        // All sorter buffers are reserved once; reject unexpected growth.
        if self.len() == self.capacity() {
            return Err(Error::Resource);
        }
        match self {
            Self::Plain(v) => {
                v.push(value);
                Ok(())
            }
            Self::Charged(v) => v.try_push(value).map_err(|_| Error::Resource),
        }
    }
}
impl<T: Copy> Buffer<T> {
    fn remove(&mut self, index: usize) {
        let len = self.len();
        self.copy_within(index + 1..len, index);
        match self {
            Self::Plain(v) => v.truncate(len - 1),
            Self::Charged(v) => v.truncate(len - 1),
        }
    }
}
impl<T> std::ops::Deref for Buffer<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        match self {
            Self::Plain(v) => v,
            Self::Charged(v) => v.as_slice(),
        }
    }
}
impl<T> std::ops::DerefMut for Buffer<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        match self {
            Self::Plain(v) => v,
            Self::Charged(v) => v.as_mut_slice(),
        }
    }
}
/// Budget-owned permutation. Only borrowed access is exposed; dropping it frees
/// backing storage before releasing its ledger charge.
pub struct Permutation(Buffer<usize>);
impl Permutation {
    pub fn as_slice(&self) -> &[usize] {
        &self.0
    }
}
const MIN_GALLOP: usize = 7;
#[derive(Clone, Copy)]
struct Run {
    base: usize,
    len: usize,
    power: usize,
}
struct Sorter<'p, 'a> {
    pairs: &'p [Pair<'a>],
    items: Buffer<usize>,
    pending: Buffer<Run>,
    min_gallop: usize,
    deadline: Instant,
    budget: Option<ssx_owned_budget::Budget>,
}
async fn tick(deadline: Instant) -> Result<(), Error> {
    tokio::task::yield_now().await;
    if Instant::now() >= deadline {
        Err(Error::Deadline)
    } else {
        Ok(())
    }
}
impl Sorter<'_, '_> {
    async fn lt(&self, a: usize, b: usize) -> Result<bool, Error> {
        pair_less_budgeted(
            self.pairs[a],
            self.pairs[b],
            self.deadline,
            self.budget.as_ref(),
        )
        .await
    }
    async fn reverse(&mut self, base: usize, n: usize) -> Result<(), Error> {
        for i in 0..n / 2 {
            if i % 1024 == 0 {
                tick(self.deadline).await?;
            }
            self.items.swap(base + i, base + n - 1 - i);
        }
        Ok(())
    }
    async fn count_run(&mut self, base: usize, remaining: usize) -> Result<usize, Error> {
        let mut n = 1;
        while n < remaining {
            if self
                .lt(self.items[base + n], self.items[base + n - 1])
                .await?
            {
                break;
            }
            n += 1;
        }
        if n == remaining {
            return Ok(n);
        }
        if n > 1 {
            if self.lt(self.items[base], self.items[base + n - 1]).await? {
                return Ok(n);
            }
            self.reverse(base, n).await?;
        }
        n += 1;
        let mut neq = 0;
        while n < remaining {
            if self
                .lt(self.items[base + n], self.items[base + n - 1])
                .await?
            {
                if neq != 0 {
                    neq += 1;
                    self.reverse(base + n - neq, neq).await?;
                    neq = 0;
                }
            } else if self
                .lt(self.items[base + n - 1], self.items[base + n])
                .await?
            {
                break;
            } else {
                neq += 1;
            }
            n += 1;
        }
        if neq != 0 {
            neq += 1;
            self.reverse(base + n - neq, neq).await?;
        }
        self.reverse(base, n).await?;
        while n < remaining {
            if self
                .lt(self.items[base + n], self.items[base + n - 1])
                .await?
            {
                break;
            }
            n += 1;
        }
        Ok(n)
    }
    async fn binarysort(&mut self, base: usize, n: usize, sorted: usize) -> Result<(), Error> {
        for at in sorted.max(1)..n {
            let pivot = self.items[base + at];
            let (mut left, mut right) = (0, at);
            while left < right {
                let mid = left + (right - left) / 2;
                if self.lt(pivot, self.items[base + mid]).await? {
                    right = mid;
                } else {
                    left = mid + 1;
                }
            }
            for i in (left + 1..=at).rev() {
                self.items[base + i] = self.items[base + i - 1];
            }
            self.items[base + left] = pivot;
        }
        Ok(())
    }
    async fn gallop_left(&self, key: usize, a: &[usize], hint: usize) -> Result<usize, Error> {
        let (mut last, mut ofs) = (0isize, 1isize);
        let hint = hint as isize;
        if self.lt(a[hint as usize], key).await? {
            let max = a.len() as isize - hint;
            while ofs < max {
                if self.lt(a[(hint + ofs) as usize], key).await? {
                    last = ofs;
                    ofs = ofs
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(1))
                        .ok_or(Error::Resource)?;
                } else {
                    break;
                }
            }
            ofs = ofs.min(max);
            last += hint;
            ofs += hint;
        } else {
            let max = hint + 1;
            while ofs < max {
                if self.lt(a[(hint - ofs) as usize], key).await? {
                    break;
                }
                last = ofs;
                ofs = ofs
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Error::Resource)?;
            }
            ofs = ofs.min(max);
            let k = last;
            last = hint - ofs;
            ofs = hint - k;
        }
        last += 1;
        while last < ofs {
            let mid = last + (ofs - last) / 2;
            if self.lt(a[mid as usize], key).await? {
                last = mid + 1;
            } else {
                ofs = mid;
            }
        }
        Ok(ofs as usize)
    }
    async fn gallop_right(&self, key: usize, a: &[usize], hint: usize) -> Result<usize, Error> {
        let (mut last, mut ofs) = (0isize, 1isize);
        let hint = hint as isize;
        if self.lt(key, a[hint as usize]).await? {
            let max = hint + 1;
            while ofs < max {
                if self.lt(key, a[(hint - ofs) as usize]).await? {
                    last = ofs;
                    ofs = ofs
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(1))
                        .ok_or(Error::Resource)?;
                } else {
                    break;
                }
            }
            ofs = ofs.min(max);
            let k = last;
            last = hint - ofs;
            ofs = hint - k;
        } else {
            let max = a.len() as isize - hint;
            while ofs < max {
                if self.lt(key, a[(hint + ofs) as usize]).await? {
                    break;
                }
                last = ofs;
                ofs = ofs
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Error::Resource)?;
            }
            ofs = ofs.min(max);
            last += hint;
            ofs += hint;
        }
        last += 1;
        while last < ofs {
            let mid = last + (ofs - last) / 2;
            if self.lt(key, a[mid as usize]).await? {
                ofs = mid;
            } else {
                last = mid + 1;
            }
        }
        Ok(ofs as usize)
    }
    async fn temp(&self, base: usize, n: usize) -> Result<Buffer<usize>, Error> {
        let mut out = Buffer::reserve(self.budget.as_ref(), n)?;
        for chunk in self.items[base..base + n].chunks(1024) {
            tick(self.deadline).await?;
            for item in chunk {
                out.push(*item)?;
            }
        }
        Ok(out)
    }
    async fn copy_temp(
        &mut self,
        tmp: &[usize],
        source: usize,
        dest: usize,
        n: usize,
    ) -> Result<(), Error> {
        for i in 0..n {
            if i % 1024 == 0 {
                tick(self.deadline).await?;
            }
            self.items[dest + i] = tmp[source + i];
        }
        Ok(())
    }
    async fn move_items(&mut self, source: usize, dest: usize, n: usize) -> Result<(), Error> {
        if dest < source {
            for i in 0..n {
                if i % 1024 == 0 {
                    tick(self.deadline).await?;
                }
                self.items[dest + i] = self.items[source + i];
            }
        } else {
            for i in (0..n).rev() {
                if i % 1024 == 0 {
                    tick(self.deadline).await?;
                }
                self.items[dest + i] = self.items[source + i];
            }
        }
        Ok(())
    }
    async fn lo_finish(
        &mut self,
        tmp: &[usize],
        a: usize,
        dest: usize,
        na: usize,
    ) -> Result<(), Error> {
        self.copy_temp(tmp, a, dest, na).await
    }
    async fn lo_copy_b(
        &mut self,
        tmp: &[usize],
        a: usize,
        b: usize,
        dest: usize,
        nb: usize,
    ) -> Result<(), Error> {
        self.move_items(b, dest, nb).await?;
        self.items[dest + nb] = tmp[a];
        Ok(())
    }
    async fn merge_lo(
        &mut self,
        base: usize,
        mut na: usize,
        bbase: usize,
        mut nb: usize,
    ) -> Result<(), Error> {
        trace!(0);
        let tmp = self.temp(base, na).await?;
        let (mut a, mut b, mut dest) = (0, bbase, base);
        self.items[dest] = self.items[b];
        dest += 1;
        b += 1;
        nb -= 1;
        if nb == 0 {
            trace!(3);
            return self.lo_finish(&tmp, a, dest, na).await;
        }
        if na == 1 {
            return self.lo_copy_b(&tmp, a, b, dest, nb).await;
        }
        let mut min = self.min_gallop;
        loop {
            let (mut acount, mut bcount) = (0, 0);
            loop {
                if self.lt(self.items[b], tmp[a]).await? {
                    self.items[dest] = self.items[b];
                    dest += 1;
                    b += 1;
                    bcount += 1;
                    acount = 0;
                    nb -= 1;
                    if nb == 0 {
                        trace!(3);
                        return self.lo_finish(&tmp, a, dest, na).await;
                    }
                    if bcount >= min {
                        break;
                    }
                } else {
                    self.items[dest] = tmp[a];
                    dest += 1;
                    a += 1;
                    acount += 1;
                    bcount = 0;
                    na -= 1;
                    if na == 1 {
                        return self.lo_copy_b(&tmp, a, b, dest, nb).await;
                    }
                    if acount >= min {
                        break;
                    }
                }
            }
            min += 1;
            loop {
                min -= usize::from(min > 1);
                self.min_gallop = min;
                acount = self.gallop_right(self.items[b], &tmp[a..a + na], 0).await?;
                if acount != 0 {
                    self.copy_temp(&tmp, a, dest, acount).await?;
                    dest += acount;
                    a += acount;
                    na -= acount;
                    if na == 1 {
                        return self.lo_copy_b(&tmp, a, b, dest, nb).await;
                    }
                    if na == 0 {
                        trace!(2);
                        return Ok(());
                    }
                }
                self.items[dest] = self.items[b];
                dest += 1;
                b += 1;
                nb -= 1;
                if nb == 0 {
                    trace!(3);
                    return self.lo_finish(&tmp, a, dest, na).await;
                }
                bcount = self.gallop_left(tmp[a], &self.items[b..b + nb], 0).await?;
                if bcount != 0 {
                    self.move_items(b, dest, bcount).await?;
                    dest += bcount;
                    b += bcount;
                    nb -= bcount;
                    if nb == 0 {
                        trace!(3);
                        return self.lo_finish(&tmp, a, dest, na).await;
                    }
                }
                self.items[dest] = tmp[a];
                dest += 1;
                a += 1;
                na -= 1;
                if na == 1 {
                    return self.lo_copy_b(&tmp, a, b, dest, nb).await;
                }
                if acount < MIN_GALLOP && bcount < MIN_GALLOP {
                    break;
                }
            }
            min += 1;
            self.min_gallop = min;
        }
    }
    async fn hi_finish(&mut self, tmp: &[usize], dest: usize, nb: usize) -> Result<(), Error> {
        self.copy_temp(tmp, 0, dest - nb, nb).await
    }
    async fn hi_copy_a(
        &mut self,
        tmp: &[usize],
        base: usize,
        dest: usize,
        na: usize,
    ) -> Result<(), Error> {
        self.move_items(base, dest - na, na).await?;
        self.items[dest - na - 1] = tmp[0];
        Ok(())
    }
    async fn merge_hi(
        &mut self,
        base: usize,
        mut na: usize,
        bbase: usize,
        mut nb: usize,
    ) -> Result<(), Error> {
        trace!(1);
        let tmp = self.temp(bbase, nb).await?;
        let mut dest = bbase + nb;
        dest -= 1;
        self.items[dest] = self.items[base + na - 1];
        na -= 1;
        if na == 0 {
            trace!(4);
            return self.hi_finish(&tmp, dest, nb).await;
        }
        if nb == 1 {
            return self.hi_copy_a(&tmp, base, dest, na).await;
        }
        let mut min = self.min_gallop;
        loop {
            let (mut acount, mut bcount) = (0, 0);
            loop {
                if self.lt(tmp[nb - 1], self.items[base + na - 1]).await? {
                    dest -= 1;
                    self.items[dest] = self.items[base + na - 1];
                    acount += 1;
                    bcount = 0;
                    na -= 1;
                    if na == 0 {
                        trace!(4);
                        return self.hi_finish(&tmp, dest, nb).await;
                    }
                    if acount >= min {
                        break;
                    }
                } else {
                    dest -= 1;
                    self.items[dest] = tmp[nb - 1];
                    bcount += 1;
                    acount = 0;
                    nb -= 1;
                    if nb == 1 {
                        return self.hi_copy_a(&tmp, base, dest, na).await;
                    }
                    if bcount >= min {
                        break;
                    }
                }
            }
            min += 1;
            loop {
                min -= usize::from(min > 1);
                self.min_gallop = min;
                let k = self
                    .gallop_right(tmp[nb - 1], &self.items[base..base + na], na - 1)
                    .await?;
                acount = na - k;
                if acount != 0 {
                    self.move_items(base + na - acount, dest - acount, acount)
                        .await?;
                    dest -= acount;
                    na -= acount;
                    if na == 0 {
                        trace!(4);
                        return self.hi_finish(&tmp, dest, nb).await;
                    }
                }
                dest -= 1;
                self.items[dest] = tmp[nb - 1];
                nb -= 1;
                if nb == 1 {
                    return self.hi_copy_a(&tmp, base, dest, na).await;
                }
                let k = self
                    .gallop_left(self.items[base + na - 1], &tmp[..nb], nb - 1)
                    .await?;
                bcount = nb - k;
                if bcount != 0 {
                    self.copy_temp(&tmp, nb - bcount, dest - bcount, bcount)
                        .await?;
                    dest -= bcount;
                    nb -= bcount;
                    if nb == 1 {
                        return self.hi_copy_a(&tmp, base, dest, na).await;
                    }
                    if nb == 0 {
                        trace!(5);
                        return Ok(());
                    }
                }
                dest -= 1;
                self.items[dest] = self.items[base + na - 1];
                na -= 1;
                if na == 0 {
                    trace!(4);
                    return self.hi_finish(&tmp, dest, nb).await;
                }
                if acount < MIN_GALLOP && bcount < MIN_GALLOP {
                    break;
                }
            }
            min += 1;
            self.min_gallop = min;
        }
    }
    async fn merge_at(&mut self, i: usize) -> Result<(), Error> {
        let a = self.pending[i];
        let b = self.pending[i + 1];
        self.pending[i].len = a.len.checked_add(b.len).ok_or(Error::Resource)?;
        self.pending.remove(i + 1);
        let k = self
            .gallop_right(self.items[b.base], &self.items[a.base..a.base + a.len], 0)
            .await?;
        let base = a.base + k;
        let na = a.len - k;
        if na == 0 {
            return Ok(());
        }
        let nb = self
            .gallop_left(
                self.items[base + na - 1],
                &self.items[b.base..b.base + b.len],
                b.len - 1,
            )
            .await?;
        if nb == 0 {
            return Ok(());
        }
        if na <= nb {
            self.merge_lo(base, na, b.base, nb).await
        } else {
            self.merge_hi(base, na, b.base, nb).await
        }
    }
    async fn found_new_run(&mut self, n2: usize) -> Result<(), Error> {
        if let Some(last) = self.pending.last().copied() {
            let power = powerloop(last.base, last.len, n2, self.items.len())?;
            while self.pending.len() > 1 && self.pending[self.pending.len() - 2].power > power {
                self.merge_at(self.pending.len() - 2).await?;
            }
            self.pending.last_mut().ok_or(Error::Resource)?.power = power;
        }
        Ok(())
    }
}
fn powerloop(s1: usize, n1: usize, n2: usize, n: usize) -> Result<usize, Error> {
    let mut a = s1
        .checked_mul(2)
        .and_then(|s| s.checked_add(n1))
        .ok_or(Error::Resource)?;
    let mut b = a
        .checked_add(n1)
        .and_then(|s| s.checked_add(n2))
        .ok_or(Error::Resource)?;
    let mut power = 0;
    loop {
        power += 1;
        if a >= n {
            a -= n;
            b -= n;
        } else if b >= n {
            break;
        }
        a = a.checked_mul(2).ok_or(Error::Resource)?;
        b = b.checked_mul(2).ok_or(Error::Resource)?;
    }
    Ok(power)
}
fn minrun(mut n: usize) -> usize {
    let mut r = 0;
    while n >= 64 {
        r |= n & 1;
        n >>= 1;
    }
    n + r
}
/// Source operation API requires pairs originating from bounded SourceJSON.
/// It returns a permutation; it never asks Rust's total-order slice sorter to
/// interpret NaNs. This compatibility wrapper retains its unbudgeted Vec API.
pub async fn order(pairs: &[Pair<'_>], deadline: Instant) -> Result<Vec<usize>, Error> {
    match order_inner(pairs, deadline, None)
        .await
        .map_err(super::scholarly_order::legacy_error)?
    {
        Buffer::Plain(v) => Ok(v),
        Buffer::Charged(_) => unreachable!("unbudgeted sorter selects plain storage"),
    }
}
/// Charge sorter index, run-stack and merge scratch backing storage. Caller
/// pair storage remains separate; recursive comparator boxes share this budget.
pub async fn order_budgeted(
    pairs: &[Pair<'_>],
    deadline: Instant,
    budget: &ssx_owned_budget::Budget,
) -> Result<Permutation, Error> {
    order_inner(pairs, deadline, Some(budget))
        .await
        .map(Permutation)
        .map_err(super::scholarly_order::legacy_error)
}
async fn order_inner(
    pairs: &[Pair<'_>],
    deadline: Instant,
    budget: Option<&ssx_owned_budget::Budget>,
) -> Result<Buffer<usize>, Error> {
    if pairs.len() > super::source_json::MAX_TEXT {
        return Err(Error::Resource);
    }
    let mut items = Buffer::reserve(budget, pairs.len())?;
    for at in 0..pairs.len() {
        if at % 1024 == 0 {
            tick(deadline).await?;
        }
        items.push(at)?;
    }
    let pending = Buffer::reserve(budget, usize::BITS as usize + 1)?;
    let mut sorter = Sorter {
        pairs,
        items,
        pending,
        min_gallop: MIN_GALLOP,
        deadline,
        budget: budget.cloned(),
    };
    let minimum = minrun(pairs.len());
    let mut base = 0;
    while base < pairs.len() {
        let remaining = pairs.len() - base;
        let mut n = sorter.count_run(base, remaining).await?;
        if n < minimum {
            let force = minimum.min(remaining);
            sorter.binarysort(base, force, n).await?;
            n = force;
        }
        sorter.found_new_run(n).await?;
        // Powers strictly increase down the stack; at most BITS+1 powers
        // exist for bounded indices. Guard capacity as well so a regression
        // cannot cause unaccounted implicit Vec growth.
        if sorter.pending.len() == sorter.pending.capacity() {
            return Err(Error::Resource);
        }
        sorter.pending.push(Run {
            base,
            len: n,
            power: 0,
        })?;
        base += n;
    }
    while sorter.pending.len() > 1 {
        let mut i = sorter.pending.len() - 2;
        if i > 0 && sorter.pending[i - 1].len < sorter.pending[i + 1].len {
            i -= 1;
        }
        sorter.merge_at(i).await?;
    }
    tick(deadline).await?;
    Ok(sorter.items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{
        scholarly_order::Ref,
        source_json::{self, Value},
    };
    use base64::Engine as _;
    use std::time::Duration;
    #[tokio::test]
    async fn full_source_order_and_errors_not_only_adapter_prefix() {
        let fixture: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-openalex-order.json"
        ))
        .unwrap();
        let complete: ssx_json::Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-openalex-full-order.json"
        ))
        .unwrap();
        let mut checked = Vec::new();
        for (row, full) in fixture["records"]
            .as_array()
            .unwrap()
            .iter()
            .zip(complete["records"].as_array().unwrap())
        {
            assert_eq!(row["id"], full["id"]);
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["responses"][0]["body_b64"].as_str().unwrap())
                .unwrap();
            let value = source_json::from_slice(&body, None).await.unwrap();
            let work = &value.get("results").unwrap().as_array().unwrap()[0];
            let inverted = work
                .get("abstract_inverted_index")
                .unwrap()
                .as_object()
                .unwrap();
            let mut pairs = Vec::new();
            for (word, positions) in inverted.iter() {
                match positions {
                    Value::Array(a) => {
                        for p in a {
                            pairs.push(Pair {
                                position: Ref::Value(p),
                                word,
                            });
                        }
                    }
                    Value::String(s) => {
                        for p in s.codepoints() {
                            pairs.push(Pair {
                                position: Ref::Character(*p),
                                word,
                            });
                        }
                    }
                    Value::Object(m) => {
                        for (p, _) in m.iter() {
                            pairs.push(Pair {
                                position: Ref::Text(p),
                                word,
                            });
                        }
                    }
                    _ => panic!("operation fixture must exercise supported source iterables"),
                }
            }
            let result = order(&pairs, Instant::now() + Duration::from_secs(10)).await;
            let budget = ssx_owned_budget::Budget::new(64 * 1024 * 1024).unwrap();
            let charged =
                order_budgeted(&pairs, Instant::now() + Duration::from_secs(10), &budget).await;
            match (&result, &charged) {
                (Ok(plain), Ok(charged)) => assert_eq!(
                    plain.as_slice(),
                    charged.as_slice(),
                    "charged {}",
                    row["id"]
                ),
                (Err(plain), Err(charged)) => {
                    assert_eq!(plain, charged, "charged error {}", row["id"])
                }
                _ => panic!("charged result mismatch {}", row["id"]),
            }
            drop(charged);
            assert_eq!(
                budget.snapshot().unwrap().used,
                0,
                "sorter release {}",
                row["id"]
            );

            let owned =
                order_owned(&pairs, Instant::now() + Duration::from_secs(10), &budget).await;
            if let Some(error) = full["error"]["message"].as_str() {
                let Err(BoundedError::Source(message)) = &owned else {
                    panic!("owned source error {}", row["id"])
                };
                assert_eq!(message.as_str(), error, "owned {}", row["id"]);
                assert!(budget.snapshot().unwrap().used >= message.capacity());
            } else {
                let actual = owned.as_ref().unwrap();
                assert_eq!(
                    actual.as_slice(),
                    result.as_ref().unwrap().as_slice(),
                    "owned {}",
                    row["id"]
                );
            }
            drop(owned);
            assert_eq!(budget.snapshot().unwrap().used, 0);

            if let Some(error) = full["error"]["message"].as_str() {
                assert_eq!(result, Err(Error::Source(error.into())), "{}", row["id"]);
            } else {
                let order = result.unwrap_or_else(|e| panic!("{} {e:?}", row["id"]));
                let abstract_text = order
                    .iter()
                    .map(|i| pairs[*i].word.as_str().unwrap())
                    .collect::<Vec<_>>()
                    .join(" ");
                assert_eq!(
                    abstract_text,
                    full["abstract"].as_str().unwrap(),
                    "full order {}",
                    row["id"]
                );
                assert_eq!(
                    abstract_text.chars().take(500).collect::<String>(),
                    row["output"]["results"][0]["content"].as_str().unwrap(),
                    "adapter prefix {}",
                    row["id"]
                );
            }
            checked.push(row["id"].as_str().unwrap().to_owned());
            for corpus in [
                "docs/rust-rewrite/a06/source-openalex-order.json",
                "docs/rust-rewrite/a06/source-openalex-full-order.json",
            ] {
                crate::adapters::a06_case::completed(
                    "ssx-infra/lib/adapters::scholarly_sort::tests::full_source_order_and_errors_not_only_adapter_prefix",
                    corpus,
                    row["id"].as_str().unwrap(),
                    "default",
                    "bounded_helper",
                );
            }
        }
        let trace = TRACE
            .each_ref()
            .map(|counter| counter.load(std::sync::atomic::Ordering::Relaxed));
        eprintln!("merge trace lo/hi/lo-na0/lo-nb0/hi-na0/hi-nb0={trace:?}");
        assert!(
            trace[0] > 0 && trace[1] > 0,
            "both merge directions must be exercised"
        );
        assert!(
            trace[2] > 0 && trace[5] > 0,
            "both nontransitive exhausted-side paths must be exercised"
        );
        assert_eq!(complete["records"].as_array().unwrap().len(), 172);
        assert_eq!(
            checked,
            (0..172)
                .map(|i| format!("openalex.order.{i}"))
                .collect::<Vec<_>>()
        );
    }
    #[tokio::test]
    async fn charged_storage_refusal_retention_and_cancellation_restore_budget() {
        let left = source_json::Text::from("b");
        let right = source_json::Text::from("a");
        let pairs = [
            Pair {
                position: Ref::Text(&left),
                word: &left,
            },
            Pair {
                position: Ref::Text(&right),
                word: &right,
            },
        ];
        let small = ssx_owned_budget::Budget::new(8).unwrap();
        assert!(matches!(
            order_budgeted(&pairs, Instant::now() + Duration::from_secs(10), &small).await,
            Err(Error::Resource)
        ));
        assert_eq!(small.snapshot().unwrap().used, 0);
        let budget = ssx_owned_budget::Budget::new(4096).unwrap();
        let result = order_budgeted(&pairs, Instant::now() + Duration::from_secs(10), &budget)
            .await
            .unwrap();
        assert_eq!(result.as_slice(), &[1, 0]);
        assert_eq!(
            budget.snapshot().unwrap().used,
            2 * std::mem::size_of::<usize>()
        );
        drop(result);
        assert_eq!(budget.snapshot().unwrap().used, 0);

        let cancelled = ssx_owned_budget::Budget::new(64 * 1024 * 1024).unwrap();
        let task_budget = cancelled.clone();
        let task = tokio::spawn(async move {
            let word = source_json::Text::from("word");
            // Caller pair storage is intentionally outside this component's
            // charge, exactly as documented by the public opt-in API.
            let pairs = vec![
                Pair {
                    position: Ref::Text(&word),
                    word: &word
                };
                100_000
            ];
            order_budgeted(
                &pairs,
                Instant::now() + Duration::from_secs(60),
                &task_budget,
            )
            .await
        });
        // Observe the actual precharged index storage at a cooperative yield,
        // rather than relying on an assumed millisecond scheduling interval.
        while cancelled.snapshot().unwrap().used == 0 {
            assert!(
                !task.is_finished(),
                "sort must expose charged work before completion"
            );
            tokio::task::yield_now().await;
        }
        task.abort();
        match task.await {
            Err(error) => assert!(error.is_cancelled()),
            Ok(_) => panic!("observed sorter must be cancelled"),
        }
        assert_eq!(cancelled.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn cancellation_during_large_index_copy() {
        let mut sorter = Sorter {
            pairs: &[],
            items: Buffer::Plain((0..1_000_000).collect()),
            pending: Buffer::Plain(Vec::new()),
            min_gallop: 7,
            deadline: Instant::now() + Duration::from_millis(1),
            budget: None,
        };
        assert_eq!(sorter.move_items(0, 1, 999_999).await, Err(Error::Deadline));
    }
    #[tokio::test]
    async fn cancellation_during_gallop_string_comparison() {
        let left = source_json::Text::from("a".repeat(1_000_000));
        let right = source_json::Text::from(format!("{}b", "a".repeat(1_000_000)));
        let word = source_json::Text::from("word");
        let pairs = [
            Pair {
                position: Ref::Text(&left),
                word: &word,
            },
            Pair {
                position: Ref::Text(&right),
                word: &word,
            },
        ];
        let sorter = Sorter {
            pairs: &pairs,
            items: Buffer::Plain(vec![0, 1]),
            pending: Buffer::Plain(Vec::new()),
            min_gallop: 7,
            deadline: Instant::now() + Duration::from_millis(1),
            budget: None,
        };
        assert_eq!(
            sorter.gallop_left(1, &[0, 1], 0).await,
            Err(Error::Deadline)
        );
    }
}

/// Actual adapters retain source diagnostic backing inside this owned error.
/// Legacy order/order_budgeted wrappers preserve their historical String API.
pub enum BoundedError {
    Source(ssx_owned_budget::OwnedString),
    Deadline,
    Resource,
    Invariant,
}
impl std::fmt::Debug for BoundedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(v) => f.debug_tuple("Source").field(&v.as_str()).finish(),
            Self::Deadline => f.write_str("Deadline"),
            Self::Resource => f.write_str("Resource"),
            Self::Invariant => f.write_str("Invariant"),
        }
    }
}
async fn owned_error(
    error: Error,
    budget: &ssx_owned_budget::Budget,
    deadline: Instant,
) -> BoundedError {
    match error {
        Error::Deadline => BoundedError::Deadline,
        Error::Resource => BoundedError::Resource,
        Error::Source(_) => BoundedError::Invariant,
        Error::ComparisonType { left, right } => {
            let result = async {
                tick(deadline).await?;
                let parts = [
                    "'<' not supported between instances of '",
                    left,
                    "' and '",
                    right,
                    "'",
                ];
                let size = parts
                    .iter()
                    .try_fold(0usize, |n, s| n.checked_add(s.len()))
                    .ok_or(Error::Resource)?;
                let mut out =
                    ssx_owned_budget::OwnedString::new(budget).map_err(|_| Error::Resource)?;
                out.try_reserve_exact(size).map_err(|_| Error::Resource)?;
                for part in parts {
                    tick(deadline).await?;
                    out.try_push_str(part).map_err(|_| Error::Resource)?;
                }
                Ok::<_, Error>(out)
            }
            .await;
            match result {
                Ok(v) => BoundedError::Source(v),
                Err(Error::Deadline) => BoundedError::Deadline,
                Err(_) => BoundedError::Resource,
            }
        }
    }
}
/// Same-root sort/comparison/error path; no uncharged source String escape.
pub async fn order_owned(
    pairs: &[Pair<'_>],
    deadline: Instant,
    budget: &ssx_owned_budget::Budget,
) -> Result<Permutation, BoundedError> {
    match order_inner(pairs, deadline, Some(budget)).await {
        Ok(v) => Ok(Permutation(v)),
        Err(e) => Err(owned_error(e, budget, deadline).await),
    }
}

#[cfg(test)]
mod owned_error_controls {
    use super::*;
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
        time::Duration,
    };
    #[tokio::test]
    async fn source_error_storage_stays_charged_through_result_drop_and_midcopy_cancel() {
        let tiny = ssx_owned_budget::Budget::new(1).unwrap();
        assert!(matches!(
            owned_error(
                Error::ComparisonType {
                    left: "dict",
                    right: "str"
                },
                &tiny,
                Instant::now() + Duration::from_secs(5)
            )
            .await,
            BoundedError::Resource
        ));
        assert_eq!(tiny.snapshot().unwrap().used, 0);
        let budget = ssx_owned_budget::Budget::new(4096).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let error = owned_error(
            Error::ComparisonType {
                left: "dict",
                right: "str",
            },
            &budget,
            deadline,
        )
        .await;
        let BoundedError::Source(text) = &error else {
            panic!()
        };
        assert_eq!(
            text.as_str(),
            "'<' not supported between instances of 'dict' and 'str'"
        );
        assert!(budget.snapshot().unwrap().used > 0);
        drop(error);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        for expire in [false, true] {
            let deadline = Instant::now() + Duration::from_secs(if expire { 2 } else { 30 });
            let mut future = Box::pin(owned_error(
                Error::ComparisonType {
                    left: "dict",
                    right: "str",
                },
                &budget,
                deadline,
            ));
            let mut cx = Context::from_waker(Waker::noop());
            let mut observed = 0;
            for _ in 0..20 {
                assert!(matches!(future.as_mut().poll(&mut cx), Poll::Pending));
                if budget.snapshot().unwrap().used > 0 {
                    observed += 1;
                    if observed == 2 {
                        break;
                    }
                }
            }
            assert_eq!(observed, 2);
            if expire {
                tokio::time::sleep_until(deadline.into()).await;
                assert!(matches!(future.await, BoundedError::Deadline));
            } else {
                drop(future);
            }
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
}
