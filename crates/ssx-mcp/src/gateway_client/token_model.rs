//! Source OAuthToken validation, distinct from SDK credential conversion.
use super::token_json::{self, JsonError};
use ssx_infra::adapters::source_json::{Map, Number, Text, Value};
use std::{fmt::Write, time::Instant};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelError {
    Validation,
    Resource,
    Deadline,
}
impl From<JsonError> for ModelError {
    fn from(value: JsonError) -> Self {
        match value {
            JsonError::Deadline => Self::Deadline,
            JsonError::Limit => Self::Resource,
            _ => Self::Validation,
        }
    }
}
/// Explicit export is needed for persistence; Debug never renders credentials.
pub struct TokenModel(Vec<u8>);
impl std::fmt::Debug for TokenModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TokenModel(<redacted>)")
    }
}
impl TokenModel {
    pub fn json(&self) -> &[u8] {
        &self.0
    }
}
pub(super) fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(n @ Number::Integer(_)) => n.integer_text() != Some("0"),
        Value::Number(Number::Float(v)) => *v != 0.0,
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}
struct IntegerScratch {
    bytes: [u8; 310],
    len: usize,
}
impl std::fmt::Write for IntegerScratch {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let end = self
            .len
            .checked_add(text.len())
            .filter(|n| *n <= self.bytes.len())
            .ok_or(std::fmt::Error)?;
        self.bytes[self.len..end].copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}
pub(super) async fn integer(value: &Value, deadline: Instant) -> Result<Value, ModelError> {
    let canonical = match value {
        Value::Number(Number::Integer(_)) => return Ok(value.clone()),
        Value::Bool(v) => {
            if *v {
                "1".into()
            } else {
                "0".into()
            }
        }
        Value::Number(Number::Float(v)) if v.is_finite() && v.fract() == 0.0 => {
            // Largest finite integral f64:309 decimal digits, plus one sign.
            let mut scratch = IntegerScratch {
                bytes: [0; 310],
                len: 0,
            };
            write!(&mut scratch, "{v:.0}").map_err(|_| ModelError::Resource)?;
            std::str::from_utf8(&scratch.bytes[..scratch.len])
                .map_err(|_| ModelError::Validation)?
                .to_owned()
        }
        Value::String(text) => {
            let scalar = text.as_str().ok_or(ModelError::Validation)?;
            // Existing pinned Unicode16 numeric normalization; this bounded
            // synchronous phase remains explicit, between deadline checks.
            let normalized = ssx_domain::storage::normalize_python_number_text(scalar)
                .map_err(|_| ModelError::Validation)?;
            let (sign, raw) = if let Some(raw) = normalized.strip_prefix('-') {
                ("-", raw)
            } else {
                ("", normalized.strip_prefix('+').unwrap_or(&normalized))
            };
            let digits = if let Some((whole, fraction)) = raw.split_once('.') {
                if fraction.is_empty() || !fraction.bytes().all(|b| b == b'0') {
                    return Err(ModelError::Validation);
                }
                whole
            } else {
                raw
            };
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(ModelError::Validation);
            }
            let digits = digits.trim_start_matches('0');
            if digits.is_empty() {
                "0".into()
            } else {
                let mut out = String::with_capacity(sign.len() + digits.len());
                out.push_str(sign);
                out.push_str(digits);
                out
            }
        }
        _ => return Err(ModelError::Validation),
    };
    if Instant::now() >= deadline {
        return Err(ModelError::Deadline);
    }
    let value = token_json::parse(canonical.as_bytes(), deadline).await?;
    match value {
        Value::Number(Number::Integer(_)) => Ok(value),
        _ => Err(ModelError::Validation),
    }
}
/// Input is the selected raw token slot, not the root document. Falsey slots
/// intentionally do not invoke model validation, exactly as source getters.
pub async fn validate(
    value: Option<&Value>,
    deadline: Instant,
) -> Result<Option<TokenModel>, ModelError> {
    if Instant::now() >= deadline {
        return Err(ModelError::Deadline);
    }
    let Some(value) = value.filter(|v| truthy(v)) else {
        return Ok(None);
    };
    let fields = value.as_object().ok_or(ModelError::Validation)?;
    let access = match fields.get("access_token") {
        Some(Value::String(v)) => Value::String(v.clone()),
        _ => return Err(ModelError::Validation),
    };
    let kind = match fields.get("token_type") {
        None => Value::String(Text::from("Bearer")),
        Some(Value::String(text))
            if text
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case("Bearer")) =>
        {
            Value::String(Text::from("Bearer"))
        }
        _ => return Err(ModelError::Validation),
    };
    let expires = match fields.get("expires_in") {
        None | Some(Value::Null) => Value::Null,
        Some(v) => integer(v, deadline).await?,
    };
    let optional = |name| match fields.get(name) {
        None | Some(Value::Null) => Ok(Value::Null),
        Some(Value::String(v)) => Ok(Value::String(v.clone())),
        _ => Err(ModelError::Validation),
    };
    let scope = optional("scope")?;
    let refresh = optional("refresh_token")?;
    let mut normalized = Map::new();
    for (key, value) in [
        ("access_token", access),
        ("token_type", kind),
        ("expires_in", expires),
        ("scope", scope),
        ("refresh_token", refresh),
    ] {
        normalized.insert(Text::from(key), value);
    }
    Ok(Some(TokenModel(
        token_json::encode(&Value::Object(normalized), deadline).await?,
    )))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn original_twenty_one_token_model_outcomes_preserve_surrogates_and_coercion() {
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        let fixture = token_json::parse(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/rust-rewrite/t07/token-model-source.json"
            )),
            deadline,
        )
        .await
        .unwrap();
        let cases = fixture.get("cases").unwrap().as_array().unwrap();
        assert_eq!(cases.len(), 21);
        let mut ids = std::collections::BTreeSet::new();
        for case in cases {
            let id = case.get("id").unwrap().as_str().unwrap();
            assert!(ids.insert(id));
            let hex = case.get("source_json_utf8_hex").unwrap().as_str().unwrap();
            let bytes: Vec<u8> = hex
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            let raw = token_json::parse(&bytes, deadline).await.unwrap();
            let result = validate(raw.get("tokens"), deadline).await;
            let expected = case.get("get_tokens").unwrap();
            if expected.get("error_type").is_some() {
                assert_eq!(
                    expected.get("error_type").unwrap().as_str(),
                    Some("ValidationError")
                );
                assert!(matches!(result, Err(ModelError::Validation)), "{id}");
            } else {
                let model = result.unwrap();
                let actual = match model {
                    None => Value::Null,
                    Some(model) => {
                        assert_eq!(format!("{model:?}"), "TokenModel(<redacted>)");
                        token_json::parse(model.json(), deadline).await.unwrap()
                    }
                };
                assert_eq!(&actual, expected.get("value").unwrap(), "{id}");
            }
            println!("T07_TOKEN_MODEL {id}");
        }
    }
}
