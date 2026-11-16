//! Lossless, bounded JSON values for opaque domain and public payloads.
//!
//! No backend, transport or network dependencies. In particular this does not
//! enable serde_json's globally unified `arbitrary_precision` feature: its private
//! number map tag collides with otherwise ordinary user object keys.
pub mod float;
pub use float::{FloatBufferError, FloatError, FloatNormalizer, parse_json_f64};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::ops::{Index, IndexMut};
pub type Error = serde_json::Error;
pub type Map<K, V> = indexmap::IndexMap<K, V>;
pub const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_JSON_DEPTH: usize = 100;
#[derive(Debug, Clone)]
pub struct Number(String);
impl Number {
    pub fn new(raw: &str) -> Result<Self, Error> {
        if raw.len() > MAX_JSON_BYTES {
            return Err(error("JSON byte limit exceeded"));
        }
        let checked: Box<serde_json::value::RawValue> = serde_json::from_str(raw)?;
        let raw = checked.get();
        if !matches!(raw.as_bytes().first(), Some(b'-' | b'0'..=b'9')) {
            return Err(error("expected JSON number"));
        }
        if raw.contains(['.', 'e', 'E']) && !parse_json_f64(raw).is_ok_and(f64::is_finite) {
            return Err(error("JSON float must be finite f64"));
        }
        Ok(Self(if raw == "-0" { "0".into() } else { raw.into() }))
    }
    /// Already typed finite float; shared formatting, without token reparsing.
    pub fn from_f64(value: f64) -> Result<Self, canonical::FloatFormatError> {
        canonical::try_python_float(value).map(Self)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn is_f64(&self) -> bool {
        self.0.contains(['.', 'e', 'E'])
    }
    pub fn as_i64(&self) -> Option<i64> {
        if self.is_f64() {
            None
        } else {
            self.0.parse().ok()
        }
    }
    pub fn as_u64(&self) -> Option<u64> {
        if self.is_f64() {
            None
        } else {
            self.0.parse().ok()
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        parse_json_f64(&self.0).ok().filter(|v| v.is_finite())
    }
}
impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        if self.0 == other.0 {
            return true;
        }
        if self.is_f64() && other.is_f64() {
            self.as_f64()
                .zip(other.as_f64())
                .is_some_and(|(a, b)| a == b)
        } else {
            self.0 == other.0
        }
    }
}
impl std::fmt::Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for Number {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Box::<serde_json::value::RawValue>::deserialize(deserializer)?;
        Self::new(raw.get()).map_err(serde::de::Error::custom)
    }
}
impl Serialize for Number {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let raw = serde_json::value::RawValue::from_string(self.0.clone())
            .map_err(serde::ser::Error::custom)?;
        raw.serialize(s)
    }
}
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Object(Map<String, Value>),
}
fn error(message: &str) -> Error {
    <Error as serde::de::Error>::custom(message)
}
fn parse(raw: &str, depth: usize) -> Result<Value, Error> {
    if depth > MAX_JSON_DEPTH {
        return Err(error("JSON depth limit exceeded"));
    }
    let raw = raw.trim();
    Ok(match raw.as_bytes().first() {
        Some(b'{') => {
            let values: Map<String, Box<serde_json::value::RawValue>> = serde_json::from_str(raw)?;
            Value::Object(
                values
                    .into_iter()
                    .map(|(k, v)| Ok((k, parse(v.get(), depth + 1)?)))
                    .collect::<Result<_, Error>>()?,
            )
        }
        Some(b'[') => {
            let values: Vec<Box<serde_json::value::RawValue>> = serde_json::from_str(raw)?;
            Value::Array(
                values
                    .into_iter()
                    .map(|v| parse(v.get(), depth + 1))
                    .collect::<Result<_, _>>()?,
            )
        }
        Some(b'"') => Value::String(serde_json::from_str(raw)?),
        Some(b't' | b'f') => Value::Bool(serde_json::from_str(raw)?),
        Some(b'n') => {
            let _: () = serde_json::from_str(raw)?;
            Value::Null
        }
        _ => Value::Number(Number::new(raw)?),
    })
}
pub fn from_str<T: DeserializeOwned>(raw: &str) -> Result<T, Error> {
    if raw.len() > MAX_JSON_BYTES {
        return Err(error("JSON byte limit exceeded"));
    }
    let _: Value = serde_json::from_str(raw)?;
    serde_json::from_str(raw)
}
pub fn from_slice<T: DeserializeOwned>(raw: &[u8]) -> Result<T, Error> {
    if raw.len() > MAX_JSON_BYTES {
        return Err(error("JSON byte limit exceeded"));
    }
    let _: Value = serde_json::from_slice(raw)?;
    serde_json::from_slice(raw)
}
pub fn to_string<T: Serialize + ?Sized>(value: &T) -> Result<String, Error> {
    serde_json::to_string(value)
}
pub fn to_vec<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(value)
}
pub fn to_value<T: Serialize>(value: T) -> Result<Value, Error> {
    from_str(&to_string(&value)?)
}
pub fn from_value<T: DeserializeOwned>(value: Value) -> Result<T, Error> {
    from_str(&to_string(&value)?)
}
impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = Box::<serde_json::value::RawValue>::deserialize(d)?;
        if raw.get().len() > MAX_JSON_BYTES {
            return Err(serde::de::Error::custom("JSON byte limit exceeded"));
        }
        parse(raw.get(), 0).map_err(serde::de::Error::custom)
    }
}
impl Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => s.serialize_unit(),
            Self::Bool(v) => s.serialize_bool(*v),
            Self::Number(v) => v.serialize(s),
            Self::String(v) => s.serialize_str(v),
            Self::Array(v) => v.serialize(s),
            Self::Object(v) => v.serialize(s),
        }
    }
}
impl Value {
    pub fn as_object(&self) -> Option<&Map<String, Value>> {
        if let Self::Object(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_object_mut(&mut self) -> Option<&mut Map<String, Value>> {
        if let Self::Object(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        if let Self::Array(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        if let Self::Array(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        if let Self::Number(v) = self {
            v.as_i64()
        } else {
            None
        }
    }
    pub fn as_u64(&self) -> Option<u64> {
        if let Self::Number(v) = self {
            v.as_u64()
        } else {
            None
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        if let Self::Number(v) = self {
            v.as_f64()
        } else {
            None
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }
    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }
    pub fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }
    pub fn is_boolean(&self) -> bool {
        matches!(self, Self::Bool(_))
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_object()?.get(key)
    }
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.as_object_mut()?.get_mut(key)
    }
}
impl Index<&str> for Value {
    type Output = Value;
    fn index(&self, key: &str) -> &Value {
        self.get(key).unwrap_or(&Value::Null)
    }
}
impl IndexMut<&str> for Value {
    fn index_mut(&mut self, key: &str) -> &mut Value {
        if self.is_null() {
            *self = Value::Object(Map::new());
        }
        self.as_object_mut()
            .expect("object index")
            .entry(key.into())
            .or_default()
    }
}
impl Index<usize> for Value {
    type Output = Value;
    fn index(&self, index: usize) -> &Value {
        self.as_array()
            .and_then(|v| v.get(index))
            .unwrap_or(&Value::Null)
    }
}
impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&to_string(self).map_err(|_| std::fmt::Error)?)
    }
}
impl From<serde_json::Value> for Value {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(v) => Self::Bool(v),
            serde_json::Value::Number(v) => {
                Self::Number(Number::new(&v.to_string()).expect("valid number"))
            }
            serde_json::Value::String(v) => Self::String(v),
            serde_json::Value::Array(v) => Self::Array(v.into_iter().map(Into::into).collect()),
            serde_json::Value::Object(v) => {
                Self::Object(v.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
        }
    }
}
impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::String(v.into())
    }
}
impl PartialEq<i32> for Value {
    fn eq(&self, v: &i32) -> bool {
        self.as_i64() == Some(i64::from(*v))
    }
}
impl PartialEq<bool> for Value {
    fn eq(&self, v: &bool) -> bool {
        self.as_bool() == Some(*v)
    }
}
impl PartialEq<&str> for Value {
    fn eq(&self, v: &&str) -> bool {
        self.as_str() == Some(*v)
    }
}
mod macros;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn number_tags_and_large_integers_roundtrip() {
        let raw = r#"{"$serde_json::private::Number":"123","nested":[18446744073709551617,-9223372036854775809]}"#;
        let v: Value = from_str(raw).unwrap();
        assert!(v.is_object());
        assert_eq!(v["nested"][0].to_string(), "18446744073709551617");
        assert_eq!(to_string(&v).unwrap(), raw);
        let again: Value = from_str(&to_string(&v).unwrap()).unwrap();
        assert_eq!(again, v);
    }
    #[test]
    fn malformed_and_bounds_fail() {
        for raw in ["01", "NaN", "+1", "1.", "{", "true false"] {
            assert!(from_str::<Value>(raw).is_err(), "{raw}");
        }
        assert!(from_str::<Value>(&"[".repeat(102)).is_err());
        assert!(from_str::<Value>(&" ".repeat(MAX_JSON_BYTES + 1)).is_err());
        assert!(Number::new("true").is_err());
    }
    #[test]
    fn typed_roundtrip() {
        #[derive(Serialize, Deserialize)]
        struct Dto {
            payload: Value,
        }
        let dto: Dto = from_str(
            r#"{"payload":{"$serde_json::private::Number":"1","x":9999999999999999999999999999}}"#,
        )
        .unwrap();
        assert_eq!(dto.payload["x"].to_string(), "9999999999999999999999999999");
        let again: Dto = from_str(&to_string(&dto).unwrap()).unwrap();
        assert_eq!(again.payload, dto.payload);
    }
}

