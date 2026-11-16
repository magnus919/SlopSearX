//! Charge-owning output primitives. Full bounded family dispatch is not yet wired.
use ssx_owned_budget::{Budget, Error as ResourceError, OwnedVec};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Resource(ResourceError),
    InvalidCodepoint,
    OutputBound,
    NonAscii,
    Deadline,
}
impl From<ResourceError> for Error {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}
/// Owned validated Python codepoints, including surrogates. No Clone or bare
/// Vec extraction: the backing capacity charge stays with the consumer value.
pub struct BudgetText {
    points: OwnedVec<u32>,
}
impl BudgetText {
    pub fn codepoints(&self) -> &[u32] {
        self.points.as_slice()
    }
    pub fn charged_bytes(&self) -> usize {
        self.points.charged_bytes()
    }
    pub fn capacity(&self) -> usize {
        self.points.capacity()
    }
}
/// Fixed output capacity, reserved before allocation. Family consumption proofs
/// provide the limit; exceeding it is an invariant error, never silent growth.
pub struct BudgetTextBuilder {
    points: OwnedVec<u32>,
    limit: usize,
}
impl BudgetTextBuilder {
    pub fn new(budget: &Budget, limit: usize) -> Result<Self, Error> {
        Ok(Self {
            points: OwnedVec::with_capacity(budget, limit)?,
            limit,
        })
    }
    pub fn try_push(&mut self, point: u32) -> Result<(), Error> {
        if point > 0x10ffff {
            return Err(Error::InvalidCodepoint);
        }
        if self.points.len() == self.limit {
            return Err(Error::OutputBound);
        }
        self.points.try_push(point)?;
        Ok(())
    }
    /// Insert into initialized fixed-capacity output. No backing storage or
    /// charge escapes; the bulk shift remains bounded by the admitted frame.
    pub fn try_insert(&mut self, index: usize, point: u32) -> Result<(), Error> {
        let len = self.points.len();
        if index > len {
            return Err(Error::OutputBound);
        }
        self.try_push(point)?;
        self.points
            .as_mut_slice()
            .copy_within(index..len, index + 1);
        self.points.as_mut_slice()[index] = point;
        Ok(())
    }
    pub fn finish(self) -> BudgetText {
        BudgetText {
            points: self.points,
        }
    }
}
/// ASCII selector result retains its own string capacity charge until dropped.
pub struct BudgetAscii {
    value: OwnedVec<u8>,
}
impl BudgetAscii {
    pub fn try_from_str(budget: &Budget, value: &str) -> Result<Self, Error> {
        if !value.is_ascii() {
            return Err(Error::NonAscii);
        }
        let mut bytes = OwnedVec::with_capacity(budget, value.len())?;
        bytes.try_extend_from_slice(value.as_bytes())?;
        Ok(Self { value: bytes })
    }
    pub fn as_str(&self) -> &str {
        // Every constructor validates ASCII before retaining the buffer.
        std::str::from_utf8(self.value.as_slice()).expect("validated ASCII storage")
    }
    pub fn charged_bytes(&self) -> usize {
        self.value.charged_bytes()
    }
}
/// Resource failures are returned separately by the future bounded strict
/// dispatcher; they must never become this enum's UnicodeFailure/fallback.
pub enum BoundedStrictAsciiOutcome {
    Ascii(BudgetAscii),
    NonAscii,
    UnicodeFailure,
}
/// Private ASCII-only retention with full decoded sequence length. Losing the
/// buffer never short-circuits later decoding, insertion validation, or errors.
pub(crate) struct AsciiCollector<'a> {
    value: Option<OwnedVec<u8>>,
    length: usize,
    limit: usize,
    checkpoint: &'a dyn Fn() -> Result<(), Error>,
}
impl<'a> AsciiCollector<'a> {
    pub(crate) fn new(
        budget: &Budget,
        limit: usize,
        checkpoint: &'a dyn Fn() -> Result<(), Error>,
    ) -> Result<Self, Error> {
        checkpoint()?;
        Ok(Self {
            value: Some(OwnedVec::with_capacity(budget, limit)?),
            length: 0,
            limit,
            checkpoint,
        })
    }
    pub(crate) fn finish(self) -> Result<BoundedStrictAsciiOutcome, Error> {
        (self.checkpoint)()?;
        Ok(match self.value {
            Some(value) => BoundedStrictAsciiOutcome::Ascii(BudgetAscii { value }),
            None => BoundedStrictAsciiOutcome::NonAscii,
        })
    }
}
impl PointSink for AsciiCollector<'_> {
    fn emit(&mut self, point: u32) -> Result<(), Error> {
        self.insert(self.length, point)
    }
}
impl PointSequence for AsciiCollector<'_> {
    fn len(&self) -> usize {
        self.length
    }
    fn insert(&mut self, index: usize, point: u32) -> Result<(), Error> {
        (self.checkpoint)()?;
        if index > self.length || self.length == self.limit {
            return Err(Error::OutputBound);
        }
        if point > 0x10ffff {
            return Err(Error::InvalidCodepoint);
        }
        if point >= 128 {
            self.value = None;
        } else if let Some(value) = &mut self.value {
            // Capacity was fully reserved before decoding. No implicit growth.
            value.try_push(point as u8)?;
            value
                .as_mut_slice()
                .copy_within(index..self.length, index + 1);
            value.as_mut_slice()[index] = point as u8;
        }
        self.length += 1;
        Ok(())
    }
}
/// Shared fallible emission seam; no separate bounded Unicode algorithm.
pub(crate) trait PointSink {
    fn emit(&mut self, point: u32) -> Result<(), Error>;
}
impl PointSink for Vec<u32> {
    fn emit(&mut self, point: u32) -> Result<(), Error> {
        self.push(point);
        Ok(())
    }
}
impl PointSink for BudgetTextBuilder {
    fn emit(&mut self, point: u32) -> Result<(), Error> {
        self.try_push(point)
    }
}

