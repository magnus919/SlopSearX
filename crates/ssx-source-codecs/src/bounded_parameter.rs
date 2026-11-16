//! Private-purpose strict RFC2231 decoding; resource failures never enter email fallback.
use crate::bounded::{AsciiCollector, BoundedStrictAsciiOutcome, Error, PointSink};
use crate::{BudgetAscii, bounded_http::BoundedDecodeError};
use ssx_owned_budget::Budget;
#[derive(Debug, PartialEq, Eq)]
pub enum BoundedParameterError {
    LookupFailure,
    ValueError(&'static str),
    RuntimeError(&'static str),
    OverflowError(&'static str),
    Output(Error),
    RegistryInvariant,
}
impl From<Error> for BoundedParameterError {
    fn from(error: Error) -> Self {
        Self::Output(error)
    }
}
fn puny_error(error: crate::punycode::Failure) -> BoundedParameterError {
    match error {
        crate::punycode::Failure::Source(crate::punycode::StrictError::Unicode { .. }) => {
            BoundedParameterError::RegistryInvariant
        }
        crate::punycode::Failure::Source(crate::punycode::StrictError::Overflow) => {
            BoundedParameterError::OverflowError("Python int too large to convert to C ssize_t")
        }
        crate::punycode::Failure::Output(error) => BoundedParameterError::Output(error),
    }
}
fn success(
    result: Result<bool, Error>,
    output: AsciiCollector<'_>,
) -> Result<BoundedStrictAsciiOutcome, BoundedParameterError> {
    if result? {
        Ok(output.finish()?)
    } else {
        Ok(BoundedStrictAsciiOutcome::UnicodeFailure)
    }
}
pub(crate) fn unicode_into(
    bytes: &[u8],
    name: &str,
    output: &mut impl PointSink,
) -> Option<Result<bool, Error>> {
    if !matches!(
        name,
        "utf-8"
            | "utf-8-sig"
            | "utf-16"
            | "utf-16-le"
            | "utf-16-be"
            | "utf-32"
            | "utf-32-le"
            | "utf-32-be"
    ) {
        return None;
    }
    Some((|| {
        match name {
            "utf-8" | "utf-8-sig" => {
                let bytes = if name == "utf-8-sig" {
                    bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)
                } else {
                    bytes
                };
                let Ok(text) = std::str::from_utf8(bytes) else {
                    return Ok(false);
                };
                for point in text.chars() {
                    output.emit(u32::from(point))?;
                }
            }
            "utf-16" | "utf-16-le" | "utf-16-be" => {
                let (bytes, little) = if name == "utf-16" {
                    if let Some(rest) = bytes.strip_prefix(&[0xff, 0xfe]) {
                        (rest, true)
                    } else if let Some(rest) = bytes.strip_prefix(&[0xfe, 0xff]) {
                        (rest, false)
                    } else {
                        (bytes, cfg!(target_endian = "little"))
                    }
                } else {
                    (bytes, name == "utf-16-le")
                };
                if !bytes.len().is_multiple_of(2) {
                    return Ok(false);
                }
                let units = bytes.as_chunks::<2>().0.iter().map(|b| {
                    if little {
                        u16::from_le_bytes(*b)
                    } else {
                        u16::from_be_bytes(*b)
                    }
                });
                for point in char::decode_utf16(units) {
                    let Ok(point) = point else {
                        return Ok(false);
                    };
                    output.emit(u32::from(point))?;
                }
            }
            "utf-32" | "utf-32-le" | "utf-32-be" => {
                let (bytes, little) = if name == "utf-32" {
                    if let Some(rest) = bytes.strip_prefix(&[0xff, 0xfe, 0, 0]) {
                        (rest, true)
                    } else if let Some(rest) = bytes.strip_prefix(&[0, 0, 0xfe, 0xff]) {
                        (rest, false)
                    } else {
                        (bytes, cfg!(target_endian = "little"))
                    }
                } else {
                    (bytes, name == "utf-32-le")
                };
                if !bytes.len().is_multiple_of(4) {
                    return Ok(false);
                }
                for chunk in bytes.as_chunks::<4>().0 {
                    let point = if little {
                        u32::from_le_bytes(*chunk)
                    } else {
                        u32::from_be_bytes(*chunk)
                    };
                    if point > 0x10ffff || (0xd800..0xe000).contains(&point) {
                        return Ok(false);
                    }
                    output.emit(point)?;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    })())
}
/// Bounded source registry lookup. Whole-response empty-body precedence is
/// caller-owned; unlike strict empty parameters, this operation always looks up.
pub fn lookup_http_encoding_bounded(
    label: &str,
    budget: &Budget,
    checkpoint: &dyn Fn() -> Result<(), Error>,
) -> Result<Option<&'static str>, BoundedParameterError> {
    checkpoint()?;
    if label.contains('\0') {
        return Err(BoundedParameterError::ValueError("embedded null character"));
    }
    crate::bounded_http::canonical(label, budget, checkpoint).map_err(|error| match error {
        BoundedDecodeError::Output(error) => BoundedParameterError::Output(error),
        _ => BoundedParameterError::RegistryInvariant,
    })
}
/// Caller must supervise the operation: non-emitting scans and bulk copies are
/// finite admitted work, not a hard wall-clock guarantee from these callbacks.
pub fn strict_parameter_ascii_bounded(
    bytes: &[u8],
    label: &str,
    budget: &Budget,
    checkpoint: &dyn Fn() -> Result<(), Error>,
) -> Result<BoundedStrictAsciiOutcome, BoundedParameterError> {
    checkpoint()?;
    if label.contains('\0') {
        return Err(BoundedParameterError::ValueError("embedded null character"));
    }
    if bytes.is_empty() {
        return Ok(BoundedStrictAsciiOutcome::Ascii(BudgetAscii::try_from_str(
            budget, "",
        )?));
    }
    let name = lookup_http_encoding_bounded(label, budget, checkpoint)?
        .ok_or(BoundedParameterError::LookupFailure)?;
    if matches!(
        name,
        "base64" | "bz2" | "hex" | "quopri" | "rot-13" | "uu" | "zlib"
    ) {
        return Err(BoundedParameterError::LookupFailure);
    }
    if name == "undefined" {
        return Ok(BoundedStrictAsciiOutcome::UnicodeFailure);
    }
    let work = crate::punycode::Work {
        budget: Some(budget),
        checkpoint,
    };
    if name == "idna" {
        return match crate::idna::ascii_outcome_work(bytes, work) {
            Ok(true) => {
                let text = std::str::from_utf8(bytes)
                    .map_err(|_| BoundedParameterError::RegistryInvariant)?;
                checkpoint()?;
                let value = BudgetAscii::try_from_str(budget, text)?;
                checkpoint()?;
                Ok(BoundedStrictAsciiOutcome::Ascii(value))
            }
            Ok(false) => Ok(BoundedStrictAsciiOutcome::NonAscii),
            Err(crate::punycode::Failure::Source(crate::punycode::StrictError::Unicode {
                ..
            })) => Ok(BoundedStrictAsciiOutcome::UnicodeFailure),
            Err(error) => Err(puny_error(error)),
        };
    }
    if let Some(table) = crate::singlebyte::table(name) {
        for &byte in bytes {
            checkpoint()?;
            if crate::strict_validity::valid(name, byte) != Some(true) {
                return Ok(BoundedStrictAsciiOutcome::UnicodeFailure);
            }
        }
        let mut output = AsciiCollector::new(budget, bytes.len(), checkpoint)?;
        for &byte in bytes {
            output.emit(table[usize::from(byte)])?;
        }
        return Ok(output.finish()?);
    }
    let prepared = if name == "punycode" {
        match crate::punycode::prepare(bytes, true, work) {
            Ok(value) => Some(value),
            Err(crate::punycode::Failure::Source(crate::punycode::StrictError::Unicode {
                ..
            })) => return Ok(BoundedStrictAsciiOutcome::UnicodeFailure),
            Err(error) => return Err(puny_error(error)),
        }
    } else {
        None
    };
    let mut output = AsciiCollector::new(budget, bytes.len(), checkpoint)?;
    if let Some(prepared) = prepared {
        return match crate::punycode::decode_prepared(prepared, work, &mut output) {
            Ok(()) => Ok(output.finish()?),
            Err(crate::punycode::Failure::Source(crate::punycode::StrictError::Unicode {
                ..
            })) => Ok(BoundedStrictAsciiOutcome::UnicodeFailure),
            Err(error) => Err(puny_error(error)),
        };
    }
    if matches!(
        name,
        "utf-8"
            | "utf-8-sig"
            | "utf-16"
            | "utf-16-le"
            | "utf-16-be"
            | "utf-32"
            | "utf-32-le"
            | "utf-32-be"
    ) {
        return success(
            unicode_into(bytes, name, &mut output).expect("closed Unicode names"),
            output,
        );
    }
    if let Some(result) = crate::doublebyte::decode_into(bytes, name, true, &mut output) {
        return success(result, output);
    }
    if let Some(result) = crate::iso2022::decode_into(bytes, name, true, &mut output) {
        return match result {
            Ok(()) => Ok(output.finish()?),
            Err(crate::iso2022::SinkError::Unicode) => {
                Ok(BoundedStrictAsciiOutcome::UnicodeFailure)
            }
            Err(crate::iso2022::SinkError::Runtime(message)) => {
                Err(BoundedParameterError::RuntimeError(message))
            }
            Err(crate::iso2022::SinkError::Output(error)) => {
                Err(BoundedParameterError::Output(error))
            }
        };
    }
    let valid = match name {
        "ascii" => {
            for &byte in bytes {
                checkpoint()?;
                if !byte.is_ascii() {
                    return Ok(BoundedStrictAsciiOutcome::UnicodeFailure);
                }
                output.emit(u32::from(byte))?;
            }
            true
        }
        "charmap" | "iso8859-1" => {
            for &byte in bytes {
                output.emit(u32::from(byte))?;
            }
            true
        }
        "utf-7" => crate::utf7::decode_into(bytes, true, &mut output)?,
        "hz" => crate::hz::decode_into(bytes, true, &mut output)?,
        "unicode-escape" => crate::unicode_escape::decode_into(bytes, true, &mut output)?,
        "raw-unicode-escape" => crate::raw_unicode_escape::decode_into(bytes, true, &mut output)?,
        _ => return Err(BoundedParameterError::RegistryInvariant),
    };
    success(Ok(valid), output)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn unhex(s: &str) -> Vec<u8> {
        s.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
            .collect()
    }
    fn check(row: &serde_json::Value) {
        let bytes = unhex(row["body_hex"].as_str().unwrap());
        let budget = Budget::new(1024 * 1024).unwrap();
        let actual = strict_parameter_ascii_bounded(
            &bytes,
            row["label"].as_str().unwrap(),
            &budget,
            &|| Ok(()),
        );
        if let Some(points) = row["decoded_codepoints"].as_array() {
            if points.iter().all(|p| p.as_u64().unwrap() < 128) {
                let Ok(BoundedStrictAsciiOutcome::Ascii(output)) = actual else {
                    panic!("ASCII {}", row["id"]);
                };
                let expected: Vec<u8> = points.iter().map(|p| p.as_u64().unwrap() as u8).collect();
                assert_eq!(output.as_str().as_bytes(), expected, "{}", row["id"]);
                drop(output);
            } else {
                assert!(
                    matches!(actual, Ok(BoundedStrictAsciiOutcome::NonAscii)),
                    "NonAscii {}",
                    row["id"]
                );
            }
        } else {
            let errors = row["exception_metadata"].as_array().unwrap();
            assert_eq!(errors.len(), 1);
            let error = &errors[0];
            match error["class"].as_str().unwrap() {
                "UnicodeError" | "UnicodeDecodeError" => assert!(
                    matches!(actual, Ok(BoundedStrictAsciiOutcome::UnicodeFailure)),
                    "Unicode {}",
                    row["id"]
                ),
                "LookupError" => assert!(
                    matches!(actual, Err(BoundedParameterError::LookupFailure)),
                    "Lookup {}",
                    row["id"]
                ),
                "ValueError" => assert_eq!(
                    actual.err(),
                    Some(BoundedParameterError::ValueError("embedded null character")),
                    "{}",
                    row["id"]
                ),
                "OverflowError" => assert_eq!(
                    actual.err(),
                    Some(BoundedParameterError::OverflowError(
                        "Python int too large to convert to C ssize_t"
                    )),
                    "{}",
                    row["id"]
                ),
                "RuntimeError" => {
                    let Err(BoundedParameterError::RuntimeError(message)) = actual else {
                        panic!("Runtime {}", row["id"]);
                    };
                    assert_eq!(message, error["message"].as_str().unwrap());
                }
                other => panic!("unexpected source error {other}"),
            }
        }
        assert_eq!(budget.snapshot().unwrap().used, 0, "{}", row["id"]);
    }
    #[test]
    fn all_2574_complete_source_strict_outcomes_and_2255_overflow_precedence() {
        for (source, count) in [
            (include_str!("../tests/strict-parameter-source.json"), 2574),
            (include_str!("../tests/strict-overflow-source.json"), 2255),
        ] {
            let source: serde_json::Value = serde_json::from_str(source).unwrap();
            let rows = source["cases"].as_array().unwrap();
            assert_eq!(rows.len(), count);
            let mut ids = std::collections::BTreeSet::new();
            for row in rows {
                assert!(ids.insert(row["id"].as_str().unwrap()));
                check(row);
            }
            assert_eq!(ids.len(), count);
        }
    }
    #[test]
    fn empty_nul_lookup_resource_and_deadline_are_distinct() {
        let budget = Budget::new(8).unwrap();
        assert!(matches!(
            strict_parameter_ascii_bounded(b"", "unknown", &budget, &|| Ok(())),
            Ok(BoundedStrictAsciiOutcome::Ascii(_))
        ));
        assert!(matches!(
            strict_parameter_ascii_bounded(b"", "bad\0", &budget, &|| Ok(())),
            Err(BoundedParameterError::ValueError("embedded null character"))
        ));
        assert!(matches!(
            strict_parameter_ascii_bounded(b"x", "unknown", &budget, &|| Ok(())),
            Err(BoundedParameterError::LookupFailure)
        ));
        assert!(matches!(
            strict_parameter_ascii_bounded(b"aaaaaaaaa", "ascii", &budget, &|| Ok(())),
            Err(BoundedParameterError::Output(Error::Resource(_)))
        ));
        assert!(matches!(
            strict_parameter_ascii_bounded(b"x", "ascii", &budget, &|| Err(Error::Deadline)),
            Err(BoundedParameterError::Output(Error::Deadline))
        ));
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