#[cfg(test)]
mod literal_tests {
    use super::*;
    #[test]
    fn embedded_lossless_value_never_rounds() {
        let value: Value = from_str("18446744073709551617").unwrap();
        let object = json!({"payload":value});
        assert_eq!(
            to_string(&object).unwrap(),
            "{\"payload\":18446744073709551617}"
        );
    }
    #[test]
    fn numeric_equality_is_reflexive() {
        let number = Number::new("1e99").unwrap();
        assert_eq!(number, number.clone());
        assert!(Number::new("1e999").is_err());
    }
}

pub mod canonical;
pub use canonical::{
    FloatFormatError, JsonProfile, canonical_json, canonical_json_text, python_float,
};
#[cfg(test)]
mod boundary_tests {
    use super::*;
    #[test]
    fn supported_boundaries_and_first_rejections() {
        let raw = format!("\"{}\"", "x".repeat(MAX_JSON_BYTES - 2));
        assert!(from_str::<Value>(&raw).is_ok());
        assert!(from_str::<Value>(&format!("{raw} ")).is_err());
        let nested = |depth| format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        assert!(from_str::<Value>(&nested(MAX_JSON_DEPTH)).is_ok());
        assert!(from_str::<Value>(&nested(MAX_JSON_DEPTH + 1)).is_err());
        assert!(Number::new("1.7976931348623157e308").is_ok());
        assert!(Number::new("1.7976931348623157e309").is_err());
    }
}
#[cfg(test)]
mod dependency_feature_tests {
    #[test]
    fn third_party_serde_json_object_behavior_is_unchanged() {
        let raw = r#"{"$serde_json::private::Number":"123"}"#;
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert!(
            value.is_object(),
            "global parser feature corrupted an ordinary dictionary"
        );
    }
}

