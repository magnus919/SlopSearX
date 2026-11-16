//! Internal search values and pure durable codecs. Public DTOs live in ssx-wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreRead<T> {
    Found(T),
    Missing,
    Expired,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterOutcome {
    Success,
    Unavailable,
    InvalidRequest,
    InternalError,
}

pub mod records;
pub use records::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainError(pub &'static str);
impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for DomainError {}
pub mod identity;
pub use identity::*;
pub mod canonical;

pub mod storage;

impl From<ssx_json::Error> for DomainError {
    fn from(_: ssx_json::Error) -> Self {
        Self("invalid bounded JSON")
    }
}

pub mod rate;
pub mod research;

pub mod browser_identity;
mod date_proof;
pub mod oauth;
pub use date_proof::PublicationDateProof;

pub mod saved;

pub mod staged;
