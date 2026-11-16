//! T04 machine OAuth values. No HTTP handlers, cookies, tenant or OIDC policy.
use crate::DomainError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Raw credentials are never serializable or included in Debug/error output.
/// ```compile_fail
/// use ssx_domain::oauth::Secret;
/// let secret = Secret::new("synthetic".into()).unwrap();
/// let _ = serde_json::to_string(&secret);
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);
impl Secret {
    pub fn new(value: String) -> Result<Self, DomainError> {
        if value.is_empty() || value.len() > 4096 {
            return Err(DomainError("invalid OAuth credential"));
        }
        Ok(Self(value))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn fingerprint(&self) -> String {
        fingerprint(self.0.as_bytes())
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}
pub fn fingerprint(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}

#[derive(Clone, PartialEq, Eq)]
pub struct ClientRegistration {
    pub client_id: String,
    pub client_secret: Option<Secret>,
    /// Lossless remaining MCP client model_dump fields. Endpoint schema checks
    /// belong to T05; null/empty metadata remains storable and round-trips.
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

impl std::fmt::Debug for ClientRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClientRegistration(<redacted>)")
    }
}
/// The endpoint validates PKCE, redirect, client credentials and requested scopes
/// before presenting this exact binding to the atomic state transition.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeGrant {
    pub client_id: String,
    pub scopes: Vec<String>,
    pub code_challenge: String,
    pub redirect_uri: String,
    pub redirect_uri_provided_explicitly: bool,
    pub resource: Option<String>,
    pub subject: Option<String>,
}
impl std::fmt::Debug for CodeGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CodeGrant(<redacted>)")
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenGrant {
    pub client_id: String,
    pub scopes: Vec<String>,
    pub resource: Option<String>,
    pub subject: Option<String>,
}
impl std::fmt::Debug for TokenGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TokenGrant(<redacted>)")
    }
}
impl From<&CodeGrant> for TokenGrant {
    fn from(v: &CodeGrant) -> Self {
        Self {
            client_id: v.client_id.clone(),
            scopes: v.scopes.clone(),
            resource: v.resource.clone(),
            subject: v.subject.clone(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedCode {
    pub code: Secret,
    pub grant: CodeGrant,
    pub expires_at: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenPair {
    pub access_token: Secret,
    pub refresh_token: Secret,
    pub grant: TokenGrant,
    pub expires_in: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedGrant {
    pub grant: TokenGrant,
    pub expires_at: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Access,
    Refresh,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OAuthTtls {
    pub client: u64,
    pub code: u64,
    pub access: u64,
    pub refresh: u64,
}
impl Default for OAuthTtls {
    fn default() -> Self {
        Self {
            client: 30 * 24 * 3600,
            code: 600,
            access: 3600,
            refresh: 30 * 24 * 3600,
        }
    }
}
impl OAuthTtls {
    pub fn validate(self) -> Result<Self, DomainError> {
        if [self.client, self.code, self.access, self.refresh]
            .iter()
            .any(|v| *v == 0 || *v > 31_536_000)
        {
            Err(DomainError("invalid OAuth TTL"))
        } else {
            Ok(self)
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denial {
    Missing,
    Expired,
    Binding,
    Scope,
    Conflict,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthError {
    Denied(Denial),
    InvalidInput,
    Unavailable,
    Closed,
    Malformed,
    CompletionUnknown,
}
/// OAuth-specific configuration projection. General MCP policy belongs to F09.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthSettings {
    pub issuer_url: String,
    pub resource_server_url: String,
    pub ttls: OAuthTtls,
}
impl OAuthSettings {
    pub fn new(issuer: &str, ttls: OAuthTtls) -> Result<Self, DomainError> {
        let issuer = issuer.trim_end_matches('/');
        let url = url::Url::parse(issuer).map_err(|_| DomainError("invalid OAuth issuer"))?;
        if !["http", "https"].contains(&url.scheme())
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(DomainError("invalid OAuth issuer"));
        }
        // Source strips raw trailing slashes before parsing the two URLs.
        // Preserve a slash introduced by dot-segment normalization; only the
        // originally empty authority path has the SDK's no-slash rendering.
        let after_scheme = issuer.split_once(':').map_or("", |(_, rest)| rest);
        let original_path = after_scheme
            .trim_start_matches(['/', '\\'])
            .contains(['/', '\\']);
        let normalized = if original_path {
            url.as_str()
        } else {
            url.as_str().trim_end_matches('/')
        };
        let resource = url::Url::parse(&format!("{issuer}/mcp"))
            .map_err(|_| DomainError("invalid OAuth issuer"))?;
        Ok(Self {
            issuer_url: normalized.into(),
            resource_server_url: resource.into(),
            ttls: ttls.validate()?,
        })
    }
}

/// Explicit composition choices, not CLI authentication precedence. The source
/// helper can wrap OAuth with static bearer (security.py:64-79); the CLI instead
/// selects OAuth over a configured static token (server.py:437-453). Both means
/// an explicitly requested helper wrapper, never automatic raw-config layering.
/// Trusted stdio and source auto-approval remain separate T05 contracts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthMode {
    Disabled,
    StaticBearer,
    OAuth(OAuthSettings),
    Both(OAuthSettings),
}
impl OAuthMode {
    /// The static flag explicitly requests an outer wrapper. A CLI caller must
    /// resolve source OAuth precedence before constructing this composition.
    pub fn configured(
        enabled: bool,
        issuer: &str,
        static_bearer_configured: bool,
        ttls: OAuthTtls,
    ) -> Result<Self, DomainError> {
        if !enabled {
            return Ok(if static_bearer_configured {
                Self::StaticBearer
            } else {
                Self::Disabled
            });
        }
        let settings = OAuthSettings::new(issuer, ttls)?;
        Ok(if static_bearer_configured {
            Self::Both(settings)
        } else {
            Self::OAuth(settings)
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_modes_and_issuer_have_source_contract() {
        assert_eq!(
            OAuthMode::configured(false, "", false, OAuthTtls::default()).unwrap(),
            OAuthMode::Disabled
        );
        assert_eq!(
            OAuthMode::configured(false, "", true, OAuthTtls::default()).unwrap(),
            OAuthMode::StaticBearer
        );
        assert!(OAuthMode::configured(true, "", false, OAuthTtls::default()).is_err());
        let mode = OAuthMode::configured(true, "https://example.com/", true, OAuthTtls::default())
            .unwrap();
        let OAuthMode::Both(settings) = mode else {
            panic!("both modes retained")
        };
        assert_eq!(settings.resource_server_url, "https://example.com/mcp");
        assert!(
            OAuthSettings::new(
                "https://private:credential@example.com",
                OAuthTtls::default()
            )
            .is_err()
        );
        assert!(
            OAuthTtls {
                access: 0,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
