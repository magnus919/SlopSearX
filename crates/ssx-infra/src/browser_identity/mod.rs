//! Dedicated browser identity authority; never shares machine credentials.
mod oidc;
mod sessions;
use aws_lc_rs::{hmac, rand};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
pub use oidc::OidcSettings;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
pub use sessions::{BrowserAuthority, BrowserAuthorityConfig, BrowserKeys, StaticBrowserDirectory};
use ssx_domain::browser_identity::{BrowserError, BrowserSecret};
use std::fmt;
pub(crate) fn random() -> Result<BrowserSecret, BrowserError> {
    let mut b = [0; 32];
    rand::fill(&mut b).map_err(|_| BrowserError::Unavailable)?;
    BrowserSecret::new(URL_SAFE_NO_PAD.encode(b))
}
pub(crate) fn mac(key: &BrowserSecret, value: &[u8]) -> String {
    hex(hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA256, key.expose().as_bytes()),
        value,
    )
    .as_ref())
}
pub(crate) fn hex(b: &[u8]) -> String {
    b.iter().map(|v| format!("{v:02x}")).collect()
}
pub(crate) fn equals(a: &str, b: &str) -> bool {
    let key = hmac::Key::new(hmac::HMAC_SHA256, b"browser-constant-time-comparison");
    let tag = hmac::sign(&key, a.as_bytes());
    hmac::verify(&key, b.as_bytes(), tag.as_ref()).is_ok()
}
/// Reject decoded duplicate keys at every depth before serde typed projection.
pub(crate) fn strict_json(bytes: &[u8]) -> Result<Value, BrowserError> {
    if bytes.len() > 65_536 {
        return Err(BrowserError::Malformed);
    }
    struct Node(usize);
    impl<'de> DeserializeSeed<'de> for Node {
        type Value = Value;
        fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
            if self.0 > 32 {
                return Err(de::Error::custom("depth"));
            }
            d.deserialize_any(self)
        }
    }
    impl<'de> Visitor<'de> for Node {
        type Value = Value;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("bounded unambiguous JSON")
        }
        fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
            Ok(Value::Bool(v))
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
            Ok(v.into())
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
            Ok(v.into())
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
            serde_json::Number::from_f64(v)
                .map(Value::Number)
                .ok_or_else(|| E::custom("number"))
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
            Ok(v.into())
        }
        fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
            Ok(v.into())
        }
        fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
            Ok(Value::Null)
        }
        fn visit_none<E: de::Error>(self) -> Result<Value, E> {
            Ok(Value::Null)
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
            let mut v = Vec::new();
            while let Some(n) = a.next_element_seed(Node(self.0 + 1))? {
                if v.len() >= 1024 {
                    return Err(de::Error::custom("array"));
                }
                v.push(n)
            }
            Ok(Value::Array(v))
        }
        fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
            let mut v = serde_json::Map::new();
            while let Some(k) = a.next_key::<String>()? {
                if v.len() >= 256 || v.contains_key(&k) {
                    return Err(de::Error::custom("duplicate or oversized object"));
                }
                v.insert(k, a.next_value_seed(Node(self.0 + 1))?);
            }
            Ok(Value::Object(v))
        }
    }
    let mut d = serde_json::Deserializer::from_slice(bytes);
    let v = Node(0)
        .deserialize(&mut d)
        .map_err(|_| BrowserError::Malformed)?;
    d.end().map_err(|_| BrowserError::Malformed)?;
    Ok(v)
}
