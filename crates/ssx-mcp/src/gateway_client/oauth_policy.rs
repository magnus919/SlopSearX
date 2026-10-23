//! Source MCP2.3 provider policy, independent of SDK convenience defaults.
use crate::Result;
use base64::Engine;
use serde_json::Value;
#[derive(Clone)]
pub struct Client {
    pub id: String,
    pub secret: Option<String>,
    pub method: Option<String>,
    pub issuer: Option<String>,
}
impl Client {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let value: Value = serde_json::from_slice(raw)?;
        Ok(Self {
            id: value["client_id"]
                .as_str()
                .ok_or("OAuth client ID missing")?
                .into(),
            secret: value["client_secret"].as_str().map(str::to_owned),
            method: value["token_endpoint_auth_method"]
                .as_str()
                .map(str::to_owned),
            issuer: value["issuer"].as_str().map(str::to_owned),
        })
    }
    pub fn usable_registration(&self) -> Result<()> {
        if !matches!(
            self.method.as_deref(),
            None | Some("none" | "client_secret_basic" | "client_secret_post")
        ) {
            return Err("OAuth minted client authentication method unsupported".into());
        }
        if matches!(
            self.method.as_deref(),
            Some("client_secret_basic" | "client_secret_post")
        ) && self.secret.is_none()
        {
            return Err("OAuth minted client secret absent".into());
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct Policy {
    pub client: Option<Client>,
    pub resource: String,
    pub expected_issuer: String,
    pub prm_scopes: Option<Vec<String>>,
    pub asm_scopes: Option<Vec<String>>,
    pub requested: Option<String>,
    pub refresh_fallback: bool,
    pub include_resource: bool,
    has_prm: bool,
}
fn origin_text(url: &str) -> Result<String> {
    let scheme = url.find("://").ok_or("OAuth absolute URL absent")?;
    let end = url[scheme + 3..]
        .find(['/', '?', '#'])
        .map_or(url.len(), |at| scheme + 3 + at);
    Ok(url[..end].to_owned())
}
pub fn canonical_resource(url: &str) -> Result<String> {
    let url = url.split('#').next().unwrap_or(url);
    let origin = origin_text(url)?;
    Ok(format!("{}{}", origin.to_lowercase(), &url[origin.len()..]))
}
pub fn issuer_matches(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (short, long) = if a.len() < b.len() { (a, b) } else { (b, a) };
    long == format!("{short}/") && origin_text(short).is_ok_and(|origin| origin == short)
}
fn allowed(requested: &str, parent: &str) -> bool {
    let Ok(a) = url::Url::parse(requested) else {
        return false;
    };
    let Ok(b) = url::Url::parse(parent) else {
        return false;
    };
    let same = origin_text(requested).ok().map(|v| v.to_lowercase())
        == origin_text(parent).ok().map(|v| v.to_lowercase());
    same && format!("{}/", a.path().trim_end_matches('/'))
        .starts_with(&format!("{}/", b.path().trim_end_matches('/')))
}
fn normalized_metadata_url(value: &str) -> Result<String> {
    use ssx_infra::adapters::source_json::{Text, Value as Source};
    let normalized = super::client_model::url_value(&Source::String(Text::from(value)), true)
        .map_err(|_| "OAuth metadata URL invalid")?;
    normalized
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "OAuth metadata URL invalid".into())
}
fn strings(value: &Value, key: &str) -> Option<Vec<String>> {
    value.get(key)?.as_array().map(|v| {
        v.iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    })
}
impl Policy {
    pub fn new(remote: &str) -> Result<Self> {
        Ok(Self {
            client: None,
            resource: canonical_resource(remote)?,
            expected_issuer: origin_text(remote)?,
            prm_scopes: None,
            asm_scopes: None,
            requested: None,
            refresh_fallback: false,
            include_resource: true,
            has_prm: false,
        })
    }
    pub fn metadata(&mut self, value: &Value) -> Result<()> {
        if let Some(servers) = value.get("authorization_servers").and_then(Value::as_array) {
            self.has_prm = true;
            self.include_resource = true;
            if let Some(issuer) = servers.first().and_then(Value::as_str) {
                self.expected_issuer = normalized_metadata_url(issuer)?
            };
            if let Some(resource) = value.get("resource").and_then(Value::as_str) {
                let resource = normalized_metadata_url(resource)?;
                if !allowed(&self.resource, &resource) {
                    return Err("OAuth protected resource mismatch".into());
                }
                self.resource = resource;
            }
            self.prm_scopes = strings(value, "scopes_supported");
        }
        if value.get("authorization_endpoint").is_some() {
            self.asm_scopes = strings(value, "scopes_supported");
        }
        Ok(())
    }
    pub fn protocol(&mut self, version: Option<&str>) {
        self.include_resource = self.has_prm || version.is_some_and(|v| v >= "2025-06-18");
    }
    pub fn select_scopes(
        &mut self,
        hint: Option<&str>,
        step_up: bool,
        granted: Option<&str>,
    ) -> Vec<String> {
        let selected = hint
            .map(str::to_owned)
            .or_else(|| self.prm_scopes.as_ref().map(|v| v.join(" ")))
            .or_else(|| self.asm_scopes.as_ref().map(|v| v.join(" ")));
        let mut selected = selected;
        if self
            .asm_scopes
            .as_ref()
            .is_some_and(|v| v.iter().any(|s| s == "offline_access"))
            && let Some(scope) = selected.as_mut()
            && !scope.split_whitespace().any(|s| s == "offline_access")
        {
            scope.push_str(" offline_access");
        }
        self.requested = if step_up {
            union(
                union(self.requested.as_deref(), granted).as_deref(),
                selected.as_deref(),
            )
        } else {
            selected
        };
        self.requested
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    }
    pub fn token_request(
        &self,
        body: &[u8],
        headers: &mut reqwest::header::HeaderMap,
    ) -> Result<Vec<u8>> {
        let mut form: Vec<(String, String)> = url::form_urlencoded::parse(body)
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .filter(|(k, _)| !matches!(k.as_str(), "client_id" | "client_secret" | "resource"))
            .collect();
        // Source refresh_data contains no scope (oauth2.py517–531), even when
        // SDK refresh convenience code added previously granted/offline scopes.
        if form
            .iter()
            .any(|(k, v)| k == "grant_type" && v == "refresh_token")
        {
            form.retain(|(k, _)| k != "scope");
        }
        let client = self.client.as_ref().ok_or("OAuth client unavailable")?;
        form.push(("client_id".into(), client.id.clone()));
        if self.include_resource {
            form.push(("resource".into(), self.resource.clone()));
        }
        headers.remove(reqwest::header::AUTHORIZATION);
        match client.method.as_deref() {
            Some("client_secret_basic")
                if client.secret.as_ref().is_some_and(|s| !s.is_empty()) =>
            {
                let secret = client.secret.as_deref().unwrap();
                let encoded = base64::engine::general_purpose::STANDARD.encode(format!(
                    "{}:{}",
                    quote(&client.id),
                    quote(secret)
                ));
                let mut header =
                    reqwest::header::HeaderValue::from_str(&format!("Basic {encoded}"))?;
                header.set_sensitive(true);
                headers.insert(reqwest::header::AUTHORIZATION, header);
            }
            Some("client_secret_post") if client.secret.as_ref().is_some_and(|s| !s.is_empty()) => {
                form.push(("client_secret".into(), client.secret.clone().unwrap()))
            }
            None
            | Some("none" | "private_key_jwt" | "client_secret_basic" | "client_secret_post") => {}
            _ => return Err("OAuth stored client authentication method unsupported".into()),
        }
        Ok(url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form)
            .finish()
            .into_bytes())
    }
}
fn quote(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char)
        } else {
            use std::fmt::Write;
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}
pub fn union(previous: Option<&str>, new: Option<&str>) -> Option<String> {
    match (
        previous.filter(|s| !s.is_empty()),
        new.filter(|s| !s.is_empty()),
    ) {
        (None, new) => new.map(str::to_owned),
        (Some(previous), None) => Some(previous.into()),
        (Some(previous), Some(new)) => {
            let mut values: Vec<_> = previous.split_whitespace().collect();
            for scope in new.split_whitespace() {
                if !values.contains(&scope) {
                    values.push(scope)
                }
            }
            Some(values.join(" "))
        }
    }
}

