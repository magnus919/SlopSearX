// SPDX-License-Identifier: MIT
use crate::{
    Text,
    bounded::{Error as OutputError, PointSink},
};
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DecodeFailure {
    Source(&'static str),
    Output(OutputError),
}
impl From<OutputError> for DecodeFailure {
    fn from(error: OutputError) -> Self {
        Self::Output(error)
    }
}
const UTF16_BOM_ERROR: &str =
    "'utf-16' codec can't decode bytes in position 0-1: Stream does not start with BOM";
const UTF32_BOM_ERROR: &str =
    "'utf-32' codec can't decode bytes in position 0-3: Stream does not start with BOM";
fn scalar_family(name: &str) -> bool {
    matches!(
        name,
        "utf-8"
            | "utf-8-sig"
            | "utf-16"
            | "utf-16-le"
            | "utf-16-be"
            | "utf-32"
            | "utf-32-le"
            | "utf-32-be"
    )
}
/// Validate source BOM failures before reserving output backing storage.
pub(crate) fn validate_input(bytes: &[u8], name: &str) -> Result<(), &'static str> {
    if name == "utf-16"
        && bytes.len() >= 2
        && !bytes.starts_with(&[0xff, 0xfe])
        && !bytes.starts_with(&[0xfe, 0xff])
    {
        return Err(UTF16_BOM_ERROR);
    }
    if name == "utf-32"
        && bytes.len() >= 4
        && !bytes.starts_with(&[0xff, 0xfe, 0, 0])
        && !bytes.starts_with(&[0, 0, 0xfe, 0xff])
    {
        return Err(UTF32_BOM_ERROR);
    }
    Ok(())
}
fn utf8_points(bytes: &[u8], output: &mut impl PointSink) -> Result<(), OutputError> {
    let mut remaining = bytes;
    loop {
        match std::str::from_utf8(remaining) {
            Ok(valid) => {
                for point in valid.chars().map(u32::from) {
                    output.emit(point)?;
                }
                return Ok(());
            }
            Err(error) => {
                let prefix = std::str::from_utf8(&remaining[..error.valid_up_to()])
                    .expect("UTF8 error identifies a valid prefix");
                for point in prefix.chars().map(u32::from) {
                    output.emit(point)?;
                }
                output.emit(0xfffd)?;
                match error.error_len() {
                    Some(length) => remaining = &remaining[error.valid_up_to() + length..],
                    None => return Ok(()),
                }
            }
        }
    }
}
/// Same source actions emit into legacy or charge-owning storage. UTF7 has its
/// own stateful writer and is intentionally not handled by this function.
pub(crate) fn decode_into(
    bytes: &[u8],
    name: &str,
    output: &mut impl PointSink,
) -> Option<Result<(), DecodeFailure>> {
    if !scalar_family(name) {
        return None;
    }
    Some((|| {
        validate_input(bytes, name).map_err(DecodeFailure::Source)?;
        match name {
            "utf-8" => utf8_points(bytes, output)?,
            "utf-8-sig" => {
                if bytes.len() < 3 && [0xef, 0xbb, 0xbf].starts_with(bytes) {
                    return Ok(());
                }
                utf8_points(
                    bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes),
                    output,
                )?;
            }
            "utf-16" | "utf-16-le" | "utf-16-be" => {
                let (little, body) = if name == "utf-16" {
                    if let Some(body) = bytes.strip_prefix(&[0xff, 0xfe]) {
                        (true, body)
                    } else if let Some(body) = bytes.strip_prefix(&[0xfe, 0xff]) {
                        (false, body)
                    } else if bytes.len() < 2 {
                        if !bytes.is_empty() {
                            output.emit(0xfffd)?;
                        }
                        return Ok(());
                    } else {
                        return Err(DecodeFailure::Source(UTF16_BOM_ERROR));
                    }
                } else {
                    (name.ends_with("le"), bytes)
                };
                let (words, tail) = body.as_chunks::<2>();
                let word = |w: &[u8; 2]| {
                    if little {
                        u16::from_le_bytes(*w)
                    } else {
                        u16::from_be_bytes(*w)
                    }
                };
                for decoded in char::decode_utf16(words.iter().map(word)) {
                    output.emit(u32::from(decoded.unwrap_or('\u{fffd}')))?;
                }
                if !tail.is_empty()
                    && !words
                        .last()
                        .is_some_and(|w| (0xd800..=0xdbff).contains(&word(w)))
                {
                    output.emit(0xfffd)?;
                }
            }
            "utf-32" | "utf-32-le" | "utf-32-be" => {
                let (little, body) = if name == "utf-32" {
                    if let Some(body) = bytes.strip_prefix(&[0xff, 0xfe, 0, 0]) {
                        (true, body)
                    } else if let Some(body) = bytes.strip_prefix(&[0, 0, 0xfe, 0xff]) {
                        (false, body)
                    } else if bytes.len() < 4 {
                        if !bytes.is_empty() {
                            output.emit(0xfffd)?;
                        }
                        return Ok(());
                    } else {
                        return Err(DecodeFailure::Source(UTF32_BOM_ERROR));
                    }
                } else {
                    (name.ends_with("le"), bytes)
                };
                let (words, tail) = body.as_chunks::<4>();
                for word in words {
                    let point = if little {
                        u32::from_le_bytes(*word)
                    } else {
                        u32::from_be_bytes(*word)
                    };
                    output.emit(u32::from(char::from_u32(point).unwrap_or('\u{fffd}')))?;
                }
                if !tail.is_empty() {
                    output.emit(0xfffd)?;
                }
            }
            _ => unreachable!("closed Unicode family checked above"),
        }
        Ok(())
    })())
}
pub fn decode(bytes: &[u8], name: &str) -> Option<Result<Text, String>> {
    if name == "utf-7" {
        return Some(Text::from_codepoints(crate::utf7::decode(bytes)).map_err(str::to_owned));
    }
    if !scalar_family(name) {
        return None;
    }
    if let Err(message) = validate_input(bytes, name) {
        return Some(Err(message.to_owned()));
    }
    let mut output = Vec::with_capacity(bytes.len());
    let result = decode_into(bytes, name, &mut output)?;
    Some(match result {
        Ok(()) => Text::from_codepoints(output).map_err(str::to_owned),
        Err(DecodeFailure::Source(message)) => Err(message.to_owned()),
        Err(DecodeFailure::Output(_)) => {
            unreachable!("legacy Vec emission has no fallible reservation")
        }
    })
}

