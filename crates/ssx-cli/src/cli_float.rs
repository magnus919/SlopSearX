//! CLI-local compatibility copy of ssx-json/src/float.rs (repository MIT).
//! The shared constructor's4MiB token guard is removed for Python CLI parity;
//! its exact decimal normalization and vetted standard IEEE conversion remain.
//! Original source SHA and change disposition: docs/rust-rewrite/t08a/provenance.json.
//! Exact decimal normalization before standard IEEE conversion. Caller policies
//! decide whether nonfinite values are acceptable; no JSON grammar is widened.
use std::fmt::Write as _;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatError {
    Malformed,
    Incomplete,
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
/// Borrowed CLI float token, already constrained by the received response length.
/// Invalid/incomplete inputs fail; no shared backend limits are changed.
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
