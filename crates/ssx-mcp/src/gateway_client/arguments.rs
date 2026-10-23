//! Original generated Python signature semantics: only top-level annotations.
use crate::Result;
use ssx_json::{Number, Value};
fn integer(value: &Value) -> Result<Value> {
    let raw = match value {
        Value::Bool(v) => {
            if *v {
                "1".into()
            } else {
                "0".into()
            }
        }
        Value::Number(n) if !n.is_f64() => n.as_str().to_owned(),
        Value::Number(n) => {
            let f = n.as_f64().ok_or("Input should be a finite number")?;
            if f.fract() != 0.0 {
                return Err(
                    "Input should be a valid integer, got a number with a fractional part".into(),
                );
            }
            format!("{f:.0}")
        }
        Value::String(s) => {
            if s.len() > 4300 {
                return Err(
                    "Unable to parse input string as an integer, exceeded maximum size".into(),
                );
            }
            let s = s.trim_matches(ssx_json::python_whitespace);
            let (sign, digits) = s
                .strip_prefix('-')
                .map_or_else(|| ("", s.strip_prefix('+').unwrap_or(s)), |v| ("-", v));
            let digits = if let Some((integer, fraction)) = digits.split_once('.') {
                if fraction.is_empty() || !fraction.bytes().all(|b| b == b'0') {
                    return Err(
                        "Input should be a valid integer, unable to parse string as an integer"
                            .into(),
                    );
                }
                integer
            } else {
                digits
            };
            if digits.is_empty()
                || digits.starts_with('_')
                || digits.ends_with('_')
                || digits.contains("__")
                || !digits.bytes().all(|b| b.is_ascii_digit() || b == b'_')
            {
                return Err(
                    "Input should be a valid integer, unable to parse string as an integer".into(),
                );
            }
            let digits = digits.replace('_', "");
            let digits = digits.trim_start_matches('0');
            if digits.is_empty() {
                "0".into()
            } else {
                format!("{sign}{digits}")
            }
        }
        _ => return Err("Input should be a valid integer".into()),
    };
    Ok(Value::Number(Number::new(&raw)?))
}
fn number(value: &Value) -> Result<Value> {
    let value = match value {
        Value::Bool(v) => {
            if *v {
                1.0
            } else {
                0.0
            }
        }
        Value::Number(n) => n.as_f64().ok_or("Input should be a finite number")?,
        Value::String(s) => {
            let s = s.trim_matches(ssx_json::python_whitespace);
            if s.as_bytes().iter().enumerate().any(|(i, b)| {
                *b == b'_'
                    && (i == 0
                        || i + 1 == s.len()
                        || !s.as_bytes()[i - 1].is_ascii_digit()
                        || !s.as_bytes()[i + 1].is_ascii_digit())
            }) {
                return Err(
                    "Input should be a valid number, unable to parse string as a number".into(),
                );
            }
            let s = s.replace('_', "");
            s.parse::<f64>()
                .map_err(|_| "Input should be a valid number, unable to parse string as a number")?
        }
        _ => return Err("Input should be a valid number".into()),
    };
    Ok(Value::Number(Number::from_f64(value).map_err(
        |_| "gateway float cannot be represented on JSON wire",
    )?))
}
fn boolean(value: &Value) -> Result<Value> {
    match value {
        Value::Bool(_) => Ok(value.clone()),
        Value::Number(n) => match n.as_f64() {
            Some(0.0) => Ok(Value::Bool(false)),
            Some(1.0) => Ok(Value::Bool(true)),
            _ => Err("Input should be a valid boolean, unable to interpret input".into()),
        },
        Value::String(s) => match s.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" | "y" | "t" => Ok(Value::Bool(true)),
            "false" | "0" | "no" | "off" | "n" | "f" => Ok(Value::Bool(false)),
            _ => Err("Input should be a valid boolean, unable to interpret input".into()),
        },
        _ => Err("Input should be a valid boolean".into()),
    }
}
fn annotation(schema: &Value) -> Option<&str> {
    fn recognized(v: &str) -> bool {
        matches!(
            v,
            "string" | "integer" | "number" | "boolean" | "array" | "object"
        )
    }
    match schema.get("type") {
        Some(Value::String(v)) if recognized(v) => Some(v),
        Some(Value::Array(types)) => types
            .iter()
            .filter_map(Value::as_str)
            .find(|v| recognized(v)),
        _ => None,
    }
}
pub fn normalize(schema: &Value, arguments: &Value) -> Result<Value> {
    let arguments = arguments.as_object().ok_or("Arguments must be an object")?;
    let empty = ssx_json::Map::new();
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    for key in arguments.keys() {
        if !properties.contains_key(key) {
            return Err(format!("{key}: Unexpected keyword argument").into());
        }
    }
    let required = schema.get("required").and_then(Value::as_array);
    let mut output = ssx_json::Map::new();
    for (name, property) in properties {
        let Some(value) = arguments.get(name) else {
            if required.is_some_and(|items| items.iter().any(|v| v.as_str() == Some(name.as_str())))
            {
                return Err(format!("{name}: Missing required argument").into());
            }
            output.insert(
                name.clone(),
                property.get("default").cloned().unwrap_or(Value::Null),
            );
            continue;
        };
        let value = match annotation(property) {
            Some("string") if !matches!(value, Value::String(_)) => {
                return Err(format!("{name}: Input should be a valid string").into());
            }
            Some("integer") => integer(value).map_err(|e| format!("{name}: {e}"))?,
            Some("number") => number(value).map_err(|e| format!("{name}: {e}"))?,
            Some("boolean") => boolean(value).map_err(|e| format!("{name}: {e}"))?,
            Some("array") if !matches!(value, Value::Array(_)) => {
                return Err(format!("{name}: Input should be a valid list").into());
            }
            Some("object") if !matches!(value, Value::Object(_)) => {
                return Err(format!("{name}: Input should be a valid dictionary").into());
            }
            _ => value.clone(),
        };
        output.insert(name.clone(), value);
    }
    Ok(Value::Object(output))
}