#[cfg(test)]
mod bounded_tests {
    use super::*;
    use crate::BudgetTextBuilder;
    use ssx_owned_budget::Budget;
    use std::io::Read;
    #[test]
    fn charged_unicode_sink_matches_every_complete_source_vector_without_string_overlap() {
        let manifest: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../tests/unicode/manifest.json")).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/unicode");
        let mut families = std::collections::BTreeSet::new();
        for group in manifest {
            let name = group["codec"].as_str().unwrap();
            families.insert(name.to_owned());
            let compressed = std::fs::read(root.join(group["file"].as_str().unwrap())).unwrap();
            let mut decoder = flate2::read::ZlibDecoder::new(compressed.as_slice());
            let mut json = String::new();
            decoder.read_to_string(&mut json).unwrap();
            let cases: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
            assert_eq!(cases.len() as u64, group["cases"].as_u64().unwrap());
            for case in cases {
                let hex = case["body_hex"].as_str().unwrap();
                let bytes: Vec<u8> = hex
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
                    .collect();
                let budget = Budget::new((bytes.len() * 4).max(1)).unwrap();
                let mut writer = BudgetTextBuilder::new(&budget, bytes.len()).unwrap();
                let result = if name == "utf-7" {
                    crate::utf7::decode_into(&bytes, false, &mut writer)
                        .map(|valid| assert!(valid))
                        .map_err(DecodeFailure::Output)
                } else {
                    decode_into(&bytes, name, &mut writer).unwrap()
                };
                if let Some(error) = case.get("error") {
                    assert_eq!(error["type"], "UnicodeDecodeError");
                    match result {
                        Err(DecodeFailure::Source(message)) => {
                            assert_eq!(message, error["message"].as_str().unwrap(), "{name}/{hex}")
                        }
                        other => panic!("expected source error for {name}/{hex}, got {other:?}"),
                    }
                    drop(writer);
                } else {
                    result.unwrap();
                    let output = writer.finish();
                    let expected: Vec<u32> = case["points"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| p.as_u64().unwrap() as u32)
                        .collect();
                    assert_eq!(output.codepoints(), expected, "{name}/{hex}");
                    assert_eq!(budget.snapshot().unwrap().used, output.charged_bytes());
                    drop(output);
                }
                assert_eq!(budget.snapshot().unwrap().used, 0);
            }
        }
        assert_eq!(
            families,
            [
                "utf-7",
                "utf-8",
                "utf-8-sig",
                "utf-16",
                "utf-16-le",
                "utf-16-be",
                "utf-32",
                "utf-32-le",
                "utf-32-be"
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
        );
    }
    #[test]
    fn emission_failure_remains_typed_and_stops_before_later_source_actions() {
        struct Fail {
            remaining: usize,
        }
        impl PointSink for Fail {
            fn emit(&mut self, _: u32) -> Result<(), OutputError> {
                if self.remaining == 0 {
                    return Err(OutputError::Resource(ssx_owned_budget::Error::Exhausted));
                }
                self.remaining -= 1;
                Ok(())
            }
        }
        assert_eq!(
            decode_into(b"abc", "utf-8", &mut Fail { remaining: 1 }).unwrap(),
            Err(DecodeFailure::Output(OutputError::Resource(
                ssx_owned_budget::Error::Exhausted
            )))
        );
        assert_eq!(
            decode_into(b"ab", "utf-16", &mut Fail { remaining: 0 }).unwrap(),
            Err(DecodeFailure::Source(
                "'utf-16' codec can't decode bytes in position 0-1: Stream does not start with BOM"
            ))
        );
        assert!(decode_into(b"abc", "utf-7", &mut Fail { remaining: 0 }).is_none());
    }
}
