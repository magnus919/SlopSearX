// SPDX-License-Identifier: MIT AND PSF-2.0 AND Unicode-3.0
// CPython3.14.7 Modules/cjkcodecs/_codecs_cn.c DECODER(hz),
// final-input/errors=replace. Change: owned codepoints replace Unicode writer;
// invalid and incomplete source consume rules remain explicit.
use crate::{
    Text,
    bounded::{Error as OutputError, PointSink},
};
pub(crate) fn decode(bytes: &[u8]) -> Text {
    decode_mode(bytes, false).expect("replacement HZ returns text")
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
struct Pair {
    points: [u32; 2],
    count: usize,
}
impl PointSink for Pair {
    fn emit(&mut self, point: u32) -> Result<(), OutputError> {
        let slot = self
            .points
            .get_mut(self.count)
            .ok_or(OutputError::OutputBound)?;
        *slot = point;
        self.count += 1;
        Ok(())
    }
}
pub(crate) fn decode_into(
    bytes: &[u8],
    strict: bool,
    out: &mut impl PointSink,
) -> Result<bool, OutputError> {
    let mut chinese = false;
    let mut cursor = 0;
    while cursor < bytes.len() {
        let first = bytes[cursor];
        if first == b'~' {
            let Some(&second) = bytes.get(cursor + 1) else {
                if strict {
                    return Ok(false);
                }
                out.emit(0xfffd)?;
                break;
            };
            match (second, chinese) {
                (b'~', false) => out.emit(126)?,
                (b'{', false) => chinese = true,
                (b'\n', false) => {}
                (b'}', true) => chinese = false,
                _ => {
                    if strict {
                        return Ok(false);
                    }
                    out.emit(0xfffd)?;
                    cursor += 1;
                    continue;
                }
            }
            cursor += 2;
            continue;
        }
        if first >= 0x80 {
            if strict {
                return Ok(false);
            }
            out.emit(0xfffd)?;
            cursor += 1;
            continue;
        }
        if !chinese {
            out.emit(u32::from(first))?;
            cursor += 1;
            continue;
        }
        let Some(&second) = bytes.get(cursor + 1) else {
            if strict {
                return Ok(false);
            }
            out.emit(0xfffd)?;
            break;
        };
        if (0x21..=0x7e).contains(&first) && (0x21..=0x7e).contains(&second) {
            let mut pair = Pair {
                points: [0; 2],
                count: 0,
            };
            let valid = crate::doublebyte::decode_into(
                &[first | 0x80, second | 0x80],
                "gb2312",
                true,
                &mut pair,
            )
            .expect("GB2312 registered family")?;
            if valid && pair.count == 1 {
                out.emit(pair.points[0])?;
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
}

#[cfg(test)]
mod bounded_tests {
    use super::*;
    #[test]
    fn charged_hz_matches_all_source_state_pairs_and_releases_output() {
        use std::io::Read;
        let mut decoder =
            flate2::read::ZlibDecoder::new(include_bytes!("../tests/hz-source.zlib").as_slice());
        let mut json = String::new();
        decoder.read_to_string(&mut json).unwrap();
        let states: Vec<Vec<Vec<u32>>> = serde_json::from_str(&json).unwrap();
        assert_eq!(states.len(), 2);
        let mut executed = 0;
        for (state, rows) in states.iter().enumerate() {
            assert_eq!(rows.len(), 65536);
            for pair in 0..=u16::MAX {
                let mut input = if state == 0 { vec![] } else { b"~{".to_vec() };
                input.extend(pair.to_be_bytes());
                input.extend(b"~}");
                let budget = ssx_owned_budget::Budget::new((input.len() * 4).max(1)).unwrap();
                let mut output = crate::BudgetTextBuilder::new(&budget, input.len()).unwrap();
                assert_eq!(decode_into(&input, false, &mut output), Ok(true));
                let output = output.finish();
                assert_eq!(
                    output.codepoints(),
                    rows[usize::from(pair)],
                    "{state}/{pair:04x}"
                );
                assert_eq!(budget.snapshot().unwrap().used, input.len() * 4);
                drop(output);
                assert_eq!(budget.snapshot().unwrap().used, 0);
                executed += 1;
            }
        }
        assert_eq!(executed, 131072);
        let budget = ssx_owned_budget::Budget::new(1).unwrap();
        let mut output = crate::BudgetTextBuilder::new(&budget, 0).unwrap();
        assert_eq!(
            decode_into(b"A~!", true, &mut output),
            Err(OutputError::OutputBound)
        );
        assert_eq!(decode_into(b"~!", true, &mut output), Ok(false));
    }
}
