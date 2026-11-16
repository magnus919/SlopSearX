//! Receipt numbers are unsigned mathematical integers. Inspect their original
//! lexemes before serde_json::Value can round decimal or exponent spellings.
use serde_json::{Value, value::RawValue};

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_DEPTH: usize = 100;

pub(super) fn receipt_value(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() > MAX_BYTES {
        return Err("receipt JSON byte limit exceeded".into());
    }
    let raw: Box<RawValue> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    normalize(&raw, 0)
}

fn normalize(raw: &RawValue, depth: usize) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err("receipt JSON depth limit exceeded".into());
    }
    let text = raw.get();
    match text.as_bytes()[0] {
        b'{' => {
            let fields = serde_json::from_str::<UniqueObject>(text)
                .map_err(|e| e.to_string())?
                .0
                .into_iter()
                .map(|(key, value)| Ok((key, normalize(&value, depth + 1)?)))
                .collect::<Result<_, String>>()?;
            Ok(Value::Object(fields))
        }
        b'[' => Ok(Value::Array(
            serde_json::from_str::<Vec<Box<RawValue>>>(text)
                .map_err(|e| e.to_string())?
                .iter()
                .map(|value| normalize(value, depth + 1))
                .collect::<Result<_, _>>()?,
        )),
        b'-' | b'0'..=b'9' => integer(text).map(Value::from),
        _ => serde_json::from_str(text).map_err(|e| e.to_string()),
    }
}

// Map collection alone silently resolves duplicates. Reject decoded keys while
// reading entries, including escaped spellings of the same key, before any
// schema or typed interpretation. normalize applies this at every object depth.
struct UniqueObject(std::collections::BTreeMap<String, Box<RawValue>>);
impl<'de> serde::Deserialize<'de> for UniqueObject {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> serde::de::Visitor<'de> for ObjectVisitor {
            type Value = UniqueObject;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a receipt object with unique decoded keys")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let mut fields = std::collections::BTreeMap::new();
                while let Some(key) = map.next_key::<String>()? {
                    if fields.contains_key(&key) {
                        return Err(serde::de::Error::custom("duplicate receipt object key"));
                    }
                    fields.insert(key, map.next_value::<Box<RawValue>>()?);
                }
                Ok(UniqueObject(fields))
            }
        }
        deserializer.deserialize_map(ObjectVisitor)
    }
}

// The caller has already validated JSON number grammar through RawValue.
fn integer(raw: &str) -> Result<u64, String> {
    let negative = raw.starts_with('-');
    let unsigned = raw.trim_start_matches('-');
    let (coefficient, exponent) = unsigned.split_once(['e', 'E']).unwrap_or((unsigned, "0"));
    let fractional = coefficient.split_once('.').map_or(0, |(_, f)| f.len());
    let digits: Vec<u8> = coefficient.bytes().filter(|b| *b != b'.').collect();
    let Some(first) = digits.iter().position(|b| *b != b'0') else {
        return Ok(0); // Signed zero is exact even with arbitrarily large exponent.
    };
    if negative {
        return Err("receipt number is negative".into());
    }
    // Beyond this bound the exponent cannot be cancelled by coefficient digits.
    // Saturation avoids overflow and never allocates exponent-sized buffers.
    let cap = raw.len() as i64 + 21;
    let exp_negative = exponent.starts_with('-');
    let mut exp = 0_i64;
    for b in exponent.trim_start_matches(['-', '+']).bytes() {
        exp = (exp * 10 + i64::from(b - b'0')).min(cap);
    }
    if exp_negative {
        exp = -exp;
    }
    let scale = exp - fractional as i64;
    let significant = &digits[first..];
    let (integral, zeros) = if scale < 0 {
        let remove = (-scale) as usize;
        if remove >= significant.len()
            || significant[significant.len() - remove..]
                .iter()
                .any(|b| *b != b'0')
        {
            return Err("receipt number is fractional".into());
        }
        (&significant[..significant.len() - remove], 0)
    } else {
        (significant, scale as usize)
    };
    if integral.len() + zeros > 20 {
        return Err("receipt integer exceeds u64".into());
    }
    integral
        .iter()
        .copied()
        .chain(std::iter::repeat_n(b'0', zeros))
        .try_fold(0_u64, |n, b| {
            n.checked_mul(10)
                .and_then(|n| n.checked_add(u64::from(b - b'0')))
                .ok_or_else(|| "receipt integer exceeds u64".into())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipt_objects_reject_duplicate_decoded_keys_at_every_depth() {
        for raw in [
            r#"{"outcome":"failure","outcome":"accepted"}"#,
            r#"{"counter":1.0,"counter":1e0}"#,
            r#"{"nested":[{"a":1,"a":2}]}"#,
            r#"{"a":1,"\u0061":2}"#,
            r#"{"nested":[{"a":1,"\u0061":2}]}"#,
        ] {
            assert!(receipt_value(raw.as_bytes()).is_err(), "{raw}");
        }
        assert!(receipt_value(br#"{"one":{"a":1},"two":{"a":2}}"#).is_ok());
    }

    #[test]
    fn exact_unsigned_integrality_and_range() {
        for (raw, expected) in [
            ("1.0", 1),
            ("1e0", 1),
            ("-0.0", 0),
            ("-0e999999999999999999999", 0),
            ("1000e-3", 1),
            ("0.0100e2", 1),
            ("1e+3", 1000),
            ("18446744073709551615.0", u64::MAX),
            ("184467440737095516150e-1", u64::MAX),
        ] {
            let v = receipt_value(raw.as_bytes()).unwrap();
            assert_eq!(v.as_u64(), Some(expected), "{raw}");
            assert_eq!(serde_json::from_value::<u64>(v).unwrap(), expected);
        }
        for raw in [
            "-1.0",
            "1.000000000000000000001",
            "0.999999999999999999999",
            "18446744073709551616.0",
            "18446744073709551616",
            "1e999999999999999999999",
            "1e-999999999999999999999",
            "10e-2",
            "1.1",
            "1e20",
        ] {
            assert!(receipt_value(raw.as_bytes()).is_err(), "{raw}");
        }
    }
    #[test]
    fn recursive_normalization_preserves_strings_and_raw_binding() {
        let bytes = br#"{"counter":1.0,"nested":[1e0,-0.0],"id":"1.000000000000000000001"}"#;
        let copy = bytes.to_vec();
        let value = receipt_value(bytes).unwrap();
        assert_eq!(value["counter"], 1);
        assert_eq!(value["nested"], serde_json::json!([1, 0]));
        assert_eq!(value["id"], "1.000000000000000000001");
        assert_eq!(bytes.as_slice(), copy);
        assert_ne!(serde_json::to_vec(&value).unwrap(), copy);
    }
    #[test]
    fn resource_bounds_are_explicit() {
        assert!(receipt_value(&vec![b' '; MAX_BYTES + 1]).is_err());
        let deep = format!(
            "{}0{}",
            "[".repeat(MAX_DEPTH + 1),
            "]".repeat(MAX_DEPTH + 1)
        );
        assert!(receipt_value(deep.as_bytes()).is_err());
        let exponent = format!("1e{}", "9".repeat(100_000));
        assert!(receipt_value(exponent.as_bytes()).is_err());
        assert_eq!(
            receipt_value(exponent.replacen('1', "0", 1).as_bytes()).unwrap(),
            0
        );
    }
}
