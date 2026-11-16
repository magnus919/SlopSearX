use super::{equals, strict_json};
use aws_lc_rs::signature;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::Value;
use ssx_domain::browser_identity::{BrowserError, BrowserSecret};
use url::Url;
#[derive(Debug, Clone)]
pub struct OidcSettings {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: BrowserSecret,
    pub callback_url: String,
}
impl OidcSettings {
    pub fn new(
        issuer: String,
        client_id: String,
        client_secret: BrowserSecret,
        callback_url: String,
    ) -> Result<Self, BrowserError> {
        fn valid(u: &Url) -> bool {
            u.scheme() == "https"
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
                && u.query().is_none()
                && u.fragment().is_none()
        }
        let i = Url::parse(&issuer).map_err(|_| BrowserError::InvalidInput)?;
        let c = Url::parse(&callback_url).map_err(|_| BrowserError::InvalidInput)?;
        if !valid(&i)
            || !valid(&c)
            || c.path() != "/auth/callback"
            || client_id.is_empty()
            || client_id.len() > 512
            || issuer.len() > 4096
            || callback_url.len() > 4096
        {
            return Err(BrowserError::InvalidInput);
        }
        Ok(Self {
            issuer: issuer.trim_end_matches('/').into(),
            client_id,
            client_secret,
            callback_url,
        })
    }
}
pub(crate) async fn bounded(response: reqwest::Response) -> Result<Value, BrowserError> {
    if !response.status().is_success() || response.content_length().is_some_and(|n| n > 65_536) {
        return Err(BrowserError::Denied);
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| BrowserError::Unavailable)?
    {
        if bytes.len() + chunk.len() > 65_536 {
            return Err(BrowserError::Malformed);
        }
        bytes.extend_from_slice(&chunk)
    }
    strict_json(&bytes)
}
pub(crate) async fn discover(
    client: &reqwest::Client,
    settings: &OidcSettings,
) -> Result<Value, BrowserError> {
    let m = bounded(
        client
            .get(format!(
                "{}/.well-known/openid-configuration",
                settings.issuer
            ))
            .send()
            .await
            .map_err(|_| BrowserError::Unavailable)?,
    )
    .await?;
    if m.get("issuer")
        .and_then(Value::as_str)
        .map(|s| s.trim_end_matches('/'))
        != Some(settings.issuer.as_str())
    {
        return Err(BrowserError::Denied);
    }
    for field in ["authorization_endpoint", "token_endpoint", "jwks_uri"] {
        let s = m
            .get(field)
            .and_then(Value::as_str)
            .ok_or(BrowserError::Malformed)?;
        let u = Url::parse(s).map_err(|_| BrowserError::Malformed)?;
        if s.len() > 4096
            || u.scheme() != "https"
            || u.host_str().is_none()
            || !u.username().is_empty()
            || u.password().is_some()
            || u.fragment().is_some()
        {
            return Err(BrowserError::Denied);
        }
    }
    if let Some(methods) = m.get("code_challenge_methods_supported")
        && !methods
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v == "S256"))
    {
        return Err(BrowserError::Denied);
    }
    Ok(m)
}
pub(crate) fn verify(
    token: &str,
    jwks: &Value,
    settings: &OidcSettings,
    nonce: &str,
    now_ms: u64,
) -> Result<(String, String), BrowserError> {
    if token.len() > 32_768 {
        return Err(BrowserError::Malformed);
    }
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(BrowserError::Malformed);
    }
    fn decode(s: &str, limit: usize) -> Result<Vec<u8>, BrowserError> {
        if s.len() > limit {
            return Err(BrowserError::Malformed);
        }
        URL_SAFE_NO_PAD
            .decode(s)
            .map_err(|_| BrowserError::Malformed)
    }
    let header = strict_json(&decode(parts[0], 2048)?)?;
    if header.get("alg").and_then(Value::as_str) != Some("RS256")
        || header.get("crit").is_some()
        || header.get("b64").is_some()
    {
        return Err(BrowserError::Denied);
    }
    let kid = header
        .get("kid")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 256)
        .ok_or(BrowserError::Denied)?;
    let keys = jwks
        .get("keys")
        .and_then(Value::as_array)
        .filter(|a| a.len() <= 32)
        .ok_or(BrowserError::Malformed)?;
    let matching: Vec<_> = keys
        .iter()
        .filter(|k| k.get("kid").and_then(Value::as_str) == Some(kid))
        .collect();
    if matching.len() != 1 {
        return Err(BrowserError::Denied);
    }
    let k = matching[0];
    if k.get("kty").and_then(Value::as_str) != Some("RSA")
        || k.get("use").is_some_and(|v| v != "sig")
        || k.get("alg").is_some_and(|v| v != "RS256")
        || k.get("key_ops").is_some_and(|v| {
            !v.as_array()
                .is_some_and(|a| !a.is_empty() && a.iter().all(|v| v == "verify"))
        })
    {
        return Err(BrowserError::Denied);
    }
    let n = decode(
        k.get("n")
            .and_then(Value::as_str)
            .ok_or(BrowserError::Malformed)?,
        1400,
    )?;
    let e = decode(
        k.get("e")
            .and_then(Value::as_str)
            .ok_or(BrowserError::Malformed)?,
        16,
    )?;
    let sig = decode(parts[2], 1400)?;
    signature::RsaPublicKeyComponents { n: &n, e: &e }
        .verify(
            &signature::RSA_PKCS1_2048_8192_SHA256,
            format!("{}.{}", parts[0], parts[1]).as_bytes(),
            &sig,
        )
        .map_err(|_| BrowserError::Denied)?;
    let claims = strict_json(&decode(parts[1], 24_000)?)?;
    let iss = claims
        .get("iss")
        .and_then(Value::as_str)
        .ok_or(BrowserError::Denied)?
        .trim_end_matches('/');
    let subject = claims
        .get("sub")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 512)
        .ok_or(BrowserError::Denied)?;
    let aud = claims.get("aud").ok_or(BrowserError::Denied)?;
    let audiences: Vec<&str> = match aud {
        Value::String(s) => vec![s],
        Value::Array(a) if a.len() <= 128 => a
            .iter()
            .map(|v| v.as_str().ok_or(BrowserError::Denied))
            .collect::<Result<_, _>>()?,
        _ => return Err(BrowserError::Denied),
    };
    if iss != settings.issuer
        || !audiences.contains(&settings.client_id.as_str())
        || (audiences.len() > 1
            && claims.get("azp").and_then(Value::as_str) != Some(&settings.client_id))
        || !equals(
            claims
                .get("nonce")
                .and_then(Value::as_str)
                .ok_or(BrowserError::Denied)?,
            nonce,
        )
    {
        return Err(BrowserError::Denied);
    }
    let time = |name: &str| -> Result<f64, BrowserError> {
        claims
            .get(name)
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite())
            .ok_or(BrowserError::Denied)
    };
    let exp = time("exp")?;
    let iat = time("iat")?;
    let nbf = if claims.get("nbf").is_some() {
        time("nbf")?
    } else {
        iat
    };
    let now = now_ms as f64 / 1000.;
    if now >= exp + 30. || now + 30. < nbf || iat > now + 30. {
        return Err(BrowserError::Denied);
    }
    Ok((iss.into(), subject.into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use aws_lc_rs::{
        rand::SystemRandom,
        rsa::KeySize,
        signature::{KeyPair as _, RSA_PKCS1_SHA256, RsaKeyPair},
    };
    use serde_json::json;
    fn setup() -> (RsaKeyPair, Value, OidcSettings) {
        let key = RsaKeyPair::generate(KeySize::Rsa2048).unwrap();
        let p = key.public_key();
        let jwks = json!({"keys":[{"kid":"test","kty":"RSA","n":URL_SAFE_NO_PAD.encode(p.modulus().big_endian_without_leading_zero()),"e":URL_SAFE_NO_PAD.encode(p.exponent().big_endian_without_leading_zero())}]});
        let settings = OidcSettings::new(
            "https://id.example".into(),
            "client".into(),
            BrowserSecret::new("synthetic".into()).unwrap(),
            "https://portal.example/auth/callback".into(),
        )
        .unwrap();
        (key, jwks, settings)
    }
    fn sign(k: &RsaKeyPair, h: &str, p: &str) -> String {
        let input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(h),
            URL_SAFE_NO_PAD.encode(p)
        );
        let mut sig = vec![0; k.public_modulus_len()];
        k.sign(
            &RSA_PKCS1_SHA256,
            &SystemRandom::new(),
            input.as_bytes(),
            &mut sig,
        )
        .unwrap();
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig))
    }
    fn claims() -> Value {
        json!({"iss":"https://id.example","sub":"subject","aud":"client","iat":1000,"exp":2000,"nonce":"nonce"})
    }
    #[test]
    fn authentic_claims_reject_finite_type_issuer_audience_nonce_and_time_boundaries() {
        let (k, j, s) = setup();
        let h = r#"{"alg":"RS256","kid":"test"}"#;
        assert!(
            verify(
                &sign(&k, h, &claims().to_string()),
                &j,
                &s,
                "nonce",
                1100000
            )
            .is_ok()
        );
        for (name, value) in [
            ("exp", json!("NaN")),
            ("iat", json!("NaN")),
            ("nbf", json!("Infinity")),
            ("iat", json!(true)),
            ("iat", json!("1000")),
            ("exp", json!(1000)),
            ("iat", json!(1131)),
            ("nbf", json!(1131)),
            ("iss", json!("https://attacker")),
            ("sub", json!("")),
            ("nonce", json!("attacker")),
            ("aud", json!(["client", false])),
            ("aud", json!(["client", "other"])),
        ] {
            let mut c = claims();
            c[name] = value;
            assert_eq!(
                verify(&sign(&k, h, &c.to_string()), &j, &s, "nonce", 1100000),
                Err(BrowserError::Denied),
                "{name}"
            );
        }
        let mut c = claims();
        c["aud"] = json!(["client", "other"]);
        c["azp"] = json!("client");
        assert!(verify(&sign(&k, h, &c.to_string()), &j, &s, "nonce", 1100000).is_ok());
        c["azp"] = json!("wrong");
        assert_eq!(
            verify(&sign(&k, h, &c.to_string()), &j, &s, "nonce", 1100000),
            Err(BrowserError::Denied)
        );
        // Source permits an irrelevant azp on a single audience; retain that exact rule.
        c["aud"] = json!("client");
        assert!(verify(&sign(&k, h, &c.to_string()), &j, &s, "nonce", 1100000).is_ok());
        assert!(
            verify(
                &sign(&k, h, &claims().to_string()),
                &j,
                &s,
                "nonce",
                2029999
            )
            .is_ok()
        );
        assert_eq!(
            verify(
                &sign(&k, h, &claims().to_string()),
                &j,
                &s,
                "nonce",
                2030000
            ),
            Err(BrowserError::Denied)
        );
    }
    #[test]
    fn forged_ambiguous_unsupported_or_oversized_jwt_and_jwk_are_denied() {
        let (k, j, s) = setup();
        let c = claims().to_string();
        let h = r#"{"alg":"RS256","kid":"test"}"#;
        let valid = sign(&k, h, &c);
        let mut signature = valid.clone();
        signature.push('A');
        assert_eq!(
            verify(&signature, &j, &s, "nonce", 1100000),
            Err(BrowserError::Denied)
        );
        for bad in ["", "a.b", "a.b.c.d", "!.a.b"] {
            assert_eq!(
                verify(bad, &j, &s, "nonce", 1100000),
                Err(BrowserError::Malformed)
            );
        }
        for header in [
            r#"{"alg":"none","kid":"test"}"#,
            r#"{"alg":"HS256","kid":"test"}"#,
            r#"{"alg":"RS256","kid":"test","crit":["x"]}"#,
            r#"{"alg":"RS256","kid":"test","b64":false}"#,
        ] {
            assert_eq!(
                verify(&sign(&k, header, &c), &j, &s, "nonce", 1100000),
                Err(BrowserError::Denied)
            );
        }
        let duplicate = r#"{"iss":"https://id.example","sub":"subject","aud":"client","iat":1000,"exp":2000,"nonce":"nonce","\u006eonce":"attacker"}"#;
        assert_eq!(
            verify(&sign(&k, h, duplicate), &j, &s, "nonce", 1100000),
            Err(BrowserError::Malformed)
        );
        for (field, value) in [
            ("use", json!("enc")),
            ("alg", json!("PS256")),
            ("key_ops", json!(["sign"])),
            ("key_ops", json!([])),
        ] {
            let mut bad = j.clone();
            bad["keys"][0][field] = value;
            assert_eq!(
                verify(&valid, &bad, &s, "nonce", 1100000),
                Err(BrowserError::Denied)
            );
        }
        let duplicate_keys = json!({"keys":[j["keys"][0],j["keys"][0]]});
        assert_eq!(
            verify(&valid, &duplicate_keys, &s, "nonce", 1100000),
            Err(BrowserError::Denied)
        );
        let mut oversized = j.clone();
        oversized["keys"][0]["n"] = json!("A".repeat(1401));
        assert_eq!(
            verify(&valid, &oversized, &s, "nonce", 1100000),
            Err(BrowserError::Malformed)
        );
        assert_eq!(
            verify(&"A".repeat(32769), &j, &s, "nonce", 1100000),
            Err(BrowserError::Malformed)
        );
    }
    #[test]
    fn bounded_json_rejects_nested_escaped_duplicates_depth_and_oversize() {
        assert!(strict_json(br#"{"a":{"x":1,"\u0078":2}}"#).is_err());
        assert!(strict_json(format!("{}0{}", "[".repeat(34), "]".repeat(34)).as_bytes()).is_err());
        assert!(strict_json(&vec![b' '; 65537]).is_err());
        assert!(strict_json(br#"{"a":1} trailing"#).is_err());
    }
}
#[cfg(test)]
mod source_replay {
    use super::*;
    use aws_lc_rs::{
        rand::SystemRandom,
        rsa::KeySize,
        signature::{KeyPair as _, RSA_PKCS1_SHA256, RsaKeyPair},
    };
    use serde_json::Value;
    #[test]
    fn exact_source_claim_signature_settings_and_return_corpus_with_named_hardening() {
        let v: Value =
            serde_json::from_str(include_str!("../../../xtask/fixtures/t09/source.json")).unwrap();
        let key = RsaKeyPair::generate(KeySize::Rsa2048).unwrap();
        let p = key.public_key();
        let jwks = serde_json::json!({"keys":[{"kty":"RSA","kid":"test","n":URL_SAFE_NO_PAD.encode(p.modulus().big_endian_without_leading_zero()),"e":URL_SAFE_NO_PAD.encode(p.exponent().big_endian_without_leading_zero())}]});
        let settings = OidcSettings::new(
            "https://id.example".into(),
            "client".into(),
            BrowserSecret::new("synthetic".into()).unwrap(),
            "https://portal.example/auth/callback".into(),
        )
        .unwrap();
        let stricter = [
            "mixed_audience",
            "nan_exp",
            "nan_iat",
            "nan_nbf",
            "infinite_exp",
            "bool_iat",
            "numeric_string_iat",
            "numeric_string_exp",
        ];
        let mut seen = std::collections::BTreeSet::new();
        for c in v["claims"].as_array().unwrap() {
            let id = c["id"].as_str().unwrap();
            let input = format!(
                "{}.{}",
                URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","kid":"test"}"#),
                URL_SAFE_NO_PAD.encode(c["claims"].to_string())
            );
            let mut signature = vec![0; key.public_modulus_len()];
            key.sign(
                &RSA_PKCS1_SHA256,
                &SystemRandom::new(),
                input.as_bytes(),
                &mut signature,
            )
            .unwrap();
            let token = format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature));
            let actual = verify(
                &token,
                &jwks,
                &settings,
                "nonce",
                (c["now"].as_f64().unwrap() * 1000.).round() as u64,
            );
            let expected = if stricter.contains(&id) {
                seen.insert(id);
                assert_eq!(c["source"]["accepted"], true);
                false
            } else {
                c["source"]["accepted"].as_bool().unwrap()
            };
            assert_eq!(actual.is_ok(), expected, "{id}");
            if expected {
                let (issuer, subject) = actual.unwrap();
                assert_eq!(
                    serde_json::json!([issuer, subject]),
                    c["source"]["identity"]
                );
            }
        }
        assert_eq!(seen.len(), stricter.len());
        for c in v["signatures"].as_array().unwrap() {
            assert_eq!(
                verify(
                    c["token"].as_str().unwrap(),
                    &c["jwks"],
                    &settings,
                    "nonce",
                    1100000
                )
                .is_ok(),
                c["source"]["accepted"].as_bool().unwrap()
            );
        }
        for c in v["settings"].as_array().unwrap() {
            let id = c["id"].as_str().unwrap();
            let got = OidcSettings::new(
                c["issuer"].as_str().unwrap().into(),
                "client".into(),
                BrowserSecret::new("synthetic".into()).unwrap(),
                c["callback"].as_str().unwrap().into(),
            );
            let expected = if id == "issuer_userinfo" {
                assert_eq!(c["source"]["accepted"], true);
                false
            } else {
                c["source"]["accepted"].as_bool().unwrap()
            };
            assert_eq!(got.is_ok(), expected, "{id}");
        }
        let mut hardened = 0;
        for c in v["return_paths"].as_array().unwrap() {
            let input = c["input"].as_str();
            let native = ssx_domain::browser_identity::browser_return_to(input);
            if matches!(
                input,
                Some("/workflows/a%5cb" | "/workflows/%252f" | "/workflows/../search")
            ) {
                assert_eq!(native, "/workflows");
                assert_ne!(c["source"], native);
                hardened += 1;
            } else {
                assert_eq!(c["source"], native)
            }
        }
        assert_eq!(hardened, 3);
    }
}
