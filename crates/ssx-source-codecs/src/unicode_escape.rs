// SPDX-License-Identifier: MIT AND PSF-2.0
// Native final-input/errors=replace port of CPython 3.14.7
// Objects/unicodeobject.c _PyUnicode_DecodeUnicodeEscapeInternal.
// Native Unicode16 name/alias tables retain exact source lookup semantics.
use crate::{
    Text,
    bounded::{Error as OutputError, PointSink},
};
pub(crate) fn decode(bytes: &[u8]) -> Option<Text> {
    decode_mode(bytes, false)
}
pub(crate) fn decode_strict(bytes: &[u8]) -> Option<Text> {
    decode_mode(bytes, true)
}
fn decode_mode(bytes: &[u8], strict: bool) -> Option<Text> {
    let mut out = Vec::with_capacity(bytes.len());
    if !decode_into(bytes, strict, &mut out).expect("legacy Vec emission is infallible") {
        return None;
    }
    Some(Text::from_codepoints(out).unwrap())
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
        if first != b'\\' {
            out.emit(u32::from(first))?;
            continue;
        }
        let Some(&marker) = bytes.get(cursor) else {
            if strict {
                return Ok(false);
            }
            out.emit(0xfffd)?;
            break;
        };
        cursor += 1;
        let simple = match marker {
            b'\\' => Some(92),
            b'\'' => Some(39),
            b'"' => Some(34),
            b'b' => Some(8),
            b'f' => Some(12),
            b't' => Some(9),
            b'n' => Some(10),
            b'r' => Some(13),
            b'v' => Some(11),
            b'a' => Some(7),
            _ => None,
        };
        if let Some(point) = simple {
            out.emit(point)?;
            continue;
        }
        if marker == b'\n' {
            continue;
        }
        if (b'0'..=b'7').contains(&marker) {
            let mut point = u32::from(marker - b'0');
            for _ in 0..2 {
                if let Some(&digit) = bytes.get(cursor).filter(|b| (b'0'..=b'7').contains(b)) {
                    point = (point << 3) + u32::from(digit - b'0');
                    cursor += 1;
                } else {
                    break;
                }
            }
            out.emit(point)?;
            continue;
        }
        let digits = match marker {
            b'x' => 2,
            b'u' => 4,
            b'U' => 8,
            b'N' => {
                if bytes.get(cursor) != Some(&b'{') {
                    if strict {
                        return Ok(false);
                    }
                    out.emit(0xfffd)?;
                    continue;
                }
                cursor += 1;
                let start = cursor;
                while cursor < bytes.len() && bytes[cursor] != b'}' {
                    cursor += 1;
                }
                if cursor == bytes.len() {
                    if strict {
                        return Ok(false);
                    }
                    out.emit(0xfffd)?;
                    break;
                }
                if cursor == start {
                    if strict {
                        return Ok(false);
                    }
                    out.emit(0xfffd)?;
                    continue;
                }
                let point = lookup_name(&bytes[start..cursor]);
                cursor += 1;
                if strict && point.is_none() {
                    return Ok(false);
                }
                out.emit(point.unwrap_or(0xfffd))?;
                continue;
            }
            _ => {
                out.emit(92)?;
                out.emit(u32::from(marker))?;
                continue;
            }
        };
        let mut value = 0;
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

fn lookup_name(bytes: &[u8]) -> Option<u32> {
    if !bytes.is_ascii() {
        return None;
    }
    let name = std::str::from_utf8(bytes).ok()?;
    for (prefix, id) in [("CJK UNIFIED IDEOGRAPH-", 1), ("TANGUT IDEOGRAPH-", 2)] {
        if name
            .get(..prefix.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
        {
            let hex = &name[prefix.len()..];
            if !(4..=6).contains(&hex.len())
                || hex.starts_with('0')
                || !hex.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return None;
            }
            let point = u32::from_str_radix(hex, 16).ok()?;
            return crate::unicode_name_ranges::RANGES
                .iter()
                .any(|&(lo, hi, kind)| kind == id && (lo..=hi).contains(&point))
                .then_some(point);
        }
    }
    crate::unicode_names::NAMES
        .binary_search_by(|(candidate, _)| {
            candidate
                .bytes()
                .cmp(bytes.iter().map(u8::to_ascii_uppercase))
        })
        .ok()
        .map(|i| crate::unicode_names::NAMES[i].1)
}

#[cfg(test)]
mod bounded_tests {
    use super::*;
    fn charged_decode(bytes: &[u8]) -> crate::BudgetText {
        let budget = ssx_owned_budget::Budget::new((bytes.len() * 4).max(1)).unwrap();
        let mut output = crate::BudgetTextBuilder::new(&budget, bytes.len()).unwrap();
        assert_eq!(decode_into(bytes, false, &mut output), Ok(true));
        let output = output.finish();
        assert_eq!(budget.snapshot().unwrap().used, bytes.len() * 4);
        output
    }
    #[test]
    fn emission_failure_does_not_become_source_unicode_failure() {
        let budget = ssx_owned_budget::Budget::new(1).unwrap();
        let mut output = crate::BudgetTextBuilder::new(&budget, 0).unwrap();
        assert_eq!(
            decode_into(b"A\\N{INVALID}", true, &mut output),
            Err(OutputError::OutputBound)
        );
        assert_eq!(decode_into(b"\\N{INVALID}", true, &mut output), Ok(false));
    }
    #[test]
    fn complete_unicode_escape_source_vectors() {
        let rows: serde_json::Value =
            serde_json::from_str(include_str!("../tests/unicode-escape-source.json")).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 340);
        for row in rows.as_array().unwrap() {
            let hex = row["input_hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>();
            let expected = row["expected_codepoints"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| u32::try_from(p.as_u64().unwrap()).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(charged_decode(&bytes).codepoints(), expected, "{hex}");
        }
    }
    #[test]
    fn every_source_name_alias_sequence_case_and_range_control() {
        let rows: Vec<(String, Vec<u32>)> =
            serde_json::from_str(include_str!("../tests/unicode-name-source.json")).unwrap();
        assert_eq!(rows.len(), 104357);
        for (name, expected) in rows {
            let body = format!("\\N{{{name}}}");
            assert_eq!(
                charged_decode(body.as_bytes()).codepoints(),
                expected,
                "name={name:?}"
            );
        }
    }
}
