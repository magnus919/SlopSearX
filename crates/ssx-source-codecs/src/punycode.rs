// SPDX-License-Identifier: MIT AND PSF-2.0
// Native CPython3.14.7 punycode.py algorithm. Legacy and charged storage share
// arithmetic; private limbs preserve Python arbitrary integers and surrogates.
use crate::{
    Text,
    bounded::{Error as OutputError, PointSequence},
};
use ssx_owned_budget::{Budget, OwnedVec};
#[derive(Clone, Copy)]
pub(crate) struct Work<'a> {
    pub(crate) budget: Option<&'a Budget>,
    pub(crate) checkpoint: &'a dyn Fn() -> Result<(), OutputError>,
}
impl Work<'_> {
    fn check(self) -> Result<(), OutputError> {
        (self.checkpoint)()
    }
}
enum Limbs {
    Legacy(Vec<u32>),
    Charged(OwnedVec<u32>),
}
struct Number(Limbs);
impl Number {
    fn from_slice(points: &[u32], work: Work<'_>) -> Result<Self, OutputError> {
        work.check()?;
        let mut value = match work.budget {
            Some(budget) => Self(Limbs::Charged(
                OwnedVec::with_capacity(budget, points.len()).map_err(OutputError::Resource)?,
            )),
            None => Self(Limbs::Legacy(Vec::with_capacity(points.len()))),
        };
        for &point in points {
            value.push(point, work)?;
        }
        Ok(value)
    }
    fn small(n: u32, work: Work<'_>) -> Result<Self, OutputError> {
        Self::from_slice(&[n], work)
    }
    fn copy(&self, work: Work<'_>) -> Result<Self, OutputError> {
        Self::from_slice(self.slice(), work)
    }
    fn slice(&self) -> &[u32] {
        match &self.0 {
            Limbs::Legacy(v) => v,
            Limbs::Charged(v) => v.as_slice(),
        }
    }
    fn mutable(&mut self) -> &mut [u32] {
        match &mut self.0 {
            Limbs::Legacy(v) => v,
            Limbs::Charged(v) => v.as_mut_slice(),
        }
    }
    fn push(&mut self, value: u32, work: Work<'_>) -> Result<(), OutputError> {
        work.check()?;
        match &mut self.0 {
            Limbs::Legacy(v) => v.push(value),
            Limbs::Charged(v) => {
                v.try_reserve_exact(1).map_err(OutputError::Resource)?;
                v.try_push(value).map_err(OutputError::Resource)?;
            }
        }
        Ok(())
    }
    fn resize(&mut self, len: usize, work: Work<'_>) -> Result<(), OutputError> {
        work.check()?;
        if let Limbs::Charged(v) = &mut self.0 {
            v.try_reserve_exact(len.saturating_sub(v.len()))
                .map_err(OutputError::Resource)?;
        }
        while self.slice().len() < len {
            self.push(0, work)?;
        }
        Ok(())
    }
    fn trim(&mut self, work: Work<'_>) -> Result<(), OutputError> {
        while self.slice().last() == Some(&0) {
            work.check()?;
            match &mut self.0 {
                Limbs::Legacy(v) => {
                    v.pop();
                }
                Limbs::Charged(v) => {
                    v.pop();
                }
            }
        }
        Ok(())
    }
    fn add(&mut self, other: &Self, work: Work<'_>) -> Result<(), OutputError> {
        let mut carry = 0u64;
        self.resize(self.slice().len().max(other.slice().len()), work)?;
        for i in 0..self.slice().len() {
            work.check()?;
            let n = u64::from(self.slice()[i])
                + u64::from(other.slice().get(i).copied().unwrap_or(0))
                + carry;
            self.mutable()[i] = n as u32;
            carry = n >> 32;
        }
        if carry != 0 {
            self.push(carry as u32, work)?;
        }
        Ok(())
    }
    fn mul(&mut self, n: u32, work: Work<'_>) -> Result<(), OutputError> {
        let mut carry = 0u64;
        for x in self.mutable() {
            work.check()?;
            let v = u64::from(*x) * u64::from(n) + carry;
            *x = v as u32;
            carry = v >> 32;
        }
        if carry != 0 {
            self.push(carry as u32, work)?;
        }
        self.trim(work)
    }
    fn div(&mut self, n: u32, work: Work<'_>) -> Result<u32, OutputError> {
        let mut remainder = 0u64;
        for x in self.mutable().iter_mut().rev() {
            work.check()?;
            let v = (remainder << 32) | u64::from(*x);
            *x = (v / u64::from(n)) as u32;
            remainder = v % u64::from(n);
        }
        self.trim(work)?;
        Ok(remainder as u32)
    }
    fn bounded(&self, max: u32) -> Option<u32> {
        if self.slice().len() > 1 {
            None
        } else {
            let n = self.slice().first().copied().unwrap_or(0);
            (n <= max).then_some(n)
        }
    }
    fn bounded_u64(&self) -> Option<u64> {
        (self.slice().len() <= 2).then(|| {
            u64::from(self.slice().first().copied().unwrap_or(0))
                | (u64::from(self.slice().get(1).copied().unwrap_or(0)) << 32)
        })
    }
}
fn adapt(mut delta: Number, first: bool, count: u32, work: Work<'_>) -> Result<usize, OutputError> {
    work.check()?;
    delta.div(if first { 700 } else { 2 }, work)?;
    let mut extra = delta.copy(work)?;
    extra.div(count, work)?;
    delta.add(&extra, work)?;
    let mut divisions = 0usize;
    while delta.bounded(455).is_none() {
        work.check()?;
        delta.div(35, work)?;
        divisions = divisions.checked_add(36).ok_or(OutputError::OutputBound)?;
    }
    let d = delta.bounded(455).unwrap();
    divisions
        .checked_add((36 * d / (d + 38)) as usize)
        .ok_or(OutputError::OutputBound)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StrictError {
    Unicode { end: u64 },
    Overflow,
}
fn failure(end: u64, offset: u64) -> StrictError {
    match end.checked_add(offset).filter(|n| *n <= isize::MAX as u64) {
        Some(end) => StrictError::Unicode { end },
        None => StrictError::Overflow,
    }
}
pub(crate) fn offset_error(error: StrictError, offset: u64) -> StrictError {
    match error {
        StrictError::Unicode { end } => failure(end, offset),
        StrictError::Overflow => StrictError::Overflow,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    Source(StrictError),
    Output(OutputError),
}
impl From<OutputError> for Failure {
    fn from(error: OutputError) -> Self {
        Self::Output(error)
    }
}
pub(crate) struct Prepared<'a> {
    bytes: &'a [u8],
    split: Option<usize>,
    strict: bool,
}
pub(crate) fn prepare<'a>(
    bytes: &'a [u8],
    strict: bool,
    work: Work<'_>,
) -> Result<Prepared<'a>, Failure> {
    let mut split = None;
    for (index, byte) in bytes.iter().enumerate() {
        work.check()?;
        if *byte == b'-' {
            split = Some(index);
        }
    }
    if strict && let Some(split) = split {
        for (index, byte) in bytes[..split].iter().enumerate() {
            work.check()?;
            if !byte.is_ascii() {
                return Err(Failure::Source(failure(index as u64 + 1, 0)));
            }
        }
    }
    Ok(Prepared {
        bytes,
        split,
        strict,
    })
}
pub(crate) fn decode(bytes: &[u8]) -> Text {
    decode_mode(bytes, false).expect("replacement punycode always returns text")
}
pub(crate) fn decode_strict(bytes: &[u8]) -> Result<Text, StrictError> {
    decode_mode(bytes, true)
}
fn decode_mode(bytes: &[u8], strict: bool) -> Result<Text, StrictError> {
    let work = Work {
        budget: None,
        checkpoint: &|| Ok(()),
    };
    let prepared = prepare(bytes, strict, work).map_err(|error| match error {
        Failure::Source(error) => error,
        Failure::Output(_) => unreachable!("legacy checkpoint infallible"),
    })?;
    let mut output = Vec::with_capacity(bytes.len());
    decode_prepared(prepared, work, &mut output).map_err(|error| match error {
        Failure::Source(error) => error,
        Failure::Output(_) => unreachable!("legacy output/checkpoint infallible"),
    })?;
    Ok(Text::from_codepoints(output).unwrap())
}
pub(crate) fn decode_prepared(
    prepared: Prepared<'_>,
    work: Work<'_>,
    base: &mut impl PointSequence,
) -> Result<(), Failure> {
    let Prepared {
        bytes,
        split,
        strict,
    } = prepared;
    let offset = split.map_or(0, |n| n + 1) as u64;
    let extended = match split {
        Some(i) => {
            for byte in &bytes[..i] {
                work.check()?;
                base.emit(if byte.is_ascii() {
                    u32::from(*byte)
                } else {
                    0xfffd
                })?;
            }
            &bytes[i + 1..]
        }
        None => bytes,
    };
    let mut character = 128;
    let mut position = None;
    let mut bias = 72;
    let mut cursor = 0;
    while cursor < extended.len() {
        work.check()?;
        let start = cursor;
        let mut delta = Number::from_slice(&[], work)?;
        let mut weight = Number::small(1, work)?;
        let mut j = 0usize;
        loop {
            work.check()?;
            let Some(byte) = extended.get(cursor) else {
                return if strict {
                    Err(Failure::Source(failure(cursor as u64 + 1, offset)))
                } else {
                    Ok(())
                };
            };
            cursor += 1;
            let digit = match byte.to_ascii_uppercase() {
                b'A'..=b'Z' => u32::from(byte.to_ascii_uppercase() - b'A'),
                b'0'..=b'9' => u32::from(*byte - 22),
                _ => {
                    return if strict {
                        Err(Failure::Source(failure(cursor as u64, offset)))
                    } else {
                        Ok(())
                    };
                }
            };
            let threshold =
                (36usize.saturating_mul(j + 1).saturating_sub(bias)).clamp(1, 26) as u32;
            let mut term = weight.copy(work)?;
            term.mul(digit, work)?;
            delta.add(&term, work)?;
            if digit < threshold {
                break;
            }
            weight.mul(36 - threshold, work)?;
            j += 1;
        }
        let mut next = delta.copy(work)?;
        if let Some(previous) = position {
            next.add(&Number::small(previous + 1, work)?, work)?;
        }
        let error_end = next.bounded_u64();
        let count = base
            .len()
            .checked_add(1)
            .and_then(|count| u32::try_from(count).ok())
            .ok_or(OutputError::OutputBound)?;
        let at = next.div(count, work)?;
        character = match next.bounded(0x10ffff - character) {
            Some(increment) => character + increment,
            None if strict => {
                return Err(Failure::Source(
                    error_end.map_or(StrictError::Overflow, |end| failure(end, offset)),
                ));
            }
            None => 63,
        };
        position = Some(at);
        work.check()?;
        base.insert(at as usize, character)?;
        bias = adapt(delta, start == 0, count, work)?;
    }
    Ok(())
}

