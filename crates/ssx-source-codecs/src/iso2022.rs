// SPDX-License-Identifier: MIT AND PSF-2.0 AND Unicode-3.0
// Native final-input/errors=replace port of CPython3.14.7
// Modules/cjkcodecs/_codecs_iso2022.c decoder/processesc/processg2.
// Owned codepoints replace source writer/state macros; mapping notices retained.
use crate::{
    Text,
    bounded::{Error as OutputError, PointSink},
};
fn end(c: u8) -> bool {
    c.is_ascii_uppercase() || c == b'@'
}
fn allowed(name: &str, mark: u8) -> bool {
    if mark == b'B' {
        return true;
    }
    match name {
        "iso2022_kr" => mark == 0xc3,
        "iso2022_jp" => [0xc2, 0xc0, b'J'].contains(&mark),
        "iso2022_jp_1" => [0xc2, 0xc0, 0xc4, b'J'].contains(&mark),
        "iso2022_jp_2" => [0xc2, 0xc0, 0xc4, 0xc3, 0xc1, b'J', b'A', b'F'].contains(&mark),
        "iso2022_jp_2004" => [0xc2, 0xd1, 0xd0].contains(&mark),
        "iso2022_jp_3" => [0xc2, 0xcf, 0xd0].contains(&mark),
        "iso2022_jp_ext" => [0xc2, 0xc0, 0xc4, b'J', b'I'].contains(&mark),
        _ => false,
    }
}
fn greek(c: u8) -> Option<u32> {
    if c < 0xa0 || (c < 0xc0 && (0x288f3bc9u32 & (1u32 << u32::from(c - 0xa0))) != 0) {
        Some(u32::from(c))
    } else if (0xb4..=0xfe).contains(&c)
        && (c >= 0xd4 || (0xbffffd77u32 & (1u32 << u32::from(c - 0xb4))) != 0)
    {
        Some(0x2d0 + u32::from(c))
    } else {
        match c {
            0xa1 => Some(0x2018),
            0xa2 => Some(0x2019),
            0xaf => Some(0x2015),
            _ => None,
        }
    }
}
fn mapped(name: &str, mark: u8, bytes: &[u8]) -> Option<([u32; 2], usize)> {
    if mark == b'J' {
        return Some((
            [
                match bytes[0] {
                    0x5c => 0xa5,
                    0x7e => 0x203e,
                    c => u32::from(c),
                },
                0,
            ],
            1,
        ));
    }
    if mark == b'I' {
        return (0x21..=0x5f)
            .contains(&bytes[0])
            .then(|| ([0xff40 + u32::from(bytes[0]), 0], 1));
    }
    let table: &[u8] = match mark {
        0xc2 | 0xc0 => include_bytes!("iso2022/jis0208.bin"),
        0xc4 => include_bytes!("iso2022/jis0212.bin"),
        0xc3 => include_bytes!("iso2022/ksx1001.bin"),
        0xc1 => include_bytes!("iso2022/gb2312.bin"),
        0xcf => include_bytes!("iso2022/jis0213_2000_1.bin"),
        0xd1 => include_bytes!("iso2022/jis0213_2004_1.bin"),
        0xd0 if name == "iso2022_jp_3" => include_bytes!("iso2022/jis0213_2000_2.bin"),
        0xd0 => include_bytes!("iso2022/jis0213_2004_2.bin"),
        _ => return None,
    };
    let pair = u16::from_be_bytes([bytes[0], bytes[1]]);
    let rows = table.as_chunks::<10>().0;
    let row = rows[rows
        .binary_search_by_key(&pair, |row| u16::from_be_bytes([row[0], row[1]]))
        .ok()?];
    let first = u32::from_be_bytes(row[2..6].try_into().unwrap());
    let second = u32::from_be_bytes(row[6..10].try_into().unwrap());
    Some(([first, second], if second == u32::MAX { 1 } else { 2 }))
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModeError {
    Unicode,
    Runtime(String),
}
pub(crate) fn decode(bytes: &[u8], name: &str) -> Option<Result<Text, String>> {
    decode_mode(bytes, name, false).map(|r| {
        r.map_err(|e| match e {
            ModeError::Runtime(message) => message,
            ModeError::Unicode => unreachable!("replacement ISO mode cannot raise Unicode failure"),
        })
    })
}
pub(crate) fn decode_strict(bytes: &[u8], name: &str) -> Option<Result<Text, ModeError>> {
    decode_mode(bytes, name, true)
}
pub(crate) fn family(name: &str) -> bool {
    [
        "iso2022_kr",
        "iso2022_jp",
        "iso2022_jp_1",
        "iso2022_jp_2",
        "iso2022_jp_2004",
        "iso2022_jp_3",
        "iso2022_jp_ext",
    ]
    .contains(&name)
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SinkError {
    Unicode,
    Runtime(&'static str),
    Output(OutputError),
}
impl From<OutputError> for SinkError {
    fn from(error: OutputError) -> Self {
        Self::Output(error)
    }
}
fn decode_mode(bytes: &[u8], name: &str, strict: bool) -> Option<Result<Text, ModeError>> {
    if !family(name) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len());
    Some(match decode_into(bytes, name, strict, &mut out)? {
        Ok(()) => Ok(Text::from_codepoints(out).unwrap()),
        Err(SinkError::Unicode) => Err(ModeError::Unicode),
        Err(SinkError::Runtime(message)) => Err(ModeError::Runtime(message.into())),
        Err(SinkError::Output(_)) => unreachable!("legacy Vec emission is infallible"),
    })
}
pub(crate) fn decode_into(
    bytes: &[u8],
    name: &str,
    strict: bool,
    out: &mut impl PointSink,
) -> Option<Result<(), SinkError>> {
    if !family(name) {
        return None;
    }
    Some((|| {
        let shift_enabled = name == "iso2022_kr";
        let g2_enabled = name == "iso2022_jp_2";
        let ext = name != "iso2022_kr";
        let mut g = [b'B'; 3];
        let mut shifted = false;
        let mut through = false;
        let mut cursor = 0;
        while cursor < bytes.len() {
            let c = bytes[cursor];
            if through {
                out.emit(u32::from(c))?;
                cursor += 1;
                if end(c) {
                    through = false;
                }
                continue;
            }
            if c == 0x1b {
                let Some(&next) = bytes.get(cursor + 1) else {
                    if strict {
                        return Err(SinkError::Unicode);
                    }
                    out.emit(0xfffd)?;
                    break;
                };
                if b"()$.&".contains(&next) {
                    let tail = &bytes[cursor..];
                    let mut i = 1;
                    let mut length = 0;
                    let mut incomplete = false;
                    while i < 16 {
                        if i >= tail.len() {
                            incomplete = true;
                            break;
                        }
                        if end(tail[i]) {
                            length = i + 1;
                            break;
                        } else if ext && tail[i] == b'&' && tail.get(i + 1) == Some(&b'@') {
                            i += 2;
                        }
                        i += 1;
                    }
                    if incomplete {
                        if strict {
                            return Err(SinkError::Unicode);
                        }
                        out.emit(0xfffd)?;
                        break;
                    }
                    if length == 0 {
                        if strict {
                            return Err(SinkError::Unicode);
                        }
                        out.emit(0xfffd)?;
                        cursor += 1;
                        continue;
                    }
                    let designation = match length {
                        3 if next == b'$' => Some((0, tail[2] | 0x80)),
                        3 if next == b'(' => Some((0, tail[2])),
                        3 if next == b')' => Some((1, tail[2])),
                        3 if g2_enabled && next == b'.' => Some((2, tail[2])),
                        4 if next == b'$' && tail[2] == b'(' => Some((0, tail[3] | 0x80)),
                        4 if next == b'$' && tail[2] == b')' => Some((1, tail[3] | 0x80)),
                        6 if ext && tail[3..6] == [0x1b, b'$', b'B'] => Some((0, 0xc2)),
                        _ => None,
                    };
                    if let Some((slot, mark)) = designation.filter(|(_, mark)| allowed(name, *mark))
                    {
                        g[slot] = mark;
                    } else {
                        if strict {
                            return Err(SinkError::Unicode);
                        }
                        out.emit(0xfffd)?;
                    }
                    cursor += length;
                    continue;
                }
                if g2_enabled && next == b'N' {
                    let Some(&third) = bytes.get(cursor + 2) else {
                        if strict {
                            return Err(SinkError::Unicode);
                        }
                        out.emit(0xfffd)?;
                        break;
                    };
                    let point = match g[2] {
                        b'A' => (third < 0x80).then_some(u32::from(third) + 0x80),
                        b'F' => greek(third ^ 0x80),
                        b'B' => (third < 0x80).then_some(u32::from(third)),
                        _ => return Err(SinkError::Runtime("internal codec error")),
                    };
                    if strict && point.is_none() {
                        return Err(SinkError::Unicode);
                    }
                    out.emit(point.unwrap_or(0xfffd))?;
                    cursor += 3;
                    continue;
                }
                out.emit(0x1b)?;
                through = true;
                cursor += 1;
                continue;
            }
            if (c == 0x0e || c == 0x0f) && shift_enabled {
                shifted = c == 0x0e;
                cursor += 1;
                continue;
            }
            if c == 0x0a {
                shifted = false;
                out.emit(10)?;
                cursor += 1;
                continue;
            }
            if c < 0x20 {
                out.emit(u32::from(c))?;
                cursor += 1;
                continue;
            }
            if c >= 0x80 {
                if strict {
                    return Err(SinkError::Unicode);
                }
                out.emit(0xfffd)?;
                cursor += 1;
                continue;
            }
            let mark = g[usize::from(shifted)];
            if mark == b'B' {
                out.emit(u32::from(c))?;
                cursor += 1;
                continue;
            }
            let width = if mark & 0x80 != 0 { 2 } else { 1 };
            if bytes.len() - cursor < width {
                if strict {
                    return Err(SinkError::Unicode);
                }
                out.emit(0xfffd)?;
                break;
            }
            if let Some((points, count)) = mapped(name, mark, &bytes[cursor..cursor + width]) {
                for &point in &points[..count] {
                    out.emit(point)?;
                }
            } else {
                if strict {
                    return Err(SinkError::Unicode);
                }
                out.emit(0xfffd)?;
            }
            cursor += width;
        }
        Ok(())
    })())
}

#[cfg(test)]
mod bounded_tests {
    use super::*;
    fn charged_decode(bytes: &[u8], name: &str) -> Result<crate::BudgetText, &'static str> {
        let budget = ssx_owned_budget::Budget::new((bytes.len() * 4).max(1)).unwrap();
        let mut output = crate::BudgetTextBuilder::new(&budget, bytes.len()).unwrap();
        match decode_into(bytes, name, false, &mut output).expect("registered ISO family") {
            Ok(()) => {
                let result = output.finish();
                assert_eq!(budget.snapshot().unwrap().used, bytes.len() * 4);
                Ok(result)
            }
            Err(SinkError::Runtime(message)) => {
                drop(output);
                assert_eq!(budget.snapshot().unwrap().used, 0);
                Err(message)
            }
            other => panic!("unexpected replacement result {other:?}"),
        }
    }
    #[test]
    fn output_failure_is_distinct_from_source_runtime_and_unicode_failure() {
        let budget = ssx_owned_budget::Budget::new(1).unwrap();
        let mut output = crate::BudgetTextBuilder::new(&budget, 0).unwrap();
        assert_eq!(
            decode_into(b"A\xff", "iso2022_jp", true, &mut output),
            Some(Err(SinkError::Output(OutputError::OutputBound)))
        );
        assert_eq!(
            decode_into(b"\xff", "iso2022_jp", true, &mut output),
            Some(Err(SinkError::Unicode))
        );
        assert_eq!(decode_into(b"", "unknown", false, &mut output), None);
    }
    use serde_json::Value;
    fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }
    #[test]
    fn charged_all_byte_pairs_in_every_iso2022_designated_state_match_complete_source() {
        use std::io::Read;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/iso2022");
        let manifest: Vec<Value> =
            serde_json::from_str(include_str!("../tests/iso2022/manifest.json")).unwrap();
        assert_eq!(manifest.len(), 39);
        for state in manifest {
            let codec = state["codec"].as_str().unwrap();
            let prefix = bytes(state["prefix_hex"].as_str().unwrap());
            assert_eq!(state["case_count"], 65536);
            let compressed = std::fs::read(root.join(state["file"].as_str().unwrap())).unwrap();
            let mut decoder = flate2::read::ZlibDecoder::new(compressed.as_slice());
            let errors: std::collections::BTreeMap<String, Value> = serde_json::from_slice(
                &std::fs::read(root.join(state["errors"].as_str().unwrap())).unwrap(),
            )
            .unwrap();
            for pair in 0..=u16::MAX {
                let mut row = [0; 21];
                decoder.read_exact(&mut row).unwrap();
                let mut body = prefix.clone();
                body.extend(pair.to_be_bytes());
                body.extend(b"\x1b(B");
                let actual = charged_decode(&body, codec);
                if row[0] == 255 {
                    let error = &errors[&pair.to_string()];
                    assert_eq!(error["type"], "RuntimeError");
                    assert_eq!(
                        actual.err().unwrap(),
                        error["message"].as_str().unwrap(),
                        "{codec}/state{prefix:?}/{pair:04x}"
                    );
                } else {
                    let count = usize::from(row[0]);
                    assert!(count <= 5);
                    let expected = (0..count)
                        .map(|i| u32::from_be_bytes(row[1 + i * 4..5 + i * 4].try_into().unwrap()))
                        .collect::<Vec<_>>();
                    assert_eq!(
                        actual.unwrap().codepoints(),
                        expected,
                        "{codec}/state{prefix:?}/{pair:04x}"
                    );
                }
            }
            let mut extra = [0];
            assert_eq!(decoder.read(&mut extra).unwrap(), 0);
        }
    }
    #[test]
    fn charged_iso2022_escape_g2_malformed_and_full_streams_match_source() {
        let cases: Vec<Value> =
            serde_json::from_str(include_str!("../tests/iso2022/targeted.json")).unwrap();
        assert_eq!(cases.len(), 44627);
        for case in cases {
            let codec = case["codec"].as_str().unwrap();
            let raw = bytes(case["body_hex"].as_str().unwrap());
            let actual = charged_decode(&raw, codec);
            if let Some(error) = case.get("error") {
                assert_eq!(error["type"], "RuntimeError");
                assert_eq!(
                    actual.err().unwrap(),
                    error["message"].as_str().unwrap(),
                    "{codec}/{raw:?}"
                );
            } else {
                let expected = case["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| p.as_u64().unwrap() as u32)
                    .collect::<Vec<_>>();
                assert_eq!(actual.unwrap().codepoints(), expected, "{codec}/{raw:?}");
            }
        }
    }
}
