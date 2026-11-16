// SPDX-License-Identifier: MIT AND PSF-2.0 AND Unicode-3.0
// CPython3.14.7 stateless one/two-byte codecs. Mapping-specific JIS/legacy
// Unicode grants and original copyrights retained in CJK-MAPPING-NOTICES.txt.
// Invalid input normally consumes one byte; explicit two-byte source errors
// (including JIS2000 emulation return2) are retained as replacement records. No Python runtime or encoding_rs fallback is used for these families.
use crate::{
    Text,
    bounded::{Error as OutputError, PointSink},
};
fn word(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes[..4].try_into().unwrap())
}
pub(crate) fn decode(bytes: &[u8], name: &str) -> Option<Text> {
    decode_mode(bytes, name, false).map(|r| r.expect("replacement CJK mode always returns text"))
}
pub(crate) fn decode_strict(bytes: &[u8], name: &str) -> Option<Result<Text, ()>> {
    decode_mode(bytes, name, true)
}
pub(crate) fn family(name: &str) -> bool {
    matches!(
        name,
        "gb18030"
            | "euc_kr"
            | "cp932"
            | "gbk"
            | "cp949"
            | "cp950"
            | "big5"
            | "big5hkscs"
            | "gb2312"
            | "johab"
            | "shift_jis"
            | "shift_jis_2004"
            | "shift_jisx0213"
            | "euc_jp"
            | "euc_jis_2004"
            | "euc_jisx0213"
    )
}
fn decode_mode(bytes: &[u8], name: &str, strict: bool) -> Option<Result<Text, ()>> {
    if !family(name) {
        return None;
    }
    let mut output = Vec::with_capacity(bytes.len());
    let valid =
        decode_into(bytes, name, strict, &mut output)?.expect("legacy Vec emission is infallible");
    Some(if valid {
        Ok(Text::from_codepoints(output)
            .expect("source mapping Unicode range checked by generator"))
    } else {
        Err(())
    })
}
pub(crate) fn decode_into(
    bytes: &[u8],
    name: &str,
    strict: bool,
    out: &mut impl PointSink,
) -> Option<Result<bool, OutputError>> {
    let (single, pairs) = match name {
        "gb18030" => (
            include_bytes!("cjk/gb18030.single.bin").as_slice(),
            include_bytes!("cjk/gb18030.pairs.bin").as_slice(),
        ),
        "euc_kr" => (
            include_bytes!("cjk/euc_kr.single.bin").as_slice(),
            include_bytes!("cjk/euc_kr.pairs.bin").as_slice(),
        ),
        "cp932" => (
            include_bytes!("cjk/cp932.single.bin").as_slice(),
            include_bytes!("cjk/cp932.pairs.bin").as_slice(),
        ),
        "gbk" => (
            include_bytes!("cjk/gbk.single.bin").as_slice(),
            include_bytes!("cjk/gbk.pairs.bin").as_slice(),
        ),
        "cp949" => (
            include_bytes!("cjk/cp949.single.bin").as_slice(),
            include_bytes!("cjk/cp949.pairs.bin").as_slice(),
        ),
        "cp950" => (
            include_bytes!("cjk/cp950.single.bin").as_slice(),
            include_bytes!("cjk/cp950.pairs.bin").as_slice(),
        ),
        "big5" => (
            include_bytes!("cjk/big5.single.bin").as_slice(),
            include_bytes!("cjk/big5.pairs.bin").as_slice(),
        ),
        "big5hkscs" => (
            include_bytes!("cjk/big5hkscs.single.bin").as_slice(),
            include_bytes!("cjk/big5hkscs.pairs.bin").as_slice(),
        ),
        "gb2312" => (
            include_bytes!("cjk/gb2312.single.bin").as_slice(),
            include_bytes!("cjk/gb2312.pairs.bin").as_slice(),
        ),
        "johab" => (
            include_bytes!("cjk/johab.single.bin").as_slice(),
            include_bytes!("cjk/johab.pairs.bin").as_slice(),
        ),
        "shift_jis" => (
            include_bytes!("cjk/shift_jis.single.bin").as_slice(),
            include_bytes!("cjk/shift_jis.pairs.bin").as_slice(),
        ),
        "shift_jis_2004" => (
            include_bytes!("cjk/shift_jis_2004.single.bin").as_slice(),
            include_bytes!("cjk/shift_jis_2004.pairs.bin").as_slice(),
        ),
        "shift_jisx0213" => (
            include_bytes!("cjk/shift_jisx0213.single.bin").as_slice(),
            include_bytes!("cjk/shift_jisx0213.pairs.bin").as_slice(),
        ),
        "euc_jp" => (
            include_bytes!("cjk/euc_jp.single.bin").as_slice(),
            include_bytes!("cjk/euc_jp.pairs.bin").as_slice(),
        ),
        "euc_jis_2004" => (
            include_bytes!("cjk/euc_jis_2004.single.bin").as_slice(),
            include_bytes!("cjk/euc_jis_2004.pairs.bin").as_slice(),
        ),
        "euc_jisx0213" => (
            include_bytes!("cjk/euc_jisx0213.single.bin").as_slice(),
            include_bytes!("cjk/euc_jisx0213.pairs.bin").as_slice(),
        ),
        _ => return None,
    };
    let triples: Option<&[u8]> = match name {
        "euc_jp" => Some(include_bytes!("cjk/euc_jp.triples.bin")),
        "euc_jis_2004" => Some(include_bytes!("cjk/euc_jis_2004.triples.bin")),
        "euc_jisx0213" => Some(include_bytes!("cjk/euc_jisx0213.triples.bin")),
        _ => None,
    };
    Some((|| {
        let mut cursor = 0;
        while cursor < bytes.len() {
            if name == "gb18030"
                && bytes[cursor] >= 0x80
                && bytes
                    .get(cursor + 1)
                    .is_some_and(|b| (0x30..=0x39).contains(b))
            {
                if bytes.len() - cursor < 4 {
                    if strict {
                        return Ok(false);
                    }
                    out.emit(0xfffd)?;
                    break;
                }
                let [a, b, d, e] = bytes[cursor..cursor + 4].try_into().unwrap();
                let point = if (0x81..=0xfe).contains(&a)
                    && (0x81..=0xfe).contains(&d)
                    && (0x30..=0x39).contains(&e)
                {
                    let index = ((u32::from(a - 0x81) * 10 + u32::from(b - 0x30)) * 126
                        + u32::from(d - 0x81))
                        * 10
                        + u32::from(e - 0x30);
                    if index < 39420 {
                        let ranges = crate::gb18030_ranges::RANGES;
                        let at = ranges.partition_point(|row| row.2 <= index) - 1;
                        Some(ranges[at].0 - ranges[at].2 + index)
                    } else if index >= 189000 && index - 123464 <= 0x10ffff {
                        Some(index - 123464)
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(point) = point {
                    out.emit(point)?;
                    cursor += 4;
                } else {
                    if strict {
                        return Ok(false);
                    }
                    out.emit(0xfffd)?;
                    cursor += 1;
                }
                continue;
            }

            if name == "euc_kr" && bytes[cursor..].starts_with(&[0xa4, 0xd4]) {
                if bytes.len() - cursor < 8 {
                    if strict {
                        return Ok(false);
                    }
                    out.emit(0xfffd)?;
                    break;
                }
                let unit = &bytes[cursor..cursor + 8];
                const L: [u8; 19] = [
                    0xa1, 0xa2, 0xa4, 0xa7, 0xa8, 0xa9, 0xb1, 0xb2, 0xb3, 0xb5, 0xb6, 0xb7, 0xb8,
                    0xb9, 0xba, 0xbb, 0xbc, 0xbd, 0xbe,
                ];
                const T: [u8; 28] = [
                    0xd4, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa9, 0xaa, 0xab, 0xac, 0xad,
                    0xae, 0xaf, 0xb0, 0xb1, 0xb2, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xba, 0xbb, 0xbc,
                    0xbd, 0xbe,
                ];
                if unit[2] == 0xa4
                    && unit[4] == 0xa4
                    && unit[6] == 0xa4
                    && (0xbf..=0xd3).contains(&unit[5])
                    && let (Some(l), Some(t)) = (
                        L.iter().position(|b| *b == unit[3]),
                        T.iter().position(|b| *b == unit[7]),
                    )
                {
                    out.emit(0xac00 + l as u32 * 588 + u32::from(unit[5] - 0xbf) * 28 + t as u32)?;
                    cursor += 8;
                } else {
                    if strict {
                        return Ok(false);
                    }
                    out.emit(0xfffd)?;
                    cursor += 1;
                }
                continue;
            }

            if bytes[cursor] == 0x8f
                && let Some(triples) = triples
            {
                if bytes.len() - cursor < 3 {
                    if strict {
                        return Ok(false);
                    }
                    out.emit(0xfffd)?;
                    break;
                }
                let suffix = u16::from_be_bytes([bytes[cursor + 1], bytes[cursor + 2]]);
                let records = triples.as_chunks::<10>().0;
                if let Ok(index) = records
                    .binary_search_by_key(&suffix, |row| u16::from_be_bytes([row[0], row[1]]))
                {
                    let row = records[index];
                    out.emit(word(&row[2..]))?;
                    let second = word(&row[6..]);
                    if second != u32::MAX {
                        out.emit(second)?;
                    }
                    cursor += 3;
                } else {
                    if strict {
                        return Ok(false);
                    }
                    out.emit(0xfffd)?;
                    cursor += 1;
                }
                continue;
            }
            let point = word(&single[usize::from(bytes[cursor]) * 4..]);
            if point != u32::MAX {
                out.emit(point)?;
                cursor += 1;
                continue;
            }
            if let Some(&second) = bytes.get(cursor + 1) {
                let pair = u16::from_be_bytes([bytes[cursor], second]);
                let records = pairs.as_chunks::<10>().0;
                if let Ok(index) =
                    records.binary_search_by_key(&pair, |row| u16::from_be_bytes([row[0], row[1]]))
                {
                    let row = records[index];
                    if strict && crate::strict_cjk_errors::pair_error(name, pair) {
                        return Ok(false);
                    }
                    out.emit(word(&row[2..]))?;
                    let next = word(&row[6..]);
                    if next != u32::MAX {
                        out.emit(next)?;
                    }
                    cursor += 2;
                    continue;
                }
            }
            if strict {
                return Ok(false);
            }
            out.emit(0xfffd)?;
            cursor += 1;
        }
        Ok(true)
    })())
}

#[cfg(test)]
mod strict_tests {
    use super::*;
    #[test]
    fn every_single_and_pair_strict_outcome_matches_actual_source_and_complete_valid_text() {
        macro_rules! row {
            ($n:literal) => {
                (
                    $n,
                    include_bytes!(concat!("../tests/cjk-strict-validity/", $n, ".valid.bin"))
                        .as_slice(),
                    include_bytes!(concat!("../tests/cjk/", $n, ".source-pairs.bin")).as_slice(),
                )
            };
        }
        let codecs = &[
            row!("big5"),
            row!("big5hkscs"),
            row!("cp932"),
            row!("cp949"),
            row!("cp950"),
            row!("euc_jis_2004"),
            row!("euc_jisx0213"),
            row!("euc_jp"),
            row!("euc_kr"),
            row!("gb18030"),
            row!("gb2312"),
            row!("gbk"),
            row!("johab"),
            row!("shift_jis"),
            row!("shift_jis_2004"),
            row!("shift_jisx0213"),
        ];
        let mut executed = 0;
        for &(name, validity, expected) in codecs {
            assert_eq!(validity.len(), 8224);
            assert_eq!(expected.len(), 65536 * 9);
            for pair in 0..=u16::MAX {
                executed += 1;
                let valid = validity[32 + usize::from(pair) / 8] & (1 << (pair % 8)) != 0;
                let input = pair.to_be_bytes();
                let budget = ssx_owned_budget::Budget::new(16).unwrap();
                let mut charged = crate::BudgetTextBuilder::new(&budget, 2).unwrap();
                assert_eq!(
                    decode_into(&input, name, false, &mut charged).unwrap(),
                    Ok(true)
                );
                let replacement = charged.finish();
                let r = &expected[usize::from(pair) * 9..][..9];
                let full: Vec<u32> = (0..usize::from(r[0]))
                    .map(|i| u32::from_be_bytes(r[1 + i * 4..5 + i * 4].try_into().unwrap()))
                    .collect();
                assert_eq!(
                    replacement.codepoints(),
                    full,
                    "charged replacement {name}/{pair:04x}"
                );
                let mut charged = crate::BudgetTextBuilder::new(&budget, 2).unwrap();
                assert_eq!(
                    decode_into(&input, name, true, &mut charged).unwrap(),
                    Ok(valid)
                );
                let strict_output = charged.finish();
                if valid {
                    assert_eq!(
                        strict_output.codepoints(),
                        full,
                        "charged strict {name}/{pair:04x}"
                    );
                }
                assert_eq!(budget.snapshot().unwrap().used, 16);
                drop((replacement, strict_output));
                assert_eq!(budget.snapshot().unwrap().used, 0);
                let actual = decode_strict(&input, name).unwrap();
                if valid {
                    let r = &expected[usize::from(pair) * 9..][..9];
                    let points: Vec<u32> = (0..usize::from(r[0]))
                        .map(|i| u32::from_be_bytes(r[1 + i * 4..5 + i * 4].try_into().unwrap()))
                        .collect();
                    assert_eq!(actual.unwrap().codepoints(), points, "{name}/{pair:04x}");
                } else {
                    assert!(actual.is_err(), "{name}/{pair:04x}");
                }
            }
            for byte in 0..=u8::MAX {
                executed += 1;
                let valid = validity[usize::from(byte) / 8] & (1 << (byte % 8)) != 0;
                let actual = decode_strict(&[byte], name).unwrap();
                if valid {
                    let r = &expected[usize::from(byte) * 256 * 9..][..9];
                    assert_eq!(r[0], 2);
                    let point = u32::from_be_bytes(r[1..5].try_into().unwrap());
                    assert_eq!(u32::from_be_bytes(r[5..9].try_into().unwrap()), 0);
                    assert_eq!(actual.unwrap().codepoints(), &[point], "{name}/{byte:02x}");
                } else {
                    assert!(actual.is_err(), "{name}/{byte:02x}");
                }
            }
        }
        assert_eq!(executed, 1052672);
    }
}

#[cfg(test)]
mod output_failure_tests {
    use super::*;
    struct Stop;
    impl PointSink for Stop {
        fn emit(&mut self, _: u32) -> Result<(), OutputError> {
            Err(OutputError::OutputBound)
        }
    }
    #[test]
    fn emission_failure_is_typed_before_later_source_error() {
        assert_eq!(
            decode_into(b"A\xff", "gb2312", true, &mut Stop),
            Some(Err(OutputError::OutputBound))
        );
        assert_eq!(
            decode_into(b"\xff", "gb2312", true, &mut Stop),
            Some(Ok(false))
        );
        assert_eq!(decode_into(b"A", "unknown", false, &mut Stop), None);
    }
}