pub(crate) trait PointSequence: PointSink {
    fn len(&self) -> usize;
    fn insert(&mut self, index: usize, point: u32) -> Result<(), Error>;
}
impl PointSequence for Vec<u32> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn insert(&mut self, index: usize, point: u32) -> Result<(), Error> {
        if index > self.len() {
            return Err(Error::OutputBound);
        }
        Vec::insert(self, index, point);
        Ok(())
    }
}
impl PointSequence for BudgetTextBuilder {
    fn len(&self) -> usize {
        self.points.len()
    }
    fn insert(&mut self, index: usize, point: u32) -> Result<(), Error> {
        self.try_insert(index, point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ascii_collector_tracks_insertions_after_non_ascii_and_releases_storage() {
        let budget = Budget::new(8).unwrap();
        let mut output = AsciiCollector::new(&budget, 8, &|| Ok(())).unwrap();
        output.emit(b'a' as u32).unwrap();
        output.insert(0, b'b' as u32).unwrap();
        assert_eq!(output.value.as_ref().unwrap().as_slice(), b"ba");
        output.insert(1, 0x100).unwrap();
        assert_eq!(output.len(), 3);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        output.insert(3, b'c' as u32).unwrap();
        output.insert(0, 0x10ffff).unwrap();
        assert_eq!(output.len(), 5);
        assert_eq!(output.insert(6, b'd' as u32), Err(Error::OutputBound));
        assert_eq!(output.insert(5, 0x110000), Err(Error::InvalidCodepoint));
        assert_eq!(output.len(), 5);
        assert!(matches!(
            output.finish().unwrap(),
            BoundedStrictAsciiOutcome::NonAscii
        ));
    }
    #[test]
    fn ascii_collector_moves_output_charge_and_propagates_late_deadline() {
        use std::cell::Cell;
        let budget = Budget::new(3).unwrap();
        let expired = Cell::new(false);
        let checkpoint = || {
            if expired.get() {
                Err(Error::Deadline)
            } else {
                Ok(())
            }
        };
        let mut output = AsciiCollector::new(&budget, 3, &checkpoint).unwrap();
        output.emit(0).unwrap();
        output.insert(0, b'z' as u32).unwrap();
        let BoundedStrictAsciiOutcome::Ascii(value) = output.finish().unwrap() else {
            panic!("ASCII")
        };
        assert_eq!(value.as_str().as_bytes(), b"z\0");
        assert_eq!(budget.snapshot().unwrap().used, 3);
        drop(value);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let mut output = AsciiCollector::new(&budget, 3, &checkpoint).unwrap();
        output.emit(0x100).unwrap();
        expired.set(true);
        assert_eq!(output.emit(b'x' as u32), Err(Error::Deadline));
        assert!(matches!(output.finish(), Err(Error::Deadline)));
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[test]
    fn insertion_preserves_fixed_capacity_charge_and_validates_before_shift() {
        let budget = Budget::new(16).unwrap();
        let mut output = BudgetTextBuilder::new(&budget, 4).unwrap();
        output.try_insert(0, 0xd800).unwrap();
        output.try_insert(0, 0x10ffff).unwrap();
        output.try_insert(1, 0).unwrap();
        assert_eq!(output.try_insert(4, 1), Err(Error::OutputBound));
        assert_eq!(output.try_insert(0, 0x110000), Err(Error::InvalidCodepoint));
        output.try_insert(3, 0xdc00).unwrap();
        assert_eq!(output.try_insert(0, 1), Err(Error::OutputBound));
        let output = output.finish();
        assert_eq!(output.codepoints(), [0x10ffff, 0, 0xd800, 0xdc00]);
        assert_eq!(output.charged_bytes(), 16);
        assert_eq!(budget.snapshot().unwrap().peak, 16);
        drop(output);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[test]
    fn returned_codepoints_keep_exact_capacity_charge_and_preserve_surrogates() {
        let budget = Budget::new(16).unwrap();
        let mut builder = BudgetTextBuilder::new(&budget, 4).unwrap();
        for point in [0, 0xd800, 0xdc00, 0x10ffff] {
            builder.try_push(point).unwrap();
        }
        assert_eq!(builder.try_push(1), Err(Error::OutputBound));
        let result = builder.finish();
        assert_eq!(result.codepoints(), [0, 0xd800, 0xdc00, 0x10ffff]);
        assert_eq!(result.charged_bytes(), 16);
        assert!(budget.reserve(1).is_err());
        drop(result);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[test]
    fn preallocation_shortage_overflow_invalid_points_and_early_drop_fail_closed() {
        let budget = Budget::new(15).unwrap();
        assert!(matches!(
            BudgetTextBuilder::new(&budget, 4),
            Err(Error::Resource(ResourceError::Exhausted))
        ));
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert!(matches!(
            BudgetTextBuilder::new(&budget, usize::MAX),
            Err(Error::Resource(ResourceError::Overflow))
        ));
        let mut builder = BudgetTextBuilder::new(&budget, 3).unwrap();
        assert_eq!(builder.try_push(0x110000), Err(Error::InvalidCodepoint));
        drop(builder);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[test]
    fn ascii_result_owns_charge_and_resource_failure_is_not_unicode_failure() {
        let budget = Budget::new(4).unwrap();
        let output = BudgetAscii::try_from_str(&budget, "utf8").unwrap();
        assert_eq!(output.as_str(), "utf8");
        assert_eq!(output.charged_bytes(), 4);
        assert!(matches!(
            BudgetAscii::try_from_str(&budget, "x"),
            Err(Error::Resource(ResourceError::Exhausted))
        ));
        assert!(matches!(
            BudgetAscii::try_from_str(&budget, "é"),
            Err(Error::NonAscii)
        ));
        let result = BoundedStrictAsciiOutcome::Ascii(output);
        assert_eq!(budget.snapshot().unwrap().used, 4);
        drop(result);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
