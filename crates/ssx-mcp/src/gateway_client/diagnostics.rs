//! Explicit troubleshooting output contains only finite classifications.
use std::{error::Error, ffi::OsStr, io::ErrorKind};
const MODE: &str = "classification-v1";
#[derive(Clone, Copy, Debug)]
pub(super) enum Stage {
    NetworkStartup,
    OAuthStartup,
    OAuthCallbackPortFailed,
    OAuthCallbackBindFailed,
    OAuthCallbackBindDeadline,
    OAuthCallbackAddressFailed,
    OAuthAuthorizationUrlFailed,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthRegistrationFailed,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthRegistrationModelMissing,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthRegistrationDecodeFailed,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthRegistrationShapeFailed,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthRegistrationClientFailed,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthRegistrationEncodeFailed,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthClientPersistenceFailed,
    OAuthCallbackComplete,
    OAuthCallbackStateMissing,
    OAuthExchangeFailed,
    OAuthExchangeComplete,
    AuthAuthorizationRequired,
    AuthAuthorizationFailed,
    AuthTokenExchangeFailed,
    AuthTokenRefreshFailed,
    AuthTokenRefreshRejected,
    AuthCredentialStore,
    AuthHttp,
    AuthOAuth,
    AuthMetadata,
    AuthPkceUnsupported,
    AuthUrl,
    AuthNoAuthorizationSupport,
    AuthInternal,
    AuthInvalidTokenType,
    AuthTokenExpired,
    AuthInvalidScope,
    AuthRegistrationFailed,
    AuthInsufficientScope,
    AuthIssuerMismatch,
    AuthIssuerMissing,
    AuthClientCredentials,
    AuthOther,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthSourceClientSynced,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthSourceClientSyncFailed,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthCodeSend,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthCodeSendFailed,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthCodeStatus,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthCodeBodyComplete,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthCodeBodyFailed,
    #[cfg(feature = "gateway-source-endpoints")]
    OAuthCodeBodyLimit,
    #[cfg(feature = "gateway-source-endpoints")]
    AuthSourceOperation,
    RemoteConstruction,
    RemoteInitialization,
    RemoteHttpResponse,
    RemoteDiscovery,
    Projection,
    Stdio,
    RemoteCleanup,
    CallbackCleanup,
    BrowserCleanup,
    NetworkCleanup,
    FileCleanup,
    #[cfg(windows)]
    TokenRead,
    #[cfg(windows)]
    TokenWrite,
}
fn selected(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new(MODE))
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Classification {
    io_kind: Option<ErrorKind>,
    os_code: Option<i32>,
    http_flags: u16,
    http_status: Option<u16>,
    chain_truncated: bool,
}
fn classify(mut error: &(dyn Error + 'static)) -> Classification {
    let mut out = Classification::default();
    for depth in 0..8 {
        if let Some(io) = error.downcast_ref::<std::io::Error>()
            && out.io_kind.is_none()
        {
            out.io_kind = Some(io.kind());
            out.os_code = io.raw_os_error();
        }
        if let Some(http) = error.downcast_ref::<reqwest::Error>() {
            out.http_flags |= u16::from(http.is_builder())
                | (u16::from(http.is_connect()) << 1)
                | (u16::from(http.is_timeout()) << 2)
                | (u16::from(http.is_request()) << 3)
                | (u16::from(http.is_body()) << 4)
                | (u16::from(http.is_decode()) << 5)
                | (u16::from(http.is_redirect()) << 6)
                | (u16::from(http.is_status()) << 7);
            out.http_status = http.status().map(|status| status.as_u16());
        }
        let Some(source) = error.source() else { break };
        if depth == 7 {
            out.chain_truncated = true;
        } else {
            error = source;
        }
    }
    out
}
fn kind_label(kind: Option<ErrorKind>) -> &'static str {
    match kind {
        None => "none",
        Some(ErrorKind::NotFound) => "not_found",
        Some(ErrorKind::PermissionDenied) => "permission_denied",
        Some(ErrorKind::ConnectionRefused) => "connection_refused",
        Some(ErrorKind::ConnectionReset) => "connection_reset",
        Some(ErrorKind::ConnectionAborted) => "connection_aborted",
        Some(ErrorKind::NotConnected) => "not_connected",
        Some(ErrorKind::AddrInUse) => "addr_in_use",
        Some(ErrorKind::AddrNotAvailable) => "addr_not_available",
        Some(ErrorKind::BrokenPipe) => "broken_pipe",
        Some(ErrorKind::AlreadyExists) => "already_exists",
        Some(ErrorKind::WouldBlock) => "would_block",
        Some(ErrorKind::InvalidInput) => "invalid_input",
        Some(ErrorKind::InvalidData) => "invalid_data",
        Some(ErrorKind::TimedOut) => "timed_out",
        Some(ErrorKind::WriteZero) => "write_zero",
        Some(ErrorKind::Interrupted) => "interrupted",
        Some(ErrorKind::UnexpectedEof) => "unexpected_eof",
        Some(ErrorKind::Unsupported) => "unsupported",
        Some(ErrorKind::OutOfMemory) => "out_of_memory",
        Some(_) => "other",
    }
}
#[cfg(test)]
fn line(mode: Option<&OsStr>, stage: Stage, error: &(dyn Error + 'static)) -> Option<String> {
    selected(mode).then(|| {
        let class = classify(error);
        render(stage, &class)
    })
}
fn render(stage: Stage, class: &Classification) -> String {
    format!(
        "gateway-classification-v1 stage={stage:?} io_kind={} os_code={:?} http_flags={} http_status={:?} chain_truncated={}",
        kind_label(class.io_kind),
        class.os_code,
        class.http_flags,
        class.http_status,
        class.chain_truncated
    )
}
const MAX_EVENTS: usize = 32;
#[derive(Clone, Copy)]
struct Event {
    stage: Stage,
    class: Classification,
}
struct Events {
    events: [Option<Event>; MAX_EVENTS],
    len: usize,
    overflow: bool,
}
impl Default for Events {
    fn default() -> Self {
        Self {
            events: [None; MAX_EVENTS],
            len: 0,
            overflow: false,
        }
    }
}
#[derive(Default)]
pub(super) struct Diagnostics {
    enabled: bool,
    events: std::sync::Mutex<Events>,
}
#[derive(Default)]
pub(super) struct Emission {
    pub written: usize,
    pub failed: bool,
    pub overflowed: bool,
}
impl Diagnostics {
    pub(super) fn new(mode: Option<&OsStr>) -> Self {
        Self {
            enabled: selected(mode),
            events: Default::default(),
        }
    }
    pub(super) fn from_env() -> Self {
        Self::new(std::env::var_os("MCP_REMOTE_DIAGNOSTICS").as_deref())
    }
    fn record_class(&self, stage: Stage, class: Classification) {
        if !self.enabled {
            return;
        }
        let mut state = self
            .events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.len == MAX_EVENTS {
            state.overflow = true;
            return;
        }
        let index = state.len;
        state.events[index] = Some(Event { stage, class });
        state.len += 1;
    }
    pub(super) fn record(&self, stage: Stage, error: &(dyn Error + 'static)) {
        if self.enabled {
            self.record_class(stage, classify(error));
        }
    }
    pub(super) fn checkpoint(&self, stage: Stage) {
        self.record_class(stage, Classification::default());
    }
    pub(super) fn record_auth_failure(
        &self,
        stage: Stage,
        error: &rmcp::transport::auth::AuthError,
    ) {
        use rmcp::transport::auth::AuthError;
        // Match variants only; their strings, URLs and issuer values are never kept.
        let kind = match error {
            AuthError::AuthorizationRequired => Stage::AuthAuthorizationRequired,
            AuthError::AuthorizationFailed(_) => Stage::AuthAuthorizationFailed,
            AuthError::TokenExchangeFailed(_) => Stage::AuthTokenExchangeFailed,
            AuthError::TokenRefreshFailed(_) => Stage::AuthTokenRefreshFailed,
            AuthError::TokenRefreshRejected(_) => Stage::AuthTokenRefreshRejected,
            AuthError::CredentialStoreError(_) => Stage::AuthCredentialStore,
            AuthError::HttpError(_) => Stage::AuthHttp,
            AuthError::OAuthError(_) => Stage::AuthOAuth,
            AuthError::MetadataError(_) => Stage::AuthMetadata,
            AuthError::PkceUnsupported => Stage::AuthPkceUnsupported,
            AuthError::UrlError(_) => Stage::AuthUrl,
            AuthError::NoAuthorizationSupport => Stage::AuthNoAuthorizationSupport,
            AuthError::InternalError(_) => Stage::AuthInternal,
            AuthError::InvalidTokenType(_) => Stage::AuthInvalidTokenType,
            AuthError::TokenExpired => Stage::AuthTokenExpired,
            AuthError::InvalidScope(_) => Stage::AuthInvalidScope,
            AuthError::RegistrationFailed(_) => Stage::AuthRegistrationFailed,
            AuthError::InsufficientScope { .. } => Stage::AuthInsufficientScope,
            AuthError::AuthorizationServerMismatch { .. } => Stage::AuthIssuerMismatch,
            AuthError::AuthorizationServerMissingIssuer { .. } => Stage::AuthIssuerMissing,
            AuthError::ClientCredentialsError(_) => Stage::AuthClientCredentials,
            #[cfg(feature = "gateway-source-endpoints")]
            AuthError::SourceOperation(_) => Stage::AuthSourceOperation,
            _ => Stage::AuthOther,
        };
        self.record(stage, error);
        self.record(kind, error);
    }
    #[cfg(feature = "gateway-source-endpoints")]
    pub(super) fn record_code_status(&self, status: reqwest::StatusCode) {
        self.record_class(
            Stage::OAuthCodeStatus,
            Classification {
                http_flags: 128,
                http_status: Some(status.as_u16()),
                ..Classification::default()
            },
        );
    }
    pub(super) fn record_http_status(&self, status: reqwest::StatusCode) {
        self.record_class(
            Stage::RemoteHttpResponse,
            Classification {
                http_flags: 128,
                http_status: Some(status.as_u16()),
                ..Classification::default()
            },
        );
    }
    // Only the runtime calls this, after all owned cleanup attempts complete.
    // No lock spans sink I/O, no sink error is formatted or propagated, and the
    // per-runtime fixed array cannot grow or borrow another connection's data.
    pub(super) fn emit(&self, sink: &mut impl std::io::Write) -> Emission {
        if !self.enabled {
            return Emission::default();
        }
        let snapshot = {
            let mut state = self
                .events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::take(&mut *state)
        };
        let mut outcome = Emission {
            overflowed: snapshot.overflow,
            ..Emission::default()
        };
        for event in snapshot.events[..snapshot.len].iter().flatten() {
            let line = format!("{}\n", render(event.stage, &event.class));
            if sink.write_all(line.as_bytes()).is_err() {
                outcome.failed = true;
                break;
            }
            outcome.written += 1;
        }
        if outcome.overflowed && !outcome.failed {
            if sink
                .write_all(b"gateway-classification-v1 events_truncated=true\n")
                .is_err()
            {
                outcome.failed = true;
            } else {
                outcome.written += 1;
            }
        }
        if sink.flush().is_err() {
            outcome.failed = true;
        }
        outcome
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct SecretError;
    impl std::fmt::Debug for SecretError {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("error Debug must never run: private-secret /private/token-path");
        }
    }
    impl std::fmt::Display for SecretError {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!(
                "error Display must never run: https://user:private-secret@example.invalid/?code=private-code /private/token-path"
            );
        }
    }
    impl Error for SecretError {}
    #[test]
    fn explicit_classification_preserves_only_finite_fields_and_never_formats_secret_errors() {
        let secret = SecretError;
        assert!(line(None, Stage::Stdio, &secret).is_none());
        assert!(line(Some(OsStr::new("private-secret")), Stage::Stdio, &secret).is_none());
        let io = std::io::Error::new(ErrorKind::PermissionDenied, secret);
        let rendered = line(Some(OsStr::new(MODE)), Stage::Stdio, &io).unwrap();
        assert!(rendered.contains("stage=Stdio io_kind=permission_denied os_code=None"));
        for forbidden in [
            "private-secret",
            "private-code",
            "example.invalid",
            "token-path",
            "https://",
        ] {
            assert!(!rendered.contains(forbidden));
        }
        // Secret-bearing AuthError payloads must never be formatted or retained.
        use rmcp::transport::auth::AuthError;
        let auth_events = Diagnostics::new(Some(OsStr::new(MODE)));
        auth_events.checkpoint(Stage::OAuthCallbackComplete);
        for error in [
            AuthError::InternalError("private-secret /private/token-path".into()),
            AuthError::TokenExchangeFailed("https://example.invalid/?code=private-code".into()),
            AuthError::AuthorizationServerMismatch {
                expected_issuer: "https://user:private-secret@example.invalid".into(),
                received_issuer: "/private/token-path".into(),
            },
        ] {
            auth_events.record_auth_failure(Stage::OAuthExchangeFailed, &error);
        }
        #[cfg(feature = "gateway-source-endpoints")]
        {
            auth_events.record_auth_failure(
                Stage::OAuthExchangeFailed,
                &AuthError::SourceOperation(Box::new(std::io::Error::other(SecretError))),
            );
            auth_events.checkpoint(Stage::OAuthSourceClientSynced);
            auth_events.checkpoint(Stage::OAuthCodeSend);
            auth_events.record_code_status(reqwest::StatusCode::BAD_REQUEST);
            auth_events.checkpoint(Stage::OAuthCodeBodyComplete);
        }
        for stage in [
            Stage::OAuthCallbackPortFailed,
            Stage::OAuthCallbackBindFailed,
            Stage::OAuthCallbackBindDeadline,
            Stage::OAuthCallbackAddressFailed,
            Stage::OAuthAuthorizationUrlFailed,
        ] {
            auth_events.record(stage, &SecretError);
        }
        #[cfg(feature = "gateway-source-endpoints")]
        for stage in [
            Stage::OAuthRegistrationFailed,
            Stage::OAuthRegistrationModelMissing,
            Stage::OAuthRegistrationDecodeFailed,
            Stage::OAuthRegistrationShapeFailed,
            Stage::OAuthRegistrationClientFailed,
            Stage::OAuthRegistrationEncodeFailed,
            Stage::OAuthClientPersistenceFailed,
        ] {
            auth_events.record(stage, &SecretError);
        }
        let mut auth_output = Vec::new();
        let emission = auth_events.emit(&mut auth_output);
        assert!(!emission.failed && !emission.overflowed);
        let auth_output = String::from_utf8(auth_output).unwrap();
        for expected in [
            "OAuthCallbackComplete",
            "AuthInternal",
            "AuthTokenExchangeFailed",
            "AuthIssuerMismatch",
        ] {
            assert!(auth_output.contains(expected));
        }
        #[cfg(feature = "gateway-source-endpoints")]
        {
            assert!(auth_output.contains("stage=AuthSourceOperation "));
            assert!(auth_output.contains("stage=OAuthCodeStatus io_kind=none os_code=None http_flags=128 http_status=Some(400)"));
        }
        for stage in [
            "OAuthCallbackPortFailed",
            "OAuthCallbackBindFailed",
            "OAuthCallbackBindDeadline",
            "OAuthCallbackAddressFailed",
            "OAuthAuthorizationUrlFailed",
        ] {
            assert!(auth_output.contains(&format!("stage={stage} ")));
        }
        #[cfg(feature = "gateway-source-endpoints")]
        for stage in [
            "OAuthRegistrationFailed",
            "OAuthRegistrationModelMissing",
            "OAuthRegistrationDecodeFailed",
            "OAuthRegistrationShapeFailed",
            "OAuthRegistrationClientFailed",
            "OAuthRegistrationEncodeFailed",
            "OAuthClientPersistenceFailed",
        ] {
            assert!(auth_output.contains(&format!("stage={stage} ")));
        }
        for forbidden in [
            "private-secret",
            "private-code",
            "example.invalid",
            "token-path",
            "https://",
        ] {
            assert!(!auth_output.contains(forbidden));
        }
        struct FailingSink;
        impl std::io::Write for FailingSink {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other(SecretError))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::other(SecretError))
            }
        }
        let collector = Diagnostics::new(Some(OsStr::new(MODE)));
        for _ in 0..MAX_EVENTS + 3 {
            collector.record(Stage::Stdio, &io);
        }
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            collector.emit(&mut FailingSink)
        }));
        assert!(outcome.is_ok());
        let outcome = outcome.unwrap();
        assert!(outcome.failed && outcome.overflowed);
        assert_eq!(outcome.written, 0);
        let collector = Diagnostics::new(Some(OsStr::new(MODE)));
        for _ in 0..MAX_EVENTS + 3 {
            collector.record(Stage::Stdio, &io);
        }
        let isolated = Diagnostics::new(Some(OsStr::new(MODE)));
        let mut isolated_output = Vec::new();
        assert_eq!(isolated.emit(&mut isolated_output).written, 0);
        assert!(isolated_output.is_empty());
        let mut output = Vec::new();
        let outcome = collector.emit(&mut output);
        assert!(!outcome.failed && outcome.overflowed);
        assert_eq!(outcome.written, MAX_EVENTS + 1);
        assert_eq!(
            output.iter().filter(|byte| **byte == b'\n').count(),
            MAX_EVENTS + 1
        );
        assert!(output.len() < 16384);
        for secret in ["private-secret", "token-path", "https://"] {
            assert!(!String::from_utf8_lossy(&output).contains(secret));
        }
        let disabled = Diagnostics::default();
        disabled.record(Stage::Stdio, &io);
        assert_eq!(disabled.emit(&mut FailingSink).written, 0);
        let raw = std::io::Error::from_raw_os_error(32);
        assert_eq!(classify(&raw).os_code, Some(32));
        let http = reqwest::Client::new()
            .get("http://private-user:private-secret@[invalid/?code=private-code")
            .build()
            .unwrap_err();
        let rendered = line(Some(OsStr::new(MODE)), Stage::RemoteInitialization, &http).unwrap();
        assert!(rendered.contains("http_flags=1"));
        for forbidden in [
            "private-user",
            "private-secret",
            "private-code",
            "invalid",
            "http://",
        ] {
            assert!(!rendered.contains(forbidden));
        }
    }
}
