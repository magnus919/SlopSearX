//! Native SDK OAuth composition with the original client_info/tokens disk schema.
//! No SDK default HTTP client or process-global auth/logging state.
use super::{
    browser::BrowserOwner,
    callback::LoopbackCallback,
    io_owner::IoOwner,
    network::Network,
    oauth_policy::{Client, Policy},
    settings::{Settings, resolved_token_path},
    token_storage::{FileTokenStorage, Slot},
};
use crate::Result;
use async_trait::async_trait;
use oauth2::TokenResponse;
#[cfg(not(feature = "gateway-source-endpoints"))]
use rmcp::transport::auth::AuthorizationMetadata;
use rmcp::transport::auth::{
    AuthError, AuthorizationManager, CredentialStore, OAuthClientConfig, OAuthHttpClient,
    OAuthHttpClientFuture, OAuthHttpRedirectPolicy, OAuthHttpRequest, OAuthTokenResponse,
    StoredCredentials,
};
use std::{
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
const AUTH_BUDGET: Duration = Duration::from_secs(300);
#[cfg(feature = "gateway-source-endpoints")]
async fn source_stored_client(raw: &[u8], deadline: Instant) -> Result<Client> {
    let value = super::token_json::parse(raw, deadline)
        .await
        .map_err(|_| "OAuth stored client projection invalid")?;
    Client::from_source_record(&value)
}
fn store_error() -> AuthError {
    AuthError::CredentialStoreError("gateway token storage failed".into())
}
#[derive(Clone)]
struct SourceStore {
    file: Arc<FileTokenStorage>,
    cancel: CancellationToken,
    expires: Arc<Mutex<Option<f64>>>,
    pending: Arc<Mutex<Option<Pending>>>,
    cache: Arc<Mutex<Option<Option<StoredCredentials>>>>,
}
impl SourceStore {
    fn deadline(&self) -> Instant {
        Instant::now() + AUTH_BUDGET
    }
}
#[async_trait]
impl CredentialStore for SourceStore {
    async fn load(&self) -> std::result::Result<Option<StoredCredentials>, AuthError> {
        let mut cache = self.cache.lock().await;
        if let Some(cached) = cache.as_ref() {
            return Ok(cached.clone());
        }
        #[cfg(feature = "gateway-source-endpoints")]
        let tokens = self
            .file
            .get_tokens(self.deadline(), &self.cancel)
            .await
            .map_err(|_| store_error())?;
        let client = self
            .file
            .get_client_info(self.deadline(), &self.cancel)
            .await
            .map_err(|_| store_error())?;
        #[cfg(feature = "gateway-source-endpoints")]
        let client = match client {
            Some(client) => Some(
                source_stored_client(client.json(), self.deadline())
                    .await
                    .map_err(|_| store_error())?,
            ),
            None => None,
        };
        #[cfg(not(feature = "gateway-source-endpoints"))]
        let client = client
            .map(|client| Client::parse(client.json()))
            .transpose()
            .map_err(|_| store_error())?;
        let id = client
            .as_ref()
            .map(|client| client.id.clone())
            .unwrap_or_default();
        #[cfg(not(feature = "gateway-source-endpoints"))]
        let tokens = self
            .file
            .get_tokens(self.deadline(), &self.cancel)
            .await
            .map_err(|_| store_error())?;
        let token_response = tokens
            .map(|model| sdk_token(model.json()))
            .transpose()
            .map_err(|_| store_error())?;
        // Original loaded tokens have no inferred receipt clock. SDK's30s refresh
        // buffer is disabled; actual expiry is owned by this instance below.
        let cached = if client.is_some() || token_response.is_some() {
            Some(
                StoredCredentials::new(id, token_response, vec![], None)
                    .with_issuer(client.and_then(|client| client.issuer)),
            )
        } else {
            None
        };
        *cache = Some(cached.clone());
        Ok(cached)
    }
    async fn save(&self, credentials: StoredCredentials) -> std::result::Result<(), AuthError> {
        *self.cache.lock().await = Some(Some(credentials.clone()));
        if let Some(tokens) = credentials.token_response {
            let pending = self.pending.lock().await.take();
            let (mut raw, expires) = if let Some(pending) = pending {
                (pending.model.json().to_vec(), pending.expires)
            } else {
                (
                    serde_json::to_vec(&tokens).map_err(|_| store_error())?,
                    None,
                )
            };
            // SDK refresh preserves a previous refresh token if the response
            // omits one; preserve that source behavior in the canonical slot.
            if let Some(refresh) = tokens.refresh_token() {
                let mut fields: std::collections::BTreeMap<
                    String,
                    Box<serde_json::value::RawValue>,
                > = serde_json::from_slice(&raw).map_err(|_| store_error())?;
                if fields
                    .get("refresh_token")
                    .is_none_or(|v| v.get() == "null")
                {
                    fields.insert(
                        "refresh_token".into(),
                        serde_json::value::RawValue::from_string(
                            serde_json::to_string(refresh.secret()).map_err(|_| store_error())?,
                        )
                        .map_err(|_| store_error())?,
                    );
                    raw = serde_json::to_vec(&fields).map_err(|_| store_error())?;
                }
            }
            let source = super::token_json::parse(&raw, self.deadline())
                .await
                .map_err(|_| store_error())?;
            let model = super::token_model::validate(Some(&source), self.deadline())
                .await
                .map_err(|_| store_error())?
                .ok_or_else(store_error)?;
            self.file
                .set_slot_json(Slot::Tokens, model.json(), self.deadline(), &self.cancel)
                .await
                .map_err(|_| store_error())?;
            *self.expires.lock().await = expires;
        }
        Ok(())
    }
    async fn clear(&self) -> std::result::Result<(), AuthError> {
        *self.cache.lock().await = Some(None);
        *self.expires.lock().await = None;
        Ok(())
    }
}
struct Pending {
    model: super::token_model::TokenModel,
    expires: Option<f64>,
}
fn wall_time() -> Result<f64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64())
}
fn source_expiry(raw: &[u8]) -> Result<Option<f64>> {
    let fields: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_slice(raw)?;
    let Some(expires) = fields.get("expires_in").filter(|v| v.get() != "null") else {
        return Ok(None);
    };
    let seconds: f64 = expires.get().parse()?;
    if !seconds.is_finite() {
        return Err("OAuth expiry integer exceeds source float range".into());
    }
    Ok(Some(wall_time()? + seconds))
}
fn sdk_token_bytes(raw: &[u8]) -> Result<Vec<u8>> {
    let mut fields: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_slice(raw)?;
    fields.remove("expires_in");
    Ok(serde_json::to_vec(&fields)?)
}
fn sdk_token(raw: &[u8]) -> Result<OAuthTokenResponse> {
    Ok(serde_json::from_slice(&sdk_token_bytes(raw)?)?)
}
/// SDK requests execute through caller-configured clients; suggested aggregate
/// SDK30s timeout is not placed over source read300/interactive300 phases.
struct Http {
    cache: Arc<Mutex<Option<Option<StoredCredentials>>>>,
    policy: Arc<Mutex<Policy>>,
    network: Arc<Network>,
    pending: Arc<Mutex<Option<Pending>>>,
    registration: Arc<Mutex<Option<Vec<u8>>>>,
}

fn token_input_json_error(error: super::token_json::JsonError, source_lane: bool) -> crate::Error {
    #[cfg(feature = "gateway-source-endpoints")]
    if source_lane
        && matches!(
            error,
            super::token_json::JsonError::Malformed | super::token_json::JsonError::Encoding
        )
    {
        return Box::new(rmcp::transport::auth::source_metadata::SourceOAuthModelInvalid);
    }
    let _ = (error, source_lane);
    "OAuth token response malformed".into()
}
fn token_input_model_error(
    error: super::token_model::ModelError,
    source_lane: bool,
) -> crate::Error {
    #[cfg(feature = "gateway-source-endpoints")]
    if source_lane && error == super::token_model::ModelError::Validation {
        return Box::new(rmcp::transport::auth::source_metadata::SourceOAuthModelInvalid);
    }
    let _ = (error, source_lane);
    "OAuth token response invalid".into()
}

