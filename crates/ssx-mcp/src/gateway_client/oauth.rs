//! Native SDK OAuth composition with the original client_info/tokens disk schema.
//! No SDK default HTTP client or process-global auth/logging state.
use super::{
    browser::BrowserOwner,
    callback::LoopbackCallback,
    io_owner::IoOwner,
    network::Network,
    oauth_policy::{Client, Policy},
    settings::{Settings, default_token_path},
    token_storage::{FileTokenStorage, Slot},
};
use crate::Result;
use async_trait::async_trait;
use oauth2::TokenResponse;
use rmcp::transport::auth::{
    AuthError, AuthorizationManager, AuthorizationMetadata, CredentialStore, OAuthClientConfig,
    OAuthHttpClient, OAuthHttpClientFuture, OAuthHttpRedirectPolicy, OAuthHttpRequest,
    OAuthTokenResponse, StoredCredentials,
};
use std::{
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
const AUTH_BUDGET: Duration = Duration::from_secs(300);
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
        let client = self
            .file
            .get_client_info(self.deadline(), &self.cancel)
            .await
            .map_err(|_| store_error())?;
        let client = client
            .map(|client| Client::parse(client.json()))
            .transpose()
            .map_err(|_| store_error())?;
        let id = client
            .as_ref()
            .map(|client| client.id.clone())
            .unwrap_or_default();
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
impl OAuthHttpClient for Http {
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
                .send_built_request(method, url.to_string(), headers, body, max_redirects)
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
                let deadline = Instant::now() + AUTH_BUDGET;
                if refresh {
                    self.policy.lock().await.refresh_fallback = true;
                }
                let source = super::token_json::parse(&bytes, deadline)
                    .await
                    .map_err(|_| "OAuth token response malformed")?;
                let model = super::token_model::validate(Some(&source), deadline)
                    .await
                    .map_err(|_| "OAuth token response invalid")?
                    .ok_or("OAuth token response empty")?;
                let mut fields: std::collections::BTreeMap<
                    String,
                    Box<serde_json::value::RawValue>,
                > = serde_json::from_slice(model.json())?;
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
                            serde_json::value::RawValue::from_string(serde_json::to_string(
                                &scope,
                            )?)?,
                        );
                    }
                }
                let raw = serde_json::to_vec(&fields)?;
                let source = super::token_json::parse(&raw, deadline)
                    .await
                    .map_err(|_| "OAuth normalized token malformed")?;
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
                bytes = sdk_token_bytes(model.json())?;
                *self.pending.lock().await = Some(Pending { model, expires });
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
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let path = match &settings.token_file {
            Some(path) => path.clone(),
            None => default_token_path(
                &settings.remote_url,
                std::env::var("XDG_CONFIG_HOME").ok().as_deref(),
                home.as_deref()
                    .ok_or("gateway home directory unavailable")?,
            ),
        };
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
        let mut policy = Policy::new(&settings.remote_url)?;
        if let Some(model) = store
            .file
            .get_client_info(store.deadline(), &cancel)
            .await
            .map_err(|_| "OAuth stored client invalid")?
        {
            policy.client = Some(Client::parse(model.json())?);
        }
        // Source _initialize reads once; later file edits cannot change this instance.
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
        let mut manager =
            AuthorizationManager::new_with_oauth_http_client(settings.remote_url.clone(), http)
                .await
                .map_err(|_| "OAuth manager initialization failed")?;
        manager.use_source_within_origin_redirects();
        manager.set_credential_store(store.clone());
        let redirect = format!("http://127.0.0.1:{}/callback", settings.callback_port);
        Ok(Arc::new(Self {
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
            match manager.refresh_token().await {
                Ok(tokens) => return Ok(Some(tokens.access_token().secret().to_owned())),
                Err(AuthError::AuthorizationRequired | AuthError::TokenRefreshRejected(_)) => {
                    self.store
                        .clear()
                        .await
                        .map_err(|_| "OAuth token clear failed")?;
                    return Ok(None);
                }
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
        self.policy.lock().await.protocol(protocol);
    }
    pub fn scope_challenge(header: Option<&str>, base: &str) -> bool {
        let Ok(base) = url::Url::parse(base) else {
            return false;
        };
        header.is_some_and(|h| {
            rmcp::transport::auth::WWWAuthenticateParams::parse(h, &base).is_insufficient_scope()
        })
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
            let hint = challenge
                .and_then(|h| rmcp::transport::auth::WWWAuthenticateParams::parse(h, &base).scope);
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
            let mut credentials = cache
                .as_ref()
                .and_then(|v| v.clone())
                .unwrap_or_else(|| StoredCredentials::new(client.id.clone(), None, vec![], None));
            credentials.client_id = client.id;
            credentials.issuer = client.issuer;
            *cache = Some(Some(credentials));
            *self.configured.lock().await = true;
        } else {
            let mut policy = self.policy.lock().await;
            let base = url::Url::parse(&policy.resource)?;
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
        let refs: Vec<_> = scopes.split_whitespace().collect();
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
        let url = manager
            .get_authorization_url(&refs)
            .await
            .map_err(|_| "OAuth authorization URL failed")?;
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
        let url = authorization.to_string();
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
        let tokens = manager
            .exchange_code_for_token(&code.code, state)
            .await
            .map_err(|_| "OAuth authorization exchange failed")?;
        Ok(tokens.access_token().secret().to_owned())
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

fn without_registration_issuer(
    source: ssx_infra::adapters::source_json::Value,
) -> ssx_infra::adapters::source_json::Value {
    use ssx_infra::adapters::source_json::{Map, Value};
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