/// Preserve nested opaque argument/schema/default JSON without projecting it
/// through the finite domain Value codec. Python's list/dict annotations are
/// shallow; their nested values are not validated by JSON Schema constraints.
pub fn normalize_raw(schema: &str, arguments: &str) -> Result<String> {
    use serde_json::value::RawValue;
    type Raw = ssx_json::Map<String, Box<RawValue>>;
    let schema: Raw = serde_json::from_str(schema)?;
    let arguments: Raw = serde_json::from_str(arguments)?;
    let properties: Raw = schema
        .get("properties")
        .filter(|v| v.get() != "null")
        .map(|v| serde_json::from_str(v.get()))
        .transpose()?
        .unwrap_or_default();
    let required: Vec<String> = schema
        .get("required")
        .filter(|v| v.get() != "null")
        .map(|v| serde_json::from_str(v.get()))
        .transpose()?
        .unwrap_or_default();
    let mut errors = Vec::new();
    if properties.keys().any(|name| {
        name.strip_prefix('.')
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
    }) {
        return Err("Missing required positional argument".into());
    }
    let mut output: Raw = ssx_json::Map::new();
    for key in arguments.keys() {
        if !properties.contains_key(key) {
            errors.push(format!("{key}: Unexpected keyword argument"));
        }
    }
    for (name, property) in properties {
        let property: Raw = serde_json::from_str(property.get())?;
        let Some(value) = arguments.get(&name) else {
            if required.contains(&name) {
                errors.push(format!("{name}: Missing required argument"));
            } else {
                output.insert(
                    name,
                    property
                        .get("default")
                        .cloned()
                        .unwrap_or(RawValue::from_string("null".into())?),
                );
            }
            continue;
        };
        let kind: Option<serde_json::Value> = property
            .get("type")
            .map(|v| serde_json::from_str(v.get()))
            .transpose()?;
        let types: Vec<&str> = match kind.as_ref() {
            Some(serde_json::Value::String(v)) => vec![v],
            Some(serde_json::Value::Array(v)) => {
                v.iter().filter_map(serde_json::Value::as_str).collect()
            }
            _ => vec![],
        };
        let kind = types.into_iter().find(|v| {
            matches!(
                *v,
                "string" | "integer" | "number" | "boolean" | "array" | "object"
            )
        });
        let prefix = value.get().as_bytes().first().copied();
        let normalized = match kind {
            Some("array") if prefix != Some(b'[') => Err("Input should be a valid list".into()),
            Some("object") if prefix != Some(b'{') => {
                Err("Input should be a valid dictionary".into())
            }
            Some("string") if prefix != Some(b'"') => Err("Input should be a valid string".into()),
            Some("integer" | "number" | "boolean") => {
                let primitive = if matches!(prefix, Some(b'[' | b'{')) {
                    Err("Input should have the declared primitive type".into())
                } else {
                    ssx_json::from_str::<Value>(value.get())
                        .map_err(|error| -> crate::Error { error.into() })
                };
                primitive
                    .and_then(|value| match kind {
                        Some("integer") => integer(&value),
                        Some("number") => number(&value),
                        _ => boolean(&value),
                    })
                    .and_then(|v| Ok(RawValue::from_string(ssx_json::to_string(&v)?)?))
            }
            _ => Ok(value.clone()),
        };
        match normalized {
            Ok(value) => {
                output.insert(name, value);
            }
            Err(error) => errors.push(format!("{name}: {error}")),
        }
    }
    if !errors.is_empty() {
        return Err(errors.join("\n").into());
    }
    let encoded = serde_json::to_string(&output)?;
    outgoing_utf8(&encoded)?;
    Ok(encoded)
}

