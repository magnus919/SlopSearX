//! Exact decimal normalization before standard IEEE conversion. Caller policies
//! decide whether nonfinite values are acceptable; no JSON grammar is widened.
use crate::MAX_JSON_BYTES;
use std::fmt::Write as _;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatError {
    ByteLimit,
    Malformed,
    Incomplete,
}
/// Storage refusal is distinct from JSON lexical validation. The caller owns
/// the backing allocation charge and keeps it alive until the normalizer drops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatBufferError {
    Input(FloatError),
    Capacity,
}
#[derive(Clone, Copy)]
enum Grammar {
    Start,
    IntegerStart,
    Zero,
    Integer,
    FractionStart,
    Fraction,
    ExponentStart,
    ExponentFirst,
    Exponent,
}
enum Prepared {
    Zero,
    Overflow,
    Decimal,
}
/// Input is borrowed and independently bounded by each caller. The constructor also
/// enforces the public codec's 4MiB ceiling for direct API safety. Invalid and
/// incomplete direct inputs return errors, never panic.
pub struct FloatNormalizer<'a> {
    raw: &'a [u8],
    at: usize,
    grammar: Grammar,
    negative: bool,
    exponent_negative: bool,
    exponent: i64,
    total: usize,
    fractional: usize,
    first: Option<usize>,
    trailing: usize,
    normalized: String,
    prepared: Option<Prepared>,
    failed: bool,
}
impl<'a> FloatNormalizer<'a> {
    pub fn new(raw: &'a str) -> Result<Self, FloatError> {
        if raw.len() > MAX_JSON_BYTES {
            return Err(FloatError::ByteLimit);
        }
        if raw.is_empty() {
            return Err(FloatError::Malformed);
        }
        Ok(Self {
            raw: raw.as_bytes(),
            at: 0,
            grammar: Grammar::Start,
            negative: false,
            exponent_negative: false,
            exponent: 0,
            total: 0,
            fractional: 0,
            first: None,
            trailing: 0,
            normalized: String::new(),
            prepared: None,
            failed: false,
        })
    }
    /// Use caller-owned preallocated scratch without implicit growth. The input
    /// bound/initial validation matches `new`. Storage must be empty and cover
    /// raw.len()+8 bytes. Digit/sign/dot construction needs at most raw.len()+1;
    /// preparation adds at most `e-324` (5 bytes), leaving two bytes of margin.
    /// The caller must retain any associated charge through this value's drop.
    pub fn with_buffer(raw: &'a str, buffer: String) -> Result<Self, FloatBufferError> {
        let mut out = Self::new(raw).map_err(FloatBufferError::Input)?;
        let required = raw.len().checked_add(8).ok_or(FloatBufferError::Capacity)?;
        if !buffer.is_empty() || buffer.capacity() < required {
            return Err(FloatBufferError::Capacity);
        }
        out.normalized = buffer;
        Ok(out)
    }
    fn digit(&mut self, byte: u8, fraction: bool) {
        let index = self.total;
        self.total += 1;
        self.fractional += usize::from(fraction);
        if self.first.is_none() && byte != b'0' {
            self.first = Some(index);
            if self.negative {
                self.normalized.push('-');
            }
            self.normalized.push(char::from(byte));
            self.normalized.push('.');
        } else if self.first.is_some() {
            self.normalized.push(char::from(byte));
        }
        if self.first.is_some() {
            if byte == b'0' {
                self.trailing += 1;
            } else {
                self.trailing = 0;
            }
        }
    }
    fn exponent_digit(&mut self, byte: u8) {
        self.exponent = self
            .exponent
            .saturating_mul(10)
            .saturating_add(i64::from(byte - b'0'));
    }
    fn consume(&mut self, byte: u8) -> Result<(), FloatError> {
        use Grammar::*;
        match self.grammar {
            Start if byte == b'-' => {
                self.negative = true;
                self.grammar = IntegerStart;
            }
            Start | IntegerStart => match byte {
                b'0' => {
                    self.digit(byte, false);
                    self.grammar = Zero;
                }
                b'1'..=b'9' => {
                    self.digit(byte, false);
                    self.grammar = Integer;
                }
                _ => return Err(FloatError::Malformed),
            },
            Zero | Integer => match byte {
                b'0'..=b'9' if matches!(self.grammar, Integer) => self.digit(byte, false),
                b'.' => self.grammar = FractionStart,
                b'e' | b'E' => self.grammar = ExponentStart,
                _ => return Err(FloatError::Malformed),
            },
            FractionStart | Fraction => match byte {
                b'0'..=b'9' => {
                    self.digit(byte, true);
                    self.grammar = Fraction;
                }
                b'e' | b'E' if matches!(self.grammar, Fraction) => self.grammar = ExponentStart,
                _ => return Err(FloatError::Malformed),
            },
            ExponentStart => match byte {
                b'+' | b'-' => {
                    self.exponent_negative = byte == b'-';
                    self.grammar = ExponentFirst;
                }
                b'0'..=b'9' => {
                    self.exponent_digit(byte);
                    self.grammar = Exponent;
                }
                _ => return Err(FloatError::Malformed),
            },
            ExponentFirst | Exponent => match byte {
                b'0'..=b'9' => {
                    self.exponent_digit(byte);
                    self.grammar = Exponent;
                }
                _ => return Err(FloatError::Malformed),
            },
        }
        Ok(())
    }
    fn prepare(&mut self) -> Result<(), FloatError> {
        if !matches!(
            self.grammar,
            Grammar::Zero | Grammar::Integer | Grammar::Fraction | Grammar::Exponent
        ) {
            return Err(FloatError::Malformed);
        }
        let Some(first) = self.first else {
            self.prepared = Some(Prepared::Zero);
            return Ok(());
        };
        let exponent = if self.exponent_negative {
            self.exponent.saturating_neg()
        } else {
            self.exponent
        };
        let power = exponent
            .saturating_add(i64::try_from(self.total - first).map_err(|_| FloatError::Malformed)?)
            .saturating_sub(i64::try_from(self.fractional).map_err(|_| FloatError::Malformed)?)
            .saturating_sub(1);
        if power > 308 {
            self.prepared = Some(Prepared::Overflow);
            return Ok(());
        }
        if power < -324 {
            self.prepared = Some(Prepared::Zero);
            return Ok(());
        }
        self.normalized
            .truncate(self.normalized.len() - self.trailing);
        if self.normalized.ends_with('.') {
            self.normalized.pop();
        }
        write!(&mut self.normalized, "e{power}").expect("String writer cannot fail");
        self.prepared = Some(Prepared::Decimal);
        Ok(())
    }
    /// Process at most `work_limit` input bytes. A zero limit makes no progress.
    /// All digit scanning/copying happens here so async callers can cooperate.
    pub fn advance(&mut self, work_limit: usize) -> Result<bool, FloatError> {
        if self.failed {
            return Err(FloatError::Malformed);
        }
        if self.prepared.is_some() {
            return Ok(true);
        }
        let end = self.at.saturating_add(work_limit).min(self.raw.len());
        while self.at < end {
            let byte = self.raw[self.at];
            self.at += 1;
            if let Err(error) = self.consume(byte) {
                self.failed = true;
                return Err(error);
            }
        }
        if self.at == self.raw.len()
            && let Err(error) = self.prepare()
        {
            self.failed = true;
            return Err(error);
        }
        Ok(self.prepared.is_some())
    }
    /// Only terminal state can convert. Standard rounding sees every significant
    /// digit. Async owners check their deadline immediately before and after.
    pub fn finish(&self) -> Result<f64, FloatError> {
        if self.failed {
            return Err(FloatError::Malformed);
        }
        match self.prepared.as_ref().ok_or(FloatError::Incomplete)? {
            Prepared::Zero => Ok(if self.negative { -0.0 } else { 0.0 }),
            Prepared::Overflow => Ok(if self.negative {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            }),
            Prepared::Decimal => self.normalized.parse().map_err(|_| FloatError::Malformed),
        }
    }
}
pub fn parse_json_f64(raw: &str) -> Result<f64, FloatError> {
    let mut value = FloatNormalizer::new(raw)?;
    while !value.advance(8192)? {}
    value.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_api_rejects_malformed_incomplete_and_bounds_without_panics() {
        for raw in [
            "", "+1", "01", "-", "1.", "1e", "1e+", " 1", "1 ", "NaN", "Infinity", "0x1", "١",
            "1_0",
        ] {
            assert!(parse_json_f64(raw).is_err(), "{raw}");
        }
        let mut normalizer = FloatNormalizer::new("1.0").unwrap();
        assert_eq!(normalizer.finish(), Err(FloatError::Incomplete));
        assert!(!normalizer.advance(0).unwrap());
        assert!(!normalizer.advance(1).unwrap());
        assert_eq!(normalizer.finish(), Err(FloatError::Incomplete));
        assert!(normalizer.advance(2).unwrap());
        assert_eq!(normalizer.finish().unwrap(), 1.0);
        assert!(matches!(
            FloatNormalizer::new(&"0".repeat(MAX_JSON_BYTES + 1)),
            Err(FloatError::ByteLimit)
        ));
    }
    #[test]
    fn signed_zero_extreme_exponents_subnormal_and_rounding_are_preserved() {
        for (raw, expected) in [
            ("-0", -0.0),
            ("-0e99999999999999999999", -0.0),
            ("-1e-99999999999999999999", -0.0),
            ("1e99999999999999999999", f64::INFINITY),
            ("-1e99999999999999999999", f64::NEG_INFINITY),
            ("4.9406564584124654e-324", f64::from_bits(1)),
            ("2.4703282292062327e-324", 0.0),
            ("1.7976931348623157e308", f64::MAX),
            ("1.7976931348623159e308", f64::INFINITY),
            (
                "1.00000000000000011102230246251565404236316680908203125",
                1.0,
            ),
            (
                "1.00000000000000011102230246251565404236316680908203126",
                f64::from_bits(1.0f64.to_bits() + 1),
            ),
        ] {
            assert_eq!(
                parse_json_f64(raw).unwrap().to_bits(),
                expected.to_bits(),
                "{raw}"
            );
        }
        let raw = format!("0.{}1e+1990000", "0".repeat(1_990_000));
        assert_eq!(parse_json_f64(&raw).unwrap(), 0.1);
    }
}

