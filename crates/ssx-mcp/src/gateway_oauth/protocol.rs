//! OAuth protocol operations used by the installed gateway's authorizer.

use oauth2::{TokenResponse, basic::BasicTokenType};
use rmcp::transport::auth::{
    AuthError, AuthorizationManager, CredentialStore, InMemoryCredentialStore, OAuthClientConfig,
    OAuthTokenResponse, StoredCredentials,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};
use url::Url;

#[derive(Debug, PartialEq, Eq)]
pub enum ProtocolError {
    Discovery,
    Endpoint,
    Client,
    Authorization,
    Exchange,
    Refresh,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Discovery => "Could not discover the OAuth authorization server",
            Self::Endpoint => "Unusable OAuth endpoint or changed authorization issuer",
            Self::Client => "Could not configure the OAuth client registration",
            Self::Authorization => "Could not start OAuth authorization",
            Self::Exchange => "Could not exchange the OAuth authorization code",
            Self::Refresh => "Could not refresh the OAuth access token",
        })
    }
}

impl Error for ProtocolError {}

#[derive(Deserialize, Serialize)]
pub struct RegisteredClient {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uris: Vec<String>,
    // Python token files created before installation have no issuer field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
}

pub struct BrowserRequest {
    pub url: String,
    pub state: String,
}

pub struct OAuthProtocol {
    manager: AuthorizationManager,
    credentials: InMemoryCredentialStore,
    issuer: Option<String>,
}

impl OAuthProtocol {
    /// MCP RPCs retain their exact-target transport. OAuth visits the discovery
    /// and authorization-server endpoints under RMCP's separate address,
    /// redirect, and metadata policies. Credential exchanges use its
    /// no-redirect client. Explicit cleartext endpoints stay on the resource origin.
    pub async fn discover(remote: &Url, challenge: Option<&str>) -> Result<Self, ProtocolError> {
        check_endpoint(remote.as_str(), remote)?;
        let credentials = InMemoryCredentialStore::new();
        let mut manager = AuthorizationManager::new(remote.clone())
            .await
            .map_err(|_| ProtocolError::Discovery)?;
        let resolution = manager
            .resolve_metadata_from_challenge(challenge)
            .await
            .map_err(|_| ProtocolError::Discovery)?;
        let metadata = resolution.metadata;
        check_endpoint(&metadata.authorization_endpoint, remote)?;
        check_endpoint(&metadata.token_endpoint, remote)?;
        if let Some(registration) = &metadata.registration_endpoint {
            check_endpoint(registration, remote)?;
        }
        if let Some(issuer) = &metadata.issuer {
            check_endpoint(issuer, remote)?;
        }
        let issuer = metadata.issuer.clone();
        manager.set_metadata(metadata);
        manager.set_credential_store(credentials.clone());
        Ok(Self {
            manager,
            credentials,
            issuer,
        })
    }

    pub async fn select_initial_scopes(&self) -> Vec<String> {
        self.manager.select_initial_scopes(&[]).await
    }

    pub fn select_scopes(&self, challenged: Option<&str>, granted: &[String]) -> Vec<String> {
        let defaults: Vec<_> = granted.iter().map(String::as_str).collect();
        let mut scopes = self.manager.select_scopes(challenged, &defaults);
        for scope in granted {
            if !scopes.contains(scope) {
                scopes.push(scope.clone());
            }
        }
        scopes
    }

    pub async fn register(
        &mut self,
        redirect_uri: &str,
        scopes: &[String],
    ) -> Result<RegisteredClient, ProtocolError> {
        let scopes: Vec<_> = scopes.iter().map(String::as_str).collect();
        let config = self
            .manager
            .register_client("slopsearx-mcp gateway", redirect_uri, &scopes)
            .await
            .map_err(|_| ProtocolError::Client)?;
        Ok(RegisteredClient {
            client_id: config.client_id,
            client_secret: config.client_secret,
            redirect_uris: vec![redirect_uri.into()],
            issuer: self.issuer.clone(),
        })
    }

    pub fn configure_client(
        &mut self,
        client: &RegisteredClient,
        redirect_uri: &str,
    ) -> Result<(), ProtocolError> {
        if client.issuer.is_some() && client.issuer != self.issuer {
            return Err(ProtocolError::Endpoint);
        }
        if client.client_id.is_empty()
            || !client.redirect_uris.iter().any(|uri| uri == redirect_uri)
        {
            return Err(ProtocolError::Client);
        }
        let mut config = OAuthClientConfig::new(client.client_id.clone(), redirect_uri);
        if let Some(secret) = client
            .client_secret
            .as_deref()
            .filter(|secret| !secret.is_empty())
        {
            config = config.with_client_secret(secret);
        }
        self.manager
            .configure_client(config)
            .map_err(|_| ProtocolError::Client)
    }

