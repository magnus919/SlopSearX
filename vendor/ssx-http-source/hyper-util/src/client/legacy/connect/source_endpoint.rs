//! Source endpoint resolution has two deliberately distinct lazy stages.
use hyper::ext::SourceRequestIdentity;
use std::{future::Future, net::SocketAddr, pin::Pin, sync::Arc};

/// Preserves the application resolver's typed stage error chain.
pub type SourceError = Box<dyn std::error::Error + Send + Sync>;
/// Owned first-stage resolution future, independent of its retirement owner.
pub type SourceResolutionFuture =
    Pin<Box<dyn Future<Output = Result<SourceResolution, SourceError>> + Send>>;
/// Synchronous per-flow ownership handle, retained outside a phase timeout.
pub struct SourceResolving {
    /// First-stage resolution; Drop cancels this lookup immediately.
    pub future: SourceResolutionFuture,
    /// Same per-flow owner used by the returned SourceResolution.
    pub owner: Arc<dyn SourceCandidateResolver>,
}
impl SourceResolving {
    /// Refuse source lookup without falling back to a DNS-only resolver.
    pub fn unsupported() -> Self {
        Self {
            future: Box::pin(async { Err(Box::new(SourceResolutionUnsupported) as SourceError) }),
            owner: Arc::new(UnsupportedScope),
        }
    }
}
struct UnsupportedScope;
impl SourceCandidateResolver for UnsupportedScope {
    fn cancel_and_wait(&self) -> SourceRetiring {
        Box::pin(async { Ok(()) })
    }
    fn resolve_candidate(&self, _: SourceName, _: SourceCandidate) -> SourceCandidateResolving {
        Box::pin(async { Err(Box::new(SourceResolutionUnsupported) as SourceError) })
    }
}
/// Owned lazy per-candidate second-stage resolution future.
pub type SourceCandidateResolving =
    Pin<Box<dyn Future<Output = Result<Vec<SocketAddr>, SourceError>> + Send>>;
/// Await original per-resolution helper retirement after cancelling losing attempts.
pub type SourceRetiring = Pin<Box<dyn Future<Output = Result<(), SourceError>> + Send>>;
/// Retains the same URL allocation and the trusted absolute DNS/TCP deadline.
#[derive(Clone, Debug)]
pub struct SourceName {
    identity: SourceRequestIdentity,
    deadline: tokio::time::Instant,
}
impl SourceName {
    /// Attach a trusted absolute deadline without changing the URL owner.
    pub fn new(identity: SourceRequestIdentity, deadline: tokio::time::Instant) -> Self {
        Self { identity, deadline }
    }
    /// Borrow the exact source host without a URI or DNS-name reparse.
    pub fn raw_host(&self) -> &str {
        self.identity.0.raw_host()
    }
    /// Borrow the effective original decimal port without a u16 conversion.
    pub fn origin_port(&self) -> Option<&str> {
        self.identity.0.origin_port()
    }
    /// Borrow the same source URL allocation retained by the request.
    pub fn identity(&self) -> &SourceRequestIdentity {
        &self.identity
    }
    /// The original DNS/TCP deadline; candidate lookups must never reset it.
    pub fn deadline(&self) -> tokio::time::Instant {
        self.deadline
    }
}
/// First-stage family used only by the outer AnyIO candidate ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFamily {
    /// IPv4 first-stage candidate.
    V4,
    /// IPv6 first-stage candidate.
    V6,
}
/// Native host text from first GAI, after AnyIO's scope-id projection.
/// First-stage sockaddr flow/port do not become a final connect sockaddr.
#[derive(Clone, Debug)]
pub struct SourceCandidate {
    /// First-stage family; it does not restrict a later lookup.
    pub family: SourceFamily,
    /// Native inet_ntop text plus AnyIO scope suffix, as supplied by the resolver.
    pub native_host_text: String,
}
/// A second lookup is created only inside a launched outer candidate attempt.
pub trait SourceCandidateResolver: Send + Sync {
    /// Cancel this resolution scope only, then join its original helper supervisors.
    /// Cancellation must start when this method is called, not only when polled.
    /// Dropped lookup futures must cancel immediately; their application runtime
    /// retains original join ownership until this wait or awaited runtime close.
    fn cancel_and_wait(&self) -> SourceRetiring;
    /// Resolve one launched candidate; returned full sockaddrs are dialled sequentially.
    fn resolve_candidate(
        &self,
        name: SourceName,
        candidate: SourceCandidate,
    ) -> SourceCandidateResolving;
}
/// First-stage candidates and their original lazy resolution owner.
pub struct SourceResolution {
    /// First-stage native order before the source family reorder.
    pub candidates: Vec<SourceCandidate>,
    /// Creates second-stage futures only when the corresponding attempt is launched.
    pub resolver: Arc<dyn SourceCandidateResolver>,
}
/// Implemented by the application-owned, cancellable native resolver adapter.
pub trait SourceEndpointResolver: Send + Sync {
    /// Resolve original endpoint semantics without converting its integer to u16.
    fn resolve_source(&self, name: SourceName) -> SourceResolving;
}
/// Resolver has no source endpoint implementation; no DNS-only fallback occurs.
#[derive(Debug)]
pub struct SourceResolutionUnsupported;
impl std::fmt::Display for SourceResolutionUnsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("source endpoint resolution is not implemented by this resolver")
    }
}
impl std::error::Error for SourceResolutionUnsupported {}
