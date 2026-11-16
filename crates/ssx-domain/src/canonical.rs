//! Explicit Python-compatible JSON profiles for durable cross-runtime identities.
use crate::DomainError;
use sha2::{Digest, Sha256};
use ssx_json::Value;
pub use ssx_json::{JsonProfile, canonical_json, canonical_json_text, python_float};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
pub fn composite_artifact_id(parts: &[&str]) -> Result<String, DomainError> {
    if parts.is_empty() || parts.iter().any(|p| p.is_empty()) {
        return Err(DomainError("composite parts must be nonempty strings"));
    }
    let value = Value::Array(parts.iter().map(|p| Value::String((*p).into())).collect());
    Ok(format!(
        "composite-v1-{}",
        hex(&canonical_json(&value, JsonProfile::AsciiCompact)?)
    ))
}
pub fn parse_composite_artifact_id(id: &str, count: usize) -> Result<Vec<String>, DomainError> {
    let raw = id
        .strip_prefix("composite-v1-")
        .ok_or(DomainError("invalid composite prefix"))?;
    if raw.len() % 2 != 0 || !raw.is_ascii() {
        return Err(DomainError("invalid composite hex"));
    }
    let bytes = (0..raw.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&raw[i..i + 2], 16).map_err(|_| DomainError("invalid composite hex"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let parts: Vec<String> =
        serde_json::from_slice(&bytes).map_err(|_| DomainError("invalid composite parts"))?;
    if parts.len() != count || parts.iter().any(String::is_empty) {
        return Err(DomainError("invalid composite parts"));
    }
    Ok(parts)
}
pub fn plan_digest(plan: &Value) -> Result<String, DomainError> {
    Ok(hex(&Sha256::digest(canonical_json(
        plan,
        JsonProfile::AsciiCompact,
    )?)))
}
pub fn manifest_artifact_id(payload: &Value) -> Result<String, DomainError> {
    Ok(format!(
        "manifest-v1-{}",
        hex(&Sha256::digest(canonical_json(
            payload,
            JsonProfile::Utf8Compact
        )?))
    ))
}
pub fn saved_identity(parts: &[Value]) -> Result<String, DomainError> {
    plan_digest(&Value::Array(parts.to_vec()))
}
/// Unicode 16 full, default case folding matching deployed CPython 3.14.
pub use ssx_json::casefold;
#[cfg(test)]
mod tests {
    use super::*;
    use ssx_json::json;
    #[test]
    fn python_float_boundaries() {
        for (value, expected) in [
            (1e-5, "1e-05"),
            (1e-4, "0.0001"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (-0.0, "-0.0"),
            (1.25, "1.25"),
            (f64::MIN_POSITIVE, "2.2250738585072014e-308"),
        ] {
            assert_eq!(python_float(value).unwrap(), expected);
        }
        assert!(python_float(f64::NAN).is_err());
    }
    #[test]
    fn unicode_and_composites() {
        assert_eq!(casefold("Straße Σς \u{1c89}"), "strasse σσ \u{1c8a}");
        let parts = ["雪😀", "a\"\\\n", "a:b"];
        let encoded = composite_artifact_id(&parts).unwrap();
        assert_eq!(parse_composite_artifact_id(&encoded, 3).unwrap(), parts);
        assert!(parse_composite_artifact_id(&encoded, 2).is_err());
        assert!(parse_composite_artifact_id("composite-v1-zz", 1).is_err());
        assert!(composite_artifact_id(&[""]).is_err());
        assert_eq!(
            canonical_json(&json!({"z":null,"a":"😀"}), JsonProfile::AsciiCompact).unwrap(),
            br#"{"a":"\ud83d\ude00","z":null}"#
        );
    }
}

#[cfg(test)]
mod unicode_release_tests {
    use super::*;
    #[test]
    fn unicode16_delta_and_full_expansions() {
        let delta: Value = ssx_json::from_str(include_str!(
            "../../../contracts/unicode/legacy-15-delta.json"
        ))
        .unwrap();
        let rows = delta["differences"].as_array().unwrap();
        assert_eq!(rows.len(), 27);
        for row in rows {
            assert_eq!(
                casefold(row["old"].as_str().unwrap()),
                row["new"].as_str().unwrap()
            );
        }
        assert_eq!(casefold("İﬃ𐐀"), "i\u{307}ffi𐐨");
    }
    #[test]
    fn arbitrary_integers_preserve_digits() {
        let raw = "[18446744073709551617,-9223372036854775809,-0,-0.0]";
        assert_eq!(
            String::from_utf8(canonical_json_text(raw, JsonProfile::AsciiCompact).unwrap())
                .unwrap(),
            "[18446744073709551617,-9223372036854775809,0,-0.0]"
        );
    }
}
#[cfg(test)]
mod json_object_tests {
    #[test]
    fn private_number_key_is_ordinary_payload() {
        let raw = r#"{"$serde_json::private::Number":"123"}"#;
        let parsed: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert!(
            parsed.is_object(),
            "ordinary user dictionary must remain an object: {parsed}"
        );
        assert_eq!(serde_json::to_string(&parsed).unwrap(), raw);
    }
}