impl Http {
    async fn finish_token(
        &self,
        bytes: &[u8],
        refresh: bool,
        source_lane: bool,
    ) -> Result<Vec<u8>> {
        let deadline = Instant::now() + AUTH_BUDGET;
        if refresh {
            self.policy.lock().await.refresh_fallback = true;
        }
        #[cfg(feature = "gateway-source-endpoints")]
        let source = if source_lane {
            super::token_json::parse_http(bytes, deadline).await
        } else {
            super::token_json::parse(bytes, deadline).await
        };
        #[cfg(not(feature = "gateway-source-endpoints"))]
        let source = super::token_json::parse(bytes, deadline).await;
        let source = source.map_err(|error| token_input_json_error(error, source_lane))?;
        let model = super::token_model::validate(Some(&source), deadline)
            .await
            .map_err(|error| token_input_model_error(error, source_lane))?
            .ok_or_else(|| {
                if source_lane {
                    token_input_model_error(super::token_model::ModelError::Validation, true)
                } else {
                    crate::Error::from("OAuth token response empty")
                }
            })?;
        let mut fields: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
            serde_json::from_slice(model.json())?;
        if fields.get("scope").is_none_or(|v| v.get() == "null") {
            let scope = if refresh {
                self.cache
                    .lock()
                    .await
                    .as_ref()
                    .and_then(|v| v.as_ref())
                    .and_then(|v| v.token_response.as_ref())
                    .and_then(|v| v.scopes())
                    .map(|scopes| {
                        scopes
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
            } else {
                self.policy.lock().await.requested.clone()
            };
            if let Some(scope) = scope {
                fields.insert(
                    "scope".into(),
                    serde_json::value::RawValue::from_string(serde_json::to_string(&scope)?)?,
                );
            }
        }
        let raw = serde_json::to_vec(&fields)?;
        #[cfg(feature = "gateway-source-endpoints")]
        let source = if source_lane {
            super::token_json::parse_http(&raw, deadline).await
        } else {
            super::token_json::parse(&raw, deadline).await
        };
        #[cfg(not(feature = "gateway-source-endpoints"))]
        let source = super::token_json::parse(&raw, deadline).await;
        let source = source.map_err(|_| "OAuth normalized token malformed")?;
        let model = super::token_model::validate(Some(&source), deadline)
            .await
            .map_err(|_| "OAuth normalized token invalid")?
            .ok_or("OAuth normalized token empty")?;
        // Overflow is not a source ValidationError: it aborts rather than
        // falling through to authorization or persisting the token.
        if refresh {
            self.policy.lock().await.refresh_fallback = false;
        }
        let expires = source_expiry(model.json())?;
        let bytes = sdk_token_bytes(model.json())?;
        *self.pending.lock().await = Some(Pending { model, expires });
        Ok(bytes)
    }
}
impl OAuthHttpClient for Http {
    #[cfg(feature = "gateway-source-endpoints")]
    fn source_oauth_endpoint(
        &self,
        request: rmcp::transport::auth::source_metadata::SourceOAuthEndpointRequest,
    ) -> rmcp::transport::auth::source_metadata::SourceOAuthEndpointFuture<'_> {
        Box::pin(async move {
            use rmcp::transport::auth::source_metadata::{SourceEndpoint, SourceOAuthEndpointKind};
            let parts = reqwest::source_pool::source_raw_urlparse(&request.base)?;
            let origin = format!("{}://{}", parts.scheme(), parts.netloc());
            let suffix = match request.kind {
                SourceOAuthEndpointKind::Authorization => "/authorize",
                SourceOAuthEndpointKind::Token => "/token",
                SourceOAuthEndpointKind::Registration => "/register",
            };
            Ok(SourceEndpoint::derived(
                reqwest::source_pool::source_raw_urljoin(&origin, suffix)?.into(),
            ))
        })
    }
    #[cfg(feature = "gateway-source-endpoints")]
    fn execute_source(
        &self,
        operation: rmcp::transport::auth::source_metadata::SourceOAuthHttpRequest,
    ) -> OAuthHttpClientFuture<'_> {
        Box::pin(async move {
            use rmcp::transport::auth::source_metadata::SourceOAuthPurpose;
            let purpose = operation.purpose;
            let discovery = purpose == SourceOAuthPurpose::Discovery;
            let registration = purpose == SourceOAuthPurpose::Registration;
            let refresh = purpose == SourceOAuthPurpose::Refresh;
            let token_exchange = matches!(
                purpose,
                SourceOAuthPurpose::CodeExchange | SourceOAuthPurpose::Refresh
            );
            if operation.method
                != (if discovery {
                    reqwest::Method::GET
                } else {
                    reqwest::Method::POST
                })
            {
                return Err("OAuth source operation method mismatch".into());
            }
            let mut headers = operation.headers;
            let body = operation.body;
            if discovery {
                headers.insert(
                    "mcp-protocol-version",
                    reqwest::header::HeaderValue::from_static("2026-07-28"),
                );
            }
            // Preserve the source manager's field order, Python quoting, native
            // application_type and policy-selected resource/authentication.
            if let Some(value) = headers.get_mut(reqwest::header::AUTHORIZATION) {
                value.set_sensitive(true);
            }
            if refresh {
                self.policy.lock().await.refresh_fallback = false;
            }
            let max_redirects = match operation.redirect_policy {
                OAuthHttpRedirectPolicy::Stop => 0,
                OAuthHttpRedirectPolicy::SourceWithinOrigin => 5,
                _ => return Err("OAuth source redirect policy unsupported".into()),
            };
            // This exact endpoint text reaches the single Network SourceUrl parse;
            // no Url or http::Uri target is constructed by the OAuth adapter.
            let mut response = self
                .network
                .send_built_request(
                    operation.method,
                    operation.endpoint.text().to_string(),
                    headers,
                    body,
                    max_redirects,
                )
                .await?;
            let status = response.status();
            let headers = response.headers().clone();
            let mut bytes = Vec::new();
            while let Some(chunk) = self.network.chunk(&mut response).await? {
                if chunk.len() > super::token_json::MAX_BYTES.saturating_sub(bytes.len()) {
                    return Err("OAuth response exceeds source document profile".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            if refresh && status != reqwest::StatusCode::OK {
                self.policy.lock().await.refresh_fallback = true;
            }
            if registration
                && matches!(
                    status,
                    reqwest::StatusCode::OK | reqwest::StatusCode::CREATED
                )
            {
                let deadline = Instant::now() + AUTH_BUDGET;
                let source = super::token_json::parse_http(&bytes, deadline)
                    .await
                    .map_err(|_| "OAuth registration malformed")?;
                let source = without_registration_issuer(source);
                let model = super::client_model::validate(Some(&source), deadline)
                    .await
                    .map_err(|_| "OAuth registration invalid")?
                    .ok_or("OAuth registration empty")?;
                let canonical = model.json().to_vec();
                let projected = super::token_json::parse_http(&canonical, deadline)
                    .await
                    .map_err(|_| "OAuth registration projection invalid")?;
                let client = Client::from_source_record(&projected)?;
                // SDK consumes only these fields. Full Any-valued metadata stays
                // in the canonical source record; NaN cannot poison serde's
                // otherwise unrelated Registered projection.
                bytes = serde_json::to_vec(&serde_json::json!({
                    "client_id": client.id, "client_secret": client.secret,
                    "token_endpoint_auth_method": client.method
                }))?;
                *self.registration.lock().await = Some(canonical);
            }
            if token_exchange
                && (if refresh {
                    status == reqwest::StatusCode::OK
                } else {
                    matches!(
                        status,
                        reqwest::StatusCode::OK | reqwest::StatusCode::CREATED
                    )
                })
            {
                bytes = self.finish_token(&bytes, refresh, true).await?;
            }
            // Preserve actual status. The source manager distinguishes code201
            // from refresh201; default oauth2's temporary200 bridge is not used.
            let mut output = oauth2::http::Response::builder().status(status);
            *output.headers_mut().expect("valid response builder") = headers;
            Ok(output.body(bytes)?)
        })
    }
    #[cfg(feature = "gateway-source-endpoints")]
    fn source_metadata_urls(
        &self,
        request: rmcp::transport::auth::source_metadata::SourceMetadataUrlsRequest,
    ) -> rmcp::transport::auth::source_metadata::SourceMetadataUrlsFuture<'_> {
        Box::pin(async move { super::source_discovery::source_metadata_urls(request) })
    }
    #[cfg(feature = "gateway-source-endpoints")]
    fn source_expected_issuer(
        &self,
        base: Arc<str>,
        selected: Option<rmcp::transport::auth::source_metadata::ModelHttpUrl>,
    ) -> rmcp::transport::auth::source_metadata::SourceExpectedIssuerFuture<'_> {
        Box::pin(async move { super::source_discovery::source_expected_issuer(base, selected) })
    }
    #[cfg(feature = "gateway-source-endpoints")]
    fn source_metadata<'a>(
        &'a self,
        kind: rmcp::transport::auth::source_metadata::SourceMetadataKind,
        body: &'a [u8],
    ) -> rmcp::transport::auth::source_metadata::SourceMetadataFuture<'a> {
        Box::pin(async move {
            Ok(
                rmcp::transport::auth::source_metadata::SourceMetadataProjection::parse(kind, body)
                    .ok(),
            )
        })
    }
    fn execute(&self, operation: OAuthHttpRequest) -> OAuthHttpClientFuture<'_> {
        Box::pin(async move {
            let (parts, body) = operation.request.into_parts();
            let token_exchange = parts.method == reqwest::Method::POST
                && url::form_urlencoded::parse(&body).any(|(key, value)| {
                    key == "grant_type"
                        && matches!(value.as_ref(), "authorization_code" | "refresh_token")
                });
            let registration = parts.method == reqwest::Method::POST
                && serde_json::from_slice::<serde_json::Value>(&body)
                    .ok()
                    .is_some_and(|v| v.get("redirect_uris").is_some());
            let method = parts.method;
            let url = reqwest::Url::parse(&parts.uri.to_string())?;
            let mut headers = parts.headers;
            let mut body = body;
            if token_exchange {
                body = self
                    .policy
                    .lock()
                    .await
                    .token_request(&body, &mut headers)?;
            }
            if registration {
                let mut value: serde_json::Value = serde_json::from_slice(&body)?;
                value
                    .as_object_mut()
                    .ok_or("OAuth registration shape invalid")?
                    .remove("application_type");
                body = serde_json::to_vec(&value)?;
            }
            let refresh = token_exchange
                && url::form_urlencoded::parse(&body)
                    .any(|(key, value)| key == "grant_type" && value == "refresh_token");
            if refresh {
                self.policy.lock().await.refresh_fallback = false;
            }

            let max_redirects = match operation.redirect_policy {
                OAuthHttpRedirectPolicy::Stop => 0,
                OAuthHttpRedirectPolicy::SourceWithinOrigin => 5,
                OAuthHttpRedirectPolicy::Follow => {
                    return Err("gateway source OAuth redirect profile was not selected".into());
                }
                _ => return Err("unsupported OAuth redirect policy".into()),
            };
            if method == reqwest::Method::GET {
                headers.insert(
                    "mcp-protocol-version",
                    reqwest::header::HeaderValue::from_static("2026-07-28"),
                );
            }
            let mut response = self
                .network
                .send_built_request(
                    method.clone(),
                    url.to_string(),
                    headers,
                    body,
                    max_redirects,
                )
                .await?;
            let status = response.status();
            let headers = response.headers().clone();
            let mut bytes = Vec::new();
            while let Some(chunk) = self.network.chunk(&mut response).await? {
                if chunk.len() > super::token_json::MAX_BYTES.saturating_sub(bytes.len()) {
                    return Err(std::io::Error::other(
                        "OAuth response exceeds source token document profile",
                    )
                    .into());
                }
                bytes.extend_from_slice(&chunk)
            }
            if refresh && status != reqwest::StatusCode::OK {
                self.policy.lock().await.refresh_fallback = true;
            }
            // Source registration and code exchange accept200/201; refresh
            // requires200, independently of whether another2xx body validates.
            // Drain is complete and refresh_fallback was set above before return.
            if registration
                && !matches!(
                    status,
                    reqwest::StatusCode::OK | reqwest::StatusCode::CREATED
                )
            {
                return Err("OAuth registration HTTP status rejected".into());
            }
            if token_exchange
                && (if refresh {
                    status != reqwest::StatusCode::OK
                } else {
                    !matches!(
                        status,
                        reqwest::StatusCode::OK | reqwest::StatusCode::CREATED
                    )
                })
            {
                return Err("OAuth token HTTP status rejected".into());
            }
            if method == reqwest::Method::GET
                && status.is_success()
                && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
            {
                self.policy.lock().await.metadata(&value)?;
            }
            if token_exchange && status.is_success() {
                bytes = self.finish_token(&bytes, refresh, false).await?;
            }
            if registration && status.is_success() {
                *self.registration.lock().await = Some(bytes.clone());
            }
            // oauth2's response codec requires200. The actual source status was
            // already admitted and its canonical model retained above: bridge only
            // authorization-code201 in this temporary SDK-facing response. Never
            // normalize refresh201 or change the underlying network/status policy.
            let sdk_status = if token_exchange && !refresh && status == reqwest::StatusCode::CREATED
            {
                reqwest::StatusCode::OK
            } else {
                status
            };
            let mut output = oauth2::http::Response::builder().status(sdk_status);
            *output.headers_mut().expect("valid response builder") = headers;
            Ok(output.body(bytes)?)
        })
    }
}
pub struct OAuth {
    // Observe the already-bound real listener only for the owned loopback test.
    #[cfg(all(test, unix, feature = "gateway-source-endpoints"))]
    callback_test_address: std::sync::Mutex<Option<std::net::SocketAddr>>,
    manager: Mutex<AuthorizationManager>,
    policy: Arc<Mutex<Policy>>,
    registration: Arc<Mutex<Option<Vec<u8>>>>,
    configured: Mutex<bool>,
    store: SourceStore,
    callback: Arc<Mutex<Option<LoopbackCallback>>>,
    browser: Arc<BrowserOwner>,
    port: String,
    no_browser: bool,
    redirect: String,
    cancel: CancellationToken,
}
impl OAuth {
    pub async fn create(
        settings: &Settings,
        io: Arc<IoOwner>,
        cancel: CancellationToken,
        network: Arc<Network>,
        callback: Arc<Mutex<Option<LoopbackCallback>>>,
        browser: Arc<BrowserOwner>,
    ) -> Result<Arc<Self>> {
        let environment = [
            "HOME",
            "XDG_CONFIG_HOME",
            "USERPROFILE",
            "HOMEDRIVE",
            "HOMEPATH",
        ]
        .into_iter()
        .filter_map(|key| std::env::var_os(key).map(|value| (key.to_owned(), value)))
        .collect();
        let path = resolved_token_path(settings, &environment)
            .ok_or("gateway home directory unavailable")?;
        let file = Arc::new(
            FileTokenStorage::load(path, io, Instant::now() + AUTH_BUDGET, &cancel)
                .await
                .map_err(|_| "gateway token storage failed")?,
        );
        let store = SourceStore {
            file,
            cancel: cancel.clone(),
            expires: Default::default(),
            pending: Default::default(),
            cache: Default::default(),
        };
        #[cfg(feature = "gateway-source-endpoints")]
        let mut policy = Policy::new_source(&settings.remote_url);
        #[cfg(not(feature = "gateway-source-endpoints"))]
        let mut policy = Policy::new(&settings.remote_url)?;
        // MCP2.3 _initialize validates tokens before client_info. Both getters
        // read this instance's one FileTokenStorage snapshot, never disk again.
        #[cfg(feature = "gateway-source-endpoints")]
        let _ = store
            .load()
            .await
            .map_err(|_| "OAuth stored tokens invalid")?;
        if let Some(model) = store
            .file
            .get_client_info(store.deadline(), &cancel)
            .await
            .map_err(|_| "OAuth stored client invalid")?
        {
            #[cfg(feature = "gateway-source-endpoints")]
            {
                policy.client = Some(source_stored_client(model.json(), store.deadline()).await?);
            }
            #[cfg(not(feature = "gateway-source-endpoints"))]
            {
                policy.client = Some(Client::parse(model.json())?);
            }
        }
        // Source _initialize reads once; later file edits cannot change this instance.
        #[cfg(not(feature = "gateway-source-endpoints"))]
        let _ = store
            .load()
            .await
            .map_err(|_| "OAuth stored tokens invalid")?;
        let policy = Arc::new(Mutex::new(policy));
        let registration = Arc::new(Mutex::new(None));
        let http = Arc::new(Http {
            cache: store.cache.clone(),
            policy: policy.clone(),
            network,
            pending: store.pending.clone(),
            registration: registration.clone(),
        });
        #[cfg(feature = "gateway-source-endpoints")]
        let mut manager = AuthorizationManager::new_source_with_oauth_http_client(
            Arc::from(settings.remote_url.as_str()),
            http,
        )
        .await
        .map_err(|_| "OAuth manager initialization failed")?;
        #[cfg(not(feature = "gateway-source-endpoints"))]
        let mut manager =
            AuthorizationManager::new_with_oauth_http_client(settings.remote_url.clone(), http)
                .await
                .map_err(|_| "OAuth manager initialization failed")?;
        manager.use_source_within_origin_redirects();
        manager.set_credential_store(store.clone());
        let redirect = format!("http://127.0.0.1:{}/callback", settings.callback_port);
        Ok(Arc::new(Self {
            #[cfg(all(test, unix, feature = "gateway-source-endpoints"))]
            callback_test_address: std::sync::Mutex::new(None),
            manager: Mutex::new(manager),
            policy,
            registration,
            configured: Mutex::new(false),
            store,
            callback,
            browser,
            port: settings.callback_port.clone(),
            no_browser: settings.no_browser,
            redirect,
            cancel,
        }))
    }
    /// Attach cached tokens before the first request, without discovery/registration.
    /// A source401 (or scope step-up403) drives authorization lazily.
    pub async fn cached_access_token(&self) -> Result<Option<String>> {
        #[cfg(feature = "gateway-source-endpoints")]
        let mut manager = self.manager.lock().await;
        #[cfg(not(feature = "gateway-source-endpoints"))]
        let manager = self.manager.lock().await;
        if self
            .store
            .expires
            .lock()
            .await
            .is_some_and(|expiry| wall_time().is_ok_and(|now| now > expiry))
        {
            if !*self.configured.lock().await {
                return Ok(None);
            }
            let can_refresh = self
                .store
                .load()
                .await
                .map_err(|_| "OAuth refresh state invalid")?
                .and_then(|v| v.token_response)
                .and_then(|v| v.refresh_token().map(|v| !v.secret().is_empty()))
                .unwrap_or(false);
            if !can_refresh {
                return Ok(None);
            }
            #[cfg(feature = "gateway-source-endpoints")]
            self.sync_source_client(&mut manager).await?;
            match manager.refresh_token().await {
                Ok(tokens) => return Ok(Some(tokens.access_token().secret().to_owned())),
                Err(AuthError::AuthorizationRequired | AuthError::TokenRefreshRejected(_)) => {
                    self.store
                        .clear()
                        .await
                        .map_err(|_| "OAuth token clear failed")?;
                    return Ok(None);
                }
                #[cfg(not(feature = "gateway-source-endpoints"))]
                Err(_) if self.policy.lock().await.refresh_fallback => {
                    self.store
                        .clear()
                        .await
                        .map_err(|_| "OAuth token clear failed")?;
                    return Ok(None);
                }
                Err(_) => return Err("OAuth refresh transport or persistence failed".into()),
            }
        }
        match manager.get_access_token().await {
            Ok(token) if !token.is_empty() => Ok(Some(token)),
            Ok(_) | Err(AuthError::AuthorizationRequired) => Ok(None),
            Err(_) => Err("OAuth cached token retrieval failed".into()),
        }
    }
    pub async fn request_protocol(&self, protocol: Option<&str>) {
        #[cfg(feature = "gateway-source-endpoints")]
        let _manager = self.manager.lock().await;
        self.policy.lock().await.protocol(protocol);
    }
    pub fn scope_challenge(header: Option<&str>, base: &str) -> bool {
        #[cfg(feature = "gateway-source-endpoints")]
        {
            let _ = base;
            header.and_then(|header| {
                super::source_discovery::source_challenge_field(header, "error=")
            }) == Some("insufficient_scope")
        }
        #[cfg(not(feature = "gateway-source-endpoints"))]
        let Ok(base) = url::Url::parse(base) else {
            return false;
        };
        #[cfg(not(feature = "gateway-source-endpoints"))]
        {
            header.is_some_and(|h| {
                rmcp::transport::auth::WWWAuthenticateParams::parse(h, &base)
                    .is_insufficient_scope()
            })
        }
    }
    pub async fn authorize(&self, challenge: Option<&str>, step_up: bool) -> Result<String> {
        let mut manager = self.manager.lock().await;
        let granted = self
            .store
            .load()
            .await
            .map_err(|_| "OAuth scope load failed")?
            .and_then(|v| v.token_response)
            .and_then(|token| {
                token.scopes().map(|scopes| {
                    scopes
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
            });
        if !step_up || !*self.configured.lock().await {
            #[cfg(feature = "gateway-source-endpoints")]
            self.prepare_source_client(&mut manager, challenge, step_up, granted.as_deref())
                .await?;
            #[cfg(not(feature = "gateway-source-endpoints"))]
            {
                let resolution = manager
                    .resolve_metadata_from_challenge(challenge)
                    .await
                    .map_err(|_| "OAuth challenge discovery failed")?;
                let mut metadata: AuthorizationMetadata = resolution.metadata;
                let (registration_target, registration_binds_issuer) = {
                    let policy = self.policy.lock().await;
                    super::oauth_policy::registration_target(
                        &policy.resource,
                        &policy.expected_issuer,
                        metadata.registration_endpoint.as_deref(),
                        metadata.issuer.is_some(),
                    )?
                };
                metadata.registration_endpoint = Some(registration_target);
                manager.set_metadata(metadata.clone());
                let mut policy = self.policy.lock().await;
                if policy
                    .client
                    .as_ref()
                    .and_then(|client| client.issuer.as_deref())
                    .is_some_and(|issuer| {
                        !super::oauth_policy::issuer_matches(issuer, &policy.expected_issuer)
                    })
                {
                    policy.client = None;
                    self.store
                        .clear()
                        .await
                        .map_err(|_| "OAuth old issuer token retirement failed")?;
                }
                let base = url::Url::parse(&policy.resource)?;
                let hint = challenge.and_then(|h| {
                    rmcp::transport::auth::WWWAuthenticateParams::parse(h, &base).scope
                });
                let scopes = policy.select_scopes(hint.as_deref(), step_up, granted.as_deref());
                let existing = policy.client.clone();
                drop(policy);
                if existing.is_none() {
                    let refs: Vec<_> = scopes.iter().map(String::as_str).collect();
                    manager
                        .register_client("slopsearx-mcp gateway", &self.redirect, &refs)
                        .await
                        .map_err(|_| "OAuth registration failed")?;
                    let bytes = self
                        .registration
                        .lock()
                        .await
                        .take()
                        .ok_or("OAuth registration response missing")?;
                    let source = super::token_json::parse(&bytes, self.store.deadline())
                        .await
                        .map_err(|_| "OAuth registration malformed")?;
                    // Wire issuer is untrusted, and must be discarded BEFORE model
                    // validation (including nonstring values), not merely overwritten.
                    let source = without_registration_issuer(source);
                    let model = super::client_model::validate(Some(&source), self.store.deadline())
                        .await
                        .map_err(|_| "OAuth registration invalid")?
                        .ok_or("OAuth registration empty")?;
                    let mut record: std::collections::BTreeMap<
                        String,
                        Box<serde_json::value::RawValue>,
                    > = serde_json::from_slice(model.json())?;
                    if registration_binds_issuer {
                        record.insert(
                            "issuer".into(),
                            serde_json::value::RawValue::from_string(serde_json::to_string(
                                &self.policy.lock().await.expected_issuer,
                            )?)?,
                        );
                    }
                    let bytes = serde_json::to_vec(&record)?;
                    let client = Client::parse(&bytes)?;
                    client.usable_registration()?;
                    self.store
                        .file
                        .set_slot_json(
                            Slot::ClientInfo,
                            &bytes,
                            self.store.deadline(),
                            &self.cancel,
                        )
                        .await
                        .map_err(|_| "OAuth client persistence failed")?;
                    self.policy.lock().await.client = Some(client);
                }
                let client = self
                    .policy
                    .lock()
                    .await
                    .client
                    .clone()
                    .ok_or("OAuth client unavailable")?;
                // The adapter applies the minted source auth method; the SDK must
                // never independently attach a secret chosen from AS defaults.
                manager
                    .configure_client(OAuthClientConfig::new(
                        client.id.clone(),
                        self.redirect.clone(),
                    ))
                    .map_err(|_| "OAuth client configuration failed")?;
                let mut cache = self.store.cache.lock().await;
                let mut credentials = cache.as_ref().and_then(|v| v.clone()).unwrap_or_else(|| {
                    StoredCredentials::new(client.id.clone(), None, vec![], None)
                });
                credentials.client_id = client.id;
                credentials.issuer = client.issuer;
                *cache = Some(Some(credentials));
                *self.configured.lock().await = true;
            }
        } else {
            let mut policy = self.policy.lock().await;
            #[cfg(feature = "gateway-source-endpoints")]
            let hint = challenge
                .and_then(|h| super::source_discovery::source_challenge_field(h, "scope="))
                .map(str::to_owned);
            #[cfg(not(feature = "gateway-source-endpoints"))]
            let base = url::Url::parse(&policy.resource)?;
            #[cfg(not(feature = "gateway-source-endpoints"))]
            let hint = challenge
                .and_then(|h| rmcp::transport::auth::WWWAuthenticateParams::parse(h, &base).scope);
            policy.select_scopes(hint.as_deref(), true, granted.as_deref());
        }
        let scopes = self
            .policy
            .lock()
            .await
            .requested
            .clone()
            .unwrap_or_default();
        #[cfg(feature = "gateway-source-endpoints")]
        let refs = vec![scopes.as_str()];
        #[cfg(not(feature = "gateway-source-endpoints"))]
        let refs: Vec<_> = scopes.split_whitespace().collect();
        #[cfg(feature = "gateway-source-endpoints")]
        self.sync_source_client(&mut manager).await?;
        // Callback is bound BEFORE URL presentation/browser spawn; all actual tasks
        // remain in this owner until explicit shutdown (including startup failure).
        let mut callback = self.callback.lock().await;
        if let Some(previous) = callback.as_mut() {
            previous
                .shutdown()
                .await
                .map_err(|_| "OAuth callback retirement failed")?;
        }
        *callback = None;
        if callback.is_none() {
            *callback = Some(
                LoopbackCallback::start(&self.port, AUTH_BUDGET)
                    .await
                    .map_err(|_| "OAuth callback startup failed")?,
            )
        }
        #[cfg(all(test, unix, feature = "gateway-source-endpoints"))]
        {
            *self
                .callback_test_address
                .lock()
                .expect("test address observation") =
                callback.as_ref().map(LoopbackCallback::address);
        }
        let url = manager
            .get_authorization_url(&refs)
            .await
            .map_err(|_| "OAuth authorization URL failed")?;
        #[cfg(not(feature = "gateway-source-endpoints"))]
        let url = {
            let mut authorization = url::Url::parse(&url)?;
            let mut pairs: Vec<(String, String)> = authorization
                .query_pairs()
                .filter(|(k, _)| k != "resource")
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            if self.policy.lock().await.include_resource {
                pairs.push(("resource".into(), self.policy.lock().await.resource.clone()));
            }
            if scopes
                .split_whitespace()
                .any(|scope| scope == "offline_access")
            {
                pairs.retain(|(key, _)| key != "prompt");
                pairs.push(("prompt".into(), "consent".into()));
            }
            authorization.query_pairs_mut().clear().extend_pairs(pairs);
            authorization.to_string()
        };
        // Explicit human authorization URL presentation is original behavior,
        // distinct from ordinary redacted errors/logging. No URL enters telemetry.
        eprintln!(
            "\nAuthorize the SlopSearX gateway by visiting:\n  {url}\n(waiting for the redirect to {} …)\n",
            self.redirect
        );
        if !self.no_browser {
            let _ = self.browser.open(&url).await;
        }
        let code = tokio::select! {biased;_ = self.cancel.cancelled()=>return Err("gateway stopped".into()),v=callback.as_mut().expect("installed callback").wait()=>v.map_err(|_|"OAuth callback failed")?};
        let state = code
            .state
            .as_deref()
            .ok_or("OAuth callback state missing")?;
        #[cfg(feature = "gateway-source-endpoints")]
        self.sync_source_client(&mut manager).await?;
        let tokens = manager
            .exchange_code_for_token(&code.code, state)
            .await
            .map_err(|_| "OAuth authorization exchange failed")?;
        Ok(tokens.access_token().secret().to_owned())
    }
    #[cfg(feature = "gateway-source-endpoints")]
    async fn sync_source_client(&self, manager: &mut AuthorizationManager) -> Result<()> {
        let (client, resource, requested) = {
            let policy = self.policy.lock().await;
            (
                policy.client.clone().ok_or("OAuth client unavailable")?,
                policy
                    .include_resource
                    .then(|| Arc::<str>::from(policy.resource.as_str())),
                policy.requested.clone(),
            )
        };
        let mut config = OAuthClientConfig::new(client.id, self.redirect.clone());
        config.client_secret = client.secret;
        config.scopes = requested.into_iter().collect();
        manager
            .configure_source_client(config, client.method, resource, client.issuer)
            .await
            .map_err(|_| "OAuth source client configuration failed")
            .map_err(Into::into)
    }
    #[cfg(feature = "gateway-source-endpoints")]
    async fn prepare_source_client(
        &self,
        manager: &mut AuthorizationManager,
        challenge: Option<&str>,
        step_up: bool,
        granted: Option<&str>,
    ) -> Result<()> {
        let base = manager
            .source_base()
            .cloned()
            .ok_or("OAuth source base missing")?;
        let gate = SourceGate {
            policy: self.policy.clone(),
            raw_base: base.clone(),
        };
        let resolution = manager
            .resolve_source_metadata_from_challenge(challenge.map(Arc::<str>::from), &gate)
            .await
            .map_err(|_| "OAuth source challenge discovery failed")?;
        let (existing, resource, requested) = {
            let mut policy = self.policy.lock().await;
            policy.expected_issuer = resolution.expected_issuer().to_owned();
            if resolution.prm().is_none() {
                policy.resource = super::source_discovery::source_resource_url(&base)?;
            }
            let hint = challenge
                .and_then(|h| super::source_discovery::source_challenge_field(h, "scope="));
            policy.select_scopes(hint, step_up, granted);
            (
                policy.client.clone(),
                policy
                    .include_resource
                    .then(|| Arc::<str>::from(policy.resource.as_str())),
                policy.requested.clone(),
            )
        };
        if existing.is_none() {
            let bound = if let Some(asm) = resolution.asm() {
                if asm.registration_endpoint().is_some() {
                    true
                } else {
                    let issuer =
                        reqwest::source_pool::source_raw_urlparse(resolution.expected_issuer())?;
                    let original = reqwest::source_pool::source_raw_urlparse(&base)?;
                    format!("{}://{}", issuer.scheme(), issuer.netloc())
                        == format!("{}://{}", original.scheme(), original.netloc())
                }
            } else {
                false
            };
            let credential_issuer = bound.then(|| resolution.expected_issuer().to_owned());
            let refs: Vec<_> = requested.as_deref().into_iter().collect();
            manager
                .register_source_client(
                    "slopsearx-mcp gateway",
                    &self.redirect,
                    &refs,
                    resource,
                    credential_issuer.clone(),
                )
                .await
                .map_err(|_| "OAuth source registration failed")?;
            // execute_source discarded wire issuer and validated this entire model.
            let bytes = self
                .registration
                .lock()
                .await
                .take()
                .ok_or("OAuth source registration model missing")?;
            use ssx_source_json::{Text, Value};
            let mut record = match super::token_json::parse_http(&bytes, self.store.deadline())
                .await
                .map_err(|_| "OAuth canonical registration invalid")?
            {
                Value::Object(record) => record,
                _ => return Err("OAuth canonical registration shape invalid".into()),
            };
            if let Some(issuer) = credential_issuer {
                record.insert(Text::from("issuer"), Value::String(Text::from(issuer)));
            }
            let record = Value::Object(record);
            let client = Client::from_source_record(&record)?;
            let bytes = super::token_json::encode(&record, self.store.deadline())
                .await
                .map_err(|_| "OAuth canonical registration encoding failed")?;
            client.usable_registration()?;
            self.store
                .file
                .set_slot_json(
                    Slot::ClientInfo,
                    &bytes,
                    self.store.deadline(),
                    &self.cancel,
                )
                .await
                .map_err(|_| "OAuth source client persistence failed")?;
            self.policy.lock().await.client = Some(client);
        }
        self.sync_source_client(manager).await?;
        let client = self
            .policy
            .lock()
            .await
            .client
            .clone()
            .ok_or("OAuth source client unavailable")?;
        let mut cache = self.store.cache.lock().await;
        let mut credentials = cache
            .as_ref()
            .and_then(|v| v.clone())
            .unwrap_or_else(|| StoredCredentials::new(client.id.clone(), None, vec![], None));
        credentials.client_id = client.id;
        credentials.issuer = client.issuer;
        *cache = Some(Some(credentials));
        *self.configured.lock().await = true;
        Ok(())
    }
    pub async fn shutdown(&self) -> Result<()> {
        self.cancel.cancel();
        let mut callback = self.callback.lock().await;
        if let Some(callback) = callback.as_mut() {
            callback
                .shutdown()
                .await
                .map_err(|_| "OAuth callback shutdown failed")?
        }
        Ok(())
    }
}

fn without_registration_issuer(source: ssx_source_json::Value) -> ssx_source_json::Value {
    use ssx_source_json::{Map, Value};
    if let Value::Object(fields) = source {
        let mut clean = Map::new();
        for (key, value) in fields.iter() {
            if key.as_str() != Some("issuer") {
                clean.insert(key.clone(), value.clone());
            }
        }
        Value::Object(clean)
    } else {
        source
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_source_expiry_is_not_an_sdk_u64_schema() {
        for expiry in ["-1", "0", "18446744073709551617"] {
            let raw = format!(
                "{{\"access_token\":\"sample\",\"token_type\":\"Bearer\",\"expires_in\":{expiry},\"refresh_token\":null,\"scope\":null}}"
            );
            let sdk = sdk_token(raw.as_bytes()).unwrap();
            assert_eq!(sdk.access_token().secret(), "sample");
            assert!(sdk.expires_in().is_none());
            assert!(raw.contains(expiry));
        }
    }
    #[test]
    fn source_received_expiry_preserves_signed_and_float_overflow_outcomes() {
        assert!(source_expiry(br#"{"expires_in":-1000}"#).unwrap().unwrap() < wall_time().unwrap());
        assert!(source_expiry(br#"{"expires_in":null}"#).unwrap().is_none());
        assert!(
            source_expiry(format!("{{\"expires_in\":{}}}", "9".repeat(400)).as_bytes()).is_err()
        );
    }
}

#[cfg(test)]
mod registration_review_controls {
    use super::*;
    #[tokio::test]
    async fn wire_issuer_is_discarded_before_model_validation_for_any_json_type() {
        for issuer in [
            "17",
            r#""https://false-binding.test""#,
            "null",
            r#"{"untrusted":true}"#,
        ] {
            let raw = format!(
                "{{\"client_id\":\"id\",\"token_endpoint_auth_method\":\"none\",\"issuer\":{issuer}}}"
            );
            let deadline = Instant::now() + AUTH_BUDGET;
            let source = super::super::token_json::parse(raw.as_bytes(), deadline)
                .await
                .unwrap();
            let clean = without_registration_issuer(source);
            let model = super::super::client_model::validate(Some(&clean), deadline)
                .await
                .unwrap()
                .unwrap();
            assert!(Client::parse(model.json()).unwrap().issuer.is_none());
        }
    }
}

// Used while the caller holds OAuth.manager. The policy lock is acquired before
// computing a complete delta; assignment is the final, infallible action in the
// successful poll. This does not assert a transaction across independent locks.
#[cfg(feature = "gateway-source-endpoints")]
struct SourceGate {
    policy: Arc<Mutex<Policy>>,
    raw_base: Arc<str>,
}
#[cfg(feature = "gateway-source-endpoints")]
impl SourceGate {
    async fn apply(
        &self,
        projection: &rmcp::transport::auth::source_metadata::SourceMetadataProjection,
        base: &str,
        expected: Option<&str>,
        has_prm: bool,
    ) -> Result<()> {
        let mut policy = self.policy.lock().await;
        let next = policy.source_metadata_delta(projection, base, expected, has_prm)?;
        *policy = next;
        Ok(())
    }
}
#[cfg(feature = "gateway-source-endpoints")]
impl rmcp::transport::auth::source_metadata::SourceMetadataGate for SourceGate {
    fn check<'a>(
        &'a self,
        projection: &'a rmcp::transport::auth::source_metadata::SourceMetadataProjection,
    ) -> rmcp::transport::auth::source_metadata::SourceMetadataGateFuture<'a> {
        Box::pin(async move { self.apply(projection, &self.raw_base, None, false).await })
    }
    fn check_with_context<'a>(
        &'a self,
        projection: &'a rmcp::transport::auth::source_metadata::SourceMetadataProjection,
        context: &'a rmcp::transport::auth::source_metadata::SourceMetadataContext,
    ) -> rmcp::transport::auth::source_metadata::SourceMetadataGateFuture<'a> {
        Box::pin(async move {
            self.apply(
                projection,
                context.base(),
                context.expected_issuer(),
                context.has_prm(),
            )
            .await
        })
    }
    fn credentials_match<'a>(
        &'a self,
        credentials: &'a StoredCredentials,
        expected: &'a str,
    ) -> rmcp::transport::auth::source_metadata::SourceCredentialsFuture<'a> {
        Box::pin(async move {
            let matches = match credentials.issuer.as_deref() {
                None => true,
                Some(issuer) => super::oauth_policy::source_issuers_match(issuer, expected)?,
            };
            if !matches {
                let mut policy = self.policy.lock().await;
                policy.client = None;
                policy.asm_scopes = None;
            }
            // On false, the manager retires its client/ASM synchronously in this
            // same poll before awaiting the credential-store clear operation.
            Ok(matches)
        })
    }
}

#[cfg(all(test, feature = "gateway-source-endpoints"))]
mod source_gate_tests {
    use super::*;
    use rmcp::transport::auth::source_metadata::{
        SourceMetadataGate, SourceMetadataKind, SourceMetadataProjection,
    };
    use std::task::Poll;

