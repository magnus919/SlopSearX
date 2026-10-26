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
