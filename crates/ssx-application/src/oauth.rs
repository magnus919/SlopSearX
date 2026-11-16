//! T04 consumer-owned authority port. T05 owns endpoint/protocol validation.
use crate::PortFuture;
use ssx_domain::oauth::*;
pub trait OAuthState: Send + Sync {
    /// Milliseconds from the durable authority clock; failure is never a local-clock fallback.
    fn now_ms(&self) -> PortFuture<'_, Result<u64, OAuthError>>;
    fn register_client(&self, client: ClientRegistration)
    -> PortFuture<'_, Result<(), OAuthError>>;
    fn get_client<'a>(
        &'a self,
        id: &'a str,
    ) -> PortFuture<'a, Result<ClientRegistration, OAuthError>>;
    fn create_code(&self, grant: CodeGrant) -> PortFuture<'_, Result<IssuedCode, OAuthError>>;
    fn load_code<'a>(&'a self, code: &'a Secret) -> PortFuture<'a, Result<IssuedCode, OAuthError>>;
    fn redeem_code<'a>(
        &'a self,
        code: &'a Secret,
        expected: &'a CodeGrant,
    ) -> PortFuture<'a, Result<TokenPair, OAuthError>>;
    fn load_token<'a>(
        &'a self,
        kind: TokenKind,
        token: &'a Secret,
    ) -> PortFuture<'a, Result<LoadedGrant, OAuthError>>;
    fn rotate_refresh<'a>(
        &'a self,
        token: &'a Secret,
        expected: &'a TokenGrant,
        scopes: &'a [String],
    ) -> PortFuture<'a, Result<TokenPair, OAuthError>>;
    fn revoke<'a>(
        &'a self,
        kind: TokenKind,
        token: &'a Secret,
        client_id: &'a str,
    ) -> PortFuture<'a, Result<(), OAuthError>>;
}