    fn prm(resource: &str) -> SourceMetadataProjection {
        SourceMetadataProjection::parse(
            SourceMetadataKind::Prm,
            &serde_json::to_vec(&serde_json::json!({
                "resource": resource,
                "authorization_servers": ["https://as.test"],
                "scopes_supported": ["read"]
            }))
            .unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn source_gate_pending_cancel_and_error_cannot_change_prior_policy() {
        let policy = Arc::new(Mutex::new(
            Policy::new("https://resource.test/api/item").unwrap(),
        ));
        let gate = SourceGate {
            policy: policy.clone(),
            raw_base: "https://resource.test/api/item".into(),
        };
        let model = prm("https://resource.test/api");
        let guard = policy.lock().await;
        let mut checking = gate.check(&model);
        assert!(
            std::future::poll_fn(|cx| Poll::Ready(checking.as_mut().poll(cx).is_pending())).await
        );
        assert_eq!(guard.resource, "https://resource.test/api/item");
        assert_eq!(guard.expected_issuer, "https://resource.test");
        drop(checking);
        drop(guard);
        assert_eq!(
            policy.lock().await.resource,
            "https://resource.test/api/item"
        );
        assert!(gate.check(&prm("https://other.test/api")).await.is_err());
        assert_eq!(policy.lock().await.expected_issuer, "https://resource.test");
        gate.check(&model).await.unwrap();
        let accepted = policy.lock().await;
        assert_eq!(accepted.resource, "https://resource.test/api");
        assert_eq!(accepted.expected_issuer, "https://as.test");
        assert_eq!(
            accepted.prm_scopes.as_deref(),
            Some(&["read".to_owned()][..])
        );
    }

    #[tokio::test]
    async fn source_gate_credentials_keep_unbound_and_reject_other_issuer_url_shaped_id() {
        let gate = SourceGate {
            policy: Arc::new(Mutex::new(
                Policy::new("https://resource.test/mcp").unwrap(),
            )),
            raw_base: "https://resource.test/mcp".into(),
        };
        let mut credentials =
            StoredCredentials::new("https://client.test/document".into(), None, vec![], None);
        assert!(
            gate.credentials_match(&credentials, "https://as.test")
                .await
                .unwrap()
        );
        credentials.issuer = Some("https://other.test".into());
        assert!(
            !gate
                .credentials_match(&credentials, "https://as.test")
                .await
                .unwrap()
        );
        credentials.issuer = Some("https://as.test/".into());
        assert!(
            gate.credentials_match(&credentials, "https://as.test")
                .await
                .unwrap()
        );
    }
}

#[cfg(all(test, feature = "gateway-source-endpoints"))]
mod source_token_error_tests {
    use super::*;
    use rmcp::transport::auth::source_metadata::SourceOAuthModelInvalid;
    #[test]
    fn source_token_model_invalid_marker_excludes_limits_deadlines_and_default_lane() {
        for error in [
            super::super::token_json::JsonError::Malformed,
            super::super::token_json::JsonError::Encoding,
        ] {
            assert!(token_input_json_error(error, true).is::<SourceOAuthModelInvalid>());
            assert!(!token_input_json_error(error, false).is::<SourceOAuthModelInvalid>());
        }
        for error in [
            super::super::token_json::JsonError::Limit,
            super::super::token_json::JsonError::Deadline,
        ] {
            assert!(!token_input_json_error(error, true).is::<SourceOAuthModelInvalid>());
        }
        assert!(
            token_input_model_error(super::super::token_model::ModelError::Validation, true)
                .is::<SourceOAuthModelInvalid>()
        );
        for error in [
            super::super::token_model::ModelError::Resource,
            super::super::token_model::ModelError::Deadline,
        ] {
            assert!(!token_input_model_error(error, true).is::<SourceOAuthModelInvalid>());
        }
    }
}

#[cfg(all(test, feature = "gateway-source-endpoints"))]
mod source_scope_roundtrip_tests {
    use super::*;
    use rmcp::transport::auth::source_metadata::SourceOAuthModelInvalid;
    #[tokio::test]
    async fn source_http_huge_known_expiry_is_valid_model_then_operational_overflow() {
        let deadline = Instant::now() + Duration::from_secs(10);
        let nested = format!(
            r#"{{"access_token":"ok","ignored":{}0{}}}"#,
            "[".repeat(250),
            "]".repeat(250)
        );
        let error = super::super::token_json::parse_http(nested.as_bytes(), deadline)
            .await
            .unwrap_err();
        assert!(token_input_json_error(error, true).is::<SourceOAuthModelInvalid>());
        // Core2.46.4 uses the same Jiter0.14 parser: its4300-digit
        // admission error is model-invalid, unlike finite expiry overflow.
        for field in ["expires_in", "ignored"] {
            let rejected = format!(
                r#"{{"access_token":"ok","token_type":"Bearer","{field}":{}}}"#,
                "9".repeat(5000)
            );
            let error = super::super::token_json::parse_http(rejected.as_bytes(), deadline)
                .await
                .unwrap_err();
            assert!(token_input_json_error(error, true).is::<SourceOAuthModelInvalid>());
        }
        let raw = format!(
            r#"{{"access_token":"ok","token_type":"Bearer","expires_in":{}}}"#,
            "9".repeat(4000)
        );
        let parsed = super::super::token_json::parse_http(raw.as_bytes(), deadline)
            .await
            .unwrap();
        let model = super::super::token_model::validate(Some(&parsed), deadline)
            .await
            .unwrap()
            .unwrap();
        let reparsed = super::super::token_json::parse_http(model.json(), deadline)
            .await
            .unwrap();
        match reparsed.get("expires_in").unwrap() {
            ssx_source_json::Value::Number(number) => {
                assert_eq!(number.integer_text().unwrap().len(), 4000)
            }
            _ => panic!("known HTTP integer was not retained"),
        }
        let error = source_expiry(model.json()).unwrap_err();
        assert!(!error.is::<SourceOAuthModelInvalid>());
    }
    #[test]
    fn cached_sdk_refresh_scope_reconstruction_preserves_exact_original_text() {
        for scope in [
            "read  write",
            " read ",
            "",
            "read\twrite",
            "read\u{2003}write",
            "read\u{001c}write",
        ] {
            let raw = serde_json::to_vec(&serde_json::json!({
                "access_token": "a", "token_type": "Bearer", "scope": scope
            }))
            .unwrap();
            let token = sdk_token(&raw).unwrap();
            let restored = token
                .scopes()
                .unwrap()
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            assert_eq!(restored, scope);
        }
    }
}

#[cfg(all(test, unix, feature = "gateway-source-endpoints"))]
mod source_oauth_wire_store_tests {
    use super::*;
    use base64::Engine as _;
    use rmcp::transport::auth::{InMemoryStateStore, StateStore, StoredAuthorizationState};
    use std::{collections::BTreeMap, time::Duration};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };

    fn require(ok: bool, reason: &'static str) -> Result<()> {
        if ok { Ok(()) } else { Err(reason.into()) }
    }
    struct CallbackStateStore {
        inner: InMemoryStateStore,
        owner: std::sync::Weak<OAuth>,
        scopes: Arc<Mutex<Vec<Vec<String>>>>,
    }
    #[async_trait]
    impl StateStore for CallbackStateStore {
        async fn save(
            &self,
            csrf: &str,
            state: StoredAuthorizationState,
        ) -> std::result::Result<(), AuthError> {
            self.scopes
                .lock()
                .await
                .push(state.requested_scopes.clone());
            self.inner.save(csrf, state).await?;
            // Authorize bound its real listener BEFORE calling state.save. Send
            // an actual HTTP callback, without replacing the callback handler.
            let owner = self
                .owner
                .upgrade()
                .ok_or_else(|| AuthError::InternalError("fixture owner retired".into()))?;
            let address = owner
                .callback_test_address
                .lock()
                .map_err(|_| AuthError::InternalError("fixture observation poisoned".into()))?
                .ok_or_else(|| {
                    AuthError::InternalError("real callback not bound before state.save".into())
                })?;
            let mut stream = TcpStream::connect(address)
                .await
                .map_err(|e| AuthError::InternalError(e.to_string()))?;
            stream.write_all(format!("GET /callback?code=a%20~%2A%2B&state={csrf}&iss=https%3A%2F%2Fignored.test HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").as_bytes()).await.map_err(|e| AuthError::InternalError(e.to_string()))?;
            let mut reply = Vec::new();
            stream
                .read_to_end(&mut reply)
                .await
                .map_err(|e| AuthError::InternalError(e.to_string()))?;
            if !reply.starts_with(b"HTTP/1.1 200") {
                return Err(AuthError::InternalError("fixture callback failed".into()));
            }
            Ok(())
        }
        async fn load(
            &self,
            csrf: &str,
        ) -> std::result::Result<Option<StoredAuthorizationState>, AuthError> {
            self.inner.load(csrf).await
        }
        async fn delete(&self, csrf: &str) -> std::result::Result<(), AuthError> {
            self.inner.delete(csrf).await
        }
    }
    struct Wire {
        line: String,
        headers: BTreeMap<String, String>,
        body: Vec<u8>,
    }
    async fn read_request(stream: &mut TcpStream) -> Result<Wire> {
        let mut head = Vec::new();
        while !head.ends_with(b"\r\n\r\n") {
            let mut one = [0];
            require(stream.read(&mut one).await? == 1, "fixture request EOF")?;
            head.push(one[0]);
            require(head.len() <= 16384, "fixture header cap")?;
        }
        let head = String::from_utf8(head)?;
        let mut lines = head.split("\r\n");
        let line = lines
            .next()
            .ok_or("fixture request line absent")?
            .to_owned();
        let headers: BTreeMap<_, _> = lines
            .filter_map(|s| s.split_once(':'))
            .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
            .collect();
        let n = headers
            .get("content-length")
            .map_or(Ok(0), |s| s.parse::<usize>())?;
        require(n <= 16384, "fixture body cap")?;
        let mut body = vec![0; n];
        stream.read_exact(&mut body).await?;
        Ok(Wire {
            line,
            headers,
            body,
        })
    }
    async fn server(
        listener: TcpListener,
        stop: CancellationToken,
        port: u16,
    ) -> Result<Vec<Wire>> {
        let mut wires = Vec::new();
        loop {
            let next = tokio::select! { biased; _ = stop.cancelled() => break, next = listener.accept() => next };
            let (mut stream, _) = next?;
            let wire =
                tokio::time::timeout(Duration::from_secs(5), read_request(&mut stream)).await??;
            let (status, body) = match wires.len() {
                0 => ("200 OK", format!(r#"{{"resource":"http://oauth.fixture:{port}/mcp","authorization_servers":["http://AUTH.fixture:{port}/issuer"],"scopes_supported":["fallback"]}}"#)),
                1 => ("200 OK", format!(r#"{{"issuer":"http://auth.fixture:{port}/issuer","authorization_endpoint":"http://AUTH.fixture:{port}/authorize?original=~*#fragment","token_endpoint":"http://AUTH.fixture:{port}/token?raw=~*","registration_endpoint":"http://AUTH.fixture:{port}/register?raw=~*","scopes_supported":["offline_access"]}}"#)),
                // Non-string wire issuer MUST be discarded before model parsing.
                2 => ("201 Created", r#"{"client_id":"client ~*","client_secret":"s+e cret*~","token_endpoint_auth_method":"client_secret_basic","issuer":{"untrusted":true},"ignored":NaN,"jwks":NaN}"#.into()),
                3 => ("201 Created", format!(r#"{{"access_token":"code-token","token_type":"bearer","refresh_token":"refresh ~*+","expires_in":-1,"ignored":{}}}"#, "9".repeat(4000))),
                4 => ("200 OK", r#"{"access_token":"refresh-token","token_type":"bearer","expires_in":-1}"#.into()),
                5 => ("201 Created", r#"{"access_token":"must-not-commit","token_type":"bearer"}"#.into()),
                _ => ("500 Unexpected", "{}".into()),
            };
            stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await?;
            stream.shutdown().await?;
            wires.push(wire);
        }
        Ok(wires)
    }
    async fn token_file_without_any(bytes: &[u8]) -> Result<serde_json::Value> {
        let value =
            super::super::token_json::parse(bytes, Instant::now() + Duration::from_secs(10))
                .await
                .map_err(|_| "fixture file parse failed")?;
        let tokens = value.get("tokens").ok_or("fixture tokens absent")?;
        let client = value.get("client_info").ok_or("fixture client absent")?;
        // These asserted scalar fields are independent of Any-valued jwks.
        Ok(
            serde_json::json!({"tokens": serde_json::from_slice::<serde_json::Value>(&super::super::token_json::encode(tokens, Instant::now() + Duration::from_secs(10)).await.map_err(|_| "fixture token encoding failed")?)?,
            "client_info": {"client_id": client.get("client_id").and_then(|v|v.as_str()), "issuer": client.get("issuer").and_then(|v|v.as_str())}}),
        )
    }
    fn check_token(value: &serde_json::Value, access: &str) -> Result<()> {
        require(
            value["tokens"]["access_token"] == access,
            "wrong persisted access token",
        )?;
        require(
            value["tokens"]["refresh_token"] == "refresh ~*+",
            "omitted refresh token lost prior token",
        )?;
        require(
            value["tokens"]["scope"] == "read  write offline_access",
            "canonical source scope spacing changed",
        )?;
        require(
            value["tokens"]["expires_in"] == -1,
            "canonical signed expiry lost in SDK bridge",
        )
    }
    #[tokio::test]
    async fn source_oauth_real_loopback_discovery_registration_code_refresh_and_store() {
        let directory =
            std::env::temp_dir().join(format!("ssx-t07-oauth-wire-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("tokens.json");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = CancellationToken::new();
        let server_stop = stop.clone();
        let mut server_task = tokio::spawn(server(listener, server_stop, port));
        let network = super::super::network::oauth_loopback_test_network();
        let io = Arc::new(IoOwner::new());
        let browser = Arc::new(BrowserOwner::default());
        let callback = Arc::new(Mutex::new(None));
        let oauth_stop = CancellationToken::new();
        let settings = Settings {
            remote_url: format!("http://OAuth.fixture:{port}/mcp?original=~*#fragment"),
            token: None,
            oauth: true,
            callback_port: "0".into(),
            no_browser: true,
            token_file: Some(path.clone()),
        };
        let scope_observations = Arc::new(Mutex::new(Vec::new()));
        let mut owner = None;
        let journey = async {
            let oauth = OAuth::create(
                &settings,
                io.clone(),
                oauth_stop.clone(),
                network.clone(),
                callback.clone(),
                browser.clone(),
            )
            .await?;
            owner = Some(oauth.clone());
            oauth
                .manager
                .lock()
                .await
                .set_state_store(CallbackStateStore {
                    inner: InMemoryStateStore::new(),
                    owner: Arc::downgrade(&oauth),
                    scopes: scope_observations.clone(),
                });
            require(
                oauth.cached_access_token().await?.is_none(),
                "empty store unexpectedly provided token",
            )?;
            // Real response-protocol observation shares the source manager guard.
            oauth.request_protocol(Some("2026-07-28")).await;
            let challenge = format!(
                "Bearer resource_metadata=\"http://OAuth.fixture:{port}/prm;keep?raw=~*\", scope=\"read  write\""
            );
            require(
                oauth.authorize(Some(&challenge), false).await? == "code-token",
                "code201 was rejected",
            )?;
            let first_bytes = std::fs::read(&path)?;
            let first_source = super::super::token_json::parse(
                &first_bytes,
                Instant::now() + Duration::from_secs(10),
            )
            .await
            .map_err(|_| "fixture source document invalid")?;
            let first = token_file_without_any(&first_bytes).await?;
            check_token(&first, "code-token")?;
            require(
                first_source
                    .get("client_info")
                    .and_then(|v| v.get("jwks"))
                    .and_then(|v| v.as_f64())
                    .is_some_and(f64::is_nan),
                "known Any jwks NaN was discarded",
            )?;
            require(
                first["client_info"]["issuer"] == format!("http://auth.fixture:{port}/issuer"),
                "wire issuer was trusted or discovered binding lost",
            )?;
            require(
                first["client_info"]["client_id"] == "client ~*",
                "registered client ID changed",
            )?;
            require(
                oauth.cached_access_token().await?.as_deref() == Some("refresh-token"),
                "refresh200 failed",
            )?;
            let second = std::fs::read(&path)?;
            check_token(&token_file_without_any(&second).await?, "refresh-token")?;
            require(
                oauth.cached_access_token().await?.is_none(),
                "refresh201 was accepted",
            )?;
            require(
                std::fs::read(&path)? == second,
                "rejected refresh changed durable source token",
            )?;
            require(
                scope_observations.lock().await.as_slice()
                    == [vec!["read  write offline_access".to_owned()]],
                "source auth scopes were split or normalized",
            )?;
            use std::os::unix::fs::PermissionsExt;
            require(
                std::fs::metadata(&path)?.permissions().mode() & 0o777 == 0o600,
                "token file is not private",
            )?;
            Ok::<(), crate::Error>(())
        };
        let outcome = tokio::time::timeout(Duration::from_secs(20), journey).await;
        // Always close/join actual owners before assertions, even a failed client
        // or fixture response. This fixture does not exercise native DNS helpers.
        let oauth_cleanup = if let Some(oauth) = owner {
            oauth.shutdown().await
        } else {
            Ok(())
        };
        let browser_cleanup = browser.shutdown().await;
        let network_cleanup = network.shutdown().await;
        let io_cleanup = io.shutdown().await;
        stop.cancel();
        let joined: Result<Vec<Wire>> =
            match tokio::time::timeout(Duration::from_secs(6), &mut server_task).await {
                Ok(result) => result.map_err(crate::Error::from).and_then(|result| result),
                Err(error) => {
                    server_task.abort();
                    let _ = server_task.await;
                    Err(error.into())
                }
            };
        let removed = std::fs::remove_dir_all(&directory);
        oauth_cleanup.unwrap();
        browser_cleanup.unwrap();
        network_cleanup.unwrap();
        io_cleanup.unwrap();
        removed.unwrap();
        outcome.unwrap().unwrap();
        let wires = joined.unwrap();
        assert_eq!(wires.len(), 6, "unexpected OAuth wire operation count");
        assert_eq!(wires[0].line, "GET /prm;keep?raw=~* HTTP/1.1");
        assert_eq!(
            wires[1].line,
            "GET /.well-known/oauth-authorization-server/issuer HTTP/1.1"
        );
        assert_eq!(wires[2].line, "POST /register?raw=~* HTTP/1.1");
        for wire in &wires[3..] {
            assert_eq!(wire.line, "POST /token?raw=~* HTTP/1.1");
        }
        assert_eq!(wires[0].headers["host"], format!("oauth.fixture:{port}"));
        assert_eq!(wires[1].headers["mcp-protocol-version"], "2026-07-28");
        let registration: serde_json::Value = serde_json::from_slice(&wires[2].body).unwrap();
        assert_eq!(registration["application_type"], "native");
        assert_eq!(registration["scope"], "read  write offline_access");
        assert_eq!(
            registration["redirect_uris"][0],
            "http://127.0.0.1:0/callback"
        );
        let code = String::from_utf8(wires[3].body.clone()).unwrap();
        assert!(code.starts_with(
            "grant_type=authorization_code&code=a+~%2A%2B&redirect_uri=http%3A%2F%2F127.0.0.1%3A"
        ));
        assert!(code.contains("&client_id=client+~%2A&code_verifier="));
        assert!(code.ends_with(&format!(
            "&resource=http%3A%2F%2Foauth.fixture%3A{port}%2Fmcp"
        )));
        for wire in &wires[3..] {
            let basic = wire.headers["authorization"]
                .strip_prefix("Basic ")
                .unwrap();
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(basic)
                    .unwrap(),
                b"client%20~%2A:s%2Be%20cret%2A~"
            );
        }
        for wire in &wires[4..] {
            let body = String::from_utf8(wire.body.clone()).unwrap();
            assert_eq!(
                body,
                format!(
                    "grant_type=refresh_token&refresh_token=refresh+~%2A%2B&client_id=client+~%2A&resource=http%3A%2F%2Foauth.fixture%3A{port}%2Fmcp"
                )
            );
            assert!(!body.contains("scope="));
        }
    }
}