    pub async fn begin(&self, scopes: &[String]) -> Result<BrowserRequest, ProtocolError> {
        let scopes: Vec<_> = scopes.iter().map(String::as_str).collect();
        let url = self
            .manager
            .get_authorization_url(&scopes)
            .await
            .map_err(|_| ProtocolError::Authorization)?;
        let parameters = Url::parse(&url).map_err(|_| ProtocolError::Authorization)?;
        let mut states = parameters.query_pairs().filter(|(key, _)| key == "state");
        let (_, state) = states.next().ok_or(ProtocolError::Authorization)?;
        if state.is_empty() || states.next().is_some() {
            return Err(ProtocolError::Authorization);
        }
        Ok(BrowserRequest {
            url,
            state: state.into_owned(),
        })
    }

    pub async fn exchange(
        &self,
        code: &str,
        state: &str,
        issuer: Option<&str>,
    ) -> Result<OAuthTokenResponse, ProtocolError> {
        let tokens = self
            .manager
            .exchange_code_for_token_with_issuer(code, state, issuer)
            .await
            .map_err(|_| ProtocolError::Exchange)?;
        valid_bearer(&tokens).ok_or(ProtocolError::Exchange)?;
        Ok(tokens)
    }

    /// A missing or definitively rejected refresh token requires browser sign-in.
    /// Preserve credentials and surface retryable network errors to the caller.
    pub async fn refresh(
        &self,
        registered: &RegisteredClient,
        tokens: OAuthTokenResponse,
        granted: Vec<String>,
    ) -> Result<Option<OAuthTokenResponse>, ProtocolError> {
        if registered.issuer.is_some() && registered.issuer != self.issuer {
            return Err(ProtocolError::Endpoint);
        }
        if tokens.refresh_token().is_none() {
            return Ok(None);
        }
        self.credentials
            .save(
                StoredCredentials::new(registered.client_id.clone(), Some(tokens), granted, None)
                    .with_issuer(self.issuer.clone()),
            )
            .await
            .map_err(|_| ProtocolError::Refresh)?;
        match self.manager.refresh_token().await {
            Ok(tokens) => {
                valid_bearer(&tokens).ok_or(ProtocolError::Refresh)?;
                Ok(Some(tokens))
            }
            Err(AuthError::TokenRefreshRejected(_) | AuthError::AuthorizationRequired) => Ok(None),
            Err(_) => Err(ProtocolError::Refresh),
        }
    }

    pub fn issuer(&self) -> Option<&str> {
        self.issuer.as_deref()
    }

    pub async fn granted_scopes(&self) -> Vec<String> {
        self.manager.get_current_scopes().await
    }
}

pub fn valid_bearer(tokens: &OAuthTokenResponse) -> Option<&str> {
    let bearer = tokens.access_token().secret();
    (tokens.token_type() == &BasicTokenType::Bearer
        && !bearer.is_empty()
        && reqwest::header::HeaderValue::from_str(bearer).is_ok())
    .then_some(bearer)
}

