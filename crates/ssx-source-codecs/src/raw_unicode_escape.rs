// SPDX-License-Identifier: MIT AND PSF-2.0
// Native port of CPython 3.14.7 Objects/unicodeobject.c
// _PyUnicode_DecodeRawUnicodeEscapeStateful, final input/errors=replace.
// Change: owned u32 codepoints replace the Python Unicode writer/error callback.
use crate::{
    Text,
    bounded::{Error as OutputError, PointSink},
};
pub(crate) fn decode(bytes: &[u8]) -> Text {
    decode_mode(bytes, false).expect("replacement rawUnicodeEscape returns text")
}
pub(crate) fn decode_strict(bytes: &[u8]) -> Option<Text> {
    decode_mode(bytes, true)
}
fn decode_mode(bytes: &[u8], strict: bool) -> Option<Text> {
    let mut out = Vec::with_capacity(bytes.len());
    if !decode_into(bytes, strict, &mut out).expect("legacy Vec emission is infallible") {
        return None;
    }
    Some(Text::from_codepoints(out).expect("decoder checks Unicode range"))
}
pub(crate) fn decode_into(
    bytes: &[u8],
    strict: bool,
    out: &mut impl PointSink,
) -> Result<bool, OutputError> {
    let mut cursor = 0;
    while cursor < bytes.len() {
        let first = bytes[cursor];
        cursor += 1;
        if first != b'\\' || cursor == bytes.len() {
            out.emit(u32::from(first))?;
            continue;
        }
        let marker = bytes[cursor];
        cursor += 1;
        let digits = match marker {
            b'u' => 4,
            b'U' => 8,
            _ => {
                out.emit(u32::from(b'\\'))?;
                out.emit(u32::from(marker))?;
                continue;
            }
        };
        let mut value = 0u32;
        let mut valid = true;
        for _ in 0..digits {
            let Some(digit) = bytes.get(cursor).and_then(|b| char::from(*b).to_digit(16)) else {
                valid = false;
                break;
            };
            value = (value << 4) | digit;
            cursor += 1;
        }
        if strict && (!valid || value > 0x10ffff) {
            return Ok(false);
        }
        out.emit(if valid && value <= 0x10ffff {
            value
        } else {
            0xfffd
        })?;
    }
    Ok(true)
}

#[cfg(test)]
mod bounded_tests {
    use super::*;
    #[test]
    fn charged_raw_escape_matches_complete_source_vectors_and_preserves_failure_stage() {
        let rows: serde_json::Value =
            serde_json::from_str(include_str!("../tests/raw-unicode-escape-source.json")).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 70);
        for row in rows.as_array().unwrap() {
            let hex = row["input_hex"].as_str().unwrap();
            let input = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>();
            let expected = row["expected_codepoints"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_u64().unwrap() as u32)
                .collect::<Vec<_>>();
            let budget = ssx_owned_budget::Budget::new((input.len() * 4).max(1)).unwrap();
            let mut output = crate::BudgetTextBuilder::new(&budget, input.len()).unwrap();
            assert_eq!(decode_into(&input, false, &mut output), Ok(true));
            let output = output.finish();
            assert_eq!(output.codepoints(), expected, "{hex}");
            assert_eq!(budget.snapshot().unwrap().used, input.len() * 4);
            drop(output);
            assert_eq!(budget.snapshot().unwrap().used, 0);
        }
        let budget = ssx_owned_budget::Budget::new(1).unwrap();
        let mut output = crate::BudgetTextBuilder::new(&budget, 0).unwrap();
        assert_eq!(
            decode_into(b"A\\u!", true, &mut output),
            Err(OutputError::OutputBound)
        );
        assert_eq!(decode_into(b"\\u!", true, &mut output), Ok(false));
    }
}
