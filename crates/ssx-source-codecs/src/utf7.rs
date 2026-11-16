// SPDX-License-Identifier: PSF-2.0
// Native port of PyUnicode_DecodeUTF7Stateful, errors=replace/final pipeline.
// Python3.14.7 Objects/unicodeobject.c:4734ff; LICENSE-PYTHON and attribution.json.
fn sextet(b: u8) -> Option<u32> {
    match b {
        b'A'..=b'Z' => Some(u32::from(b - b'A')),
        b'a'..=b'z' => Some(u32::from(b - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(b - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
pub fn decode(bytes: &[u8]) -> Vec<u32> {
    let mut output = Vec::with_capacity(bytes.len());
    let valid = decode_into(bytes, false, &mut output).expect("legacy Vec emission is infallible");
    assert!(valid, "replacement UTF7 always returns text");
    output
}
pub(crate) fn decode_strict(bytes: &[u8]) -> Option<Vec<u32>> {
    let mut output = Vec::with_capacity(bytes.len());
    decode_into(bytes, true, &mut output)
        .expect("legacy Vec emission is infallible")
        .then_some(output)
}
pub(crate) fn decode_into(
    bytes: &[u8],
    strict: bool,
    out: &mut impl crate::bounded::PointSink,
) -> Result<bool, crate::bounded::Error> {
    let (mut index, mut shift, mut bits, mut buffer, mut surrogate) = (0, false, 0u32, 0u32, 0u32);
    while index < bytes.len() {
        let byte = bytes[index];
        if shift {
            if let Some(value) = sextet(byte) {
                buffer = (buffer << 6) | value;
                bits += 6;
                index += 1;
                if bits >= 16 {
                    let value = buffer >> (bits - 16);
                    bits -= 16;
                    buffer &= (1 << bits) - 1;
                    if surrogate != 0 {
                        if (0xdc00..=0xdfff).contains(&value) {
                            out.emit(0x10000 + ((surrogate - 0xd800) << 10) + (value - 0xdc00))?;
                            surrogate = 0;
                            continue;
                        }
                        out.emit(surrogate)?;
                        surrogate = 0;
                    }
                    if (0xd800..=0xdbff).contains(&value) {
                        surrogate = value;
                    } else {
                        out.emit(value)?;
                    }
                }
            } else {
                shift = false;
                if bits >= 6 || (bits > 0 && buffer != 0) {
                    index += 1;
                    if strict {
                        return Ok(false);
                    }
                    out.emit(0xfffd)?;
                    continue;
                }
                if surrogate != 0 && byte <= 127 && byte != b'+' {
                    out.emit(surrogate)?;
                }
                surrogate = 0;
                if byte == b'-' {
                    index += 1;
                }
            }
        } else if byte == b'+' {
            index += 1;
            if bytes.get(index) == Some(&b'-') {
                index += 1;
                out.emit(u32::from(b'+'))?;
            } else if bytes.get(index).is_some_and(|b| sextet(*b).is_none()) {
                index += 1;
                if strict {
                    return Ok(false);
                }
                out.emit(0xfffd)?;
            } else {
                shift = true;
                surrogate = 0;
                bits = 0;
                buffer = 0;
            }
        } else {
            index += 1;
            if strict && byte > 127 {
                return Ok(false);
            }
            out.emit(if byte <= 127 { u32::from(byte) } else { 0xfffd })?;
        }
    }
    if shift && (surrogate != 0 || bits >= 6 || (bits > 0 && buffer != 0)) {
        if strict {
            return Ok(false);
        }
        out.emit(0xfffd)?;
    }
    Ok(true)
}