fn check_endpoint(endpoint: &str, remote: &Url) -> Result<(), ProtocolError> {
    let endpoint = Url::parse(endpoint).map_err(|_| ProtocolError::Endpoint)?;
    let allowed_scheme = endpoint.scheme() == "https"
        || (endpoint.scheme() == "http" && endpoint.origin() == remote.origin());
    if !allowed_scheme
        || endpoint.host().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.fragment().is_some()
    {
        return Err(ProtocolError::Endpoint);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Form, Json, Router,
        extract::State,
        http::{HeaderMap, header},
        routing::{get, post},
    };
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::{
        collections::BTreeMap,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };
    use tokio_util::sync::CancellationToken;

    #[derive(Default)]
    struct Fixture {
        challenge: Mutex<String>,
        code_exchanges: AtomicUsize,
    }

    fn origin(headers: &HeaderMap) -> String {
        format!("http://{}", headers[header::HOST].to_str().unwrap())
    }

    #[tokio::test]
    async fn discovers_registers_exchanges_pkce_and_keeps_the_refresh_token() -> crate::Result<()> {
        let fixture = Arc::new(Fixture::default());
        let router = Router::new()
            .route(
                "/.well-known/oauth-protected-resource/mcp",
                get(|headers: HeaderMap| async move {
                    let origin = origin(&headers);
                    Json(json!({
                        "resource": format!("{origin}/mcp"),
                        "authorization_servers": [origin],
                        "scopes_supported": ["search"]
                    }))
                }),
            )
            .route(
                "/.well-known/oauth-authorization-server",
                get(|headers: HeaderMap| async move {
                    let origin = origin(&headers);
                    Json(json!({
                        "issuer": origin,
                        "authorization_endpoint": format!("{origin}/authorize"),
                        "token_endpoint": format!("{origin}/token"),
                        "registration_endpoint": format!("{origin}/register"),
                        "code_challenge_methods_supported": ["S256"],
                        "authorization_response_iss_parameter_supported": true,
                        "response_types_supported": ["code"]
                    }))
                }),
            )
            .route(
                "/register",
                post(|Json(body): Json<Value>| async move {
                    assert_eq!(body["token_endpoint_auth_method"], "none");
                    Json(json!({
                        "client_id": "registered",
                        "redirect_uris": body["redirect_uris"]
                    }))
                }),
            )
            .route(
                "/token",
                post(
                    |State(fixture): State<Arc<Fixture>>, headers: HeaderMap,
                     Form(body): Form<BTreeMap<String, String>>| async move {
                        assert_eq!(body["resource"], format!("{}/mcp", origin(&headers)));
                        assert_eq!(body["client_id"], "registered");
                        match body["grant_type"].as_str() {
                            "authorization_code" => {
                                fixture.code_exchanges.fetch_add(1, Ordering::Relaxed);
                                assert_eq!(body["code"], "fixture-code");
                                let pkce = URL_SAFE_NO_PAD.encode(Sha256::digest(body["code_verifier"].as_bytes()));
                                assert_eq!(*fixture.challenge.lock().unwrap(), pkce);
                                Json(json!({
                                    "access_token": "initial", "token_type": "Bearer",
                                    "refresh_token": "persistent", "scope": "search"
                                }))
                            }
                            "refresh_token" => {
                                assert_eq!(body["refresh_token"], "persistent");
                                Json(json!({"access_token":"renewed","token_type":"Bearer","scope":"search"}))
                            }
                            _ => panic!("unexpected grant type"),
                        }
                    },
                ),
            )
            .with_state(fixture.clone());
        let host = crate::harness::Host::start(router, CancellationToken::new()).await?;
        let origin = format!("http://{}", host.address);
        let remote = Url::parse(&format!("{origin}/mcp"))?;

        tokio::time::timeout(Duration::from_secs(8), async {
            let mut protocol = OAuthProtocol::discover(&remote, None).await?;
            assert_eq!(protocol.issuer(), Some(origin.as_str()));
            let scopes = protocol.select_initial_scopes().await;
            assert_eq!(scopes, ["search"]);
            let redirect = "http://127.0.0.1:8765/callback";
            let client = protocol.register(redirect, &scopes).await?;
            for issuer in [None, Some("https://wrong.test")] {
                let rejected = protocol.begin(&scopes).await?;
                assert!(matches!(
                    protocol
                        .exchange("fixture-code", &rejected.state, issuer)
                        .await,
                    Err(ProtocolError::Exchange)
                ));
            }
            assert_eq!(fixture.code_exchanges.load(Ordering::Relaxed), 0);
            let start = protocol.begin(&scopes).await?;
            let url = Url::parse(&start.url)?;
            let params: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
            assert_eq!(params["code_challenge_method"], "S256");
            assert_eq!(params["redirect_uri"], redirect);
            assert_eq!(params["scope"], "search");
            *fixture.challenge.lock().unwrap() = params["code_challenge"].clone();
            let tokens = protocol
                .exchange("fixture-code", &start.state, Some(&origin))
                .await?;
            assert_eq!(valid_bearer(&tokens), Some("initial"));
            assert_eq!(fixture.code_exchanges.load(Ordering::Relaxed), 1);
            let refreshed = protocol
                .refresh(&client, tokens, scopes.clone())
                .await?
                .unwrap();
            assert_eq!(valid_bearer(&refreshed), Some("renewed"));
            assert_eq!(refreshed.refresh_token().unwrap().secret(), "persistent");
            assert_eq!(protocol.granted_scopes().await, scopes);
            assert_eq!(
                protocol.select_scopes(Some("research"), &scopes),
                ["search", "research"]
            );
            Ok::<_, crate::Error>(())
        })
        .await??;
        host.shutdown().await?;
        Ok(())
    }

    #[test]
    fn endpoint_policy_rejects_cleartext_cross_origin_and_userinfo() {
        let resource = Url::parse("https://resource.test/mcp").unwrap();
        assert!(check_endpoint("https://issuer.test/token", &resource).is_ok());
        for endpoint in [
            "http://issuer.test/token",
            "https://identity:secret@issuer.test/token",
            "https://issuer.test/token#fragment",
        ] {
            assert_eq!(
                check_endpoint(endpoint, &resource),
                Err(ProtocolError::Endpoint)
            );
        }
        let loopback = Url::parse("http://127.0.0.1:1234/mcp").unwrap();
        assert!(check_endpoint("http://127.0.0.1:1234/token", &loopback).is_ok());
        assert_eq!(
            check_endpoint("http://127.0.0.1:2345/token", &loopback),
            Err(ProtocolError::Endpoint)
        );
    }
}
