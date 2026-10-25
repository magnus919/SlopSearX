//! Opt-in HTTP/1 identity supplied by the immutable source URL owner.
use std::{fmt, sync::Arc};

/// Narrow read-only contract: implementations retain the actual original owner.
/// Returned strings are projections, never recovered from a normalized `Uri`.
pub trait SourceIdentity: fmt::Debug + Send + Sync + 'static {
    /// Source-normalized scheme, without a URI projection.
    fn scheme(&self) -> &str;
    /// Exact source host bytes represented as text; IPv6 has no brackets.
    fn raw_host(&self) -> &str;
    /// Effective original decimal origin port; zero uses the source scheme default.
    fn origin_port(&self) -> Option<&str>;
    /// Optional source URL port, preserving explicit zero separately from origin port.
    fn explicit_port(&self) -> Option<&str>;
    /// HTTP Host value: source netloc including IPv6 brackets and optional port.
    fn authority(&self) -> String;
    /// Source path and optional query delimiter, excluding credentials and fragment.
    fn origin_target(&self) -> String;
}
/// Same allocation as the caller's SourceUrl, carried through every attempt.
#[derive(Clone)]
pub struct SourceRequestIdentity(
    #[doc = "The original immutable source URL allocation."] pub Arc<dyn SourceIdentity>,
);
impl fmt::Debug for SourceRequestIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceRequestIdentity")
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Copy, Debug)]
/// Source protocol serialization selected after a concrete connection is assigned.
pub enum SourceTargetForm {
    /// Direct HTTP request target.
    Origin,
    /// HTTP proxy target using HTTPcore raw-host serialization.
    Absolute,
    /// CONNECT raw host and effective origin port.
    Connect,
}
/// Exact source request-target. The HTTP/1 encoder checks ASCII/control safety
/// when writing it; no http::Uri authority parser changes source admissibility.
#[derive(Clone)]
pub struct SourceRequestTarget {
    /// Same immutable allocation retained by the request.
    pub identity: SourceRequestIdentity,
    /// Actual wire form, independent of the carrier URI.
    pub form: SourceTargetForm,
}
impl SourceRequestTarget {
    /// Derive exact source bytes and reject whitespace/control injection.
    pub fn bytes(&self) -> Result<Vec<u8>, SourceTargetError> {
        let owner = &self.identity.0;
        let target = match self.form {
            SourceTargetForm::Origin => owner.origin_target(),
            SourceTargetForm::Absolute => {
                // httpcore URL.__bytes__ uses raw host without IPv6 brackets.
                let port = owner
                    .explicit_port()
                    .map(|port| format!(":{port}"))
                    .unwrap_or_default();
                format!(
                    "{}://{}{}{}",
                    owner.scheme(),
                    owner.raw_host(),
                    port,
                    owner.origin_target()
                )
            }
            SourceTargetForm::Connect => format!(
                "{}:{}",
                owner.raw_host(),
                owner.origin_port().ok_or(SourceTargetError)?
            ),
        };
        if target.is_empty() || !target.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
            return Err(SourceTargetError);
        }
        Ok(target.into_bytes())
    }
}
impl fmt::Debug for SourceRequestTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceRequestTarget")
            .field("form", &self.form)
            .finish_non_exhaustive()
    }
}
/// Source wire target is empty, non-ASCII or contains control/space bytes.
#[derive(Clone, Copy, Debug)]
pub struct SourceTargetError;
impl fmt::Display for SourceTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid source HTTP/1 request target")
    }
}
impl std::error::Error for SourceTargetError {}
