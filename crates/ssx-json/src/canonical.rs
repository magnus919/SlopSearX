//! Sorted Python-compatible JSON byte profiles for cross-runtime serialization.
use crate::{Error, Value};
fn codec_error(message: &str) -> Error {
    <Error as serde::de::Error>::custom(message)
}
#[derive(Debug, Clone, Copy)]
pub enum JsonProfile {
    AsciiCompact,
    Utf8Compact,
    AsciiSpaced,
}
impl JsonProfile {
    fn ascii(self) -> bool {
        !matches!(self, Self::Utf8Compact)
    }
    fn spaced(self) -> bool {
        matches!(self, Self::AsciiSpaced)
    }
}
fn string(value: &str, ascii: bool, out: &mut String) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 32 || (ascii && ((c as u32) >= 127)) => {
                for unit in c.encode_utf16(&mut [0; 2]).iter() {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
/// CPython JSON uses shortest-roundtrip digits with its own fixed/scientific cutoffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatFormatError {
    Nonfinite,
    Allocation,
    UnexpectedCapacity,
    Invariant,
}
pub fn python_float(value: f64) -> Result<String, Error> {
    try_python_float(value).map_err(|error| {
        codec_error(match error {
            FloatFormatError::Nonfinite => "nonfinite durable JSON number",
            FloatFormatError::Allocation => "float formatting allocation failed",
            FloatFormatError::UnexpectedCapacity => "float formatting capacity mismatch",
            FloatFormatError::Invariant => "float formatting invariant failed",
        })
    })
}
pub fn try_python_float(value: f64) -> Result<String, FloatFormatError> {
    if !value.is_finite() {
        return Err(FloatFormatError::Nonfinite);
    }
    // Ryu's f64 spelling fits 24 bytes. Python's transformed spelling fits
    // 25 (sign +17 digits +dot +e/sign/three exponent digits); stack capacity
    // 32 also covers fixed notation at the existing -4/16 cutoffs.
    struct Output {
        bytes: [u8; 32],
        len: usize,
    }
    impl Output {
        fn byte(&mut self, byte: u8) {
            self.bytes[self.len] = byte;
            self.len += 1;
        }
        fn bytes(&mut self, bytes: &[u8]) {
            for &byte in bytes {
                self.byte(byte);
            }
        }
        fn zeros(&mut self, n: usize) {
            for _ in 0..n {
                self.byte(b'0');
            }
        }
    }
    let mut output = Output {
        bytes: [0; 32],
        len: 0,
    };
    if value.is_sign_negative() {
        output.byte(b'-');
    }
    if value == 0.0 {
        output.bytes(b"0.0");
    } else {
        let mut buffer = ryu::Buffer::new();
        let raw = buffer.format_finite(value.abs());
        let (mantissa, exponent) = raw.split_once('e').map_or((raw, 0), |(m, e)| {
            (m, e.parse::<i32>().expect("Ryu exponent"))
        });
        let dot = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
        let mut all = [0u8; 24];
        let mut end = 0usize;
        for byte in mantissa.bytes() {
            if byte != b'.' {
                all[end] = byte;
                end += 1;
            }
        }
        let mut leading = 0usize;
        while leading < end && all[leading] == b'0' {
            leading += 1;
        }
        while end > leading && all[end - 1] == b'0' {
            end -= 1;
        }
        let digits = &all[leading..end];
        // Nonzero finite values always have a significant digit.
        debug_assert!(!digits.is_empty());
        let power = exponent + dot - leading as i32 - 1;
        if !(-4..16).contains(&power) {
            output.byte(digits[0]);
            if digits.len() > 1 {
                output.byte(b'.');
                output.bytes(&digits[1..]);
            }
            output.byte(b'e');
            output.byte(if power >= 0 { b'+' } else { b'-' });
            let magnitude = power.unsigned_abs();
            if magnitude >= 100 {
                output.byte(b'0' + (magnitude / 100) as u8);
            }
            output.byte(b'0' + ((magnitude / 10) % 10) as u8);
            output.byte(b'0' + (magnitude % 10) as u8);
        } else {
            let decimal = power + 1;
            if decimal <= 0 {
                output.bytes(b"0.");
                output.zeros((-decimal) as usize);
                output.bytes(digits);
            } else if decimal as usize >= digits.len() {
                output.bytes(digits);
                output.zeros(decimal as usize - digits.len());
                output.bytes(b".0");
            } else {
                let at = decimal as usize;
                output.bytes(&digits[..at]);
                output.byte(b'.');
                output.bytes(&digits[at..]);
            }
        }
    }
    if output.len > 25 {
        return Err(FloatFormatError::Invariant);
    }
    let mut text = String::new();
    text.try_reserve_exact(output.len)
        .map_err(|_| FloatFormatError::Allocation)?;
    if text.capacity() != output.len {
        return Err(FloatFormatError::UnexpectedCapacity);
    }
    // All generated bytes are ASCII digits/punctuation; no decoding allocation.
    text.push_str(std::str::from_utf8(&output.bytes[..output.len]).expect("ASCII float"));
    Ok(text)
}
pub fn canonical_json(value: &Value, profile: JsonProfile) -> Result<Vec<u8>, Error> {
    canonical_json_text(
        &crate::to_string(value).map_err(|_| codec_error("invalid JSON"))?,
        profile,
    )
}
/// Parse lossless JSON tokens without serde's private arbitrary-precision map tag.
/// Large integers stay decimal integers and ordinary user dictionaries stay objects.
pub fn canonical_json_text(raw: &str, profile: JsonProfile) -> Result<Vec<u8>, Error> {
    if raw.len() > crate::MAX_JSON_BYTES {
        return Err(codec_error("JSON byte limit exceeded"));
    }
    use serde_json::value::RawValue;
    fn write(raw: &str, p: JsonProfile, out: &mut String, depth: usize) -> Result<(), Error> {
        if depth > 100 {
            return Err(codec_error("durable JSON exceeds depth limit"));
        }
        let raw = raw.trim();
        match raw.as_bytes().first() {
            Some(b'{') => {
                let object: std::collections::BTreeMap<String, Box<RawValue>> =
                    serde_json::from_str(raw).map_err(|_| codec_error("invalid JSON object"))?;
                out.push('{');
                for (index, (key, value)) in object.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                        if p.spaced() {
                            out.push(' ');
                        }
                    }
                    string(key, p.ascii(), out);
                    out.push(':');
                    if p.spaced() {
                        out.push(' ');
                    }
                    write(value.get(), p, out, depth + 1)?;
                }
                out.push('}');
            }
            Some(b'[') => {
                let array: Vec<Box<RawValue>> =
                    serde_json::from_str(raw).map_err(|_| codec_error("invalid JSON array"))?;
                out.push('[');
                for (index, value) in array.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                        if p.spaced() {
                            out.push(' ');
                        }
                    }
                    write(value.get(), p, out, depth + 1)?;
                }
                out.push(']');
            }
            Some(b'"') => {
                let value: String =
                    serde_json::from_str(raw).map_err(|_| codec_error("invalid JSON string"))?;
                string(&value, p.ascii(), out);
            }
            _ => {
                let _: Box<RawValue> =
                    serde_json::from_str(raw).map_err(|_| codec_error("invalid JSON token"))?;
                if ["true", "false", "null"].contains(&raw) {
                    out.push_str(raw);
                } else if raw.contains(['.', 'e', 'E']) {
                    out.push_str(&python_float(
                        raw.parse().map_err(|_| codec_error("invalid float"))?,
                    )?);
                } else {
                    out.push_str(if raw == "-0" { "0" } else { raw });
                }
            }
        }
        Ok(())
    }
    let mut out = String::new();
    write(raw, profile, &mut out, 0)?;
    Ok(out.into_bytes())
}
