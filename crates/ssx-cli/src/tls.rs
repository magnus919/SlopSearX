//! OpenSSL trust-directory/chain/hostname semantics with Rustls handshake verification.
//!
//! CPython3.13.6 source context: CERT_REQUIRED, SSL-server purpose,
//! TRUSTED_FIRST|PARTIAL_CHAIN|X509_STRICT, NO_PARTIAL_WILDCARDS, CN fallback.
//! OpenSSL owns hashed-directory lookup, path lists, expiry, constraints and critical
//! extensions. Rustls owns TLS and verifies every TLS12/TLS13 handshake signature.
//! Default paths are deliberate portable OS paths, never vendored build paths;
//! FILE and DIR environment overrides replace only their respective defaults.
use crate::Failure;
use openssl::{
    stack::Stack,
    x509::{
        X509, X509PurposeId, X509StoreContext,
        store::X509StoreBuilder,
        verify::{X509CheckFlags, X509VerifyFlags, X509VerifyParam},
    },
};
use rustls::{
    DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use std::{path::PathBuf, sync::Arc};
struct Verify {
    provider: Arc<rustls::crypto::CryptoProvider>,
}
impl std::fmt::Debug for Verify {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OpenSSL source-compatible X509 verifier")
    }
}
fn default_file() -> Option<PathBuf> {
    // This source-only client intentionally uses OS OpenSSL bundles, never a
    // Python installation's private certifi path or a baked Cargo build path.
    [
        "/etc/ssl/cert.pem",
        "/etc/ssl/certs/ca-certificates.crt",
        "/etc/pki/tls/certs/ca-bundle.crt",
        "/etc/ssl/ca-bundle.pem",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
}
fn default_dir() -> Option<PathBuf> {
    ["/etc/ssl/certs", "/etc/pki/tls/certs"]
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.is_dir())
}
fn store() -> Result<X509StoreBuilder, Failure> {
    let mut store = X509StoreBuilder::new().map_err(|_| Failure::Trust)?;
    store
        .set_flags(
            X509VerifyFlags::TRUSTED_FIRST
                | X509VerifyFlags::PARTIAL_CHAIN
                | X509VerifyFlags::X509_STRICT,
        )
        .map_err(|_| Failure::Trust)?;
    store
        .set_purpose(X509PurposeId::SSL_SERVER)
        .map_err(|_| Failure::Trust)?;
    let file = std::env::var_os("SSL_CERT_FILE")
        .map(PathBuf::from)
        .or_else(default_file);
    if let Some(file) = file.filter(|p| !p.as_os_str().is_empty()) {
        // OpenSSL default-path loading ignores absent/bad default files; the
        // independently configured DIR remains usable. No fallback on override.
        let _ = ssx_cli_openssl_store::load_source_file(&mut store, file.as_os_str());
    }
    let directory = std::env::var_os("SSL_CERT_DIR")
        .map(PathBuf::from)
        .or_else(default_dir);
    if let Some(directory) = directory.filter(|p| !p.as_os_str().is_empty()) {
        // OpenSSL owns raw hash_dir path-list and complete store-URI lookup.
        let _ = ssx_cli_openssl_store::add_source_hash_directory(&mut store, directory.as_os_str());
        let _ = ssx_cli_openssl_store::add_source_directory_os(&mut store, directory.as_os_str());
    }
    Ok(store)
}
fn invalid() -> rustls::Error {
    rustls::Error::General("certificate verification failed".into())
}
impl ServerCertVerifier for Verify {
    fn verify_server_cert(
        &self,
        leaf: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        name: &ServerName<'_>,
        _ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        #[cfg(feature = "source-transport-test-fixtures")]
        crate::diagnostics::record(crate::diagnostics::Stage::VerificationEntered);
        let leaf = X509::from_der(leaf.as_ref()).map_err(|_| invalid())?;
        let mut chain = Stack::new().map_err(|_| invalid())?;
        for cert in intermediates {
            chain
                .push(X509::from_der(cert.as_ref()).map_err(|_| invalid())?)
                .map_err(|_| invalid())?;
        }
        let mut params = X509VerifyParam::new().map_err(|_| invalid())?;
        params.set_hostflags(X509CheckFlags::NO_PARTIAL_WILDCARDS);
        params
            .set_purpose(X509PurposeId::SSL_SERVER)
            .map_err(|_| invalid())?;
        params
            .set_flags(
                X509VerifyFlags::TRUSTED_FIRST
                    | X509VerifyFlags::PARTIAL_CHAIN
                    | X509VerifyFlags::X509_STRICT,
            )
            .map_err(|_| invalid())?;
        params.set_auth_level(2);
        params.set_time(now.as_secs().try_into().map_err(|_| invalid())?);
        let host = name.to_str();
        if let Ok(ip) = host.parse() {
            params.set_ip(ip).map_err(|_| invalid())?;
        } else {
            params.set_host(&host).map_err(|_| invalid())?;
        }
        let mut store = store().map_err(|_| invalid())?;
        store.set_param(&params).map_err(|_| invalid())?;
        let store = store.build();
        let mut context = X509StoreContext::new().map_err(|_| invalid())?;
        let (verified, _code, _depth) = context
            .init(&store, &leaf, &chain, |ctx| {
                let verified = ctx.verify_cert()?;
                Ok((verified, ctx.error().as_raw(), ctx.error_depth()))
            })
            .map_err(|_| invalid())?;
        #[cfg(feature = "source-transport-test-fixtures")]
        crate::diagnostics::record(crate::diagnostics::Stage::Certificate {
            valid: verified,
            code: _code,
            depth: _depth,
        });
        if verified {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(invalid())
        }
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        let result = rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        );
        #[cfg(feature = "source-transport-test-fixtures")]
        crate::diagnostics::record(crate::diagnostics::Stage::Signature {
            tls: 12,
            valid: result.is_ok(),
        });
        result
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        let result = rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        );
        #[cfg(feature = "source-transport-test-fixtures")]
        crate::diagnostics::record(crate::diagnostics::Stage::Signature {
            tls: 13,
            valid: result.is_ok(),
        });
        result
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
pub fn config() -> Result<rustls::ClientConfig, Failure> {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    #[cfg(windows)]
    if std::env::var_os("SSL_CERT_FILE").is_none() && std::env::var_os("SSL_CERT_DIR").is_none() {
        let verifier = rustls_platform_verifier::Verifier::new(provider.clone())
            .map_err(|_| Failure::Trust)?;
        return Ok(rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|_| Failure::Trust)?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_no_client_auth());
    }
    let verifier = Arc::new(Verify {
        provider: provider.clone(),
    });
    Ok(rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| Failure::Trust)?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth())
}
