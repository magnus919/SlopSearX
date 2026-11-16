//! W07-only original MCP input compatibility. The shared F05 typed parser stays
//! strict; only this named input boundary accepts Python's equality profile.
use ssx_domain::{ArtifactRef, DomainError};
use ssx_json::Value;

pub fn artifact(value: &Value) -> Result<ArtifactRef, DomainError> {
    let shape = DomainError("artifact must contain exactly contract, version, kind, and id");
    let object = value.as_object().ok_or_else(|| shape.clone())?;
    if object.len() != 4
        || !["contract", "version", "kind", "id"]
            .iter()
            .all(|key| object.contains_key(*key))
    {
        return Err(shape);
    }
    let version = &object["version"];
    let one = version.as_bool() == Some(true)
        || version.as_i64() == Some(1)
        || version.as_u64() == Some(1)
        || version.as_f64() == Some(1.0);
    if object["contract"].as_str() != Some("slopsearx.artifact_ref") || !one {
        return Err(DomainError(
            "unsupported artifact reference contract or version",
        ));
    }
    let (Some(kind), Some(id)) = (object["kind"].as_str(), object["id"].as_str()) else {
        return Err(DomainError("artifact kind and id must be strings"));
    };
    let kind = ssx_json::from_value(Value::String(kind.into()))
        .map_err(|_| DomainError("unsupported artifact kind"))?;
    if id.is_empty() || id.chars().count() > 1024 {
        return Err(DomainError("artifact id must contain 1 to 1024 characters"));
    }
    if id.chars().any(|c| c.is_ascii_control()) {
        return Err(DomainError("artifact id contains control characters"));
    }
    ArtifactRef::new(kind, id)
}

/// Python bytes.fromhex allows ASCII whitespace only between complete bytes.
/// This does not normalize identities: the original external ID is retained;
/// only component lookup decodes the same byte sequence as the source helper.
pub fn composite(value: &str, count: usize) -> Result<Vec<String>, DomainError> {
    let invalid = DomainError("invalid composite artifact id");
    let raw = value
        .strip_prefix("composite-v1-")
        .ok_or_else(|| invalid.clone())?;
    let mut compact = String::with_capacity(value.len());
    compact.push_str("composite-v1-");
    let mut first = true;
    for byte in raw.bytes() {
        if matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) && first {
            continue;
        }
        if !byte.is_ascii_hexdigit() {
            return Err(invalid);
        }
        compact.push(char::from(byte));
        first = !first;
    }
    if !first {
        return Err(invalid);
    }
    ssx_domain::canonical::parse_composite_artifact_id(&compact, count).map_err(|_| invalid)
}
