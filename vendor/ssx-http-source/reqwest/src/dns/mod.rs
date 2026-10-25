//! DNS resolution

pub use resolve::{Addrs, Name, Resolve, Resolving};
pub(crate) use resolve::{DnsResolverWithOverrides, DynResolver};

#[cfg(docsrs)]
pub use resolve::IntoResolve;

pub(crate) mod gai;
#[cfg(feature = "hickory-dns")]
pub(crate) mod hickory;
pub(crate) mod resolve;

#[cfg(feature = "ssx-http1-source-pool")]
pub use resolve::{
    SourceCandidate, SourceCandidateResolver, SourceCandidateResolving, SourceFamily, SourceName,
    SourceResolution, SourceResolutionFuture, SourceResolutionUnsupported, SourceResolving,
    SourceRetiring,
};
