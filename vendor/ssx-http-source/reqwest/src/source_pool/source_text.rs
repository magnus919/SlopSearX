//! CPython scalar/surrogate strings used before HTTPX URL parsing. No lossy OS
//! conversion, host Unicode database, Python runtime or UTS46 substitution.
#[path = "printable_data.rs"]
mod data;
pub(super) type Text = Vec<u32>;
pub(super) fn text(s: &str) -> Text {
    s.chars().map(u32::from).collect()
}
pub(super) fn string(s: &[u32]) -> Option<String> {
    s.iter().map(|c| char::from_u32(*c)).collect()
}
pub(super) fn lower(s: &[u32]) -> Text {
    let mut result = Vec::new();
    let mut run = String::new();
    for cp in s.iter().copied().chain(Some(0x110000)) {
        if let Some(ch) = char::from_u32(cp) {
            run.push(ch);
        } else {
            result.extend(super::source_lower(&run).chars().map(u32::from));
            run.clear();
            if cp != 0x110000 {
                result.push(cp);
            }
        }
    }
    result
}
pub(super) fn upper(input: &[u32]) -> Text {
    let mut result = Vec::new();
    for cp in input {
        if let Ok(at) = data::UPPER.binary_search_by_key(cp, |r| r.0) {
            result.extend_from_slice(data::UPPER[at].1);
        } else {
            result.push(*cp);
        }
    }
    result
}
pub(super) fn utf16(units: &[u16]) -> Text {
    std::char::decode_utf16(units.iter().copied())
        .map(|c| {
            c.map(u32::from)
                .unwrap_or_else(|e| u32::from(e.unpaired_surrogate()))
        })
        .collect()
}
pub(super) fn surrogateescape(mut bytes: &[u8]) -> Text {
    let mut out = Vec::new();
    while !bytes.is_empty() {
        match std::str::from_utf8(bytes) {
            Ok(valid) => {
                out.extend(valid.chars().map(u32::from));
                break;
            }
            Err(e) => {
                let valid = e.valid_up_to();
                out.extend(
                    std::str::from_utf8(&bytes[..valid])
                        .expect("validated prefix")
                        .chars()
                        .map(u32::from),
                );
                let len = e.error_len().unwrap_or(bytes.len() - valid);
                for b in &bytes[valid..valid + len] {
                    out.push(0xdc00 + u32::from(*b));
                }
                bytes = &bytes[valid + len..];
            }
        }
    }
    out
}
fn printable(cp: u32) -> bool {
    let at = data::PRINTABLE.partition_point(|r| r.0 <= cp);
    at > 0 && cp <= data::PRINTABLE[at - 1].1
}
fn quote(s: &[u32], bytes: bool) -> Text {
    let q = if s.contains(&39) && !s.contains(&34) {
        34
    } else {
        39
    };
    let mut out = Vec::new();
    if bytes {
        out.push(98);
    }
    out.push(q);
    for &cp in s {
        let escaped = match cp {
            9 => Some("\\t".to_owned()),
            10 => Some("\\n".to_owned()),
            13 => Some("\\r".to_owned()),
            92 => Some("\\\\".to_owned()),
            v if v == q => Some(format!("\\{}", char::from_u32(v).unwrap())),
            v if if bytes {
                !(32..127).contains(&v)
            } else {
                !printable(v)
            } =>
            {
                Some(if v <= 0xff {
                    format!("\\x{v:02x}")
                } else if v <= 0xffff {
                    format!("\\u{v:04x}")
                } else {
                    format!("\\U{v:08x}")
                })
            }
            _ => None,
        };
        if let Some(escaped) = escaped {
            out.extend(text(&escaped));
        } else {
            out.push(cp);
        }
    }
    out.push(q);
    out
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Value {
    Integer(u64),
    Text(Text),
    List(Vec<Text>),
    Bytes(Vec<u8>),
    None,
}
impl Value {
    pub(super) fn truth(&self) -> bool {
        match self {
            Self::Integer(v) => *v != 0,
            Self::Text(v) => !v.is_empty(),
            Self::List(v) => !v.is_empty(),
            Self::Bytes(v) => !v.is_empty(),
            Self::None => false,
        }
    }
    pub(super) fn spelling(&self) -> Text {
        match self {
            Self::Integer(v) => text(&v.to_string()),
            Self::Text(v) => v.clone(),
            Self::None => text("None"),
            Self::Bytes(v) => quote(&v.iter().map(|v| u32::from(*v)).collect::<Vec<_>>(), true),
            Self::List(v) => {
                let mut out = text("[");
                for (i, s) in v.iter().enumerate() {
                    if i > 0 {
                        out.extend(text(", "));
                    }
                    out.extend(quote(s, false));
                }
                out.extend(text("]"));
                out
            }
        }
    }
}
/// Exact Reg2Py string/cardinality rules, including odd-byte floor division,
/// missing final terminators and leading empty MULTI_SZ elements.
pub(super) fn registry(kind: u32, bytes: &[u8]) -> crate::Result<Value> {
    match kind {
        4 | 11 => {
            let width = if kind == 4 { 4 } else { 8 };
            if bytes.is_empty() {
                return Ok(Value::Integer(0));
            }
            if bytes.len() < width {
                return Err(crate::error::builder(
                    "source winreg numeric value would read uninitialized bytes",
                ));
            }
            let mut value = [0u8; 8];
            value[..width].copy_from_slice(&bytes[..width]);
            Ok(Value::Integer(u64::from_le_bytes(value)))
        }
        1 | 2 | 7 => {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect();
            if kind != 7 {
                let end = units.iter().position(|v| *v == 0).unwrap_or(units.len());
                return Ok(Value::Text(utf16(&units[..end])));
            }
            let q = units.len() - usize::from(units.last() == Some(&0));
            let mut at = 0;
            let mut result = Vec::new();
            while at < q {
                let end = units[at..q]
                    .iter()
                    .position(|v| *v == 0)
                    .map_or(q, |v| at + v);
                result.push(utf16(&units[at..end]));
                at = end + 1;
            }
            Ok(Value::List(result))
        }
        _ => Ok(if bytes.is_empty() {
            Value::None
        } else {
            Value::Bytes(bytes.to_vec())
        }),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(units: &[u16]) -> Vec<u8> {
        units.iter().flat_map(|c| c.to_le_bytes()).collect()
    }
    #[test]
    fn registry_multisz_empty_prefix_and_terminator_cardinality() {
        for (units, expected) in [
            (vec![], vec![]),
            (vec![0], vec![]),
            (vec![0, 0], vec![vec![]]),
            (vec![0, 97, 0, 0], vec![vec![], vec![97]]),
            (vec![97], vec![vec![97]]),
        ] {
            let v = registry(7, &bytes(&units)).unwrap();
            assert_eq!(
                v.truth(),
                !expected.is_empty(),
                "source MULTI_SZ list cardinality controls truthiness"
            );
            assert_eq!(v, Value::List(expected));
        }
    }
    #[test]
    fn registry_text_surrogates_odd_bytes_and_numeric_zero() {
        assert_eq!(
            registry(1, &[0, 216, 65]).unwrap(),
            Value::Text(vec![0xd800])
        );
        assert_eq!(
            registry(2, &bytes(&[65, 0, 66])).unwrap(),
            Value::Text(vec![65])
        );
        assert_eq!(registry(4, &[]).unwrap(), Value::Integer(0));
        assert_eq!(registry(11, &[]).unwrap(), Value::Integer(0));
        assert!(registry(4, &[1]).is_err());
    }
    #[test]
    fn registry_source_spelling_all_value_families() {
        assert_eq!(
            string(&registry(3, &[]).unwrap().spelling()).unwrap(),
            "None"
        );
        assert_eq!(
            string(&registry(3, b"a'\n\xff").unwrap().spelling()).unwrap(),
            "b\"a'\\n\\xff\""
        );
        assert_eq!(
            string(&registry(7, &bytes(&[0xd800, 0, 0])).unwrap().spelling()).unwrap(),
            "['\\ud800']"
        );
        assert_eq!(
            string(&registry(4, &42u32.to_le_bytes()).unwrap().spelling()).unwrap(),
            "42"
        );
    }
    #[test]
    fn surrogateescape_and_wide_preserve_original_nonunicode() {
        assert_eq!(
            surrogateescape(&[b'a', 0xff, 0xc3, 0xa9]),
            vec![97, 0xdcff, 233]
        );
        assert_eq!(
            utf16(&[0xd800, 65, 0xdc00, 0xd83d, 0xde00]),
            vec![0xd800, 65, 0xdc00, 0x1f600]
        );
        assert_eq!(lower(&[0x212a, 0xd800, 0x3a3]), vec![107, 0xd800, 0x3c3]);
    }
    #[test]
    fn printable_table_matches_every_pinned_c_scalar_and_surrogate() {
        let header = include_str!("python-text-source/Objects__unicodetype_db.h.txt");
        let array = |name: &str| -> Vec<i64> {
            let start = format!("{name}[] = {{");
            let body = header
                .split_once(&start)
                .unwrap()
                .1
                .split_once("};")
                .unwrap()
                .0;
            body.split(|ch: char| !(ch.is_ascii_digit() || ch == '-'))
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().unwrap())
                .collect()
        };
        let records = array("_PyUnicode_TypeRecords");
        let first = array("index1");
        let second = array("index2");
        let extended = array("_PyUnicode_ExtendedCase");
        for cp in 0..0x110000u32 {
            let row =
                second[((first[(cp >> 6) as usize] as usize) << 6) + (cp as usize & 63)] as usize;
            assert_eq!(
                printable(cp),
                records[row * 6 + 5] & 0x400 != 0,
                "source printable U+{cp:X}"
            );
            let u = records[row * 6];
            let expected = if records[row * 6 + 5] & 0x4000 != 0 {
                extended[(u as usize & 0xffff)..(u as usize & 0xffff) + (u as usize >> 24)]
                    .iter()
                    .map(|v| *v as u32)
                    .collect::<Vec<_>>()
            } else {
                vec![(i64::from(cp) + u) as u32]
            };
            assert_eq!(upper(&[cp]), expected, "source upper U+{cp:X}");
        }
        assert_eq!(
            string(&quote(&[0xd800, 0x200b, 0x1f600], false)).unwrap(),
            "'\\ud800\\u200b😀'"
        );
    }
}