/// Source utils create_client_registration_request always falls back to the
/// resource-origin /register when metadata omits its registration endpoint.
/// Binding is established only by a published endpoint or same-origin AS fallback.
pub fn registration_target(
    resource: &str,
    issuer: &str,
    published: Option<&str>,
    discovered: bool,
) -> Result<(String, bool)> {
    let base = origin_text(resource)?;
    Ok((
        published
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{base}/register")),
        discovered && (published.is_some() || origin_text(issuer)? == base),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_issuer_root_equivalence_does_not_make_url_client_ids_portable() {
        assert!(issuer_matches("https://a.test", "https://a.test/"));
        assert!(!issuer_matches("https://a.test/x", "https://a.test/x/"));
        assert!(!issuer_matches("https://a.test", "https://b.test"));
    }
    #[test]
    fn source_resource_keeps_query_and_explicit_port_distinction() {
        assert_eq!(
            canonical_resource("HTTPS://EXAMPLE.TEST:443/mcp?q=X#f").unwrap(),
            "https://example.test:443/mcp?q=X"
        );
        assert!(!allowed(
            "https://example.test:443/mcp",
            "https://example.test/"
        ));
        assert!(allowed(
            "https://example.test/a/b",
            "https://example.test/a"
        ));
        assert!(!allowed(
            "https://example.test/ab",
            "https://example.test/a"
        ));
    }
    #[test]
    fn source_scope_priority_and_stepup_union_keep_granted_scope() {
        let mut p = Policy::new("https://a.test/mcp").unwrap();
        p.prm_scopes = Some(vec!["read".into()]);
        p.asm_scopes = Some(vec!["write".into(), "offline_access".into()]);
        assert_eq!(
            p.select_scopes(None, false, None),
            ["read", "offline_access"]
        );
        assert_eq!(
            p.select_scopes(Some("write"), true, Some("saved")),
            ["read", "offline_access", "saved", "write"]
        );
    }
    #[test]
    fn minted_method_controls_secret_and_basic_uses_percent20() {
        let mut p = Policy::new("https://a.test/mcp").unwrap();
        p.client = Some(Client {
            id: "client id".into(),
            secret: Some("secret space".into()),
            method: Some("client_secret_basic".into()),
            issuer: None,
        });
        let mut headers = Default::default();
        let body = p
            .token_request(
                b"grant_type=refresh_token&client_secret=wrong",
                &mut headers,
            )
            .unwrap();
        let basic = headers[reqwest::header::AUTHORIZATION]
            .to_str()
            .unwrap()
            .strip_prefix("Basic ")
            .unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(basic)
                .unwrap(),
            b"client%20id:secret%20space"
        );
        assert!(!String::from_utf8(body).unwrap().contains("client_secret"));
        p.client.as_mut().unwrap().method = Some("none".into());
        let _ = p
            .token_request(b"grant_type=refresh_token", &mut headers)
            .unwrap();
        assert!(!headers.contains_key(reqwest::header::AUTHORIZATION));
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    #[test]
    fn resource_parameter_depends_on_prm_or_protocol_and_interactive_url_resource_keeps_userinfo_query()
     {
        let mut p = Policy::new("http://User:Pass@EXAMPLE.test/mcp?q=X#f").unwrap();
        assert_eq!(p.resource, "http://user:pass@example.test/mcp?q=X");
        p.protocol(Some("2024-11-05"));
        assert!(!p.include_resource);
        p.protocol(Some("2025-06-18"));
        assert!(p.include_resource);
        p.has_prm = true;
        p.protocol(None);
        assert!(p.include_resource);
    }
}

#[cfg(test)]
mod review_controls {
    use super::*;
    #[test]
    fn refresh_form_omits_scope_without_changing_authorization_code_form() {
        let mut p = Policy::new("https://resource.test/mcp").unwrap();
        p.client = Some(Client {
            id: "id".into(),
            secret: None,
            method: Some("none".into()),
            issuer: None,
        });
        let mut headers = reqwest::header::HeaderMap::new();
        let refresh = p
            .token_request(
                b"grant_type=refresh_token&refresh_token=r&scope=read+offline_access",
                &mut headers,
            )
            .unwrap();
        assert!(!url::form_urlencoded::parse(&refresh).any(|(k, _)| k == "scope"));
        let code = p
            .token_request(b"grant_type=authorization_code&scope=read", &mut headers)
            .unwrap();
        assert!(url::form_urlencoded::parse(&code).any(|(k, v)| k == "scope" && v == "read"));
    }
    #[test]
    fn discovered_metadata_missing_registration_endpoint_uses_resource_origin_with_honest_binding()
    {
        assert_eq!(
            registration_target(
                "https://resource.test/mcp?x=1",
                "https://other.test/as",
                None,
                true
            )
            .unwrap(),
            ("https://resource.test/register".into(), false)
        );
        assert!(
            registration_target(
                "https://resource.test/mcp",
                "https://resource.test/as",
                None,
                true
            )
            .unwrap()
            .1
        );
        assert!(
            registration_target(
                "https://resource.test/mcp",
                "https://other.test/as",
                Some("https://other.test/dcr"),
                true
            )
            .unwrap()
            .1
        );
        assert!(
            !registration_target(
                "https://resource.test/mcp",
                "https://resource.test",
                None,
                false
            )
            .unwrap()
            .1
        );
    }
}
