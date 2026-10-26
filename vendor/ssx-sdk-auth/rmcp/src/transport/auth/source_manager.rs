//! Existing authorization manager's opt-in raw discovery path.
//! Derivation is a finite injected gateway operation, never a second URL parser.
use super::source_metadata::{
    ModelHttpUrl, SourceMetadataContext, SourceMetadataDecision, SourceMetadataGate,
    SourceMetadataGateFuture, SourceMetadataKind, SourceMetadataProjection,
    SourceMetadataResolution, SourceMetadataUrlsRequest, SourceOAuthHttpRequest,
    SourceOperationUnsupported,
};
use super::{
    AuthError, AuthorizationManager, ManagerBase, OAuthHttpClient, OAuthHttpRedirectPolicy,
};
use std::sync::Arc;
struct ContextGate<'a> {
    gate: &'a dyn SourceMetadataGate,
    context: SourceMetadataContext,
}
impl SourceMetadataGate for ContextGate<'_> {
    fn check<'a>(
        &'a self,
        projection: &'a SourceMetadataProjection,
    ) -> SourceMetadataGateFuture<'a> {
        self.gate.check_with_context(projection, &self.context)
    }
}
impl AuthorizationManager {
    /// Construct a source manager retaining the exact raw base allocation before
    /// IntoUrl. Requires an injected source HTTP/derivation/model implementation.
    /// No default Url, placeholder URI or implicit metadata request is created.
    #[cfg(feature = "ssx-source-metadata")]
    pub async fn new_source_with_oauth_http_client(
        base: Arc<str>,
        http_client: Arc<dyn OAuthHttpClient>,
    ) -> Result<Self, AuthError> {
        let mut manager = Self::with_base(
            ManagerBase::Source(base),
            http_client,
            OAuthHttpRedirectPolicy::SourceWithinOrigin,
        );
        manager.source_within_origin_redirects = true;
        Ok(manager)
    }
    /// Borrow the actual source base owner; default managers return None.
    pub fn source_base(&self) -> Option<&Arc<str>> {
        match &self.base_url {
            ManagerBase::Source(base) => Some(base),
            ManagerBase::Default(_) => None,
        }
    }
    /// Discover PRM then ASM from the actual raw challenge, without calling the
    /// default WWWAuthenticateParams/Url/URI discovery or capability checks.
    /// Project callbacks implement the admitted finite source URL operations.
    /// Credential and stale-ASM clearing occur after PRM and before ASM URL
    /// generation. Token/DCR source endpoint adoption is a separate operation.
    pub async fn resolve_source_metadata_from_challenge(
        &mut self,
        raw_challenge: Option<Arc<str>>,
        gate: &dyn SourceMetadataGate,
    ) -> Result<SourceMetadataResolution, AuthError> {
        let base = self
            .source_base()
            .ok_or_else(|| AuthError::SourceOperation(Box::new(SourceOperationUnsupported)))?
            .clone();
        let urls = self
            .http_client
            .source_metadata_urls(SourceMetadataUrlsRequest {
                kind: SourceMetadataKind::Prm,
                base: base.clone(),
                raw_challenge,
                selected_server: None,
            })
            .await
            .map_err(AuthError::SourceOperation)?;
        {
            let state = &mut self.source_metadata_state;
            state.begin_prm_search();
            let contextual = ContextGate {
                gate,
                context: SourceMetadataContext {
                    base: base.clone(),
                    expected_issuer: None,
                    has_prm: false,
                },
            };
            for endpoint in urls {
                let decision = state
                    .request_candidate(
                        self.http_client.as_ref(),
                        &contextual,
                        SourceMetadataKind::Prm,
                        SourceOAuthHttpRequest::discovery(endpoint),
                    )
                    .await
                    .map_err(AuthError::SourceOperation)?;
                if decision == SourceMetadataDecision::Accepted {
                    break;
                }
            }
            state
                .finish_prm_search()
                .map_err(|error| AuthError::SourceOperation(Box::new(error)))?;
        }
        let selected: Option<ModelHttpUrl> = self
            .source_metadata_state
            .prm()
            .map(|prm| prm.authorization_servers()[0].clone());
        let expected = self
            .http_client
            .source_expected_issuer(base.clone(), selected.clone())
            .await
            .map_err(AuthError::SourceOperation)?;
        if let Some(credentials) = self.credential_store.load().await? {
            if !gate
                .credentials_match(&credentials, &expected)
                .await
                .map_err(AuthError::SourceOperation)?
            {
                // Clear all in-process endpoint state synchronously before an
                // awaited credential store can yield/fail. No stale ASM can be
                // resurrected by cancellation during source token clearing.
                self.oauth_client = None;
                self.source_client = None;
                self.metadata = None;
                self.source_metadata_state.clear_asm();
                self.credential_store.clear().await?;
            }
        }
        // This callback is deliberately AFTER credentials/stale metadata clear.
        // Even source join/parse failures cannot resurrect the old ASM endpoints.
        let urls = self
            .http_client
            .source_metadata_urls(SourceMetadataUrlsRequest {
                kind: SourceMetadataKind::Asm,
                base: base.clone(),
                raw_challenge: None,
                selected_server: selected.clone(),
            })
            .await
            .map_err(AuthError::SourceOperation)?;
        let state = &mut self.source_metadata_state;
        let contextual = ContextGate {
            gate,
            context: SourceMetadataContext {
                base,
                expected_issuer: Some(expected.clone()),
                has_prm: selected.is_some(),
            },
        };
        for endpoint in urls {
            match state
                .request_candidate(
                    self.http_client.as_ref(),
                    &contextual,
                    SourceMetadataKind::Asm,
                    SourceOAuthHttpRequest::discovery(endpoint),
                )
                .await
                .map_err(AuthError::SourceOperation)?
            {
                SourceMetadataDecision::Accepted | SourceMetadataDecision::Stop => break,
                SourceMetadataDecision::Continue => (),
            }
        }
        Ok(SourceMetadataResolution {
            prm: state.prm().cloned(),
            asm: state.asm().cloned(),
            expected_issuer: expected,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::source_metadata::{
        SourceCredentialsFuture, SourceEndpoint, SourceExpectedIssuerFuture, SourceMetadataFuture,
        SourceMetadataUrlsFuture,
    };
    use super::super::{
        CredentialStore, OAuthHttpClientFuture, OAuthHttpRequest, StoredCredentials,
    };
    use super::*;
    use std::{collections::VecDeque, sync::Mutex};
    type Events = Arc<Mutex<Vec<String>>>;
    fn events(e: &Events, value: &str) {
        e.lock().unwrap().push(value.into());
    }
    fn prm() -> Vec<u8> {
        br#"{"resource":"https://resource.example/mcp","authorization_servers":["https://FIRST.example","https://second.example"]}"#.to_vec()
    }
    fn asm() -> Vec<u8> {
        br#"{"issuer":"https://first.example","authorization_endpoint":"https://first.example/a","token_endpoint":"https://first.example/t","response_types_supported":[],"code_challenge_methods_supported":[]}"#.to_vec()
    }
    struct Client {
        base: Arc<str>,
        challenge: Arc<str>,
        events: Events,
        replies: Mutex<VecDeque<(u16, Vec<u8>)>>,
    }
    impl OAuthHttpClient for Client {
        fn execute(&self, _: OAuthHttpRequest) -> OAuthHttpClientFuture<'_> {
            panic!("raw source discovery must not construct a default URI operation")
        }
        fn execute_source(&self, request: SourceOAuthHttpRequest) -> OAuthHttpClientFuture<'_> {
            events(&self.events, &format!("get:{}", request.endpoint.text()));
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("finite source request count");
            assert_eq!(request.method, oauth2::http::Method::GET);
            assert_eq!(
                request.redirect_policy,
                OAuthHttpRedirectPolicy::SourceWithinOrigin
            );
            Box::pin(async move {
                Ok(oauth2::http::Response::builder()
                    .status(reply.0)
                    .body(reply.1)
                    .unwrap())
            })
        }
        fn source_metadata<'a>(
            &'a self,
            kind: SourceMetadataKind,
            body: &'a [u8],
        ) -> SourceMetadataFuture<'a> {
            Box::pin(async move { Ok(SourceMetadataProjection::parse(kind, body).ok()) })
        }
        fn source_metadata_urls(
            &self,
            request: SourceMetadataUrlsRequest,
        ) -> SourceMetadataUrlsFuture<'_> {
            assert!(Arc::ptr_eq(&request.base, &self.base));
            let urls = if request.kind == SourceMetadataKind::Prm {
                assert!(Arc::ptr_eq(
                    request.raw_challenge.as_ref().unwrap(),
                    &self.challenge
                ));
                assert!(request.selected_server.is_none());
                events(&self.events, "prm-plan");
                vec![
                    SourceEndpoint::advertised(Arc::from("http://META.test:0/../advertised?")),
                    SourceEndpoint::derived(Arc::from("http://RAW.test:-1/.well-known/prm")),
                ]
            } else {
                assert!(request.raw_challenge.is_none());
                assert_eq!(
                    request.selected_server.as_ref().unwrap().as_str(),
                    "https://first.example"
                );
                events(&self.events, "asm-plan");
                vec![
                    SourceEndpoint::derived(Arc::from("https://first.example/asm-one")),
                    SourceEndpoint::derived(Arc::from("https://first.example/asm-two")),
                ]
            };
            Box::pin(async move { Ok(urls) })
        }
        fn source_expected_issuer(
            &self,
            base: Arc<str>,
            server: Option<ModelHttpUrl>,
        ) -> SourceExpectedIssuerFuture<'_> {
            assert!(Arc::ptr_eq(&base, &self.base));
            assert_eq!(server.unwrap().as_str(), "https://first.example");
            events(&self.events, "expected-issuer");
            Box::pin(async { Ok(Arc::from("https://first.example")) })
        }
    }
    struct Gate {
        events: Events,
        reject_resource: bool,
    }
    impl SourceMetadataGate for Gate {
        fn check<'a>(&'a self, _: &'a SourceMetadataProjection) -> SourceMetadataGateFuture<'a> {
            Box::pin(async { Ok(()) })
        }
        fn check_with_context<'a>(
            &'a self,
            projection: &'a SourceMetadataProjection,
            context: &'a SourceMetadataContext,
        ) -> SourceMetadataGateFuture<'a> {
            let resource = projection.kind() == SourceMetadataKind::Prm;
            events(
                &self.events,
                if resource {
                    "resource-check"
                } else {
                    "issuer-check"
                },
            );
            if !resource {
                assert!(context.has_prm());
                assert_eq!(context.expected_issuer(), Some("https://first.example"));
            }
            Box::pin(async move {
                if resource && self.reject_resource {
                    Err("fixture semantic resource mismatch".into())
                } else {
                    Ok(())
                }
            })
        }
        fn credentials_match<'a>(
            &'a self,
            stored: &'a StoredCredentials,
            expected: &'a str,
        ) -> SourceCredentialsFuture<'a> {
            assert_eq!(stored.client_id, "old-client");
            assert_eq!(expected, "https://first.example");
            events(&self.events, "credential-check");
            Box::pin(async { Ok(false) })
        }
    }
    #[derive(Clone)]
    struct Store {
        events: Events,
        value: Arc<Mutex<Option<StoredCredentials>>>,
    }
    #[async_trait::async_trait]
    impl CredentialStore for Store {
        async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
            events(&self.events, "credential-load");
            Ok(self.value.lock().unwrap().clone())
        }
        async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
            *self.value.lock().unwrap() = Some(credentials);
            Ok(())
        }
        async fn clear(&self) -> Result<(), AuthError> {
            events(&self.events, "credential-clear");
            *self.value.lock().unwrap() = None;
            Ok(())
        }
    }
    async fn fixture(
        replies: Vec<(u16, Vec<u8>)>,
        stored: bool,
        reject_resource: bool,
    ) -> (AuthorizationManager, Arc<Client>, Gate, Store) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let base: Arc<str> = "http://RAW.test:-1/mcp?empty=#fragment".into();
        let challenge: Arc<str> =
            "Bearer resource_metadata=\"http://META.test:0/../advertised?\", scope=\" x  y \""
                .into();
        let client = Arc::new(Client {
            base: base.clone(),
            challenge,
            events: events.clone(),
            replies: Mutex::new(replies.into()),
        });
        let mut manager =
            AuthorizationManager::new_source_with_oauth_http_client(base.clone(), client.clone())
                .await
                .unwrap();
        assert!(Arc::ptr_eq(manager.source_base().unwrap(), &base));
        let value = if stored {
            Some(StoredCredentials::new(
                "old-client".into(),
                None,
                vec![],
                None,
            ))
        } else {
            None
        };
        let store = Store {
            events: events.clone(),
            value: Arc::new(Mutex::new(value)),
        };
        manager.set_credential_store(store.clone());
        (
            manager,
            client,
            Gate {
                events,
                reject_resource,
            },
            store,
        )
    }
    #[tokio::test]
    async fn source_manager_raw_plan_body_pipeline_and_credential_clear_order_are_real_calls() {
        let (mut manager, client, gate, store) = fixture(
            vec![
                (200, b"invalid".to_vec()),
                (200, prm()),
                (404, vec![]),
                (200, asm()),
            ],
            true,
            false,
        )
        .await;
        let result = manager
            .resolve_source_metadata_from_challenge(Some(client.challenge.clone()), &gate)
            .await
            .unwrap();
        assert_eq!(
            result.prm().unwrap().authorization_servers()[0].as_str(),
            "https://first.example"
        );
        assert!(
            result.asm().unwrap().response_types().is_empty(),
            "default code/S256 rejection was not added to source discovery"
        );
        assert!(store.value.lock().unwrap().is_none());
        assert_eq!(
            *client.events.lock().unwrap(),
            [
                "prm-plan",
                "get:http://META.test:0/../advertised?",
                "get:http://RAW.test:-1/.well-known/prm",
                "resource-check",
                "expected-issuer",
                "credential-load",
                "credential-check",
                "credential-clear",
                "asm-plan",
                "get:https://first.example/asm-one",
                "get:https://first.example/asm-two",
                "issuer-check"
            ]
        );
        assert!(client.replies.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn source_manager_old_normalized_discovery_and_client_replacement_refuse_before_io() {
        let (mut manager, client, _, _) = fixture(vec![], false, false).await;
        assert!(matches!(
            manager
                .resolve_metadata_from_challenge(Some(&client.challenge))
                .await,
            Err(AuthError::SourceOperation(_))
        ));
        assert!(
            manager
                .configure_client_id("must-not-enter-default-token-builder")
                .is_err()
        );
        assert!(client.events.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn source_manager_prm_transient_failure_stops_before_issuer_credentials_or_asm() {
        let (mut manager, client, gate, _) =
            fixture(vec![(500, vec![]), (200, b"invalid".to_vec())], true, false).await;
        let error = manager
            .resolve_source_metadata_from_challenge(Some(client.challenge.clone()), &gate)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Protected resource metadata request failed: HTTP 500")
        );
        assert!(manager.source_metadata_state.prm().is_none());
        assert!(
            !client
                .events
                .lock()
                .unwrap()
                .iter()
                .any(|s| s == "expected-issuer" || s == "credential-clear" || s == "asm-plan")
        );
    }
    #[tokio::test]
    async fn source_manager_semantic_mismatch_is_not_invalid_document_fallback() {
        let (mut manager, client, gate, _) = fixture(vec![(200, prm())], true, true).await;
        let error = manager
            .resolve_source_metadata_from_challenge(Some(client.challenge.clone()), &gate)
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "fixture semantic resource mismatch");
        assert!(manager.source_metadata_state.prm().is_none());
        assert_eq!(
            client.events.lock().unwrap().last().unwrap(),
            "resource-check"
        );
    }
    #[tokio::test]
    async fn source_manager_cleared_asm_stays_absent_when_first_asm_status_stops_search() {
        let (mut manager, client, gate, _) =
            fixture(vec![(200, prm()), (500, vec![])], true, false).await;
        manager
            .source_metadata_state
            .consider_response(client.as_ref(), &gate, SourceMetadataKind::Asm, 200, &asm())
            .await
            .unwrap();
        client.events.lock().unwrap().clear();
        let result = manager
            .resolve_source_metadata_from_challenge(Some(client.challenge.clone()), &gate)
            .await
            .unwrap();
        assert!(result.prm().is_some());
        assert!(result.asm().is_none());
        assert!(
            !client
                .events
                .lock()
                .unwrap()
                .iter()
                .any(|s| s == "get:https://first.example/asm-two")
        );
        assert!(store_is_before(
            &client.events,
            "credential-clear",
            "asm-plan"
        ));
    }
    #[tokio::test]
    async fn source_default_manager_keeps_genuine_url_and_source_constructor_never_parses_one() {
        struct DefaultClient;
        impl OAuthHttpClient for DefaultClient {
            fn execute(&self, _: OAuthHttpRequest) -> OAuthHttpClientFuture<'_> {
                panic!("no default HTTP fallback")
            }
        }
        let client: Arc<dyn OAuthHttpClient> = Arc::new(DefaultClient);
        let normal = AuthorizationManager::new_with_oauth_http_client(
            "HTTPS://DEFAULT.EXAMPLE",
            client.clone(),
        )
        .await
        .unwrap();
        assert_eq!(
            normal.default_base_url().unwrap().as_str(),
            "https://default.example/"
        );
        assert!(normal.source_base().is_none());
        let raw: Arc<str> = "not a default URL :-1".into();
        let mut source =
            AuthorizationManager::new_source_with_oauth_http_client(raw.clone(), client)
                .await
                .unwrap();
        assert!(Arc::ptr_eq(source.source_base().unwrap(), &raw));
        let gate = Gate {
            events: Arc::new(Mutex::new(Vec::new())),
            reject_resource: false,
        };
        let error = source
            .resolve_source_metadata_from_challenge(None, &gate)
            .await
            .unwrap_err();
        match error {
            AuthError::SourceOperation(error) => {
                assert!(error.downcast_ref::<SourceOperationUnsupported>().is_some())
            }
            _ => panic!("must refuse absent source derivation, not eagerly parse default URL"),
        }
    }
    #[tokio::test]
    async fn source_manager_cancelled_credential_clear_cannot_restore_old_asm() {
        struct HeldClear;
        #[async_trait::async_trait]
        impl CredentialStore for HeldClear {
            async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
                Ok(Some(StoredCredentials::new(
                    "old-client".into(),
                    None,
                    vec![],
                    None,
                )))
            }
            async fn save(&self, _: StoredCredentials) -> Result<(), AuthError> {
                panic!("no credential save during discovery")
            }
            async fn clear(&self) -> Result<(), AuthError> {
                std::future::pending().await
            }
        }
        let (mut manager, client, gate, _) = fixture(vec![(200, prm())], true, false).await;
        manager
            .source_metadata_state
            .consider_response(client.as_ref(), &gate, SourceMetadataKind::Asm, 200, &asm())
            .await
            .unwrap();
        manager.set_credential_store(HeldClear);
        let mut discovery = Box::pin(
            manager.resolve_source_metadata_from_challenge(Some(client.challenge.clone()), &gate),
        );
        std::future::poll_fn(|cx| {
            use std::future::Future;
            assert!(discovery.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(discovery);
        assert!(manager.source_metadata_state.prm().is_some());
        assert!(
            manager.source_metadata_state.asm().is_none(),
            "stale endpoints cleared before credential-store yield"
        );
        assert!(
            !client
                .events
                .lock()
                .unwrap()
                .iter()
                .any(|s| s == "asm-plan")
        );
    }
    fn store_is_before(events: &Events, a: &str, b: &str) -> bool {
        let rows = events.lock().unwrap();
        rows.iter().position(|s| s == a).unwrap() < rows.iter().position(|s| s == b).unwrap()
    }
}

// Source OAuth2 request construction follows MCP2.3 oauth2.py385–558, not
// oauth2::Client endpoint builders. Project model codecs run in execute_source.
use super::source_metadata::{
    SourceEndpoint, SourceOAuthEndpointKind, SourceOAuthEndpointRequest, SourceOAuthPurpose,
};
use super::{
    CsrfToken, OAuthClientConfig, OAuthTokenResponse, PkceCodeChallenge, RefreshToken,
    StoredAuthorizationState, StoredCredentials, TokenResponse,
};
use base64::Engine as _;

#[derive(Clone)]
pub(super) struct SourceClient {
    config: OAuthClientConfig,
    auth_method: Option<String>,
    authorization: SourceEndpoint,
    token: SourceEndpoint,
    resource: Option<Arc<str>>,
    callback_issuer: Option<String>,
    credential_issuer: Option<String>,
    require_issuer: bool,
}
// No Debug implementation: configuration contains client secrets.
// MCP PKCEParameters.generate chooses 128 independent characters uniformly
// from the 66-character unreserved alphabet. The production byte source is
// rand's cryptographic thread RNG; no RNG is retained across an await.
fn source_verifier(mut next: impl FnMut() -> u8) -> String {
    const ALPHABET: &[u8; 66] =
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~";
    let mut out = String::with_capacity(128);
    while out.len() < 128 {
        let byte = next();
        // Exactly three complete alphabet copies fit in 256. Reject the tail
        // instead of reducing its extra 58 values modulo 66 (which biases it).
        if byte < 198 {
            out.push(char::from(ALPHABET[usize::from(byte % 66)]));
        }
    }
    out
}
fn quote(text: &str, plus: bool) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
        } else if plus && byte == b' ' {
            out.push('+');
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(byte >> 4)]));
            out.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    out
}
fn form(fields: &[(String, String)]) -> Vec<u8> {
    fields
        .iter()
        .map(|(k, v)| format!("{}={}", quote(k, true), quote(v, true)))
        .collect::<Vec<_>>()
        .join("&")
        .into_bytes()
}
fn post(
    endpoint: SourceEndpoint,
    purpose: SourceOAuthPurpose,
    content_type: &'static str,
    body: Vec<u8>,
) -> SourceOAuthHttpRequest {
    let mut operation = SourceOAuthHttpRequest::discovery(endpoint);
    operation.purpose = purpose;
    operation.method = oauth2::http::Method::POST;
    operation.headers.insert(
        oauth2::http::header::CONTENT_TYPE,
        oauth2::http::HeaderValue::from_static(content_type),
    );
    operation.body = body;
    operation
}
fn authenticate(
    client: &SourceClient,
    fields: &mut Vec<(String, String)>,
    request: &mut SourceOAuthHttpRequest,
) -> Result<(), AuthError> {
    let secret = client
        .config
        .client_secret
        .as_deref()
        .filter(|s| !s.is_empty());
    match client.auth_method.as_deref() {
        Some("client_secret_basic") if secret.is_some() => {
            // Python quote(safe="") uses %20, not form '+'; client_id stays in body.
            let raw = format!(
                "{}:{}",
                quote(&client.config.client_id, false),
                quote(secret.unwrap_or_default(), false)
            );
            let value = format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(raw)
            );
            let mut header = oauth2::http::HeaderValue::from_str(&value)
                .map_err(|e| AuthError::OAuthError(e.to_string()))?;
            header.set_sensitive(true);
            request
                .headers
                .insert(oauth2::http::header::AUTHORIZATION, header);
        }
        Some("client_secret_post") if secret.is_some() => {
            fields.push(("client_secret".into(), secret.unwrap_or_default().into()));
        }
        None | Some("none" | "client_secret_basic" | "client_secret_post" | "private_key_jwt") => {
            ()
        }
        Some(_) => {
            return Err(AuthError::OAuthError(
                "Registered client uses unsupported token_endpoint_auth_method".into(),
            ));
        }
    }
    request.body = form(fields);
    Ok(())
}
impl AuthorizationManager {
    async fn source_endpoint(
        &self,
        kind: SourceOAuthEndpointKind,
    ) -> Result<SourceEndpoint, AuthError> {
        let base = self
            .source_base()
            .ok_or_else(|| AuthError::SourceOperation(Box::new(SourceOperationUnsupported)))?;
        if let Some(asm) = self.source_metadata_state.asm() {
            let model = match kind {
                SourceOAuthEndpointKind::Authorization => Some(asm.authorization_endpoint()),
                SourceOAuthEndpointKind::Token => Some(asm.token_endpoint()),
                SourceOAuthEndpointKind::Registration => asm.registration_endpoint(),
            };
            if let Some(model) = model {
                return Ok(model.endpoint());
            }
        }
        // MCP OAuthContext.get_authorization_base_url uses the RESOURCE base
        // origin, even when a PRM advertised a different authorization server.
        self.http_client
            .source_oauth_endpoint(SourceOAuthEndpointRequest {
                base: base.clone(),
                kind,
            })
            .await
            .map_err(AuthError::SourceOperation)
    }
    /// Configure the source lane without parsing endpoints or redirect URI through
    /// oauth2::Client. The resource indicator was selected by project source
    /// protocol/resource policy; None means omit it, including on refresh.
    /// Endpoints come only from the current validated ASM or injected raw fallback.
    /// The credential issuer is the actual client record's project-validated
    /// binding, not inferred from ASM. Callback issuer remains the ASM issuer.
    pub async fn configure_source_client(
        &mut self,
        config: OAuthClientConfig,
        token_endpoint_auth_method: Option<String>,
        resource: Option<Arc<str>>,
        credential_issuer: Option<String>,
    ) -> Result<(), AuthError> {
        self.source_base()
            .ok_or_else(|| AuthError::SourceOperation(Box::new(SourceOperationUnsupported)))?;
        let authorization = self
            .source_endpoint(SourceOAuthEndpointKind::Authorization)
            .await?;
        let token = self.source_endpoint(SourceOAuthEndpointKind::Token).await?;
        let issuer = self
            .source_metadata_state
            .asm()
            .map(|m| m.issuer().as_str().to_owned());
        let require_issuer = self
            .source_metadata_state
            .asm()
            .and_then(|m| m.response_iss_supported())
            .unwrap_or(false);
        let client = SourceClient {
            config,
            auth_method: token_endpoint_auth_method,
            authorization,
            token,
            resource,
            callback_issuer: issuer,
            credential_issuer,
            require_issuer,
        };
        // Construct everything before replacing an existing configured source client.
        self.source_client = Some(client);
        Ok(())
    }
    /// Dynamically register through the exact model endpoint or raw /register
    /// fallback. The adapter discards wire issuer and runs the existing client
    /// model before returning these bytes. Only HTTP200/201 are accepted.
    /// Unusable methods are rejected before source client state is configured.
    /// The caller supplies the source registration binding decision for the
    /// actual target; wire issuer was discarded by the project before validation.
    pub async fn register_source_client(
        &mut self,
        name: &str,
        redirect_uri: &str,
        scopes: &[&str],
        resource: Option<Arc<str>>,
        credential_issuer: Option<String>,
    ) -> Result<OAuthClientConfig, AuthError> {
        let endpoint = self
            .source_endpoint(SourceOAuthEndpointKind::Registration)
            .await?;
        // MCP2.3 shared/auth.py67–130: inherited model fields precede child
        // fields. Serialize this exposed client-metadata subset in source order;
        // this is a request DTO, not another client-model validator.
        #[derive(serde::Serialize)]
        struct Registration<'a> {
            response_types: [&'a str; 1],
            #[serde(skip_serializing_if = "Option::is_none")]
            scope: Option<String>,
            client_name: &'a str,
            redirect_uris: [&'a str; 1],
            token_endpoint_auth_method: &'a str,
            grant_types: [&'a str; 2],
            application_type: &'a str,
        }
        let body = serde_json::to_vec(&Registration {
            response_types: ["code"],
            scope: (!scopes.is_empty()).then(|| scopes.join(" ")),
            client_name: name,
            redirect_uris: [redirect_uri],
            token_endpoint_auth_method: "none",
            grant_types: ["authorization_code", "refresh_token"],
            application_type: self
                .application_type
                .as_deref()
                .unwrap_or(super::DEFAULT_APPLICATION_TYPE),
        })
        .map_err(|e| AuthError::RegistrationFailed(e.to_string()))?;
        let reply = self
            .http_client
            .execute_source(post(
                endpoint,
                SourceOAuthPurpose::Registration,
                "application/json",
                body,
            ))
            .await
            .map_err(AuthError::SourceOperation)?;
        if !matches!(reply.status().as_u16(), 200 | 201) {
            return Err(AuthError::RegistrationFailed(format!(
                "Registration failed: {}",
                reply.status()
            )));
        }
        #[derive(serde::Deserialize)]
        struct Registered {
            client_id: String,
            client_secret: Option<String>,
            token_endpoint_auth_method: Option<String>,
        }
        let record: Registered = serde_json::from_slice(reply.body())
            .map_err(|e| AuthError::RegistrationFailed(e.to_string()))?;
        match record.token_endpoint_auth_method.as_deref() {
            None | Some("none") => (),
            Some("client_secret_post" | "client_secret_basic") if record.client_secret.is_some() => (),
            _ => return Err(AuthError::RegistrationFailed("Authorization server registered an unusable token_endpoint_auth_method or missing client_secret".into())),
        }
        let config = OAuthClientConfig {
            client_id: record.client_id,
            client_secret: record.client_secret,
            redirect_uri: redirect_uri.into(),
            scopes: scopes.iter().map(|s| (*s).into()).collect(),
            application_type: self.application_type.clone(),
        };
        self.configure_source_client(
            config.clone(),
            record.token_endpoint_auth_method,
            resource,
            credential_issuer,
        )
        .await?;
        Ok(config)
    }
    fn source_configured(&self) -> Result<&SourceClient, AuthError> {
        self.source_client
            .as_ref()
            .ok_or_else(|| AuthError::InternalError("Source OAuth client not configured".into()))
    }
    pub(super) async fn source_authorization_url(
        &self,
        scopes: &[&str],
    ) -> Result<String, AuthError> {
        let client = self.source_configured()?;
        let verifier = super::PkceCodeVerifier::new(source_verifier(rand::random::<u8>));
        let challenge = PkceCodeChallenge::from_code_verifier_sha256(&verifier);
        let csrf = CsrfToken::new_random_len(32);
        let mut fields = vec![
            ("response_type".into(), "code".into()),
            ("client_id".into(), client.config.client_id.clone()),
            ("redirect_uri".into(), client.config.redirect_uri.clone()),
            ("state".into(), csrf.secret().clone()),
            ("code_challenge".into(), challenge.as_str().into()),
            ("code_challenge_method".into(), "S256".into()),
        ];
        if let Some(resource) = &client.resource {
            fields.push(("resource".into(), resource.to_string()));
        }
        let scope = scopes.join(" ");
        if !scope.is_empty() {
            // Callers preserve a source scope string by passing one element;
            // never split and rejoin it before this request construction.
            let offline = scope
                .split(|c: char| {
                    matches!(c,
                '\u{0009}'..='\u{000d}' | '\u{001c}'..='\u{0020}' | '\u{0085}' |
                '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' |
                '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
                })
                .any(|word| word == "offline_access");
            fields.push(("scope".into(), scope));
            if offline {
                fields.push(("prompt".into(), "consent".into()));
            }
        }
        // Python concatenates a literal '?' even when endpoint already has query
        // or fragment. Do not silently repair this using Url query_pairs_mut.
        let encoded =
            String::from_utf8(form(&fields)).map_err(|e| AuthError::OAuthError(e.to_string()))?;
        let url = format!("{}?{encoded}", client.authorization.text());
        let state = StoredAuthorizationState::new_with_expected_issuer(
            &verifier,
            &csrf,
            client.callback_issuer.clone(),
            client.require_issuer,
        )
        .with_requested_scopes(scopes.iter().map(|s| (*s).into()).collect());
        self.state_store.save(csrf.secret(), state).await?;
        Ok(url)
    }
    pub(super) async fn source_exchange_code(
        &self,
        code: &str,
        csrf: &str,
        received_issuer: Option<&str>,
    ) -> Result<OAuthTokenResponse, AuthError> {
        let client = self.source_configured()?;
        let state = self
            .state_store
            .load(csrf)
            .await?
            .ok_or_else(|| AuthError::InternalError("Authorization state not found".into()))?;
        Self::validate_authorization_response_issuer(&state, received_issuer)?;
        if code.is_empty() {
            return Err(AuthError::TokenExchangeFailed(
                "No authorization code received".into(),
            ));
        }
        self.state_store.delete(csrf).await?;
        let requested = state.requested_scopes.clone();
        let verifier = state.into_pkce_verifier();
        let mut fields = vec![
            ("grant_type".into(), "authorization_code".into()),
            ("code".into(), code.into()),
            ("redirect_uri".into(), client.config.redirect_uri.clone()),
            ("client_id".into(), client.config.client_id.clone()),
            ("code_verifier".into(), verifier.secret().clone()),
        ];
        if let Some(resource) = &client.resource {
            fields.push(("resource".into(), resource.to_string()));
        }
        let mut operation = post(
            client.token.clone(),
            SourceOAuthPurpose::CodeExchange,
            "application/x-www-form-urlencoded",
            Vec::new(),
        );
        authenticate(client, &mut fields, &mut operation)?;
        let reply = self
            .http_client
            .execute_source(operation)
            .await
            .map_err(AuthError::SourceOperation)?;
        if !matches!(reply.status().as_u16(), 200 | 201) {
            return Err(AuthError::TokenExchangeFailed(format!(
                "Token exchange failed ({})",
                reply.status()
            )));
        }
        let token: OAuthTokenResponse = serde_json::from_slice(reply.body())
            .map_err(|e| AuthError::TokenExchangeFailed(e.to_string()))?;
        let response_scopes = token
            .scopes()
            .map(|s| s.iter().map(|scope| scope.as_ref().to_owned()).collect());
        let scopes = {
            let current = self.current_scopes.read().await;
            Self::resolve_granted_scopes(response_scopes, &requested, &current)
        };
        *self.current_scopes.write().await = scopes.clone();
        *self.scope_upgrade_attempts.write().await = 0;
        self.credential_store
            .save(StoredCredentials {
                client_id: client.config.client_id.clone(),
                token_response: Some(token.clone()),
                granted_scopes: scopes,
                token_received_at: Some(Self::now_epoch_secs()),
                issuer: client.credential_issuer.clone(),
            })
            .await?;
        Ok(token)
    }
    pub(super) async fn source_refresh_token(&self) -> Result<OAuthTokenResponse, AuthError> {
        let client = self.source_configured()?;
        let _guard = self.credential_store.acquire_refresh_guard().await?;
        let stored = self
            .credential_store
            .load()
            .await?
            .ok_or(AuthError::AuthorizationRequired)?;
        if client.config.client_id.is_empty()
            || stored.client_id.is_empty()
            || stored.client_id != client.config.client_id
        {
            return Err(AuthError::AuthorizationRequired);
        }
        let prior = stored
            .token_response
            .as_ref()
            .ok_or(AuthError::AuthorizationRequired)?;
        let refresh = prior
            .refresh_token()
            .filter(|value| !value.secret().is_empty())
            .ok_or(AuthError::AuthorizationRequired)?;
        let mut fields = vec![
            ("grant_type".into(), "refresh_token".into()),
            ("refresh_token".into(), refresh.secret().clone()),
            ("client_id".into(), client.config.client_id.clone()),
        ];
        if let Some(resource) = &client.resource {
            fields.push(("resource".into(), resource.to_string()));
        }
        // Source refresh sends no scope field and does not add offline_access.
        let mut operation = post(
            client.token.clone(),
            SourceOAuthPurpose::Refresh,
            "application/x-www-form-urlencoded",
            Vec::new(),
        );
        authenticate(client, &mut fields, &mut operation)?;
        let reply = match self.http_client.execute_source(operation).await {
            Ok(reply) => reply,
            Err(error) if error.is::<super::source_metadata::SourceOAuthModelInvalid>() => {
                let _ = self
                    .credential_store
                    .on_source_refresh_fallback(&stored)
                    .await;
                return Err(AuthError::TokenRefreshRejected(
                    "Invalid source refresh token model".into(),
                ));
            }
            Err(error) => return Err(AuthError::SourceOperation(error)),
        };
        let mut token: OAuthTokenResponse = if reply.status().as_u16() == 200 {
            // A failure HERE is a project-to-SDK projection defect, not a source
            // ValidationError: do not silently turn it into reauthorization.
            serde_json::from_slice(reply.body())
                .map_err(|e| AuthError::TokenRefreshFailed(e.to_string()))?
        } else {
            // Source _handle_refresh_response returns false on EVERY non200,
            // not merely invalid_grant. Preserve existing guarded store hook.
            let _ = self
                .credential_store
                .on_source_refresh_fallback(&stored)
                .await;
            return Err(AuthError::TokenRefreshRejected(format!(
                "Token refresh failed: {}",
                reply.status()
            )));
        };
        if token.refresh_token().is_none() {
            token.set_refresh_token(Some(RefreshToken::new(refresh.secret().clone())));
        }
        let response_scopes = token
            .scopes()
            .map(|s| s.iter().map(|scope| scope.as_ref().to_owned()).collect());
        let scopes = {
            let current = self.current_scopes.read().await;
            Self::resolve_granted_scopes(response_scopes, &stored.granted_scopes, &current)
        };
        *self.current_scopes.write().await = scopes.clone();
        self.credential_store
            .save(StoredCredentials {
                client_id: client.config.client_id.clone(),
                token_response: Some(token.clone()),
                granted_scopes: scopes,
                token_received_at: Some(Self::now_epoch_secs()),
                issuer: client.credential_issuer.clone(),
            })
            .await?;
        Ok(token)
    }
}

#[cfg(test)]
mod source_operation_tests {
    use super::super::source_metadata::{SourceOAuthEndpointFuture, SourceOAuthModelInvalid};
    use super::super::{OAuthHttpClientFuture, OAuthHttpRequest};
    use super::*;
    use std::{collections::VecDeque, sync::Mutex};
    enum Reply {
        Http(u16, &'static [u8]),
        ModelInvalid,
        Transport,
        Pending,
    }
    struct Client {
        base: Arc<str>,
        operations: Mutex<Vec<SourceOAuthHttpRequest>>,
        replies: Mutex<VecDeque<Reply>>,
    }
    impl OAuthHttpClient for Client {
        fn execute(&self, _: OAuthHttpRequest) -> OAuthHttpClientFuture<'_> {
            panic!("source OAuth must not construct an ordinary typed URI operation")
        }
        fn execute_source(&self, request: SourceOAuthHttpRequest) -> OAuthHttpClientFuture<'_> {
            self.operations.lock().unwrap().push(request);
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("finite request count");
            Box::pin(async move {
                match reply {
                    Reply::Http(status, body) => Ok(oauth2::http::Response::builder()
                        .status(status)
                        .body(body.to_vec())
                        .unwrap()),
                    Reply::ModelInvalid => {
                        Err(Box::new(SourceOAuthModelInvalid) as super::super::OAuthHttpClientError)
                    }
                    Reply::Transport => Err("source transport failed".into()),
                    Reply::Pending => std::future::pending().await,
                }
            })
        }
        fn source_metadata<'a>(
            &'a self,
            kind: SourceMetadataKind,
            body: &'a [u8],
        ) -> super::super::source_metadata::SourceMetadataFuture<'a> {
            Box::pin(async move { Ok(SourceMetadataProjection::parse(kind, body).ok()) })
        }
        fn source_oauth_endpoint(
            &self,
            request: SourceOAuthEndpointRequest,
        ) -> SourceOAuthEndpointFuture<'_> {
            assert!(Arc::ptr_eq(&request.base, &self.base));
            let text = match request.kind {
                SourceOAuthEndpointKind::Authorization => "http://RAW.test:-1/authorize?prior#frag",
                SourceOAuthEndpointKind::Token => "http://RAW.test:0/../token?",
                SourceOAuthEndpointKind::Registration => "http://RAW.test:-1/register?",
            };
            Box::pin(async move { Ok(SourceEndpoint::derived(text.into())) })
        }
    }
    async fn manager(replies: Vec<Reply>) -> (AuthorizationManager, Arc<Client>) {
        let base: Arc<str> = "\0HTTP://Raw.test:-1/a;b?".into();
        let client = Arc::new(Client {
            base: base.clone(),
            operations: Mutex::new(Vec::new()),
            replies: Mutex::new(replies.into()),
        });
        let manager = AuthorizationManager::new_source_with_oauth_http_client(base, client.clone())
            .await
            .unwrap();
        (manager, client)
    }
    fn config() -> OAuthClientConfig {
        OAuthClientConfig::new("client id*", "http://Callback.test:0/../callback?")
            .with_client_secret("s+e cret")
    }
    async fn configure(manager: &mut AuthorizationManager, method: Option<&str>) {
        manager
            .configure_source_client(
                config(),
                method.map(str::to_owned),
                Some(Arc::from("http://Resource.test/api")),
                None,
            )
            .await
            .unwrap();
    }
    async fn seed(manager: &AuthorizationManager) {
        let token: OAuthTokenResponse = serde_json::from_slice(br#"{"access_token":"old","token_type":"bearer","refresh_token":"old-refresh","scope":"read"}"#).unwrap();
        manager
            .credential_store
            .save(StoredCredentials::new(
                "client id*".into(),
                Some(token),
                vec!["read".into()],
                None,
            ))
            .await
            .unwrap();
    }
    #[test]
    fn python_form_and_basic_quote_keep_tilde_escape_star_and_differ_on_space() {
        assert_eq!(quote("a ~*+é", true), "a+~%2A%2B%C3%A9");
        assert_eq!(quote("a ~*+é", false), "a%20~%2A%2B%C3%A9");
        assert_eq!(
            form(&[
                ("scope".into(), "read write".into()),
                ("x".into(), "".into())
            ]),
            b"scope=read+write&x="
        );
    }
    #[tokio::test]
    async fn source_code201_keeps_raw_target_literal_authorization_suffix_and_basic_body() {
        let (mut manager, client) = manager(vec![Reply::Http(
            201,
            br#"{"access_token":"new","token_type":"bearer"}"#,
        )])
        .await;
        configure(&mut manager, Some("client_secret_basic")).await;
        let token_endpoint = manager.source_configured().unwrap().token.text().clone();
        let url = manager
            .get_authorization_url(&["read", "offline_access"])
            .await
            .unwrap();
        assert!(url.starts_with("http://RAW.test:-1/authorize?prior#frag?response_type=code&client_id=client+id%2A&redirect_uri=http%3A%2F%2FCallback.test%3A0%2F..%2Fcallback%3F&state="));
        assert!(url.ends_with("&code_challenge_method=S256&resource=http%3A%2F%2FResource.test%2Fapi&scope=read+offline_access&prompt=consent"));
        let query = url.rsplit_once('?').unwrap().1;
        let state = query
            .split('&')
            .find_map(|s| s.strip_prefix("state="))
            .unwrap();
        assert_eq!(
            state.len(),
            43,
            "source secrets.token_urlsafe(32) state length"
        );
        assert!(
            state
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        );
        let stored_state = manager.state_store.load(state).await.unwrap().unwrap();
        let verifier = stored_state.into_pkce_verifier();
        assert_eq!(verifier.secret().len(), 128, "source PKCE verifier length");
        assert!(
            verifier
                .secret()
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
        );
        let challenge = PkceCodeChallenge::from_code_verifier_sha256(&verifier);
        assert!(
            query
                .split('&')
                .any(|field| field == format!("code_challenge={}", challenge.as_str())),
            "URL challenge must derive from the same stored source verifier"
        );
        let token = manager
            .exchange_code_for_token("a+b ~*", state)
            .await
            .unwrap();
        assert_eq!(token.access_token().secret(), "new");
        let requests = client.operations.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let operation = &requests[0];
        assert_eq!(operation.purpose, SourceOAuthPurpose::CodeExchange);
        assert_eq!(
            operation.endpoint.text().as_ref(),
            "http://RAW.test:0/../token?"
        );
        assert!(Arc::ptr_eq(operation.endpoint.text(), &token_endpoint));
        assert!(String::from_utf8_lossy(&operation.body).starts_with("grant_type=authorization_code&code=a%2Bb+~%2A&redirect_uri=http%3A%2F%2FCallback.test%3A0%2F..%2Fcallback%3F&client_id=client+id%2A&code_verifier="));
        assert!(!String::from_utf8_lossy(&operation.body).contains("client_secret="));
        assert!(
            String::from_utf8_lossy(&operation.body)
                .split('&')
                .any(|field| field == format!("code_verifier={}", verifier.secret())),
            "token request must use the same source128 verifier that produced the URL challenge"
        );
        assert!(operation.headers[oauth2::http::header::AUTHORIZATION].is_sensitive());
        let basic = operation.headers[oauth2::http::header::AUTHORIZATION]
            .to_str()
            .unwrap()
            .strip_prefix("Basic ")
            .unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(basic)
                .unwrap(),
            b"client%20id%2A:s%2Be%20cret"
        );
    }
    #[tokio::test]
    async fn source_refresh_sends_no_scope_and_retains_missing_refresh_and_grant() {
        let (mut manager, client) = manager(vec![Reply::Http(
            200,
            br#"{"access_token":"new","token_type":"bearer"}"#,
        )])
        .await;
        configure(&mut manager, Some("client_secret_post")).await;
        seed(&manager).await;
        let token = manager.refresh_token().await.unwrap();
        assert_eq!(token.refresh_token().unwrap().secret(), "old-refresh");
        let requests = client.operations.lock().unwrap();
        assert_eq!(requests[0].purpose, SourceOAuthPurpose::Refresh);
        assert_eq!(requests[0].body, b"grant_type=refresh_token&refresh_token=old-refresh&client_id=client+id%2A&resource=http%3A%2F%2FResource.test%2Fapi&client_secret=s%2Be+cret");
        drop(requests);
        assert_eq!(
            manager
                .credential_store
                .load()
                .await
                .unwrap()
                .unwrap()
                .granted_scopes,
            ["read"]
        );
    }
    #[tokio::test]
    async fn source_refresh201_model_invalid_and_transport_have_distinct_outcomes() {
        let (mut manager, client) = manager(vec![
            Reply::Http(201, br#"{"access_token":"ignored","token_type":"bearer"}"#),
            Reply::ModelInvalid,
            Reply::Transport,
        ])
        .await;
        configure(&mut manager, None).await;
        seed(&manager).await;
        assert!(matches!(
            manager.refresh_token().await,
            Err(AuthError::TokenRefreshRejected(_))
        ));
        assert!(matches!(
            manager.refresh_token().await,
            Err(AuthError::TokenRefreshRejected(_))
        ));
        assert!(matches!(
            manager.refresh_token().await,
            Err(AuthError::SourceOperation(_))
        ));
        assert_eq!(client.operations.lock().unwrap().len(), 3);
        assert_eq!(
            manager
                .credential_store
                .load()
                .await
                .unwrap()
                .unwrap()
                .token_response
                .unwrap()
                .access_token()
                .secret(),
            "old"
        );
    }
    #[tokio::test]
    async fn source_unknown_auth_and_wrong_client_do_not_dispatch_refresh() {
        let (mut manager, client) = manager(vec![]).await;
        configure(&mut manager, Some("unsupported")).await;
        seed(&manager).await;
        assert!(matches!(
            manager.refresh_token().await,
            Err(AuthError::OAuthError(_))
        ));
        configure(&mut manager, None).await;
        manager
            .credential_store
            .save(StoredCredentials::new(
                "other-client".into(),
                None,
                vec![],
                None,
            ))
            .await
            .unwrap();
        assert!(matches!(
            manager.refresh_token().await,
            Err(AuthError::AuthorizationRequired)
        ));
        assert!(client.operations.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn source_registration201_rejects_unusable_method_before_client_commit() {
        let (mut manager, client) = manager(vec![Reply::Http(201, br#"{"client_id":"issued","client_secret":"secret","token_endpoint_auth_method":"private_key_jwt"}"#)]).await;
        assert!(matches!(
            manager
                .register_source_client("client", "http://CB.test:0/../?", &[], None, None)
                .await,
            Err(AuthError::RegistrationFailed(_))
        ));
        assert!(manager.source_client.is_none());
        let requests = client.operations.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].purpose, SourceOAuthPurpose::Registration);
        assert_eq!(
            requests[0].endpoint.text().as_ref(),
            "http://RAW.test:-1/register?"
        );
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["redirect_uris"][0], "http://CB.test:0/../?");
        assert_eq!(body["token_endpoint_auth_method"], "none");
        assert_eq!(body["application_type"], "native");
    }
    #[tokio::test]
    async fn source_bad_issuer_retains_state_and_never_dispatches_code() {
        let (mut manager, client) = manager(vec![]).await;
        configure(&mut manager, None).await;
        let source = manager.source_client.as_mut().unwrap();
        source.callback_issuer = Some("https://AS.test".into());
        source.require_issuer = true;
        let url = manager.get_authorization_url(&[]).await.unwrap();
        let state = url
            .rsplit_once('?')
            .unwrap()
            .1
            .split('&')
            .find_map(|s| s.strip_prefix("state="))
            .unwrap();
        assert!(
            manager
                .exchange_code_for_token_with_issuer("code", state, Some("https://other.test"))
                .await
                .is_err()
        );
        assert!(manager.state_store.load(state).await.unwrap().is_some());
        assert!(client.operations.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn source_model_endpoint_arc_and_callback_issuer_do_not_forge_client_binding() {
        struct Accept;
        impl SourceMetadataGate for Accept {
            fn check<'a>(
                &'a self,
                _: &'a SourceMetadataProjection,
            ) -> SourceMetadataGateFuture<'a> {
                Box::pin(async { Ok(()) })
            }
        }
        let (mut manager, client) = manager(vec![Reply::Http(
            201,
            br#"{"access_token":"new","token_type":"bearer"}"#,
        )])
        .await;
        manager.source_metadata_state.consider_response(client.as_ref(), &Accept, SourceMetadataKind::Asm, 200,
            br#"{"issuer":"https://AS.test/","authorization_endpoint":"https://AS.test:443/auth?prior#f","token_endpoint":"https://AS.test:443/t?","authorization_response_iss_parameter_supported":true}"#).await.unwrap();
        let expected = manager
            .source_metadata_state
            .asm()
            .unwrap()
            .token_endpoint()
            .endpoint()
            .text()
            .clone();
        manager
            .configure_source_client(config(), None, None, Some("https://as.test".into()))
            .await
            .unwrap();
        let url = manager.get_authorization_url(&[]).await.unwrap();
        let csrf = url
            .rsplit_once('?')
            .unwrap()
            .1
            .split('&')
            .find_map(|s| s.strip_prefix("state="))
            .unwrap();
        let state = manager.state_store.load(csrf).await.unwrap().unwrap();
        assert_eq!(state.expected_issuer.as_deref(), Some("https://as.test/"));
        manager
            .exchange_code_for_token_with_issuer("code", csrf, Some("https://as.test/"))
            .await
            .unwrap();
        assert!(Arc::ptr_eq(
            client.operations.lock().unwrap()[0].endpoint.text(),
            &expected
        ));
        assert_eq!(
            manager
                .credential_store
                .load()
                .await
                .unwrap()
                .unwrap()
                .issuer
                .as_deref(),
            Some("https://as.test")
        );
    }

    #[tokio::test]
    async fn source_pending_refresh_drop_preserves_credentials_and_exact_raw_scope_prompt() {
        let (mut manager, client) = manager(vec![Reply::Pending]).await;
        configure(&mut manager, None).await;
        seed(&manager).await;
        let url = manager
            .get_authorization_url(&["read\u{1c}offline_access"])
            .await
            .unwrap();
        assert!(url.ends_with("&scope=read%1Coffline_access&prompt=consent"));
        let empty = manager.get_authorization_url(&[""]).await.unwrap();
        assert!(!empty.contains("&scope="));
        let mut refreshing = Box::pin(manager.refresh_token());
        let pending = std::future::poll_fn(|cx| {
            std::task::Poll::Ready(refreshing.as_mut().poll(cx).is_pending())
        })
        .await;
        assert!(pending);
        drop(refreshing);
        assert_eq!(client.operations.lock().unwrap().len(), 1);
        assert_eq!(
            manager
                .credential_store
                .load()
                .await
                .unwrap()
                .unwrap()
                .token_response
                .unwrap()
                .access_token()
                .secret(),
            "old"
        );
    }
    #[test]
    fn source_verifier_rejects_biased_tail_and_covers_exact_unreserved_alphabet() {
        let mut bytes = [198u8, 255, 0, 65].into_iter().chain(std::iter::repeat(0));
        let verifier = source_verifier(|| bytes.next().unwrap());
        assert_eq!(verifier.len(), 128);
        assert!(verifier.starts_with("a~"));
        let mut next = 0u8;
        let all = source_verifier(|| {
            let value = next;
            next = next.wrapping_add(1);
            value
        });
        assert_eq!(
            &all[..66],
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~"
        );
    }
    #[tokio::test]
    async fn source_empty_client_or_refresh_token_never_dispatches() {
        let (mut manager, client) = manager(vec![]).await;
        configure(&mut manager, None).await;
        for (configured, stored, refresh) in [
            ("", "", "old-refresh"),
            ("", "other", "old-refresh"),
            ("client id*", "", "old-refresh"),
            ("client id*", "client id*", ""),
        ] {
            manager.source_client.as_mut().unwrap().config.client_id = configured.into();
            let token:OAuthTokenResponse=serde_json::from_value(serde_json::json!({"access_token":"old","token_type":"bearer","refresh_token":refresh})).unwrap();
            manager
                .credential_store
                .save(StoredCredentials::new(
                    stored.into(),
                    Some(token),
                    vec![],
                    None,
                ))
                .await
                .unwrap();
            assert!(matches!(
                manager.refresh_token().await,
                Err(AuthError::AuthorizationRequired)
            ));
            let retained = manager.credential_store.load().await.unwrap().unwrap();
            assert_eq!(retained.client_id, stored);
            assert_eq!(
                retained
                    .token_response
                    .unwrap()
                    .refresh_token()
                    .unwrap()
                    .secret(),
                refresh
            );
            assert!(
                client.operations.lock().unwrap().is_empty(),
                "empty source credentials dispatched refresh"
            );
        }
    }
}