#[cfg(test)]
mod buffer_tests {
    use super::*;
    #[test]
    fn provided_scratch_never_reallocates_across_grammar_and_numeric_paths() {
        let mut cases = vec![
            "-0".to_owned(),
            "0.000e999999999999999999999".to_owned(),
            "-1e-99999999999999999999".to_owned(),
            "1e99999999999999999999".to_owned(),
            "-1e99999999999999999999".to_owned(),
            "4.9406564584124654e-324".to_owned(),
            "2.4703282292062327e-324".to_owned(),
            "1.7976931348623157e308".to_owned(),
            "1.7976931348623159e308".to_owned(),
            "1.00000000000000011102230246251565404236316680908203126".to_owned(),
            "-".to_owned(),
            "1.".to_owned(),
            "1e+".to_owned(),
            "+1".to_owned(),
            "01".to_owned(),
        ];
        cases.push(format!("0.{}1e+1990000", "0".repeat(1_990_000)));
        cases.push(format!("-0.{}e+1990000", "0".repeat(1_990_000)));
        cases.push(format!("-1.{}e-308", "1".repeat(100_000)));
        cases.push(format!("1.{}e0", "1".repeat(MAX_JSON_BYTES - 4)));
        for raw in cases {
            let expected = parse_json_f64(&raw);
            let buffer = String::with_capacity(raw.len() + 8);
            let pointer = buffer.as_ptr();
            let capacity = buffer.capacity();
            let mut value = FloatNormalizer::with_buffer(&raw, buffer).unwrap();
            let actual = loop {
                let step = value.advance(1024);
                assert_eq!(value.normalized.as_ptr(), pointer);
                assert_eq!(value.normalized.capacity(), capacity);
                match step {
                    Ok(true) => break value.finish(),
                    Ok(false) => {}
                    Err(error) => break Err(error),
                }
            };
            assert_eq!(actual.map(f64::to_bits), expected.map(f64::to_bits));
        }
    }
    #[test]
    fn storage_refusal_and_constructor_errors_are_distinct() {
        assert!(matches!(
            FloatNormalizer::with_buffer("1", String::new()),
            Err(FloatBufferError::Capacity)
        ));
        let mut nonempty = String::with_capacity(32);
        nonempty.push('x');
        assert!(matches!(
            FloatNormalizer::with_buffer("1", nonempty),
            Err(FloatBufferError::Capacity)
        ));
        assert!(matches!(
            FloatNormalizer::with_buffer("", String::new()),
            Err(FloatBufferError::Input(FloatError::Malformed))
        ));
        let huge = "0".repeat(MAX_JSON_BYTES + 1);
        assert!(matches!(
            FloatNormalizer::with_buffer(&huge, String::new()),
            Err(FloatBufferError::Input(FloatError::ByteLimit))
        ));
    }
}