pub fn validate_schema(raw: &str) -> Result<()> {
    use serde_json::value::RawValue;
    type Map = ssx_json::Map<String, Box<RawValue>>;
    let schema: Map = serde_json::from_str(raw)?;
    let properties: Map = schema
        .get("properties")
        .filter(|v| v.get() != "null")
        .map(|v| serde_json::from_str(v.get()))
        .transpose()?
        .unwrap_or_default();
    let mut keyword_seen = false;
    let mut seen = std::collections::BTreeSet::new();
    for (name, property) in properties {
        let implicit = name
            .strip_prefix('.')
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()));
        let identity = if implicit {
            format!("implicit{}", &name[1..])
        } else {
            name.clone()
        };
        if !seen.insert(identity) {
            return Err("duplicate parameter name".into());
        }
        if implicit {
            if keyword_seen {
                return Err(
                    "non-default positional parameter follows keyword-only parameter".into(),
                );
            }
        } else {
            keyword_seen = true;
            if !super::title::identifier(&name) {
                return Err("remote schema property is not a valid Python parameter name".into());
            }
        }
        let _: Map = serde_json::from_str(property.get())?;
    }
    Ok(())
}
/// HTTPX sends JSON with ensure_ascii=False then strict UTF8 encoding. An
/// unpaired escaped surrogate in normalized args fails before any HTTP dispatch.
fn outgoing_utf8(raw: &str) -> Result<()> {
    let b = raw.as_bytes();
    let mut i = 0;
    let mut quoted = false;
    while i < b.len() {
        if b[i] == b'"' {
            quoted = !quoted;
            i += 1;
            continue;
        }
        if quoted && b[i] == b'\\' {
            if b.get(i + 1) == Some(&b'u') {
                let code = u16::from_str_radix(&raw[i + 2..i + 6], 16)?;
                if (0xd800..=0xdbff).contains(&code) {
                    if b.get(i + 6..i + 8) != Some(b"\\u") {
                        return Err("tool arguments cannot be encoded as UTF8".into());
                    }
                    let low = u16::from_str_radix(
                        raw.get(i + 8..i + 12)
                            .ok_or("tool arguments cannot be encoded as UTF8")?,
                        16,
                    )?;
                    if !(0xdc00..=0xdfff).contains(&low) {
                        return Err("tool arguments cannot be encoded as UTF8".into());
                    }
                    i += 12;
                    continue;
                }
                if (0xdc00..=0xdfff).contains(&code) {
                    return Err("tool arguments cannot be encoded as UTF8".into());
                }
                i += 6;
                continue;
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssx_json::json;
    #[test]
    fn generated_signature_defaults_are_unvalidated_and_unknown_keywords_fail() {
        let schema = json!({"properties":{"n":{"type":"integer","default":"unvalidated"},"optional":{"type":"string"}}});
        assert_eq!(
            normalize(&schema, &json!({})).unwrap(),
            json!({"n":"unvalidated","optional":null})
        );
        assert!(normalize(&schema, &json!({"extra":1})).is_err());
        assert!(normalize(&schema, &json!({"optional":null})).is_err());
    }
    #[test]
    fn first_recognized_union_annotation_wins_and_nested_schema_is_discovery_only() {
        let schema = json!({"properties":{"items":{"type":["null","array"],"items":{"type":"string"}},"count":{"type":"integer"}},"required":["items","count"]});
        assert_eq!(
            normalize(
                &schema,
                &json!({"items":[1,{}],"count":"18_446_744_073_709_551_617.0"})
            )
            .unwrap(),
            ssx_json::from_str::<Value>(r#"{"items":[1,{}],"count":18446744073709551617}"#)
                .unwrap()
        );
        assert!(normalize(&schema, &json!({"items":null,"count":1})).is_err());
    }
    #[test]
    fn generated_lax_primitives_preserve_integer_width_and_number_float_type() {
        assert_eq!(integer(&json!(true)).unwrap(), json!(1));
        assert!(integer(&json!(1.5)).is_err());
        assert_eq!(boolean(&json!("OFF")).unwrap(), json!(false));
        let Value::Number(n) = number(&json!("2")).unwrap() else {
            panic!("float number")
        };
        assert_eq!(n.as_str(), "2.0");
        assert!(boolean(&json!(" true ")).is_err());
    }
}

#[cfg(test)]
mod raw_tests {
    use super::*;
    #[test]
    fn shallow_signature_keeps_deep_unknown_nested_arguments_and_defaults_raw() {
        let deep = format!("{}0{}", "[".repeat(200), "]".repeat(200));
        let schema = [
            "{\"properties\":{\"items\":{\"type\":\"array\",\"items\":{\"unknown\":",
            &deep,
            "}},\"default\":{\"default\":",
            &deep,
            "}}}",
        ]
        .concat();
        let args = format!("{{\"items\":{deep}}}");
        let normalized = normalize_raw(&schema, &args).unwrap();
        assert_eq!(normalized.matches(&deep).count(), 2);
    }
}

#[cfg(test)]
mod encoding_tests {
    use super::*;
    #[test]
    fn source_outgoing_utf8_rejects_unpaired_surrogates_but_keeps_pairs_and_literal_backslashes() {
        assert!(outgoing_utf8(r#"{"x":"\ud800"}"#).is_err());
        assert!(outgoing_utf8(r#"{"x":"\udc00"}"#).is_err());
        assert!(outgoing_utf8(r#"{"x":"\ud800\udc00"}"#).is_ok());
        assert!(outgoing_utf8(r#"{"x":"\\ud800"}"#).is_ok());
    }
}