#[cfg(test)]
mod bounded_tests {
    use super::*;
    fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }
    #[test]
    fn charged_replacement_and_strict_results_match_complete_original_source() {
        let rows: serde_json::Value =
            serde_json::from_str(include_str!("../tests/punycode-source.json")).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 2055);
        for row in rows.as_array().unwrap() {
            let input = bytes(row["input_hex"].as_str().unwrap());
            let expected = row["expected_codepoints"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_u64().unwrap() as u32)
                .collect::<Vec<_>>();
            let budget = Budget::new(2 * 1024 * 1024).unwrap();
            let work = Work {
                budget: Some(&budget),
                checkpoint: &|| Ok(()),
            };
            let prepared = prepare(&input, false, work).unwrap();
            let mut output = crate::BudgetTextBuilder::new(&budget, input.len()).unwrap();
            assert_eq!(decode_prepared(prepared, work, &mut output), Ok(()));
            let output = output.finish();
            assert_eq!(output.codepoints(), expected, "{}", row["input_hex"]);
            assert_eq!(budget.snapshot().unwrap().used, input.len() * 4);
            drop(output);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../tests/strict-overflow-source.json")).unwrap();
        let rows = source["cases"].as_array().unwrap();
        assert_eq!(rows.len(), 2255);
        let mut executed = 0;
        for row in rows.iter().filter(|r| r["label"] == "punycode") {
            executed += 1;
            let input = bytes(row["body_hex"].as_str().unwrap());
            let budget = Budget::new(2 * 1024 * 1024).unwrap();
            let work = Work {
                budget: Some(&budget),
                checkpoint: &|| Ok(()),
            };
            let result = match prepare(&input, true, work) {
                Err(error) => Err(error),
                Ok(prepared) => {
                    let mut output = crate::BudgetTextBuilder::new(&budget, input.len()).unwrap();
                    decode_prepared(prepared, work, &mut output).map(|()| output.finish())
                }
            };
            if let Some(points) = row["decoded_codepoints"].as_array() {
                let output = result.unwrap();
                let expected = points
                    .iter()
                    .map(|p| p.as_u64().unwrap() as u32)
                    .collect::<Vec<_>>();
                assert_eq!(output.codepoints(), expected, "{}", row["id"]);
                assert_eq!(budget.snapshot().unwrap().used, input.len() * 4);
                drop(output);
            } else {
                let error = result.err().expect("source failure");
                match row["exception_metadata"][0]["class"].as_str().unwrap() {
                    "OverflowError" => assert_eq!(
                        error,
                        Failure::Source(StrictError::Overflow),
                        "{}",
                        row["id"]
                    ),
                    "UnicodeDecodeError" => assert!(
                        matches!(error, Failure::Source(StrictError::Unicode { .. })),
                        "{}: {error:?}",
                        row["id"]
                    ),
                    other => panic!("unexpected source error {other}"),
                }
                assert_eq!(
                    Some(error),
                    decode_strict(&input).err().map(Failure::Source)
                );
            }
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        assert_eq!(executed, 2099);
    }
    #[test]
    fn scratch_copy_growth_and_output_keep_independent_charges() {
        let budget = Budget::new(16).unwrap();
        let work = Work {
            budget: Some(&budget),
            checkpoint: &|| Ok(()),
        };
        let mut number = Number::small(u32::MAX, work).unwrap();
        let one = Number::small(1, work).unwrap();
        number.add(&one, work).unwrap();
        assert_eq!(number.slice(), [0, 1]);
        assert_eq!(budget.snapshot().unwrap().used, 12);
        assert_eq!(budget.snapshot().unwrap().peak, 16);
        assert!(matches!(
            number.copy(work),
            Err(OutputError::Resource(ssx_owned_budget::Error::Exhausted))
        ));
        assert_eq!(budget.snapshot().unwrap().used, 12);
        drop((number, one));
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let budget = Budget::new(16).unwrap();
        let work = Work {
            budget: Some(&budget),
            checkpoint: &|| Ok(()),
        };
        let prepared = prepare(b"a", false, work).unwrap();
        let mut output = crate::BudgetTextBuilder::new(&budget, 1).unwrap();
        assert_eq!(decode_prepared(prepared, work, &mut output), Ok(()));
        let output = output.finish();
        assert_eq!(output.codepoints(), [128]);
        assert_eq!(budget.snapshot().unwrap().used, 4);
        assert_eq!(budget.snapshot().unwrap().peak, 12);
        drop(output);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let budget = Budget::new(11).unwrap();
        let work = Work {
            budget: Some(&budget),
            checkpoint: &|| Ok(()),
        };
        let prepared = prepare(b"a", false, work).unwrap();
        let mut output = crate::BudgetTextBuilder::new(&budget, 1).unwrap();
        assert_eq!(
            decode_prepared(prepared, work, &mut output),
            Err(Failure::Output(OutputError::Resource(
                ssx_owned_budget::Error::Exhausted
            )))
        );
        drop(output);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[test]
    fn every_adaptation_checkpoint_propagates_deadline_without_leaking_scratch() {
        let budget = Budget::new(4096).unwrap();
        let initial = Work {
            budget: Some(&budget),
            checkpoint: &|| Ok(()),
        };
        let checks = std::cell::Cell::new(0);
        let check = || {
            checks.set(checks.get() + 1);
            Ok(())
        };
        let delta = Number::from_slice(&[u32::MAX; 4], initial).unwrap();
        assert!(
            adapt(
                delta,
                false,
                3,
                Work {
                    budget: Some(&budget),
                    checkpoint: &check
                }
            )
            .is_ok()
        );
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let total = checks.get();
        assert!(total > 20);
        for stop in 1..=total {
            let delta = Number::from_slice(&[u32::MAX; 4], initial).unwrap();
            let checks = std::cell::Cell::new(0);
            let check = || {
                let n = checks.get() + 1;
                checks.set(n);
                if n == stop {
                    Err(OutputError::Deadline)
                } else {
                    Ok(())
                }
            };
            assert_eq!(
                adapt(
                    delta,
                    false,
                    3,
                    Work {
                        budget: Some(&budget),
                        checkpoint: &check
                    }
                ),
                Err(OutputError::Deadline)
            );
            assert_eq!(checks.get(), stop);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
    }
    #[test]
    fn prefix_source_failure_precedes_output_admission_and_checkpoints_stop_limb_work() {
        let budget = Budget::new(16).unwrap();
        let held = budget.reserve(16).unwrap();
        let work = Work {
            budget: Some(&budget),
            checkpoint: &|| Ok(()),
        };
        assert!(matches!(
            prepare(b"\xff-", true, work),
            Err(Failure::Source(StrictError::Unicode { end: 1 }))
        ));
        drop(held);
        let work = Work {
            budget: Some(&budget),
            checkpoint: &|| Ok(()),
        };
        let mut number = Number::from_slice(&[1, 2, 3], work).unwrap();
        for operation in [0, 1] {
            let steps = std::cell::Cell::new(0);
            let check = || {
                let n = steps.get() + 1;
                steps.set(n);
                if n == 2 {
                    Err(OutputError::Deadline)
                } else {
                    Ok(())
                }
            };
            let work = Work {
                budget: Some(&budget),
                checkpoint: &check,
            };
            let result = if operation == 0 {
                number.mul(35, work).map(|()| 0)
            } else {
                number.div(35, work)
            };
            assert_eq!(result, Err(OutputError::Deadline));
            assert_eq!(steps.get(), 2);
            assert_eq!(budget.snapshot().unwrap().used, 12);
        }
        drop(number);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let input = [vec![b'9'; 4096], vec![b'a']].concat();
        let budget = Budget::new(128 * 1024).unwrap();
        let work = Work {
            budget: Some(&budget),
            checkpoint: &|| Ok(()),
        };
        let prepared = prepare(&input, false, work).unwrap();
        let mut output = crate::BudgetTextBuilder::new(&budget, input.len()).unwrap();
        let steps = std::cell::Cell::new(0);
        let check = || {
            let n = steps.get() + 1;
            steps.set(n);
            if n == 100 {
                Err(OutputError::Deadline)
            } else {
                Ok(())
            }
        };
        assert_eq!(
            decode_prepared(
                prepared,
                Work {
                    budget: Some(&budget),
                    checkpoint: &check
                },
                &mut output
            ),
            Err(Failure::Output(OutputError::Deadline))
        );
        assert_eq!(steps.get(), 100);
        assert_eq!(budget.snapshot().unwrap().used, input.len() * 4);
        drop(output);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
