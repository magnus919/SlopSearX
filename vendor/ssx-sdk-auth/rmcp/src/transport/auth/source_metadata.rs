//! Opt-in source metadata documents and buffered HTTP projection.
//!
//! Raw endpoints are immutable text, not normalized URIs. Model endpoints are
//! validated with the source model's distinct URL primitive. Default SDK auth
//! discovery does not use this module or its candidate/state policy.
use super::{
    MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES, OAuthHttpClient, OAuthHttpClientError,
    OAuthHttpRedirectPolicy,
};
use jiter::{JsonObject, JsonValue, PartialMode};
use reqwest::Url;
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

/// Explicit discovery purpose; never inferred from a URL or response keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMetadataKind {
    /// Protected resource metadata, with a required nonempty server list.
    Prm,
    /// Authorization server metadata, with required issuer and endpoints.
    Asm,
}
/// Model-invalid candidate. Detailed validator diagnostics do not escape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidMetadata;
impl std::fmt::Display for InvalidMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid source metadata document")
    }
}
impl std::error::Error for InvalidMetadata {}
/// Default source operation refusal; default SDK auth does not adopt this lane.
#[derive(Clone, Copy, Debug)]
pub struct SourceOperationUnsupported;
impl std::fmt::Display for SourceOperationUnsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("source OAuth operation requires an injected adapter")
    }
}
impl std::error::Error for SourceOperationUnsupported {}
/// Future for model validation; None means model-invalid, never IO failure.
pub type SourceMetadataFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<Option<SourceMetadataProjection>, OAuthHttpClientError>>
            + Send
            + 'a,
    >,
>;
/// Where endpoint text entered the source operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceEndpointOrigin {
    /// Exact advertised challenge text, not resolved through a default Url.
    Advertised,
    /// Source-owned discovery/fallback derivation, supplied by the caller.
    Derived,
    /// Serialization of an actually validated model URL.
    Model,
}
/// Immutable SDK endpoint text. The gateway owns the one raw SourceUrl parse.
#[derive(Clone)]
pub struct SourceEndpoint {
    text: Arc<str>,
    origin: SourceEndpointOrigin,
}
impl std::fmt::Debug for SourceEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceEndpoint")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}
impl SourceEndpoint {
    /// Retain exact raw challenge text without trimming, joining or URI parsing.
    pub fn advertised(text: Arc<str>) -> Self {
        Self {
            text,
            origin: SourceEndpointOrigin::Advertised,
        }
    }
    /// Retain an already source-derived fallback/discovery URL without reparsing.
    pub fn derived(text: Arc<str>) -> Self {
        Self {
            text,
            origin: SourceEndpointOrigin::Derived,
        }
    }
    /// Borrow the same immutable text allocation carried by this endpoint.
    pub fn text(&self) -> &Arc<str> {
        &self.text
    }
    /// Return the actual construction boundary, not guessed document identity.
    pub fn origin(&self) -> SourceEndpointOrigin {
        self.origin
    }
}
/// A source model validation failure, distinct from transport, deadline,
/// resource, expiry-conversion and persistence failures. Only refresh may
/// consume this marker as source reauthorization fallback.
#[derive(Debug)]
pub struct SourceOAuthModelInvalid;
impl std::fmt::Display for SourceOAuthModelInvalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid source OAuth response model")
    }
}
impl std::error::Error for SourceOAuthModelInvalid {}
/// Exact source request purpose; adapters select the existing model codec by
/// this value, never by guessing body keys, method, or endpoint spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceOAuthPurpose {
    /// PRM/ASM GET, whose whole-document model validation follows transport.
    Discovery,
    /// Dynamic registration; discard wire issuer before client-model validation.
    Registration,
    /// Authorization-code token exchange, accepting source HTTP 200 or 201.
    CodeExchange,
    /// Refresh token exchange, accepting source HTTP 200 only. The project
    /// model adapter carries omitted/null scope from the prior raw token before
    /// returning the SDK-compatible DTO; missing refresh_token is retained here.
    Refresh,
}
/// Missing-ASM endpoint to derive using the source's raw resource-origin helper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceOAuthEndpointKind {
    /// Source origin joined with /authorize.
    Authorization,
    /// Source origin joined with /token.
    Token,
    /// Source origin joined with /register, including absent ASM registration.
    Registration,
}
/// Input to the finite injected raw endpoint derivation operation.
#[derive(Clone)]
pub struct SourceOAuthEndpointRequest {
    /// Original immutable resource base, not a normalized authorization URL.
    pub base: Arc<str>,
    /// Exact fallback purpose, selected before transport.
    pub kind: SourceOAuthEndpointKind,
}
impl std::fmt::Debug for SourceOAuthEndpointRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceOAuthEndpointRequest")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}
/// Result of deriving a missing endpoint; no default URI is constructed.
pub type SourceOAuthEndpointFuture<'a> =
    Pin<Box<dyn Future<Output = Result<SourceEndpoint, OAuthHttpClientError>> + Send + 'a>>;
/// Buffered source operation, with no mandatory typed URI or placeholder URL.
/// Body replay is explicit: clones retain the same endpoint and buffered bytes.
#[derive(Clone)]
pub struct SourceOAuthHttpRequest {
    /// Explicit codec and status-policy purpose.
    pub purpose: SourceOAuthPurpose,
    /// Actual method selected by source preparation.
    pub method: oauth2::http::Method,
    /// Original raw or validated-model endpoint owner.
    pub endpoint: SourceEndpoint,
    /// Prepared headers, including explicit authentication policy.
    pub headers: oauth2::http::HeaderMap,
    /// Already buffered bytes; no arbitrary stream is read to make a replay.
    pub body: Vec<u8>,
    /// Adapter-owned source redirect contract.
    pub redirect_policy: OAuthHttpRedirectPolicy,
    /// Suggested budget; adapters retain their explicit transport policy.
    pub timeout: Option<Duration>,
}
impl std::fmt::Debug for SourceOAuthHttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceOAuthHttpRequest")
            .field("purpose", &self.purpose)
            .field("method", &self.method)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}
