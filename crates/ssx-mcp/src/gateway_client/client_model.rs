//! Source registration-response model; distinct from SDK StoredCredentials.
use super::{
    token_json,
    token_model::{self, ModelError},
};
use ssx_infra::adapters::source_json::{Map, Text, Value};
use std::time::Instant;
pub struct ClientModel(Vec<u8>);
impl std::fmt::Debug for ClientModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClientModel(<redacted>)")
    }
}
impl ClientModel {
    pub fn json(&self) -> &[u8] {
        &self.0
    }
}
fn member<'a>(fields: &'a Map, name: &str) -> Option<&'a Value> {
    fields
        .get(name)
        .filter(|v| !matches!(v, Value::Null) && !matches!(v,Value::String(t) if t.is_empty()))
}
fn string(value: Option<&Value>) -> Result<Value, ModelError> {
    match value {
        None => Ok(Value::Null),
        Some(Value::String(t)) => Ok(Value::String(t.clone())),
        _ => Err(ModelError::Validation),
    }
}
async fn strings(
    value: Option<&Value>,
    default: Value,
    deadline: Instant,
) -> Result<Value, ModelError> {
    match value {
        None => Ok(default),
        Some(Value::Array(values)) => {
            let mut out = Vec::new();
            out.try_reserve(values.len())
                .map_err(|_| ModelError::Resource)?;
            for (index, value) in values.iter().enumerate() {
                if index.is_multiple_of(128) {
                    tokio::task::yield_now().await;
                    if Instant::now() >= deadline {
                        return Err(ModelError::Deadline);
                    }
                }
                let Value::String(text) = value else {
                    return Err(ModelError::Validation);
                };
                out.push(Value::String(text.clone()));
            }
            Ok(Value::Array(out))
        }
        _ => Err(ModelError::Validation),
    }
}
pub(super) fn url_value(value: &Value, http_only: bool) -> Result<Value, ModelError> {
    let text = value.as_str().ok_or(ModelError::Validation)?;
    let parsed = url::Url::parse(text).map_err(|_| ModelError::Validation)?;
    if http_only && !matches!(parsed.scheme(), "http" | "https") {
        return Err(ModelError::Validation);
    }
    let mut normalized = parsed.as_str().to_owned();
    // Pinned model config url_preserve_empty_path=True. Preserve explicit '/'
    // (including normalized dot segments); remove only parser-inserted root.
    let raw = text.trim_matches(|c: char| c <= ' ');
    let after_scheme = raw.split_once(':').map(|(_, v)| v).unwrap_or("");
    let tail = after_scheme.strip_prefix("//").unwrap_or(after_scheme);
    let has_path = tail.split(['?', '#']).next().unwrap_or("").contains('/');
    if !parsed.cannot_be_a_base() && parsed.path() == "/" && !has_path {
        let end = normalized.find(['?', '#']).unwrap_or(normalized.len());
        if normalized.as_bytes().get(end.wrapping_sub(1)) == Some(&b'/') {
            normalized.remove(end - 1);
        }
    }
    Ok(Value::String(Text::from(normalized)))
}
fn default_strings(values: &[&str]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|v| Value::String(Text::from(*v)))
            .collect(),
    )
}
pub async fn validate(
    value: Option<&Value>,
    deadline: Instant,
) -> Result<Option<ClientModel>, ModelError> {
    if Instant::now() >= deadline {
        return Err(ModelError::Deadline);
    }
    let Some(value) = value.filter(|v| token_model::truthy(v)) else {
        return Ok(None);
    };
    let fields = value.as_object().ok_or(ModelError::Validation)?;
    let mut out = Map::new();
    out.insert(
        Text::from("response_types"),
        strings(
            member(fields, "response_types"),
            default_strings(&["code"]),
            deadline,
        )
        .await?,
    );
    for name in ["scope", "client_name"] {
        out.insert(Text::from(name), string(member(fields, name))?);
    }
    for name in ["client_uri", "logo_uri"] {
        out.insert(
            Text::from(name),
            match member(fields, name) {
                None => Value::Null,
                Some(v) => url_value(v, true)?,
            },
        );
    }
    out.insert(
        Text::from("contacts"),
        strings(member(fields, "contacts"), Value::Null, deadline).await?,
    );
    for name in ["tos_uri", "policy_uri", "jwks_uri"] {
        out.insert(
            Text::from(name),
            match member(fields, name) {
                None => Value::Null,
                Some(v) => url_value(v, true)?,
            },
        );
    }
    // Arbitrary jwks can include nonfinite/large/surrogate SourceJSON. Copy via
    // cooperative bounded codec instead of recursive Value::clone on a subtree.
    let jwks = match member(fields, "jwks") {
        None => Value::Null,
        Some(value) => {
            let encoded = token_json::encode(value, deadline).await?;
            token_json::parse(&encoded, deadline).await?
        }
    };
    out.insert(Text::from("jwks"), jwks);
    for name in ["software_id", "software_version"] {
        out.insert(Text::from(name), string(member(fields, name))?);
    }
    let redirects = match member(fields, "redirect_uris") {
        None => Value::Null,
        Some(Value::Array(values)) => {
            let mut normalized = Vec::new();
            normalized
                .try_reserve(values.len())
                .map_err(|_| ModelError::Resource)?;
            for (index, value) in values.iter().enumerate() {
                if index.is_multiple_of(128) {
                    tokio::task::yield_now().await;
                    if Instant::now() >= deadline {
                        return Err(ModelError::Deadline);
                    }
                }
                normalized.push(url_value(value, false)?);
            }
            Value::Array(normalized)
        }
        _ => return Err(ModelError::Validation),
    };
    out.insert(Text::from("redirect_uris"), redirects);
    out.insert(
        Text::from("token_endpoint_auth_method"),
        string(member(fields, "token_endpoint_auth_method"))?,
    );
    out.insert(
        Text::from("grant_types"),
        strings(
            member(fields, "grant_types"),
            default_strings(&["authorization_code", "refresh_token"]),
            deadline,
        )
        .await?,
    );
    out.insert(
        Text::from("application_type"),
        string(member(fields, "application_type"))?,
    );
    out.insert(
        Text::from("client_id"),
        match member(fields, "client_id") {
            Some(Value::String(t)) => Value::String(t.clone()),
            _ => return Err(ModelError::Validation),
        },
    );
    out.insert(
        Text::from("client_secret"),
        string(member(fields, "client_secret"))?,
    );
    for name in ["client_id_issued_at", "client_secret_expires_at"] {
        out.insert(
            Text::from(name),
            match member(fields, name) {
                None => Value::Null,
                Some(v) => token_model::integer(v, deadline).await?,
            },
        );
    }
    out.insert(Text::from("issuer"), string(member(fields, "issuer"))?);
    Ok(Some(ClientModel(
        token_json::encode(&Value::Object(out), deadline).await?,
    )))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn original_twenty_one_client_model_outcomes() {
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
            let (bytes, rest) = hex.as_bytes().as_chunks::<2>();
            assert!(rest.is_empty());
            let bytes: Vec<u8> = bytes
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            let raw = token_json::parse(&bytes, deadline).await.unwrap();
            let expected = case.get("get_client_info").unwrap();
            let result = validate(raw.get("client_info"), deadline).await;
            if expected.get("error_type").is_some() {
                assert_eq!(
                    expected.get("error_type").unwrap().as_str(),
                    Some("ValidationError")
                );
                assert!(matches!(result, Err(ModelError::Validation)), "{id}");
            } else {
                let actual = match result.unwrap() {
                    None => Value::Null,
                    Some(model) => {
                        assert_eq!(format!("{model:?}"), "ClientModel(<redacted>)");
                        token_json::parse(model.json(), deadline).await.unwrap()
                    }
                };
                assert_eq!(&actual, expected.get("value").unwrap(), "{id}");
            }
            println!("T07_CLIENT_MODEL {id}");
        }
    }
    #[tokio::test]
    async fn pinned_client_url_model_controls() {
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        let fixture = token_json::parse(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/rust-rewrite/t07/client-urls-source.json"
            )),
            deadline,
        )
        .await
        .unwrap();
        let cases = fixture.get("cases").unwrap().as_array().unwrap();
        assert_eq!(cases.len(), 24);
        for case in cases {
            let id = case.get("id").unwrap().as_str().unwrap();
            let result = validate(case.get("input"), deadline).await;
            if let Some(expected) = case.get("value") {
                let model = result.unwrap().unwrap();
                assert_eq!(
                    &token_json::parse(model.json(), deadline).await.unwrap(),
                    expected,
                    "{id}"
                );
            } else {
                assert!(matches!(result, Err(ModelError::Validation)), "{id}");
            }
            println!("T07_CLIENT_URL {id}");
        }
    }
}