#[cfg(test)]
mod object_order_tests {
    #[test]
    fn opaque_objects_retain_insertion_order_and_canonical_profiles_sort_explicitly() {
        let raw = r#"{"z":true,"a":null}"#;
        let value: super::Value = super::from_str(raw).unwrap();
        assert_eq!(super::to_string(&value).unwrap(), raw);
        assert_eq!(
            String::from_utf8(
                super::canonical_json(&value, super::JsonProfile::Utf8Compact).unwrap()
            )
            .unwrap(),
            r#"{"a":null,"z":true}"#
        );
    }
}

fn casefold_table() -> &'static std::collections::BTreeMap<String, String> {
    static TABLE: std::sync::OnceLock<std::collections::BTreeMap<String, String>> =
        std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../contracts/unicode/casefold-16.0.0.json"
        ))
        .expect("reviewed Unicode table")
    })
}
/// Borrow one Unicode16 full casefold mapping without allocating a lookup key.
/// None preserves the original scalar. Surrogates cannot be Rust chars.
/// Cold immutable-table initialization belongs to shared-process startup, not
/// caller output accounting; consumers must retain their own destination owner.
pub fn casefold_char(ch: char) -> Option<&'static str> {
    let mut encoded = [0; 4];
    casefold_table()
        .get(ch.encode_utf8(&mut encoded))
        .map(String::as_str)
}
pub fn casefold(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if let Some(folded) = casefold_char(ch) {
            out.push_str(folded)
        } else {
            out.push(ch)
        }
    }
    out
}
#[cfg(test)]
mod borrowed_casefold_tests {
    #[test]
    fn every_reviewed_mapping_matches_borrowed_and_existing_whole_string_paths() {
        let table: std::collections::BTreeMap<String, String> = serde_json::from_str(include_str!(
            "../../../contracts/unicode/casefold-16.0.0.json"
        ))
        .unwrap();
        assert!(!table.is_empty());
        for (key, value) in &table {
            let mut chars = key.chars();
            let ch = chars.next().unwrap();
            assert!(chars.next().is_none());
            assert_eq!(super::casefold_char(ch), Some(value.as_str()));
            assert_eq!(super::casefold(key), *value);
        }
        for ch in ['0', ' ', '🙂', '\u{d7ff}', '\u{e000}', '\u{10ffff}'] {
            assert_eq!(super::casefold_char(ch), None);
            assert_eq!(super::casefold(&ch.to_string()), ch.to_string());
        }
        assert!(char::from_u32(0xd800).is_none());
        assert!(char::from_u32(0xdfff).is_none());
    }
}

/// Python Unicode whitespace, including the four ASCII information separators.
pub fn python_whitespace(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '\u{1c}'..='\u{1f}')
}
/// Unicode 16 decimal digit category, matching Python regex \d.
pub fn decimal_digit(ch: char) -> bool {
    decimal_value(ch).is_some()
}
/// Unicode 16 decimal value, using the same reviewed F05 zero-codepoint table.
pub fn decimal_value(ch: char) -> Option<u8> {
    static ZEROES: std::sync::OnceLock<Vec<u32>> = std::sync::OnceLock::new();
    let zeroes = ZEROES.get_or_init(|| {
        let table: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/unicode/python-text-16.0.0.json"
        ))
        .expect("reviewed Unicode table");
        table["decimal_zeroes"]
            .as_array()
            .expect("decimal zeroes")
            .iter()
            .map(|v| v.as_u64().expect("codepoint") as u32)
            .collect()
    });
    zeroes.iter().find_map(|zero| {
        (ch as u32)
            .checked_sub(*zero)
            .filter(|v| *v < 10)
            .map(|v| v as u8)
    })
}

/// Frozen source Unicode16 word boundaries and streamed lowercase operations.
pub mod python_text;