impl SourceOAuthHttpRequest {
    /// Build an explicit source discovery GET without constructing an http::Uri.
    pub fn discovery(endpoint: SourceEndpoint) -> Self {
        Self {
            purpose: SourceOAuthPurpose::Discovery,
            method: oauth2::http::Method::GET,
            endpoint,
            headers: Default::default(),
            body: Vec::new(),
            redirect_policy: OAuthHttpRedirectPolicy::SourceWithinOrigin,
            timeout: Some(super::DEFAULT_HTTP_TIMEOUT),
        }
    }
}
/// Pydantic AnyHttpUrl projection, deliberately distinct from raw HTTPX SourceUrl.
#[derive(Clone, PartialEq, Eq)]
pub struct ModelHttpUrl {
    parsed: Url,
    serialized: Arc<str>,
}
impl std::fmt::Debug for ModelHttpUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ModelHttpUrl(<validated>)")
    }
}
impl ModelHttpUrl {
    /// Validate an actual AnyHttpUrl input with the same model URL primitive.
    /// This also supports the source's legacy origin TypeAdapter boundary after
    /// raw urlparse derives scheme://netloc; it is not raw HTTPX URL parsing.
    /// No synthetic metadata document or placeholder endpoint is constructed.
    pub fn parse(text: &str) -> Result<Self, InvalidMetadata> {
        let parsed = Url::options().parse(text).map_err(|_| InvalidMetadata)?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(InvalidMetadata);
        }
        // Core2.46.4 url.rs709–742: inspect original text only to determine
        // whether the special URL parser inserted its sole slash. Comparison
        // and request truth otherwise use the actual parsed serialization.
        let preserve = parsed.path() == "/"
            && text.split_once(':').is_some_and(|(_, rest)| {
                !matches!(
                    rest.trim_start_matches('/')
                        .chars()
                        .find(|c| matches!(c, '/' | '?' | '#')),
                    Some('/')
                )
            });
        let serialized: Arc<str> = if preserve {
            format!(
                "{}{}",
                &parsed[..url::Position::BeforePath],
                &parsed[url::Position::AfterPath..]
            )
            .into()
        } else {
            parsed.as_str().into()
        };
        Ok(Self { parsed, serialized })
    }
    /// Exact normalized model serialization, including preserved empty path.
    pub fn as_str(&self) -> &str {
        &self.serialized
    }
    /// The native URL primitive; its inserted slash is not model serialization.
    pub fn parsed(&self) -> &Url {
        &self.parsed
    }
    /// Preserve this validated serialization as an immutable source endpoint.
    pub fn endpoint(&self) -> SourceEndpoint {
        SourceEndpoint {
            text: self.serialized.clone(),
            origin: SourceEndpointOrigin::Model,
        }
    }
}
/// Whole valid PRM projection. Private fields prevent an unvalidated document.
#[derive(Clone, Debug)]
pub struct ValidatedPrm {
    resource: ModelHttpUrl,
    servers: Vec<ModelHttpUrl>,
    scopes: Option<Vec<String>>,
    bearer_methods: Option<Vec<String>>,
}
impl ValidatedPrm {
    /// Validated resource; semantic matching still precedes state commit.
    pub fn resource(&self) -> &ModelHttpUrl {
        &self.resource
    }
    /// Every member was validated; this list is nonempty and source order is retained.
    pub fn authorization_servers(&self) -> &[ModelHttpUrl] {
        &self.servers
    }
    /// None permits scope fallback; Some(empty) deliberately suppresses it.
    pub fn scopes(&self) -> Option<&[String]> {
        self.scopes.as_deref()
    }
    /// Missing defaults to header; explicit null remains None.
    pub fn bearer_methods(&self) -> Option<&[String]> {
        self.bearer_methods.as_deref()
    }
}
/// Whole valid ASM projection; unused declared fields were also validated.
#[derive(Clone, Debug)]
pub struct ValidatedAsm {
    issuer: ModelHttpUrl,
    authorization: ModelHttpUrl,
    token: ModelHttpUrl,
    registration: Option<ModelHttpUrl>,
    scopes: Option<Vec<String>>,
    response_types: Vec<String>,
    client_id_document: Option<bool>,
    response_iss: Option<bool>,
}
impl ValidatedAsm {
    /// Source-normalized issuer for exact semantic comparison.
    pub fn issuer(&self) -> &ModelHttpUrl {
        &self.issuer
    }
    /// Actual validated authorization endpoint.
    pub fn authorization_endpoint(&self) -> &ModelHttpUrl {
        &self.authorization
    }
    /// Actual validated token endpoint.
    pub fn token_endpoint(&self) -> &ModelHttpUrl {
        &self.token
    }
    /// Actual optional registration endpoint; no empty-string-to-None coercion.
    pub fn registration_endpoint(&self) -> Option<&ModelHttpUrl> {
        self.registration.as_ref()
    }
    /// None permits scope fallback; empty remains an accepted empty selection.
    pub fn scopes(&self) -> Option<&[String]> {
        self.scopes.as_deref()
    }
    /// Missing defaults to code; an explicit empty array remains valid.
    pub fn response_types(&self) -> &[String] {
        &self.response_types
    }
    /// Lax source boolean normalized before client-ID policy reads it.
    pub fn client_id_document_supported(&self) -> Option<bool> {
        self.client_id_document
    }
    /// Lax source boolean normalized before authorization response iss checks.
    pub fn response_iss_supported(&self) -> Option<bool> {
        self.response_iss
    }
}
/// A fully typed document of an explicit source purpose, with extras discarded.
#[derive(Clone, Debug)]
pub enum SourceMetadataProjection {
    /// Valid protected resource document; server zero is selected only after semantic policy.
    Prm(Box<ValidatedPrm>),
    /// Valid authorization server document; default SDK capability checks are not added.
    Asm(Box<ValidatedAsm>),
}
fn member<'a, 's>(object: &'a JsonObject<'s>, key: &str) -> Option<&'a JsonValue<'s>> {
    // Pydantic Core lookup_key.rs140–154 selects the final duplicate member.
    object
        .iter()
        .rev()
        .find(|(name, _)| name.as_ref() == key)
        .map(|(_, value)| value)
}
fn string(value: &JsonValue<'_>) -> Result<String, InvalidMetadata> {
    match value {
        JsonValue::Str(s) => Ok(s.to_string()),
        _ => Err(InvalidMetadata),
    }
}
fn model_url(value: &JsonValue<'_>) -> Result<ModelHttpUrl, InvalidMetadata> {
    match value {
        JsonValue::Str(s) => ModelHttpUrl::parse(s),
        _ => Err(InvalidMetadata),
    }
}
fn required_url(o: &JsonObject<'_>, key: &str) -> Result<ModelHttpUrl, InvalidMetadata> {
    model_url(member(o, key).ok_or(InvalidMetadata)?)
}
fn optional_url(o: &JsonObject<'_>, key: &str) -> Result<Option<ModelHttpUrl>, InvalidMetadata> {
    match member(o, key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(v) => model_url(v).map(Some),
    }
}
fn string_list(value: &JsonValue<'_>) -> Result<Vec<String>, InvalidMetadata> {
    match value {
        JsonValue::Array(v) => v.iter().map(string).collect(),
        _ => Err(InvalidMetadata),
    }
}
fn optional_list(o: &JsonObject<'_>, key: &str) -> Result<Option<Vec<String>>, InvalidMetadata> {
    match member(o, key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(v) => string_list(v).map(Some),
    }
}
fn lax_bool(value: &JsonValue<'_>) -> Result<bool, InvalidMetadata> {
    match value {
        JsonValue::Bool(v) => Ok(*v),
        JsonValue::Int(0) => Ok(false),
        JsonValue::Int(1) => Ok(true),
        JsonValue::Float(n) if *n == 0.0 => Ok(false),
        JsonValue::Float(n) if *n == 1.0 => Ok(true),
        JsonValue::Str(s) => match s.to_ascii_lowercase().as_str() {
            "0" | "f" | "n" | "no" | "off" | "false" => Ok(false),
            "1" | "t" | "y" | "on" | "yes" | "true" => Ok(true),
            _ => Err(InvalidMetadata),
        },
        _ => Err(InvalidMetadata),
    }
}
fn optional_bool(o: &JsonObject<'_>, key: &str) -> Result<Option<bool>, InvalidMetadata> {
    match member(o, key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(v) => lax_bool(v).map(Some),
    }
}
const ASM_URLS: &[&str] = &[
    "registration_endpoint",
    "service_documentation",
    "op_policy_uri",
    "op_tos_uri",
    "revocation_endpoint",
    "introspection_endpoint",
];
const ASM_LISTS: &[&str] = &[
    "scopes_supported",
    "response_modes_supported",
    "grant_types_supported",
    "token_endpoint_auth_methods_supported",
    "token_endpoint_auth_signing_alg_values_supported",
    "ui_locales_supported",
    "revocation_endpoint_auth_methods_supported",
    "revocation_endpoint_auth_signing_alg_values_supported",
    "introspection_endpoint_auth_methods_supported",
    "introspection_endpoint_auth_signing_alg_values_supported",
    "code_challenge_methods_supported",
    "authorization_grant_profiles_supported",
];
const ASM_BOOLS: &[&str] = &[
    "client_id_metadata_document_supported",
    "authorization_response_iss_parameter_supported",
];
const PRM_URLS: &[&str] = &[
    "jwks_uri",
    "resource_documentation",
    "resource_policy_uri",
    "resource_tos_uri",
];
const PRM_LISTS: &[&str] = &[
    "scopes_supported",
    "bearer_methods_supported",
    "resource_signing_alg_values_supported",
    "authorization_details_types_supported",
    "dpop_signing_alg_values_supported",
];
const PRM_BOOLS: &[&str] = &[
    "tls_client_certificate_bound_access_tokens",
    "dpop_bound_access_tokens_required",
];
impl SourceMetadataProjection {
    /// Validate the complete raw JSON document with Core-compatible grammar.
    /// Unknown fields are parsed then ignored. Detailed model errors stay internal.
    pub fn parse(kind: SourceMetadataKind, body: &[u8]) -> Result<Self, InvalidMetadata> {
        let value = JsonValue::parse_with_config(body, true, PartialMode::Off)
            .map_err(|_| InvalidMetadata)?;
        let JsonValue::Object(o) = value else {
            return Err(InvalidMetadata);
        };
        match kind {
            SourceMetadataKind::Prm => {
                let resource = required_url(&o, "resource")?;
                let Some(JsonValue::Array(values)) = member(&o, "authorization_servers") else {
                    return Err(InvalidMetadata);
                };
                if values.is_empty() {
                    return Err(InvalidMetadata);
                }
                let servers = values
                    .iter()
                    .map(model_url)
                    .collect::<Result<Vec<_>, _>>()?;
                for name in PRM_URLS {
                    optional_url(&o, name)?;
                }
                for name in PRM_LISTS {
                    optional_list(&o, name)?;
                }
                for name in PRM_BOOLS {
                    optional_bool(&o, name)?;
                }
                if let Some(v) = member(&o, "resource_name")
                    && !matches!(v, JsonValue::Null)
                {
                    string(v)?;
                }
                let bearer_methods = if member(&o, "bearer_methods_supported").is_none() {
                    Some(vec!["header".into()])
                } else {
                    optional_list(&o, "bearer_methods_supported")?
                };
                Ok(Self::Prm(Box::new(ValidatedPrm {
                    resource,
                    servers,
                    scopes: optional_list(&o, "scopes_supported")?,
                    bearer_methods,
                })))
            }
            SourceMetadataKind::Asm => {
                let issuer = required_url(&o, "issuer")?;
                let authorization = required_url(&o, "authorization_endpoint")?;
                let token = required_url(&o, "token_endpoint")?;
                for name in ASM_URLS {
                    optional_url(&o, name)?;
                }
                for name in ASM_LISTS {
                    optional_list(&o, name)?;
                }
                for name in ASM_BOOLS {
                    optional_bool(&o, name)?;
                }
                let response_types = match member(&o, "response_types_supported") {
                    None => vec!["code".into()],
                    Some(v) => string_list(v)?,
                };
                Ok(Self::Asm(Box::new(ValidatedAsm {
                    issuer,
                    authorization,
                    token,
                    registration: optional_url(&o, "registration_endpoint")?,
                    scopes: optional_list(&o, "scopes_supported")?,
                    response_types,
                    client_id_document: optional_bool(&o, "client_id_metadata_document_supported")?,
                    response_iss: optional_bool(
                        &o,
                        "authorization_response_iss_parameter_supported",
                    )?,
                })))
            }
        }
    }
    /// Return the actual document purpose without inspecting response keys.
    pub fn kind(&self) -> SourceMetadataKind {
        match self {
            Self::Prm(_) => SourceMetadataKind::Prm,
            Self::Asm(_) => SourceMetadataKind::Asm,
        }
    }
}
/// Semantic gate future; resource/issuer mismatch is an error, not model fallback.
pub type SourceMetadataGateFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), OAuthHttpClientError>> + Send + 'a>>;
/// Project resource/issuer policy, awaited after whole validation and before commit.
/// Pending/Err must leave policy state unchanged. An implementation may apply its
/// validated policy delta as its FINAL successful action before Ready(Ok), with
/// no subsequent await/error: SDK state is committed immediately in the same poll.
pub trait SourceMetadataGate: Send + Sync {
    /// Source manager supplies exact raw base and already selected discovery
    /// issuer. Default delegates to the original check-only policy method.
    fn check_with_context<'a>(
        &'a self,
        projection: &'a SourceMetadataProjection,
        _context: &'a SourceMetadataContext,
    ) -> SourceMetadataGateFuture<'a> {
        self.check(projection)
    }
    /// Match loaded credentials before any ASM candidate generation or fetch.
    /// Project policy owns configured client-metadata URL/root equivalence.
    /// Default refuses rather than treating URL-shaped client IDs as portable.
    fn credentials_match<'a>(
        &'a self,
        _credentials: &'a super::StoredCredentials,
        _expected_issuer: &'a str,
    ) -> SourceCredentialsFuture<'a> {
        Box::pin(async { Err(Box::new(SourceOperationUnsupported) as OAuthHttpClientError) })
    }
    /// Apply the correct resource or issuer policy to this already valid document.
    fn check<'a>(
        &'a self,
        projection: &'a SourceMetadataProjection,
    ) -> SourceMetadataGateFuture<'a>;
}
/// Candidate outcome from Python's active utility status contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMetadataDecision {
    /// Invalid/unserved candidate; continue the caller's source candidate order.
    Continue,
    /// ASM unexpected/server status; stop this ASM candidate search.
    Stop,
    /// Whole valid document passed semantic policy and was committed.
    Accepted,
}
/// Remembered PRM transient status when every candidate failed this search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtectedResourceRequestFailed(
    /// Latest429/5xx status remembered by this PRM candidate search.
    pub u16,
);
impl std::fmt::Display for ProtectedResourceRequestFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Protected resource metadata request failed: HTTP {}",
            self.0
        )
    }
}
impl std::error::Error for ProtectedResourceRequestFailed {}
/// Per-document accepted state. This is not a replacement OAuth/token manager.
/// A valid PRM remains committed if later ASM discovery fails. Credential clearing
/// between the two searches remains the caller's explicit source policy boundary.
#[derive(Default)]
pub struct SourceMetadataState {
    prm: Option<ValidatedPrm>,
    asm: Option<ValidatedAsm>,
    prm_failed_status: Option<u16>,
    prm_accepted_this_search: bool,
}
impl SourceMetadataState {
    /// Retain accepted PRM while resetting only this search's failure accounting.
    pub fn begin_prm_search(&mut self) {
        self.prm_failed_status = None;
        self.prm_accepted_this_search = false;
    }
    /// Return the complete accepted PRM, never a partially parsed candidate.
    pub fn prm(&self) -> Option<&ValidatedPrm> {
        self.prm.as_ref()
    }
    /// Return accepted ASM. Call clear_asm when issuer-bound credentials are cleared.
    pub fn asm(&self) -> Option<&ValidatedAsm> {
        self.asm.as_ref()
    }
    /// Clear stale ASM at the source credential-change boundary, before fetching ASM.
    pub fn clear_asm(&mut self) {
        self.asm = None;
    }
    /// Fail only when this search accepted nothing and remembered429/5xx.
    pub fn finish_prm_search(&self) -> Result<(), ProtectedResourceRequestFailed> {
        if !self.prm_accepted_this_search
            && let Some(status) = self.prm_failed_status
        {
            return Err(ProtectedResourceRequestFailed(status));
        }
        Ok(())
    }
    /// Execute an owned buffered operation. Transport/body errors propagate before
    /// candidate interpretation; no URI target is reconstructed from the body.
    pub async fn request_candidate(
        &mut self,
        client: &dyn OAuthHttpClient,
        gate: &dyn SourceMetadataGate,
        kind: SourceMetadataKind,
        request: SourceOAuthHttpRequest,
    ) -> Result<SourceMetadataDecision, OAuthHttpClientError> {
        let response = client.execute_source(request).await?;
        let (parts, body) = response.into_parts();
        self.consider_response(client, gate, kind, parts.status.as_u16(), &body)
            .await
    }
    /// Interpret one complete response and commit only after whole validation and
    /// the awaited semantic gate. Model-invalid is Continue; IO/gate errors escape.
    pub async fn consider_response(
        &mut self,
        client: &dyn OAuthHttpClient,
        gate: &dyn SourceMetadataGate,
        kind: SourceMetadataKind,
        status: u16,
        body: &[u8],
    ) -> Result<SourceMetadataDecision, OAuthHttpClientError> {
        if kind == SourceMetadataKind::Prm && (status >= 500 || status == 429) {
            self.prm_failed_status = Some(status);
        }
        if status != 200 {
            return Ok(
                if kind == SourceMetadataKind::Asm && !(300..500).contains(&status) {
                    SourceMetadataDecision::Stop
                } else {
                    SourceMetadataDecision::Continue
                },
            );
        }
        if body.len() > MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES {
            return Err(Box::new(super::OAuthHttpError::ResponseBodyTooLarge(
                MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES,
            )));
        }
        let Some(projection) = client.source_metadata(kind, body).await? else {
            return Ok(SourceMetadataDecision::Continue);
        };
        if projection.kind() != kind {
            return Err("source metadata adapter returned wrong document purpose".into());
        }
        gate.check(&projection).await?;
        // No await or fallible operation separates gate Ready(Ok) from this
        // field move. A final successful project-policy delta and SDK commit
        // therefore share one poll; task cancellation cannot split them.
        match projection {
            SourceMetadataProjection::Prm(prm) => {
                self.prm = Some(*prm);
                self.prm_accepted_this_search = true;
            }
            SourceMetadataProjection::Asm(asm) => self.asm = Some(*asm),
        }
        Ok(SourceMetadataDecision::Accepted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    fn asm() -> serde_json::Value {
        serde_json::json!({"issuer":"https://as.example", "authorization_endpoint":"https://as.example/a", "token_endpoint":"https://as.example/t"})
    }
    fn prm() -> serde_json::Value {
        serde_json::json!({"resource":"https://resource.example/mcp", "authorization_servers":["https://as.example"]})
    }
    fn parse(
        kind: SourceMetadataKind,
        value: &serde_json::Value,
    ) -> Result<SourceMetadataProjection, InvalidMetadata> {
        SourceMetadataProjection::parse(kind, &serde_json::to_vec(value).unwrap())
    }
    #[test]
    fn source_models_required_fields_missing_null_and_wrong_type_reject_whole_document() {
        for (kind, base, keys) in [
            (
                SourceMetadataKind::Asm,
                asm(),
                vec!["issuer", "authorization_endpoint", "token_endpoint"],
            ),
            (
                SourceMetadataKind::Prm,
                prm(),
                vec!["resource", "authorization_servers"],
            ),
        ] {
            assert!(parse(kind, &base).is_ok());
            for key in keys {
                let mut value = base.clone();
                value.as_object_mut().unwrap().remove(key);
                assert!(parse(kind, &value).is_err(), "missing {key}");
                for bad in [
                    serde_json::Value::Null,
                    serde_json::json!(3),
                    serde_json::json!(false),
                ] {
                    let mut value = base.clone();
                    value[key] = bad;
                    assert!(parse(kind, &value).is_err(), "invalid {key}");
                }
            }
        }
    }
    #[test]
    fn source_models_validate_every_optional_url_including_unused_fields() {
        for (kind, base, names) in [
            (SourceMetadataKind::Asm, asm(), ASM_URLS),
            (SourceMetadataKind::Prm, prm(), PRM_URLS),
        ] {
            for key in names {
                for good in [
                    serde_json::Value::Null,
                    serde_json::json!("https://unused.example"),
                ] {
                    let mut v = base.clone();
                    v[*key] = good;
                    assert!(parse(kind, &v).is_ok(), "valid {key}");
                }
                for bad in [
                    serde_json::json!(""),
                    serde_json::json!("ftp://unused.example"),
                    serde_json::json!(7),
                    serde_json::json!([]),
                ] {
                    let mut v = base.clone();
                    v[*key] = bad;
                    assert!(parse(kind, &v).is_err(), "invalid unused {key}");
                }
            }
        }
    }
    #[test]
    fn source_models_lists_have_exact_null_empty_and_string_member_boundaries() {
        for (kind, base, names) in [
            (SourceMetadataKind::Asm, asm(), ASM_LISTS),
            (SourceMetadataKind::Prm, prm(), PRM_LISTS),
        ] {
            for key in names {
                for good in [
                    serde_json::Value::Null,
                    serde_json::json!([]),
                    serde_json::json!(["", " a,b "]),
                ] {
                    let mut v = base.clone();
                    v[*key] = good;
                    assert!(parse(kind, &v).is_ok(), "valid {key}");
                }
                for bad in [
                    serde_json::json!("a,b"),
                    serde_json::json!([null]),
                    serde_json::json!([1]),
                    serde_json::json!([false]),
                    serde_json::json!([[]]),
                ] {
                    let mut v = base.clone();
                    v[*key] = bad;
                    assert!(parse(kind, &v).is_err(), "invalid {key}");
                }
            }
        }
        let mut v = asm();
        v["response_types_supported"] = serde_json::Value::Null;
        assert!(parse(SourceMetadataKind::Asm, &v).is_err());
        v["response_types_supported"] = serde_json::json!([]);
        v["code_challenge_methods_supported"] = serde_json::json!([]);
        assert!(
            parse(SourceMetadataKind::Asm, &v).is_ok(),
            "no invented code/S256 metadata capability rejection"
        );
        let mut v = prm();
        v["resource_name"] = serde_json::json!("");
        assert!(parse(SourceMetadataKind::Prm, &v).is_ok());
        v["resource_name"] = serde_json::json!(1);
        assert!(parse(SourceMetadataKind::Prm, &v).is_err());
    }
    #[test]
    fn source_models_lax_boolean_acceptance_is_not_as_bool_or_trimmed_text() {
        for (kind, base, names) in [
            (SourceMetadataKind::Asm, asm(), ASM_BOOLS),
            (SourceMetadataKind::Prm, prm(), PRM_BOOLS),
        ] {
            for key in names {
                for good in [
                    serde_json::json!(true),
                    serde_json::json!(false),
                    serde_json::json!(0),
                    serde_json::json!(1),
                    serde_json::json!(0.0),
                    serde_json::json!(1.0),
                    serde_json::json!("TrUe"),
                    serde_json::json!("OFF"),
                    serde_json::Value::Null,
                ] {
                    let mut v = base.clone();
                    v[*key] = good;
                    assert!(parse(kind, &v).is_ok(), "valid {key}");
                }
                for bad in [
                    serde_json::json!(" true "),
                    serde_json::json!(2),
                    serde_json::json!(0.5),
                    serde_json::json!([]),
                    serde_json::json!({}),
                ] {
                    let mut v = base.clone();
                    v[*key] = bad;
                    assert!(parse(kind, &v).is_err(), "invalid {key}");
                }
            }
        }
        let mut v = asm();
        v["client_id_metadata_document_supported"] = serde_json::json!("YES");
        v["authorization_response_iss_parameter_supported"] = serde_json::json!(1.0);
        let SourceMetadataProjection::Asm(v) = parse(SourceMetadataKind::Asm, &v).unwrap() else {
            panic!()
        };
        assert_eq!(v.client_id_document_supported(), Some(true));
        assert_eq!(v.response_iss_supported(), Some(true));
    }
    #[test]
    fn source_models_servers_validate_all_members_and_ignore_singular_extra() {
        let mut v = prm();
        v["authorization_servers"] = serde_json::json!(["https://valid.example", "bad"]);
        assert!(parse(SourceMetadataKind::Prm, &v).is_err());
        v["authorization_servers"] = serde_json::json!([]);
        assert!(parse(SourceMetadataKind::Prm, &v).is_err());
        v.as_object_mut().unwrap().remove("authorization_servers");
        v["authorization_server"] = serde_json::json!("https://singular.example");
        assert!(parse(SourceMetadataKind::Prm, &v).is_err());
        let mut v = prm();
        v["authorization_servers"] =
            serde_json::json!(["https://first.example", "https://second.example"]);
        let SourceMetadataProjection::Prm(v) = parse(SourceMetadataKind::Prm, &v).unwrap() else {
            panic!()
        };
        assert_eq!(
            v.authorization_servers()[0].as_str(),
            "https://first.example"
        );
        assert_eq!(v.authorization_servers().len(), 2);
    }
    #[test]
    fn source_models_duplicates_use_final_value_without_skipping_json_grammar() {
        let before = br#"{"resource":3,"resource":"https://resource.example","authorization_servers":["https://as.example"]}"#;
        assert!(SourceMetadataProjection::parse(SourceMetadataKind::Prm, before).is_ok());
        let after = br#"{"resource":"https://resource.example","resource":null,"authorization_servers":["https://as.example"]}"#;
        assert!(SourceMetadataProjection::parse(SourceMetadataKind::Prm, after).is_err());
        let malformed = br#"{"resource":bad,"resource":"https://resource.example","authorization_servers":["https://as.example"]}"#;
        assert!(SourceMetadataProjection::parse(SourceMetadataKind::Prm, malformed).is_err());
        let ignored = format!(
            r#"{{"issuer":"https://as.example","authorization_endpoint":"https://as.example/a","token_endpoint":"https://as.example/t","jwks_uri":17,"extra":[NaN,Infinity,-Infinity,1e999,{}]}}"#,
            "9".repeat(100)
        );
        assert!(
            SourceMetadataProjection::parse(SourceMetadataKind::Asm, ignored.as_bytes()).is_ok(),
            "valid generic extras do not become typed metadata"
        );
        let known = ignored.replace("\"extra\"", "\"scopes_supported\"");
        assert!(
            SourceMetadataProjection::parse(SourceMetadataKind::Asm, known.as_bytes()).is_err()
        );
        let nan_flag = ignored.replace(
            "\"extra\":[",
            "\"client_id_metadata_document_supported\":NaN,\"extra\":[",
        );
        assert!(
            SourceMetadataProjection::parse(SourceMetadataKind::Asm, nan_flag.as_bytes()).is_err()
        );
    }
    #[test]
    fn source_models_json_lane_rejects_bom_surrogate_utf16_and_trailing_values() {
        let good = serde_json::to_vec(&asm()).unwrap();
        let mut bom = vec![0xef, 0xbb, 0xbf];
        bom.extend_from_slice(&good);
        assert!(SourceMetadataProjection::parse(SourceMetadataKind::Asm, &bom).is_err());
        let mut trailing = good.clone();
        trailing.extend_from_slice(b" null");
        assert!(SourceMetadataProjection::parse(SourceMetadataKind::Asm, &trailing).is_err());
        for bad in [
            b"\xff".as_slice(),
            br#"{"x":"\ud800"}"#.as_slice(),
            b"{\0}\0".as_slice(),
        ] {
            assert!(SourceMetadataProjection::parse(SourceMetadataKind::Asm, bad).is_err());
        }
    }
    #[test]
    fn source_model_urls_preserve_only_inserted_slash_and_never_raw_port_policy() {
        for (input, expected) in [
            ("http://HOST", "http://host"),
            ("http://host/", "http://host/"),
            ("http://host?x#f", "http://host?x#f"),
            ("http:////host", "http://host"),
            ("http:////host/", "http://host/"),
            ("http://u:p@HOST:80?x", "http://u:p@host?x"),
        ] {
            assert_eq!(ModelHttpUrl::parse(input).unwrap().as_str(), expected);
        }
        for bad in ["", "ftp://host", "http://host:-1", "http://host:65536"] {
            assert!(ModelHttpUrl::parse(bad).is_err());
        }
        let long = format!("https://host/{}", "a".repeat(2200));
        assert_eq!(
            ModelHttpUrl::parse(&long).unwrap().as_str(),
            long,
            "AnyHttpUrl has no HttpUrl2083 cap"
        );
    }
    #[test]
    fn source_model_defaults_and_empty_scope_projection_are_preserved() {
        let SourceMetadataProjection::Asm(a) = parse(SourceMetadataKind::Asm, &asm()).unwrap()
        else {
            panic!()
        };
        assert_eq!(a.response_types(), &["code".to_owned()]);
        assert_eq!(a.client_id_document_supported(), None);
        assert_eq!(a.scopes(), None);
        let SourceMetadataProjection::Prm(p) = parse(SourceMetadataKind::Prm, &prm()).unwrap()
        else {
            panic!()
        };
        assert_eq!(p.bearer_methods(), Some(["header".to_owned()].as_slice()));
        assert_eq!(p.scopes(), None);
        let mut p = prm();
        p["scopes_supported"] = serde_json::json!([]);
        p["bearer_methods_supported"] = serde_json::Value::Null;
        let SourceMetadataProjection::Prm(p) = parse(SourceMetadataKind::Prm, &p).unwrap() else {
            panic!()
        };
        assert_eq!(p.scopes(), Some(&[] as &[String]));
        assert_eq!(p.bearer_methods(), None);
    }
    #[test]
    fn source_buffered_request_retains_raw_owner_and_model_origin_without_uri() {
        let raw: Arc<str> = " http://u:p@RAW.test:-1/a?#f ".into();
        let request = SourceOAuthHttpRequest::discovery(SourceEndpoint::advertised(raw.clone()));
        let cloned = request.clone();
        assert!(Arc::ptr_eq(&raw, request.endpoint.text()));
        assert!(Arc::ptr_eq(request.endpoint.text(), cloned.endpoint.text()));
        assert_eq!(request.endpoint.text().as_ref(), raw.as_ref());
        assert_eq!(request.endpoint.origin(), SourceEndpointOrigin::Advertised);
        let model = ModelHttpUrl::parse("http://HOST?x").unwrap();
        let endpoint = model.endpoint();
        assert_eq!(endpoint.origin(), SourceEndpointOrigin::Model);
        assert_eq!(endpoint.text().as_ref(), "http://host?x");
        assert!(Arc::ptr_eq(&model.serialized, endpoint.text()));
        assert!(!format!("{request:?}").contains("u:p"));
    }
    #[derive(Debug)]
    struct TransportFailure;
    impl std::fmt::Display for TransportFailure {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("fixture transport read failed")
        }
    }
    impl std::error::Error for TransportFailure {}
    struct ParserClient;
    impl OAuthHttpClient for ParserClient {
        fn execute_source(
            &self,
            _: SourceOAuthHttpRequest,
        ) -> super::super::OAuthHttpClientFuture<'_> {
            Box::pin(async { Err(Box::new(TransportFailure) as OAuthHttpClientError) })
        }
        fn execute(
            &self,
            _: super::super::OAuthHttpRequest,
        ) -> super::super::OAuthHttpClientFuture<'_> {
            panic!("source metadata never enters default URI request path")
        }
        fn source_metadata<'a>(
            &'a self,
            kind: SourceMetadataKind,
            body: &'a [u8],
        ) -> SourceMetadataFuture<'a> {
            Box::pin(async move { Ok(SourceMetadataProjection::parse(kind, body).ok()) })
        }
    }
    struct Gate {
        calls: AtomicUsize,
        reject: bool,
    }
    impl SourceMetadataGate for Gate {
        fn check<'a>(&'a self, _: &'a SourceMetadataProjection) -> SourceMetadataGateFuture<'a> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if self.reject {
                    Err("semantic resource/issuer mismatch".into())
                } else {
                    Ok(())
                }
            })
        }
    }
    #[tokio::test]
    async fn source_candidate_commits_only_valid_documents_after_semantic_gate() {
        let mut state = SourceMetadataState::default();
        let accept = Gate {
            calls: AtomicUsize::new(0),
            reject: false,
        };
        let reject = Gate {
            calls: AtomicUsize::new(0),
            reject: true,
        };
        let invalid = br#"{"resource":"https://resource.example","authorization_servers":["https://first.example",null]}"#;
        assert_eq!(
            state
                .consider_response(
                    &ParserClient,
                    &accept,
                    SourceMetadataKind::Prm,
                    200,
                    invalid
                )
                .await
                .unwrap(),
            SourceMetadataDecision::Continue
        );
        assert!(state.prm().is_none());
        assert_eq!(accept.calls.load(Ordering::SeqCst), 0);
        let body = serde_json::to_vec(&prm()).unwrap();
        assert!(
            state
                .consider_response(&ParserClient, &reject, SourceMetadataKind::Prm, 200, &body)
                .await
                .is_err()
        );
        assert!(state.prm().is_none());
        assert_eq!(
            state
                .consider_response(&ParserClient, &accept, SourceMetadataKind::Prm, 200, &body)
                .await
                .unwrap(),
            SourceMetadataDecision::Accepted
        );
        assert!(state.prm().is_some());
        let body = serde_json::to_vec(&asm()).unwrap();
        state
            .consider_response(&ParserClient, &accept, SourceMetadataKind::Asm, 200, &body)
            .await
            .unwrap();
        assert!(state.asm().is_some());
        state.clear_asm();
        state
            .consider_response(
                &ParserClient,
                &accept,
                SourceMetadataKind::Asm,
                200,
                b"invalid",
            )
            .await
            .unwrap();
        assert!(state.prm().is_some());
        assert!(
            state.asm().is_none(),
            "invalid rediscovery cannot resurrect cleared ASM"
        );
    }
    #[tokio::test]
    async fn source_candidate_status_fallback_and_remembered_prm_failure_are_distinct() {
        let mut state = SourceMetadataState::default();
        let gate = Gate {
            calls: AtomicUsize::new(0),
            reject: false,
        };
        state.begin_prm_search();
        for status in [500, 404, 429, 200] {
            assert_eq!(
                state
                    .consider_response(
                        &ParserClient,
                        &gate,
                        SourceMetadataKind::Prm,
                        status,
                        b"invalid"
                    )
                    .await
                    .unwrap(),
                SourceMetadataDecision::Continue
            );
        }
        assert_eq!(state.finish_prm_search().unwrap_err().0, 429);
        let body = serde_json::to_vec(&prm()).unwrap();
        state
            .consider_response(&ParserClient, &gate, SourceMetadataKind::Prm, 200, &body)
            .await
            .unwrap();
        state.finish_prm_search().unwrap();
        for (status, expected) in [
            (200, SourceMetadataDecision::Continue),
            (300, SourceMetadataDecision::Continue),
            (404, SourceMetadataDecision::Continue),
            (499, SourceMetadataDecision::Continue),
            (500, SourceMetadataDecision::Stop),
            (204, SourceMetadataDecision::Stop),
        ] {
            assert_eq!(
                state
                    .consider_response(
                        &ParserClient,
                        &gate,
                        SourceMetadataKind::Asm,
                        status,
                        b"invalid"
                    )
                    .await
                    .unwrap(),
                expected
            );
        }
    }
    #[tokio::test]
    async fn source_default_refusal_body_limit_and_transport_errors_never_become_model_fallback() {
        struct DefaultClient;
        impl OAuthHttpClient for DefaultClient {
            fn execute(
                &self,
                _: super::super::OAuthHttpRequest,
            ) -> super::super::OAuthHttpClientFuture<'_> {
                panic!("no source-to-default fallback")
            }
        }
        let error = DefaultClient
            .execute_source(SourceOAuthHttpRequest::discovery(SourceEndpoint::derived(
                Arc::from("http://unparsed.invalid:-1"),
            )))
            .await
            .unwrap_err();
        assert!(error.downcast_ref::<SourceOperationUnsupported>().is_some());
        let gate = Gate {
            calls: AtomicUsize::new(0),
            reject: false,
        };
        let mut state = SourceMetadataState::default();
        let oversized = vec![b' '; MAX_OAUTH_HTTP_RESPONSE_BODY_BYTES + 1];
        assert!(
            state
                .consider_response(
                    &ParserClient,
                    &gate,
                    SourceMetadataKind::Prm,
                    200,
                    &oversized
                )
                .await
                .is_err()
        );
        assert!(state.prm().is_none());
        assert!(
            state
                .request_candidate(
                    &ParserClient,
                    &gate,
                    SourceMetadataKind::Prm,
                    SourceOAuthHttpRequest::discovery(SourceEndpoint::derived(Arc::from(
                        "http://unparsed.invalid:-1"
                    )))
                )
                .await
                .unwrap_err()
                .downcast_ref::<TransportFailure>()
                .is_some()
        );
        assert!(state.prm().is_none());
    }
    #[tokio::test]
    async fn source_cancelled_semantic_gate_leaves_accepted_document_unchanged() {
        struct PendingGate;
        impl SourceMetadataGate for PendingGate {
            fn check<'a>(
                &'a self,
                _: &'a SourceMetadataProjection,
            ) -> SourceMetadataGateFuture<'a> {
                Box::pin(std::future::pending())
            }
        }
        let mut state = SourceMetadataState::default();
        let body = serde_json::to_vec(&prm()).unwrap();
        let mut pending = Box::pin(state.consider_response(
            &ParserClient,
            &PendingGate,
            SourceMetadataKind::Prm,
            200,
            &body,
        ));
        std::future::poll_fn(|cx| {
            assert!(pending.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(pending);
        assert!(state.prm().is_none());
    }
}

/// Exact inputs for finite source discovery derivation, before any typed URI.
pub struct SourceMetadataUrlsRequest {
    /// Explicit PRM or ASM candidate purpose.
    pub kind: SourceMetadataKind,
    /// Same raw base owner retained by the manager's source constructor.
    pub base: Arc<str>,
    /// Entire original challenge header for source field extraction, untrimmed.
    /// Only PRM purpose supplies it; absence means no advertised candidate.
    pub raw_challenge: Option<Arc<str>>,
    /// Accepted server zero, preserving validated model serialization.
    pub selected_server: Option<ModelHttpUrl>,
}
/// Source URL derivation future; ordered candidates are never sorted/deduped.
pub type SourceMetadataUrlsFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<SourceEndpoint>, OAuthHttpClientError>> + Send + 'a>>;
/// Exact expected issuer derivation, separate from later ASM URL generation.
pub type SourceExpectedIssuerFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Arc<str>, OAuthHttpClientError>> + Send + 'a>>;
/// Credential-binding check future; false requests source credential clearing.
pub type SourceCredentialsFuture<'a> =
    Pin<Box<dyn Future<Output = Result<bool, OAuthHttpClientError>> + Send + 'a>>;
/// Immutable context for the source resource/issuer semantic gate.
pub struct SourceMetadataContext {
    pub(super) base: Arc<str>,
    pub(super) expected_issuer: Option<Arc<str>>,
    pub(super) has_prm: bool,
}
impl SourceMetadataContext {
    /// Original raw source base, never projected through SDK Url.
    pub fn base(&self) -> &Arc<str> {
        &self.base
    }
    /// Exact expected issuer; Some only on the ASM phase.
    pub fn expected_issuer(&self) -> Option<&str> {
        self.expected_issuer.as_deref()
    }
    /// Whether accepted PRM supplied the selected server, versus legacy origin.
    pub fn has_prm(&self) -> bool {
        self.has_prm
    }
}
/// Accepted source discovery state returned without default metadata conversion.
#[derive(Clone)]
pub struct SourceMetadataResolution {
    pub(super) prm: Option<ValidatedPrm>,
    pub(super) asm: Option<ValidatedAsm>,
    pub(super) expected_issuer: Arc<str>,
}
impl std::fmt::Debug for SourceMetadataResolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceMetadataResolution")
            .field("prm", &self.prm)
            .field("asm", &self.asm)
            .field("expected_issuer", &"[REDACTED]")
            .finish()
    }
}
impl SourceMetadataResolution {
    /// Accepted PRM, retained even when ASM discovery found no valid candidate.
    pub fn prm(&self) -> Option<&ValidatedPrm> {
        self.prm.as_ref()
    }
    /// Accepted ASM, without default SDK code/S256 capability rejection.
    pub fn asm(&self) -> Option<&ValidatedAsm> {
        self.asm.as_ref()
    }
    /// Expected issuer used before credential clearing and ASM discovery.
    pub fn expected_issuer(&self) -> &str {
        &self.expected_issuer
    }
}
